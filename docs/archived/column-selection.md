# Column Selection (Rectangular/Box Selection)

Select rectangular regions of text and convert to multi-cursor

> **Status:** ✅ Implemented
> **Implemented:** v0.3.x
> **Keybindings:** Middle-mouse drag for rectangle select
> **Created:** 2025-12-19

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

The editor currently supports:
- Multi-cursor via Cmd+Click (toggle cursor at position)
- Add cursor above/below (Option+Option+Arrow)
- Rectangle selection via middle mouse drag (`RectangleSelectionState`)
- Selection expansion with Option+Up/Down

The existing `RectangleSelectionState` in `src/model/editor.rs` uses **visual columns** (screen position) for consistent rectangle selection across lines of different lengths.

### Goals

1. **Keyboard-driven rectangle selection**: Alt+Shift+Arrow to extend rectangular selection
2. **Mouse rectangle selection**: Middle mouse drag (already exists) + Alt+Shift+Click+Drag
3. **Visual column semantics**: Rectangle defined by screen position, not character index
4. **Tab handling**: Tabs expand to visual columns correctly
5. **Convert to multi-cursor**: On completion, create cursor at each line's column position
6. **Copy/paste rectangle**: Copy as rectangular block, paste as multi-line insert

### Non-Goals

- Virtual space editing (inserting past end of line)
- Column-mode insert/overtype (VS Code-style, where typing affects all lines)
- Right-to-left text support

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                          Rectangle Selection Flow                            │
│                                                                              │
│  ┌──────────────────────────────────────────────────────────────────────┐   │
│  │                        Input Events                                   │   │
│  │                                                                       │   │
│  │  Alt+Shift+Arrow  ───────▶ EditorMsg::ExtendRectangleSelection       │   │
│  │  Middle Mouse Down ──────▶ EditorMsg::StartRectangleSelection        │   │
│  │  Alt+Shift+Click  ───────▶ EditorMsg::StartRectangleSelection        │   │
│  │  Middle Mouse Drag ──────▶ EditorMsg::UpdateRectangleSelection       │   │
│  │  Alt+Shift+Drag   ───────▶ EditorMsg::UpdateRectangleSelection       │   │
│  │  Mouse Up / Esc   ───────▶ EditorMsg::FinishRectangleSelection       │   │
│  │  Escape           ───────▶ EditorMsg::CancelRectangleSelection       │   │
│  └──────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
│  ┌──────────────────────────────────────────────────────────────────────┐   │
│  │                     RectangleSelectionState                           │   │
│  │                                                                       │   │
│  │   active: bool                                                        │   │
│  │   start_line: usize                                                   │   │
│  │   start_visual_col: usize  ◄── Screen column, not char column        │   │
│  │   current_line: usize                                                 │   │
│  │   current_visual_col: usize                                           │   │
│  │   preview_cursors: Vec<Position>  ◄── Updated during drag            │   │
│  │                                                                       │   │
│  │   Helper methods:                                                     │   │
│  │     top_line() / bottom_line()                                        │   │
│  │     left_visual_col() / right_visual_col()                            │   │
│  └──────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
│  ┌──────────────────────────────────────────────────────────────────────┐   │
│  │                        Visual Column Mapping                          │   │
│  │                                                                       │   │
│  │   char_to_visual_col(line, char_col, tab_width) -> visual_col        │   │
│  │   visual_to_char_col(line, visual_col, tab_width) -> char_col        │   │
│  │                                                                       │   │
│  │   Example with tab_width=4:                                           │   │
│  │     Line: "a\tb"                                                      │   │
│  │     Char:  0 1 2                                                      │   │
│  │     Vis:   0 4 5   (tab at char 1 expands to visual 1,2,3,4)         │   │
│  └──────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
│  ┌──────────────────────────────────────────────────────────────────────┐   │
│  │                       Finish Selection                                │   │
│  │                                                                       │   │
│  │   For each line in [top_line..=bottom_line]:                          │   │
│  │     1. Map left_visual_col to char_col                                │   │
│  │     2. Map right_visual_col to char_col                               │   │
│  │     3. Clamp to line length                                           │   │
│  │     4. Create Selection(anchor=left, head=right)                      │   │
│  │     5. Create Cursor at head position                                 │   │
│  │                                                                       │   │
│  │   Result: Multiple cursors with selections                            │   │
│  │   Clear rectangle_selection state                                     │   │
│  └──────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Visual Column Concept

