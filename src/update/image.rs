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
            let center_x = if scaled_w < area_w { (area_w - scaled_w) / 2.0 } else { 0.0 };
            let center_y = if scaled_h < area_h { (area_h - scaled_h) / 2.0 } else { 0.0 };

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
            let new_center_x = if new_scaled_w < area_w { (area_w - new_scaled_w) / 2.0 } else { 0.0 };
            let new_center_y = if new_scaled_h < area_h { (area_h - new_scaled_h) / 2.0 } else { 0.0 };

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
            // Store raw window coords (zoom handler does its own conversion)
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
