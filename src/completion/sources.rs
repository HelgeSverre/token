//! Menu sources: `words` (rope scan around the cursor) and `snippets` (a
//! small static per-language table). Plain functions, not a trait —
//! autocomplete.md: "Not a trait for v1 (two hardcoded sources; a trait with
//! one call site is speculation)."

use super::context::{identifier_at, CompletionContext};
use crate::model::{Cursor, Document};
use crate::syntax::LanguageId;

use super::menu::{MenuInsert, MenuItem, MenuItemKind, MenuSourceId};

/// Identifiers within this many lines of the cursor on either side
/// (autocomplete.md: "Zed scans ±5000; we start smaller").
const WINDOW_LINES: usize = 1000;
/// Cap on collected words, so a huge window on a very wide file can't blow
/// up the filter/sort pass.
const MAX_WORDS: usize = 500;

/// Collect candidate words from lines within `WINDOW_LINES` of `cursor`,
/// deduplicated and excluding `query` itself (suggesting the word the user
/// already finished typing, verbatim, is never useful). The exclusion is
/// case-insensitive: typing `Value` must not suggest `value` — the query is
/// what the user already committed to, whatever its casing.
pub fn collect_words(
    document: &Document,
    cursor: Cursor,
    query: &str,
    min_word_length: usize,
) -> Vec<MenuItem> {
    let offset = document.cursor_to_offset(cursor.line, cursor.column);
    let (line, column) = document.offset_to_cursor(offset.saturating_sub(query.chars().count()));
    let context = CompletionContext::at(document, Cursor::at(line, column));
    if !context.allows_words() {
        return Vec::new();
    }
    let line_count = document.line_count();
    let start_line = cursor.line.saturating_sub(WINDOW_LINES);
    let end_line = (cursor.line + WINDOW_LINES).min(line_count.saturating_sub(1));

    let mut seen = std::collections::HashSet::new();
    let query_lower = query.to_ascii_lowercase();
    let mut out = Vec::new();
    for line_idx in lines_nearest_first(start_line, cursor.line.min(end_line), end_line) {
        let Some(line) = document.get_line_cow(line_idx) else {
            continue;
        };
        for (column, word) in extract_words(&line, min_word_length.max(1)) {
            if (context == CompletionContext::Code && !identifier_at(document, line_idx, column))
                || !word.to_ascii_lowercase().starts_with(&query_lower)
                || word.eq_ignore_ascii_case(query)
                || !seen.insert(word.clone())
            {
                continue;
            }
            out.push(MenuItem {
                label: word.clone(),
                filter_text: word.clone(),
                insert: MenuInsert::Text(word),
                kind: MenuItemKind::Variable,
                source: MenuSourceId::Words,
                detail: None,
                sort_text: None,
                preselect: false,
            });
            if out.len() >= MAX_WORDS {
                return out;
            }
        }
    }
    out
}

/// Line indices from `start..=end` ordered by distance to `center`
/// (`center` first, then alternating outward: center+1, center-1, center+2,
/// center-2, ...). `collect_words` scans in this order so the `MAX_WORDS`
/// cap — which a large window can hit well before the whole range is
/// scanned — is spent on lines nearest the cursor rather than always on the
/// top of the window.
fn lines_nearest_first(start: usize, center: usize, end: usize) -> Vec<usize> {
    let mut out = Vec::with_capacity(end - start + 1);
    out.push(center);
    let mut below = center + 1;
    let mut above = center;
    loop {
        let mut moved = false;
        if below <= end {
            out.push(below);
            below += 1;
            moved = true;
        }
        if above > start {
            above -= 1;
            out.push(above);
            moved = true;
        }
        if !moved {
            break;
        }
    }
    out
}

/// Extract Unicode identifier-shaped words, keeping character columns for
/// syntax lookup. Numbers and punctuation are not completion candidates.
fn extract_words(line: &str, min_word_length: usize) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut start = 0;
    for (column, ch) in line.chars().enumerate() {
        if unicode_ident::is_xid_continue(ch) {
            if current.is_empty() {
                start = column;
            }
            current.push(ch);
        } else if !current.is_empty() {
            push_word(&mut out, start, &mut current, min_word_length);
        }
    }
    if !current.is_empty() {
        push_word(&mut out, start, &mut current, min_word_length);
    }
    out
}

