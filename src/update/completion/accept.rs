//! Pure completion edit planning and completion-specific validation.

use crate::model::{Cursor, Document, DocumentId, EditorId};

use super::super::text_edits::{EditCarets, PlannedEdit};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct AcceptanceGuard {
    pub document_id: DocumentId,
    pub editor_id: EditorId,
    pub revision: u64,
    pub cursors: Vec<Cursor>,
}

#[derive(Debug)]
pub(super) struct CompletionEditPlan {
    pub guard: AcceptanceGuard,
    pub edits: Vec<PlannedEdit>,
    pub offsets: Vec<usize>,
}

impl CompletionEditPlan {
    pub fn carets(&self) -> EditCarets<'_> {
        EditCarets::Place {
            editor_id: self.guard.editor_id,
            offsets: &self.offsets,
            before: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AcceptanceFailure {
    SingleCursorRequired,
    InvalidEdits,
}

impl AcceptanceFailure {
    pub fn message(self) -> &'static str {
        match self {
            Self::SingleCursorRequired => "This completion needs a single cursor.",
            Self::InvalidEdits => "This completion contains invalid or overlapping edits.",
        }
    }
}

/// Strict completion-only LSP conversion. Generic workspace edits retain their
/// existing tolerant policy; completion cannot silently clamp or drop a required
/// edit because acceptance must be atomic.
pub(super) fn plan_lsp_edits(
    document: &Document,
    edits: &[(lsp_types::Range, String)],
) -> Result<Vec<PlannedEdit>, AcceptanceFailure> {
    let mut spans = Vec::with_capacity(edits.len());
    for (index, (range, inserted)) in edits.iter().enumerate() {
        let start = strict_offset(document, range.start)?;
        let end = strict_offset(document, range.end)?;
        if end < start {
            return Err(AcceptanceFailure::InvalidEdits);
        }
        spans.push((start, end, index, inserted));
    }
    spans.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)).then(b.2.cmp(&a.2)));
    let mut planned = Vec::with_capacity(spans.len());
    let mut min_start_kept = usize::MAX;
    for (start, end, _, inserted) in spans {
        if end > min_start_kept {
            return Err(AcceptanceFailure::InvalidEdits);
        }
        min_start_kept = start;
        planned.push(PlannedEdit {
            start,
            deleted: document.buffer.slice(start..end).to_string(),
            inserted: inserted.clone(),
        });
    }
    Ok(planned)
}

pub(super) fn strict_offset(
    document: &Document,
    position: lsp_types::Position,
) -> Result<usize, AcceptanceFailure> {
    if position.line as usize >= document.line_count() {
        return Err(AcceptanceFailure::InvalidEdits);
    }
    let converted = crate::lsp::lsp_to_position(document, position);
    if crate::lsp::position_to_lsp(document, converted) != position {
        return Err(AcceptanceFailure::InvalidEdits);
    }
    Ok(document.cursor_to_offset(converted.line, converted.column))
}

pub(super) fn overlaps(a: &PlannedEdit, b: &PlannedEdit) -> bool {
    a.start < b.end() && a.end() > b.start
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strict_planning_rejects_clamped_utf16_and_overlaps() {
        let document = Document::with_text("a🦀b\n");
        let invalid = lsp_types::Range::new(
            lsp_types::Position::new(0, 2),
            lsp_types::Position::new(0, 3),
        );
        assert_eq!(
            plan_lsp_edits(&document, &[(invalid, "x".into())]),
            Err(AcceptanceFailure::InvalidEdits)
        );
        let range = lsp_types::Range::new(
            lsp_types::Position::new(0, 0),
            lsp_types::Position::new(0, 1),
        );
        assert_eq!(
            plan_lsp_edits(&document, &[(range, "x".into()), (range, "y".into())]),
            Err(AcceptanceFailure::InvalidEdits)
        );
    }
}
