# Roadmap

Planned features and improvements for rust-editor.

For completed work, see [CHANGELOG.md](CHANGELOG.md).

---

## Recently Completed

### Configurable Keymapping System ✅

**Design:** [archived/KEYMAPPING_IMPLEMENTATION_PLAN.md](archived/KEYMAPPING_IMPLEMENTATION_PLAN.md) | **Completed:** 2025-12-15

Data-driven keybinding system with YAML configuration:

- **Core module** (`src/keymap/`): Types, commands, bindings, context, YAML parser
- **74 default bindings** in `keymap.yaml` (embedded at compile time)
- **Platform-aware `cmd` modifier**: Maps to Cmd on macOS, Ctrl elsewhere
- **Context-aware bindings**: `when: ["has_selection"]` for conditional activation
- **User configuration**: Layered loading from embedded → project → user config
  - User keymap at `~/.config/token-editor/keymap.yaml`
  - `merge_bindings()` combines base + user with override semantics
  - `command: Unbound` to disable default bindings
- **Bridge integration**: Keymap tried first, input.rs fallback for complex behaviors
- **Chord infrastructure**: `KeyAction::AwaitMore` for multi-key sequences
- **Cleanup**: input.rs reduced 54% (477→220 lines), removed all redundant match arms
- Tab behavior: Indent with selection, insert tab without
- Escape cascade: Multi-cursor → selection → nothing
- Option double-tap preserved for multi-cursor gestures
- 74 keymap tests, 546 total tests passing

### GUI Phase 1 – Frame/Painter Abstraction ✅

**Design:** [GUI-CLEANUP.md](GUI-CLEANUP.md) | **Completed:** 2025-12-08

Centralized drawing primitives for cleaner rendering code:

- `Frame` struct wraps pixel buffer with safe drawing methods
- `TextPainter` struct wraps fontdue + glyph cache for text
- All render functions migrated (`render_*_static`, `render_perf_overlay`)
- Removed legacy `draw_text()` and `draw_sparkline()` functions
- Next: Phase 2 (Widget Extraction) or Phase 3 (Modal/Focus System)

### Debug Tracing & Instrumentation ✅

**Design:** [feature/tracing-instrumentation.md](feature/tracing-instrumentation.md) | **Completed:** 2025-12-08

Debug instrumentation for multi-cursor state transitions:

- `tracing` crate replaces `log`/`env_logger`
- `CursorSnapshot` captures before/after state with diffing
- `update_traced()` wrapper logs message flow and cursor changes
- `assert_invariants_with_context()` for contextual assertion failures
- F8 toggle for in-editor debug overlay
- `make trace` runs with `RUST_LOG=debug`
- Human-readable message names (e.g., `Editor::MoveCursor(Up)` instead of discriminants)

### Multi-Cursor Line Operations ✅

**Completed:** 2025-12-07

Fixed line-based operations to work with all cursors:

- **IndentLines** - now indents lines at all cursor positions
- **UnindentLines** - now unindents lines at all cursor positions
- **DeleteLine** - now deletes lines at all cursor positions
- **AddCursorAbove/Below** - now expands from edge cursors, not primary
- Uses `lines_covered_by_all_cursors()` helper for unique line collection
- 10 new tests in `tests/multi_cursor.rs`

### Test Extraction ✅

**Completed:** 2025-12-07

Extracted inline tests from production code to `tests/` folder:

- `tests/editor_area.rs` - 7 tests (Rect, layout, hit testing)
- `tests/overlay.rs` - 7 tests (anchor positioning, alpha blending)
- `tests/theme.rs` - 10 tests (Color parsing, YAML themes, builtins)
- Tests in `src/main.rs` (33) remain - they test binary-only `handle_key()`

### Codebase Organization ✅

**Design:** [archived/ORGANIZATION-CODEBASE.md](archived/ORGANIZATION-CODEBASE.md) | **Completed:** 2025-12-06

Restructured large files for maintainability:

- Converted `update.rs` (2900 lines) → `update/` module directory with 5 submodules
- Extracted from `main.rs`: `view.rs`, `app.rs`, `input.rs`, `perf.rs`
- `main.rs` now ~20 lines (entry point) + tests
- `update/mod.rs` is a pure 36-line dispatcher