fn push_word(
    out: &mut Vec<(usize, String)>,
    start: usize,
    current: &mut String,
    min_word_length: usize,
) {
    if current.chars().count() >= min_word_length
        && current.starts_with(|ch: char| unicode_ident::is_xid_start(ch) || ch == '_')
    {
        out.push((start, std::mem::take(current)));
    } else {
        current.clear();
    }
}

/// Snippet bodies, flattened to plain text (autocomplete.md Phase 1: "Bodies
/// flattened to plain text until `snippets.md` lands"). A per-language
/// static table, but implemented as a plain match here rather than a new
/// field on `LanguageDefinition` (ponytail: the registry's `language!` macro
/// is invoked ~40 times, once per language — threading a new field through
/// every call site is a large mechanical diff for "a handful of snippets for
/// 2-3 languages to prove the path", which is all Phase 1 asks for; add the
/// `LanguageDefinition` field, following `selection`/`outline`'s pattern,
/// when a real per-language snippet count grows past what a match reads
/// comfortably).
fn snippet_table(language: LanguageId) -> &'static [(&'static str, &'static str)] {
    match language {
        LanguageId::Rust => &[
            ("fn", "fn name() {\n    \n}"),
            ("println", "println!(\"{}\", );"),
            ("derive", "#[derive()]"),
            ("test", "#[test]\nfn name() {\n    \n}"),
        ],
        LanguageId::JavaScript | LanguageId::TypeScript => &[
            ("function", "function name() {\n    \n}"),
            ("log", "console.log();"),
            ("arrow", "() => {\n    \n}"),
        ],
        LanguageId::Python => &[
            ("def", "def name():\n    pass"),
            (
                "class",
                "class Name:\n    def __init__(self):\n        pass",
            ),
            ("main", "if __name__ == \"__main__\":\n    main()"),
        ],
        _ => &[],
    }
}

