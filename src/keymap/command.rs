//! Command enum representing all executable editor actions
//!
//! Commands are the bridge between keybindings and the message system.
//! Each command maps to one or more `Msg` values for the Elm-style update loop.

use crate::messages::{
    AppMsg, CsvMsg, Direction, DocumentMsg, EditorMsg, LayoutMsg, Msg, UiMsg, WorkspaceMsg,
};
use crate::model::editor_area::SplitDirection;
use crate::model::ModalId;

/// All executable editor commands that can be bound to keys
///
/// This enum covers every action that can be triggered via keyboard.
/// Commands are organized by category for clarity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Command {
    // ========================================================================
    // Cursor Movement (no selection)
    // ========================================================================
    /// Move cursor up one line
    MoveCursorUp,
    /// Move cursor down one line
    MoveCursorDown,
    /// Move cursor left one character
    MoveCursorLeft,
    /// Move cursor right one character
    MoveCursorRight,
    /// Move cursor to start of line
    MoveCursorLineStart,
    /// Move cursor to end of line
    MoveCursorLineEnd,
    /// Move cursor to start of document
    MoveCursorDocumentStart,
    /// Move cursor to end of document
    MoveCursorDocumentEnd,
    /// Move cursor left by word
    MoveCursorWordLeft,
    /// Move cursor right by word
    MoveCursorWordRight,
    /// Move cursor up by page
    PageUp,
    /// Move cursor down by page
    PageDown,

    // ========================================================================
    // Selection Movement (extend selection)
    // ========================================================================
    /// Move cursor up, extending selection
    MoveCursorUpWithSelection,
    /// Move cursor down, extending selection
    MoveCursorDownWithSelection,
    /// Move cursor left, extending selection
    MoveCursorLeftWithSelection,
    /// Move cursor right, extending selection
    MoveCursorRightWithSelection,
    /// Move to line start, extending selection
    MoveCursorLineStartWithSelection,
    /// Move to line end, extending selection
    MoveCursorLineEndWithSelection,
    /// Move to document start, extending selection
    MoveCursorDocumentStartWithSelection,
    /// Move to document end, extending selection
    MoveCursorDocumentEndWithSelection,
    /// Move left by word, extending selection
    MoveCursorWordLeftWithSelection,
    /// Move right by word, extending selection
    MoveCursorWordRightWithSelection,
    /// Page up, extending selection
    PageUpWithSelection,
    /// Page down, extending selection
    PageDownWithSelection,

    // ========================================================================
    // Selection Commands
    // ========================================================================
    /// Select all text in document
    SelectAll,
    /// Select current word
    SelectWord,
    /// Select current line
    SelectLine,
    /// Clear selection (collapse to cursor)
    ClearSelection,
    /// Expand selection to larger semantic unit
    ExpandSelection,
    /// Shrink selection to smaller semantic unit
    ShrinkSelection,

    // ========================================================================
    // Multi-Cursor
    // ========================================================================
    /// Add a cursor above the current cursor
    AddCursorAbove,
    /// Add a cursor below the current cursor
    AddCursorBelow,
    /// Collapse to single cursor (remove all secondary cursors)
    CollapseToSingleCursor,
    /// Select next occurrence of current word/selection
    SelectNextOccurrence,
    /// Unselect the last added occurrence
    UnselectOccurrence,

    // ========================================================================
    // Text Editing
    // ========================================================================
    /// Insert a newline at cursor
    InsertNewline,
    /// Delete character before cursor (backspace)
    DeleteBackward,
    /// Delete character at cursor (delete)
    DeleteForward,
    /// Delete word before cursor
    DeleteWordBackward,
    /// Delete word after cursor
    DeleteWordForward,
    /// Delete entire line
    DeleteLine,
    /// Duplicate current line or selection
    Duplicate,
    /// Indent selected lines
    IndentLines,
    /// Unindent selected lines
    UnindentLines,
    /// Insert a tab character (when no selection)
    InsertTab,

    // ========================================================================
    // Clipboard
    // ========================================================================
    /// Copy selection to clipboard
    Copy,
    /// Cut selection to clipboard
    Cut,
    /// Paste from clipboard
    Paste,

    // ========================================================================
    // Undo/Redo
    // ========================================================================
    /// Undo last change
    Undo,
    /// Redo last undone change
    Redo,

    // ========================================================================
    // File Operations
    // ========================================================================
    /// Save current file
    SaveFile,
    /// Save current file with new name
    SaveFileAs,
    /// Open file dialog
    OpenFile,
    /// Open folder dialog
    OpenFolder,
    /// Create new file
    NewFile,
    /// Quit application
    Quit,

    // ========================================================================
    // Modals/Dialogs
    // ========================================================================
    /// Toggle command palette
    ToggleCommandPalette,
    /// Toggle goto line dialog
    ToggleGotoLine,
    /// Toggle find/replace dialog
    ToggleFindReplace,

    // ========================================================================
    // Layout (Tabs/Splits)
    // ========================================================================
    /// Create new tab
    NewTab,
    /// Close current tab
    CloseTab,
    /// Switch to next tab
    NextTab,
    /// Switch to previous tab
    PrevTab,
    /// Split editor horizontally (side by side)
    SplitHorizontal,
    /// Split editor vertically (stacked)
    SplitVertical,
    /// Focus next editor group
    FocusNextGroup,
    /// Focus previous editor group
    FocusPrevGroup,
    /// Focus editor group by index (1-4)
    FocusGroup1,
    FocusGroup2,
    FocusGroup3,
    FocusGroup4,

    // ========================================================================
    // Workspace (Sidebar/File Tree)
    // ========================================================================
    /// Toggle sidebar visibility
    ToggleSidebar,
    /// Reveal active file in sidebar
    RevealInSidebar,
    /// Select previous item in file tree
    FileTreeSelectPrevious,
    /// Select next item in file tree
    FileTreeSelectNext,
    /// Open selected file or toggle folder in file tree
    FileTreeOpenOrToggle,
    /// Refresh the file tree from disk
    FileTreeRefresh,

    // ========================================================================
    // Special
    // ========================================================================
    /// Escape key behavior: collapse multi-cursor, then clear selection
    EscapeSmartClear,
    /// Explicitly unbound - disables a default binding
    Unbound,

    // ========================================================================
    // CSV Mode
    // ========================================================================
    /// Toggle CSV view mode
    CsvToggle,
    /// CSV navigation commands (used when csv_mode context is active)
    CsvMoveUp,
    CsvMoveDown,
    CsvMoveLeft,
    CsvMoveRight,
    CsvNextCell,
    CsvPrevCell,
    CsvFirstCell,
    CsvLastCell,
    CsvRowStart,
    CsvRowEnd,
    CsvPageUp,
    CsvPageDown,
    CsvExit,
}

