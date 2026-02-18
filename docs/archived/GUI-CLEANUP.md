# GUI Cleanup & Architecture Improvement Plan

**Status:** 🚧 In Progress  
**Created:** 2025-12-07  
**Last Updated:** 2025-12-08

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

**Status:** ✅ Mostly Complete (2025-12-07)  
**Goal:** Reorganize codebase to clearly reflect Elm's Model-Update-View architecture; move all tests to `tests/`.  
**Effort:** M (1.5–2h)  
**User Impact:** None (internal restructure)

**Completed:**
- Created `view/` module with `mod.rs` (Renderer) and `frame.rs` (Frame/TextPainter)
- Created `runtime/` module with `app.rs`, `input.rs`, `perf.rs`
- `model/` and `update/` already existed

**Remaining (optional, low priority):**
- Rename `messages.rs` → `msg.rs`
- Rename `commands.rs` → `cmd.rs`  
- Move `overlay.rs` → `view/overlay.rs`

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

**Status:** ✅ Complete (2025-12-08)  
**Goal:** Centralize drawing primitives; stop indexing pixel buffer directly everywhere.  
**Effort:** M (1–3h)  
**User Impact:** None (internal refactor, unblocks everything else)

**Completed:**

1. Created `src/view/frame.rs` with `Frame` and `TextPainter` structs
2. `Frame` provides: `clear()`, `fill_rect()`, `fill_rect_px()`, `set_pixel()`, `get_pixel()`, `blend_pixel()`, `blend_rect()`, `dim()`, `draw_sparkline()`
3. `TextPainter` provides: `draw()`, `measure_width()`
4. Migrated all rendering functions to use Frame/TextPainter:
   - `render_all_groups_static()` - now takes `Frame` + `TextPainter`
   - `render_editor_group_static()` - all pixel ops use Frame methods
   - `render_tab_bar_static()` - uses Frame/TextPainter
   - `render_splitters_static()` - simplified from ~15 lines to 4 lines
   - `render_perf_overlay()` - fully migrated to Frame/TextPainter
   - Status bar rendering - uses Frame/TextPainter
5. Removed standalone `draw_text()` and `draw_sparkline()` functions
6. All 451 tests pass

**API:**

```rust
pub struct Frame<'a> {
    pub buffer: &'a mut [u32],
    pub width: usize,
    pub height: usize,
}

impl<'a> Frame<'a> {
    pub fn clear(&mut self, color: u32);
    pub fn fill_rect(&mut self, rect: Rect, color: u32);
    pub fn fill_rect_px(&mut self, x: usize, y: usize, w: usize, h: usize, color: u32);
    pub fn blend_pixel(&mut self, x: usize, y: usize, color: u32);
    pub fn blend_rect(&mut self, rect: Rect, color: u32);
}

pub struct TextPainter<'a> {
    font: &'a Font,
    glyph_cache: &'a mut GlyphCache,
    font_size: f32,
    ascent: f32,
}

impl<'a> TextPainter<'a> {
    pub fn draw(&mut self, frame: &mut Frame, x: usize, y: usize, text: &str, color: u32);
    pub fn measure_width(&mut self, text: &str) -> f32;
}
```

---

## Phase 2 – Widget Extraction & Geometry Centralization

**Status:** ✅ Complete (2025-12-09)  
**Goal:** Transform monolithic render function into composable widget functions.  
**Effort:** M–L (3–8h, incremental)  
**User Impact:** Invisible, improves maintainability

**Completed:**

1. Created `src/view/geometry.rs` with centralized geometry helpers:
   - `TAB_BAR_HEIGHT`, `TABULATOR_WIDTH` constants
   - `compute_visible_lines()`, `compute_visible_columns()`
   - `expand_tabs_for_display()`, `char_col_to_visual_col()`, `visual_col_to_char_col()`
   - `is_in_status_bar()`, `is_in_tab_bar()`, `is_in_group_tab_bar()`
   - `tab_at_position()`, `pixel_to_cursor()`, `pixel_to_cursor_in_group()`
   - `group_content_rect()`, `group_gutter_rect()`, `group_text_area_rect()`
   - Re-exports `text_start_x`, `gutter_border_x` from model

2. Extracted widget renderers in `src/view/mod.rs`:
   - `render_editor_area_static()` – top-level: all groups + splitters
   - `render_editor_group_static()` – orchestrates tab bar, gutter, text area
   - `render_tab_bar_static()` – tab bar background, tabs, active highlight
   - `render_gutter_static()` – line numbers, gutter border
   - `render_text_area_static()` – current line highlight, selections, text, cursors
   - `render_splitters_static()` – splitter bars between groups
   - `render_status_bar_static()` – status bar with segments and separators

