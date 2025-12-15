# Multi-Selection Behaviours

This document describes the expected behavior for rectangle (block) selection, based on JetBrains IntelliJ IDEA's implementation.

## Rectangle Selection Overview

Rectangle selection (triggered by middle-click drag) creates multiple cursors and selections spanning a rectangular region. The key insight is that **cursors** and **selection highlights** behave differently:

| Component | Behavior |
|-----------|----------|
| **Preview cursors** | Appear at the dragged-to column on each line (can extend into "virtual space" past line end) |
| **Selection highlight** | Only covers **actual text** - clamped to each line's length |

## Visual Example

Given these lines with different lengths:

```
Line 0: "Hello World"     (11 chars)
Line 1: "Hi"              (2 chars)  
Line 2: "Good morning"    (12 chars)
Line 3: "Bye"             (3 chars)
```

User drags rectangle from (line 0, col 3) to (line 3, col 8):

### Expected Preview During Drag

```
Line 0: "Hel[lo Wo]rld"   ← highlight cols 3-8, cursor at col 8
Line 1: "Hi[_]|"          ← highlight cols 2-2 (empty!), cursor at col 8 (virtual)
Line 2: "Goo[d mor]ning"  ← highlight cols 3-8, cursor at col 8
Line 3: "Bye[_]|"         ← highlight cols 3-3 (empty!), cursor at col 8 (virtual)
```

Where:
- `[...]` = selection highlight (actual text only)
- `|` = preview cursor position
- Lines 1 and 3 show cursor beyond line end but NO highlight (both boundaries past line end)

### After Mouse Release (FinishRectangleSelection)

Cursors are **clamped to line length**:

```
Line 0: cursor at col 8, selection [3, 8)
Line 1: cursor at col 2, no selection (zero width)
Line 2: cursor at col 8, selection [3, 8)
Line 3: cursor at col 3, no selection (zero width)
```

## Detailed Rules

### 1. Preview Cursor Positioning

During drag, preview cursors are placed at `rectangle_selection.current.column` on each line, regardless of line length. This creates a visually consistent vertical "cursor line" during drag.

**Rationale**: Users expect to see where their cursors will land. Showing virtual-space cursors provides clear feedback.

### 2. Selection Highlight Rules (Per Line)

For each line in the rectangle:

```rust
let line_len = document.line_length(line);
let start_col = top_left.column.min(line_len);
let end_col = bottom_right.column.min(line_len);

if start_col < end_col {
    // Draw highlight from start_col to end_col
} else {
    // No highlight for this line (cursor only)
}
```

**Cases:**

| Condition | Result |
|-----------|--------|
| Both columns within line | Highlight `[start, end)` |
| Start within, end past line | Highlight `[start, line_len)` |
| Both columns past line end | **No highlight** (cursor only) |
| Start past line end | **No highlight** (cursor only) |

### 3. Final Cursor Placement (FinishRectangleSelection)

On mouse release, cursors are **clamped to line length**:

```rust
let clamped_cursor_col = cursor_col.min(line_len);
```

This matches current behavior in `editor.rs:864`.

### 4. Zero-Width Rectangles

When `start.column == current.column`:
- No selection highlights shown
- Preview cursors appear on each line
- After release: cursors placed, empty selections

## Implementation Changes Required

### Current State (Broken)

In `src/view/mod.rs`, the rectangle highlight uses `top_left.column` and `bottom_right.column` directly without per-line clamping, causing highlights to extend past line ends.

### Required Fix

Modify the rectangle selection highlight rendering to clamp columns per-line:

```rust
// Rectangle selection highlight (middle mouse drag preview)
if editor.rectangle_selection.active {
    let rect_sel = &editor.rectangle_selection;
    let top_left = rect_sel.top_left();
    let bottom_right = rect_sel.bottom_right();

    let visible_start = top_left.line.max(editor.viewport.top_line);
    let visible_end = (bottom_right.line + 1).min(end_line);

    for doc_line in visible_start..visible_end {
        let line_len = document.line_length(doc_line);
        
        // CLAMP columns to actual line length
        let start_col = top_left.column.min(line_len);
        let end_col = bottom_right.column.min(line_len);
        
        // Skip lines where both columns are at/past line end
        if start_col >= end_col {
            continue;
        }

        // ... rest of rendering (use start_col/end_col instead of top_left.column/bottom_right.column)
    }
}
```

### Preview Cursors (Already Correct)

Current implementation places preview cursors at `current.column` without clamping, which is correct for the preview phase.

## Comparison: JetBrains vs Our Implementation

| Aspect | JetBrains | Our Implementation |
|--------|-----------|-------------------|
| Preview cursor position | Virtual space (past line end) | ✅ Same |
| Selection highlight | Clamped per-line | ❌ **Needs fix** |
| Final cursor position | Clamped to line length | ✅ Same |
| Column mode toggle | Yes (Ctrl modifier changes behavior) | Not implemented |

## Future: Column Mode

JetBrains has a "Column Mode" toggle that changes selection behavior:

- **Normal mode** (current): Selections clamped to actual text
- **Column mode**: Selections extend into virtual space uniformly

This could be a future enhancement but is not required for basic rectangle selection.

## Testing Checklist

- [ ] Drag rectangle where all lines are longer than selection → highlights on all lines
- [ ] Drag rectangle past short lines → short lines show cursor only, no highlight
- [ ] Drag to same column (zero width) → cursors only, no highlights
- [ ] Release mouse → cursors clamped to line lengths
- [ ] Scroll during drag → rendering stays correct
- [ ] Cancel selection (Escape) → all previews disappear
