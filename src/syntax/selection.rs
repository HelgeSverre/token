use std::collections::HashSet;
use std::ops::Range;

use tree_sitter::Node;

use super::LanguageId;
use crate::model::{Document, Position, Selection};

#[derive(Debug)]
struct BoundarySelectionProfile {
    completed_node_kinds: &'static [&'static str],
    implicit_element_node_kinds: &'static [&'static str],
    container_node_kinds: &'static [&'static str],
    opening_node_kinds: &'static [&'static str],
    closing_node_kinds: &'static [&'static str],
}

const XML_BOUNDARIES: BoundarySelectionProfile = BoundarySelectionProfile {
    completed_node_kinds: &["element", "EmptyElemTag"],
    implicit_element_node_kinds: &[],
    container_node_kinds: &["element"],
    opening_node_kinds: &["STag"],
    closing_node_kinds: &["ETag"],
};

const HTML_BOUNDARIES: BoundarySelectionProfile = BoundarySelectionProfile {
    completed_node_kinds: &[
        "element",
        "self_closing_tag",
        "script_element",
        "style_element",
    ],
    implicit_element_node_kinds: &["element"],
    container_node_kinds: &["element", "script_element", "style_element"],
    opening_node_kinds: &["start_tag"],
    closing_node_kinds: &["end_tag"],
};

const JSX_BOUNDARIES: BoundarySelectionProfile = BoundarySelectionProfile {
    completed_node_kinds: &["jsx_element", "jsx_self_closing_element"],
    implicit_element_node_kinds: &[],
    container_node_kinds: &["jsx_element"],
    opening_node_kinds: &["jsx_opening_element"],
    closing_node_kinds: &["jsx_closing_element"],
};

fn boundary_profile(language: LanguageId) -> Option<&'static BoundarySelectionProfile> {
    match language {
        LanguageId::Xml => Some(&XML_BOUNDARIES),
        LanguageId::Html | LanguageId::Vue | LanguageId::Svelte => Some(&HTML_BOUNDARIES),
        LanguageId::Jsx | LanguageId::Tsx => Some(&JSX_BOUNDARIES),
        _ => None,
    }
}

/// Immutable parse tree associated with one exact document revision.
#[derive(Debug, Clone)]
pub struct SyntaxTreeSnapshot {
    pub revision: u64,
    pub language: LanguageId,
    pub tree: tree_sitter::Tree,
}

impl SyntaxTreeSnapshot {
    pub fn new(revision: u64, language: LanguageId, tree: tree_sitter::Tree) -> Self {
        Self {
            revision,
            language,
            tree,
        }
    }
}

/// Return strict, increasingly large syntax ranges containing `selection`.
///
/// The caller is responsible for checking that the snapshot revision and
/// language match the document. The document root is omitted so the editor can
/// preserve its line-before-document fallback.
pub fn expansion_candidates(
    document: &Document,
    snapshot: &SyntaxTreeSnapshot,
    selection: Selection,
) -> Vec<Selection> {
    if snapshot.revision != document.revision || snapshot.language != document.language {
        return Vec::new();
    }

    let start_byte = position_to_byte(document, selection.start());
    let end_byte = position_to_byte(document, selection.end());
    let root = snapshot.tree.root_node();
    let Some(mut node) = root.named_descendant_for_byte_range(start_byte, end_byte) else {
        return Vec::new();
    };

    let preferred_boundary =
        completed_node_before_eol(document, root, snapshot.language, selection);
    let mut ranges = preferred_boundary.clone().into_iter().collect::<Vec<_>>();
    loop {
        if !node.is_missing() {
            if let Some(inner) = delimiter_interior(document, node) {
                ranges.push(inner);
            }
            if let Some(inner) = markup_element_interior(snapshot.language, node) {
                ranges.push(inner);
            }
            let range = normalized_node_range(document, snapshot.language, node);
            if node != root {
                ranges.push(range);
            }
            if let Some(attached) = rust_attached_item_range(document, snapshot.language, node) {
                ranges.push(attached);
            }
        }

        let Some(parent) = node.parent() else {
            break;
        };
        node = parent;
    }

    ranges.sort_by_key(|range| (range.end.saturating_sub(range.start), range.start));
    let mut seen = HashSet::new();
    let mut ranges: Vec<_> = ranges
        .into_iter()
        .filter(|range| {
            let is_preferred_boundary = preferred_boundary.as_ref() == Some(range);
            (is_preferred_boundary
                || (range.start <= start_byte
                    && range.end >= end_byte
                    && (range.start < start_byte || range.end > end_byte)))
                && range.start < range.end
                && range_contains_non_whitespace(document, range)
                && seen.insert((range.start, range.end))
        })
        .collect();
    if let Some(preferred) = preferred_boundary {
        if let Some(index) = ranges.iter().position(|range| *range == preferred) {
            ranges[..=index].rotate_right(1);
        }
    }
    ranges
        .into_iter()
        .map(|range| selection_from_byte_range(document, range.start, range.end, selection))
        .collect()
}

