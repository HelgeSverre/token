//! Tree-sitter parser state and highlighting extraction
//!
//! Manages parsers, trees, and queries for syntax highlighting.

use std::collections::HashMap;

use streaming_iterator::StreamingIterator;
use tree_sitter::{Parser, Query, QueryCursor, Tree};

use super::highlights::{highlight_id_for_name, HighlightToken, SyntaxHighlights};
use super::languages::LanguageId;
use crate::model::editor_area::DocumentId;

// Embedded query files
// Use our custom queries for YAML and Markdown
const YAML_HIGHLIGHTS: &str = include_str!("../../queries/yaml/highlights.scm");
const MARKDOWN_HIGHLIGHTS: &str = include_str!("../../queries/markdown/highlights.scm");
// Use the built-in queries for languages that provide them
const RUST_HIGHLIGHTS: &str = tree_sitter_rust::HIGHLIGHTS_QUERY;
const HTML_HIGHLIGHTS: &str = include_str!("../../queries/html/highlights.scm");
const CSS_HIGHLIGHTS: &str = include_str!("../../queries/css/highlights.scm");
const JAVASCRIPT_HIGHLIGHTS: &str = include_str!("../../queries/javascript/highlights.scm");

/// Thread-local parser state (tree-sitter parsers are !Sync)
pub struct ParserState {
    /// Parser instances per language
    parsers: HashMap<LanguageId, Parser>,
    /// Compiled queries per language
    queries: HashMap<LanguageId, Query>,
}

impl ParserState {
    /// Create a new parser state with initialized languages
    pub fn new() -> Self {
        let mut state = Self {
            parsers: HashMap::new(),
            queries: HashMap::new(),
        };

        // Initialize Phase 1 languages
        state.init_language(LanguageId::Yaml);
        state.init_language(LanguageId::Markdown);
        state.init_language(LanguageId::Rust);

        // Initialize Phase 2 languages (web stack)
        state.init_language(LanguageId::Html);
        state.init_language(LanguageId::Css);
        state.init_language(LanguageId::JavaScript);

        state
    }

    /// Initialize a language's parser and query
    fn init_language(&mut self, lang: LanguageId) {
        let (ts_lang, highlights_scm) = match lang {
            LanguageId::Yaml => (tree_sitter_yaml::language(), YAML_HIGHLIGHTS),
            LanguageId::Markdown => (tree_sitter_md::LANGUAGE.into(), MARKDOWN_HIGHLIGHTS),
            LanguageId::Rust => (tree_sitter_rust::LANGUAGE.into(), RUST_HIGHLIGHTS),
            LanguageId::Html => (tree_sitter_html::LANGUAGE.into(), HTML_HIGHLIGHTS),
            LanguageId::Css => (tree_sitter_css::LANGUAGE.into(), CSS_HIGHLIGHTS),
            LanguageId::JavaScript => (
                tree_sitter_javascript::LANGUAGE.into(),
                JAVASCRIPT_HIGHLIGHTS,
            ),
            LanguageId::PlainText => return,
        };

        // Create parser
        let mut parser = Parser::new();
        if let Err(e) = parser.set_language(&ts_lang) {
            tracing::error!("Failed to set language for {:?}: {}", lang, e);
            return;
        }
        self.parsers.insert(lang, parser);

        // Create query (may fail if query syntax is invalid)
        match Query::new(&ts_lang, highlights_scm) {
            Ok(query) => {
                self.queries.insert(lang, query);
            }
            Err(e) => {
                tracing::error!("Failed to compile query for {:?}: {:?}", lang, e);
            }
        }
    }