```
Line content: "Hello\tWorld"
Tab width: 4

Character positions:  H  e  l  l  o  \t  W  o  r  l  d
Character index:      0  1  2  3  4  5   6  7  8  9  10
Visual column:        0  1  2  3  4  8   9  10 11 12 13
                                    ^
                                    Tab expands from visual 5-7

Rectangle selection from visual (4,0) to (10,2):
  ┌──────┐
  │Hello ├──┐  (visual cols 4-8)
  │World │  │  (visual cols 4-8 on this line)
  │Foo   │  │  (only "Foo" selected if line is short)
  └──────┘──┘
```

---

## Data Structures

### RectangleSelectionState (Existing, Enhanced)

```rust
// In src/model/editor.rs (already exists, verify/enhance)

/// State for an in-progress rectangle selection (middle mouse drag)
/// Uses VISUAL columns (screen position) rather than character columns
/// so rectangle selection works consistently across lines of different lengths.
#[derive(Debug, Clone, Default)]
pub struct RectangleSelectionState {
    /// Whether a rectangle selection is currently active
    pub active: bool,

    /// Starting line
    pub start_line: usize,

    /// Starting visual column (screen position)
    pub start_visual_col: usize,

    /// Current line (where mouse/cursor is now)
    pub current_line: usize,

    /// Current visual column (screen position)
    pub current_visual_col: usize,

    /// Preview cursor positions (computed during drag, shown before commit)
    /// These are CHARACTER positions, not visual positions
    pub preview_cursors: Vec<Position>,

    /// Preview selections (parallel to preview_cursors)
    pub preview_selections: Vec<Selection>,

    /// Whether selection was started via keyboard (vs mouse)
    pub keyboard_mode: bool,
}

impl RectangleSelectionState {
    /// Get the top line of the rectangle
    pub fn top_line(&self) -> usize {
        self.start_line.min(self.current_line)
    }

    /// Get the bottom line of the rectangle
    pub fn bottom_line(&self) -> usize {
        self.start_line.max(self.current_line)
    }

    /// Get the left visual column of the rectangle
    pub fn left_visual_col(&self) -> usize {
        self.start_visual_col.min(self.current_visual_col)
    }

    /// Get the right visual column of the rectangle
    pub fn right_visual_col(&self) -> usize {
        self.start_visual_col.max(self.current_visual_col)
    }

    /// Start a new rectangle selection
    pub fn start(&mut self, line: usize, visual_col: usize, keyboard_mode: bool) {
        self.active = true;
        self.start_line = line;
        self.start_visual_col = visual_col;
        self.current_line = line;
        self.current_visual_col = visual_col;
        self.preview_cursors.clear();
        self.preview_selections.clear();
        self.keyboard_mode = keyboard_mode;
    }

    /// Update the current corner of the rectangle
    pub fn update(&mut self, line: usize, visual_col: usize) {
        self.current_line = line;
        self.current_visual_col = visual_col;
    }

    /// Cancel the rectangle selection
    pub fn cancel(&mut self) {
        self.active = false;
        self.preview_cursors.clear();
        self.preview_selections.clear();
    }

    /// Check if this is an empty (zero-area) selection
    pub fn is_empty(&self) -> bool {
        self.start_line == self.current_line
            && self.start_visual_col == self.current_visual_col
    }
}
```

### Visual Column Utilities

