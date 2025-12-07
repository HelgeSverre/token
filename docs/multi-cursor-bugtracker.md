# Multi-Cursor Bug Tracker

This document tracks known issues where the editor assumes single-cursor behavior or has incomplete multi-cursor support.

## Status Legend
- 🔴 **Open** - Not yet addressed
- 🟡 **In Progress** - Being worked on
- 🟢 **Fixed** - Completed and tested

---

## Critical Priority

### Bug #1: AddCursorBelow Not Visible
- **Status:** 🟢 Fixed
- **File:** `src/model/editor.rs`
- **Description:** When adding a cursor below with Option+Option+↓, the new cursor is added but not set as the "active" cursor. After sorting, the original cursor remains at index 0 (primary), so the viewport doesn't scroll to show the new cursor.
- **Resolution:** Added `active_cursor_index` field to EditorState. `add_cursor_at()` and `toggle_cursor_at()` now set the new cursor as active. `sort_cursors()` tracks the active cursor through sorting.

### Bug #2: Viewport Only Follows Primary Cursor
- **Status:** 🟢 Fixed
- **File:** `src/model/editor.rs`
- **Description:** `ensure_cursor_visible()` always uses `cursors[0]`, ignoring which cursor the user is actually working with.
- **Resolution:** `ensure_cursor_visible_with_mode()` now uses `cursors[active_cursor_index]` instead of `cursors[0]`.

### Bug #3: Undo Loses Multi-Cursor State
- **Status:** 🔴 Open
- **File:** `src/update/document.rs`
- **Lines:** 1064-1178
- **Description:** For non-Batch undo operations (Insert, Delete, Replace), only the primary cursor position is stored and restored. Secondary cursors are lost on undo.
- **Current Behavior:** After undo, only one cursor remains at the primary position
- **Expected Behavior:** All cursor positions should be preserved and restored
- **Fix:** Store full cursor/selection vectors in EditOperation, restore all cursors on undo

---

## High Priority

### Bug #4: Duplicate Only Works on Primary Cursor
- **Status:** 🔴 Open
- **File:** `src/update/document.rs`
- **Lines:** 814-903
- **Description:** The Duplicate operation (Cmd+D) only reads and duplicates based on the primary cursor's position/selection.
- **Current Behavior:** Only duplicates content at/around primary cursor
- **Expected Behavior:** Should duplicate at each cursor position
- **Fix:** Iterate over all cursors in reverse document order

### Bug #5: Indent Only Works on Primary Selection
- **Status:** 🔴 Open
- **File:** `src/update/document.rs`
- **Lines:** 912-1000
- **Description:** IndentLines only processes the line range from the primary selection.
- **Current Behavior:** Only indents lines covered by primary selection
- **Expected Behavior:** Should indent lines covered by all selections
- **Fix:** Collect all unique lines from all selections, process in reverse order

### Bug #6: Unindent Only Works on Primary Selection
- **Status:** 🔴 Open
- **File:** `src/update/document.rs`
- **Lines:** 1001-1047
- **Description:** Same as Bug #5 but for unindent operation.
- **Fix:** Same approach as Bug #5

### Bug #7: Expand/Shrink Selection Only Works on Primary
- **Status:** 🔴 Open
- **File:** `src/update/editor.rs`
- **Lines:** 963-1016
- **Description:** Selection expansion/shrinkage (Option+Up/Down) only operates on the primary cursor's selection.
- **Current Behavior:** Only primary selection expands/shrinks
- **Expected Behavior:** Should expand/shrink the active cursor's selection
- **Fix:** Use `active_selection()` instead of `selection()`

---

## Medium Priority

### NOT-ACTUALLY-A Bug #8: Line Highlighting Only for Primary Cursor 
- **Status:** 🔴 Open (by design)
- **File:** `src/view.rs`
- **Description:** Only the primary cursor's line gets the current-line background highlight.
- **Note:** User confirmed this is the desired behavior - only primary cursor line should be highlighted.

### Bug #9: Line Number Coloring Only for Primary Cursor
- **Status:** 🔴 Open (by design)
- **File:** `src/view.rs`
- **Description:** Line numbers are only colored differently for the primary cursor's line.
- **Note:** User confirmed this is the desired behavior - only primary cursor line number should be colored.

### Bug #10: Arrow Key Navigation Ignores Secondary Selections
- **Status:** 🔴 Open
- **File:** `src/input.rs`
- **Lines:** 260-376
- **Description:** When pressing arrow keys with selections, only the primary selection's start/end is used for cursor positioning.
- **Current Behavior:** Secondary cursors don't move correctly relative to their selections
- **Expected Behavior:** Each cursor should move to its selection's start/end
- **Fix:** Handle all cursor/selection pairs in movement logic

