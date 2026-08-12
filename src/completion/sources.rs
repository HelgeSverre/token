//! Menu sources: `words` (rope scan around the cursor) and `snippets` (a
//! small static per-language table). Plain functions, not a trait —
//! autocomplete.md: "Not a trait for v1 (two hardcoded sources; a trait with
//! one call site is speculation)."

use crate::model::{Cursor, Document};
use crate::syntax::LanguageId;
use crate::util::text::{char_type, CharType};

use super::menu::{MenuItem, MenuItemKind, MenuSourceId};

/// Identifiers within this many lines of the cursor on either side
/// (autocomplete.md: "Zed scans ±5000; we start smaller").
const WINDOW_LINES: usize = 1000;
/// Shortest word worth suggesting.
const MIN_WORD_LEN: usize = 3;
/// Cap on collected words, so a huge window on a very wide file can't blow
/// up the filter/sort pass.
const MAX_WORDS: usize = 500;

/// Collect candidate words from lines within `WINDOW_LINES` of `cursor`,
/// deduplicated and excluding `query` itself (suggesting the word the user
/// already finished typing, verbatim, is never useful).
pub fn collect_words(document: &Document, cursor: Cursor, query: &str) -> Vec<MenuItem> {
    let line_count = document.line_count();
    let start_line = cursor.line.saturating_sub(WINDOW_LINES);
    let end_line = (cursor.line + WINDOW_LINES).min(line_count.saturating_sub(1));

    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for line_idx in lines_nearest_first(start_line, cursor.line.min(end_line), end_line) {
        let Some(line) = document.get_line_cow(line_idx) else {
            continue;
        };
        for word in extract_words(&line) {
            if word == query || !seen.insert(word.clone()) {
                continue;
            }
            out.push(MenuItem {
                label: word.clone(),
                filter_text: word.clone(),
                insert_text: word,
                kind: MenuItemKind::Variable,
                source: MenuSourceId::Words,
                detail: None,
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

/// Extract maximal runs of `CharType::WordChar` at least `MIN_WORD_LEN`
/// chars long — unicode-identifier-safe since `char_type` classifies by
/// `is_whitespace`/boundary-symbol membership, not ASCII ranges.
fn extract_words(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    for ch in line.chars() {
        if char_type(ch) == CharType::WordChar {
            current.push(ch);
        } else if !current.is_empty() {
            push_word(&mut out, &mut current);
        }
    }
    if !current.is_empty() {
        push_word(&mut out, &mut current);
    }
    out
}

fn push_word(out: &mut Vec<String>, current: &mut String) {
    if current.chars().count() >= MIN_WORD_LEN {
        out.push(std::mem::take(current));
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
            insert_text: body.to_string(),
            kind: MenuItemKind::Keyword,
            source: MenuSourceId::Snippets,
            detail: Some("snippet".to_string()),
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
    fn collects_words_within_window_deduped_and_min_length() {
        let doc = doc_with("let value = compute();\nlet other = value + 1;\nlet x = 2;\n");
        let items = collect_words(&doc, Cursor::at(0, 0), "");
        let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
        assert!(labels.contains(&"value"));
        assert!(labels.contains(&"compute"));
        assert!(labels.contains(&"other"));
        // "let" and "x" are below MIN_WORD_LEN or excluded — "let" is 3
        // chars so it IS included; "x" (1 char) must not be.
        assert!(!labels.contains(&"x"));
        // Deduped: "value" appears twice in source but once in output.
        assert_eq!(labels.iter().filter(|&&l| l == "value").count(), 1);
    }

    #[test]
    fn excludes_the_query_itself() {
        let doc = doc_with("let value = 1;\nlet valueOther = 2;\n");
        let items = collect_words(&doc, Cursor::at(0, 0), "value");
        let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
        assert!(!labels.contains(&"value"));
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

        let items = collect_words(&doc, Cursor::at(cursor_line, 0), "targ");
        let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
        assert!(
            labels.contains(&"target_word_here"),
            "word on the line directly above the cursor must survive the MAX_WORDS cap"
        );
    }

    #[test]
    fn extracts_unicode_identifiers() {
        let words = extract_words("let café = 1; let naïve_x = 2;");
        assert!(words.contains(&"café".to_string()));
        assert!(words.contains(&"naïve_x".to_string()));
    }

    #[test]
    fn snippets_scoped_per_language_and_empty_for_unhandled() {
        assert!(!collect_snippets(LanguageId::Rust).is_empty());
        assert!(collect_snippets(LanguageId::PlainText).is_empty());
    }
}