```rust
// In src/util.rs or src/model/editor.rs

/// Tab width for visual column calculations
pub const TAB_WIDTH: usize = 4;

/// Convert a character column to a visual (screen) column
///
/// Handles tabs by expanding them to the next tab stop.
/// Example: with TAB_WIDTH=4, a tab at char 1 expands to visual 4.
pub fn char_to_visual_col(line: &str, char_col: usize, tab_width: usize) -> usize {
    let mut visual = 0;
    for (i, ch) in line.chars().enumerate() {
        if i >= char_col {
            break;
        }
        if ch == '\t' {
            // Advance to next tab stop
            visual = (visual / tab_width + 1) * tab_width;
        } else {
            visual += 1;
        }
    }
    visual
}

/// Convert a visual (screen) column to a character column
///
/// Returns the character column that corresponds to the given visual column.
/// If the visual column falls within a tab, returns the tab's character position.
/// If the visual column is past the end of the line, returns the line length.
pub fn visual_to_char_col(line: &str, visual_col: usize, tab_width: usize) -> usize {
    let mut visual = 0;
    for (i, ch) in line.chars().enumerate() {
        if visual >= visual_col {
            return i;
        }
        if ch == '\t' {
            let next_visual = (visual / tab_width + 1) * tab_width;
            if visual_col < next_visual {
                return i; // Visual column is inside this tab
            }
            visual = next_visual;
        } else {
            visual += 1;
        }
    }
    // Visual column is past end of line
    line.chars().count()
}

/// Get the visual width of a line (accounting for tabs)
pub fn line_visual_width(line: &str, tab_width: usize) -> usize {
    let mut visual = 0;
    for ch in line.chars() {
        if ch == '\t' {
            visual = (visual / tab_width + 1) * tab_width;
        } else if ch != '\n' && ch != '\r' {
            visual += 1;
        }
    }
    visual
}
```

### Messages

```rust
// In src/messages.rs

pub enum EditorMsg {
    // ... existing variants ...

    // === Rectangle Selection (Enhanced) ===
    /// Start rectangle selection at position (visual column = screen position)
    StartRectangleSelection { line: usize, visual_col: usize },

    /// Update rectangle selection to position (visual column = screen position)
    UpdateRectangleSelection { line: usize, visual_col: usize },

    /// Finish rectangle selection and commit to multi-cursor
    FinishRectangleSelection,

    /// Cancel rectangle selection
    CancelRectangleSelection,

    /// Start keyboard-driven rectangle selection from current cursor
    StartKeyboardRectangleSelection,

    /// Extend keyboard rectangle selection in direction
    ExtendRectangleSelection(Direction),
}
```

---

## Keybindings

| Action | Mac | Windows/Linux | Message |
|--------|-----|---------------|---------|
| Start keyboard rect select | Alt+Shift+Arrow (first) | Alt+Shift+Arrow (first) | `StartKeyboardRectangleSelection` |
| Extend rect up | Alt+Shift+Up | Alt+Shift+Up | `ExtendRectangleSelection(Up)` |
| Extend rect down | Alt+Shift+Down | Alt+Shift+Down | `ExtendRectangleSelection(Down)` |
| Extend rect left | Alt+Shift+Left | Alt+Shift+Left | `ExtendRectangleSelection(Left)` |
| Extend rect right | Alt+Shift+Right | Alt+Shift+Right | `ExtendRectangleSelection(Right)` |
| Mouse rect select | Middle Drag | Middle Drag | `StartRectangleSelection` + `UpdateRectangleSelection` |
| Alt+Mouse rect select | Alt+Shift+Drag | Alt+Shift+Drag | Same as middle drag |
| Commit selection | Release mouse / Any key | Release mouse / Any key | `FinishRectangleSelection` |
| Cancel selection | Escape | Escape | `CancelRectangleSelection` |

### Keymap Entry

```yaml
# keymap.yaml
- keys: [alt+shift+up]
  command: ExtendRectangleSelection
  args: { direction: up }

- keys: [alt+shift+down]
  command: ExtendRectangleSelection
  args: { direction: down }

- keys: [alt+shift+left]
  command: ExtendRectangleSelection
  args: { direction: left }

- keys: [alt+shift+right]
  command: ExtendRectangleSelection
  args: { direction: right }
```

