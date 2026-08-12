//! Command types for the Elm-style architecture
//!
//! Commands represent side effects that should be performed after an update.

use std::path::PathBuf;
use std::sync::Arc;

use crate::keymap::{Command as KeymapCommand, Keymap};
use crate::model::editor_area::DocumentId;
use crate::syntax::LanguageId;

// ============================================================================
// Command Palette Registry
// ============================================================================

/// Identifies a command that can be executed via the command palette
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CommandId {
    // File operations
    NewFile,
    OpenFile,
    FuzzyFileFinder,
    SaveFile,
    SaveFileAs,

    // Edit operations
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,
    SelectAll,

    // Navigation
    GotoLine,
    GotoDefinition,
    NavigateBack,
    NavigateForward,
    ShowHover,

    // View operations
    SplitHorizontal,
    SplitVertical,
    CloseGroup,
    NextTab,
    PrevTab,
    CloseTab,

    // Find/Replace
    Find,

    // UI
    ShowCommandPalette,

    // Theme
    SwitchTheme,

    // Settings
    OpenConfigDirectory,
    OpenKeybindings,
    ReloadConfiguration,

    // CSV
    ToggleCsvView,

    // Markdown
    ToggleMarkdownPreview,

    // Debug/Troubleshooting
    OpenLogFile,

    // Workspace
    OpenFolder,

    // Panels/Docks
    ToggleFileExplorer,
    ToggleTerminal,
    ToggleOutline,
    CloseFocusedDock,

    // File path operations
    RevealInFinder,
    RevealInSidebar,
    CopyAbsolutePath,
    CopyRelativePath,

    // Recent files
    OpenRecentFiles,

    // Completion (autocomplete.md Phase 1)
    TriggerCompletionMenu,

    // Language servers (lsp-integration.md Phase 1)
    RestartLanguageServer,
    ToggleLsp,
    ManageLanguageServers,

    // Application
    Quit,

    // Debug overlays (only available in debug builds)
    #[cfg(debug_assertions)]
    TogglePerfOverlay,
    #[cfg(debug_assertions)]
    ToggleDebugOverlay,
    /// Cycle the cursor-anchored popup shell for manual testing
    /// (overlay-surface.md Phase 5: closed -> completion demo -> hover
    /// demo -> closed).
    #[cfg(debug_assertions)]
    CycleCursorOverlayDemo,
}

/// Broad grouping used to pick the command palette row icon (see the "Rows"
/// section of overlay-surface.md's Visual Language). Deliberately coarse:
/// per-command icon curation is an open question, not scoped here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandCategory {
    File,
    Edit,
    Nav,
    View,
    Panel,
    System,
}

impl CommandCategory {
    /// Row icon glyph for this category, drawn in the palette's icon slot.
    /// Chosen from JetBrains Mono's confirmed glyph coverage (see the font's
    /// `cmap`), not for literal semantics.
    pub fn glyph(self) -> char {
        match self {
            CommandCategory::File => '▫',
            CommandCategory::Edit => '¶',
            CommandCategory::Nav => '→',
            CommandCategory::View => '◇',
            CommandCategory::Panel => '☰',
            CommandCategory::System => '§',
        }
    }
}

/// A command definition for the command palette
#[derive(Debug, Clone)]
pub struct CommandDef {
    pub id: CommandId,
    pub label: &'static str,
    pub keybinding: Option<&'static str>,
    pub category: CommandCategory,
}

