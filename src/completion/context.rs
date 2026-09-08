//! Shared source policy for dropdown completion. Syntax remains the authority
//! for code versus literals; receiver/type inference belongs to the LSP server.

use crate::model::{Cursor, Document};
use crate::syntax::{LanguageId, HIGHLIGHT_NAMES};

/// Which local sources are appropriate at a completion query's start.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CompletionContext {
    #[default]
    Text,
    Code,
    Member,
    Literal,
    Path,
    /// The background parser has not classified this revision yet.
    Unknown,
}

impl CompletionContext {
    pub fn at(document: &Document, query_start: Cursor) -> Self {
        if matches!(
            document.language,
            LanguageId::PlainText | LanguageId::Markdown
        ) {
            return Self::Text;
        }
        let fresh = document.syntax_highlights.as_ref().filter(|syntax| {
            syntax.revision == document.revision && syntax.language == document.language
        });
        if let Some(line) = fresh.and_then(|syntax| syntax.get_line(query_start.line)) {
            let kind = line
                .highlight_at(query_start.column)
                .or_else(|| {
                    query_start
                        .column
                        .checked_sub(1)
                        .and_then(|col| line.highlight_at(col))
                })
                .and_then(|id| HIGHLIGHT_NAMES.get(id as usize));
            if kind.is_some_and(|kind| {
                kind.starts_with("comment") || kind.starts_with("string") || *kind == "escape"
            }) {
                return Self::Literal;
            }
        }
        let offset = document.cursor_to_offset(query_start.line, query_start.column);
        let mut before = document
            .buffer
            .chars_at(offset)
            .reversed()
            .skip_while(|ch| ch.is_whitespace());
        if matches!(
            (before.next(), before.next()),
            (Some('.'), _) | (Some(':'), Some(':')) | (Some('>'), Some('-'))
        ) {
            return Self::Member;
        }
        if fresh.is_some() {
            Self::Code
        } else {
            Self::Unknown
        }
    }

    pub fn allows_words(self) -> bool {
        matches!(self, Self::Text | Self::Code)
    }

    pub fn allows_snippets(self) -> bool {
        self == Self::Code
    }
}

/// Reject non-identifier syntax captures. Ordinary identifiers can be uncolored
/// (Rust's highlight query deliberately leaves local variables unclassified),
/// so Unicode identifier-shaped words in uncolored code remain eligible.
/// Never use stale highlights to exclude comments/strings.
pub(super) fn identifier_at(document: &Document, line: usize, column: usize) -> bool {
    let Some(syntax) = document.syntax_highlights.as_ref().filter(|syntax| {
        syntax.revision == document.revision && syntax.language == document.language
    }) else {
        return false;
    };
    syntax
        .get_line(line)
        .and_then(|line| line.highlight_at(column))
        .and_then(|id| HIGHLIGHT_NAMES.get(id as usize))
        .is_none_or(|kind| {
            [
                "variable",
                "function",
                "type",
                "property",
                "constant",
                "constructor",
                "label",
            ]
            .iter()
            .any(|prefix| {
                kind.strip_prefix(prefix)
                    .is_some_and(|tail| tail.is_empty() || tail.starts_with('.'))
            })
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syntax::ParserState;

    #[test]
    fn member_separators_include_multiline_chains_and_partial_queries() {
        for (text, start) in [
            ("builder.", 8),
            ("Build::", 7),
            ("ptr->", 5),
            ("builder.compile", 8),
        ] {
            let mut doc = Document::with_text(text);
            doc.language = LanguageId::Rust;
            assert_eq!(
                CompletionContext::at(&doc, Cursor::at(0, start)),
                CompletionContext::Member
            );
        }
        let mut doc =
            Document::with_text("cc::Build::new()\n    .file(scanner)\n    .\n    compile");
        doc.language = LanguageId::Rust;
        assert_eq!(
            CompletionContext::at(&doc, Cursor::at(3, 4)),
            CompletionContext::Member
        );
    }

    #[test]
    fn fresh_syntax_distinguishes_code_literals_and_comments() {
        let mut doc = Document::with_text(
            "fn main() {\nlet value = 1;\n// value_comment\nlet text = \"value_string\";\n}\n",
        );
        doc.language = LanguageId::Rust;
        assert_eq!(
            CompletionContext::at(&doc, Cursor::at(1, 4)),
            CompletionContext::Unknown
        );
        doc.syntax_highlights = Some(ParserState::new().parse_and_highlight(
            &doc.buffer.to_string(),
            doc.language,
            crate::model::DocumentId(1),
            doc.revision,
        ));
        assert_eq!(
            CompletionContext::at(&doc, Cursor::at(1, 4)),
            CompletionContext::Code
        );
        assert_eq!(
            CompletionContext::at(&doc, Cursor::at(2, 3)),
            CompletionContext::Literal
        );
        assert_eq!(
            CompletionContext::at(&doc, Cursor::at(3, 12)),
            CompletionContext::Literal
        );
        assert!(identifier_at(&doc, 1, 4));
        assert!(!identifier_at(&doc, 2, 3));
        assert!(!identifier_at(&doc, 3, 12));
        doc.revision += 1;
        assert!(!identifier_at(&doc, 1, 4));
        assert_eq!(
            CompletionContext::at(&doc, Cursor::at(1, 4)),
            CompletionContext::Unknown
        );
    }
}