---

## Implementation Plan

### Phase 1: Visual Column Utilities

**Estimated effort: 1-2 days**

1. [ ] Add `char_to_visual_col()` to `src/util.rs`
2. [ ] Add `visual_to_char_col()` to `src/util.rs`
3. [ ] Add `line_visual_width()` to `src/util.rs`
4. [ ] Add comprehensive unit tests for tab handling
5. [ ] Handle edge cases (empty line, all tabs, mixed content)

**Test:** Verify visual column mapping with various tab scenarios

### Phase 2: Enhance RectangleSelectionState

**Estimated effort: 1 day**

1. [ ] Add `keyboard_mode` field
2. [ ] Add `preview_selections` parallel to `preview_cursors`
3. [ ] Add helper methods for starting/updating/canceling
4. [ ] Ensure preview is computed on each update

**Test:** State management works correctly

### Phase 3: Preview Cursor Computation

**Estimated effort: 2 days**

1. [ ] Create `compute_rectangle_preview()` function
2. [ ] For each line in range, map visual columns to char columns
3. [ ] Clamp to line length (no virtual space)
4. [ ] Create `Position` for cursor and `Selection` for each line
5. [ ] Store in `preview_cursors` and `preview_selections`

```rust
fn compute_rectangle_preview(
    state: &mut RectangleSelectionState,
    document: &Document,
    tab_width: usize,
) {
    state.preview_cursors.clear();
    state.preview_selections.clear();

    let left_vis = state.left_visual_col();
    let right_vis = state.right_visual_col();

    for line_idx in state.top_line()..=state.bottom_line() {
        if let Some(line_text) = document.get_line(line_idx) {
            let line_text = line_text.trim_end_matches('\n');
            let line_len = line_text.chars().count();

            // Map visual columns to character columns
            let left_char = visual_to_char_col(line_text, left_vis, tab_width);
            let right_char = visual_to_char_col(line_text, right_vis, tab_width);

            // Clamp to line length
            let left_char = left_char.min(line_len);
            let right_char = right_char.min(line_len);

            // Create cursor at right edge (head of selection)
            state.preview_cursors.push(Position::new(line_idx, right_char));

            // Create selection from left to right
            state.preview_selections.push(Selection::from_positions(
                Position::new(line_idx, left_char),
                Position::new(line_idx, right_char),
            ));
        }
    }
}
```

**Test:** Preview cursors computed correctly for various rectangles

### Phase 4: Mouse Rectangle Selection (Enhance Existing)

**Estimated effort: 1-2 days**

1. [ ] Verify existing middle-mouse handling in `handle_mouse_event()`
2. [ ] Map pixel position to (line, visual_col)
3. [ ] Call `compute_rectangle_preview()` on mouse move
4. [ ] On mouse release, call `FinishRectangleSelection`
5. [ ] Add Alt+Shift+Drag as alternative to middle mouse

**Test:** Mouse rectangle selection works end-to-end

### Phase 5: Keyboard Rectangle Selection

**Estimated effort: 2-3 days**

1. [ ] Handle `StartKeyboardRectangleSelection` message
2. [ ] Initialize from current cursor position (convert to visual col)
3. [ ] Handle `ExtendRectangleSelection(Direction)` messages
4. [ ] Move `current_line/current_visual_col` by one step
5. [ ] Recompute preview on each extend
6. [ ] Commit on next non-extend key (e.g., typing, Escape)

**Test:** Keyboard rectangle selection matches expected behavior

### Phase 6: Finish Selection (Commit to Multi-Cursor)

**Estimated effort: 1-2 days**

1. [ ] Handle `FinishRectangleSelection` message
2. [ ] Replace `cursors` and `selections` with preview values
3. [ ] Clear rectangle selection state
4. [ ] Deduplicate cursors (in case of zero-width selection)
5. [ ] Set `active_cursor_index` to last cursor

