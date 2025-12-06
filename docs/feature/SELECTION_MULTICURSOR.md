# Selection & Multi-Cursor Design

Comprehensive design for text selection and multi-cursor editing with JetBrains-style keyboard shortcuts.

---

## Overview

### Current State

The codebase already has foundational types in `model/editor.rs`:

```rust
// Already implemented
pub struct Position { pub line: usize, pub column: usize }
pub struct Selection { pub anchor: Position, pub head: Position }
pub struct Cursor { pub line: usize, pub column: usize, pub desired_column: Option<usize> }
pub struct EditorState {
    pub cursors: Vec<Cursor>,
    pub selections: Vec<Selection>,  // Parallel to cursors
    // ...
}
```

**What's missing:**

- Selection movement (Shift+Arrow)
- Selection rendering
- Multi-cursor editing logic
- Rectangle selection
- Clipboard operations with selections
- Word/line selection

### Goals

| Feature              | Description                                            |
| -------------------- | ------------------------------------------------------ |
| **Text Selection**   | Shift+movement extends selection from anchor           |
| **Multi-Cursor**     | Multiple independent cursors editing simultaneously    |
| **Rectangle Select** | Column-mode selection via middle mouse drag            |
| **Clone Cursor**     | Add cursor above/below via Option+Option+Arrow         |
| **Clipboard**        | Cut/copy/paste respecting selections                   |
| **Word/Line Select** | Double-click word, triple-click line, Cmd+D next match |

---

## Part 1: Selection Model

### Selection Semantics

A selection has two key positions:

- **Anchor**: Where the selection started (fixed)
- **Head**: Where the cursor currently is (moves with arrow keys)

```
Forward selection (head after anchor):
    anchor          head
       │              │
       ▼              ▼
       Hello, World!
       └──────────────┘
         selected text

Backward selection (head before anchor):
       head          anchor
       │              │
       ▼              ▼
       Hello, World!
       └──────────────┘
         selected text
```

**Direction preservation** is critical:

- Shift+Right with forward selection: head moves right
- Shift+Left with forward selection: head moves left (may cross anchor)
- When head crosses anchor, selection reverses direction

### Selection State

Each cursor has a parallel selection:

```rust
// In EditorState
pub cursors: Vec<Cursor>,      // Cursor positions
pub selections: Vec<Selection>, // Parallel selections (cursors[i] pairs with selections[i])

// Invariant: cursors.len() == selections.len()
// Invariant: cursors[i].to_position() == selections[i].head
```

### Selection Operations

```rust
impl Selection {
    /// Extend selection by moving head to new position
    pub fn extend_to(&mut self, new_head: Position) {
        self.head = new_head;
    }

    /// Collapse selection to cursor position (clear selection)
    pub fn collapse_to_head(&mut self) {
        self.anchor = self.head;
    }

    /// Collapse to anchor (used when pressing left with selection)
    pub fn collapse_to_start(&mut self) {
        let start = self.start();
        self.anchor = start;
        self.head = start;
    }

    /// Collapse to end (used when pressing right with selection)
    pub fn collapse_to_end(&mut self) {
        let end = self.end();
        self.anchor = end;
        self.head = end;
    }

    /// Get selected text from document
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

    /// Check if position is within selection
    pub fn contains(&self, pos: Position) -> bool {
        pos >= self.start() && pos < self.end()
    }
}
```

---

## Part 2: Keyboard Shortcuts

### Selection Movement

| Shortcut    | Action                 | Message                                         |
| ----------- | ---------------------- | ----------------------------------------------- |
| Shift+←     | Extend selection left  | `EditorMsg::MoveCursorWithSelection(Left)`      |
| Shift+→     | Extend selection right | `EditorMsg::MoveCursorWithSelection(Right)`     |
| Shift+↑     | Extend selection up    | `EditorMsg::MoveCursorWithSelection(Up)`        |
| Shift+↓     | Extend selection down  | `EditorMsg::MoveCursorWithSelection(Down)`      |
| Shift+Home  | Extend to line start   | `EditorMsg::MoveCursorLineStartWithSelection`   |
| Shift+End   | Extend to line end     | `EditorMsg::MoveCursorLineEndWithSelection`     |
| Shift+Cmd+← | Extend word left       | `EditorMsg::MoveCursorWordWithSelection(Left)`  |
| Shift+Cmd+→ | Extend word right      | `EditorMsg::MoveCursorWordWithSelection(Right)` |
| Cmd+A       | Select all             | `EditorMsg::SelectAll`                          |
| Escape      | Clear selection        | `EditorMsg::ClearSelection`                     |

