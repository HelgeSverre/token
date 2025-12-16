//! Command types for the Elm-style architecture
//!
//! Commands represent side effects that should be performed after an update.

use std::path::PathBuf;

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
    OpenFolder,
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

    // CSV
    ToggleCsvView,
}

/// A command definition for the command palette
#[derive(Debug, Clone)]
pub struct CommandDef {
    pub id: CommandId,
    pub label: &'static str,
    pub keybinding: Option<&'static str>,
}

/// Static registry of all available commands
pub static COMMANDS: &[CommandDef] = &[
    CommandDef {
        id: CommandId::NewFile,
        label: "New File",
        keybinding: Some("⇧⌘N"),
    },
    CommandDef {
        id: CommandId::OpenFile,
        label: "Open File...",
        keybinding: Some("⌘O"),
    },
    CommandDef {
        id: CommandId::OpenFolder,
        label: "Open Folder...",
        keybinding: Some("⇧⌘O"),
    },
    CommandDef {
        id: CommandId::SaveFile,
        label: "Save File",
        keybinding: Some("⌘S"),
    },
    CommandDef {
        id: CommandId::SaveFileAs,
        label: "Save File As...",
        keybinding: Some("⇧⌘S"),
    },
    CommandDef {
        id: CommandId::Undo,
        label: "Undo",
        keybinding: Some("⌘Z"),
    },
    CommandDef {
        id: CommandId::Redo,
        label: "Redo",
        keybinding: Some("⇧⌘Z"),
    },
    CommandDef {
        id: CommandId::Cut,
        label: "Cut",
        keybinding: Some("⌘X"),
    },
    CommandDef {
        id: CommandId::Copy,
        label: "Copy",
        keybinding: Some("⌘C"),
    },
    CommandDef {
        id: CommandId::Paste,
        label: "Paste",
        keybinding: Some("⌘V"),
    },
    CommandDef {
        id: CommandId::SelectAll,
        label: "Select All",
        keybinding: Some("⌘A"),
    },
    CommandDef {
        id: CommandId::GotoLine,
        label: "Go to Line...",
        keybinding: Some("⌘L"),
    },
    CommandDef {
        id: CommandId::SplitHorizontal,
        label: "Split Editor Right",
        keybinding: Some("⇧⌥⌘H"),
    },
    CommandDef {
        id: CommandId::SplitVertical,
        label: "Split Editor Down",
        keybinding: Some("⇧⌥⌘V"),
    },
    CommandDef {
        id: CommandId::CloseGroup,
        label: "Close Editor Group",
        keybinding: None,
    },
    CommandDef {
        id: CommandId::NextTab,
        label: "Next Tab",
        keybinding: Some("⌥⌘→"),
    },
    CommandDef {
        id: CommandId::PrevTab,
        label: "Previous Tab",
        keybinding: Some("⌥⌘←"),
    },
    CommandDef {
        id: CommandId::CloseTab,
        label: "Close Tab",
        keybinding: Some("⌘W"),
    },
    CommandDef {
        id: CommandId::Find,
        label: "Find...",
        keybinding: Some("⌘F"),
    },
    CommandDef {
        id: CommandId::ShowCommandPalette,
        label: "Show Command Palette",
        keybinding: Some("⇧⌘A"),
    },
    CommandDef {
        id: CommandId::SwitchTheme,
        label: "Switch Theme...",
        keybinding: None,
    },
    CommandDef {
        id: CommandId::OpenConfigDirectory,
        label: "Open Config Directory",
        keybinding: None,
    },
    CommandDef {
        id: CommandId::OpenKeybindings,
        label: "Open Keymap",
        keybinding: None,
    },
    CommandDef {
        id: CommandId::ToggleCsvView,
        label: "Toggle CSV View",
        keybinding: None,
    },
];

