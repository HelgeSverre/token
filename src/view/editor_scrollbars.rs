//! Text editor scrollbar rendering.

use std::sync::Arc;

use crate::model::{
    ui::FindResults, AppModel, Document, EditorState, Mark, ModalState, OverviewProjection,
};

use super::frame::Frame;
use super::geometry;
use super::scrollbar::{
    render_scrollbar, track_row_for_position, ScrollbarColors, ScrollbarGeometry, ScrollbarState,
};

/// Reduce all producers through the same bounded, pixel-row projection.
fn project_overview_marks(
    track_height: f32,
    total_lines: usize,
    ticks: impl IntoIterator<Item = (usize, Mark)>,
) -> Arc<[Option<Mark>]> {
    if total_lines == 0 || !track_height.is_finite() || track_height <= 0.0 {
        return Arc::from([]);
    }
    let last_row = track_row_for_position(track_height, total_lines, total_lines - 1);
    let mut rows = vec![None; last_row + 1];
    for (line, mark) in ticks {
        let row = track_row_for_position(track_height, total_lines, line);
        rows[row] = rows[row].max(Some(mark));
    }
    rows.into()
}

fn same_identity<T>(a: &Option<Arc<T>>, b: &Option<Arc<T>>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => Arc::ptr_eq(a, b),
        (None, None) => true,
        _ => false,
    }
}

fn overview_rows(
    editor: &EditorState,
    document: &Document,
    find: Option<Arc<FindResults>>,
    track_height: f32,
) -> Arc<[Option<Mark>]> {
    let map = editor.viewport_map(document);
    let total_rows = map.row_count();
    let wrap_identity = (editor.soft_wrap && editor.is_plain_text_mode())
        .then(|| editor.wrap_cache.layout_identity())
        .flatten();
    let mut cache = editor.overview_cache.0.borrow_mut();
    if let Some(cached) = cache.as_ref() {
        if cached.revision == document.revision
            && cached.buffer.is_instance(&document.buffer)
            && same_identity(&cached.wrap_identity, &wrap_identity)
            && same_identity(&cached.find, &find)
            && cached.total_rows == total_rows
            && cached.track_height == track_height
            && cached.diagnostics.len() == document.diagnostics.len()
            && cached.diagnostics.iter().zip(&document.diagnostics).all(
                |((range, mark), diagnostic)| {
                    *range == diagnostic.range
                        && *mark == crate::model::diagnostic_mark(diagnostic.severity)
                },
            )
        {
            return Arc::clone(&cached.rows);
        }
    }
    let find_ticks = find
        .iter()
        .flat_map(|results| results.lines().iter().copied())
        .map(|line| (line, Mark::Match));
    let ticks = find_ticks
        .chain(super::diagnostic_ticks(document))
        .map(|(line, mark)| (map.visual_line_for_position(line, 0), mark));
    let rows = project_overview_marks(track_height, total_rows, ticks);
    *cache = Some(OverviewProjection {
        buffer: document.buffer.clone(),
        revision: document.revision,
        wrap_identity,
        find,
        diagnostics: document
            .diagnostics
            .iter()
            .map(|d| (d.range, crate::model::diagnostic_mark(d.severity)))
            .collect(),
        total_rows,
        track_height,
        rows: Arc::clone(&rows),
    });
    rows
}

