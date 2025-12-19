//! Tree-sitter parser state and highlighting extraction
//!
//! Manages parsers, trees, and queries for syntax highlighting.
//! Supports incremental parsing by caching trees and computing edits.

use std::collections::HashMap;

use streaming_iterator::StreamingIterator;
use tree_sitter::{InputEdit, Parser, Point, Query, QueryCursor, Tree};

use super::highlights::{highlight_id_for_name, HighlightToken, SyntaxHighlights};
use super::languages::LanguageId;
use crate::model::editor_area::DocumentId;

/// Cached parse state for a document (enables incremental parsing)
struct DocParseState {
    /// The language this tree was parsed with
    language: LanguageId,
    /// The parsed tree
    tree: Tree,
    /// The source text that was parsed (needed for computing edits)
    source: String,
}

/// Convert a byte offset to a tree-sitter Point (row, column in bytes)
fn byte_to_point(text: &str, byte_offset: usize) -> Point {
    let mut row = 0usize;
    let mut col = 0usize;
    let bytes = text.as_bytes();

    for &byte in bytes.iter().take(byte_offset) {
        if byte == b'\n' {
            row += 1;
            col = 0;
        } else {
            col += 1;
        }
    }

    Point { row, column: col }
}

/// Compute an InputEdit by diffing old and new source text.
/// Returns None if the sources are identical.
fn compute_incremental_edit(old_src: &str, new_src: &str) -> Option<InputEdit> {
    if old_src == new_src {
        return None;
    }

    let old_bytes = old_src.as_bytes();
    let new_bytes = new_src.as_bytes();

    // Find common prefix length (in bytes)
    let mut start = 0;
    let max_start = old_bytes.len().min(new_bytes.len());
    while start < max_start && old_bytes[start] == new_bytes[start] {
        start += 1;
    }

    // Find common suffix length (in bytes), not overlapping prefix
    let mut old_end = old_bytes.len();
    let mut new_end = new_bytes.len();
    while old_end > start && new_end > start && old_bytes[old_end - 1] == new_bytes[new_end - 1] {
        old_end -= 1;
        new_end -= 1;
    }

    // The edit is: old_src[start..old_end] replaced by new_src[start..new_end]
    let start_position = byte_to_point(old_src, start);
    let old_end_position = byte_to_point(old_src, old_end);
    let new_end_position = byte_to_point(new_src, new_end);

    Some(InputEdit {
        start_byte: start,
        old_end_byte: old_end,
        new_end_byte: new_end,
        start_position,
        old_end_position,
        new_end_position,
    })
}

// Embedded query files
// Phase 1 languages
const YAML_HIGHLIGHTS: &str = include_str!("../../queries/yaml/highlights.scm");
const MARKDOWN_HIGHLIGHTS: &str = include_str!("../../queries/markdown/highlights.scm");
const RUST_HIGHLIGHTS: &str = tree_sitter_rust::HIGHLIGHTS_QUERY;

// Phase 2 languages (web stack)
const HTML_HIGHLIGHTS: &str = include_str!("../../queries/html/highlights.scm");
const CSS_HIGHLIGHTS: &str = include_str!("../../queries/css/highlights.scm");
const JAVASCRIPT_HIGHLIGHTS: &str = include_str!("../../queries/javascript/highlights.scm");

// Phase 3 languages (priority)
const TYPESCRIPT_HIGHLIGHTS: &str = include_str!("../../queries/typescript/highlights.scm");
const JSON_HIGHLIGHTS: &str = include_str!("../../queries/json/highlights.scm");
const TOML_HIGHLIGHTS: &str = include_str!("../../queries/toml/highlights.scm");

// Phase 4 languages (common) - use built-in queries where available
const PYTHON_HIGHLIGHTS: &str = tree_sitter_python::HIGHLIGHTS_QUERY;
const GO_HIGHLIGHTS: &str = tree_sitter_go::HIGHLIGHTS_QUERY;
const PHP_HIGHLIGHTS: &str = tree_sitter_php::HIGHLIGHTS_QUERY;

// Phase 5 languages (extended) - use built-in queries (some use HIGHLIGHT_QUERY singular)
const C_HIGHLIGHTS: &str = tree_sitter_c::HIGHLIGHT_QUERY;
const CPP_HIGHLIGHTS: &str = tree_sitter_cpp::HIGHLIGHT_QUERY;
const JAVA_HIGHLIGHTS: &str = tree_sitter_java::HIGHLIGHTS_QUERY;
const BASH_HIGHLIGHTS: &str = tree_sitter_bash::HIGHLIGHT_QUERY;

