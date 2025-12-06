# Codebase Organization Plan

A pragmatic restructuring of the codebase to improve maintainability as it grows.

---

## Current State (Post Split View Implementation)

| File                       | Lines | Contents                                                                       |
| -------------------------- | ----- | ------------------------------------------------------------------------------ |
| `src/main.rs`              | ~3100 | Renderer, PerfStats, App, ApplicationHandler, handle_key, draw_text, main()    |
| `src/update.rs`            | ~2900 | update dispatcher, update_editor/document/layout/ui/app, cursor/layout helpers |
| `src/model/editor_area.rs` | ~770  | EditorArea, EditorGroup, LayoutNode, SplitContainer, Tab, IDs, layout compute  |
| `src/model/editor.rs`      | ~660  | EditorState, Cursor, Selection, Viewport, OccurrenceState                      |
| `src/theme.rs`             | ~540  | Theme loading, Color, TabBarTheme, SplitterTheme                               |
| `src/model/status_bar.rs`  | ~450  | StatusBar, StatusSegment, sync_status_bar, TransientMessage                    |
| `src/overlay.rs`           | ~285  | Overlay rendering utilities                                                    |
| `src/model/mod.rs`         | ~275  | AppModel, layout constants, accessor methods                                   |
| `src/messages.rs`          | ~260  | Msg, EditorMsg, DocumentMsg, UiMsg, LayoutMsg, AppMsg                          |
| `src/model/document.rs`    | ~245  | Document, EditOperation, Rope buffer                                           |
| `src/model/ui.rs`          | ~85   | UiState (cursor blink, transient messages)                                     |
| `src/util.rs`              | ~65   | CharType enum, is_punctuation, char_type                                       |
| `src/commands.rs`          | ~55   | Cmd enum (Redraw, SaveFile, LoadFile, Batch)                                   |
| `src/lib.rs`               | ~18   | Library root, module exports                                                   |

**Test Coverage:** 351+ tests across 9 test files (~5800 lines)

**Problem**: `main.rs` and `update.rs` are too large and mix concerns.

---

## Proposed Structure

```
src/
  lib.rs                 # Library root (unchanged)
  main.rs                # Entry point only (~100-200 lines)

  # Elm Core (library)
  model/
    mod.rs               # AppModel, layout constants
    document.rs          # Document, EditOperation
    editor.rs            # EditorState, Cursor, Selection, Viewport
    editor_area.rs       # EditorArea, groups, tabs, layout tree
    status_bar.rs        # StatusBar, segments, sync
    ui.rs                # UiState

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
- Tab bar rendering functions
- Any color/geometry helpers for rendering
- `GlyphCache` type alias and glyph caching logic
- Tab expansion helpers (`expand_tabs_for_display`, `char_col_to_visual_col`, `visual_col_to_char_col`)

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
- Cursor blink timer management

### `src/input.rs` (NEW)

Extract from main.rs:

- `fn handle_key(...)` (~400+ lines)
- Key chord handling logic (physical_key support for numpad)
- Mouse-to-message mapping helpers
- Modifier state handling

### `src/update/mod.rs`

Keep only:

```rust
mod editor;
mod document;
mod layout;
mod ui;
mod app;

pub use editor::update_editor;
pub use document::update_document;
pub use layout::update_layout;
pub use ui::update_ui;
pub use app::update_app;

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
- Occurrence/find-next logic (`SelectNextOccurrence`, `SelectAllOccurrences`, etc.)

### `src/update/document.rs`

Move from update.rs:

- `pub fn update_document(...)`
- `delete_selection`, `cursors_in_reverse_order`
- Text insertion/deletion helpers
- Clipboard operations
- Multi-cursor batch edit logic

### `src/update/layout.rs`

Move from update.rs:

- `pub fn update_layout(...)`
- `split_focused_group`, `split_group`
- `insert_split_in_layout`, `remove_group_from_layout`
- `close_group`, `close_tab`, `move_tab`
- `collect_group_ids`, `focus_adjacent_group`
- Tab navigation (`NextTab`, `PrevTab`, `SwitchToTab`)

### `src/update/ui.rs`

Move from update.rs:

- `pub fn update_ui(...)`
- Status bar updates
- Transient message handling
- Cursor blink toggle

### `src/update/app.rs`

Move from update.rs:

- `pub fn update_app(...)`
- File save/load handling
- Window resize handling (updates all editor viewports)

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

3. **Move update_layout + helpers** (start here - cleanest boundaries)
   - Cut `update_layout` and layout helpers into `layout.rs`
   - Add imports, update `mod.rs` to call `layout::update_layout`
   - Run tests

4. **Move update_ui**
   - Smallest handler, quick win

5. **Move update_app**
   - File I/O handlers

