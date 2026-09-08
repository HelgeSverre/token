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
    ShowSignatureHelp,
    RenameSymbol,
    ShowCodeActions,
    FormatDocument,
    FormatSelection,
    FindUsages,
    NextDiagnostic,
    PrevDiagnostic,
    ShowUsages,

    // Context menu (context-menu.md)
    ShowContextMenu,

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
    OpenSettings,
    OpenConfigDirectory,
    OpenKeybindings,
    ReloadConfiguration,

    // CSV
    ToggleCsvView,
    ToggleSoftWrap,

    // Markdown
    ToggleMarkdownPreview,

    // Debug/Troubleshooting
    OpenLogFile,

    // Workspace
    OpenFolder,

    // Panels/Docks
    ToggleFileExplorer,
    ToggleTerminal,
    NewTerminal,
    CloseTerminal,
    NextTerminal,
    PreviousTerminal,
    ToggleOutline,
    ToggleProblems,
    ToggleUsages,
    ToggleProblemsScope,
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
    /// Inline ghost-text suggestion (autocomplete.md Phase 2)
    TriggerInlineSuggestion,
    AcceptInlineSuggestion,
    AcceptInlineWord,
    AcceptInlineLine,
    NextInlineSuggestion,
    PrevInlineSuggestion,
    DismissInlineSuggestion,
    OpenInlineStatistics,

    // Language servers (lsp-integration.md Phase 1)
    RestartLanguageServer,
    ToggleLsp,
    ToggleAutocomplete,
    ManageLanguageServers,
    /// "Set Language..." picker (session-pinned language override).
    SetLanguage,

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
    /// Shared keyboard action, when the palette command has one.
    pub action: Option<KeymapCommand>,
    pub id: CommandId,
    pub label: &'static str,
    pub category: CommandCategory,
}