3. Updated `Renderer` hit-testing methods to delegate to `view::geometry`:
   - `is_in_status_bar()`, `is_in_tab_bar()`, `tab_at_position()`, `pixel_to_cursor()`

**Widget Hierarchy:**

```
render() (Renderer entry point)
├── render_editor_area_static()
│   ├── render_editor_group_static() (per group)
│   │   ├── render_tab_bar_static()
│   │   ├── render_text_area_static()
│   │   └── render_gutter_static()
│   └── render_splitters_static()
├── render_status_bar_static()
├── render_perf_overlay() (debug only)
└── render_debug_overlay() (debug only)
```

---

## Phase 3 – Basic Modal/Focus System

**Status:** ✅ Complete (2025-12-09)  
**Goal:** Add minimal modal overlay + focus capture mechanism.  
**Effort:** M (1–3h)  
**User Impact:** Foundation only (add placeholder modal to test)

**Completed:**

1. Added modal state types to `src/model/ui.rs`:
   - `ModalId` enum: `CommandPalette`, `GotoLine`, `FindReplace`
   - `ModalState` enum with per-modal state structs
   - `CommandPaletteState`, `GotoLineState`, `FindReplaceState`
   - `UiState::active_modal: Option<ModalState>` field
   - Helper methods: `has_modal()`, `open_modal()`, `close_modal()`

2. Added modal messages to `src/messages.rs`:
   - `ModalMsg` enum with variants: `Open*`, `Close`, `SetInput`, `InsertChar`, `DeleteBackward`, `SelectPrevious`, `SelectNext`, `Confirm`
   - `UiMsg::Modal(ModalMsg)` and `UiMsg::ToggleModal(ModalId)`

3. Added modal update handler in `src/update/ui.rs`:
   - `update_modal()` handles all modal message variants
   - Goto Line `Confirm` parses input and jumps to line

4. Added focus capture in `src/runtime/input.rs`:
   - `handle_modal_key()` routes keyboard input to modal when active
   - Modal consumes Escape, Enter, arrows, backspace, character input
   - Editor key handling bypassed when modal is open

5. Added modal rendering in `src/view/mod.rs`:
   - `render_modals()` draws modal overlay layer
   - 40% dimmed background via `frame.dim()`
   - Centered modal dialog with title, input field, blinking cursor
   - Rendered after status bar, before debug overlays

6. Added keyboard shortcuts:
   - `Cmd+P` / `Ctrl+P` - Toggle Command Palette
   - `Cmd+G` / `Ctrl+G` - Toggle Go to Line
   - `Cmd+F` / `Ctrl+F` - Toggle Find/Replace
   - `Escape` - Close modal
   - `Enter` - Confirm action

