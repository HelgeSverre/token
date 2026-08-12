//! Outline extraction from tree-sitter parse trees
//!
//! Walks the tree-sitter AST to extract structural symbols.
//! Runs on the syntax worker thread.

use tree_sitter::{Node, Tree};

use super::{OutlineData, OutlineKind, OutlineNode, OutlineRange};
use crate::syntax::LanguageId;

pub(crate) trait OutlineBehavior: Sync {
    fn extract(&self, root: Node<'_>, source: &str) -> Vec<OutlineNode>;
}

struct EmptyOutline;

impl OutlineBehavior for EmptyOutline {
    fn extract(&self, _root: Node<'_>, _source: &str) -> Vec<OutlineNode> {
        Vec::new()
    }
}

pub(crate) static NO_OUTLINE: &dyn OutlineBehavior = &EmptyOutline;

/// Extract outline from a tree-sitter parse tree
pub fn extract_outline(
    tree: &Tree,
    source: &str,
    language: LanguageId,
    revision: u64,
) -> OutlineData {
    let root = tree.root_node();

    let nodes = crate::syntax::registry::language(language)
        .outline
        .extract(root, source);

    OutlineData {
        revision,
        roots: nodes,
    }
}

struct FlatOutlineBehavior {
    extract_flat: for<'tree> fn(Node<'tree>, &str) -> Vec<FlatSymbol>,
}

impl OutlineBehavior for FlatOutlineBehavior {
    fn extract(&self, root: Node<'_>, source: &str) -> Vec<OutlineNode> {
        build_tree_by_containment((self.extract_flat)(root, source))
    }
}

struct MarkdownOutline;

impl OutlineBehavior for MarkdownOutline {
    fn extract(&self, root: Node<'_>, source: &str) -> Vec<OutlineNode> {
        extract_markdown_headings(root, source)
    }
}

macro_rules! flat_outline {
    ($static_name:ident, $impl_name:ident, $extractor:expr) => {
        static $impl_name: FlatOutlineBehavior = FlatOutlineBehavior {
            extract_flat: $extractor,
        };
        pub(crate) static $static_name: &dyn OutlineBehavior = &$impl_name;
    };
}

static MARKDOWN_OUTLINE_IMPL: MarkdownOutline = MarkdownOutline;
pub(crate) static MARKDOWN_OUTLINE: &dyn OutlineBehavior = &MARKDOWN_OUTLINE_IMPL;

flat_outline!(RUST_OUTLINE, RUST_OUTLINE_IMPL, extract_rust_symbols);
flat_outline!(
    JAVASCRIPT_OUTLINE,
    JAVASCRIPT_OUTLINE_IMPL,
    extract_js_ts_symbols
);
flat_outline!(PYTHON_OUTLINE, PYTHON_OUTLINE_IMPL, extract_python_symbols);
flat_outline!(GO_OUTLINE, GO_OUTLINE_IMPL, extract_go_symbols);
flat_outline!(JAVA_OUTLINE, JAVA_OUTLINE_IMPL, extract_java_symbols);
flat_outline!(PHP_OUTLINE, PHP_OUTLINE_IMPL, extract_php_symbols);
flat_outline!(YAML_OUTLINE, YAML_OUTLINE_IMPL, extract_yaml_symbols);
flat_outline!(HTML_OUTLINE, HTML_OUTLINE_IMPL, extract_html_symbols);
flat_outline!(BLADE_OUTLINE, BLADE_OUTLINE_IMPL, extract_blade_symbols);
flat_outline!(
    COMPONENT_OUTLINE,
    COMPONENT_OUTLINE_IMPL,
    extract_vue_symbols
);
flat_outline!(
    APPLESCRIPT_OUTLINE,
    APPLESCRIPT_OUTLINE_IMPL,
    extract_applescript_symbols
);
flat_outline!(R_OUTLINE, R_OUTLINE_IMPL, extract_r_symbols);
flat_outline!(ELIXIR_OUTLINE, ELIXIR_OUTLINE_IMPL, extract_elixir_symbols);

fn extract_c_symbols(root: Node<'_>, source: &str) -> Vec<FlatSymbol> {
    extract_c_cpp_symbols(root, source, LanguageId::C)
}

fn extract_cpp_symbols(root: Node<'_>, source: &str) -> Vec<FlatSymbol> {
    extract_c_cpp_symbols(root, source, LanguageId::Cpp)
}

