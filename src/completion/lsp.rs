//! LSP completion items -> [`MenuItem`] conversion (lsp-integration.md
//! Phase 5). Pure — no server handles, no model access — so the runtime's
//! interception pass can call it and unit tests can drive it directly.

use lsp_types::CompletionItemKind;

use super::menu::{LspInsert, MenuInsert, MenuItem, MenuItemKind, MenuSourceId};
use crate::lsp::LspServerId;
use crate::model::StyledText;

/// Cap on items kept from one server response. ts-ls routinely returns
/// hundreds; a pathological server returning tens of thousands would make
/// every refilter pass visible. rust-analyzer's full member lists sit well
/// under this.
const MAX_LSP_ITEMS: usize = 1000;

/// Converts one server response using its `completionProvider` snapshot.
/// Resolve support and inherited commit characters are derived together at this
/// boundary; update handlers never need to consult a server handle.
pub fn items_to_menu_items(
    items: Vec<lsp_types::CompletionItem>,
    server_id: &LspServerId,
    root: &std::path::Path,
    options: Option<&lsp_types::CompletionOptions>,
) -> Vec<MenuItem> {
    let can_resolve = options.is_some_and(|options| options.resolve_provider == Some(true));
    let default_commit_characters = normalize_commit_characters(
        options
            .and_then(|options| options.all_commit_characters.as_deref())
            .unwrap_or_default(),
    );
    items
        .into_iter()
        .take(MAX_LSP_ITEMS)
        .filter_map(|item| {
            completion_item_to_menu_item(
                item,
                server_id,
                root,
                can_resolve,
                &default_commit_characters,
            )
        })
        .collect()
}

/// Commit characters are a set, not strings to paste. Ignore malformed entries
/// and share the normalized server default across all items that inherit it.
fn normalize_commit_characters(characters: &[String]) -> std::sync::Arc<[char]> {
    let mut result: Vec<_> = characters
        .iter()
        .filter_map(|text| {
            let mut chars = text.chars();
            let ch = chars.next()?;
            chars.next().is_none().then_some(ch)
        })
        .collect();
    result.sort_unstable();
    result.dedup();
    result.into()
}

/// Per-item conversion. Returns `None` for items with neither a usable
/// label nor any insertable text (`label` is technically optional when
/// `textEdit`/`insertText` carry everything, but an item we can't name or
/// insert has nothing to offer the menu).
fn completion_item_to_menu_item(
    item: lsp_types::CompletionItem,
    server_id: &LspServerId,
    root: &std::path::Path,
    can_resolve: bool,
    default_commit_characters: &std::sync::Arc<[char]>,
) -> Option<MenuItem> {
    // Keep the original typed item intact for resolve. Building JSON for every
    // candidate duplicates its fields and opaque data before any row is chosen.
    let raw = std::sync::Arc::new(item);
    let item = raw.as_ref();
    let detail = item_detail(item);
    let preselect = item.preselect.unwrap_or(false);
    // An explicit empty item list opts out of all server defaults. Do not union
    // the two sets or insert inherited fields into the raw resolve payload.
    let commit_characters = item
        .commit_characters
        .as_deref()
        .map(normalize_commit_characters)
        .unwrap_or_else(|| default_commit_characters.clone());

    let mut text_edit = match item.text_edit.as_ref() {
        Some(lsp_types::CompletionTextEdit::Edit(edit)) => {
            Some((edit.range, edit.new_text.clone()))
        }
        // InsertAndReplace (3.16) — we advertise no support for it; use
        // the insert half rather than dropping the item outright.
        Some(lsp_types::CompletionTextEdit::InsertAndReplace(edit)) => {
            Some((edit.insert, edit.new_text.clone()))
        }
        None => None,
    };

    let lsp_types::CompletionItem {
        label,
        filter_text,
        insert_text_format,
        kind,
        sort_text,
        documentation,
        additional_text_edits,
        ..
    } = item;
    let mut insert_text = item.insert_text.clone();

    // We advertise `snippetSupport: false`, but rust-analyzer still sends
    // snippet bodies. Insert readable text; the caret lands at `$0`. The
    // primary text (`textEdit` if present) decides the caret.
    let mut caret_offset = None;
    if *insert_text_format == Some(lsp_types::InsertTextFormat::SNIPPET) {
        if let Some(text) = insert_text.as_mut() {
            (*text, caret_offset) = strip_snippet(text);
        }
        if let Some((_, text)) = text_edit.as_mut() {
            (*text, caret_offset) = strip_snippet(text);
        }
    }

    let text = insert_text.or_else(|| text_edit.as_ref().map(|(_, new_text)| new_text.clone()));
    if label.is_empty() && text.is_none() {
        return None;
    }

    // autocomplete.md Phase 5 rule: matching runs against
    // `filterText ?? label` — never bare `label` alone (rust-analyzer
    // labels embed type signatures; matching them ranks visibly wrong).
    let filter_text = filter_text.clone().unwrap_or_else(|| label.clone());
    // `insertText ?? textEdit.newText ?? label`.
    let plain_text = text.unwrap_or_else(|| label.clone());

    Some(MenuItem {
        label: label.clone(),
        filter_text,
        insert: MenuInsert::Lsp(Box::new(LspInsert {
            text: plain_text,
            server_id: server_id.clone(),
            root: root.to_path_buf(),
            raw: std::sync::Arc::clone(&raw),
            can_resolve,
            resolved: false,
            text_edit,
            additional_text_edits: additional_text_edits
                .as_deref()
                .unwrap_or_default()
                .iter()
                .map(|edit| (edit.range, edit.new_text.clone()))
                .collect(),
            commit_characters,
            caret_offset,
            documentation: documentation.as_ref().and_then(documentation_to_styled),
        })),
        kind: map_kind(*kind),
        source: MenuSourceId::Lsp,
        detail,
        sort_text: sort_text.clone(),
        preselect,
    })
}

