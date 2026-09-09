//! Folding extracted from the same immutable base and injected syntax trees.

use super::{LanguageId, SyntaxTreeSnapshot};
use crate::folding::{self, FoldCandidates, FoldRegion, FoldStamp};
use crate::util::text::TabStops;

#[derive(Debug, Clone, Copy)]
pub(crate) struct FoldingProfile(&'static [&'static str]);

pub(crate) const fn profile(language: LanguageId) -> FoldingProfile {
    FoldingProfile(match language {
        LanguageId::Rust => &[
            "function_item",
            "impl_item",
            "trait_item",
            "struct_item",
            "enum_item",
            "mod_item",
            "block",
            "match_block",
            "field_declaration_list",
            "enum_variant_list",
            "array_expression",
            "block_comment",
            "raw_string_literal",
        ],
        LanguageId::JavaScript | LanguageId::TypeScript | LanguageId::Jsx | LanguageId::Tsx => &[
            "function_declaration",
            "method_definition",
            "class_declaration",
            "interface_declaration",
            "statement_block",
            "class_body",
            "object",
            "object_type",
            "array",
            "switch_body",
            "comment",
            "template_string",
            "jsx_element",
        ],
        LanguageId::Python => &[
            "function_definition",
            "class_definition",
            "if_statement",
            "for_statement",
            "while_statement",
            "try_statement",
            "with_statement",
            "match_statement",
            "list",
            "dictionary",
            "set",
            "tuple",
            "string",
        ],
        LanguageId::Json => &["object", "array"],
        LanguageId::Yaml => &[
            "block_mapping_pair",
            "block_sequence_item",
            "flow_mapping",
            "flow_sequence",
            "block_scalar",
        ],
        LanguageId::Html => &["element", "script_element", "style_element", "comment"],
        LanguageId::Css => &[
            "rule_set",
            "media_statement",
            "keyframes_statement",
            "block",
            "comment",
        ],
        LanguageId::Markdown => &[
            "section",
            "fenced_code_block",
            "indented_code_block",
            "list",
            "block_quote",
            "html_block",
        ],
        _ => &[],
    })
}

pub fn detect(
    source: &str,
    stamp: FoldStamp,
    tabs: TabStops,
    tree: Option<&SyntaxTreeSnapshot>,
) -> FoldCandidates {
    let buffer = ropey::Rope::from_str(source);
    let mut regions = Vec::new();
    // Bound initial full scans and derived state independently of rendering.
    if source.len() <= crate::util::ByteSize::mebibytes(32).as_usize() {
        let supported = !super::registry::language(stamp.language)
            .folding
            .0
            .is_empty();
        if let Some(snapshot) = tree.filter(|tree| {
            supported
                && tree.language == stamp.language
                && tree.revision == stamp.revision
                && !tree.tree.root_node().has_error()
        }) {
            collect(
                &buffer,
                &snapshot.tree,
                snapshot.language,
                0..source.len(),
                &mut regions,
            );
            for injection in snapshot.injections() {
                collect(
                    &buffer,
                    &injection.tree,
                    injection.language,
                    injection.range.clone(),
                    &mut regions,
                );
            }
        } else {
            regions = folding::indentation(&buffer, tabs);
        }
    }
    FoldCandidates {
        stamp,
        regions: folding::normalize(regions, buffer.len_lines()),
        content_fingerprint: folding::digest(source.bytes()),
    }
}

fn collect(
    buffer: &ropey::Rope,
    tree: &tree_sitter::Tree,
    language: LanguageId,
    bounds: std::ops::Range<usize>,
    regions: &mut Vec<FoldRegion>,
) {
    let profile = super::registry::language(language).folding;
    if profile.0.is_empty() {
        return;
    }
    let root = tree.root_node();
    let mut cursor = tree.walk();
    loop {
        let node = cursor.node();
        if node != root
            && !node.is_error()
            && !node.is_missing()
            && profile.0.contains(&node.kind())
        {
            let start = node.start_byte().max(bounds.start).min(buffer.len_bytes());
            let end = node.end_byte().min(bounds.end).min(buffer.len_bytes());
            if start < end {
                let header = buffer.byte_to_line(start);
                let end_line = buffer.byte_to_line(end);
                let end_offset = buffer.byte_to_char(end);
                let remaining = buffer
                    .slice(end_offset..buffer.line_to_char((end_line + 1).min(buffer.len_lines())));
                let end_line = if remaining.chars().all(char::is_whitespace)
                    && end != buffer.line_to_byte(end_line)
                {
                    end_line + 1
                } else {
                    end_line
                };
                let kind = format!("{}:{}", language.display_name(), node.kind());
                regions.extend(folding::region(buffer, header, end_line, &kind));
            }
        }
        if regions.len() >= 200_000 {
            break;
        }
        if cursor.goto_first_child() {
            continue;
        }
        loop {
            if cursor.goto_next_sibling() {
                break;
            }
            if !cursor.goto_parent() {
                return;
            }
        }
    }
}