/// Static registry of all available commands
pub static COMMANDS: &[CommandDef] = &[
    CommandDef {
        id: CommandId::NewFile,
        category: CommandCategory::File,
        label: "New File",
        keybinding: Some("⇧⌘N"),
    },
    CommandDef {
        id: CommandId::OpenFile,
        category: CommandCategory::File,
        label: "Open File...",
        keybinding: Some("⌘O"),
    },
    CommandDef {
        id: CommandId::FuzzyFileFinder,
        category: CommandCategory::File,
        label: "Go to File...",
        keybinding: Some("⇧⌘O"),
    },
    CommandDef {
        id: CommandId::SaveFile,
        category: CommandCategory::File,
        label: "Save File",
        keybinding: Some("⌘S"),
    },
    CommandDef {
        id: CommandId::SaveFileAs,
        category: CommandCategory::File,
        label: "Save File As...",
        keybinding: Some("⇧⌘S"),
    },
    CommandDef {
        id: CommandId::Undo,
        category: CommandCategory::Edit,
        label: "Undo",
        keybinding: Some("⌘Z"),
    },
    CommandDef {
        id: CommandId::Redo,
        category: CommandCategory::Edit,
        label: "Redo",
        keybinding: Some("⇧⌘Z"),
    },
    CommandDef {
        id: CommandId::Cut,
        category: CommandCategory::Edit,
        label: "Cut",
        keybinding: Some("⌘X"),
    },
    CommandDef {
        id: CommandId::Copy,
        category: CommandCategory::Edit,
        label: "Copy",
        keybinding: Some("⌘C"),
    },
    CommandDef {
        id: CommandId::Paste,
        category: CommandCategory::Edit,
        label: "Paste",
        keybinding: Some("⌘V"),
    },
    CommandDef {
        id: CommandId::SelectAll,
        category: CommandCategory::Edit,
        label: "Select All",
        keybinding: Some("⌘A"),
    },
    CommandDef {
        id: CommandId::GotoLine,
        category: CommandCategory::Nav,
        label: "Go to Line...",
        keybinding: Some("⌘L"),
    },
    CommandDef {
        id: CommandId::GotoDefinition,
        category: CommandCategory::Nav,
        label: "Go to Definition",
        keybinding: Some("⌘B"),
    },
    CommandDef {
        id: CommandId::NavigateBack,
        category: CommandCategory::Nav,
        label: "Navigate Back",
        keybinding: Some("⌘["),
    },
    CommandDef {
        id: CommandId::NavigateForward,
        category: CommandCategory::Nav,
        label: "Navigate Forward",
        keybinding: Some("⌘]"),
    },
    CommandDef {
        id: CommandId::ShowHover,
        category: CommandCategory::Nav,
        label: "Show Hover",
        keybinding: Some("⇧⌘D"),
    },
    CommandDef {
        id: CommandId::SplitHorizontal,
        category: CommandCategory::View,
        label: "Split Editor Right",
        keybinding: Some("⇧⌥⌘H"),
    },
    CommandDef {
        id: CommandId::SplitVertical,
        category: CommandCategory::View,
        label: "Split Editor Down",
        keybinding: Some("⇧⌥⌘V"),
    },
    CommandDef {
        id: CommandId::CloseGroup,
        category: CommandCategory::View,
        label: "Close Editor Group",
        keybinding: None,
    },
    CommandDef {
        id: CommandId::NextTab,
        category: CommandCategory::View,
        label: "Next Tab",
        keybinding: Some("⌥⌘→"),
    },
    CommandDef {
        id: CommandId::PrevTab,
        category: CommandCategory::View,
        label: "Previous Tab",
        keybinding: Some("⌥⌘←"),
    },
    CommandDef {
        id: CommandId::CloseTab,
        category: CommandCategory::View,
        label: "Close Tab",
        keybinding: Some("⌘W"),
    },
    CommandDef {
        id: CommandId::Find,
        category: CommandCategory::Nav,
        label: "Find...",
        keybinding: Some("⌘F"),
    },
    CommandDef {
        id: CommandId::ShowCommandPalette,
        category: CommandCategory::System,
        label: "Show Command Palette",
        keybinding: Some("⇧⌘A"),
    },
    CommandDef {
        id: CommandId::SwitchTheme,
        category: CommandCategory::View,
        label: "Switch Theme...",
        keybinding: None,
    },
    CommandDef {
        id: CommandId::OpenConfigDirectory,
        category: CommandCategory::System,
        label: "Open Config Directory",
        keybinding: None,
    },
    CommandDef {
        id: CommandId::OpenKeybindings,
        category: CommandCategory::System,
        label: "Open Keymap",
        keybinding: None,
    },
    CommandDef {
        id: CommandId::ReloadConfiguration,
        category: CommandCategory::System,
        label: "Reload Configuration",
        keybinding: None,
    },
    CommandDef {
        id: CommandId::ToggleCsvView,
        category: CommandCategory::View,
        label: "Toggle CSV View",
        keybinding: None,
    },
    CommandDef {
        id: CommandId::ToggleMarkdownPreview,
        category: CommandCategory::View,
        label: "Markdown: Toggle Preview",
        keybinding: Some("⇧⌘V"),
    },
    CommandDef {
        id: CommandId::OpenLogFile,
        category: CommandCategory::System,
        label: "Open Log File",
        keybinding: None,
    },
    CommandDef {
        id: CommandId::OpenFolder,
        category: CommandCategory::File,
        label: "Open Folder...",
        keybinding: None,
    },
    CommandDef {
        id: CommandId::ToggleFileExplorer,
        category: CommandCategory::Panel,
        label: "View: Toggle File Explorer",
        keybinding: Some("⌘1"),
    },
    CommandDef {
        id: CommandId::ToggleTerminal,
        category: CommandCategory::Panel,
        label: "View: Toggle Terminal",
        keybinding: Some("⌘2"),
    },
    CommandDef {
        id: CommandId::ToggleOutline,
        category: CommandCategory::Panel,
        label: "View: Toggle Outline",
        keybinding: Some("⌘7"),
    },
    CommandDef {
        id: CommandId::CloseFocusedDock,
        category: CommandCategory::Panel,
        label: "View: Close Panel",
        keybinding: None,
    },
    CommandDef {
        id: CommandId::RevealInFinder,
        category: CommandCategory::File,
        label: "Reveal Current File in Finder",
        keybinding: None,
    },
    CommandDef {
        id: CommandId::RevealInSidebar,
        category: CommandCategory::File,
        label: "Reveal in File Explorer",
        keybinding: Some("\u{2318}\u{21e7}R"),
    },
    CommandDef {
        id: CommandId::CopyAbsolutePath,
        category: CommandCategory::File,
        label: "Copy Absolute Path",
        keybinding: None,
    },
    CommandDef {
        id: CommandId::CopyRelativePath,
        category: CommandCategory::File,
        label: "Copy Relative Path",
        keybinding: None,
    },
    CommandDef {
        id: CommandId::OpenRecentFiles,
        category: CommandCategory::File,
        label: "Open Recent Files",
        keybinding: Some("⌘E"),
    },
    CommandDef {
        id: CommandId::TriggerCompletionMenu,
        category: CommandCategory::Edit,
        label: "Trigger Completion",
        keybinding: Some("⌃Space"),
    },
    CommandDef {
        id: CommandId::RestartLanguageServer,
        category: CommandCategory::System,
        label: "Restart Language Server",
        keybinding: None,
    },
    CommandDef {
        id: CommandId::ToggleLsp,
        category: CommandCategory::System,
        label: "Toggle LSP",
        keybinding: None,
    },
    CommandDef {
        id: CommandId::ManageLanguageServers,
        category: CommandCategory::System,
        label: "Language Servers...",
        keybinding: None,
    },
    CommandDef {
        id: CommandId::Quit,
        category: CommandCategory::System,
        label: "Quit",
        keybinding: Some("⌘Q"),
    },
];

