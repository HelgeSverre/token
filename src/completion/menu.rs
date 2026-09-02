//! `CompletionMenuState`: the popup's data model, plus filtering/sorting.
//!
//! autocomplete.md's `MenuItem`/`MenuInsert` split was collapsed to a single
//! `insert_text` field for Phase 1 ("one variant until LSP's
//! `additionalTextEdits` needs a second") — LSP completion
//! (lsp-integration.md Phase 5) is that second shape, so [`MenuInsert`] is
//! back. Words/snippets still replace exactly `[query_start..cursor)`, so
//! `CompletionMenuState::query_start` remains the only start position those
//! sources need; LSP items carry their own `textEdit` ranges.

use std::path::PathBuf;

use nucleo_matcher::{Config, Matcher, Utf32Str};

use crate::lsp::LspServerId;
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

/// Which source produced a `MenuItem` — ranking tiebreak (LSP > Snippets >
/// Words per autocomplete.md; the LSP tier was reserved by the doc and is
/// wired up by lsp-integration.md Phase 5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuSourceId {
    Lsp,
    Snippets,
    Words,
}

impl MenuSourceId {
    fn tier(self) -> u8 {
        match self {
            MenuSourceId::Lsp => 0,
            MenuSourceId::Snippets => 1,
            MenuSourceId::Words => 2,
        }
    }
}

/// What accepting a `MenuItem` inserts. `Text` replaces the query range
/// with a literal string; `Lsp` carries the protocol data accept needs
/// (resolve-before-accept, `textEdit` ranges) — see `update/completion.rs`'s
/// `apply_lsp_accept`. The LSP payload is boxed to keep `MenuItem` (and the
/// message enums carrying it) small.
#[derive(Debug, Clone)]
pub enum MenuInsert {
    Text(String),
    Lsp(Box<LspInsert>),
}

