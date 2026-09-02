//! LSP completion items -> [`MenuItem`] conversion (lsp-integration.md
//! Phase 5). Pure — no server handles, no model access — so the runtime's
//! interception pass can call it and unit tests can drive it directly.

use lsp_types::CompletionItemKind;

use super::menu::{LspInsert, MenuInsert, MenuItem, MenuItemKind, MenuSourceId};
use crate::lsp::LspServerId;

/// Cap on items kept from one server response. ts-ls routinely returns
/// hundreds; a pathological server returning tens of thousands would make
/// every refilter pass visible. rust-analyzer's full member lists sit well
/// under this.
const MAX_LSP_ITEMS: usize = 1000;

/// Converts one server response's items. `can_resolve` comes from the
/// responding server's capability snapshot (`completionProvider.resolveProvider`)
/// and rides on each item so accept can decide resolve-before-apply without
/// re-consulting the handle.
pub fn items_to_menu_items(
    items: Vec<lsp_types::CompletionItem>,
    server_id: &LspServerId,
    root: &std::path::Path,
    can_resolve: bool,
) -> Vec<MenuItem> {
    items
        .into_iter()
        .take(MAX_LSP_ITEMS)
        .filter_map(|item| completion_item_to_menu_item(item, server_id, root, can_resolve))
        .collect()
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
) -> Option<MenuItem> {
    // Serialize the *whole* item before destructuring it:
    // `completionItem/resolve` requires the same item object round-tripped,
    // including server-specific extension fields under `data`, and
    // `CompletionItem` isn't `Clone`.
    let raw = std::sync::Arc::new(serde_json::to_value(&item).unwrap_or(serde_json::Value::Null));

    let mut text_edit = match item.text_edit {
        Some(lsp_types::CompletionTextEdit::Edit(edit)) => Some((edit.range, edit.new_text)),
        // InsertAndReplace (3.16) — we advertise no support for it; use
        // the insert half rather than dropping the item outright.
        Some(lsp_types::CompletionTextEdit::InsertAndReplace(edit)) => {
            Some((edit.insert, edit.new_text))
        }
        None => None,
    };

    let lsp_types::CompletionItem {
        label,
        filter_text,
        mut insert_text,
        insert_text_format,
        kind,
        detail,
        sort_text,
        documentation,
        ..
    } = item;

    // We advertise `snippetSupport: false`, but rust-analyzer still sends
    // snippet bodies. Insert readable text; the caret lands at `$0`. The
    // primary text (`textEdit` if present) decides the caret.
    let mut caret_offset = None;
    if insert_text_format == Some(lsp_types::InsertTextFormat::SNIPPET) {
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
    let filter_text = filter_text.unwrap_or_else(|| label.clone());
    // `insertText ?? textEdit.newText ?? label`.
    let plain_text = text.unwrap_or_else(|| label.clone());

    Some(MenuItem {
        label,
        filter_text,
        insert: MenuInsert::Lsp(Box::new(LspInsert {
            text: plain_text,
            server_id: server_id.clone(),
            root: root.to_path_buf(),
            raw,
            can_resolve,
            resolved: false,
            text_edit,
            additional_text_edits: Vec::new(),
            caret_offset,
            documentation: documentation.as_ref().and_then(documentation_to_plain_text),
        })),
        kind: map_kind(kind),
        source: MenuSourceId::Lsp,
        detail,
        sort_text,
    })
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

/// Flattens `completionItem.documentation` to plaintext the way the hover
/// card does (`MarkupContent` per its `kind`; a bare string is plaintext
/// per the spec). `None` when empty after trimming.
pub fn documentation_to_plain_text(doc: &lsp_types::Documentation) -> Option<String> {
    let text = match doc {
        lsp_types::Documentation::String(s) => s.clone(),
        lsp_types::Documentation::MarkupContent(markup) => match markup.kind {
            lsp_types::MarkupKind::PlainText => markup.value.clone(),
            lsp_types::MarkupKind::Markdown => {
                crate::lsp::client::markdown_to_plain_text(&markup.value)
            }
        },
    };
    (!text.trim().is_empty()).then_some(text)
}

/// Maps the (open-ended) `CompletionItemKind` enum onto the menu's coarse
/// badge kinds. Unlisted kinds fall into `Other` (`?` badge).
fn map_kind(kind: Option<CompletionItemKind>) -> MenuItemKind {
    use CompletionItemKind as K;
    match kind {
        Some(K::FUNCTION | K::CONSTRUCTOR | K::METHOD | K::EVENT | K::OPERATOR) => {
            MenuItemKind::Function
        }
        Some(K::VARIABLE | K::VALUE | K::ENUM_MEMBER | K::TEXT) => MenuItemKind::Variable,
        Some(
            K::STRUCT | K::CLASS | K::ENUM | K::INTERFACE | K::TYPE_PARAMETER | K::UNIT | K::COLOR,
        ) => MenuItemKind::Type,
        Some(K::MODULE | K::FOLDER | K::FILE | K::REFERENCE) => MenuItemKind::Module,
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
        completion_item_to_menu_item(item, &id, &root, true)
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
        assert_eq!(insert.raw["data"]["autoImport"], serde_json::json!(true));
        assert_eq!(insert.raw["label"], serde_json::json!("imported_fn"));
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
        assert_eq!(insert.documentation.as_deref(), Some("Bold doc\nfn f()"));

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
            items_to_menu_items(items, &id, &root, false).len(),
            MAX_LSP_ITEMS
        );
    }
}
