//! In-editor debug overlay for real-time state visibility
//!
//! Toggle with F8 in debug builds.

use std::collections::VecDeque;
use std::time::Instant;

use crate::model::{AppModel, EditorState};

/// Maximum number of messages to retain in history
const MESSAGE_HISTORY_SIZE: usize = 50;

/// Maximum number of syntax events to retain
const SYNTAX_EVENT_HISTORY_SIZE: usize = 20;

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
    /// Show syntax highlighting state
    pub show_syntax: bool,
    /// Recent message history
    pub message_history: VecDeque<MessageEntry>,
    /// Recent syntax events history
    pub syntax_events: VecDeque<SyntaxEventEntry>,
}

#[derive(Debug, Clone)]
pub struct MessageEntry {
    pub timestamp: Instant,
    pub msg_type: String,
    pub cursor_diff: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SyntaxEventEntry {
    pub timestamp: Instant,
    pub event_type: SyntaxEventType,
    pub doc_id: u64,
    pub revision: u64,
    pub details: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyntaxEventType {
    ParseScheduled,
    ParseStarted,
    ParseCompleted,
    ParseStale,
    HighlightsCleared,
    HighlightsApplied,
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
            show_syntax: true,
            message_history: VecDeque::with_capacity(MESSAGE_HISTORY_SIZE),
            syntax_events: VecDeque::with_capacity(SYNTAX_EVENT_HISTORY_SIZE),
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

    /// Record a syntax event
    pub fn record_syntax_event(
        &mut self,
        event_type: SyntaxEventType,
        doc_id: u64,
        revision: u64,
        details: String,
    ) {
        if self.syntax_events.len() >= SYNTAX_EVENT_HISTORY_SIZE {
            self.syntax_events.pop_front();
        }
        self.syntax_events.push_back(SyntaxEventEntry {
            timestamp: Instant::now(),
            event_type,
            doc_id,
            revision,
            details,
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

        if self.show_syntax {
            lines.push(String::new());
            lines.extend(self.render_syntax_info(model));
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

    fn render_syntax_info(&self, model: &AppModel) -> Vec<String> {
        let mut lines = vec!["Syntax Highlighting:".to_string()];

        // Get current document info
        if let Some(doc_id) = model.document().id {
            let doc = model.document();
            let lang_name = doc.language.display_name();
            let revision = doc.revision;
            let _has_highlights = doc.syntax_highlights.is_some();

            lines.push(format!("  Doc ID: {} rev: {}", doc_id.0, revision));
            lines.push(format!("  Language: {}", lang_name));

            if let Some(ref highlights) = doc.syntax_highlights {
                let line_count = highlights.lines.len();
                let total_tokens: usize = highlights.lines.values().map(|lh| lh.tokens.len()).sum();
                let hl_revision = highlights.revision;
                let revision_match = if hl_revision == revision { "✓" } else { "✗ STALE" };

                lines.push(format!(
                    "  Highlights: {} lines, {} tokens (rev {} {})",
                    line_count, total_tokens, hl_revision, revision_match
                ));

                // Show tokens for the current cursor line
                let cursor_line = model.editor().primary_cursor().line;
                if let Some(line_highlights) = highlights.lines.get(&cursor_line) {
                    lines.push(format!("  Line {} tokens:", cursor_line));
                    for (i, tok) in line_highlights.tokens.iter().take(5).enumerate() {
                        let hl_name = crate::syntax::HIGHLIGHT_NAMES
                            .get(tok.highlight as usize)
                            .unwrap_or(&"?");
                        lines.push(format!(
                            "    {}: col {}..{} @{}",
                            i, tok.start_col, tok.end_col, hl_name
                        ));
                    }
                    if line_highlights.tokens.len() > 5 {
                        lines.push(format!(
                            "    ... {} more tokens",
                            line_highlights.tokens.len() - 5
                        ));
                    }
                } else {
                    lines.push(format!("  Line {} tokens: (none)", cursor_line));
                }
            } else {
                lines.push("  Highlights: NONE (pending parse)".to_string());
            }
        } else {
            lines.push("  No document".to_string());
        }

        // Show recent syntax events
        if !self.syntax_events.is_empty() {
            lines.push(String::new());
            lines.push("Recent Syntax Events:".to_string());
            for entry in self.syntax_events.iter().rev().take(8) {
                let age_ms = entry.timestamp.elapsed().as_millis();
                let event_symbol = match entry.event_type {
                    SyntaxEventType::ParseScheduled => "📋",
                    SyntaxEventType::ParseStarted => "⚙️",
                    SyntaxEventType::ParseCompleted => "✅",
                    SyntaxEventType::ParseStale => "⏭️",
                    SyntaxEventType::HighlightsCleared => "🗑️",
                    SyntaxEventType::HighlightsApplied => "🎨",
                };
                lines.push(format!(
                    "  [{:>4}ms] {} rev:{} {}",
                    age_ms, event_symbol, entry.revision, entry.details
                ));
            }
        }

        lines
    }
}
