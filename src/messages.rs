//! Message types for the Elm-style architecture
//!
//! All state changes flow through these message types.

use std::path::PathBuf;

/// Direction for cursor movement
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

/// Editor-specific messages (cursor movement, viewport scrolling)
#[derive(Debug, Clone)]
pub enum EditorMsg {
    // === Basic Movement ===
    /// Move cursor in a direction
    MoveCursor(Direction),
    /// Move cursor to start of line (Home key)
    MoveCursorLineStart,
    /// Move cursor to end of line (End key)
    MoveCursorLineEnd,
    /// Move cursor to start of document (Ctrl+Home)
    MoveCursorDocumentStart,
    /// Move cursor to end of document (Ctrl+End)
    MoveCursorDocumentEnd,
    /// Move cursor by word (Option+Left/Right on Mac)
    MoveCursorWord(Direction),
    /// Page up
    PageUp,
    /// Page down
    PageDown,
    /// Set cursor to specific position (from mouse click)
    SetCursorPosition { line: usize, column: usize },
    /// Scroll viewport vertically (positive = down, negative = up)
    Scroll(i32),
    /// Scroll viewport horizontally (positive = right, negative = left)
    ScrollHorizontal(i32),

    // === Selection Movement (Shift+key) ===
    /// Move cursor with selection (Shift+Arrow)
    MoveCursorWithSelection(Direction),
    /// Move to line start with selection (Shift+Home)
    MoveCursorLineStartWithSelection,
    /// Move to line end with selection (Shift+End)
    MoveCursorLineEndWithSelection,
    /// Move to document start with selection (Shift+Ctrl+Home)
    MoveCursorDocumentStartWithSelection,
    /// Move to document end with selection (Shift+Ctrl+End)
    MoveCursorDocumentEndWithSelection,
    /// Move word with selection (Shift+Option+Arrow)
    MoveCursorWordWithSelection(Direction),
    /// Page up with selection
    PageUpWithSelection,
    /// Page down with selection
    PageDownWithSelection,

    // === Selection Commands ===
    /// Select all text (Cmd+A)
    SelectAll,
    /// Select word at cursor (double-click)
    SelectWord,
    /// Select entire line (triple-click)
    SelectLine,
    /// Extend selection to position (Shift+Click)
    ExtendSelectionToPosition { line: usize, column: usize },
    /// Clear all selections (collapse to cursors)
    ClearSelection,

    // === Multi-Cursor ===
    /// Toggle cursor at position (Option+Click)
    ToggleCursorAtPosition { line: usize, column: usize },
    /// Add cursor above current (Option+Option+Up)
    AddCursorAbove,
    /// Add cursor below current (Option+Option+Down)
    AddCursorBelow,
    /// Collapse to single cursor (Escape with multiple cursors)
    CollapseToSingleCursor,
    /// Remove cursor by index
    RemoveCursor(usize),

    // === Find & Select ===
    /// Select next occurrence of word/selection (Cmd+J)
    SelectNextOccurrence,
    /// Unselect last added occurrence (Shift+Cmd+J)
    UnselectOccurrence,
    /// Select all occurrences (Cmd+Shift+L)
    SelectAllOccurrences,

    // === Expand/Shrink Selection ===
    /// Expand selection to next semantic level (Option+Up)
    /// Progression: cursor → word → line → all
    ExpandSelection,
    /// Shrink selection to previous level (Option+Down)
    /// Restores previous selection from history stack
    ShrinkSelection,

    // === Rectangle Selection (Middle mouse) ===
    /// Start rectangle selection at position (visual column = screen position)
    StartRectangleSelection { line: usize, visual_col: usize },
    /// Update rectangle selection to position (visual column = screen position)
    UpdateRectangleSelection { line: usize, visual_col: usize },
    /// Finish rectangle selection
    FinishRectangleSelection,
    /// Cancel rectangle selection
    CancelRectangleSelection,
}

/// Document-specific messages (text editing, undo/redo)
#[derive(Debug, Clone)]
pub enum DocumentMsg {
    /// Insert a character at cursor
    InsertChar(char),
    /// Insert a newline at cursor
    InsertNewline,
    /// Delete character before cursor (Backspace)
    DeleteBackward,
    /// Delete word before cursor (Option+Backspace)
    DeleteWordBackward,
    /// Delete word after cursor (Option+Delete)
    DeleteWordForward,
    /// Delete character at cursor (Delete)
    DeleteForward,
    /// Delete entire current line (Cmd+Backspace)
    DeleteLine,
    /// Undo last edit
    Undo,
    /// Redo last undone edit
    Redo,
    /// Copy selection to clipboard (Cmd+C)
    Copy,
    /// Cut selection to clipboard (Cmd+X)
    Cut,
    /// Paste from clipboard (Cmd+V)
    Paste,
    /// Duplicate current line or selection (Cmd+D)
    Duplicate,
    /// Indent selected lines (Tab with selection)
    IndentLines,
    /// Unindent current line or selected lines (Shift+Tab)
    UnindentLines,
}

use crate::model::{GroupId, ModalId, SegmentContent, SegmentId, SplitDirection, TabId};

/// Modal-specific messages (command palette, goto line, find/replace)
#[derive(Debug, Clone)]
pub enum ModalMsg {
    /// Open command palette
    OpenCommandPalette,
    /// Open goto line dialog
    OpenGotoLine,
    /// Open find/replace dialog
    OpenFindReplace,
    /// Close the currently active modal
    Close,
    /// Update modal input text
    SetInput(String),
    /// Insert character into modal input
    InsertChar(char),
    /// Delete character from modal input (backspace)
    DeleteBackward,
    /// Delete word backward from modal input (Option+Backspace)
    DeleteWordBackward,
    /// Move cursor word left in modal input (Option+Left)
    MoveCursorWordLeft,
    /// Move cursor word right in modal input (Option+Right)
    MoveCursorWordRight,
    /// Move selection up in list (e.g., command palette results)
    SelectPrevious,
    /// Move selection down in list
    SelectNext,
    /// Confirm/execute the modal action (Enter)
    Confirm,
}

