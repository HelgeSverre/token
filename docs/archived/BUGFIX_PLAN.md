# Bugfix & Feature Implementation Plan

**Created:** 2025-12-06
**Status:** Ready for implementation
**Estimated Total Effort:** L (Large) - ~15-20 focused sessions

---

## Overview

This plan addresses all critical bugs and missing features identified in the deep codebase analysis. Work is organized into 5 priority levels with clear dependencies.

```
Priority 0 (Critical Bugs) ──┬──> Priority 1 (Multi-Pane) ──> Priority 2 (Undo/Redo)
                             │
                             └──> Priority 3 (Selection Phase 9)
                                           │
                                           └──> Priority 4 (Rectangle Selection)
```

---

## Priority 0: Critical Bugs (Blocking)

**Effort:** M (Medium) - Must be done first
**Files:** `document.rs`, `editor.rs`, `update.rs`

### 0.1 Unicode Byte/Char Mismatch in Search

**File:** `model/document.rs:169-183`

**Problem:** `find_all_occurrences` uses `String::find()` which returns byte offsets, but treats them as char indices.

**Fix:**

```rust
pub fn find_all_occurrences(&self, needle: &str) -> Vec<(usize, usize)> {
    if needle.is_empty() {
        return Vec::new();
    }

    let haystack = self.buffer.to_string();
    let needle_char_len = needle.chars().count();

    // Build byte→char mapping
    let mut byte_to_char: Vec<(usize, usize)> = Vec::new();
    for (byte_idx, (char_idx, _)) in haystack.char_indices().enumerate()
        .map(|(i, (b, c))| (b, i)) // This is wrong, fix below
    {
        byte_to_char.push((byte_idx, char_idx));
    }

    // Better approach:
    let byte_to_char: std::collections::HashMap<usize, usize> =
        haystack.char_indices()
            .enumerate()
            .map(|(char_idx, (byte_idx, _))| (byte_idx, char_idx))
            .collect();
    let total_chars = haystack.chars().count();

    let mut results = Vec::new();
    let mut start_byte = 0;

    while let Some(rel) = haystack[start_byte..].find(needle) {
        let match_start_byte = start_byte + rel;
        let start_char = *byte_to_char.get(&match_start_byte).unwrap();
        let end_char = start_char + needle_char_len;
        results.push((start_char, end_char));
        start_byte = match_start_byte + 1; // Allow overlapping
    }

    results
}
```

**Tests to Add:**

- ASCII: `"abc abc abc"` with needle `"abc"` → `[(0,3), (4,7), (8,11)]`
- Unicode: `"äbc äbc"` with needle `"äbc"` → `[(0,3), (4,7)]`
- Mixed: Multi-byte chars in needle and haystack

---

### 0.2 Unicode Bug in `word_under_cursor`

**File:** `model/editor.rs:431-470`

**Problem:** Compares `cursor.column` (char index) to `String::len()` (bytes).

**Fix:**

```rust
pub fn word_under_cursor(&self, document: &Document) -> Option<(String, Position, Position)> {
    let cursor = self.cursor();
    let line = document.get_line(cursor.line)?;
    let line = line.trim_end_matches('\n');

    if line.is_empty() {
        return None;
    }

    let chars: Vec<char> = line.chars().collect();
    if chars.is_empty() {
        return None;
    }

    // FIX: Clamp to chars.len(), not line.len()
    let col = cursor.column.min(chars.len().saturating_sub(1));

    if char_type(chars[col]) != CharType::WordChar {
        return None;
    }

    // Rest of implementation unchanged...
}
```

**Tests to Add:**

- Cursor on ASCII word
- Cursor on word containing `ä`, `é`, emoji
- Cursor at end-of-line with multi-byte last char

---

### 0.3 SelectNextOccurrence Wrong Offset

**File:** `update.rs:359-363`

**Problem:** Always searches from primary cursor instead of `last_search_offset`.

**Fix:**

