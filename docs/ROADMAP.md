# Roadmap

Planned features and improvements for rust-editor.

For completed work, see [CHANGELOG.md](CHANGELOG.md).

---

## Recently Completed

### Code Outline Panel ✅

**Design:** [feature/code-outline.md](archived/code-outline.md) | **Completed:** 2026-02-19 (v0.3.19)

Tree-sitter based code outline in the right dock panel:

- **Symbol extraction**: Walks tree-sitter ASTs using range-containment algorithm for code languages, level-based hierarchy for Markdown headings
- **10 languages**: Rust, TypeScript, JavaScript, Python, Go, Java, PHP, C/C++, Markdown, YAML
- **Dock integration**: Rendered in right dock with collapsible nodes, scroll support, click-to-select, double-click-to-jump
- **Worker thread**: Runs alongside syntax highlighting on the syntax worker thread — `OutlineData` returned with `SyntaxHighlights`
- **State management**: `OutlinePanelState` tracks selection, scroll offset, collapsed nodes using `(OutlineKind, OutlineRange)` keys
- **Hit-test fix**: Added `hit_test_docks` before `hit_test_editor` to prevent click-through

### Recent Files ✅

**Completed:** 2026-02-19 (v0.3.19)

Persistent recent files list with Cmd+E modal:

- **Persistence**: `RecentFiles` saved to `~/.config/token-editor/recent.json`, up to 50 entries
- **Modal UI**: Fuzzy filtering with file type icons and "time ago" timestamps
- **MRU ordering**: Most recently used first; Cmd+E then Enter swaps to previous file
- **Tracking**: Files opened via CLI, file dialog, quick open, drag-and-drop, sidebar automatically tracked

### Workspace Management Improvements ✅

**Design:** [feature/workspace-management.md](archived/workspace-management.md) | **Completed:** 2025-12-17 (v0.3.7)

Sidebar resize, file tree keyboard navigation, and focus management:

- **Sidebar resize drag** - Click-and-drag to resize sidebar width with proper cursor feedback
- **File tree keyboard navigation:**
  - Arrow Up/Down to navigate between items
  - Arrow Right expands folders or moves to next item
  - Arrow Left collapses folders or jumps to parent folder
  - Enter opens files or toggles folders
  - Space toggles folder expansion
  - Escape returns focus to editor
- **Focus management system:**
  - New `FocusTarget` enum: `Editor`, `Sidebar`, `Modal`
  - Explicit focus tracking in `UiState.focus`
  - Click transfers focus appropriately; modals capture/release on open/close
  - Global shortcuts (Cmd+Shift+A, Cmd+S, etc.) work regardless of focus state
- **Cursor icon cleanup** - I-beam only over editable text, default pointer elsewhere
- **Workspace root display** - Root folder shown as first item, auto-expanded on open
- **New commands:** `FileTreeSelectPrevious`, `FileTreeSelectNext`, `FileTreeOpenOrToggle`, `FileTreeRefresh`
- **New messages:** `WorkspaceMsg::SelectParent` for parent folder navigation

Remaining: Phase 7 (file system watching), Phase 8 (tab integration, preview tabs, open file highlighting)

### CSV Viewer/Editor Phases 1-2 ✅

**Design:** [feature/csv-editor.md](archived/csv-editor.md) | **Completed:** 2025-12-16 (v0.3.6)

Spreadsheet-like view for CSV/TSV/PSV files with grid rendering, cell navigation, and editing:

**Phase 1 (Read-Only):**
- **Module structure:** `src/csv/` with model.rs, parser.rs, viewport.rs, navigation.rs, render.rs
- **Data model:** `CsvData` with memory-efficient row storage (0xFA delimiter), `CsvState`, `CellPosition`
- **Parsing:** RFC 4180 compliant via `csv` crate, auto-detect delimiter from content or extension
- **ViewMode enum:** Added to `EditorState` for switching between Text and Csv modes
- **Grid rendering:** Row numbers, column headers (A, B, C...), cell grid, selected cell highlight
- **Navigation:** Arrow keys, Tab/Shift+Tab, Page Up/Down, Home/End, Cmd+Home/End
- **Theme support:** `CsvTheme` with header, grid lines, selection, and number colors
- **Command integration:** "Toggle CSV View" in command palette