### Movement with Selection

When moving **without** Shift and a selection exists:

- **Left/Up**: Collapse to selection start, then move
- **Right/Down**: Collapse to selection end, then move

```rust
fn move_cursor_left(model: &mut AppModel, extend_selection: bool) {
    let selection = model.editor.selection_mut();

    if extend_selection {
        // Move head left, anchor stays
        let new_pos = compute_position_left(selection.head, &model.document);
        selection.head = new_pos;
        model.editor.cursor_mut().line = new_pos.line;
        model.editor.cursor_mut().column = new_pos.column;
    } else if !selection.is_empty() {
        // Collapse to start
        let start = selection.start();
        selection.collapse_to_start();
        model.editor.cursor_mut().line = start.line;
        model.editor.cursor_mut().column = start.column;
    } else {
        // Normal movement
        move_cursor_left_impl(model);
        selection.collapse_to_head();
    }
}
```

### Multi-Cursor Shortcuts

| Shortcut        | Action                                   | Message                                              |
| --------------- | ---------------------------------------- | ---------------------------------------------------- |
| Cmd+Click       | Toggle cursor at position                | `EditorMsg::ToggleCursorAtPosition { line, column }` |
| Cmd+D           | Select next occurrence of word/selection | `EditorMsg::SelectNextOccurrence`                    |
| Cmd+Shift+L     | Select all occurrences                   | `EditorMsg::SelectAllOccurrences`                    |
| Option+Option+↑ | Add cursor above                         | `EditorMsg::AddCursorAbove`                          |
| Option+Option+↓ | Add cursor below                         | `EditorMsg::AddCursorBelow`                          |
| Escape          | Collapse to single cursor                | `EditorMsg::CollapseToSingleCursor`                  |

### Word & Line Selection

| Action           | Trigger           | Behavior                                 |
| ---------------- | ----------------- | ---------------------------------------- |
| Select word      | Double-click      | Select word under cursor                 |
| Select line      | Triple-click      | Select entire line including newline     |
| Expand selection | Cmd+W (JetBrains) | Expand: word → quotes → brackets → block |

---

## Part 3: Messages

```rust
// In messages.rs

#[derive(Debug, Clone)]
pub enum EditorMsg {
    // Existing movement
    MoveCursor(Direction),
    MoveCursorLineStart,
    MoveCursorLineEnd,
    MoveCursorDocumentStart,
    MoveCursorDocumentEnd,
    MoveCursorWord(Direction),
    PageUp,
    PageDown,
    SetCursorPosition { line: usize, column: usize },
    Scroll(i32),
    ScrollHorizontal(i32),

    // === NEW: Selection Movement ===
    MoveCursorWithSelection(Direction),
    MoveCursorLineStartWithSelection,
    MoveCursorLineEndWithSelection,
    MoveCursorDocumentStartWithSelection,
    MoveCursorDocumentEndWithSelection,
    MoveCursorWordWithSelection(Direction),
    PageUpWithSelection,
    PageDownWithSelection,

    // === NEW: Selection Commands ===
    SelectAll,
    SelectWord,                          // Select word at cursor
    SelectLine,                          // Select entire line
    ExtendSelectionToPosition { line: usize, column: usize },  // Shift+Click
    ClearSelection,                      // Collapse all selections

    // === NEW: Multi-Cursor ===
    ToggleCursorAtPosition { line: usize, column: usize },  // Cmd+Click
    AddCursorAbove,                      // Option+Option+Up
    AddCursorBelow,                      // Option+Option+Down
    CollapseToSingleCursor,              // Escape with multiple cursors
    RemoveCursor(usize),                 // Remove cursor by index

    // === NEW: Occurrence Selection (JetBrains-style) ===
    AddSelectionForNextOccurrence,       // Cmd+J
    UnselectOccurrence,                  // Shift+Cmd+J

    // === NEW: Rectangle Selection ===
    StartRectangleSelection { line: usize, column: usize },
    UpdateRectangleSelection { line: usize, column: usize },
    FinishRectangleSelection,
    CancelRectangleSelection,
}
```

---

## Part 4: Multi-Cursor Editing

### Cursor Ordering

