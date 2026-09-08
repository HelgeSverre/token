//! Menu completion (autocomplete.md Phase 1: buffer words + snippets;
//! lsp-integration.md Phase 5 adds the LSP source), fuzzy-filtered,
//! rendered through the overlay-surface Completion context.
//!
//! Inline suggestions share a cancelable provider boundary and pure ghost state.

pub mod context;
pub mod fim;
pub mod inline;
pub mod lsp;
pub mod menu;
pub mod path;
pub mod postprocess;
pub mod prompt;
pub mod provider;
pub mod recency;
pub mod sources;
pub mod statistics;

pub use menu::{CompletionMenuState, MenuItem, MenuSourceId};
