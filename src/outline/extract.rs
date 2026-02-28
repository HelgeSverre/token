//! Outline extraction from tree-sitter parse trees
//!
//! Walks the tree-sitter AST to extract structural symbols.
//! Runs on the syntax worker thread.

use tree_sitter::{Node, Tree};

use super::{OutlineData, OutlineKind, OutlineNode, OutlineRange};
use crate::syntax::LanguageId;

/// Extract outline from a tree-sitter parse tree
pub fn extract_outline(
    tree: &Tree,
    source: &str,
    language: LanguageId,
    revision: u64,
) -> OutlineData {
    let root = tree.root_node();

    let nodes = match language {
        LanguageId::Markdown => extract_markdown_headings(root, source),
        LanguageId::Rust => {
            let flat = extract_rust_symbols(root, source);
            build_tree_by_containment(flat)
        }
        LanguageId::TypeScript | LanguageId::Tsx | LanguageId::JavaScript | LanguageId::Jsx => {
            let flat = extract_js_ts_symbols(root, source);
            build_tree_by_containment(flat)
        }
        LanguageId::Python => {
            let flat = extract_python_symbols(root, source);
            build_tree_by_containment(flat)
        }
        LanguageId::Go => {
            let flat = extract_go_symbols(root, source);
            build_tree_by_containment(flat)
        }
        LanguageId::Java => {
            let flat = extract_java_symbols(root, source);
            build_tree_by_containment(flat)
        }
        LanguageId::Php => {
            let flat = extract_php_symbols(root, source);
            build_tree_by_containment(flat)
        }
        LanguageId::C | LanguageId::Cpp => {
            let flat = extract_c_cpp_symbols(root, source, language);
            build_tree_by_containment(flat)
        }
        LanguageId::Yaml => {
            let flat = extract_yaml_symbols(root, source);
            build_tree_by_containment(flat)
        }
        LanguageId::Html => {
            let flat = extract_html_symbols(root, source);
            build_tree_by_containment(flat)
        }
        LanguageId::Blade => {
            let flat = extract_blade_symbols(root, source);
            build_tree_by_containment(flat)
        }
        LanguageId::Vue => {
            let flat = extract_vue_symbols(root, source);
            build_tree_by_containment(flat)
        }
        _ => Vec::new(),
    };

    OutlineData {
        revision,
        roots: nodes,
    }
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
// Rust symbol extraction
// =============================================================================

fn extract_rust_symbols(root: Node, source: &str) -> Vec<FlatSymbol> {
    let mut symbols = Vec::new();
    collect_rust_symbols(root, source, &mut symbols);
    symbols
}

fn collect_rust_symbols(node: Node, source: &str, symbols: &mut Vec<FlatSymbol>) {
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

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_rust_symbols(child, source, symbols);
    }
}

// =============================================================================
// TypeScript/JavaScript symbol extraction
// =============================================================================

fn extract_js_ts_symbols(root: Node, source: &str) -> Vec<FlatSymbol> {
    let mut symbols = Vec::new();
    collect_js_ts_symbols(root, source, &mut symbols);
    symbols
}

fn collect_js_ts_symbols(node: Node, source: &str, symbols: &mut Vec<FlatSymbol>) {
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
            for child in node.children(&mut cursor) {
                if child.kind() != "export_statement" {
                    collect_js_ts_symbols(child, source, symbols);
                }
            }
            return;
        }
        _ => {}
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_js_ts_symbols(child, source, symbols);
    }
}

// =============================================================================
// Python symbol extraction
// =============================================================================

fn extract_python_symbols(root: Node, source: &str) -> Vec<FlatSymbol> {
    let mut symbols = Vec::new();
    collect_python_symbols(root, source, &mut symbols);
    symbols
}

fn collect_python_symbols(node: Node, source: &str, symbols: &mut Vec<FlatSymbol>) {
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
            for child in node.children(&mut cursor) {
                if child.kind() == "function_definition" || child.kind() == "class_definition" {
                    collect_python_symbols(child, source, symbols);
                }
            }
            return;
        }
        _ => {}
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_python_symbols(child, source, symbols);
    }
}

// =============================================================================
// Go symbol extraction
// =============================================================================

fn extract_go_symbols(root: Node, source: &str) -> Vec<FlatSymbol> {
    let mut symbols = Vec::new();
    collect_go_symbols(root, source, &mut symbols);
    symbols
}

fn collect_go_symbols(node: Node, source: &str, symbols: &mut Vec<FlatSymbol>) {
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

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_go_symbols(child, source, symbols);
    }
}

// =============================================================================
// Java symbol extraction
// =============================================================================

fn extract_java_symbols(root: Node, source: &str) -> Vec<FlatSymbol> {
    let mut symbols = Vec::new();
    collect_java_symbols(root, source, &mut symbols);
    symbols
}

fn collect_java_symbols(node: Node, source: &str, symbols: &mut Vec<FlatSymbol>) {
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

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_java_symbols(child, source, symbols);
    }
}

// =============================================================================
// PHP symbol extraction
// =============================================================================

fn extract_php_symbols(root: Node, source: &str) -> Vec<FlatSymbol> {
    let mut symbols = Vec::new();
    collect_php_symbols(root, source, &mut symbols);
    symbols
}

fn collect_php_symbols(node: Node, source: &str, symbols: &mut Vec<FlatSymbol>) {
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

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_php_symbols(child, source, symbols);
    }
}

// =============================================================================
// C/C++ symbol extraction
// =============================================================================

fn extract_c_cpp_symbols(root: Node, source: &str, language: LanguageId) -> Vec<FlatSymbol> {
    let mut symbols = Vec::new();
    collect_c_cpp_symbols(root, source, &mut symbols, language);
    symbols
}

fn collect_c_cpp_symbols(
    node: Node,
    source: &str,
    symbols: &mut Vec<FlatSymbol>,
    language: LanguageId,
) {
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

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_c_cpp_symbols(child, source, symbols, language);
    }
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
    collect_yaml_symbols(root, source, &mut symbols);
    symbols
}

fn collect_yaml_symbols(node: Node, source: &str, symbols: &mut Vec<FlatSymbol>) {
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

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_yaml_symbols(child, source, symbols);
    }
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
    collect_html_symbols(root, source, &mut symbols);
    symbols
}

fn collect_html_symbols(node: Node, source: &str, symbols: &mut Vec<FlatSymbol>) {
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

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_html_symbols(child, source, symbols);
    }
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
    collect_blade_symbols(root, source, &mut symbols);
    symbols
}

fn collect_blade_symbols(node: Node, source: &str, symbols: &mut Vec<FlatSymbol>) {
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
                    // Not structural — skip but still recurse into children
                    let mut cursor = node.walk();
                    for child in node.children(&mut cursor) {
                        collect_blade_symbols(child, source, symbols);
                    }
                    return;
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

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_blade_symbols(child, source, symbols);
    }
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
    collect_vue_symbols(root, source, &mut symbols);
    symbols
}

fn collect_vue_symbols(node: Node, source: &str, symbols: &mut Vec<FlatSymbol>) {
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

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_vue_symbols(child, source, symbols);
    }
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
}
