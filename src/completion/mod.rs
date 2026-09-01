//! Menu completion (autocomplete.md Phase 1: buffer words + snippets;
//! lsp-integration.md Phase 5 adds the LSP source), fuzzy-filtered,
//! rendered through the overlay-surface Completion context.
//!
//! Inline suggestions (ghost text, FIM backends) are a later phase and have
//! no code here yet.

pub mod lsp;
pub mod menu;
pub mod sources;

pub use menu::{CompletionMenuState, MenuItem, MenuSourceId};