flat_outline!(C_OUTLINE, C_OUTLINE_IMPL, extract_c_symbols);
flat_outline!(CPP_OUTLINE, CPP_OUTLINE_IMPL, extract_cpp_symbols);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct OutlineRule {
    pub node_kind: &'static str,
    pub symbol_kind: OutlineKind,
    pub name: OutlineName,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OutlineName {
    Field(&'static str),
    DescendantKind(&'static str),
}

pub(crate) struct RuleOutline {
    rules: &'static [OutlineRule],
}

impl RuleOutline {
    pub(crate) const fn new(rules: &'static [OutlineRule]) -> Self {
        Self { rules }
    }
}

impl OutlineBehavior for RuleOutline {
    fn extract(&self, root: Node<'_>, source: &str) -> Vec<OutlineNode> {
        build_tree_by_containment(extract_rule_symbols(root, source, self.rules))
    }
}

macro_rules! rule_outline {
    ($name:ident, [$($rule:expr),+ $(,)?]) => {
        pub(crate) static $name: RuleOutline = RuleOutline::new(&[$($rule),+]);
    };
}

macro_rules! rule_outline_behavior {
    ($name:ident, $implementation:ident, [$($rule:expr),+ $(,)?]) => {
        static $implementation: RuleOutline = RuleOutline::new(&[$($rule),+]);
        pub(crate) static $name: &dyn OutlineBehavior = &$implementation;
    };
}

rule_outline_behavior!(
    CSHARP_OUTLINE,
    CSHARP_OUTLINE_IMPL,
    [
        OutlineRule {
            node_kind: "class_declaration",
            symbol_kind: OutlineKind::Class,
            name: OutlineName::Field("name")
        },
        OutlineRule {
            node_kind: "struct_declaration",
            symbol_kind: OutlineKind::Struct,
            name: OutlineName::Field("name")
        },
        OutlineRule {
            node_kind: "enum_declaration",
            symbol_kind: OutlineKind::Enum,
            name: OutlineName::Field("name")
        },
        OutlineRule {
            node_kind: "interface_declaration",
            symbol_kind: OutlineKind::Interface,
            name: OutlineName::Field("name")
        },
        OutlineRule {
            node_kind: "method_declaration",
            symbol_kind: OutlineKind::Method,
            name: OutlineName::Field("name")
        },
    ]
);
rule_outline_behavior!(
    RUBY_OUTLINE,
    RUBY_OUTLINE_IMPL,
    [
        OutlineRule {
            node_kind: "class",
            symbol_kind: OutlineKind::Class,
            name: OutlineName::Field("name")
        },
        OutlineRule {
            node_kind: "module",
            symbol_kind: OutlineKind::Module,
            name: OutlineName::Field("name")
        },
        OutlineRule {
            node_kind: "method",
            symbol_kind: OutlineKind::Method,
            name: OutlineName::Field("name")
        },
        OutlineRule {
            node_kind: "singleton_method",
            symbol_kind: OutlineKind::Method,
            name: OutlineName::Field("name")
        },
    ]
);
rule_outline_behavior!(
    LUA_OUTLINE,
    LUA_OUTLINE_IMPL,
    [OutlineRule {
        node_kind: "function_declaration",
        symbol_kind: OutlineKind::Function,
        name: OutlineName::Field("name")
    },]
);
rule_outline_behavior!(
    SWIFT_OUTLINE,
    SWIFT_OUTLINE_IMPL,
    [
        OutlineRule {
            node_kind: "class_declaration",
            symbol_kind: OutlineKind::Class,
            name: OutlineName::Field("name")
        },
        OutlineRule {
            node_kind: "protocol_declaration",
            symbol_kind: OutlineKind::Interface,
            name: OutlineName::Field("name")
        },
        OutlineRule {
            node_kind: "function_declaration",
            symbol_kind: OutlineKind::Function,
            name: OutlineName::Field("name")
        },
    ]
);
rule_outline_behavior!(
    GLEAM_OUTLINE,
    GLEAM_OUTLINE_IMPL,
    [OutlineRule {
        node_kind: "function",
        symbol_kind: OutlineKind::Function,
        name: OutlineName::Field("name")
    },]
);
rule_outline_behavior!(
    SOLIDITY_OUTLINE,
    SOLIDITY_OUTLINE_IMPL,
    [
        OutlineRule {
            node_kind: "contract_declaration",
            symbol_kind: OutlineKind::Class,
            name: OutlineName::Field("name")
        },
        OutlineRule {
            node_kind: "interface_declaration",
            symbol_kind: OutlineKind::Interface,
            name: OutlineName::Field("name")
        },
        OutlineRule {
            node_kind: "struct_declaration",
            symbol_kind: OutlineKind::Struct,
            name: OutlineName::Field("name")
        },
        OutlineRule {
            node_kind: "enum_declaration",
            symbol_kind: OutlineKind::Enum,
            name: OutlineName::Field("name")
        },
        OutlineRule {
            node_kind: "function_definition",
            symbol_kind: OutlineKind::Function,
            name: OutlineName::Field("name")
        },
    ]
);

rule_outline!(
    KOTLIN_RULE_OUTLINE,
    [
        OutlineRule {
            node_kind: "class_declaration",
            symbol_kind: OutlineKind::Class,
            name: OutlineName::Field("name")
        },
        OutlineRule {
            node_kind: "function_declaration",
            symbol_kind: OutlineKind::Function,
            name: OutlineName::Field("name")
        },
    ]
);
rule_outline!(
    DART_RULE_OUTLINE,
    [
        OutlineRule {
            node_kind: "class_declaration",
            symbol_kind: OutlineKind::Class,
            name: OutlineName::Field("name")
        },
        OutlineRule {
            node_kind: "function_signature",
            symbol_kind: OutlineKind::Function,
            name: OutlineName::DescendantKind("identifier")
        },
    ]
);
rule_outline!(
    VHDL_RULE_OUTLINE,
    [OutlineRule {
        node_kind: "entity_declaration",
        symbol_kind: OutlineKind::Class,
        name: OutlineName::Field("entity")
    },]
);
rule_outline!(
    PROTOBUF_RULE_OUTLINE,
    [
        OutlineRule {
            node_kind: "message",
            symbol_kind: OutlineKind::Struct,
            name: OutlineName::DescendantKind("message_name")
        },
        OutlineRule {
            node_kind: "enum",
            symbol_kind: OutlineKind::Enum,
            name: OutlineName::DescendantKind("enum_name")
        },
        OutlineRule {
            node_kind: "service",
            symbol_kind: OutlineKind::Interface,
            name: OutlineName::DescendantKind("service_name")
        },
        OutlineRule {
            node_kind: "rpc",
            symbol_kind: OutlineKind::Method,
            name: OutlineName::DescendantKind("rpc_name")
        },
    ]
);
rule_outline!(
    PKL_RULE_OUTLINE,
    [
        OutlineRule {
            node_kind: "clazz",
            symbol_kind: OutlineKind::Class,
            name: OutlineName::DescendantKind("identifier")
        },
        OutlineRule {
            node_kind: "classMethod",
            symbol_kind: OutlineKind::Method,
            name: OutlineName::DescendantKind("identifier")
        },
        OutlineRule {
            node_kind: "typeAlias",
            symbol_kind: OutlineKind::Struct,
            name: OutlineName::DescendantKind("identifier")
        },
    ]
);
rule_outline!(
    WIT_RULE_OUTLINE,
    [OutlineRule {
        node_kind: "interface_item",
        symbol_kind: OutlineKind::Interface,
        name: OutlineName::Field("name")
    },]
);
rule_outline!(
    NIM_RULE_OUTLINE,
    [
        OutlineRule {
            node_kind: "proc_declaration",
            symbol_kind: OutlineKind::Function,
            name: OutlineName::Field("name")
        },
        OutlineRule {
            node_kind: "method_declaration",
            symbol_kind: OutlineKind::Method,
            name: OutlineName::Field("name")
        },
    ]
);
rule_outline!(
    WGSL_RULE_OUTLINE,
    [OutlineRule {
        node_kind: "function_declaration",
        symbol_kind: OutlineKind::Function,
        name: OutlineName::Field("name")
    },]
);
rule_outline!(
    V_RULE_OUTLINE,
    [
        OutlineRule {
            node_kind: "function_declaration",
            symbol_kind: OutlineKind::Function,
            name: OutlineName::Field("name")
        },
        OutlineRule {
            node_kind: "interface_declaration",
            symbol_kind: OutlineKind::Interface,
            name: OutlineName::Field("name")
        },
    ]
);
rule_outline!(
    PONY_RULE_OUTLINE,
    [
        OutlineRule {
            node_kind: "class",
            symbol_kind: OutlineKind::Class,
            name: OutlineName::DescendantKind("identifier")
        },
        OutlineRule {
            node_kind: "interface",
            symbol_kind: OutlineKind::Interface,
            name: OutlineName::DescendantKind("identifier")
        },
        OutlineRule {
            node_kind: "method",
            symbol_kind: OutlineKind::Method,
            name: OutlineName::DescendantKind("identifier")
        },
    ]
);

fn extract_rule_symbols(root: Node, source: &str, rules: &[OutlineRule]) -> Vec<FlatSymbol> {
    let mut symbols = Vec::new();
    walk_and_collect(root, source, &mut symbols, &|node, source, symbols| {
        for rule in rules.iter().filter(|rule| rule.node_kind == node.kind()) {
            let Some(name_node) = outline_name_node(node, rule.name) else {
                continue;
            };
            let Some(name) = node_name(&name_node, source).map(str::trim) else {
                continue;
            };
            if !name.is_empty() {
                symbols.push(flat_sym(rule.symbol_kind, name, &node));
            }
        }
        None
    });
    symbols
}

fn outline_name_node(node: Node<'_>, name: OutlineName) -> Option<Node<'_>> {
    match name {
        OutlineName::Field(field) => node.child_by_field_name(field),
        OutlineName::DescendantKind(node_kind) => descendant_of_kind(node, node_kind),
    }
}

fn descendant_of_kind<'tree>(node: Node<'tree>, node_kind: &str) -> Option<Node<'tree>> {
    if node.kind() == node_kind {
        return Some(node);
    }
    let mut cursor = node.walk();
    let descendant = node
        .named_children(&mut cursor)
        .find_map(|child| descendant_of_kind(child, node_kind));
    descendant
}

fn extract_r_symbols(root: Node, source: &str) -> Vec<FlatSymbol> {
    let mut symbols = Vec::new();
    walk_and_collect(root, source, &mut symbols, &|node, source, symbols| {
        if node.kind() == "binary_operator"
            && node
                .child_by_field_name("rhs")
                .is_some_and(|rhs| rhs.kind() == "function_definition")
        {
            if let Some(name) = node
                .child_by_field_name("lhs")
                .filter(|lhs| lhs.kind() == "identifier")
                .and_then(|lhs| node_name(&lhs, source))
            {
                symbols.push(flat_sym(OutlineKind::Function, name, &node));
            }
        }
        None
    });
    symbols
}

fn extract_elixir_symbols(root: Node, source: &str) -> Vec<FlatSymbol> {
    let mut symbols = Vec::new();
    walk_and_collect(root, source, &mut symbols, &|node, source, symbols| {
        if node.kind() != "call" {
            return None;
        }
        let target_node = node.child_by_field_name("target")?;
        let target = node_name(&target_node, source)?;
        let kind = match target {
            "defmodule" | "defprotocol" => OutlineKind::Module,
            "def" | "defp" | "defmacro" | "defmacrop" => OutlineKind::Function,
            _ => return None,
        };
        let mut cursor = node.walk();
        let name = node
            .named_children(&mut cursor)
            .find(|child| child.start_byte() > target_node.end_byte())
            .and_then(|argument| node_name(&argument, source))
            .and_then(|text| text.split(['(', ',', ' ']).next())
            .filter(|name| !name.is_empty())
            .unwrap_or(target);
        symbols.push(flat_sym(kind, name, &node));
        None
    });
    symbols
}

fn extract_applescript_symbols(root: Node, source: &str) -> Vec<FlatSymbol> {
    let mut symbols = Vec::new();
    walk_and_collect(root, source, &mut symbols, &|node, source, symbols| {
        match node.kind() {
            "handler_definition" => {
                if let Some(name) =
                    child_by_field(&node, "name").and_then(|name| node_name(&name, source))
                {
                    symbols.push(flat_sym(OutlineKind::Function, name, &node));
                }
            }
            "property_declaration" => {
                if let Some(name) =
                    child_by_field(&node, "name").and_then(|name| node_name(&name, source))
                {
                    symbols.push(flat_sym(OutlineKind::Property, name, &node));
                }
            }
            _ => {}
        }
        None
    });
    symbols
}

// =============================================================================
// Flat symbol for pre-nesting
// =============================================================================

struct FlatSymbol {
    kind: OutlineKind,
    name: String,
    start_byte: usize,
    end_byte: usize,
    range: OutlineRange,
}

fn node_range(node: &Node) -> OutlineRange {
    let start = node.start_position();
    let end = node.end_position();
    OutlineRange {
        start_line: start.row,
        start_col: start.column,
        end_line: end.row,
        end_col: end.column,
    }
}

fn node_name<'a>(node: &Node, source: &'a str) -> Option<&'a str> {
    node.utf8_text(source.as_bytes()).ok()
}

