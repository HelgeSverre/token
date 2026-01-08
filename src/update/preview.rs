//! Markdown preview update handler

use crate::commands::Cmd;
use crate::messages::PreviewMsg;
use crate::model::AppModel;

pub fn update_preview(model: &mut AppModel, msg: PreviewMsg) -> Option<Cmd> {
    let group_id = model.editor_area.focused_group_id;

    match msg {
        PreviewMsg::Toggle => {
            let opened = model.editor_area.toggle_focused_preview();
            tracing::info!(
                "Markdown preview toggled: {}",
                if opened { "opened" } else { "closed" }
            );
            Some(Cmd::Redraw)
        }
        PreviewMsg::Open => {
            model.editor_area.open_preview_for_focused_group();
            tracing::info!("Markdown preview opened");
            Some(Cmd::Redraw)
        }
        PreviewMsg::Close => {
            if let Some(preview_id) = model.editor_area.find_preview_for_group(group_id) {
                model.editor_area.close_preview(preview_id);
                tracing::info!("Markdown preview closed");
                Some(Cmd::Redraw)
            } else {
                None
            }
        }
        PreviewMsg::Refresh => Some(Cmd::Redraw),
        PreviewMsg::ScrollToLine(line) => {
            if let Some(preview) = model.editor_area.preview_for_group_mut(group_id) {
                if preview.scroll_sync_enabled {
                    preview.scroll_offset = line;
                    return Some(Cmd::Redraw);
                }
            }
            None
        }
        PreviewMsg::SyncFromPreview(line) => {
            if let Some(preview_id) = model.editor_area.find_preview_for_group(group_id) {
                if let Some(preview) = model.editor_area.preview(preview_id) {
                    if preview.scroll_sync_enabled {
                        model.editor_mut().viewport.top_line = line;
                        return Some(Cmd::Redraw);
                    }
                }
            }
            None
        }
        PreviewMsg::ToggleSync => {
            if let Some(preview) = model.editor_area.preview_for_group_mut(group_id) {
                preview.scroll_sync_enabled = !preview.scroll_sync_enabled;
                tracing::info!(
                    "Preview scroll sync: {}",
                    if preview.scroll_sync_enabled {
                        "enabled"
                    } else {
                        "disabled"
                    }
                );
                return Some(Cmd::Redraw);
            }
            None
        }
    }
}
