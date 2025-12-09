//! Command types for the Elm-style architecture
//!
//! Commands represent side effects that should be performed after an update.

use std::path::PathBuf;

// ============================================================================
// Command Palette Registry
// ============================================================================

/// Identifies a command that can be executed via the command palette
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CommandId {
    // File operations
    NewFile,
    SaveFile,

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
        id: CommandId::SaveFile,
        label: "Save File",
        keybinding: Some("⌘S"),
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
];

/// Filter commands by a search query (fuzzy match on label)
pub fn filter_commands(query: &str) -> Vec<&'static CommandDef> {
    if query.is_empty() {
        return COMMANDS.iter().collect();
    }

    let query_lower = query.to_lowercase();
    COMMANDS
        .iter()
        .filter(|cmd| {
            let label_lower = cmd.label.to_lowercase();
            // Simple substring match for now
            label_lower.contains(&query_lower)
        })
        .collect()
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
    /// Execute multiple commands
    Batch(Vec<Cmd>),
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
            Cmd::SaveFile { .. } => true, // Show "Saving..." status
            Cmd::LoadFile { .. } => true, // Show "Loading..." status
            Cmd::Batch(cmds) => cmds.iter().any(|c| c.needs_redraw()),
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