/// Static registry of all available commands
pub static COMMANDS: &[CommandDef] = &[
    CommandDef {
        id: CommandId::NewFile,
        action: Some(KeymapCommand::NewTab),
        category: CommandCategory::File,
        label: "New File",
    },
    CommandDef {
        id: CommandId::OpenFile,
        action: Some(KeymapCommand::OpenFile),
        category: CommandCategory::File,
        label: "Open File...",
    },
    CommandDef {
        id: CommandId::FuzzyFileFinder,
        action: Some(KeymapCommand::FuzzyFileFinder),
        category: CommandCategory::File,
        label: "Go to File...",
    },
    CommandDef {
        id: CommandId::SaveFile,
        action: Some(KeymapCommand::SaveFile),
        category: CommandCategory::File,
        label: "Save File",
    },
    CommandDef {
        id: CommandId::SaveFileAs,
        action: Some(KeymapCommand::SaveFileAs),
        category: CommandCategory::File,
        label: "Save File As...",
    },
    CommandDef {
        id: CommandId::Undo,
        action: Some(KeymapCommand::Undo),
        category: CommandCategory::Edit,
        label: "Undo",
    },
    CommandDef {
        id: CommandId::Redo,
        action: Some(KeymapCommand::Redo),
        category: CommandCategory::Edit,
        label: "Redo",
    },
    CommandDef {
        id: CommandId::Cut,
        action: Some(KeymapCommand::Cut),
        category: CommandCategory::Edit,
        label: "Cut",
    },
    CommandDef {
        id: CommandId::Copy,
        action: Some(KeymapCommand::Copy),
        category: CommandCategory::Edit,
        label: "Copy",
    },
    CommandDef {
        id: CommandId::Paste,
        action: Some(KeymapCommand::Paste),
        category: CommandCategory::Edit,
        label: "Paste",
    },
    CommandDef {
        id: CommandId::SelectAll,
        action: Some(KeymapCommand::SelectAll),
        category: CommandCategory::Edit,
        label: "Select All",
    },
    CommandDef {
        id: CommandId::GotoLine,
        action: Some(KeymapCommand::ToggleGotoLine),
        category: CommandCategory::Nav,
        label: "Go to Line...",
    },
    CommandDef {
        id: CommandId::GotoDefinition,
        action: Some(KeymapCommand::GotoDefinition),
        category: CommandCategory::Nav,
        label: "Go to Definition",
    },
    CommandDef {
        id: CommandId::NavigateBack,
        action: Some(KeymapCommand::NavigateBack),
        category: CommandCategory::Nav,
        label: "Navigate Back",
    },
    CommandDef {
        id: CommandId::NavigateForward,
        action: Some(KeymapCommand::NavigateForward),
        category: CommandCategory::Nav,
        label: "Navigate Forward",
    },
    CommandDef {
        id: CommandId::ShowHover,
        action: Some(KeymapCommand::ShowHover),
        category: CommandCategory::Nav,
        label: "Show Hover",
    },
    CommandDef {
        id: CommandId::ShowSignatureHelp,
        action: Some(KeymapCommand::ShowSignatureHelp),
        category: CommandCategory::Nav,
        label: "Show Signature Help",
    },
    CommandDef {
        id: CommandId::RenameSymbol,
        action: Some(KeymapCommand::RenameSymbol),
        category: CommandCategory::Nav,
        label: "Rename Symbol",
    },
    CommandDef {
        id: CommandId::ShowCodeActions,
        action: Some(KeymapCommand::ShowCodeActions),
        category: CommandCategory::Edit,
        label: "Show Code Actions",
    },
    CommandDef {
        id: CommandId::FormatDocument,
        action: Some(KeymapCommand::FormatDocument),
        category: CommandCategory::Edit,
        label: "Format Document",
    },
    CommandDef {
        id: CommandId::FormatSelection,
        action: Some(KeymapCommand::FormatSelection),
        category: CommandCategory::Edit,
        label: "Format Selection",
    },
    CommandDef {
        id: CommandId::FindUsages,
        action: Some(KeymapCommand::FindUsages),
        category: CommandCategory::Nav,
        label: "Find Usages",
    },
    CommandDef {
        id: CommandId::ShowUsages,
        action: Some(KeymapCommand::ShowUsages),
        category: CommandCategory::Nav,
        label: "Show Usages",
    },
    CommandDef {
        id: CommandId::NextDiagnostic,
        action: Some(KeymapCommand::NextDiagnostic),
        category: CommandCategory::Nav,
        label: "Next Diagnostic",
    },
    CommandDef {
        id: CommandId::PrevDiagnostic,
        action: Some(KeymapCommand::PrevDiagnostic),
        category: CommandCategory::Nav,
        label: "Previous Diagnostic",
    },
    CommandDef {
        id: CommandId::ShowContextMenu,
        action: Some(KeymapCommand::ShowContextMenu),
        category: CommandCategory::Nav,
        label: "Show Context Menu",
    },
    CommandDef {
        id: CommandId::SplitHorizontal,
        action: Some(KeymapCommand::SplitHorizontal),
        category: CommandCategory::View,
        label: "Split Editor Right",
    },
    CommandDef {
        id: CommandId::SplitVertical,
        action: Some(KeymapCommand::SplitVertical),
        category: CommandCategory::View,
        label: "Split Editor Down",
    },
    CommandDef {
        id: CommandId::CloseGroup,
        action: None,
        category: CommandCategory::View,
        label: "Close Editor Group",
    },
    CommandDef {
        id: CommandId::NextTab,
        action: Some(KeymapCommand::NextTab),
        category: CommandCategory::View,
        label: "Next Tab",
    },
    CommandDef {
        id: CommandId::PrevTab,
        action: Some(KeymapCommand::PrevTab),
        category: CommandCategory::View,
        label: "Previous Tab",
    },
    CommandDef {
        id: CommandId::CloseTab,
        action: Some(KeymapCommand::CloseTab),
        category: CommandCategory::View,
        label: "Close Tab",
    },
    CommandDef {
        id: CommandId::Find,
        action: Some(KeymapCommand::ToggleFindReplace),
        category: CommandCategory::Nav,
        label: "Find...",
    },
    CommandDef {
        id: CommandId::ShowCommandPalette,
        action: Some(KeymapCommand::ToggleCommandPalette),
        category: CommandCategory::System,
        label: "Show Command Palette",
    },
    CommandDef {
        id: CommandId::SwitchTheme,
        action: None,
        category: CommandCategory::View,
        label: "Switch Theme...",
    },
    CommandDef {
        id: CommandId::OpenConfigDirectory,
        action: None,
        category: CommandCategory::System,
        label: "Open Config Directory",
    },
    CommandDef {
        id: CommandId::OpenSettings,
        action: Some(KeymapCommand::OpenSettings),
        category: CommandCategory::System,
        label: "Open Settings",
    },
    CommandDef {
        id: CommandId::OpenKeybindings,
        action: None,
        category: CommandCategory::System,
        label: "Open Keymap",
    },
    CommandDef {
        id: CommandId::ReloadConfiguration,
        action: None,
        category: CommandCategory::System,
        label: "Reload Configuration",
    },
    CommandDef {
        id: CommandId::ToggleCsvView,
        action: Some(KeymapCommand::CsvToggle),
        category: CommandCategory::View,
        label: "Toggle CSV View",
    },
    CommandDef {
        id: CommandId::ToggleSoftWrap,
        action: Some(KeymapCommand::ToggleSoftWrap),
        category: CommandCategory::View,
        label: "Toggle Soft Wrap",
    },
    CommandDef {
        id: CommandId::ToggleMarkdownPreview,
        action: Some(KeymapCommand::MarkdownTogglePreview),
        category: CommandCategory::View,
        label: "Markdown: Toggle Preview",
    },
    CommandDef {
        id: CommandId::OpenLogFile,
        action: Some(KeymapCommand::OpenLogFile),
        category: CommandCategory::System,
        label: "Open Log File",
    },
    CommandDef {
        id: CommandId::OpenFolder,
        action: None,
        category: CommandCategory::File,
        label: "Open Folder...",
    },
    CommandDef {
        id: CommandId::ToggleFileExplorer,
        action: Some(KeymapCommand::ToggleFileExplorer),
        category: CommandCategory::Panel,
        label: "View: Toggle File Explorer",
    },
    CommandDef {
        id: CommandId::ToggleTerminal,
        action: Some(KeymapCommand::ToggleTerminal),
        category: CommandCategory::Panel,
        label: "View: Toggle Terminal",
    },
    CommandDef {
        id: CommandId::ToggleOutline,
        action: Some(KeymapCommand::ToggleOutline),
        category: CommandCategory::Panel,
        label: "View: Toggle Outline",
    },
    CommandDef {
        id: CommandId::NewTerminal,
        action: Some(KeymapCommand::NewTerminal),
        category: CommandCategory::Panel,
        label: "Terminal: New Tab",
    },
    CommandDef {
        id: CommandId::CloseTerminal,
        action: Some(KeymapCommand::CloseTerminal),
        category: CommandCategory::Panel,
        label: "Terminal: Close Tab",
    },
    CommandDef {
        id: CommandId::NextTerminal,
        action: Some(KeymapCommand::NextTerminal),
        category: CommandCategory::Panel,
        label: "Terminal: Next Tab",
    },
    CommandDef {
        id: CommandId::PreviousTerminal,
        action: Some(KeymapCommand::PreviousTerminal),
        category: CommandCategory::Panel,
        label: "Terminal: Previous Tab",
    },
    CommandDef {
        id: CommandId::ToggleProblems,
        action: Some(KeymapCommand::ToggleProblems),
        category: CommandCategory::Panel,
        label: "View: Toggle Problems",
    },
    CommandDef {
        id: CommandId::ToggleProblemsScope,
        action: None,
        category: CommandCategory::Panel,
        label: "Problems: Toggle Current File Only",
    },
    CommandDef {
        id: CommandId::ToggleUsages,
        action: Some(KeymapCommand::ToggleUsages),
        category: CommandCategory::Panel,
        label: "View: Toggle Usages",
    },
    CommandDef {
        id: CommandId::CloseFocusedDock,
        action: Some(KeymapCommand::CloseFocusedDock),
        category: CommandCategory::Panel,
        label: "View: Close Panel",
    },
    CommandDef {
        id: CommandId::RevealInFinder,
        action: None,
        category: CommandCategory::File,
        label: "Reveal Current File in Finder",
    },
    CommandDef {
        id: CommandId::RevealInSidebar,
        action: Some(KeymapCommand::RevealInSidebar),
        category: CommandCategory::File,
        label: "Reveal in File Explorer",
    },
    CommandDef {
        id: CommandId::CopyAbsolutePath,
        action: None,
        category: CommandCategory::File,
        label: "Copy Absolute Path",
    },
    CommandDef {
        id: CommandId::CopyRelativePath,
        action: None,
        category: CommandCategory::File,
        label: "Copy Relative Path",
    },
    CommandDef {
        id: CommandId::OpenRecentFiles,
        action: Some(KeymapCommand::OpenRecentFiles),
        category: CommandCategory::File,
        label: "Open Recent Files",
    },
    CommandDef {
        id: CommandId::TriggerCompletionMenu,
        action: Some(KeymapCommand::TriggerCompletionMenu),
        category: CommandCategory::Edit,
        label: "Trigger Completion",
    },
    CommandDef {
        id: CommandId::TriggerInlineSuggestion,
        action: Some(KeymapCommand::TriggerInlineSuggestion),
        category: CommandCategory::Edit,
        label: "Trigger Inline Suggestion",
    },
    CommandDef {
        id: CommandId::AcceptInlineSuggestion,
        action: Some(KeymapCommand::AcceptInlineSuggestion),
        category: CommandCategory::Edit,
        label: "Accept Inline Suggestion",
    },
    CommandDef {
        id: CommandId::AcceptInlineWord,
        action: Some(KeymapCommand::AcceptInlineWord),
        category: CommandCategory::Edit,
        label: "Accept Inline Suggestion Word",
    },
    CommandDef {
        id: CommandId::AcceptInlineLine,
        action: Some(KeymapCommand::AcceptInlineLine),
        category: CommandCategory::Edit,
        label: "Accept Inline Suggestion Line",
    },
    CommandDef {
        id: CommandId::DismissInlineSuggestion,
        action: Some(KeymapCommand::DismissInlineSuggestion),
        category: CommandCategory::Edit,
        label: "Dismiss Inline Suggestion",
    },
    CommandDef {
        id: CommandId::OpenInlineStatistics,
        action: Some(KeymapCommand::OpenInlineStatistics),
        category: CommandCategory::Edit,
        label: "Open Inline Completion Statistics",
    },
    CommandDef {
        id: CommandId::NextInlineSuggestion,
        action: Some(KeymapCommand::NextInlineSuggestion),
        category: CommandCategory::Edit,
        label: "Next Inline Suggestion",
    },
    CommandDef {
        id: CommandId::PrevInlineSuggestion,
        action: Some(KeymapCommand::PrevInlineSuggestion),
        category: CommandCategory::Edit,
        label: "Previous Inline Suggestion",
    },
    CommandDef {
        id: CommandId::RestartLanguageServer,
        action: Some(KeymapCommand::RestartLanguageServer),
        category: CommandCategory::System,
        label: "Restart Language Server",
    },
    CommandDef {
        id: CommandId::ToggleLsp,
        action: None,
        category: CommandCategory::System,
        label: "Toggle LSP",
    },
    CommandDef {
        id: CommandId::ToggleAutocomplete,
        action: None,
        category: CommandCategory::System,
        label: "Toggle Autocomplete",
    },
    CommandDef {
        id: CommandId::ManageLanguageServers,
        action: None,
        category: CommandCategory::System,
        label: "Language Servers...",
    },
    CommandDef {
        id: CommandId::SetLanguage,
        action: None,
        category: CommandCategory::System,
        label: "Set Language...",
    },
    CommandDef {
        id: CommandId::Quit,
        action: Some(KeymapCommand::Quit),
        category: CommandCategory::System,
        label: "Quit",
    },
];