// Phase 6 languages (specialized)
const SCHEME_HIGHLIGHTS: &str = tree_sitter_racket::HIGHLIGHTS_QUERY;
const INI_HIGHLIGHTS: &str = tree_sitter_ini::HIGHLIGHTS_QUERY;
const XML_HIGHLIGHTS: &str = tree_sitter_xml::XML_HIGHLIGHT_QUERY;

/// Thread-local parser state (tree-sitter parsers are !Sync)
pub struct ParserState {
    /// Parser instances per language
    parsers: HashMap<LanguageId, Parser>,
    /// Compiled queries per language
    queries: HashMap<LanguageId, Query>,
    /// Cached parse state per document (for incremental parsing)
    doc_cache: HashMap<DocumentId, DocParseState>,
}

impl ParserState {
    /// Create a new parser state with initialized languages
    pub fn new() -> Self {
        let mut state = Self {
            parsers: HashMap::new(),
            queries: HashMap::new(),
            doc_cache: HashMap::new(),
        };

        // Initialize Phase 1 languages
        state.init_language(LanguageId::Yaml);
        state.init_language(LanguageId::Markdown);
        state.init_language(LanguageId::Rust);

        // Initialize Phase 2 languages (web stack)
        state.init_language(LanguageId::Html);
        state.init_language(LanguageId::Css);
        state.init_language(LanguageId::JavaScript);

        // Initialize Phase 3 languages (priority)
        state.init_language(LanguageId::TypeScript);
        state.init_language(LanguageId::Tsx);
        state.init_language(LanguageId::Json);
        state.init_language(LanguageId::Toml);

        // Initialize Phase 4 languages (common)
        state.init_language(LanguageId::Python);
        state.init_language(LanguageId::Go);
        state.init_language(LanguageId::Php);

        // Initialize Phase 5 languages (extended)
        state.init_language(LanguageId::C);
        state.init_language(LanguageId::Cpp);
        state.init_language(LanguageId::Java);
        state.init_language(LanguageId::Bash);

        // Initialize Phase 6 languages (specialized)
        state.init_language(LanguageId::Scheme);
        state.init_language(LanguageId::Ini);
        state.init_language(LanguageId::Xml);

        state
    }

