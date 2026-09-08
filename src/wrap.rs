//! Soft-wrap segmentation and logical/visual position mapping.

use crate::model::Document;
use crate::util::text::TABULATOR_WIDTH;

/// One visual-row segment of a logical document line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WrapSegment {
    /// Character offset within the logical line where this segment starts.
    pub start_col: usize,
    /// Number of document characters in this segment.
    pub len: usize,
    /// Global visual-row index after wrapping all preceding logical lines.
    pub visual_line: usize,
    /// Whether this is a continuation of an earlier segment on the same line.
    pub is_continuation: bool,
}

impl WrapSegment {
    #[inline]
    pub fn end_col(self) -> usize {
        self.start_col.saturating_add(self.len)
    }
}

/// Cached soft-wrap layout for one editor pane.
///
/// The cache belongs to an editor rather than a document because panes showing
/// the same document can have different viewport widths and wrap settings.
#[derive(Debug, Clone, Default)]
pub struct WrapCache {
    lines: Vec<Vec<WrapSegment>>,
    visual_rows: Vec<(usize, usize)>,
    total_visual_lines: usize,
    wrap_width: usize,
    revision: u64,
    valid: bool,
    source: Option<ropey::Rope>,
    layout_identity: std::sync::Arc<()>,
}

impl WrapCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn invalidate(&mut self) {
        self.valid = false;
    }

    #[inline]
    pub fn is_valid(&self) -> bool {
        self.valid
    }

    /// Identity of the actual mapping, including replacements with equal row counts.
    pub(crate) fn layout_identity(&self) -> Option<std::sync::Arc<()>> {
        self.valid.then(|| self.layout_identity.clone())
    }

    #[inline]
    pub fn total_visual_lines(&self) -> usize {
        self.total_visual_lines
    }

    pub fn needs_rebuild(&self, document_revision: u64, wrap_width: usize) -> bool {
        !self.valid || self.revision != document_revision || self.wrap_width != wrap_width.max(1)
    }

    /// Also checks rope identity so reloads and buffer replacements cannot reuse
    /// stale layout even when their revision number happens to match.
    pub fn needs_refresh(&self, document: &Document, wrap_width: usize) -> bool {
        self.needs_rebuild(document.revision, wrap_width)
            || !self
                .source
                .as_ref()
                .is_some_and(|source| source.is_instance(&document.buffer))
    }

    /// Recompute only the changed logical lines when the width is unchanged.
    /// Rope clones share storage; equal prefix/suffix chunks bound the changed
    /// region without allocating or visiting every line in a large document.
    pub fn refresh(&mut self, document: &Document, wrap_width: usize) {
        if !self.needs_refresh(document, wrap_width) {
            return;
        }
        let Some(source) = self
            .source
            .as_ref()
            .filter(|_| self.valid && self.wrap_width == wrap_width.max(1))
        else {
            self.rebuild(document, wrap_width);
            return;
        };
        let prefix_bytes: usize = source
            .chunks()
            .zip(document.buffer.chunks())
            .take_while(|(old, new)| old == new)
            .map(|(chunk, _)| chunk.len())
            .sum();
        let suffix_bytes: usize = source
            .chunks_at_byte(source.len_bytes())
            .0
            .reversed()
            .zip(
                document
                    .buffer
                    .chunks_at_byte(document.buffer.len_bytes())
                    .0
                    .reversed(),
            )
            .take_while(|(old, new)| old == new)
            .map(|(chunk, _)| chunk.len())
            .sum();
        let suffix_bytes = suffix_bytes.min(
            source
                .len_bytes()
                .min(document.buffer.len_bytes())
                .saturating_sub(prefix_bytes),
        );
        let first = source
            .byte_to_line(prefix_bytes)
            .min(document.buffer.byte_to_line(prefix_bytes));
        let old_end =
            (source.byte_to_line(source.len_bytes() - suffix_bytes) + 1).min(source.len_lines());
        let new_end = (document
            .buffer
            .byte_to_line(document.buffer.len_bytes() - suffix_bytes)
            + 1)
        .min(document.line_count());
        let first_row = self.logical_line_to_visual(first);
        let old_end_row = self
            .lines
            .get(old_end)
            .and_then(|segments| segments.first())
            .map_or(self.total_visual_lines, |s| s.visual_line);
        let mut next_row = first_row;
        let replacement: Vec<_> = (first..new_end)
            .map(|line| {
                let text = document.get_line_cow(line).unwrap_or_default();
                let segments = compute_line_wraps(&text, self.wrap_width, next_row);
                next_row += segments.len();
                segments
            })
            .collect();
        self.lines.splice(first..old_end, replacement);
        if old_end == new_end && next_row == old_end_row {
            // Common keystroke: no row counts changed, so the suffix keeps its
            // existing indexes and segment allocations.
            let mut row = first_row;
            for line in first..new_end {
                for segment_index in 0..self.lines[line].len() {
                    self.visual_rows[row] = (line, segment_index);
                    row += 1;
                }
            }
        } else {
            self.visual_rows.truncate(first_row);
            for (line, segments) in self.lines.iter_mut().enumerate().skip(first) {
                for (segment_index, segment) in segments.iter_mut().enumerate() {
                    segment.visual_line = self.visual_rows.len();
                    self.visual_rows.push((line, segment_index));
                }
            }
        }
        self.total_visual_lines = self.visual_rows.len();
        self.source = Some(document.buffer.clone());
        self.revision = document.revision;
        self.layout_identity = std::sync::Arc::new(());
    }

    /// Rebuild all wrap segments for `document` at the supplied character width.
    pub fn rebuild(&mut self, document: &Document, wrap_width: usize) {
        let wrap_width = wrap_width.max(1);
        self.lines.clear();
        self.visual_rows.clear();

        for logical_line in 0..document.line_count() {
            let line = document.get_line_cow(logical_line).unwrap_or_default();
            let segments = compute_line_wraps(&line, wrap_width, self.visual_rows.len());
            for segment_index in 0..segments.len() {
                self.visual_rows.push((logical_line, segment_index));
            }
            self.lines.push(segments);
        }

        self.total_visual_lines = self.visual_rows.len();
        self.wrap_width = wrap_width;
        self.revision = document.revision;
        self.valid = true;
        self.source = Some(document.buffer.clone());
        self.layout_identity = std::sync::Arc::new(());
    }

    /// Convert a logical document position to a global visual-row position.
    pub fn logical_to_visual(&self, line: usize, column: usize) -> (usize, usize) {
        let Some(segments) = self.lines.get(line) else {
            return (line, column);
        };

        let index = segments.partition_point(|segment| segment.end_col() <= column);
        if let Some(segment) = segments.get(index) {
            return (
                segment.visual_line,
                column.saturating_sub(segment.start_col),
            );
        }

        segments
            .last()
            .map_or((line, column), |segment| (segment.visual_line, segment.len))
    }

    /// Convert a global visual-row position to a logical document position.
    pub fn visual_to_logical(&self, visual_line: usize, visual_column: usize) -> (usize, usize) {
        let Some(&(logical_line, segment_index)) = self.visual_rows.get(visual_line) else {
            return (visual_line, visual_column);
        };
        let segment = self.lines[logical_line][segment_index];
        (
            logical_line,
            segment.start_col + visual_column.min(segment.len),
        )
    }

    #[inline]
    pub fn logical_line_to_visual(&self, logical_line: usize) -> usize {
        self.lines
            .get(logical_line)
            .and_then(|segments| segments.first())
            .map_or(logical_line, |segment| segment.visual_line)
    }

    #[inline]
    pub fn visual_line_to_logical(&self, visual_line: usize) -> usize {
        self.visual_rows
            .get(visual_line)
            .map_or(visual_line, |&(logical_line, _)| logical_line)
    }

    pub fn segment_for_visual_line(&self, visual_line: usize) -> Option<&WrapSegment> {
        let &(logical_line, segment_index) = self.visual_rows.get(visual_line)?;
        self.lines.get(logical_line)?.get(segment_index)
    }

    pub fn segments_for_logical_line(&self, logical_line: usize) -> Option<&[WrapSegment]> {
        self.lines.get(logical_line).map(Vec::as_slice)
    }

    pub fn visual_line_count(&self, logical_line: usize) -> usize {
        self.lines.get(logical_line).map_or(1, Vec::len)
    }

    pub fn is_continuation(&self, visual_line: usize) -> bool {
        self.segment_for_visual_line(visual_line)
            .is_some_and(|segment| segment.is_continuation)
    }
}

