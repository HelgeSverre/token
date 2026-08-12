//! Menu completion (autocomplete.md Phase 1): buffer words + snippets,
//! fuzzy-filtered, rendered through the overlay-surface Completion context.
//!
//! Inline suggestions (ghost text, FIM backends) are a later phase and have
//! no code here yet.

pub mod menu;
pub mod sources;

pub use menu::{CompletionMenuState, MenuItem, MenuSourceId};