    /// Initialize a language's parser and query
    fn init_language(&mut self, lang: LanguageId) {
        let (ts_lang, highlights_scm) = match lang {
            // Phase 1 languages
            LanguageId::Yaml => (tree_sitter_yaml::language(), YAML_HIGHLIGHTS),
            LanguageId::Markdown => (tree_sitter_md::LANGUAGE.into(), MARKDOWN_HIGHLIGHTS),
            LanguageId::Rust => (tree_sitter_rust::LANGUAGE.into(), RUST_HIGHLIGHTS),
            // Phase 2 languages (web stack)
            LanguageId::Html => (tree_sitter_html::LANGUAGE.into(), HTML_HIGHLIGHTS),
            LanguageId::Css => (tree_sitter_css::LANGUAGE.into(), CSS_HIGHLIGHTS),
            LanguageId::JavaScript => (
                tree_sitter_javascript::LANGUAGE.into(),
                JAVASCRIPT_HIGHLIGHTS,
            ),
            // Phase 3 languages (priority)
            LanguageId::TypeScript => (
                tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
                TYPESCRIPT_HIGHLIGHTS,
            ),
            LanguageId::Tsx => (
                tree_sitter_typescript::LANGUAGE_TSX.into(),
                TYPESCRIPT_HIGHLIGHTS, // TSX uses same highlights as TypeScript
            ),
            LanguageId::Json => (tree_sitter_json::LANGUAGE.into(), JSON_HIGHLIGHTS),
            LanguageId::Toml => (tree_sitter_toml_ng::LANGUAGE.into(), TOML_HIGHLIGHTS),
            // Phase 4 languages (common)
            LanguageId::Python => (tree_sitter_python::LANGUAGE.into(), PYTHON_HIGHLIGHTS),
            LanguageId::Go => (tree_sitter_go::LANGUAGE.into(), GO_HIGHLIGHTS),
            LanguageId::Php => (tree_sitter_php::LANGUAGE_PHP.into(), PHP_HIGHLIGHTS),
            // Phase 5 languages (extended)
            LanguageId::C => (tree_sitter_c::LANGUAGE.into(), C_HIGHLIGHTS),
            LanguageId::Cpp => (tree_sitter_cpp::LANGUAGE.into(), CPP_HIGHLIGHTS),
            LanguageId::Java => (tree_sitter_java::LANGUAGE.into(), JAVA_HIGHLIGHTS),
            LanguageId::Bash => (tree_sitter_bash::LANGUAGE.into(), BASH_HIGHLIGHTS),
            // Phase 6 languages (specialized)
            LanguageId::Scheme => (tree_sitter_racket::LANGUAGE.into(), SCHEME_HIGHLIGHTS),
            LanguageId::Ini => (tree_sitter_ini::LANGUAGE.into(), INI_HIGHLIGHTS),
            LanguageId::Xml => (tree_sitter_xml::LANGUAGE_XML.into(), XML_HIGHLIGHTS),
            // No highlighting for plain text
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

    /// Parse document and extract highlights.
    /// Uses incremental parsing when a cached tree is available.
    pub fn parse_and_highlight(
        &mut self,
        source: &str,
        language: LanguageId,
        doc_id: DocumentId,
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

        // Try incremental parsing if we have a cached tree for this document
        let tree = if let Some(cached) = self.doc_cache.get_mut(&doc_id) {
            if cached.language == language {
                // Same language, try incremental parse
                if let Some(edit) = compute_incremental_edit(&cached.source, source) {
                    // Apply the edit to the cached tree
                    cached.tree.edit(&edit);

                    tracing::trace!(
                        "Incremental parse: edit at byte {}..{} -> {}..{}",
                        edit.start_byte,
                        edit.old_end_byte,
                        edit.start_byte,
                        edit.new_end_byte
                    );

                    // Parse with the edited old tree for incremental reuse
                    match parser.parse(source, Some(&cached.tree)) {
                        Some(new_tree) => {
                            // Update cache with new tree and source
                            cached.tree = new_tree.clone();
                            cached.source = source.to_owned();
                            new_tree
                        }
                        None => {
                            // Incremental parse failed, fall back to full parse
                            tracing::warn!(
                                "Incremental parse failed for {:?}, falling back to full parse",
                                language
                            );
                            self.doc_cache.remove(&doc_id);
                            match parser.parse(source, None) {
                                Some(t) => {
                                    self.doc_cache.insert(
                                        doc_id,
                                        DocParseState {
                                            language,
                                            tree: t.clone(),
                                            source: source.to_owned(),
                                        },
                                    );
                                    t
                                }
                                None => {
                                    tracing::error!("Full parse also failed for {:?}", language);
                                    return SyntaxHighlights::new(language, revision);
                                }
                            }
                        }
                    }
                } else {
                    // No edit (source unchanged), reuse cached tree
                    tracing::trace!("Source unchanged, reusing cached tree");
                    cached.tree.clone()
                }
            } else {
                // Language changed, do full parse
                tracing::debug!(
                    "Language changed from {:?} to {:?}, doing full parse",
                    cached.language,
                    language
                );
                self.doc_cache.remove(&doc_id);
                match parser.parse(source, None) {
                    Some(t) => {
                        self.doc_cache.insert(
                            doc_id,
                            DocParseState {
                                language,
                                tree: t.clone(),
                                source: source.to_owned(),
                            },
                        );
                        t
                    }
                    None => {
                        tracing::error!("Parse failed for {:?}", language);
                        return SyntaxHighlights::new(language, revision);
                    }
                }
            }
        } else {
            // No cached tree, do full parse
            match parser.parse(source, None) {
                Some(t) => {
                    self.doc_cache.insert(
                        doc_id,
                        DocParseState {
                            language,
                            tree: t.clone(),
                            source: source.to_owned(),
                        },
                    );
                    t
                }
                None => {
                    tracing::error!("Parse failed for {:?}", language);
                    return SyntaxHighlights::new(language, revision);
                }
            }
        };

        // Extract highlights
        self.extract_highlights(source, &tree, language, revision)
    }

    /// Remove cached parse state for a document (call when document is closed)
    pub fn clear_doc_cache(&mut self, doc_id: DocumentId) {
        self.doc_cache.remove(&doc_id);
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
            // Phase 1
            LanguageId::Yaml,
            LanguageId::Markdown,
            LanguageId::Rust,
            // Phase 2
            LanguageId::Html,
            LanguageId::Css,
            LanguageId::JavaScript,
            // Phase 3
            LanguageId::TypeScript,
            LanguageId::Tsx,
            LanguageId::Json,
            LanguageId::Toml,
            // Phase 4
            LanguageId::Python,
            LanguageId::Go,
            LanguageId::Php,
            // Phase 5
            LanguageId::C,
            LanguageId::Cpp,
            LanguageId::Java,
            LanguageId::Bash,
            // Phase 6
            LanguageId::Scheme,
            LanguageId::Ini,
            LanguageId::Xml,
        ];

        for lang in languages_with_queries {
            assert!(
                state.queries.contains_key(&lang),
                "Query failed to compile for {:?}",
                lang
            );
        }
    }

    /// Test that each query file compiles correctly and show detailed errors if not.
    /// These tests are separate per language to make failures more specific.
    mod query_compilation_tests {
        use super::*;
        use tree_sitter::Query;

        fn assert_query_compiles(
            lang_name: &str,
            ts_lang: tree_sitter::Language,
            query_source: &str,
        ) {
            match Query::new(&ts_lang, query_source) {
                Ok(query) => {
                    // Also verify query has captures
                    assert!(
                        !query.capture_names().is_empty(),
                        "{} query compiled but has no captures - check query syntax",
                        lang_name
                    );
                }
                Err(e) => {
                    // Format a detailed error message
                    let error_line = query_source
                        .lines()
                        .nth(e.row)
                        .unwrap_or("<line not found>");
                    panic!(
                        "\n{} query compilation FAILED at row {}, column {}:\n\
                         Error: {:?}\n\
                         Line {}: {}\n\
                         {}^\n\
                         \n\
                         Check queries/{}/highlights.scm for syntax errors.",
                        lang_name,
                        e.row,
                        e.column,
                        e.kind,
                        e.row + 1,
                        error_line,
                        " ".repeat(e.column.min(error_line.len())),
                        lang_name.to_lowercase()
                    );
                }
            }
        }

        #[test]
        fn test_yaml_query_compiles() {
            assert_query_compiles("YAML", tree_sitter_yaml::language(), YAML_HIGHLIGHTS);
        }

        #[test]
        fn test_markdown_query_compiles() {
            assert_query_compiles(
                "Markdown",
                tree_sitter_md::LANGUAGE.into(),
                MARKDOWN_HIGHLIGHTS,
            );
        }

        #[test]
        fn test_rust_query_compiles() {
            assert_query_compiles("Rust", tree_sitter_rust::LANGUAGE.into(), RUST_HIGHLIGHTS);
        }

        #[test]
        fn test_html_query_compiles() {
            assert_query_compiles("HTML", tree_sitter_html::LANGUAGE.into(), HTML_HIGHLIGHTS);
        }

        #[test]
        fn test_css_query_compiles() {
            assert_query_compiles("CSS", tree_sitter_css::LANGUAGE.into(), CSS_HIGHLIGHTS);
        }

        #[test]
        fn test_javascript_query_compiles() {
            assert_query_compiles(
                "JavaScript",
                tree_sitter_javascript::LANGUAGE.into(),
                JAVASCRIPT_HIGHLIGHTS,
            );
        }

        // Phase 3 languages

        #[test]
        fn test_typescript_query_compiles() {
            assert_query_compiles(
                "TypeScript",
                tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
                TYPESCRIPT_HIGHLIGHTS,
            );
        }

        #[test]
        fn test_tsx_query_compiles() {
            assert_query_compiles(
                "TSX",
                tree_sitter_typescript::LANGUAGE_TSX.into(),
                TYPESCRIPT_HIGHLIGHTS,
            );
        }

        #[test]
        fn test_json_query_compiles() {
            assert_query_compiles("JSON", tree_sitter_json::LANGUAGE.into(), JSON_HIGHLIGHTS);
        }

        #[test]
        fn test_toml_query_compiles() {
            assert_query_compiles(
                "TOML",
                tree_sitter_toml_ng::LANGUAGE.into(),
                TOML_HIGHLIGHTS,
            );
        }

        // Phase 4 languages

        #[test]
        fn test_python_query_compiles() {
            assert_query_compiles(
                "Python",
                tree_sitter_python::LANGUAGE.into(),
                PYTHON_HIGHLIGHTS,
            );
        }

        #[test]
        fn test_go_query_compiles() {
            assert_query_compiles("Go", tree_sitter_go::LANGUAGE.into(), GO_HIGHLIGHTS);
        }

        #[test]
        fn test_php_query_compiles() {
            assert_query_compiles("PHP", tree_sitter_php::LANGUAGE_PHP.into(), PHP_HIGHLIGHTS);
        }

        // Phase 5 languages

        #[test]
        fn test_c_query_compiles() {
            assert_query_compiles("C", tree_sitter_c::LANGUAGE.into(), C_HIGHLIGHTS);
        }

        #[test]
        fn test_cpp_query_compiles() {
            assert_query_compiles("C++", tree_sitter_cpp::LANGUAGE.into(), CPP_HIGHLIGHTS);
        }

        #[test]
        fn test_java_query_compiles() {
            assert_query_compiles("Java", tree_sitter_java::LANGUAGE.into(), JAVA_HIGHLIGHTS);
        }

        #[test]
        fn test_bash_query_compiles() {
            assert_query_compiles("Bash", tree_sitter_bash::LANGUAGE.into(), BASH_HIGHLIGHTS);
        }

        // Phase 6 languages

        #[test]
        fn test_scheme_query_compiles() {
            assert_query_compiles(
                "Scheme",
                tree_sitter_racket::LANGUAGE.into(),
                SCHEME_HIGHLIGHTS,
            );
        }

        #[test]
        fn test_ini_query_compiles() {
            assert_query_compiles("INI", tree_sitter_ini::LANGUAGE.into(), INI_HIGHLIGHTS);
        }

        #[test]
        fn test_xml_query_compiles() {
            assert_query_compiles("XML", tree_sitter_xml::LANGUAGE_XML.into(), XML_HIGHLIGHTS);
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

    // Phase 3 parsing tests

    #[test]
    fn test_typescript_parsing() {
        let mut state = ParserState::new();
        let source = r#"interface User {
    name: string;
    age: number;
}

function greet(user: User): string {
    return `Hello, ${user.name}!`;
}

const x: number = 42;"#;
        let doc_id = DocumentId(20);
        let highlights = state.parse_and_highlight(source, LanguageId::TypeScript, doc_id, 1);

        assert_eq!(highlights.language, LanguageId::TypeScript);
        assert!(!highlights.lines.is_empty());
    }

    #[test]
    fn test_tsx_parsing() {
        let mut state = ParserState::new();
        let source = r#"interface Props {
    name: string;
}

function Hello({ name }: Props) {
    return <div className="greeting">Hello, {name}!</div>;
}

export default Hello;"#;
        let doc_id = DocumentId(21);
        let highlights = state.parse_and_highlight(source, LanguageId::Tsx, doc_id, 1);

        assert_eq!(highlights.language, LanguageId::Tsx);
        assert!(!highlights.lines.is_empty());
    }

    #[test]
    fn test_json_parsing() {
        let mut state = ParserState::new();
        let source = r#"{
    "name": "token",
    "version": "0.3.2",
    "dependencies": {
        "tree-sitter": "0.24"
    },
    "count": 42,
    "enabled": true,
    "data": null
}"#;
        let doc_id = DocumentId(22);
        let highlights = state.parse_and_highlight(source, LanguageId::Json, doc_id, 1);

        assert_eq!(highlights.language, LanguageId::Json);
        assert!(!highlights.lines.is_empty());
    }