fn completed_node_before_eol(
    document: &Document,
    root: Node<'_>,
    language: LanguageId,
    selection: Selection,
) -> Option<Range<usize>> {
    let profile = boundary_profile(language)?;
    if !selection.is_empty() {
        return None;
    }

    let cursor = selection.head;
    if cursor.column != document.line_length(cursor.line) {
        return None;
    }
    let line_start_char = document.cursor_to_offset(cursor.line, 0);
    let line_start = document.buffer.char_to_byte(line_start_char);
    let cursor_char = document.cursor_to_offset(cursor.line, cursor.column);
    let mut boundary = document.buffer.char_to_byte(cursor_char);
    while boundary > line_start
        && matches!(document.buffer.get_byte(boundary - 1), Some(b' ' | b'\t'))
    {
        boundary -= 1;
    }
    if boundary == line_start {
        return None;
    }

    // Start on the final token (often an anonymous `>` node) and walk upward.
    // A named-only lookup may skip the closing tag and land in its container.
    let mut node = root.descendant_for_byte_range(boundary - 1, boundary)?;
    loop {
        if node.end_byte() == boundary && profile.completed_node_kinds.contains(&node.kind()) {
            return Some(node.start_byte()..node.end_byte());
        }
        if node.end_byte() == boundary && profile.opening_node_kinds.contains(&node.kind()) {
            if let Some(parent) = node.parent() {
                if let Some(range) = implicit_markup_element_range(profile, parent) {
                    return Some(range);
                }
            }
        }
        node = node.parent()?;
    }
}

fn markup_element_interior(language: LanguageId, node: Node<'_>) -> Option<Range<usize>> {
    let profile = boundary_profile(language)?;
    if !profile.container_node_kinds.contains(&node.kind()) {
        return None;
    }

    let mut cursor = node.walk();
    let mut opening_end = None;
    let mut closing_start = None;
    for child in node.named_children(&mut cursor) {
        if profile.opening_node_kinds.contains(&child.kind()) {
            opening_end = Some(child.end_byte());
        } else if profile.closing_node_kinds.contains(&child.kind()) {
            closing_start = Some(child.start_byte());
        }
    }
    let range = opening_end?..closing_start?;
    (range.start < range.end).then_some(range)
}

fn implicit_markup_element_range(
    profile: &BoundarySelectionProfile,
    node: Node<'_>,
) -> Option<Range<usize>> {
    if !profile.implicit_element_node_kinds.contains(&node.kind()) {
        return None;
    }

    let mut cursor = node.walk();
    let mut children = node.named_children(&mut cursor);
    let opening = children.next()?;
    if !profile.opening_node_kinds.contains(&opening.kind()) || children.next().is_some() {
        return None;
    }
    Some(node.start_byte()..opening.end_byte())
}

fn range_contains_non_whitespace(document: &Document, range: &Range<usize>) -> bool {
    document
        .buffer
        .byte_slice(range.clone())
        .chars()
        .any(|ch| !ch.is_whitespace())
}

fn normalized_node_range(
    document: &Document,
    language: LanguageId,
    node: Node<'_>,
) -> Range<usize> {
    let range = match language {
        LanguageId::Yaml => yaml_range_without_boundary_comments(node),
        LanguageId::Ini if node.kind() == "setting_value" => {
            trim_horizontal_whitespace(document, node.start_byte()..node.end_byte())
        }
        _ => node.start_byte()..node.end_byte(),
    };
    let range = boundary_profile(language)
        .and_then(|profile| implicit_markup_element_range(profile, node))
        .unwrap_or(range);

    // Some grammars include the structural line ending in a node while others
    // stop at the last content byte. Keep selection behavior consistent without
    // removing internal newlines or intentional blank lines.
    trim_trailing_line_ending(document, range)
}

