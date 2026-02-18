# Undo/Redo Behavior Contract

This document defines the invariants and expected behavior for undo/redo operations in Token Editor. Implementations MUST preserve these guarantees.

---

## Core Types

### EditOperation

```rust
pub enum EditOperation {
    /// Text insertion
    Insert {
        position: usize,      // Byte offset in buffer
        text: String,         // Inserted text
        cursor_before: Cursor,
        cursor_after: Cursor,
    },

    /// Text deletion
    Delete {
        position: usize,      // Byte offset where deletion starts
        text: String,         // Deleted text (for redo)
        cursor_before: Cursor,
        cursor_after: Cursor,
    },

    /// Atomic replace (selection → new text)
    Replace {
        position: usize,
        deleted_text: String,
        inserted_text: String,
        cursor_before: Cursor,
        cursor_after: Cursor,
    },

    /// Batch for multi-cursor operations
    Batch {
        operations: Vec<EditOperation>,
        cursors_before: Vec<Cursor>,
        cursors_after: Vec<Cursor>,
    },
}
```

### Document Stacks

```rust
pub struct Document {
    pub undo_stack: Vec<EditOperation>,
    pub redo_stack: Vec<EditOperation>,
    // ...
}
```

---

## Undo/Redo Invariants

### INV-UNDO-01: Stack Semantics

> Undo and redo stacks follow LIFO (Last-In-First-Out) ordering.

The most recent operation is always undone first.

### INV-UNDO-02: Redo Clears on Edit

> Any new edit operation clears the redo stack.

```
Edit A → Edit B → Undo → Redo stack has [B]
Edit C → Redo stack is now empty
```

### INV-UNDO-03: Cursor Restoration

> Undo/redo restores cursor positions exactly as they were before/after the operation.

```
Before edit: cursor at (5, 10)
After edit: cursor at (5, 15)
After undo: cursor at (5, 10)
After redo: cursor at (5, 15)
```

### INV-UNDO-04: Content Restoration

> Undo/redo restores document content exactly.

The sequence `Edit → Undo` must produce identical document content to before the edit.

### INV-UNDO-05: Multi-Cursor Atomicity

> Multi-cursor edits are atomic.

A single Undo command reverses ALL cursor edits from the operation, not just one.

---

## Operation Semantics

### Insert

**Undo:** Delete the inserted text, restore cursor_before
**Redo:** Re-insert the text, restore cursor_after

```
Original: "hello"
Insert "X" at position 2
Result: "heXllo"

Undo: Delete at position 2, length 1
Result: "hello"
```

### Delete

**Undo:** Re-insert the deleted text, restore cursor_before
**Redo:** Delete the text again, restore cursor_after

```
Original: "hello"
Delete at position 2, length 2 ("ll")
Result: "heo"

Undo: Insert "ll" at position 2
Result: "hello"
```

### Replace

Used when typing over a selection (atomic delete + insert):

**Undo:** Delete inserted_text, insert deleted_text, restore cursor_before
**Redo:** Delete deleted_text, insert inserted_text, restore cursor_after

```
Original: "hello" with "ell" selected
Replace with "X"
Result: "hXo"

Undo: Delete "X", insert "ell"
Result: "hello"
```

### Batch

For multi-cursor operations:

**Undo:** Apply all operations in reverse order, restore cursors_before
**Redo:** Apply all operations in forward order, restore cursors_after

```
Batch {
    operations: [Insert at 0, Insert at 10, Insert at 20],
    cursors_before: [Cursor(0,0), Cursor(10,0), Cursor(20,0)],
    cursors_after: [Cursor(0,1), Cursor(11,0), Cursor(21,0)],
}

Undo: Reverse operations [Delete at 20, Delete at 10, Delete at 0]
      Restore 3 cursors to cursors_before positions
```

---

## Multi-Cursor Undo Behavior

### Processing Order

Multi-cursor edits are processed in **reverse document order** to prevent position invalidation:

```
Cursors at lines: 5, 10, 15
Edit order: line 15 first, then 10, then 5

This prevents:
- Edit at line 5 shifting positions of cursors at 10, 15
```

### Batch Creation

All edits from a single multi-cursor command are wrapped in a Batch:

```rust
fn insert_at_all_cursors(text: &str, cursors: &[Cursor]) -> EditOperation {
    EditOperation::Batch {
        operations: cursors.iter().rev().map(|c| {
            EditOperation::Insert { /* ... */ }
        }).collect(),
        cursors_before: cursors.to_vec(),
        cursors_after: /* adjusted cursor positions */,
    }
}
```

### Cursor Count Changes

If undo/redo changes cursor count (e.g., if cursors were merged):

- Restore exact cursor count from the operation
- Cursor positions from the operation take precedence

---

## Dirty State

### INV-DIRTY-01: Modification Tracking

> `document.is_modified` reflects whether content differs from disk.

```
Open file → is_modified = false
Edit → is_modified = true
Undo all → is_modified = false (if back to saved state)
Save → is_modified = false
```

### Implementation Note

Current implementation uses simple dirty flag. Future: may track "clean" stack position for accurate dirty detection after undo.

---

## Undo Coalescing (Future Enhancement)

### Current Behavior

Each keystroke creates a separate undo entry:

```
Type "hello" → 5 undo entries (h, e, l, l, o)
```

### Planned Behavior

Group rapid consecutive edits:

```
Type "hello" quickly → 1 undo entry ("hello")
```

**Coalescing rules (when implemented):**

1. **Same type:** Only coalesce same operation types (insert with insert)
2. **Adjacent position:** New edit must be adjacent to previous
3. **Time threshold:** Edits within 300ms are candidates for coalescing
4. **Break on movement:** Any cursor movement breaks coalescing
5. **Break on pause:** Typing pause > threshold starts new group

---

## Edge Cases

### Empty Undo Stack

- Undo command is a no-op
- No error or warning

### Empty Redo Stack

- Redo command is a no-op
- No error or warning

### Very Large Operations

- Large paste operations are single entries
- May cause memory pressure with huge undo stacks
- Consider: undo stack size limit (not currently implemented)

### External File Changes

If file changes externally and user has undo history:
- Current: undo history remains (may produce inconsistent states)
- Future: consider clearing or marking undo stack

---

## Test Requirements

### Basic Operations

1. Single insert → undo → redo cycle
2. Single delete → undo → redo cycle
3. Replace → undo → redo cycle
4. Multiple operations → multiple undos → multiple redos

### Multi-Cursor

1. Multi-cursor insert → single undo reverses all
2. Multi-cursor delete → single undo restores all
3. Cursor positions restored exactly

### Stack Behavior

1. Redo cleared after new edit
2. Undo doesn't affect redo if no edit between
3. Empty stack operations are no-ops

### Cursor Restoration

1. Cursor position restored on undo
2. Selection restored on undo
3. Multiple cursors restored on undo
4. desired_column restored on undo

### Content Integrity

1. Undo produces identical content to before edit
2. Multiple undo/redo cycles produce identical content
3. Unicode text handles correctly
4. Multi-byte characters handle correctly

---

## References

- `src/model/document.rs` - Document and EditOperation definitions
- `src/update/document.rs` - Undo/redo implementation
- `tests/text_editing.rs` - Undo/redo test suite

