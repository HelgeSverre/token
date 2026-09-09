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
pub mod folding;
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

/// Highlight a small, independent documentation snippet with the editor's
/// grammars. Reuse compiled queries, but never retain snippet document trees.
/// Call during documentation conversion, not layout or painting.
pub(crate) fn highlight_snippet(source: &str, language: LanguageId) -> SyntaxHighlights {
    if source.is_empty()
        || language == LanguageId::PlainText
        || source.len() > crate::util::ByteSize::kibibytes(32).as_usize()
    {
        return SyntaxHighlights::new(language, 0);
    }
    thread_local! {
        static PARSER: std::cell::RefCell<ParserState> =
            std::cell::RefCell::new(ParserState::new());
    }
    PARSER.with_borrow_mut(|parser| {
        // This parser is private to snippets, separate from editor documents.
        let document = crate::model::DocumentId(0);
        let highlights = parser.parse_and_highlight(source, language, document, 0);
        parser.clear_doc_cache(document);
        highlights
    })
}