/// Prefer structured label metadata (parameters and return type/owner), without
/// changing the insertable label or its filter-text character coordinates.
pub fn item_detail(item: &lsp_types::CompletionItem) -> Option<String> {
    fn nonempty(text: Option<&str>) -> Option<&str> {
        text.map(str::trim).filter(|text| !text.is_empty())
    }
    let parameters = item
        .label_details
        .as_ref()
        .and_then(|details| nonempty(details.detail.as_deref()));
    let description = item
        .label_details
        .as_ref()
        .and_then(|details| nonempty(details.description.as_deref()));
    match parameters {
        Some(parameters) => Some(match description {
            Some(description) => format!("{parameters} {description}"),
            None => parameters.to_owned(),
        }),
        // A return type/owner alone must not hide an available full signature.
        None => nonempty(item.detail.as_deref())
            .or(description)
            .map(str::to_owned),
    }
}

/// Flattens an LSP snippet body to plain text: `$n`/`${n}` vanish,
/// `${n:text}` keeps `text` (nested placeholders stripped), `${n|a,b|}`
/// keeps the first choice, `\$` `\}` `\\` unescape. Returns the char
/// offset (in the output) of the first `$0`, if any. Malformed input is
/// passed through verbatim — never panics, never drops text.
pub(crate) fn strip_snippet(body: &str) -> (String, Option<usize>) {
    let chars: Vec<char> = body.chars().collect();
    let mut out = String::with_capacity(body.len());
    let mut caret = None;
    strip_into(&chars, &mut out, &mut caret);
    (out, caret)
}

fn strip_into(chars: &[char], out: &mut String, caret: &mut Option<usize>) {
    let mut i = 0;
    while i < chars.len() {
        let next = chars.get(i + 1).copied();
        match (chars[i], next) {
            ('\\', Some(c @ ('$' | '}' | '\\'))) => {
                out.push(c);
                i += 2;
            }
            ('$', Some(d)) if d.is_ascii_digit() => {
                let end = digits_end(chars, i + 1);
                if is_zero(&chars[i + 1..end]) {
                    caret.get_or_insert_with(|| out.chars().count());
                }
                i = end;
            }
            ('$', Some('{')) => match braced_end(chars, i + 2) {
                Some(close) if strip_placeholder(&chars[i + 2..close], out, caret) => {
                    i = close + 1;
                }
                _ => {
                    out.push('$');
                    i += 1;
                }
            },
            (c, _) => {
                out.push(c);
                i += 1;
            }
        }
    }
}

