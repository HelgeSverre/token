# Plan: Refactors #4 and #6 — column_to_pixel helper & color palette

**Status:** Proposed
**Created:** 2026-03-10

## Context

Two small, self-contained refactors from `docs/POTENTIAL-REFACTORS.md` that are independent of the larger view layer redesign. Both reduce repetition in the rendering hot path without changing architecture.

---

## Refactor #4: Extract `visual_col_to_pixel_x()` helper

### Problem

The column-to-pixel calculation appears 11 times in `src/view/editor.rs` with three minor variants:

- **Range variant** (4x): two calls for start/end of selection/rectangle
- **Single column variant** (2x): bracket matching
- **Cursor variant** (3x): casts `base_x` to f32 first

Plus 2 token-adjustment sites that do the `saturating_sub` but not the pixel math.

### Implementation

Add to `src/view/geometry.rs`:

```rust
/// Convert a visual column to a pixel x-coordinate.
#[inline]
pub fn visual_col_to_pixel_x(
    visual_col: usize,
    viewport_left_column: usize,
    text_start_x: usize,
    char_width: f32,
) -> usize {
    let visible_col = visual_col.saturating_sub(viewport_left_column);
    text_start_x + (visible_col as f32 * char_width).round() as usize
}
```

Then replace each occurrence in `src/view/editor.rs`:
- Lines 150-152, 192-194, 410-412, 466-468 (range selections)
- Lines 221, 495 (bracket match)
- Lines 305-306, 589-590, 636-637 (cursors — note these cast `group_text_start_x` to f32 first; the helper's integer add-then-round produces the same result since `text_start_x` is already pixel-aligned)

### Files to modify

- `src/view/geometry.rs` — add function
- `src/view/editor.rs` — replace ~11 inline calculations

---

## Refactor #6: Centralize editor color extraction

### Problem

`to_argb_u32()` is called 90 times across 6 view files. The worst duplication is in `editor.rs` where `render_cursor_lines_only()` (lines 62-69) and `render_text_area()` (lines 358-369) extract the exact same ~10 colors at their top.

### Design decision

Don't create one massive global palette. Instead, create focused palette structs for domains with actual duplication:

1. **`EditorColors`** — covers `editor.rs` (eliminates ~15 duplicate extractions between the two main functions)
2. Leave `modal.rs`, `panels.rs`, `button.rs`, `mod.rs` as-is — they already extract colors at function top or use context structs, and their colors are domain-specific with little cross-file sharing.

### Implementation

Add to `src/view/editor.rs` (file-local, not public):

```rust
struct EditorColors {
    bg: u32,
    fg: u32,
    current_line_bg: u32,
    selection_bg: u32,
    cursor: u32,
    secondary_cursor: u32,
    bracket_match_bg: u32,
    gutter_bg: u32,
    gutter_fg: u32,
    gutter_fg_active: u32,
    gutter_border: u32,
}

impl EditorColors {
    fn from_theme(theme: &Theme) -> Self {
        Self {
            bg: theme.editor.background.to_argb_u32(),
            fg: theme.editor.foreground.to_argb_u32(),
            current_line_bg: theme.editor.current_line_background.to_argb_u32(),
            selection_bg: theme.editor.selection_background.to_argb_u32(),
            cursor: theme.editor.cursor_color.to_argb_u32(),
            secondary_cursor: theme.editor.secondary_cursor_color.to_argb_u32(),
            bracket_match_bg: theme.editor.bracket_match_background.to_argb_u32(),
            gutter_bg: theme.gutter.background.to_argb_u32(),
            gutter_fg: theme.gutter.foreground.to_argb_u32(),
            gutter_fg_active: theme.gutter.foreground_active.to_argb_u32(),
            gutter_border: theme.gutter.border_color.to_argb_u32(),
        }
    }
}
```

Then in `render_cursor_lines_only()` and `render_text_area()`, replace the individual `let bg_color = ...` lines with `let colors = EditorColors::from_theme(&model.theme);` and use `colors.bg`, `colors.fg`, etc. throughout.

`render_gutter()` and `render_scrollbars()` can also take `&EditorColors` instead of extracting their own.

### Files to modify

- `src/view/editor.rs` — add struct, refactor 4 functions

---

## Verification

```bash
make test          # all tests pass
make lint          # no new clippy warnings
make build         # compiles clean
```

Grep to confirm reduction:
```bash
# Should show fewer .to_argb_u32() calls in editor.rs (from ~30 to ~5)
grep -c 'to_argb_u32' src/view/editor.rs
```