    /// Parse document and extract highlights
    pub fn parse_and_highlight(
        &mut self,
        source: &str,
        language: LanguageId,
        _doc_id: DocumentId,
        revision: u64,
    ) -> SyntaxHighlights {
        // Skip plain text
        if language == LanguageId::PlainText {
            return SyntaxHighlights::new(language, revision);
        }

        let parser = match self.parsers.get_mut(&language) {
            Some(p) => p,
            None => {
                tracing::warn!("No parser for language {:?}", language);
                return SyntaxHighlights::new(language, revision);
            }
        };

        // For correct incremental parsing, we would need to call tree.edit() on the
        // old tree with edit information. Since we don't track edits, we do a full
        // reparse by passing None. Passing an unedited old tree can cause tree-sitter
        // to incorrectly reuse nodes, leading to misaligned highlights.
        // TODO: Implement proper incremental parsing in Phase 2

        // Parse the source (full reparse)
        let tree = match parser.parse(source, None) {
            Some(t) => t,
            None => {
                tracing::error!("Parse failed for {:?}", language);
                return SyntaxHighlights::new(language, revision);
            }
        };

        // Extract highlights
        self.extract_highlights(source, &tree, language, revision)
    }

    /// Extract highlight tokens from a parsed tree
    fn extract_highlights(
        &self,
        source: &str,
        tree: &Tree,
        language: LanguageId,
        revision: u64,
    ) -> SyntaxHighlights {
        let query = match self.queries.get(&language) {
            Some(q) => q,
            None => return SyntaxHighlights::new(language, revision),
        };

        let mut highlights = SyntaxHighlights::new(language, revision);
        let mut cursor = QueryCursor::new();
        let source_bytes = source.as_bytes();

        // Pre-split into lines for byte→char column conversion
        let lines: Vec<&str> = source.lines().collect();

        // Helper: convert byte column to character column on a given line
        // Tree-sitter positions are in bytes, but we need character indices
        fn byte_to_char_col(line: &str, byte_col: usize) -> usize {
            // Clamp to line length
            let byte_col = byte_col.min(line.len());
            // Find the nearest valid char boundary at or before byte_col
            let mut valid_byte = byte_col;
            while valid_byte > 0 && !line.is_char_boundary(valid_byte) {
                valid_byte -= 1;
            }
            line[..valid_byte].chars().count()
        }

        // Run query and collect captures using StreamingIterator
        let mut captures = cursor.captures(query, tree.root_node(), source_bytes);
        while let Some((query_match, capture_idx)) = captures.next() {
            let capture = &query_match.captures[*capture_idx];
            let capture_name = &query.capture_names()[capture.index as usize];

            // Map capture name to highlight ID
            let highlight_id = match highlight_id_for_name(capture_name) {
                Some(id) => id,
                None => continue, // Skip unknown captures
            };

            let node = capture.node;
            let start = node.start_position();
            let end = node.end_position();

            // Handle multi-line nodes
            if start.row == end.row {
                // Single line token
                let row = start.row;
                let line = lines.get(row).copied().unwrap_or("");
                let start_char = byte_to_char_col(line, start.column);
                let end_char = byte_to_char_col(line, end.column);

                if start_char < end_char {
                    let line_highlights = highlights.lines.entry(row).or_default();
                    line_highlights.tokens.push(HighlightToken {
                        start_col: start_char,
                        end_col: end_char,
                        highlight: highlight_id,
                    });
                }
            } else {
                // Multi-line token: split across lines
                for row in start.row..=end.row {
                    let line = lines.get(row).copied().unwrap_or("");
                    let line_char_len = line.chars().count();

                    let (start_char, end_char) = if row == start.row {
                        // First line: from start to end of line
                        let start_char = byte_to_char_col(line, start.column);
                        (start_char, line_char_len)
                    } else if row == end.row {
                        // Last line: from start of line to end position
                        let end_char = byte_to_char_col(line, end.column);
                        (0, end_char)
                    } else {
                        // Middle lines: entire line
                        (0, line_char_len)
                    };

                    if start_char < end_char {
                        let line_highlights = highlights.lines.entry(row).or_default();
                        line_highlights.tokens.push(HighlightToken {
                            start_col: start_char,
                            end_col: end_char,
                            highlight: highlight_id,
                        });
                    }
                }
            }
        }

        // Sort tokens within each line by start column
        for line_highlights in highlights.lines.values_mut() {
            line_highlights
                .tokens
                .sort_by_key(|t| (t.start_col, t.end_col));
        }

        highlights
    }
}

