//! Message types for the Elm-style architecture
//!
//! All state changes flow through these message types.

use std::path::PathBuf;

use crate::editable::{EditContext, TextEditMsg};
use crate::lsp::{LspServerId, ServerState};

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
    /// Insert a string at cursor as a single atomic operation (one rope
    /// edit, one undo record). Preferred over looping `InsertChar` for
    /// multi-char inserts (e.g. paste or IME commit).
    InsertText(String),
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
    /// Delete character after cursor (delete)
    DeleteForward,
    /// Delete word backward from modal input (Option+Backspace)
    DeleteWordBackward,

    // === Cursor Movement ===
    /// Move cursor left one character
    MoveCursorLeft,
    /// Move cursor right one character
    MoveCursorRight,
    /// Move cursor to start of line (Home)
    MoveCursorHome,
    /// Move cursor to end of line (End)
    MoveCursorEnd,
    /// Move cursor word left in modal input (Option+Left)
    MoveCursorWordLeft,
    /// Move cursor word right in modal input (Option+Right)
    MoveCursorWordRight,

    // === Selection Movement ===
    /// Move cursor left with selection (Shift+Left)
    MoveCursorLeftWithSelection,
    /// Move cursor right with selection (Shift+Right)
    MoveCursorRightWithSelection,
    /// Move cursor to start with selection (Shift+Home)
    MoveCursorHomeWithSelection,
    /// Move cursor to end with selection (Shift+End)
    MoveCursorEndWithSelection,
    /// Move cursor word left with selection (Shift+Option+Left)
    MoveCursorWordLeftWithSelection,
    /// Move cursor word right with selection (Shift+Option+Right)
    MoveCursorWordRightWithSelection,
    /// Select all text in modal input (Cmd+A)
    SelectAll,

    // === Clipboard ===
    /// Copy selection to clipboard (Cmd+C)
    Copy,
    /// Cut selection to clipboard (Cmd+X)
    Cut,
    /// Paste from clipboard (Cmd+V)
    Paste,
    /// Paste given text into the modal input
    PasteText(String),

    // === List Navigation ===
    /// Move selection up in list (e.g., command palette results)
    SelectPrevious,
    /// Move selection down in list
    SelectNext,
    /// Page selection up by a full visible page (command palette)
    PageUp,
    /// Page selection down by a full visible page (command palette)
    PageDown,
    /// Confirm/execute the modal action (Enter)
    Confirm,
    /// Set selection to a specific row and confirm in one step (row click).
    ActivateRow(usize),
    /// Toggle the pinned flag on the selected row (Recent Files, Commands
    /// tab: `⌘.`).
    TogglePin,
    /// Search Everywhere: switch to the next tab, skipping `Unavailable`
    /// ones (⇥).
    NextTab,
    /// Search Everywhere: switch to the previous tab (⇧⇥).
    PrevTab,
    /// Search Everywhere: switch to tab `index` (`SearchTab::ORDER` order) —
    /// a no-op if that tab is `Unavailable` (tab click).
    ActivateTab(usize),
    /// Move the scroll window by `delta` rows without moving selection
    /// (mouse wheel over a list-body modal).
    Scroll(isize),

    // === Find/Replace Specific ===
    /// Toggle between query and replace fields (Tab)
    ToggleFindReplaceField,
    /// Toggle case sensitivity for find/replace
    ToggleFindReplaceCaseSensitive,
    /// Toggle whole-word matching for find/replace
    ToggleFindReplaceWholeWord,
    /// Toggle regex interpretation for find/replace
    ToggleFindReplaceRegex,
    /// Find next occurrence (Enter in find field or F3)
    FindNext,
    /// Find previous occurrence (Shift+Enter or Shift+F3)
    FindPrevious,
    /// Replace current match and find next
    ReplaceAndFindNext,
    /// Replace all occurrences
    ReplaceAll,
}

/// UI-specific messages (status bar, cursor blink, modals)
#[derive(Debug, Clone)]
pub enum UiMsg {
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

    /// Open fuzzy file finder modal (Cmd+Shift+O)
    OpenFuzzyFileFinder,

