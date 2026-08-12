//! Text editor scrollbar rendering.

use crate::model::{AppModel, Document, EditorState, Mark};

use super::frame::Frame;
use super::geometry;
use super::scrollbar::{
    render_scrollbar, track_row_for_position, ScrollbarColors, ScrollbarGeometry, ScrollbarState,
};

/// Render one overview tick per track pixel row that has a mark, highest
/// priority winning where multiple lines collapse onto the same row. Fed
/// find-match ticks (`Mark::Match`) from `view::mod::find_match_ticks` today
/// (see find-enhancements.md); future producers (LSP diagnostics, ...) pass
/// their own ticks the same way.
fn render_overview_marks(
    frame: &mut Frame,
    track: crate::model::Rect,
    total_lines: usize,
    ticks: impl IntoIterator<Item = (usize, Mark)>,
    color_for: impl Fn(Mark) -> u32,
) {
    if total_lines == 0 || track.height <= 0.0 {
        return;
    }

    let mut rows: std::collections::BTreeMap<usize, Mark> = std::collections::BTreeMap::new();
    for (line, mark) in ticks {
        let row = track_row_for_position(track.height, total_lines, line);
        rows.entry(row)
            .and_modify(|existing| {
                if mark > *existing {
                    *existing = mark;
                }
            })
            .or_insert(mark);
    }

    let x = track.x.round() as usize;
    let w = track.width.round() as usize;
    let y0 = track.y.round() as usize;
    for (row, mark) in rows {
        frame.fill_rect_px(x, y0 + row, w, 1, color_for(mark));
    }
}

fn overview_mark_color(model: &AppModel, mark: Mark) -> u32 {
    let overlay = &model.theme.overlay;
    match mark {
        Mark::Match => model.theme.editor.bracket_match_background.to_argb_u32(),
        Mark::Bookmark => overlay.severity_hint.to_argb_u32(),
        Mark::Info => overlay.severity_info.to_argb_u32(),
        Mark::Warning => overlay.severity_warning.to_argb_u32(),
        Mark::Error => overlay.severity_error.to_argb_u32(),
        Mark::Breakpoint => model.theme.editor.cursor_color.to_argb_u32(),
    }
}

/// Render vertical (and horizontal if needed) scrollbars for a text editor
/// pane. `ticks` are (line, Mark) pairs for the overview lane — empty until
/// a producer (find-enhancements, LSP diagnostics, ...) supplies real data.
pub fn render_editor_scrollbars(
    frame: &mut Frame,
    model: &AppModel,
    editor: &EditorState,
    document: &Document,
    layout: &geometry::GroupLayout,
    ticks: &[(usize, Mark)],
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

        // Overview marks must not appear on documents that fit the
        // viewport — same needs_scroll guard the horizontal bar already has.
        if v_state.needs_scroll() {
            render_overview_marks(frame, v_track, line_count, ticks.iter().copied(), |mark| {
                overview_mark_color(model, mark)
            });
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Rect;

    fn make_frame(width: usize, height: usize) -> (Vec<u32>, usize, usize) {
        (vec![0u32; width * height], width, height)
    }

    #[test]
    fn overview_marks_draw_one_tick_per_row_highest_priority_wins() {
        let (mut buf, w, h) = make_frame(20, 100);
        let mut frame = Frame::new(&mut buf, w, h);
        let track = Rect::new(0.0, 0.0, 12.0, 100.0);

        // Two marks land on the same track row (lines 49/50 of 100 map to
        // roughly the same pixel row); Error must win over Info.
        render_overview_marks(
            &mut frame,
            track,
            100,
            [(49, Mark::Info), (50, Mark::Error)],
            |mark| match mark {
                Mark::Error => 0xFFFF0000,
                Mark::Info => 0xFF00FF00,
                _ => 0xFF000000,
            },
        );

        let row = track_row_for_position(track.height, 100, 50);
        assert_eq!(frame.get_pixel(0, row), 0xFFFF0000);
    }

    #[test]
    fn overview_marks_empty_ticks_draw_nothing() {
        let (mut buf, w, h) = make_frame(20, 100);
        let mut frame = Frame::new(&mut buf, w, h);
        let track = Rect::new(0.0, 0.0, 12.0, 100.0);

        render_overview_marks(&mut frame, track, 100, std::iter::empty(), |_| 0xFFFF0000);

        assert!(buf.iter().all(|&px| px == 0));
    }

    #[test]
    fn overview_marks_zero_total_lines_does_not_panic() {
        let (mut buf, w, h) = make_frame(20, 100);
        let mut frame = Frame::new(&mut buf, w, h);
        let track = Rect::new(0.0, 0.0, 12.0, 100.0);

        render_overview_marks(&mut frame, track, 0, [(0, Mark::Error)], |_| 0xFFFF0000);
    }
}
