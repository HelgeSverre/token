# GUI Cleanup & Architecture Improvement Plan

**Status:** 🚧 In Progress  
**Created:** 2025-12-07

This document consolidates the GUI improvement roadmap, including Elm-style directory restructuring and the phased rendering/modal system improvements.

---

## Table of Contents

1. [Phase 0 – Elm-Style Directory Restructure](#phase-0--elm-style-directory-restructure)
2. [Phase 1 – Frame/Painter Abstraction](#phase-1--framepainter-abstraction)
3. [Phase 2 – Widget Extraction & Geometry Centralization](#phase-2--widget-extraction--geometry-centralization)
4. [Phase 3 – Basic Modal/Focus System](#phase-3--basic-modalfocus-system)
5. [Phase 4 – Command Palette](#phase-4--command-palette-full-vertical-slice)
6. [Phase 5 – General Overlay/Compositor & Mouse Blocking](#phase-5--general-overlaycompositor--mouse-blocking)
7. [Phase 6 – Goto Line & Find/Replace Modals](#phase-6--goto-line--findreplace-modals)
8. [Phase 7 – Damage Tracking](#phase-7--damage-tracking-after-ui-stabilizes)
9. [Summary Timeline](#summary-timeline)
10. [Research Summary](#research-summary)

---

## Phase 0 – Elm-Style Directory Restructure

**Goal:** Reorganize codebase to clearly reflect Elm's Model-Update-View architecture; move all tests to `tests/`.  
**Effort:** M (1.5–2h)  
**User Impact:** None (internal restructure)

### Current Structure

```
src/
├── model/              # ✓ State types (exists)
├── update/             # ✓ State handlers (exists)
├── view.rs             # Rendering (flat file)
├── overlay.rs          # Overlay utilities (flat file)
├── app.rs              # winit glue
├── input.rs            # Key/mouse handling
├── perf.rs             # Debug overlay
├── messages.rs         # Msg enums
├── commands.rs         # Cmd enum
├── main.rs             # Entry + ~669 lines of tests
└── ...
```

### Target Structure

```
src/
├── model/              # ✓ Keep as-is
│   ├── document.rs
│   ├── editor.rs
│   ├── editor_area.rs
│   ├── status_bar.rs
│   ├── ui.rs
│   └── mod.rs
│
├── update/             # ✓ Keep as-is
│   ├── app.rs
│   ├── document.rs
│   ├── editor.rs
│   ├── layout.rs
│   ├── ui.rs
│   └── mod.rs
│
├── view/               # NEW: Promote to module
│   ├── mod.rs          # Renderer struct, render_root()
│   ├── editor.rs       # render_editor_area, render_text_area, render_gutter
│   ├── chrome.rs       # render_tab_bar, render_status_bar
│   └── overlay.rs      # Move from src/overlay.rs
│
├── runtime/            # NEW: winit/platform glue
│   ├── mod.rs
│   ├── app.rs          # Move from src/app.rs
│   ├── input.rs        # Move from src/input.rs
│   └── perf.rs         # Move from src/perf.rs
│
├── msg.rs              # Rename from messages.rs
├── cmd.rs              # Rename from commands.rs
├── theme.rs            # Keep
├── util.rs             # Keep
├── lib.rs              # Exports model, update, view, msg, cmd
└── main.rs             # Entry point only (~20 lines)

tests/                  # All tests here
├── common/             # Test helpers (exists)
├── model/              # Tests for model types
│   └── editor_area.rs  # Move from src/model/editor_area.rs
├── view/               # Tests for rendering
│   ├── overlay.rs      # Move from src/overlay.rs
│   └── theme.rs        # Move from src/theme.rs
├── integration/        # End-to-end tests
│   └── main.rs         # Move from src/main.rs
└── ... (existing test files)
```

### Migration Steps

| Step | Action | Files |
|------|--------|-------|
| 1 | Rename `messages.rs` → `msg.rs` | `src/messages.rs` |
| 2 | Rename `commands.rs` → `cmd.rs` | `src/commands.rs` |
| 3 | Create `runtime/` module | `src/runtime/mod.rs` |
| 4 | Move `app.rs` → `runtime/app.rs` | `src/app.rs` |
| 5 | Move `input.rs` → `runtime/input.rs` | `src/input.rs` |
| 6 | Move `perf.rs` → `runtime/perf.rs` | `src/perf.rs` |
| 7 | Create `view/` module | `src/view/mod.rs` |
| 8 | Move `view.rs` content → `view/mod.rs` | `src/view.rs` |
| 9 | Move `overlay.rs` → `view/overlay.rs` | `src/overlay.rs` |
| 10 | Move inline tests from `src/main.rs` → `tests/integration/main.rs` | `src/main.rs` |
| 11 | Move inline tests from `src/model/editor_area.rs` → `tests/model/editor_area.rs` | `src/model/editor_area.rs` |
| 12 | Move inline tests from `src/overlay.rs` → `tests/view/overlay.rs` | `src/overlay.rs` |
| 13 | Move inline tests from `src/theme.rs` → `tests/view/theme.rs` | `src/theme.rs` |
| 14 | Update `lib.rs` exports | `src/lib.rs` |
| 15 | Fix all imports throughout codebase | Various |

### Why This Structure

- **Elm-faithful**: `model/`, `update/`, `view/` are immediately recognizable
- **Clear separation**: `runtime/` isolates platform-specific winit glue from pure logic
- **Shorter names**: `msg.rs` and `cmd.rs` match Elm terminology
- **Tests external**: All `#[test]` code lives in `tests/`, making `src/` cleaner

---

## Phase 1 – Frame/Painter Abstraction

**Goal:** Centralize drawing primitives; stop indexing pixel buffer directly everywhere.  
**Effort:** M (1–3h)  
**User Impact:** None (internal refactor, unblocks everything else)

**Files to modify:**

- `src/view/mod.rs` – Add `Frame` and `TextPainter` structs
- `src/view/overlay.rs` – Migrate to use `Frame` helpers

**Steps:**

1. Add `Frame` struct with `clear()`, `fill_rect()`, `blend_pixel()` methods
2. Add `TextPainter` wrapper for fontdue + glyph cache
3. Wrap `Renderer::render_impl()` to create `Frame` from softbuffer
4. Migrate existing pixel loops to `Frame` methods (status bar → tab bar → gutter → text area)
5. Migrate overlay to take `&mut Frame` instead of raw buffer

```rust
pub struct Frame<'a> {
    pub buffer: &'a mut [u32],
    pub width: usize,
    pub height: usize,
}

impl<'a> Frame<'a> {
    pub fn clear(&mut self, color: u32);
    pub fn fill_rect(&mut self, rect: Rect, color: u32);
    pub fn blend_pixel(&mut self, x: usize, y: usize, color: u32);
}

pub struct TextPainter<'a> {
    font: &'a Font,
    glyph_cache: &'a mut GlyphCache,
}
```

---

## Phase 2 – Widget Extraction & Geometry Centralization

**Goal:** Transform monolithic render function into composable widget functions.  
**Effort:** M–L (3–8h, incremental)  
**User Impact:** Invisible, improves maintainability

**Files to modify:**

- `src/view/mod.rs` – Extract widget functions
- `src/view/editor.rs` – Editor area widgets
- `src/view/chrome.rs` – Tab bar, status bar
- `src/model/editor_area.rs` or new `src/view/geometry.rs` – Centralize geometry helpers

**Steps:**

1. Extract high-level widget renderers:
   - `render_root()` – orchestrates all rendering
   - `render_editor_area()` – groups + splitters
   - `render_editor_group()` – tab bar + editor pane
   - `render_tab_bar()`, `render_gutter()`, `render_text_area()`
   - `render_splitters()`, `render_status_bar()`

2. Centralize geometry helpers (from EDITOR_UI_REFERENCE.md):
   - `compute_visible_lines()`
   - Line/column ↔ pixel conversions
   - Gutter width computation

3. Unify hit-testing geometry between `runtime/input.rs` and `view/`
   - Single source of truth for tab bar rect, text area rect, gutter rect per group

---

## Phase 3 – Basic Modal/Focus System

**Goal:** Add minimal modal overlay + focus capture mechanism.  
**Effort:** M (1–3h)  
**User Impact:** Foundation only (add placeholder modal to test)

**Files to modify:**

- `src/model/ui.rs` – Add `ModalState` enum, extend `UiState`
- `src/msg.rs` – Add `ModalMsg`, extend `UiMsg`
- `src/update/ui.rs` – Handle modal state changes
- `src/runtime/input.rs` – Implement keyboard focus capture
- `src/view/mod.rs` – Add `render_modals()` with dim background

**Model changes:**

```rust
pub enum ModalState {
    CommandPalette(CommandPaletteState),
    GotoLine(GotoLineState),
    FindReplace(FindReplaceState),
}

pub struct UiState {
    pub active_modal: Option<ModalState>,
    // existing fields...
}
```

**Input routing (focus capture):**

```rust
pub fn handle_key(model: &AppModel, event: &KeyEvent) -> Option<Msg> {
    if model.ui.active_modal.is_some() {
        return handle_modal_key(model, event);
    }
    handle_editor_key(model, event)
}
```

**Rendering (layer 2):**

```rust
fn render_modals(frame: &mut Frame, text: &mut TextPainter, ui: &UiState, theme: &Theme) {
    let Some(modal) = &ui.active_modal else { return };

    // 1. Dim background (Zed-style BlockMouse)
    let dim_color = 0x80000000;
    for pixel in frame.buffer.iter_mut() {
        *pixel = blend_pixel(dim_color, *pixel);
    }

    // 2. Render modal content
    match modal { /* ... */ }
}
```

---

## Phase 4 – Command Palette (Full Vertical Slice)

**Goal:** Ship real, useful command palette on modal system.  
**Effort:** L (1–2d)  
**User Impact:** HIGH – visible feature, anchors modal system

**Files to create/modify:**

- `src/cmd.rs` – Add `CommandId` enum, `COMMANDS` registry
- `src/msg.rs` – Add `AppMsg::ExecuteCommand`
- `src/model/ui.rs` – Add `CommandPaletteState`, `CommandPaletteMsg`
- `src/update/app.rs` – Add `execute_command()` dispatcher
- `src/update/ui.rs` – Add `update_command_palette()`
- `src/runtime/input.rs` – Extend modal key routing, add Cmd+P binding
- `src/view/mod.rs` – Add `render_command_palette()`

**Command registry:**

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CommandId {
    ShowCommandPalette,
    GotoLine,
    SaveFile,
    SplitRight,
    SplitDown,
    Undo,
    Redo,
    // ...
}

pub struct Command {
    pub id: CommandId,
    pub label: &'static str,
    pub description: &'static str,
    pub default_keybinding: Option<&'static str>,
}

pub static COMMANDS: &[Command] = &[/* ... */];
```

**Execute command flow:**

```rust
fn execute_command(model: &mut AppModel, cmd_id: CommandId) -> Option<Cmd> {
    match cmd_id {
        CommandId::ShowCommandPalette => { /* open palette */ }
        CommandId::SaveFile => update_app(model, AppMsg::SaveFile),
        CommandId::SplitRight => update_layout(model, LayoutMsg::SplitFocusedHorizontal),
        CommandId::Undo => update_document(model, DocumentMsg::Undo),
        // ...
    }
}
```

---

## Phase 5 – General Overlay/Compositor & Mouse Blocking

**Goal:** Make overlays first-class layers with predictable z-order and event behavior.  
**Effort:** M (1–3h)  
**User Impact:** Subtle – clicks don't leak through modals

**Files to modify:**

- `src/view/mod.rs` – Formalize layer ordering
- `src/model/ui.rs` – Add `ModalRuntimeGeometry` for hit-testing
- `src/runtime/input.rs` – Add mouse blocking for active modal

**Layer stack (conceptual):**

```rust
pub fn render_root(...) {
    render_editor_area(...);    // layer 0
    render_status_bar(...);     // layer 1
    render_modals(...);         // layer 2
    render_perf_overlay(...);   // layer 3 (debug)
}
```

**Mouse blocking:**

```rust
pub fn handle_mouse(model: &AppModel, event: &MouseEvent) -> Option<Msg> {
    if let Some(geom) = &model.ui.active_modal_geometry {
        if point_in_rect(event.position, geom.rect) {
            // Route to modal (future: clicking in search results)
            return Some(Msg::Ui(UiMsg::Modal(/* ... */)));
        } else {
            // Click outside modal → close
            return Some(Msg::Ui(UiMsg::CloseModal));
        }
    }
    handle_editor_mouse(model, event)
}
```

---

## Phase 6 – Goto Line & Find/Replace Modals

**Goal:** Reuse modal infrastructure for high-value overlays.  
**Effort:** L (1–3h each, ~1d total)  
**User Impact:** HIGH – complete basic modal feature set

**Goto Line:**

- Add `GotoLineState` with `input: String`
- Add `GotoLineMsg` variants
- Parse input, move cursor, adjust scroll using EDITOR_UI_REFERENCE helpers
- Render centered smaller modal with single input line

**Find/Replace:**

- Add `FindReplaceState` with `query`, `replacement`, `mode`, `case_sensitive`
- Wire into existing occurrence selection in `update/editor.rs`
- Render as bar at top or modal dialog

**Keyboard shortcuts:**

- `Cmd+G` or `Ctrl+G` → `GotoLine`
- `Cmd+F` → `Find`
- `Cmd+Shift+F` or `Cmd+H` → `Replace`

---

## Phase 7 – Damage Tracking (After UI Stabilizes)

**Goal:** Add modest damage system for partial redraws.  
**Effort:** L–XL (1–3d)  
**User Impact:** Better performance on large files / high-DPI

**Files to modify:**

- `src/view/mod.rs` – Add `FrameDamage` type
- `src/cmd.rs` – Extend `Cmd` with damage hints
- `src/runtime/app.rs` – Store damage in `Renderer`

**Damage types:**

```rust
#[derive(Default, Clone)]
pub struct FrameDamage {
    pub full: bool,
    pub rects: Vec<Rect>,
}

impl FrameDamage {
    pub fn full() -> Self { Self { full: true, rects: Vec::new() } }
    pub fn add_rect(&mut self, rect: Rect) {
        if !self.full { self.rects.push(rect); }
    }
}
```

**Extended Cmd (optional):**

```rust
pub enum Cmd {
    Redraw,
    RedrawRect(Rect),
    RedrawLines { group_id: GroupId, line_range: Range<usize> },
    // ...
}
```

**Guardrail:** Start with `full = true` for all changes; only enable partial after thorough testing. Use feature flag for rollout.

---

## Summary Timeline

| Phase                        | Effort      | Dependencies | Priority                   |
| ---------------------------- | ----------- | ------------ | -------------------------- |
| 0. Elm-Style Restructure     | M (1.5–2h)  | None         | **P0** (do first)          |
| 1. Frame/Painter             | M (1–3h)    | Phase 0      | **P0** (foundation)        |
| 2. Widget Extraction         | M–L (3–8h)  | Phase 1      | **P0** (foundation)        |
| 3. Modal/Focus System        | M (1–3h)    | Phase 1      | **P0** (unblocks features) |
| 4. Command Palette           | L (1–2d)    | Phase 3      | **P1** (high user value)   |
| 5. Compositor/Mouse          | M (1–3h)    | Phase 3      | **P2** (polish)            |
| 6. Goto/Find Modals          | L (1d)      | Phase 4      | **P1** (high user value)   |
| 7. Damage Tracking           | L–XL (1–3d) | Phase 2      | **P3** (optimization)      |

**Total estimated effort:** 2–3 weeks of focused work

---

## Research Summary

Based on analysis of Zed (GPUI), Helix, Alacritty, and Wezterm:

| Project        | Key Patterns Adopted                                                          | Patterns Skipped                                            |
| -------------- | ----------------------------------------------------------------------------- | ----------------------------------------------------------- |
| **Zed (GPUI)** | Modal focus capture, FocusHandle concept, hitbox blocking for overlays        | Full 3-phase render, entity system, DispatchTree complexity |
| **Helix**      | Compositor with layer stack, overlay wrapper, EventResult::Consumed/Ignored   | Full trait-based Component system, callback pattern         |
| **Alacritty**  | Line-level damage tracking, frame damage accumulation, double-buffered damage | Platform-specific Wayland optimizations                     |
| **Wezterm**    | Layer-based z-ordering concept                                                | Quad-based GPU rendering, triple-buffered vertices          |

---

## When to Consider Advanced Path

Revisit more complex designs (Zed-style 3-phase render, trait-based components, plugin architecture) only if:

1. You want **reactive UI widgets** beyond the editor (embedded terminals, complex file trees)
2. You add a **second frontend** (TUI or web), making trait-based design worthwhile
3. Profiling shows **still CPU/GPU bound** after damage tracking on large files

Until then, this phased plan keeps the system simple, testable, and incremental.

---

## References

- [GUI-REVIEW-FINDINGS.md](archived/GUI-REVIEW-FINDINGS.md) – Original comprehensive analysis (archived)
- [EDITOR_UI_REFERENCE.md](EDITOR_UI_REFERENCE.md) – Text editor UI geometry reference
- [ORGANIZATION-CODEBASE.md](archived/ORGANIZATION-CODEBASE.md) – Previous restructuring work