Cursors must be processed in **reverse document order** (highest offset first) to prevent edits from shifting subsequent cursor positions.

```rust
/// Get cursor indices sorted by document offset descending
fn cursors_in_edit_order(editor: &EditorState, document: &Document) -> Vec<usize> {
    let mut indices: Vec<usize> = (0..editor.cursors.len()).collect();
    indices.sort_by(|&a, &b| {
        let offset_a = document.cursor_to_offset(
            editor.cursors[a].line,
            editor.cursors[a].column
        );
        let offset_b = document.cursor_to_offset(
            editor.cursors[b].line,
            editor.cursors[b].column
        );
        offset_b.cmp(&offset_a)  // Descending
    });
    indices
}
```

### Insert Character at All Cursors

```rust
fn insert_char_multi(model: &mut AppModel, ch: char) {
    let indices = cursors_in_edit_order(&model.editor, &model.document);

    for &i in &indices {
        let cursor = &model.editor.cursors[i];
        let selection = &model.editor.selections[i];

        // If selection exists, delete it first
        if !selection.is_empty() {
            delete_selection(model, i);
        }

        // Insert character
        let pos = model.document.cursor_to_offset(cursor.line, cursor.column);
        model.document.buffer.insert_char(pos, ch);

        // Move cursor right
        model.editor.cursors[i].column += 1;
        model.editor.selections[i] = Selection::new(
            model.editor.cursors[i].to_position()
        );
    }

    // Record undo operation (combined for all cursors)
    // ...

    deduplicate_cursors(model);
}
```

### Cursor Deduplication

After operations, cursors may overlap. Remove duplicates while preserving primary cursor (index 0).

```rust
fn deduplicate_cursors(model: &mut AppModel) {
    let mut seen: HashSet<(usize, usize)> = HashSet::new();
    let mut to_remove: Vec<usize> = Vec::new();

    // Always keep primary cursor
    seen.insert((model.editor.cursors[0].line, model.editor.cursors[0].column));

    for i in 1..model.editor.cursors.len() {
        let key = (model.editor.cursors[i].line, model.editor.cursors[i].column);
        if seen.contains(&key) {
            to_remove.push(i);
        } else {
            seen.insert(key);
        }
    }

    // Remove in reverse order to preserve indices
    for i in to_remove.into_iter().rev() {
        model.editor.cursors.remove(i);
        model.editor.selections.remove(i);
    }
}
```

### Selection Merging

When selections overlap, merge them:

```rust
fn merge_overlapping_selections(editor: &mut EditorState) {
    if editor.selections.len() <= 1 {
        return;
    }

    // Sort by start position
    let mut indexed: Vec<(usize, Selection)> = editor.selections
        .iter()
        .cloned()
        .enumerate()
        .collect();
    indexed.sort_by_key(|(_, s)| s.start());

    let mut merged: Vec<(usize, Selection)> = Vec::new();

    for (i, sel) in indexed {
        if let Some((_, last)) = merged.last_mut() {
            if sel.start() <= last.end() {
                // Overlapping - extend last selection
                if sel.end() > last.end() {
                    last.head = sel.end();  // Extend to cover both
                }
                // Remove the cursor for this merged selection
                // (handled separately)
                continue;
            }
        }
        merged.push((i, sel));
    }

    // Rebuild cursors and selections from merged set
    // Keep only cursors whose selections weren't merged away
    // ...
}
```

---

## Part 5: Rectangle Selection

### State

```rust
// In EditorState
pub struct EditorState {
    // ... existing fields

    /// Rectangle selection state (None when not in rectangle mode)
    pub rectangle_selection: Option<RectangleSelectionState>,
}

#[derive(Debug, Clone, Copy)]
pub struct RectangleSelectionState {
    /// Starting corner of rectangle
    pub anchor: Position,
    /// Current corner (follows mouse)
    pub current: Position,
}
```

### Algorithm