fn child_by_field<'a>(node: &Node<'a>, field: &str) -> Option<Node<'a>> {
    node.child_by_field_name(field)
}

fn flat_sym(kind: OutlineKind, name: &str, node: &Node) -> FlatSymbol {
    FlatSymbol {
        kind,
        name: name.to_string(),
        start_byte: node.start_byte(),
        end_byte: node.end_byte(),
        range: node_range(node),
    }
}

// =============================================================================
// Markdown: level-based heading hierarchy
// =============================================================================

fn extract_markdown_headings(root: Node, source: &str) -> Vec<OutlineNode> {
    let mut headings: Vec<(u8, String, OutlineRange)> = Vec::new();
    collect_headings_recursive(root, source, &mut headings);
    build_heading_tree(headings)
}

fn collect_headings_recursive(
    node: Node,
    source: &str,
    headings: &mut Vec<(u8, String, OutlineRange)>,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "atx_heading" || child.kind() == "setext_heading" {
            if let Some((level, text)) = parse_heading(&child, source) {
                headings.push((level, text, node_range(&child)));
            }
        }
        // Recurse into section nodes (tree-sitter-markdown wraps in sections)
        if child.kind() == "section" || child.kind() == "document" {
            collect_headings_recursive(child, source, headings);
        }
    }
}

fn parse_heading(node: &Node, source: &str) -> Option<(u8, String)> {
    if node.kind() == "atx_heading" {
        let mut level = 1u8;
        let mut text = String::new();

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            let kind = child.kind();
            if kind.starts_with("atx_h") && kind.ends_with("_marker") {
                if let Some(marker_text) = node_name(&child, source) {
                    level = marker_text.chars().filter(|c| *c == '#').count().min(6) as u8;
                }
            } else if kind == "inline" || kind == "heading_content" {
                if let Some(t) = node_name(&child, source) {
                    text = t.trim().to_string();
                }
            }
        }

        // Fallback: extract from full heading text
        if text.is_empty() {
            if let Some(full) = node_name(node, source) {
                text = full.trim_start_matches('#').trim().to_string();
            }
        }

        if !text.is_empty() {
            return Some((level, text));
        }
    }

    if node.kind() == "setext_heading" {
        let mut text = String::new();
        let mut level = 2u8;

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            let kind = child.kind();
            if kind == "heading_content" || kind == "paragraph" || kind == "inline" {
                if let Some(t) = node_name(&child, source) {
                    text = t.trim().to_string();
                }
            } else if kind == "setext_h1_underline" {
                level = 1;
            } else if kind == "setext_h2_underline" {
                level = 2;
            }
        }

        if !text.is_empty() {
            return Some((level, text));
        }
    }

    None
}

fn build_heading_tree(headings: Vec<(u8, String, OutlineRange)>) -> Vec<OutlineNode> {
    let mut roots: Vec<OutlineNode> = Vec::new();
    let mut stack: Vec<(u8, OutlineNode)> = Vec::new();

    for (level, text, range) in headings {
        let node = OutlineNode {
            kind: OutlineKind::Heading { level },
            name: text,
            range,
            children: Vec::new(),
        };

        // Pop everything at same level or deeper
        while let Some((top_level, _)) = stack.last() {
            if *top_level >= level {
                let (_, finished) = stack.pop().unwrap();
                if let Some((_, parent)) = stack.last_mut() {
                    parent.children.push(finished);
                } else {
                    roots.push(finished);
                }
            } else {
                break;
            }
        }

        stack.push((level, node));
    }

    // Flush remaining stack
    while let Some((_, finished)) = stack.pop() {
        if let Some((_, parent)) = stack.last_mut() {
            parent.children.push(finished);
        } else {
            roots.push(finished);
        }
    }

    roots
}

// =============================================================================
// Range-containment nesting (for code languages)
// =============================================================================

fn build_tree_by_containment(mut symbols: Vec<FlatSymbol>) -> Vec<OutlineNode> {
    if symbols.is_empty() {
        return Vec::new();
    }

    // Sort by (start_byte asc, end_byte desc) — parents before children
    symbols.sort_by(|a, b| {
        a.start_byte
            .cmp(&b.start_byte)
            .then(b.end_byte.cmp(&a.end_byte))
    });

    let mut roots: Vec<OutlineNode> = Vec::new();
    // Stack: (end_byte, node)
    let mut stack: Vec<(usize, OutlineNode)> = Vec::new();

    for sym in symbols {
        let node = OutlineNode {
            kind: sym.kind,
            name: sym.name,
            range: sym.range,
            children: Vec::new(),
        };

        // Pop items that don't contain this symbol
        while let Some((top_end, _)) = stack.last() {
            if *top_end <= sym.start_byte {
                let (_, finished) = stack.pop().unwrap();
                if let Some((_, parent)) = stack.last_mut() {
                    parent.children.push(finished);
                } else {
                    roots.push(finished);
                }
            } else {
                break;
            }
        }

        stack.push((sym.end_byte, node));
    }

    // Flush remaining stack
    while let Some((_, finished)) = stack.pop() {
        if let Some((_, parent)) = stack.last_mut() {
            parent.children.push(finished);
        } else {
            roots.push(finished);
        }
    }

    roots
}