fn yaml_range_without_boundary_comments(node: Node<'_>) -> Range<usize> {
    if node.kind() == "comment" {
        return node.start_byte()..node.end_byte();
    }

    yaml_semantic_start(node)..yaml_semantic_end(node)
}

fn yaml_semantic_start(node: Node<'_>) -> usize {
    let Some(mut child) = node.named_child(0) else {
        return node.start_byte();
    };

    while child.kind() == "comment" {
        let Some(next) = child.next_named_sibling() else {
            return node.end_byte();
        };
        child = next;
    }

    if child.start_byte() == node.start_byte() {
        yaml_semantic_start(child)
    } else {
        node.start_byte()
    }
}

fn yaml_semantic_end(node: Node<'_>) -> usize {
    let Some(mut child) = node.named_child(node.named_child_count().saturating_sub(1)) else {
        return node.end_byte();
    };

    while child.kind() == "comment" {
        let Some(previous) = child.prev_named_sibling() else {
            return node.start_byte();
        };
        child = previous;
    }

    if child.end_byte() == node.end_byte() {
        yaml_semantic_end(child)
    } else {
        child.end_byte()
    }
}

fn trim_horizontal_whitespace(document: &Document, range: Range<usize>) -> Range<usize> {
    let text = document.buffer.byte_slice(range.clone()).to_string();
    let trimmed_start = text.trim_start_matches([' ', '\t']);
    let leading = text.len() - trimmed_start.len();
    let trimmed = trimmed_start.trim_end_matches([' ', '\t']);
    range.start + leading..range.start + leading + trimmed.len()
}

fn trim_trailing_line_ending(document: &Document, range: Range<usize>) -> Range<usize> {
    if range.end <= range.start || document.buffer.get_byte(range.end - 1) != Some(b'\n') {
        return range;
    }

    let mut end = range.end - 1;
    if end > range.start && document.buffer.get_byte(end - 1) == Some(b'\r') {
        end -= 1;
    }
    range.start..end
}

fn rust_attached_item_range(
    document: &Document,
    language: LanguageId,
    node: Node<'_>,
) -> Option<Range<usize>> {
    if language != LanguageId::Rust || !is_rust_attributable_item(node.kind()) {
        return None;
    }

    let mut start = node.start_byte();
    let mut adjacent_start = start;
    let mut sibling = node.prev_named_sibling();
    let mut found_prefix = false;

    while let Some(prefix) = sibling {
        if !is_rust_attached_prefix(document, prefix)
            || has_blank_line(document, prefix.end_byte()..adjacent_start)
        {
            break;
        }
        found_prefix = true;
        start = prefix.start_byte();
        adjacent_start = start;
        sibling = prefix.prev_named_sibling();
    }

    found_prefix.then_some(start..node.end_byte())
}

fn is_rust_attributable_item(kind: &str) -> bool {
    kind.ends_with("_item") && !matches!(kind, "attribute_item" | "inner_attribute_item")
}

fn is_rust_attached_prefix(document: &Document, node: Node<'_>) -> bool {
    if node.kind() == "attribute_item" {
        return true;
    }
    if !matches!(node.kind(), "line_comment" | "block_comment") {
        return false;
    }

    let text = document
        .buffer
        .byte_slice(node.start_byte()..node.end_byte())
        .to_string();
    let trimmed = text.trim_start();
    trimmed.starts_with("///")
        || trimmed.starts_with("//!")
        || trimmed.starts_with("/**")
        || trimmed.starts_with("/*!")
}

fn has_blank_line(document: &Document, range: Range<usize>) -> bool {
    document
        .buffer
        .byte_slice(range)
        .chars()
        .filter(|ch| *ch == '\n')
        .count()
        > 1
}

