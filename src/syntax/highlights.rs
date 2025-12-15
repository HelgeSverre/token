//! Syntax highlighting data structures
//!
//! Defines tokens, line highlights, and document-level syntax state.

use std::collections::HashMap;

use super::languages::LanguageId;

/// Standard tree-sitter capture names mapped to theme colors.
/// Index into this array is the HighlightId.
pub const HIGHLIGHT_NAMES: &[&str] = &[
    "attribute",             // @attribute
    "boolean",               // @boolean (true, false)
    "comment",               // @comment
    "constant",              // @constant
    "constant.builtin",      // @constant.builtin (null, nil)
    "constructor",           // @constructor (new Foo)
    "escape",                // @escape (string escapes)
    "function",              // @function
    "function.builtin",      // @function.builtin (echo, print)
    "function.method",       // @function.method
    "keyword",               // @keyword
    "keyword.return",        // @keyword.return
    "keyword.function",      // @keyword.function (function, fn)
    "keyword.operator",      // @keyword.operator (and, or)
    "label",                 // @label (anchors, aliases in YAML)
    "number",                // @number
    "operator",              // @operator
    "property",              // @property
    "punctuation",           // @punctuation (general)
    "punctuation.bracket",   // @punctuation.bracket
    "punctuation.delimiter", // @punctuation.delimiter
    "punctuation.special",   // @punctuation.special
    "string",                // @string
    "string.special",        // @string.special (regex, heredoc)
    "tag",                   // @tag (HTML tags, YAML tags)
    "tag.attribute",         // @tag.attribute
    "text",                  // @text (plain text in markdown)
    "text.emphasis",         // @text.emphasis (*italic*)
    "text.strong",           // @text.strong (**bold**)
    "text.title",            // @text.title (headings)
    "text.uri",              // @text.uri (URLs)
    "type",                  // @type
    "type.builtin",          // @type.builtin (int, string, bool)
    "variable",              // @variable
    "variable.builtin",      // @variable.builtin ($this, self)
    "variable.parameter",    // @variable.parameter
];

/// Index into HIGHLIGHT_NAMES
pub type HighlightId = u16;

/// A single highlighted span within a line
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HighlightToken {
    /// Start column (0-indexed, inclusive)
    pub start_col: usize,
    /// End column (exclusive)
    pub end_col: usize,
    /// Index into HIGHLIGHT_NAMES
    pub highlight: HighlightId,
}

/// Highlight information for a single line
#[derive(Debug, Clone, Default)]
pub struct LineHighlights {
    /// Tokens sorted by start_col
    pub tokens: Vec<HighlightToken>,
}

impl LineHighlights {
    /// Get the highlight ID for a given column, if any
    pub fn highlight_at(&self, col: usize) -> Option<HighlightId> {
        for token in &self.tokens {
            if col >= token.start_col && col < token.end_col {
                return Some(token.highlight);
            }
            if token.start_col > col {
                break; // tokens are sorted, no need to continue
            }
        }
        None
    }
}

/// Complete highlight state for a document
#[derive(Debug, Clone)]
pub struct SyntaxHighlights {
    /// Map of line number (0-indexed) → tokens
    pub lines: HashMap<usize, LineHighlights>,
    /// Document revision this corresponds to
    pub revision: u64,
    /// Primary language of document
    pub language: LanguageId,
}

impl Default for SyntaxHighlights {
    fn default() -> Self {
        Self {
            lines: HashMap::new(),
            revision: 0,
            language: LanguageId::PlainText,
        }
    }
}

impl SyntaxHighlights {
    /// Create new empty highlights for a language
    pub fn new(language: LanguageId, revision: u64) -> Self {
        Self {
            lines: HashMap::new(),
            revision,
            language,
        }
    }

    /// Get highlights for a specific line
    pub fn get_line(&self, line: usize) -> Option<&LineHighlights> {
        self.lines.get(&line)
    }

    /// Get highlight tokens for a line, or empty slice if none
    pub fn get_line_tokens(&self, line: usize) -> &[HighlightToken] {
        self.lines
            .get(&line)
            .map(|lh| lh.tokens.as_slice())
            .unwrap_or(&[])
    }
}

/// Look up highlight ID by capture name
pub fn highlight_id_for_name(name: &str) -> Option<HighlightId> {
    // Handle hierarchical names: "keyword.function" should match "keyword.function",
    // but "keyword" should also match as a fallback
    if let Some(pos) = HIGHLIGHT_NAMES.iter().position(|&n| n == name) {
        return Some(pos as HighlightId);
    }

    // Try parent capture (e.g., "keyword.function" -> "keyword")
    if let Some(dot_pos) = name.rfind('.') {
        let parent = &name[..dot_pos];
        if let Some(pos) = HIGHLIGHT_NAMES.iter().position(|&n| n == parent) {
            return Some(pos as HighlightId);
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_highlight_id_lookup() {
        assert!(highlight_id_for_name("keyword").is_some());
        assert!(highlight_id_for_name("keyword.function").is_some());
        assert!(highlight_id_for_name("string").is_some());
        assert!(highlight_id_for_name("nonexistent").is_none());
    }

    #[test]
    fn test_line_highlights_at() {
        let line = LineHighlights {
            tokens: vec![
                HighlightToken {
                    start_col: 0,
                    end_col: 5,
                    highlight: 1,
                },
                HighlightToken {
                    start_col: 10,
                    end_col: 15,
                    highlight: 2,
                },
            ],
        };

        assert_eq!(line.highlight_at(0), Some(1));
        assert_eq!(line.highlight_at(4), Some(1));
        assert_eq!(line.highlight_at(5), None);
        assert_eq!(line.highlight_at(10), Some(2));
        assert_eq!(line.highlight_at(14), Some(2));
        assert_eq!(line.highlight_at(15), None);
    }
}
