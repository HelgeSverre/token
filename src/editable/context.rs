//! Edit context for the unified text editing system.
//!
//! Identifies which editing context (main editor, modals, CSV cells) a message is for.

use super::constraints::EditConstraints;

/// Identifies which editing context a message is for.
///
/// This enum is used for routing text editing messages to the appropriate
/// handler and determining which constraints apply.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EditContext {
    /// Main document editor (identified by index into active editors)
    Editor,

    /// Command palette input
    CommandPalette,

    /// Go to line dialog input
    GotoLine,

    /// Find/Replace query field
    FindQuery,

    /// Find/Replace replacement field
    ReplaceQuery,

    /// CSV cell being edited (row, column)
    CsvCell { row: usize, col: usize },
}

impl EditContext {
    /// Get the constraints for this context
    pub fn constraints(&self) -> EditConstraints {
        match self {
            EditContext::Editor => EditConstraints::editor(),
            EditContext::CommandPalette => EditConstraints::single_line(),
            EditContext::GotoLine => EditConstraints::numeric(),
            EditContext::FindQuery => EditConstraints::single_line(),
            EditContext::ReplaceQuery => EditConstraints::single_line(),
            EditContext::CsvCell { .. } => EditConstraints::csv_cell(),
        }
    }

    /// Check if this is a modal context
    pub fn is_modal(&self) -> bool {
        matches!(
            self,
            EditContext::CommandPalette
                | EditContext::GotoLine
                | EditContext::FindQuery
                | EditContext::ReplaceQuery
        )
    }

    /// Check if this is the main editor
    pub fn is_editor(&self) -> bool {
        matches!(self, EditContext::Editor)
    }

    /// Check if this is a CSV cell
    pub fn is_csv_cell(&self) -> bool {
        matches!(self, EditContext::CsvCell { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_editor_context() {
        let ctx = EditContext::Editor;
        assert!(ctx.is_editor());
        assert!(!ctx.is_modal());
        assert!(ctx.constraints().allow_multiline);
        assert!(ctx.constraints().allow_multi_cursor);
    }

    #[test]
    fn test_modal_contexts() {
        assert!(EditContext::CommandPalette.is_modal());
        assert!(EditContext::GotoLine.is_modal());
        assert!(EditContext::FindQuery.is_modal());
        assert!(EditContext::ReplaceQuery.is_modal());
    }

    #[test]
    fn test_goto_line_constraints() {
        let ctx = EditContext::GotoLine;
        let c = ctx.constraints();
        assert!(c.is_char_allowed('5'));
        assert!(!c.is_char_allowed('a'));
    }

    #[test]
    fn test_csv_cell_context() {
        let ctx = EditContext::CsvCell { row: 1, col: 2 };
        assert!(ctx.is_csv_cell());
        assert!(!ctx.is_modal());
        assert!(!ctx.constraints().allow_multiline);
    }
}