// =============================================================================
// Shared recursive walking driver
// =============================================================================
//
// Every language's `extract_X_symbols` walks the tree-sitter AST the same
// way: visit a node, let the language decide what (if anything) to record,
// then recurse into children. The only per-language variation is:
//   1. which node kinds produce a `FlatSymbol`, and
//   2. occasionally, which subset of children to recurse into (e.g. Python
//      skips decorator nodes so nested decls inside decorator arguments
//      aren't picked up, and JS/TS's `export_statement` filters out any
//      nested `export_statement` child to avoid double-visiting).
//
// `classify` captures that per-language logic. It may push zero or more
// symbols for `node`, and returns:
//   - `None` to recurse into all of `node`'s children (the common case), or
//   - `Some(children)` to recurse only into that explicit list (which may
//     be empty, effectively stopping descent into `node`).
fn walk_and_collect<'tree>(
    node: Node<'tree>,
    source: &str,
    symbols: &mut Vec<FlatSymbol>,
    classify: &impl Fn(Node<'tree>, &str, &mut Vec<FlatSymbol>) -> Option<Vec<Node<'tree>>>,
) {
    let next_children = classify(node, source, symbols);

    match next_children {
        Some(children) => {
            for child in children {
                walk_and_collect(child, source, symbols, classify);
            }
        }
        None => {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                walk_and_collect(child, source, symbols, classify);
            }
        }
    }
}

// =============================================================================
// Rust symbol extraction
// =============================================================================

fn extract_rust_symbols(root: Node, source: &str) -> Vec<FlatSymbol> {
    let mut symbols = Vec::new();
    walk_and_collect(root, source, &mut symbols, &classify_rust_node);
    symbols
}

fn classify_rust_node<'tree>(
    node: Node<'tree>,
    source: &str,
    symbols: &mut Vec<FlatSymbol>,
) -> Option<Vec<Node<'tree>>> {
    match node.kind() {
        "function_item" => {
            if let Some(name_node) = child_by_field(&node, "name") {
                if let Some(name) = node_name(&name_node, source) {
                    symbols.push(flat_sym(OutlineKind::Function, name, &node));
                }
            }
        }
        "struct_item" => {
            if let Some(name_node) = child_by_field(&node, "name") {
                if let Some(name) = node_name(&name_node, source) {
                    symbols.push(flat_sym(OutlineKind::Struct, name, &node));
                }
            }
        }
        "enum_item" => {
            if let Some(name_node) = child_by_field(&node, "name") {
                if let Some(name) = node_name(&name_node, source) {
                    symbols.push(flat_sym(OutlineKind::Enum, name, &node));
                }
            }
        }
        "enum_variant" => {
            if let Some(name_node) = child_by_field(&node, "name") {
                if let Some(name) = node_name(&name_node, source) {
                    symbols.push(flat_sym(OutlineKind::EnumVariant, name, &node));
                }
            }
        }
        "impl_item" => {
            if let Some(type_node) = child_by_field(&node, "type") {
                if let Some(name) = node_name(&type_node, source) {
                    let label = if let Some(trait_node) = child_by_field(&node, "trait") {
                        if let Some(trait_name) = node_name(&trait_node, source) {
                            format!("{} for {}", trait_name, name)
                        } else {
                            name.to_string()
                        }
                    } else {
                        name.to_string()
                    };
                    symbols.push(flat_sym(OutlineKind::Impl, &label, &node));
                }
            }
        }
        "trait_item" => {
            if let Some(name_node) = child_by_field(&node, "name") {
                if let Some(name) = node_name(&name_node, source) {
                    symbols.push(flat_sym(OutlineKind::Trait, name, &node));
                }
            }
        }
        "const_item" | "static_item" => {
            if let Some(name_node) = child_by_field(&node, "name") {
                if let Some(name) = node_name(&name_node, source) {
                    symbols.push(flat_sym(OutlineKind::Constant, name, &node));
                }
            }
        }
        "mod_item" => {
            if let Some(name_node) = child_by_field(&node, "name") {
                if let Some(name) = node_name(&name_node, source) {
                    symbols.push(flat_sym(OutlineKind::Module, name, &node));
                }
            }
        }
        "field_declaration" => {
            if let Some(name_node) = child_by_field(&node, "name") {
                if let Some(name) = node_name(&name_node, source) {
                    symbols.push(flat_sym(OutlineKind::Field, name, &node));
                }
            }
        }
        _ => {}
    }

    None
}

// =============================================================================
// TypeScript/JavaScript symbol extraction
// =============================================================================

fn extract_js_ts_symbols(root: Node, source: &str) -> Vec<FlatSymbol> {
    let mut symbols = Vec::new();
    walk_and_collect(root, source, &mut symbols, &classify_js_ts_node);
    symbols
}

fn classify_js_ts_node<'tree>(
    node: Node<'tree>,
    source: &str,
    symbols: &mut Vec<FlatSymbol>,
) -> Option<Vec<Node<'tree>>> {
    match node.kind() {
        "function_declaration" => {
            if let Some(name_node) = child_by_field(&node, "name") {
                if let Some(name) = node_name(&name_node, source) {
                    symbols.push(flat_sym(OutlineKind::Function, name, &node));
                }
            }
        }
        "class_declaration" => {
            if let Some(name_node) = child_by_field(&node, "name") {
                if let Some(name) = node_name(&name_node, source) {
                    symbols.push(flat_sym(OutlineKind::Class, name, &node));
                }
            }
        }
        "method_definition" => {
            if let Some(name_node) = child_by_field(&node, "name") {
                if let Some(name) = node_name(&name_node, source) {
                    symbols.push(flat_sym(OutlineKind::Method, name, &node));
                }
            }
        }
        "interface_declaration" => {
            if let Some(name_node) = child_by_field(&node, "name") {
                if let Some(name) = node_name(&name_node, source) {
                    symbols.push(flat_sym(OutlineKind::Interface, name, &node));
                }
            }
        }
        "type_alias_declaration" => {
            if let Some(name_node) = child_by_field(&node, "name") {
                if let Some(name) = node_name(&name_node, source) {
                    symbols.push(flat_sym(OutlineKind::Interface, name, &node));
                }
            }
        }
        "enum_declaration" => {
            if let Some(name_node) = child_by_field(&node, "name") {
                if let Some(name) = node_name(&name_node, source) {
                    symbols.push(flat_sym(OutlineKind::Enum, name, &node));
                }
            }
        }
        "public_field_definition" | "property_signature" => {
            if let Some(name_node) = child_by_field(&node, "name") {
                if let Some(name) = node_name(&name_node, source) {
                    symbols.push(flat_sym(OutlineKind::Property, name, &node));
                }
            }
        }
        "lexical_declaration" | "variable_declaration" => {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() == "variable_declarator" {
                    if let Some(name_node) = child_by_field(&child, "name") {
                        if let Some(name) = node_name(&name_node, source) {
                            let kind = if child
                                .child_by_field_name("value")
                                .map(|v| v.kind() == "arrow_function" || v.kind() == "function")
                                .unwrap_or(false)
                            {
                                OutlineKind::Function
                            } else {
                                OutlineKind::Constant
                            };
                            symbols.push(flat_sym(kind, name, &node));
                        }
                    }
                }
            }
        }
        "export_statement" => {
            let mut cursor = node.walk();
            let children: Vec<Node> = node
                .children(&mut cursor)
                .filter(|c| c.kind() != "export_statement")
                .collect();
            return Some(children);
        }
        _ => {}
    }

    None
}