/// Compute the visual-row segments for one newline-free logical line.
pub fn compute_line_wraps(
    line: &str,
    wrap_width: usize,
    starting_visual_line: usize,
) -> Vec<WrapSegment> {
    if line.is_empty() {
        return vec![WrapSegment {
            start_col: 0,
            len: 0,
            visual_line: starting_visual_line,
            is_continuation: false,
        }];
    }

    let wrap_width = wrap_width.max(1);
    let mut segments = Vec::new();
    let mut start = 0;
    let mut remaining = line;

    while !remaining.is_empty() {
        let mut visual_width = 0;
        let mut chars_fit = 0;
        let mut bytes_fit = 0;
        let mut last_break = None;

        for (byte, ch) in remaining.char_indices() {
            let char_width = if ch == '\t' {
                TABULATOR_WIDTH - (visual_width % TABULATOR_WIDTH)
            } else {
                // The editor's existing column and glyph-placement model is
                // character based. Keep wrapping on that same model until the
                // renderer adopts cell widths for wide Unicode everywhere.
                1
            };

            if visual_width + char_width > wrap_width && chars_fit > 0 {
                break;
            }

            visual_width += char_width;
            chars_fit += 1;
            bytes_fit = byte + ch.len_utf8();
            if ch.is_whitespace() {
                last_break = Some((chars_fit, bytes_fit));
            }
        }

        let (len, bytes) = if bytes_fit == remaining.len() {
            (chars_fit, bytes_fit)
        } else {
            last_break.unwrap_or((chars_fit, bytes_fit))
        };

        segments.push(WrapSegment {
            start_col: start,
            len,
            visual_line: starting_visual_line + segments.len(),
            is_continuation: !segments.is_empty(),
        });
        start += len;
        remaining = &remaining[bytes..];
    }

    segments
}