/// Debug-only commands (only available in debug builds)
#[cfg(debug_assertions)]
pub static DEBUG_COMMANDS: &[CommandDef] = &[
    CommandDef {
        id: CommandId::TogglePerfOverlay,
        action: None,
        category: CommandCategory::System,
        label: "Toggle Performance Overlay",
    },
    CommandDef {
        id: CommandId::ToggleDebugOverlay,
        action: None,
        category: CommandCategory::System,
        label: "Toggle Debug Overlay",
    },
    CommandDef {
        id: CommandId::CycleCursorOverlayDemo,
        action: None,
        category: CommandCategory::System,
        label: "Cycle Cursor Overlay Demo",
    },
];

/// Get all available commands (including debug commands in debug builds), in
/// registry order. Fuzzy filtering/ranking lives in `update::ui` alongside
/// the file finder's nucleo matching (`resolve_palette_rows`) — this just
/// hands back the raw pool.
pub(crate) fn all_commands() -> impl Iterator<Item = &'static CommandDef> {
    let cmds = COMMANDS.iter();
    #[cfg(debug_assertions)]
    let cmds = cmds.chain(DEBUG_COMMANDS.iter());
    cmds
}

/// Look up a command's registry entry (label, category, keybinding hint) by
/// id — used by the context menu's `MenuItem::from_command` to reuse the
/// palette's own keycap hint instead of threading `Keymap` into a second
/// lookup path (context-menu.md "Adjustment 2").
pub(crate) fn command_def(id: CommandId) -> Option<&'static CommandDef> {
    all_commands().find(|def| def.id == id)
}