```rust
fn update_rectangle_selection(model: &mut AppModel, current: Position) {
    let Some(rect_state) = &model.editor.rectangle_selection else {
        return;
    };

    let anchor = rect_state.anchor;

    // Compute rectangle bounds
    let start_line = anchor.line.min(current.line);
    let end_line = anchor.line.max(current.line);
    let start_col = anchor.column.min(current.column);
    let end_col = anchor.column.max(current.column);

    // Clear existing cursors/selections
    model.editor.cursors.clear();
    model.editor.selections.clear();

    // Create cursor/selection on each line within bounds
    for line in start_line..=end_line {
        let line_len = model.document.line_length(line);

        // Clamp columns to line length
        let clamped_start = start_col.min(line_len);
        let clamped_end = end_col.min(line_len);

        // Cursor at the "current" side of rectangle
        let cursor_col = if current.column > anchor.column {
            clamped_end
        } else {
            clamped_start
        };

        model.editor.cursors.push(Cursor::at(line, cursor_col));

        // Selection covers the rectangle width (or empty if zero-width)
        if clamped_start != clamped_end {
            model.editor.selections.push(Selection::from_anchor_head(
                Position::new(line, clamped_start),
                Position::new(line, clamped_end),
            ));
        } else {
            // Zero-width: empty selection (just cursor)
            model.editor.selections.push(Selection::new(
                Position::new(line, cursor_col)
            ));
        }
    }

    // Update the rectangle state
    model.editor.rectangle_selection = Some(RectangleSelectionState {
        anchor,
        current,
    });
}
```

### Visual Feedback

During rectangle selection, render:

1. Semi-transparent rectangle overlay showing bounds
2. Cursors on each line (even if clamped)
3. Selection backgrounds if width > 0

```rust
fn render_rectangle_selection_overlay(
    buffer: &mut [u32],
    rect_state: &RectangleSelectionState,
    model: &AppModel,
    // ... rendering params
) {
    let anchor = rect_state.anchor;
    let current = rect_state.current;

    // Convert to screen coordinates
    let x1 = column_to_pixel(anchor.column.min(current.column));
    let x2 = column_to_pixel(anchor.column.max(current.column));
    let y1 = line_to_pixel(anchor.line.min(current.line));
    let y2 = line_to_pixel(anchor.line.max(current.line) + 1);  // +1 for bottom of last line

    // Draw semi-transparent rectangle border
    let border_color = model.theme.editor.selection_border.to_argb_u32();
    draw_rect_outline(buffer, x1, y1, x2, y2, border_color);
}
```

---

## Part 6: Double-Tap Option Detection

### State

```rust
// In App (event loop state, not EditorState)
struct DoubleTapDetector {
    last_key_down: Option<(Key, Instant)>,
    is_held: bool,
}

impl DoubleTapDetector {
    const DOUBLE_TAP_WINDOW: Duration = Duration::from_millis(300);

    fn on_key_down(&mut self, key: Key) -> bool {
        let now = Instant::now();

        if let Some((last_key, last_time)) = &self.last_key_down {
            if *last_key == key && now.duration_since(*last_time) < Self::DOUBLE_TAP_WINDOW {
                // Double-tap detected!
                self.is_held = true;
                self.last_key_down = None;
                return true;
            }
        }

        self.last_key_down = Some((key, now));
        false
    }

    fn on_key_up(&mut self, key: Key) {
        if self.is_held {
            self.is_held = false;
        }
        if let Some((last_key, _)) = &self.last_key_down {
            if *last_key == key {
                // Key released before double-tap
                // Keep last_key_down for potential second tap
            }
        }
    }

    fn is_in_add_cursor_mode(&self) -> bool {
        self.is_held
    }
}
```

### Integration

```rust
// In handle_key
if self.option_detector.is_in_add_cursor_mode() {
    match key {
        Key::Named(NamedKey::ArrowUp) => {
            return Some(Msg::Editor(EditorMsg::AddCursorAbove));
        }
        Key::Named(NamedKey::ArrowDown) => {
            return Some(Msg::Editor(EditorMsg::AddCursorBelow));
        }
        _ => {}
    }
}
```

---

## Part 7: Clipboard Operations

### Copy

```rust
fn copy_selections(model: &AppModel) -> String {
    if model.editor.cursors.len() == 1 {
        // Single cursor: copy selection or current line
        let selection = model.editor.selection();
        if selection.is_empty() {
            // Copy entire line
            model.document.get_line(model.editor.cursor().line)
                .unwrap_or_default()
        } else {
            selection.get_text(&model.document)
        }
    } else {
        // Multi-cursor: join all selections with newlines
        model.editor.selections.iter()
            .map(|s| s.get_text(&model.document))
            .collect::<Vec<_>>()
            .join("\n")
    }
}
```

### Paste