// =============================================================================
// Python symbol extraction
// =============================================================================

fn extract_python_symbols(root: Node, source: &str) -> Vec<FlatSymbol> {
    let mut symbols = Vec::new();
    walk_and_collect(root, source, &mut symbols, &classify_python_node);
    symbols
}

fn classify_python_node<'tree>(
    node: Node<'tree>,
    source: &str,
    symbols: &mut Vec<FlatSymbol>,
) -> Option<Vec<Node<'tree>>> {
    match node.kind() {
        "function_definition" => {
            if let Some(name_node) = child_by_field(&node, "name") {
                if let Some(name) = node_name(&name_node, source) {
                    symbols.push(flat_sym(OutlineKind::Function, name, &node));
                }
            }
        }
        "class_definition" => {
            if let Some(name_node) = child_by_field(&node, "name") {
                if let Some(name) = node_name(&name_node, source) {
                    symbols.push(flat_sym(OutlineKind::Class, name, &node));
                }
            }
        }
        "decorated_definition" => {
            let mut cursor = node.walk();
            let children: Vec<Node> = node
                .children(&mut cursor)
                .filter(|c| c.kind() == "function_definition" || c.kind() == "class_definition")
                .collect();
            return Some(children);
        }
        _ => {}
    }

    None
}

// =============================================================================
// Go symbol extraction
// =============================================================================

fn extract_go_symbols(root: Node, source: &str) -> Vec<FlatSymbol> {
    let mut symbols = Vec::new();
    walk_and_collect(root, source, &mut symbols, &classify_go_node);
    symbols
}

fn classify_go_node<'tree>(
    node: Node<'tree>,
    source: &str,
    symbols: &mut Vec<FlatSymbol>,
) -> Option<Vec<Node<'tree>>> {
    match node.kind() {
        "function_declaration" => {
            if let Some(name_node) = child_by_field(&node, "name") {
                if let Some(name) = node_name(&name_node, source) {
                    symbols.push(flat_sym(OutlineKind::Function, name, &node));
                }
            }
        }
        "method_declaration" => {
            if let Some(name_node) = child_by_field(&node, "name") {
                if let Some(name) = node_name(&name_node, source) {
                    symbols.push(flat_sym(OutlineKind::Method, name, &node));
                }
            }
        }
        "type_declaration" => {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() == "type_spec" {
                    if let Some(name_node) = child_by_field(&child, "name") {
                        if let Some(name) = node_name(&name_node, source) {
                            let type_node = child_by_field(&child, "type");
                            let kind = match type_node.as_ref().map(|n| n.kind()) {
                                Some("struct_type") => OutlineKind::Struct,
                                Some("interface_type") => OutlineKind::Interface,
                                _ => OutlineKind::Interface,
                            };
                            symbols.push(flat_sym(kind, name, &child));
                        }
                    }
                }
            }
        }
        "const_declaration" | "var_declaration" => {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() == "const_spec" || child.kind() == "var_spec" {
                    if let Some(name_node) = child_by_field(&child, "name") {
                        if let Some(name) = node_name(&name_node, source) {
                            symbols.push(flat_sym(OutlineKind::Constant, name, &child));
                        }
                    }
                }
            }
        }
        _ => {}
    }

    None
}

// =============================================================================
// Java symbol extraction
// =============================================================================

fn extract_java_symbols(root: Node, source: &str) -> Vec<FlatSymbol> {
    let mut symbols = Vec::new();
    walk_and_collect(root, source, &mut symbols, &classify_java_node);
    symbols
}

fn classify_java_node<'tree>(
    node: Node<'tree>,
    source: &str,
    symbols: &mut Vec<FlatSymbol>,
) -> Option<Vec<Node<'tree>>> {
    match node.kind() {
        "class_declaration" => {
            if let Some(name_node) = child_by_field(&node, "name") {
                if let Some(name) = node_name(&name_node, source) {
                    symbols.push(flat_sym(OutlineKind::Class, name, &node));
                }
            }
        }
        "interface_declaration" => {
            if let Some(name_node) = child_by_field(&node, "name") {
                if let Some(name) = node_name(&name_node, source) {
                    symbols.push(flat_sym(OutlineKind::Interface, name, &node));
                }
            }
        }
        "enum_declaration" => {
            if let Some(name_node) = child_by_field(&node, "name") {
                if let Some(name) = node_name(&name_node, source) {
                    symbols.push(flat_sym(OutlineKind::Enum, name, &node));
                }
            }
        }
        "method_declaration" | "constructor_declaration" => {
            if let Some(name_node) = child_by_field(&node, "name") {
                if let Some(name) = node_name(&name_node, source) {
                    symbols.push(flat_sym(OutlineKind::Method, name, &node));
                }
            }
        }
        "field_declaration" => {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() == "variable_declarator" {
                    if let Some(name_node) = child_by_field(&child, "name") {
                        if let Some(name) = node_name(&name_node, source) {
                            symbols.push(flat_sym(OutlineKind::Field, name, &node));
                        }
                    }
                }
            }
        }
        _ => {}
    }

    None
}

// =============================================================================
// PHP symbol extraction
// =============================================================================

fn extract_php_symbols(root: Node, source: &str) -> Vec<FlatSymbol> {
    let mut symbols = Vec::new();
    walk_and_collect(root, source, &mut symbols, &classify_php_node);
    symbols
}

fn classify_php_node<'tree>(
    node: Node<'tree>,
    source: &str,
    symbols: &mut Vec<FlatSymbol>,
) -> Option<Vec<Node<'tree>>> {
    match node.kind() {
        "class_declaration" => {
            if let Some(name_node) = child_by_field(&node, "name") {
                if let Some(name) = node_name(&name_node, source) {
                    symbols.push(flat_sym(OutlineKind::Class, name, &node));
                }
            }
        }
        "function_definition" => {
            if let Some(name_node) = child_by_field(&node, "name") {
                if let Some(name) = node_name(&name_node, source) {
                    symbols.push(flat_sym(OutlineKind::Function, name, &node));
                }
            }
        }
        "method_declaration" => {
            if let Some(name_node) = child_by_field(&node, "name") {
                if let Some(name) = node_name(&name_node, source) {
                    symbols.push(flat_sym(OutlineKind::Method, name, &node));
                }
            }
        }
        "interface_declaration" => {
            if let Some(name_node) = child_by_field(&node, "name") {
                if let Some(name) = node_name(&name_node, source) {
                    symbols.push(flat_sym(OutlineKind::Interface, name, &node));
                }
            }
        }
        "trait_declaration" => {
            if let Some(name_node) = child_by_field(&node, "name") {
                if let Some(name) = node_name(&name_node, source) {
                    symbols.push(flat_sym(OutlineKind::Trait, name, &node));
                }
            }
        }
        "namespace_definition" => {
            if let Some(name_node) = child_by_field(&node, "name") {
                if let Some(name) = node_name(&name_node, source) {
                    symbols.push(flat_sym(OutlineKind::Namespace, name, &node));
                }
            }
        }
        _ => {}
    }

    None
}

// =============================================================================
// C/C++ symbol extraction
// =============================================================================