```rust
EditorMsg::SelectNextOccurrence => {
    // ... get search_text ...

    let doc = model.document();
    let editor = model.editor();

    // FIX: Use last_search_offset when available
    let start_offset = if let Some(state) = &editor.occurrence_state {
        state.last_search_offset
    } else {
        let sel = editor.selection();
        let end = sel.end();
        doc.cursor_to_offset(end.line, end.column)
    };

    if let Some((start_off, end_off)) = doc.find_next_occurrence(&search_text, start_offset) {
        // ... add cursor ...

        // Update last_search_offset
        if let Some(state) = &mut model.editor_mut().occurrence_state {
            state.last_search_offset = end_off;
        }
    }
}
```

**Tests to Add:**

- Call `SelectNextOccurrence` repeatedly on `"abc abc abc"`
- Assert each call adds new cursor at next occurrence
- Assert "no more" after last occurrence

---

### 0.4 Cursor/Selection Invariant Violations

**File:** `update.rs` (multiple handlers), `model/editor.rs`

**Problem:** Non-shift movements update cursor but not selection.head.

**Step 1 - Add helper to `EditorState`:**

```rust
impl EditorState {
    /// Collapse all selections so anchor == head == cursor position
    pub fn collapse_selections_to_cursors(&mut self) {
        for (cursor, selection) in self.cursors.iter().zip(self.selections.iter_mut()) {
            let pos = cursor.to_position();
            selection.anchor = pos;
            selection.head = pos;
        }
    }
}
```

**Step 2 - Call after every non-shift move in `update.rs`:**

- `MoveCursor(Direction::*)` (without selection)
- `MoveCursorLineStart` / `MoveCursorLineEnd`
- `MoveCursorDocumentStart` / `MoveCursorDocumentEnd`
- `MoveCursorWord`
- `PageUp` / `PageDown`
- `SetCursorPosition`

```rust
// After cursor mutation:
model.editor_mut().collapse_selections_to_cursors();
```

**Tests to Add:**

- With selection, press arrow key → selection cleared
- With multi-cursor selections, move → all collapsed
- Debug build: call `assert_invariants()` after movement battery

---

## Priority 1: Multi-Pane Correctness

**Effort:** M (Medium)
**Files:** `model/mod.rs`, `model/editor_area.rs`
**Depends on:** Priority 0 complete

### 1.1 Viewport Resize All Editors

**File:** `model/mod.rs:151-161`

**Problem:** Only focused editor viewport is updated on resize.

**Fix:**

```rust
pub fn resize(&mut self, width: u32, height: u32) {
    self.window_size = (width, height);

    let text_x = text_start_x(self.char_width).round();
    let visible_columns = ((width as f32 - text_x) / self.char_width).floor() as usize;
    let visible_lines = (height as usize) / self.line_height;

    // FIX: Update ALL editors, not just focused
    for editor in self.editor_area.editors.values_mut() {
        editor.resize_viewport(visible_lines, visible_columns);
    }
}

pub fn set_char_width(&mut self, char_width: f32) {
    self.char_width = char_width;
    let (width, _) = self.window_size;
    let text_x = text_start_x(char_width).round();
    let visible_columns = ((width as f32 - text_x) / char_width).floor() as usize;

    // FIX: Update ALL editors
    for editor in self.editor_area.editors.values_mut() {
        editor.viewport.visible_columns = visible_columns;
    }
}
```

---

### 1.2 Sync Viewports After Layout Changes

**File:** `model/editor_area.rs`

**Add helper:**

```rust
impl EditorArea {
    pub fn sync_all_viewports(&mut self, line_height: usize, char_width: f32) {
        for (group_id, group) in &self.groups {
            let rect = group.rect;
            let visible_lines = (rect.height as usize) / line_height;

            // Calculate visible columns from group width
            let text_x = crate::util::text_start_x(char_width);
            let available_width = (rect.width - text_x).max(0.0);
            let visible_columns = (available_width / char_width).floor() as usize;

            // Update all editors in this group
            for tab in &group.tabs {
                if let Some(editor) = self.editors.get_mut(&tab.editor_id) {
                    editor.resize_viewport(visible_lines, visible_columns);
                }
            }
        }
    }
}
```

