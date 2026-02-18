# UI Scaling Review: Retina/HiDPI Display Support

**Status:** ✅ Implemented (v0.3.4)
**Priority:** P1 (affects all retina users)
**Effort:** M (2-4 hours)
**Created:** 2025-12-16
**Completed:** 2025-12-16

This document provides a comprehensive analysis of how the Token editor handles pixel coordinates, scale factors, and HiDPI/Retina displays, identifies current issues, and proposes a fix.

---

## Table of Contents

1. [Executive Summary](#executive-summary)
2. [Current Architecture Analysis](#current-architecture-analysis)
3. [Identified Issues](#identified-issues)
4. [Proposed Solution](#proposed-solution)
5. [Implementation Plan](#implementation-plan)
6. [Coordinate System Guidelines](#coordinate-system-guidelines)
7. [Testing Strategy](#testing-strategy)
8. [References](#references)

---

## Executive Summary

### What Works Today

- Font rendering scales correctly (14pt * scale_factor)
- Window dimensions are correctly handled in physical pixels
- Mouse input is correctly received in physical pixels
- Character widths and line heights are correctly computed from scaled fonts

### What Was Fixed (v0.3.4)

- **UI layout constants now scale**: `ScaledMetrics` struct holds all scaled values
- **Scale factor is stored**: `model.metrics.scale_factor` available throughout
- **Scale factor change handling**: `ScaleFactorChanged` event triggers full renderer reinitialization
- **Surface properly resized**: New Surface explicitly resized after creation
- **Dynamic tab bar height**: Computed from glyph metrics (`line_height + padding * 2`)
- **Buffer bounds checking**: `Frame::new` validates buffer size to prevent panics

### Previous Impact (Now Fixed)

On a 2x Retina display (before fix):
- Tab bar appeared half the intended size (28px physical = 14pt logical)
- Gutter padding appeared too tight
- Click targets for tabs were misaligned
- Overall UI looked cramped while text looked correct
- Switching displays caused incorrect rendering until window resize

---

## Current Architecture Analysis

### 1. Scale Factor Retrieval (Correct)

**Location:** `src/view/mod.rs:42-58`

```rust
impl Renderer {
    pub fn new(window: Rc<Window>, ...) -> Result<Self> {
        let scale_factor = window.scale_factor();  // Correctly retrieved
        // ...
        let font_size = 14.0 * scale_factor as f32;  // Font correctly scaled
        // ...
    }
}
```

The scale factor is retrieved correctly from winit, and font size is properly scaled. However, the scale factor is **not stored** for later use.

### 2. Window Coordinates (Physical Pixels Throughout)

Winit provides everything in **physical pixels**:

| Event | Type | Unit |
|-------|------|------|
| `WindowEvent::Resized` | `PhysicalSize<u32>` | Physical pixels |
| `WindowEvent::CursorMoved` | `PhysicalPosition<f64>` | Physical pixels |
| `window.inner_size()` | `PhysicalSize<u32>` | Physical pixels |

**Source:** [winit documentation](https://docs.rs/winit/latest/winit/event/enum.WindowEvent.html)

The code correctly uses these physical values throughout rendering.

### 3. Hardcoded Layout Constants (THE BUG)

**Problematic constants:**

| Constant | Location | Current Value | Issue |
|----------|----------|---------------|-------|
| `TAB_BAR_HEIGHT` | `src/view/geometry.rs:18` | `28` | Not scaled |
| `SPLITTER_WIDTH` | `src/model/editor_area.rs:602` | `6.0` | Not scaled |
| `GUTTER_PADDING_PX` | `src/model/mod.rs:224` | `4.0` | Not scaled |
| `TEXT_AREA_PADDING_PX` | `src/model/mod.rs:226` | `8.0` | Not scaled |

**Inline hardcoded values in `src/view/mod.rs`:**
- Tab padding: `rect_x + 4`, `tab_x + 8`, `tab_y + 4`
- Tab gap: `tab_width + 2`, `+ 16` for text padding
- Border width: `1` pixel

These constants work correctly on 1x displays but produce half-sized UI elements on 2x Retina displays because:
- Font renders at 28pt (14 * 2) - takes up 2x physical pixels per logical pixel
- Tab bar is 28 physical pixels (but should be 56 for correct proportions)

### 4. Window Creation (Logical → Physical Conversion)

**Location:** `src/runtime/app.rs:877-881`

```rust
let window_attributes = Window::default_attributes()
    .with_title("Token")
    .with_inner_size(LogicalSize::new(800, 600));  // Logical size
```

The window is created with a **logical size**, which winit correctly converts to physical size based on the display's scale factor. This is correct behavior.

### 5. Missing Event Handler

No handler for `WindowEvent::ScaleFactorChanged`:
- If user drags window to a monitor with different DPI, the UI won't adapt
- The renderer would need to reinitialize font metrics and layout

---

## Identified Issues

### Issue 1: UI Elements Don't Scale with Display DPI

**Severity:** High
**Affected Users:** All Retina/HiDPI display users

**Symptoms:**
- Tab bar appears too short
- Gutter padding appears cramped
- Overall UI feels "tight" compared to text

**Root Cause:** Layout constants are in physical pixels but aren't multiplied by scale factor.

### Issue 2: No Scale Factor Storage

**Severity:** Medium

**Root Cause:** Scale factor is retrieved at Renderer init but never stored. It's needed for:
- Scaling layout constants
- Converting between coordinate systems
- Recreating layout after scale factor change

### Issue 3: No Scale Factor Change Handling

**Severity:** Medium
**Affected:** Users with multiple monitors of different DPI

**Root Cause:** Missing handler for `WindowEvent::ScaleFactorChanged`.

### Issue 4: Inconsistent Coordinate Types

**Severity:** Low (code quality)

The codebase uses multiple types without clear semantic meaning:
- `f32` for some dimensions
- `f64` for mouse positions
- `usize` for rendering pixels
- `u32` for window sizes

No type-level distinction between physical and logical pixels.

---

## Proposed Solution

### Strategy: Physical Pixels with Scaled Constants

Continue working entirely in **physical pixels** (current approach), but:
1. Store the scale factor
2. Scale all layout constants by the scale factor
3. Handle scale factor changes

This minimizes refactoring while fixing the core issues.

### Why Not Logical Pixels?

Working in logical pixels would require:
- Converting all mouse input from physical to logical
- Converting all rendering output from logical to physical
- More places for conversion bugs

Since rendering (softbuffer) works in physical pixels and winit provides physical coordinates, staying in physical pixels is simpler.

### Core Changes

#### 1. Store Scale Factor in Model

```rust
// src/model/mod.rs

pub struct AppModel {
    // ... existing fields ...

    /// Display scale factor (1.0 for standard, 2.0 for retina)
    pub scale_factor: f64,
}
```

#### 2. Add Scaled Layout Metrics

```rust
// src/model/mod.rs (new)

/// Layout metrics scaled for the current display
#[derive(Debug, Clone, Copy)]
pub struct ScaledMetrics {
    /// Tab bar height in physical pixels
    pub tab_bar_height: usize,
    /// Splitter width in physical pixels
    pub splitter_width: f32,
    /// Gutter padding in physical pixels
    pub gutter_padding: f32,
    /// Text area padding in physical pixels
    pub text_area_padding: f32,
    /// Standard UI padding (small) in physical pixels
    pub padding_small: usize,
    /// Standard UI padding (medium) in physical pixels
    pub padding_medium: usize,
}

impl ScaledMetrics {
    /// Base values at scale factor 1.0
    const BASE_TAB_BAR_HEIGHT: f64 = 28.0;
    const BASE_SPLITTER_WIDTH: f64 = 6.0;
    const BASE_GUTTER_PADDING: f64 = 4.0;
    const BASE_TEXT_AREA_PADDING: f64 = 8.0;
    const BASE_PADDING_SMALL: f64 = 2.0;
    const BASE_PADDING_MEDIUM: f64 = 4.0;

    pub fn new(scale_factor: f64) -> Self {
        Self {
            tab_bar_height: (Self::BASE_TAB_BAR_HEIGHT * scale_factor).round() as usize,
            splitter_width: (Self::BASE_SPLITTER_WIDTH * scale_factor) as f32,
            gutter_padding: (Self::BASE_GUTTER_PADDING * scale_factor) as f32,
            text_area_padding: (Self::BASE_TEXT_AREA_PADDING * scale_factor) as f32,
            padding_small: (Self::BASE_PADDING_SMALL * scale_factor).round() as usize,
            padding_medium: (Self::BASE_PADDING_MEDIUM * scale_factor).round() as usize,
        }
    }
}
```

#### 3. Update Model to Include Scaled Metrics

```rust
// src/model/mod.rs

pub struct AppModel {
    // ... existing fields ...
    pub scale_factor: f64,
    pub metrics: ScaledMetrics,
}

impl AppModel {
    pub fn new(window_width: u32, window_height: u32, scale_factor: f64, ...) -> Self {
        let metrics = ScaledMetrics::new(scale_factor);
        // ... rest of initialization
    }

    /// Update scale factor and recalculate metrics
    pub fn set_scale_factor(&mut self, scale_factor: f64) {
        self.scale_factor = scale_factor;
        self.metrics = ScaledMetrics::new(scale_factor);
        // Also need to update font metrics via renderer
    }
}
```

#### 4. Handle Scale Factor Changes

```rust
// src/runtime/app.rs

WindowEvent::ScaleFactorChanged { scale_factor, inner_size_writer } => {
    // Update model
    update(&mut self.model, Msg::App(AppMsg::ScaleFactorChanged(scale_factor)));

    // Reinitialize renderer with new font size
    if let Some(window) = &self.window {
        if let Some(context) = &self.context {
            self.renderer = Some(Renderer::new(Rc::clone(window), context, scale_factor)?);
            // Sync char_width and line_height to model
            if let Some(renderer) = &self.renderer {
                update(&mut self.model,
                    Msg::App(AppMsg::FontMetricsChanged(
                        renderer.char_width(),
                        renderer.line_height()
                    ))
                );
            }
        }
    }

    Some(Cmd::Redraw)
}
```

#### 5. Update Renderer to Accept Scale Factor

```rust
// src/view/mod.rs

impl Renderer {
    pub fn new(window: Rc<Window>, context: &softbuffer::Context<Rc<Window>>) -> Result<Self> {
        let scale_factor = window.scale_factor();
        Self::with_scale_factor(window, context, scale_factor)
    }

    pub fn with_scale_factor(
        window: Rc<Window>,
        context: &softbuffer::Context<Rc<Window>>,
        scale_factor: f64
    ) -> Result<Self> {
        let font_size = 14.0 * scale_factor as f32;
        // ... existing initialization ...
    }
}
```

#### 6. Replace Hardcoded Constants

Replace all uses of hardcoded constants with `model.metrics.*`:

```rust
// Before (src/view/mod.rs)
frame.fill_rect_px(rect_x, rect_y, rect_w, TAB_BAR_HEIGHT, tab_bar_bg);
let tab_x = rect_x + 4;

// After
frame.fill_rect_px(rect_x, rect_y, rect_w, model.metrics.tab_bar_height, tab_bar_bg);
let tab_x = rect_x + model.metrics.padding_medium;
```

---

## Implementation Plan

### Phase 1: Add Infrastructure (30 min)

1. Add `scale_factor: f64` field to `AppModel`
2. Add `ScaledMetrics` struct with all layout constants
3. Add `metrics: ScaledMetrics` field to `AppModel`
4. Update `AppModel::new()` to accept scale factor
5. Add `AppMsg::ScaleFactorChanged(f64)` message variant
6. Add `AppMsg::FontMetricsChanged(f32, usize)` message variant

**Files:** `src/model/mod.rs`, `src/messages.rs`

### Phase 2: Update App Initialization (30 min)

1. Pass scale factor from Renderer to App on init
2. Handle `WindowEvent::ScaleFactorChanged`
3. Reinitialize renderer on scale factor change
4. Sync font metrics back to model

**Files:** `src/runtime/app.rs`, `src/view/mod.rs`

### Phase 3: Replace Hardcoded Values (1-2 hours)

1. Replace `TAB_BAR_HEIGHT` with `model.metrics.tab_bar_height`
2. Replace `SPLITTER_WIDTH` with `model.metrics.splitter_width`
3. Replace `GUTTER_PADDING_PX` with `model.metrics.gutter_padding`
4. Replace `TEXT_AREA_PADDING_PX` with `model.metrics.text_area_padding`
5. Replace inline padding values (`+ 4`, `+ 8`, `+ 16`, etc.)
6. Update geometry helpers to accept metrics

**Files:**
- `src/view/mod.rs` (tab bar, gutter rendering)
- `src/view/geometry.rs` (hit testing)
- `src/model/editor_area.rs` (splitter calculations)
- `src/model/mod.rs` (gutter calculations)

### Phase 4: Update Tests (30 min)

1. Update test helpers to include scale factor
2. Add tests for scaled metrics calculation
3. Verify hit testing works at different scale factors

**Files:** `tests/common/mod.rs`, `tests/layout.rs`

### Phase 5: Documentation (15 min)

1. Update CLAUDE.md if needed
2. Add comments explaining coordinate systems
3. Archive this document in docs/archived/ after implementation

---

## Coordinate System Guidelines

### For Future Development

1. **All rendering coordinates are in physical pixels**
   - Framebuffer dimensions
   - Drawing positions
   - Rectangle bounds

2. **All layout constants should use `ScaledMetrics`**
   - Never hardcode pixel values in rendering code
   - Define base values at scale factor 1.0
   - Multiply by scale factor to get physical pixels

3. **Window creation uses logical pixels**
   - `LogicalSize::new(800, 600)` - winit converts to physical

4. **Mouse input is in physical pixels**
   - `CursorMoved.position` is `PhysicalPosition<f64>`
   - No conversion needed for hit testing

5. **Document coordinates are scale-independent**
   - Line numbers, column numbers
   - Selection ranges
   - Cursor positions

### Type Conventions (Recommended for Future Refactor)

Consider adding type aliases for clarity:

```rust
/// Physical pixel coordinate (actual display pixels)
type PhysicalPx = f64;

/// Logical point (at scale factor 1.0, multiply by scale_factor for physical)
type LogicalPt = f64;
```

---

## Testing Strategy

### Manual Testing Checklist

- [ ] Launch on standard 1x display - UI should look unchanged
- [ ] Launch on 2x Retina display - UI elements should be correctly sized
- [ ] Drag window from 1x to 2x display - UI should adapt
- [ ] Drag window from 2x to 1x display - UI should adapt
- [ ] Tab clicks work correctly on both displays
- [ ] Gutter clicks (line numbers) work correctly
- [ ] Splitter drag works correctly
- [ ] Text cursor positioning matches clicks

### Automated Tests

```rust
#[test]
fn test_scaled_metrics_standard() {
    let metrics = ScaledMetrics::new(1.0);
    assert_eq!(metrics.tab_bar_height, 28);
    assert_eq!(metrics.splitter_width, 6.0);
}

#[test]
fn test_scaled_metrics_retina() {
    let metrics = ScaledMetrics::new(2.0);
    assert_eq!(metrics.tab_bar_height, 56);
    assert_eq!(metrics.splitter_width, 12.0);
}

#[test]
fn test_scaled_metrics_fractional() {
    // Windows uses 1.25, 1.5 scales
    let metrics = ScaledMetrics::new(1.5);
    assert_eq!(metrics.tab_bar_height, 42);  // 28 * 1.5 = 42
}
```

---

## References

### winit Documentation

- [WindowEvent::CursorMoved](https://docs.rs/winit/latest/winit/event/enum.WindowEvent.html) - Returns `PhysicalPosition<f64>`
- [WindowEvent::Resized](https://docs.rs/winit/latest/winit/event/enum.WindowEvent.html) - Returns `PhysicalSize<u32>`
- [WindowEvent::ScaleFactorChanged](https://docs.rs/winit/latest/winit/event/enum.WindowEvent.html) - DPI change handling

### Related Issues

- [winit#1371](https://github.com/rust-windowing/winit/issues/1371) - Historical bug with CursorMoved on macOS
- [winit#1406](https://github.com/rust-windowing/winit/issues/1406) - LogicalPosition vs PhysicalPosition inconsistency

### Internal Documentation

- [EDITOR_UI_REFERENCE.md](../EDITOR_UI_REFERENCE.md) - Chapter 10.6 covers High-DPI
- [TEXT_PHYSICS_LAYER.md](./TEXT_PHYSICS_LAYER.md) - Coordinate systems for cursor/viewport

---

## Summary

The current implementation correctly handles font scaling but fails to scale UI layout constants. The fix involves:

1. Storing the scale factor in the model
2. Creating a `ScaledMetrics` struct with all layout constants
3. Replacing hardcoded pixel values throughout the codebase
4. Handling scale factor changes when moving between monitors

This maintains the current "physical pixels everywhere" approach while ensuring all UI elements scale correctly with the display.