    // === File Drag-and-Drop ===
    /// File is being hovered over the window
    FileHovered(PathBuf),
    /// Hover was cancelled (dragged away from window)
    FileHoverCancelled,

    // === Scrollbar Interaction ===
    /// User clicked the vertical scrollbar track (not the thumb); jump to position
    ScrollbarTrackClickedVertical {
        editor_id: crate::model::editor_area::EditorId,
        /// New absolute scroll position computed from click location
        new_position: usize,
    },
    /// User clicked the horizontal scrollbar track; jump to position
    ScrollbarTrackClickedHorizontal {
        editor_id: crate::model::editor_area::EditorId,
        new_position: usize,
    },
    /// User pressed the mouse on a vertical scrollbar thumb; begin drag
    ScrollbarThumbPressedVertical {
        editor_id: crate::model::editor_area::EditorId,
        grab_offset: f32,
        track_start: f32,
        track_size: f32,
        thumb_size: f32,
        max_scroll: usize,
    },
    /// User pressed the mouse on a horizontal scrollbar thumb; begin drag
    ScrollbarThumbPressedHorizontal {
        editor_id: crate::model::editor_area::EditorId,
        grab_offset: f32,
        track_start: f32,
        track_size: f32,
        thumb_size: f32,
        max_scroll: usize,
    },
    /// Mouse moved during scrollbar thumb drag; primary axis coordinate (y or x)
    ScrollbarDragUpdate { mouse_coord: f32 },
    /// Mouse released; end scrollbar drag
    ScrollbarDragEnd,
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

    /// Move a tab to a new index within its current group (drag reorder)
    ReorderTab { tab_id: TabId, to_index: usize },

    /// Scroll a group's tab bar horizontally by a pixel delta
    ScrollTabBar { group_id: GroupId, delta_px: i32 },

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

    /// Open the focused file with the system's default application
    OpenWithDefaultApp(PathBuf),

    // === Splitter Dragging ===
    /// Mouse pressed on a splitter - begin potential drag
    BeginSplitterDrag {
        splitter_index: usize,
        position: (f32, f32),
    },

    /// Mouse moved during splitter drag
    UpdateSplitterDrag { position: (f32, f32) },

    /// Mouse released - finish drag
    EndSplitterDrag,

    /// Cancel drag (e.g., Escape key pressed)
    CancelSplitterDrag,
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
    /// Reload configuration from disk
    ReloadConfiguration,
    /// Restart the language server for the active document's language
    /// (palette/automation entry point for `LspMsg::RestartServer`; the
    /// server id is resolved from the active document here, since the
    /// palette command carries no arguments).
    RestartLanguageServer,

    // === File Dialog Messages ===
    /// User requested "Save As..." dialog
    SaveFileAs,
    /// Save As dialog returned a path (or None if cancelled)
    SaveFileAsDialogResult { path: Option<PathBuf> },
    /// Save-As write completed (async result) — the LSP identity swap
    /// (didClose(old) + didOpen(new)) and the diagnostics-projection clear
    /// wait for this rather than firing in `SaveFileAsDialogResult`, since
    /// `path_to_uri` canonicalizes and a not-yet-written file resolves to
    /// a different URI than the same path once it exists on disk (see
    /// design doc's URIs and Paths section).
    SaveAsCompleted {
        document_id: crate::model::editor_area::DocumentId,
        old_path: Option<PathBuf>,
        new_path: PathBuf,
        result: Result<(), String>,
    },

    /// User requested "Open File..." dialog
    OpenFileDialog,
    /// Open File dialog returned paths (empty if cancelled)
    OpenFileDialogResult { paths: Vec<PathBuf> },

    /// User requested "Open Folder..." dialog
    OpenFolderDialog,
    /// Open Folder dialog returned folder (or None if cancelled)
    OpenFolderDialogResult { folder: Option<PathBuf> },

