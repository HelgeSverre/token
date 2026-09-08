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

/// Completion-item kind shared by sources and rows. The view owns badge
/// glyphs/colors; this semantic type does not depend on rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuItemKind {
    Function,
    Method,
    Variable,
    Type,
    Keyword,
    Field,
    Module,
    File,
    Folder,
    Constant,
    Other,
}

/// Which source produced a `MenuItem` — ranking tiebreak (LSP > Snippets >
/// Words per autocomplete.md; the LSP tier was reserved by the doc and is
/// wired up by lsp-integration.md Phase 5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuSourceId {
    Lsp,
    Paths,
    Snippets,
    Words,
}

impl MenuSourceId {
    fn tier(self) -> u8 {
        match self {
            MenuSourceId::Lsp => 0,
            MenuSourceId::Paths => 1,
            MenuSourceId::Snippets => 2,
            MenuSourceId::Words => 3,
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
    /// Original protocol item, shared through menu refreshes and resolve
    /// debouncing. Serialized only at the runtime's resolve request boundary;
    /// presentation/snippet normalization must never alter this snapshot.
    pub raw: std::sync::Arc<lsp_types::CompletionItem>,
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
    /// Single-character commit set from this item or its server defaults.
    /// Shared across inherited items and clones during query refinement.
    pub commit_characters: std::sync::Arc<[char]>,
    /// Char offset within the primary inserted text where the caret lands
    /// after accept — the snippet's `$0`. `None`: after the text.
    pub caret_offset: Option<usize>,
    /// Plaintext `documentation` (markdown lightly stripped, same as the
    /// hover card), from the item itself or its resolve round trip.
    /// Shown in the docs card beside the menu.
    pub documentation: Option<crate::model::StyledText>,
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
    /// Server preference for initial selection, never overriding user navigation.
    pub preselect: bool,
}

/// Guards the one literal keystroke staged while an item resolves. No document
/// or history clone: a changed revision, pane, selection or menu cancels it.
#[derive(Debug, Clone)]
pub(crate) struct PendingCommit {
    pub character: char,
    pub document_id: DocumentId,
    pub editor_id: crate::model::EditorId,
    pub file_path: std::path::PathBuf,
    pub language: crate::syntax::LanguageId,
    pub revision: u64,
    pub cursors: Vec<Cursor>,
    pub active_cursor_index: usize,
    pub undo_len: usize,
    pub selected: usize,
}

/// The completion popup's state, held on `UiState::completion_menu`.
/// Selection/scroll live on `ui.cursor_overlay` instead of duplicated here —
/// the same `SelectableListViewport`-style state every other cursor-anchored
/// popup already uses (deviation from the doc's sketch, which put `selected`
/// / `viewport_offset` on this struct directly; overlay-p5 had already built
/// the shared home for that state by the time this shipped).
#[derive(Debug, Clone)]
pub struct CompletionMenuState {
    /// Source policy at this query's start, refreshed after a pending parse.
    pub context: super::context::CompletionContext,
    /// User navigation takes precedence over a later server `preselect`.
    pub selection_changed: bool,
    pub document_id: DocumentId,
    /// Document revision the items were collected against — the staleness
    /// guard on accept (autocomplete.md's `RequestSnapshot` shape, minus the
    /// async-only `request_id`: Phase 1 sources are synchronous, so there is
    /// never more than one in-flight collection to supersede).
    pub revision: u64,
    /// Word or filename-component start. Path queries may decode Markdown's
    /// percent escapes; their request snapshot guards the original source range.
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
    pub fn preferred_index(&self) -> usize {
        self.filtered
            .iter()
            .position(|(_, index, _)| self.items[*index].preselect)
            .unwrap_or(0)
    }
    pub fn selected_item(&self, selected: usize) -> Option<&MenuItem> {
        let (_, idx, _) = self.filtered.get(selected)?;
        self.items.get(*idx)
    }

    pub fn selected_documentation(&self, selected: usize) -> Option<&crate::model::StyledText> {
        let MenuInsert::Lsp(data) = &self.selected_item(selected)?.insert else {
            return None;
        };
        data.documentation
            .as_ref()
            .filter(|docs| !docs.text.trim().is_empty())
    }
}

/// Server relevance leads the LSP block, ahead of all local candidates.
/// Local prefix matches order by exactness, source and proximity (words)
/// or fuzzy score (snippets). Semantic matches retain server ordering.
fn tier_key<'a>(
    item: &'a MenuItem,
    filter_text_lower: &str,
    query_lower: &str,
    score: u32,
) -> ((u8, &'a str), bool, bool, u8, u32, &'a str) {
    let exact = filter_text_lower == query_lower;
    let starts = filter_text_lower.starts_with(query_lower);
    let lsp_key = match item.source {
        MenuSourceId::Lsp => (0, item.sort_text.as_deref().unwrap_or(item.label.as_str())),
        _ => (1, ""),
    };
    (
        lsp_key,
        !exact,
        !starts,
        item.source.tier(),
        u32::MAX - score,
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
        deduplicate_paths(items, &mut idxs);
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
            // Local candidates have no semantic authority. Require a real
            // prefix; arbitrary subsequence matches are only useful for LSP
            // candidates already selected for this receiver/scope.
            if item.source != MenuSourceId::Lsp && !haystack_lower.starts_with(&query_lower) {
                return None;
            }
            if item.source == MenuSourceId::Paths {
                // Filesystem prefixes are already matched above. Nucleo's
                // normalization expects a normalized needle and can otherwise
                // discard an exact accented filename prefix such as `hé`.
                let count = query_lower.chars().count() as u32;
                return Some((count, i, haystack_lower, (0..count).collect()));
            }
            let mut haystack_buf = Vec::new();
            let haystack = Utf32Str::new(&haystack_lower, &mut haystack_buf);
            let mut indices = Vec::new();
            let score = matcher.fuzzy_indices(haystack, needle, &mut indices)?;
            Some((score as u32, i, haystack_lower, indices))
        })
        .collect();

    scored.sort_by(|(s1, i1, l1, _), (s2, i2, l2, _)| {
        let score = |index: usize, fuzzy| {
            if items[index].source == MenuSourceId::Words {
                u32::MAX - index.min(u32::MAX as usize) as u32
            } else {
                fuzzy
            }
        };
        tier_key(&items[*i1], l1, &query_lower, score(*i1, *s1)).cmp(&tier_key(
            &items[*i2],
            l2,
            &query_lower,
            score(*i2, *s2),
        ))
    });
    let mut filtered = scored
        .into_iter()
        .map(|(s, i, _, indices)| (s, i, indices))
        .collect();
    deduplicate_paths(items, &mut filtered);
    filtered
}

/// Prefer server semantics when both visible sources would insert the same
/// filename. Keep distinct server edits/labels, even if their prefix matches.
fn deduplicate_paths(items: &[MenuItem], filtered: &mut Vec<(u32, usize, Vec<u32>)>) {
    if !filtered
        .iter()
        .any(|(_, index, _)| items[*index].source == MenuSourceId::Paths)
    {
        return;
    }
    fn text(item: &MenuItem) -> &str {
        match &item.insert {
            MenuInsert::Text(text) => text,
            MenuInsert::Lsp(data) => data
                .text_edit
                .as_ref()
                .map_or(data.text.as_str(), |(_, text)| text.as_str()),
        }
    }
    let server: std::collections::HashSet<_> = filtered
        .iter()
        .filter_map(|(_, index, _)| {
            let item = &items[*index];
            (item.source == MenuSourceId::Lsp).then(|| (item.label.as_str(), text(item)))
        })
        .collect();
    filtered.retain(|(_, index, _)| {
        let item = &items[*index];
        item.source != MenuSourceId::Paths || !server.contains(&(item.label.as_str(), text(item)))
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_completion_filter_deduplicates_matching_server_insertions() {
        let items = vec![
            item("asset.rs", MenuSourceId::Paths),
            item("asset.rs", MenuSourceId::Lsp),
            item("assets/", MenuSourceId::Paths),
        ];
        for query in ["", "as"] {
            let filtered = filter_and_sort(&items, query);
            assert_eq!(filtered.len(), 2);
            assert_eq!(items[filtered[0].1].source, MenuSourceId::Lsp);
        }
        let items = vec![item("héllo.rs", MenuSourceId::Paths)];
        let filtered = filter_and_sort(&items, "hé");
        assert_eq!(filtered[0].2, vec![0, 1]);
    }

    fn item(label: &str, source: MenuSourceId) -> MenuItem {
        MenuItem {
            label: label.to_string(),
            filter_text: label.to_string(),
            insert: MenuInsert::Text(label.to_string()),
            kind: MenuItemKind::Other,
            source,
            detail: None,
            sort_text: None,
            preselect: false,
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
    fn server_relevance_outranks_local_exact_and_lsp_prefix_matches() {
        let mut preferred = item("collect_words", MenuSourceId::Lsp);
        preferred.sort_text = Some("000".into());
        let mut prefix = item("word", MenuSourceId::Lsp);
        prefix.sort_text = Some("999".into());
        let items = vec![
            item("wo", MenuSourceId::Words),
            prefix,
            preferred,
            item("collect_words", MenuSourceId::Words),
        ];
        let sorted = filter_and_sort(&items, "wo");
        assert_eq!(
            sorted.iter().map(|(_, i, _)| *i).collect::<Vec<_>>(),
            [2, 1, 0]
        );
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
