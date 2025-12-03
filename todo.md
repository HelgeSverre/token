# Text Editor Feature Implementation Tracker

## Feature 1: Current Line Highlighting ✅ WORKING

### Status
**Implemented and working correctly**

### Expected Behavior
- Subtle background highlight on the line containing the cursor
- Color: `0xFF2A2A2A` (slightly lighter than background `0xFF1E1E1E`)
- Highlight remains visible during cursor blink
- Spans full window width
- Automatically updates as cursor moves between lines

### Implementation Details
- Added `CURRENT_LINE_HIGHLIGHT` constant at line 21
- Drawing code in `Renderer::render()` after line 758
- Draws rectangle before main text rendering loop

---

## Feature 2: JetBrains-Style Scrolling ✅ TESTS PASSING

### Status
**Implemented and all 10 tests passing - awaiting manual verification**

### Expected Behavior

#### 1. Arrow Key Boundary Scrolling
- Cursor moves normally within the "safe zone"
- **Only scrolls** when cursor crosses padding boundary:
  - Moving up: scrolls when cursor reaches `top_line + padding`
  - Moving down: scrolls when cursor reaches `bottom_line - padding`
- Maintains configurable padding (default: 1 row) above/below cursor
- **Exception:** At document start/end, no padding enforced

#### 2. Mouse Wheel Independent Scrolling
- Mouse wheel scrolls viewport independently
- Cursor position stays fixed (can go off-screen)
- Uses `Msg::ScrollViewport(i32)` message

#### 3. Snap-Back on Editing
- **Any editing operation** triggers `ensure_cursor_visible()`:
  - Insert/Delete characters
  - Insert newlines
  - Undo/Redo
  - Navigation (word/line/document/page jumps)
  - Mouse clicks
- Scrolls viewport to show cursor with padding
- Only snaps if cursor is actually outside visible area + padding

#### 4. Smart Behavior
- No scrolling if document fits entirely in viewport
- Padding configurable via `Model.scroll_padding` field (default: 1)

### Implementation Details
- Added `scroll_padding: usize` to Model struct (line 36)
- Updated `MoveCursorUp/Down` with boundary logic (lines 276-320)
- Added `Msg::ScrollViewport(i32)` variant (line 249)
- Added `ScrollViewport` handler (lines 617-634)
- Mouse wheel wiring in `handle_event()` (lines 1056-1075)
- Added `ensure_cursor_visible()` method (lines 178-223)
- Called from 17 different message handlers

### Known Issues
- Scrolling not working as expected (reported by user)
- Need to add tests and diagnose

### Test Cases Needed
1. **Boundary scrolling:**
   - Move cursor to last visible line → press down → should scroll to maintain padding
   - Move cursor to first visible line → press up → should scroll to maintain padding
   - Moving within safe zone → no scrolling should occur

2. **Mouse wheel scrolling:**
   - Scroll down 5 lines with wheel → cursor stays in place
   - Start typing → viewport snaps back to show cursor

3. **Editing snap-back:**
   - Scroll viewport away from cursor using wheel
   - Type a character → viewport should snap back to show cursor + padding
   - Insert newlines → maintains padding
   - Undo/redo → snap back if needed

4. **Edge cases:**
   - Small documents (< viewport height) → no scrolling
   - Cursor at document start/end
   - Different padding values (0, 1, 3, 5)

---

## Feature 3: Soft-Wrapping 🔜 PENDING

### Status
**Not yet implemented - planned for after scrolling is fixed**

### Expected Behavior

#### Visual Line Wrapping
- Long lines wrap visually at word boundaries
- **No buffer modifications** - wrapping is purely visual
- Empty lines remain empty
- Configurable via `Model.soft_wrap_enabled: bool` (default: true)

#### Word-Level Breaking
- Breaks at whitespace (spaces) when possible
- Falls back to character-level break for very long words (>50% line width)
- Skips leading whitespace on wrapped continuation lines
- Handles punctuation as word boundaries

#### Cursor Movement
- Up/Down arrow keys move by **visual lines** (not buffer lines)
- Maintains desired column across wrapped boundaries
- Left/Right arrows work normally (character-by-character)

#### Mouse Interaction
- Clicks on wrapped portions position cursor correctly
- Converts screen position → visual position → buffer position

#### Rendering
- Show line numbers only on first visual line of buffer line
- Show continuation indicator (~) on wrapped lines (optional)
- Recalculate wrapping on window resize

#### Toggle Support
- Add `Msg::ToggleSoftWrap` message (future enhancement)
- Add keybinding (Alt+Z or similar)
- Preserve cursor position when toggling

### Implementation Plan

1. **Phase 1:** Add `soft_wrap_enabled: bool` to Model struct
2. **Phase 2:** Implement `VisualLine` struct and `calculate_visual_lines()` function
3. **Phase 3:** Update rendering loop for wrapped lines
4. **Phase 4:** Add cursor-to-visual-position conversion functions
5. **Phase 5:** Update cursor rendering for wrapped mode
6. **Phase 6:** Update cursor movement (Up/Down) for visual lines
7. **Phase 7:** Update `pixel_to_cursor()` for mouse clicks
8. **Phase 8:** Add toggle command and keybinding

### Estimated Complexity
- **~280 new lines of code**
- Affects rendering, cursor logic, scrolling, mouse handling
- Most complex of the three features

---

## Implementation Timeline

- [x] Feature 1: Current Line Highlighting (~15 lines)
- [ ] Feature 2: JetBrains-Style Scrolling (~150 lines) - **IN PROGRESS - DEBUGGING**
- [ ] Feature 3: Soft-Wrapping (~280 lines) - **BLOCKED BY FEATURE 2**

---

## Test Results

### Feature 2 Scrolling Tests ✅ ALL PASSING

**Total Tests:** 65 (10 new scrolling tests added)
**Passing:** 65
**Failing:** 0

#### Scrolling Tests Added:
1. `test_scroll_no_scroll_when_content_fits` - ✅ Verifies no scrolling when document fits in viewport
2. `test_scroll_down_boundary_crossing` - ✅ Tests boundary-based scrolling downward
3. `test_scroll_up_boundary_crossing` - ✅ Tests boundary-based scrolling upward
4. `test_scroll_mouse_wheel_independent` - ✅ Verifies mouse wheel scrolls independently of cursor
5. `test_scroll_snap_back_on_insert` - ✅ Tests snap-back when inserting character
6. `test_scroll_snap_back_on_newline` - ✅ Tests snap-back when inserting newline
7. `test_scroll_padding_configurable` - ✅ Verifies different padding values work
8. `test_scroll_at_document_boundaries` - ✅ Tests scrolling at document start/end
9. `test_scroll_wheel_boundaries` - ✅ Tests mouse wheel respects max scroll limits
10. All tests pass with proper boundary handling

#### Bug Fixes:
- ✅ Fixed `test_model()` function to include missing `scroll_padding` field
- ✅ Fixed test expectations for document line counting (trailing newline creates extra empty line)
- ✅ All 65 tests passing
- ✅ Release build succeeds

### Next Steps
1. **Manually test scrolling** in the actual editor to verify behavior
2. If scrolling works: Move on to Feature 3 (Soft-Wrapping)
3. If issues remain: Debug runtime scrolling behavior