/// UI-specific messages (status bar, cursor blink, modals)
#[derive(Debug, Clone)]
pub enum UiMsg {
    /// Set status bar message (legacy, for backward compatibility)
    SetStatus(String),
    /// Toggle cursor blink state
    BlinkCursor,
    /// Update a specific status bar segment
    UpdateSegment {
        id: SegmentId,
        content: SegmentContent,
    },
    /// Set a transient message that auto-expires
    SetTransientMessage { text: String, duration_ms: u64 },
    /// Clear the transient message
    ClearTransientMessage,
    /// Modal messages
    Modal(ModalMsg),
    /// Toggle a modal (open if closed, close if open)
    ToggleModal(ModalId),

    // === File Drag-and-Drop ===
    /// File is being hovered over the window
    FileHovered(PathBuf),
    /// Hover was cancelled (dragged away from window)
    FileHoverCancelled,
}

/// Layout messages (split views, tabs, groups)
#[derive(Debug, Clone)]
pub enum LayoutMsg {
    /// Create a new untitled document in the focused group
    NewTab,

    /// Open a file in a new tab in the focused group
    OpenFileInNewTab(PathBuf),

    /// Split the focused group in the given direction
    /// Creates a new group with a copy of the current editor view
    SplitFocused(SplitDirection),

    /// Split a specific group in the given direction
    SplitGroup {
        group_id: GroupId,
        direction: SplitDirection,
    },

    /// Close a group (and all its tabs)
    /// If this is the last group, does nothing
    CloseGroup(GroupId),

    /// Close the focused group
    CloseFocusedGroup,

    /// Focus a specific group
    FocusGroup(GroupId),

    /// Focus the next group (cycle through groups)
    FocusNextGroup,

    /// Focus the previous group (cycle through groups)
    FocusPrevGroup,

    /// Focus group by index (1-indexed for keyboard shortcuts)
    FocusGroupByIndex(usize),

    /// Move a tab to a different group
    MoveTab { tab_id: TabId, to_group: GroupId },

    /// Close a specific tab
    CloseTab(TabId),

    /// Close the active tab in the focused group
    CloseFocusedTab,

    /// Switch to next tab in focused group
    NextTab,

    /// Switch to previous tab in focused group
    PrevTab,

    /// Switch to tab by index in focused group (0-indexed)
    SwitchToTab(usize),
}

/// Application-level messages (file operations, window events)
#[derive(Debug, Clone)]
pub enum AppMsg {
    /// Window resized
    Resize(u32, u32),
    /// Display scale factor changed (e.g., moving between monitors)
    ScaleFactorChanged(f64),
    /// Save current file
    SaveFile,
    /// Load a file
    LoadFile(PathBuf),
    /// Create a new file
    NewFile,
    /// File save completed (async result)
    SaveCompleted(Result<(), String>),
    /// File load completed (async result)
    FileLoaded {
        path: PathBuf,
        result: Result<String, String>,
    },
    /// Quit the application
    Quit,

    // === File Dialog Messages ===
    /// User requested "Save As..." dialog
    SaveFileAs,
    /// Save As dialog returned a path (or None if cancelled)
    SaveFileAsDialogResult { path: Option<PathBuf> },

    /// User requested "Open File..." dialog
    OpenFileDialog,
    /// Open File dialog returned paths (empty if cancelled)
    OpenFileDialogResult { paths: Vec<PathBuf> },

    /// User requested "Open Folder..." dialog
    OpenFolderDialog,
    /// Open Folder dialog returned folder (or None if cancelled)
    OpenFolderDialogResult { folder: Option<PathBuf> },
}

/// Syntax highlighting messages
#[derive(Debug, Clone)]
pub enum SyntaxMsg {
    /// Parse is ready to be performed (after debounce delay)
    ParseReady {
        document_id: crate::model::editor_area::DocumentId,
        revision: u64,
    },
    /// Parse has completed with results
    ParseCompleted {
        document_id: crate::model::editor_area::DocumentId,
        revision: u64,
        highlights: crate::syntax::SyntaxHighlights,
    },
    /// Language changed for a document (triggers re-parse)
    LanguageChanged {
        document_id: crate::model::editor_area::DocumentId,
        language: crate::syntax::LanguageId,
    },
}

/// Top-level message type
#[derive(Debug, Clone)]
pub enum Msg {
    /// Editor messages (cursor, viewport)
    Editor(EditorMsg),
    /// Document messages (text editing)
    Document(DocumentMsg),
    /// UI messages (status, animation)
    Ui(UiMsg),
    /// Layout messages (splits, tabs, groups)
    Layout(LayoutMsg),
    /// App messages (file I/O, window)
    App(AppMsg),
    /// Syntax highlighting messages
    Syntax(SyntaxMsg),
}

// Convenience constructors for common messages
impl Msg {
    /// Create a cursor movement message
    pub fn move_cursor(direction: Direction) -> Self {
        Msg::Editor(EditorMsg::MoveCursor(direction))
    }

    /// Create an insert character message
    pub fn insert_char(ch: char) -> Self {
        Msg::Document(DocumentMsg::InsertChar(ch))
    }

    /// Create a resize message
    pub fn resize(width: u32, height: u32) -> Self {
        Msg::App(AppMsg::Resize(width, height))
    }
}
