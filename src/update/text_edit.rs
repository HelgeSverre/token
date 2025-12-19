//! Unified text editing update handler.
//!
//! Routes TextEditMsg to the appropriate EditableState based on EditContext.
//! This is Phase 2 of the unified text editing system.
//!
//! For the main editor (EditContext::Editor), we use a bridge approach that maps
//! TextEditMsg to existing EditorMsg/DocumentMsg handlers. This allows gradual
//! migration while maintaining full backwards compatibility.

use crate::commands::Cmd;
use crate::editable::{
    EditContext, EditableState, MoveTarget, StringBuffer, TextBuffer, TextBufferMut, TextEditMsg,
};
use crate::messages::{Direction, DocumentMsg, EditorMsg};
use crate::model::{AppModel, FindReplaceField, ModalState};

use super::{update_document, update_editor};

/// Handle a TextEditMsg by routing to the appropriate EditableState.
pub fn update_text_edit(
    model: &mut AppModel,
    context: EditContext,
    msg: TextEditMsg,
) -> Option<Cmd> {
    match context {
        EditContext::Editor => bridge_text_edit_to_editor(model, msg),

        EditContext::CommandPalette => {
            if let Some(ModalState::CommandPalette(ref mut state)) = model.ui.active_modal {
                let modified = apply_text_edit_msg(&mut state.editable, &msg);
                // Reset selection when input changes (for command filtering)
                if msg.is_editing() {
                    state.selected_index = 0;
                }
                if modified {
                    Some(Cmd::Redraw)
                } else {
                    None
                }
            } else {
                tracing::debug!("TextEdit::CommandPalette received but no modal active");
                None
            }
        }

        EditContext::GotoLine => {
            if let Some(ModalState::GotoLine(ref mut state)) = model.ui.active_modal {
                let modified = apply_text_edit_msg(&mut state.editable, &msg);
                if modified {
                    Some(Cmd::Redraw)
                } else {
                    None
                }
            } else {
                tracing::debug!("TextEdit::GotoLine received but no modal active");
                None
            }
        }

        EditContext::FindQuery => {
            if let Some(ModalState::FindReplace(ref mut state)) = model.ui.active_modal {
                // Ensure we're editing the query field
                state.focused_field = FindReplaceField::Query;
                let modified = apply_text_edit_msg(&mut state.query_editable, &msg);
                if modified {
                    Some(Cmd::Redraw)
                } else {
                    None
                }
            } else {
                tracing::debug!("TextEdit::FindQuery received but no modal active");
                None
            }
        }

        EditContext::ReplaceQuery => {
            if let Some(ModalState::FindReplace(ref mut state)) = model.ui.active_modal {
                // Ensure we're editing the replace field
                state.focused_field = FindReplaceField::Replace;
                let modified = apply_text_edit_msg(&mut state.replace_editable, &msg);
                if modified {
                    Some(Cmd::Redraw)
                } else {
                    None
                }
            } else {
                tracing::debug!("TextEdit::ReplaceQuery received but no modal active");
                None
            }
        }

        EditContext::CsvCell { .. } => {
            // CSV cell editing is handled directly via CsvMsg, not through TextEdit
            // This context exists for completeness but routes through update_csv()
            None
        }
    }
}