    /// Paste text retrieved from system clipboard
    PasteFromClipboard(String),
    /// Default keymap file was created asynchronously
    KeymapCreated {
        path: PathBuf,
        result: Result<(), String>,
    },
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
        syntax_tree: Option<crate::syntax::SyntaxTreeSnapshot>,
        outline: Option<crate::outline::OutlineData>,
        timing: Box<SyntaxWorkerTiming>,
        replace_line_ranges: Option<Vec<std::ops::Range<usize>>>,
    },
    /// Language changed for a document (triggers re-parse)
    LanguageChanged {
        document_id: crate::model::editor_area::DocumentId,
        language: crate::syntax::LanguageId,
    },
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SyntaxWorkerTiming {
    pub snapshot_ms: f64,
    pub queue_ms: f64,
    pub parse_highlight_ms: f64,
    pub parse_ms: Option<f64>,
    pub highlight_ms: Option<f64>,
    pub outline_ms: f64,
    pub worker_total_ms: f64,
    pub outline_extracted: bool,
    pub highlighted_line_count: usize,
    pub replaced_range_count: usize,
}

/// Markdown preview messages
#[derive(Debug, Clone)]
pub enum PreviewMsg {
    /// Toggle preview for current document
    Toggle,
    /// Open preview to the side
    Open,
    /// Close preview
    Close,
    /// Update preview content (after document edit)
    Refresh,
    /// Scroll preview to line (from source scroll)
    ScrollToLine(usize),
    /// Scroll source to line (from preview scroll via IPC)
    SyncFromPreview(usize),
    /// Toggle scroll synchronization
    ToggleSync,
}

/// CSV mode messages
#[derive(Debug, Clone)]
pub enum CsvMsg {
    /// Toggle CSV view mode (parse if entering, discard state if exiting)
    Toggle,
    /// Move cell selection
    MoveUp,
    MoveDown,
    MoveLeft,
    MoveRight,
    /// Move to next cell (Tab)
    NextCell,
    /// Move to previous cell (Shift+Tab)
    PrevCell,
    /// Move to first cell (Cmd+Home)
    FirstCell,
    /// Move to last cell (Cmd+End)
    LastCell,
    /// Move to first column in row (Home)
    RowStart,
    /// Move to last column in row (End)
    RowEnd,
    /// Page up
    PageUp,
    /// Page down
    PageDown,
    /// Exit CSV mode (Escape) or cancel edit
    Exit,
    /// Select a specific cell (from mouse click)
    SelectCell {
        row: usize,
        col: usize,
    },
    /// Scroll viewport vertically (from mouse wheel)
    ScrollVertical(i32),
    /// Scroll viewport horizontally (from mouse wheel)
    ScrollHorizontal(i32),

    // Cell editing
    /// Start editing the selected cell (Enter or F2)
    StartEditing,
    /// Start editing with initial character (typing replaces content)
    StartEditingWithChar(char),
    /// Confirm edit and save changes, then move down (Enter while editing)
    ConfirmEdit,
    /// Confirm edit and move up (Up arrow while editing)
    ConfirmEditUp,
    /// Cancel edit and discard changes (Escape while editing)
    CancelEdit,
    /// Insert character into edit buffer
    EditInsertChar(char),
    /// Delete character before cursor (Backspace)
    EditDeleteBackward,
    /// Delete character at cursor (Delete)
    EditDeleteForward,
    /// Move edit cursor left
    EditCursorLeft,
    /// Move edit cursor right
    EditCursorRight,
    /// Move edit cursor to start (Home while editing)
    EditCursorHome,
    /// Move edit cursor to end (End while editing)
    EditCursorEnd,

    // === Enhanced Cell Editing (via unified editable system) ===
    /// Move edit cursor left by word (Option+Left while editing)
    EditCursorWordLeft,
    /// Move edit cursor right by word (Option+Right while editing)
    EditCursorWordRight,
    /// Delete word before cursor (Option+Backspace while editing)
    EditDeleteWordBackward,
    /// Delete word after cursor (Option+Delete while editing)
    EditDeleteWordForward,
    /// Select all text in cell (Cmd+A while editing)
    EditSelectAll,
    /// Undo last edit operation (Cmd+Z while editing)
    EditUndo,
    /// Redo last undone operation (Cmd+Shift+Z while editing)
    EditRedo,

