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
            let editor = model.editor_area.editors.get_mut(&editor_id)?;
            let state = editor.view_mode.as_image_mut()?;

            // Compute the image-space point under the cursor before zoom
            let img_x = state.offset_x + mouse_x / state.scale;
            let img_y = state.offset_y + mouse_y / state.scale;

            // Apply zoom
            let factor = 1.0 + delta * ZOOM_FACTOR;
            let new_scale = (state.scale * factor).clamp(MIN_SCALE, MAX_SCALE);
            state.scale = new_scale;

            // Adjust offset so the cursor-point stays stationary
            state.offset_x = img_x - mouse_x / new_scale;
            state.offset_y = img_y - mouse_y / new_scale;

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
            let vh = (group.rect.height as usize)
                .saturating_sub(model.metrics.tab_bar_height) as u32;

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
            let editor = model.editor_area.editors.get_mut(&editor_id)?;
            let state = editor.view_mode.as_image_mut()?;
            state.last_mouse_x = x;
            state.last_mouse_y = y;
            None
        }

        ImageMsg::ViewportResized { width, height } => {
            let editor = model.editor_area.editors.get_mut(&editor_id)?;
            let state = editor.view_mode.as_image_mut()?;
            if !state.user_zoomed {
                state.scale = crate::image::ImageState::compute_fit_scale(
                    state.width,
                    state.height,
                    width,
                    height,
                );
                state.offset_x = 0.0;
                state.offset_y = 0.0;
            }
            Some(Cmd::redraw_editor())
        }
    }
}
