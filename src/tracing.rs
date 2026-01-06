//! Debug tracing infrastructure for development diagnostics
//!
//! Provides structured logging with scoped filtering for debugging
//! multi-cursor, selection, and state transition issues.
//!
//! # Usage
//!
//! Configure via RUST_LOG environment variable:
//! - `RUST_LOG=debug` - all debug logs
//! - `RUST_LOG=cursor=trace,selection=debug` - scoped filtering
//! - `RUST_LOG=token::update=debug` - module-level filtering
//!
//! # Log Files
//!
//! Logs are written to `~/.config/token-editor/logs/token.log` with daily rotation.
//! File logging uses debug level by default for more verbose troubleshooting.

use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer};

use crate::model::EditorState;

/// Initialize tracing subscriber with console and file logging
///
/// Console output respects RUST_LOG env var for filtering:
/// - `RUST_LOG=debug` - all debug logs
/// - `RUST_LOG=cursor=trace,selection=debug` - scoped filtering
/// - `RUST_LOG=token::update::editor=debug` - module-level filtering
///
/// File logging writes to `~/.config/token-editor/logs/token.log` with daily rotation.
pub fn init() {
    let console_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn"));

    // Console layer - respects RUST_LOG
    let console_layer = fmt::layer()
        .with_target(true)
        .with_line_number(true)
        .with_filter(console_filter);

    // File layer - always debug level for troubleshooting
    let file_layer = match crate::config_paths::ensure_logs_dir() {
        Ok(logs_dir) => {
            let file_appender = tracing_appender::rolling::daily(logs_dir, "token.log");
            Some(
                fmt::layer()
                    .with_writer(file_appender)
                    .with_ansi(false)
                    .with_target(true)
                    .with_line_number(true)
                    .with_filter(EnvFilter::new("debug")),
            )
        }
        Err(e) => {
            eprintln!("Warning: Could not initialize file logging: {}", e);
            None
        }
    };

    tracing_subscriber::registry()
        .with(console_layer)
        .with(file_layer)
        .init();
}

/// Lightweight snapshot of cursor/selection state for diffing
#[derive(Debug, Clone)]
pub struct CursorSnapshot {
    pub cursor_count: usize,
    pub active_idx: usize,
    pub cursors: Vec<CursorInfo>,
}

#[derive(Debug, Clone)]
pub struct CursorInfo {
    pub line: usize,
    pub column: usize,
    pub anchor: (usize, usize),
    pub head: (usize, usize),
    pub selection_empty: bool,
}

impl CursorSnapshot {
    pub fn from_editor(editor: &EditorState) -> Self {
        Self {
            cursor_count: editor.cursors.len(),
            active_idx: editor.active_cursor_index,
            cursors: editor
                .cursors
                .iter()
                .zip(&editor.selections)
                .map(|(c, s)| CursorInfo {
                    line: c.line,
                    column: c.column,
                    anchor: (s.anchor.line, s.anchor.column),
                    head: (s.head.line, s.head.column),
                    selection_empty: s.is_empty(),
                })
                .collect(),
        }
    }

    /// Generate a diff description between two snapshots
    pub fn diff(&self, other: &CursorSnapshot) -> Option<String> {
        if self.cursor_count != other.cursor_count {
            return Some(format!(
                "cursor count: {} → {}",
                self.cursor_count, other.cursor_count
            ));
        }

        let mut changes = Vec::new();
        for (i, (before, after)) in self.cursors.iter().zip(&other.cursors).enumerate() {
            if before.line != after.line || before.column != after.column {
                changes.push(format!(
                    "#{}: ({},{}) → ({},{})",
                    i, before.line, before.column, after.line, after.column
                ));
            }
            if before.selection_empty != after.selection_empty {
                let status = if after.selection_empty {
                    "cleared"
                } else {
                    "active"
                };
                changes.push(format!("#{}: selection {}", i, status));
            }
        }

        if changes.is_empty() {
            None
        } else {
            Some(changes.join("; "))
        }
    }
}