impl Command {
    /// Convert this command to message(s) for the Elm update loop
    ///
    /// Some commands map to multiple messages or require context to decide.
    /// Returns a Vec to handle compound commands.
    pub fn to_msgs(self) -> Vec<Msg> {
        use Command::*;

        match self {
            // Cursor movement
            MoveCursorUp => vec![Msg::Editor(EditorMsg::MoveCursor(Direction::Up))],
            MoveCursorDown => vec![Msg::Editor(EditorMsg::MoveCursor(Direction::Down))],
            MoveCursorLeft => vec![Msg::Editor(EditorMsg::MoveCursor(Direction::Left))],
            MoveCursorRight => vec![Msg::Editor(EditorMsg::MoveCursor(Direction::Right))],
            MoveCursorLineStart => vec![Msg::Editor(EditorMsg::MoveCursorLineStart)],
            MoveCursorLineEnd => vec![Msg::Editor(EditorMsg::MoveCursorLineEnd)],
            MoveCursorDocumentStart => vec![Msg::Editor(EditorMsg::MoveCursorDocumentStart)],
            MoveCursorDocumentEnd => vec![Msg::Editor(EditorMsg::MoveCursorDocumentEnd)],
            MoveCursorWordLeft => {
                vec![Msg::Editor(EditorMsg::MoveCursorWord(Direction::Left))]
            }
            MoveCursorWordRight => {
                vec![Msg::Editor(EditorMsg::MoveCursorWord(Direction::Right))]
            }
            PageUp => vec![Msg::Editor(EditorMsg::PageUp)],
            PageDown => vec![Msg::Editor(EditorMsg::PageDown)],

            // Selection movement
            MoveCursorUpWithSelection => {
                vec![Msg::Editor(EditorMsg::MoveCursorWithSelection(
                    Direction::Up,
                ))]
            }
            MoveCursorDownWithSelection => {
                vec![Msg::Editor(EditorMsg::MoveCursorWithSelection(
                    Direction::Down,
                ))]
            }
            MoveCursorLeftWithSelection => {
                vec![Msg::Editor(EditorMsg::MoveCursorWithSelection(
                    Direction::Left,
                ))]
            }
            MoveCursorRightWithSelection => {
                vec![Msg::Editor(EditorMsg::MoveCursorWithSelection(
                    Direction::Right,
                ))]
            }
            MoveCursorLineStartWithSelection => {
                vec![Msg::Editor(EditorMsg::MoveCursorLineStartWithSelection)]
            }
            MoveCursorLineEndWithSelection => {
                vec![Msg::Editor(EditorMsg::MoveCursorLineEndWithSelection)]
            }
            MoveCursorDocumentStartWithSelection => {
                vec![Msg::Editor(EditorMsg::MoveCursorDocumentStartWithSelection)]
            }
            MoveCursorDocumentEndWithSelection => {
                vec![Msg::Editor(EditorMsg::MoveCursorDocumentEndWithSelection)]
            }
            MoveCursorWordLeftWithSelection => vec![Msg::Editor(
                EditorMsg::MoveCursorWordWithSelection(Direction::Left),
            )],
            MoveCursorWordRightWithSelection => vec![Msg::Editor(
                EditorMsg::MoveCursorWordWithSelection(Direction::Right),
            )],
            PageUpWithSelection => vec![Msg::Editor(EditorMsg::PageUpWithSelection)],
            PageDownWithSelection => vec![Msg::Editor(EditorMsg::PageDownWithSelection)],

            // Selection commands
            SelectAll => vec![Msg::Editor(EditorMsg::SelectAll)],
            SelectWord => vec![Msg::Editor(EditorMsg::SelectWord)],
            SelectLine => vec![Msg::Editor(EditorMsg::SelectLine)],
            ClearSelection => vec![Msg::Editor(EditorMsg::ClearSelection)],
            ExpandSelection => vec![Msg::Editor(EditorMsg::ExpandSelection)],
            ShrinkSelection => vec![Msg::Editor(EditorMsg::ShrinkSelection)],

            // Multi-cursor
            AddCursorAbove => vec![Msg::Editor(EditorMsg::AddCursorAbove)],
            AddCursorBelow => vec![Msg::Editor(EditorMsg::AddCursorBelow)],
            CollapseToSingleCursor => vec![Msg::Editor(EditorMsg::CollapseToSingleCursor)],
            SelectNextOccurrence => vec![Msg::Editor(EditorMsg::SelectNextOccurrence)],
            UnselectOccurrence => vec![Msg::Editor(EditorMsg::UnselectOccurrence)],

            // Text editing
            InsertNewline => vec![Msg::Document(DocumentMsg::InsertNewline)],
            DeleteBackward => vec![Msg::Document(DocumentMsg::DeleteBackward)],
            DeleteForward => vec![Msg::Document(DocumentMsg::DeleteForward)],
            DeleteWordBackward => vec![Msg::Document(DocumentMsg::DeleteWordBackward)],
            DeleteWordForward => vec![Msg::Document(DocumentMsg::DeleteWordForward)],
            DeleteLine => vec![Msg::Document(DocumentMsg::DeleteLine)],
            Duplicate => vec![Msg::Document(DocumentMsg::Duplicate)],
            IndentLines => vec![Msg::Document(DocumentMsg::IndentLines)],
            UnindentLines => vec![Msg::Document(DocumentMsg::UnindentLines)],
            InsertTab => vec![Msg::Document(DocumentMsg::InsertChar('\t'))],

            // Clipboard
            Copy => vec![Msg::Document(DocumentMsg::Copy)],
            Cut => vec![Msg::Document(DocumentMsg::Cut)],
            Paste => vec![Msg::Document(DocumentMsg::Paste)],

            // Undo/Redo
            Undo => vec![Msg::Document(DocumentMsg::Undo)],
            Redo => vec![Msg::Document(DocumentMsg::Redo)],

            // File operations
            SaveFile => vec![Msg::App(AppMsg::SaveFile)],
            SaveFileAs => vec![Msg::App(AppMsg::SaveFileAs)],
            OpenFile => vec![Msg::App(AppMsg::OpenFileDialog)],
            OpenFolder => vec![Msg::App(AppMsg::OpenFolderDialog)],
            NewFile => vec![Msg::App(AppMsg::NewFile)],
            Quit => vec![Msg::App(AppMsg::Quit)],

            // Modals
            ToggleCommandPalette => {
                vec![Msg::Ui(UiMsg::ToggleModal(ModalId::CommandPalette))]
            }
            ToggleGotoLine => vec![Msg::Ui(UiMsg::ToggleModal(ModalId::GotoLine))],
            ToggleFindReplace => vec![Msg::Ui(UiMsg::ToggleModal(ModalId::FindReplace))],

            // Layout
            NewTab => vec![Msg::Layout(LayoutMsg::NewTab)],
            CloseTab => vec![Msg::Layout(LayoutMsg::CloseFocusedTab)],
            NextTab => vec![Msg::Layout(LayoutMsg::NextTab)],
            PrevTab => vec![Msg::Layout(LayoutMsg::PrevTab)],
            SplitHorizontal => {
                vec![Msg::Layout(LayoutMsg::SplitFocused(
                    SplitDirection::Horizontal,
                ))]
            }
            SplitVertical => {
                vec![Msg::Layout(LayoutMsg::SplitFocused(
                    SplitDirection::Vertical,
                ))]
            }
            FocusNextGroup => vec![Msg::Layout(LayoutMsg::FocusNextGroup)],
            FocusPrevGroup => vec![Msg::Layout(LayoutMsg::FocusPrevGroup)],
            FocusGroup1 => vec![Msg::Layout(LayoutMsg::FocusGroupByIndex(1))],
            FocusGroup2 => vec![Msg::Layout(LayoutMsg::FocusGroupByIndex(2))],
            FocusGroup3 => vec![Msg::Layout(LayoutMsg::FocusGroupByIndex(3))],
            FocusGroup4 => vec![Msg::Layout(LayoutMsg::FocusGroupByIndex(4))],

            // Workspace
            ToggleSidebar => vec![Msg::Workspace(WorkspaceMsg::ToggleSidebar)],
            RevealInSidebar => vec![Msg::Workspace(WorkspaceMsg::RevealActiveFile)],
            FileTreeSelectPrevious => vec![Msg::Workspace(WorkspaceMsg::SelectPrevious)],
            FileTreeSelectNext => vec![Msg::Workspace(WorkspaceMsg::SelectNext)],
            FileTreeOpenOrToggle => vec![Msg::Workspace(WorkspaceMsg::OpenOrToggle)],
            FileTreeRefresh => vec![Msg::Workspace(WorkspaceMsg::Refresh)],

            // Special - these need context-aware handling
            EscapeSmartClear => {
                // This is handled specially in the keymap dispatch
                // because it needs to check model state
                vec![]
            }
            Unbound => vec![], // Explicitly does nothing

            // CSV mode
            CsvToggle => vec![Msg::Csv(CsvMsg::Toggle)],
            CsvMoveUp => vec![Msg::Csv(CsvMsg::MoveUp)],
            CsvMoveDown => vec![Msg::Csv(CsvMsg::MoveDown)],
            CsvMoveLeft => vec![Msg::Csv(CsvMsg::MoveLeft)],
            CsvMoveRight => vec![Msg::Csv(CsvMsg::MoveRight)],
            CsvNextCell => vec![Msg::Csv(CsvMsg::NextCell)],
            CsvPrevCell => vec![Msg::Csv(CsvMsg::PrevCell)],
            CsvFirstCell => vec![Msg::Csv(CsvMsg::FirstCell)],
            CsvLastCell => vec![Msg::Csv(CsvMsg::LastCell)],
            CsvRowStart => vec![Msg::Csv(CsvMsg::RowStart)],
            CsvRowEnd => vec![Msg::Csv(CsvMsg::RowEnd)],
            CsvPageUp => vec![Msg::Csv(CsvMsg::PageUp)],
            CsvPageDown => vec![Msg::Csv(CsvMsg::PageDown)],
            CsvExit => vec![Msg::Csv(CsvMsg::Exit)],
        }
    }