/// Debug-only commands (only available in debug builds)
#[cfg(debug_assertions)]
pub static DEBUG_COMMANDS: &[CommandDef] = &[
    CommandDef {
        id: CommandId::TogglePerfOverlay,
        category: CommandCategory::System,
        label: "Toggle Performance Overlay",
        keybinding: Some("F2"),
    },
    CommandDef {
        id: CommandId::ToggleDebugOverlay,
        category: CommandCategory::System,
        label: "Toggle Debug Overlay",
        keybinding: Some("F8"),
    },
    CommandDef {
        id: CommandId::CycleCursorOverlayDemo,
        category: CommandCategory::System,
        label: "Cycle Cursor Overlay Demo",
        keybinding: Some("F9"),
    },
];

/// Get all available commands (including debug commands in debug builds), in
/// registry order. Fuzzy filtering/ranking lives in `update::ui` alongside
/// the file finder's nucleo matching (`resolve_palette_rows`) — this just
/// hands back the raw pool.
pub(crate) fn all_commands() -> Vec<&'static CommandDef> {
    #[allow(unused_mut)]
    let mut cmds: Vec<&'static CommandDef> = COMMANDS.iter().collect();
    #[cfg(debug_assertions)]
    cmds.extend(DEBUG_COMMANDS.iter());
    cmds
}

/// Map CommandId to keymap::Command for keybinding lookup
impl CommandId {
    pub fn to_keymap_command(self) -> Option<KeymapCommand> {
        match self {
            CommandId::NewFile => Some(KeymapCommand::NewTab), // NewFile maps to NewTab
            CommandId::OpenFile => Some(KeymapCommand::OpenFile),
            CommandId::FuzzyFileFinder => Some(KeymapCommand::FuzzyFileFinder),
            CommandId::SaveFile => Some(KeymapCommand::SaveFile),
            CommandId::SaveFileAs => Some(KeymapCommand::SaveFileAs),
            CommandId::Undo => Some(KeymapCommand::Undo),
            CommandId::Redo => Some(KeymapCommand::Redo),
            CommandId::Cut => Some(KeymapCommand::Cut),
            CommandId::Copy => Some(KeymapCommand::Copy),
            CommandId::Paste => Some(KeymapCommand::Paste),
            CommandId::SelectAll => Some(KeymapCommand::SelectAll),
            CommandId::GotoLine => Some(KeymapCommand::ToggleGotoLine),
            CommandId::GotoDefinition => Some(KeymapCommand::GotoDefinition),
            CommandId::NavigateBack => Some(KeymapCommand::NavigateBack),
            CommandId::NavigateForward => Some(KeymapCommand::NavigateForward),
            CommandId::ShowHover => Some(KeymapCommand::ShowHover),
            CommandId::SplitHorizontal => Some(KeymapCommand::SplitHorizontal),
            CommandId::SplitVertical => Some(KeymapCommand::SplitVertical),
            CommandId::CloseGroup => None, // No direct mapping yet
            CommandId::NextTab => Some(KeymapCommand::NextTab),
            CommandId::PrevTab => Some(KeymapCommand::PrevTab),
            CommandId::CloseTab => Some(KeymapCommand::CloseTab),
            CommandId::Find => Some(KeymapCommand::ToggleFindReplace),
            CommandId::ShowCommandPalette => Some(KeymapCommand::ToggleCommandPalette),
            CommandId::SwitchTheme => None,
            CommandId::OpenConfigDirectory => None,
            CommandId::OpenKeybindings => None,
            CommandId::ReloadConfiguration => None,
            CommandId::ToggleCsvView => Some(KeymapCommand::CsvToggle),
            CommandId::ToggleMarkdownPreview => Some(KeymapCommand::MarkdownTogglePreview),
            CommandId::OpenLogFile => Some(KeymapCommand::OpenLogFile),
            CommandId::OpenFolder => None,
            CommandId::ToggleFileExplorer => Some(KeymapCommand::ToggleFileExplorer),
            CommandId::ToggleTerminal => Some(KeymapCommand::ToggleTerminal),
            CommandId::ToggleOutline => Some(KeymapCommand::ToggleOutline),
            CommandId::CloseFocusedDock => Some(KeymapCommand::CloseFocusedDock),
            CommandId::RevealInFinder => None,
            CommandId::RevealInSidebar => Some(KeymapCommand::RevealInSidebar),
            CommandId::CopyAbsolutePath => None,
            CommandId::CopyRelativePath => None,
            CommandId::OpenRecentFiles => Some(KeymapCommand::OpenRecentFiles),
            CommandId::TriggerCompletionMenu => Some(KeymapCommand::TriggerCompletionMenu),
            CommandId::RestartLanguageServer => Some(KeymapCommand::RestartLanguageServer),
            CommandId::ToggleLsp => None,
            CommandId::ManageLanguageServers => None,
            CommandId::Quit => Some(KeymapCommand::Quit),
            #[cfg(debug_assertions)]
            CommandId::TogglePerfOverlay => None,
            #[cfg(debug_assertions)]
            CommandId::ToggleDebugOverlay => None,
            #[cfg(debug_assertions)]
            CommandId::CycleCursorOverlayDemo => None,
        }
    }
}

/// Get keybinding display string for a command from the keymap
pub fn keybinding_for_command(id: CommandId, keymap: &Keymap) -> Option<String> {
    let keymap_cmd = id.to_keymap_command()?;
    keymap.display_for(keymap_cmd)
}

