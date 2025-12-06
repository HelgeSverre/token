# Multi-Cursor Selection Gaps Implementation

**Status:** 🚧 In Progress  
**Created:** 2025-12-06  
**Effort:** M (1-3 hours)

---

## Overview

This document tracks the remaining gaps between the SELECTION_MULTICURSOR.md design and actual implementation. Most multi-cursor features are complete, but several selection operations only work with the primary cursor.

### Already Implemented ✅

| Feature | Notes |
|---------|-------|
| MoveCursorWithSelection (all directions) | Uses `move_all_cursors_*_with_selection()` |
| PageUp/DownWithSelection | Multi-cursor aware |
| MoveCursorWordWithSelection | Multi-cursor aware |
| AddCursorAbove/Below | Works correctly |
| ToggleCursorAtPosition | Works |
| SelectNextOccurrence/SelectAllOccurrences | Works |
| Rectangle selection | Works |
| Expand/ShrinkSelection | Single cursor only (acceptable) |
| Multi-cursor editing (insert, delete) | Uses reverse order, batch undo |

### Gaps to Fix ❌

| Feature | Location | Issue |
|---------|----------|-------|
| **SelectWord** | update.rs:399-441 | Only operates on primary cursor |
| **SelectLine** | update.rs:444-467 | Only operates on primary cursor |
| **SelectAll** | update.rs:388-396 | Doesn't properly collapse to single cursor |
| **ExtendSelectionToPosition** | update.rs:470-488 | Only extends primary selection |
| **merge_overlapping_selections()** | Not implemented | Design doc specifies this |

---

## Design Decisions

Based on Oracle consultation:

| Feature | Behavior |
|---------|----------|
| **SelectAll** | Collapse to single cursor + single full-document selection (standard editor behavior) |
| **SelectWord** | Per-cursor word selection → then merge overlapping |
| **SelectLine** | Per-cursor line selection → then merge overlapping |
| **ExtendSelectionToPosition** | Collapse to primary cursor first, then extend |
| **Expand/ShrinkSelection** | Keep single-cursor only (complexity not worth it) |

---

## Implementation Plan

### Phase 1: `merge_overlapping_selections()` in EditorState

**Location:** `src/model/editor.rs`

**Algorithm:**
1. If ≤1 selection, return early
2. Collect `(start, end, index)` tuples for all selections
3. Sort by `start`, then `end`
4. Sweep through and merge if `next.start <= current.end`
5. For merged ranges: `merged_start = min(starts)`, `merged_end = max(ends)`
6. Create canonical forward selections with cursor at `merged_end`
7. Rebuild `cursors` and `selections` vectors

```rust
impl EditorState {
    pub fn merge_overlapping_selections(&mut self) {
        if self.selections.len() <= 1 {
            return;
        }
        
        // 1) Collect (start, end, original_index)
        let mut indexed: Vec<(Position, Position, usize)> = self.selections
            .iter()
            .enumerate()
            .map(|(i, s)| (s.start(), s.end(), i))
            .collect();
        
        // 2) Sort by start, then end
        indexed.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
        
        // 3) Sweep and merge
        let mut merged: Vec<(Position, Position)> = Vec::new();
        for (start, end, _) in indexed {
            if let Some((_, last_end)) = merged.last_mut() {
                if start <= *last_end {
                    // Overlapping or touching - extend
                    if end > *last_end {
                        *last_end = end;
                    }
                    continue;
                }
            }
            merged.push((start, end));
        }
        
        // 4) Rebuild cursors and selections
        self.cursors.clear();
        self.selections.clear();
        
        for (start, end) in merged {
            self.cursors.push(Cursor::from_position(end));
            self.selections.push(Selection::from_positions(start, end));
        }
    }
}
```

**Call sites:**
- After `SelectWord` (when multi-cursor)
- After `SelectLine` (when multi-cursor)
- Optionally before multi-cursor edits

**Tests:**
- Non-overlapping stays separate: `[0,0..0,3]`, `[0,5..0,7]` → unchanged
- Overlapping on same line: `[0,0..0,3]`, `[0,2..0,5]` → `[0,0..0,5]`
- Touching (adjacent): `[0,0..0,3]`, `[0,3..0,5]` → `[0,0..0,5]`
- Multi-line overlap: `[0,5..1,2]`, `[0,7..2,0]` → `[0,5..2,0]`
- Duplicates: two identical → one remains

---

### Phase 2: Fix `SelectAll`

**Current behavior:** Uses `cursor_mut()` and `selection_mut()` (primary only)

**New behavior:** Collapse to single cursor + single full-document selection

```rust
EditorMsg::SelectAll => {
    let doc = model.document();
    let last_line = doc.line_count().saturating_sub(1);
    let last_col = doc.line_length(last_line);
    let start = Position::new(0, 0);
    let end = Position::new(last_line, last_col);
    
    let editor = model.editor_mut();
    editor.cursors.clear();
    editor.selections.clear();
    editor.cursors.push(Cursor::from_position(end));
    editor.selections.push(Selection::from_positions(start, end));
    
    model.reset_cursor_blink();
    Some(Cmd::Redraw)
}
```

**Tests:**
- Single cursor → full doc selection
- Multiple cursors → collapses to single + full doc selection

---

### Phase 3: Multi-cursor `SelectWord`

**Add helper to EditorState:**

```rust
impl EditorState {
    /// Get word under cursor at index (refactored from word_under_cursor)
    pub fn word_under_cursor_at(
        &self,
        doc: &Document,
        idx: usize,
    ) -> Option<(String, Position, Position)> {
        let cursor = &self.cursors[idx];
        // Same logic as existing word_under_cursor() but using cursors[idx]
        // ...
    }
}
```

