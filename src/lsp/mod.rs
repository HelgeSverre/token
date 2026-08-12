//! Language Server Protocol client.
//!
//! Phase 1 (this module, current state): pure protocol library code —
//! Content-Length framing, canonical path <-> URI conversion, and
//! editor <-> LSP position conversion. No runtime wiring yet; see
//! `docs/feature/lsp-integration.md` for the full plan.

pub mod position;
pub mod transport;
pub mod uri;

pub use position::{lsp_to_position, position_to_lsp};
pub use uri::{path_to_uri, uri_to_path};