**Call after:**

- `compute_layout()`
- Split operations
- Group close operations

---

### 1.3 Avoid Document Cloning

**File:** `model/mod.rs:214-241`

**Current (clones):**

```rust
pub fn ensure_cursor_visible(&mut self) {
    let doc = self.editor_area.focused_document().unwrap().clone(); // BAD
    self.editor_mut().ensure_cursor_visible(&doc);
}
```

**Fix - Move to EditorArea:**

```rust
impl EditorArea {
    pub fn ensure_focused_cursor_visible(&mut self, mode: ScrollRevealMode) {
        let doc_id = match self.focused_document_id() {
            Some(id) => id,
            None => return,
        };
        let editor_id = match self.focused_editor_id() {
            Some(id) => id,
            None => return,
        };

        // Get document reference first, then editor
        let doc = self.documents.get(&doc_id).unwrap();
        let doc_ptr = doc as *const Document;

        // Safe: we only read from doc while mutating editor
        let doc_ref = unsafe { &*doc_ptr };
        if let Some(editor) = self.editors.get_mut(&editor_id) {
            editor.ensure_cursor_visible_with_mode(doc_ref, mode);
        }
    }
}

// In AppModel:
pub fn ensure_cursor_visible(&mut self) {
    self.editor_area.ensure_focused_cursor_visible(ScrollRevealMode::Minimal);
}
```

---

## Priority 2: Multi-Cursor Undo/Redo

**Effort:** L (Large)
**Files:** `model/document.rs`, `update.rs`
**Depends on:** Priority 0, 1 complete

### 2.1 Add EditOperation::Batch

**File:** `model/document.rs`

```rust
#[derive(Debug, Clone)]
pub enum EditOperation {
    Insert {
        position: usize,
        text: String,
        cursor_before: Cursor,
        cursor_after: Cursor,
    },
    Delete {
        position: usize,
        text: String,
        cursor_before: Cursor,
        cursor_after: Cursor,
    },
    Replace {
        position: usize,
        deleted_text: String,
        inserted_text: String,
        cursor_before: Cursor,
        cursor_after: Cursor,
    },
    // NEW
    Batch {
        operations: Vec<EditOperation>,
        cursors_before: Vec<Cursor>,
        cursors_after: Vec<Cursor>,
    },
}
```

### 2.2 Update Undo/Redo Handling

```rust
impl Document {
    pub fn undo(&mut self, editor: &mut EditorState) -> bool {
        let op = match self.undo_stack.pop() {
            Some(op) => op,
            None => return false,
        };

        match &op {
            EditOperation::Batch { operations, cursors_before, .. } => {
                // Undo in reverse order
                for inner_op in operations.iter().rev() {
                    self.apply_undo_single(inner_op);
                }
                // Restore cursor state
                editor.cursors = cursors_before.clone();
                editor.collapse_selections_to_cursors();
            }
            _ => {
                self.apply_undo_single(&op);
                // Restore single cursor
                if let Some(cursor) = op.cursor_before() {
                    editor.cursors[0] = cursor;
                    editor.collapse_selections_to_cursors();
                }
            }
        }

        self.redo_stack.push(op);
        self.is_modified = true;
        true
    }

    fn apply_undo_single(&mut self, op: &EditOperation) {
        match op {
            EditOperation::Insert { position, text, .. } => {
                let end = position + text.chars().count();
                self.buffer.remove(*position..end);
            }
            EditOperation::Delete { position, text, .. } => {
                self.buffer.insert(*position, text);
            }
            EditOperation::Replace { position, deleted_text, inserted_text, .. } => {
                let end = position + inserted_text.chars().count();
                self.buffer.remove(*position..end);
                self.buffer.insert(*position, deleted_text);
            }
            EditOperation::Batch { .. } => {
                // Nested batches not supported
            }
        }
    }
}
```

### 2.3 Use Batch in Multi-Cursor Edits

**File:** `update.rs` - InsertChar, DeleteBackward, etc.

