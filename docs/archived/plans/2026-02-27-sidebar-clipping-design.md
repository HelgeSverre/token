# Sidebar Clipping and Overflow Fix

**Date:** 2026-02-27

## Problem

Long filenames in the file sidebar can render outside the sidebar area, overflowing into the editor. Three compounding issues:

1. **No clipping region support** — `Frame` only clips against full window dimensions. The sidebar renders after the editor (line 3116 of `view/mod.rs`), so overflowing sidebar text overwrites editor content.
2. **Hardcoded char width estimate** — Truncation logic uses `estimated_char_width = 8` instead of the actual font metric from `painter.char_width()`.
3. **Byte-based truncation** — `node.name.len()` counts bytes, not chars. Slicing at byte offsets can panic or mis-truncate multibyte UTF-8 names.

## Design

### Fix 1: Clip rectangle support on Frame

Add an optional clip rect to `Frame` that constrains all pixel writes to a sub-region of the buffer.

```rust
pub struct Frame<'a> {
    buffer: &'a mut [u32],
    width: usize,
    height: usize,
    clip: Option<ClipRect>,
}

#[derive(Clone, Copy)]
struct ClipRect {
    x0: usize, y0: usize,
    x1: usize, y1: usize,
}
```

- `Frame::with_clip(rect: Rect) -> Frame` creates a frame with active clip bounds.
- All drawing methods (`fill_rect`, `fill_rect_blended`, `set_pixel`, `blend_pixel`) clamp coordinates to the clip rect when present, falling back to window bounds when absent.
- `TextPainter::draw()` pixel writes also respect the clip rect via the Frame methods.
- The clip rect is applied in `render_sidebar()` before any content is drawn, scoped to `(0, 0, sidebar_width, sidebar_height)`.

### Fix 2: Use actual char width

Replace the hardcoded estimate with the real font metric:

```rust
let char_width = painter.char_width() as usize;
```

### Fix 3: Char-safe truncation

Replace byte-length check with char count and use char-boundary-safe slicing:

```rust
let name_chars: usize = node.name.chars().count();
let needs_truncation = name_chars > max_chars && max_chars > 3;
// truncate using char_indices for safe boundary
```

## Scope

- **In scope:** Fix 1 + 2 + 3 as described above.
- **Out of scope:** Horizontal scrolling for sidebar (can be added later). The clip rect + proper truncation handles the overflow. Horizontal scroll is a UX enhancement, not a correctness fix.

## Files Modified

- `src/view/frame.rs` — Add `ClipRect`, `with_clip()`, update drawing methods
- `src/view/mod.rs` — Apply clip in `render_sidebar()`, fix truncation logic