```rust
fn finish_rectangle_selection(editor: &mut EditorState) {
    if !editor.rectangle_selection.active {
        return;
    }

    // Replace cursors with preview
    editor.cursors = editor.rectangle_selection.preview_cursors
        .iter()
        .map(|p| Cursor::from_position(*p))
        .collect();

    editor.selections = editor.rectangle_selection.preview_selections.clone();

    // Set active to last cursor
    editor.active_cursor_index = editor.cursors.len().saturating_sub(1);

    // Clear rectangle state
    editor.rectangle_selection.cancel();

    // Deduplicate if any cursors overlap
    editor.deduplicate_cursors();
}
```

**Test:** Multi-cursor state correct after commit

### Phase 7: Rendering

**Estimated effort: 1-2 days**

1. [ ] Render rectangle overlay during selection
2. [ ] Show column markers at left/right visual columns
3. [ ] Render preview cursors with distinct style
4. [ ] Handle scrolling during rectangle drag

**Test:** Visual feedback matches selection state

### Phase 8: Copy/Paste Rectangle

**Estimated effort: 2 days**

1. [ ] On copy with rectangle selection, format as rectangular block
2. [ ] Store in clipboard with special format indicator
3. [ ] On paste, detect rectangular content
4. [ ] If rectangular and multi-cursor, paste per-line
5. [ ] If rectangular and single cursor, paste as block

**Test:** Copy rectangle, paste at single cursor, verify block layout

---

## Testing Strategy

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_char_to_visual_simple() {
        let line = "hello";
        assert_eq!(char_to_visual_col(line, 0, 4), 0);
        assert_eq!(char_to_visual_col(line, 3, 4), 3);
        assert_eq!(char_to_visual_col(line, 5, 4), 5);
    }

    #[test]
    fn test_char_to_visual_with_tab() {
        let line = "a\tb";  // 'a' at 0, tab at 1, 'b' at 2
        assert_eq!(char_to_visual_col(line, 0, 4), 0);  // 'a'
        assert_eq!(char_to_visual_col(line, 1, 4), 1);  // before tab
        assert_eq!(char_to_visual_col(line, 2, 4), 4);  // 'b' (after tab)
    }

    #[test]
    fn test_visual_to_char_simple() {
        let line = "hello";
        assert_eq!(visual_to_char_col(line, 0, 4), 0);
        assert_eq!(visual_to_char_col(line, 3, 4), 3);
        assert_eq!(visual_to_char_col(line, 10, 4), 5); // past end
    }

    #[test]
    fn test_visual_to_char_with_tab() {
        let line = "a\tb";
        assert_eq!(visual_to_char_col(line, 0, 4), 0);  // 'a'
        assert_eq!(visual_to_char_col(line, 1, 4), 1);  // inside tab
        assert_eq!(visual_to_char_col(line, 2, 4), 1);  // inside tab
        assert_eq!(visual_to_char_col(line, 3, 4), 1);  // inside tab
        assert_eq!(visual_to_char_col(line, 4, 4), 2);  // 'b'
    }

    #[test]
    fn test_visual_to_char_roundtrip() {
        let line = "ab\tcd\tef";
        for char_col in 0..line.chars().count() {
            let visual = char_to_visual_col(line, char_col, 4);
            let back = visual_to_char_col(line, visual, 4);
            assert_eq!(back, char_col, "roundtrip failed for char_col {}", char_col);
        }
    }

    #[test]
    fn test_rectangle_bounds() {
        let mut state = RectangleSelectionState::default();
        state.start(5, 10, false);  // line 5, visual col 10
        state.update(10, 5);         // line 10, visual col 5

        assert_eq!(state.top_line(), 5);
        assert_eq!(state.bottom_line(), 10);
        assert_eq!(state.left_visual_col(), 5);
        assert_eq!(state.right_visual_col(), 10);
    }

    #[test]
    fn test_rectangle_preview_generation() {
        let doc = Document::with_text("hello\nworld\ntest");
        let mut state = RectangleSelectionState::default();
        state.start(0, 1, false);  // line 0, visual col 1
        state.update(2, 4);         // line 2, visual col 4

        compute_rectangle_preview(&mut state, &doc, 4);

        assert_eq!(state.preview_cursors.len(), 3);
        assert_eq!(state.preview_selections.len(), 3);

        // Line 0: "hello" -> select cols 1-4 -> "ello"
        assert_eq!(state.preview_cursors[0], Position::new(0, 4));
        assert_eq!(state.preview_selections[0].start(), Position::new(0, 1));
        assert_eq!(state.preview_selections[0].end(), Position::new(0, 4));
    }
}
```

### Integration Tests

```rust
#[test]
fn test_keyboard_rectangle_selection() {
    let mut model = test_model_with_text("aaaa\nbbbb\ncccc\ndddd");

    // Position cursor at (1, 1)
    set_cursor(&mut model, 1, 1);

    // Start keyboard rect selection
    dispatch(&mut model, Msg::Editor(EditorMsg::StartKeyboardRectangleSelection));

    // Extend down and right
    dispatch(&mut model, Msg::Editor(EditorMsg::ExtendRectangleSelection(Direction::Down)));
    dispatch(&mut model, Msg::Editor(EditorMsg::ExtendRectangleSelection(Direction::Right)));

    // Commit
    dispatch(&mut model, Msg::Editor(EditorMsg::FinishRectangleSelection));

    // Should have 2 cursors (lines 1 and 2)
    assert_eq!(model.editor().cursors.len(), 2);
    assert_eq!(model.editor().cursors[0].line, 1);
    assert_eq!(model.editor().cursors[1].line, 2);
}

