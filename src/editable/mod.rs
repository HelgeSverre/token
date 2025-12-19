//! Unified text editing system for the Token editor.
//!
//! This module provides a unified abstraction for text editing across all contexts:
//! - Main document editor (multi-line, multi-cursor)
//! - Command palette input (single-line)
//! - Go-to-line dialog (single-line, numeric only)
//! - Find/Replace inputs (single-line)
//! - CSV cell editor (single-line)
//!
//! # Architecture
//!
//! The core components are:
//!
//! - [`TextBuffer`] / [`TextBufferMut`]: Traits abstracting over buffer implementations
//! - [`StringBuffer`]: Buffer for single-line inputs (backed by `String`)
//! - [`RopeBuffer`]: Buffer for multi-line documents (backed by `ropey::Rope`)
//! - [`EditableState`]: Main state container with cursor, selection, and history
//! - [`EditConstraints`]: Context-specific restrictions
//! - [`EditContext`]: Identifies which editing context a message is for
//! - [`TextEditMsg`]: Unified message type for all editing operations
//!
//! # Example
//!
//! ```ignore
//! use token::editable::{EditableState, StringBuffer, EditConstraints};
//!
//! // Create a single-line input
//! let mut state = EditableState::new(
//!     StringBuffer::from_text("hello"),
//!     EditConstraints::single_line(),
//! );
//!
//! // Move cursor and edit
//! state.move_word_right(false);
//! state.insert_char('!');
//!
//! assert_eq!(state.text(), "hello!");
//! ```

mod buffer;
mod constraints;
mod context;
mod cursor;
mod history;
mod messages;
mod selection;
mod state;

// Re-export main types
pub use buffer::{RopeBuffer, StringBuffer, TextBuffer, TextBufferMut};
pub use constraints::{CharFilter, EditConstraints};
pub use context::EditContext;
pub use cursor::{Cursor, Position};
pub use history::{EditHistory, EditOperation};
pub use messages::{MoveTarget, TextEditMsg};
pub use selection::Selection;
pub use state::EditableState;