fn extract_c_cpp_symbols(root: Node, source: &str, language: LanguageId) -> Vec<FlatSymbol> {
    let mut symbols = Vec::new();
    walk_and_collect(root, source, &mut symbols, &|node, source, symbols| {
        classify_c_cpp_node(node, source, symbols, language)
    });
    symbols
}

fn classify_c_cpp_node<'tree>(
    node: Node<'tree>,
    source: &str,
    symbols: &mut Vec<FlatSymbol>,
    language: LanguageId,
) -> Option<Vec<Node<'tree>>> {
    match node.kind() {
        "function_definition" => {
            if let Some(decl) = child_by_field(&node, "declarator") {
                if let Some(name) = extract_c_function_name(&decl, source) {
                    symbols.push(flat_sym(OutlineKind::Function, &name, &node));
                }
            }
        }
        "struct_specifier" => {
            if let Some(name_node) = child_by_field(&node, "name") {
                if let Some(name) = node_name(&name_node, source) {
                    symbols.push(flat_sym(OutlineKind::Struct, name, &node));
                }
            }
        }
        "enum_specifier" => {
            if let Some(name_node) = child_by_field(&node, "name") {
                if let Some(name) = node_name(&name_node, source) {
                    symbols.push(flat_sym(OutlineKind::Enum, name, &node));
                }
            }
        }
        "class_specifier" if language == LanguageId::Cpp => {
            if let Some(name_node) = child_by_field(&node, "name") {
                if let Some(name) = node_name(&name_node, source) {
                    symbols.push(flat_sym(OutlineKind::Class, name, &node));
                }
            }
        }
        "namespace_definition" if language == LanguageId::Cpp => {
            if let Some(name_node) = child_by_field(&node, "name") {
                if let Some(name) = node_name(&name_node, source) {
                    symbols.push(flat_sym(OutlineKind::Namespace, name, &node));
                }
            }
        }
        _ => {}
    }

    None
}

fn extract_c_function_name(declarator: &Node, source: &str) -> Option<String> {
    match declarator.kind() {
        "function_declarator" => {
            if let Some(name_node) = child_by_field(declarator, "declarator") {
                return extract_c_function_name(&name_node, source);
            }
        }
        "pointer_declarator" => {
            if let Some(decl) = child_by_field(declarator, "declarator") {
                return extract_c_function_name(&decl, source);
            }
        }
        "identifier" | "field_identifier" | "qualified_identifier" | "destructor_name" => {
            return node_name(declarator, source).map(|s| s.to_string());
        }
        _ => {}
    }
    None
}

// =============================================================================
// YAML symbol extraction
// =============================================================================

fn extract_yaml_symbols(root: Node, source: &str) -> Vec<FlatSymbol> {
    let mut symbols = Vec::new();
    walk_and_collect(root, source, &mut symbols, &classify_yaml_node);
    symbols
}

fn classify_yaml_node<'tree>(
    node: Node<'tree>,
    source: &str,
    symbols: &mut Vec<FlatSymbol>,
) -> Option<Vec<Node<'tree>>> {
    match node.kind() {
        "block_mapping_pair" | "flow_pair" => {
            if let Some(key_node) = child_by_field(&node, "key") {
                if let Some(key_text) = node_name(&key_node, source) {
                    let name = key_text.trim().trim_matches('"').trim_matches('\'');
                    if !name.is_empty() {
                        symbols.push(flat_sym(OutlineKind::Property, name, &node));
                    }
                }
            }
        }
        _ => {}
    }

    None
}

// =============================================================================
// HTML symbol extraction
// =============================================================================

/// Tags worth showing in the outline (structural/semantic elements)
const HTML_OUTLINE_TAGS: &[&str] = &[
    "html", "head", "body", "header", "footer", "nav", "main", "aside", "section", "article",
    "div", "form", "table", "thead", "tbody", "tfoot", "ul", "ol", "dl", "details", "dialog",
    "fieldset", "figure", "template", "slot",
];

fn extract_html_symbols(root: Node, source: &str) -> Vec<FlatSymbol> {
    let mut symbols = Vec::new();
    walk_and_collect(root, source, &mut symbols, &classify_html_node);
    symbols
}

fn classify_html_node<'tree>(
    node: Node<'tree>,
    source: &str,
    symbols: &mut Vec<FlatSymbol>,
) -> Option<Vec<Node<'tree>>> {
    if node.kind() == "element" {
        if let Some(start_tag) = node.child_by_field_name("start_tag").or_else(|| {
            node.children(&mut node.walk())
                .find(|c| c.kind() == "start_tag")
        }) {
            if let Some(tag_name_node) = start_tag
                .children(&mut start_tag.walk())
                .find(|c| c.kind() == "tag_name")
            {
                if let Some(tag_name) = node_name(&tag_name_node, source) {
                    let tag_lower = tag_name.to_lowercase();
                    if HTML_OUTLINE_TAGS.contains(&tag_lower.as_str()) {
                        let label = html_element_label(&tag_lower, &start_tag, source);
                        symbols.push(flat_sym(OutlineKind::Element, &label, &node));
                    }
                }
            }
        }
    }

    None
}

/// Build a display label like `div#app` or `section.hero` from attributes
fn html_element_label(tag_name: &str, start_tag: &Node, source: &str) -> String {
    let mut id = None;
    let mut class = None;

    let mut cursor = start_tag.walk();
    for attr in start_tag.children(&mut cursor) {
        if attr.kind() != "attribute" {
            continue;
        }
        let attr_name = attr
            .children(&mut attr.walk())
            .find(|c| c.kind() == "attribute_name")
            .and_then(|n| node_name(&n, source));
        let attr_val = attr
            .children(&mut attr.walk())
            .find(|c| c.kind() == "quoted_attribute_value" || c.kind() == "attribute_value")
            .and_then(|n| node_name(&n, source))
            .map(|v| v.trim_matches('"').trim_matches('\''));

        match attr_name {
            Some("id") => id = attr_val.map(|s| s.to_string()),
            Some("class") => class = attr_val.map(|s| s.to_string()),
            _ => {}
        }
    }

    let mut label = tag_name.to_string();
    if let Some(id_val) = id {
        label.push('#');
        label.push_str(&id_val);
    } else if let Some(class_val) = class {
        // Use first class only to keep labels short
        if let Some(first_class) = class_val.split_whitespace().next() {
            label.push('.');
            label.push_str(first_class);
        }
    }
    label
}

// =============================================================================
// Blade symbol extraction
// =============================================================================

/// Structural directives worth showing in the outline.
/// Control flow (@if, @foreach, etc.) and attribute helpers (@class, @checked, etc.)
/// are excluded to reduce noise — they are implementation details, not document structure.
const BLADE_OUTLINE_DIRECTIVES: &[&str] = &[
    // Layout / composition
    "extends",
    "include",
    "includeIf",
    "includeWhen",
    "includeUnless",
    "includeFirst",
    "each",
    // Sections / slots
    "section",
    "yield",
    "fragment",
    // Stacks
    "stack",
    "push",
    "pushOnce",
    "prepend",
    "prependOnce",
    // Special blocks
    "verbatim",
    "once",
    // Livewire
    "livewire",
    "persist",
    "teleport",
    "volt",
    "script",
    "assets",
];

fn extract_blade_symbols(root: Node, source: &str) -> Vec<FlatSymbol> {
    let mut symbols = Vec::new();
    walk_and_collect(root, source, &mut symbols, &classify_blade_node);
    symbols
}

