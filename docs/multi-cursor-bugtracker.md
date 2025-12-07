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

### Bug #23: CollapseToSingleCursor Doesn't Reset active_cursor_index (CRASH)
- **Status:** 🟢 Fixed
- **File:** `src/update/editor.rs`
- **Lines:** 486-494
- **Description:** `CollapseToSingleCursor` (Escape key) manually reset cursors/selections vectors but forgot to reset `active_cursor_index`. If active_cursor_index > 0, subsequent calls to `active_cursor()` would panic with index out of bounds.
- **Symptoms:** Crash (SIGABRT) when pressing Escape after having multiple cursors
- **Resolution:** Changed to use `collapse_to_primary()` method which properly resets `active_cursor_index` to 0.
- **Tests:** `tests/multi_cursor.rs::test_collapse_to_single_cursor_*`

### Bug #24: AddCursorBelow Uses Primary Instead of Edge Cursor
- **Status:** 🟢 Fixed
- **File:** `src/update/editor.rs`
- **Lines:** 493-519
- **Description:** `AddCursorAbove/Below` used `primary_cursor()` (top-most) to determine where to add new cursor. When expanding downward, it kept trying to add below the top cursor which already had a cursor, so nothing happened.
- **Symptoms:** Repeated Option+Down adds cursor once then stops working
- **Resolution:** Added `top_cursor()`, `bottom_cursor()`, and `edge_cursor_vertical(up)` helpers. `AddCursorAbove` now uses top edge, `AddCursorBelow` uses bottom edge.
- **Tests:** `tests/multi_cursor.rs::test_add_cursor_*_expands_from_*_edge`

### Bug #25: DeleteLine Only Works on Primary Cursor
- **Status:** 🟢 Fixed
- **File:** `src/update/document.rs`
- **Lines:** 470-632
- **Description:** `DeleteLine` only deleted the line at `primary_cursor()`. With 3 cursors on lines 1, 2, 3, only line 1 was deleted.
- **Symptoms:** Multi-cursor DeleteLine only deletes one line
- **Resolution:** Added `lines_covered_by_all_cursors()` helper. DeleteLine now collects unique lines from all cursors, deletes them in reverse order, and collapses to single cursor.
- **Tests:** `tests/multi_cursor.rs::test_delete_line_multi_cursor_*`

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
- **Status:** 🟢 Fixed
- **File:** `src/update/document.rs`
- **Lines:** 995-1050
- **Description:** IndentLines only processed the line range from the primary selection.
- **Resolution:** Now uses `lines_covered_by_all_cursors()` to collect unique lines from all cursors/selections. Processes in reverse document order, adjusts all cursor/selection columns, records as Batch for proper undo.
- **Tests:** `tests/multi_cursor.rs::test_indent_multi_cursor_*`

### Bug #6: Unindent Only Works on Primary Selection
- **Status:** 🟢 Fixed
- **File:** `src/update/document.rs`
- **Lines:** 1052-1127
- **Description:** UnindentLines only processed the line range from the primary selection.
- **Resolution:** Same approach as Bug #5 - uses `lines_covered_by_all_cursors()`, tracks per-line removal amounts, adjusts all cursor/selection columns accordingly.
- **Tests:** `tests/multi_cursor.rs::test_unindent_multi_cursor_*`

### Bug #7: Expand/Shrink Selection Only Works on Primary
- **Status:** 🟢 Fixed
- **File:** `src/update/editor.rs`
- **Lines:** 963-1016
- **Description:** Selection expansion/shrinkage (Option+Up/Down) only operates on the primary cursor's selection.
- **Resolution:** API refactor now uses `active_selection()` for these operations.

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
- **Status:** 🟢 Fixed
- **File:** `src/input.rs`
- **Lines:** 260-376
- **Description:** When pressing arrow keys with selections, only the primary selection's start/end is used for cursor positioning.
- **Resolution:** API refactor now uses `active_selection()` and `active_cursor_mut()` for arrow key navigation.

