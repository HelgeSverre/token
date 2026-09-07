//! Shared editing primitives and small-field editing state for the Token editor.
//!
//! Cursor, position and selection types are shared with document editors. The
//! editable state and buffer abstractions serve small text fields:
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
//! - [`EditableState`]: Main state container with cursor, selection, and history
//! - [`EditConstraints`]: Context-specific restrictions
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
mod cursor;
mod history;
mod messages;
mod selection;
mod state;

// Re-export main types
pub use buffer::{StringBuffer, TextBuffer, TextBufferMut};
pub use constraints::{CharFilter, EditConstraints};
pub use cursor::{Cursor, Position};
pub use history::{EditHistory, EditOperation};
pub use messages::MoveTarget;
pub use selection::Selection;
pub use state::EditableState;
