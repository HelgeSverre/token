# Find Enhancements

Highlight all matches, regex support, whole word matching, and case sensitivity toggle.

> **Status:** Planned
> **Priority:** P1
> **Effort:** M
> **Created:** 2025-12-19
> **Milestone:** 2 - Search & Editing

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

### Current State

The Find/Replace modal exists in `src/model/ui.rs` as `FindReplaceState`:

```rust
pub struct FindReplaceState {
    pub query_editable: EditableState<StringBuffer>,
    pub replace_editable: EditableState<StringBuffer>,
    pub focused_field: FindReplaceField,
    pub replace_mode: bool,
    pub case_sensitive: bool,
}
```

Current capabilities:
- Basic text search with case sensitivity toggle
- Single match navigation (Find Next / Find Previous)
- Simple replace functionality

Missing features:
- No visual highlighting of all matches in document
- No regex support
- No whole word matching option
- No match count display

### Goals

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

- [ ] Create `SearchQuery` struct with compilation
- [ ] Implement literal search with case sensitivity
- [ ] Add whole word matching with `\b` boundaries
- [ ] Add regex support with error handling
- [ ] Create `Match` and `SearchResults` types
- [ ] Add unit tests for search patterns

**Test:** `SearchQuery::find_all("hello", "hello world hello")` returns 2 matches.

### Phase 2: Enhanced State

**Files:** `src/model/ui.rs`

- [ ] Add search option fields to `FindReplaceState`
- [ ] Add `SearchResults` field
- [ ] Implement `build_query()` method
- [ ] Implement `search()` method
- [ ] Add toggle methods for each option

**Test:** Toggling case sensitivity updates search results.

### Phase 3: Incremental Search

**Files:** `src/update/modal.rs`

- [ ] Re-search on query input change (debounced)
- [ ] Re-search on option toggle
- [ ] Invalidate results on document edit
- [ ] Lazy re-search on next navigation

**Test:** Typing in find bar updates match count in real-time.

### Phase 4: Match Highlighting

**Files:** `src/view/editor.rs`, `src/theme.rs`

- [ ] Add `SearchHighlightTheme` to theme
- [ ] Render match highlights in visible viewport
- [ ] Distinguish current match from other matches
- [ ] Use semi-transparent overlays for readability

**Test:** All matches visible in viewport are highlighted.

### Phase 5: UI Rendering

**Files:** `src/view/modal.rs`

- [ ] Render option toggles (Aa, W, .*, =) as buttons
- [ ] Show match count ("3 of 42")
- [ ] Highlight active toggles
- [ ] Show regex error message when applicable
- [ ] Show "No matches" when query has no results

**Test:** Toggle buttons reflect current state.

### Phase 6: Navigation

**Files:** `src/update/modal.rs`

- [ ] Find Next jumps to next match, scrolling if needed
- [ ] Find Previous jumps to previous match
- [ ] Wrap around at document boundaries
- [ ] Set current match near cursor on first search

**Test:** Find Next cycles through all matches.

### Phase 7: Selection Scope

**Files:** `src/update/modal.rs`

- [ ] Store selection range when find opened
- [ ] Filter matches to selection range
- [ ] Show selection indicator in UI
- [ ] Handle selection changes gracefully

**Test:** Find with selection only matches within selection.

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
- **Rendering:** `src/view/editor.rs` - Text rendering