    // === Selection Movement (Shift+Arrow while editing) ===
    /// Move cursor left with selection (Shift+Left while editing)
    EditCursorLeftWithSelection,
    /// Move cursor right with selection (Shift+Right while editing)
    EditCursorRightWithSelection,
    /// Move to start with selection (Shift+Home while editing)
    EditCursorHomeWithSelection,
    /// Move to end with selection (Shift+End while editing)
    EditCursorEndWithSelection,
    /// Move word left with selection (Shift+Option+Left while editing)
    EditCursorWordLeftWithSelection,
    /// Move word right with selection (Shift+Option+Right while editing)
    EditCursorWordRightWithSelection,

    // === Clipboard (while editing) ===
    /// Copy selection to clipboard (Cmd+C while editing)
    EditCopy,
    /// Cut selection to clipboard (Cmd+X while editing)
    EditCut,
    /// Paste from clipboard (Cmd+V while editing)
    EditPaste,
    /// Paste given text into the active CSV cell editing buffer
    EditPasteText(String),
}

/// Outline panel messages
#[derive(Debug, Clone)]
pub enum OutlineMsg {
    /// Jump to a symbol at the given line
    JumpToSymbol { line: usize, col: usize },
    /// Toggle expand/collapse of a node
    ToggleNode { line: usize, name: String },
    /// Click on a row in the outline panel
    ClickRow {
        index: usize,
        click_count: u8,
        on_chevron: bool,
    },
    /// Navigate up in the outline
    SelectPrevious,
    /// Navigate down in the outline
    SelectNext,
    /// Expand selected node
    ExpandSelected,
    /// Collapse selected node (or go to parent)
    CollapseSelected,
    /// Jump to the selected node
    OpenSelected,
    /// Scroll the outline panel
    Scroll { lines: i32 },
}

/// Terminal panel messages.
///
/// Toggle/focus/panel switching is handled by the existing `DockMsg` --
/// not duplicated here. See `docs/feature/embedded-terminal.md`.
#[derive(Debug, Clone)]
pub enum TerminalMsg {
    /// Spawn a new terminal session.
    NewSession,
    /// Close the active terminal session.
    CloseSession,
    /// Raw bytes received from the PTY (sent by the reader worker thread).
    PtyOutput {
        session_id: usize,
        data: Vec<u8>,
    },
    /// PTY process exited.
    ProcessExited {
        session_id: usize,
        code: i32,
    },
    /// Write bytes to the PTY (keyboard input or an escape-sequence
    /// response the terminal emulator needs to send back, e.g. a
    /// cursor-position report).
    WriteToPty {
        session_id: usize,
        bytes: Vec<u8>,
    },
    /// Paste clipboard text into the active terminal session.
    Paste(String),
    /// OSC title sequence changed the session's title (empty string resets
    /// to the default).
    TitleChanged {
        session_id: usize,
        title: String,
    },
    /// Terminal bell rang.
    Bell {
        session_id: usize,
    },
    /// Terminal content changed and needs a redraw (coalesced from
    /// multiple lower-level VT events).
    Redraw {
        session_id: usize,
    },
    /// Scroll terminal scrollback.
    ScrollUp(usize),
    ScrollDown(usize),
    ScrollToBottom,
    /// Clear the terminal buffer.
    Clear,
    /// Resize the terminal grid (triggered by dock resize).
    Resize {
        rows: u16,
        cols: u16,
    },
}

/// Dock-level messages (panel switching, resizing, visibility)
#[derive(Debug, Clone)]
pub enum DockMsg {
    /// Focus-then-toggle: If dock not focused, focus and open panel.
    /// If dock already focused on this panel, close dock.
    /// This is the primary keybinding action (Cmd+1, Cmd+2, etc.)
    FocusOrTogglePanel(crate::panel::PanelId),

    /// Pure toggle: Open if closed, close if open. Focus-agnostic.
    /// Used by command palette where focus state is irrelevant.
    TogglePanel(crate::panel::PanelId),