    #[test]
    fn test_toml_parsing() {
        let mut state = ParserState::new();
        let source = r#"[package]
name = "token"
version = "0.3.2"

[dependencies]
tree-sitter = "0.24"
serde = { version = "1.0", features = ["derive"] }

[[bin]]
name = "token"
path = "src/main.rs""#;
        let doc_id = DocumentId(23);
        let highlights = state.parse_and_highlight(source, LanguageId::Toml, doc_id, 1);

        assert_eq!(highlights.language, LanguageId::Toml);
        assert!(!highlights.lines.is_empty());
    }

    // Phase 4 parsing tests

    #[test]
    fn test_python_parsing() {
        let mut state = ParserState::new();
        let source = r#"def greet(name: str) -> str:
    """Say hello."""
    return f"Hello, {name}!"

class Person:
    def __init__(self, name: str):
        self.name = name

if __name__ == "__main__":
    greet("World")"#;
        let doc_id = DocumentId(24);
        let highlights = state.parse_and_highlight(source, LanguageId::Python, doc_id, 1);

        assert_eq!(highlights.language, LanguageId::Python);
        assert!(!highlights.lines.is_empty());
    }

    #[test]
    fn test_go_parsing() {
        let mut state = ParserState::new();
        let source = r#"package main

import "fmt"

type Person struct {
    Name string
    Age  int
}

func (p Person) Greet() string {
    return fmt.Sprintf("Hello, %s!", p.Name)
}

func main() {
    p := Person{Name: "World", Age: 42}
    fmt.Println(p.Greet())
}"#;
        let doc_id = DocumentId(25);
        let highlights = state.parse_and_highlight(source, LanguageId::Go, doc_id, 1);

        assert_eq!(highlights.language, LanguageId::Go);
        assert!(!highlights.lines.is_empty());
    }

