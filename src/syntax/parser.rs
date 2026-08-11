//! Tree-sitter parser state and highlighting extraction
//!
//! Manages parsers, trees, and queries for syntax highlighting.
//! Supports incremental parsing by caching trees and computing edits.

use std::collections::HashMap;
use std::ops::Range;
use std::time::Instant;

use streaming_iterator::StreamingIterator;
use tree_sitter::{InputEdit, Parser, Point, Query, QueryCursor, Tree, TreeCursor};

use super::highlights::{highlight_id_for_name, HighlightToken, SyntaxHighlights};
use super::languages::LanguageId;
use super::selection::{InjectedSyntaxTree, SyntaxTreeSnapshot};
use crate::model::editor_area::DocumentId;

/// Cached parse state for a document (enables incremental parsing)
struct DocParseState {
    /// The language this tree was parsed with
    language: LanguageId,
    /// The parsed tree
    tree: Tree,
    /// The source text that was parsed (needed for computing edits)
    source: String,
    highlights: Option<SyntaxHighlights>,
}

/// Convert byte column to character column (handles UTF-8 multi-byte chars)
fn byte_to_char_col(text: &str, byte_col: usize) -> usize {
    let byte_col = byte_col.min(text.len());
    let mut valid_byte = byte_col;
    while valid_byte > 0 && !text.is_char_boundary(valid_byte) {
        valid_byte -= 1;
    }
    text[..valid_byte].chars().count()
}

#[derive(Debug, Clone)]
pub(crate) struct InjectionRegion {
    pub language: LanguageId,
    pub range: Range<usize>,
}

pub(crate) trait InjectionBehavior: Sync {
    fn regions(&self, tree: &Tree, source: &str) -> Vec<InjectionRegion>;
}

struct NoInjections;

impl InjectionBehavior for NoInjections {
    fn regions(&self, _tree: &Tree, _source: &str) -> Vec<InjectionRegion> {
        Vec::new()
    }
}

type InjectionMatcher =
    for<'tree> fn(tree_sitter::Node<'tree>, &str) -> Option<(LanguageId, tree_sitter::Node<'tree>)>;

struct NodeInjections {
    matcher: InjectionMatcher,
}

impl InjectionBehavior for NodeInjections {
    fn regions(&self, tree: &Tree, source: &str) -> Vec<InjectionRegion> {
        let mut regions = Vec::new();
        let mut stack = vec![tree.root_node()];
        while let Some(node) = stack.pop() {
            if let Some((language, content)) = (self.matcher)(node, source) {
                regions.push(InjectionRegion {
                    language,
                    range: content.start_byte()..content.end_byte(),
                });
            }
            let mut cursor = node.walk();
            stack.extend(node.named_children(&mut cursor));
        }
        regions
    }
}

fn markdown_injection<'tree>(
    node: tree_sitter::Node<'tree>,
    source: &str,
) -> Option<(LanguageId, tree_sitter::Node<'tree>)> {
    if node.kind() == "fenced_code_block" {
        let mut injected_language = None;
        let mut content = None;
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            match child.kind() {
                "info_string" => {
                    injected_language = child
                        .named_child(0)
                        .filter(|language| language.kind() == "language")
                        .and_then(|language| language.utf8_text(source.as_bytes()).ok())
                        .and_then(LanguageId::from_code_fence_info);
                }
                "code_fence_content" => content = Some(child),
                _ => {}
            }
        }
        if let (Some(injected_language), Some(content)) = (injected_language, content) {
            return Some((injected_language, content));
        }
    }
    None
}

fn html_injection<'tree>(
    node: tree_sitter::Node<'tree>,
    _source: &str,
) -> Option<(LanguageId, tree_sitter::Node<'tree>)> {
    if node.kind() == "raw_text" {
        let parent = node.parent();
        let injected_language = match parent.map(|parent| parent.kind()) {
            Some("script_element") => Some(LanguageId::JavaScript),
            Some("style_element") => Some(LanguageId::Css),
            _ => None,
        };
        if let Some(injected_language) = injected_language {
            return Some((injected_language, node));
        }
    }
    None
}

fn component_injection<'tree>(
    node: tree_sitter::Node<'tree>,
    source: &str,
) -> Option<(LanguageId, tree_sitter::Node<'tree>)> {
    if node.kind() != "raw_text" {
        return None;
    }
    let parent = node.parent()?;
    let language = match parent.kind() {
        "script_element" => detect_component_script_language(parent, source),
        "style_element" => detect_component_style_language(parent, source),
        _ => return None,
    };
    Some((language, node))
}

fn astro_injection<'tree>(
    node: tree_sitter::Node<'tree>,
    source: &str,
) -> Option<(LanguageId, tree_sitter::Node<'tree>)> {
    if node.kind() == "frontmatter_js_block" {
        Some((LanguageId::TypeScript, node))
    } else {
        component_injection(node, source)
    }
}

fn docker_injection<'tree>(
    node: tree_sitter::Node<'tree>,
    _source: &str,
) -> Option<(LanguageId, tree_sitter::Node<'tree>)> {
    (node.kind() == "shell_command").then_some((LanguageId::Bash, node))
}

fn make_injection<'tree>(
    node: tree_sitter::Node<'tree>,
    _source: &str,
) -> Option<(LanguageId, tree_sitter::Node<'tree>)> {
    matches!(node.kind(), "shell_text" | "shell_command").then_some((LanguageId::Bash, node))
}

fn tera_injection<'tree>(
    node: tree_sitter::Node<'tree>,
    _source: &str,
) -> Option<(LanguageId, tree_sitter::Node<'tree>)> {
    (node.kind() == "content"
        && node
            .parent()
            .is_some_and(|parent| parent.kind() == "frontmatter"))
    .then_some((LanguageId::Yaml, node))
}

fn hurl_injection<'tree>(
    node: tree_sitter::Node<'tree>,
    _source: &str,
) -> Option<(LanguageId, tree_sitter::Node<'tree>)> {
    match node.kind() {
        "json_value" if node.parent().is_some_and(|parent| parent.kind() == "bytes") => {
            Some((LanguageId::Json, node))
        }
        "xml" => Some((LanguageId::Xml, node)),
        _ => None,
    }
}

fn typst_injection<'tree>(
    node: tree_sitter::Node<'tree>,
    source: &str,
) -> Option<(LanguageId, tree_sitter::Node<'tree>)> {
    if node.kind() == "blob"
        && node
            .parent()
            .is_some_and(|parent| parent.kind() == "raw_blck")
    {
        let injected_language = node
            .parent()
            .and_then(|parent| parent.child_by_field_name("lang"))
            .and_then(|lang| lang.utf8_text(source.as_bytes()).ok())
            .and_then(LanguageId::from_code_fence_info);
        if let Some(injected_language) = injected_language {
            return Some((injected_language, node));
        }
    }
    None
}

static NO_INJECTIONS_IMPL: NoInjections = NoInjections;
pub(crate) static NO_INJECTIONS: &dyn InjectionBehavior = &NO_INJECTIONS_IMPL;

macro_rules! node_injections {
    ($name:ident, $impl_name:ident, $matcher:ident) => {
        static $impl_name: NodeInjections = NodeInjections { matcher: $matcher };
        pub(crate) static $name: &dyn InjectionBehavior = &$impl_name;
    };
}

node_injections!(
    MARKDOWN_INJECTIONS,
    MARKDOWN_INJECTIONS_IMPL,
    markdown_injection
);
node_injections!(HTML_INJECTIONS, HTML_INJECTIONS_IMPL, html_injection);
node_injections!(
    COMPONENT_INJECTIONS,
    COMPONENT_INJECTIONS_IMPL,
    component_injection
);
node_injections!(ASTRO_INJECTIONS, ASTRO_INJECTIONS_IMPL, astro_injection);
node_injections!(DOCKER_INJECTIONS, DOCKER_INJECTIONS_IMPL, docker_injection);
node_injections!(MAKE_INJECTIONS, MAKE_INJECTIONS_IMPL, make_injection);
node_injections!(TERA_INJECTIONS, TERA_INJECTIONS_IMPL, tera_injection);
node_injections!(HURL_INJECTIONS, HURL_INJECTIONS_IMPL, hurl_injection);
node_injections!(TYPST_INJECTIONS, TYPST_INJECTIONS_IMPL, typst_injection);

