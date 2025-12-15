# Rectangle Selection Preview Rendering

## Problem

Middle-click rectangle selection (block selection) does not render properly during drag:
1. The selection rectangle is not drawn
2. The preview cursors are not shown

The user cannot see where their selection will be until they release the mouse button.

## Root Cause Analysis

### State is Properly Computed

In `src/update/editor.rs:818-841`, `UpdateRectangleSelection` correctly computes:
- `rectangle_selection.current` - updated on each mouse move
- `rectangle_selection.preview_cursors` - a Vec<Position> with one position per line in the rectangle

```rust
EditorMsg::UpdateRectangleSelection { line, column } => {
    if model.editor().rectangle_selection.active {
        model.editor_mut().rectangle_selection.current = Position::new(line, column);

        // Compute preview cursor positions
        let top_left = model.editor().rectangle_selection.top_left();
        let bottom_right = model.editor().rectangle_selection.bottom_right();
        let cursor_col = model.editor().rectangle_selection.current.column;

        model.editor_mut().rectangle_selection.preview_cursors.clear();
        for preview_line in top_left.line..=bottom_right.line {
            model.editor_mut().rectangle_selection.preview_cursors
                .push(Position::new(preview_line, cursor_col));
        }
    }
    Some(Cmd::Redraw)
}
```

### Rendering is Missing

In `src/view/mod.rs:render_text_area()` (lines 378-540):
- **Only `editor.cursors` is rendered** (lines 507-538)
- **`editor.rectangle_selection.preview_cursors` is never rendered**
- **The rectangle outline/fill is never drawn**

## Fix Required

Add two rendering sections in `render_text_area()` after line 469 (after selection highlights):

### 1. Rectangle Selection Highlight

When `editor.rectangle_selection.active` is true, draw a filled rectangle showing the selection area using the existing `selection_color` (from `model.theme.editor.selection_background`).

```rust
// Rectangle selection highlight (middle mouse drag)
if editor.rectangle_selection.active {
    let rect_sel = &editor.rectangle_selection;
    let top_left = rect_sel.top_left();
    let bottom_right = rect_sel.bottom_right();

    // Only render lines that are visible
    let visible_start = top_left.line.max(editor.viewport.top_line);
    let visible_end = (bottom_right.line + 1).min(end_line);

    for doc_line in visible_start..visible_end {
        let screen_line = doc_line - editor.viewport.top_line;
        let y_start = content_y + screen_line * line_height;
        let y_end = (y_start + line_height).min(content_y + content_h);

        // Get line text for tab expansion
        let line_text = document.get_line(doc_line).unwrap_or_default();
        let line_text_trimmed = if line_text.ends_with('\n') {
            &line_text[..line_text.len() - 1]
        } else {
            &line_text
        };

        let visual_start_col = char_col_to_visual_col(line_text_trimmed, top_left.column);
        let visual_end_col = char_col_to_visual_col(line_text_trimmed, bottom_right.column);

        let visible_start_col = visual_start_col.saturating_sub(editor.viewport.left_column);
        let visible_end_col = visual_end_col.saturating_sub(editor.viewport.left_column);

        let x_start = group_text_start_x + (visible_start_col as f32 * char_width).round() as usize;
        let x_end = (group_text_start_x + (visible_end_col as f32 * char_width).round() as usize)
            .min(rect_x + rect_w);

        if x_end > x_start {
            frame.fill_rect_px(
                x_start,
                y_start,
                x_end.saturating_sub(x_start),
                y_end.saturating_sub(y_start),
                selection_color,
            );
        }
    }
}
```

### 2. Preview Cursor Rendering

Add to the cursor rendering section (after line 538) to also render `preview_cursors` when rectangle selection is active:

```rust
// Preview cursors for rectangle selection (always visible during drag, no blink)
if is_focused && editor.rectangle_selection.active {
    let secondary_cursor_color = model.theme.editor.secondary_cursor_color.to_argb_u32();
    let actual_visible_columns =
        ((rect_w as f32 - text_start_x_offset as f32) / char_width).floor() as usize;

    for preview_pos in &editor.rectangle_selection.preview_cursors {
        let cursor_in_vertical_view = preview_pos.line >= editor.viewport.top_line
            && preview_pos.line < editor.viewport.top_line + visible_lines;

        let line_text = document.get_line(preview_pos.line).unwrap_or_default();
        let line_text_trimmed = if line_text.ends_with('\n') {
            &line_text[..line_text.len() - 1]
        } else {
            &line_text
        };
        let visual_cursor_col = char_col_to_visual_col(line_text_trimmed, preview_pos.column);

        let cursor_in_horizontal_view = visual_cursor_col >= editor.viewport.left_column
            && visual_cursor_col < editor.viewport.left_column + actual_visible_columns;

        if cursor_in_vertical_view && cursor_in_horizontal_view {
            let screen_line = preview_pos.line - editor.viewport.top_line;
            let cursor_visual_column = visual_cursor_col - editor.viewport.left_column;
            let x = (group_text_start_x as f32 + cursor_visual_column as f32 * char_width)
                .round() as usize;
            let y = content_y + screen_line * line_height;

            // Preview cursors: use secondary color, 2px wide
            frame.fill_rect_px(x, y + 1, 2, line_height.saturating_sub(2), secondary_cursor_color);
        }
    }
}
```

## Files to Modify

| File | Change |
|------|--------|
| `src/view/mod.rs` | Add rectangle selection rendering after line 469 |
| `src/view/mod.rs` | Add preview cursor rendering after line 538 |

## Design Decisions

1. **Reuse existing colors**: Use `selection_background` for the rectangle fill and `secondary_cursor_color` for preview cursors - no new theme fields needed

2. **Preview cursors don't blink**: They should always be visible during drag (unlike normal cursors which follow `model.ui.cursor_visible`)

3. **Tab-aware rendering**: Use `char_col_to_visual_col()` for proper tab expansion, matching existing selection/cursor rendering

4. **Viewport clipping**: Only render visible portions, consistent with existing rendering

## Testing

Manual test procedure:
1. Open any file with multiple lines
2. Middle-click and drag to create a rectangle selection
3. Verify: selection rectangle is highlighted as you drag
4. Verify: preview cursors appear on each line within the rectangle
5. Release mouse button - cursors should appear exactly where previewed
