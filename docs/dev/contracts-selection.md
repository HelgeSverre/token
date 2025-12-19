# Selection Behavior Contract

This document defines the invariants and expected behavior for selections and multi-cursor operations in Token Editor. Implementations MUST preserve these guarantees.

---

## Core Types

### Position

A location in the document:

```rust
pub struct Position {
    pub line: usize,   // 0-indexed
    pub column: usize, // 0-indexed, in characters
}
```

**Invariants:**
- `line` must be `< document.line_count()`
- `column` must be `<= line.char_count()` (can be at end of line)
- Column is in Unicode code points, not bytes or graphemes

### Selection

A range from anchor to head:

```rust
pub struct Selection {
    pub anchor: Position, // Fixed point (where selection started)
    pub head: Position,   // Moving point (current cursor position)
}
```

**Invariants:**
- Both `anchor` and `head` must be valid positions
- Empty selection: `anchor == head` (cursor without selection)
- Forward selection: `anchor < head`
- Backward selection: `head < anchor`

### Cursor

Extends Selection with vertical navigation state:

```rust
pub struct Cursor {
    pub selection: Selection,
    pub desired_column: usize, // Preserved during vertical movement
}
```

---

## Selection Invariants

### INV-SEL-01: Valid Positions

> All positions in selections must reference valid document locations.

When document content changes, all cursor/selection positions must be clamped to valid ranges.

### INV-SEL-02: No Gaps

> Selections are contiguous. A selection covers all characters from `start()` to `end()`.

There is no way to select non-contiguous text within a single selection. Multi-cursor provides disjoint selections.

### INV-SEL-03: Anchor/Head Independence

> The anchor and head can be in any relative order.

Backward selections (head before anchor) are valid and must be handled correctly by all operations.

---

## Multi-Cursor Invariants

### INV-MC-01: Primary Cursor

> Index 0 is always the "primary" cursor.

The primary cursor:
- Determines scroll position
- Is used for single-cursor fallback operations
- Is highlighted differently (solid vs semi-transparent)

### INV-MC-02: No Overlapping Selections

> Cursors must not have overlapping selections.

After any operation that could cause overlap, `merge_overlapping_selections()` must be called.

### INV-MC-03: Sorted Order

> Cursors should be sorted by position (primary first, then by start position).

This simplifies rendering and edit operations.

### INV-MC-04: Deduplication

> No two cursors may occupy the same position.

After movement operations, cursors that collide must be deduplicated.

---

## Movement Operations

### Single-Cursor Movement

Movement commands (`MoveCursorUp`, `MoveCursorDown`, etc.) apply to **all cursors** simultaneously.

```
Before: Cursor at (0, 5), (2, 5), (4, 5)
Action: MoveCursorDown
After:  Cursor at (1, 5), (3, 5), (5, 5)
```

### Desired Column Preservation

When moving vertically through lines of varying length:

```
Line 0: "short"          (5 chars)
Line 1: "much longer line" (16 chars)
Line 2: "tiny"           (4 chars)

Cursor at (0, 5), desired_column = 5
MoveCursorDown → (1, 5), desired_column = 5
MoveCursorDown → (2, 4), desired_column = 5  // Clamped but preserved
MoveCursorUp   → (1, 5), desired_column = 5  // Restored
```

**Rule:** Horizontal movement resets `desired_column` to current column. Vertical movement preserves it.

### Selection Extension

Commands with `*WithSelection` suffix:
1. Keep `anchor` fixed
2. Move `head` according to the base command
3. Update `desired_column` accordingly

---

## Selection Operations

### SelectAll

Collapses to single cursor covering entire document:

```
Before: Multiple cursors with various selections
After:  Single cursor at (0, 0) to (last_line, last_col)
```

### SelectWord

For each cursor:
1. If cursor has selection, select word at head
2. If cursor is in word, select the word
3. If cursor is in whitespace, select the whitespace
4. Merge overlapping selections

### SelectLine

For each cursor:
1. Extend selection to cover full line(s)
2. Include newline at end (except for last line)
3. Merge overlapping selections

### ExpandSelection (Option+Up)

Progressive expansion:
1. No selection → select word
2. Word selected → select line
3. Line selected → select all

History stack preserves previous selection for `ShrinkSelection`.

### ShrinkSelection (Option+Down)

Restore previous selection from history stack.

---

## Multi-Cursor Addition

### AddCursorAbove / AddCursorBelow

Adds cursor at same column on adjacent line, extending from **edge cursors** (not primary):

```
Before: Cursors at lines 5, 8
AddCursorAbove → Add at line 4 (above line 5)
AddCursorBelow → Add at line 9 (below line 8)
```

### Cmd+Click

Toggle cursor at clicked position:
- If no cursor exists: add cursor
- If cursor exists at position: remove it
- If removing would leave 0 cursors: keep it

### SelectNextOccurrence (Cmd+J)

1. Get text of primary selection (or word at cursor)
2. Find next occurrence in document
3. Add cursor selecting that occurrence
4. Merge if overlap

---

## Edit Operations with Multi-Cursor

### Text Insertion

For each cursor (processed in reverse document order):
1. Delete selection if non-empty
2. Insert text at cursor position
3. Move cursor to end of inserted text
4. Adjust all subsequent cursor positions

**Reverse order processing** prevents position invalidation.

### Text Deletion

For each cursor (reverse order):
1. If selection: delete selected text
2. Else: delete character(s) according to command
3. Move cursor to deletion point
4. Adjust subsequent positions

### Batch Undo

Multi-cursor edits are batched into single undo operation:

```rust
EditOperation::Batch {
    operations: Vec<EditOperation>,
    cursors_before: Vec<Cursor>,
    cursors_after: Vec<Cursor>,
}
```

---

## Selection Merge Rules

### merge_overlapping_selections()

Called after operations that may cause overlap:

1. Sort cursors by start position
2. Merge if `cursor[i].end >= cursor[i+1].start`
3. Merged selection uses combined range
4. Primary cursor (index 0) status is preserved

### Merge Examples

```
Before: [(0,0)-(0,5)], [(0,3)-(0,8)]
After:  [(0,0)-(0,8)]

Before: [(0,0)-(0,5)], [(0,5)-(0,10)]  // Touching
After:  [(0,0)-(0,10)]

Before: [(0,0)-(0,5)], [(1,0)-(1,5)]   // Disjoint
After:  [(0,0)-(0,5)], [(1,0)-(1,5)]   // No merge
```

---

## Edge Cases

### Empty Document

- Single cursor at (0, 0)
- All movement commands are no-ops
- Selection operations produce empty or single-char selections

### End of Line

- Cursor can be at `column == line.len()` (after last character)
- Movement right at EOL moves to start of next line
- Selection extension includes the newline character

### Last Line (No Trailing Newline)

- No newline character to select
- MoveCursorDown at last line is no-op
- SelectLine does not include phantom newline

### Tab Characters

- Tab occupies 1 column in document coordinates
- Visual width varies (typically 4 spaces)
- Selection column counts tabs as 1 character

---

## Test Requirements

Every selection operation must have tests for:

1. **Empty document**
2. **Single line document**
3. **Multi-line document**
4. **With and without existing selection**
5. **Forward and backward selections**
6. **Multi-cursor scenarios**
7. **Edge positions** (start of line, end of line, start of document, end of document)

---

## References

- `src/model/editor.rs` - Position, Selection, Cursor definitions
- `src/update/editor.rs` - Movement and selection operations
- `tests/selection.rs` - Selection test suite
- `tests/multi_cursor.rs` - Multi-cursor test suite