6. **Move update_document + helpers**
   - Text manipulation code, clipboard, multi-cursor batch edits

7. **Move update_editor + cursor helpers**
   - Largest chunk, do last

### Phase 2: Extract rendering from main.rs (1-2 hours)

8. **Create view.rs**
   - Add `mod view;` to main.rs
   - Move `Renderer` struct + impl
   - Move `draw_text`, `draw_sparkline`
   - Move tab expansion helpers
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

## Feature File Structure

For planned features, follow these patterns:

### Expand/Shrink Selection (docs/feature/TEXT-SHRINK-EXPAND-SELECTION.md)

Files to modify:

```
src/model/editor.rs          # Add selection_history: Vec<Selection>
src/messages.rs              # Add ExpandSelection, ShrinkSelection to EditorMsg
src/update/editor.rs         # Implement expand/shrink logic (after migration)
  OR src/update.rs           # If before migration
src/input.rs                 # Add Option+Up/Down keybindings (after migration)
  OR src/main.rs             # If before migration
```

### File Dropping (docs/feature/handle-file-dropping.md)

New files:

```
src/model/drop.rs            # DropState struct
```

Files to modify:

```
src/model/mod.rs             # Add mod drop, extend UiState
src/model/ui.rs              # Add drop_state: DropState field
src/messages.rs              # Add DropMsg enum, extend Msg
src/update/app.rs            # Add update_drop() (after migration)
  OR src/update.rs           # If before migration
src/theme.rs                 # Add DropZoneTheme
src/view.rs                  # Add render_drop_overlay() (after migration)
  OR src/main.rs             # If before migration
src/app.rs                   # Handle WindowEvent::DroppedFile/HoveredFile (after migration)
  OR src/main.rs             # If before migration
themes/*.yaml                # Add drop_zone colors
```

### Workspace Management (docs/feature/workspace-management.md)

New files:

```
src/cli.rs                   # CliArgs, StartupConfig, clap integration
src/model/workspace.rs       # Workspace, FileTree, FileNode structs
src/fs_watcher.rs            # FileSystemWatcher using notify crate
```

Files to modify:

```
src/model/mod.rs             # Add mod workspace, extend AppModel
src/messages.rs              # Add WorkspaceMsg enum
src/update/workspace.rs      # NEW: update_workspace() handler
src/update/mod.rs            # Add workspace module dispatch
src/theme.rs                 # Add SidebarTheme, FileTreeTheme
src/view.rs                  # Add render_sidebar(), render_file_tree() (after migration)
  OR src/main.rs             # If before migration
src/input.rs                 # Add Cmd+B, arrow keys for tree (after migration)
  OR src/main.rs             # If before migration
Cargo.toml                   # Add clap, notify dependencies
```

---

## Guidelines

### When adding new code:

- **Model changes** → `src/model/` appropriate file
- **New message type** → `src/messages.rs` + handler in `src/update/` appropriate file
- **New keyboard shortcut** → `src/input.rs` (or `src/main.rs` if before migration)
- **New rendering logic** → `src/view.rs` (or `src/main.rs` if before migration)
- **New UI component** → consider if it's model (state) or view (rendering)

### When a file gets too large:

- `update/editor.rs` > 800 lines → consider `update/editor/cursor.rs`, `update/editor/selection.rs`
- `view.rs` > 1200 lines → consider `view/` module directory with `view/editor.rs`, `view/tabs.rs`
- `model/editor_area.rs` > 1000 lines → consider `model/layout.rs` for LayoutNode/SplitContainer

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
| **CLI/Startup**       | `cli.rs` (future)                  |
| **File Watching**     | `fs_watcher.rs` (future)           |

---

## Not Doing (Yet)

These are deferred until actually needed:

- **Separate crates** (`core`, `gui`) - Only if we add another frontend (TUI, web)
- **Trait abstractions** for rendering - YAGNI for single frontend
- **Plugin system** - Wait for stable feature set first
- **Async runtime** - Not needed for current sync file I/O

---

## Deferred Split View Items

These items were identified during split view implementation but deferred:

- **Cursor adjustment when other views edit same document** - Requires notification system
- **Splitter drag resize** - Splitters render but not draggable yet
- **Tab drag-and-drop between groups** - Low priority, keyboard shortcuts work

---

## Current Keybindings (Split View)

| Shortcut              | Action               |
| --------------------- | -------------------- |
| Numpad 1-4            | Focus group 1-4      |
| Numpad -              | Split horizontal     |
| Numpad +              | Split vertical       |
| Cmd+W                 | Close tab            |
| Option+Cmd+Left/Right | Previous/Next tab    |
| Shift+Cmd+1-4         | Focus group by index |
| Ctrl+Tab              | Focus next group     |