fn render_overview_rows(
    frame: &mut Frame,
    track: crate::model::Rect,
    rows: &[Option<Mark>],
    color_for: impl Fn(Mark) -> u32,
) {
    let x = track.x.round() as usize;
    let w = track.width.round() as usize;
    let y0 = track.y.round() as usize;
    for (row, mark) in rows.iter().enumerate() {
        if let Some(mark) = mark {
            frame.fill_rect_px(x, y0 + row, w, 1, color_for(*mark));
        }
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

/// Physical-pixel extents shared by scrollbar painting and hit testing.
pub(super) fn scrollbar_states(
    model: &AppModel,
    editor: &EditorState,
    document: &Document,
    layout: &geometry::GroupLayout,
) -> (ScrollbarState, ScrollbarState) {
    let viewport = &editor.viewport;
    let vertical = ScrollbarState::new(
        editor
            .viewport_map(document)
            .row_count()
            .saturating_mul(model.line_height),
        layout.content_h(),
        (viewport.top_line as f64 * model.line_height as f64 + viewport.pixels.y.offset).round()
            as usize,
    );
    let horizontal = ScrollbarState::new(
        (editor.scrollable_columns(document) as f64 * model.char_width as f64).ceil() as usize,
        (layout.rect_x() + layout.rect_w()).saturating_sub(layout.text_start_x),
        (viewport.left_column as f64 * model.char_width as f64 + viewport.pixels.x.offset).round()
            as usize,
    );
    (vertical, horizontal)
}

/// Paint scrollbars using the same pixel extents used by pointer hit testing.
pub fn render_editor_scrollbars(
    frame: &mut Frame,
    model: &AppModel,
    editor: &EditorState,
    document: &Document,
    layout: &geometry::GroupLayout,
    is_focused: bool,
) {
    let sw = model.metrics.scrollbar_width;
    let colors = ScrollbarColors::from(&model.theme.scrollbar);

    let (v_state, h_state) = scrollbar_states(model, editor, document, layout);

    if let Some(v_track) = layout.v_scrollbar_rect(sw) {
        let v_geo = ScrollbarGeometry::vertical(v_track, &v_state);
        render_scrollbar(frame, &v_geo, false, &colors);

        // Overview marks must not appear on documents that fit the
        // viewport — same needs_scroll guard the horizontal bar already has.
        if v_state.needs_scroll() {
            let find = match &model.ui.active_modal {
                Some(ModalState::FindReplace(state)) if is_focused => {
                    state.display_results(document)
                }
                _ => None,
            };
            let rows = overview_rows(editor, document, find, v_track.height);
            render_overview_rows(frame, v_track, &rows, |mark| {
                overview_mark_color(model, mark)
            });
        }
    }

    if let Some(h_track) = layout.h_scrollbar_rect(sw).filter(|_| !editor.soft_wrap) {
        if h_state.needs_scroll() {
            let h_geo = ScrollbarGeometry::horizontal(h_track, &h_state);
            render_scrollbar(frame, &h_geo, false, &colors);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{FindReplaceState, Rect};

    fn render_overview_marks(
        frame: &mut Frame,
        track: Rect,
        total_lines: usize,
        ticks: impl IntoIterator<Item = (usize, Mark)>,
        color_for: impl Fn(Mark) -> u32,
    ) {
        let rows = project_overview_marks(track.height, total_lines, ticks);
        render_overview_rows(frame, track, &rows, color_for);
    }

    fn document(text: &str) -> Document {
        let mut document = Document::new();
        document.buffer = ropey::Rope::from_str(text);
        document
    }

    #[test]
    fn overview_cache_reuses_rows_but_tracks_every_find_input_and_focus() {
        let document = document("cat\nCat\nscatter\ncat\n");
        let editor = EditorState::new();
        let mut find = FindReplaceState::default();
        find.set_query("cat");
        let first = overview_rows(&editor, &document, Some(find.results(&document)), 100.0);
        let again = overview_rows(&editor, &document, Some(find.results(&document)), 100.0);
        assert!(Arc::ptr_eq(&first, &again));
        let mut previous = first;
        for change in 0..5 {
            match change {
                0 => find.case_sensitive = true,
                1 => find.whole_word = true,
                2 => {
                    find.selection_only = true;
                    find.scope = Some((0, 4));
                }
                3 => find.use_regex = true,
                _ => find.set_query("Cat"),
            }
            let rows = overview_rows(&editor, &document, Some(find.results(&document)), 100.0);
            assert!(!Arc::ptr_eq(&previous, &rows));
            previous = rows;
        }
        let unfocused = overview_rows(&editor, &document, None, 100.0);
        assert!(!Arc::ptr_eq(&previous, &unfocused));
        assert!(unfocused.iter().all(Option::is_none));
    }

    #[test]
    fn overview_cache_tracks_buffer_identity_revision_and_fractional_height() {
        let mut document = document("a\nb\nc\n");
        let editor = EditorState::new();
        let first = overview_rows(&editor, &document, None, 10.4);
        let resized = overview_rows(&editor, &document, None, 10.6);
        assert!(!Arc::ptr_eq(&first, &resized));
        document.buffer = ropey::Rope::from_str("c\nb\na\n");
        let replaced = overview_rows(&editor, &document, None, 10.6);
        assert!(!Arc::ptr_eq(&resized, &replaced));
        document.revision += 1;
        let revised = overview_rows(&editor, &document, None, 10.6);
        assert!(!Arc::ptr_eq(&replaced, &revised));
    }

    #[test]
    fn overview_combines_find_and_diagnostics_in_the_same_priority_lane() {
        let mut document = document("a\nb\nc\n");
        let editor = EditorState::new();
        let mut find = FindReplaceState::default();
        find.set_query("b");
        document.diagnostics.push(lsp_types::Diagnostic {
            range: lsp_types::Range::new(
                lsp_types::Position::new(1, 0),
                lsp_types::Position::new(1, 1),
            ),
            severity: Some(lsp_types::DiagnosticSeverity::WARNING),
            ..Default::default()
        });
        let results = find.results(&document);
        let combined = overview_rows(&editor, &document, Some(results.clone()), 100.0);
        assert_eq!(combined[33], Some(Mark::Warning));
        document.diagnostics.clear();
        let find_only = overview_rows(&editor, &document, Some(results), 100.0);
        assert_eq!(find_only[33], Some(Mark::Match));
    }

    #[test]
    fn overview_cache_tracks_diagnostics_in_place_and_discards_vanished_ranges() {
        let mut document = document("a\nb\nc\n");
        let editor = EditorState::new();
        document.diagnostics.push(lsp_types::Diagnostic {
            range: lsp_types::Range::new(
                lsp_types::Position::new(1, 0),
                lsp_types::Position::new(1, 1),
            ),
            severity: Some(lsp_types::DiagnosticSeverity::WARNING),
            ..Default::default()
        });
        let first = overview_rows(&editor, &document, None, 100.0);
        assert_eq!(first[33], Some(Mark::Warning));
        document.diagnostics[0].message = "message does not affect geometry".into();
        assert!(Arc::ptr_eq(
            &first,
            &overview_rows(&editor, &document, None, 100.0)
        ));
        document.diagnostics[0].severity = Some(lsp_types::DiagnosticSeverity::ERROR);
        let severe = overview_rows(&editor, &document, None, 100.0);
        assert_eq!(severe[33], Some(Mark::Error));
        document.diagnostics[0].range.start.line = 2;
        document.diagnostics[0].range.end.line = 2;
        let moved = overview_rows(&editor, &document, None, 100.0);
        assert_eq!(moved[33], None);
        assert_eq!(moved[66], Some(Mark::Error));
        document.diagnostics[0].range.start.line = 99;
        document.diagnostics[0].range.end.line = 99;
        let vanished = overview_rows(&editor, &document, None, 100.0);
        assert!(vanished.iter().all(Option::is_none));
        document.diagnostics.clear();
        assert!(!Arc::ptr_eq(
            &vanished,
            &overview_rows(&editor, &document, None, 100.0)
        ));
    }

    #[test]
    fn overview_cache_uses_actual_wrap_mapping_and_matches_fresh_projection() {
        let document = document("abcdefghij\nmatch\nabcdefghij\n");
        let mut editor = EditorState::new();
        let mut find = FindReplaceState::default();
        find.set_query("match");
        let results = find.results(&document);
        let plain = overview_rows(&editor, &document, Some(results.clone()), 100.0);
        editor.soft_wrap = true;
        editor.wrap_cache.rebuild(&document, 4);
        let wrapped = overview_rows(&editor, &document, Some(results.clone()), 100.0);
        assert!(!Arc::ptr_eq(&plain, &wrapped));
        let map = editor.viewport_map(&document);
        let expected = project_overview_marks(
            100.0,
            map.row_count(),
            [(map.visual_line_for_position(1, 0), Mark::Match)],
        );
        assert_eq!(wrapped, expected);
        editor.wrap_cache.rebuild(&document, 5);
        let resized = overview_rows(&editor, &document, Some(results.clone()), 100.0);
        assert!(!Arc::ptr_eq(&wrapped, &resized));
        // Even an explicit replacement with equal row count has its own identity.
        editor.wrap_cache = crate::wrap::WrapCache::new();
        editor.wrap_cache.rebuild(&document, 5);
        let replaced = overview_rows(&editor, &document, Some(results.clone()), 100.0);
        assert!(!Arc::ptr_eq(&resized, &replaced));
        editor.wrap_cache.invalidate();
        assert_eq!(
            plain,
            overview_rows(&editor, &document, Some(results), 100.0)
        );
    }

    #[test]
    fn overview_projection_matches_reference_reducer_and_uses_current_colors() {
        for height in [0.5, 1.0, 1.5, 10.4, 10.6, 100.0] {
            let ticks: Vec<_> = (0..1000)
                .map(|line| {
                    (
                        line,
                        if line % 7 == 0 {
                            Mark::Error
                        } else {
                            Mark::Match
                        },
                    )
                })
                .collect();
            let rows = project_overview_marks(height, 1000, ticks.iter().copied());
            let mut expected = std::collections::BTreeMap::new();
            for (line, mark) in ticks {
                let row = track_row_for_position(height, 1000, line);
                let value = expected.entry(row).or_insert(mark);
                *value = (*value).max(mark);
            }
            assert_eq!(
                rows.iter()
                    .enumerate()
                    .filter_map(|(row, mark)| mark.map(|m| (row, m)))
                    .collect::<Vec<_>>(),
                expected.into_iter().collect::<Vec<_>>()
            );
        }
        let rows = project_overview_marks(10.0, 100, [(0, Mark::Match)]);
        let mut pixels = vec![0; 100];
        let mut frame = Frame::new(&mut pixels, 10, 10);
        render_overview_rows(&mut frame, Rect::new(0.0, 0.0, 1.0, 10.0), &rows, |_| 1);
        assert_eq!(frame.get_pixel(0, 0), 1);
        render_overview_rows(&mut frame, Rect::new(3.0, 2.0, 2.0, 10.0), &rows, |_| 2);
        assert_eq!(frame.get_pixel(3, 2), 2);
        assert_eq!(frame.get_pixel(4, 2), 2);
    }

    fn make_frame(width: usize, height: usize) -> (Vec<u32>, usize, usize) {
        (vec![0u32; width * height], width, height)
    }

    #[test]
    fn overview_marks_draw_one_tick_per_row_highest_priority_wins() {
        let (mut buf, w, h) = make_frame(20, 100);
        let mut frame = Frame::new(&mut buf, w, h);
        let track = Rect::new(0.0, 0.0, 12.0, 100.0);

        // Both lines collapse onto row 5; Error must win over Info.
        render_overview_marks(
            &mut frame,
            track,
            1000,
            [(50, Mark::Error), (51, Mark::Info)],
            |mark| match mark {
                Mark::Error => 0xFFFF0000,
                Mark::Info => 0xFF00FF00,
                _ => 0xFF000000,
            },
        );

        let row = track_row_for_position(track.height, 1000, 50);
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
