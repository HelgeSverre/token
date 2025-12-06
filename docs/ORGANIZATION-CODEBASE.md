# Codebase Organization Plan

A pragmatic restructuring of the codebase to improve maintainability as it grows.

---

## Current State

| File              | Lines | Contents                                                                                                                |
| ----------------- | ----- | ----------------------------------------------------------------------------------------------------------------------- |
| `src/main.rs`     | ~3000 | Renderer, PerfStats, App, ApplicationHandler, handle_key, draw_text, main()                                             |
| `src/update.rs`   | ~2600 | update dispatcher, update_editor, update_document, update_layout, update_ui, update_app, cursor helpers, layout helpers |
| `src/model/`      | ~2300 | Well-organized: document.rs, editor.rs, editor_area.rs, status_bar.rs, ui.rs                                            |
| `src/theme.rs`    | ~540  | Theme loading and color types                                                                                           |
| `src/overlay.rs`  | ~285  | Overlay rendering utilities                                                                                             |
| `src/messages.rs` | ~260  | Msg, EditorMsg, DocumentMsg, UiMsg, LayoutMsg, AppMsg                                                                   |
| Other             | ~200  | commands.rs, util.rs, lib.rs                                                                                            |

**Problem**: `main.rs` and `update.rs` are too large and mix concerns.

---

## Proposed Structure

```
src/
  lib.rs                 # Library root (unchanged)
  main.rs                # Entry point only (~100-200 lines)

  # Elm Core (library)
  model/
    mod.rs
    document.rs
    editor.rs
    editor_area.rs
    status_bar.rs
    ui.rs

  update/                # NEW: Module directory
    mod.rs               # Dispatcher only
    editor.rs            # EditorMsg handling + cursor helpers
    document.rs          # DocumentMsg handling + text manipulation
    layout.rs            # LayoutMsg handling + split/tab helpers
    ui.rs                # UiMsg handling
    app.rs               # AppMsg handling

  # Binary-only modules (winit/softbuffer frontend)
  view.rs                # NEW: Renderer struct + all drawing functions
  input.rs               # NEW: handle_key + event→Msg mapping
  app.rs                 # NEW: App struct + ApplicationHandler
  perf.rs                # NEW: PerfStats + debug overlay (debug only)

  # Existing modules (unchanged)
  commands.rs
  messages.rs
  theme.rs
  overlay.rs
  util.rs
```

---

## What Goes Where

### `src/main.rs` (after refactor)

Keep only:

- `fn main()` - CLI args, EventLoop creation, App initialization
- `mod` declarations for binary modules
- Minimal wiring code

### `src/view.rs` (NEW)

Extract from main.rs:

- `struct Renderer` and all `impl Renderer` methods
- `render_all_groups_static`, `render_editor_group_static`, `render_splitters_static`
- `draw_text`, `draw_sparkline`
- Any color/geometry helpers for rendering
- `GlyphCache` type alias and glyph caching logic

### `src/perf.rs` (NEW)

Extract from main.rs:

- `struct PerfStats` and its `impl`
- `PERF_HISTORY_SIZE` constant
- `render_perf_overlay` function
- Wrap in `#[cfg(debug_assertions)]`

### `src/app.rs` (NEW - binary module)

Extract from main.rs:

- `struct App` with all fields (model, renderer, channels, timers, mouse state)
- `impl ApplicationHandler for App` (window_event, resumed, etc.)
- Drag state, modifier tracking, mouse handling

### `src/input.rs` (NEW)

Extract from main.rs:

- `fn handle_key(...)` (~400 lines)
- Key chord handling logic
- Mouse-to-message mapping helpers

### `src/update/mod.rs`

Keep only:

```rust
mod editor;
mod document;
mod layout;
mod ui;
mod app;

pub fn update(model: &mut AppModel, msg: Msg) -> Option<Cmd> {
    let result = match msg {
        Msg::Editor(m) => editor::update_editor(model, m),
        Msg::Document(m) => document::update_document(model, m),
        Msg::Ui(m) => ui::update_ui(model, m),
        Msg::Layout(m) => layout::update_layout(model, m),
        Msg::App(m) => app::update_app(model, m),
    };
    sync_status_bar(model);
    result
}
```

### `src/update/editor.rs`

Move from update.rs:

- `pub fn update_editor(...)`
- Cursor movement: `move_cursor_up/down/left/right`, `move_cursor_word_left/right`
- Selection helpers
- Multi-cursor helpers
- Rectangle selection logic
- Occurrence/find-next logic

### `src/update/document.rs`

Move from update.rs:

- `pub fn update_document(...)`
- `delete_selection`, `cursors_in_reverse_order`
- Text insertion/deletion helpers
- Clipboard operations

### `src/update/layout.rs`

Move from update.rs:

- `pub fn update_layout(...)`
- `split_focused_group`, `split_group`
- `insert_split_in_layout`, `remove_group_from_layout`
- `close_group`, `close_tab`, `move_tab`
- `collect_group_ids`, `focus_adjacent_group`

### `src/update/ui.rs`

Move from update.rs:

- `pub fn update_ui(...)`
- Status bar updates
- Transient message handling

### `src/update/app.rs`

Move from update.rs:

- `pub fn update_app(...)`
- File save/load handling
- Window resize handling

---

## Migration Plan

Execute incrementally - each step should compile and pass tests.

### Phase 1: Restructure update/ (1-2 hours)

1. **Convert to module directory**
   ```bash
   mkdir -p src/update
   git mv src/update.rs src/update/mod.rs
   ```
2. **Create skeleton submodules**

   ```bash
   touch src/update/{editor,document,layout,ui,app}.rs
   ```

   Add `mod` declarations to `src/update/mod.rs`

3. **Move update_layout + helpers** (start here - smallest)
   - Cut `update_layout` and layout helpers into `layout.rs`
   - Add imports, update `mod.rs` to call `layout::update_layout`
   - Run tests

4. **Move update_ui**
   - Smallest handler, quick win

5. **Move update_app**
   - File I/O handlers

6. **Move update_document + helpers**
   - Text manipulation code

7. **Move update_editor + cursor helpers**
   - Largest chunk, do last

### Phase 2: Extract rendering from main.rs (1-2 hours)

8. **Create view.rs**
   - Add `mod view;` to main.rs
   - Move `Renderer` struct + impl
   - Move `draw_text`, `draw_sparkline`
   - Move rendering static functions

9. **Create perf.rs**
   - Add `mod perf;` to main.rs
   - Move `PerfStats` + `render_perf_overlay`
   - Keep behind `#[cfg(debug_assertions)]`

### Phase 3: Extract app glue (1 hour)

10. **Create app.rs (binary module)**
    - Move `App` struct
    - Move `ApplicationHandler` impl

11. **Create input.rs**
    - Move `handle_key`
    - Update app.rs to use it

12. **Cleanup main.rs**
    - Should be ~100-200 lines
    - Just CLI, EventLoop, wiring

---

## Guidelines

### When adding new code:

- **Model changes** → `src/model/` appropriate file
- **New message type** → `src/messages.rs` + handler in `src/update/` appropriate file
- **New keyboard shortcut** → `src/input.rs`
- **New rendering logic** → `src/view.rs`
- **New UI component** → consider if it's model (state) or view (rendering)

### When a file gets too large:

- `update/editor.rs` > 800 lines → consider `update/editor_cursor.rs`, `update/editor_selection.rs`
- `view.rs` > 1200 lines → consider `view/` module directory

### Cross-module dependencies:

- Update submodules should NOT call each other directly
- All cross-domain routing goes through the main `update` dispatcher
- Shared helpers used by multiple update modules → keep in `update/mod.rs` or move to `model/`

---

## File Ownership Summary

| Concern               | Files                              |
| --------------------- | ---------------------------------- |
| **State/Model**       | `model/*.rs`                       |
| **Messages**          | `messages.rs`                      |
| **State Updates**     | `update/*.rs`                      |
| **Rendering**         | `view.rs`, `perf.rs`, `overlay.rs` |
| **Input Handling**    | `input.rs`                         |
| **Winit Integration** | `app.rs` (binary), `main.rs`       |
| **Theming**           | `theme.rs`                         |
| **Commands**          | `commands.rs`                      |
| **Utilities**         | `util.rs`                          |

---

## Not Doing (Yet)

These are deferred until actually needed:

- **Separate crates** (`core`, `gui`) - Only if we add another frontend (TUI, web)
- **Trait abstractions** for rendering - YAGNI for single frontend
- **Plugin system** - Wait for stable feature set first
- **Async runtime** - Not needed for current sync file I/O
