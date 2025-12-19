//! Message types for the unified text editing system.

/// Target for cursor movement operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveTarget {
    /// Move one character left
    Left,
    /// Move one character right
    Right,
    /// Move one line up
    Up,
    /// Move one line down
    Down,
    /// Move to start of line
    LineStart,
    /// Move to end of line
    LineEnd,
    /// Smart line start (toggle between first non-whitespace and column 0)
    LineStartSmart,
    /// Move one word left
    WordLeft,
    /// Move one word right
    WordRight,
    /// Move to start of document
    DocumentStart,
    /// Move to end of document
    DocumentEnd,
    /// Move one page up
    PageUp,
    /// Move one page down
    PageDown,
}

/// Unified message type for all text editing operations.
#[derive(Debug, Clone, PartialEq)]
pub enum TextEditMsg {
    // === Movement ===
    /// Move cursor without affecting selection
    Move(MoveTarget),
    /// Move cursor and extend selection
    MoveWithSelection(MoveTarget),

    // === Insertion ===
    /// Insert a single character
    InsertChar(char),
    /// Insert a string (e.g., from paste or completion)
    InsertText(String),
    /// Insert a newline (or confirm in single-line context)
    InsertNewline,

    // === Deletion ===
    /// Delete character before cursor (Backspace)
    DeleteBackward,
    /// Delete character after cursor (Delete)
    DeleteForward,
    /// Delete word before cursor (Ctrl/Option+Backspace)
    DeleteWordBackward,
    /// Delete word after cursor (Ctrl/Option+Delete)
    DeleteWordForward,
    /// Delete entire line
    DeleteLine,

    // === Selection ===
    /// Select all text
    SelectAll,
    /// Select current word
    SelectWord,
    /// Select current line
    SelectLine,
    /// Collapse selection to cursor position
    CollapseSelection,

    // === Multi-Cursor (editor only, ignored in single-cursor contexts) ===
    /// Add cursor above current cursor
    AddCursorAbove,
    /// Add cursor below current cursor
    AddCursorBelow,
    /// Add cursor at next occurrence of selected text
    AddCursorAtNextOccurrence,
    /// Add cursors at all occurrences of selected text
    AddCursorsAtAllOccurrences,
    /// Collapse to single cursor
    CollapseCursors,

    // === Clipboard ===
    /// Copy selection to clipboard
    Copy,
    /// Cut selection to clipboard
    Cut,
    /// Paste text from clipboard
    Paste(String),

    // === Undo/Redo ===
    /// Undo last edit
    Undo,
    /// Redo last undone edit
    Redo,

    // === Indentation and Line Operations (editor only) ===
    /// Indent current line(s)
    Indent,
    /// Unindent current line(s)
    Unindent,
    /// Duplicate current line(s)
    Duplicate,
    /// Move current line(s) up
    MoveLineUp,
    /// Move current line(s) down
    MoveLineDown,
}

impl TextEditMsg {
    /// Check if this message modifies the buffer
    pub fn is_editing(&self) -> bool {
        matches!(
            self,
            TextEditMsg::InsertChar(_)
                | TextEditMsg::InsertText(_)
                | TextEditMsg::InsertNewline
                | TextEditMsg::DeleteBackward
                | TextEditMsg::DeleteForward
                | TextEditMsg::DeleteWordBackward
                | TextEditMsg::DeleteWordForward
                | TextEditMsg::DeleteLine
                | TextEditMsg::Cut
                | TextEditMsg::Paste(_)
                | TextEditMsg::Undo
                | TextEditMsg::Redo
                | TextEditMsg::Indent
                | TextEditMsg::Unindent
                | TextEditMsg::Duplicate
                | TextEditMsg::MoveLineUp
                | TextEditMsg::MoveLineDown
        )
    }

    /// Check if this message is a movement operation
    pub fn is_movement(&self) -> bool {
        matches!(
            self,
            TextEditMsg::Move(_) | TextEditMsg::MoveWithSelection(_)
        )
    }

    /// Check if this message is a selection operation
    pub fn is_selection(&self) -> bool {
        matches!(
            self,
            TextEditMsg::MoveWithSelection(_)
                | TextEditMsg::SelectAll
                | TextEditMsg::SelectWord
                | TextEditMsg::SelectLine
        )
    }

    /// Check if this message requires multi-cursor support
    pub fn requires_multi_cursor(&self) -> bool {
        matches!(
            self,
            TextEditMsg::AddCursorAbove
                | TextEditMsg::AddCursorBelow
                | TextEditMsg::AddCursorAtNextOccurrence
                | TextEditMsg::AddCursorsAtAllOccurrences
        )
    }

    /// Check if this message requires multiline support
    pub fn requires_multiline(&self) -> bool {
        matches!(
            self,
            TextEditMsg::InsertNewline
                | TextEditMsg::Move(MoveTarget::Up)
                | TextEditMsg::Move(MoveTarget::Down)
                | TextEditMsg::Move(MoveTarget::PageUp)
                | TextEditMsg::Move(MoveTarget::PageDown)
                | TextEditMsg::Move(MoveTarget::DocumentStart)
                | TextEditMsg::Move(MoveTarget::DocumentEnd)
                | TextEditMsg::MoveWithSelection(MoveTarget::Up)
                | TextEditMsg::MoveWithSelection(MoveTarget::Down)
                | TextEditMsg::MoveWithSelection(MoveTarget::PageUp)
                | TextEditMsg::MoveWithSelection(MoveTarget::PageDown)
                | TextEditMsg::MoveWithSelection(MoveTarget::DocumentStart)
                | TextEditMsg::MoveWithSelection(MoveTarget::DocumentEnd)
                | TextEditMsg::AddCursorAbove
                | TextEditMsg::AddCursorBelow
                | TextEditMsg::MoveLineUp
                | TextEditMsg::MoveLineDown
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_editing() {
        assert!(TextEditMsg::InsertChar('a').is_editing());
        assert!(TextEditMsg::DeleteBackward.is_editing());
        assert!(TextEditMsg::Undo.is_editing());
        assert!(!TextEditMsg::Move(MoveTarget::Left).is_editing());
        assert!(!TextEditMsg::SelectAll.is_editing());
    }

    #[test]
    fn test_is_movement() {
        assert!(TextEditMsg::Move(MoveTarget::Left).is_movement());
        assert!(TextEditMsg::MoveWithSelection(MoveTarget::Right).is_movement());
        assert!(!TextEditMsg::InsertChar('a').is_movement());
    }

    #[test]
    fn test_requires_multi_cursor() {
        assert!(TextEditMsg::AddCursorAbove.requires_multi_cursor());
        assert!(TextEditMsg::AddCursorAtNextOccurrence.requires_multi_cursor());
        assert!(!TextEditMsg::Move(MoveTarget::Left).requires_multi_cursor());
    }

    #[test]
    fn test_requires_multiline() {
        assert!(TextEditMsg::InsertNewline.requires_multiline());
        assert!(TextEditMsg::Move(MoveTarget::Up).requires_multiline());
        assert!(!TextEditMsg::Move(MoveTarget::Left).requires_multiline());
        assert!(!TextEditMsg::InsertChar('a').requires_multiline());
    }
}