```rust
// Example for InsertChar with multiple cursors
DocumentMsg::InsertChar(ch) => {
    let cursors_before = model.editor().cursors.clone();
    let multi = cursors_before.len() > 1;
    let mut operations = Vec::new();

    // Process in reverse order (existing logic)
    for i in (0..model.editor().cursors.len()).rev() {
        let cursor = model.editor().cursors[i].clone();
        let offset = model.document().cursor_to_offset(cursor.line, cursor.column);

        // Insert char
        model.document_mut().buffer.insert_char(offset, ch);

        // Record operation
        operations.push(EditOperation::Insert {
            position: offset,
            text: ch.to_string(),
            cursor_before: cursor.clone(),
            cursor_after: /* new cursor position */,
        });

        // Update cursor
        // ...
    }

    let cursors_after = model.editor().cursors.clone();

    // Push as batch or single
    if multi {
        model.document_mut().push_edit(EditOperation::Batch {
            operations,
            cursors_before,
            cursors_after,
        });
    } else {
        model.document_mut().push_edit(operations.into_iter().next().unwrap());
    }
}
```

---

## Priority 3: Selection Phase 9

**Effort:** M (Medium)
**Files:** `model/editor.rs`, `update.rs`, `messages.rs`
**Depends on:** Priority 0 complete

### 3.1 Selection::get_text()

**File:** `model/editor.rs`

```rust
impl Selection {
    pub fn get_text(&self, document: &Document) -> String {
        if self.is_empty() {
            return String::new();
        }

        let start = self.start();
        let end = self.end();

        let start_offset = document.cursor_to_offset(start.line, start.column);
        let end_offset = document.cursor_to_offset(end.line, end.column);

        document.buffer.slice(start_offset..end_offset).to_string()
    }
}
```

### 3.2 Complete SelectAllOccurrences

**File:** `update.rs`

```rust
EditorMsg::SelectAllOccurrences => {
    // Get search text (from selection or word under cursor)
    let search_text = {
        let editor = model.editor();
        let sel = editor.selection();
        if !sel.is_empty() {
            sel.get_text(model.document())
        } else {
            match editor.word_under_cursor(model.document()) {
                Some((word, _, _)) => word,
                None => return Some(Cmd::Redraw),
            }
        }
    };

    if search_text.is_empty() {
        return Some(Cmd::Redraw);
    }

    // Find all occurrences
    let occurrences = model.document().find_all_occurrences(&search_text);

    if occurrences.is_empty() {
        return Some(Cmd::Redraw);
    }

    // Build cursors and selections
    let mut new_cursors = Vec::new();
    let mut new_selections = Vec::new();

    for (start_off, end_off) in &occurrences {
        let (start_line, start_col) = model.document().offset_to_cursor(*start_off);
        let (end_line, end_col) = model.document().offset_to_cursor(*end_off);

        let start_pos = Position::new(start_line, start_col);
        let end_pos = Position::new(end_line, end_col);

        new_cursors.push(Cursor::at(end_line, end_col));
        new_selections.push(Selection::from_anchor_head(start_pos, end_pos));
    }

    // Replace editor state
    let editor = model.editor_mut();
    editor.cursors = new_cursors;
    editor.selections = new_selections;
    editor.deduplicate_cursors();

    // Set up occurrence state
    editor.occurrence_state = Some(OccurrenceState {
        search_text,
        added_cursor_indices: (0..editor.cursors.len()).collect(),
        last_search_offset: occurrences.last().map(|(_, e)| *e).unwrap_or(0),
    });

    model.ensure_cursor_visible();
    Some(Cmd::Redraw)
}
```

### 3.3 Add UnselectOccurrence Message

**File:** `messages.rs` - Already exists, verify handler in `update.rs`

The handler should:

1. Pop from `added_cursor_indices`
2. Remove corresponding cursor/selection (if more than 1 cursor)
3. Clear `occurrence_state` when empty

---

## Priority 4: Rectangle Selection

**Effort:** M (Medium)
**Files:** `update.rs`
**Depends on:** Priority 0 complete

### 4.1 Complete FinishRectangleSelection