    #[test]
    fn test_php_parsing() {
        let mut state = ParserState::new();
        let source = r#"<?php
namespace App\Models;

class User {
    private string $name;

    public function __construct(string $name) {
        $this->name = $name;
    }

    public function greet(): string {
        return "Hello, {$this->name}!";
    }
}

$user = new User("World");
echo $user->greet();
?>"#;
        let doc_id = DocumentId(26);
        let highlights = state.parse_and_highlight(source, LanguageId::Php, doc_id, 1);

        assert_eq!(highlights.language, LanguageId::Php);
        assert!(!highlights.lines.is_empty());
    }

    // Phase 5 parsing tests

    #[test]
    fn test_c_parsing() {
        let mut state = ParserState::new();
        let source = r#"#include <stdio.h>

struct Person {
    char* name;
    int age;
};

void greet(struct Person* p) {
    printf("Hello, %s!\n", p->name);
}

int main() {
    struct Person p = {"World", 42};
    greet(&p);
    return 0;
}"#;
        let doc_id = DocumentId(27);
        let highlights = state.parse_and_highlight(source, LanguageId::C, doc_id, 1);

        assert_eq!(highlights.language, LanguageId::C);
        assert!(!highlights.lines.is_empty());
    }

    #[test]
    fn test_cpp_parsing() {
        let mut state = ParserState::new();
        let source = r#"#include <iostream>
#include <string>

class Person {
public:
    std::string name;
    int age;

    Person(std::string n, int a) : name(n), age(a) {}

    void greet() const {
        std::cout << "Hello, " << name << "!" << std::endl;
    }
};

int main() {
    Person p("World", 42);
    p.greet();
    return 0;
}"#;
        let doc_id = DocumentId(28);
        let highlights = state.parse_and_highlight(source, LanguageId::Cpp, doc_id, 1);

        assert_eq!(highlights.language, LanguageId::Cpp);
        assert!(!highlights.lines.is_empty());
    }