/// The palette registry shares its action identity with keyboard dispatch.
impl CommandId {
    pub fn to_keymap_command(self) -> Option<KeymapCommand> {
        command_def(self).and_then(|def| def.action)
    }
}

/// Resolve a command's hint using the same active bindings and conditions as dispatch.
pub(crate) fn keybinding_for_command(
    id: CommandId,
    keymap: &Keymap,
    context: &crate::keymap::KeyContext,
) -> Option<String> {
    keymap.display_for(id.to_keymap_command()?, context)
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

/// Why a `completionItem/resolve` was issued — see
/// `Cmd::LspResolveCompletionItem`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolvePurpose {
    /// Documentation for the selected row; a timeout is silent.
    Docs,
    /// A deferred accept is blocked on it; a timeout unblocks the accept.
    Accept,
}

/// Configuration resources opened by user actions. Path discovery and preparation
/// belong to the runtime, not keymap translation or update handlers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigResource {
    Directory,
    Keybindings,
    Log,
    InlineStatistics,
}

/// Commands returned by update functions
#[derive(Debug, Clone, Default)]
pub enum Cmd {
    /// Replace the workspace-wide symbol query; None cancels current work.
    WorkspaceSymbols(Option<crate::lsp::workspace_symbols::SymbolSearchRequest>),
    /// Coalesced background Find scan against an immutable document snapshot.
    RunFindSearch(std::sync::Arc<crate::model::ui::FindSearchRequest>),
    /// No command - do nothing
    #[default]
    None,
    /// Request a full redraw of the UI (legacy, always safe)
    Redraw,
    /// Request a partial redraw of specific areas (optimization)
    RedrawAreas(Vec<DamageArea>),
    /// Save or Save As. The ordered runtime writer returns the exact snapshot
    /// written, so subsequent edits cannot be marked saved by an old reply.
    SaveFile {
        target: crate::model::FileRequest,
        path: PathBuf,
        content: ropey::Rope,
    },
    /// Load file asynchronously
    LoadFile {
        target: crate::model::FileRequest,
        path: PathBuf,
    },
    /// Prepare a file or configuration resource off the UI thread.
    PrepareFileOpen(crate::model::FileOpenRequest),
    /// Notify runtime waiters after installation (or rejection), not after reading.
    FileOpenFinished {
        request_id: u64,
        document_id: Option<DocumentId>,
    },
    /// Open a path in the system file explorer/finder
    OpenInExplorer {
        path: PathBuf,
    },
    /// Reveal a file in the system file manager (select it)
    RevealFileInFinder {
        path: PathBuf,
    },
    /// Execute multiple commands
    Batch(Vec<Cmd>),