**File:** `update.rs`

```rust
EditorMsg::FinishRectangleSelection => {
    if !model.editor().rectangle_selection.active {
        return Some(Cmd::Redraw);
    }

    let rect_state = &model.editor().rectangle_selection;
    let top_left = rect_state.top_left();
    let bottom_right = rect_state.bottom_right();
    let start_col = rect_state.start.column;
    let end_col = rect_state.current.column;

    // Determine if we have a selection or just cursors
    let has_selection = start_col != end_col;
    let (left_col, right_col) = if start_col <= end_col {
        (start_col, end_col)
    } else {
        (end_col, start_col)
    };

    let mut new_cursors = Vec::new();
    let mut new_selections = Vec::new();

    for line in top_left.line..=bottom_right.line {
        let line_len = model.document().line_length(line);

        // Clamp columns to line length
        let clamped_left = left_col.min(line_len);
        let clamped_right = right_col.min(line_len);
        let cursor_col = end_col.min(line_len);

        let cursor = Cursor::at(line, cursor_col);

        let selection = if has_selection && clamped_left < clamped_right {
            let anchor = Position::new(line, clamped_left);
            let head = Position::new(line, clamped_right);

            // Respect drag direction
            if start_col <= end_col {
                Selection::from_anchor_head(anchor, head)
            } else {
                Selection::from_anchor_head(head, anchor)
            }
        } else {
            Selection::new(Position::new(line, cursor_col))
        };

        new_cursors.push(cursor);
        new_selections.push(selection);
    }

    // Apply to editor
    let editor = model.editor_mut();
    editor.cursors = new_cursors;
    editor.selections = new_selections;
    editor.deduplicate_cursors();

    // Clear rectangle state
    editor.rectangle_selection.active = false;
    editor.rectangle_selection.preview_cursors.clear();

    model.ensure_cursor_visible();
    Some(Cmd::Redraw)
}
```

---

## Implementation Order & Dependencies

```
Week 1: Priority 0 (Critical Bugs)
├── 0.1 Unicode in find_all_occurrences
├── 0.2 Unicode in word_under_cursor
├── 0.3 SelectNextOccurrence offset
└── 0.4 Cursor/selection invariants

Week 2: Priority 1 (Multi-Pane)
├── 1.1 Resize all viewports
├── 1.2 Sync after layout changes
└── 1.3 Avoid document cloning

Week 3: Priority 2 (Undo/Redo)
├── 2.1 EditOperation::Batch variant
├── 2.2 Undo/redo batch handling
└── 2.3 Multi-cursor edits use batch

Week 4: Priority 3 & 4 (Selection)
├── 3.1 Selection::get_text()
├── 3.2 SelectAllOccurrences
├── 3.3 UnselectOccurrence
└── 4.1 FinishRectangleSelection
```

---

## Test Coverage Requirements

Each fix must include tests:

| Priority | Test Type          | Count   |
| -------- | ------------------ | ------- |
| 0.1      | Unit (document.rs) | 4+      |
| 0.2      | Unit (editor.rs)   | 4+      |
| 0.3      | Integration        | 3+      |
| 0.4      | Integration        | 3+      |
| 1.x      | Integration        | 2+ each |
| 2.x      | Integration        | 5+      |
| 3.x      | Integration        | 3+ each |
| 4.1      | Integration        | 4+      |

**Total new tests:** ~30-40

---

## Risk Mitigation

1. **Unicode fixes may break existing behavior** → Run full test suite after each change
2. **Batch undo complexity** → Start with minimal implementation, no nested batches
3. **Unsafe code for borrowing** → Encapsulate in small helper, extensive testing
4. **Multi-pane regressions** → Test with 2+ split views after each P1 change

---

## Success Criteria

- [ ] All 293 existing tests still pass
- [ ] 30+ new tests added and passing
- [ ] `make build` succeeds
- [ ] Manual testing of:
  - [ ] Cmd+J on Unicode text
  - [ ] Multi-cursor typing + undo
  - [ ] Split view resize
  - [ ] Rectangle selection commit