/// Body between `${` and its `}`: `n`, `n:text`, or `n|a,b|`. Returns
/// `false` when it isn't a numbered placeholder so the caller can pass the
/// text through verbatim.
fn strip_placeholder(inner: &[char], out: &mut String, caret: &mut Option<usize>) -> bool {
    let end = digits_end(inner, 0);
    if end == 0 {
        return false;
    }
    match inner.get(end) {
        None => {
            if is_zero(&inner[..end]) {
                caret.get_or_insert_with(|| out.chars().count());
            }
        }
        Some(':') => strip_into(&inner[end + 1..], out, caret),
        Some('|') => {
            let first: String = inner[end + 1..]
                .iter()
                .take_while(|c| !matches!(c, ',' | '|'))
                .collect();
            out.push_str(&first);
        }
        Some(_) => return false,
    }
    true
}

fn digits_end(chars: &[char], start: usize) -> usize {
    start
        + chars[start..]
            .iter()
            .take_while(|c| c.is_ascii_digit())
            .count()
}

fn is_zero(digits: &[char]) -> bool {
    digits.iter().all(|c| *c == '0')
}

/// Index of the `}` closing a `${` whose body starts at `start`, honouring
/// nesting and backslash escapes. `None` when unterminated.
fn braced_end(chars: &[char], start: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut i = start;
    while i < chars.len() {
        match chars[i] {
            '\\' => i += 1,
            '{' => depth += 1,
            '}' if depth == 0 => return Some(i),
            '}' => depth -= 1,
            _ => {}
        }
        i += 1;
    }
    None
}

/// Reduces `completionItem.documentation` to styled text the way the hover
/// card does (`MarkupContent` per its `kind`; a bare string is plaintext
/// per the spec). `None` when empty after trimming.
pub fn documentation_to_styled(doc: &lsp_types::Documentation) -> Option<StyledText> {
    let text = match doc {
        lsp_types::Documentation::String(s) => StyledText::plain(s.clone()),
        lsp_types::Documentation::MarkupContent(markup) => match markup.kind {
            lsp_types::MarkupKind::PlainText => StyledText::plain(markup.value.clone()),
            lsp_types::MarkupKind::Markdown => {
                crate::lsp::markdown::markdown_to_styled(&markup.value)
            }
        },
    };
    (!text.text.trim().is_empty()).then_some(text)
}