/// Get keybinding display string using the static fallback (for when keymap isn't available)
pub fn keybinding_for_command_static(id: CommandId) -> Option<&'static str> {
    COMMANDS
        .iter()
        .find(|cmd| cmd.id == id)
        .and_then(|cmd| cmd.keybinding)
}

// ============================================================================
// Damage Tracking (partial redraw optimization)
// ============================================================================

/// Represents which parts of the UI need redrawing
///
/// Used for partial redraw optimization to avoid full-frame rendering
/// on every update. When in doubt, use `Damage::Full` for correctness.
#[derive(Debug, Clone, Default)]
pub enum Damage {
    /// No redraw needed (default state for accumulation)
    #[default]
    None,
    /// Redraw everything (always safe fallback)
    Full,
    /// Redraw specific areas only
    Areas(Vec<DamageArea>),
}

/// High-level UI regions that can be independently redrawn
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DamageArea {
    /// All editor groups, tab bars, gutters, text areas, splitters
    EditorArea,
    /// Bottom status bar only
    StatusBar,
    /// Specific lines for cursor blink optimization (line numbers are document-relative)
    /// This enables the most fine-grained optimization for cursor blink which happens
    /// at 2Hz and would otherwise require full EditorArea redraws.
    CursorLines(Vec<usize>),
}

impl Damage {
    /// Create damage for specific areas
    pub fn areas(areas: Vec<DamageArea>) -> Self {
        if areas.is_empty() {
            Damage::Full // Empty areas means nothing to redraw, but treat as full for safety
        } else {
            Damage::Areas(areas)
        }
    }

    /// Create damage for just the editor area
    pub fn editor_area() -> Self {
        Damage::Areas(vec![DamageArea::EditorArea])
    }

    /// Create damage for just the status bar
    pub fn status_bar() -> Self {
        Damage::Areas(vec![DamageArea::StatusBar])
    }

    /// Create damage for specific cursor lines
    pub fn cursor_lines(lines: Vec<usize>) -> Self {
        if lines.is_empty() {
            Damage::Areas(vec![]) // No lines to redraw
        } else {
            Damage::Areas(vec![DamageArea::CursorLines(lines)])
        }
    }

    /// Merge another damage into this one
    ///
    /// If either damage is Full, the result is Full.
    /// If either damage is None, the other takes precedence.
    /// Otherwise, areas are combined with deduplication.
    pub fn merge(&mut self, other: Damage) {
        match (&mut *self, other) {
            // None is identity for merge
            (Damage::None, other) => *self = other,
            (_, Damage::None) => {} // Nothing to merge
            // Full absorbs everything
            (Damage::Full, _) => {} // Already full, nothing to do
            (this, Damage::Full) => *this = Damage::Full,
            // Merge areas
            (Damage::Areas(areas), Damage::Areas(other_areas)) => {
                for area in other_areas {
                    // Merge CursorLines specially (combine line lists)
                    if let DamageArea::CursorLines(ref lines) = area {
                        if let Some(existing) = areas.iter_mut().find_map(|a| {
                            if let DamageArea::CursorLines(ref mut l) = a {
                                Some(l)
                            } else {
                                Option::None
                            }
                        }) {
                            // Merge line numbers, avoiding duplicates
                            for &line in lines {
                                if !existing.contains(&line) {
                                    existing.push(line);
                                }
                            }
                            continue;
                        }
                    }
                    // For EditorArea/StatusBar, just add if not present
                    if !areas.contains(&area) {
                        areas.push(area);
                    }
                }
            }
        }
    }

    /// Check if this damage is a full redraw
    pub fn is_full(&self) -> bool {
        matches!(self, Damage::Full)
    }

    /// Check if this damage includes the editor area (or is full)
    pub fn includes_editor(&self) -> bool {
        match self {
            Damage::None => false,
            Damage::Full => true,
            Damage::Areas(areas) => areas.iter().any(|a| {
                matches!(a, DamageArea::EditorArea) || matches!(a, DamageArea::CursorLines(_))
            }),
        }
    }

    /// Check if this damage includes the status bar (or is full)
    pub fn includes_status_bar(&self) -> bool {
        match self {
            Damage::None => false,
            Damage::Full => true,
            Damage::Areas(areas) => areas.iter().any(|a| matches!(a, DamageArea::StatusBar)),
        }
    }