/// Snippet items for `language`, matched against `query` by the shared
/// nucleo filter downstream (one filtering codepath for both sources rather
/// than a bespoke prefix-only match for snippets).
pub fn collect_snippets(language: LanguageId) -> Vec<MenuItem> {
    snippet_table(language)
        .iter()
        .map(|&(prefix, body)| MenuItem {
            label: prefix.to_string(),
            filter_text: prefix.to_string(),
            insert: MenuInsert::Text(body.to_string()),
            kind: MenuItemKind::Keyword,
            source: MenuSourceId::Snippets,
            detail: Some("snippet".to_string()),
            sort_text: None,
            preselect: false,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Document;

    fn doc_with(text: &str) -> Document {
        Document::with_text(text)
    }

    #[test]
    fn completion_menu_config_word_length_counts_characters_and_keeps_identifiers_only() {
        let doc = doc_with("a ab abc abcd éé ééé 12345 12abc 😀😀😀");
        let labels = |minimum| {
            collect_words(&doc, Cursor::at(0, 0), "", minimum)
                .into_iter()
                .map(|item| item.label)
                .collect::<Vec<_>>()
        };
        assert_eq!(labels(0), vec!["a", "ab", "abc", "abcd", "éé", "ééé"]);
        assert_eq!(labels(0), labels(1));
        assert_eq!(labels(3), vec!["abc", "abcd", "ééé"]);
        assert_eq!(labels(4), vec!["abcd"]);
        assert!(labels(usize::MAX).is_empty());
    }

    #[test]
    fn collects_words_within_window_deduped_and_min_length() {
        let doc = doc_with("let value = compute();\nlet other = value + 1;\nlet x = 2;\n");
        let items = collect_words(&doc, Cursor::at(0, 0), "", 3);
        let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
        assert!(labels.contains(&"value"));
        assert!(labels.contains(&"compute"));
        assert!(labels.contains(&"other"));
        // "let" has the minimum three characters; "x" (one) does not.
        assert!(!labels.contains(&"x"));
        // Deduped: "value" appears twice in source but once in output.
        assert_eq!(labels.iter().filter(|&&l| l == "value").count(), 1);
    }

    #[test]
    fn excludes_the_query_itself() {
        let doc = doc_with("let value = 1;\nlet valueOther = 2;\n");
        let items = collect_words(&doc, Cursor::at(0, 0), "value", 3);
        let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
        assert!(!labels.contains(&"value"));
        assert!(labels.contains(&"valueOther"));
    }

    #[test]
    fn excludes_the_query_case_insensitively() {
        // Typing `Value` must not suggest the buffer's `value` — the
        // exact-match exclusion is about "the user already finished this
        // word", which doesn't depend on casing.
        let doc = doc_with("let value = 1;\nlet valueOther = 2;\n");
        let items = collect_words(&doc, Cursor::at(0, 0), "Value", 3);
        let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
        assert!(
            !labels.contains(&"value"),
            "case-variant of the query must be excluded"
        );
        assert!(labels.contains(&"valueOther"));
    }

    #[test]
    fn a_word_near_the_cursor_survives_the_max_words_cap() {
        // MAX_WORDS unique filler identifiers, each on its own line, all
        // above the cursor — enough to fill the cap on their own. A
        // distinct word sits on the line directly above the cursor. If the
        // scan fills the cap from the top of the window (the regression),
        // the near-cursor word is never reached.
        let mut text = String::new();
        for i in 0..MAX_WORDS + 100 {
            text.push_str(&format!("let filler_word_{i} = 0;\n"));
        }
        text.push_str("let target_word_here = 1;\n");
        let cursor_line = text.matches('\n').count(); // line after the last one
        let doc = doc_with(&text);

        let items = collect_words(&doc, Cursor::at(cursor_line, 0), "targ", 3);
        let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
        assert!(
            labels.contains(&"target_word_here"),
            "word on the line directly above the cursor must survive the MAX_WORDS cap"
        );
    }

    #[test]
    fn extracts_unicode_identifiers() {
        let words = extract_words("let café = 1; let naïve_x = 2;", 3);
        assert!(words.iter().any(|(_, word)| word == "café"));
        assert!(words.iter().any(|(_, word)| word == "naïve_x"));
    }

    #[test]
    fn words_reject_numbers_symbols_and_subsequence_noise() {
        let doc = doc_with("123456 😀😀😀 vector_value value_valid cafe\u{301}\nva");
        let items = collect_words(&doc, Cursor::at(1, 2), "va", 3);
        assert_eq!(
            items.iter().map(|i| i.label.as_str()).collect::<Vec<_>>(),
            ["value_valid"]
        );
        let words = extract_words("123456 😀😀😀 cafe\u{301} _value 123invalid", 3);
        assert_eq!(
            words.iter().map(|(_, w)| w.as_str()).collect::<Vec<_>>(),
            ["cafe\u{301}", "_value"]
        );
    }

    #[test]
    fn code_words_exclude_comment_and_string_contents() {
        let mut doc = doc_with("fn main() {\nlet value_real = 1;\n// value_comment\nlet text = \"value_string\";\nva\n}\n");
        doc.language = LanguageId::Rust;
        doc.syntax_highlights = Some(crate::syntax::ParserState::new().parse_and_highlight(
            &doc.buffer.to_string(),
            doc.language,
            crate::model::DocumentId(1),
            doc.revision,
        ));
        let items = collect_words(&doc, Cursor::at(4, 2), "va", 3);
        assert_eq!(
            items.iter().map(|i| i.label.as_str()).collect::<Vec<_>>(),
            ["value_real"]
        );
        assert!(collect_words(&doc, Cursor::at(2, 5), "va", 3).is_empty());
        doc.revision += 1;
        assert!(collect_words(&doc, Cursor::at(4, 2), "va", 3).is_empty());
    }

    #[test]
    fn nearest_prefix_match_ranks_first_and_unrelated_words_do_not_spend_cap() {
        let mut text = "noise\n".repeat(MAX_WORDS + 1);
        text.push_str("value_far\n\nvalue_near_longer\nva");
        let doc = doc_with(&text);
        let items = collect_words(&doc, Cursor::at(doc.line_count() - 1, 2), "va", 3);
        let sorted = super::super::menu::filter_and_sort(&items, "va");
        assert_eq!(items[sorted[0].1].label, "value_near_longer");
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn snippets_scoped_per_language_and_empty_for_unhandled() {
        assert!(!collect_snippets(LanguageId::Rust).is_empty());
        assert!(collect_snippets(LanguageId::PlainText).is_empty());
    }
}