/// Maps the (open-ended) `CompletionItemKind` enum onto the menu's coarse
/// badge kinds. Unlisted kinds fall into `Other` (`?` badge).
fn map_kind(kind: Option<CompletionItemKind>) -> MenuItemKind {
    use CompletionItemKind as K;
    match kind {
        Some(K::METHOD) => MenuItemKind::Method,
        Some(K::FUNCTION | K::CONSTRUCTOR | K::EVENT | K::OPERATOR) => MenuItemKind::Function,
        Some(K::VARIABLE | K::VALUE | K::ENUM_MEMBER | K::TEXT) => MenuItemKind::Variable,
        Some(
            K::STRUCT | K::CLASS | K::ENUM | K::INTERFACE | K::TYPE_PARAMETER | K::UNIT | K::COLOR,
        ) => MenuItemKind::Type,
        Some(K::MODULE | K::REFERENCE) => MenuItemKind::Module,
        Some(K::FILE) => MenuItemKind::File,
        Some(K::FOLDER) => MenuItemKind::Folder,
        Some(K::KEYWORD) => MenuItemKind::Keyword,
        Some(K::FIELD | K::PROPERTY | K::SNIPPET) => MenuItemKind::Field,
        Some(K::CONSTANT) => MenuItemKind::Constant,
        _ => MenuItemKind::Other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn server() -> (LspServerId, std::path::PathBuf) {
        (
            LspServerId::from("rust-analyzer"),
            std::path::PathBuf::from("/tmp/proj"),
        )
    }

    fn convert(item: lsp_types::CompletionItem) -> Option<MenuItem> {
        let (id, root) = server();
        let options = lsp_types::CompletionOptions {
            resolve_provider: Some(true),
            ..Default::default()
        };
        items_to_menu_items(vec![item], &id, &root, Some(&options))
            .into_iter()
            .next()
    }

    fn base_item(label: &str) -> lsp_types::CompletionItem {
        lsp_types::CompletionItem {
            label: label.to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn filter_text_prefers_filtertext_over_label() {
        let mut item = base_item("push(self: &mut Vec)");
        item.filter_text = Some("push".to_owned());
        let menu_item = convert(item).unwrap();
        assert_eq!(menu_item.filter_text, "push");
        assert_eq!(menu_item.label, "push(self: &mut Vec)");
    }

    #[test]
    fn structured_method_metadata_and_preselection_survive_conversion() {
        let mut item = base_item("compile");
        item.label_details = Some(lsp_types::CompletionItemLabelDetails {
            detail: Some("(output: &str)".into()),
            description: Some("()".into()),
        });
        item.detail = Some("legacy detail".into());
        item.preselect = Some(true);
        item.kind = Some(CompletionItemKind::METHOD);
        let result = convert(item).unwrap();
        assert_eq!(result.label, "compile");
        assert_eq!(result.filter_text, "compile");
        assert_eq!(result.detail.as_deref(), Some("(output: &str) ()"));
        assert_eq!(result.kind, MenuItemKind::Method);
        assert!(result.preselect);
        let MenuInsert::Lsp(data) = result.insert else {
            panic!("LSP item")
        };
        assert_eq!(data.text, "compile");
    }

    #[test]
    fn empty_label_details_fall_back_to_legacy_detail() {
        let mut item = base_item("compile");
        item.label_details = Some(lsp_types::CompletionItemLabelDetails::default());
        item.detail = Some("fn compile(&self, output: &str)".into());
        assert_eq!(item_detail(&item), item.detail);
        item.label_details.as_mut().unwrap().description = Some("cc::Build".into());
        assert_eq!(
            item_detail(&item),
            item.detail,
            "an owner alone must not hide the signature"
        );
    }

    #[test]
    fn filter_text_falls_back_to_label_not_insert_text() {
        // Spec: `filterText` defaults to `label`. Filtering against a
        // `textEdit.newText` like `self.foo` would lose the word-start
        // tier for the query `fo`.
        let mut item = base_item("foo");
        item.insert_text = Some("self.foo".to_owned());
        assert_eq!(convert(item).unwrap().filter_text, "foo");
    }

    #[test]
    fn insert_text_falls_back_through_inserttext_textedit_then_label() {
        let menu_item = convert(base_item("foo")).unwrap();
        let MenuInsert::Lsp(insert) = &menu_item.insert else {
            panic!("expected LSP insert");
        };
        assert_eq!(insert.text, "foo");

        let mut with_insert = base_item("foo");
        with_insert.insert_text = Some("foobar".to_owned());
        let MenuInsert::Lsp(insert) = &convert(with_insert).unwrap().insert else {
            panic!("expected LSP insert");
        };
        assert_eq!(insert.text, "foobar");

        let mut with_edit = base_item("self.push");
        with_edit.text_edit = Some(lsp_types::CompletionTextEdit::Edit(lsp_types::TextEdit {
            range: lsp_types::Range::new(
                lsp_types::Position::new(0, 0),
                lsp_types::Position::new(0, 0),
            ),
            new_text: "self.push(x)".to_owned(),
        }));
        let menu_item = convert(with_edit).unwrap();
        let MenuInsert::Lsp(insert) = &menu_item.insert else {
            panic!("expected LSP insert");
        };
        assert_eq!(insert.text, "self.push(x)");
        assert_eq!(insert.text_edit.as_ref().unwrap().1, "self.push(x)");
    }

    #[test]
    fn raw_round_trips_the_full_item_including_data() {
        // `completionItem/resolve` must receive the same item back —
        // ts-ls keys auto-import edits off the opaque `data` field.
        let mut item = base_item("imported_fn");
        item.data = Some(serde_json::json!({ "autoImport": true }));
        let menu_item = convert(item).unwrap();
        let MenuInsert::Lsp(insert) = &menu_item.insert else {
            panic!("expected LSP insert");
        };
        let wire = serde_json::to_value(insert.raw.as_ref()).unwrap();
        assert_eq!(wire["data"]["autoImport"], serde_json::json!(true));
        assert_eq!(wire["label"], serde_json::json!("imported_fn"));
    }

    #[test]
    fn completion_metadata_item_commits_override_defaults_including_explicit_empty() {
        let options = lsp_types::CompletionOptions {
            resolve_provider: Some(true),
            all_commit_characters: Some(vec![".".into(), "(".into()]),
            ..Default::default()
        };
        let mut own = base_item("own");
        own.commit_characters = Some(vec![";".into()]);
        let mut empty = base_item("empty");
        empty.commit_characters = Some(vec![]);
        let (id, root) = server();
        let items = items_to_menu_items(
            vec![base_item("inherited"), own, empty, base_item("shared")],
            &id,
            &root,
            Some(&options),
        );
        let inserts: Vec<_> = items
            .iter()
            .map(|item| {
                let MenuInsert::Lsp(data) = &item.insert else {
                    panic!("LSP insert")
                };
                assert!(data.can_resolve);
                data
            })
            .collect();
        assert_eq!(inserts[0].commit_characters.as_ref(), &['(', '.']);
        assert_eq!(inserts[1].commit_characters.as_ref(), &[';']);
        assert!(inserts[2].commit_characters.is_empty());
        assert!(std::sync::Arc::ptr_eq(
            &inserts[0].commit_characters,
            &inserts[3].commit_characters
        ));
        assert!(
            inserts[0].raw.commit_characters.is_none(),
            "resolve payload must remain the original item"
        );
        assert_eq!(
            inserts[2].raw.commit_characters.as_deref(),
            Some([].as_slice())
        );
    }

    #[test]
    fn completion_metadata_rejects_non_character_entries_without_inheriting_overrides() {
        let mut item = base_item("item");
        item.commit_characters = Some(
            ["", "::", "e\u{301}", "é", "🦀", ".", "."]
                .into_iter()
                .map(str::to_owned)
                .collect(),
        );
        let MenuInsert::Lsp(data) = convert(item).unwrap().insert else {
            panic!("LSP insert")
        };
        assert_eq!(data.commit_characters.as_ref(), &['.', 'é', '🦀']);
        let options = lsp_types::CompletionOptions {
            all_commit_characters: Some(vec![".".into()]),
            ..Default::default()
        };
        let mut invalid_override = base_item("invalid");
        invalid_override.commit_characters = Some(vec!["::".into()]);
        let (id, root) = server();
        let items = items_to_menu_items(vec![invalid_override], &id, &root, Some(&options));
        let MenuInsert::Lsp(data) = &items[0].insert else {
            panic!("LSP insert")
        };
        assert!(data.commit_characters.is_empty());
    }

    #[test]
    fn completion_metadata_missing_options_never_guess_commit_characters_or_resolve_support() {
        let (id, root) = server();
        for options in [None, Some(lsp_types::CompletionOptions::default())] {
            let items = items_to_menu_items(vec![base_item("item")], &id, &root, options.as_ref());
            let MenuInsert::Lsp(data) = &items[0].insert else {
                panic!("LSP insert")
            };
            assert!(!data.can_resolve);
            assert!(data.commit_characters.is_empty());
        }
    }

    #[test]
    fn completion_metadata_keeps_upfront_edits_literal_and_raw_payload_unchanged() {
        let item: lsp_types::CompletionItem = serde_json::from_value(serde_json::json!({
            "label": "method", "insertText": "method($0)", "insertTextFormat": 2,
            "commitCharacters": [";", ";", "::"], "data": {"opaque": 7},
            "additionalTextEdits": [
                {"range": {"start": {"line": 0, "character": 2}, "end": {"line": 0, "character": 2}}, "newText": "$0"},
                {"range": {"start": {"line": 1, "character": 3}, "end": {"line": 1, "character": 5}}, "newText": "é"}
            ]
        })).unwrap();
        let expected_raw = serde_json::to_value(&item).unwrap();
        let expected_edits: Vec<_> = item
            .additional_text_edits
            .as_ref()
            .unwrap()
            .iter()
            .map(|edit| (edit.range, edit.new_text.clone()))
            .collect();
        let MenuInsert::Lsp(data) = convert(item).unwrap().insert else {
            panic!("LSP insert")
        };
        assert_eq!(
            serde_json::to_value(data.raw.as_ref()).unwrap(),
            expected_raw
        );
        assert_eq!(data.additional_text_edits, expected_edits);
        assert_eq!(data.text, "method()");
        assert_eq!(data.caret_offset, Some(7));
        assert_eq!(data.commit_characters.as_ref(), &[';']);
        let copy = data.clone();
        assert!(std::sync::Arc::ptr_eq(&data.raw, &copy.raw));
        assert!(std::sync::Arc::ptr_eq(
            &data.commit_characters,
            &copy.commit_characters
        ));
    }

    #[test]
    fn documentation_is_flattened_to_plaintext() {
        let mut item = base_item("f");
        item.documentation = Some(lsp_types::Documentation::MarkupContent(
            lsp_types::MarkupContent {
                kind: lsp_types::MarkupKind::Markdown,
                value: "**Bold** doc\n```rust\nfn f()\n```".to_owned(),
            },
        ));
        let MenuInsert::Lsp(insert) = convert(item).unwrap().insert else {
            panic!("expected LSP insert");
        };
        assert_eq!(
            insert.documentation.as_ref().map(|t| t.text.as_str()),
            Some("Bold doc\nfn f()")
        );

        let mut empty = base_item("g");
        empty.documentation = Some(lsp_types::Documentation::String("  ".to_owned()));
        let MenuInsert::Lsp(insert) = convert(empty).unwrap().insert else {
            panic!("expected LSP insert");
        };
        assert!(insert.documentation.is_none());
    }

    #[test]
    fn kinds_map_onto_badge_kinds() {
        let mut item = base_item("f");
        item.kind = Some(CompletionItemKind::METHOD);
        assert_eq!(convert(item).unwrap().kind, MenuItemKind::Method);

        let mut item = base_item("f");
        item.kind = Some(CompletionItemKind::FUNCTION);
        assert_eq!(convert(item).unwrap().kind, MenuItemKind::Function);

        let mut item = base_item("c");
        item.kind = Some(CompletionItemKind::CONSTANT);
        assert_eq!(convert(item).unwrap().kind, MenuItemKind::Constant);
    }

    #[test]
    fn items_without_label_or_text_are_dropped() {
        let item = lsp_types::CompletionItem::default();
        assert!(convert(item).is_none());
    }

    #[test]
    fn snippet_items_are_stripped_and_carry_the_caret() {
        let mut item = base_item("vec!");
        item.insert_text = Some("vec![$0]".to_owned());
        item.insert_text_format = Some(lsp_types::InsertTextFormat::SNIPPET);
        let MenuInsert::Lsp(insert) = convert(item).unwrap().insert else {
            panic!("expected LSP insert");
        };
        assert_eq!(insert.text, "vec![]");
        assert_eq!(insert.caret_offset, Some(5));

        let mut plain = base_item("price");
        plain.insert_text = Some("$price".to_owned());
        plain.insert_text_format = Some(lsp_types::InsertTextFormat::PLAIN_TEXT);
        let MenuInsert::Lsp(insert) = convert(plain).unwrap().insert else {
            panic!("expected LSP insert");
        };
        assert_eq!(insert.text, "$price");
        assert_eq!(insert.caret_offset, None);
    }

    #[test]
    fn strip_snippet_leaves_plain_text_alone() {
        assert_eq!(strip_snippet("foo(a)"), ("foo(a)".to_owned(), None));
    }

    #[test]
    fn strip_snippet_flattens_placeholders_and_records_the_caret() {
        assert_eq!(
            strip_snippet("foo(${1:a}, ${2:b})$0"),
            ("foo(a, b)".to_owned(), Some(9))
        );
        assert_eq!(strip_snippet("f($1, ${2})"), ("f(, )".to_owned(), None));
        // First `$0` wins.
        assert_eq!(strip_snippet("${0}x$0"), ("x".to_owned(), Some(0)));
    }

    #[test]
    fn strip_snippet_takes_the_first_choice() {
        assert_eq!(strip_snippet("${1|x,y|}"), ("x".to_owned(), None));
    }

    #[test]
    fn strip_snippet_recurses_into_nested_placeholders() {
        assert_eq!(strip_snippet("${1:${2:x}}"), ("x".to_owned(), None));
        assert_eq!(strip_snippet("${1:a$0b}"), ("ab".to_owned(), Some(1)));
    }

    #[test]
    fn strip_snippet_unescapes() {
        assert_eq!(strip_snippet(r"\$1 \} \\"), (r"$1 } \".to_owned(), None));
    }

    #[test]
    fn strip_snippet_passes_malformed_input_through() {
        assert_eq!(strip_snippet("${1:"), ("${1:".to_owned(), None));
        assert_eq!(strip_snippet("${foo}"), ("${foo}".to_owned(), None));
        assert_eq!(strip_snippet("a}b{"), ("a}b{".to_owned(), None));
        assert_eq!(strip_snippet("$"), ("$".to_owned(), None));
    }

    #[test]
    fn conversion_is_capped() {
        let items: Vec<lsp_types::CompletionItem> = (0..MAX_LSP_ITEMS + 50)
            .map(|i| base_item(&format!("item_{i}")))
            .collect();
        let (id, root) = server();
        assert_eq!(
            items_to_menu_items(items, &id, &root, None).len(),
            MAX_LSP_ITEMS
        );
    }
}
