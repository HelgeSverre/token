//! Syntax highlighting module
//!
//! Provides tree-sitter based syntax highlighting with:
//! - Language detection from file extensions
//! - Background parsing in worker thread
//! - Highlight extraction for rendering
//!
//! ## Architecture
//!
//! ```text
//! Document Edit → Cmd::DebouncedSyntaxParse → (50ms timer)
//!              → Msg::SyntaxParseReady → Cmd::RunSyntaxParse
//!              → (worker thread) → Msg::SyntaxUpdated → Cmd::Redraw
//! ```
//!
//! ## Supported Languages (Phase 1)
//!
//! - YAML
//! - Markdown
//! - Rust

mod highlights;
mod languages;
mod parser;

pub use highlights::{
    highlight_id_for_name, HighlightId, HighlightToken, LineHighlights, SyntaxHighlights,
    HIGHLIGHT_NAMES,
};
pub use languages::LanguageId;
pub use parser::ParserState;
