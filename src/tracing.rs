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

use tracing_subscriber::{fmt, prelude::*, EnvFilter};

use crate::model::EditorState;

/// Initialize tracing subscriber
///
/// Respects RUST_LOG env var for filtering:
/// - `RUST_LOG=debug` - all debug logs
/// - `RUST_LOG=cursor=trace,selection=debug` - scoped filtering
/// - `RUST_LOG=token::update::editor=debug` - module-level filtering
pub fn init() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn"));

    tracing_subscriber::registry()
        .with(fmt::layer().with_target(true).with_line_number(true))
        .with(filter)
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