/// Sort each line's tokens by span and resolve identical-span duplicates
/// with last-capture-wins.
///
/// Tree-sitter queries follow the last-match-wins convention: generic
/// fallbacks (e.g. `(identifier) @variable`) come first in the query file
/// and specific overrides (function names, keywords gated by predicates)
/// come last. Keeping the last capture for a span makes those overrides
/// take effect; keeping the first would paint everything with the fallback.
fn finalize_line_tokens(highlights: &mut SyntaxHighlights) {
    for line_highlights in highlights.lines.values_mut() {
        let tokens = &mut line_highlights.tokens;
        // Stable sort preserves capture order within identical spans
        tokens.sort_by_key(|t| (t.start_col, t.end_col));
        // Collapse runs of identical spans, keeping the LAST capture
        tokens.dedup_by(|next, kept| {
            if next.start_col == kept.start_col && next.end_col == kept.end_col {
                kept.highlight = next.highlight;
                true
            } else {
                false
            }
        });
    }
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

fn merge_line_ranges(ranges: &mut Vec<Range<usize>>) {
    ranges.sort_by_key(|range| range.start);
    let mut merged: Vec<Range<usize>> = Vec::with_capacity(ranges.len());
    for range in ranges.drain(..) {
        if let Some(previous) = merged
            .last_mut()
            .filter(|previous| range.start <= previous.end)
        {
            previous.end = previous.end.max(range.end);
        } else {
            merged.push(range);
        }
    }
    *ranges = merged;
}

// Embedded query files
pub(crate) const YAML_HIGHLIGHTS: &str = include_str!("../../queries/yaml/highlights.scm");
pub(crate) const MARKDOWN_HIGHLIGHTS: &str = include_str!("../../queries/markdown/highlights.scm");
pub(crate) const MARKDOWN_INLINE_HIGHLIGHTS: &str =
    include_str!("../../queries/markdown/inline-highlights.scm");
pub(crate) const RUST_HIGHLIGHTS: &str = tree_sitter_rust::HIGHLIGHTS_QUERY;

pub(crate) const HTML_HIGHLIGHTS: &str = include_str!("../../queries/html/highlights.scm");
pub(crate) const CSS_HIGHLIGHTS: &str = include_str!("../../queries/css/highlights.scm");
pub(crate) const JAVASCRIPT_HIGHLIGHTS: &str =
    include_str!("../../queries/javascript/highlights.scm");
pub(crate) const JSX_HIGHLIGHTS: &str = tree_sitter_javascript::JSX_HIGHLIGHT_QUERY;

pub(crate) const TYPESCRIPT_HIGHLIGHTS: &str =
    include_str!("../../queries/typescript/highlights.scm");
pub(crate) const TSX_HIGHLIGHTS: &str = include_str!("../../queries/tsx/highlights.scm");
pub(crate) const JSON_HIGHLIGHTS: &str = include_str!("../../queries/json/highlights.scm");
pub(crate) const TOML_HIGHLIGHTS: &str = include_str!("../../queries/toml/highlights.scm");

pub(crate) const PYTHON_HIGHLIGHTS: &str = tree_sitter_python::HIGHLIGHTS_QUERY;
pub(crate) const GO_HIGHLIGHTS: &str = tree_sitter_go::HIGHLIGHTS_QUERY;
pub(crate) const PHP_HIGHLIGHTS: &str = tree_sitter_php::HIGHLIGHTS_QUERY;

pub(crate) const C_HIGHLIGHTS: &str = tree_sitter_c::HIGHLIGHT_QUERY;
pub(crate) const CPP_HIGHLIGHTS: &str = tree_sitter_cpp::HIGHLIGHT_QUERY;
pub(crate) const JAVA_HIGHLIGHTS: &str = tree_sitter_java::HIGHLIGHTS_QUERY;
pub(crate) const BASH_HIGHLIGHTS: &str = tree_sitter_bash::HIGHLIGHT_QUERY;

pub(crate) const SCHEME_HIGHLIGHTS: &str = tree_sitter_racket::HIGHLIGHTS_QUERY;
pub(crate) const INI_HIGHLIGHTS: &str = tree_sitter_ini::HIGHLIGHTS_QUERY;
pub(crate) const XML_HIGHLIGHTS: &str = tree_sitter_xml::XML_HIGHLIGHT_QUERY;
pub(crate) const SEMA_HIGHLIGHTS: &str = include_str!("../../queries/sema/highlights.scm");

pub(crate) const BLADE_HIGHLIGHTS: &str = include_str!("../../queries/blade/highlights.scm");
pub(crate) const SVELTE_HIGHLIGHTS: &str = tree_sitter_svelte_ng::HIGHLIGHTS_QUERY;

pub(crate) const JUST_HIGHLIGHTS: &str = tree_sitter_just::HIGHLIGHTS_QUERY;
pub(crate) const APPLESCRIPT_HIGHLIGHTS: &str =
    include_str!("../../queries/applescript/highlights.scm");
pub(crate) const CSHARP_HIGHLIGHTS: &str = tree_sitter_c_sharp::HIGHLIGHTS_QUERY;
pub(crate) const RUBY_HIGHLIGHTS: &str = tree_sitter_ruby::HIGHLIGHTS_QUERY;
pub(crate) const LUA_HIGHLIGHTS: &str = tree_sitter_lua::HIGHLIGHTS_QUERY;
pub(crate) const R_HIGHLIGHTS: &str = tree_sitter_r::HIGHLIGHTS_QUERY;
pub(crate) const SWIFT_HIGHLIGHTS: &str = tree_sitter_swift::HIGHLIGHTS_QUERY;
pub(crate) const ELIXIR_HIGHLIGHTS: &str = tree_sitter_elixir::HIGHLIGHTS_QUERY;
pub(crate) const GLEAM_HIGHLIGHTS: &str = tree_sitter_gleam::HIGHLIGHT_QUERY;
pub(crate) const SOLIDITY_HIGHLIGHTS: &str = tree_sitter_solidity::HIGHLIGHT_QUERY;
pub(crate) const KOTLIN_HIGHLIGHTS: &str = include_str!("../../queries/kotlin/highlights.scm");
pub(crate) const DART_HIGHLIGHTS: &str = tree_sitter_dart::HIGHLIGHTS_QUERY;
pub(crate) const JULIA_HIGHLIGHTS: &str = include_str!("../../queries/julia/highlights.scm");
pub(crate) const HASKELL_HIGHLIGHTS: &str = tree_sitter_haskell::HIGHLIGHTS_QUERY;
pub(crate) const OCAML_HIGHLIGHTS: &str = tree_sitter_ocaml::HIGHLIGHTS_QUERY;
pub(crate) const OCAML_INTERFACE_HIGHLIGHTS: &str =
    include_str!("../../queries/ocaml-interface/highlights.scm");
pub(crate) const D_HIGHLIGHTS: &str = include_str!("../../queries/d/highlights.scm");
pub(crate) const OBJC_HIGHLIGHTS: &str = tree_sitter_objc::HIGHLIGHTS_QUERY;
pub(crate) const VHDL_HIGHLIGHTS: &str = tree_sitter_vhdl::HIGHLIGHTS_QUERY;
pub(crate) const ODIN_HIGHLIGHTS: &str = tree_sitter_odin::HIGHLIGHTS_QUERY;
pub(crate) const FISH_HIGHLIGHTS: &str = tree_sitter_fish::HIGHLIGHTS_QUERY;
pub(crate) const ASM_HIGHLIGHTS: &str = tree_sitter_asm::HIGHLIGHTS_QUERY;
pub(crate) const SCSS_HIGHLIGHTS: &str = tree_sitter_scss::HIGHLIGHTS_QUERY;
pub(crate) const CMAKE_HIGHLIGHTS: &str = tree_sitter_cmake::HIGHLIGHTS_QUERY;
pub(crate) const MAKE_HIGHLIGHTS: &str = tree_sitter_make::HIGHLIGHTS_QUERY;
pub(crate) const COMMONLISP_HIGHLIGHTS: &str =
    include_str!("../../queries/commonlisp/highlights.scm");
pub(crate) const ZIG_HIGHLIGHTS: &str = tree_sitter_zig::HIGHLIGHTS_QUERY;
pub(crate) const GLSL_HIGHLIGHTS: &str = tree_sitter_glsl::HIGHLIGHTS_QUERY;
pub(crate) const GRAPHQL_HIGHLIGHTS: &str = include_str!("../../queries/graphql/highlights.scm");
pub(crate) const HCL_HIGHLIGHTS: &str = include_str!("../../queries/hcl/highlights.scm");
pub(crate) const NIX_HIGHLIGHTS: &str = tree_sitter_nix::HIGHLIGHTS_QUERY;
pub(crate) const ADA_HIGHLIGHTS: &str = include_str!("../../queries/ada/highlights.scm");
pub(crate) const ERLANG_HIGHLIGHTS: &str = tree_sitter_erlang::HIGHLIGHTS_QUERY;
pub(crate) const CLOJURE_HIGHLIGHTS: &str = include_str!("../../queries/clojure/highlights.scm");
pub(crate) const NU_HIGHLIGHTS: &str = tree_sitter_nu::HIGHLIGHTS_QUERY;
pub(crate) const SED_HIGHLIGHTS: &str = include_str!("../../queries/sed/highlights.scm");
pub(crate) const TCL_HIGHLIGHTS: &str = include_str!("../../queries/tcl/highlights.scm");
pub(crate) const ROC_HIGHLIGHTS: &str = include_str!("../../queries/roc/highlights.scm");
pub(crate) const JANET_HIGHLIGHTS: &str = include_str!("../../queries/janet/highlights.scm");
pub(crate) const FORTH_HIGHLIGHTS: &str = tree_sitter_forth::HIGHLIGHTS_QUERY;
pub(crate) const PROTO_HIGHLIGHTS: &str = include_str!("../../queries/proto/highlights.scm");
pub(crate) const DHALL_HIGHLIGHTS: &str = tree_sitter_dhall::HIGHLIGHTS_QUERY;
pub(crate) const PKL_HIGHLIGHTS: &str = include_str!("../../queries/pkl/highlights.scm");
pub(crate) const HURL_HIGHLIGHTS: &str = include_str!("../../queries/hurl/highlights.scm");
pub(crate) const WIT_HIGHLIGHTS: &str = tree_sitter_wit::HIGHLIGHTS_QUERY;
pub(crate) const SML_HIGHLIGHTS: &str = include_str!("../../queries/sml/highlights.scm");
pub(crate) const NIM_HIGHLIGHTS: &str = include_str!("../../queries/nim/highlights.scm");
pub(crate) const ASTRO_HIGHLIGHTS: &str = tree_sitter_astro_next::HIGHLIGHTS_QUERY;
pub(crate) const AWK_HIGHLIGHTS: &str = arborium_awk::HIGHLIGHTS_QUERY;
pub(crate) const KDL_HIGHLIGHTS: &str = arborium_kdl::HIGHLIGHTS_QUERY;
pub(crate) const TERA_HIGHLIGHTS: &str = include_str!("../../queries/tera/highlights.scm");
pub(crate) const TYPST_HIGHLIGHTS: &str = include_str!("../../queries/typst/highlights.scm");
pub(crate) const DOCKERFILE_HIGHLIGHTS: &str = tree_sitter_containerfile::HIGHLIGHTS_QUERY;
pub(crate) const WGSL_HIGHLIGHTS: &str = include_str!("../../queries/wgsl/highlights.scm");
pub(crate) const SQL_HIGHLIGHTS: &str = tree_sitter_sequel::HIGHLIGHTS_QUERY;
pub(crate) const V_HIGHLIGHTS: &str = include_str!("../../queries/v/highlights.scm");
pub(crate) const CUE_HIGHLIGHTS: &str = include_str!("../../queries/cue/highlights.scm");
pub(crate) const FENNEL_HIGHLIGHTS: &str = include_str!("../../queries/fennel/highlights.scm");
pub(crate) const PEST_HIGHLIGHTS: &str = include_str!("../../queries/pest/highlights.scm");
pub(crate) const PONY_HIGHLIGHTS: &str = include_str!("../../queries/pony/highlights.scm");

/// Thread-local parser state (tree-sitter parsers are !Sync)
pub struct ParserState {
    /// Parser instances per language
    parsers: HashMap<LanguageId, Parser>,
    /// Compiled queries per language
    queries: HashMap<LanguageId, Query>,
    /// Cached parse state per document (for incremental parsing)
    doc_cache: HashMap<DocumentId, DocParseState>,
    /// Markdown inline grammar parser (for two-pass parsing)
    markdown_inline_parser: Option<Parser>,
    /// Markdown inline grammar query
    markdown_inline_query: Option<Query>,
    last_timing: ParserTiming,
    last_changed_line_ranges: Option<Vec<Range<usize>>>,
    last_highlight_patch: Option<SyntaxHighlights>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ParserTiming {
    pub parse_ms: Option<f64>,
    pub highlight_ms: Option<f64>,
}

impl ParserState {
    /// Get cached tree for a document (for outline extraction on worker thread)
    pub fn get_cached_tree(&self, doc_id: DocumentId) -> Option<(&tree_sitter::Tree, LanguageId)> {
        self.doc_cache
            .get(&doc_id)
            .map(|state| (&state.tree, state.language))
    }

    /// Clone the host tree and parse supported embedded regions with document-
    /// relative ranges so structural selection can cross language boundaries.
    pub fn syntax_tree_snapshot(
        &mut self,
        doc_id: DocumentId,
        revision: u64,
    ) -> Option<SyntaxTreeSnapshot> {
        let cached = self.doc_cache.get(&doc_id)?;
        let source = cached.source.clone();
        let language = cached.language;
        let tree = cached.tree.clone();
        let regions = crate::syntax::registry::language(language)
            .injections
            .regions(&tree, &source);
        let mut injections = Vec::with_capacity(regions.len());
        for region in regions {
            self.ensure_language(region.language);
            let Some(parser) = self.parsers.get_mut(&region.language) else {
                continue;
            };
            let included_range = tree_sitter::Range {
                start_byte: region.range.start,
                end_byte: region.range.end,
                start_point: byte_to_point(&source, region.range.start),
                end_point: byte_to_point(&source, region.range.end),
            };
            if parser.set_included_ranges(&[included_range]).is_err() {
                continue;
            }
            let injected_tree = parser.parse(&source, None);
            let _ = parser.set_included_ranges(&[]);
            if let Some(injected_tree) = injected_tree {
                injections.push(InjectedSyntaxTree {
                    language: region.language,
                    range: region.range,
                    tree: injected_tree,
                });
            }
        }
        Some(SyntaxTreeSnapshot::new(revision, language, tree).with_injections(injections))
    }

    /// Create a new parser state with initialized languages
    pub fn new() -> Self {
        let mut state = Self {
            parsers: HashMap::new(),
            queries: HashMap::new(),
            doc_cache: HashMap::new(),
            markdown_inline_parser: None,
            markdown_inline_query: None,
            last_timing: ParserTiming::default(),
            last_changed_line_ranges: None,
            last_highlight_patch: None,
        };

        // Initialize markdown inline parser for two-pass parsing
        state.init_markdown_inline();

        state
    }

    fn ensure_language(&mut self, language: LanguageId) {
        if super::registry::language(language).parser.is_some()
            && !self.parsers.contains_key(&language)
        {
            self.init_language(language);
        }
    }

    /// Initialize the markdown inline grammar parser and query
    fn init_markdown_inline(&mut self) {
        let ts_lang: tree_sitter::Language = tree_sitter_md::INLINE_LANGUAGE.into();

        // Create inline parser
        let mut parser = Parser::new();
        if let Err(e) = parser.set_language(&ts_lang) {
            tracing::error!("Failed to set markdown inline language: {}", e);
            return;
        }

        // Create inline query
        match Query::new(&ts_lang, MARKDOWN_INLINE_HIGHLIGHTS) {
            Ok(query) => {
                self.markdown_inline_parser = Some(parser);
                self.markdown_inline_query = Some(query);
                tracing::debug!("Markdown inline parser initialized successfully");
            }
            Err(e) => {
                tracing::error!("Failed to compile markdown inline query: {:?}", e);
            }
        }
    }

    /// Initialize a language's parser and query
    fn init_language(&mut self, lang: LanguageId) {
        let Some(definition) = super::registry::language(lang).parser else {
            return;
        };
        let ts_lang = (definition.grammar)();
        let highlights = definition.highlights.query();

        // Create parser
        let mut parser = Parser::new();
        if let Err(e) = parser.set_language(&ts_lang) {
            tracing::error!("Failed to set language for {:?}: {}", lang, e);
            return;
        }
        self.parsers.insert(lang, parser);

        // Create query (may fail if query syntax is invalid)
        match Query::new(&ts_lang, highlights.as_ref()) {
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
        self.last_timing = ParserTiming::default();
        self.last_changed_line_ranges = None;
        self.last_highlight_patch = None;
        // Skip plain text
        if language == LanguageId::PlainText {
            return SyntaxHighlights::new(language, revision);
        }

        self.ensure_language(language);

        // Use specialized two-pass parsing for markdown (block + inline)
        if language == LanguageId::Markdown {
            let highlights = self.parse_and_highlight_markdown(source, doc_id, revision);
            return highlights;
        }

        // Use specialized parsing with language injection for HTML
        if language == LanguageId::Html {
            let highlights = self.parse_and_highlight_html(source, doc_id, revision);
            return highlights;
        }

        // Use specialized parsing with language injection for Vue SFC
        if language == LanguageId::Vue {
            let highlights = self.parse_and_highlight_vue(source, doc_id, revision);
            return highlights;
        }

        let parser = match self.parsers.get_mut(&language) {
            Some(p) => p,
            None => {
                tracing::warn!("No parser for language {:?}", language);
                return SyntaxHighlights::new(language, revision);
            }
        };

        // Try incremental parsing if we have a cached tree for this document
        let parse_started = Instant::now();
        let tree = if let Some(cached) = self.doc_cache.get_mut(&doc_id) {
            if cached.language == language {
                // Same language, try incremental parse
                if let Some(edit) = compute_incremental_edit(&cached.source, source) {
                    let old_line_count =
                        cached.source.bytes().filter(|byte| *byte == b'\n').count() + 1;
                    let new_line_count = source.bytes().filter(|byte| *byte == b'\n').count() + 1;
                    if let Some(highlights) = &mut cached.highlights {
                        highlights.shift_for_edit(
                            edit.start_position.row,
                            old_line_count,
                            new_line_count,
                        );
                    }
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
                            let line_count =
                                source.bytes().filter(|byte| *byte == b'\n').count() + 1;
                            let mut changed_ranges: Vec<_> = cached
                                .tree
                                .changed_ranges(&new_tree)
                                .map(|range| {
                                    range.start_point.row.saturating_sub(1)
                                        ..(range.end_point.row + 2).min(line_count)
                                })
                                .collect();
                            if changed_ranges.is_empty() {
                                changed_ranges.push(
                                    edit.start_position.row.saturating_sub(1)
                                        ..(edit.new_end_position.row + 2).min(line_count),
                                );
                            }
                            merge_line_ranges(&mut changed_ranges);
                            self.last_changed_line_ranges = Some(changed_ranges);
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
                                            highlights: None,
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
                                highlights: None,
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
                            highlights: None,
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
        self.last_timing.parse_ms = Some(parse_started.elapsed().as_secs_f64() * 1000.0);

        // Extract highlights
        if self.last_changed_line_ranges.is_some()
            && self
                .doc_cache
                .get(&doc_id)
                .is_none_or(|cached| cached.highlights.is_none())
        {
            self.last_changed_line_ranges = None;
        }
        // Component injection currently rebuilds the combined highlight set.
        // Keep tree parsing incremental, but query the complete outer tree so
        // unchanged outer-language tokens are not replaced by a partial patch.
        if language == LanguageId::Svelte {
            self.last_changed_line_ranges = None;
        }
        let highlight_started = Instant::now();
        let mut patch = self.extract_highlights(
            source,
            &tree,
            language,
            revision,
            self.last_changed_line_ranges.as_deref(),
        );
        let mut highlights = if let (Some(ranges), Some(cached)) = (
            self.last_changed_line_ranges.as_ref(),
            self.doc_cache.get_mut(&doc_id),
        ) {
            if let Some(mut existing) = cached.highlights.take() {
                let patch_for_worker = patch.clone();
                for range in ranges {
                    existing.lines.retain(|line, _| !range.contains(line));
                }
                existing.lines.extend(std::mem::take(&mut patch.lines));
                existing.revision = revision;
                existing.language = language;
                self.last_highlight_patch = Some(patch_for_worker);
                cached.highlights = Some(existing.clone());
                existing
            } else {
                self.last_changed_line_ranges = None;
                cached.highlights = Some(patch.clone());
                patch
            }
        } else {
            if let Some(cached) = self.doc_cache.get_mut(&doc_id) {
                cached.highlights = Some(patch.clone());
            }
            patch
        };
        if language == LanguageId::Svelte {
            self.extract_component_embedded_highlights(source, &tree, &mut highlights);
            finalize_line_tokens(&mut highlights);
            if let Some(cached) = self.doc_cache.get_mut(&doc_id) {
                cached.highlights = Some(highlights.clone());
            }
        }
        self.last_timing.highlight_ms = Some(highlight_started.elapsed().as_secs_f64() * 1000.0);
        highlights
    }

    pub fn last_timing(&self) -> ParserTiming {
        self.last_timing
    }

    pub fn last_changed_line_ranges(&self) -> Option<Vec<Range<usize>>> {
        self.last_changed_line_ranges.clone()
    }

    pub fn take_last_highlight_patch(&mut self) -> Option<SyntaxHighlights> {
        self.last_highlight_patch.take()
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
        line_ranges: Option<&[Range<usize>]>,
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

        let mut extract_range = |line_range: Range<usize>| {
            cursor.set_point_range(Point::new(line_range.start, 0)..Point::new(line_range.end, 0));
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
        };
        if let Some(ranges) = line_ranges {
            for range in ranges {
                extract_range(range.clone());
            }
        } else {
            extract_range(0..lines.len().max(1));
        }

        finalize_line_tokens(&mut highlights);

        highlights
    }

    /// Two-pass markdown parsing: block structure + inline elements
    fn parse_and_highlight_markdown(
        &mut self,
        source: &str,
        doc_id: DocumentId,
        revision: u64,
    ) -> SyntaxHighlights {
        let language = LanguageId::Markdown;

        // Step 1: Parse block structure with existing block parser
        let parser = match self.parsers.get_mut(&language) {
            Some(p) => p,
            None => {
                tracing::warn!("No parser for markdown");
                return SyntaxHighlights::new(language, revision);
            }
        };

        // Try incremental parsing if we have a cached tree
        let block_tree = if let Some(cached) = self.doc_cache.get_mut(&doc_id) {
            if cached.language == language {
                if let Some(edit) = compute_incremental_edit(&cached.source, source) {
                    cached.tree.edit(&edit);
                    match parser.parse(source, Some(&cached.tree)) {
                        Some(new_tree) => {
                            cached.tree = new_tree.clone();
                            cached.source = source.to_owned();
                            new_tree
                        }
                        None => {
                            self.doc_cache.remove(&doc_id);
                            match parser.parse(source, None) {
                                Some(t) => {
                                    self.doc_cache.insert(
                                        doc_id,
                                        DocParseState {
                                            language,
                                            tree: t.clone(),
                                            source: source.to_owned(),
                                            highlights: None,
                                        },
                                    );
                                    t
                                }
                                None => return SyntaxHighlights::new(language, revision),
                            }
                        }
                    }
                } else {
                    cached.tree.clone()
                }
            } else {
                self.doc_cache.remove(&doc_id);
                match parser.parse(source, None) {
                    Some(t) => {
                        self.doc_cache.insert(
                            doc_id,
                            DocParseState {
                                language,
                                tree: t.clone(),
                                source: source.to_owned(),
                                highlights: None,
                            },
                        );
                        t
                    }
                    None => return SyntaxHighlights::new(language, revision),
                }
            }
        } else {
            match parser.parse(source, None) {
                Some(t) => {
                    self.doc_cache.insert(
                        doc_id,
                        DocParseState {
                            language,
                            tree: t.clone(),
                            source: source.to_owned(),
                            highlights: None,
                        },
                    );
                    t
                }
                None => return SyntaxHighlights::new(language, revision),
            }
        };

        // Step 2: Extract block-level highlights
        let mut highlights = self.extract_highlights(source, &block_tree, language, revision, None);

        // Step 3: Parse inline content if we have the inline parser
        if self.markdown_inline_parser.is_some() && self.markdown_inline_query.is_some() {
            self.parse_markdown_inline_regions(source, &block_tree, &mut highlights);
        }

        // Step 4: Language injection for fenced code blocks
        self.extract_fenced_code_highlights(source, &block_tree, &mut highlights);

        // Re-sort tokens after adding inline and injected highlights
        finalize_line_tokens(&mut highlights);

        highlights
    }

    /// Parse inline content regions within markdown block tree
    fn parse_markdown_inline_regions(
        &mut self,
        source: &str,
        block_tree: &Tree,
        highlights: &mut SyntaxHighlights,
    ) {
        // Node kinds that contain inline content
        const INLINE_NODE_KINDS: &[&str] = &["paragraph", "heading_content", "pipe_table_cell"];

        let lines: Vec<&str> = source.lines().collect();

        // Walk block tree to find nodes with inline content
        let mut cursor = block_tree.walk();
        self.visit_inline_nodes(&mut cursor, source, &lines, highlights, INLINE_NODE_KINDS);
    }

    /// Recursively visit nodes and parse inline content
    fn visit_inline_nodes(
        &mut self,
        cursor: &mut TreeCursor,
        source: &str,
        lines: &[&str],
        highlights: &mut SyntaxHighlights,
        inline_node_kinds: &[&str],
    ) {
        loop {
            let node = cursor.node();

            if inline_node_kinds.contains(&node.kind()) {
                // Parse this node's text with inline grammar
                let start_byte = node.start_byte();
                let end_byte = node.end_byte();

                if start_byte < end_byte && end_byte <= source.len() {
                    let inline_source = &source[start_byte..end_byte];
                    let base_row = node.start_position().row;
                    let base_col = node.start_position().column;

                    self.parse_and_extract_inline_highlights(
                        inline_source,
                        lines,
                        highlights,
                        base_row,
                        base_col,
                    );
                }
            }

            // Recurse into children
            if cursor.goto_first_child() {
                self.visit_inline_nodes(cursor, source, lines, highlights, inline_node_kinds);
                cursor.goto_parent();
            }

            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }

    /// Parse inline source and extract highlights with offset adjustment
    fn parse_and_extract_inline_highlights(
        &mut self,
        inline_source: &str,
        _lines: &[&str],
        highlights: &mut SyntaxHighlights,
        base_row: usize,
        base_col: usize,
    ) {
        let inline_parser = match self.markdown_inline_parser.as_mut() {
            Some(p) => p,
            None => return,
        };
        let inline_query = match &self.markdown_inline_query {
            Some(q) => q,
            None => return,
        };

        // Parse inline content
        let inline_tree = match inline_parser.parse(inline_source, None) {
            Some(t) => t,
            None => return,
        };

        let inline_lines: Vec<&str> = inline_source.lines().collect();
        let source_bytes = inline_source.as_bytes();

        let mut cursor = QueryCursor::new();
        let mut captures = cursor.captures(inline_query, inline_tree.root_node(), source_bytes);

        while let Some((query_match, capture_idx)) = captures.next() {
            let capture = &query_match.captures[*capture_idx];
            let capture_name = &inline_query.capture_names()[capture.index as usize];

            let highlight_id = match highlight_id_for_name(capture_name) {
                Some(id) => id,
                None => continue,
            };

            let node = capture.node;
            let start = node.start_position();
            let end = node.end_position();

            // Handle single-line inline tokens
            if start.row == end.row {
                let local_line = inline_lines.get(start.row).copied().unwrap_or("");
                let start_char = byte_to_char_col(local_line, start.column);
                let end_char = byte_to_char_col(local_line, end.column);

                // Adjust for base position
                let actual_row = base_row + start.row;
                let (actual_start_col, actual_end_col) = if start.row == 0 {
                    // First line: add base column offset
                    let base_char = byte_to_char_col(inline_source.lines().next().unwrap_or(""), 0);
                    let _ = base_char; // base_col is already in chars for first line
                    (base_col + start_char, base_col + end_char)
                } else {
                    (start_char, end_char)
                };

                if actual_start_col < actual_end_col {
                    let line_highlights = highlights.lines.entry(actual_row).or_default();
                    line_highlights.tokens.push(HighlightToken {
                        start_col: actual_start_col,
                        end_col: actual_end_col,
                        highlight: highlight_id,
                    });
                }
            } else {
                // Multi-line inline tokens: split across lines
                for row in start.row..=end.row {
                    let local_line = inline_lines.get(row).copied().unwrap_or("");
                    let line_char_len = local_line.chars().count();

                    let (start_char, end_char) = if row == start.row {
                        let start_char = byte_to_char_col(local_line, start.column);
                        (start_char, line_char_len)
                    } else if row == end.row {
                        let end_char = byte_to_char_col(local_line, end.column);
                        (0, end_char)
                    } else {
                        (0, line_char_len)
                    };

                    let actual_row = base_row + row;
                    let (actual_start_col, actual_end_col) = if row == 0 {
                        (base_col + start_char, base_col + end_char)
                    } else {
                        (start_char, end_char)
                    };

                    if actual_start_col < actual_end_col {
                        let line_highlights = highlights.lines.entry(actual_row).or_default();
                        line_highlights.tokens.push(HighlightToken {
                            start_col: actual_start_col,
                            end_col: actual_end_col,
                            highlight: highlight_id,
                        });
                    }
                }
            }
        }
    }

    /// Extract syntax highlights for fenced code blocks using language injection
    fn extract_fenced_code_highlights(
        &mut self,
        source: &str,
        block_tree: &Tree,
        highlights: &mut SyntaxHighlights,
    ) {
        let lines: Vec<&str> = source.lines().collect();

        // Walk block tree to find fenced_code_block nodes
        let mut cursor = block_tree.walk();
        self.visit_code_blocks(&mut cursor, source, &lines, highlights);
    }

    /// Recursively visit nodes to find and process fenced code blocks
    fn visit_code_blocks(
        &mut self,
        cursor: &mut TreeCursor,
        source: &str,
        lines: &[&str],
        highlights: &mut SyntaxHighlights,
    ) {
        loop {
            let node = cursor.node();

            if node.kind() == "fenced_code_block" {
                self.process_fenced_code_block(node, source, lines, highlights);
            }

            // Recurse into children
            if cursor.goto_first_child() {
                self.visit_code_blocks(cursor, source, lines, highlights);
                cursor.goto_parent();
            }

            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }

    /// Process a single fenced code block for language injection
    fn process_fenced_code_block(
        &mut self,
        node: tree_sitter::Node,
        source: &str,
        _lines: &[&str],
        highlights: &mut SyntaxHighlights,
    ) {
        // Find info_string and code_fence_content children
        let mut language_name: Option<&str> = None;
        let mut content_node: Option<tree_sitter::Node> = None;

        let mut child_cursor = node.walk();
        if child_cursor.goto_first_child() {
            loop {
                let child = child_cursor.node();
                match child.kind() {
                    "info_string" => {
                        // Get the language from info_string's first child (usually "language" node)
                        if let Some(lang_node) = child.child(0) {
                            if lang_node.kind() == "language" {
                                if let Ok(text) = lang_node.utf8_text(source.as_bytes()) {
                                    language_name = Some(text);
                                }
                            }
                        }
                    }
                    "code_fence_content" => {
                        content_node = Some(child);
                    }
                    _ => {}
                }

                if !child_cursor.goto_next_sibling() {
                    break;
                }
            }
        }

        // If we have both language and content, inject highlighting
        let (Some(lang_str), Some(content)) = (language_name, content_node) else {
            return;
        };

        // Map language name to LanguageId
        let Some(lang_id) = LanguageId::from_code_fence_info(lang_str) else {
            return;
        };

        self.ensure_language(lang_id);

        // Skip if we don't have a parser for this language
        let Some(parser) = self.parsers.get_mut(&lang_id) else {
            return;
        };

        // Get the code content
        let start_byte = content.start_byte();
        let end_byte = content.end_byte();
        if start_byte >= end_byte || end_byte > source.len() {
            return;
        }

        let code_source = &source[start_byte..end_byte];
        let base_row = content.start_position().row;
        let base_col = content.start_position().column;

        // Parse with the language's parser
        let Some(code_tree) = parser.parse(code_source, None) else {
            return;
        };

        // Get the query for this language
        let Some(query) = self.queries.get(&lang_id) else {
            return;
        };

        // Extract highlights from the parsed code
        let code_lines: Vec<&str> = code_source.lines().collect();
        let source_bytes = code_source.as_bytes();

        let mut cursor = QueryCursor::new();
        let mut captures = cursor.captures(query, code_tree.root_node(), source_bytes);

        while let Some((query_match, capture_idx)) = captures.next() {
            let capture = &query_match.captures[*capture_idx];
            let capture_name = &query.capture_names()[capture.index as usize];

            let highlight_id = match highlight_id_for_name(capture_name) {
                Some(id) => id,
                None => continue,
            };

            let cap_node = capture.node;
            let start = cap_node.start_position();
            let end = cap_node.end_position();

            // Handle single-line tokens
            if start.row == end.row {
                let local_line = code_lines.get(start.row).copied().unwrap_or("");
                let start_char = byte_to_char_col(local_line, start.column);
                let end_char = byte_to_char_col(local_line, end.column);

                // Adjust for base position
                let actual_row = base_row + start.row;
                let (actual_start_col, actual_end_col) = if start.row == 0 {
                    (base_col + start_char, base_col + end_char)
                } else {
                    (start_char, end_char)
                };

                if actual_start_col < actual_end_col {
                    // Remove any existing highlight for this range (block query may have
                    // marked the whole content as @string)
                    let line_highlights = highlights.lines.entry(actual_row).or_default();
                    line_highlights.tokens.push(HighlightToken {
                        start_col: actual_start_col,
                        end_col: actual_end_col,
                        highlight: highlight_id,
                    });
                }
            } else {
                // Multi-line tokens: split across lines
                for row in start.row..=end.row {
                    let local_line = code_lines.get(row).copied().unwrap_or("");
                    let line_char_len = local_line.chars().count();

                    let (start_char, end_char) = if row == start.row {
                        let start_char = byte_to_char_col(local_line, start.column);
                        (start_char, line_char_len)
                    } else if row == end.row {
                        let end_char = byte_to_char_col(local_line, end.column);
                        (0, end_char)
                    } else {
                        (0, line_char_len)
                    };

                    let actual_row = base_row + row;
                    let (actual_start_col, actual_end_col) = if row == 0 {
                        (base_col + start_char, base_col + end_char)
                    } else {
                        (start_char, end_char)
                    };

                    if actual_start_col < actual_end_col {
                        let line_highlights = highlights.lines.entry(actual_row).or_default();
                        line_highlights.tokens.push(HighlightToken {
                            start_col: actual_start_col,
                            end_col: actual_end_col,
                            highlight: highlight_id,
                        });
                    }
                }
            }
        }
    }

    /// Specialized HTML parsing with script/style language injection
    fn parse_and_highlight_html(
        &mut self,
        source: &str,
        doc_id: DocumentId,
        revision: u64,
    ) -> SyntaxHighlights {
        let language = LanguageId::Html;

        // Step 1: Parse HTML structure
        let parser = match self.parsers.get_mut(&language) {
            Some(p) => p,
            None => {
                tracing::warn!("No parser for HTML");
                return SyntaxHighlights::new(language, revision);
            }
        };

        // Try incremental parsing if cached
        let html_tree = if let Some(cached) = self.doc_cache.get_mut(&doc_id) {
            if cached.language == language {
                if let Some(edit) = compute_incremental_edit(&cached.source, source) {
                    cached.tree.edit(&edit);
                    match parser.parse(source, Some(&cached.tree)) {
                        Some(new_tree) => {
                            cached.tree = new_tree.clone();
                            cached.source = source.to_owned();
                            new_tree
                        }
                        None => {
                            self.doc_cache.remove(&doc_id);
                            match parser.parse(source, None) {
                                Some(t) => {
                                    self.doc_cache.insert(
                                        doc_id,
                                        DocParseState {
                                            language,
                                            tree: t.clone(),
                                            source: source.to_owned(),
                                            highlights: None,
                                        },
                                    );
                                    t
                                }
                                None => return SyntaxHighlights::new(language, revision),
                            }
                        }
                    }
                } else {
                    cached.tree.clone()
                }
            } else {
                self.doc_cache.remove(&doc_id);
                match parser.parse(source, None) {
                    Some(t) => {
                        self.doc_cache.insert(
                            doc_id,
                            DocParseState {
                                language,
                                tree: t.clone(),
                                source: source.to_owned(),
                                highlights: None,
                            },
                        );
                        t
                    }
                    None => return SyntaxHighlights::new(language, revision),
                }
            }
        } else {
            match parser.parse(source, None) {
                Some(t) => {
                    self.doc_cache.insert(
                        doc_id,
                        DocParseState {
                            language,
                            tree: t.clone(),
                            source: source.to_owned(),
                            highlights: None,
                        },
                    );
                    t
                }
                None => return SyntaxHighlights::new(language, revision),
            }
        };

        // Step 2: Extract HTML-level highlights
        let mut highlights = self.extract_highlights(source, &html_tree, language, revision, None);

        // Step 3: Language injection for <script> and <style> elements
        self.extract_html_embedded_highlights(source, &html_tree, &mut highlights);

        // Re-sort tokens after adding injected highlights
        finalize_line_tokens(&mut highlights);

        highlights
    }

    /// Extract highlights for embedded script/style content in HTML
    fn extract_html_embedded_highlights(
        &mut self,
        source: &str,
        html_tree: &Tree,
        highlights: &mut SyntaxHighlights,
    ) {
        let mut cursor = html_tree.walk();
        self.visit_html_embedded_elements(&mut cursor, source, highlights);
    }

    /// Recursively visit nodes to find script/style elements
    fn visit_html_embedded_elements(
        &mut self,
        cursor: &mut TreeCursor,
        source: &str,
        highlights: &mut SyntaxHighlights,
    ) {
        loop {
            let node = cursor.node();

            match node.kind() {
                "script_element" => {
                    self.process_script_element(node, source, highlights);
                }
                "style_element" => {
                    self.process_style_element(node, source, highlights);
                }
                _ => {}
            }

            if cursor.goto_first_child() {
                self.visit_html_embedded_elements(cursor, source, highlights);
                cursor.goto_parent();
            }

            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }

    /// Process <script> element - inject JavaScript highlighting
    fn process_script_element(
        &mut self,
        node: tree_sitter::Node,
        source: &str,
        highlights: &mut SyntaxHighlights,
    ) {
        // Find raw_text child (the script content)
        let mut raw_text_node: Option<tree_sitter::Node> = None;

        let mut child_cursor = node.walk();
        if child_cursor.goto_first_child() {
            loop {
                let child = child_cursor.node();
                if child.kind() == "raw_text" {
                    raw_text_node = Some(child);
                    break;
                }
                if !child_cursor.goto_next_sibling() {
                    break;
                }
            }
        }

        let Some(raw_text) = raw_text_node else {
            return;
        };

        // Default to JavaScript (could check type attribute for TypeScript)
        self.inject_html_language_highlights(raw_text, source, LanguageId::JavaScript, highlights);
    }

    /// Process <style> element - inject CSS highlighting
    fn process_style_element(
        &mut self,
        node: tree_sitter::Node,
        source: &str,
        highlights: &mut SyntaxHighlights,
    ) {
        // Find raw_text child (the style content)
        let mut raw_text_node: Option<tree_sitter::Node> = None;

        let mut child_cursor = node.walk();
        if child_cursor.goto_first_child() {
            loop {
                let child = child_cursor.node();
                if child.kind() == "raw_text" {
                    raw_text_node = Some(child);
                    break;
                }
                if !child_cursor.goto_next_sibling() {
                    break;
                }
            }
        }

        let Some(raw_text) = raw_text_node else {
            return;
        };

        self.inject_html_language_highlights(raw_text, source, LanguageId::Css, highlights);
    }

    /// Specialized Vue SFC parsing - HTML structure with smart script language detection
    fn parse_and_highlight_vue(
        &mut self,
        source: &str,
        doc_id: DocumentId,
        revision: u64,
    ) -> SyntaxHighlights {
        let language = LanguageId::Vue;

        // Step 1: Parse with HTML grammar (Vue SFC is structurally valid HTML)
        let parser = match self.parsers.get_mut(&language) {
            Some(p) => p,
            None => {
                tracing::warn!("No parser for Vue");
                return SyntaxHighlights::new(language, revision);
            }
        };

        // Try incremental parsing if cached
        let tree = if let Some(cached) = self.doc_cache.get_mut(&doc_id) {
            if cached.language == language {
                if let Some(edit) = compute_incremental_edit(&cached.source, source) {
                    cached.tree.edit(&edit);
                    match parser.parse(source, Some(&cached.tree)) {
                        Some(new_tree) => {
                            cached.tree = new_tree.clone();
                            cached.source = source.to_owned();
                            new_tree
                        }
                        None => {
                            self.doc_cache.remove(&doc_id);
                            match parser.parse(source, None) {
                                Some(t) => {
                                    self.doc_cache.insert(
                                        doc_id,
                                        DocParseState {
                                            language,
                                            tree: t.clone(),
                                            source: source.to_owned(),
                                            highlights: None,
                                        },
                                    );
                                    t
                                }
                                None => return SyntaxHighlights::new(language, revision),
                            }
                        }
                    }
                } else {
                    cached.tree.clone()
                }
            } else {
                self.doc_cache.remove(&doc_id);
                match parser.parse(source, None) {
                    Some(t) => {
                        self.doc_cache.insert(
                            doc_id,
                            DocParseState {
                                language,
                                tree: t.clone(),
                                source: source.to_owned(),
                                highlights: None,
                            },
                        );
                        t
                    }
                    None => return SyntaxHighlights::new(language, revision),
                }
            }
        } else {
            match parser.parse(source, None) {
                Some(t) => {
                    self.doc_cache.insert(
                        doc_id,
                        DocParseState {
                            language,
                            tree: t.clone(),
                            source: source.to_owned(),
                            highlights: None,
                        },
                    );
                    t
                }
                None => return SyntaxHighlights::new(language, revision),
            }
        };

        // Step 2: Extract HTML-level highlights (Vue uses same query as HTML)
        let mut highlights = self.extract_highlights(source, &tree, language, revision, None);

        // Step 3: Language injection for script/style with component lang detection
        self.extract_component_embedded_highlights(source, &tree, &mut highlights);

        // Re-sort tokens after adding injected highlights
        finalize_line_tokens(&mut highlights);

        highlights
    }

    /// Extract highlights for embedded script/style content in component files.
    fn extract_component_embedded_highlights(
        &mut self,
        source: &str,
        tree: &Tree,
        highlights: &mut SyntaxHighlights,
    ) {
        let mut cursor = tree.walk();
        self.visit_component_embedded_elements(&mut cursor, source, highlights);
    }

    /// Recursively visit component nodes to find script/style elements.
    fn visit_component_embedded_elements(
        &mut self,
        cursor: &mut TreeCursor,
        source: &str,
        highlights: &mut SyntaxHighlights,
    ) {
        loop {
            let node = cursor.node();

            match node.kind() {
                "script_element" => {
                    self.process_component_script_element(node, source, highlights);
                }
                "style_element" => {
                    // Style injection is identical to HTML
                    self.process_style_element(node, source, highlights);
                }
                _ => {}
            }

            if cursor.goto_first_child() {
                self.visit_component_embedded_elements(cursor, source, highlights);
                cursor.goto_parent();
            }

            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }

    /// Process a component <script> element and honor its language attribute.
    fn process_component_script_element(
        &mut self,
        node: tree_sitter::Node,
        source: &str,
        highlights: &mut SyntaxHighlights,
    ) {
        let lang_id = detect_component_script_language(node, source);

        // Find raw_text child (the script content)
        let mut child_cursor = node.walk();
        if child_cursor.goto_first_child() {
            loop {
                let child = child_cursor.node();
                if child.kind() == "raw_text" {
                    self.inject_html_language_highlights(child, source, lang_id, highlights);
                    break;
                }
                if !child_cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    /// Inject language highlights for HTML embedded content
    fn inject_html_language_highlights(
        &mut self,
        raw_text: tree_sitter::Node,
        source: &str,
        lang_id: LanguageId,
        highlights: &mut SyntaxHighlights,
    ) {
        let start_byte = raw_text.start_byte();
        let end_byte = raw_text.end_byte();

        if start_byte >= end_byte || end_byte > source.len() {
            return;
        }

        let code_source = &source[start_byte..end_byte];
        let base_row = raw_text.start_position().row;
        let base_col = raw_text.start_position().column;

        self.ensure_language(lang_id);

        // Get parser for this language
        let Some(parser) = self.parsers.get_mut(&lang_id) else {
            return;
        };

        // Parse the embedded code
        let Some(code_tree) = parser.parse(code_source, None) else {
            return;
        };

        // Get query for this language
        let Some(query) = self.queries.get(&lang_id) else {
            return;
        };

        // Extract highlights
        let code_lines: Vec<&str> = code_source.lines().collect();
        let source_bytes = code_source.as_bytes();

        let mut cursor = QueryCursor::new();
        let mut captures = cursor.captures(query, code_tree.root_node(), source_bytes);

        while let Some((query_match, capture_idx)) = captures.next() {
            let capture = &query_match.captures[*capture_idx];
            let capture_name = &query.capture_names()[capture.index as usize];

            let highlight_id = match highlight_id_for_name(capture_name) {
                Some(id) => id,
                None => continue,
            };

            let cap_node = capture.node;
            let start = cap_node.start_position();
            let end = cap_node.end_position();

            // Handle single-line tokens
            if start.row == end.row {
                let local_line = code_lines.get(start.row).copied().unwrap_or("");
                let start_char = byte_to_char_col(local_line, start.column);
                let end_char = byte_to_char_col(local_line, end.column);

                let actual_row = base_row + start.row;
                let (actual_start_col, actual_end_col) = if start.row == 0 {
                    (base_col + start_char, base_col + end_char)
                } else {
                    (start_char, end_char)
                };

                if actual_start_col < actual_end_col {
                    let line_highlights = highlights.lines.entry(actual_row).or_default();
                    line_highlights.tokens.push(HighlightToken {
                        start_col: actual_start_col,
                        end_col: actual_end_col,
                        highlight: highlight_id,
                    });
                }
            } else {
                // Multi-line tokens
                for row in start.row..=end.row {
                    let local_line = code_lines.get(row).copied().unwrap_or("");
                    let line_char_len = local_line.chars().count();

                    let (start_char, end_char) = if row == start.row {
                        let sc = byte_to_char_col(local_line, start.column);
                        (sc, line_char_len)
                    } else if row == end.row {
                        let ec = byte_to_char_col(local_line, end.column);
                        (0, ec)
                    } else {
                        (0, line_char_len)
                    };

                    let actual_row = base_row + row;
                    let (actual_start_col, actual_end_col) = if row == 0 {
                        (base_col + start_char, base_col + end_char)
                    } else {
                        (start_char, end_char)
                    };

                    if actual_start_col < actual_end_col {
                        let line_highlights = highlights.lines.entry(actual_row).or_default();
                        line_highlights.tokens.push(HighlightToken {
                            start_col: actual_start_col,
                            end_col: actual_end_col,
                            highlight: highlight_id,
                        });
                    }
                }
            }
        }
    }
}

/// Detect the script language from a component `<script>` element's `lang` attribute.
/// Returns TypeScript for `lang="ts"` or `lang="typescript"`, TSX for `lang="tsx"`,
/// and JavaScript as default.
fn detect_component_script_language(script_node: tree_sitter::Node, source: &str) -> LanguageId {
    match component_lang_attribute(script_node, source) {
        Some(language) if language.eq_ignore_ascii_case("ts") => LanguageId::TypeScript,
        Some(language) if language.eq_ignore_ascii_case("typescript") => LanguageId::TypeScript,
        Some(language) if language.eq_ignore_ascii_case("tsx") => LanguageId::Tsx,
        _ => LanguageId::JavaScript,
    }
}

/// Detect the language from a component `<style>` element's `lang` attribute.
fn detect_component_style_language(style_node: tree_sitter::Node, source: &str) -> LanguageId {
    match component_lang_attribute(style_node, source) {
        Some(language) if language.eq_ignore_ascii_case("scss") => LanguageId::Scss,
        _ => LanguageId::Css,
    }
}

fn component_lang_attribute<'source>(
    element: tree_sitter::Node<'_>,
    source: &'source str,
) -> Option<&'source str> {
    let start_tag = element
        .named_children(&mut element.walk())
        .find(|child| child.kind() == "start_tag")?;

    start_tag
        .named_children(&mut start_tag.walk())
        .filter(|child| child.kind() == "attribute")
        .find_map(|attribute| {
            let mut name = None;
            let mut value = None;
            for part in attribute.named_children(&mut attribute.walk()) {
                match part.kind() {
                    "attribute_name" => name = part.utf8_text(source.as_bytes()).ok(),
                    "quoted_attribute_value" | "attribute_value" => {
                        value = part
                            .utf8_text(source.as_bytes())
                            .ok()
                            .map(|text| text.trim_matches(['"', '\'']))
                    }
                    _ => {}
                }
            }
            name.filter(|name| name.eq_ignore_ascii_case("lang"))
                .and(value)
        })
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
    fn languages_are_initialized_lazily() {
        let mut state = ParserState::new();
        assert!(state.parsers.is_empty());
        assert!(state.queries.is_empty());

        state.parse_and_highlight("fn main() {}", LanguageId::Rust, DocumentId(300), 1);

        assert_eq!(state.parsers.len(), 1);
        assert_eq!(state.queries.len(), 1);
        assert!(state.parsers.contains_key(&LanguageId::Rust));
        assert!(state.queries.contains_key(&LanguageId::Rust));
    }

    #[test]
    fn syntax_snapshot_contains_markdown_fence_tree() {
        let source = "```rust\nlet answer = make(user.name);\n```\n";
        let document = DocumentId(301);
        let mut state = ParserState::new();
        state.parse_and_highlight(source, LanguageId::Markdown, document, 1);

        let snapshot = state.syntax_tree_snapshot(document, 1).unwrap();

        assert_eq!(snapshot.injections().len(), 1);
        assert_eq!(snapshot.injections()[0].language, LanguageId::Rust);
        assert_eq!(
            &source[snapshot.injections()[0].range.clone()],
            "let answer = make(user.name);\n"
        );
    }

    #[test]
    fn syntax_snapshot_contains_astro_frontmatter_tree() {
        let source = "---\nconst name: string = 'world';\n---\n<h1>{name}</h1>\n";
        let document = DocumentId(302);
        let mut state = ParserState::new();
        state.parse_and_highlight(source, LanguageId::Astro, document, 1);

        let snapshot = state.syntax_tree_snapshot(document, 1).unwrap();
        assert!(snapshot.injections().iter().any(|injection| {
            injection.language == LanguageId::TypeScript
                && source[injection.range.clone()].contains("const name")
        }));
    }

    #[test]
    fn syntax_snapshot_contains_docker_shell_tree() {
        let source = "FROM alpine\nRUN echo hello && touch /tmp/ready\n";
        let document = DocumentId(303);
        let mut state = ParserState::new();
        state.parse_and_highlight(source, LanguageId::Dockerfile, document, 1);

        let snapshot = state.syntax_tree_snapshot(document, 1).unwrap();
        assert!(snapshot
            .injections()
            .iter()
            .any(|injection| injection.language == LanguageId::Bash));
    }

    #[test]
    fn syntax_snapshot_contains_tera_frontmatter_tree() {
        let source = "---\ntitle: Example\n---\n<h1>{{ title }}</h1>\n";
        let document = DocumentId(304);
        let mut state = ParserState::new();
        state.parse_and_highlight(source, LanguageId::Tera, document, 1);

        let snapshot = state.syntax_tree_snapshot(document, 1).unwrap();
        assert!(snapshot
            .injections()
            .iter()
            .any(|injection| injection.language == LanguageId::Yaml));
    }

    #[test]
    fn syntax_snapshot_contains_hurl_json_tree() {
        let source = "POST https://example.com/users\nContent-Type: application/json\n{\n  \"user\": {\"name\": \"Ada\"},\n  \"roles\": [\"admin\", \"editor\"]\n}\nHTTP 201\n";
        let document = DocumentId(305);
        let mut state = ParserState::new();
        state.parse_and_highlight(source, LanguageId::Hurl, document, 1);

        let snapshot = state.syntax_tree_snapshot(document, 1).unwrap();
        let json_injections: Vec<_> = snapshot
            .injections()
            .iter()
            .filter(|injection| injection.language == LanguageId::Json)
            .collect();
        assert_eq!(
            json_injections.len(),
            1,
            "only the outer JSON body is injected"
        );
        assert!(source[json_injections[0].range.clone()].starts_with('{'));
    }

    #[test]
    fn component_scss_style_uses_scss_injection() {
        for (index, language) in [LanguageId::Vue, LanguageId::Svelte, LanguageId::Astro]
            .into_iter()
            .enumerate()
        {
            let source = "<style lang=\"scss\">\n.card { &__title { color: red; } }\n</style>\n";
            let document = DocumentId(310 + index as u64);
            let mut state = ParserState::new();
            state.parse_and_highlight(source, language, document, 1);

            let snapshot = state
                .syntax_tree_snapshot(document, 1)
                .expect("component syntax tree should be available");
            assert!(
                snapshot
                    .injections()
                    .iter()
                    .any(|injection| injection.language == LanguageId::Scss),
                "{language:?} should inject SCSS for style lang=scss"
            );
        }
    }

    #[test]
    fn syntax_snapshot_contains_typst_raw_block_tree() {
        let source = "```rust\nfn answer() -> u8 { 42 }\n```\n";
        let document = DocumentId(306);
        let mut state = ParserState::new();
        state.parse_and_highlight(source, LanguageId::Typst, document, 1);

        let snapshot = state.syntax_tree_snapshot(document, 1).unwrap();
        assert!(snapshot
            .injections()
            .iter()
            .any(|injection| injection.language == LanguageId::Rust));
    }

    #[test]
    fn extended_language_samples_parse_and_highlight() {
        let samples = [
            (
                LanguageId::CSharp,
                include_str!("../../samples/syntax/sample.cs"),
            ),
            (
                LanguageId::Ruby,
                include_str!("../../samples/syntax/sample.rb"),
            ),
            (
                LanguageId::Lua,
                include_str!("../../samples/syntax/sample.lua"),
            ),
            (LanguageId::R, include_str!("../../samples/syntax/sample.r")),
            (
                LanguageId::Swift,
                include_str!("../../samples/syntax/sample.swift"),
            ),
            (
                LanguageId::Elixir,
                include_str!("../../samples/syntax/sample.ex"),
            ),
            (
                LanguageId::Gleam,
                include_str!("../../samples/syntax/sample.gleam"),
            ),
            (
                LanguageId::Solidity,
                include_str!("../../samples/syntax/sample.sol"),
            ),
            (
                LanguageId::Kotlin,
                include_str!("../../samples/syntax/sample.kt"),
            ),
            (
                LanguageId::Dart,
                include_str!("../../samples/syntax/sample.dart"),
            ),
            (
                LanguageId::Julia,
                include_str!("../../samples/syntax/sample.jl"),
            ),
            (
                LanguageId::Haskell,
                include_str!("../../samples/syntax/sample.hs"),
            ),
            (
                LanguageId::Ocaml,
                include_str!("../../samples/syntax/sample.ml"),
            ),
            (LanguageId::D, include_str!("../../samples/syntax/sample.d")),
            (
                LanguageId::ObjectiveC,
                include_str!("../../samples/syntax/sample.m"),
            ),
            (
                LanguageId::Vhdl,
                include_str!("../../samples/syntax/sample.vhdl"),
            ),
            (
                LanguageId::Odin,
                include_str!("../../samples/syntax/sample.odin"),
            ),
            (
                LanguageId::Fish,
                include_str!("../../samples/syntax/sample.fish"),
            ),
            (
                LanguageId::Assembly,
                include_str!("../../samples/syntax/sample.asm"),
            ),
            (
                LanguageId::Scss,
                include_str!("../../samples/syntax/sample.scss"),
            ),
            (
                LanguageId::Cmake,
                include_str!("../../samples/syntax/CMakeLists.txt"),
            ),
            (
                LanguageId::Make,
                include_str!("../../samples/syntax/Makefile"),
            ),
            (
                LanguageId::CommonLisp,
                include_str!("../../samples/syntax/sample.lisp"),
            ),
            (LanguageId::OcamlInterface, "val greet : string -> string\n"),
            (
                LanguageId::Zig,
                include_str!("../../samples/syntax/sample.zig"),
            ),
            (
                LanguageId::Glsl,
                include_str!("../../samples/syntax/sample.glsl"),
            ),
            (
                LanguageId::Graphql,
                include_str!("../../samples/syntax/sample.graphql"),
            ),
            (
                LanguageId::Hcl,
                include_str!("../../samples/syntax/sample.tf"),
            ),
            (
                LanguageId::Nix,
                include_str!("../../samples/syntax/sample.nix"),
            ),
            (
                LanguageId::Ada,
                include_str!("../../samples/syntax/sample.adb"),
            ),
            (
                LanguageId::Erlang,
                include_str!("../../samples/syntax/sample.erl"),
            ),
            (
                LanguageId::Clojure,
                include_str!("../../samples/syntax/sample.clj"),
            ),
            (
                LanguageId::Nushell,
                include_str!("../../samples/syntax/sample.nu"),
            ),
            (
                LanguageId::Sed,
                include_str!("../../samples/syntax/sample.sed"),
            ),
            (
                LanguageId::Tcl,
                include_str!("../../samples/syntax/sample.tcl"),
            ),
            (
                LanguageId::Roc,
                include_str!("../../samples/syntax/sample.roc"),
            ),
            (
                LanguageId::Janet,
                include_str!("../../samples/syntax/sample.janet"),
            ),
            (
                LanguageId::Forth,
                include_str!("../../samples/syntax/sample.forth"),
            ),
            (
                LanguageId::Protobuf,
                include_str!("../../samples/syntax/sample.proto"),
            ),
            (
                LanguageId::Dhall,
                include_str!("../../samples/syntax/sample.dhall"),
            ),
            (
                LanguageId::Pkl,
                include_str!("../../samples/syntax/sample.pkl"),
            ),
            (
                LanguageId::Hurl,
                include_str!("../../samples/syntax/sample.hurl"),
            ),
            (
                LanguageId::Wit,
                include_str!("../../samples/syntax/sample.wit"),
            ),
            (
                LanguageId::StandardMl,
                include_str!("../../samples/syntax/sample.sml"),
            ),
            (
                LanguageId::Nim,
                include_str!("../../samples/syntax/sample.nim"),
            ),
            (
                LanguageId::Astro,
                include_str!("../../samples/syntax/sample.astro"),
            ),
            (
                LanguageId::Awk,
                include_str!("../../samples/syntax/sample.awk"),
            ),
            (
                LanguageId::Kdl,
                include_str!("../../samples/syntax/sample.kdl"),
            ),
            (
                LanguageId::Tera,
                include_str!("../../samples/syntax/sample.tera"),
            ),
            (
                LanguageId::Typst,
                include_str!("../../samples/syntax/sample.typ"),
            ),
            (
                LanguageId::Dockerfile,
                include_str!("../../samples/syntax/sample.Dockerfile"),
            ),
            (
                LanguageId::Wgsl,
                include_str!("../../samples/syntax/sample.wgsl"),
            ),
            (
                LanguageId::Sql,
                include_str!("../../samples/syntax/sample.sql"),
            ),
            (LanguageId::V, include_str!("../../samples/syntax/sample.v")),
            (
                LanguageId::Cue,
                include_str!("../../samples/syntax/sample.cue"),
            ),
            (
                LanguageId::Fennel,
                include_str!("../../samples/syntax/sample.fnl"),
            ),
            (
                LanguageId::Pest,
                include_str!("../../samples/syntax/sample.pest"),
            ),
            (
                LanguageId::Pony,
                include_str!("../../samples/syntax/sample.pony"),
            ),
        ];
        let mut state = ParserState::new();

        for (index, (language, source)) in samples.into_iter().enumerate() {
            let document = DocumentId(400 + index as u64);
            let highlights = state.parse_and_highlight(source, language, document, 1);
            assert!(
                !highlights.lines.is_empty(),
                "{language:?} produced no highlights"
            );
            assert!(
                state.get_cached_tree(document).is_some(),
                "{language:?} produced no tree"
            );
        }
    }

    #[test]
    fn test_all_query_files_compile() {
        // This test ensures all .scm query files are valid and compile without errors
        let mut state = ParserState::new();

        for definition in crate::syntax::registry::ALL_LANGUAGES {
            let lang = definition.id;
            if lang == LanguageId::PlainText {
                continue;
            }
            state.ensure_language(lang);
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
        fn test_kotlin_query_compiles() {
            assert_query_compiles(
                "kotlin",
                tree_sitter_kotlin_ng::LANGUAGE.into(),
                KOTLIN_HIGHLIGHTS,
            );
        }

        #[test]
        fn test_ocaml_interface_query_compiles() {
            assert_query_compiles(
                "ocaml-interface",
                tree_sitter_ocaml::LANGUAGE_OCAML_INTERFACE.into(),
                OCAML_INTERFACE_HIGHLIGHTS,
            );
        }

        #[test]
        fn test_commonlisp_query_compiles() {
            assert_query_compiles(
                "commonlisp",
                tree_sitter_commonlisp::LANGUAGE_COMMONLISP.into(),
                COMMONLISP_HIGHLIGHTS,
            );
        }

        #[test]
        fn test_hurl_query_compiles() {
            assert_query_compiles("hurl", tree_sitter_hurl::LANGUAGE.into(), HURL_HIGHLIGHTS);
        }

        #[test]
        fn test_wgsl_query_compiles() {
            assert_query_compiles(
                "wgsl",
                tree_sitter_wgsl_bevy::LANGUAGE.into(),
                WGSL_HIGHLIGHTS,
            );
        }

        #[test]
        fn test_v_query_compiles() {
            assert_query_compiles("v", tree_sitter_v::LANGUAGE.into(), V_HIGHLIGHTS);
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
                TSX_HIGHLIGHTS,
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

        #[test]
        fn test_blade_query_compiles() {
            assert_query_compiles(
                "Blade",
                tree_sitter_blade::LANGUAGE.into(),
                BLADE_HIGHLIGHTS,
            );
        }

        #[test]
        fn test_just_query_compiles() {
            assert_query_compiles("Just", tree_sitter_just::LANGUAGE.into(), JUST_HIGHLIGHTS);
        }

        #[test]
        fn test_sema_query_compiles() {
            assert_query_compiles("Sema", tree_sitter_sema::LANGUAGE.into(), SEMA_HIGHLIGHTS);
        }
    }

    /// Guard the last-match-wins capture precedence: generic fallbacks come
    /// first in the query files, specific overrides last, and the override
    /// must win for identical spans (see `finalize_line_tokens`).
    #[test]
    fn test_capture_precedence_last_match_wins() {
        let mut state = ParserState::new();

        let source = "(define (factorial n) 1)\n";
        let highlights = state.parse_and_highlight(source, LanguageId::Sema, DocumentId(60), 1);
        let line = &highlights.lines[&0];

        let name_at = |col: usize| {
            line.highlight_at(col)
                .map(|id| crate::syntax::highlights::HIGHLIGHT_NAMES[id as usize])
                .unwrap_or("<none>")
        };
        // `define` is a special form, not the generic `(symbol) @variable`
        assert_eq!(name_at(1), "keyword");
        // `factorial` is the defined function name
        assert_eq!(name_at(9), "function");
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
    fn test_sema_parsing() {
        let mut state = ParserState::new();
        let source = r#";; hello.sema — Basic Sema demo

(define (factorial n)
  (if (<= n 1) 1 (* n (factorial (- n 1)))))

(display "Factorial of 10: ")
(println (factorial 10))

(define numbers (range 1 11))
(define squares (map (lambda (x) (* x x)) numbers))
(println squares)

(let ((x 10)
      (y 20))
  (println (+ x y)))
"#;
        let doc_id = DocumentId(52);
        let highlights = state.parse_and_highlight(source, LanguageId::Sema, doc_id, 1);

        assert_eq!(highlights.language, LanguageId::Sema);
        assert!(!highlights.lines.is_empty());
    }

    #[test]
    fn test_sema_workflow_and_alias_highlights() {
        let mut state = ParserState::new();
        let source = "(def answer 42)\n(defn run (x) (workflow/run x))\n(defworkflow deploy (step \"build\"))\n(defpolicy safe (approval \"ship\"))\n";
        let highlights = state.parse_and_highlight(source, LanguageId::Sema, DocumentId(53), 1);

        assert_eq!(highlights.language, LanguageId::Sema);
        assert!(highlights.lines.keys().all(|line| *line < 4));
        assert_eq!(highlights.lines.len(), 4);
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
    fn test_just_parsing() {
        let mut state = ParserState::new();
        let source = r#"set shell := ["bash", "-cu"]

build target="release":
    cargo build --{{target}}

test:
    cargo test
"#;
        let doc_id = DocumentId(53);
        let highlights = state.parse_and_highlight(source, LanguageId::Just, doc_id, 1);

        assert_eq!(highlights.language, LanguageId::Just);
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
    fn incremental_highlighting_returns_full_state_and_bounded_patch() {
        let source = (0..100)
            .map(|line| format!("fn function_{line}() {{}}\n"))
            .collect::<String>();
        let mut state = ParserState::new();
        let doc_id = DocumentId(110);
        let initial = state.parse_and_highlight(&source, LanguageId::Rust, doc_id, 1);
        assert!(initial.lines.contains_key(&0));
        assert!(initial.lines.contains_key(&99));

        let edited = source.replacen("function_50", "renamed_50", 1);
        let updated = state.parse_and_highlight(&edited, LanguageId::Rust, doc_id, 2);
        let ranges = state
            .last_changed_line_ranges()
            .expect("incremental parse should report changed lines");
        let patch = state
            .take_last_highlight_patch()
            .expect("incremental parse should retain a bounded patch");

        assert!(updated.lines.contains_key(&0));
        assert!(updated.lines.contains_key(&99));
        assert!(ranges.iter().all(|range| range.end - range.start <= 3));
        assert!(patch
            .lines
            .keys()
            .all(|line| ranges.iter().any(|range| range.contains(line))));
    }

    #[test]
    fn incremental_patch_matches_full_parse_inside_multiline_capture() {
        let source = format!("fn main() {{\n/*\n{}*/\n}}\n", "comment line\n".repeat(100));
        let edited = source.replacen(
            "comment line\ncomment line",
            "changed line\ncomment line",
            1,
        );
        let doc_id = DocumentId(111);
        let mut incremental = ParserState::new();
        incremental.parse_and_highlight(&source, LanguageId::Rust, doc_id, 1);
        let updated = incremental.parse_and_highlight(&edited, LanguageId::Rust, doc_id, 2);

        let mut fresh = ParserState::new();
        let expected = fresh.parse_and_highlight(&edited, LanguageId::Rust, DocumentId(112), 2);
        assert_eq!(updated.lines, expected.lines);
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

    // Markdown inline highlighting tests

    #[test]
    fn test_markdown_inline_query_compiles() {
        // Verify inline query compiles successfully
        let ts_lang: tree_sitter::Language = tree_sitter_md::INLINE_LANGUAGE.into();
        let query = Query::new(&ts_lang, MARKDOWN_INLINE_HIGHLIGHTS);
        assert!(
            query.is_ok(),
            "Markdown inline query failed to compile: {:?}",
            query.err()
        );
    }

    #[test]
    fn test_markdown_inline_emphasis() {
        let mut state = ParserState::new();
        let source = "This is *italic* text";
        let doc_id = DocumentId(100);
        let highlights = state.parse_and_highlight(source, LanguageId::Markdown, doc_id, 1);

        assert_eq!(highlights.language, LanguageId::Markdown);
        // Should have highlights for inline content
        let line0 = highlights.lines.get(&0);
        assert!(line0.is_some(), "Line 0 should have highlights");

        // Check that we have a text.emphasis highlight
        let line = line0.unwrap();
        let has_emphasis = line.tokens.iter().any(|t| {
            let name = super::super::highlights::HIGHLIGHT_NAMES
                .get(t.highlight as usize)
                .unwrap_or(&"");
            *name == "text.emphasis"
        });
        assert!(
            has_emphasis,
            "Should have text.emphasis highlight for *italic*, found: {:?}",
            line.tokens
        );
    }

    #[test]
    fn test_markdown_inline_strong() {
        let mut state = ParserState::new();
        let source = "This is **bold** text";
        let doc_id = DocumentId(101);
        let highlights = state.parse_and_highlight(source, LanguageId::Markdown, doc_id, 1);

        assert_eq!(highlights.language, LanguageId::Markdown);
        let line0 = highlights.lines.get(&0);
        assert!(line0.is_some(), "Line 0 should have highlights");

        let line = line0.unwrap();
        let has_strong = line.tokens.iter().any(|t| {
            let name = super::super::highlights::HIGHLIGHT_NAMES
                .get(t.highlight as usize)
                .unwrap_or(&"");
            *name == "text.strong"
        });
        assert!(
            has_strong,
            "Should have text.strong highlight for **bold**, found: {:?}",
            line.tokens
        );
    }

    #[test]
    fn test_markdown_code_span() {
        let mut state = ParserState::new();
        let source = "Use `code` here";
        let doc_id = DocumentId(102);
        let highlights = state.parse_and_highlight(source, LanguageId::Markdown, doc_id, 1);

        assert_eq!(highlights.language, LanguageId::Markdown);
        let line0 = highlights.lines.get(&0);
        assert!(line0.is_some(), "Line 0 should have highlights");

        let line = line0.unwrap();
        let has_code = line.tokens.iter().any(|t| {
            let name = super::super::highlights::HIGHLIGHT_NAMES
                .get(t.highlight as usize)
                .unwrap_or(&"");
            *name == "string" // code_span maps to @string
        });
        assert!(
            has_code,
            "Should have string highlight for `code`, found: {:?}",
            line.tokens
        );
    }

    #[test]
    fn test_markdown_fenced_code_injection() {
        let mut state = ParserState::new();
        let source = r#"# Code Example

```rust
fn main() {
    println!("Hello");
}
```
"#;
        let doc_id = DocumentId(103);
        let highlights = state.parse_and_highlight(source, LanguageId::Markdown, doc_id, 1);

        assert_eq!(highlights.language, LanguageId::Markdown);

        // Line 3 should have "fn" highlighted as keyword (rust code inside code block)
        // Note: Line 0 = "# Code Example", Line 1 = "", Line 2 = "```rust", Line 3 = "fn main..."
        let line3 = highlights.lines.get(&3);
        assert!(line3.is_some(), "Line 3 (fn main) should have highlights");

        let line = line3.unwrap();
        let has_keyword = line.tokens.iter().any(|t| {
            let name = super::super::highlights::HIGHLIGHT_NAMES
                .get(t.highlight as usize)
                .unwrap_or(&"");
            name.starts_with("keyword")
        });
        assert!(
            has_keyword,
            "Should have keyword highlight for 'fn' in rust code block, found: {:?}",
            line.tokens
        );
    }

    #[test]
    fn test_markdown_full_document() {
        let mut state = ParserState::new();
        let source = r#"# Heading

This paragraph has *italic*, **bold**, and `code`.

- List item with [link](url)

```python
def hello():
    print("Hello")
```
"#;
        let doc_id = DocumentId(104);
        let highlights = state.parse_and_highlight(source, LanguageId::Markdown, doc_id, 1);

        // Should have highlights for heading
        assert!(
            highlights.lines.contains_key(&0),
            "Should highlight heading"
        );
        // Should have highlights for paragraph with inline elements
        assert!(
            highlights.lines.contains_key(&2),
            "Should highlight paragraph with inline elements"
        );
        // Should have highlights for code block
        assert!(
            highlights.lines.contains_key(&7),
            "Should highlight python code block"
        );
    }

    #[test]
    fn test_markdown_debug_output() {
        let mut state = ParserState::new();
        let source = r#"# Heading

This has *italic* and **bold** and `code`.

```rust
fn main() {}
```
"#;
        let doc_id = DocumentId(200);
        let highlights = state.parse_and_highlight(source, LanguageId::Markdown, doc_id, 1);

        // Verify specific highlights exist
        // Line 0: # Heading - should have punctuation.special and text.title
        let line0 = highlights
            .lines
            .get(&0)
            .expect("Line 0 should have highlights");
        assert!(
            line0.tokens.iter().any(|t| {
                super::super::highlights::HIGHLIGHT_NAMES.get(t.highlight as usize)
                    == Some(&"punctuation.special")
            }),
            "Line 0 should have punctuation.special for #"
        );
        assert!(
            line0.tokens.iter().any(|t| {
                super::super::highlights::HIGHLIGHT_NAMES.get(t.highlight as usize)
                    == Some(&"text.title")
            }),
            "Line 0 should have text.title for Heading"
        );

        // Line 2: inline elements
        let line2 = highlights
            .lines
            .get(&2)
            .expect("Line 2 should have highlights");
        let highlight_names: Vec<&str> = line2
            .tokens
            .iter()
            .filter_map(|t| {
                super::super::highlights::HIGHLIGHT_NAMES
                    .get(t.highlight as usize)
                    .copied()
            })
            .collect();

        assert!(
            highlight_names.contains(&"text.emphasis"),
            "Line 2 should have text.emphasis, found: {:?}",
            highlight_names
        );
        assert!(
            highlight_names.contains(&"text.strong"),
            "Line 2 should have text.strong, found: {:?}",
            highlight_names
        );
        assert!(
            highlight_names.contains(&"string"),
            "Line 2 should have string for code, found: {:?}",
            highlight_names
        );

        // Line 5: rust code block content should have keyword (line 4 is ```rust)
        let line5 = highlights
            .lines
            .get(&5)
            .expect("Line 5 (fn main) should have highlights");
        let has_keyword = line5.tokens.iter().any(|t| {
            let name = super::super::highlights::HIGHLIGHT_NAMES
                .get(t.highlight as usize)
                .unwrap_or(&"");
            name.starts_with("keyword")
        });
        assert!(
            has_keyword,
            "Line 5 should have keyword for 'fn', got: {:?}",
            line5
                .tokens
                .iter()
                .filter_map(|t| super::super::highlights::HIGHLIGHT_NAMES.get(t.highlight as usize))
                .collect::<Vec<_>>()
        );
    }

    // HTML language injection tests

    #[test]
    fn test_html_script_injection() {
        let mut state = ParserState::new();
        let source = r#"<!DOCTYPE html>
<html>
<head>
<script>
function hello() {
    const x = 42;
    return x;
}
</script>
</head>
</html>"#;
        let doc_id = DocumentId(300);
        let highlights = state.parse_and_highlight(source, LanguageId::Html, doc_id, 1);

        assert_eq!(highlights.language, LanguageId::Html);

        // Line 4 should have "function" as keyword.function
        let line4 = highlights.lines.get(&4);
        assert!(
            line4.is_some(),
            "Line 4 (function hello) should have highlights"
        );

        let line = line4.unwrap();
        let has_keyword = line.tokens.iter().any(|t| {
            let name = super::super::highlights::HIGHLIGHT_NAMES
                .get(t.highlight as usize)
                .unwrap_or(&"");
            name.starts_with("keyword")
        });
        assert!(
            has_keyword,
            "Should have keyword highlight for 'function', got: {:?}",
            line.tokens
                .iter()
                .filter_map(|t| super::super::highlights::HIGHLIGHT_NAMES.get(t.highlight as usize))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_html_style_injection() {
        let mut state = ParserState::new();
        let source = r#"<!DOCTYPE html>
<html>
<head>
<style>
.container {
    color: red;
    font-size: 16px;
}
</style>
</head>
</html>"#;
        let doc_id = DocumentId(301);
        let highlights = state.parse_and_highlight(source, LanguageId::Html, doc_id, 1);

        assert_eq!(highlights.language, LanguageId::Html);

        // Line 4 should have ".container" class highlighted as @type
        let line4 = highlights.lines.get(&4);
        assert!(
            line4.is_some(),
            "Line 4 (.container) should have highlights"
        );

        let line = line4.unwrap();
        let has_type = line.tokens.iter().any(|t| {
            let name = super::super::highlights::HIGHLIGHT_NAMES
                .get(t.highlight as usize)
                .unwrap_or(&"");
            *name == "type"
        });
        assert!(
            has_type,
            "Should have type highlight for class selector, got: {:?}",
            line.tokens
                .iter()
                .filter_map(|t| super::super::highlights::HIGHLIGHT_NAMES.get(t.highlight as usize))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_vue_sfc_highlighting() {
        let mut state = ParserState::new();
        let source = r#"<template>
  <div>{{ message }}</div>
</template>

<script lang="ts">
export default {
  data() {
    return { message: "Hello" }
  }
}
</script>

<style>
.container {
  color: red;
}
</style>"#;
        let doc_id = DocumentId(400);
        let highlights = state.parse_and_highlight(source, LanguageId::Vue, doc_id, 1);

        assert_eq!(highlights.language, LanguageId::Vue);
        // Should have HTML highlights in template
        assert!(
            highlights.lines.contains_key(&1),
            "Should highlight template div"
        );
        // Should have TypeScript highlights in script (lang="ts")
        assert!(
            highlights.lines.contains_key(&5),
            "Should highlight export default"
        );
        // Should have CSS highlights in style
        assert!(
            highlights.lines.contains_key(&15),
            "Should highlight .container"
        );
    }

    #[test]
    fn test_vue_sfc_javascript_default() {
        let mut state = ParserState::new();
        let source = r#"<script>
export default {
  data() {
    return {}
  }
}
</script>"#;
        let doc_id = DocumentId(401);
        let highlights = state.parse_and_highlight(source, LanguageId::Vue, doc_id, 1);

        assert_eq!(highlights.language, LanguageId::Vue);
        // Should have JavaScript highlights in script (default, no lang attr)
        assert!(
            highlights.lines.contains_key(&1),
            "Should highlight export default"
        );
    }

    #[test]
    fn test_svelte_highlighting_and_injection() {
        let mut state = ParserState::new();
        let source = r#"<script lang="ts">
let count: number = 1;
</script>

{#if count > 0}
  <p>{count}</p>
{/if}

<style>
p { color: red; }
</style>"#;
        let doc_id = DocumentId(402);
        let highlights = state.parse_and_highlight(source, LanguageId::Svelte, doc_id, 1);

        assert_eq!(highlights.language, LanguageId::Svelte);
        assert!(
            highlights.lines.contains_key(&1),
            "TypeScript should be highlighted"
        );
        assert!(
            highlights.lines.contains_key(&4),
            "Svelte blocks should be highlighted"
        );
        assert!(
            highlights.lines.contains_key(&9),
            "CSS should be highlighted"
        );
        let tree = state
            .get_cached_tree(doc_id)
            .expect("Svelte should retain its outer syntax tree");
        assert!(!tree.0.root_node().has_error());

        let edited = source.replace("count: number = 1", "count: number = 2");
        let updated = state.parse_and_highlight(&edited, LanguageId::Svelte, doc_id, 2);
        assert!(
            updated.lines.contains_key(&4),
            "unchanged Svelte tokens should remain"
        );
        assert!(
            updated.lines.contains_key(&9),
            "unchanged CSS tokens should remain"
        );
        for line in updated.lines.values() {
            let mut spans = line
                .tokens
                .iter()
                .map(|token| (token.start_col, token.end_col, token.highlight))
                .collect::<Vec<_>>();
            spans.sort_unstable();
            spans.dedup();
            assert_eq!(
                spans.len(),
                line.tokens.len(),
                "injected tokens must not duplicate"
            );
        }
    }

    #[test]
    fn test_html_full_document_with_injection() {
        let mut state = ParserState::new();
        let source = r#"<!DOCTYPE html>
<html>
<head>
    <style>
        body { margin: 0; }
    </style>
    <script>
        console.log("Hello");
    </script>
</head>
<body>
    <h1>Title</h1>
</body>
</html>"#;
        let doc_id = DocumentId(302);
        let highlights = state.parse_and_highlight(source, LanguageId::Html, doc_id, 1);

        // Should have highlights for HTML tags
        assert!(highlights.lines.contains_key(&1), "Should highlight <html>");
        // Should have highlights for CSS inside style
        assert!(
            highlights.lines.contains_key(&4),
            "Should highlight CSS body selector"
        );
        // Should have highlights for JavaScript inside script
        assert!(
            highlights.lines.contains_key(&7),
            "Should highlight JS console.log"
        );
    }
}
