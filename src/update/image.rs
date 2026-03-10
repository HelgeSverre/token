//! Image viewer update handlers
//!
//! Processes ImageMsg messages to update pan/zoom state.

use crate::commands::Cmd;
use crate::messages::ImageMsg;
use crate::model::AppModel;

/// Minimum zoom level (10%)
const MIN_SCALE: f64 = 0.1;
/// Maximum zoom level (1000%)
const MAX_SCALE: f64 = 10.0;
/// Zoom sensitivity per scroll tick
const ZOOM_FACTOR: f64 = 0.1;

pub fn update_image(model: &mut AppModel, msg: ImageMsg) -> Option<Cmd> {
    let editor_id = model.editor_area.focused_editor_id()?;

    match msg {
        ImageMsg::Zoom {
            delta,
            mouse_x,
            mouse_y,
        } => {
            // Get content area origin (group rect + tab bar offset)
            let group_id = model.editor_area.focused_group_id;
            let group = model.editor_area.groups.get(&group_id)?;
            let area_x = group.rect.x as f64;
            let area_y = group.rect.y as f64 + model.metrics.tab_bar_height as f64;

            let area_w = group.rect.width as f64;
            let area_h = group.rect.height as f64 - model.metrics.tab_bar_height as f64;

            let editor = model.editor_area.editors.get_mut(&editor_id)?;
            let state = editor.view_mode.as_image_mut()?;

            // For keyboard zoom (0,0), anchor at last known mouse position
            let raw_x = if mouse_x == 0.0 && mouse_y == 0.0 {
                state.last_mouse_x
            } else {
                mouse_x
            };
            let raw_y = if mouse_x == 0.0 && mouse_y == 0.0 {
                state.last_mouse_y
            } else {
                mouse_y
            };

            // Convert window coords to content-area-local coords
            let local_x = raw_x - area_x;
            let local_y = raw_y - area_y;

            // Account for centering offset (matches render.rs logic)
            let scaled_w = state.width as f64 * state.scale;
            let scaled_h = state.height as f64 * state.scale;
            let center_x = if scaled_w < area_w {
                (area_w - scaled_w) / 2.0
            } else {
                0.0
            };
            let center_y = if scaled_h < area_h {
                (area_h - scaled_h) / 2.0
            } else {
                0.0
            };

            let anchor_x = local_x - center_x;
            let anchor_y = local_y - center_y;

            // Compute the image-space point under the cursor before zoom
            let img_x = state.offset_x + anchor_x / state.scale;
            let img_y = state.offset_y + anchor_y / state.scale;

            // Apply zoom
            let factor = 1.0 + delta * ZOOM_FACTOR;
            let new_scale = (state.scale * factor).clamp(MIN_SCALE, MAX_SCALE);
            state.scale = new_scale;

            // Recompute centering for the new scale
            let new_scaled_w = state.width as f64 * new_scale;
            let new_scaled_h = state.height as f64 * new_scale;
            let new_center_x = if new_scaled_w < area_w {
                (area_w - new_scaled_w) / 2.0
            } else {
                0.0
            };
            let new_center_y = if new_scaled_h < area_h {
                (area_h - new_scaled_h) / 2.0
            } else {
                0.0
            };

            // Adjust offset so the anchor point stays stationary
            let new_anchor_x = local_x - new_center_x;
            let new_anchor_y = local_y - new_center_y;
            state.offset_x = img_x - new_anchor_x / new_scale;
            state.offset_y = img_y - new_anchor_y / new_scale;

            state.user_zoomed = true;
            Some(Cmd::redraw_editor())
        }

        ImageMsg::StartPan { x, y } => {
            let editor = model.editor_area.editors.get_mut(&editor_id)?;
            let state = editor.view_mode.as_image_mut()?;

            state.drag = Some(crate::image::DragState {
                start_mouse_x: x,
                start_mouse_y: y,
                start_offset_x: state.offset_x,
                start_offset_y: state.offset_y,
            });
            Some(Cmd::redraw_editor())
        }

        ImageMsg::UpdatePan { x, y } => {
            let editor = model.editor_area.editors.get_mut(&editor_id)?;
            let state = editor.view_mode.as_image_mut()?;

            if let Some(drag) = &state.drag {
                let dx = (x - drag.start_mouse_x) / state.scale;
                let dy = (y - drag.start_mouse_y) / state.scale;
                state.offset_x = drag.start_offset_x - dx;
                state.offset_y = drag.start_offset_y - dy;
            }
            Some(Cmd::redraw_editor())
        }

        ImageMsg::EndPan => {
            let editor = model.editor_area.editors.get_mut(&editor_id)?;
            let state = editor.view_mode.as_image_mut()?;
            state.drag = None;
            Some(Cmd::redraw_editor())
        }

        ImageMsg::FitToWindow => {
            let group_id = model.editor_area.focused_group_id;
            let group = model.editor_area.groups.get(&group_id)?;
            let vw = group.rect.width as u32;
            let vh =
                (group.rect.height as usize).saturating_sub(model.metrics.tab_bar_height) as u32;

            let editor = model.editor_area.editors.get_mut(&editor_id)?;
            let state = editor.view_mode.as_image_mut()?;
            state.scale =
                crate::image::ImageState::compute_fit_scale(state.width, state.height, vw, vh);
            state.offset_x = 0.0;
            state.offset_y = 0.0;
            state.user_zoomed = false;
            Some(Cmd::redraw_editor())
        }

        ImageMsg::ActualSize => {
            let editor = model.editor_area.editors.get_mut(&editor_id)?;
            let state = editor.view_mode.as_image_mut()?;
            state.scale = 1.0;
            state.offset_x = 0.0;
            state.offset_y = 0.0;
            state.user_zoomed = true;
            Some(Cmd::redraw_editor())
        }

        ImageMsg::MouseMove { x, y } => {
            // Store raw window coords (zoom handler does its own conversion)
            let editor = model.editor_area.editors.get_mut(&editor_id)?;
            let state = editor.view_mode.as_image_mut()?;
            state.last_mouse_x = x;
            state.last_mouse_y = y;
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::update_image;
    use crate::commands::Cmd;
    use crate::image::ImageState;
    use crate::messages::ImageMsg;
    use crate::model::{AppModel, Rect, ViewMode};

    fn make_image_model(
        group_x: f32,
        group_y: f32,
        content_width: u32,
        content_height: u32,
        image_width: u32,
        image_height: u32,
    ) -> AppModel {
        let mut model = AppModel::new(content_width, content_height, 1.0, vec![]);
        let tab_bar_height = model.metrics.tab_bar_height as f32;
        let group_id = model.editor_area.focused_group_id;
        model.editor_area.groups.get_mut(&group_id).unwrap().rect = Rect::new(
            group_x,
            group_y,
            content_width as f32,
            content_height as f32 + tab_bar_height,
        );

        let pixels = vec![0; (image_width * image_height * 4) as usize];
        model.editor_mut().view_mode = ViewMode::Image(Box::new(ImageState::new(
            pixels,
            image_width,
            image_height,
            0,
            "PNG".into(),
            content_width,
            content_height,
        )));

        model
    }

    fn focused_image(model: &AppModel) -> &ImageState {
        model
            .editor_area
            .focused_editor()
            .unwrap()
            .view_mode
            .as_image()
            .unwrap()
    }

    #[test]
    fn zoom_from_center_of_centered_image_keeps_offsets_stable() {
        let mut model = make_image_model(0.0, 0.0, 800, 600, 100, 100);
        let mouse_x = 400.0;
        let mouse_y = model.metrics.tab_bar_height as f64 + 300.0;

        let cmd = update_image(
            &mut model,
            ImageMsg::Zoom {
                delta: 1.0,
                mouse_x,
                mouse_y,
            },
        );

        assert!(cmd.as_ref().is_some_and(Cmd::needs_redraw));

        let image = focused_image(&model);
        assert!((image.scale - 1.1).abs() < 1e-9);
        assert!(image.offset_x.abs() < 1e-9);
        assert!(image.offset_y.abs() < 1e-9);
        assert!(image.user_zoomed);
    }

    #[test]
    fn keyboard_zoom_uses_last_window_mouse_position() {
        let group_x = 100.0;
        let group_y = 20.0;
        let mut explicit = make_image_model(group_x, group_y, 800, 600, 100, 100);
        let tab_bar_height = explicit.metrics.tab_bar_height as f64;
        let mouse_x = group_x as f64 + 400.0;
        let mouse_y = group_y as f64 + tab_bar_height + 300.0;

        update_image(
            &mut explicit,
            ImageMsg::Zoom {
                delta: 1.0,
                mouse_x,
                mouse_y,
            },
        );

        let explicit_state = focused_image(&explicit).clone();

        let mut keyboard = make_image_model(group_x, group_y, 800, 600, 100, 100);
        assert!(update_image(
            &mut keyboard,
            ImageMsg::MouseMove {
                x: mouse_x,
                y: mouse_y,
            },
        )
        .is_none());

        let zoom_cmd = update_image(
            &mut keyboard,
            ImageMsg::Zoom {
                delta: 1.0,
                mouse_x: 0.0,
                mouse_y: 0.0,
            },
        );
        assert!(zoom_cmd.as_ref().is_some_and(Cmd::needs_redraw));

        let keyboard_state = focused_image(&keyboard);
        assert!((keyboard_state.scale - explicit_state.scale).abs() < 1e-9);
        assert!((keyboard_state.offset_x - explicit_state.offset_x).abs() < 1e-9);
        assert!((keyboard_state.offset_y - explicit_state.offset_y).abs() < 1e-9);
    }
}