    /// Get cursor lines if this is a cursor-lines-only damage
    pub fn cursor_lines_only(&self) -> Option<&[usize]> {
        match self {
            Damage::Areas(areas) if areas.len() == 1 => {
                if let Some(DamageArea::CursorLines(lines)) = areas.first() {
                    Some(lines)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// Check if any redraw is needed
    pub fn needs_redraw(&self) -> bool {
        match self {
            Damage::None => false,
            Damage::Full => true,
            Damage::Areas(areas) => !areas.is_empty(),
        }
    }
}

// ============================================================================
// Side-Effect Commands (returned from update)
// ============================================================================

/// Commands returned by update functions
#[derive(Debug, Clone, Default)]
pub enum Cmd {
    /// No command - do nothing
    #[default]
    None,
    /// Request a full redraw of the UI (legacy, always safe)
    Redraw,
    /// Request a partial redraw of specific areas (optimization)
    RedrawAreas(Vec<DamageArea>),
    /// Save file asynchronously
    SaveFile { path: PathBuf, content: String },
    /// Save-As write, asynchronously — distinct from `SaveFile` so its
    /// completion (`AppMsg::SaveAsCompleted`) can carry the document/path
    /// identity needed to defer the LSP didClose/didOpen pair until the
    /// file actually exists on disk (see that message's doc comment).
    SaveFileAs {
        document_id: crate::model::editor_area::DocumentId,
        old_path: Option<PathBuf>,
        new_path: PathBuf,
        content: String,
    },
    /// Load file asynchronously
    LoadFile { path: PathBuf },
    /// Open a path in the system file explorer/finder
    OpenInExplorer { path: PathBuf },
    /// Reveal a file in the system file manager (select it)
    RevealFileInFinder { path: PathBuf },
    /// Open a file in a new tab for editing
    OpenFileInEditor { path: PathBuf },
    /// Execute multiple commands
    Batch(Vec<Cmd>),

    // File dialogs
    /// Show native open file dialog
    ShowOpenFileDialog {
        /// Allow selecting multiple files
        allow_multi: bool,
        /// Starting directory for the dialog
        start_dir: Option<PathBuf>,
    },
    /// Show native save file dialog
    ShowSaveFileDialog {
        /// Suggested file path (for pre-filling name/directory)
        suggested_path: Option<PathBuf>,
    },
    /// Show native open folder dialog
    ShowOpenFolderDialog {
        /// Starting directory for the dialog
        start_dir: Option<PathBuf>,
    },

    // === Syntax Highlighting Commands ===
    /// Start debounce timer for syntax parsing
    /// After delay_ms, sends Msg::Syntax(ParseReady)
    DebouncedSyntaxParse {
        document_id: DocumentId,
        revision: u64,
        delay_ms: u64,
    },
    /// Run syntax parsing in background worker
    /// Sends Msg::Syntax(ParseCompleted) when done
    RunSyntaxParse {
        document_id: DocumentId,
        revision: u64,
        // Arc<str>, not String: `check_lsp_did_change_deadlines` shares
        // this exact snapshot with a coincident LSP didChange deadline
        // via a cheap refcount clone instead of a second full-buffer
        // copy (lsp-integration.md's Document Synchronization).
        source: Arc<str>,
        language: LanguageId,
        snapshot_ms: f64,
    },
    /// Drop debounced parse state and worker-side cached parse trees for a document.
    ClearSyntaxState { document_id: DocumentId },

    // === Display Commands ===
    /// Reinitialize the renderer (e.g., after scale factor change)
    ReinitializeRenderer,
    /// Re-derive status bar height from the renderer's font metrics
    /// (config reload may change `status_bar_font_size`)
    SyncStatusBarMetrics,

    // === Application Commands ===
    /// Request application exit
    Quit,

    /// Save recent files list asynchronously
    SaveRecentFiles {
        recent: crate::recent_files::RecentFiles,
    },
    /// Save command palette usage/pin history asynchronously
    /// (overlay-surface.md Phase 4).
    SaveCommandHistory {
        history: crate::command_history::CommandHistory,
    },
    /// Copy a string to the system clipboard
    CopyToClipboard(String),
    /// Request pasting text from the system clipboard
    RequestClipboardPaste,
    /// Create default keymap file asynchronously
    CreateDefaultKeymapFile { path: PathBuf },

    // === Terminal Commands ===
    /// Spawn a PTY + shell for a new terminal session. The runtime spawns
    /// the PTY reader/writer threads (see `terminal::spawn_pty`) and routes
    /// `Msg::Terminal(PtyOutput)`/`ProcessExited` back through the update
    /// loop, matching the async-worker pattern used for syntax parsing.
    SpawnTerminal {
        session_id: usize,
        rows: u16,
        cols: u16,
    },

    // === Language Server Commands (lsp-integration.md) ===
    /// Spawn a server for `language` rooted for `file_path`, if one is
    /// registered, enabled, and not already running for that root.
    /// Idempotent — the runtime's `LspManager` is the source of truth
    /// for "already spawned".
    LspEnsureServer {
        language: LanguageId,
        file_path: PathBuf,
    },
    /// Kill and respawn every running instance of a server (manual
    /// restart, e.g. from the command palette).
    LspRestartServer { server_id: crate::lsp::LspServerId },
    /// A matching document gained a file path + language — send
    /// `textDocument/didOpen` (spawning the server first if needed).
    /// Idempotent from the model's point of view; the runtime's
    /// `LspManager` decides whether a server is actually registered/
    /// ready and no-ops otherwise.
    LspDidOpen {
        document_id: DocumentId,
        file_path: PathBuf,
        language: LanguageId,
    },
    /// Debounce an edit's `didChange` (deadline-map + max-wait cap,
    /// mirroring `DebouncedSyntaxParse`). A no-op if `document_id` isn't
    /// currently open on any server.
    LspScheduleDidChange {
        document_id: DocumentId,
        revision: u64,
    },
    /// Send `textDocument/didSave` for a just-saved document, with text
    /// iff the server's capabilities asked for it.
    LspDidSave { document_id: DocumentId },
    /// Send `textDocument/didClose` — call only when the document is
    /// released (`release_document_if_unreferenced`), never on tab
    /// close alone (documents are refcounted).
    LspDidClose { document_id: DocumentId },
    /// Drop the authoritative diagnostics-store entry for `document_id`'s
    /// current file (lsp-integration.md "cleared on ... language
    /// change") — unlike `LspDidClose`, which never touches the store
    /// (diagnostics for unopened files are meant to be retained), this
    /// is for the one case where the *same* URI's retained diagnostics
    /// must not survive: the language association changed, so a
    /// subsequent `didOpen` must not resurrect them.
    LspClearDiagnostics { document_id: DocumentId },
    /// `textDocument/definition` for `document_id` at `position`
    /// (already UTF-16-converted), tagged with the document's `revision`
    /// at request time and the `origin` to record in jump history on
    /// success (design doc's "capture (document_id, revision, position)"
    /// in `update_lsp`, and "push jump-history entry" once resolved).
    /// Supersedes any still-pending definition request for the same
    /// document via `$/cancelRequest` (advisory — the runtime still
    /// consumes and discards the superseded request's late reply).
    LspRequestDefinition {
        document_id: DocumentId,
        position: lsp_types::Position,
        revision: u64,
        origin: crate::model::JumpEntry,
    },
    /// `textDocument/didOpen` against a specific, already-known
    /// `(server_id, root)` — bypasses `LspEnsureServer`'s root
    /// resolution/spawn entirely. Only emitted for a definition-jump
    /// target outside every root (`LspUiState::route_hint`); the design
    /// doc's "route to the resolving server, never spawn a new server
    /// rooted in a toolchain directory". A no-op if that server isn't
    /// running (shouldn't happen — it just answered the request).
    LspDidOpenOnServer {
        document_id: DocumentId,
        file_path: PathBuf,
        server_id: crate::lsp::LspServerId,
        root: PathBuf,
    },
    /// `textDocument/hover` for `document_id` at `position` (already
    /// UTF-16-converted), tagged with the document's `revision` and
    /// (char-column) `cursor` at request time — mirrors
    /// `LspRequestDefinition`. Supersedes any still-pending hover request
    /// for the same document via `$/cancelRequest`.
    LspRequestHover {
        document_id: DocumentId,
        position: lsp_types::Position,
        cursor: crate::model::editor::Position,
        revision: u64,
    },
    /// The master `lsp.enabled` switch flipped (`CommandId::ToggleLsp`).
    /// Disabling tears down every running server — a non-quit variant of
    /// `Cmd::Quit`'s graceful teardown, with the same bounded grace — and
    /// clears their diagnostics; enabling clears the missing-server memo
    /// so `ensure_lsp_server` can re-attempt spawns lazily on the next
    /// open/edit rather than staying skipped forever.
    LspSetEnabled { enabled: bool },
    /// A single server's `lsp.servers.<id>.enabled` override flipped from
    /// the Language Servers picker modal — same semantics as
    /// `LspSetEnabled`, scoped to one server id.
    LspSetServerEnabled {
        server_id: crate::lsp::LspServerId,
        enabled: bool,
    },

    // === Debug Commands ===
    /// Toggle performance overlay (debug builds only)
    #[cfg(debug_assertions)]
    TogglePerfOverlay,
}

impl Cmd {
    /// Create a batch of commands
    pub fn batch(cmds: Vec<Cmd>) -> Self {
        Cmd::Batch(cmds)
    }

    /// Check if this command requires a redraw
    ///
    /// Derived from `damage()` so the two never drift out of sync. For
    /// `Batch`, this is equivalent to short-circuiting on the first
    /// sub-command that needs a redraw: `damage()` merges sub-damages and
    /// stops early once it hits `Damage::Full`, and any sub-command whose
    /// own `needs_redraw()` would be true contributes either `Full` or a
    /// non-empty `Areas(..)`, both of which make the merged result's
    /// `needs_redraw()` true.
    pub fn needs_redraw(&self) -> bool {
        self.damage().needs_redraw()
    }

    /// Get the damage for this command
    ///
    /// Returns the combined damage from this command. For batch commands,
    /// merges all sub-command damages.
    pub fn damage(&self) -> Damage {
        match self {
            Cmd::None => Damage::Areas(vec![]), // No damage
            Cmd::Redraw => Damage::Full,
            Cmd::RedrawAreas(areas) => {
                if areas.is_empty() {
                    Damage::Areas(vec![])
                } else {
                    Damage::Areas(areas.clone())
                }
            }
            // File operations may cause full redraw (file load changes content)
            Cmd::SaveFile { .. } | Cmd::SaveFileAs { .. } => Damage::Full,
            Cmd::LoadFile { .. } => Damage::Full,
            Cmd::OpenInExplorer { .. } => Damage::Full,
            Cmd::RevealFileInFinder { .. } => Damage::Areas(vec![]),
            Cmd::OpenFileInEditor { .. } => Damage::Full,
            // Batch: merge all damages
            Cmd::Batch(cmds) => {
                let mut damage = Damage::Areas(vec![]);
                for cmd in cmds {
                    damage.merge(cmd.damage());
                    // Short-circuit if we hit Full
                    if damage.is_full() {
                        break;
                    }
                }
                damage
            }
            // Dialogs don't need immediate redraw
            Cmd::ShowOpenFileDialog { .. } => Damage::Areas(vec![]),
            Cmd::ShowSaveFileDialog { .. } => Damage::Areas(vec![]),
            Cmd::ShowOpenFolderDialog { .. } => Damage::Areas(vec![]),
            // Syntax commands don't need immediate redraw
            Cmd::DebouncedSyntaxParse { .. } => Damage::Areas(vec![]),
            Cmd::RunSyntaxParse { .. } => Damage::Areas(vec![]),
            Cmd::ClearSyntaxState { .. } => Damage::Areas(vec![]),
            // Reinitialize triggers full redraw
            Cmd::ReinitializeRenderer => Damage::Full,
            Cmd::SyncStatusBarMetrics => Damage::Full,
            // Quit doesn't need redraw - app is exiting
            Cmd::Quit => Damage::Areas(vec![]),
            Cmd::SaveRecentFiles { .. } => Damage::Areas(vec![]),
            Cmd::SaveCommandHistory { .. } => Damage::Areas(vec![]),
            Cmd::CopyToClipboard(_) => Damage::Areas(vec![]),
            Cmd::RequestClipboardPaste => Damage::Areas(vec![]),
            Cmd::CreateDefaultKeymapFile { .. } => Damage::Areas(vec![]),
            // Spawning doesn't need immediate redraw; the PtyOutput that
            // follows shortly after will request one.
            Cmd::SpawnTerminal { .. } => Damage::Areas(vec![]),
            // Spawning/restarting a server has no immediate visual effect;
            // ServerStateChanged (once it arrives) requests its own redraw.
            Cmd::LspEnsureServer { .. } => Damage::Areas(vec![]),
            Cmd::LspRestartServer { .. } => Damage::Areas(vec![]),
            Cmd::LspDidOpen { .. } => Damage::Areas(vec![]),
            Cmd::LspScheduleDidChange { .. } => Damage::Areas(vec![]),
            Cmd::LspDidSave { .. } => Damage::Areas(vec![]),
            Cmd::LspDidClose { .. } => Damage::Areas(vec![]),
            Cmd::LspClearDiagnostics { .. } => Damage::Areas(vec![]),
            Cmd::LspRequestDefinition { .. } => Damage::Areas(vec![]),
            Cmd::LspDidOpenOnServer { .. } => Damage::Areas(vec![]),
            Cmd::LspRequestHover { .. } => Damage::Areas(vec![]),
            // No immediate visual effect; a `ServerStateChanged` (or the
            // batched `Cmd::Redraw`/`redraw_status_bar` these are always
            // paired with at the call site) requests its own redraw.
            Cmd::LspSetEnabled { .. } => Damage::Areas(vec![]),
            Cmd::LspSetServerEnabled { .. } => Damage::Areas(vec![]),
            // Debug overlay toggle triggers full redraw
            #[cfg(debug_assertions)]
            Cmd::TogglePerfOverlay => Damage::Full,
        }
    }

    /// Create a command to redraw the editor area and status bar
    pub fn redraw_editor() -> Self {
        Cmd::RedrawAreas(vec![DamageArea::EditorArea, DamageArea::StatusBar])
    }

    /// Create a command to redraw just the status bar
    pub fn redraw_status_bar() -> Self {
        Cmd::RedrawAreas(vec![DamageArea::StatusBar])
    }

    /// Create a command to redraw specific cursor lines
    pub fn redraw_cursor_lines(lines: Vec<usize>) -> Self {
        if lines.is_empty() {
            Cmd::None
        } else {
            Cmd::RedrawAreas(vec![DamageArea::CursorLines(lines)])
        }
    }

    /// Convert Option<Cmd> with None to Cmd::None
    pub fn from_option(opt: Option<Cmd>) -> Self {
        opt.unwrap_or(Cmd::None)
    }
}

// Allow converting Option<Cmd> to Cmd
impl From<Option<Cmd>> for Cmd {
    fn from(opt: Option<Cmd>) -> Self {
        opt.unwrap_or(Cmd::None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cmd_damage_computation() {
        // None -> empty areas
        assert!(matches!(Cmd::None.damage(), Damage::Areas(a) if a.is_empty()));

        // Redraw -> Full
        assert!(matches!(Cmd::Redraw.damage(), Damage::Full));

        // RedrawAreas preserves areas
        let cmd = Cmd::RedrawAreas(vec![DamageArea::EditorArea]);
        assert!(matches!(cmd.damage(), Damage::Areas(a) if a.contains(&DamageArea::EditorArea)));

        // Empty RedrawAreas -> empty areas (not Full)
        let cmd = Cmd::RedrawAreas(vec![]);
        assert!(matches!(cmd.damage(), Damage::Areas(a) if a.is_empty()));
    }

    #[test]
    fn test_cmd_damage_batch_full() {
        // Batch with Full -> Full
        let batch = Cmd::Batch(vec![
            Cmd::RedrawAreas(vec![DamageArea::StatusBar]),
            Cmd::Redraw,
        ]);
        assert!(matches!(batch.damage(), Damage::Full));
    }

    #[test]
    fn test_cmd_damage_batch_merge() {
        // Batch without Full -> merged areas
        let batch = Cmd::Batch(vec![
            Cmd::RedrawAreas(vec![DamageArea::StatusBar]),
            Cmd::RedrawAreas(vec![DamageArea::EditorArea]),
        ]);
        let damage = batch.damage();
        match damage {
            Damage::Areas(a) => {
                assert_eq!(a.len(), 2);
                assert!(a.contains(&DamageArea::StatusBar));
                assert!(a.contains(&DamageArea::EditorArea));
            }
            _ => panic!("Expected Damage::Areas, got {:?}", damage),
        }
    }

    #[test]
    fn test_cmd_damage_cursor_lines() {
        // CursorLines damage
        let cmd = Cmd::redraw_cursor_lines(vec![5, 10, 15]);
        let damage = cmd.damage();
        match damage {
            Damage::Areas(a) => {
                assert_eq!(a.len(), 1);
                if let DamageArea::CursorLines(lines) = &a[0] {
                    assert_eq!(lines.len(), 3);
                    assert!(lines.contains(&5));
                    assert!(lines.contains(&10));
                    assert!(lines.contains(&15));
                } else {
                    panic!("Expected DamageArea::CursorLines");
                }
            }
            _ => panic!("Expected Damage::Areas"),
        }
    }

    #[test]
    fn test_cmd_damage_merge_cursor_lines() {
        // Merge cursor lines with existing cursor lines
        let mut damage = Damage::Areas(vec![DamageArea::CursorLines(vec![1, 2])]);
        damage.merge(Damage::Areas(vec![DamageArea::CursorLines(vec![2, 3])]));
        match damage {
            Damage::Areas(a) => {
                assert_eq!(a.len(), 1);
                if let DamageArea::CursorLines(lines) = &a[0] {
                    assert_eq!(lines.len(), 3);
                    assert!(lines.contains(&1));
                    assert!(lines.contains(&2));
                    assert!(lines.contains(&3));
                } else {
                    panic!("Expected DamageArea::CursorLines");
                }
            }
            _ => panic!("Expected Damage::Areas"),
        }
    }

    #[test]
    fn test_cmd_damage_merge_editor_area() {
        // Merge EditorArea with existing EditorArea (should dedupe)
        let mut damage = Damage::Areas(vec![DamageArea::EditorArea]);
        damage.merge(Damage::Areas(vec![DamageArea::EditorArea]));
        match damage {
            Damage::Areas(a) => {
                assert_eq!(a.len(), 1);
                assert!(a.contains(&DamageArea::EditorArea));
            }
            _ => panic!("Expected Damage::Areas"),
        }
    }

    #[test]
    fn test_cmd_damage_merge_none() {
        // None is identity for merge
        let mut damage = Damage::Areas(vec![DamageArea::EditorArea]);
        damage.merge(Damage::None);
        match damage {
            Damage::Areas(a) => {
                assert_eq!(a.len(), 1);
                assert!(a.contains(&DamageArea::EditorArea));
            }
            _ => panic!("Expected Damage::Areas"),
        }
    }

    #[test]
    fn test_cmd_damage_merge_full_absorbs() {
        // Full absorbs everything
        let mut damage = Damage::Areas(vec![DamageArea::EditorArea, DamageArea::StatusBar]);
        damage.merge(Damage::Full);
        assert!(damage.is_full());

        // Full is also absorbed (stays Full)
        let mut damage = Damage::Full;
        damage.merge(Damage::Areas(vec![DamageArea::EditorArea]));
        assert!(damage.is_full());
    }

    #[test]
    fn test_damage_editor_area_helper() {
        let damage = Damage::editor_area();
        assert!(damage.includes_editor());
        assert!(!damage.includes_status_bar());
    }

    #[test]
    fn test_damage_status_bar_helper() {
        let damage = Damage::status_bar();
        assert!(damage.includes_status_bar());
        assert!(!damage.includes_editor());
    }

    #[test]
    fn test_damage_cursor_lines_helper() {
        let damage = Damage::cursor_lines(vec![5, 10]);
        assert!(damage.includes_editor());
        if let Some(lines) = damage.cursor_lines_only() {
            assert_eq!(lines.len(), 2);
        } else {
            panic!("Expected cursor_lines_only to return Some");
        }
    }

    #[test]
    fn test_damage_needs_redraw() {
        assert!(!Damage::None.needs_redraw());
        assert!(Damage::Full.needs_redraw());
        assert!(Damage::Areas(vec![DamageArea::EditorArea]).needs_redraw());
        assert!(!Damage::Areas(vec![]).needs_redraw());
    }

    #[test]
    fn test_needs_redraw_agrees_with_damage_needs_redraw() {
        // Guards against `needs_redraw()` and `damage()` drifting apart now
        // that `needs_redraw()` is implemented in terms of `damage()`.
        let samples: Vec<Cmd> = vec![
            Cmd::None,
            Cmd::Redraw,
            Cmd::RedrawAreas(vec![DamageArea::EditorArea]),
            Cmd::RedrawAreas(vec![]),
            Cmd::CopyToClipboard("no visual effect".to_string()),
            Cmd::Quit,
            // Batch mixing a damaging command with a non-damaging one.
            Cmd::Batch(vec![
                Cmd::CopyToClipboard("x".to_string()),
                Cmd::RedrawAreas(vec![DamageArea::StatusBar]),
            ]),
            // Batch containing only non-damaging commands.
            Cmd::Batch(vec![Cmd::CopyToClipboard("x".to_string()), Cmd::Quit]),
            // Batch containing a full redraw.
            Cmd::Batch(vec![Cmd::CopyToClipboard("x".to_string()), Cmd::Redraw]),
        ];

        for cmd in samples {
            assert_eq!(
                cmd.needs_redraw(),
                cmd.damage().needs_redraw(),
                "needs_redraw() and damage().needs_redraw() disagree for {cmd:?}"
            );
        }
    }

    #[test]
    fn test_cmd_redraw_helpers() {
        // redraw_editor includes both EditorArea and StatusBar
        let cmd = Cmd::redraw_editor();
        let damage = cmd.damage();
        match damage {
            Damage::Areas(a) => {
                assert_eq!(a.len(), 2);
                assert!(a.contains(&DamageArea::EditorArea));
                assert!(a.contains(&DamageArea::StatusBar));
            }
            _ => panic!("Expected Damage::Areas"),
        }

        // redraw_status_bar only includes StatusBar
        let cmd = Cmd::redraw_status_bar();
        let damage = cmd.damage();
        match damage {
            Damage::Areas(a) => {
                assert_eq!(a.len(), 1);
                assert!(a.contains(&DamageArea::StatusBar));
            }
            _ => panic!("Expected Damage::Areas"),
        }

        // redraw_cursor_lines with empty vec returns None
        let cmd = Cmd::redraw_cursor_lines(vec![]);
        assert!(matches!(cmd, Cmd::None));
    }
}
