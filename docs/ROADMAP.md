# Roadmap

Planned features and improvements for rust-editor.

For completed work, see [CHANGELOG.md](CHANGELOG.md).
For archived phases, see [archived/old-roadmap-file.md](archived/old-roadmap-file.md).

---

## In Progress

### Multi-Cursor Selection Gaps 🚧

**Design:** [feature/MULTI_CURSOR_SELECTION_GAPS.md](feature/MULTI_CURSOR_SELECTION_GAPS.md) | **Started:** 2025-12-06

Fix remaining selection operations to work with multiple cursors:

- `merge_overlapping_selections()` - merge overlapping/touching selections
- `SelectWord` - select word at each cursor (currently single-cursor only)
- `SelectLine` - select line at each cursor (currently single-cursor only)
- `SelectAll` - properly collapse to single full-document selection
- `ExtendSelectionToPosition` - collapse multi-cursor first, then extend

---

## Recently Completed

### Expand/Shrink Selection ✅

**Design:** [archived/TEXT-SHRINK-EXPAND-SELECTION.md](archived/TEXT-SHRINK-EXPAND-SELECTION.md) | **Completed:** 2025-12-06

Progressive selection expansion with history stack:

- Option+Up: Expand (cursor → word → line → all)
- Option+Down: Shrink (restore previous from history)
- 18 tests in `tests/expand_shrink_selection.rs`

### Multi-Cursor Movement ✅

**Design:** [archived/MULTI_CURSOR_MOVEMENT.md](archived/MULTI_CURSOR_MOVEMENT.md) | **Completed:** 2025-12-06

All cursor movement operations now work with multiple cursors:

- Arrow keys, Home/End, Word navigation, Page Up/Down move ALL cursors
- Shift+movement extends selection for ALL cursors
- Cursor deduplication when cursors collide
- Each cursor preserves its own `desired_column`
- 10 new tests in `tests/cursor_movement.rs`

---

## Planned Features

### File Dropping

**Design:** [feature/handle-file-dropping.md](feature/handle-file-dropping.md)

Drag-and-drop file handling:

- Handle `WindowEvent::DroppedFile` and `HoveredFile` from winit
- Visual overlay during drag hover
- Open files in tabs, switch to existing if already open
- Binary file detection and size limits

### Workspace Management

**Design:** [feature/workspace-management.md](feature/workspace-management.md)

CLI arguments and file tree sidebar:

- Support `red file1 file2` for multiple files
- Support `red ./src` to open directory as workspace
- File tree sidebar with expand/collapse
- File system watching for external changes
- Dependencies: `clap` for CLI, `notify` for FS watching

### Codebase Organization

**Design:** [ORGANIZATION-CODEBASE.md](ORGANIZATION-CODEBASE.md)

Restructure large files for maintainability:

- Convert `update.rs` to `update/` module directory
- Extract from `main.rs`: `view.rs`, `app.rs`, `input.rs`, `perf.rs`
- Target: `main.rs` ~100-200 lines, update submodules ~400-600 lines each

### Undo Coalescing (Future)

Group rapid consecutive edits into single undo entries:

- Time-based grouping (e.g., 300ms threshold)
- Coalesce consecutive character insertions
- Break on cursor movement or pause
- Improves undo ergonomics for normal typing

---

## Feature Design Documents

| Feature | Status | Design Doc |
|---------|--------|------------|
| Multi-Cursor Selection Gaps | 🚧 In Progress | [feature/MULTI_CURSOR_SELECTION_GAPS.md](feature/MULTI_CURSOR_SELECTION_GAPS.md) |
| Theming System | ✅ Complete | [feature/THEMING.md](feature/THEMING.md) |
| Status Bar | ✅ Complete | [feature/STATUS_BAR.md](feature/STATUS_BAR.md) |
| Split View | ✅ Complete | [feature/SPLIT_VIEW.md](feature/SPLIT_VIEW.md) |
| Selection & Multi-Cursor | ✅ Complete | [archived/SELECTION_MULTICURSOR.md](archived/SELECTION_MULTICURSOR.md) |
| Multi-Cursor Movement | ✅ Complete | [archived/MULTI_CURSOR_MOVEMENT.md](archived/MULTI_CURSOR_MOVEMENT.md) |
| Expand/Shrink Selection | ✅ Complete | [archived/TEXT-SHRINK-EXPAND-SELECTION.md](archived/TEXT-SHRINK-EXPAND-SELECTION.md) |
| File Dropping | Planned | [feature/handle-file-dropping.md](feature/handle-file-dropping.md) |
| Workspace Management | Planned | [feature/workspace-management.md](feature/workspace-management.md) |
| Codebase Organization | Planned | [ORGANIZATION-CODEBASE.md](ORGANIZATION-CODEBASE.md) |

---

## Deferred Items (from Split View)

- Cursor adjustment when other views edit same document
- Splitter drag resize (splitters render but not draggable)
- Tab drag-and-drop between groups

---

## Current Module Structure

```
src/
├── main.rs              # Entry point, event loop, App, Renderer, handle_key (~3100 lines)
├── lib.rs               # Library root with module exports
├── model/
│   ├── mod.rs           # AppModel struct, layout constants, accessors (~275 lines)
│   ├── document.rs      # Document struct (buffer, undo/redo, file_path) (~245 lines)
│   ├── editor.rs        # EditorState, Cursor, Selection, Viewport (~660 lines)
│   ├── editor_area.rs   # EditorArea, groups, tabs, layout tree (~770 lines)
│   ├── ui.rs            # UiState (cursor blink, transient messages) (~85 lines)
│   └── status_bar.rs    # StatusBar, StatusSegment, sync_status_bar() (~450 lines)
├── messages.rs          # Msg, EditorMsg, DocumentMsg, UiMsg, LayoutMsg, AppMsg (~260 lines)
├── commands.rs          # Cmd enum (Redraw, SaveFile, LoadFile, Batch) (~55 lines)
├── update.rs            # update() dispatcher + all handlers (~2900 lines)
├── theme.rs             # Theme, Color, TabBarTheme, SplitterTheme (~540 lines)
├── overlay.rs           # OverlayConfig, OverlayBounds, render functions (~285 lines)
└── util.rs              # CharType enum, is_punctuation, char_type (~65 lines)

themes/
├── dark.yaml            # Default dark theme (VS Code-inspired)
├── fleet-dark.yaml      # JetBrains Fleet dark theme
├── github-dark.yaml     # GitHub dark theme
└── github-light.yaml    # GitHub light theme

tests/                   # Integration tests (~5800 lines total)
├── common/mod.rs        # Shared test helpers (test_model, etc.)
├── cursor_movement.rs   # 38 tests
├── text_editing.rs      # 44 tests (includes multi-cursor undo)
├── selection.rs         # 29 tests
├── document_cursor.rs   # 32 tests
├── scrolling.rs         # 33 tests
├── edge_cases.rs        # 9 tests
├── monkey_tests.rs      # 34 tests (resize edge cases)
├── layout.rs            # 47 tests (split view)
└── status_bar.rs        # 47 tests
```

**Test count:** 383 total (24 lib + 14 main + 345 integration)
