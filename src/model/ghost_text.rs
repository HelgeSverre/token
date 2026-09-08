//! A local insertion projection for inline suggestions. Only the anchor's
//! logical line is reflowed; the document, syntax and undo state stay untouched.

use std::{ops::Range, sync::Arc};

use super::{Document, Position};
use crate::{util::text::char_col_to_visual_col, wrap::compute_line_wraps};

/// Per-pane derived ghost geometry. It is synchronized by the update lifecycle.
#[derive(Debug, Clone, Default)]
pub struct GhostText(pub(crate) Option<Arc<GhostProjection>>);

#[derive(Debug)]
pub(crate) struct GhostProjection {
    source: ropey::Rope,
    pub anchor: Position,
    suggestion: String,
    width: Option<usize>,
    inserted_chars: usize,
    pub rows: Vec<GhostRow>,
}

#[derive(Debug)]
pub(crate) struct GhostRow {
    pub text: String,
    /// Character offset in prefix + insertion + suffix, including line endings.
    start: usize,
    len: usize,
    wraps: bool,
    pub sources: [Option<SourceSpan>; 2],
    /// Character range within this row; never includes source text.
    pub ghost: Range<usize>,
}

/// A contiguous source fragment on a visual row. A row can contain both a
/// prefix and suffix separated by ghost text, so one source range is not enough.
#[derive(Debug, Clone)]
pub(crate) struct SourceSpan {
    pub columns: Range<usize>,
    pub row_start: usize,
}

impl SourceSpan {
    pub fn visual_column(&self, text: &str, column: usize) -> usize {
        char_col_to_visual_col(
            text,
            self.row_start + column.saturating_sub(self.columns.start),
        )
    }
}

impl GhostProjection {
    pub fn new(
        document: &Document,
        anchor: Position,
        suggestion: &str,
        width: Option<usize>,
    ) -> Option<Self> {
        if suggestion.is_empty() || anchor.column > document.line_length(anchor.line) {
            return None;
        }
        let line = document.get_line_cow(anchor.line)?;
        let byte = line
            .char_indices()
            .nth(anchor.column)
            .map_or(line.len(), |(i, _)| i);
        let mut projected = line.into_owned();
        projected.insert_str(byte, suggestion);
        let inserted_chars = suggestion.chars().count();
        let ghost_end = anchor.column + inserted_chars;
        let projected_chars = projected.chars().count();
        let mut rows = Vec::new();
        let mut offset = 0;
        // Use the document's line definition, including Unicode separators and
        // the final empty row. Trim display endings exactly as get_line_cow does.
        let projected = ropey::Rope::from_str(&projected);
        for part in projected.lines() {
            let raw: std::borrow::Cow<'_, str> = part.into();
            let text = raw
                .strip_suffix("\r\n")
                .or_else(|| raw.strip_suffix('\n'))
                .unwrap_or(&raw);
            let line_chars = text.chars().count();
            let segments = compute_line_wraps(text, width.unwrap_or(usize::MAX), rows.len());
            let mut chars = text.chars();
            for segment in segments {
                let start = offset + segment.start_col;
                let end = start + segment.len;
                let source_span = |range: Range<usize>, shift: usize| {
                    let lo = range.start.max(start);
                    let hi = range.end.min(end);
                    (lo < hi).then(|| SourceSpan {
                        columns: lo - shift..hi - shift,
                        row_start: lo - start,
                    })
                };
                rows.push(GhostRow {
                    text: chars.by_ref().take(segment.len).collect(),
                    start,
                    len: segment.len,
                    wraps: segment.end_col() < line_chars,
                    sources: [
                        source_span(0..anchor.column, 0),
                        source_span(ghost_end..projected_chars, inserted_chars),
                    ],
                    ghost: anchor.column.clamp(start, end) - start
                        ..ghost_end.clamp(start, end) - start,
                });
            }
            offset += part.len_chars();
        }
        Some(Self {
            source: document.buffer.clone(),
            anchor,
            suggestion: suggestion.to_owned(),
            width,
            inserted_chars,
            rows,
        })
    }