**Phase 2 (Cell Editing):**
- **Cell editing:** Enter or typing starts editing the selected cell
- **Edit buffer:** `CellEditState` tracks buffer, cursor position, original value
- **Cursor navigation:** Left/Right arrows, Home/End within cell editor
- **Edit operations:** Insert characters, Backspace, Delete
- **Confirm/Cancel:** Enter confirms and moves down, Tab moves to next cell, Escape cancels
- **Document sync:** Edits update underlying text buffer with proper CSV escaping (RFC 4180)
- **Quoted fields:** Values with delimiters, quotes, or newlines properly escaped

Remaining:
- **Phase 3:** Copy support, virtual scrolling for large files, row/column insertion/deletion

### HiDPI Display Switching Fixes ✅

**Design:** [archived/UI-SCALING-REVIEW.md](archived/UI-SCALING-REVIEW.md) | **Completed:** 2025-12-16 (v0.3.4)

Fixed critical issues when switching between displays with different DPI/scale factors:

- **Surface resize on display change** - The softbuffer Surface is now explicitly resized after creation
- **Buffer bounds checking** - `Frame::new` validates buffer size matches dimensions, preventing panics
- **Dynamic tab bar height** - Computed from glyph metrics (`line_height + padding * 2`) instead of hardcoded
- **Scaled metrics throughout** - `resize()` and `set_char_width()` use properly scaled metrics
- **Viewport calculation** - Now accounts for tab bar height when computing visible lines

Key implementation:
- `ScaledMetrics` struct with all DPI-aware layout constants
- `recompute_tab_bar_height_from_line_height()` for font-metric-based sizing
- `Renderer::with_scale_factor()` explicitly resizes Surface after creation
- `ScaleFactorChanged` triggers `Cmd::Batch([ReinitializeRenderer, Redraw])`

### Benchmark Suite Improvements ✅

**Design:** [feature/benchmark-improvements.md](archived/benchmark-improvements.md) | **Completed:** 2025-12-15

Comprehensive audit and improvement of the benchmark suite in `benches/`:

- **Fixed inaccurate benchmarks:**
  - Rewrote `glyph_cache.rs` to use actual fontdue rasterization
  - Created shared `token::rendering::blend_pixel_u8` function
  - Removed duplicated blend code from `rendering.rs` and `support.rs`
- **Added multi-cursor benchmarks:** Setup, insert, delete, move, select word (10-500 cursors)
- **Added large file scaling:** 100k, 500k, 1M line tests in `rope_operations.rs`
- **New `benches/search.rs`:** Literal search, case-insensitive, whole word, visible range
- **New `benches/layout.rs`:** Line width, visible lines, char position, viewport layout
- **New Makefile targets:** `bench-loop`, `bench-search`, `bench-layout`, `bench-multicursor`, `bench-large`

Remaining: Phase 3 (criterion throughput metrics for CI), syntax highlighting benchmarks (when feature ready).

### Syntax Highlighting MVP ✅

**Design:** [feature/syntax-highlighting.md](archived/syntax-highlighting.md) | **Completed:** 2025-12-15

Tree-sitter based syntax highlighting with async background parsing:

- **Core data structures:** `HighlightToken`, `LineHighlights`, `SyntaxHighlights`, `LanguageId`
- **Async parser:** Background worker thread with mpsc channels
- **Debouncing:** 30ms timer prevents parsing on every keystroke
- **Revision tracking:** Staleness checks discard outdated parse results
- **Languages:** 17 languages supported:
  - Phase 1: YAML, Markdown, Rust
  - Phase 2: HTML, CSS, JavaScript
  - Phase 3: TypeScript, TSX, JSON, TOML
  - Phase 4: Python, Go, PHP
  - Phase 5: C, C++, Java, Bash