**API (as implemented):**

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
    if model.ui.has_modal() {
        return handle_modal_key(model, event);
    }
    // Normal editor key handling...
}
```

**Rendering (layer 2):**

```rust
fn render_modals(frame: &mut Frame, painter: &mut TextPainter, model: &AppModel, ...) {
    let Some(modal) = &model.ui.active_modal else { return };

    // 1. Dim background (40% black overlay)
    frame.dim(0x66);

    // 2. Draw modal dialog with title, input field, cursor
    // 3. Modal content varies by type (command list, line number, search results)
}
```

---

## Phase 4 – Command Palette (Full Vertical Slice)

**Status:** ✅ Complete (2025-12-09)  
**Goal:** Ship real, useful command palette on modal system.  
**Effort:** L (1–2d)  
**User Impact:** HIGH – visible feature, anchors modal system

**Completed:**

1. Added command registry to `src/commands.rs`:
   - `CommandId` enum with 17 commands
   - `CommandDef` struct with id, label, keybinding
   - `COMMANDS` static registry
   - `filter_commands(query)` for substring matching

2. Added `execute_command()` dispatcher to `src/update/app.rs`:
   - Routes CommandId to appropriate update functions
   - Supports all file, edit, navigation, view commands

3. Updated `render_modals()` in `src/view/mod.rs`:
   - Shows filtered command list below input
   - Highlights selected item
   - Displays keybindings right-aligned
   - Shows "... and N more" for truncated lists

4. Wired Confirm handler in `src/update/ui.rs`:
   - Gets selected command from filtered list
   - Executes via `execute_command()`

**Available Commands:**
NewFile, SaveFile, Undo, Redo, Cut, Copy, Paste, SelectAll, GotoLine, SplitHorizontal, SplitVertical, CloseGroup, NextTab, PrevTab, CloseTab, Find, ShowCommandPalette

---

## Phase 5 – General Overlay/Compositor & Mouse Blocking

**Status:** ✅ Complete (2025-12-09)  
**Goal:** Make overlays first-class layers with predictable z-order and event behavior.  
**Effort:** M (1–3h)  
**User Impact:** Subtle – clicks don't leak through modals

**Completed:**

1. Added modal geometry helpers to `src/view/geometry.rs`:
   - `modal_bounds()` - calculates modal position/size
   - `point_in_modal()` - hit-test for modal area

2. Added mouse blocking to `src/runtime/app.rs`:
   - Click outside modal closes it
   - Click inside modal is consumed (doesn't leak to editor)
   - Uses centralized `point_in_modal()` for hit-testing

3. Refactored `render_modals()` to use `geometry::modal_bounds()`:
   - Single source of truth for modal sizing
   - Consistent between rendering and hit-testing

4. Added `Frame::draw_bordered_rect()` helper:
   - Reduces code duplication for bordered rectangles

**Layer Stack (as implemented):**

```rust
pub fn render(...) {
    render_editor_area(...);    // layer 0
    render_status_bar(...);     // layer 1
    render_modals(...);         // layer 2
    render_perf_overlay(...);   // layer 3 (debug)
    render_debug_overlay(...);  // layer 4 (debug)
}
```

---

## Phase 6 – Goto Line & Find/Replace Modals

**Status:** ✅ Goto Line Complete (2025-12-09), Find/Replace Pending  
**Goal:** Reuse modal infrastructure for high-value overlays.  
**Effort:** L (1–3h each, ~1d total)  
**User Impact:** HIGH – complete basic modal feature set

**Goto Line (✅ Complete):**

- `GotoLineState` with `input: String` (already existed)
- Supports `line:col` format parsing (e.g., "42:10" goes to line 42, column 10)
- Keyboard shortcut: `Cmd+L` / `Ctrl+L`
- Allows digits and colon in input

**Modal Input Improvements (✅ Complete):**

- Word deletion: `Option+Backspace` deletes word backward
- Word navigation: `Option+Left/Right` (placeholder, full cursor tracking TBD)
- Command palette selection resets to 0 when input changes

**Keyboard Shortcut Updates:**

- Command Palette: `Shift+Cmd+A` (was `Cmd+P`)
- Go to Line: `Cmd+L` (was `Cmd+G`)
- Find/Replace: `Cmd+F` (unchanged)

**Theme System Integration (✅ Complete):**

- Added `input_background` and `selection_background` to `OverlayTheme`
- Modal rendering uses themed colors for all UI elements
- All theme YAML files updated with new overlay properties

**Theme Picker (✅ Complete):**

- `ThemePickerState` with `selected_index`
- Lists all built-in themes with current theme checkmark
- Accessible via Command Palette → "Switch Theme..."
- Live theme switching on confirm

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

| Phase                        | Effort      | Dependencies | Priority                   | Status           |
| ---------------------------- | ----------- | ------------ | -------------------------- | ---------------- |
| 0. Elm-Style Restructure     | M (1.5–2h)  | None         | **P0** (do first)          | ✅ Mostly Done   |
| 1. Frame/Painter             | M (1–3h)    | Phase 0      | **P0** (foundation)        | ✅ Complete      |
| 2. Widget Extraction         | M–L (3–8h)  | Phase 1      | **P0** (foundation)        | ✅ Complete      |
| 3. Modal/Focus System        | M (1–3h)    | Phase 2      | **P0** (unblocks features) | ✅ Complete      |
| 4. Command Palette           | L (1–2d)    | Phase 3      | **P1** (high user value)   | ✅ Complete      |
| 5. Compositor/Mouse          | M (1–3h)    | Phase 3      | **P2** (polish)            | ✅ Complete      |
| 6. Goto/Find Modals          | L (1d)      | Phase 4      | **P1** (high user value)   | 🔶 Goto Done     |
| 7. Damage Tracking           | L–XL (1–3d) | Phase 2      | **P3** (optimization)      | Planned          |

**Total estimated effort:** 2–3 weeks of focused work  
**Progress:** Phase 0–5 complete, Phase 6 (Goto Line) complete (~12h total), Find/Replace + Phase 7 remaining

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

- [GUI-REVIEW-FINDINGS.md](GUI-REVIEW-FINDINGS.md) – Original comprehensive analysis (archived)
- [EDITOR_UI_REFERENCE.md](../EDITOR_UI_REFERENCE.md) – Text editor UI geometry reference
- [ORGANIZATION-CODEBASE.md](ORGANIZATION-CODEBASE.md) – Previous restructuring work
