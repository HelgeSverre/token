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
