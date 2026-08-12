# Find Enhancements (Advanced)

Regex support, whole word matching, match count display, and visual highlighting.

> **Status:** Partially implemented — search engine (regex/whole-word/case), decoration-pipeline match highlighting + scrollbar overview marks, and Find Previous/Tab keybindings shipped; UI rendering (Phase 5: toggle buttons, match count, regex error display) and selection scope (Phase 7) remain
> **Priority:** P1
> **Effort:** M
> **Created:** 2025-12-19
> **Milestone:** 2 - Search & Editing
> **Prerequisite:** Basic find implemented (see `docs/archived/find-basic.md`)

---

## Table of Contents

1. [Overview](#overview)
2. [Architecture](#architecture)
3. [Data Structures](#data-structures)
4. [Keybindings](#keybindings)
5. [Implementation Plan](#implementation-plan)
6. [Testing Strategy](#testing-strategy)
7. [References](#references)

---

## Overview

### Already Implemented (v0.3.11)

- Basic text search with case sensitivity toggle
- Single match navigation (Find Next / Find Previous)
- Current match highlighting

### Remaining Goals

1. **Highlight all matches** - Visual indication of all matches in the viewport
2. **Regex support** - Full regular expression search capability
3. **Whole word matching** - Match complete words only (not substrings)
4. **Match count** - Display "N of M matches" in the find bar
5. **Incremental search** - Update matches as user types
6. **Selection scope** - Option to search within selection only

### Non-Goals

- Find in files / workspace search (separate feature)
- Search history persistence (can add later)
- Saved search patterns / regex library

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         Enhanced Find System                                 │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ┌────────────────────────────────────────────────────────────────────────┐ │
│  │                          Find Bar UI                                    │ │
│  │                                                                         │ │
│  │  ┌─────────────────────────────────────────────────────────────────┐   │ │
│  │  │  Query: [ search term                             ] (3 of 42)   │   │ │
│  │  │                                                                 │   │ │
│  │  │  [Aa] Case   [W] Whole   [.*] Regex   [=] Selection            │   │ │
│  │  └─────────────────────────────────────────────────────────────────┘   │ │
│  └────────────────────────────────────────────────────────────────────────┘ │
│                                 │                                            │
│                                 │ Query changes                              │
│                                 ▼                                            │
│  ┌────────────────────────────────────────────────────────────────────────┐ │
│  │                        Search Engine                                    │ │
│  │                                                                         │ │
│  │  ┌─────────────────────────────────────────────────────────────────┐   │ │
│  │  │  SearchQuery                                                    │   │ │
│  │  │  - pattern: String                                              │   │ │
│  │  │  - case_sensitive: bool                                         │   │ │
│  │  │  - whole_word: bool                                             │   │ │
│  │  │  - regex: bool                                                  │   │ │
│  │  │  - selection_only: bool                                         │   │ │
│  │  └─────────────────────────────────────────────────────────────────┘   │ │
│  │                          │                                              │ │
│  │                          ▼                                              │ │
│  │  ┌─────────────────────────────────────────────────────────────────┐   │ │
│  │  │  SearchResults                                                  │   │ │
│  │  │  - matches: Vec<Match>  (start_offset, end_offset, line)       │   │ │
│  │  │  - current_index: Option<usize>                                │   │ │
│  │  │  - total_count: usize                                          │   │ │
│  │  └─────────────────────────────────────────────────────────────────┘   │ │
│  └────────────────────────────────────────────────────────────────────────┘ │
│                                 │                                            │
│                                 ▼                                            │
│  ┌────────────────────────────────────────────────────────────────────────┐ │
│  │                      Highlight Rendering                                │ │
│  │                                                                         │ │
│  │  Document Text:  The quick brown fox jumps over the lazy dog.          │ │
│  │                      ═════         ═══                                  │ │
│  │  Highlight:      [match]      [current match]                          │ │
│  │                  (dim)        (bright + border)                         │ │
│  └────────────────────────────────────────────────────────────────────────┘ │
│                                                                              │
└──────────────────────────────────────────────────────────────────────────────┘
```

### Search Pipeline

```
Query Input
     │
     ▼
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│  Parse Query    │────▶│  Build Pattern  │────▶│  Execute Search │
│                 │     │                 │     │                 │
│  - Validate     │     │  - Literal or   │     │  - Find all     │
│  - Check regex  │     │    Regex        │     │    matches      │
│  - Escape chars │     │  - Case flags   │     │  - Cache results│
│                 │     │  - Word bounds  │     │                 │
└─────────────────┘     └─────────────────┘     └─────────────────┘
                                                       │
                                                       ▼
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│  Update Matches │◀────│  Invalidate on  │◀────│  Document Edit  │
│  in Viewport    │     │  Document Change│     │  (any)          │
└─────────────────┘     └─────────────────┘     └─────────────────┘
```

---

## Data Structures

### Search Query

```rust
// src/search.rs

use regex::Regex;

/// A compiled search query with all options
#[derive(Debug, Clone)]
pub struct SearchQuery {
    /// Original pattern string
    pub pattern: String,
    /// Case-sensitive matching
    pub case_sensitive: bool,
    /// Match whole words only
    pub whole_word: bool,
    /// Interpret pattern as regex
    pub is_regex: bool,
    /// Search only within selection
    pub selection_only: bool,
    /// Compiled regex (if valid)
    compiled: Option<Regex>,
    /// Error message if regex compilation failed
    pub error: Option<String>,
}

impl SearchQuery {
    /// Create a new search query
    pub fn new(
        pattern: String,
        case_sensitive: bool,
        whole_word: bool,
        is_regex: bool,
        selection_only: bool,
    ) -> Self {
        let mut query = Self {
            pattern: pattern.clone(),
            case_sensitive,
            whole_word,
            is_regex,
            selection_only,
            compiled: None,
            error: None,
        };

        query.compile();
        query
    }

    /// Compile the search pattern
    fn compile(&mut self) {
        if self.pattern.is_empty() {
            self.compiled = None;
            self.error = None;
            return;
        }

        let pattern = if self.is_regex {
            self.pattern.clone()
        } else {
            // Escape regex special characters for literal search
            regex::escape(&self.pattern)
        };

        // Add word boundaries if whole word matching
        let pattern = if self.whole_word {
            format!(r"\b{}\b", pattern)
        } else {
            pattern
        };

        // Build regex with case sensitivity flag
        let regex_pattern = if self.case_sensitive {
            pattern
        } else {
            format!("(?i){}", pattern)
        };

        match Regex::new(&regex_pattern) {
            Ok(re) => {
                self.compiled = Some(re);
                self.error = None;
            }
            Err(e) => {
                self.compiled = None;
                self.error = Some(e.to_string());
            }
        }
    }

    /// Check if query is valid and ready to search
    pub fn is_valid(&self) -> bool {
        !self.pattern.is_empty() && self.compiled.is_some()
    }

    /// Check if query has an error
    pub fn has_error(&self) -> bool {
        self.error.is_some()
    }

    /// Find all matches in text
    pub fn find_all(&self, text: &str) -> Vec<Match> {
        let Some(regex) = &self.compiled else {
            return Vec::new();
        };

        regex
            .find_iter(text)
            .map(|m| Match {
                start: m.start(),
                end: m.end(),
                line: text[..m.start()].chars().filter(|c| *c == '\n').count(),
            })
            .collect()
    }

    /// Find all matches in text within a specific range
    pub fn find_in_range(&self, text: &str, start: usize, end: usize) -> Vec<Match> {
        let Some(regex) = &self.compiled else {
            return Vec::new();
        };

        let slice = &text[start..end.min(text.len())];
        regex
            .find_iter(slice)
            .map(|m| Match {
                start: start + m.start(),
                end: start + m.end(),
                line: text[..start + m.start()].chars().filter(|c| *c == '\n').count(),
            })
            .collect()
    }
}

/// A single match in the document
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Match {
    /// Start byte offset in document
    pub start: usize,
    /// End byte offset in document (exclusive)
    pub end: usize,
    /// Line number containing the match (0-indexed)
    pub line: usize,
}

impl Match {
    /// Get the length of the match in bytes
    pub fn len(&self) -> usize {
        self.end - self.start
    }

    /// Check if match is empty
    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }

    /// Check if a position is within this match
    pub fn contains(&self, offset: usize) -> bool {
        offset >= self.start && offset < self.end
    }
}
```

### Search Results

```rust
// src/search.rs

/// Cached search results for a document
#[derive(Debug, Clone)]
pub struct SearchResults {
    /// All matches found
    pub matches: Vec<Match>,
    /// Index of currently focused match (for navigation)
    pub current_index: Option<usize>,
    /// Document revision when search was performed
    pub revision: u64,
    /// Query that produced these results
    pub query: SearchQuery,
}

impl SearchResults {
    /// Create empty results
    pub fn empty() -> Self {
        Self {
            matches: Vec::new(),
            current_index: None,
            revision: 0,
            query: SearchQuery::new(String::new(), false, false, false, false),
        }
    }

    /// Perform search and create results
    pub fn search(query: SearchQuery, text: &str, revision: u64) -> Self {
        let matches = query.find_all(text);
        Self {
            matches,
            current_index: None,
            revision,
            query,
        }
    }

    /// Get total match count
    pub fn count(&self) -> usize {
        self.matches.len()
    }

    /// Get current match (if any)
    pub fn current_match(&self) -> Option<&Match> {
        self.current_index.and_then(|i| self.matches.get(i))
    }

    /// Get display string for match count (e.g., "3 of 42")
    pub fn count_display(&self) -> String {
        if self.matches.is_empty() {
            "No matches".to_string()
        } else if let Some(idx) = self.current_index {
            format!("{} of {}", idx + 1, self.matches.len())
        } else {
            format!("{} matches", self.matches.len())
        }
    }

    /// Move to next match
    pub fn next(&mut self) {
        if self.matches.is_empty() {
            return;
        }
        self.current_index = Some(match self.current_index {
            Some(i) => (i + 1) % self.matches.len(),
            None => 0,
        });
    }

    /// Move to previous match
    pub fn previous(&mut self) {
        if self.matches.is_empty() {
            return;
        }
        self.current_index = Some(match self.current_index {
            Some(i) => {
                if i == 0 {
                    self.matches.len() - 1
                } else {
                    i - 1
                }
            }
            None => self.matches.len() - 1,
        });
    }

    /// Find the match nearest to a position and set it as current
    pub fn set_current_near(&mut self, offset: usize) {
        if self.matches.is_empty() {
            self.current_index = None;
            return;
        }

        // Find match containing offset, or next match after offset
        let idx = self.matches.iter().position(|m| m.end > offset);
        self.current_index = idx.or(Some(0));
    }

    /// Get matches visible in a line range
    pub fn matches_in_lines(&self, start_line: usize, end_line: usize) -> Vec<&Match> {
        self.matches
            .iter()
            .filter(|m| m.line >= start_line && m.line <= end_line)
            .collect()
    }

    /// Invalidate results (e.g., after document edit)
    pub fn invalidate(&mut self) {
        self.matches.clear();
        self.current_index = None;
        self.revision = 0;
    }
}
```

### Enhanced FindReplaceState

```rust
// Updated in src/model/ui.rs

use crate::search::{SearchQuery, SearchResults};

/// State for the find/replace modal
#[derive(Debug, Clone)]
pub struct FindReplaceState {
    /// Editable state for the query field
    pub query_editable: EditableState<StringBuffer>,
    /// Editable state for the replacement field
    pub replace_editable: EditableState<StringBuffer>,
    /// Which field is currently focused
    pub focused_field: FindReplaceField,
    /// Whether replace mode is active (vs find-only)
    pub replace_mode: bool,

    // === Search Options ===
    /// Case-sensitive search
    pub case_sensitive: bool,
    /// Match whole words only
    pub whole_word: bool,
    /// Use regular expressions
    pub use_regex: bool,
    /// Search within selection only
    pub selection_only: bool,

    // === Search State ===
    /// Current search results
    pub results: SearchResults,
    /// Selection range when search started (for selection_only mode)
    pub original_selection: Option<(usize, usize)>,
}

impl Default for FindReplaceState {
    fn default() -> Self {
        Self {
            query_editable: EditableState::new(StringBuffer::new(), EditConstraints::single_line()),
            replace_editable: EditableState::new(StringBuffer::new(), EditConstraints::single_line()),
            focused_field: FindReplaceField::Query,
            replace_mode: false,
            case_sensitive: false,
            whole_word: false,
            use_regex: false,
            selection_only: false,
            results: SearchResults::empty(),
            original_selection: None,
        }
    }
}

impl FindReplaceState {
    /// Build search query from current options
    pub fn build_query(&self) -> SearchQuery {
        SearchQuery::new(
            self.query_editable.text(),
            self.case_sensitive,
            self.whole_word,
            self.use_regex,
            self.selection_only,
        )
    }

    /// Perform search and update results
    pub fn search(&mut self, text: &str, revision: u64) {
        let query = self.build_query();
        self.results = if self.selection_only {
            if let Some((start, end)) = self.original_selection {
                let mut results = SearchResults::search(query, text, revision);
                // Filter to selection range
                results.matches.retain(|m| m.start >= start && m.end <= end);
                results
            } else {
                SearchResults::search(query, text, revision)
            }
        } else {
            SearchResults::search(query, text, revision)
        };
    }

    /// Toggle case sensitivity and re-search
    pub fn toggle_case_sensitive(&mut self) {
        self.case_sensitive = !self.case_sensitive;
    }

    /// Toggle whole word matching
    pub fn toggle_whole_word(&mut self) {
        self.whole_word = !self.whole_word;
    }

    /// Toggle regex mode
    pub fn toggle_regex(&mut self) {
        self.use_regex = !self.use_regex;
    }

    /// Toggle selection-only mode
    pub fn toggle_selection_only(&mut self) {
        self.selection_only = !self.selection_only;
    }

    /// Check if there's a regex error
    pub fn has_regex_error(&self) -> bool {
        self.results.query.has_error()
    }

    /// Get regex error message
    pub fn regex_error(&self) -> Option<&str> {
        self.results.query.error.as_deref()
    }
}
```

### Theme Extensions

```rust
// Add to src/theme.rs

/// Colors for search match highlighting
#[derive(Debug, Clone)]
pub struct SearchHighlightTheme {
    /// Background color for matches (not current)
    pub match_background: Color,
    /// Border color for matches
    pub match_border: Color,
    /// Background color for current match
    pub current_match_background: Color,
    /// Border color for current match
    pub current_match_border: Color,
}

impl Default for SearchHighlightTheme {
    fn default() -> Self {
        Self {
            match_background: Color::rgba(0xFF, 0xE0, 0x00, 0x40), // Semi-transparent yellow
            match_border: Color::rgb(0xFF, 0xE0, 0x00),
            current_match_background: Color::rgba(0xFF, 0xA5, 0x00, 0x60), // Orange
            current_match_border: Color::rgb(0xFF, 0xA5, 0x00),
        }
    }
}
```

---

## Keybindings

| Action | Mac | Windows/Linux | Notes |
|--------|-----|---------------|-------|
| Open Find | Cmd+F | Ctrl+F | Open find bar |
| Find Next | Cmd+G / Enter | F3 / Enter | Go to next match |
| Find Previous | Shift+Cmd+G | Shift+F3 | Go to previous match |
| Toggle Case | Option+Cmd+C | Alt+C | Toggle case sensitivity |
| Toggle Whole Word | Option+Cmd+W | Alt+W | Toggle whole word |
| Toggle Regex | Option+Cmd+R | Alt+R | Toggle regex mode |
| Toggle Selection | Option+Cmd+L | Alt+L | Toggle selection scope |
| Close | Escape | Escape | Close find bar |

---

## Implementation Plan

### Phase 1: Search Engine

**Files:** `src/search.rs`

- [x] Create `SearchQuery` struct with compilation
- [x] Implement literal search with case sensitivity
- [x] Add whole word matching with `\b` boundaries
- [x] Add regex support with error handling
- [~] Create `Match` and `SearchResults` types — `Match` shipped as specified (char offsets, not byte offsets like the doc's sketch — this codebase's `Document` API is char-offset throughout). `SearchResults` (the cached-results/current-index/revision wrapper) was **not** built: there is no caching layer at all — `Document::search_matches` recomputes from the live buffer on every call, exactly like the pre-existing `find_all_occurrences_with_options` it sits next to. See Phase 3 note for why.
- [x] Add unit tests for search patterns

**Test:** `SearchQuery::find_all("hello", "hello world hello")` returns 2 matches — see `search::tests::literal_search_is_case_insensitive_by_default`.

**Status:** Real and live — `find_next_in_document`/`find_prev_in_document`/`replace_and_find_next`/`replace_all` (src/update/ui.rs) and the decoration/tick builders (src/view/mod.rs) all route through `Document::search_matches`/`SearchQuery`, replacing the old literal-only `find_*_occurrence_with_options` call sites.

### Phase 2: Enhanced State

**Files:** `src/model/ui.rs`

- [x] Add search option fields to `FindReplaceState` — `whole_word`, `use_regex` (alongside the pre-existing `case_sensitive`).
- [ ] Add `SearchResults` field — not built; see Phase 1 note (no caching layer).
- [x] Implement `build_query()` method
- [~] Implement `search()` method — no stateful `search()`/results cache exists (see above); `build_query()` + `Document::search_matches` is the equivalent stateless call.
- [x] Add toggle methods for each option — `ModalMsg::ToggleFindReplaceWholeWord`/`ToggleFindReplaceRegex` added, mirroring the pre-existing (already-shipped but keyboard-unreachable) `ToggleFindReplaceCaseSensitive`. **Not done:** no keybinding or button drives these two new toggles either — same gap `case_sensitive` already had before this unit; Phase 5 (buttons) is the only place a user could reach them, and that's explicitly out of scope here. They're reachable today via `ModalMsg` dispatch (automation/tests) only.

**Test:** Toggling case sensitivity updates search results — exercised transitively via `search::tests` (options feed `SearchQuery::new` identically regardless of toggle path) and `view::find_match_decoration_tests`.

### Phase 3: Incremental Search

**Files:** `src/update/modal.rs`

- [~] Re-search on query input change (debounced) — happens for free: every render recomputes `active_find_matches` from the live query text, no debounce needed since there's no caching layer to invalidate.
- [~] Re-search on option toggle — same: stateless recompute makes this automatic.
- [ ] Invalidate results on document edit — not applicable; nothing is cached to go stale.
- [x] Lazy re-search on next navigation — `find_next_in_document`/`find_prev_in_document` call `Document::search_matches` fresh each time, same as the pre-existing behavior.

**Deviation:** the doc's `SearchResults` caching design (Phase 1–3) was deliberately skipped in favor of always recomputing from the buffer, matching the codebase's existing find/replace precedent (`find_all_occurrences_with_options` was never cached either). This trades a small amount of redundant scanning per redraw for a much smaller diff and zero staleness bugs; a cache is the natural next step if profiling ever shows find-heavy redraws on huge files are slow.

**Test:** Typing in find bar updates match count in real-time — not directly testable without Phase 5 UI (no match-count display exists to assert against); the underlying live-recompute behavior is covered by `view::find_match_decoration_tests`.

### Phase 4: Match Highlighting

**Files:** `src/view/mod.rs`, `src/view/editor_text.rs`, `src/view/editor_scrollbars.rs`, `src/model/decorations.rs`

- [ ] Add `SearchHighlightTheme` to theme — **not done, by design.** `DecorationKind::BackgroundTint`'s own doc comment (editor-decorations.md) already scopes it to "Find matches, documentHighlight, bracket match" sharing one tint; match highlighting reuses the existing `theme.editor.bracket_match_background` instead of adding a parallel color struct nobody themes independently yet. Per the assignment brief, the decoration doc's shapes win where the two docs disagree.
- [x] Render match highlights in visible viewport through the text decoration pipeline — `view::find_match_decorations` builds `BackgroundTint` `RangeDecoration`s from `Document::search_matches`, wired into `render_text_area` for the focused pane only (find navigation only ever targets the focused pane). Off-screen matches are cheap no-ops (`render_one_decoration` clips to the viewport already).
- [x] Distinguish current match from other matches — the match under the current selection (set by find-next/find-previous) is excluded from the tint list, since the ordinary selection-background pass already highlights it distinctly; tinting it again would just muddy the color.
- [x] Use semi-transparent overlays for readability — `BackgroundTint` paints via `Frame::blend_rect_px`, and `bracket_match_background` is already a translucent color in the bundled themes.
- [x] (Beyond the doc's Phase 4 list, but explicitly named as in-scope by editor-decorations.md's producer table) Scrollbar overview marks — `view::find_match_ticks` + a new `Mark::Match` variant (`src/model/decorations.rs`) feed `editor_scrollbars::render_overview_marks` with one tick per match's starting line, for the focused pane.

**Test:** All matches visible in viewport are highlighted — `view::find_match_decoration_tests` (`find_match_decorations_excludes_the_current_selection`, `find_match_ticks_map_matches_to_their_starting_line`, etc.).

### Phase 5: UI Rendering

**Files:** `src/view/modal.rs`

**Out of scope for this unit** (per assignment brief: "this unit is about search behavior + decorations, not modal chrome"). None of this phase's checkboxes were attempted; the find/replace modal's visual chrome is unchanged from the OverlaySurface Fields-body migration. `whole_word`/`use_regex` toggles and regex-error/match-count display all need this phase's button/label rendering to become reachable by a human user.

### Phase 6: Navigation

**Files:** `src/update/ui.rs`, `src/runtime/input.rs`

- [x] Find Next jumps to next match, scrolling if needed — pre-existing, now routed through `SearchQuery`/`Document::search_matches`.
- [x] Find Previous jumps to previous match — the logic already existed (`find_prev_in_document`) but was **keyboard-unreachable** (no key dispatched `ModalMsg::FindPrevious`); this unit wires Shift+Enter to it in `handle_modal_key`.
- [x] Wrap around at document boundaries — pre-existing behavior, preserved (first/last match on no match past the cursor).
- [ ] Set current match near cursor on first search — not implemented; find-next/find-previous already start from the cursor/selection position on every call (not just "first search"), so a separate "on first search" special case wasn't needed for the shipped behavior, but the doc's specific `SearchResults::set_current_near` API doesn't exist since there's no `SearchResults` cache (see Phase 1).

**Also wired (not in the doc's Phase 6 list but a direct consequence of making Find Previous reachable):** Tab now toggles between the query/replace fields (`ModalMsg::ToggleFindReplaceField`, previously also keyboard-unreachable).

**Test:** Find Next cycles through all matches — pre-existing document.rs find tests, plus the new `search.rs`/`view::find_match_decoration_tests` coverage of the underlying engine.

### Phase 7: Selection Scope

**Files:** `src/update/ui.rs`

**Not implemented.** `selection_only` scoping depends on a UI affordance to turn it on (Phase 5, out of scope) exactly like `whole_word`/`use_regex`, and unlike those two, filtering an already-computed match list to a stored selection range is cheap to add later with zero risk to the shipped behavior — deferred rather than adding an unreachable field with no consumer.

---

## Testing Strategy

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_literal_search() {
        let query = SearchQuery::new("hello".to_string(), false, false, false, false);
        let matches = query.find_all("Hello world, hello there");
        assert_eq!(matches.len(), 2); // Case insensitive
    }

    #[test]
    fn test_case_sensitive() {
        let query = SearchQuery::new("Hello".to_string(), true, false, false, false);
        let matches = query.find_all("Hello world, hello there");
        assert_eq!(matches.len(), 1); // Only uppercase match
    }

    #[test]
    fn test_whole_word() {
        let query = SearchQuery::new("the".to_string(), false, true, false, false);
        let matches = query.find_all("the other there");
        assert_eq!(matches.len(), 1); // Only "the", not "there"
    }

    #[test]
    fn test_regex_search() {
        let query = SearchQuery::new(r"\d+".to_string(), false, false, true, false);
        let matches = query.find_all("abc 123 def 456 ghi");
        assert_eq!(matches.len(), 2);
    }

    #[test]
    fn test_invalid_regex() {
        let query = SearchQuery::new(r"[invalid".to_string(), false, false, true, false);
        assert!(query.has_error());
        assert!(!query.is_valid());
    }

    #[test]
    fn test_match_navigation() {
        let mut results = SearchResults {
            matches: vec![
                Match { start: 0, end: 5, line: 0 },
                Match { start: 10, end: 15, line: 0 },
                Match { start: 20, end: 25, line: 1 },
            ],
            current_index: Some(0),
            revision: 1,
            query: SearchQuery::new("test".to_string(), false, false, false, false),
        };

        results.next();
        assert_eq!(results.current_index, Some(1));

        results.next();
        assert_eq!(results.current_index, Some(2));

        results.next(); // Wrap around
        assert_eq!(results.current_index, Some(0));
    }

    #[test]
    fn test_count_display() {
        let results = SearchResults {
            matches: vec![
                Match { start: 0, end: 5, line: 0 },
                Match { start: 10, end: 15, line: 0 },
            ],
            current_index: Some(0),
            revision: 1,
            query: SearchQuery::new("test".to_string(), false, false, false, false),
        };

        assert_eq!(results.count_display(), "1 of 2");
    }
}
```

### Integration Tests

```rust
// tests/find_tests.rs

#[test]
fn test_find_highlights_all_matches() {
    // Open document with multiple occurrences
    // Open find bar
    // Type search term
    // Verify all matches are highlighted
}

#[test]
fn test_find_next_scrolls_to_match() {
    // Open long document
    // Search for term at bottom
    // Verify viewport scrolls to match
}

#[test]
fn test_regex_error_display() {
    // Open find bar
    // Toggle regex mode
    // Enter invalid regex
    // Verify error message displayed
}

#[test]
fn test_selection_only_scope() {
    // Select portion of document
    // Open find
    // Toggle selection only
    // Verify matches outside selection not found
}
```

---

## References

- **Existing code:** `src/model/ui.rs` - `FindReplaceState`
- **Regex crate:** `regex` for pattern matching
- **VS Code:** Find bar with options and highlighting
- **Sublime Text:** Incremental search with regex support
- **Theme:** `src/theme.rs` - Color definitions
- **Rendering:** `src/view/editor_text.rs` - Text rendering / viewport highlights