#[test]
fn test_rectangle_selection_with_short_lines() {
    let mut model = test_model_with_text("looooong\nhi\nmedium");

    // Select rectangle from (0, 2) to (2, 6)
    start_rectangle(&mut model, 0, 2);
    update_rectangle(&mut model, 2, 6);
    finish_rectangle(&mut model);

    // Line 1 "hi" is short, cursor should be at column 2 (end of line)
    assert_eq!(model.editor().cursors[1].column, 2);
}
```

### Manual Testing Checklist

- [ ] Middle mouse drag creates rectangle selection
- [ ] Alt+Shift+Drag creates rectangle selection
- [ ] Alt+Shift+Arrow extends from cursor
- [ ] Rectangle respects visual columns (tabs expand correctly)
- [ ] Short lines clamp to their length
- [ ] Escape cancels rectangle selection
- [ ] Commit creates multi-cursor with selections
- [ ] Copy rectangle, paste at single cursor
- [ ] Undo removes all multi-cursor changes atomically

---

## Edge Cases and Invariants

### Edge Cases

1. **Zero-width rectangle**: Same start/end column; create cursors without selection
2. **Zero-height rectangle**: Single line; equivalent to normal selection
3. **Past end of line**: Clamp to line length (no virtual space)
4. **Empty lines**: Cursor at column 0
5. **Tabs in selection**: Map visual columns correctly
6. **Mixed tabs/spaces**: Each character type occupies correct visual width
7. **Unicode**: Wide characters occupy multiple visual columns (future)
8. **Scrolling during drag**: Viewport scrolls when mouse near edge

### Invariants

1. `preview_cursors.len() == preview_selections.len()` during active selection
2. `preview_cursors.len() == (bottom_line - top_line + 1)` (one per line)
3. All `preview_cursors` positions are within document bounds
4. After commit, `cursors.len() == selections.len()`
5. Rectangle selection clears on any non-rectangle-related edit

---

## References

- VS Code column selection: https://code.visualstudio.com/docs/editor/codebasics#_column-box-selection
- Sublime Text column selection: Ctrl+Shift+Arrow (similar pattern)
- Existing rectangle state: `src/model/editor.rs` (RectangleSelectionState)
- Multi-cursor handling: `src/update/editor.rs`
- Existing middle mouse handling: `src/input.rs`
