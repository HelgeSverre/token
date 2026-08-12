//! `CompletionMenuState`: the popup's data model, plus filtering/sorting.
//!
//! autocomplete.md's `MenuItem`/`MenuInsert` split is collapsed to a single
//! `insert_text` field here (ponytail: `MenuInsert` is a single-variant enum
//! in the doc too — "one variant until LSP's `additionalTextEdits` needs a
//! second"; skipping the wrapper enum entirely removes one layer of
//! indirection for the same reason, and costs nothing to add back when a
//! second insert shape exists). `replace_start` isn't stored per-item either
//! — every Phase 1 source replaces exactly `[query_start..cursor)`, so
//! `CompletionMenuState::query_start` is the only start position that
//! exists.

use nucleo_matcher::{Config, Matcher, Utf32Str};

use crate::model::{Cursor, DocumentId};

/// Coarse completion-item kind, mirrored by `view::overlay_surface`'s
/// `CompletionKind` (which owns the badge glyph/color — a view-layer
/// concern). Kept separate so this module doesn't depend on `view`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuItemKind {
    Function,
    Variable,
    Type,
    Keyword,
    Field,
    Module,
    Constant,
    Other,
}

/// Which source produced a `MenuItem` — ranking tiebreak only (LSP > Snippets
/// > Words per autocomplete.md; Phase 1 has no LSP source yet).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuSourceId {
    Words,
    Snippets,
}

impl MenuSourceId {
    fn tier(self) -> u8 {
        match self {
            MenuSourceId::Snippets => 0,
            MenuSourceId::Words => 1,
        }
    }
}

#[derive(Debug, Clone)]
pub struct MenuItem {
    pub label: String,
    /// Matched against the typed query. Equal to `label` for both Phase 1
    /// sources.
    pub filter_text: String,
    /// Text inserted at `[query_start..cursor)` on accept.
    pub insert_text: String,
    pub kind: MenuItemKind,
    pub source: MenuSourceId,
    pub detail: Option<String>,
}

/// The completion popup's state, held on `UiState::completion_menu`.
/// Selection/scroll live on `ui.cursor_overlay` instead of duplicated here —
/// the same `SelectableListViewport`-style state every other cursor-anchored
/// popup already uses (deviation from the doc's sketch, which put `selected`
/// / `viewport_offset` on this struct directly; overlay-p5 had already built
/// the shared home for that state by the time this shipped).
#[derive(Debug, Clone)]
pub struct CompletionMenuState {
    pub document_id: DocumentId,
    /// Document revision the items were collected against — the staleness
    /// guard on accept (autocomplete.md's `RequestSnapshot` shape, minus the
    /// async-only `request_id`: Phase 1 sources are synchronous, so there is
    /// never more than one in-flight collection to supersede).
    pub revision: u64,
    /// Word start; `query = text[query_start..cursor]`.
    pub query_start: Cursor,
    pub items: Vec<MenuItem>,
    /// `(score, index into items, nucleo match indices into
    /// `items[index].filter_text`)`, filtered and sorted; empty query keeps
    /// every item in source/label order (score `0`, no indices).
    pub filtered: Vec<(u32, usize, Vec<u32>)>,
}

impl CompletionMenuState {
    pub fn selected_item(&self, selected: usize) -> Option<&MenuItem> {
        let (_, idx, _) = self.filtered.get(selected)?;
        self.items.get(*idx)
    }
}

/// Tier key: exact match, then word-start match, then score, then source,
/// then label — autocomplete.md's ranking rule ("nucleo-matcher over
/// `filter_text`, sort by (exact match, word-start match tier, score desc,
/// source tier ..., label)").
fn tier_key<'a>(
    item: &'a MenuItem,
    filter_text_lower: &str,
    query_lower: &str,
    score: u32,
) -> (bool, bool, u32, u8, &'a str) {
    let exact = filter_text_lower == query_lower;
    let starts = filter_text_lower.starts_with(query_lower);
    (
        !exact,
        !starts,
        u32::MAX - score,
        item.source.tier(),
        item.label.as_str(),
    )
}