```rust
fn paste_at_cursors(model: &mut AppModel, text: &str) {
    let lines: Vec<&str> = text.lines().collect();
    let indices = cursors_in_edit_order(&model.editor, &model.document);

    if lines.len() == model.editor.cursors.len() {
        // Multi-line paste to multi-cursor: one line per cursor
        for (i, &cursor_idx) in indices.iter().enumerate() {
            let line_to_paste = lines.get(i).unwrap_or(&"");
            insert_text_at_cursor(model, cursor_idx, line_to_paste);
        }
    } else {
        // Paste same text at all cursors
        for &cursor_idx in &indices {
            insert_text_at_cursor(model, cursor_idx, text);
        }
    }

    deduplicate_cursors(model);
}
```

### Cut

```rust
fn cut_selections(model: &mut AppModel) -> String {
    let copied = copy_selections(model);

    // Delete all selections (in reverse order)
    let indices = cursors_in_edit_order(&model.editor, &model.document);
    for &i in &indices {
        if !model.editor.selections[i].is_empty() {
            delete_selection(model, i);
        } else {
            // No selection: cut entire line
            delete_line(model, i);
        }
    }

    deduplicate_cursors(model);
    copied
}
```

---

## Part 8: Theme Integration

```rust
// In theme.rs

pub struct EditorTheme {
    pub background: Color,
    pub foreground: Color,
    pub current_line_background: Color,
    pub cursor_color: Color,

    // NEW: Selection colors
    pub selection_background: Color,
    pub selection_background_inactive: Color,  // Unfocused window
    pub selection_border: Option<Color>,       // Optional border

    // NEW: Multi-cursor indicators
    pub secondary_cursor_color: Color,         // Non-primary cursors
}
```

### Default Colors

```rust
// In Theme::default_dark()
EditorTheme {
    // ... existing
    selection_background: Color::rgb(0x26, 0x4F, 0x78),        // #264F78 (VSCode blue)
    selection_background_inactive: Color::rgb(0x1D, 0x3A, 0x5C), // Dimmer
    selection_border: None,
    secondary_cursor_color: Color::rgba(0xFF, 0xFF, 0xFF, 0x80), // Semi-transparent
}
```

---

## Part 9: Rendering

### Selection Rendering Order

```
1. Clear background
2. Draw current line highlight(s) - one per cursor
3. Draw selections (background color)
4. Draw text
5. Draw cursors (all of them)
6. Draw rectangle selection overlay (if active)
```

### Selection Rendering

```rust
fn render_selections(
    buffer: &mut [u32],
    model: &AppModel,
    viewport: &Viewport,
    line_height: usize,
    char_width: f32,
    text_start_x: usize,
) {
    let selection_bg = model.theme.editor.selection_background.to_argb_u32();

    for selection in &model.editor.selections {
        if selection.is_empty() {
            continue;
        }

        let start = selection.start();
        let end = selection.end();

        // For each line in selection
        for line in start.line..=end.line {
            // Skip if line not in viewport
            if line < viewport.top_line || line >= viewport.top_line + viewport.visible_lines {
                continue;
            }

            let screen_line = line - viewport.top_line;
            let y = screen_line * line_height;

            // Calculate selection bounds on this line
            let line_len = model.document.line_length(line);

            let sel_start_col = if line == start.line { start.column } else { 0 };
            let sel_end_col = if line == end.line { end.column } else { line_len };

            // Adjust for horizontal scroll
            let visible_start = sel_start_col.saturating_sub(viewport.left_column);
            let visible_end = sel_end_col.saturating_sub(viewport.left_column);

            if visible_end <= visible_start {
                continue;
            }

            let x1 = text_start_x + (visible_start as f32 * char_width) as usize;
            let x2 = text_start_x + (visible_end as f32 * char_width) as usize;

            // Fill selection rectangle
            fill_rect(buffer, x1, y, x2 - x1, line_height, selection_bg);
        }
    }
}
```

### Multi-Cursor Rendering