### Multi-Cursor Selection Gaps ✅

**Design:** [archived/MULTI_CURSOR_SELECTION_GAPS.md](archived/MULTI_CURSOR_SELECTION_GAPS.md) | **Completed:** 2025-12-06

Fixed remaining selection operations to work with multiple cursors:

- `merge_overlapping_selections()` - merge overlapping/touching selections
- `SelectWord` - select word at each cursor with automatic merge
- `SelectLine` - select line at each cursor with automatic merge
- `SelectAll` - properly collapses to single full-document selection
- `ExtendSelectionToPosition` - collapses multi-cursor first, then extends

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

### Command Palette & Modal System

VS Code-style command palette and modal overlays:

- `Cmd+Shift+P` opens command palette
- Fuzzy search over all available commands
- Go to Line, Find/Replace as modal dialogs
- Focus capture for modal key routing
- Builds on existing overlay system

### GUI Architecture Improvements

**Design:** [GUI-CLEANUP.md](GUI-CLEANUP.md)

Thin, editor-focused view layer abstractions:

- Frame/Painter abstraction for drawing primitives ✅
- TextPainter for text rendering ✅
- Widget extraction (tab bar, gutter, text area, status bar)
- Centralized coordinate space conversions
- Keep existing winit/softbuffer/fontdue stack

### Undo Coalescing

Group rapid consecutive edits into single undo entries:

- Time-based grouping (e.g., 300ms threshold)
- Coalesce consecutive character insertions
- Break on cursor movement or pause
- Improves undo ergonomics for normal typing

---

## Future Enhancements

| Feature | Design Doc |
|---------|------------|
| Keymap Hot-Reload & Chords | [future/keymap-enhancements.md](future/keymap-enhancements.md) |
| Syntax Highlighting | [feature/syntax-highlighting.md](feature/syntax-highlighting.md) |

---

## Feature Design Documents

| Feature                     | Status      | Design Doc                                                                           |
| --------------------------- | ----------- | ------------------------------------------------------------------------------------ |
| GUI Cleanup (Frame/Painter) | ✅ Phase 1  | [GUI-CLEANUP.md](GUI-CLEANUP.md)                                                     |
| Debug Tracing               | ✅ Complete | [feature/tracing-instrumentation.md](feature/tracing-instrumentation.md)             |
| Codebase Organization       | ✅ Complete | [archived/ORGANIZATION-CODEBASE.md](archived/ORGANIZATION-CODEBASE.md)               |
| Multi-Cursor Selection Gaps | ✅ Complete | [archived/MULTI_CURSOR_SELECTION_GAPS.md](archived/MULTI_CURSOR_SELECTION_GAPS.md)   |
| Theming System              | ✅ Complete | [feature/THEMING.md](feature/THEMING.md)                                             |
| Status Bar                  | ✅ Complete | [archived/STATUS_BAR.md](archived/STATUS_BAR.md)                                     |
| Split View                  | ✅ Complete | [archived/SPLIT_VIEW.md](archived/SPLIT_VIEW.md)                                     |
| Selection & Multi-Cursor    | ✅ Complete | [archived/SELECTION_MULTICURSOR.md](archived/SELECTION_MULTICURSOR.md)               |
| Multi-Cursor Movement       | ✅ Complete | [archived/MULTI_CURSOR_MOVEMENT.md](archived/MULTI_CURSOR_MOVEMENT.md)               |
| Expand/Shrink Selection     | ✅ Complete | [archived/TEXT-SHRINK-EXPAND-SELECTION.md](archived/TEXT-SHRINK-EXPAND-SELECTION.md) |
| Configurable Keymapping     | ✅ Complete | [archived/KEYMAPPING_IMPLEMENTATION_PLAN.md](archived/KEYMAPPING_IMPLEMENTATION_PLAN.md) |
| Keymap Enhancements         | Future      | [future/keymap-enhancements.md](future/keymap-enhancements.md)                       |
| File Dropping               | Planned     | [feature/handle-file-dropping.md](feature/handle-file-dropping.md)                   |
| Workspace Management        | Planned     | [feature/workspace-management.md](feature/workspace-management.md)                   |
| Syntax Highlighting         | Planned     | [feature/syntax-highlighting.md](feature/syntax-highlighting.md)                     |

