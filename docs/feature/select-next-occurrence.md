# Select Next Occurrence

Multi-cursor selection at next matching text (Cmd+D style).

> **Status:** Planned
> **Priority:** P1
> **Effort:** S
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

The editor has multi-cursor support and an occurrence selection feature. In `src/model/editor.rs`:

```rust
pub struct OccurrenceState {
    pub search_text: String,
    pub added_cursor_indices: Vec<usize>,
    pub last_search_offset: usize,
}
```

And in `src/keymap/command.rs`:
```rust
Command::SelectNextOccurrence,
Command::UnselectOccurrence,
// Note: SelectAllOccurrences exists as EditorMsg but is not yet wired to a Command
```

These commands exist but may need enhancement for full VS Code-style behavior.

### Goals

1. **Select word under cursor** - If no selection, select current word first
2. **Add cursor at next match** - Find next occurrence and add cursor with selection
3. **Wrap around** - Continue from document start after reaching end
4. **Unselect last** - Remove most recently added occurrence (Shift+Cmd+D)
5. **Select all occurrences** - Add cursors at all matches (Cmd+Shift+L)
6. **Case sensitivity** - Match case of original selection
7. **Whole word** - Auto-enable whole word when selecting from word

### Non-Goals

- Find/replace integration (separate feature F-050/F-060)
- Fuzzy matching for occurrences
- Cross-file occurrence selection

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                     Select Next Occurrence Flow                              │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ┌────────────────────────────────────────────────────────────────────────┐ │
│  │                          Initial State                                  │ │
│  │                                                                         │ │
│  │  Cursor Position (no selection)    OR    Text Selected                 │ │
│  │         │                                      │                        │ │
│  │         ▼                                      ▼                        │ │
│  │  ┌────────────────┐                    ┌────────────────┐              │ │
│  │  │ Expand to word │                    │ Use selection  │              │ │
│  │  │ under cursor   │                    │ as search text │              │ │
│  │  └────────────────┘                    └────────────────┘              │ │
│  │         │                                      │                        │ │
│  │         └──────────────────┬───────────────────┘                        │ │
│  │                            ▼                                            │ │
│  │                   ┌────────────────┐                                    │ │
│  │                   │ Set search_text│                                    │ │
│  │                   │ in Occurrence- │                                    │ │
│  │                   │ State          │                                    │ │
│  │                   └────────────────┘                                    │ │
│  └────────────────────────────────────────────────────────────────────────┘ │
│                                │                                             │
│                                ▼                                             │
│  ┌────────────────────────────────────────────────────────────────────────┐ │
│  │                          Cmd+D (Select Next)                            │ │
│  │                                                                         │ │
│  │  ┌─────────────────────────────────────────────────────────────────┐   │ │
│  │  │  Search from last_search_offset                                 │   │ │
│  │  │  ─────────────────────────────────────────────────────────────  │   │ │
│  │  │  1. Find next occurrence of search_text                         │   │ │
│  │  │  2. If not found, wrap to document start                        │   │ │
│  │  │  3. If still not found (or already have all), stop              │   │ │
│  │  │  4. Add cursor with selection at match                          │   │ │
│  │  │  5. Record cursor index in added_cursor_indices                 │   │ │
│  │  │  6. Update last_search_offset to end of match                   │   │ │
│  │  │  7. Scroll to show new cursor if needed                         │   │ │
│  │  └─────────────────────────────────────────────────────────────────┘   │ │
│  └────────────────────────────────────────────────────────────────────────┘ │
│                                │                                             │
│                                ▼                                             │
│  ┌────────────────────────────────────────────────────────────────────────┐ │
│  │                          Shift+Cmd+D (Unselect)                         │ │
│  │                                                                         │ │
│  │  ┌─────────────────────────────────────────────────────────────────┐   │ │
│  │  │  Pop last entry from added_cursor_indices                       │   │ │
│  │  │  Remove that cursor (preserving primary if it's not the last)   │   │ │
│  │  │  Update last_search_offset to previous match                    │   │ │
│  │  └─────────────────────────────────────────────────────────────────┘   │ │
│  └────────────────────────────────────────────────────────────────────────┘ │
│                                                                              │
└──────────────────────────────────────────────────────────────────────────────┘
```

### Visual Example

```
Document: "The quick brown fox jumps over the lazy fox"

Step 1: Cursor on "fox", no selection
        The quick brown fox jumps over the lazy fox
                        ^^^                     ^^^
                        cursor here

Step 2: Cmd+D (first time) - selects word under cursor
        The quick brown [fox] jumps over the lazy fox
                        ═════
                        selected

Step 3: Cmd+D (second time) - adds cursor at next "fox"
        The quick brown [fox] jumps over the lazy [fox]
                        ═════                     ═════
                        primary                   new cursor

Step 4: Shift+Cmd+D - removes last added cursor
        The quick brown [fox] jumps over the lazy fox
                        ═════
                        back to single cursor
```

---

## Data Structures

### Enhanced OccurrenceState

```rust
// src/model/editor.rs

/// Tracks occurrence selection state for Cmd+D (select next occurrence)
#[derive(Debug, Clone, Default)]
pub struct OccurrenceState {
    /// The text being searched for
    pub search_text: String,
    /// Stack of cursor indices added via Cmd+D (for undo with Shift+Cmd+D)
    /// Stored in order of addition, so last element is most recent
    pub added_cursor_indices: Vec<usize>,
    /// Byte offset after the last found match (for finding "next")
    pub last_search_offset: usize,
    /// Whether to match whole words only
    pub whole_word: bool,
    /// Whether to match case
    pub case_sensitive: bool,
    /// All match positions in document (cached for Select All Occurrences)
    pub all_matches: Option<Vec<(usize, usize)>>,
}

impl OccurrenceState {
    /// Create a new occurrence state for a search term
    pub fn new(search_text: String, initial_offset: usize, whole_word: bool) -> Self {
        Self {
            search_text,
            added_cursor_indices: Vec::new(),
            last_search_offset: initial_offset,
            whole_word,
            case_sensitive: true, // Default to case sensitive
            all_matches: None,
        }
    }

    /// Check if the search text has mixed case (to determine case sensitivity)
    pub fn infer_case_sensitivity(text: &str) -> bool {
        text.chars().any(|c| c.is_uppercase())
    }

    /// Record that a cursor was added
    pub fn push_cursor(&mut self, cursor_index: usize) {
        self.added_cursor_indices.push(cursor_index);
    }

    /// Remove the most recently added cursor index
    pub fn pop_cursor(&mut self) -> Option<usize> {
        self.added_cursor_indices.pop()
    }

    /// Check if we can unselect (have added cursors)
    pub fn can_unselect(&self) -> bool {
        !self.added_cursor_indices.is_empty()
    }

    /// Clear cached matches (call when document changes)
    pub fn invalidate_cache(&mut self) {
        self.all_matches = None;
    }
}
```

### Occurrence Selection Commands

```rust
// src/update/editor.rs

impl EditorState {
    /// Select next occurrence of current word/selection (Cmd+D)
    ///
    /// If no selection: select word under cursor
    /// If selection exists: find and add cursor at next occurrence
    pub fn select_next_occurrence(&mut self, doc: &Document) {
        // Get current selection text (or word under cursor)
        let (search_text, start_offset) = if self.primary_selection().is_empty() {
            // No selection - select word under cursor first
            if let Some((word, start_pos, end_pos)) = self.word_under_cursor(doc) {
                // Select the word
                let start = Position::new(start_pos.line, start_pos.column);
                let end = Position::new(end_pos.line, end_pos.column);
                self.selections[0] = Selection::from_anchor_head(start, end);
                self.cursors[0] = Cursor::from_position(end);

                // Initialize occurrence state
                let offset = doc.cursor_to_offset(end_pos.line, end_pos.column);
                self.occurrence_state = Some(OccurrenceState::new(
                    word.clone(),
                    offset,
                    true, // whole word when selecting from cursor
                ));

                return; // First Cmd+D just selects the word
            } else {
                return; // No word under cursor
            }
        } else {
            // Have selection - use it as search text
            let sel = self.primary_selection();
            let text = sel.get_text(doc);
            let offset = doc.cursor_to_offset(sel.end().line, sel.end().column);
            (text, offset)
        };

        // Initialize or update occurrence state
        let state = self.occurrence_state.get_or_insert_with(|| {
            OccurrenceState::new(
                search_text.clone(),
                start_offset,
                false, // not whole word when using existing selection
            )
        });

        // If search text changed, reset
        if state.search_text != search_text {
            *state = OccurrenceState::new(search_text.clone(), start_offset, false);
        }

        // Find next occurrence
        if let Some((match_start, match_end)) = self.find_next_occurrence(doc, state) {
            // Check if we already have a cursor at this position
            let match_end_pos = doc.offset_to_cursor(match_end);
            let already_exists = self.cursors.iter().any(|c| {
                c.line == match_end_pos.0 && c.column == match_end_pos.1
            });

            if !already_exists {
                // Add cursor with selection at match
                let start_pos = doc.offset_to_cursor(match_start);
                let end_pos = doc.offset_to_cursor(match_end);

                let new_cursor = Cursor::at(end_pos.0, end_pos.1);
                let new_selection = Selection::from_anchor_head(
                    Position::new(start_pos.0, start_pos.1),
                    Position::new(end_pos.0, end_pos.1),
                );

                self.cursors.push(new_cursor);
                self.selections.push(new_selection);

                // Record the cursor index
                let cursor_idx = self.cursors.len() - 1;
                if let Some(ref mut occ) = self.occurrence_state {
                    occ.push_cursor(cursor_idx);
                    occ.last_search_offset = match_end;
                }

                // Sort and deduplicate
                self.sort_cursors();
                self.deduplicate_cursors();
            }
        }
    }

    /// Find next occurrence from current offset
    fn find_next_occurrence(
        &self,
        doc: &Document,
        state: &OccurrenceState,
    ) -> Option<(usize, usize)> {
        let text = doc.buffer.to_string();
        let search = &state.search_text;

        if search.is_empty() {
            return None;
        }

        // Build search pattern
        let pattern = if state.whole_word {
            format!(r"\b{}\b", regex::escape(search))
        } else {
            regex::escape(search)
        };

        let regex_pattern = if state.case_sensitive {
            pattern
        } else {
            format!("(?i){}", pattern)
        };

        let regex = regex::Regex::new(&regex_pattern).ok()?;

        // Search from last offset
        let search_start = state.last_search_offset;

        // Try to find after current position
        if let Some(m) = regex.find_at(&text, search_start) {
            return Some((m.start(), m.end()));
        }

        // Wrap around to start
        if let Some(m) = regex.find(&text) {
            // Make sure we haven't wrapped all the way around
            if m.start() < search_start {
                return Some((m.start(), m.end()));
            }
        }

        None
    }

    /// Unselect the most recently added occurrence (Shift+Cmd+D)
    pub fn unselect_occurrence(&mut self) {
        if let Some(ref mut state) = self.occurrence_state {
            if let Some(cursor_idx) = state.pop_cursor() {
                // Find and remove the cursor
                // Note: indices may have shifted due to sorting/dedup
                // We need to find by position, not index
                if cursor_idx < self.cursors.len() && self.cursors.len() > 1 {
                    self.cursors.remove(cursor_idx);
                    self.selections.remove(cursor_idx);

                    // Update active cursor index if needed
                    if self.active_cursor_index >= self.cursors.len() {
                        self.active_cursor_index = self.cursors.len() - 1;
                    }

                    // Update last_search_offset to end of previous match
                    // (so next Cmd+D continues from there)
                    // This is a simplification; ideally we'd track positions
                }
            }
        }
    }

    /// Select all occurrences of current word/selection (Cmd+Shift+L)
    pub fn select_all_occurrences(&mut self, doc: &Document) {
        // Get selection text (or word under cursor)
        let search_text = if self.primary_selection().is_empty() {
            self.word_under_cursor(doc).map(|(w, _, _)| w)
        } else {
            Some(self.primary_selection().get_text(doc))
        };

        let Some(search_text) = search_text else {
            return;
        };

        if search_text.is_empty() {
            return;
        }

        let text = doc.buffer.to_string();

        // Build pattern
        let whole_word = self.primary_selection().is_empty();
        let pattern = if whole_word {
            format!(r"\b{}\b", regex::escape(&search_text))
        } else {
            regex::escape(&search_text)
        };

        let case_sensitive = OccurrenceState::infer_case_sensitivity(&search_text);
        let regex_pattern = if case_sensitive {
            pattern
        } else {
            format!("(?i){}", pattern)
        };

        let Ok(regex) = regex::Regex::new(&regex_pattern) else {
            return;
        };

        // Find all matches
        let matches: Vec<_> = regex
            .find_iter(&text)
            .map(|m| (m.start(), m.end()))
            .collect();

        if matches.is_empty() {
            return;
        }

        // Clear existing cursors and create new ones at all matches
        self.cursors.clear();
        self.selections.clear();

        for (start, end) in &matches {
            let start_pos = doc.offset_to_cursor(*start);
            let end_pos = doc.offset_to_cursor(*end);

            self.cursors.push(Cursor::at(end_pos.0, end_pos.1));
            self.selections.push(Selection::from_anchor_head(
                Position::new(start_pos.0, start_pos.1),
                Position::new(end_pos.0, end_pos.1),
            ));
        }

        self.active_cursor_index = 0;
        self.sort_cursors();

        // Initialize occurrence state
        self.occurrence_state = Some(OccurrenceState {
            search_text,
            added_cursor_indices: (0..matches.len()).collect(),
            last_search_offset: matches.last().map(|(_, e)| *e).unwrap_or(0),
            whole_word,
            case_sensitive,
            all_matches: Some(matches),
        });
    }

    /// Clear occurrence state (called when selection changes by other means)
    pub fn clear_occurrence_state(&mut self) {
        self.occurrence_state = None;
    }
}
```

---

## Keybindings

| Action | Mac | Windows/Linux | Notes |
|--------|-----|---------------|-------|
| Select Next Occurrence | Cmd+D | Ctrl+D | Add cursor at next match |
| Unselect Occurrence | Shift+Cmd+D | Shift+Ctrl+D | Remove last added cursor |
| Select All Occurrences | Cmd+Shift+L | Ctrl+Shift+L | Cursors at all matches |
| Skip Occurrence | Cmd+K, Cmd+D | Ctrl+K, Ctrl+D | Skip current, find next |

Note: The existing `Command::SelectNextOccurrence` (mapped to Cmd+J in some configs) should be consolidated with this feature.

---

## Implementation Plan

### Phase 1: Core Selection Logic

**Files:** `src/model/editor.rs`

- [ ] Ensure `OccurrenceState` struct is complete
- [ ] Add `whole_word` and `case_sensitive` fields
- [ ] Add case sensitivity inference
- [ ] Implement `find_next_occurrence()` helper

**Test:** Finding next occurrence wraps around document.

### Phase 2: Select Next (Cmd+D)

**Files:** `src/update/editor.rs`

- [ ] Implement `select_next_occurrence()` method
- [ ] Handle first invocation (select word)
- [ ] Handle subsequent invocations (find next)
- [ ] Track added cursors in `added_cursor_indices`
- [ ] Sort and deduplicate cursors after adding

**Test:** Three Cmd+D presses create 3 cursors at 3 matches.

### Phase 3: Unselect (Shift+Cmd+D)

**Files:** `src/update/editor.rs`

- [ ] Implement `unselect_occurrence()` method
- [ ] Pop last added cursor
- [ ] Handle edge case of single cursor
- [ ] Update `last_search_offset` for continuation

**Test:** Unselect removes most recently added cursor.

### Phase 4: Select All (Cmd+Shift+L)

**Files:** `src/update/editor.rs`

- [ ] Implement `select_all_occurrences()` method
- [ ] Find all matches in document
- [ ] Create cursors with selections at all matches
- [ ] Cache match positions in occurrence state

**Test:** Select All creates cursors at every match.

### Phase 5: Command Integration

**Files:** `src/keymap/command.rs`, `src/keymap/defaults.rs`

- [ ] Verify `Command::SelectNextOccurrence` mapping
- [ ] Add `Command::SkipOccurrence` if needed
- [ ] Update default keybindings
- [ ] Handle command→message translation

**Test:** Cmd+D triggers SelectNextOccurrence command.

### Phase 6: Visual Feedback

**Files:** `src/view/editor.rs`

- [ ] Highlight all occurrences of selected text
- [ ] Use different color for occurrence highlights
- [ ] Show occurrence count in status bar
- [ ] Animate/flash new cursor position

**Test:** All occurrences highlighted when word selected.

### Phase 7: Edge Cases

**Files:** `src/update/editor.rs`

- [ ] Handle document edits (invalidate occurrence state)
- [ ] Handle cursor movement (clear state or keep?)
- [ ] Handle selection changes (update search text?)
- [ ] Handle wrap-around with all occurrences selected

**Test:** Editing document clears occurrence state.

---

## Testing Strategy

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_first_cmd_d_selects_word() {
        let doc = Document::from_string("hello world hello");
        let mut editor = EditorState::new();
        editor.cursors[0] = Cursor::at(0, 0); // Cursor on "hello"

        editor.select_next_occurrence(&doc);

        // Should select "hello"
        let sel = editor.primary_selection();
        assert!(!sel.is_empty());
        assert_eq!(sel.get_text(&doc), "hello");
    }

    #[test]
    fn test_second_cmd_d_adds_cursor() {
        let doc = Document::from_string("hello world hello");
        let mut editor = EditorState::new();
        editor.cursors[0] = Cursor::at(0, 0);

        editor.select_next_occurrence(&doc); // Select "hello"
        editor.select_next_occurrence(&doc); // Find next "hello"

        assert_eq!(editor.cursors.len(), 2);
    }

    #[test]
    fn test_unselect_removes_last() {
        let doc = Document::from_string("a a a");
        let mut editor = EditorState::new();
        editor.cursors[0] = Cursor::at(0, 0);

        editor.select_next_occurrence(&doc);
        editor.select_next_occurrence(&doc);
        editor.select_next_occurrence(&doc);

        assert_eq!(editor.cursors.len(), 3);

        editor.unselect_occurrence();

        assert_eq!(editor.cursors.len(), 2);
    }

    #[test]
    fn test_select_all_occurrences() {
        let doc = Document::from_string("foo bar foo baz foo");
        let mut editor = EditorState::new();
        editor.cursors[0] = Cursor::at(0, 0);

        editor.select_all_occurrences(&doc);

        assert_eq!(editor.cursors.len(), 3);
    }

    #[test]
    fn test_case_sensitivity_inference() {
        assert!(OccurrenceState::infer_case_sensitivity("Hello")); // Has uppercase
        assert!(!OccurrenceState::infer_case_sensitivity("hello")); // All lowercase
    }

    #[test]
    fn test_wrap_around() {
        let doc = Document::from_string("foo bar");
        let mut editor = EditorState::new();
        // Start with selection at end
        editor.cursors[0] = Cursor::at(0, 7);
        editor.selections[0] = Selection::new(Position::new(0, 7));

        // Searching for "foo" should wrap to start
        // (This test depends on implementation details)
    }
}
```

### Integration Tests

```rust
// tests/occurrence_tests.rs

#[test]
fn test_occurrence_selection_flow() {
    // Type some text with repeated words
    // Place cursor on a word
    // Press Cmd+D multiple times
    // Verify correct number of cursors
    // Press Shift+Cmd+D
    // Verify cursor removed
}

#[test]
fn test_occurrence_with_editing() {
    // Select next occurrence
    // Type replacement text
    // Verify all occurrences replaced
}

#[test]
fn test_select_all_then_edit() {
    // Cmd+Shift+L to select all
    // Type replacement
    // Verify all replaced
    // Undo
    // Verify all restored
}
```

---

## References

- **Existing code:** `src/model/editor.rs` - `OccurrenceState`
- **Commands:** `src/keymap/command.rs` - `SelectNextOccurrence`
- **Multi-cursor:** `src/model/editor.rs` - Cursor and selection management
- **VS Code:** Cmd+D / Ctrl+D behavior
- **Sublime Text:** "Quick Add Next" functionality
- **Find enhancement:** F-050 for search infrastructure