```rust
fn render_cursors(
    buffer: &mut [u32],
    model: &AppModel,
    viewport: &Viewport,
    line_height: usize,
    char_width: f32,
    text_start_x: usize,
) {
    if !model.ui.cursor_visible {
        return;  // Blink off
    }

    for (i, cursor) in model.editor.cursors.iter().enumerate() {
        // Skip if not in viewport
        if cursor.line < viewport.top_line ||
           cursor.line >= viewport.top_line + viewport.visible_lines {
            continue;
        }

        let screen_line = cursor.line - viewport.top_line;
        let screen_col = cursor.column.saturating_sub(viewport.left_column);

        let x = text_start_x + (screen_col as f32 * char_width) as usize;
        let y = screen_line * line_height;

        // Primary cursor uses main color, secondary uses dimmer color
        let cursor_color = if i == 0 {
            model.theme.editor.cursor_color.to_argb_u32()
        } else {
            model.theme.editor.secondary_cursor_color.to_argb_u32()
        };

        // Draw pipe cursor (2px wide)
        fill_rect(buffer, x, y, 2, line_height, cursor_color);
    }
}
```

---

## Part 10: Undo/Redo with Multi-Cursor

### Enhanced EditOperation

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
    // NEW: Multi-cursor batch operation
    Batch {
        operations: Vec<EditOperation>,
        cursors_before: Vec<Cursor>,
        selections_before: Vec<Selection>,
        cursors_after: Vec<Cursor>,
        selections_after: Vec<Selection>,
    },
}
```

### Recording Batch Operations

```rust
fn record_multi_cursor_edit(
    model: &mut AppModel,
    operations: Vec<EditOperation>,
    cursors_before: Vec<Cursor>,
    selections_before: Vec<Selection>,
) {
    let cursors_after = model.editor.cursors.clone();
    let selections_after = model.editor.selections.clone();

    model.document.push_edit(EditOperation::Batch {
        operations,
        cursors_before,
        selections_before,
        cursors_after,
        selections_after,
    });
}
```

### Undo Batch

```rust
fn undo_batch(model: &mut AppModel, batch: &EditOperation::Batch) {
    // Undo individual operations in reverse order
    for op in batch.operations.iter().rev() {
        match op {
            EditOperation::Insert { position, text, .. } => {
                model.document.buffer.remove(*position..*position + text.chars().count());
            }
            EditOperation::Delete { position, text, .. } => {
                model.document.buffer.insert(*position, text);
            }
            _ => {}
        }
    }

    // Restore cursors and selections
    model.editor.cursors = batch.cursors_before.clone();
    model.editor.selections = batch.selections_before.clone();
}
```

---

## Implementation Plan

### Phase 1: Basic Selection (Foundation)

- [ ] Add `selection_background` to theme
- [ ] Implement `MoveCursorWithSelection(Direction)` messages
- [ ] Update `handle_key()` for Shift+Arrow detection
- [ ] Render selections before text
- [ ] Handle Shift+Click for `ExtendSelectionToPosition`
- [ ] Implement selection collapse on non-shift movement
- [ ] Escape clears selection

**Test:** Shift+Arrow creates visible selection, movement without shift collapses it.

### Phase 2: Selection Editing

- [ ] Delete selection on Backspace/Delete
- [ ] Replace selection when typing
- [ ] Update `InsertChar` to delete selection first
- [ ] Update `InsertNewline` to delete selection first
- [ ] Handle selection with word delete (Option+Backspace)

**Test:** Type with selection replaces text.

### Phase 3: Word & Line Selection

- [ ] Implement `SelectWord` (double-click or Cmd+W)
- [ ] Implement `SelectLine` (triple-click)
- [ ] Implement `SelectAll` (Cmd+A)
- [ ] Add word boundary detection for selection

**Test:** Double-click selects word, triple-click selects line.

### Phase 4: Multi-Cursor Basics

- [ ] Implement `ToggleCursorAtPosition` for Cmd+Click
- [ ] Render all cursors (with secondary color)
- [ ] Highlight all cursor lines
- [ ] Implement `CollapseToSingleCursor` on Escape
- [ ] Add cursor deduplication

**Test:** Cmd+Click adds/removes cursors, Escape collapses.

### Phase 5: Multi-Cursor Editing

- [ ] Implement reverse-order editing for all cursors
- [ ] Handle offset adjustments for subsequent cursors
- [ ] Implement selection merging
- [ ] Update all edit operations (insert, delete, newline)
- [ ] Add batch undo/redo support

**Test:** Type with multiple cursors, all insert correctly.

### Phase 6: Clipboard

- [ ] Implement `copy_selections()`
- [ ] Implement `paste_at_cursors()` with line matching
- [ ] Implement `cut_selections()`
- [ ] Handle empty selection copy (copy line)

**Test:** Copy/paste works with multiple cursors.

### Phase 7: Rectangle Selection

- [ ] Add `RectangleSelectionState` to EditorState
- [ ] Handle middle mouse down/drag/up
- [ ] Implement `update_rectangle_selection()` algorithm
- [ ] Render rectangle overlay during drag
- [ ] Handle line clamping for short lines

**Test:** Middle-drag creates cursors on each line.

### Phase 8: Double-Tap Option

- [ ] Add `DoubleTapDetector` to App
- [ ] Track Option key press/release
- [ ] Implement `AddCursorAbove` / `AddCursorBelow`
- [ ] Exit add-cursor mode on other keys

**Test:** Double-tap Option, hold, Up/Down adds cursors.

### Phase 9: Occurrence Selection (JetBrains-style)

Select occurrences of the current word or selection to create multiple cursors.

#### Shortcuts

| Shortcut    | Action              | Message                                    |
| ----------- | ------------------- | ------------------------------------------ |
| Cmd+J       | Add next occurrence | `EditorMsg::AddSelectionForNextOccurrence` |
| Shift+Cmd+J | Remove last added   | `EditorMsg::UnselectOccurrence`            |

#### Tasks

- [ ] Implement `AddSelectionForNextOccurrence` (Cmd+J)
- [ ] Implement `UnselectOccurrence` (Shift+Cmd+J)
- [ ] Add word-under-cursor detection (reuse `char_type()`)
- [ ] Track occurrence history for unselect (stack of added cursors)

#### Behavior

**AddSelectionForNextOccurrence (Cmd+J):**

1. If no selection: select word under cursor, then find next occurrence
2. If selection exists: find next occurrence of selected text
3. Create new cursor+selection at the match
4. Search wraps around to beginning of document
5. If no more matches found, do nothing (or show status message)

**UnselectOccurrence (Shift+Cmd+J):**

1. Remove the most recently added cursor/selection
2. Keep at least one cursor (primary)
3. Restores to previous state before last Cmd+J

#### Example Workflow

```
Text: "each person, each animal, each animator"

