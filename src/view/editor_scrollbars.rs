//! Text editor scrollbar rendering.

use crate::model::{AppModel, Document, EditorState};

use super::frame::Frame;
use super::geometry;
use super::scrollbar::{render_scrollbar, ScrollbarColors, ScrollbarGeometry, ScrollbarState};

/// Render vertical (and horizontal if needed) scrollbars for a text editor pane.
pub fn render_editor_scrollbars(
    frame: &mut Frame,
    model: &AppModel,
    editor: &EditorState,
    document: &Document,
    layout: &geometry::GroupLayout,
) {
    let sw = model.metrics.scrollbar_width;
    let colors = ScrollbarColors {
        track: model.theme.scrollbar.track.to_argb_u32(),
        thumb: model.theme.scrollbar.thumb.to_argb_u32(),
        thumb_hover: model.theme.scrollbar.thumb_hover.to_argb_u32(),
    };

    let viewport = &editor.viewport;
    let line_count = document.line_count();
    let visible_lines = layout.visible_lines(model.line_height);
    let visible_columns = layout.visible_columns(model.char_width);

    if let Some(v_track) = layout.v_scrollbar_rect(sw) {
        let v_state = ScrollbarState::new(line_count, visible_lines, viewport.top_line);
        let v_geo = ScrollbarGeometry::vertical(v_track, &v_state);
        render_scrollbar(frame, &v_geo, false, &colors);
    }

    if let Some(h_track) = layout.h_scrollbar_rect(sw) {
        let top = viewport.top_line;
        let bottom = (top + visible_lines).min(line_count);
        let max_len = (top..bottom)
            .map(|i| document.line_length(i))
            .max()
            .unwrap_or(0);
        let h_state = ScrollbarState::new(max_len, visible_columns, viewport.left_column);
        if h_state.needs_scroll() {
            let h_geo = ScrollbarGeometry::horizontal(h_track, &h_state);
            render_scrollbar(frame, &h_geo, false, &colors);
        }
    }
}