fn classify_blade_node<'tree>(
    node: Node<'tree>,
    source: &str,
    symbols: &mut Vec<FlatSymbol>,
) -> Option<Vec<Node<'tree>>> {
    match node.kind() {
        // HTML elements (reuse HTML logic)
        "element" => {
            if let Some(start_tag) = node
                .children(&mut node.walk())
                .find(|c| c.kind() == "start_tag")
            {
                if let Some(tag_name_node) = start_tag
                    .children(&mut start_tag.walk())
                    .find(|c| c.kind() == "tag_name")
                {
                    if let Some(tag_name) = node_name(&tag_name_node, source) {
                        let tag_lower = tag_name.to_lowercase();
                        if HTML_OUTLINE_TAGS.contains(&tag_lower.as_str())
                            || tag_name.starts_with("x-")
                        {
                            let label = if tag_name.starts_with("x-") {
                                format!("<{}>", tag_name)
                            } else {
                                html_element_label(&tag_lower, &start_tag, source)
                            };
                            symbols.push(flat_sym(OutlineKind::Element, &label, &node));
                        }
                    }
                }
            }
        }
        // Blade sections: @section, @fragment, @stack, and other structural block directives
        "section" | "fragment" | "stack" | "once" | "verbatim" | "livewire" => {
            let ident = blade_directive_ident(&node, source);
            if let Some(ref name) = ident {
                if !BLADE_OUTLINE_DIRECTIVES.contains(&name.as_str()) {
                    // Not structural — skip pushing a symbol, but still recurse
                    // into children normally (falls through to `None` below).
                    return None;
                }
            }
            let kind = match node.kind() {
                "section" | "fragment" | "stack" => OutlineKind::Section,
                _ => OutlineKind::Directive,
            };
            let label = blade_directive_label(&node, source);
            symbols.push(flat_sym(kind, &label, &node));
        }
        // Control flow (conditional, loop, switch) — skip entirely, these are
        // implementation details not document structure. Still recurse for nested elements.
        "conditional" | "loop" | "switch" => {}
        _ => {}
    }

    None
}

/// Extract the directive identifier from a node's raw text by scanning for `@`.
/// Returns just the identifier (e.g. "section", "foreach") without the `@` prefix.
/// This is robust against node ranges that include leading whitespace or control characters.
fn blade_directive_ident(node: &Node, source: &str) -> Option<String> {
    let mut cursor = node.walk();
    let raw = node
        .children(&mut cursor)
        .find(|c| c.kind() == "directive_start" || c.kind() == "directive")
        .and_then(|d| node_name(&d, source))?;

    parse_directive_ident(raw)
}

/// Parse a directive identifier from raw node text.
/// Finds the first `@` and reads the alphanumeric identifier after it.
fn parse_directive_ident(raw: &str) -> Option<String> {
    let at = raw.find('@')?;
    let s = &raw[at + 1..];
    let end = s
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .unwrap_or(s.len());
    if end == 0 {
        return None;
    }
    Some(s[..end].to_string())
}

/// Build a display label for a directive node, e.g. `@section('content')` or `@verbatim`.
fn blade_directive_label(node: &Node, source: &str) -> String {
    let ident = blade_directive_ident(node, source).unwrap_or_else(|| "?".to_string());
    let directive = format!("@{}", ident);

    // Try to extract the first string parameter for named directives
    let param = node
        .children(&mut node.walk())
        .find(|c| c.kind() == "parameter")
        .and_then(|p| node_name(&p, source));

    if let Some(param_text) = param {
        let cleaned = param_text
            .trim_matches(|c: char| c == '(' || c == ')')
            .trim()
            .split(',')
            .next()
            .unwrap_or("")
            .trim()
            .trim_matches(|c: char| c == '\'' || c == '"');
        if !cleaned.is_empty() {
            return format!("{}('{}')", directive, cleaned);
        }
    }

    directive
}

// =============================================================================
// Vue SFC symbol extraction
// =============================================================================

fn extract_vue_symbols(root: Node, source: &str) -> Vec<FlatSymbol> {
    let mut symbols = Vec::new();
    walk_and_collect(root, source, &mut symbols, &classify_vue_node);
    symbols
}