- **Theme integration:** `SyntaxTheme` struct with VS Code-like default colors
- **Rendering:** Highlighted text rendering with proper tab expansion
- **Auto-trigger:** Parsing on document load and content changes
- **No FOUC:** Old highlights preserved until new ones arrive

**v0.3.3 - Phase 3-5 languages:**
- Added TypeScript/TSX with `tree-sitter-typescript` (0.23)
- Added JSON with `tree-sitter-json` (0.24)
- Added TOML with `tree-sitter-toml-ng` (0.7)
- Added Python with `tree-sitter-python` (0.25) using built-in queries
- Added Go with `tree-sitter-go` (0.25) using built-in queries
- Added PHP with `tree-sitter-php` (0.24) using built-in queries
- Added C with `tree-sitter-c` (0.24) using built-in queries
- Added C++ with `tree-sitter-cpp` (0.23) using built-in queries
- Added Java with `tree-sitter-java` (0.23) using built-in queries
- Added Bash with `tree-sitter-bash` (0.25) using built-in queries
- Upgraded tree-sitter core from 0.24 to 0.25 (ABI compatibility)
- 671 total tests passing

**Performance improvements (v0.3.2):**
- Implemented proper incremental parsing with `tree.edit()` and tree caching
- `ParserState` caches trees per document for incremental updates
- `compute_incremental_edit()` diffs old/new source to generate `InputEdit`
- Added comprehensive benchmark suite (`benches/syntax.rs`)
- Parse times: 67µs (100 lines) to 7.5ms (10000 lines)

**Bug fixes (v0.3.1):**
- Fixed tree-sitter incremental parsing bug causing misaligned highlights
- Fixed flash of unstyled text during re-parsing
- Fixed tab expansion in highlight token rendering

Future phases: Language injection for PHP/HTML/Vue, semantic highlighting via LSP.

### File Operations – Phases 1-5 Complete ✅

**Design:** [feature/file-operations.md](archived/file-operations.md) | **Completed:** 2025-12-15

Comprehensive file handling with dialogs, CLI arguments, and drag-drop feedback:

- **Phase 1:** Refactored `AppModel::new()` with `ViewportGeometry`, `load_config_and_theme()`, `create_initial_session()` helpers
- **Phase 2:** Native file dialogs via `rfd` crate (⌘O Open File, ⇧⌘O Open Folder, ⇧⌘S Save As)
- **Phase 3:** Visual feedback for file drag-hover (`DropState`, overlay rendering)
- **Phase 4:** File validation (`FileOpenError`, binary detection, 50MB size limit)
- **Phase 5:** CLI arguments with `clap` (`--new`, `--wait`, `--line`, `--column`)
- **Duplicate file detection:** Already-open files focus existing tab
- Added `workspace_root: Option<PathBuf>` to `AppModel`
- 703 total tests passing

### Centralized Config Paths ✅

**Completed:** 2025-12-15

Single source of truth for configuration directories:

- New `src/config_paths.rs` module centralizing all path logic
- XDG-compliant paths on Unix (`~/.config/token-editor/`)
- Windows support (`%APPDATA%\token-editor\`)
- Moved inline tests to `tests/config.rs`
- Fixed command palette navigation clamping bug
- 597 total tests passing

### User Theme Configuration ✅

**Completed:** 2025-12-15

Theme loading from user config directories with persistence:

- **Layered loading**: User config → embedded builtins
  - User themes at `~/.config/token-editor/themes/*.yaml`
  - 4 built-in themes (default-dark, fleet-dark, github-dark, github-light)
- **Config persistence**: Theme selection saved to `~/.config/token-editor/config.yaml`
- **ThemePicker improvements**: Sectioned list showing User/Builtin themes
- **New APIs**: `load_theme(id)`, `list_available_themes()`, `ThemeInfo`, `ThemeSource`
- **EditorConfig**: New config module for persistent editor settings
- **Fallback handling**: Falls back to default-dark if saved theme not found
- 15 theme tests, 556 total tests passing

### Configurable Keymapping System ✅

**Design:** [archived/KEYMAPPING_IMPLEMENTATION_PLAN.md](archived/KEYMAPPING_IMPLEMENTATION_PLAN.md) | **Completed:** 2025-12-15

Data-driven keybinding system with YAML configuration:

- **Core module** (`src/keymap/`): Types, commands, bindings, context, YAML parser
- **74 default bindings** in `keymap.yaml` (embedded at compile time)
- **Platform-aware `cmd` modifier**: Maps to Cmd on macOS, Ctrl elsewhere
- **Context-aware bindings**: `when: ["has_selection"]` for conditional activation
- **User configuration**: Layered loading from embedded → user config
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

**Design:** [GUI-CLEANUP.md](archived/GUI-CLEANUP.md) | **Completed:** 2025-12-08

Centralized drawing primitives for cleaner rendering code:

- `Frame` struct wraps pixel buffer with safe drawing methods
- `TextPainter` struct wraps fontdue + glyph cache for text
- All render functions migrated (`render_*_static`, `render_perf_overlay`)
- Removed legacy `draw_text()` and `draw_sparkline()` functions
- Next: Phase 2 (Widget Extraction) or Phase 3 (Modal/Focus System)

### Debug Tracing & Instrumentation ✅

**Design:** [tracing-instrumentation.md](archived/tracing-instrumentation.md) | **Completed:** 2025-12-08

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

### Workspace Management - Remaining Phases

**Design:** [feature/workspace-management.md](archived/workspace-management.md)

Phases 0-6 complete. Remaining work:

**Phase 7 - File System Watching:**
- Add `notify` crate dependency
- Create `src/fs_watcher.rs` module
- Integrate watcher into event loop
- Handle create/modify/delete/rename events
- Auto-refresh tree on external changes

**Phase 8 - Tab Integration:**
- Preview tabs (single-click opens preview, double-click makes permanent)
- Highlight open files in tree
- Sync tree selection with active tab
- Support opening files in new split pane

### Command Palette & Modal System

VS Code-style command palette and modal overlays:

- `Cmd+Shift+P` opens command palette
- Fuzzy search over all available commands
- Go to Line, Find/Replace as modal dialogs
- Focus capture for modal key routing
- Builds on existing overlay system

### GUI Architecture Improvements

**Design:** [GUI-CLEANUP.md](archived/GUI-CLEANUP.md)

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
| Gesture Bindings (alt+alt+key) | [feature/gesture-bindings.md](feature/gesture-bindings.md) |
| Keymap Hot-Reload & Chords | [future/keymap-enhancements.md](future/keymap-enhancements.md) |
| Syntax Highlighting Phase 2+ | [feature/syntax-highlighting.md](archived/syntax-highlighting.md) |

---

## Feature Design Documents

| Feature                     | Status      | Design Doc                                                                               |
| --------------------------- | ----------- |------------------------------------------------------------------------------------------|
| File Operations             | ✅ P1-5     | [feature/file-operations.md](archived/file-operations.md)                                 |
| GUI Cleanup (Frame/Painter) | ✅ Phase 1  | [GUI-CLEANUP.md](archived/GUI-CLEANUP.md)                                                |
| Debug Tracing               | ✅ Complete | [tracing-instrumentation.md](archived/tracing-instrumentation.md)                        |
| Codebase Organization       | ✅ Complete | [archived/ORGANIZATION-CODEBASE.md](archived/ORGANIZATION-CODEBASE.md)                   |
| Multi-Cursor Selection Gaps | ✅ Complete | [archived/MULTI_CURSOR_SELECTION_GAPS.md](archived/MULTI_CURSOR_SELECTION_GAPS.md)       |
| Theming System              | ✅ Complete | [archived/THEMING.md](archived/THEMING.md)                                               |
| Status Bar                  | ✅ Complete | [archived/STATUS_BAR.md](archived/STATUS_BAR.md)                                         |
| Split View                  | ✅ Complete | [archived/SPLIT_VIEW.md](archived/SPLIT_VIEW.md)                                         |
| Selection & Multi-Cursor    | ✅ Complete | [archived/SELECTION_MULTICURSOR.md](archived/SELECTION_MULTICURSOR.md)                   |
| Multi-Cursor Movement       | ✅ Complete | [archived/MULTI_CURSOR_MOVEMENT.md](archived/MULTI_CURSOR_MOVEMENT.md)                   |
| Expand/Shrink Selection     | ✅ Complete | [archived/TEXT-SHRINK-EXPAND-SELECTION.md](archived/TEXT-SHRINK-EXPAND-SELECTION.md)     |
| Configurable Keymapping     | ✅ Complete | [archived/KEYMAPPING_IMPLEMENTATION_PLAN.md](archived/KEYMAPPING_IMPLEMENTATION_PLAN.md) |
| Gesture Bindings            | Planned     | [feature/gesture-bindings.md](feature/gesture-bindings.md)                               |
| Keymap Enhancements         | Future      | [future/keymap-enhancements.md](future/keymap-enhancements.md)                           |
| Workspace Management        | ✅ P0-6     | [feature/workspace-management.md](archived/workspace-management.md)                       |
| Syntax Highlighting         | ✅ MVP      | [feature/syntax-highlighting.md](archived/syntax-highlighting.md)                         |
| CSV Viewer/Editor           | ✅ P1-2     | [feature/csv-editor.md](archived/csv-editor.md)                                           |
| Code Outline Panel          | ✅ Complete | [feature/code-outline.md](archived/code-outline.md)                                       |
| Recent Files                | ✅ Complete | —                                                                                         |

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
│   ├── ui.rs            # UiState (cursor blink, sidebar resize, splitter drag)
│   ├── status_bar.rs    # StatusBar, StatusSegment, sync_status_bar()
│   └── workspace.rs     # Workspace, FileTree, FileNode, FileExtension
├── update/
│   ├── mod.rs           # Pure dispatcher
│   ├── editor.rs        # Cursor, selection, expand/shrink
│   ├── document.rs      # Text editing, undo/redo
│   ├── layout.rs        # Split views, tabs, groups
│   ├── app.rs           # File operations, window resize
│   ├── ui.rs            # Status bar, cursor blink
│   ├── csv.rs           # CSV cell editing
│   ├── dock.rs          # Dock panel toggle, resize
│   ├── outline.rs       # Outline panel selection, navigation
│   ├── preview.rs       # Markdown/HTML preview
│   ├── syntax.rs        # Syntax highlighting updates
│   ├── text_edit.rs     # Unified text editing dispatch
│   └── workspace.rs     # File tree operations
├── view/
│   ├── mod.rs           # Renderer struct, render functions
│   ├── frame.rs         # Frame (pixel buffer) + TextPainter abstractions
│   ├── geometry.rs      # ViewportGeometry, GroupLayout, Rect helpers
│   ├── helpers.rs       # Drawing helper functions
│   ├── hit_test.rs      # HitTarget enum, priority-ordered hit testing
│   └── text_field.rs    # Text field rendering utilities
├── runtime/
│   ├── mod.rs           # Module exports
│   ├── app.rs           # App struct, ApplicationHandler impl
│   ├── input.rs         # handle_key, keyboard→Msg mapping (~220 lines)
│   ├── mouse.rs         # Mouse event handling, hit-test dispatch
│   ├── perf.rs          # PerfStats, debug overlay (debug only)
│   └── webview.rs       # WKWebView integration for preview
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
├── syntax/              # Syntax highlighting system
│   ├── mod.rs           # Public exports, HIGHLIGHT_NAMES
│   ├── highlights.rs    # HighlightToken, LineHighlights, SyntaxHighlights
│   ├── languages.rs     # LanguageId, language detection from extensions
│   └── worker.rs        # SyntaxWorker, async parsing, debouncing
├── csv/                 # CSV viewer/editor mode
│   ├── mod.rs           # Module exports
│   ├── model.rs         # CsvData, CsvState, CellPosition, Delimiter
│   ├── parser.rs        # RFC 4180 parsing with csv crate
│   ├── viewport.rs      # CsvViewport for scrolling
│   ├── navigation.rs    # Cell navigation logic
│   └── render.rs        # Grid rendering helpers
├── editable/            # Unified text editing system
│   ├── mod.rs           # Module exports
│   ├── state.rs         # EditableState<B: TextBuffer>
│   ├── buffer.rs        # StringBuffer implementation
│   ├── cursor.rs        # Cursor operations
│   ├── selection.rs     # Selection operations
│   ├── messages.rs      # TextEditMsg enum
│   ├── history.rs       # EditHistory, undo/redo
│   ├── constraints.rs   # EditConstraints
│   └── context.rs       # EditContext
├── outline/             # Code outline panel
│   ├── mod.rs           # OutlineData, OutlineNode, OutlineKind
│   └── extract.rs       # Tree-sitter AST walking, symbol extraction
├── panel/               # Dock panel system
│   ├── mod.rs           # Module exports
│   └── dock.rs          # DockLayout, DockPanel, PanelId
├── panels/              # Panel implementations
│   ├── mod.rs           # Panel registry
│   └── placeholder.rs   # Placeholder panel rendering
├── markdown/            # Markdown rendering
├── recent_files.rs      # RecentFiles, RecentEntry, persistence
├── fs_watcher.rs        # File system watcher integration
├── debug_dump.rs        # Debug state dumping
├── debug_overlay.rs     # Debug overlay rendering
├── tracing.rs           # Tracing instrumentation
├── config.rs            # EditorConfig, theme persistence
├── config_paths.rs      # Centralized config directory paths
├── theme.rs             # Theme, Color, ThemeInfo, load_theme(), SyntaxTheme
├── overlay.rs           # OverlayConfig, OverlayBounds, render functions
└── util/
    ├── mod.rs           # Module exports, re-exports
    ├── text.rs          # CharType enum, is_punctuation, char_type
    └── file_validation.rs # FileOpenError, validate_file_for_opening, is_likely_binary

themes/
├── dark.yaml            # Default dark theme (VS Code-inspired)
├── dracula.yaml         # Dracula theme
├── fleet-dark.yaml      # JetBrains Fleet dark theme
├── github-dark.yaml     # GitHub dark theme
├── github-light.yaml    # GitHub light theme
├── gruvbox-dark.yaml    # Gruvbox dark theme
├── mocha.yaml           # Catppuccin Mocha theme
├── nord.yaml            # Nord theme
└── tokyo-night.yaml     # Tokyo Night theme

tests/                   # Integration tests
├── common/mod.rs        # Shared test helpers (test_model, etc.)
├── config.rs            # Config paths, editor config, keymap merge
├── cursor_movement.rs   # Cursor movement tests
├── document_cursor.rs   # Document-level cursor tests
├── edge_cases.rs        # Edge case tests
├── editor_area.rs       # Layout, hit testing
├── expand_shrink_selection.rs # Selection expansion tests
├── file_path_commands.rs # File path command tests
├── geometry.rs          # Geometry/viewport tests
├── layout.rs            # Split view tests
├── modal.rs             # Command palette, goto line, find/replace
├── monkey_tests.rs      # Resize edge cases
├── multi_cursor.rs      # Multi-cursor tests
├── overlay.rs           # Anchor, blending
├── scrolling.rs         # Scrolling tests
├── selection.rs         # Selection tests
├── status_bar.rs        # Status bar tests
├── text_editing.rs      # Text editing, multi-cursor undo
├── theme.rs             # Color, YAML parsing, theme loading
└── workspace.rs         # Workspace/file tree tests
```

**Test count:** 1,049 total