/// Fuzzy-filter `items` against `query` and sort by the tiered rule above.
/// Empty query keeps every item (source tier, then label order) — the same
/// bypass `resolve_palette_rows` uses for the command palette. The third
/// tuple element is nucleo's matched-char indices into `filter_text`, for
/// the popup to bold the typed substring (autocomplete.md's Rendering
/// section: rows are `KindBadge` + label with `match_indices`).
pub fn filter_and_sort(items: &[MenuItem], query: &str) -> Vec<(u32, usize, Vec<u32>)> {
    if query.is_empty() {
        let mut idxs: Vec<(u32, usize, Vec<u32>)> =
            (0..items.len()).map(|i| (0u32, i, Vec::new())).collect();
        idxs.sort_by(|(_, a, _), (_, b, _)| {
            (items[*a].source.tier(), items[*a].label.as_str())
                .cmp(&(items[*b].source.tier(), items[*b].label.as_str()))
        });
        return idxs;
    }

    let query_lower = query.to_lowercase();
    let mut matcher = Matcher::new(Config::DEFAULT);
    let mut query_buf = Vec::new();
    let needle = Utf32Str::new(&query_lower, &mut query_buf);

    // Keep each item's lowercased `filter_text` alongside its score so the
    // sort comparator below reuses it instead of re-lowercasing (and
    // reallocating) on every comparison.
    let mut scored: Vec<(u32, usize, String, Vec<u32>)> = items
        .iter()
        .enumerate()
        .filter_map(|(i, item)| {
            let haystack_lower = item.filter_text.to_lowercase();
            let mut haystack_buf = Vec::new();
            let haystack = Utf32Str::new(&haystack_lower, &mut haystack_buf);
            let mut indices = Vec::new();
            let score = matcher.fuzzy_indices(haystack, needle, &mut indices)?;
            Some((score as u32, i, haystack_lower, indices))
        })
        .collect();

    scored.sort_by(|(s1, i1, l1, _), (s2, i2, l2, _)| {
        tier_key(&items[*i1], l1, &query_lower, *s1).cmp(&tier_key(&items[*i2], l2, &query_lower, *s2))
    });
    scored
        .into_iter()
        .map(|(s, i, _, indices)| (s, i, indices))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(label: &str, source: MenuSourceId) -> MenuItem {
        MenuItem {
            label: label.to_string(),
            filter_text: label.to_string(),
            insert_text: label.to_string(),
            kind: MenuItemKind::Other,
            source,
            detail: None,
        }
    }

    #[test]
    fn empty_query_keeps_every_item_sorted_by_source_then_label() {
        let items = vec![
            item("zebra", MenuSourceId::Words),
            item("apple", MenuSourceId::Snippets),
            item("mango", MenuSourceId::Words),
        ];
        let filtered = filter_and_sort(&items, "");
        let labels: Vec<&str> = filtered
            .iter()
            .map(|(_, i, _)| items[*i].label.as_str())
            .collect();
        // Snippets (tier 0) before Words (tier 1); label order within a tier.
        assert_eq!(labels, vec!["apple", "mango", "zebra"]);
    }

    #[test]
    fn exact_match_outranks_prefix_and_fuzzy() {
        let items = vec![
            item("length", MenuSourceId::Words),
            item("len", MenuSourceId::Words),
            item("lend", MenuSourceId::Words),
        ];
        let filtered = filter_and_sort(&items, "len");
        assert_eq!(items[filtered[0].1].label, "len");
    }

    #[test]
    fn word_start_match_outranks_pure_fuzzy_match_at_equal_score_tier() {
        let items = vec![
            item("collect_words", MenuSourceId::Words), // contains "wo" mid-word too
            item("word_start", MenuSourceId::Words),    // starts with "wo"
        ];
        let filtered = filter_and_sort(&items, "wo");
        assert_eq!(items[filtered[0].1].label, "word_start");
    }

    #[test]
    fn source_tier_breaks_ties_snippets_before_words() {
        let items = vec![
            item("format", MenuSourceId::Words),
            item("format", MenuSourceId::Snippets),
        ];
        let filtered = filter_and_sort(&items, "format");
        assert_eq!(items[filtered[0].1].source, MenuSourceId::Snippets);
    }

    #[test]
    fn filtered_carries_match_indices_for_highlighting() {
        let items = vec![item("word_start", MenuSourceId::Words)];
        let filtered = filter_and_sort(&items, "wo");
        let (_, _, indices) = &filtered[0];
        assert_eq!(indices.as_slice(), &[0u32, 1u32]);
    }

    #[test]
    fn no_match_is_dropped() {
        let items = vec![item("value", MenuSourceId::Words)];
        let filtered = filter_and_sort(&items, "xyz123notpresent");
        assert!(filtered.is_empty());
    }
}