    /// Check if this command is "simple" (doesn't need context)
    ///
    /// Simple commands can be dispatched directly without checking model state.
    /// Complex commands (like Escape behavior) need special handling.
    pub fn is_simple(self) -> bool {
        !matches!(self, Command::EscapeSmartClear)
    }

    /// Global commands work regardless of focus state (sidebar, CSV editing, etc.)
    ///
    /// These are typically modal toggles and app-level actions that should
    /// always be available.
    pub fn is_global(self) -> bool {
        matches!(
            self,
            Command::ToggleCommandPalette
                | Command::ToggleGotoLine
                | Command::ToggleFindReplace
                | Command::ToggleSidebar
                | Command::Quit
                | Command::SaveFile
                | Command::NewTab
                | Command::CloseTab
        )
    }

    /// Get a display name for this command (for command palette, etc.)
    pub fn display_name(self) -> &'static str {
        use Command::*;

        match self {
            MoveCursorUp => "Move Cursor Up",
            MoveCursorDown => "Move Cursor Down",
            MoveCursorLeft => "Move Cursor Left",
            MoveCursorRight => "Move Cursor Right",
            MoveCursorLineStart => "Move to Line Start",
            MoveCursorLineEnd => "Move to Line End",
            MoveCursorDocumentStart => "Move to Document Start",
            MoveCursorDocumentEnd => "Move to Document End",
            MoveCursorWordLeft => "Move Word Left",
            MoveCursorWordRight => "Move Word Right",
            PageUp => "Page Up",
            PageDown => "Page Down",

            MoveCursorUpWithSelection => "Select Up",
            MoveCursorDownWithSelection => "Select Down",
            MoveCursorLeftWithSelection => "Select Left",
            MoveCursorRightWithSelection => "Select Right",
            MoveCursorLineStartWithSelection => "Select to Line Start",
            MoveCursorLineEndWithSelection => "Select to Line End",
            MoveCursorDocumentStartWithSelection => "Select to Document Start",
            MoveCursorDocumentEndWithSelection => "Select to Document End",
            MoveCursorWordLeftWithSelection => "Select Word Left",
            MoveCursorWordRightWithSelection => "Select Word Right",
            PageUpWithSelection => "Select Page Up",
            PageDownWithSelection => "Select Page Down",

            SelectAll => "Select All",
            SelectWord => "Select Word",
            SelectLine => "Select Line",
            ClearSelection => "Clear Selection",
            ExpandSelection => "Expand Selection",
            ShrinkSelection => "Shrink Selection",

            AddCursorAbove => "Add Cursor Above",
            AddCursorBelow => "Add Cursor Below",
            CollapseToSingleCursor => "Single Cursor",
            SelectNextOccurrence => "Select Next Occurrence",
            UnselectOccurrence => "Unselect Occurrence",

            InsertNewline => "Insert Newline",
            DeleteBackward => "Delete Backward",
            DeleteForward => "Delete Forward",
            DeleteWordBackward => "Delete Word Backward",
            DeleteWordForward => "Delete Word Forward",
            DeleteLine => "Delete Line",
            Duplicate => "Duplicate Line",
            IndentLines => "Indent",
            UnindentLines => "Unindent",
            InsertTab => "Insert Tab",

            Copy => "Copy",
            Cut => "Cut",
            Paste => "Paste",

            Undo => "Undo",
            Redo => "Redo",

            SaveFile => "Save File",
            SaveFileAs => "Save File As",
            OpenFile => "Open File",
            OpenFolder => "Open Folder",
            NewFile => "New File",
            Quit => "Quit",

            ToggleCommandPalette => "Command Palette",
            ToggleGotoLine => "Go to Line",
            ToggleFindReplace => "Find and Replace",

            NewTab => "New Tab",
            CloseTab => "Close Tab",
            NextTab => "Next Tab",
            PrevTab => "Previous Tab",
            SplitHorizontal => "Split Right",
            SplitVertical => "Split Down",
            FocusNextGroup => "Focus Next Group",
            FocusPrevGroup => "Focus Previous Group",
            FocusGroup1 => "Focus Group 1",
            FocusGroup2 => "Focus Group 2",
            FocusGroup3 => "Focus Group 3",
            FocusGroup4 => "Focus Group 4",

            ToggleSidebar => "Toggle Sidebar",
            RevealInSidebar => "Reveal in Sidebar",
            FileTreeSelectPrevious => "File Tree: Select Previous",
            FileTreeSelectNext => "File Tree: Select Next",
            FileTreeOpenOrToggle => "File Tree: Open/Toggle",
            FileTreeRefresh => "File Tree: Refresh",

            EscapeSmartClear => "Escape",
            Unbound => "Unbound",

            CsvToggle => "Toggle CSV View",
            CsvMoveUp => "CSV Move Up",
            CsvMoveDown => "CSV Move Down",
            CsvMoveLeft => "CSV Move Left",
            CsvMoveRight => "CSV Move Right",
            CsvNextCell => "CSV Next Cell",
            CsvPrevCell => "CSV Previous Cell",
            CsvFirstCell => "CSV First Cell",
            CsvLastCell => "CSV Last Cell",
            CsvRowStart => "CSV Row Start",
            CsvRowEnd => "CSV Row End",
            CsvPageUp => "CSV Page Up",
            CsvPageDown => "CSV Page Down",
            CsvExit => "Exit CSV View",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_to_msgs_simple() {
        let msgs = Command::Undo.to_msgs();
        assert_eq!(msgs.len(), 1);
        assert!(matches!(msgs[0], Msg::Document(DocumentMsg::Undo)));
    }

    #[test]
    fn test_command_to_msgs_movement() {
        let msgs = Command::MoveCursorUp.to_msgs();
        assert_eq!(msgs.len(), 1);
        assert!(matches!(
            msgs[0],
            Msg::Editor(EditorMsg::MoveCursor(Direction::Up))
        ));
    }

    #[test]
    fn test_command_unbound_empty() {
        let msgs = Command::Unbound.to_msgs();
        assert!(msgs.is_empty());
    }

    #[test]
    fn test_command_is_simple() {
        assert!(Command::Undo.is_simple());
        assert!(Command::SaveFile.is_simple());
        assert!(!Command::EscapeSmartClear.is_simple());
    }
}
