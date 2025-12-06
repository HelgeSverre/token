# Multi-Cursor Movement Implementation

**Status:** ✅ Complete  
**Completed:** 2025-12-06  
**Effort:** M (1–3 hours)

---

## Problem Statement

Multi-cursor **editing** works correctly (InsertChar, DeleteBackward, DeleteForward operate on ALL cursors), but multi-cursor **movement** only affects the primary cursor (index 0).

### Current Behavior

- `MoveCursor(Direction)` → only primary cursor moves
- `MoveCursorLineStart/End` → only primary cursor moves
- `MoveCursorWord(Direction)` → only primary cursor moves
- `PageUp/PageDown` → only primary cursor moves
- All `*WithSelection` variants → only primary cursor's selection extends

### Root Cause

Movement handlers use `cursor_mut()` which always returns `&mut cursors[0]`:

```rust
// model/editor.rs
pub fn cursor_mut(&mut self) -> &mut Cursor {
    &mut self.cursors[0]  // Always primary!
}
```

---

## Implementation Plan

### Phase 0: Per-Cursor Movement Primitives

Add index-based movement helpers to `EditorState` in `model/editor.rs`:

```rust
impl EditorState {
    // Basic movement
    fn move_cursor_left_at(&mut self, doc: &Document, idx: usize);
    fn move_cursor_right_at(&mut self, doc: &Document, idx: usize);
    fn move_cursor_up_at(&mut self, doc: &Document, idx: usize);
    fn move_cursor_down_at(&mut self, doc: &Document, idx: usize);

    // Word movement
    fn move_cursor_word_left_at(&mut self, doc: &Document, idx: usize);
    fn move_cursor_word_right_at(&mut self, doc: &Document, idx: usize);

    // Line navigation
    fn move_cursor_line_start_at(&mut self, doc: &Document, idx: usize);
    fn move_cursor_line_end_at(&mut self, doc: &Document, idx: usize);

    // Document navigation
    fn move_cursor_document_start_at(&mut self, idx: usize);
    fn move_cursor_document_end_at(&mut self, doc: &Document, idx: usize);

    // Paging
    fn page_up_at(&mut self, doc: &Document, jump: usize, idx: usize);
    fn page_down_at(&mut self, doc: &Document, jump: usize, idx: usize);
}
```

**Key changes:**
- Replace `cursor_mut()` with `self.cursors[idx]`
- Use per-line whitespace helpers (see below)
- Preserve `desired_column` for vertical movement

### Phase 1: All-Cursors Movement Helpers

Add multi-cursor wrappers that iterate over all cursors:

```rust
impl EditorState {
    pub fn move_all_cursors_left(&mut self, doc: &Document) {
        for i in 0..self.cursors.len() {
            self.move_cursor_left_at(doc, i);
        }
        self.deduplicate_cursors();
    }

    pub fn move_all_cursors_right(&mut self, doc: &Document);
    pub fn move_all_cursors_up(&mut self, doc: &Document);
    pub fn move_all_cursors_down(&mut self, doc: &Document);
    pub fn move_all_cursors_word_left(&mut self, doc: &Document);
    pub fn move_all_cursors_word_right(&mut self, doc: &Document);
    pub fn move_all_cursors_line_start(&mut self, doc: &Document);
    pub fn move_all_cursors_line_end(&mut self, doc: &Document);
    pub fn move_all_cursors_document_start(&mut self, doc: &Document);
    pub fn move_all_cursors_document_end(&mut self, doc: &Document);
    pub fn page_up_all_cursors(&mut self, doc: &Document, jump: usize);
    pub fn page_down_all_cursors(&mut self, doc: &Document, jump: usize);
}
```

**Important:** Every multi-cursor helper ends with `deduplicate_cursors()` to handle collisions.

### Phase 2: Update Movement Handlers in update.rs

Replace single-cursor logic with all-cursor helpers:

```rust
EditorMsg::MoveCursor(direction) => {
    {
        let doc = model.document();
        let editor = model.editor_mut();
        match direction {
            Direction::Up => editor.move_all_cursors_up(doc),
            Direction::Down => editor.move_all_cursors_down(doc),
            Direction::Left => editor.move_all_cursors_left(doc),
            Direction::Right => editor.move_all_cursors_right(doc),
        }
        editor.collapse_selections_to_cursors();
    }
    model.ensure_cursor_visible_directional(...);
    model.reset_cursor_blink();
    Some(Cmd::Redraw)
}
```

**Handlers to update:**
- `MoveCursor(Direction)` - arrow keys
- `MoveCursorLineStart` - Home
- `MoveCursorLineEnd` - End
- `MoveCursorDocumentStart` - Cmd+Home
- `MoveCursorDocumentEnd` - Cmd+End
- `MoveCursorWord(Direction)` - Option+Arrow
- `PageUp` / `PageDown`

### Phase 3: Selection Movement Helpers

Add selection-extending variants:

