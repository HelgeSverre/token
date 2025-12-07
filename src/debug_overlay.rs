//! In-editor debug overlay for real-time state visibility
//!
//! Toggle with F8 in debug builds.

use std::collections::VecDeque;
use std::time::Instant;

use crate::model::{AppModel, EditorState};

/// Maximum number of messages to retain in history
const MESSAGE_HISTORY_SIZE: usize = 50;

#[derive(Debug)]
pub struct DebugOverlay {
    /// Whether the overlay is visible
    pub visible: bool,
    /// Show cursor position details
    pub show_cursors: bool,
    /// Show selection ranges
    pub show_selections: bool,
    /// Show recent message history
    pub show_messages: bool,
    /// Recent message history
    pub message_history: VecDeque<MessageEntry>,
}

#[derive(Debug, Clone)]
pub struct MessageEntry {
    pub timestamp: Instant,
    pub msg_type: String,
    pub cursor_diff: Option<String>,
}

impl Default for DebugOverlay {
    fn default() -> Self {
        Self::new()
    }
}

impl DebugOverlay {
    pub fn new() -> Self {
        Self {
            visible: false,
            show_cursors: true,
            show_selections: true,
            show_messages: true,
            message_history: VecDeque::with_capacity(MESSAGE_HISTORY_SIZE),
        }
    }

    /// Toggle overlay visibility
    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    /// Record a message in history
    pub fn record_message(&mut self, msg_type: String, cursor_diff: Option<String>) {
        if self.message_history.len() >= MESSAGE_HISTORY_SIZE {
            self.message_history.pop_front();
        }
        self.message_history.push_back(MessageEntry {
            timestamp: Instant::now(),
            msg_type,
            cursor_diff,
        });
    }

    /// Generate overlay text lines for rendering
    pub fn render_lines(&self, model: &AppModel) -> Vec<String> {
        if !self.visible {
            return Vec::new();
        }

        let mut lines = vec!["─── DEBUG OVERLAY (F8 to hide) ───".to_string()];

        if let Some(editor) = model.focused_editor() {
            if self.show_cursors {
                lines.push(String::new());
                lines.extend(self.render_cursor_info(editor));
            }

            if self.show_selections {
                lines.push(String::new());
                lines.extend(self.render_selection_info(editor));
            }
        }

        if self.show_messages && !self.message_history.is_empty() {
            lines.push(String::new());
            lines.push("Recent Messages:".to_string());
            for entry in self.message_history.iter().rev().take(10) {
                let age_ms = entry.timestamp.elapsed().as_millis();
                let diff_str = entry.cursor_diff.as_deref().unwrap_or("-");
                lines.push(format!(
                    "  [{:>4}ms] {} → {}",
                    age_ms, entry.msg_type, diff_str
                ));
            }
        }

        lines
    }

    fn render_cursor_info(&self, editor: &EditorState) -> Vec<String> {
        let mut lines = vec![format!(
            "Cursors: {} (active: #{})",
            editor.cursor_count(),
            editor.active_cursor_index
        )];

        for (i, cursor) in editor.cursors.iter().enumerate() {
            let marker = if i == editor.active_cursor_index {
                "→"
            } else {
                " "
            };
            let desired = cursor
                .desired_column
                .map(|c| format!(" (desired: {})", c))
                .unwrap_or_default();
            lines.push(format!(
                "  {} #{}: L{}:C{}{}",
                marker, i, cursor.line, cursor.column, desired
            ));
        }

        lines
    }

    fn render_selection_info(&self, editor: &EditorState) -> Vec<String> {
        let mut lines = vec!["Selections:".to_string()];

        for (i, sel) in editor.selections.iter().enumerate() {
            let status = if sel.is_empty() { "empty" } else { "active" };
            let reversed = if sel.is_reversed() { " [rev]" } else { "" };
            lines.push(format!(
                "  #{}: ({},{})→({},{}) [{}]{}",
                i,
                sel.anchor.line,
                sel.anchor.column,
                sel.head.line,
                sel.head.column,
                status,
                reversed
            ));
        }

        lines
    }
}