impl Default for ParserState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_yaml_parsing() {
        let mut state = ParserState::new();
        let source = r#"# Comment
key: value
number: 42
enabled: true
"#;
        let doc_id = DocumentId(1);
        let highlights = state.parse_and_highlight(source, LanguageId::Yaml, doc_id, 1);

        assert_eq!(highlights.language, LanguageId::Yaml);
        assert!(!highlights.lines.is_empty());

        // Check that comment line has a comment token
        if let Some(line0) = highlights.lines.get(&0) {
            assert!(!line0.tokens.is_empty());
        }
    }

    #[test]
    fn test_rust_parsing() {
        let mut state = ParserState::new();
        let source = r#"fn main() {
    let x = 42;
    println!("Hello");
}
"#;
        let doc_id = DocumentId(2);
        let highlights = state.parse_and_highlight(source, LanguageId::Rust, doc_id, 1);

        assert_eq!(highlights.language, LanguageId::Rust);
        assert!(!highlights.lines.is_empty());
    }

    #[test]
    fn test_plain_text_no_parsing() {
        let mut state = ParserState::new();
        let source = "Hello, world!";
        let doc_id = DocumentId(3);
        let highlights = state.parse_and_highlight(source, LanguageId::PlainText, doc_id, 1);

        assert_eq!(highlights.language, LanguageId::PlainText);
        assert!(highlights.lines.is_empty());
    }

    #[test]
    fn test_all_query_files_compile() {
        // This test ensures all .scm query files are valid and compile without errors
        let state = ParserState::new();

        // All languages with highlighting should have compiled queries
        let languages_with_queries = [
            LanguageId::Yaml,
            LanguageId::Markdown,
            LanguageId::Rust,
            LanguageId::Html,
            LanguageId::Css,
            LanguageId::JavaScript,
        ];

        for lang in languages_with_queries {
            assert!(
                state.queries.contains_key(&lang),
                "Query failed to compile for {:?}",
                lang
            );
        }
    }

    #[test]
    fn test_html_parsing() {
        let mut state = ParserState::new();
        let source = r#"<!DOCTYPE html>
<html>
<head><title>Test</title></head>
<body>
  <h1 class="title">Hello</h1>
</body>
</html>"#;
        let doc_id = DocumentId(4);
        let highlights = state.parse_and_highlight(source, LanguageId::Html, doc_id, 1);

        assert_eq!(highlights.language, LanguageId::Html);
        assert!(!highlights.lines.is_empty());
    }

    #[test]
    fn test_css_parsing() {
        let mut state = ParserState::new();
        let source = r#".container {
  color: #ff0000;
  font-size: 16px;
}
#main { display: flex; }"#;
        let doc_id = DocumentId(5);
        let highlights = state.parse_and_highlight(source, LanguageId::Css, doc_id, 1);

        assert_eq!(highlights.language, LanguageId::Css);
        assert!(!highlights.lines.is_empty());
    }

    #[test]
    fn test_javascript_parsing() {
        let mut state = ParserState::new();
        let source = r#"function hello(name) {
  const greeting = `Hello, ${name}!`;
  return greeting;
}
const x = 42;"#;
        let doc_id = DocumentId(6);
        let highlights = state.parse_and_highlight(source, LanguageId::JavaScript, doc_id, 1);

        assert_eq!(highlights.language, LanguageId::JavaScript);
        assert!(!highlights.lines.is_empty());
    }

    #[test]
    fn test_markdown_parsing() {
        let mut state = ParserState::new();
        let source = r#"# Heading

This is a paragraph.

- List item 1
- List item 2

```rust
fn main() {}
```"#;
        let doc_id = DocumentId(7);
        let highlights = state.parse_and_highlight(source, LanguageId::Markdown, doc_id, 1);

        assert_eq!(highlights.language, LanguageId::Markdown);
        assert!(!highlights.lines.is_empty());
    }

    #[test]
    fn test_incremental_parsing_updates() {
        let mut state = ParserState::new();
        let doc_id = DocumentId(10);

        // Initial parse
        let source1 = "let x = 1;";
        let highlights1 = state.parse_and_highlight(source1, LanguageId::JavaScript, doc_id, 1);
        assert!(!highlights1.lines.is_empty(), "Initial parse should produce highlights");

        // Second parse with modified content (simulating edit)
        let source2 = "let x = 1;\nlet y = 2;";
        let highlights2 = state.parse_and_highlight(source2, LanguageId::JavaScript, doc_id, 2);
        assert!(!highlights2.lines.is_empty(), "Second parse should produce highlights");
        assert_eq!(highlights2.revision, 2, "Revision should match");

        // Verify we have highlights for line 0
        assert!(
            highlights2.lines.contains_key(&0),
            "Should have highlights for line 0"
        );
        // Line 1 may or may not have highlights depending on the query
        // (some elements might not be captured if they're on the same line as other captures)
    }

    #[test]
    fn test_revision_tracking() {
        let mut state = ParserState::new();
        let doc_id = DocumentId(11);

        let source = "fn test() {}";
        let highlights = state.parse_and_highlight(source, LanguageId::Rust, doc_id, 42);

        assert_eq!(highlights.revision, 42, "Revision should be preserved");
    }

    #[test]
    fn test_parsing_with_leading_newline() {
        let mut state = ParserState::new();
        let doc_id = DocumentId(20);

        // Source with leading newline - like pressing Enter at start of file
        let source = "\nfn main() {}";
        let highlights = state.parse_and_highlight(source, LanguageId::Rust, doc_id, 1);

        eprintln!("Source: {:?}", source);
        eprintln!("Lines from source.lines(): {:?}", source.lines().collect::<Vec<_>>());
        eprintln!("Highlights lines: {:?}", highlights.lines.keys().collect::<Vec<_>>());
        for (line, lh) in &highlights.lines {
            eprintln!("Line {}: {:?}", line, lh.tokens);
        }

        // Should have highlights on line 1 (the fn main line)
        assert!(
            !highlights.lines.is_empty(),
            "Should have some highlights"
        );
        assert!(
            highlights.lines.contains_key(&1),
            "Should have highlights on line 1 where 'fn main' is"
        );
    }

    #[test]
    fn test_highlights_alignment_after_insert_newline() {
        use ropey::Rope;

        let mut state = ParserState::new();
        let doc_id = DocumentId(30);

        // Simulate: Rope has "\nfn main() {}" after inserting newline at start
        let rope_content = "\nfn main() {}";
        let rope = Rope::from(rope_content);

        // Parse the content
        let source = rope.to_string();
        let highlights = state.parse_and_highlight(&source, LanguageId::Rust, doc_id, 1);

        eprintln!("=== Alignment test ===");
        eprintln!("Rope line count: {}", rope.len_lines());
        for i in 0..rope.len_lines() {
            let line = rope.line(i);
            eprintln!("Rope line {}: {:?}", i, line.to_string());
        }

        eprintln!("\nHighlights:");
        for (line_num, lh) in &highlights.lines {
            eprintln!("Line {}: {} tokens", line_num, lh.tokens.len());
            for tok in &lh.tokens {
                eprintln!("  col {}..{}: highlight {}", tok.start_col, tok.end_col, tok.highlight);
            }
        }

        // Line 0 in rope should be empty (just "\n" or "")
        // Line 1 in rope should be "fn main() {}"
        // Highlights should be on line 1

        let rope_line_1 = rope.line(1).to_string();
        eprintln!("\nRope line 1: {:?}", rope_line_1);
        eprintln!("Highlights for line 1 exist: {}", highlights.lines.contains_key(&1));

        // The highlight tokens for line 1 should match the text on rope line 1
        if let Some(line_highlights) = highlights.lines.get(&1) {
            // "fn main() {}" - "fn" is keyword at columns 0..2
            let fn_token = line_highlights.tokens.iter().find(|t| t.start_col == 0 && t.end_col == 2);
            assert!(fn_token.is_some(), "Should have 'fn' token at 0..2 on line 1");
        } else {
            panic!("No highlights for line 1!");
        }
    }
}