### Bug #11: delete_selection Helper is Single-Cursor
- **Status:** 🔴 Open
- **File:** `src/update/editor.rs`
- **Lines:** 919-956
- **Description:** The `delete_selection` helper function only handles the primary selection.
- **Note:** This may be intentional for some call sites; needs audit
- **Fix:** Either document as single-cursor helper or make multi-cursor aware

### Bug #12-14: Helper Functions Use Index 0
- **Status:** 🔴 Open
- **File:** `src/model/editor.rs`
- **Lines:** 628-641
- **Description:** `move_cursor_to_offset()`, `cursor_offset()`, and `current_line_length()` all hardcode index 0.
- **Fix:** Add `_at(idx)` variants or use active cursor index

### Bug #19: Model-Level Cursor Helpers Use Index 0
- **Status:** 🔴 Open
- **File:** `src/model/mod.rs`
- **Lines:** 201-217
- **Description:** `set_cursor_position()` and `move_cursor_to_position()` directly modify `cursors[0]`.
- **Current Behavior:** Only moves/sets primary cursor position
- **Expected Behavior:** Should operate on active cursor or take index parameter
- **Fix:** Use `active_cursor_index` or add `_at(idx)` variants

### Bug #20: Find Next/Previous Only Uses Primary Cursor
- **Status:** 🔴 Open
- **File:** `src/update/editor.rs`
- **Lines:** 460-474
- **Description:** Find operations use `selections[0]` and `cursors[0]` directly with comment "Single selection semantics".
- **Current Behavior:** Search starts from primary cursor only
- **Expected Behavior:** Should search from active cursor position
- **Fix:** Use `active_cursor_index` for find operations

### Bug #21: Page Up/Down Ignores Secondary Selections
- **Status:** 🔴 Open
- **File:** `src/input.rs`
- **Lines:** 273-289
- **Description:** Page Up/Down uses `selection()` and `cursor_mut()` which only affect primary cursor.
- **Current Behavior:** Only primary cursor jumps to selection boundary before page movement
- **Expected Behavior:** Each cursor should handle its selection appropriately
- **Fix:** Guard with `has_multiple_cursors()` check like arrow key fix

### Bug #22: Status Bar Only Shows Primary Selection Info
- **Status:** 🔴 Open
- **File:** `src/model/status_bar.rs`
- **Lines:** 425-440
- **Description:** `calculate_selection_info()` uses `selections.first()` - only shows primary selection.
- **Current Behavior:** Selection info in status bar ignores secondary selections
- **Expected Behavior:** Could show aggregate info or active selection info
- **Fix:** Show active selection info or total selected chars across all selections

---

## Low Priority (Enhancements)

### Enhancement #15: ExtendSelectionToPosition Collapses Multi-Cursor
- **Status:** 🔴 Open
- **File:** `src/update/editor.rs`
- **Lines:** 449-478
- **Description:** This operation explicitly collapses to a single cursor by design.
- **Note:** May be intentional for Shift+Click behavior
- **Fix:** Consider if this should extend from active cursor only

### Enhancement #16: Status Bar Cursor Count
- **Status:** 🔴 Open
- **Description:** Status bar could show cursor count when multiple cursors exist (e.g., "3 cursors")
- **Fix:** Add cursor count display to status bar

### Enhancement #17: Cycle Active Cursor Keybinding
- **Status:** 🔴 Open
- **Description:** Could add a keybinding to cycle through cursors, making each one "active" in turn.
- **Fix:** Add new EditorMsg and keybinding

### Enhancement #18: Per-Cursor Selection History
- **Status:** 🔴 Open
- **File:** `src/model/editor.rs`
- **Line:** 279
- **Description:** There's only one `selection_history` stack shared by all cursors.
- **Fix:** Could track history per cursor for better expand/shrink behavior

---

## Architecture Notes

### Current State
- Cursors stored in `Vec<Cursor>` sorted by document position (line, column)
- Primary cursor is always `cursors[0]` (top-most in document)
- `cursor()` and `selection()` methods return index 0

### Planned Changes
- Add `active_cursor_index: usize` field to `EditorState`
- Add `active_cursor()` and `active_selection()` methods
- Update `sort_cursors()` to track active cursor through reordering
- Update `deduplicate_cursors()` to handle active cursor removal