```rust
impl EditorState {
    pub fn move_all_cursors_left_with_selection(&mut self, doc: &Document) {
        for i in 0..self.cursors.len() {
            self.move_cursor_left_at(doc, i);
            let pos = self.cursors[i].to_position();
            self.selections[i].head = pos;
        }
        self.deduplicate_cursors();
    }

    // Same pattern for: right, up, down, word_left, word_right,
    // line_start, line_end, document_start, document_end, page_up, page_down
}
```

**Note:** DO NOT call `collapse_selections_to_cursors()` for selection moves.

### Phase 4: Update Selection Movement Handlers

```rust
EditorMsg::MoveCursorWithSelection(direction) => {
    {
        let doc = model.document();
        let editor = model.editor_mut();
        match direction {
            Direction::Up => editor.move_all_cursors_up_with_selection(doc),
            Direction::Down => editor.move_all_cursors_down_with_selection(doc),
            Direction::Left => editor.move_all_cursors_left_with_selection(doc),
            Direction::Right => editor.move_all_cursors_right_with_selection(doc),
        }
        // NO collapse_selections_to_cursors() here!
    }
    model.ensure_cursor_visible_directional(...);
    model.reset_cursor_blink();
    Some(Cmd::Redraw)
}
```

**Handlers to update:**
- `MoveCursorWithSelection(Direction)` - Shift+Arrow
- `MoveCursorLineStartWithSelection` - Shift+Home
- `MoveCursorLineEndWithSelection` - Shift+End
- `MoveCursorDocumentStartWithSelection` - Shift+Cmd+Home
- `MoveCursorDocumentEndWithSelection` - Shift+Cmd+End
- `MoveCursorWordWithSelection(Direction)` - Shift+Option+Arrow
- `PageUpWithSelection` - Shift+PageUp
- `PageDownWithSelection` - Shift+PageDown

### Phase 5: Per-Line Helper Functions

Add helpers for line-specific operations (needed for multi-cursor Home/End):

```rust
// In Document or as standalone functions
fn first_non_whitespace_column_for_line(doc: &Document, line: usize) -> usize;
fn last_non_whitespace_column_for_line(doc: &Document, line: usize) -> usize;
fn line_length_for_line(doc: &Document, line: usize) -> usize;
```

These replace `model.first_non_whitespace_column()` which only works for primary cursor's line.

---

## Testing Strategy

### Single-Cursor Regression Tests

Ensure all existing single-cursor tests pass:
- `tests/cursor_movement.rs` - 38 tests
- Movement with/without selection
- Word movement
- Page up/down

### New Multi-Cursor Movement Tests

```rust
#[test]
fn multi_cursor_arrow_left_moves_all() {
    let mut model = test_model_with_content("hello world");
    // Add cursors at positions 3 and 8
    model.editor_mut().cursors.push(Cursor::at(0, 3));
    model.editor_mut().cursors.push(Cursor::at(0, 8));
    
    update(&mut model, Msg::Editor(EditorMsg::MoveCursor(Direction::Left)));
    
    assert_eq!(model.editor().cursors[0].column, 2);
    assert_eq!(model.editor().cursors[1].column, 7);
}

#[test]
fn multi_cursor_movement_deduplicates_collisions() {
    let mut model = test_model_with_content("abc");
    model.editor_mut().cursors.push(Cursor::at(0, 1));
    model.editor_mut().cursors.push(Cursor::at(0, 2));
    
    // Move left twice - cursors should collide and dedupe
    update(&mut model, Msg::Editor(EditorMsg::MoveCursor(Direction::Left)));
    update(&mut model, Msg::Editor(EditorMsg::MoveCursor(Direction::Left)));
    
    // Only one cursor should remain at position 0
    assert_eq!(model.editor().cursors.len(), 1);
}

#[test]
fn multi_cursor_vertical_preserves_desired_column() {
    // Test that each cursor maintains its own desired_column
}

#[test]
fn multi_cursor_shift_arrow_extends_all_selections() {
    // Test selection extension for all cursors
}
```

---

## Files to Modify

| File | Changes |
|------|---------|
| `src/model/editor.rs` | Add per-cursor and all-cursor movement methods |
| `src/model/document.rs` | Add per-line helper functions |
| `src/update.rs` | Update all movement handlers to use new methods |
| `tests/cursor_movement.rs` | Add multi-cursor movement tests |

---

## Success Criteria

- [ ] Arrow keys move ALL cursors simultaneously
- [ ] Home/End moves ALL cursors to their respective line starts/ends
- [ ] Word movement (Option+Arrow) moves ALL cursors
- [ ] PageUp/PageDown moves ALL cursors
- [ ] Shift+Arrow extends selection for ALL cursors
- [ ] Cursors that collide are deduplicated
- [ ] Each cursor preserves its own `desired_column` for vertical movement
- [ ] Single-cursor behavior unchanged (regression tests pass)
- [ ] Viewport follows primary cursor (existing behavior)

---

## Deferred Items

- Viewport that follows "extreme" cursor (topmost/bottommost) instead of primary
- Selection merging when overlapping after movement
- Configurable movement semantics (Vim motions, etc.)