fn delimiter_interior(document: &Document, node: Node<'_>) -> Option<std::ops::Range<usize>> {
    let start = node.start_byte();
    let end = node.end_byte();
    if end <= start + 1 || end > document.buffer.len_bytes() {
        return None;
    }

    let (inner_start, inner_end) = if is_string_node(node) {
        let text = document.buffer.byte_slice(start..end).to_string();
        string_interior(&text)?
    } else {
        let start_char = document.buffer.byte_to_char(start);
        let end_char = document.buffer.byte_to_char(end);
        let first = document.buffer.get_char(start_char)?;
        let last = document.buffer.get_char(end_char.checked_sub(1)?)?;
        if !matches!((first, last), ('(', ')') | ('[', ']') | ('{', '}')) {
            return None;
        }
        (
            first.len_utf8(),
            end.saturating_sub(start + last.len_utf8()),
        )
    };

    let absolute_start = start + inner_start;
    let absolute_end = start + inner_end;
    (absolute_start < absolute_end).then_some(absolute_start..absolute_end)
}

fn is_string_node(node: Node<'_>) -> bool {
    node.kind().contains("string")
}

fn string_interior(text: &str) -> Option<(usize, usize)> {
    let (opening, quote) = text
        .char_indices()
        .find(|(_, ch)| matches!(ch, '\'' | '"' | '`'))?;

    // C++ raw strings use R"delimiter(contents)delimiter". The delimiter may
    // be empty, so locate the opening parenthesis rather than stripping quotes.
    if quote == '"' && text[..opening].ends_with('R') {
        let after_quote = opening + quote.len_utf8();
        let open_paren = text[after_quote..].find('(')? + after_quote;
        let delimiter = &text[after_quote..open_paren];
        let closing_marker = format!("){delimiter}\"");
        let closing = text.rfind(&closing_marker)?;
        return (closing > open_paren).then_some((open_paren + 1, closing));
    }

    // Python and similar languages use repeated quote delimiters for multiline
    // strings. Match the whole opening run so `"""value"""` yields `value`.
    let quote_len = quote.len_utf8();
    let delimiter_len = text[opening..]
        .chars()
        .take_while(|ch| *ch == quote)
        .count()
        * quote_len;
    let delimiter = &text[opening..opening + delimiter_len];
    let closing = text.rfind(delimiter)?;
    (closing > opening).then_some((opening + delimiter_len, closing))
}

fn position_to_byte(document: &Document, position: Position) -> usize {
    let char_offset = document.cursor_to_offset(position.line, position.column);
    document.buffer.char_to_byte(char_offset)
}

fn byte_to_position(document: &Document, byte: usize) -> Position {
    let clamped = byte.min(document.buffer.len_bytes());
    let char_offset = document.buffer.byte_to_char(clamped);
    let (line, column) = document.offset_to_cursor(char_offset);
    Position::new(line, column)
}

