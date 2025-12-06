# Roadmap

Planned features and improvements for rust-editor.

For completed work, see [CHANGELOG.md](CHANGELOG.md).
For archived phases, see [archived/old-roadmap-file.md](archived/old-roadmap-file.md).

---

## Recently Completed

### Multi-Cursor Batch Undo/Redo ✅

**Completed:** 2025-12-06

All multi-cursor edits now use `EditOperation::Batch` for atomic undo/redo:

- `InsertChar`, `InsertNewline`, `DeleteBackward`, `DeleteForward` batch operations
- Proper cursor restoration on undo/redo
- 6 new tests for multi-cursor undo behavior

### Bugfix & Stabilization Pass ✅

**Completed:** 2025-12-06 | **Archived:** [archived/BUGFIX_PLAN.md](archived/BUGFIX_PLAN.md)

Critical bug fixes from deep codebase analysis:

- Unicode-safe `find_all_occurrences()` - uses char indices not bytes
- Unicode-safe `word_under_cursor()` - clamps to char count
- `SelectNextOccurrence` uses `last_search_offset` for proper cycling
- Cursor/selection invariants enforced via `collapse_selections_to_cursors()`
- Multi-pane viewport resize updates all editors
- `SelectAllOccurrences` fully implemented

### Click+Drag Selection ✅

**Completed:** 2025-12-06

Standard mouse drag selection:

- Left mouse button down starts drag mode
- CursorMoved extends selection while dragging
- Button release ends drag
- Reuses `ExtendSelectionToPosition` message

### Structured Status Bar ✅

**Design:** [feature/STATUS_BAR.md](feature/STATUS_BAR.md) | **Completed:** 2025-12-06

Segment-based status bar system:

- Left/right alignment with separators
- `sync_status_bar()` auto-updates from model
- Transient messages with auto-expiry
- 47 tests in `tests/status_bar.rs`

### Overlay System ✅

**Completed:** 2025-12-06

Reusable overlay rendering (`src/overlay.rs`):

- Anchor positioning (TopLeft, TopRight, etc.)
- Alpha-blended backgrounds
- Optional themed borders
- Theme integration (background, foreground, highlight, warning, error, border)

### Split View Implementation ✅

**Design:** [feature/SPLIT_VIEW.md](feature/SPLIT_VIEW.md) | **Completed:** 2025-12-06

| Phase                         | Status      | Description                                                    |
| ----------------------------- | ----------- | -------------------------------------------------------------- |
| Phase 1: Core Data Structures | ✅ Complete | ID types, EditorArea, Tab, EditorGroup, LayoutNode             |
| Phase 2: Layout System        | ✅ Complete | compute_layout(), group_at_point(), splitter hit testing       |
| Phase 3: Update AppModel      | ✅ Complete | Replace Document/EditorState with EditorArea, accessor methods |
| Phase 4: Messages             | ✅ Complete | LayoutMsg enum, split/close/focus operations, 17 tests         |
| Phase 5: Rendering            | ✅ Complete | Multi-group rendering, tab bars, splitters, focus indicators   |
| Phase 6: Document Sync        | ✅ Complete | Shared document architecture (cursor adjustment deferred)      |
| Phase 7: Keyboard Shortcuts   | ✅ Complete | Cmd+\\, Cmd+W, Cmd+1/2/3/4, Ctrl+Tab                           |

---

## Planned Features

### Expand/Shrink Selection

**Design:** [feature/TEXT-SHRINK-EXPAND-SELECTION.md](feature/TEXT-SHRINK-EXPAND-SELECTION.md)

Progressive selection expansion:

- Option+Up: Expand (word → line → all)
- Option+Down: Shrink (restore previous)
- Selection history stack

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

---

## Feature Design Documents

| Feature                  | Status      | Design Doc                                                                         |
| ------------------------ | ----------- | ---------------------------------------------------------------------------------- |
| Theming System           | ✅ Complete | [feature/THEMING.md](feature/THEMING.md)                                           |
| Selection & Multi-Cursor | ✅ Complete | [feature/SELECTION_MULTICURSOR.md](feature/SELECTION_MULTICURSOR.md)               |
| Status Bar               | ✅ Complete | [feature/STATUS_BAR.md](feature/STATUS_BAR.md)                                     |
| Overlay System           | ✅ Complete | (inline in CHANGELOG)                                                              |
| Split View               | ✅ Complete | [feature/SPLIT_VIEW.md](feature/SPLIT_VIEW.md)                                     |
| Expand/Shrink Selection  | Planned     | [feature/TEXT-SHRINK-EXPAND-SELECTION.md](feature/TEXT-SHRINK-EXPAND-SELECTION.md) |
| File Dropping            | Planned     | [feature/handle-file-dropping.md](feature/handle-file-dropping.md)                 |
| Workspace Management     | Planned     | [feature/workspace-management.md](feature/workspace-management.md)                 |

---

## Current Module Structure

```
src/
├── main.rs              # Entry point, event loop, App struct, Renderer
├── lib.rs               # Library root with module exports
├── model/
│   ├── mod.rs           # AppModel struct (includes Theme), re-exports
│   ├── document.rs      # Document struct (buffer, undo/redo, file_path)
│   ├── editor.rs        # EditorState, Cursor, Selection, Viewport, ScrollRevealMode
│   ├── editor_area.rs   # EditorArea, layout tree, splitters (split view foundation)
│   ├── ui.rs            # UiState (status, cursor blink, loading states)
│   └── status_bar.rs    # StatusBar, StatusSegment, sync_status_bar(), layout
├── messages.rs          # Msg, EditorMsg, DocumentMsg, UiMsg, AppMsg, Direction
├── commands.rs          # Cmd enum (Redraw, SaveFile, LoadFile, Batch)
├── update.rs            # update() dispatcher + update_editor/document/ui/app
├── theme.rs             # Theme, Color, OverlayTheme, YAML theme loading
├── overlay.rs           # OverlayConfig, OverlayBounds, render functions
└── util.rs              # CharType enum, is_punctuation, char_type

themes/
├── dark.yaml            # Default dark theme (VS Code-inspired)
├── fleet-dark.yaml      # JetBrains Fleet dark theme
├── github-dark.yaml     # GitHub dark theme
└── github-light.yaml    # GitHub light theme

tests/
├── common/mod.rs        # Shared test helpers
├── cursor_movement.rs   # 38 tests
├── text_editing.rs      # 44 tests (includes multi-cursor undo)
├── selection.rs         # 29 tests
├── document_cursor.rs   # 32 tests
├── scrolling.rs         # 33 tests
├── edge_cases.rs        # 9 tests
├── monkey_tests.rs      # 34 tests (expanded resize edge cases)
├── layout.rs            # 47 tests
└── status_bar.rs        # 47 tests
```

**Test count:** 351 (24 lib + 14 main + 313 integration)
