//! Gutter marks contract shared by find/diagnostics/bookmark producers.
//!
//! See `docs/feature/editor-decorations.md`. LSP diagnostics
//! (lsp-integration.md Phase 2) is the first real mark source, wired into
//! `collect_line_marks` below.

use super::Document;

/// One glyph's worth of gutter-marks-lane state for a line.
///
/// Declaration order is priority order, lowest to highest: derived `Ord`
/// resolves "breakpoint > error > warning > info > bookmark" via `max()`.
///
/// `Match` (a find/replace search hit) sits below every other variant —
/// it's only ever used for scrollbar overview ticks (find-enhancements
/// opts out of the gutter marks lane entirely), so its relative priority
/// only matters if a real gutter-marks producer ever collides with it on
/// the same track row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Mark {
    Match,
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
/// LSP diagnostics is the first producer: any diagnostic whose range
/// touches `doc_line` contributes a candidate, highest severity winning
/// via `best_mark`.
///
/// ponytail: this is a linear scan over every diagnostic per visible
/// line (O(diagnostics × viewport) per frame), not the "O(log n) or
/// better" the design doc asks a source to hit on its own. Diagnostic
/// counts are typically dozens and viewports dozens of lines, so this is
/// unmeasured-but-fine; switch `Document.diagnostics` to a
/// sorted-by-start-line `Vec` plus a binary search (or a per-line map)
/// if profiling ever shows otherwise.
pub fn collect_line_marks(doc: &Document, doc_line: usize) -> LineMarks {
    if doc_line >= doc.line_count() {
        return LineMarks::default();
    }

    let candidates = doc
        .diagnostics
        .iter()
        .filter(|d| diagnostic_touches_line(d, doc_line))
        .map(|d| diagnostic_mark(d.severity));

    LineMarks {
        mark: best_mark(candidates),
    }
}

/// Whether a diagnostic's (LSP, 0-based) range covers `doc_line`.
fn diagnostic_touches_line(diagnostic: &lsp_types::Diagnostic, doc_line: usize) -> bool {
    let line = doc_line as u32;
    diagnostic.range.start.line <= line && line <= diagnostic.range.end.line
}

/// Maps an LSP severity to a gutter `Mark`. `None` (server left severity
/// to the client) and `ERROR` both resolve to `Mark::Error` — the more
/// visible choice when a server declines to say. `HINT` folds into
/// `Mark::Info`: the marks lane has no separate hint glyph. Shared with
/// the scrollbar-overview-tick and range-decoration producers
/// (`view::mod`) so every diagnostic surface agrees on severity->Mark.
pub fn diagnostic_mark(severity: Option<lsp_types::DiagnosticSeverity>) -> Mark {
    match severity {
        Some(lsp_types::DiagnosticSeverity::WARNING) => Mark::Warning,
        Some(lsp_types::DiagnosticSeverity::INFORMATION)
        | Some(lsp_types::DiagnosticSeverity::HINT) => Mark::Info,
        _ => Mark::Error,
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
    fn collect_line_marks_in_bounds_line_without_diagnostics_is_default() {
        let mut doc = Document::new();
        doc.buffer = ropey::Rope::from("a\nb\nc\n");
        assert_eq!(collect_line_marks(&doc, 1), LineMarks::default());
    }

    fn diagnostic_at(
        line: u32,
        severity: Option<lsp_types::DiagnosticSeverity>,
    ) -> lsp_types::Diagnostic {
        lsp_types::Diagnostic {
            range: lsp_types::Range {
                start: lsp_types::Position { line, character: 0 },
                end: lsp_types::Position { line, character: 3 },
            },
            severity,
            message: "test".into(),
            ..Default::default()
        }
    }

    #[test]
    fn collect_line_marks_surfaces_a_diagnostic_on_its_line() {
        let mut doc = Document::new();
        doc.buffer = ropey::Rope::from("a\nb\nc\n");
        doc.diagnostics = vec![diagnostic_at(1, Some(lsp_types::DiagnosticSeverity::ERROR))];
        assert_eq!(collect_line_marks(&doc, 1).mark, Some(Mark::Error));
        assert_eq!(collect_line_marks(&doc, 0).mark, None);
    }

    #[test]
    fn collect_line_marks_picks_the_highest_severity_on_a_shared_line() {
        let mut doc = Document::new();
        doc.buffer = ropey::Rope::from("a\nb\nc\n");
        doc.diagnostics = vec![
            diagnostic_at(0, Some(lsp_types::DiagnosticSeverity::HINT)),
            diagnostic_at(0, Some(lsp_types::DiagnosticSeverity::WARNING)),
        ];
        assert_eq!(collect_line_marks(&doc, 0).mark, Some(Mark::Warning));
    }

    #[test]
    fn collect_line_marks_treats_missing_severity_as_error() {
        let mut doc = Document::new();
        doc.buffer = ropey::Rope::from("a\nb\nc\n");
        doc.diagnostics = vec![diagnostic_at(0, None)];
        assert_eq!(collect_line_marks(&doc, 0).mark, Some(Mark::Error));
    }

    #[test]
    fn collect_line_marks_spans_a_multi_line_diagnostic_range() {
        let mut doc = Document::new();
        doc.buffer = ropey::Rope::from("a\nb\nc\n");
        let mut diagnostic = diagnostic_at(0, Some(lsp_types::DiagnosticSeverity::ERROR));
        diagnostic.range.end.line = 2;
        doc.diagnostics = vec![diagnostic];
        assert_eq!(collect_line_marks(&doc, 1).mark, Some(Mark::Error));
    }
}