fn classify_vue_node<'tree>(
    node: Node<'tree>,
    source: &str,
    symbols: &mut Vec<FlatSymbol>,
) -> Option<Vec<Node<'tree>>> {
    match node.kind() {
        "element" | "script_element" | "style_element" => {
            if let Some(tag_name) = vue_element_tag_name(node, source) {
                match tag_name.as_str() {
                    "template" | "script" | "style" => {
                        symbols.push(flat_sym(
                            OutlineKind::Section,
                            &format!("<{}>", tag_name),
                            &node,
                        ));
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }

    None
}

/// Get the tag name from an HTML element node (element, script_element, style_element)
fn vue_element_tag_name(node: Node, source: &str) -> Option<String> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "start_tag" {
            let mut tag_cursor = child.walk();
            for tag_child in child.children(&mut tag_cursor) {
                if tag_child.kind() == "tag_name" {
                    return node_name(&tag_child, source).map(|s| s.to_string());
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_heading_tree_basic() {
        let headings = vec![
            (
                1,
                "Title".to_string(),
                OutlineRange {
                    start_line: 0,
                    start_col: 0,
                    end_line: 0,
                    end_col: 7,
                },
            ),
            (
                2,
                "Section 1".to_string(),
                OutlineRange {
                    start_line: 2,
                    start_col: 0,
                    end_line: 2,
                    end_col: 12,
                },
            ),
            (
                3,
                "Sub 1.1".to_string(),
                OutlineRange {
                    start_line: 4,
                    start_col: 0,
                    end_line: 4,
                    end_col: 11,
                },
            ),
            (
                2,
                "Section 2".to_string(),
                OutlineRange {
                    start_line: 6,
                    start_col: 0,
                    end_line: 6,
                    end_col: 12,
                },
            ),
        ];

        let tree = build_heading_tree(headings);
        assert_eq!(tree.len(), 1, "Should have one root (H1)");
        assert_eq!(tree[0].name, "Title");
        assert_eq!(tree[0].children.len(), 2, "H1 should have 2 H2 children");
        assert_eq!(tree[0].children[0].name, "Section 1");
        assert_eq!(
            tree[0].children[0].children.len(),
            1,
            "First H2 should have 1 H3 child"
        );
        assert_eq!(tree[0].children[1].name, "Section 2");
    }

    #[test]
    fn test_heading_tree_no_h1() {
        let headings = vec![
            (
                2,
                "A".to_string(),
                OutlineRange {
                    start_line: 0,
                    start_col: 0,
                    end_line: 0,
                    end_col: 4,
                },
            ),
            (
                2,
                "B".to_string(),
                OutlineRange {
                    start_line: 2,
                    start_col: 0,
                    end_line: 2,
                    end_col: 4,
                },
            ),
            (
                3,
                "B1".to_string(),
                OutlineRange {
                    start_line: 4,
                    start_col: 0,
                    end_line: 4,
                    end_col: 6,
                },
            ),
        ];

        let tree = build_heading_tree(headings);
        assert_eq!(tree.len(), 2, "Should have two root H2s");
        assert_eq!(tree[1].children.len(), 1, "Second H2 should have H3 child");
    }

    #[test]
    fn test_containment_nesting() {
        let symbols = vec![
            FlatSymbol {
                kind: OutlineKind::Struct,
                name: "MyStruct".to_string(),
                start_byte: 0,
                end_byte: 100,
                range: OutlineRange {
                    start_line: 0,
                    start_col: 0,
                    end_line: 5,
                    end_col: 1,
                },
            },
            FlatSymbol {
                kind: OutlineKind::Field,
                name: "field_a".to_string(),
                start_byte: 20,
                end_byte: 40,
                range: OutlineRange {
                    start_line: 1,
                    start_col: 4,
                    end_line: 1,
                    end_col: 20,
                },
            },
            FlatSymbol {
                kind: OutlineKind::Function,
                name: "standalone".to_string(),
                start_byte: 110,
                end_byte: 200,
                range: OutlineRange {
                    start_line: 7,
                    start_col: 0,
                    end_line: 10,
                    end_col: 1,
                },
            },
        ];

        let tree = build_tree_by_containment(symbols);
        assert_eq!(tree.len(), 2, "Should have struct + standalone fn");
        assert_eq!(tree[0].name, "MyStruct");
        assert_eq!(tree[0].children.len(), 1, "Struct should contain field_a");
        assert_eq!(tree[0].children[0].name, "field_a");
        assert_eq!(tree[1].name, "standalone");
    }

    #[test]
    fn test_parse_directive_ident() {
        // Normal case
        assert_eq!(parse_directive_ident("@section"), Some("section".into()));
        assert_eq!(parse_directive_ident("@foreach"), Some("foreach".into()));
        assert_eq!(parse_directive_ident("@if"), Some("if".into()));

        // With leading whitespace/newlines (the bug case)
        assert_eq!(
            parse_directive_ident("\n        @class"),
            Some("class".into())
        );
        assert_eq!(
            parse_directive_ident("\n\n@forelse"),
            Some("forelse".into())
        );
        assert_eq!(parse_directive_ident("  \t@push"), Some("push".into()));

        // With parameters after
        assert_eq!(
            parse_directive_ident("@section('content')"),
            Some("section".into())
        );

        // No @ sign
        assert_eq!(parse_directive_ident("noatsign"), None);

        // Empty after @
        assert_eq!(parse_directive_ident("@"), None);
    }

    #[test]
    fn applescript_outline_contains_handlers_and_properties() {
        let source = "property greeting : \"Hello\"\n\non greet(personName)\n\treturn personName\nend greet\n";
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_applescript::language())
            .expect("AppleScript grammar should load");
        let tree = parser
            .parse(source, None)
            .expect("AppleScript source should parse");

        let outline = extract_outline(&tree, source, LanguageId::AppleScript, 1);
        assert!(outline
            .roots
            .iter()
            .any(|node| { node.kind == OutlineKind::Property && node.name == "greeting" }));
        assert!(outline
            .roots
            .iter()
            .any(|node| { node.kind == OutlineKind::Function && node.name == "greet" }));
    }

    #[test]
    fn extended_languages_extract_top_level_symbols() {
        let cases = [
            (LanguageId::CSharp, tree_sitter_c_sharp::LANGUAGE.into(), "class Greeter { string Greet(string name) { return name; } }", "Greeter"),
            (LanguageId::Ruby, tree_sitter_ruby::LANGUAGE.into(), "class Greeter\n  def greet(name) = name\nend\n", "Greeter"),
            (LanguageId::Lua, tree_sitter_lua::LANGUAGE.into(), "function greet(name) return name end", "greet"),
            (LanguageId::R, tree_sitter_r::LANGUAGE.into(), "greet <- function(name) { name }", "greet"),
            (LanguageId::Swift, tree_sitter_swift::LANGUAGE.into(), "func greet(name: String) -> String { name }", "greet"),
            (LanguageId::Elixir, tree_sitter_elixir::LANGUAGE.into(), "defmodule Greeter do\n  def greet(name), do: name\nend", "Greeter"),
            (LanguageId::Gleam, tree_sitter_gleam::LANGUAGE.into(), "pub fn greet(name: String) { name }", "greet"),
            (LanguageId::Solidity, tree_sitter_solidity::LANGUAGE.into(), "contract Greeter { function greet(string memory name) public pure returns (string memory) { return name; } }", "Greeter"),
            (LanguageId::Kotlin, tree_sitter_kotlin_ng::LANGUAGE.into(), "class Greeter { fun greet(name: String): String = name }", "Greeter"),
            (LanguageId::Dart, tree_sitter_dart::LANGUAGE.into(), "class Greeter { String greet(String name) => name; }", "Greeter"),
            (LanguageId::Vhdl, tree_sitter_vhdl::LANGUAGE.into(), "entity greeter is end entity;", "greeter"),
            (LanguageId::Protobuf, tree_sitter_proto::LANGUAGE.into(), "message Greeter { string name = 1; }", "Greeter"),
            (LanguageId::Pkl, tree_sitter_pkl::LANGUAGE.into(), "class Greeter { name: String }", "Greeter"),
            (LanguageId::Wit, tree_sitter_wit::LANGUAGE.into(), "interface greeter { greet: func(name: string) -> string; }", "greeter"),
            (LanguageId::Nim, tree_sitter_nim::language(), "proc greet(name: string): string = name", "greet"),
            (LanguageId::Wgsl, tree_sitter_wgsl_bevy::LANGUAGE.into(), "fn greet(name: u32) -> u32 { return name; }", "greet"),
            (LanguageId::V, tree_sitter_v::LANGUAGE.into(), "fn greet(name string) string { return name }", "greet"),
        ];

        for (language, grammar, source, expected) in cases {
            let mut parser = tree_sitter::Parser::new();
            parser.set_language(&grammar).expect("grammar should load");
            let tree = parser.parse(source, None).expect("source should parse");
            let outline = extract_outline(&tree, source, language, 1);
            assert!(
                outline.roots.iter().any(|node| node.name == expected),
                "{language:?}: expected {expected}, got {:?}; tree: {}",
                outline
                    .roots
                    .iter()
                    .map(|node| &node.name)
                    .collect::<Vec<_>>(),
                tree.root_node().to_sexp()
            );
        }
    }

    #[test]
    fn markdown_outline_through_the_real_worker_path() {
        // Exactly what the syntax worker does (runtime/app.rs): parse via
        // ParserState (injection-aware), then extract from the cached tree.
        use crate::syntax::ParserState;

        let source = "# Title\n\ntext\n\n## Section A\n\n### Sub A1\n";
        let doc_id = crate::model::DocumentId(1);
        let mut parser_state = ParserState::new();
        parser_state.parse_and_highlight(source, LanguageId::Markdown, doc_id, 1);
        let (tree, lang) = parser_state
            .get_cached_tree(doc_id)
            .expect("markdown parse must cache a tree");
        let outline = extract_outline(tree, source, lang, 1);
        assert_eq!(
            outline.roots.len(),
            1,
            "one H1 root, got {:?}",
            outline.roots
        );
        assert_eq!(outline.roots[0].name, "Title");
        assert_eq!(outline.roots[0].children[0].name, "Section A");
    }

    #[test]
    fn markdown_headings_nest_by_level() {
        let source = "# Title\n\ntext\n\n## Section A\n\n### Sub A1\n\n## Section B\n";
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_md::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(source, None).unwrap();
        let outline = extract_outline(&tree, source, LanguageId::Markdown, 1);
        assert_eq!(outline.roots.len(), 1);
        let title = &outline.roots[0];
        assert_eq!(title.name, "Title");
        assert_eq!(title.children.len(), 2);
        assert_eq!(title.children[0].name, "Section A");
        assert_eq!(title.children[0].children[0].name, "Sub A1");
        assert_eq!(title.children[1].name, "Section B");
    }
}
