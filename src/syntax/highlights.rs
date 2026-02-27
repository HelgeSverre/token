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

    /// Shift highlights to account for a text edit, keeping old highlights
    /// visually aligned while the background parser catches up.
    ///
    /// - `edit_line`: the line where the edit occurred (0-indexed)
    /// - `old_line_count`: number of lines in the document before the edit
    /// - `new_line_count`: number of lines in the document after the edit
    ///
    /// For insertions (new > old), lines after the edit are shifted down.
    /// For deletions (new < old), affected lines are removed and remaining shifted up.
    /// The edit line itself is cleared since its content has changed.
    pub fn shift_for_edit(
        &mut self,
        edit_line: usize,
        old_line_count: usize,
        new_line_count: usize,
    ) {
        let delta = new_line_count as isize - old_line_count as isize;
        if delta == 0 {
            // Single-line edit: just clear the edited line's highlights
            self.lines.remove(&edit_line);
            return;
        }

        let mut new_lines = HashMap::new();

        for (line, highlights) in self.lines.drain() {
            if line < edit_line {
                new_lines.insert(line, highlights);
            } else if delta > 0 {
                // Insertion: skip the edit line, shift everything after down
                if line > edit_line {
                    new_lines.insert((line as isize + delta) as usize, highlights);
                }
            } else {
                // Deletion: skip lines in the deleted range, shift rest up
                let deleted_lines = (-delta) as usize;
                if line > edit_line + deleted_lines {
                    new_lines.insert((line as isize + delta) as usize, highlights);
                }
            }
        }

        self.lines = new_lines;
    }
}

/// Look up highlight ID by capture name
pub fn highlight_id_for_name(name: &str) -> Option<HighlightId> {
    // Handle hierarchical names: try exact match first, then progressively shorter
    // parents (e.g. "keyword.control.import" -> "keyword.control" -> "keyword").
    let mut current = name;
    loop {
        if let Some(pos) = HIGHLIGHT_NAMES.iter().position(|&n| n == current) {
            return Some(pos as HighlightId);
        }

        let Some(dot_pos) = current.rfind('.') else {
            break;
        };
        current = &current[..dot_pos];
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
        assert!(highlight_id_for_name("keyword.control.import").is_some());
        assert!(highlight_id_for_name("string").is_some());
        assert!(highlight_id_for_name("nonexistent").is_none());
    }

    #[test]
    fn test_shift_for_edit_insert_line() {
        let mut highlights = SyntaxHighlights::new(super::super::LanguageId::Rust, 1);
        highlights.lines.insert(
            0,
            LineHighlights {
                tokens: vec![HighlightToken {
                    start_col: 0,
                    end_col: 2,
                    highlight: 1,
                }],
            },
        );
        highlights.lines.insert(
            1,
            LineHighlights {
                tokens: vec![HighlightToken {
                    start_col: 0,
                    end_col: 3,
                    highlight: 2,
                }],
            },
        );
        highlights.lines.insert(
            2,
            LineHighlights {
                tokens: vec![HighlightToken {
                    start_col: 0,
                    end_col: 4,
                    highlight: 3,
                }],
            },
        );

        // Insert a line at line 1 (old 3 lines -> new 4 lines)
        highlights.shift_for_edit(1, 3, 4);

        // Line 0 should be unchanged
        assert!(highlights.lines.contains_key(&0));
        // Line 1 should be cleared (edit line)
        assert!(!highlights.lines.contains_key(&1));
        // Old line 2 should now be at line 3
        assert!(highlights.lines.contains_key(&3));
        assert_eq!(highlights.lines.get(&3).unwrap().tokens[0].highlight, 3);
    }

    #[test]
    fn test_shift_for_edit_delete_line() {
        let mut highlights = SyntaxHighlights::new(super::super::LanguageId::Rust, 1);
        highlights.lines.insert(
            0,
            LineHighlights {
                tokens: vec![HighlightToken {
                    start_col: 0,
                    end_col: 2,
                    highlight: 1,
                }],
            },
        );
        highlights.lines.insert(
            1,
            LineHighlights {
                tokens: vec![HighlightToken {
                    start_col: 0,
                    end_col: 3,
                    highlight: 2,
                }],
            },
        );
        highlights.lines.insert(
            2,
            LineHighlights {
                tokens: vec![HighlightToken {
                    start_col: 0,
                    end_col: 4,
                    highlight: 3,
                }],
            },
        );
        highlights.lines.insert(
            3,
            LineHighlights {
                tokens: vec![HighlightToken {
                    start_col: 0,
                    end_col: 5,
                    highlight: 4,
                }],
            },
        );

        // Delete line at line 1 (old 4 lines -> new 3 lines)
        highlights.shift_for_edit(1, 4, 3);

        // Line 0 unchanged
        assert!(highlights.lines.contains_key(&0));
        // Lines 1-2 (edit line + deleted range) should be gone
        // Old line 3 should now be at line 2
        assert!(highlights.lines.contains_key(&2));
        assert_eq!(highlights.lines.get(&2).unwrap().tokens[0].highlight, 4);
    }

    #[test]
    fn test_shift_for_edit_same_line() {
        let mut highlights = SyntaxHighlights::new(super::super::LanguageId::Rust, 1);
        highlights.lines.insert(
            0,
            LineHighlights {
                tokens: vec![HighlightToken {
                    start_col: 0,
                    end_col: 5,
                    highlight: 1,
                }],
            },
        );
        highlights.lines.insert(
            1,
            LineHighlights {
                tokens: vec![HighlightToken {
                    start_col: 0,
                    end_col: 3,
                    highlight: 2,
                }],
            },
        );

        // Same-line edit (no line count change)
        highlights.shift_for_edit(0, 2, 2);

        // Line 0 should be cleared
        assert!(!highlights.lines.contains_key(&0));
        // Line 1 should remain
        assert!(highlights.lines.contains_key(&1));
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
