//! UI state - status bar, cursor blink, modals, and other UI concerns

use super::status_bar::{StatusBar, TransientMessage};
use std::time::{Duration, Instant};

// ============================================================================
// Modal System
// ============================================================================

/// Identifies which modal is currently active
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModalId {
    /// Command palette (Shift+Cmd+A)
    CommandPalette,
    /// Go to line dialog (Cmd+L)
    GotoLine,
    /// Find/Replace dialog (Cmd+F)
    FindReplace,
    /// Theme picker
    ThemePicker,
}

/// State for the command palette modal
#[derive(Debug, Clone, Default)]
pub struct CommandPaletteState {
    /// Current input text
    pub input: String,
    /// Index of selected command in filtered list
    pub selected_index: usize,
}

/// State for the goto line modal
#[derive(Debug, Clone, Default)]
pub struct GotoLineState {
    /// Current input text (line number)
    pub input: String,
}

/// State for the find/replace modal
#[derive(Debug, Clone, Default)]
pub struct FindReplaceState {
    /// Search query
    pub query: String,
    /// Replacement text
    pub replacement: String,
    /// Whether replace mode is active (vs find-only)
    pub replace_mode: bool,
    /// Case-sensitive search
    pub case_sensitive: bool,
}

/// State for the theme picker modal
#[derive(Debug, Clone, Default)]
pub struct ThemePickerState {
    /// Index of selected theme in list
    pub selected_index: usize,
}

/// Union of all modal states
#[derive(Debug, Clone)]
pub enum ModalState {
    CommandPalette(CommandPaletteState),
    GotoLine(GotoLineState),
    FindReplace(FindReplaceState),
    ThemePicker(ThemePickerState),
}

impl ModalState {
    /// Get the modal ID for this state
    pub fn id(&self) -> ModalId {
        match self {
            ModalState::CommandPalette(_) => ModalId::CommandPalette,
            ModalState::GotoLine(_) => ModalId::GotoLine,
            ModalState::FindReplace(_) => ModalId::FindReplace,
            ModalState::ThemePicker(_) => ModalId::ThemePicker,
        }
    }
}

/// UI state - status messages and cursor animation
#[derive(Debug, Clone)]
pub struct UiState {
    /// Message displayed in the status bar (legacy, kept for compatibility)
    pub status_message: String,
    /// Structured status bar with segments
    pub status_bar: StatusBar,
    /// Transient message with auto-expiry
    pub transient_message: Option<TransientMessage>,
    /// Whether the cursor is currently visible (for blinking)
    pub cursor_visible: bool,
    /// Timestamp of last cursor blink state change
    pub last_cursor_blink: Instant,
    /// Whether a file is currently being loaded
    pub is_loading: bool,
    /// Whether a file is currently being saved
    pub is_saving: bool,
    /// Currently active modal (if any)
    pub active_modal: Option<ModalState>,
}

impl UiState {
    /// Create a new UI state with default settings
    pub fn new() -> Self {
        Self {
            status_message: String::new(),
            status_bar: StatusBar::new(),
            transient_message: None,
            cursor_visible: true,
            last_cursor_blink: Instant::now(),
            is_loading: false,
            is_saving: false,
            active_modal: None,
        }
    }

    /// Create a UI state with an initial status message
    pub fn with_status(message: impl Into<String>) -> Self {
        Self {
            status_message: message.into(),
            status_bar: StatusBar::new(),
            transient_message: None,
            cursor_visible: true,
            last_cursor_blink: Instant::now(),
            is_loading: false,
            is_saving: false,
            active_modal: None,
        }
    }

    /// Check if a modal is currently active
    pub fn has_modal(&self) -> bool {
        self.active_modal.is_some()
    }

    /// Open a modal
    pub fn open_modal(&mut self, state: ModalState) {
        self.active_modal = Some(state);
    }

    /// Close the active modal
    pub fn close_modal(&mut self) {
        self.active_modal = None;
    }

    /// Reset cursor blink timer (call after user input)
    pub fn reset_cursor_blink(&mut self) {
        self.cursor_visible = true;
        self.last_cursor_blink = Instant::now();
    }

    /// Update cursor blink state based on elapsed time
    /// Returns true if the state changed (needs redraw)
    pub fn update_cursor_blink(&mut self, blink_interval: Duration) -> bool {
        if self.last_cursor_blink.elapsed() >= blink_interval {
            self.cursor_visible = !self.cursor_visible;
            self.last_cursor_blink = Instant::now();
            true
        } else {
            false
        }
    }

    /// Set the status message
    pub fn set_status(&mut self, message: impl Into<String>) {
        self.status_message = message.into();
    }

    /// Check if the UI is busy (loading or saving)
    pub fn is_busy(&self) -> bool {
        self.is_loading || self.is_saving
    }
}

impl Default for UiState {
    fn default() -> Self {
        Self::new()
    }
}
