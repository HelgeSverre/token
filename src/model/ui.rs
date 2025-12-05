//! UI state - status bar, cursor blink, and other UI concerns

use super::status_bar::{StatusBar, TransientMessage};
use std::time::{Duration, Instant};

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
        }
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