    #[test]
    fn test_java_parsing() {
        let mut state = ParserState::new();
        let source = r#"package com.example;

public class Person {
    private String name;
    private int age;

    public Person(String name, int age) {
        this.name = name;
        this.age = age;
    }

    public String greet() {
        return "Hello, " + name + "!";
    }

    public static void main(String[] args) {
        Person p = new Person("World", 42);
        System.out.println(p.greet());
    }
}"#;
        let doc_id = DocumentId(29);
        let highlights = state.parse_and_highlight(source, LanguageId::Java, doc_id, 1);

        assert_eq!(highlights.language, LanguageId::Java);
        assert!(!highlights.lines.is_empty());
    }

    #[test]
    fn test_bash_parsing() {
        let mut state = ParserState::new();
        let source = r#"#!/bin/bash

# A simple greeting script
NAME="World"

function greet() {
    local name=$1
    echo "Hello, $name!"
}

for i in 1 2 3; do
    greet "$NAME"
done

if [[ -n "$NAME" ]]; then
    echo "Name is set"
fi"#;
        let doc_id = DocumentId(30);
        let highlights = state.parse_and_highlight(source, LanguageId::Bash, doc_id, 1);

        assert_eq!(highlights.language, LanguageId::Bash);
        assert!(!highlights.lines.is_empty());
    }

    #[test]
    fn test_scheme_parsing() {
        let mut state = ParserState::new();
        let source = r#"; Tree-sitter highlights query for Scheme
(comment) @comment

(string) @string

[
  "define"
  "lambda"
  "let"
] @keyword

(symbol) @variable
"#;
        let doc_id = DocumentId(50);
        let highlights = state.parse_and_highlight(source, LanguageId::Scheme, doc_id, 1);

        assert_eq!(highlights.language, LanguageId::Scheme);
        assert!(!highlights.lines.is_empty());
    }