### Bug #11: delete_selection Helper is Single-Cursor
- **Status:** 🔴 Open
- **File:** `src/update/editor.rs`
- **Lines:** 919-956
- **Description:** The `delete_selection` helper function only handles the primary selection.
- **Note:** This may be intentional for some call sites; needs audit
- **Fix:** Either document as single-cursor helper or make multi-cursor aware

### Bug #12-14: Helper Functions Use Index 0
- **Status:** 🟢 Fixed
- **File:** `src/model/editor.rs`
- **Lines:** 628-641
- **Description:** `move_cursor_to_offset()`, `cursor_offset()`, and `current_line_length()` all hardcode index 0.
- **Resolution:** API refactor - call sites now use appropriate `active_cursor()` or `primary_cursor()` based on intent.

### Bug #19: Model-Level Cursor Helpers Use Index 0
- **Status:** 🟢 Fixed
- **File:** `src/model/mod.rs`
- **Lines:** 201-217
- **Description:** `set_cursor_position()` and `move_cursor_to_position()` directly modify `cursors[0]`.
- **Resolution:** Call sites now use `active_cursor()` for UI operations.

### Bug #20: Find Next/Previous Only Uses Primary Cursor
- **Status:** 🟢 Fixed
- **File:** `src/update/editor.rs`
- **Lines:** 460-474
- **Description:** Find operations use `selections[0]` and `cursors[0]` directly with comment "Single selection semantics".
- **Resolution:** API refactor now uses `active_cursor()` for find operations.

### Bug #21: Page Up/Down Ignores Secondary Selections
- **Status:** 🟢 Fixed
- **File:** `src/input.rs`
- **Lines:** 273-289
- **Description:** Page Up/Down uses `selection()` and `cursor_mut()` which only affect primary cursor.
- **Resolution:** API refactor now uses `active_selection()` and `active_cursor_mut()` for page navigation.

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

### Current State (after refactor)
- Cursors stored in `Vec<Cursor>` sorted by document position (line, column)
- Primary cursor is always `cursors[0]` (top-most in document)
- `active_cursor_index` tracks which cursor the user is focused on
- API explicitly distinguishes:
  - `primary_cursor()` / `primary_selection()` - index 0, for undo metadata
  - `active_cursor()` / `active_selection()` - user's focus, for UI/viewport
  - `top_cursor()` / `bottom_cursor()` - edge cursors for directional expansion
  - `edge_cursor_vertical(up)` - generic edge accessor
  - `lines_covered_by_all_cursors()` - unique lines for line-based ops (DeleteLine, Indent)
  - Direct `cursors[idx]` access - for multi-cursor iteration

### Completed Refactor (2024-12)
- ✅ Renamed `cursor()` → `primary_cursor()`, `selection()` → `primary_selection()`
- ✅ Renamed `cursor_mut()` → `primary_cursor_mut()`, `selection_mut()` → `primary_selection_mut()`
- ✅ All ~116 call sites classified and updated:
  - UI/viewport code → `active_cursor()` / `active_selection()`
  - Undo/redo metadata → `primary_cursor()` / `primary_selection()`
  - Multi-cursor operations → iterate all cursors
- ✅ Compiler now forces explicit choice at every call site

### Bugs Fixed by Refactor
The following bugs were fixed by switching to `active_cursor()`/`active_selection()`:
- Bug #7: Expand/Shrink Selection - now uses active selection
- Bug #10: Arrow Key Navigation - now uses active selection for jump-to-boundary
- Bug #12-14: Helper functions - now use active cursor index
- Bug #19: Model-level helpers - now use active cursor
- Bug #20: Find Next/Previous - now uses active cursor position
- Bug #21: Page Up/Down - now uses active selection

### Remaining Work
These bugs require additional logic beyond the API refactor:
- Bug #3: Undo loses multi-cursor state (needs Batch cursor restoration)
- Bug #4: Duplicate only works on primary (needs multi-cursor iteration)
- Bug #11: delete_selection helper (needs audit of call sites)
- Bug #22: Status bar selection info (enhancement)
