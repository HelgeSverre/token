# Sidebar Clipping Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Prevent sidebar content from overflowing into the editor area by adding clip rectangle support to Frame and fixing truncation bugs.

**Architecture:** Add an optional ClipRect to Frame that constrains all pixel writes to a sub-region. Update TextPainter to route pixel writes through Frame methods that respect clipping. Fix the sidebar's truncation logic to use real font metrics and char-safe slicing.

**Tech Stack:** Rust, fontdue (font rasterization), softbuffer (pixel buffer)

---

### Task 1: Add ClipRect struct and clip field to Frame

**Files:**
- Modify: `src/view/frame.rs:34-65` (Frame struct and constructors)
- Test: `src/view/frame.rs` (inline #[cfg(test)] module)

**Step 1: Write failing tests for clip rect construction**

Add to the existing `mod tests` block at the bottom of `src/view/frame.rs`:

```rust
#[test]
fn test_frame_with_clip_restricts_fill_rect() {
    let mut buffer = vec![0u32; 100 * 100];
    let mut frame = Frame::new(&mut buffer, 100, 100);
    frame.set_clip(Rect { x: 10.0, y: 10.0, width: 30.0, height: 30.0 });

    // Fill the entire frame — should be clipped to 10..40 x 10..40
    frame.fill_rect(Rect { x: 0.0, y: 0.0, width: 100.0, height: 100.0 }, 0xFFFF0000);

    // Inside clip: should be red
    assert_eq!(frame.get_pixel(20, 20), 0xFFFF0000);
    // Outside clip: should be untouched (0)
    assert_eq!(frame.get_pixel(5, 5), 0);
    assert_eq!(frame.get_pixel(50, 50), 0);
    // Edge of clip: 10 is inside, 40 is outside (exclusive)
    assert_eq!(frame.get_pixel(10, 10), 0xFFFF0000);
    assert_eq!(frame.get_pixel(39, 39), 0xFFFF0000);
    assert_eq!(frame.get_pixel(40, 40), 0);
}

#[test]
fn test_frame_no_clip_unchanged_behavior() {
    let mut buffer = vec![0u32; 50 * 50];
    let mut frame = Frame::new(&mut buffer, 50, 50);
    // No clip set — default behavior
    frame.fill_rect(Rect { x: 0.0, y: 0.0, width: 50.0, height: 50.0 }, 0xFF00FF00);
    assert_eq!(frame.get_pixel(0, 0), 0xFF00FF00);
    assert_eq!(frame.get_pixel(49, 49), 0xFF00FF00);
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --lib view::frame::tests -- test_frame_with_clip`
Expected: compilation error — `set_clip` method doesn't exist

**Step 3: Implement ClipRect and set_clip**

In `src/view/frame.rs`, add the `ClipRect` struct before `Frame`, add the `clip` field, and add `set_clip()` / `clear_clip()` methods plus private helpers:

```rust
/// Clipping rectangle in pixel coordinates (inclusive start, exclusive end).
#[derive(Clone, Copy, Debug)]
struct ClipRect {
    x0: usize,
    y0: usize,
    x1: usize,
    y1: usize,
}

pub struct Frame<'a> {
    buffer: &'a mut [u32],
    width: usize,
    height: usize,
    clip: Option<ClipRect>,
}
```

In `Frame::new()`, initialize `clip: None`.

Add methods:

```rust
/// Set a clipping rectangle. All subsequent drawing operations will be
/// constrained to this region. Coordinates are in pixels (Rect uses f32).
pub fn set_clip(&mut self, rect: Rect) {
    let x0 = (rect.x.max(0.0) as usize).min(self.width);
    let y0 = (rect.y.max(0.0) as usize).min(self.height);
    let x1 = ((rect.x + rect.width) as usize).min(self.width);
    let y1 = ((rect.y + rect.height) as usize).min(self.height);
    self.clip = Some(ClipRect { x0, y0, x1, y1 });
}

/// Remove the clipping rectangle, restoring full-frame drawing.
pub fn clear_clip(&mut self) {
    self.clip = None;
}

/// Effective max x (exclusive), considering clip rect.
#[inline]
fn max_x(&self) -> usize {
    self.clip.map_or(self.width, |c| c.x1)
}

/// Effective max y (exclusive), considering clip rect.
#[inline]
fn max_y(&self) -> usize {
    self.clip.map_or(self.height, |c| c.y1)
}

/// Effective min x (inclusive), considering clip rect.
#[inline]
fn min_x(&self) -> usize {
    self.clip.map_or(0, |c| c.x0)
}

/// Effective min y (inclusive), considering clip rect.
#[inline]
fn min_y(&self) -> usize {
    self.clip.map_or(0, |c| c.y0)
}
```

**Step 4: Update `fill_rect` to respect clip bounds**

Replace the bounds clamping in `fill_rect`:

```rust
pub fn fill_rect(&mut self, rect: Rect, color: u32) {
    let x0 = (rect.x.max(0.0) as usize).min(self.width).max(self.min_x());
    let y0 = (rect.y.max(0.0) as usize).min(self.height).max(self.min_y());
    let x1 = ((rect.x + rect.width) as usize).min(self.max_x());
    let y1 = ((rect.y + rect.height) as usize).min(self.max_y());
    // ... loop unchanged
}
```

Apply the same pattern to: `fill_rect_px`, `fill_rect_blended`, `blend_rect_px`, `blend_rect`.

**Step 5: Update `set_pixel` and `blend_pixel` to respect clip bounds**

```rust
pub fn set_pixel(&mut self, x: usize, y: usize, color: u32) {
    if x >= self.min_x() && x < self.max_x() && y >= self.min_y() && y < self.max_y() {
        self.buffer[y * self.width + x] = color;
    }
}

pub fn blend_pixel(&mut self, x: usize, y: usize, color: u32) {
    if x < self.min_x() || x >= self.max_x() || y < self.min_y() || y >= self.max_y() {
        return;
    }
    // ... rest unchanged
}
```

**Step 6: Run tests to verify they pass**

Run: `cargo test --lib view::frame::tests`
Expected: all tests pass including the two new ones plus the three existing ones

**Step 7: Run lint**

Run: `cargo clippy -- -D warnings`
Expected: clean

**Step 8: Commit**

```
feat: add clip rectangle support to Frame
```

---

### Task 2: Add blend_text_pixel to Frame and update TextPainter

**Files:**
- Modify: `src/view/frame.rs:176-214` (pixel methods), `src/view/frame.rs:452-502` (TextPainter::draw)
- Test: `src/view/frame.rs` (inline tests)

**Step 1: Write failing test for text clipping**

```rust
#[test]
fn test_frame_clip_set_pixel() {
    let mut buffer = vec![0u32; 20 * 20];
    let mut frame = Frame::new(&mut buffer, 20, 20);
    frame.set_clip(Rect { x: 5.0, y: 5.0, width: 10.0, height: 10.0 });

    // Inside clip
    frame.set_pixel(7, 7, 0xFFAA0000);
    assert_eq!(frame.get_pixel(7, 7), 0xFFAA0000);

    // Outside clip — should be ignored
    frame.set_pixel(3, 3, 0xFFBB0000);
    assert_eq!(frame.get_pixel(3, 3), 0);

    // At clip boundary (exclusive end)
    frame.set_pixel(15, 15, 0xFFCC0000);
    assert_eq!(frame.get_pixel(15, 15), 0);
}

#[test]
fn test_frame_clip_blend_pixel() {
    let mut buffer = vec![0xFFFFFFFF_u32; 20 * 20]; // white
    let mut frame = Frame::new(&mut buffer, 20, 20);
    frame.set_clip(Rect { x: 5.0, y: 5.0, width: 10.0, height: 10.0 });

    // Blend inside clip — should modify pixel
    frame.blend_pixel(7, 7, 0x80000000); // 50% black
    let result = frame.get_pixel(7, 7);
    let r = (result >> 16) & 0xFF;
    assert!(r > 100 && r < 160, "R channel: {}", r);

    // Blend outside clip — should remain white
    frame.blend_pixel(3, 3, 0x80000000);
    assert_eq!(frame.get_pixel(3, 3), 0xFFFFFFFF);
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --lib view::frame::tests -- test_frame_clip`
Expected: FAIL (set_pixel/blend_pixel don't respect clip yet from Task 1)

Note: If Task 1 already updated set_pixel/blend_pixel, these should pass immediately. Either way, verify.

**Step 3: Add `blend_text_pixel` method to Frame**

This method is purpose-built for TextPainter: takes separate alpha, respects clip, avoids redundant alpha extraction:

```rust
/// Blend a pixel with a given alpha value (0.0-1.0), respecting clip rect.
/// Optimized for text rendering where alpha comes from glyph bitmap.
#[inline]
pub fn blend_text_pixel(&mut self, x: usize, y: usize, color: u32, alpha: f32) {
    if x < self.min_x() || x >= self.max_x() || y < self.min_y() || y >= self.max_y() {
        return;
    }
    let idx = y * self.width + x;
    self.buffer[idx] = blend_colors(self.buffer[idx], color, alpha);
}
```

**Step 4: Update TextPainter::draw to use Frame method**

Replace the direct buffer access in `TextPainter::draw` (lines 481-494):

```rust
// Before:
if px >= 0 && py >= 0 {
    let px = px as usize;
    let py = py as usize;
    if px < frame.width && py < frame.height {
        let alpha_f = alpha as f32 / 255.0;
        let idx = py * frame.width + px;
        frame.buffer[idx] = blend_colors(frame.buffer[idx], color, alpha_f);
    }
}

// After:
if px >= 0 && py >= 0 {
    frame.blend_text_pixel(
        px as usize,
        py as usize,
        color,
        alpha as f32 / 255.0,
    );
}
```

**Step 5: Run tests**

Run: `cargo test --lib view::frame::tests`
Expected: all pass

**Step 6: Run lint**

Run: `cargo clippy -- -D warnings`
Expected: clean

**Step 7: Commit**

```
feat: route TextPainter pixel writes through Frame clipping
```

---

### Task 3: Apply clip rect in render_sidebar

**Files:**
- Modify: `src/view/mod.rs:1116-1135` (render_sidebar top)

**Step 1: Add set_clip call at start of render_sidebar**

After the background fill (line 1135), add:

```rust
// Clip all subsequent drawing to the sidebar bounds
frame.set_clip(Rect::new(0.0, 0.0, sidebar_width as f32, sidebar_height as f32));
```

At the end of `render_sidebar` (before the closing brace at line 1304), add:

```rust
frame.clear_clip();
```

**Step 2: Build and test**

Run: `cargo build && cargo test`
Expected: compiles cleanly, all tests pass

**Step 3: Manual test**

Run: `cargo run --release -- samples/` (or a directory with long filenames)
Expected: sidebar content stays within sidebar bounds; no text bleeds into editor area

**Step 4: Commit**

```
fix: clip sidebar rendering to prevent overflow into editor area
```

---

### Task 4: Fix truncation — use real char width and char-safe slicing

**Files:**
- Modify: `src/view/mod.rs:1243-1263` (truncation logic in render_node)

**Step 1: Replace hardcoded char width with real metric**

The `SidebarRenderContext` struct needs the char width. Add a field:

```rust
struct SidebarRenderContext {
    // ... existing fields ...
    char_width: usize,
}
```

Initialize it in `render_sidebar`:

```rust
let ctx = SidebarRenderContext {
    // ... existing fields ...
    char_width: painter.char_width().ceil() as usize,
};
```

**Step 2: Fix the truncation logic in render_node**

Replace lines 1247-1263:

```rust
// Calculate available width for text
let right_padding = 8;
let available_width = ctx.sidebar_width.saturating_sub(text_x + right_padding);

// Use actual char width from font metrics
let max_chars = if ctx.char_width > 0 {
    available_width / ctx.char_width
} else {
    available_width / 8 // fallback
};

let name_chars = node.name.chars().count();
let needs_truncation = name_chars > max_chars && max_chars > 3;

if needs_truncation {
    // Use char_indices for safe UTF-8 boundary slicing
    let truncate_at = max_chars.saturating_sub(1);
    let byte_end = node.name.char_indices()
        .nth(truncate_at)
        .map(|(i, _)| i)
        .unwrap_or(node.name.len());
    let mut display_name = String::with_capacity(byte_end + 3);
    display_name.push_str(&node.name[..byte_end]);
    display_name.push('\u{2026}'); // ellipsis
    painter.draw(frame, text_x, text_y, &display_name, fg);
} else {
    painter.draw(frame, text_x, text_y, &node.name, fg);
}
```

**Step 3: Build and test**

Run: `cargo build && cargo test`
Expected: compiles cleanly, all tests pass

**Step 4: Run lint**

Run: `cargo clippy -- -D warnings`
Expected: clean

**Step 5: Commit**

```
fix: use actual font metrics for sidebar truncation, fix UTF-8 safety
```

---

### Task 5: Final verification

**Step 1: Run full test suite**

Run: `make test`
Expected: all pass

**Step 2: Run lint**

Run: `make lint`
Expected: clean

**Step 3: Manual verification with edge cases**

- Open a directory with very long filenames (50+ chars)
- Open a directory with Unicode filenames (emoji, CJK characters)
- Resize sidebar to minimum width (150px) — verify truncation works
- Resize sidebar to maximum width (500px) — verify no unnecessary truncation
