//! Embedded terminal panel: PTY spawning + VT100/ANSI terminal emulation.
//!
//! Token owns PTY process management (`pty.rs`) and rendering (via the
//! existing fontdue `TextPainter`, wired in `src/panels/terminal.rs` in a
//! later phase). Terminal escape-sequence parsing and grid/cursor/scrollback
//! state are delegated to `alacritty_terminal`, wrapped by `TerminalSession`
//! so the dependency can be swapped later without touching callers.
//!
//! See `docs/archived/embedded-terminal.md` for the original MVP design.

mod links;
mod pty;
mod session;
pub mod translate_keys;

pub use links::TerminalLink;
pub use pty::{spawn_pty, PtyHandle};
pub use session::{TerminalEventProxy, TerminalSession};
pub use translate_keys::{translate_key, TerminalKeyModifiers};

/// Actions shared by terminal tab buttons, keyboard commands and updates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TabAction {
    New,
    Close,
    Previous,
    Next,
    Select(usize),
}

/// Result returned by a background PTY spawn, consumed on the main thread to
/// build a [`TerminalSession`]. Kept outside the `Msg` enum because `PtyHandle`
/// is not `Clone`.
pub struct TerminalSpawnResult {
    pub session_id: usize,
    pub rows: usize,
    pub cols: usize,
    pub pty: PtyHandle,
}

/// Terminal state — lives in `AppModel` alongside `dock_layout`.
///
/// Visibility, dock height, and resizing are owned by `Dock`; this only
/// tracks the terminal sessions themselves.
#[derive(Default)]
pub struct TerminalState {
    /// Active terminal sessions.
    pub sessions: Vec<TerminalSession>,
    /// Index of the active session (into `sessions`).
    pub active: usize,
    /// Horizontal tab-strip offset in physical pixels.
    pub tab_scroll: f32,
    pub hovered_tab: Option<TabAction>,
    pub hovered_link: Option<(usize, TerminalLink)>,
    /// Session that owns the current pointer-selection gesture.
    pub selection_drag: Option<usize>,
    next_session_id: usize,
    /// Session ids whose PTY spawn command has been issued but whose
    /// `TerminalSession` has not been installed yet.
    pending_spawn_ids: Vec<usize>,
}

// `alacritty_terminal::Term` doesn't implement `Debug`, so this is written
// by hand rather than derived (`AppModel` derives `Debug` and holds this
// transitively).
impl std::fmt::Debug for TerminalState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TerminalState")
            .field("session_count", &self.sessions.len())
            .field("active", &self.active)
            .field("pending_spawn_count", &self.pending_spawn_ids.len())
            .finish()
    }
}

impl TerminalState {
    /// The currently active session, if any.
    pub fn active_session(&self) -> Option<&TerminalSession> {
        self.sessions.get(self.active)
    }

    /// Mutable access to the currently active session, if any.
    pub fn active_session_mut(&mut self) -> Option<&mut TerminalSession> {
        self.sessions.get_mut(self.active)
    }

    /// Find a session by its id, mutably.
    pub fn session_mut(&mut self, session_id: usize) -> Option<&mut TerminalSession> {
        self.sessions.iter_mut().find(|s| s.id == session_id)
    }

    /// Whether any terminal spawn is currently in progress.
    pub fn has_pending_spawn(&self) -> bool {
        !self.pending_spawn_ids.is_empty()
    }

    /// Whether the given session id is currently spawning.
    pub fn is_spawn_pending(&self, session_id: usize) -> bool {
        self.pending_spawn_ids.contains(&session_id)
    }

    /// Record that a PTY spawn has been requested for this session id.
    pub fn mark_spawn_pending(&mut self, session_id: usize) {
        self.next_session_id = self.next_session_id.max(session_id.saturating_add(1));
        if !self.is_spawn_pending(session_id) {
            self.pending_spawn_ids.push(session_id);
        }
    }

    /// Clear the pending marker for a completed or discarded spawn.
    pub fn clear_spawn_pending(&mut self, session_id: usize) {
        self.pending_spawn_ids.retain(|id| *id != session_id);
    }

    /// Reserve a never-reused identity; the runtime serializes PTY startup.
    pub fn begin_spawn(&mut self) -> Option<usize> {
        if self.has_pending_spawn() {
            return None;
        }
        let id = self.next_session_id;
        self.next_session_id = id.checked_add(1)?;
        self.mark_spawn_pending(id);
        Some(id)
    }
}
