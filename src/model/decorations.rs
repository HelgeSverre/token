//! Gutter marks contract shared by find/diagnostics/bookmark producers.
//!
//! See `docs/feature/editor-decorations.md`. No feature currently populates
//! a mark source — this module is the collection seam the next consumer
//! (find-enhancements or LSP diagnostics) extends with one branch in
//! `collect_line_marks`, per the doc's acceptance criteria.

use super::Document;

/// One glyph's worth of gutter-marks-lane state for a line.
///
/// Declaration order is priority order, lowest to highest: derived `Ord`
/// resolves "breakpoint > error > warning > info > bookmark" via `max()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Mark {
    Bookmark,
    Info,
    Warning,
    Error,
    Breakpoint,
}

/// Pick the highest-priority mark among candidates for the same line.
///
/// One lane, one slot: callers never need more than the winner.
pub fn best_mark(candidates: impl IntoIterator<Item = Mark>) -> Option<Mark> {
    candidates.into_iter().max()
}

/// Marks-lane state for one visible line, gathered during the gutter pass.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LineMarks {
    pub mark: Option<Mark>,
}

/// Collect marks-lane state for `doc_line`.
///
/// No producer exists yet (see module docs), so this always returns
/// `LineMarks::default()` — the hook point is wired into the real gutter
/// render pass already so a future producer adds one candidate-gathering
/// branch here, not new plumbing.
pub fn collect_line_marks(doc: &Document, doc_line: usize) -> LineMarks {
    if doc_line >= doc.line_count() {
        return LineMarks::default();
    }

    let candidates: [Option<Mark>; 0] = [];
    LineMarks {
        mark: best_mark(candidates.into_iter().flatten()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn breakpoint_outranks_every_other_mark() {
        assert!(Mark::Breakpoint > Mark::Error);
        assert!(Mark::Error > Mark::Warning);
        assert!(Mark::Warning > Mark::Info);
        assert!(Mark::Info > Mark::Bookmark);
    }

    #[test]
    fn best_mark_picks_highest_priority_regardless_of_input_order() {
        let candidates = [Mark::Info, Mark::Bookmark, Mark::Warning];
        assert_eq!(best_mark(candidates), Some(Mark::Warning));
    }

    #[test]
    fn best_mark_of_empty_candidates_is_none() {
        assert_eq!(best_mark(std::iter::empty()), None);
    }

    #[test]
    fn collect_line_marks_out_of_bounds_line_is_default() {
        let doc = Document::new();
        assert_eq!(collect_line_marks(&doc, 5), LineMarks::default());
    }

    #[test]
    fn collect_line_marks_in_bounds_line_has_no_marks_yet() {
        let mut doc = Document::new();
        doc.buffer = ropey::Rope::from("a\nb\nc\n");
        assert_eq!(collect_line_marks(&doc, 1), LineMarks::default());
    }
}