/// Calculate fuzzy match score. Returns None if no match, Some(score) if matches.
/// Higher score = better match. Consecutive matches and word-start matches score higher.
fn fuzzy_match_score(query: &str, target: &str) -> Option<i32> {
    let query_chars: Vec<char> = query.to_lowercase().chars().collect();
    let target_lower = target.to_lowercase();
    let target_chars: Vec<char> = target_lower.chars().collect();

    if query_chars.is_empty() {
        return Some(0);
    }

    let mut query_idx = 0;
    let mut score = 0;
    let mut prev_matched = false;
    let mut prev_was_separator = true; // Start of string counts as separator

    for (i, &tc) in target_chars.iter().enumerate() {
        let is_separator = tc == ' ' || tc == '_' || tc == '-';

        if query_idx < query_chars.len() && tc == query_chars[query_idx] {
            // Match found
            score += 1;

            // Bonus for consecutive matches
            if prev_matched {
                score += 2;
            }

            // Bonus for matching at word start (after separator or at beginning)
            if prev_was_separator {
                score += 3;
            }

            // Bonus for matching at string start
            if i == 0 {
                score += 5;
            }

            query_idx += 1;
            prev_matched = true;
        } else {
            prev_matched = false;
        }

        prev_was_separator = is_separator;
    }

    // All query chars must be found
    if query_idx == query_chars.len() {
        Some(score)
    } else {
        None
    }
}

/// Filter commands by a search query (fuzzy match on label)
pub fn filter_commands(query: &str) -> Vec<&'static CommandDef> {
    if query.is_empty() {
        return COMMANDS.iter().collect();
    }

    let mut matches: Vec<(&'static CommandDef, i32)> = COMMANDS
        .iter()
        .filter_map(|cmd| fuzzy_match_score(query, cmd.label).map(|score| (cmd, score)))
        .collect();

    // Sort by score descending (best matches first)
    matches.sort_by(|a, b| b.1.cmp(&a.1));

    matches.into_iter().map(|(cmd, _)| cmd).collect()
}

/// Map CommandId to keymap::Command for keybinding lookup
impl CommandId {
    pub fn to_keymap_command(self) -> Option<KeymapCommand> {
        match self {
            CommandId::NewFile => Some(KeymapCommand::NewTab), // NewFile maps to NewTab
            CommandId::OpenFile => Some(KeymapCommand::OpenFile),
            CommandId::OpenFolder => Some(KeymapCommand::OpenFolder),
            CommandId::SaveFile => Some(KeymapCommand::SaveFile),
            CommandId::SaveFileAs => Some(KeymapCommand::SaveFileAs),
            CommandId::Undo => Some(KeymapCommand::Undo),
            CommandId::Redo => Some(KeymapCommand::Redo),
            CommandId::Cut => Some(KeymapCommand::Cut),
            CommandId::Copy => Some(KeymapCommand::Copy),
            CommandId::Paste => Some(KeymapCommand::Paste),
            CommandId::SelectAll => Some(KeymapCommand::SelectAll),
            CommandId::GotoLine => Some(KeymapCommand::ToggleGotoLine),
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
            CommandId::ToggleCsvView => Some(KeymapCommand::CsvToggle),
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
// Side-Effect Commands (returned from update)
// ============================================================================

/// Commands returned by update functions
#[derive(Debug, Clone, Default)]
pub enum Cmd {
    /// No command - do nothing
    #[default]
    None,
    /// Request a redraw of the UI
    Redraw,
    /// Save file asynchronously
    SaveFile { path: PathBuf, content: String },
    /// Load file asynchronously
    LoadFile { path: PathBuf },
    /// Open a path in the system file explorer/finder
    OpenInExplorer { path: PathBuf },
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
        source: String,
        language: LanguageId,
    },

    // === Display Commands ===
    /// Reinitialize the renderer (e.g., after scale factor change)
    ReinitializeRenderer,
}

impl Cmd {
    /// Create a batch of commands
    pub fn batch(cmds: Vec<Cmd>) -> Self {
        Cmd::Batch(cmds)
    }

    /// Check if this command requires a redraw
    pub fn needs_redraw(&self) -> bool {
        match self {
            Cmd::None => false,
            Cmd::Redraw => true,
            Cmd::SaveFile { .. } => true,
            Cmd::LoadFile { .. } => true,
            Cmd::OpenInExplorer { .. } => true,
            Cmd::OpenFileInEditor { .. } => true,
            Cmd::Batch(cmds) => cmds.iter().any(|c| c.needs_redraw()),
            // Dialogs don't need immediate redraw - they'll trigger messages when done
            Cmd::ShowOpenFileDialog { .. } => false,
            Cmd::ShowSaveFileDialog { .. } => false,
            Cmd::ShowOpenFolderDialog { .. } => false,
            // Syntax commands don't need immediate redraw - ParseCompleted triggers redraw
            Cmd::DebouncedSyntaxParse { .. } => false,
            Cmd::RunSyntaxParse { .. } => false,
            // Reinitialize triggers a full redraw after renderer is recreated
            Cmd::ReinitializeRenderer => true,
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