    /// Activate a panel without ever closing its dock: open the dock,
    /// make the panel active, and focus it. Used by dock header tab clicks,
    /// which must not toggle the dock closed.
    ActivatePanel(crate::panel::PanelId),

    /// Close the currently focused dock and return focus to editor
    CloseFocusedDock,

    /// Start resizing a dock
    StartResize {
        position: crate::panel::DockPosition,
        initial_coord: f64,
    },

    /// Update dock size during resize
    UpdateResize { coord: f64 },

    /// End dock resize
    EndResize,

    /// Cycle to next panel in focused dock
    NextPanelInDock,

    /// Cycle to previous panel in focused dock
    PrevPanelInDock,

    /// Focus a specific dock (without toggling)
    FocusDock(crate::panel::DockPosition),

    /// Toggle dock visibility at position
    ToggleDock(crate::panel::DockPosition),
}

/// Workspace messages (file tree sidebar)
#[derive(Debug, Clone)]
pub enum WorkspaceMsg {
    /// Toggle sidebar visibility (Cmd+B)
    ToggleSidebar,

    /// Toggle folder expanded/collapsed state
    ToggleFolder(std::path::PathBuf),

    /// Expand a folder
    ExpandFolder(std::path::PathBuf),

    /// Collapse a folder
    CollapseFolder(std::path::PathBuf),

    /// Select an item in the file tree
    SelectItem(std::path::PathBuf),

    /// Select previous item in file tree (Arrow Up)
    SelectPrevious,

    /// Select next item in file tree (Arrow Down)
    SelectNext,

    /// Select parent folder of current item (Arrow Left on collapsed/file)
    SelectParent,

    /// Open file from tree (single-click = preview, double-click = permanent)
    OpenFile {
        path: std::path::PathBuf,
        preview: bool,
    },

    /// Open selected file or toggle folder (Enter key behavior)
    OpenOrToggle,

    /// Reveal the active file in the tree (Cmd+Shift+R)
    RevealActiveFile,

    /// Start resizing sidebar
    StartSidebarResize { initial_x: f64 },

    /// Update sidebar width during resize
    UpdateSidebarResize { x: f64 },

    /// End sidebar resize
    EndSidebarResize,

    /// Refresh the file tree from disk
    Refresh,

    /// Scroll the sidebar file tree (positive = down, negative = up)
    Scroll { lines: i32 },

    /// File system change detected by watcher (triggers tree refresh)
    /// Contains the paths that changed for incremental updates.
    FileSystemChange { paths: Vec<PathBuf> },
}

/// Image viewer messages
#[derive(Debug, Clone)]
pub enum ImageMsg {
    /// Zoom by delta at mouse position (scroll wheel or keyboard)
    Zoom {
        delta: f64,
        mouse_x: f64,
        mouse_y: f64,
    },
    /// Start panning (mouse down)
    StartPan { x: f64, y: f64 },
    /// Update pan position (mouse drag)
    UpdatePan { x: f64, y: f64 },
    /// End panning (mouse up)
    EndPan,
    /// Fit image to window
    FitToWindow,
    /// Show image at actual size (1:1)
    ActualSize,
    /// Track mouse position for zoom-toward-cursor
    MouseMove { x: f64, y: f64 },
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
    /// CSV mode messages
    Csv(CsvMsg),
    /// Image viewer messages
    Image(ImageMsg),
    /// Markdown preview messages
    Preview(PreviewMsg),
    /// Workspace messages (file tree)
    Workspace(WorkspaceMsg),
    /// Dock messages (panel visibility, focus, resize)
    Dock(DockMsg),
    /// Outline panel messages
    Outline(OutlineMsg),
    /// Unified text editing messages.
    TextEdit(EditContext, TextEditMsg),
    /// Terminal panel messages
    Terminal(TerminalMsg),
    /// Menu completion messages (autocomplete.md Phase 1)
    Completion(CompletionMsg),
    /// Language server messages (lsp-integration.md)
    Lsp(LspMsg),
}