#[cfg(test)]
mod tests {
    use super::{compute_line_wraps, WrapCache, WrapSegment};
    use crate::model::Document;

    #[test]
    fn short_and_empty_lines_occupy_one_visual_row() {
        assert_eq!(
            compute_line_wraps("", 10, 4),
            vec![WrapSegment {
                start_col: 0,
                len: 0,
                visual_line: 4,
                is_continuation: false,
            }]
        );
        assert_eq!(compute_line_wraps("hello", 10, 2)[0].len, 5);
    }

    #[test]
    fn wraps_at_the_last_whitespace_boundary() {
        let segments = compute_line_wraps("hello world foo", 12, 0);
        assert_eq!(segments.len(), 2);
        assert_eq!((segments[0].start_col, segments[0].len), (0, 12));
        assert_eq!((segments[1].start_col, segments[1].len), (12, 3));
        assert!(segments[1].is_continuation);
    }

    #[test]
    fn forces_long_words_to_wrap() {
        let segments = compute_line_wraps("abcdefghij", 4, 0);
        let ranges: Vec<_> = segments
            .iter()
            .map(|segment| (segment.start_col, segment.len))
            .collect();
        assert_eq!(ranges, vec![(0, 4), (4, 4), (8, 2)]);
    }

    #[test]
    fn tabs_use_the_shared_display_width_and_always_advance() {
        let segments = compute_line_wraps("a\tb\tc", 5, 0);
        assert_eq!(segments.len(), 2);
        assert_eq!((segments[0].start_col, segments[0].len), (0, 2));
        assert_eq!((segments[1].start_col, segments[1].len), (2, 3));

        let narrow = compute_line_wraps("\t", 1, 0);
        assert_eq!(narrow[0].len, 1);
    }

    #[test]
    fn cache_maps_positions_in_both_directions() {
        let document = Document::with_text("hello world foo\n\nbar");
        let mut cache = WrapCache::new();
        cache.rebuild(&document, 12);

        assert_eq!(cache.total_visual_lines(), 4);
        for column in 0..=15 {
            let (visual_line, visual_column) = cache.logical_to_visual(0, column);
            assert_eq!(
                cache.visual_to_logical(visual_line, visual_column),
                (0, column)
            );
        }
        assert_eq!(cache.visual_to_logical(2, 99), (1, 0));
        assert_eq!(cache.visual_line_to_logical(3), 2);
        assert_eq!(cache.logical_line_to_visual(2), 3);
    }

    #[test]
    fn revision_and_width_control_cache_validity() {
        let mut document = Document::with_text("hello");
        let mut cache = WrapCache::new();
        cache.rebuild(&document, 10);
        assert!(!cache.needs_rebuild(document.revision, 10));
        assert!(cache.needs_rebuild(document.revision, 5));

        document.revision = document.revision.wrapping_add(1);
        assert!(cache.needs_rebuild(document.revision, 10));
        cache.invalidate();
        assert!(!cache.is_valid());
    }

    #[test]
    fn incremental_layout_matches_rebuild_after_edits_and_reloads() {
        let mut document = Document::with_text(&"alpha beta\t終わり🙂\r\n".repeat(400));
        let mut incremental = WrapCache::new();
        incremental.rebuild(&document, 12);
        let mut random = 7123u64;
        for iteration in 0..180 {
            random = random.wrapping_mul(6364136223846793005).wrapping_add(1);
            let offset = random as usize % (document.buffer.len_chars() + 1);
            if iteration % 3 == 0 && offset < document.buffer.len_chars() {
                let end = (offset + 31).min(document.buffer.len_chars());
                document.buffer.remove(offset..end);
            } else {
                document.buffer.insert(
                    offset,
                    ["🙂", "\nnew line\n", "\t\t", "some words "][iteration % 4],
                );
            }
            // Include edits that replace a buffer without bumping revision.
            if iteration % 5 != 0 {
                document.revision = document.revision.wrapping_add(1);
            }
            let width = if iteration % 17 == 0 { 5 } else { 12 };
            incremental.refresh(&document, width);
            let mut rebuilt = WrapCache::new();
            rebuilt.rebuild(&document, width);
            assert_eq!(incremental.lines, rebuilt.lines, "iteration {iteration}");
            assert_eq!(
                incremental.visual_rows, rebuilt.visual_rows,
                "iteration {iteration}"
            );
            assert_eq!(
                incremental.total_visual_lines(),
                rebuilt.total_visual_lines()
            );
        }
        document.buffer = ropey::Rope::from_str("");
        incremental.refresh(&document, 12);
        assert_eq!(incremental.total_visual_lines(), 1);
        assert_eq!(incremental.logical_to_visual(0, 0), (0, 0));
    }
}