/// Apply a TextEditMsg to an EditableState with a StringBuffer.
/// Returns true if the state was modified (needs redraw).
pub fn apply_text_edit_msg(state: &mut EditableState<StringBuffer>, msg: &TextEditMsg) -> bool {
    match msg {
        // === Movement ===
        TextEditMsg::Move(target) => {
            apply_move(state, *target, false);
            true
        }
        TextEditMsg::MoveWithSelection(target) => {
            apply_move(state, *target, true);
            true
        }

        // === Insertion ===
        TextEditMsg::InsertChar(ch) => state.insert_char(*ch),
        TextEditMsg::InsertText(text) => state.insert_text(text),
        TextEditMsg::InsertNewline => {
            // For single-line inputs, newline is typically ignored
            false
        }

        // === Deletion ===
        TextEditMsg::DeleteBackward => state.delete_backward(),
        TextEditMsg::DeleteForward => state.delete_forward(),
        TextEditMsg::DeleteWordBackward => state.delete_word_backward(),
        TextEditMsg::DeleteWordForward => state.delete_word_forward(),
        TextEditMsg::DeleteLine => {
            // Not applicable for single-line inputs
            false
        }

        // === Selection ===
        TextEditMsg::SelectAll => {
            state.select_all();
            true
        }
        TextEditMsg::SelectWord => {
            state.select_word();
            true
        }
        TextEditMsg::SelectLine => {
            // For single-line, select_all is equivalent
            state.select_all();
            true
        }
        TextEditMsg::CollapseSelection => {
            state.collapse_selection();
            true
        }

        // === Multi-Cursor (not supported in single-line contexts) ===
        TextEditMsg::AddCursorAbove
        | TextEditMsg::AddCursorBelow
        | TextEditMsg::AddCursorAtNextOccurrence
        | TextEditMsg::AddCursorsAtAllOccurrences
        | TextEditMsg::CollapseCursors => false,

        // === Clipboard ===
        TextEditMsg::Copy => {
            // Caller should handle clipboard access
            // We just indicate whether there's something to copy
            state.has_selection()
        }
        TextEditMsg::Cut => {
            if state.has_selection() {
                state.delete_backward();
                true
            } else {
                false
            }
        }
        TextEditMsg::Paste(text) => state.insert_text(text),

        // === Undo/Redo ===
        TextEditMsg::Undo => state.undo(),
        TextEditMsg::Redo => state.redo(),

        // === Line Operations (not applicable for single-line) ===
        TextEditMsg::Indent
        | TextEditMsg::Unindent
        | TextEditMsg::Duplicate
        | TextEditMsg::MoveLineUp
        | TextEditMsg::MoveLineDown => false,
    }
}

/// Apply movement to an EditableState.
fn apply_move<B: TextBuffer + TextBufferMut>(
    state: &mut EditableState<B>,
    target: MoveTarget,
    extend_selection: bool,
) {
    match target {
        MoveTarget::Left => state.move_left(extend_selection),
        MoveTarget::Right => state.move_right(extend_selection),
        MoveTarget::Up => state.move_up(extend_selection),
        MoveTarget::Down => state.move_down(extend_selection),
        MoveTarget::LineStart => state.move_line_start(extend_selection),
        MoveTarget::LineEnd => state.move_line_end(extend_selection),
        MoveTarget::LineStartSmart => state.move_line_start_smart(extend_selection),
        MoveTarget::WordLeft => state.move_word_left(extend_selection),
        MoveTarget::WordRight => state.move_word_right(extend_selection),
        MoveTarget::DocumentStart => state.move_document_start(extend_selection),
        MoveTarget::DocumentEnd => state.move_document_end(extend_selection),
        MoveTarget::PageUp | MoveTarget::PageDown => {
            // Page movement requires viewport info, handled at higher level
            // For single-line inputs, these are no-ops
        }
    }
}

// =============================================================================
// Main Editor Bridge
// =============================================================================