---

## Deferred Items (from Split View)

- Cursor adjustment when other views edit same document
- Splitter drag resize (splitters render but not draggable)
- Tab drag-and-drop between groups

---

## Current Module Structure

```
src/
├── main.rs              # Entry point (~20 lines) + tests
├── lib.rs               # Library root with module exports
├── model/
│   ├── mod.rs           # AppModel struct, layout constants, accessors
│   ├── document.rs      # Document struct (buffer, undo/redo, file_path)
│   ├── editor.rs        # EditorState, Cursor, Selection, Viewport
│   ├── editor_area.rs   # EditorArea, groups, tabs, layout tree
│   ├── ui.rs            # UiState (cursor blink, transient messages)
│   └── status_bar.rs    # StatusBar, StatusSegment, sync_status_bar()
├── update/
│   ├── mod.rs           # Pure dispatcher
│   ├── editor.rs        # Cursor, selection, expand/shrink
│   ├── document.rs      # Text editing, undo/redo
│   ├── layout.rs        # Split views, tabs, groups
│   ├── app.rs           # File operations, window resize
│   └── ui.rs            # Status bar, cursor blink
├── view/
│   ├── mod.rs           # Renderer struct, render functions
│   └── frame.rs         # Frame (pixel buffer) + TextPainter abstractions
├── runtime/
│   ├── mod.rs           # Module exports
│   ├── app.rs           # App struct, ApplicationHandler impl
│   ├── input.rs         # handle_key, keyboard→Msg mapping (~220 lines)
│   └── perf.rs          # PerfStats, debug overlay (debug only)
├── messages.rs          # Msg, EditorMsg, DocumentMsg, UiMsg, LayoutMsg, AppMsg
├── commands.rs          # Cmd enum (Redraw, SaveFile, LoadFile, Batch)
├── keymap/              # Configurable keybinding system
│   ├── mod.rs           # Module exports
│   ├── types.rs         # KeyCode, Keystroke, Modifiers
│   ├── command.rs       # Command enum with to_msgs()
│   ├── binding.rs       # Keybinding struct
│   ├── context.rs       # KeyContext, Condition
│   ├── config.rs        # YAML parsing
│   ├── keymap.rs        # Keymap lookup engine
│   ├── defaults.rs      # Default bindings loader + user config merge
│   ├── winit_adapter.rs # winit key event conversion
│   └── tests.rs         # 74 keymap tests
├── theme.rs             # Theme, Color, TabBarTheme, SplitterTheme
├── overlay.rs           # OverlayConfig, OverlayBounds, render functions
└── util.rs              # CharType enum, is_punctuation, char_type

themes/
├── dark.yaml            # Default dark theme (VS Code-inspired)
├── fleet-dark.yaml      # JetBrains Fleet dark theme
├── github-dark.yaml     # GitHub dark theme
└── github-light.yaml    # GitHub light theme

tests/                   # Integration tests
├── common/mod.rs        # Shared test helpers (test_model, etc.)
├── cursor_movement.rs   # 48 tests
├── document_cursor.rs   # 32 tests
├── edge_cases.rs        # 9 tests
├── editor_area.rs       # 7 tests (layout, hit testing)
├── expand_shrink_selection.rs # 23 tests
├── layout.rs            # 55 tests (split view)
├── monkey_tests.rs      # 34 tests (resize edge cases)
├── multi_cursor.rs      # 41 tests (1 ignored)
├── overlay.rs           # 7 tests (anchor, blending)
├── scrolling.rs         # 33 tests
├── selection.rs         # 47 tests
├── status_bar.rs        # 47 tests
├── text_editing.rs      # 47 tests (includes multi-cursor undo)
└── theme.rs             # 10 tests (Color, YAML parsing)
```

**Test count:** 546 total (74 keymap + 33 main + 439 integration, 2 ignored)
