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
//! ## Supported Languages
//!
//! - YAML
//! - Markdown
//! - Rust

mod compat;
mod highlights;
mod languages;
mod parser;
pub(crate) mod registry;
mod selection;

pub use highlights::{
    highlight_id_for_name, HighlightId, HighlightToken, LineHighlights, SyntaxHighlights,
    HIGHLIGHT_NAMES,
};
pub use languages::LanguageId;
pub use parser::{ParserState, ParserTiming};
pub use selection::{expansion_candidates, InjectedSyntaxTree, SyntaxTreeSnapshot};
