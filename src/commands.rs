//! Command types for the Elm-style architecture
//!
//! Commands represent side effects that should be performed after an update.

use std::path::PathBuf;

/// Commands returned by update functions
#[derive(Debug, Clone)]
pub enum Cmd {
    /// No command - do nothing
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

impl Default for Cmd {
    fn default() -> Self {
        Cmd::None
    }
}

// Allow converting Option<Cmd> to Cmd
impl From<Option<Cmd>> for Cmd {
    fn from(opt: Option<Cmd>) -> Self {
        opt.unwrap_or(Cmd::None)
    }
}