    #[test]
    fn test_ini_parsing() {
        let mut state = ParserState::new();
        let source = r#"; This is a comment
[section]
key = value
another_key = "quoted value"
number = 42
"#;
        let doc_id = DocumentId(51);
        let highlights = state.parse_and_highlight(source, LanguageId::Ini, doc_id, 1);

        assert_eq!(highlights.language, LanguageId::Ini);
        assert!(!highlights.lines.is_empty());
    }

    #[test]
    fn test_xml_parsing() {
        let mut state = ParserState::new();
        let source = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN">
<plist version="1.0">
    <dict>
        <key>CFBundleName</key>
        <string>MyApp</string>
    </dict>
</plist>
"#;
        let doc_id = DocumentId(52);
        let highlights = state.parse_and_highlight(source, LanguageId::Xml, doc_id, 1);

        assert_eq!(highlights.language, LanguageId::Xml);
        assert!(!highlights.lines.is_empty());
    }

    #[test]
    fn test_incremental_parsing_updates() {
        let mut state = ParserState::new();
        let doc_id = DocumentId(10);

        // Initial parse
        let source1 = "let x = 1;";
        let highlights1 = state.parse_and_highlight(source1, LanguageId::JavaScript, doc_id, 1);
        assert!(
            !highlights1.lines.is_empty(),
            "Initial parse should produce highlights"
        );

        // Second parse with modified content (simulating edit)
        let source2 = "let x = 1;\nlet y = 2;";
        let highlights2 = state.parse_and_highlight(source2, LanguageId::JavaScript, doc_id, 2);
        assert!(
            !highlights2.lines.is_empty(),
            "Second parse should produce highlights"
        );
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
        eprintln!(
            "Lines from source.lines(): {:?}",
            source.lines().collect::<Vec<_>>()
        );
        eprintln!(
            "Highlights lines: {:?}",
            highlights.lines.keys().collect::<Vec<_>>()
        );
        for (line, lh) in &highlights.lines {
            eprintln!("Line {}: {:?}", line, lh.tokens);
        }

        // Should have highlights on line 1 (the fn main line)
        assert!(!highlights.lines.is_empty(), "Should have some highlights");
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
                eprintln!(
                    "  col {}..{}: highlight {}",
                    tok.start_col, tok.end_col, tok.highlight
                );
            }
        }

        // Line 0 in rope should be empty (just "\n" or "")
        // Line 1 in rope should be "fn main() {}"
        // Highlights should be on line 1

        let rope_line_1 = rope.line(1).to_string();
        eprintln!("\nRope line 1: {:?}", rope_line_1);
        eprintln!(
            "Highlights for line 1 exist: {}",
            highlights.lines.contains_key(&1)
        );

        // The highlight tokens for line 1 should match the text on rope line 1
        if let Some(line_highlights) = highlights.lines.get(&1) {
            // "fn main() {}" - "fn" is keyword at columns 0..2
            let fn_token = line_highlights
                .tokens
                .iter()
                .find(|t| t.start_col == 0 && t.end_col == 2);
            assert!(
                fn_token.is_some(),
                "Should have 'fn' token at 0..2 on line 1"
            );
        } else {
            panic!("No highlights for line 1!");
        }
    }

    #[test]
    fn test_incremental_parse_insert_char() {
        let mut state = ParserState::new();
        let doc_id = DocumentId(100);

        // Initial parse
        let source1 = "let x = 1;";
        let h1 = state.parse_and_highlight(source1, LanguageId::JavaScript, doc_id, 1);
        assert!(!h1.lines.is_empty());

        // Insert a character (simulates typing)
        let source2 = "let x = 12;";
        let h2 = state.parse_and_highlight(source2, LanguageId::JavaScript, doc_id, 2);
        assert!(!h2.lines.is_empty());

        // Cache should be populated
        assert!(state.doc_cache.contains_key(&doc_id));
    }

    #[test]
    fn test_incremental_parse_delete_char() {
        let mut state = ParserState::new();
        let doc_id = DocumentId(101);

        // Initial parse
        let source1 = "let x = 123;";
        let h1 = state.parse_and_highlight(source1, LanguageId::JavaScript, doc_id, 1);
        assert!(!h1.lines.is_empty());

        // Delete characters
        let source2 = "let x = 1;";
        let h2 = state.parse_and_highlight(source2, LanguageId::JavaScript, doc_id, 2);
        assert!(!h2.lines.is_empty());
    }

    #[test]
    fn test_incremental_parse_multiline_insert() {
        let mut state = ParserState::new();
        let doc_id = DocumentId(102);

        // Initial parse
        let source1 = "fn main() {}";
        let h1 = state.parse_and_highlight(source1, LanguageId::Rust, doc_id, 1);
        assert!(!h1.lines.is_empty());

        // Insert newline and content
        let source2 = "fn main() {\n    let x = 1;\n}";
        let h2 = state.parse_and_highlight(source2, LanguageId::Rust, doc_id, 2);
        assert!(!h2.lines.is_empty());
        // Should have highlights on multiple lines now
        assert!(
            h2.lines.len() >= 2,
            "Should have highlights on multiple lines"
        );
    }

    #[test]
    fn test_incremental_parse_language_change() {
        let mut state = ParserState::new();
        let doc_id = DocumentId(103);

        // Parse as Rust
        let source = "let x = 1;";
        let h1 = state.parse_and_highlight(source, LanguageId::Rust, doc_id, 1);
        assert!(!h1.lines.is_empty());

        // Parse same source as JavaScript (language change should trigger full reparse)
        let h2 = state.parse_and_highlight(source, LanguageId::JavaScript, doc_id, 2);
        assert!(!h2.lines.is_empty());

        // Cache should reflect JavaScript now
        assert_eq!(
            state.doc_cache.get(&doc_id).unwrap().language,
            LanguageId::JavaScript
        );
    }

    #[test]
    fn test_incremental_parse_unchanged_source() {
        let mut state = ParserState::new();
        let doc_id = DocumentId(104);

        let source = "let x = 1;";

        // First parse
        let h1 = state.parse_and_highlight(source, LanguageId::JavaScript, doc_id, 1);
        assert!(!h1.lines.is_empty());

        // Same source again (should reuse cached tree)
        let h2 = state.parse_and_highlight(source, LanguageId::JavaScript, doc_id, 2);
        assert!(!h2.lines.is_empty());

        // Results should be identical
        assert_eq!(h1.lines.len(), h2.lines.len());
    }

    #[test]
    fn test_clear_doc_cache() {
        let mut state = ParserState::new();
        let doc_id = DocumentId(105);

        // Parse to populate cache
        let source = "let x = 1;";
        state.parse_and_highlight(source, LanguageId::JavaScript, doc_id, 1);
        assert!(state.doc_cache.contains_key(&doc_id));

        // Clear cache
        state.clear_doc_cache(doc_id);
        assert!(!state.doc_cache.contains_key(&doc_id));
    }

    #[test]
    fn test_compute_incremental_edit_insert() {
        // Insert "X" at position 5
        let old = "helloworld";
        let new = "helloXworld";
        let edit = compute_incremental_edit(old, new).unwrap();

        assert_eq!(edit.start_byte, 5);
        assert_eq!(edit.old_end_byte, 5);
        assert_eq!(edit.new_end_byte, 6);
    }

    #[test]
    fn test_compute_incremental_edit_delete() {
        // Delete "X" at position 5
        let old = "helloXworld";
        let new = "helloworld";
        let edit = compute_incremental_edit(old, new).unwrap();

        assert_eq!(edit.start_byte, 5);
        assert_eq!(edit.old_end_byte, 6);
        assert_eq!(edit.new_end_byte, 5);
    }

    #[test]
    fn test_compute_incremental_edit_replace() {
        // Replace "foo" with "bar"
        let old = "hello foo world";
        let new = "hello bar world";
        let edit = compute_incremental_edit(old, new).unwrap();

        assert_eq!(edit.start_byte, 6);
        assert_eq!(edit.old_end_byte, 9);
        assert_eq!(edit.new_end_byte, 9);
    }

    #[test]
    fn test_compute_incremental_edit_identical() {
        let source = "hello world";
        assert!(compute_incremental_edit(source, source).is_none());
    }

    #[test]
    fn test_byte_to_point_simple() {
        let text = "hello\nworld";
        // "hello" is on row 0, "world" is on row 1

        let p0 = byte_to_point(text, 0);
        assert_eq!(p0.row, 0);
        assert_eq!(p0.column, 0);

        let p5 = byte_to_point(text, 5);
        assert_eq!(p5.row, 0);
        assert_eq!(p5.column, 5);

        let p6 = byte_to_point(text, 6);
        assert_eq!(p6.row, 1);
        assert_eq!(p6.column, 0);

        let p11 = byte_to_point(text, 11);
        assert_eq!(p11.row, 1);
        assert_eq!(p11.column, 5);
    }
}