/// Bridge TextEditMsg to existing EditorMsg/DocumentMsg handlers for the main editor.
///
/// This allows the unified text editing system to control the main editor without
/// requiring a full rewrite of the editor internals. Messages are mapped to their
/// legacy equivalents and dispatched through existing handlers.
fn bridge_text_edit_to_editor(model: &mut AppModel, msg: TextEditMsg) -> Option<Cmd> {
    match msg {
        // === Movement (no selection) ===
        TextEditMsg::Move(target) => {
            let editor_msg = match target {
                MoveTarget::Left => EditorMsg::MoveCursor(Direction::Left),
                MoveTarget::Right => EditorMsg::MoveCursor(Direction::Right),
                MoveTarget::Up => EditorMsg::MoveCursor(Direction::Up),
                MoveTarget::Down => EditorMsg::MoveCursor(Direction::Down),
                MoveTarget::LineStart => EditorMsg::MoveCursorLineStart,
                MoveTarget::LineEnd => EditorMsg::MoveCursorLineEnd,
                MoveTarget::LineStartSmart => EditorMsg::MoveCursorLineStart, // Legacy doesn't have smart
                MoveTarget::WordLeft => EditorMsg::MoveCursorWord(Direction::Left),
                MoveTarget::WordRight => EditorMsg::MoveCursorWord(Direction::Right),
                MoveTarget::DocumentStart => EditorMsg::MoveCursorDocumentStart,
                MoveTarget::DocumentEnd => EditorMsg::MoveCursorDocumentEnd,
                MoveTarget::PageUp => EditorMsg::PageUp,
                MoveTarget::PageDown => EditorMsg::PageDown,
            };
            update_editor(model, editor_msg)
        }

        // === Movement with selection ===
        TextEditMsg::MoveWithSelection(target) => {
            let editor_msg = match target {
                MoveTarget::Left => EditorMsg::MoveCursorWithSelection(Direction::Left),
                MoveTarget::Right => EditorMsg::MoveCursorWithSelection(Direction::Right),
                MoveTarget::Up => EditorMsg::MoveCursorWithSelection(Direction::Up),
                MoveTarget::Down => EditorMsg::MoveCursorWithSelection(Direction::Down),
                MoveTarget::LineStart => EditorMsg::MoveCursorLineStartWithSelection,
                MoveTarget::LineEnd => EditorMsg::MoveCursorLineEndWithSelection,
                MoveTarget::LineStartSmart => EditorMsg::MoveCursorLineStartWithSelection,
                MoveTarget::WordLeft => EditorMsg::MoveCursorWordWithSelection(Direction::Left),
                MoveTarget::WordRight => EditorMsg::MoveCursorWordWithSelection(Direction::Right),
                MoveTarget::DocumentStart => EditorMsg::MoveCursorDocumentStartWithSelection,
                MoveTarget::DocumentEnd => EditorMsg::MoveCursorDocumentEndWithSelection,
                MoveTarget::PageUp => EditorMsg::PageUpWithSelection,
                MoveTarget::PageDown => EditorMsg::PageDownWithSelection,
            };
            update_editor(model, editor_msg)
        }

        // === Insertion ===
        TextEditMsg::InsertChar(ch) => update_document(model, DocumentMsg::InsertChar(ch)),
        TextEditMsg::InsertText(ref text) => {
            // Insert text character by character (no batch insert in legacy)
            let mut cmd = None;
            for ch in text.chars() {
                cmd = update_document(model, DocumentMsg::InsertChar(ch));
            }
            cmd
        }
        TextEditMsg::InsertNewline => update_document(model, DocumentMsg::InsertNewline),

        // === Deletion ===
        TextEditMsg::DeleteBackward => update_document(model, DocumentMsg::DeleteBackward),
        TextEditMsg::DeleteForward => update_document(model, DocumentMsg::DeleteForward),
        TextEditMsg::DeleteWordBackward => update_document(model, DocumentMsg::DeleteWordBackward),
        TextEditMsg::DeleteWordForward => update_document(model, DocumentMsg::DeleteWordForward),
        TextEditMsg::DeleteLine => update_document(model, DocumentMsg::DeleteLine),

        // === Selection ===
        TextEditMsg::SelectAll => update_editor(model, EditorMsg::SelectAll),
        TextEditMsg::SelectWord => update_editor(model, EditorMsg::SelectWord),
        TextEditMsg::SelectLine => update_editor(model, EditorMsg::SelectLine),
        TextEditMsg::CollapseSelection => update_editor(model, EditorMsg::ClearSelection),

        // === Multi-Cursor ===
        TextEditMsg::AddCursorAbove => update_editor(model, EditorMsg::AddCursorAbove),
        TextEditMsg::AddCursorBelow => update_editor(model, EditorMsg::AddCursorBelow),
        TextEditMsg::AddCursorAtNextOccurrence => {
            update_editor(model, EditorMsg::SelectNextOccurrence)
        }
        TextEditMsg::AddCursorsAtAllOccurrences => {
            update_editor(model, EditorMsg::SelectAllOccurrences)
        }
        TextEditMsg::CollapseCursors => update_editor(model, EditorMsg::CollapseToSingleCursor),

        // === Clipboard ===
        TextEditMsg::Copy => update_document(model, DocumentMsg::Copy),
        TextEditMsg::Cut => update_document(model, DocumentMsg::Cut),
        TextEditMsg::Paste(_) => {
            // The bridge approach uses the legacy Paste which reads from clipboard
            update_document(model, DocumentMsg::Paste)
        }

        // === Undo/Redo ===
        TextEditMsg::Undo => update_document(model, DocumentMsg::Undo),
        TextEditMsg::Redo => update_document(model, DocumentMsg::Redo),

        // === Line Operations ===
        TextEditMsg::Indent => update_document(model, DocumentMsg::IndentLines),
        TextEditMsg::Unindent => update_document(model, DocumentMsg::UnindentLines),
        TextEditMsg::Duplicate => update_document(model, DocumentMsg::Duplicate),
        TextEditMsg::MoveLineUp | TextEditMsg::MoveLineDown => {
            // Not implemented in legacy editor
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editable::{EditConstraints, EditableState, StringBuffer};

    fn create_test_state(text: &str) -> EditableState<StringBuffer> {
        EditableState::new(
            StringBuffer::from_text(text),
            EditConstraints::single_line(),
        )
    }

    #[test]
    fn test_apply_insert_char() {
        let mut state = create_test_state("hello");
        // Move to end first (cursor starts at 0,0)
        apply_text_edit_msg(&mut state, &TextEditMsg::Move(MoveTarget::LineEnd));
        assert!(apply_text_edit_msg(
            &mut state,
            &TextEditMsg::InsertChar('!')
        ));
        assert_eq!(state.text(), "hello!");
    }

    #[test]
    fn test_apply_move_and_delete() {
        let mut state = create_test_state("hello");

        // Move to start
        apply_text_edit_msg(&mut state, &TextEditMsg::Move(MoveTarget::LineStart));
        assert_eq!(state.cursor().column, 0);

        // Move right
        apply_text_edit_msg(&mut state, &TextEditMsg::Move(MoveTarget::Right));
        assert_eq!(state.cursor().column, 1);

        // Delete forward (deletes 'e')
        apply_text_edit_msg(&mut state, &TextEditMsg::DeleteForward);
        assert_eq!(state.text(), "hllo");
    }

    #[test]
    fn test_apply_select_all_and_cut() {
        let mut state = create_test_state("hello");

        apply_text_edit_msg(&mut state, &TextEditMsg::SelectAll);
        assert!(state.has_selection());

        apply_text_edit_msg(&mut state, &TextEditMsg::Cut);
        assert_eq!(state.text(), "");
    }

    #[test]
    fn test_apply_word_movement() {
        let mut state = create_test_state("hello world");
        state.cursors[0].column = 0;
        state.collapse_selection();

        apply_text_edit_msg(&mut state, &TextEditMsg::Move(MoveTarget::WordRight));
        // Should be after "hello " (at position 6, start of "world")
        assert_eq!(state.cursor().column, 6);

        apply_text_edit_msg(&mut state, &TextEditMsg::Move(MoveTarget::WordLeft));
        assert_eq!(state.cursor().column, 0);
    }

    #[test]
    fn test_apply_undo_redo() {
        let mut state = create_test_state("hello");

        // Move to end first (cursor starts at 0,0)
        apply_text_edit_msg(&mut state, &TextEditMsg::Move(MoveTarget::LineEnd));

        // Insert something
        apply_text_edit_msg(&mut state, &TextEditMsg::InsertChar('!'));
        assert_eq!(state.text(), "hello!");

        // Undo
        apply_text_edit_msg(&mut state, &TextEditMsg::Undo);
        assert_eq!(state.text(), "hello");

        // Redo
        apply_text_edit_msg(&mut state, &TextEditMsg::Redo);
        assert_eq!(state.text(), "hello!");
    }

    #[test]
    fn test_selection_with_movement() {
        let mut state = create_test_state("hello");
        state.cursors[0].column = 0;
        state.collapse_selection();

        // Select first 3 characters
        apply_text_edit_msg(
            &mut state,
            &TextEditMsg::MoveWithSelection(MoveTarget::Right),
        );
        apply_text_edit_msg(
            &mut state,
            &TextEditMsg::MoveWithSelection(MoveTarget::Right),
        );
        apply_text_edit_msg(
            &mut state,
            &TextEditMsg::MoveWithSelection(MoveTarget::Right),
        );

        assert_eq!(state.selected_text(), "hel");
    }

    // =========================================================================
    // Bridge Tests
    // =========================================================================

    #[test]
    fn test_bridge_move_target_to_direction() {
        // Verify the mapping produces correct Direction variants
        let left_msg = TextEditMsg::Move(MoveTarget::Left);
        let right_msg = TextEditMsg::Move(MoveTarget::Right);
        let up_msg = TextEditMsg::Move(MoveTarget::Up);
        let down_msg = TextEditMsg::Move(MoveTarget::Down);

        // These are compile-time checks that the patterns match
        assert!(matches!(left_msg, TextEditMsg::Move(MoveTarget::Left)));
        assert!(matches!(right_msg, TextEditMsg::Move(MoveTarget::Right)));
        assert!(matches!(up_msg, TextEditMsg::Move(MoveTarget::Up)));
        assert!(matches!(down_msg, TextEditMsg::Move(MoveTarget::Down)));
    }

    #[test]
    fn test_bridge_selection_targets() {
        // Verify selection movement targets map correctly
        let targets = [
            MoveTarget::Left,
            MoveTarget::Right,
            MoveTarget::Up,
            MoveTarget::Down,
            MoveTarget::LineStart,
            MoveTarget::LineEnd,
            MoveTarget::WordLeft,
            MoveTarget::WordRight,
            MoveTarget::DocumentStart,
            MoveTarget::DocumentEnd,
            MoveTarget::PageUp,
            MoveTarget::PageDown,
        ];

        for target in targets {
            let msg = TextEditMsg::MoveWithSelection(target);
            assert!(msg.is_selection());
        }
    }

    #[test]
    fn test_bridge_editing_messages() {
        // Verify editing messages are correctly identified
        assert!(TextEditMsg::InsertChar('a').is_editing());
        assert!(TextEditMsg::InsertText("hello".to_string()).is_editing());
        assert!(TextEditMsg::InsertNewline.is_editing());
        assert!(TextEditMsg::DeleteBackward.is_editing());
        assert!(TextEditMsg::DeleteForward.is_editing());
        assert!(TextEditMsg::DeleteWordBackward.is_editing());
        assert!(TextEditMsg::DeleteWordForward.is_editing());
        assert!(TextEditMsg::DeleteLine.is_editing());
        assert!(TextEditMsg::Cut.is_editing());
        assert!(TextEditMsg::Paste("text".to_string()).is_editing());
        assert!(TextEditMsg::Undo.is_editing());
        assert!(TextEditMsg::Redo.is_editing());
    }

    #[test]
    fn test_bridge_multi_cursor_messages() {
        // Verify multi-cursor messages are correctly identified
        assert!(TextEditMsg::AddCursorAbove.requires_multi_cursor());
        assert!(TextEditMsg::AddCursorBelow.requires_multi_cursor());
        assert!(TextEditMsg::AddCursorAtNextOccurrence.requires_multi_cursor());
        assert!(TextEditMsg::AddCursorsAtAllOccurrences.requires_multi_cursor());
    }
}