    // File dialogs
    /// Show native open file dialog
    ShowOpenFileDialog {
        group_id: crate::model::GroupId,
        /// Allow selecting multiple files
        allow_multi: bool,
        /// Starting directory for the dialog
        start_dir: Option<PathBuf>,
    },
    /// Show native save file dialog
    ShowSaveFileDialog {
        target: crate::model::FileRequest,
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
    ClearSyntaxState {
        document_id: DocumentId,
    },

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
    /// Merge one aggregate inline outcome on the ordered file worker.
    RecordInlineUsage(crate::completion::statistics::UsageEvent),
    /// Read a directory on the replaceable speculative worker, never in update.
    CompletePaths(std::sync::Arc<crate::completion::path::PathRequest>),
    CancelPathCompletion,
    /// Resolve a documentation scroll against the same layout used for paint.
    PageCompletionDocumentation {
        forward: bool,
    },
    /// Copy a string to the system clipboard
    CopyToClipboard(String),
    /// Open a validated web URL in the system browser.
    OpenWebUrl(String),
    /// Request pasting text from the system clipboard
    RequestClipboardPaste,
    /// Read or conditionally replace the keymap on the ordered file worker.
    PrepareKeymap {
        session: std::sync::Arc<()>,
        save: Option<Box<crate::keymap::preferences::KeymapSave>>,
    },
    /// Persist configuration in runtime order, never from an update handler.
    SaveConfiguration {
        config: Box<crate::config::EditorConfig>,
    },
    /// Read configuration and its theme, then deliver ConfigurationLoaded.
    ReloadConfiguration,
    /// Load a theme for preview, restore, or confirmation.
    LoadTheme {
        id: String,
        persist: bool,
    },

    // === Terminal Commands ===
    /// Terminate and remove only the named terminal session.
    CloseTerminal {
        session_id: usize,
    },
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
    LspRestartServer {
        server_id: crate::lsp::LspServerId,
    },
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
    LspDidSave {
        document_id: DocumentId,
        saved_text: ropey::Rope,
    },
    /// Send `textDocument/didClose` — call only when the document is
    /// released (`release_document_if_unreferenced`), never on tab
    /// close alone (documents are refcounted).
    LspDidClose {
        document_id: DocumentId,
    },
    /// Drop the authoritative diagnostics-store entry for `document_id`'s
    /// current file (lsp-integration.md "cleared on ... language
    /// change") — unlike `LspDidClose`, which never touches the store
    /// (diagnostics for unopened files are meant to be retained), this
    /// is for the one case where the *same* URI's retained diagnostics
    /// must not survive: the language association changed, so a
    /// subsequent `didOpen` must not resurrect them.
    LspClearDiagnostics {
        document_id: DocumentId,
    },
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
    /// `textDocument/signatureHelp`, mirroring `LspRequestHover`. `trigger`
    /// is the typed trigger/retrigger character (`triggerKind:
    /// TriggerCharacter`), `None` for an explicit invoke; `is_retrigger`
    /// is set while the float is already open.
    LspRequestSignatureHelp {
        document_id: DocumentId,
        position: lsp_types::Position,
        cursor: crate::model::editor::Position,
        revision: u64,
        trigger: Option<String>,
        is_retrigger: bool,
    },
    /// Rename Symbol entry point: `textDocument/prepareRename` at
    /// `position` when the server supports it, otherwise the prompt opens
    /// straight away with `fallback` (the word under the caret). Either
    /// way the prompt is opened by `LspMsg::PrepareRenameResolved`.
    LspRequestPrepareRename {
        document_id: DocumentId,
        position: lsp_types::Position,
        cursor: crate::model::editor::Position,
        revision: u64,
        fallback: String,
    },
    /// `textDocument/rename` at `position` with `newName`; the reply is a
    /// `WorkspaceEdit` applied via `apply_workspace_edit`.
    LspRequestRename {
        document_id: DocumentId,
        position: lsp_types::Position,
        revision: u64,
        new_name: String,
    },
    /// `textDocument/codeAction` for `range` (selection or caret), with
    /// the overlapping `diagnostics` as `context.diagnostics`. Tagged with
    /// `revision` and `cursor` like `LspRequestHover`.
    LspRequestCodeActions {
        document_id: DocumentId,
        position: lsp_types::Position,
        range: lsp_types::Range,
        cursor: crate::model::editor::Position,
        revision: u64,
        diagnostics: Vec<lsp_types::Diagnostic>,
    },
    /// `workspace/executeCommand` on the server that owns `document_id`
    /// (a code action's `command`). The reply is dropped; any resulting
    /// edits arrive as a server-initiated `workspace/applyEdit`.
    LspExecuteCommand {
        document_id: DocumentId,
        command: String,
        arguments: Option<Vec<serde_json::Value>>,
    },
    /// `textDocument/formatting` (`range: None`) or `rangeFormatting`.
    /// `then_save` marks a `format_on_save` request: the resolution (or
    /// its gate/timeout fallback) performs the save the user asked for.
    LspRequestFormatting {
        document_id: DocumentId,
        revision: u64,
        range: Option<lsp_types::Range>,
        options: lsp_types::FormattingOptions,
        then_save: bool,
    },
    /// `textDocument/references` (Show Usages / Find Usages), tagged with
    /// the document's `revision` and (char-column) `cursor` at request
    /// time — mirrors `LspRequestHover`. `context.includeDeclaration` is
    /// always sent `true`.
    LspRequestReferences {
        target: crate::model::usages::ReferencesTarget,
        document_id: DocumentId,
        position: lsp_types::Position,
        cursor: crate::model::editor::Position,
        revision: u64,
    },
    /// Arm (or reset) the per-document completion-request debounce —
    /// emitted by `update/completion.rs` whenever the menu opens or its
    /// query changes. Unlike definition/hover (single user-initiated
    /// requests), completion is typing-driven, so the request itself is
    /// debounced runtime-side (`COMPLETION_DEBOUNCE`) and flush-before-
    /// request applies when it fires. A no-op if the document isn't open
    /// on a server that supports completion.
    LspScheduleCompletion {
        document_id: DocumentId,
        position: lsp_types::Position,
        revision: u64,
        trigger_character: Option<String>,
    },
    /// Arm (or re-arm) the inline-suggestion debounce for a document; the
    /// runtime replays `CompletionMsg::InlineDeadlineFired` when it elapses.
    ScheduleInlineRequest {
        snapshot: crate::completion::inline::RequestSnapshot,
        delay_ms: u64,
        explicit: bool,
    },
    /// Collect opt-in context before handing the request to the provider.
    PrepareInlineRequest(Box<crate::completion::provider::InlineJob>),
    /// Hand a context-prepared request to the completion worker thread.
    RunInlineRequest(Box<crate::completion::provider::InlineJob>),
    /// Stop the pending debounce and drop the worker's in-flight future.
    CancelInlineRequest,
    /// The menu closed (or its document changed): drop any pending
    /// completion debounce and supersede the in-flight request for this
    /// document. Without this a request fired just after dismissal would
    /// be answered into a closed menu and dropped anyway.
    LspCancelCompletion {
        document_id: DocumentId,
    },
    /// `completionItem/resolve` for the raw item the selected menu row was
    /// converted from. `Accept` purpose is the deferred half of
    /// accept-when-resolve-support-is-advertised (ts-ls returns minimal
    /// items whose auto-import `additionalTextEdits` only exist after
    /// resolve; skipping resolve silently drops imports) and arms the
    /// unblock timeout; `Docs` purpose fetches documentation for the
    /// selected row and times out silently. `selected` echoes the menu
    /// selection so a resolution whose selection has since moved is
    /// dropped.
    LspResolveCompletionItem {
        document_id: DocumentId,
        revision: u64,
        server_id: crate::lsp::LspServerId,
        root: PathBuf,
        raw_item: std::sync::Arc<lsp_types::CompletionItem>,
        selected: usize,
        purpose: ResolvePurpose,
    },
    /// Arm (or reset) the per-document debounce for a `Docs`-purpose
    /// `LspResolveCompletionItem` — emitted on every selection change so
    /// arrowing through the list coalesces into one resolve. Dropped by
    /// `LspCancelCompletion` and superseded by any direct resolve.
    LspScheduleResolve {
        document_id: DocumentId,
        revision: u64,
        server_id: crate::lsp::LspServerId,
        root: PathBuf,
        raw_item: std::sync::Arc<lsp_types::CompletionItem>,
        selected: usize,
    },
    /// The master `lsp.enabled` switch flipped (`CommandId::ToggleLsp`).
    /// Disabling tears down every running server — a non-quit variant of
    /// `Cmd::Quit`'s graceful teardown, with the same bounded grace — and
    /// clears their diagnostics; enabling clears the missing-server memo
    /// so `ensure_lsp_server` can re-attempt spawns lazily on the next
    /// open/edit rather than staying skipped forever.
    LspSetEnabled {
        enabled: bool,
    },
    /// A single server's `lsp.servers.<id>.enabled` override flipped from
    /// the Language Servers picker modal — same semantics as
    /// `LspSetEnabled`, scoped to one server id.
    LspSetServerEnabled {
        server_id: crate::lsp::LspServerId,
        enabled: bool,
    },
    /// Reply to a server -> client request the reader thread forwarded
    /// to `update()` (`workspace/applyEdit`) instead of answering itself.
    LspRespondToServer {
        server_id: crate::lsp::LspServerId,
        root: PathBuf,
        request_id: serde_json::Value,
        result: serde_json::Value,
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
            Cmd::SaveFile { .. } => Damage::Full,
            Cmd::LoadFile { .. } => Damage::Full,
            Cmd::PrepareFileOpen(_) => Damage::status_bar(),
            Cmd::FileOpenFinished { .. } => Damage::None,
            Cmd::OpenInExplorer { .. } => Damage::Full,
            Cmd::RevealFileInFinder { .. } => Damage::Areas(vec![]),
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
            // Starting a new Find scan clears stale marks and shows Searching.
            Cmd::RunFindSearch(_) => Damage::Full,
            Cmd::ClearSyntaxState { .. } => Damage::Areas(vec![]),
            // Reinitialize triggers full redraw
            Cmd::ReinitializeRenderer => Damage::Full,
            Cmd::SyncStatusBarMetrics => Damage::Full,
            // Quit doesn't need redraw - app is exiting
            Cmd::Quit => Damage::Areas(vec![]),
            Cmd::SaveRecentFiles { .. } => Damage::Areas(vec![]),
            Cmd::SaveCommandHistory { .. } => Damage::Areas(vec![]),
            Cmd::RecordInlineUsage(_) => Damage::Areas(vec![]),
            Cmd::CompletePaths(_)
            | Cmd::CancelPathCompletion
            | Cmd::PageCompletionDocumentation { .. } => Damage::Areas(vec![]),
            Cmd::CopyToClipboard(_) | Cmd::OpenWebUrl(_) => Damage::Areas(vec![]),
            Cmd::RequestClipboardPaste => Damage::Areas(vec![]),
            // Preparation completes synchronously in runtime command order and
            // can immediately open a tab or display an error.
            Cmd::SaveConfiguration { .. }
            | Cmd::ReloadConfiguration
            | Cmd::LoadTheme { .. }
            | Cmd::PrepareKeymap { .. } => Damage::Areas(vec![]),
            // Creating/closing a session also changes dock chrome immediately.
            Cmd::SpawnTerminal { .. } | Cmd::CloseTerminal { .. } => Damage::Full,
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
            Cmd::LspRequestSignatureHelp { .. } => Damage::Areas(vec![]),
            Cmd::LspRequestPrepareRename { .. } => Damage::Areas(vec![]),
            Cmd::LspRequestRename { .. } => Damage::Areas(vec![]),
            Cmd::LspRequestFormatting { .. } => Damage::Areas(vec![]),
            Cmd::LspRequestReferences { .. } | Cmd::WorkspaceSymbols(_) => Damage::Areas(vec![]),
            Cmd::LspRequestCodeActions { .. } => Damage::Areas(vec![]),
            Cmd::LspExecuteCommand { .. } => Damage::Areas(vec![]),
            Cmd::LspScheduleCompletion { .. } => Damage::Areas(vec![]),
            Cmd::ScheduleInlineRequest { .. }
            | Cmd::PrepareInlineRequest(_)
            | Cmd::RunInlineRequest(_)
            | Cmd::CancelInlineRequest => Damage::Areas(vec![]),
            Cmd::LspCancelCompletion { .. } => Damage::Areas(vec![]),
            Cmd::LspResolveCompletionItem { .. } => Damage::Areas(vec![]),
            Cmd::LspScheduleResolve { .. } => Damage::Areas(vec![]),
            // No immediate visual effect; a `ServerStateChanged` (or the
            // batched `Cmd::Redraw`/`redraw_status_bar` these are always
            // paired with at the call site) requests its own redraw.
            Cmd::LspSetEnabled { .. } => Damage::Areas(vec![]),
            Cmd::LspSetServerEnabled { .. } => Damage::Areas(vec![]),
            Cmd::LspRespondToServer { .. } => Damage::Areas(vec![]),
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
    fn palette_registry_has_unique_ids_and_roundtrippable_actions() {
        let mut ids = std::collections::HashSet::new();
        for def in all_commands() {
            assert!(
                ids.insert(def.id),
                "duplicate palette identity: {:?}",
                def.id
            );
            if let Some(action) = def.action {
                assert_eq!(format!("{action:?}").parse::<KeymapCommand>(), Ok(action));
                assert_eq!(def.id.to_keymap_command(), Some(action));
            }
        }
    }

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
