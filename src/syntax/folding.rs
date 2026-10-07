//! Folding extracted from the same immutable base and injected syntax trees.

use super::{LanguageId, SyntaxTreeSnapshot};
use crate::folding::{self, FoldCandidates, FoldRegion, FoldStamp};
use crate::util::text::TabStops;

pub const MAX_FOLD_SCAN_SIZE: crate::util::ByteSize = crate::util::ByteSize::mebibytes(256);

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
        LanguageId::Sema => &[
            "list",
            "short_lambda",
            "vector",
            "hash_map",
            "byte_vector",
            "block_comment",
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
    source: &ropey::Rope,
    stamp: FoldStamp,
    tabs: TabStops,
    tree: Option<&SyntaxTreeSnapshot>,
) -> FoldCandidates {
    // No candidates means there is nothing to restore from a fingerprint.
    // Avoid hashing a document whose scan is disabled.
    if source.len_bytes() > MAX_FOLD_SCAN_SIZE.as_usize() {
        return FoldCandidates {
            stamp,
            regions: Vec::new(),
            content_fingerprint: 0,
        };
    }
    let mut regions = Vec::new();
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
            source,
            &snapshot.tree,
            snapshot.language,
            0..source.len_bytes(),
            &mut regions,
        );
        for injection in snapshot.injections() {
            collect(
                source,
                &injection.tree,
                injection.language,
                injection.range.clone(),
                &mut regions,
            );
        }
    } else {
        regions = folding::indentation(source, tabs);
    }
    FoldCandidates {
        stamp,
        regions: folding::normalize(regions, source.len_lines()),
        content_fingerprint: folding::digest(source.chunks().flat_map(str::bytes)),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folding_rope_preserves_unicode_crlf_regions_and_fingerprint_across_chunks() {
        let text = format!("root\n  {}\r\nlast\n", "æ".repeat(1024));
        let source = ropey::Rope::from_str(&text);
        assert!(source.chunks().count() > 1);
        let candidates = detect(
            &source,
            FoldStamp {
                revision: 7,
                language: LanguageId::PlainText,
                policy_generation: 3,
            },
            TabStops::default(),
            None,
        );
        assert_eq!(
            candidates.content_fingerprint,
            folding::digest(text.bytes())
        );
        assert_eq!(candidates.regions.len(), 1);
        assert_eq!(
            (candidates.regions[0].header, candidates.regions[0].end),
            (0, 2)
        );
    }

    #[test]
    fn folding_budget_includes_boundary_and_skips_oversized_fingerprint() {
        let stamp = FoldStamp {
            revision: 1,
            language: LanguageId::PlainText,
            policy_generation: 0,
        };
        let mut source = "root\n  child\n".to_owned();
        source.push_str(&"x".repeat(MAX_FOLD_SCAN_SIZE.as_usize() - source.len()));
        let mut source = ropey::Rope::from_str(&source);
        let candidates = detect(&source, stamp, TabStops::default(), None);
        assert_eq!(candidates.regions.len(), 1);
        assert_eq!(
            (candidates.regions[0].header, candidates.regions[0].end),
            (0, 2)
        );
        assert_ne!(candidates.content_fingerprint, 0);
        source.insert(source.len_chars(), "x");
        let skipped = detect(&source, stamp, TabStops::default(), None);
        assert!(skipped.regions.is_empty());
        assert_eq!(skipped.content_fingerprint, 0);
        assert_eq!(skipped.stamp, stamp);
    }
}
