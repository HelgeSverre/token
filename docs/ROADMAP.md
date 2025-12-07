# Roadmap

Planned features and improvements for rust-editor.

For completed work, see [CHANGELOG.md](CHANGELOG.md).
For archived phases, see [archived/old-roadmap-file.md](archived/old-roadmap-file.md).

---

## Recently Completed

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
- Tests in `src/main.rs` (14) remain - they test binary-only `handle_key()`

### Codebase Organization ✅

**Design:** [archived/ORGANIZATION-CODEBASE.md](archived/ORGANIZATION-CODEBASE.md) | **Completed:** 2025-12-06

Restructured large files for maintainability:

- Converted `update.rs` (2900 lines) → `update/` module directory with 5 submodules
- Extracted from `main.rs`: `view.rs`, `app.rs`, `input.rs`, `perf.rs`
- `main.rs` now ~20 lines (entry point) + 14 tests
- `update/mod.rs` is a pure 36-line dispatcher

### Multi-Cursor Selection Gaps ✅

**Design:** [feature/MULTI_CURSOR_SELECTION_GAPS.md](archived/MULTI_CURSOR_SELECTION_GAPS.md) | **Completed:** 2025-12-06

Fixed remaining selection operations to work with multiple cursors:

- `merge_overlapping_selections()` - merge overlapping/touching selections
- `SelectWord` - select word at each cursor with automatic merge
- `SelectLine` - select line at each cursor with automatic merge
- `SelectAll` - properly collapses to single full-document selection
- `ExtendSelectionToPosition` - collapses multi-cursor first, then extends
- 18 new tests, total now 401

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

### Configurable Keymapping

**Design:** [feature/KEYMAPPING.md](feature/KEYMAPPING.md)

User-configurable keyboard mapping system:

- TOML config files (`~/.config/token-editor/keymap.toml`)
- Platform-agnostic modifiers (`mod+s` = Cmd on macOS, Ctrl elsewhere)
- Multi-key chord sequences (`Ctrl+K Ctrl+C`)
- Context-aware bindings (editor focus, selection active, etc.)
- Maps to existing Msg enum for Elm-style dispatch

### Command Palette & Modal System

**Design:** [GUI-REVIEW-FINDINGS.md](GUI-REVIEW-FINDINGS.md) (Section 6)

VS Code-style command palette and modal overlays:

- `Cmd+Shift+P` opens command palette
- Fuzzy search over all available commands
- Go to Line, Find/Replace as modal dialogs
- Focus capture for modal key routing
- Builds on existing overlay system

### GUI Architecture Improvements

**Design:** [GUI-REVIEW-FINDINGS.md](GUI-REVIEW-FINDINGS.md)

Thin, editor-focused view layer abstractions:

- Frame/Painter abstraction for drawing primitives
- TextPainter for text rendering
- Widget extraction (tab bar, gutter, text area, status bar)
- Centralized coordinate space conversions
- Keep existing winit/softbuffer/fontdue stack

### Undo Coalescing (Future)

Group rapid consecutive edits into single undo entries:

- Time-based grouping (e.g., 300ms threshold)
- Coalesce consecutive character insertions
- Break on cursor movement or pause
- Improves undo ergonomics for normal typing

---

## Feature Design Documents

| Feature                     | Status      | Design Doc                                                                           |
| --------------------------- | ----------- | ------------------------------------------------------------------------------------ |
| Codebase Organization       | ✅ Complete | [archived/ORGANIZATION-CODEBASE.md](archived/ORGANIZATION-CODEBASE.md)               |
| Multi-Cursor Selection Gaps | ✅ Complete | [feature/MULTI_CURSOR_SELECTION_GAPS.md](archived/MULTI_CURSOR_SELECTION_GAPS.md)     |
| Theming System              | ✅ Complete | [feature/THEMING.md](feature/THEMING.md)                                             |
| Status Bar                  | ✅ Complete | [feature/STATUS_BAR.md](archived/STATUS_BAR.md)                                       |
| Split View                  | ✅ Complete | [feature/SPLIT_VIEW.md](archived/SPLIT_VIEW.md)                                       |
| Selection & Multi-Cursor    | ✅ Complete | [archived/SELECTION_MULTICURSOR.md](archived/SELECTION_MULTICURSOR.md)               |
| Multi-Cursor Movement       | ✅ Complete | [archived/MULTI_CURSOR_MOVEMENT.md](archived/MULTI_CURSOR_MOVEMENT.md)               |
| Expand/Shrink Selection     | ✅ Complete | [archived/TEXT-SHRINK-EXPAND-SELECTION.md](archived/TEXT-SHRINK-EXPAND-SELECTION.md) |
| GUI Architecture            | Planned     | [GUI-REVIEW-FINDINGS.md](GUI-REVIEW-FINDINGS.md)                                     |
| Configurable Keymapping     | Planned     | [feature/KEYMAPPING.md](feature/KEYMAPPING.md)                                       |
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
├── main.rs              # Entry point (~20 lines) + tests (~669 lines)
├── lib.rs               # Library root with module exports
├── app.rs               # App struct, ApplicationHandler impl (~520 lines)
├── input.rs             # handle_key, keyboard→Msg mapping (~402 lines)
├── view.rs              # Renderer, drawing functions (~1072 lines)
├── perf.rs              # PerfStats, debug overlay (debug only, ~406 lines)
├── model/
│   ├── mod.rs           # AppModel struct, layout constants, accessors (~273 lines)
│   ├── document.rs      # Document struct (buffer, undo/redo, file_path) (~245 lines)
│   ├── editor.rs        # EditorState, Cursor, Selection, Viewport (~1131 lines)
│   ├── editor_area.rs   # EditorArea, groups, tabs, layout tree (~895 lines)
│   ├── ui.rs            # UiState (cursor blink, transient messages) (~85 lines)
│   └── status_bar.rs    # StatusBar, StatusSegment, sync_status_bar() (~446 lines)
├── update/              # Update module directory
│   ├── mod.rs           # Pure dispatcher (~36 lines)
│   ├── editor.rs        # Cursor, selection, expand/shrink (~1123 lines)
│   ├── document.rs      # Text editing, undo/redo (~1231 lines)
│   ├── layout.rs        # Split views, tabs, groups (~472 lines)
│   ├── app.rs           # File operations, window resize (~83 lines)
│   └── ui.rs            # Status bar, cursor blink (~55 lines)
├── messages.rs          # Msg, EditorMsg, DocumentMsg, UiMsg, LayoutMsg, AppMsg (~260 lines)
├── commands.rs          # Cmd enum (Redraw, SaveFile, LoadFile, Batch) (~55 lines)
├── theme.rs             # Theme, Color, TabBarTheme, SplitterTheme (~540 lines)
├── overlay.rs           # OverlayConfig, OverlayBounds, render functions (~285 lines)
└── util.rs              # CharType enum, is_punctuation, char_type (~65 lines)

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
├── expand_shrink_selection.rs # 18 tests
├── layout.rs            # 51 tests (split view)
├── monkey_tests.rs      # 34 tests (resize edge cases)
├── multi_cursor.rs      # 25 tests (2 ignored)
├── overlay.rs           # 7 tests (anchor, blending)
├── scrolling.rs         # 33 tests
├── selection.rs         # 47 tests
├── status_bar.rs        # 47 tests
├── text_editing.rs      # 44 tests (includes multi-cursor undo)
└── theme.rs             # 10 tests (Color, YAML parsing)
```

**Test count:** 426 total (14 main + 412 integration, 2 ignored)