/// The protocol data an LSP completion item keeps past conversion, so
/// accept can resolve-then-apply without the runtime re-deriving anything.
#[derive(Debug, Clone)]
pub struct LspInsert {
    /// Plain text inserted when the item carries no usable `textEdit`
    /// (`insertText ?? label ?? textEdit.newText`) — also the fallback for
    /// secondary cursors in a multi-cursor accept (`textEdit` ranges are
    /// absolute and only meaningful for the active cursor).
    pub text: String,
    /// The server that produced this item — routes `completionItem/resolve`
    /// back to it.
    pub server_id: LspServerId,
    pub root: PathBuf,
    /// The raw item JSON, replayed verbatim as `completionItem/resolve`'s
    /// parameter (the spec requires the *same* item object round-tripped).
    /// Shared, not owned: carried LSP items are cloned on every keystroke
    /// refresh, and a deep `Value` clone per item dominated that path
    /// (~3.5 ms / 84k allocations per keystroke at 1000 items).
    pub raw: std::sync::Arc<serde_json::Value>,
    /// Whether the server advertised `completionProvider.resolveProvider`,
    /// captured at conversion time from its capabilities snapshot.
    pub can_resolve: bool,
    /// Set once a resolve round trip has folded its results in — never
    /// resolves the same item twice.
    pub resolved: bool,
    /// Primary `textEdit`: `(range, new_text)`. Applied in place of the
    /// query range for the active cursor, re-anchored to the live cursor
    /// (the type-then-Enter race is one character wide but common).
    pub text_edit: Option<(lsp_types::Range, String)>,
    /// `additionalTextEdits` known up front plus anything a resolve round
    /// trip added (ts-ls auto-imports live here). Absolute ranges, applied
    /// atomically with the primary edit as one undo step.
    pub additional_text_edits: Vec<(lsp_types::Range, String)>,
    /// Char offset within the primary inserted text where the caret lands
    /// after accept — the snippet's `$0`. `None`: after the text.
    pub caret_offset: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct MenuItem {
    pub label: String,
    /// Matched against the typed query. Equal to `label` for words;
    /// `filterText ?? label` for LSP items (rust-analyzer labels embed type
    /// signatures — matching bare labels ranks visibly wrong).
    pub filter_text: String,
    /// What accept inserts — see [`MenuInsert`].
    pub insert: MenuInsert,
    pub kind: MenuItemKind,
    pub source: MenuSourceId,
    pub detail: Option<String>,
    /// Server-provided `sortText`: the ordering authority among LSP items
    /// (lsp-integration.md Phase 5: "server `sortText` first, nucleo
    /// fuzzy score as tiebreak"). `None` for offline sources, which order
    /// by label.
    pub sort_text: Option<String>,
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
    /// The query the current `items` were collected/last filtered against.
    /// LSP items are carried across keystrokes only while the query grows
    /// or shrinks along the same word (`query` is a prefix of the previous
    /// one or vice versa) — a divergent edit invalidates them.
    pub query: String,
    pub items: Vec<MenuItem>,
    /// `(score, index into items, nucleo match indices into
    /// `items[index].filter_text`)`, filtered and sorted; empty query keeps
    /// every item in source/label order (score `0`, no indices).
    pub filtered: Vec<(u32, usize, Vec<u32>)>,
    /// The last LSP response's `isIncomplete` flag: the item set is *not*
    /// a superset for further typing, so local filtering is instant
    /// feedback only and every keystroke re-requests (ts-ls sets this
    /// routinely). Never set by offline sources.
    pub is_incomplete: bool,
    /// Set while an accept on an unresolved LSP item waits for its
    /// `completionItem/resolve` round trip: the selected index the resolve
    /// was issued for. Enter/click accepts are ignored while `Some` (the
    /// deferred accept applies when the resolution lands); any edit or
    /// selection change drops it via the revision/query guards.
    pub pending_resolve: Option<usize>,
}

impl CompletionMenuState {
    pub fn selected_item(&self, selected: usize) -> Option<&MenuItem> {
        let (_, idx, _) = self.filtered.get(selected)?;
        self.items.get(*idx)
    }
}

/// Tier key: exact match, then word-start match, then the LSP block
/// (ordered by server `sortText`, label when absent) above every offline
/// item, then score, source tier, label. This is autocomplete.md's rule
/// ("exact, word-start, score desc, source tier, label") for words and
/// snippets, with lsp-integration.md Phase 5's "server `sortText` first,
/// fuzzy score as tiebreak" applied to LSP items — the LSP tier never
/// mixes with offline items, so the two rules never conflict.
fn tier_key<'a>(
    item: &'a MenuItem,
    filter_text_lower: &str,
    query_lower: &str,
    score: u32,
) -> (bool, bool, (u8, &'a str), u32, u8, &'a str) {
    let exact = filter_text_lower == query_lower;
    let starts = filter_text_lower.starts_with(query_lower);
    let lsp_key = match item.source {
        MenuSourceId::Lsp => (0, item.sort_text.as_deref().unwrap_or(item.label.as_str())),
        _ => (1, ""),
    };
    (
        !exact,
        !starts,
        lsp_key,
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
        // Same key as the scored path's (tier before sortText), so an
        // empty-query listing (explicit Ctrl+Space) orders LSP items by
        // server `sortText` too.
        idxs.sort_by(|(_, a, _), (_, b, _)| {
            (
                items[*a].source.tier(),
                items[*a]
                    .sort_text
                    .as_deref()
                    .unwrap_or(items[*a].label.as_str()),
                items[*a].label.as_str(),
            )
                .cmp(&(
                    items[*b].source.tier(),
                    items[*b]
                        .sort_text
                        .as_deref()
                        .unwrap_or(items[*b].label.as_str()),
                    items[*b].label.as_str(),
                ))
        });
        return idxs;
    }
    // ASCII-only lowercasing, not `str::to_lowercase`: the matched-char
    // indices below are char positions into this lowercased string, but
    // `completion_rows` (view/modal.rs) applies them to `item.label`
    // unchanged. Full Unicode lowercasing can change a string's char count
    // (e.g. 'İ' -> 2 chars), which would desync the indices from the label
    // they're bolding; ASCII case-folding is length-preserving by
    // construction, at the cost of not case-folding non-ASCII letters.
    let query_lower = query.to_ascii_lowercase();
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
            let haystack_lower = item.filter_text.to_ascii_lowercase();
            let mut haystack_buf = Vec::new();
            let haystack = Utf32Str::new(&haystack_lower, &mut haystack_buf);
            let mut indices = Vec::new();
            let score = matcher.fuzzy_indices(haystack, needle, &mut indices)?;
            Some((score as u32, i, haystack_lower, indices))
        })
        .collect();

    scored.sort_by(|(s1, i1, l1, _), (s2, i2, l2, _)| {
        tier_key(&items[*i1], l1, &query_lower, *s1).cmp(&tier_key(
            &items[*i2],
            l2,
            &query_lower,
            *s2,
        ))
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
            insert: MenuInsert::Text(label.to_string()),
            kind: MenuItemKind::Other,
            source,
            detail: None,
            sort_text: None,
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
    fn lsp_items_order_by_sort_text_before_fuzzy_score() {
        // rust-analyzer's relevance lives in sortText; a shorter label's
        // higher nucleo score must not override it within the LSP block.
        let mut short = item("vacs", MenuSourceId::Lsp);
        short.sort_text = Some("zzz".to_owned());
        let mut long = item("vacuum_cleaner", MenuSourceId::Lsp);
        long.sort_text = Some("aaa".to_owned());
        let items = vec![short, long];
        let sorted = filter_and_sort(&items, "vac");
        assert_eq!(items[sorted[0].1].label, "vacuum_cleaner");
        assert_eq!(items[sorted[1].1].label, "vacs");
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
