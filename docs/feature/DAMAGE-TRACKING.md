# Damage Tracking System

**Status:** 📋 Planned  
**Priority:** P3 (optimization)  
**Effort:** L–XL (1–3 days)  
**Created:** 2025-12-15

Partial redraw system to avoid full-frame rendering on every update.

---

## Table of Contents

1. [Overview](#overview)
2. [Current Architecture](#current-architecture)
3. [Design Goals](#design-goals)
4. [Proposed Design](#proposed-design)
5. [Implementation Plan](#implementation-plan)
6. [API Changes](#api-changes)
7. [Migration Strategy](#migration-strategy)
8. [Testing Strategy](#testing-strategy)
9. [Risks & Guardrails](#risks--guardrails)
10. [Future Enhancements](#future-enhancements)

---

## Overview

The editor currently redraws the entire window on every frame, even for small changes like cursor blinks. This document outlines a damage tracking system to enable partial redraws while maintaining correctness.

**Key Principle:** Correctness over performance. Any uncertainty should fall back to full redraw.

---

## Current Architecture

### Render Flow

```
render()
├── time_clear: frame.clear(bg_color)
├── time_text: render_editor_area()
│   ├── render_editor_group() × N groups
│   │   ├── render_tab_bar()
│   │   ├── render_text_area()
│   │   └── render_gutter()
│   └── render_splitters()
├── time_status_bar: render_status_bar()
├── render_modals() (if modal active, calls frame.dim() first)
├── render_perf_overlay() (debug only)
└── render_debug_overlay() (debug only)
```

### Current Cmd Enum

```rust
pub enum Cmd {
    None,
    Redraw,                              // Full redraw
    SaveFile { path: PathBuf, content: String },
    LoadFile { path: PathBuf },
    Batch(Vec<Cmd>),
}
```

### Frame API

```rust
impl Frame {
    pub fn clear(&mut self, color: u32);
    pub fn fill_rect(&mut self, rect: Rect, color: u32);
    pub fn fill_rect_px(&mut self, x, y, w, h, color: u32);
    pub fn blend_pixel(&mut self, x, y, color: u32);
    pub fn blend_rect(&mut self, rect: Rect, color: u32);
    pub fn dim(&mut self, alpha: u8);  // Dims entire frame
}
```

---

## Design Goals

1. **Minimal complexity** - Coarse region-level granularity, not per-pixel or per-line
2. **Elm-style compatible** - Damage hints flow through `Cmd`, keeping model pure
3. **Safe fallback** - Any ambiguity triggers full redraw
4. **Incremental adoption** - Existing code keeps working; opt-in to partial redraws
5. **No Frame API changes** - Existing `Frame` methods continue to work

---

## Proposed Design

### Damage Granularity

Two high-level regions aligned with render structure:

| Region | Covers | Typical Triggers |
|--------|--------|------------------|
| `EditorArea` | All groups + tab bars + gutters + text areas + splitters | Text edits, cursor movement, scrolling, selection |
| `StatusBar` | Bottom status line | Status text changes |

**Why coarse?** Fine-grained (per-line, per-group) adds complexity with diminishing returns for a lightweight editor. The common case (cursor blink, typing) only needs EditorArea.

### Damage Types

```rust
/// Represents which parts of the UI need redrawing
#[derive(Debug, Clone)]
pub enum Damage {
    /// Redraw everything (current behavior)
    Full,
    /// Redraw specific areas only
    Areas(Vec<DamageArea>),
}

/// High-level UI regions that can be independently redrawn
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DamageArea {
    /// All editor groups, tab bars, gutters, text areas, splitters
    EditorArea,
    /// Bottom status bar
    StatusBar,
}

impl Default for Damage {
    fn default() -> Self {
        Damage::Full
    }
}
```

### Extended Cmd Enum

```rust
pub enum Cmd {
    #[default]
    None,
    /// Request full redraw (legacy, always safe)
    Redraw,
    /// Request partial redraw of specific areas
    RedrawAreas(Vec<DamageArea>),
    SaveFile { path: PathBuf, content: String },
    LoadFile { path: PathBuf },
    Batch(Vec<Cmd>),
}
```

### Damage Computation

```rust
impl Cmd {
    /// Compute the aggregate damage from this command
    pub fn damage(&self) -> Damage {
        match self {
            Cmd::None => Damage::Areas(vec![]),
            Cmd::Redraw => Damage::Full,
            Cmd::RedrawAreas(areas) => Damage::Areas(areas.clone()),
            Cmd::SaveFile { .. } | Cmd::LoadFile { .. } => Damage::Full,
            Cmd::Batch(cmds) => {
                let mut areas = HashSet::new();
                for cmd in cmds {
                    match cmd.damage() {
                        Damage::Full => return Damage::Full,
                        Damage::Areas(a) => areas.extend(a),
                    }
                }
                Damage::Areas(areas.into_iter().collect())
            }
        }
    }
}
```

---

## Implementation Plan

### Phase 1: Add Types & Cmd Extension (30 min)

**Files:** `src/commands.rs`

1. Add `Damage` and `DamageArea` types
2. Add `Cmd::RedrawAreas(Vec<DamageArea>)` variant
3. Implement `Cmd::damage()` method
4. Keep `Cmd::needs_redraw()` working (backward compatible)

### Phase 2: Renderer Signature Change (30 min)

**Files:** `src/view/mod.rs`, `src/runtime/app.rs`

1. Change `Renderer::render()` to accept `&Damage` parameter
2. Update call sites to pass `Damage::Full` initially (no behavior change)
3. Verify all tests pass

### Phase 3: Implement Partial Redraw Logic (1-2 hours)

**Files:** `src/view/mod.rs`

1. Compute effective damage (force Full for modals, overlays, etc.)
2. Implement conditional rendering paths
3. Only clear damaged regions, not full frame

```rust
pub fn render(&mut self, model: &mut AppModel, perf: &mut PerfStats, damage: &Damage) -> Result<()> {
    // ... existing resize/layout logic ...
    
    // Force full redraw for complex cases
    let effective_damage = if model.ui.active_modal.is_some() {
        Damage::Full
    } else {
        damage.clone()
    };
    
    #[cfg(debug_assertions)]
    let effective_damage = if perf.should_show_overlay() || model.debug_overlay.is_some() {
        Damage::Full
    } else {
        effective_damage
    };
    
    match effective_damage {
        Damage::Full => {
            // Current full render path
        }
        Damage::Areas(ref areas) => {
            // Partial render path - only clear and render damaged areas
        }
    }
}
```

### Phase 4: Wire Up Update Functions (1-2 hours)

**Files:** `src/update/*.rs`

Convert `Cmd::Redraw` to `Cmd::RedrawAreas` where appropriate:

| Update Function | Damage |
|----------------|--------|
| Text insert/delete | `EditorArea` |
| Cursor movement | `EditorArea` |
| Scrolling | `EditorArea` |
| Selection changes | `EditorArea` |
| Cursor blink toggle | `EditorArea` |
| Status text change | `StatusBar` |
| Tab dirty indicator | `EditorArea + StatusBar` |
| Modal open/close | `Full` (keep as `Cmd::Redraw`) |
| Theme change | `Full` |
| Window resize | `Full` |
| Split/close group | `Full` |

### Phase 5: Testing & Validation (1-2 hours)

1. Manual testing of all UI interactions
2. Verify no visual artifacts from partial redraws
3. Profile to confirm reduced CPU usage
4. Add integration tests for damage calculation

---

## API Changes

### Commands Module

```rust
// New types
pub enum Damage { Full, Areas(Vec<DamageArea>) }
pub enum DamageArea { EditorArea, StatusBar }

// Extended Cmd
pub enum Cmd {
    None,
    Redraw,                         // existing - implies Damage::Full
    RedrawAreas(Vec<DamageArea>),   // new - partial redraw
    SaveFile { path, content },
    LoadFile { path },
    Batch(Vec<Cmd>),
}

// New method
impl Cmd {
    pub fn damage(&self) -> Damage;
}
```

### Renderer

```rust
// Before
pub fn render(&mut self, model: &mut AppModel, perf: &mut PerfStats) -> Result<()>

// After
pub fn render(&mut self, model: &mut AppModel, perf: &mut PerfStats, damage: &Damage) -> Result<()>
```

### Event Loop (runtime/app.rs)

```rust
// Before
if cmd.needs_redraw() {
    renderer.render(&mut model, &mut perf)?;
}

// After
if cmd.needs_redraw() {
    renderer.render(&mut model, &mut perf, &cmd.damage())?;
}
```

---

## Migration Strategy

1. **Phase 1-2:** All existing code continues to work unchanged
2. **Phase 3:** Partial redraw implemented but not used (all calls pass `Damage::Full`)
3. **Phase 4:** Gradually convert update functions to use `Cmd::RedrawAreas`
4. **Rollback:** If issues arise, convert back to `Cmd::Redraw` (single-line change per call site)

---

## Testing Strategy

### Unit Tests

```rust
#[test]
fn test_cmd_damage_computation() {
    // None -> empty areas
    assert!(matches!(Cmd::None.damage(), Damage::Areas(a) if a.is_empty()));
    
    // Redraw -> Full
    assert!(matches!(Cmd::Redraw.damage(), Damage::Full));
    
    // RedrawAreas preserves areas
    let cmd = Cmd::RedrawAreas(vec![DamageArea::EditorArea]);
    assert!(matches!(cmd.damage(), Damage::Areas(a) if a.contains(&DamageArea::EditorArea)));
    
    // Batch with Full -> Full
    let batch = Cmd::Batch(vec![
        Cmd::RedrawAreas(vec![DamageArea::StatusBar]),
        Cmd::Redraw,
    ]);
    assert!(matches!(batch.damage(), Damage::Full));
    
    // Batch without Full -> merged areas
    let batch = Cmd::Batch(vec![
        Cmd::RedrawAreas(vec![DamageArea::StatusBar]),
        Cmd::RedrawAreas(vec![DamageArea::EditorArea]),
    ]);
    assert!(matches!(batch.damage(), Damage::Areas(a) if a.len() == 2));
}
```

### Integration Tests

- Cursor blink only redraws editor area
- Status message change only redraws status bar
- Modal open triggers full redraw
- Theme change triggers full redraw

### Manual Validation Checklist

- [ ] Cursor blink doesn't cause status bar flicker
- [ ] Typing in one group doesn't affect other groups visually
- [ ] Scrolling works correctly
- [ ] Selection highlighting updates properly
- [ ] Modal dim effect renders correctly
- [ ] Debug overlays don't leave artifacts
- [ ] Window resize clears everything properly
- [ ] Theme switch updates all regions

---

## Risks & Guardrails

### Risk: Stale Pixels

**Problem:** Partial redraw leaves old pixels if damage underestimated.

**Guardrails:**
- Conservative damage hints (when in doubt, include more areas)
- Force `Damage::Full` for any global state change
- Debug mode can highlight damaged regions

### Risk: Modal Dim Accumulation

**Problem:** `frame.dim()` darkens existing pixels. Re-applying it compounds the effect.

**Guardrail:** Always use `Damage::Full` when modal is active.

### Risk: Overlapping Regions

**Problem:** If two regions overlap (they don't currently), partial redraw order matters.

**Guardrail:** Current regions (EditorArea, StatusBar) don't overlap. If adding more, ensure clear boundaries.

### Risk: Buffer Invalidation

**Problem:** Window resize or surface recreation invalidates all pixels.

**Guardrail:** Resize path already forces redraw; surface recreation triggers full render.

---

## Future Enhancements

Consider these only if profiling shows need:

### Per-Group Damage

```rust
pub enum DamageArea {
    EditorArea,
    EditorGroup(GroupId),  // New: specific group only
    StatusBar,
}
```

Useful if editing in one group shouldn't redraw others.

### Line-Level Damage

```rust
pub enum DamageArea {
    EditorArea,
    EditorLines { group: GroupId, lines: Range<usize> },
    StatusBar,
}
```

Useful for very large files where redrawing all visible lines is expensive.

### Offscreen Modal Buffer

Instead of `dim()` on the live buffer, composite modal onto a cached background. Enables modal-only redraws without full refresh.

---

## References

- [GUI-CLEANUP.md](../archived/GUI-CLEANUP.md) – Original GUI improvement plan (archived)
- [EDITOR_UI_REFERENCE.md](../EDITOR_UI_REFERENCE.md) – UI geometry reference
- Alacritty damage tracking: Line-level damage accumulation
- Helix compositor: Layer-based EventResult::Consumed pattern