/// Language server lifecycle messages (lsp-integration.md Phase 1).
/// Feature messages (diagnostics, definition, hover, completion) are
/// added by later phases once there is data to route.
#[derive(Debug, Clone)]
pub enum LspMsg {
    /// Worker -> update: drives the `LspUiState` model mirror. `root`
    /// identifies which `(server_id, root)` instance changed state — the
    /// model mirror (keyed by `server_id` only) ignores it, but the
    /// runtime uses it to scope restart-attempt bookkeeping per root
    /// (see `LspManager::restart_attempts`).
    ServerStateChanged {
        server_id: LspServerId,
        root: std::path::PathBuf,
        state: ServerState,
    },
    /// Worker -> update: the child process exited (crash or clean
    /// shutdown); the runtime's `LspManager` owns backoff/restart.
    /// `generation` disambiguates this incarnation from a replacement
    /// spawned at the same id/root (see `lsp::client::spawn_server`).
    ServerExited {
        server_id: LspServerId,
        generation: u64,
    },
    /// User/automation-initiated restart (palette, `RestartLanguageServer`).
    RestartServer {
        server_id: LspServerId,
    },
    /// Worker -> main thread: the response to a `shutdown` request this
    /// server's handle sent arrived. Consumed by
    /// `ServerHandle::graceful_shutdown`'s own blocking poll of `msg_rx`
    /// during quit teardown, not by `update()` — see its doc comment.
    ShutdownAcked {
        server_id: LspServerId,
        generation: u64,
    },
    /// Worker -> update: `textDocument/publishDiagnostics` for `uri`, a
    /// full replacement of that URI's diagnostics (lsp-integration.md
    /// Phase 2). `version`, when the server sends one, is used only to
    /// discard out-of-order publishes — never as a `Document.revision`
    /// equality guard (diagnostics are explicitly exempt; see the design
    /// doc's Requests, Guards, Timeouts). The runtime's `LspManager`
    /// checks staleness and updates its authoritative store *before*
    /// this reaches `update()`; `update_lsp` only projects onto whatever
    /// document (if any) has `uri` open.
    DiagnosticsPublished {
        uri: lsp_types::Uri,
        version: Option<i64>,
        diagnostics: Vec<lsp_types::Diagnostic>,
    },

    // ==== Go to Definition + Jump History (lsp-integration.md Phase 3) ====
    /// User intent (F12 / palette). `update_lsp` reads the focused
    /// document + cursor synchronously and captures
    /// `(document_id, revision, position)` into `Cmd::LspRequestDefinition`
    /// — see the design doc's Message Flow example.
    GotoDefinition,
    /// User intent (palette / default keybinding): pop the focused
    /// group's most recent jump-history entry (`update/navigation.rs`).
    NavigateBack,
    NavigateForward,
    /// Runtime -> update: the outcome of a `textDocument/definition`
    /// request issued from `(document_id, revision, origin)`.
    /// Revision-guarded: dropped if `document_id`'s revision has since
    /// advanced (stale-response guard, diagnostics excepted per the
    /// design doc — this is a feature response, not diagnostics).
    DefinitionResolved {
        document_id: crate::model::editor_area::DocumentId,
        revision: u64,
        origin: crate::model::JumpEntry,
        outcome: DefinitionOutcome,
    },
    /// Worker -> runtime only: the raw response (or discard-worthy
    /// supersession) to a `textDocument/definition` request, keyed by
    /// `(server_id, root, request_id)` — the runtime's `LspManager`
    /// looks up the request's `(document_id, revision, origin)` context
    /// (only it has that mapping) and translates this into
    /// `DefinitionResolved` before `update()` ever sees it; see
    /// `process_async_messages`'s interception pass. Never reaches
    /// `update_lsp` in practice, but the match must stay exhaustive.
    DefinitionResponseFromServer {
        server_id: LspServerId,
        root: std::path::PathBuf,
        request_id: i64,
        locations: Vec<lsp_types::Location>,
        /// Echoes `PendingEntry::abandoned` — a superseded request's late
        /// reply is consumed and discarded, never forwarded as
        /// `DefinitionResolved`.
        abandoned: bool,
    },