fn selection_from_byte_range(
    document: &Document,
    start: usize,
    end: usize,
    current: Selection,
) -> Selection {
    let start = byte_to_position(document, start);
    let end = byte_to_position(document, end);
    if current.is_reversed() {
        Selection::from_anchor_head(end, start)
    } else {
        Selection::from_anchor_head(start, end)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::editor_area::DocumentId;
    use crate::syntax::ParserState;

    fn parsed_document(source: &str, language: LanguageId) -> (Document, SyntaxTreeSnapshot) {
        let mut document = Document::with_text(source);
        document.language = language;
        document.revision = 1;
        let mut parser = ParserState::new();
        let doc_id = DocumentId(99);
        parser.parse_and_highlight(source, language, doc_id, 1);
        let tree = parser
            .get_cached_tree(doc_id)
            .expect("test language should produce a tree")
            .0
            .clone();
        let snapshot = SyntaxTreeSnapshot::new(1, language, tree);
        (document, snapshot)
    }

    fn candidate_texts(source: &str, language: LanguageId, needle: &str) -> Vec<String> {
        let (document, snapshot) = parsed_document(source, language);
        let start = source.find(needle).expect("needle should exist");
        let start_char = document.buffer.byte_to_char(start);
        let end_char = document.buffer.byte_to_char(start + needle.len());
        let (start_line, start_column) = document.offset_to_cursor(start_char);
        let (end_line, end_column) = document.offset_to_cursor(end_char);
        let current = Selection::from_positions(
            Position::new(start_line, start_column),
            Position::new(end_line, end_column),
        );
        expansion_candidates(&document, &snapshot, current)
            .into_iter()
            .map(|selection| selection.get_text(&document))
            .collect()
    }

    fn candidate_texts_at_cursor(
        source: &str,
        language: LanguageId,
        line: usize,
        column: usize,
    ) -> Vec<String> {
        let (document, snapshot) = parsed_document(source, language);
        let current = Selection::new(Position::new(line, column));
        expansion_candidates(&document, &snapshot, current)
            .into_iter()
            .map(|selection| selection.get_text(&document))
            .collect()
    }

    #[test]
    fn rust_candidates_are_strict_and_monotonic() {
        let source = "fn main() { let value = calculate(foo.bar(), \"hello\"); }";
        let (document, snapshot) = parsed_document(source, LanguageId::Rust);
        let column = source.find("bar").expect("bar should exist");
        let current =
            Selection::from_positions(Position::new(0, column), Position::new(0, column + 3));

        let candidates = expansion_candidates(&document, &snapshot, current);
        assert!(!candidates.is_empty());
        assert!(candidates.iter().all(|candidate| {
            candidate.start() <= current.start()
                && candidate.end() >= current.end()
                && *candidate != current
        }));
        assert!(candidates
            .windows(2)
            .all(|pair| pair[0].start() >= pair[1].start() && pair[0].end() <= pair[1].end()));
    }

    #[test]
    fn string_has_inner_and_outer_candidates() {
        let source = "fn main() { let greeting = \"hello world\"; }";
        let (document, snapshot) = parsed_document(source, LanguageId::Rust);
        let hello = source.find("hello").expect("hello should exist");
        let current =
            Selection::from_positions(Position::new(0, hello), Position::new(0, hello + 5));
        let texts: Vec<_> = expansion_candidates(&document, &snapshot, current)
            .into_iter()
            .map(|selection| selection.get_text(&document))
            .collect();

        assert!(texts.iter().any(|text| text == "hello world"));
        assert!(texts.iter().any(|text| text == "\"hello world\""));
    }

    #[test]
    fn string_interior_handles_multicharacter_delimiters() {
        assert_eq!(string_interior("\"\"\"hello\"\"\""), Some((3, 8)));
        assert_eq!(string_interior("r###\"hello\"###"), Some((5, 10)));
        assert_eq!(string_interior("R\"tag(hello)tag\""), Some((6, 11)));
    }

    #[test]
    fn javascript_candidates_include_member_expression() {
        let texts = candidate_texts(
            "const value = user.profile.name;",
            LanguageId::JavaScript,
            "name",
        );
        assert!(texts.iter().any(|text| text == "user.profile.name"));
    }

    #[test]
    fn typescript_candidates_include_call_expression() {
        let texts = candidate_texts(
            "const value: string = format(user.name);",
            LanguageId::TypeScript,
            "name",
        );
        assert!(texts.iter().any(|text| text == "format(user.name)"));
    }

    #[test]
    fn python_candidates_include_call_expression() {
        let texts = candidate_texts("result = format(value)", LanguageId::Python, "value");
        assert!(texts.iter().any(|text| text == "format(value)"));
    }

    #[test]
    fn html_candidates_include_owning_element() {
        let texts = candidate_texts(
            "<main><span>content</span></main>",
            LanguageId::Html,
            "content",
        );
        assert!(texts.iter().any(|text| text == "<span>content</span>"));
    }

    #[test]
    fn xml_eol_prefers_completed_leaf_element() {
        let source =
            "<book>\n  <price currency=\"USD\">5.95</price>\n  <title>Next</title>\n</book>\n";
        let column = source
            .lines()
            .nth(1)
            .expect("fixture line should exist")
            .len();
        let texts = candidate_texts_at_cursor(source, LanguageId::Xml, 1, column);
        assert_eq!(
            texts.first().map(String::as_str),
            Some("<price currency=\"USD\">5.95</price>")
        );
    }

    #[test]
    fn xml_opening_tag_eol_keeps_right_affinity_to_content() {
        let source = "<book>\n  <price>5.95</price>\n</book>\n";
        let texts = candidate_texts_at_cursor(source, LanguageId::Xml, 0, 6);
        assert!(
            texts.first().is_some_and(|text| {
                text.starts_with('\n')
                    && text.contains("<price>5.95</price>")
                    && !text.contains("<book>")
            }),
            "{texts:?}"
        );
    }

    #[test]
    fn markup_eol_recovery_is_shared_by_html_vue_svelte_jsx_and_tsx() {
        for language in [
            LanguageId::Html,
            LanguageId::Vue,
            LanguageId::Svelte,
            LanguageId::Jsx,
            LanguageId::Tsx,
        ] {
            let source = "const view = <section>\n  <Price value=\"5.95\">Sale</Price>   \n  <Next />\n</section>;\n";
            let source = if matches!(
                language,
                LanguageId::Html | LanguageId::Vue | LanguageId::Svelte
            ) {
                "<section>\n  <p data-value=\"5.95\">Sale</p>   \n  <br>\n</section>\n"
            } else {
                source
            };
            let line = source.lines().nth(1).expect("fixture line should exist");
            let texts = candidate_texts_at_cursor(source, language, 1, line.chars().count());
            assert!(
                texts.first().is_some_and(|text| {
                    text == "<p data-value=\"5.95\">Sale</p>"
                        || text == "<Price value=\"5.95\">Sale</Price>"
                }),
                "{language:?}: {texts:?}"
            );
        }
    }

    #[test]
    fn markup_eol_recovery_handles_self_closing_and_void_elements() {
        for (language, source, expected) in [
            (LanguageId::Xml, "<root>\n  <next />\n</root>\n", "<next />"),
            (LanguageId::Html, "<main>\n  <br>\n</main>\n", "<br>"),
            (
                LanguageId::Html,
                "<main>\n  <img src=\"cover.png\">\n  <p>Caption</p>\n</main>\n",
                "<img src=\"cover.png\">",
            ),
            (
                LanguageId::Svelte,
                "<main>\n  <ProductCard />\n</main>\n",
                "<ProductCard />",
            ),
            (
                LanguageId::Tsx,
                "const view = <main>\n  <ProductCard />\n</main>;\n",
                "<ProductCard />",
            ),
        ] {
            let column = source
                .lines()
                .nth(1)
                .expect("fixture line should exist")
                .len();
            let texts = candidate_texts_at_cursor(source, language, 1, column);
            assert_eq!(
                texts.first().map(String::as_str),
                Some(expected),
                "{language:?}: {texts:?}"
            );
        }
    }

    #[test]
    fn html_void_elements_exclude_following_sibling_indentation() {
        let source = "<audio controls>\n    <source src=\"/audio.mp3\" type=\"audio/mpeg\">\n    <source src=\"/audio.ogg\" type=\"audio/ogg\">\n</audio>\n";
        for (line_number, expected) in [
            (1, "<source src=\"/audio.mp3\" type=\"audio/mpeg\">"),
            (2, "<source src=\"/audio.ogg\" type=\"audio/ogg\">"),
        ] {
            let column = source
                .lines()
                .nth(line_number)
                .expect("fixture line should exist")
                .len();
            let texts = candidate_texts_at_cursor(source, LanguageId::Html, line_number, column);
            assert_eq!(
                texts.first().map(String::as_str),
                Some(expected),
                "{texts:?}"
            );
        }
    }

    #[test]
    fn implicit_void_element_normalization_is_shared_by_component_markup() {
        for language in [LanguageId::Vue, LanguageId::Svelte] {
            let source = "<audio>\n  <source src=\"track.mp3\">   \n  <p>Fallback</p>\n</audio>\n";
            let column = source
                .lines()
                .nth(1)
                .expect("fixture line should exist")
                .len();
            let texts = candidate_texts_at_cursor(source, language, 1, column);
            assert_eq!(
                texts.first().map(String::as_str),
                Some("<source src=\"track.mp3\">"),
                "{language:?}: {texts:?}"
            );
        }
    }

    #[test]
    fn implicit_void_normalization_preserves_container_content_affinity() {
        let source = "<audio controls>\n  <source src=\"track.mp3\">\n</audio>\n";
        let texts = candidate_texts_at_cursor(source, LanguageId::Html, 0, 16);
        assert!(
            texts.first().is_some_and(|text| {
                text.starts_with('\n')
                    && text.contains("<source src=\"track.mp3\">")
                    && !text.contains("<audio")
            }),
            "{texts:?}"
        );

        let malformed = "<div>content\n<p>next</p>\n";
        let column = malformed
            .lines()
            .next()
            .expect("fixture line should exist")
            .len();
        let texts = candidate_texts_at_cursor(malformed, LanguageId::Html, 0, column);
        assert!(
            texts.iter().any(|text| text.contains("content")),
            "{texts:?}"
        );
    }

    #[test]
    fn jsx_opening_element_eol_expands_to_interior() {
        let source = "const view = <Panel>\n  <Child />\n</Panel>;\n";
        let column = source
            .lines()
            .next()
            .expect("fixture line should exist")
            .len();
        let texts = candidate_texts_at_cursor(source, LanguageId::Jsx, 0, column);
        assert!(
            texts.first().is_some_and(|text| {
                text.starts_with('\n') && text.contains("<Child />") && !text.contains("<Panel>")
            }),
            "{texts:?}"
        );
    }

    #[test]
    fn closing_tag_eol_prefers_complete_element() {
        let source = "<book>\n  <price>5.95</price>\n</book>\n";
        let texts = candidate_texts_at_cursor(source, LanguageId::Xml, 2, 7);
        assert_eq!(
            texts.first().map(String::as_str),
            Some("<book>\n  <price>5.95</price>\n</book>")
        );
    }

    #[test]
    fn boundary_recovery_does_not_cross_lines_or_apply_to_rust() {
        let xml = "<root>\n<price>5.95</price>\n   \n<next />\n</root>\n";
        let texts = candidate_texts_at_cursor(xml, LanguageId::Xml, 2, 3);
        assert_ne!(
            texts.first().map(String::as_str),
            Some("<price>5.95</price>")
        );

        let rust = "fn value() {\n    calculate();\n}\n";
        let (document, snapshot) = parsed_document(rust, LanguageId::Rust);
        let root = snapshot.tree.root_node();
        let selection = Selection::new(Position::new(1, 16));
        assert!(completed_node_before_eol(&document, root, LanguageId::Rust, selection).is_none());
    }

    #[test]
    fn markdown_candidates_include_heading_scope() {
        let texts = candidate_texts("# Semantic heading", LanguageId::Markdown, "Semantic");
        assert!(texts.iter().any(|text| text == "Semantic heading"));
    }

    #[test]
    fn yaml_nested_scopes_exclude_following_sibling_comments() {
        let source = "server:\n  host: localhost\n  port: 8080\n\n# Lists\nlanguages:\n  - rust\n";
        let texts = candidate_texts(source, LanguageId::Yaml, "localhost");

        assert!(texts
            .iter()
            .any(|text| text == "server:\n  host: localhost\n  port: 8080"));
        let server_scope = texts
            .iter()
            .filter(|text| text.starts_with("server:"))
            .min_by_key(|text| text.len())
            .expect("server scope should exist");
        assert!(!server_scope.contains("# Lists"), "{texts:?}");
    }

    #[test]
    fn yaml_direct_comment_selection_is_preserved() {
        let source = "server:\n  host: localhost\n\n# Lists\nlanguages:\n  - rust\n";
        let texts = candidate_texts(source, LanguageId::Yaml, "Lists");
        assert!(texts.iter().any(|text| text == "# Lists"));
    }

    #[test]
    fn ini_values_exclude_separator_whitespace() {
        let source =
            "[general]\nname = Token Editor\ncompact=two words\ntheme = \"dark\"\npath = /var/log/token.log\n";
        for (needle, expected) in [
            ("Token", "Token Editor"),
            ("two", "two words"),
            ("dark", "\"dark\""),
            ("log", "/var/log/token.log"),
        ] {
            let texts = candidate_texts(source, LanguageId::Ini, needle);
            assert!(texts.iter().any(|text| text == expected), "{texts:?}");
            assert!(texts.iter().all(|text| !text.starts_with(' ')));
        }
    }

    #[test]
    fn ini_candidates_exclude_trailing_line_endings() {
        for source in [
            "[general]\nname = Token Editor\nnext = value\n",
            "[general]\r\nname = Token Editor\r\nnext = value\r\n",
        ] {
            let texts = candidate_texts(source, LanguageId::Ini, "Token");
            assert!(texts.iter().any(|text| text == "name = Token Editor"));
            assert!(
                texts.iter().all(|text| !text.ends_with(['\r', '\n'])),
                "{texts:?}"
            );
        }
    }

    #[test]
    fn syntax_candidates_consistently_exclude_trailing_line_endings() {
        for (source, language, needle, expected) in [
            (
                "[general]\nname = \"Token Editor\"\nnext = \"value\"\n",
                LanguageId::Toml,
                "Token",
                "name = \"Token Editor\"",
            ),
            (
                "name: Token Editor\nnext: value\n",
                LanguageId::Yaml,
                "Token",
                "name: Token Editor",
            ),
            (
                "name = 'Token Editor'\nnext = 'value'\n",
                LanguageId::Python,
                "Token",
                "name = 'Token Editor'",
            ),
            (
                "# Token Editor\n\nNext paragraph.\n",
                LanguageId::Markdown,
                "Token",
                "# Token Editor",
            ),
        ] {
            let texts = candidate_texts(source, language, needle);
            assert!(texts.iter().any(|text| text == expected), "{texts:?}");
            assert!(
                texts.iter().all(|text| !text.ends_with(['\r', '\n'])),
                "{texts:?}"
            );
        }
    }

    #[test]
    fn trailing_line_ending_normalization_removes_only_one_ending() {
        let document = Document::with_text("value\n\n");
        assert_eq!(trim_trailing_line_ending(&document, 0..7), 0..6);

        let document = Document::with_text("value\r\n\r\n");
        assert_eq!(trim_trailing_line_ending(&document, 0..9), 0..7);
    }

    #[test]
    fn normalized_document_root_remains_omitted() {
        let source = "first = 'Token Editor'\nsecond = 'value'\n";
        let texts = candidate_texts(source, LanguageId::Python, "Token");
        assert!(
            !texts.iter().any(|text| text.contains("second =")),
            "{texts:?}"
        );
    }

    #[test]
    fn rust_item_candidate_includes_attached_docs_and_attributes() {
        let source = "mod tests {\n    /// Verifies behavior.\n    #[test]\n    #[should_panic]\n    fn verifies_behavior() { panic!(); }\n}\n";
        let texts = candidate_texts(source, LanguageId::Rust, "verifies_behavior");
        assert!(texts
            .iter()
            .any(|text| text.starts_with("fn verifies_behavior")));
        assert!(texts.iter().any(|text| {
            text.starts_with("/// Verifies behavior.\n    #[test]\n    #[should_panic]\n    fn verifies_behavior")
        }));
    }

    #[test]
    fn rust_item_candidate_stops_at_blank_line_and_ordinary_comment() {
        let source = "/// Unattached docs.\n\n// setup note\n#[test]\nfn verifies_behavior() {}\n";
        let texts = candidate_texts(source, LanguageId::Rust, "verifies_behavior");
        let attributed = texts
            .iter()
            .find(|text| text.starts_with("#[test]"))
            .expect("attribute and function candidate should exist");
        assert!(!attributed.contains("setup note"));
        assert!(!attributed.contains("Unattached docs"));
    }

    #[test]
    fn stale_snapshot_has_no_candidates() {
        let (mut document, snapshot) = parsed_document("fn main() {}", LanguageId::Rust);
        document.revision += 1;
        assert!(
            expansion_candidates(&document, &snapshot, Selection::new(Position::new(0, 3)))
                .is_empty()
        );
    }

    #[test]
    fn candidates_preserve_reversed_selection_direction() {
        let source = "fn main() { foo.bar(); }";
        let (document, snapshot) = parsed_document(source, LanguageId::Rust);
        let bar = source.find("bar").expect("bar should exist");
        let current = Selection::from_anchor_head(Position::new(0, bar + 3), Position::new(0, bar));

        let candidates = expansion_candidates(&document, &snapshot, current);
        assert!(!candidates.is_empty());
        assert!(candidates.iter().all(Selection::is_reversed));
    }

    #[test]
    fn unicode_before_node_uses_character_columns() {
        let source = "fn main() { let café = value; }";
        let (document, snapshot) = parsed_document(source, LanguageId::Rust);
        let char_column = source[..source.find("value").expect("value should exist")]
            .chars()
            .count();
        let current = Selection::from_positions(
            Position::new(0, char_column),
            Position::new(0, char_column + 5),
        );
        assert!(!expansion_candidates(&document, &snapshot, current).is_empty());
    }
}