1. Cursor on first "each" → Cmd+J → selects "each"
2. Cmd+J again → adds cursor at second "each" (2 cursors)
3. Cmd+J again → adds cursor at third "each" (3 cursors)
4. Shift+Cmd+J → removes third cursor (back to 2)
5. Type "every" → replaces all selected "each" with "every"
```

**Test:** Cmd+J adds occurrences, Shift+Cmd+J removes last added.

---

## Files to Modify

| File                    | Changes                                                                |
| ----------------------- | ---------------------------------------------------------------------- |
| `src/theme.rs`          | Add selection colors to EditorTheme                                    |
| `src/messages.rs`       | Add ~20 new EditorMsg variants                                         |
| `src/model/editor.rs`   | Add RectangleSelectionState, selection helpers                         |
| `src/model/document.rs` | Add EditOperation::Batch variant                                       |
| `src/update.rs`         | Selection movement, multi-cursor editing logic                         |
| `src/main.rs`           | Selection/cursor rendering, mouse/keyboard handling, DoubleTapDetector |

---

## Edge Cases

| Case                         | Handling                               |
| ---------------------------- | -------------------------------------- |
| Overlapping cursors          | Deduplicate after operations           |
| Overlapping selections       | Merge into single selection            |
| Selection past EOF           | Clamp to document end                  |
| Empty document               | Allow cursor at (0,0), empty selection |
| Rectangle on short lines     | Clamp column to line length            |
| Undo with multi-cursor       | Restore all cursors/selections         |
| Primary cursor deleted       | Promote next cursor to primary         |
| All cursors on same position | Keep only one                          |

---

## Success Criteria

- [ ] Shift+Arrow creates/extends visible selection
- [ ] Shift+Click extends selection to position
- [ ] Double-click selects word, triple-click selects line
- [ ] Cmd+A selects all
- [ ] Typing with selection replaces selected text
- [ ] Cmd+Click toggles cursor at position
- [ ] Middle mouse drag creates rectangle selection
- [ ] Double-tap Option + Arrow adds cursors
- [ ] Typing with multiple cursors inserts at all positions
- [ ] Cut/Copy/Paste works with selections and multi-cursor
- [ ] Cmd+J adds next occurrence to selection
- [ ] Shift+Cmd+J removes last added occurrence
- [ ] Undo/Redo restores all cursors and selections
- [ ] Escape clears selection / collapses to single cursor
- [ ] All cursors render with appropriate colors
- [ ] All selections render with background color