    // ==== Hover (lsp-integration.md Phase 4) ====
    /// User intent (keybinding / palette). `update_lsp` reads the focused
    /// document + cursor synchronously and captures
    /// `(document_id, revision, position)` into `Cmd::LspRequestHover`,
    /// mirroring `GotoDefinition`'s message flow.
    ShowHover,
    /// Mouse-dwell intent (the Zed-style hover-on-mouse feature): the
    /// runtime fires this from `about_to_wait` once the pointer has sat
    /// still over editor text for `hover_delay_ms`, carrying the hovered
    /// text position rather than reading the caret. Shares
    /// `Cmd::LspRequestHover`'s request path with `ShowHover` — only the
    /// captured position differs.
    ShowHoverAt {
        line: usize,
        col: usize,
    },
    /// Runtime -> update: the outcome of a `textDocument/hover` request
    /// issued from `(document_id, revision, cursor)`. Revision- *and*
    /// cursor-guarded: dropped if the document has been edited, or the
    /// cursor has moved, since the request was sent — a late reply must
    /// never open a card anchored at a position it wasn't computed for.
    HoverResolved {
        document_id: crate::model::editor_area::DocumentId,
        revision: u64,
        cursor: crate::model::editor::Position,
        outcome: HoverOutcome,
    },
    /// Worker -> runtime only: the raw response (or discard-worthy
    /// supersession) to a `textDocument/hover` request, keyed by
    /// `(server_id, root, request_id)` — the runtime's `LspManager` looks
    /// up the request's `(document_id, revision, cursor)` context (only it
    /// has that mapping) and translates this into `HoverResolved` before
    /// `update()` ever sees it, mirroring `DefinitionResponseFromServer`.
    /// Never reaches `update_lsp` in practice, but the match must stay
    /// exhaustive.
    HoverResponseFromServer {
        server_id: LspServerId,
        root: std::path::PathBuf,
        request_id: i64,
        /// Already reduced to plaintext (markdown lightly stripped) by
        /// `lsp::client::parse_hover_result` — `None` for a `null` or
        /// unparseable result, matching the definition parser's
        /// permissive-and-collapsed handling.
        content: Option<String>,
        abandoned: bool,
    },
}

/// The result of a `textDocument/hover` request, distinguishing the two
/// status transients the design doc calls for from an actual result. A
/// server that supports hover but genuinely has nothing to say for this
/// position (`result: null`) is `Content(None)`, not `NoResult` — the card
/// can still open showing diagnostics-only content in that case.
#[derive(Debug, Clone)]
pub enum HoverOutcome {
    Content(Option<String>),
    StillIndexing,
    NotSupported,
}

/// The result of a `textDocument/definition` request, distinguishing the
/// three status transients the design doc calls for ("still indexing…"
/// / "no definition found" / "not supported by this server") from an
/// actual result.
#[derive(Debug, Clone)]
pub enum DefinitionOutcome {
    Locations {
        locations: Vec<lsp_types::Location>,
        /// The server that resolved these locations — routed back into
        /// `LspUiState::route_hint` when the (first) location lands
        /// outside every root, so the generic file-open path doesn't
        /// re-derive (and possibly spawn a server for) its own root; see
        /// the design doc's "route to the resolving server".
        resolving_server: LspServerId,
        resolving_root: std::path::PathBuf,
    },
    StillIndexing,
    NotSupported,
    NoResult,
}

/// Menu completion messages (autocomplete.md Phase 1: "words + snippets,
/// fully offline"). Inline-suggestion messages (`TriggerInline`,
/// `AcceptInline`, ...) aren't here yet — that's Phase 2.
#[derive(Debug, Clone)]
pub enum CompletionMsg {
    /// Ctrl+Space or any other explicit-trigger binding.
    TriggerMenu,
    MenuNext,
    MenuPrev,
    /// Enter/Tab while the menu is visible.
    AcceptMenuItem,
    /// Escape, or any dismiss-causing edit/cursor-move.
    Dismiss,
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