**Update handler:**

```rust
EditorMsg::SelectWord => {
    let doc = model.document().clone();
    {
        let editor = model.editor_mut();
        
        for i in 0..editor.cursors.len() {
            if let Some((_word, start, end)) = editor.word_under_cursor_at(&doc, i) {
                editor.selections[i].anchor = start;
                editor.selections[i].head = end;
                editor.cursors[i].line = end.line;
                editor.cursors[i].column = end.column;
                editor.cursors[i].desired_column = None;
            }
            // If no word under cursor (whitespace), leave unchanged
        }
        
        editor.merge_overlapping_selections();
    }
    
    model.ensure_cursor_visible();
    model.reset_cursor_blink();
    Some(Cmd::Redraw)
}
```

**Tests:**
- Single cursor: middle/start/end of word
- Single cursor: on whitespace (no change)
- Multi-cursor: different words → both selected
- Multi-cursor: same word → merged to one cursor/selection
- Unicode words (café, emoji)

---

### Phase 4: Multi-cursor `SelectLine`

**Update handler:**

```rust
EditorMsg::SelectLine => {
    let doc = model.document().clone();
    {
        let editor = model.editor_mut();
        let total_lines = doc.line_count();
        
        for i in 0..editor.cursors.len() {
            let line = editor.cursors[i].line;
            let line_len = doc.line_length(line);
            
            let start = Position::new(line, 0);
            let end = if line + 1 < total_lines {
                Position::new(line + 1, 0)  // Include newline
            } else {
                Position::new(line, line_len)  // Last line
            };
            
            editor.selections[i].anchor = start;
            editor.selections[i].head = end;
            editor.cursors[i].line = end.line;
            editor.cursors[i].column = end.column;
            editor.cursors[i].desired_column = None;
        }
        
        editor.merge_overlapping_selections();
    }
    
    model.ensure_cursor_visible();
    model.reset_cursor_blink();
    Some(Cmd::Redraw)
}
```

**Tests:**
- Single cursor: whole line selected
- Multi-cursor: different lines → both selected
- Multi-cursor: same line → merged to one

---

### Phase 5: Fix `ExtendSelectionToPosition`

**New behavior:** Collapse to primary cursor if multi-cursor, then extend

```rust
EditorMsg::ExtendSelectionToPosition { line, column } => {
    let new_pos = Position::new(line, column);
    {
        let editor = model.editor_mut();
        
        // If multiple cursors, collapse to primary first
        if editor.cursors.len() > 1 {
            editor.cursors.truncate(1);
            editor.selections.truncate(1);
        }
        
        // Single selection semantics
        let sel = &mut editor.selections[0];
        let cur = &mut editor.cursors[0];
        
        if sel.is_empty() {
            sel.anchor = cur.to_position();
        }
        sel.head = new_pos;
        
        cur.line = new_pos.line;
        cur.column = new_pos.column;
        cur.desired_column = None;
    }
    
    model.ensure_cursor_visible();
    model.reset_cursor_blink();
    Some(Cmd::Redraw)
}
```

**Tests:**
- Single cursor: extends selection to position
- Multi-cursor: collapses first, then extends

---

### Phase 6: Comprehensive Test Suite

Add to `tests/selection.rs`:

```rust
// === merge_overlapping_selections tests ===
#[test] fn test_merge_non_overlapping_unchanged() { ... }
#[test] fn test_merge_overlapping_same_line() { ... }
#[test] fn test_merge_touching_adjacent() { ... }
#[test] fn test_merge_multiline_overlap() { ... }
#[test] fn test_merge_duplicates() { ... }

// === SelectWord tests ===
#[test] fn test_select_word_single_cursor() { ... }
#[test] fn test_select_word_on_whitespace() { ... }
#[test] fn test_select_word_multi_cursor_different_words() { ... }
#[test] fn test_select_word_multi_cursor_same_word_merges() { ... }

// === SelectLine tests ===
#[test] fn test_select_line_single_cursor() { ... }
#[test] fn test_select_line_multi_cursor_different_lines() { ... }
#[test] fn test_select_line_multi_cursor_same_line_merges() { ... }

// === SelectAll tests ===
#[test] fn test_select_all_single_cursor() { ... }
#[test] fn test_select_all_collapses_multi_cursor() { ... }

// === ExtendSelectionToPosition tests ===
#[test] fn test_extend_selection_single_cursor() { ... }
#[test] fn test_extend_selection_collapses_multi_cursor() { ... }
```

---

## Files to Modify

| File | Changes |
|------|---------|
| `src/model/editor.rs` | Add `merge_overlapping_selections()`, `word_under_cursor_at()` |
| `src/update.rs` | Update SelectWord, SelectLine, SelectAll, ExtendSelectionToPosition handlers |
| `tests/selection.rs` | Add ~15-20 new tests |

---

## Success Criteria

- [ ] `merge_overlapping_selections()` handles all overlap cases
- [ ] `SelectWord` works on all cursors, merges overlaps
- [ ] `SelectLine` works on all cursors, merges overlaps  
- [ ] `SelectAll` properly collapses to single cursor + full doc
- [ ] `ExtendSelectionToPosition` collapses multi-cursor first
- [ ] All invariants maintained: `cursors.len() == selections.len()`, `cursor[i].to_position() == selection[i].head`
- [ ] Existing tests pass (no regression)
- [ ] New tests cover edge cases

---

## References

- Original design: `docs/archived/SELECTION_MULTICURSOR.md`
- Multi-cursor movement: `docs/archived/MULTI_CURSOR_MOVEMENT.md`
- Expand/shrink: `docs/archived/TEXT-SHRINK-EXPAND-SELECTION.md`