    pub fn matches(
        &self,
        document: &Document,
        anchor: Position,
        suggestion: &str,
        width: Option<usize>,
    ) -> bool {
        self.source_is_current(document, width)
            && self.anchor == anchor
            && self.suggestion == suggestion
    }

    pub fn source_is_current(&self, document: &Document, width: Option<usize>) -> bool {
        self.source.is_instance(&document.buffer) && self.width == width
    }

    pub fn reflow(&self, document: &Document, width: Option<usize>) -> Option<Self> {
        if !self.source.is_instance(&document.buffer) {
            return None;
        }
        Self::new(document, self.anchor, &self.suggestion, width)
    }

    /// Caret affinity at the insertion is before the ghost. Source glyphs use
    /// SourceSpan instead, so the suffix at that same column is shifted after it.
    pub fn display_position(&self, column: usize) -> (usize, usize) {
        let offset = column
            + if column > self.anchor.column {
                self.inserted_chars
            } else {
                0
            };
        let row = self
            .rows
            .partition_point(|r| r.start <= offset)
            .saturating_sub(1);
        let data = &self.rows[row];
        (
            row,
            char_col_to_visual_col(&data.text, offset.saturating_sub(data.start).min(data.len)),
        )
    }

    pub fn source_column(&self, row: usize, display_column: usize) -> usize {
        let data = &self.rows[row];
        let mut end = data.len;
        if data.wraps {
            end = end.saturating_sub(1);
        }
        let column =
            crate::util::text::visual_col_to_char_col_from(&data.text, 0, end, display_column);
        let offset = data.start + column;
        if offset <= self.anchor.column {
            offset
        } else if offset < self.anchor.column + self.inserted_chars {
            self.anchor.column
        } else {
            offset - self.inserted_chars
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::EditorState;

    #[test]
    fn ghost_projection_matches_real_insertion_geometry_without_editing_source() {
        for source in [
            "",
            "abc",
            "before\n\tα🙂tail\r\nafter",
            "\n\nend",
            "long words wrap here\nnext",
        ] {
            for inserted in [
                "x",
                "\n",
                "\n\n",
                "\tfoo\n  bar",
                "α\r\n\t🙂\r\n",
                "x\ry",
                "x\u{2028}y",
            ] {
                let document = Document::with_text(source);
                for line in 0..document.line_count() {
                    for column in 0..=document.line_length(line) {
                        for width in [None, Some(1), Some(4), Some(10)] {
                            let anchor = Position::new(line, column);
                            let insertion_offset = document.cursor_to_offset(line, column);
                            let inserted_chars = inserted.chars().count();
                            let mut accepted = Document::with_text(source);
                            accepted.buffer.insert(insertion_offset, inserted);
                            let mut actual = EditorState::with_viewport(1000, width.unwrap_or(80));
                            actual.soft_wrap = width.is_some();
                            actual.ensure_wrap_cache(&document);
                            actual.ghost_text.0 = Some(Arc::new(
                                GhostProjection::new(&document, anchor, inserted, width).unwrap(),
                            ));
                            let mut expected =
                                EditorState::with_viewport(1000, width.unwrap_or(80));
                            expected.soft_wrap = width.is_some();
                            expected.ensure_wrap_cache(&accepted);
                            let map = actual.viewport_map(&document);
                            let oracle = expected.viewport_map(&accepted);
                            assert_eq!(
                                map.row_count(),
                                oracle.row_count(),
                                "{source:?} + {inserted:?} at {anchor:?}, {width:?}"
                            );
                            for source_line in 0..document.line_count() {
                                for source_column in 0..=document.line_length(source_line) {
                                    let offset =
                                        document.cursor_to_offset(source_line, source_column);
                                    let projected_offset = offset
                                        + if offset > insertion_offset {
                                            inserted_chars
                                        } else {
                                            0
                                        };
                                    let (l, c) = accepted.offset_to_cursor(projected_offset);
                                    assert_eq!(map.display_position(&document, source_line, source_column), oracle.display_position(&accepted, l, c), "{source:?} + {inserted:?} at {anchor:?}, source {source_line}:{source_column}, {width:?}");
                                }
                            }
                            for row in 0..map.row_count() {
                                let expected_segment =
                                    oracle.segment_for_visible_row(&accepted, row).unwrap();
                                let expected_line = oracle.doc_line_for_visible_row(row).unwrap();
                                let expected_text = accepted
                                    .get_line_slice(expected_line)
                                    .unwrap()
                                    .slice(expected_segment.start_col..expected_segment.end_col())
                                    .to_string();
                                let actual_text = if let Some(ghost) =
                                    map.ghost_row_for_visible_row(row)
                                {
                                    ghost.text.clone()
                                } else {
                                    let segment =
                                        map.segment_for_visible_row(&document, row).unwrap();
                                    document
                                        .get_line_slice(map.doc_line_for_visible_row(row).unwrap())
                                        .unwrap()
                                        .slice(segment.start_col..segment.end_col())
                                        .to_string()
                                };
                                assert_eq!(actual_text, expected_text);
                                for column in 0..=16 {
                                    let hit =
                                        oracle.position_at_display_column(&accepted, row, column);
                                    let offset = accepted.cursor_to_offset(hit.line, hit.column);
                                    let source_offset = if offset <= insertion_offset {
                                        offset
                                    } else if offset < insertion_offset + inserted_chars {
                                        insertion_offset
                                    } else {
                                        offset - inserted_chars
                                    };
                                    let (l, c) = document.offset_to_cursor(source_offset);
                                    assert_eq!(map.position_at_display_column(&document, row, column), Position::new(l, c), "hit {row}:{column}, {source:?} + {inserted:?} at {anchor:?}, {width:?}");
                                }
                            }
                            assert_eq!(document.buffer.to_string(), source);
                            assert!(document.undo_stack.is_empty());
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn ghost_source_fragments_exclude_insertion_and_preserve_suffix_tab_stops() {
        let document = Document::with_text("a\tz");
        let projection = GhostProjection::new(&document, Position::new(0, 1), "xy", None).unwrap();
        let row = &projection.rows[0];
        assert_eq!(row.text, "axy\tz");
        assert_eq!(row.ghost, 1..3);
        let prefix = row.sources[0].as_ref().unwrap();
        let suffix = row.sources[1].as_ref().unwrap();
        assert_eq!(prefix.columns, 0..1);
        assert_eq!(suffix.columns, 1..3);
        assert_eq!(suffix.visual_column(&row.text, 1), 3);
        assert_eq!(suffix.visual_column(&row.text, 2), 4);
        assert_eq!(projection.display_position(1), (0, 1));
        assert_eq!(projection.display_position(2), (0, 4));
        assert_eq!(projection.source_column(0, 2), 1);
        assert_eq!(projection.source_column(0, 4), 2);
    }

    #[test]
    fn ghost_projection_preserves_source_top_when_removed_and_rejects_replaced_buffers() {
        let mut document = Document::with_text("zero\none\ntail\nthree\nfour\nfive\nsix\nseven");
        let mut editor = EditorState::with_viewport(2, 20);
        let projection =
            GhostProjection::new(&document, Position::new(2, 2), "x\ny\nz", None).unwrap();
        editor.set_ghost_text(&document, Some(Arc::new(projection)));
        editor.viewport.top_line = 5;
        let map = editor.viewport_map(&document);
        assert_eq!(map.doc_line_for_visible_row(0), Some(3));
        assert_eq!(map.visible_row_for_doc_line(3), Some(0));
        assert_eq!(map.visible_doc_lines(), 3..5);
        assert_eq!(
            map.position_for_pixel(&document, 16.0, 0.0, 8.0, 16.0),
            Position::new(3, 2)
        );
        editor.set_ghost_text(&document, None);
        assert_eq!(editor.viewport.top_line, 3);
        let projection =
            GhostProjection::new(&document, Position::new(2, 2), "x\ny\nz", None).unwrap();
        editor.set_ghost_text(&document, Some(Arc::new(projection)));
        // The same revision number does not prove the same source.
        document.buffer = ropey::Rope::from_str("short");
        editor.ensure_wrap_cache(&document);
        assert!(editor.ghost_text.0.is_none());
        assert_eq!(editor.viewport.top_line, 0);
    }
}
