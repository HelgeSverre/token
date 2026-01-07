# Changelog

All notable changes to rust-editor are documented in this file.

---

## Unreleased

### Added
- Docs: Updated developer OVERVIEW and archived obsolete docs to reflect recent refactors and module moves.
- Tests: Comprehensive damage computation tests added for redraw/damage logic.

### Changed
- Refactor (app): Introduced specific redraw helpers and tighter damage accumulation to reduce unnecessary full redraws; clearer separation between damage computation and command processing.
- Refactor (layout): Added `redraw_editor`-style helpers to limit redraw scope when layout changes are localized.
- Refactor (ui): Optimized cursor-blink related redraw behavior to prevent selection flicker and avoid unnecessary redraws of unrelated regions.

### Fixed
- Rendering: Minor fixes to selection/cursor rendering behavior related to the cursor-blink optimization and partial redraws.

---


## v0.3.13 - 2026-01-07

### Fixed - CSV Cell Editing

**Column Truncation:**
- Fixed column truncation issue during CSV cell editing
- Cells now properly handle content that exceeds initial column width
- No data loss when editing longer values

**Horizontal Scrolling:**
- Added horizontal scroll support for CSV cell editing
- Long cell content now scrollable within the edit field
- Improved UX for editing cells with long text values

**Log File Discovery:**
- Fixed log file discovery to find most recent dated log file
- Improved config path resolution for logging

### Added - Command Palette Enhancements

**New Commands:**
- Added "Open Folder..." command to open workspace directories
- Added "Quit" command (⌘Q) to close the application
- Added "Toggle Performance Overlay" command (F2) - debug builds only
- Added "Toggle Debug Overlay" command (F8) - debug builds only

**Implementation:**
- Added `Cmd::Quit` for proper application exit flow
- Debug commands use conditional compilation (`#[cfg(debug_assertions)]`)
- Added `DEBUG_COMMANDS` array for debug-only command definitions

### Added - Configuration and Command System

**Configuration Reload:**
- Implemented `ReloadConfiguration` command for hot-reloading config without restart
- Command palette now includes "Reload Configuration" command
- Allows changes to config.yaml and keymap.yaml to take effect immediately

**Application Commands:**
- Added `ApplicationCommand` enum for app-level operations (quit, reload config)
- Added `WorkspaceCommand` enum for workspace operations (toggle sidebar, refresh, etc.)
- Unified command structure with `Cmd::Application` and `Cmd::Workspace` variants

**Theme Picker Enhancements:**
- Implemented theme preview with live updates
- Theme changes are applied immediately while browsing
- Restore original theme on cancel (Escape)
- Confirm theme selection with Enter
- Updated theme picker tests for new preview/restore API

**Configuration Integration:**
- Cursor blink interval now configurable via `editor.cursor_blink_interval_ms` in config.yaml
- Performance overlay toggle now configurable via `editor.show_perf_overlay` in config.yaml
- Runtime respects user configuration preferences

### Fixed - Rendering Bugs

**Selection Highlights Disappearing:**
- Selection highlights were disappearing after ~1 second due to cursor blink optimization
- `render_cursor_lines_only()` now properly renders selection highlights and rectangle selections
- Cursor blink interval changed from 500ms to 600ms

**CSV Cell Editor Cursor:**
- Fixed cursor appearing at top of editor instead of inside cell when editing CSV
- Skip cursor-lines-only optimization for CSV mode (falls back to full render)

### Changed - Build System

**Cargo.toml Enhancements:**
- Added package metadata: authors, readme, keywords, categories, exclude
- Added `default-run = "token"` to handle multiple binaries
- New build profiles:
  - `dev`: Fast compile with `debug = "line-tables-only"`, no debug info for deps
  - `debugging`: Full debug info for debuggers
  - `release`: Thin LTO, `panic = "abort"` for local testing
  - `dist`: Fat LTO, `codegen-units = 1`, stripped for distribution
  - `profiling`: Debug symbols, no LTO for flamegraph/samply

**Makefile Updates:**
- Added `make dist` and `make debugging` targets
- Cross-compilation targets now use `dist` profile for maximum optimization
- `make bundle-macos` uses `dist` profile for smallest binary
- Updated help text

---

## v0.3.12 - 2025-12-20

### Added - Find/Replace Implementation

Complete implementation of the Find/Replace modal (Cmd+F):

**Core Search Functions:**
- `find_all_occurrences_with_options()` with case sensitivity support
- `find_next_occurrence_with_options()` for forward search with wrapping
- `find_prev_occurrence_with_options()` for backward search with wrapping
- Full Unicode support (Greek, Japanese, emoji, accented characters)
- Overlapping match detection

**New Modal Messages:**
- `ToggleFindReplaceField` - Tab to switch between query/replace fields
- `ToggleFindReplaceCaseSensitive` - Toggle case-sensitive search
- `FindNext` / `FindPrevious` - Navigate through matches
- `ReplaceAndFindNext` - Replace current match and find next
- `ReplaceAll` - Replace all occurrences at once

**UX Improvements:**
- Query persists when reopening Cmd+F (like command palette)
- Transient messages show "No matches found" or "Replaced N occurrences"
- Selection highlights the current match
- Cursor scrolls to ensure match is visible

**Test Coverage:**
- 25+ new tests for find functionality
- Case sensitivity tests
- Unicode edge cases (emoji, CJK, Greek letters)
- Overlapping pattern matching
- Empty document and needle handling

---

## v0.3.11 - 2025-12-20

### Fixed - Event Loop Performance

Critical fix for the event loop spinning issue that caused ~7 FPS in multi-split scenarios:

**Root Cause:** `ControlFlow::Poll` was spinning the event loop constantly at ~100% CPU even when idle.

**Fix:** Changed to `ControlFlow::WaitUntil` in `src/runtime/app.rs`:
- Event loop now sleeps until the next cursor blink timer (500ms)
- Wakes immediately for user input, async messages, or file system changes
- Idle CPU usage dropped from ~100% to ~0%
- Multi-split FPS improved from ~7 to 60 (VSync limited)

**Performance Profile (30-second live session):**
| Category | Time |
|----------|------|
| Idle/Waiting | 77.6% |
| Event Handling | 21.5% |
| Rendering | ~0.9% |

### Fixed - Debug Overlay HiDPI Scaling

Fixed hard-coded pixel values in the performance overlay (`src/runtime/perf.rs`) that didn't scale on Retina displays:

- Overlay dimensions now scale based on `line_height` ratio
- Chart widths derived from approximate character width
- Stacked bar calculation fixed with `saturating_sub` to prevent overflow panic

### Added - Profiling Documentation

Enhanced [docs/profiling-guide.md](profiling-guide.md) with recommended workflow:

- Step-by-step process: headless benchmark → sample command → Instruments
- Interpreting `sample` output (mach_msg2_trap, CFRunLoop patterns)
- Common issues & solutions troubleshooting table
- Example healthy profile output

### Changed

- Archived detailed allocation analysis to [performance-analysis-v1.md](performance-analysis-v1.md)
- Created new [performance-analysis.md](performance-analysis.md) with summary of all optimizations

---

## v0.3.10 - 2025-12-19

### Added - File System Watcher & New Languages

**Workspace File Watching:**
- Integrated `notify` crate for real-time file system monitoring
- `FileSystemChange` event for workspace updates
- Automatic refresh when files change externally

**New Language Support:**
- **Scheme** - tree-sitter-scheme parser and highlighting
- **INI** - tree-sitter-ini parser and highlighting  
- **XML** - tree-sitter-xml parser and highlighting

**File Operations:**
- Support for opening files via sidebar
- Support for creating new files
- `Document::new_with_path` constructor with comprehensive tests

**Theme Improvements:**
- Added CSV highlighting colors to all themes
- Restructured CSV theme initialization

**Documentation:**
- Added comprehensive documentation suite
- Updated workspace feature documentation
- Added workspace performance benchmarks and testing guide

### Changed

- Removed legacy highlight query files
- Cleaned up obsolete assets

---

## v0.3.9 - 2025-12-19

### Added - SelectWord for EditableState

- Implemented `select_word()` method for `EditableState` in `src/editable/state.rs`
- Wired `TextEditMsg::SelectWord` in `src/update/text_edit.rs` (previously a TODO stub)
- Added 4 tests for select_word covering middle-of-word, on-space, at-start, and at-end scenarios

### Fixed - Sidebar Folder Indicator Spacing

- Increased spacing between +/- folder indicators and folder names in sidebar
- Changed `text_x` offset from 16px to 20px for better visual separation

---

## v0.3.8 - 2025-12-19

### Added - Unified Text Editing System

Major refactoring to unify text editing across all input contexts (modals, CSV cells) with consistent behavior:

**Core Architecture (`src/editable/` module):**
- **`TextBuffer` / `TextBufferMut` traits** - Abstract over String and Rope buffer backends
- **`StringBuffer`** - Efficient single-line buffer for modals and CSV cells
- **`EditableState<B>`** - Unified state container with cursor, selection, and undo history
- **`EditConstraints`** - Context-specific restrictions (multiline, multi-cursor, char filters)
- **`TextEditMsg` / `MoveTarget`** - Unified message types for all editing operations
- **`EditContext`** - Identifies which input area is being edited
- **`TextFieldRenderer`** - Unified text field rendering with selection support

**Modal Input Improvements:**
- Full cursor navigation in all modals (Left/Right, Home/End)
- Selection support (Shift+Arrow) in command palette, goto line, find/replace
- Word movement (Option+Arrow) in all modals
- Word deletion (Option+Backspace/Delete) in all modals
- Select all (Cmd+A) in all modals
- Undo/redo within modal inputs
- Delete forward (Delete key) now works in modals
- Clipboard integration (Cmd+C/X/V) in all modals

**CSV Cell Editor Enhancements:**
- Migrated `CellEditState` to use `EditableState<StringBuffer>`
- Word movement (Option+Left/Right) while editing cells
- Word deletion (Option+Backspace/Delete) while editing cells
- Select all (Cmd+A) while editing cells
- Undo/redo (Cmd+Z / Cmd+Shift+Z) within cell editing session
- Selection support (Shift+Arrow, Shift+Home/End, Shift+Option+Arrow) while editing cells
- Clipboard integration (Cmd+C/X/V) while editing cells

**New Messages:**
- `CsvMsg::EditCursorWordLeft`, `EditCursorWordRight`
- `CsvMsg::EditDeleteWordBackward`, `EditDeleteWordForward`
- `CsvMsg::EditSelectAll`, `EditUndo`, `EditRedo`
- `CsvMsg::EditCursorLeftWithSelection`, `EditCursorRightWithSelection`, etc. - Selection movement
- `CsvMsg::EditCopy`, `EditCut`, `EditPaste` - Clipboard operations
- `ModalMsg::Copy`, `Cut`, `Paste` - Modal clipboard operations
- `Msg::TextEdit(EditContext, TextEditMsg)` - Unified text editing dispatch

**Main Editor Bridge:**
- `bridge_text_edit_to_editor()` maps `TextEditMsg` to legacy `EditorMsg`/`DocumentMsg`
- Enables unified message system to control main editor via bridge pattern
- All movement, selection, editing, clipboard, undo/redo, and multi-cursor operations bridged
- Allows gradual migration without breaking existing functionality

**New Files:**
- `src/editable/mod.rs` - Module exports
- `src/editable/buffer.rs` - TextBuffer traits and implementations
- `src/editable/cursor.rs` - Position and Cursor types
- `src/editable/selection.rs` - Selection operations
- `src/editable/history.rs` - EditOperation and EditHistory
- `src/editable/constraints.rs` - EditConstraints
- `src/editable/state.rs` - EditableState implementation
- `src/editable/context.rs` - EditContext enum
- `src/editable/messages.rs` - TextEditMsg and MoveTarget
- `src/update/text_edit.rs` - TextEditMsg routing, application, and editor bridge
- `src/view/text_field.rs` - TextFieldRenderer

---

## v0.3.7 - 2025-12-17

### Added - Workspace Management & Focus System

Complete workspace management with sidebar file tree and comprehensive focus handling:

**Sidebar Resize:**
- Click-and-drag to resize sidebar width
- ColResize cursor shown on hover over resize border
- `SidebarResizeState` tracks drag operation
- Width persists in logical pixels for DPI-independence

**File Tree Keyboard Navigation:**
- Arrow Up/Down to navigate between items
- Arrow Right expands folders or moves to next item
- Arrow Left collapses folders or jumps to parent folder (standard file tree behavior)
- Enter opens files or toggles folders
- Space toggles folder expansion
- Escape returns focus to editor

**Workspace Root Display:**
- Workspace root folder is now displayed as the first item in the file tree
- Root folder is auto-expanded when workspace opens
- Root path is canonicalized to ensure proper display name (fixes "." showing as empty)
- Folder expand/collapse indicators: `-` for expanded, `+` for collapsed

**Focus Management System:**
- New `FocusTarget` enum: `Editor`, `Sidebar`, `Modal`
- Explicit focus tracking in `UiState.focus`
- Click on sidebar transfers focus to sidebar
- Click outside sidebar returns focus to editor
- Modals automatically capture/release focus on open/close
- Hiding sidebar while focused returns focus to editor
- `KeyContext.sidebar_focused` and `editor_focused` now reflect actual focus state

**Global Shortcuts:**
- Command palette (Cmd+Shift+A), Save (Cmd+S), Quit (Cmd+Q), and other global shortcuts now work regardless of focus state
- New `Command::is_global()` method identifies shortcuts that bypass focus-based input routing
- Global commands include: ToggleCommandPalette, ToggleGotoLine, ToggleFindReplace, ToggleSidebar, Quit, SaveFile, NewTab, CloseTab

**Cursor Icon Cleanup:**
- I-beam cursor only appears over editable text areas
- Default pointer for sidebar, tab bars, status bar, modals, gutter
- ColResize/RowResize for splitters and sidebar resize border

**Keyboard Routing Fixes:**
- CSV cell editing now properly bypasses keymap (arrow keys work in cell editor)
- Added `AppModel::is_csv_editing()` helper method for consistent checking

**New Commands:**
- `FileTreeSelectPrevious`, `FileTreeSelectNext`
- `FileTreeOpenOrToggle`, `FileTreeRefresh`

**New Messages:**
- `WorkspaceMsg::OpenOrToggle` - opens file or toggles folder
- `WorkspaceMsg::SelectParent` - navigate to parent folder

---

## v0.3.6 - 2025-12-16

### Added - CSV Cell Editing (Phase 2)

Full cell editing support for CSV mode with document synchronization:

**Editing:**
- **Enter or typing** starts editing the selected cell
- **Typing replaces** cell content when starting with a character
- **Edit cursor** navigation with Left/Right arrows, Home/End
- **Backspace/Delete** for character deletion
- **Enter confirms** edit and moves to next row
- **Tab confirms** edit and moves to next cell
- **Escape cancels** edit, restoring original value

**Document Sync:**
- Edits update the underlying text buffer in real-time
- Proper CSV escaping for values with delimiters, quotes, or newlines
- Quoted fields are handled correctly (embedded commas don't break parsing)
- File becomes "modified" after edits, triggers save prompt

**New types:**
- `CellEditState` - tracks edit buffer, cursor position, original value
- `CellEdit` - represents a completed edit for sync/undo

**New messages:**
- `CsvMsg::StartEditing`, `StartEditingWithChar(char)`
- `CsvMsg::ConfirmEdit`, `CancelEdit`
- `CsvMsg::EditInsertChar`, `EditDeleteBackward`, `EditDeleteForward`
- `CsvMsg::EditCursorLeft`, `EditCursorRight`, `EditCursorHome`, `EditCursorEnd`

---

## v0.3.5 - 2025-12-16

### Added - CSV Viewer Mode (Phase 1)

Spreadsheet-like view for CSV, TSV, and PSV files with full navigation support:

**Core Features:**
- **Grid rendering** with row numbers (1, 2, 3...) and column headers (A, B, C...)
- **Cell selection** via mouse click with proper hit-testing
- **Delimiter detection** from file extension or content sniffing
- **Column width auto-calculation** based on content (sampled from first 100 rows)
- **Theme integration** with configurable colors for headers, grid lines, selection
- **Memory-efficient storage** using delimited strings (Tablecruncher pattern)

**Navigation:**
- Arrow keys move cell selection
- Tab/Shift+Tab for next/previous cell with row wrapping
- Home/End for first/last column in row
- Cmd+Home/End for first/last cell in document
- Page Up/Down jumps by viewport height
- Mouse wheel scrolling (vertical and horizontal)
- Click to select cell

**Integration:**
- Command palette: "Toggle CSV View" command
- Escape exits CSV mode
- Status bar shows CSV mode indicator
- Works with split views

**New files:**
- `src/csv/` module with `mod.rs`, `model.rs`, `parser.rs`, `render.rs`, `navigation.rs`, `viewport.rs`
- `samples/large_data.csv` - 10,001-line test file
- `make csv` target for testing

**Messages added:**
- `CsvMsg` enum with Toggle, Move*, NextCell, PrevCell, FirstCell, LastCell, RowStart, RowEnd, PageUp, PageDown, Exit, SelectCell, ScrollVertical, ScrollHorizontal

**Documentation:** See [docs/feature/csv-editor.md](feature/csv-editor.md)

---

## v0.3.4 - 2025-12-16

### Fixed - HiDPI Display Switching

Fixed critical issues when switching between displays with different DPI/scale factors (e.g., moving window between Retina and non-Retina monitors):

- **Surface resize on display change** - The softbuffer Surface is now explicitly resized after creation, fixing incorrect rendering when switching displays
- **Buffer bounds checking in Frame** - Frame::new now validates buffer size matches dimensions, preventing panics during display transitions
- **Dynamic tab bar height** - Tab bar height is now computed from actual glyph metrics (`line_height + padding * 2`) instead of hardcoded values
- **Scaled metrics throughout** - Model's `resize()` and `set_char_width()` now use properly scaled metrics for viewport calculations

### Changed

- `ScaleFactorChanged` now triggers both `ReinitializeRenderer` and `Redraw` commands to ensure immediate visual update
- `reinit_renderer` recomputes tab bar height and viewport geometry after font metrics change
- Viewport visible lines calculation now accounts for tab bar height

### Technical Details

- `Renderer::with_scale_factor` explicitly calls `surface.resize()` after creation
- `Frame::new` adjusts height if buffer is smaller than expected (`width * height`)
- `recompute_tab_bar_height_from_line_height()` added to AppModel for font-metric-based sizing
- `group_content_rect_scaled()` now used in rendering for DPI-aware content areas

---

## v0.3.3 - 2025-12-15

### Added - Phase 3-5 Languages for Syntax Highlighting

Added 11 new languages to syntax highlighting, completing the roadmap phases 3-5:

**Phase 3 (Priority):**
- **TypeScript** - `tree-sitter-typescript` v0.23 with custom queries
- **TSX** - Shares queries with TypeScript, separate parser
- **JSON** - `tree-sitter-json` v0.24 with custom queries
- **TOML** - `tree-sitter-toml-ng` v0.7 with custom queries

**Phase 4 (Common):**
- **Python** - `tree-sitter-python` v0.25 using built-in highlights query
- **Go** - `tree-sitter-go` v0.25 using built-in highlights query
- **PHP** - `tree-sitter-php` v0.24 using built-in highlights query

**Phase 5 (Extended):**
- **C** - `tree-sitter-c` v0.24 using built-in highlights query
- **C++** - `tree-sitter-cpp` v0.23 using built-in highlights query
- **Java** - `tree-sitter-java` v0.23 using built-in highlights query
- **Bash** - `tree-sitter-bash` v0.25 using built-in highlights query

**Total languages now supported: 17** (PlainText excluded)

### Changed

- Upgraded `tree-sitter` from 0.24 to 0.25 for ABI compatibility with newer grammars
- `LanguageId` enum now includes all 17 language variants
- `from_extension()` recognizes extended file extensions (.tsx, .mts, .cts, .pyw, etc.)
- `from_path()` recognizes special filenames (Makefile, Dockerfile, .bashrc, etc.)

### Technical Details

- Custom queries created for TypeScript, JSON, TOML (in `queries/` directory)
- Phase 4-5 languages use built-in `HIGHLIGHTS_QUERY` or `HIGHLIGHT_QUERY` constants
- Query compilation tests added for all 17 languages
- Parsing tests added for all new languages
- 671 total tests passing

---

## v0.3.2 - 2025-12-15

### Added - Incremental Parsing with tree.edit()

Implemented proper incremental parsing for significantly faster syntax highlighting on edits:

**Tree Caching:**
- `ParserState` now caches parsed trees per document in `doc_cache: HashMap<DocumentId, DocParseState>`
- Each `DocParseState` stores the tree, source text, and language
- Cache enables incremental parsing on subsequent edits

**Edit Diffing:**
- `compute_incremental_edit()` computes `InputEdit` by diffing old vs new source
- Finds common prefix/suffix to minimize the edited region
- `byte_to_point()` converts byte offsets to tree-sitter `Point` (row, column)

**Incremental Parse Flow:**
1. On edit, diff cached source against new source
2. Call `tree.edit(&input_edit)` on cached tree
3. Pass edited tree to `parser.parse(source, Some(&old_tree))`
4. Tree-sitter reuses unchanged nodes, only re-parsing the changed region

**Performance Results:**
| File Size | Full Reparse | Incremental |
|-----------|--------------|-------------|
| 100 lines | 67µs | 68µs |
| 500 lines | 330µs | 356µs |
| 1000 lines | 660µs | 728µs |
| 5000 lines | 3.4ms | 3.6ms |

*Note: Incremental is similar speed due to highlight extraction dominating; benefit increases for larger files.*

**New Benchmark Suite** (`benches/syntax.rs`):
- `parse_sample` - Parse small samples for each language
- `parse_only_sample` - Isolated parse time (pre-initialized parser)
- `parse_only_large_rust` - Large file scaling (100-10000 lines)
- `incremental_parse_small_edit` - Incremental with append
- `incremental_parse_middle_edit` - Incremental with mid-file edit
- `full_reparse_comparison` - Fresh parser baseline

**New Makefile Target:**
- `make bench-syntax` - Run syntax highlighting benchmarks

### Changed

- `ParserState` now includes `doc_cache` for tree caching
- `parse_and_highlight()` uses cached trees for incremental parsing
- Added `clear_doc_cache()` method for document cleanup

---

## v0.3.1 - 2025-12-15

### Fixed - Syntax Highlighting Bugs

Critical fixes for the syntax highlighting system:

**Tree-sitter Incremental Parsing Bug:**
- Fixed misaligned highlights after document edits (e.g., pressing Enter)
- Root cause: Passing old cached tree to tree-sitter without calling `tree.edit()`
- Tree-sitter incorrectly reused nodes from stale tree, producing wrong line/column positions
- Fix: Always do full reparse by passing `None` instead of cached tree
- Removed unused tree caching from `ParserState` until proper incremental parsing is implemented

**Flash of Unstyled Text (FOUC):**
- Fixed jarring unstyled flash during syntax re-parsing
- Old highlights are now preserved until new ones arrive
- Revision checks ensure only matching highlights are applied
- Reduced debounce from 50ms to 30ms for snappier updates

**Tab Expansion in Highlighting:**
- Fixed highlight token columns not accounting for tab expansion
- Token character columns are now converted to visual columns using `char_col_to_visual_col()`

### Changed

- `SYNTAX_DEBOUNCE_MS` reduced from 50ms to 30ms
- `ParserState` no longer caches syntax trees (simplified until incremental parsing)

---

## v0.3.0 - 2025-12-15

### Added - Benchmark Suite Improvements

Comprehensive audit and improvement of the benchmark suite:

**Phase 1: Fixed Inaccurate Benchmarks**
- **Rewrote `glyph_cache.rs`** — Now uses actual fontdue rasterization instead of fictional patterns
  - `glyph_rasterize` tests real character rasterization at various font sizes
  - `glyph_cache_realistic_paragraph` tests actual cache hit/miss patterns
  - `glyph_cache_code_sample` tests with code-like content
  - `font_metrics_extraction` tests line width measurement
- **Created `token::rendering::blend_pixel_u8`** — Shared blend function in lib.rs
- **Updated `rendering.rs` and `support.rs`** — Use shared blend function instead of duplicated code

**Phase 2: Added Missing Benchmarks**
- **Multi-cursor benchmarks in `main_loop.rs`:**
  - `multi_cursor_setup` (10, 100, 500 cursors)
  - `multi_cursor_insert_char`, `multi_cursor_delete`, `multi_cursor_move_down`
  - `multi_cursor_select_word`, `multi_cursor_add_cursor_above_below`
- **Large file scaling in `rope_operations.rs`** (100k, 500k, 1M lines):
  - `insert_middle_large_file`, `insert_start_large_file`, `insert_end_large_file`
  - `delete_middle_large_file`, `navigate_large_file`
  - `sequential_inserts_large_file`, `sequential_deletes_large_file`
- **New `benches/search.rs`** — Search operation benchmarks:
  - `search_literal_string`, `search_case_insensitive`, `search_whole_word`
  - `count_occurrences`, `find_first_occurrence`, `search_visible_range`
- **New `benches/layout.rs`** — Text layout benchmarks:
  - `measure_line_width`, `calculate_visible_lines`, `char_position_in_line`
  - `column_from_x_position`, `full_viewport_layout`, `viewport_layout_with_cache`

**New Makefile Targets:**
- `make bench-loop` — Main loop benchmarks
- `make bench-search` — Search benchmarks
- `make bench-layout` — Layout benchmarks
- `make bench-multicursor` — Multi-cursor specific benchmarks
- `make bench-large` — Large file (500k+) benchmarks

---

### Added - Syntax Highlighting MVP

Tree-sitter based syntax highlighting with async background parsing:

**New `src/syntax/` module:**
- `highlights.rs` - `HighlightToken`, `LineHighlights`, `SyntaxHighlights` data structures
- `languages.rs` - `LanguageId` enum with extension-based language detection
- `worker.rs` - `SyntaxWorker` with background thread, mpsc channels, debouncing

**Features:**
- **Async parsing** - Background worker thread prevents UI blocking
- **Debouncing** - 50ms timer prevents parsing on every keystroke
- **Revision tracking** - Staleness checks discard outdated parse results
- **Phase 1 languages** - YAML, Markdown, Rust support
- **Theme integration** - `SyntaxTheme` struct in theme.rs with VS Code-like default colors
- **Auto-trigger** - Parsing on document load and content changes

**New messages:**
- `SyntaxMsg::ParseCompleted` - Delivers highlights from background thread
- `Cmd::ParseSyntax` - Triggers async syntax parsing

**Dependencies added:**
- `tree-sitter = "0.24"`
- `tree-sitter-yaml = "0.6"`
- `tree-sitter-md = "0.3"`
- `tree-sitter-rust = "0.23"`

---

### Added - CLI Arguments with clap

Full command-line argument parsing using clap:

- **New `src/cli.rs` module** with `CliArgs`, `StartupConfig`, `StartupMode`
- **Supported flags:**
  - `token file.rs` - open file
  - `token --new` / `-n` - start with empty buffer
  - `token --line 42 file.rs` - open file at line 42
  - `token --line 42 --column 10 file.rs` - open at line 42, column 10
  - `token --wait` / `-w` - wait mode for git integration (parsed, not yet implemented)
  - `token ./src` - open directory as workspace (sets workspace_root)
- 1-indexed user input converted to 0-indexed internal representation
- 7 new CLI tests

### Added - Duplicate File Detection

Already-open files now focus existing tab instead of creating duplicates:

- Added `find_open_file()` and `is_file_open()` methods to `EditorArea`
- Uses canonicalized paths to handle symlinks and relative paths
- Status bar shows "Switched to: filename" when focusing existing tab
- Integrated into `open_file_in_new_tab()` in layout.rs

### Added - Visual Feedback for File Drag-Hover

File drag-and-drop now shows visual feedback overlay:

- **`DropState` struct** in `UiState` with `hovered_files` and `is_hovering`
- **New `UiMsg` variants:** `FileHovered(PathBuf)`, `FileHoverCancelled`
- Handles `WindowEvent::HoveredFile` and `HoveredFileCancelled`
- Semi-transparent overlay centered in window with "Drop to open: filename" text
- Overlay disappears when drag leaves window

### Changed

- Test count: 703 (was 603)
- Dependencies: Added `clap = "4"` with derive feature

---

## v0.2.1 - 2025-12-15

### Added - Centralized Config Paths

Single source of truth for all configuration directories:

- **New `src/config_paths.rs` module** with all config path functions:
  - `config_dir()` → `~/.config/token-editor/` (Unix) or `%APPDATA%\token-editor\` (Windows)
  - `config_file()` → `config.yaml` path
  - `keymap_file()` → `keymap.yaml` path
  - `themes_dir()` → themes subdirectory
  - `ensure_all_config_dirs()` → creates directory structure
- Respects `XDG_CONFIG_HOME` on Unix if set
- Explicitly uses `~/.config` on macOS (not `~/Library/Application Support`)

### Changed - Test Organization

Moved inline tests to integration test folder:

- Extracted tests from `config.rs`, `config_paths.rs`, `keymap/defaults.rs`
- New `tests/config.rs` with 22 tests covering config paths, editor config, and keymap merge logic
- Total test count: 597

### Fixed - Command Palette Navigation

- Selection index now properly clamped to filtered item count
- Prevents selecting beyond visible items when filter reduces list

### Changed

- Renamed "Open Keybindings" → "Open Keymap" in command palette

---

## 2025-12-15

### Added - Configurable Keymapping System

Complete data-driven keybinding system with YAML configuration:

**Core Module** (`src/keymap/`):
- `KeyCode`, `Keystroke`, `Modifiers` - platform-agnostic key representation
- `Command` enum - 52 bindable editor commands with `to_msgs()` conversion
- `Keybinding` struct - binds keystroke to command with optional conditions
- `Keymap` - lookup engine with context-aware binding resolution
- `KeyContext` - captures editor state (has_selection, has_multiple_cursors, modal_active, editor_focused)
- `Condition` enum - 7 conditions for context-aware bindings
- YAML parser with platform-specific binding support

**Default Bindings** (`keymap.yaml`):
- 74 default bindings embedded at compile time
- Platform-aware `cmd` modifier (Cmd on macOS, Ctrl elsewhere)
- macOS-specific bindings (meta+arrow for line navigation)
- Context-aware Tab (indent with selection, insert tab without)
- Context-aware Escape (collapse multi-cursor → clear selection → nothing)

**Integration** (`src/runtime/app.rs`):
- Keymap tried first for all key events
- Fallback to `input.rs` for complex behaviors (option double-tap, selection collapse on arrows)
- `KeyContext` extracted from model state for binding evaluation

**Commands Added**:
- `DeleteWordForward` - Option+Delete deletes word after cursor
- `InsertTab` - Insert tab character (for Tab without selection)
- `EscapeSmartClear` - No-op fallback for Escape key

### Fixed - Expand Selection Line Behavior

Fixed line selection to exclude the newline character:

**Before**: Expanding selection to line selected through the newline, placing cursor at start of next line
**After**: Line selection ends at last character of line content, cursor stays on same line

- Changed `select_line_at()` to end at last character of line content
- Reordered checks in `expand_selection()` to check `is_line_selection_at` before `is_word_selection_at`
- This fixes single-word lines where word selection equals line selection
- Simplified `is_line_selection_at()` to only check same-line selection ending at line length

### Fixed - Option Double-Tap for Multi-Cursor

Fixed Option double-tap gesture being intercepted by keymap:

**Before**: Keymap would handle Alt+Up/Down before input.rs could check for double-tap
**After**: Keymap lookup is skipped when `option_double_tapped && alt` is true

This preserves the Option+Option+Arrow gesture for adding cursors above/below.

### Changed

- Test count: 539 (was 537)
- 66 keymap-specific tests
- 2 new expand selection tests for line behavior

---

## 2025-12-09

### Added - Command Palette (GUI Phase 4)

Fully functional command palette with searchable command list:

**Command Registry** (`src/commands.rs`):
- `CommandId` enum with 17 commands (file, edit, navigation, view operations)
- `CommandDef` struct with id, label, keybinding
- `COMMANDS` static registry with all available commands
- `filter_commands(query)` for fuzzy substring matching

**Command Execution** (`src/update/app.rs`):
- `execute_command(model, cmd_id)` dispatches to appropriate update functions
- Commands routed through existing message handlers (DocumentMsg, LayoutMsg, etc.)

**Command Palette UI** (`src/view/mod.rs`):
- Filtered command list displayed below input field
- Selected item highlighted with selection background
- Keybindings shown right-aligned (dimmed)
- Up/Down arrows navigate list
- Enter executes selected command
- Shows "... and N more" when list is truncated

**Available Commands**:
New File, Save File, Undo, Redo, Cut, Copy, Paste, Select All, Go to Line, Split Editor Right/Down, Close Editor Group, Next/Prev Tab, Close Tab, Find, Show Command Palette

---

### Added - Mouse Blocking & Compositor (GUI Phase 5)

Modal overlays now properly capture mouse events:

**Mouse Blocking** (`src/runtime/app.rs`):
- Click outside modal closes it
- Click inside modal is consumed (doesn't leak to editor)
- Uses centralized `geometry::point_in_modal()` for hit-testing

**Modal Geometry** (`src/view/geometry.rs`):
- `modal_bounds()` - calculates modal position and size
- `point_in_modal()` - hit-test for modal area
- Shared between rendering and input handling

---

### Added - Frame Helpers

New drawing primitives for cleaner rendering:

- `Frame::draw_bordered_rect()` - fill with 1px border in single call
- Reduces code duplication in modal rendering

---

### Added - Basic Modal/Focus System (GUI Phase 3)

Added minimal modal overlay infrastructure with focus capture:

**Modal State Types** (`src/model/ui.rs`):
- `ModalId` enum: `CommandPalette`, `GotoLine`, `FindReplace`
- `ModalState` enum with per-modal state structs
- `CommandPaletteState`, `GotoLineState`, `FindReplaceState`
- `UiState::active_modal: Option<ModalState>` field
- Helper methods: `has_modal()`, `open_modal()`, `close_modal()`

**Modal Messages** (`src/messages.rs`):
- `ModalMsg` enum with variants: `Open*`, `Close`, `SetInput`, `InsertChar`, `DeleteBackward`, `SelectPrevious`, `SelectNext`, `Confirm`
- `UiMsg::Modal(ModalMsg)` and `UiMsg::ToggleModal(ModalId)`

**Focus Capture** (`src/runtime/input.rs`):
- `handle_modal_key()` routes all keyboard input to modal when active
- Modal consumes Escape, Enter, arrows, backspace, and character input
- Editor key handling bypassed when modal is open

**Modal Rendering** (`src/view/mod.rs`):
- `render_modals()` draws modal overlay layer
- 40% dimmed background over entire window
- Centered modal dialog with title, input field, and blinking cursor
- Rendered after status bar, before debug overlays

**Keyboard Shortcuts**:
- `Cmd+P` / `Ctrl+P` - Toggle Command Palette
- `Cmd+G` / `Ctrl+G` - Toggle Go to Line
- `Cmd+F` / `Ctrl+F` - Toggle Find/Replace
- `Escape` - Close modal
- `Enter` - Confirm (Go to Line jumps to entered line number)

#### Benefits

- Foundation for Command Palette (Phase 4) and other overlays
- Focus capture prevents editor input while modal is open
- Modals are first-class layers with proper z-ordering
- Clean separation of modal state, messages, and rendering

---

### Added - Widget Extraction & Geometry Centralization (GUI Phase 2)

Transformed monolithic render function into composable widget functions:

**New `src/view/geometry.rs` module** - centralized geometry helpers:
- Constants: `TAB_BAR_HEIGHT`, `TABULATOR_WIDTH`
- Viewport sizing: `compute_visible_lines()`, `compute_visible_columns()`
- Tab handling: `expand_tabs_for_display()`, `char_col_to_visual_col()`, `visual_col_to_char_col()`
- Hit-testing: `is_in_status_bar()`, `is_in_tab_bar()`, `tab_at_position()`, `pixel_to_cursor()`
- Layout helpers: `group_content_rect()`, `group_gutter_rect()`, `group_text_area_rect()`

**Extracted widget renderers:**
- `render_editor_area_static()` - top-level: all groups + splitters
- `render_editor_group_static()` - orchestrates tab bar, gutter, text area
- `render_tab_bar_static()` - tab bar background, tabs, active highlight
- `render_gutter_static()` - line numbers, gutter border
- `render_text_area_static()` - current line highlight, selections, text, cursors
- `render_splitters_static()` - splitter bars between groups
- `render_status_bar_static()` - status bar with segments and separators

**Updated hit-testing** to delegate to `view::geometry`:
- `Renderer::is_in_status_bar()`, `is_in_tab_bar()`, `tab_at_position()`, `pixel_to_cursor()`

#### Benefits

- Clear widget hierarchy with single responsibility per function
- Centralized geometry calculations - single source of truth
- Hit-testing and rendering share the same geometry logic
- Prepared for future modal system (Phase 3)

---

## 2025-12-08

### Fixed - Debug Tracing Message Names

Fixed `msg_type_name()` to show human-readable variant names instead of opaque discriminants:

**Before:** `msg=Ui::Discriminant(1)`, `msg=Document::Discriminant(0)`  
**After:** `msg=Ui::BlinkCursor`, `msg=Document::InsertChar('a')`

- Changed from `std::mem::discriminant()` to Debug formatting (`{:?}`)
- Includes variant arguments which helps debug multi-cursor/selection issues
- Zero dependencies, zero maintenance overhead

### Added - Frame/Painter Abstraction (GUI Phase 1)

Centralized drawing primitives for cleaner, more maintainable rendering code:

- **`Frame` struct** (`src/view/frame.rs`) - wraps pixel buffer with safe drawing methods:
  - `clear()`, `fill_rect()`, `fill_rect_px()` - solid color fills
  - `set_pixel()`, `get_pixel()` - single pixel operations
  - `blend_pixel()`, `blend_rect()` - alpha blending
  - `dim()` - modal background dimming
  - `draw_sparkline()` - debug chart rendering

- **`TextPainter` struct** - wraps fontdue + glyph cache:
  - `draw()` - render text at position with color
  - `measure_width()` - calculate text width in pixels

- **Migrated all rendering functions** to use Frame/TextPainter:
  - `render_all_groups_static()` - takes Frame + TextPainter
  - `render_editor_group_static()` - all pixel ops use Frame
  - `render_tab_bar_static()` - uses Frame/TextPainter
  - `render_splitters_static()` - simplified from ~15 lines to 4 lines
  - `render_perf_overlay()` - fully migrated
  - Status bar rendering - uses Frame/TextPainter

- **Removed legacy functions**: `draw_text()`, `draw_sparkline()` standalone functions

#### Benefits

- Simpler APIs with automatic bounds checking
- Fewer parameters passed through render functions
- Consistent abstraction for all pixel operations
- Prepared for future widget extraction (Phase 2)

---

## 2025-12-07

### Added - File Dropping & Multi-File Arguments

Open multiple files from command line or by drag-and-drop:

- **Multi-file CLI**: `cargo run -- file1.rs file2.rs file3.rs` opens all files as tabs
- **Drag-and-drop**: Drop files onto the window to open them in new tabs
- **LayoutMsg::OpenFileInNewTab(PathBuf)**: New message for opening files as tabs
- First file becomes active tab, additional files added to same group

#### Implementation

- `src/messages.rs`: Added `LayoutMsg::OpenFileInNewTab(PathBuf)`
- `src/update/layout.rs`: Added `open_file_in_new_tab()` handler
- `src/app.rs`: Handle `WindowEvent::DroppedFile` events
- `src/main.rs`: Parse all CLI args as file paths (removed TODO)
- `src/model/mod.rs`: `AppModel::new()` now accepts `Vec<PathBuf>`

### Fixed - Tab Click to Switch

- Clicking on tabs now switches to the clicked tab
- Added `Renderer::tab_at_position()` for tab hit-testing
- Tab bar click handler now detects tab index and dispatches `SwitchToTab`

### Refactored - Document Display Name

- Added `Document::display_name()` method centralizing naming logic
- Added `tab_title()` helper in view.rs to avoid duplication
- Tab bar rendering and hit-testing now use the same helper
- Keeps numbered untitled names (Untitled, Untitled-2, etc.) for UX

---

## 2025-12-07

### Changed - Test Extraction

Extracted inline tests from production code to `tests/` folder:

- `tests/editor_area.rs` - 7 tests (Rect, layout, hit testing)
- `tests/overlay.rs` - 7 tests (anchor positioning, alpha blending)
- `tests/theme.rs` - 10 tests (Color parsing, YAML themes, builtins)

Tests remaining in `src/main.rs` (14 tests) cannot be moved - they test `handle_key()` which is binary-only code.

### Fixed - Multi-Cursor Duplicate

- **Duplicate** (Cmd+D) now works on all cursors, not just primary
- Line duplication: duplicates line at each cursor position
- Selection duplication: duplicates selected text at each cursor
- Processes in reverse document order, records as Batch for proper undo
- 3 new tests in `tests/multi_cursor.rs`

### Fixed - Multi-Cursor Indent/Unindent

- **IndentLines** now works on all cursors/selections, not just primary
- **UnindentLines** now works on all cursors/selections, not just primary
- Both use `lines_covered_by_all_cursors()` helper for unique line collection
- Proper Batch undo/redo with cursor state restoration
- 5 new tests in `tests/multi_cursor.rs`

### Fixed - Multi-Cursor DeleteLine

- **DeleteLine** (Cmd+Backspace) now deletes lines at all cursor positions
- Uses same `lines_covered_by_all_cursors()` pattern
- Collapses to single cursor after deletion
- 3 new tests in `tests/multi_cursor.rs`

### Fixed - Multi-Cursor Edge Expansion

- **AddCursorAbove** now expands from top-most cursor, not primary
- **AddCursorBelow** now expands from bottom-most cursor, not primary
- Added `top_cursor()`, `bottom_cursor()`, `edge_cursor_vertical()` helpers
- 2 new tests in `tests/multi_cursor.rs`

---

## 2025-12-07

### Fixed - Multi-Cursor Selection Rendering & Cmd+J

Fixed three bugs in multi-cursor functionality:

#### Selection Rendering

- **Fixed**: All selections now render, not just the primary selection
- Previously only `editor.selection()` (primary) was rendered
- Now iterates over `editor.selections` to render all multi-cursor selections

#### Cmd+J (SelectNextOccurrence)

- **Fixed**: First invocation now searches from current selection position, not offset 0
- **Fixed**: Loop now skips already-selected occurrences instead of doing nothing
- Shows "All occurrences selected" message when all are already selected
- Proper wrap-around detection to avoid infinite loops

---

## 2025-12-06

### Changed - Codebase Organization

Major restructuring of large files for improved maintainability:

#### Update Module (`update/`)

Converted monolithic `update.rs` (2900 lines) into a module directory:

| File          | Lines | Contents                                  |
| ------------- | ----- | ----------------------------------------- |
| `mod.rs`      | 36    | Pure dispatcher only                      |
| `editor.rs`   | 1123  | Cursor movement, selection, expand/shrink |
| `document.rs` | 1231  | Text editing, undo/redo helpers           |
| `layout.rs`   | 472   | Split views, tabs, groups                 |
| `app.rs`      | 83    | File operations, window resize            |
| `ui.rs`       | 55    | Status bar, cursor blink                  |

#### Binary Modules

Extracted from `main.rs` (was 3100 lines, now ~20 lines entry + 669 tests):

| File       | Lines | Contents                                 |
| ---------- | ----- | ---------------------------------------- |
| `app.rs`   | 520   | App struct, ApplicationHandler impl      |
| `input.rs` | 402   | handle_key, keyboard→Msg mapping         |
| `view.rs`  | 1072  | Renderer, drawing functions, tab helpers |
| `perf.rs`  | 406   | PerfStats, debug overlay (debug only)    |

#### Benefits

- `main.rs` is now a clean ~20 line entry point
- `update/mod.rs` is a pure 36-line dispatcher
- Clear separation: Model → Messages → Update → View
- Prepared for future Frame/TextPainter abstraction
- All 401 tests pass

---

## 2025-12-06

### Added - Multi-Cursor Selection Gaps

Fixed remaining selection operations to work with multiple cursors:

- **`merge_overlapping_selections()`**: New method in `EditorState` that merges overlapping or touching selections into single selections, maintaining cursor/selection invariants
- **`SelectWord`**: Now operates on ALL cursors, selecting word at each position, then merging overlaps
- **`SelectLine`**: Now operates on ALL cursors, selecting line at each position, then merging overlaps
- **`SelectAll`**: Properly collapses to single cursor + single full-document selection
- **`ExtendSelectionToPosition`**: Collapses multi-cursor first, then extends from primary cursor
- **`word_under_cursor_at(doc, idx)`**: New helper refactored from `word_under_cursor()` for per-cursor word detection

#### Tests Added

- 6 tests for `merge_overlapping_selections()` (non-overlapping, overlapping, touching, multiline, duplicates, invariants)
- 4 tests for `SelectWord` (single cursor, whitespace, multi-cursor different words, same word merges)
- 4 tests for `SelectLine` (single cursor, last line, multi-cursor different lines, same line merges)
- 2 tests for `SelectAll` (single cursor, collapses multi-cursor)
- 2 tests for `ExtendSelectionToPosition` (single cursor, collapses multi-cursor)

### Changed

- Test count: 401 (was 383)
- Added 18 new selection tests

---

## 2025-12-06

### Added - Expand/Shrink Selection (Already Implemented)

Progressive selection expansion with history stack:

- **Option+Up**: Expand selection (cursor → word → line → all)
- **Option+Down**: Shrink selection (restore previous from history)
- Selection history stack in `EditorState.selection_history`
- 18 tests in `tests/expand_shrink_selection.rs`

_(Feature was already implemented, roadmap updated to reflect completion)_

### Added - Multi-Cursor Movement

All cursor movement operations now work with multiple cursors:

- **Arrow keys** (Up/Down/Left/Right) move ALL cursors simultaneously
- **Home/End** moves all cursors to their respective line starts/ends (smart behavior preserved)
- **Word navigation** (Option+Arrow) moves all cursors by word
- **Page Up/Down** moves all cursors
- **Shift+movement** extends selection for ALL cursors
- **Cursor deduplication** when cursors collide after movement
- Each cursor preserves its own `desired_column` for vertical movement through ragged lines

#### Implementation Details

- Per-cursor primitives in `EditorState`: `move_cursor_*_at(doc, idx)`
- All-cursors wrappers: `move_all_cursors_*(doc)`
- Selection variants: `move_all_cursors_*_with_selection(doc)`
- Removed legacy single-cursor movement functions from `update.rs`
- 10 new multi-cursor movement tests in `tests/cursor_movement.rs`

### Changed

- Test count: 383 (was 351)
- Added 10 multi-cursor movement tests, 22 other improvements

---

## 2025-12-06

### Added - Split View Implementation (All 7 Phases)

Complete multi-pane editor with split views, tabs, and shared documents.

#### Phase 1: Core Data Structures

- `DocumentId`, `EditorId`, `GroupId`, `TabId` - typed identifiers
- `Tab` struct with editor reference, pinned/preview flags
- `EditorGroup` with tabs, active tab index, layout rect
- `LayoutNode` enum: `Group(GroupId)` or `Split(SplitContainer)`
- `SplitContainer` with direction, children, ratios, min_sizes
- `EditorArea` managing documents, editors, groups, and layout tree

#### Phase 2: Layout System

- `Rect` type for layout calculations with `contains()` hit testing
- `compute_layout()` recursive algorithm for layout tree
- `group_at_point()` for mouse hit testing
- `SplitterBar` struct for splitter positions
- `splitter_at_point()` for resize handle detection
- `SPLITTER_WIDTH` constant (4px)

#### Phase 3: AppModel Migration

- Replaced single `Document`/`EditorState` with `EditorArea`
- Backward-compatible accessor methods: `document()`, `editor()`, etc.
- `ensure_focused_cursor_visible()` helper avoiding document cloning
- `resize()` now updates ALL editors (fixes multi-pane viewport bug)

#### Phase 4: LayoutMsg Handlers

- `SplitFocused(SplitDirection)` - split current group
- `SplitGroup { group_id, direction }` - split specific group
- `CloseGroup`, `CloseFocusedGroup` - close with layout cleanup
- `FocusGroup`, `FocusNextGroup`, `FocusPrevGroup` - navigation
- `FocusGroupByIndex(usize)` - keyboard shortcuts (1-indexed)
- `CloseTab`, `CloseFocusedTab`, `MoveTab` - tab operations
- `NextTab`, `PrevTab`, `SwitchToTab` - tab navigation

#### Phase 5: Multi-Group Rendering

- `render_all_groups_static()` iterates over layout
- `render_editor_group_static()` renders single pane
- Tab bar rendering with active/inactive styling
- Splitter bar rendering between groups
- Focus indicator (border) on focused group

#### Phase 6: Document Synchronization

- Documents shared across views (same `DocumentId`)
- Independent cursor/viewport per `EditorState`
- Edits reflect immediately in all views of same document

#### Phase 7: Keyboard Shortcuts

- Numpad 1-4: Focus group by index
- Numpad -/+: Split horizontal/vertical
- Cmd+W: Close tab
- Option+Cmd+Left/Right: Previous/Next tab
- Ctrl+Tab: Focus next group
- `physical_key` support in `handle_key()` for numpad detection

### Fixed - Split View Bugs

- `close_tab` on last group's only tab now prevented (was leaving invalid state)
- `move_tab` to invalid group now no-op (was losing tabs)
- Viewport resize updates all editors, not just focused one

### Added - Performance Overlay Sparklines

- Historical sparkline charts for render timing breakdown
- 60-frame rolling history per metric (clear, highlight, gutter, text, cursor, status, present)
- `draw_sparkline()` function with 1px bar visualization
- `record_render_history()` pushes timing to VecDeque histories

### Added - Multi-Cursor Batch Undo/Redo

- `EditOperation::Batch` for atomic multi-cursor operations
- InsertChar, InsertNewline, DeleteBackward, DeleteForward now batch
- Proper cursor restoration on undo/redo
- 6 new tests for multi-cursor undo behavior

### Added - SelectAllOccurrences (Cmd+Shift+L)

- Finds all occurrences of word/selection in document
- Creates cursor+selection for each occurrence
- Status message shows count: "Selected N occurrences"

### Added - Layout Tests

- 47 new tests in `tests/layout.rs`
- Split operations, close operations, focus navigation
- Tab operations (close, move, switch)
- Independent viewport/cursor per editor
- Edge cases (nested splits, invalid IDs)

### Changed

- Test count: 351 (was 246)
- Added 47 layout tests, 6 multi-cursor undo tests, selection tests

---

## 2025-12-06

### Added - Caret Count in Status Bar

- Shows "X carets" segment when multiple cursors are active
- New `SegmentId::CaretCount` variant
- Auto-syncs via `sync_status_bar()` when cursor count changes

### Fixed - Multi-Cursor Click Modifier

- Changed from Cmd+Click to Option+Click for adding/removing cursors
- Matches standard macOS editor conventions

### Added - Click+Drag Selection

- Standard click-and-drag text selection with left mouse button
- `left_mouse_down` state tracking in App struct
- CursorMoved handler extends selection while dragging
- Reuses existing `ExtendSelectionToPosition` message

### Added - Delete Line Command

- `DocumentMsg::DeleteLine` for deleting entire current line
- Cmd+Backspace keybinding (Ctrl+Backspace on non-Mac)
- Smart cursor positioning after delete:
  - First/middle line: stays on same line number
  - Last line: moves to end of previous line
  - Empty line after trailing newline: moves up
- Full undo/redo support
- 8 new tests in `tests/text_editing.rs`

### Added - Duplicate Line/Selection (Cmd+D)

- `DocumentMsg::Duplicate` for duplicating current line or selection
- No selection: duplicates entire line below cursor
- With selection: duplicates selected text in place
- Full undo/redo support
- 4 new tests in `tests/text_editing.rs`

### Added - Atomic Replace for Selection Editing

- `EditOperation::Replace` variant for atomic undo of selection replacement
- When typing over selection, undo restores both deleted text and removes inserted text in one operation
- Prevents "two-step undo" bug where user had to undo twice

### Fixed - Undo/Redo Keybindings on macOS

- Cmd+Z now properly triggers Undo (was inserting 'z')
- Cmd+Shift+Z now properly triggers Redo
- Fixed by adding `logo` modifier support alongside `ctrl`

### Fixed - Overflow Panics in Edge Cases

- `move_cursor_down()`: Fixed overflow when `visible_lines` is 0
- `ensure_cursor_visible_with_mode()`: Fixed horizontal scroll overflow
- `StatusBarLayout`: Fixed separator position overflow
- All arithmetic now uses `saturating_add`/`saturating_sub`

### Added - Expanded Monkey Tests

- 12 new window resize edge case tests in `tests/monkey_tests.rs`:
  - Maximum u32 dimensions
  - Very wide/narrow and very tall/narrow
  - Resize then cursor movement/scrolling
  - Oscillating zero/non-zero sizes
  - Resize with active selection
  - Cursor beyond viewport after resize
  - Powers of two dimensions
  - Interleaved resize and text operations
  - Status bar edge (height = line_height)

### Added - Status Bar Click Capture

- Clicks on status bar no longer propagate to editor
- `Renderer::is_in_status_bar(y)` method for hit testing

### Changed

- Test count: 246 (was 227)
- Added 8 delete line tests, 4 duplicate tests, 12 resize tests

---

## 2025-12-06

### Added - Status Bar System

- Structured, segment-based status bar per `docs/feature/STATUS_BAR.md`
- `StatusBar`, `StatusSegment`, `SegmentId`, `SegmentContent` types
- `sync_status_bar()` auto-updates segments from model state
- `StatusBarLayout` for rendering with separator positions
- Transient messages with auto-expiry (`TransientMessage`)
- Left/right segment alignment with separators
- 47 new status bar tests (`tests/status_bar.rs`)

### Added - Overlay System

- Reusable overlay rendering module (`src/overlay.rs`)
- `OverlayAnchor` enum (TopLeft, TopRight, BottomLeft, BottomRight, Center)
- `OverlayConfig` with builder pattern for configuration
- `render_overlay_background()` with alpha blending
- `render_overlay_border()` for optional 1px borders
- `blend_pixel()` for ARGB alpha compositing
- 7 overlay unit tests

### Added - Overlay Theme Integration

- `OverlayTheme` with themed colors: background, foreground, highlight, warning, error, border
- `OverlayThemeData` for YAML parsing (all fields optional for backward compatibility)
- Perf overlay now uses theme colors instead of hardcoded values
- Optional border rendering when theme specifies border color
- Added overlay sections to all 4 theme files

### Fixed

- Status bar separator lines now span full height (was inset 4px)
- Direction-aware scroll reveal with `ScrollRevealMode` enum
- `ensure_cursor_visible_with_mode()` primitive for scroll behavior
- Arrow key viewport snap-back behavior
- MoveCursor now properly calls ensure_cursor_visible()
- Directional reveal: Up→TopAligned, Down→BottomAligned for natural UX

### Changed

- Test count: 227 (was 185)
- Added 11 scroll reveal tests, 47 status bar tests, 7 overlay tests

---

## 2025-12-05

### Added - Selection & Multi-Cursor (Phase 7)

#### Phase 7.1: Basic Selection

- Theme support for `selection_background` and `secondary_cursor_color`
- ~25 new EditorMsg variants for selection/multi-cursor operations
- Shift+Arrow extends selection, Shift+Home/End, Shift+Click
- Selection rendering with blue highlight
- Escape clears selection or collapses multi-cursor

#### Phase 7.2: Selection Editing

- `delete_selection()` helper for selection range deletion
- InsertChar/InsertNewline deletes selection before inserting
- DeleteBackward/DeleteForward deletes selection instead of single char

#### Phase 7.3: Word & Line Selection

- SelectWord handler using `char_type` for word boundaries
- SelectLine handler (selects entire line including newline)
- Double-click selects word, triple-click selects line
- Click count tracking with wrap at 4

#### Phase 7.4: Multi-Cursor Basics

- `toggle_cursor_at()` in EditorState
- ToggleCursorAtPosition handler for Cmd+Click
- Multi-cursor rendering (primary=white, secondary=semi-transparent)

#### Phase 7.5: Multi-Cursor Editing

- `cursors_in_reverse_order()` helper
- InsertChar/InsertNewline at all cursors in reverse order
- DeleteBackward/DeleteForward at all cursors in reverse order

#### Phase 7.6: Clipboard

- arboard dependency for clipboard support
- Copy (Cmd+C) - copies selection or entire line
- Cut (Cmd+X) - copies and deletes selection
- Paste (Cmd+V) - multi-cursor aware, line-per-cursor distribution

#### Phase 7.7: Rectangle Selection

- `RectangleSelectionState` in EditorState
- Middle mouse down starts rectangle mode
- Mouse drag updates rectangle, mouse up finishes
- Creates cursors/selections for each line in rectangle
- Ghost cursor preview during drag

#### Phase 7.8: AddCursorAbove/Below

- Selection helper methods: `extend_to`, `collapse_to_start/end`, `contains`
- `deduplicate_cursors()` removes duplicate positions
- `assert_invariants()` for debug builds
- AddCursorAbove/Below handlers with column preservation
- Option+Option+Arrow double-tap detection (300ms threshold)

### Changed

- Moved 101 tests to tests/ folder (8 remaining in main.rs)
- Total test count: 185 (10 theme + 11 keyboard + 164 integration)

---

## 2025-12-04

### Added - Architecture Refactoring (Phases 1-6)

#### Phase 1: Split Model

- Created `model/` module hierarchy
- `Document` struct (buffer, undo/redo, file_path)
- `EditorState` struct (cursor, viewport)
- `UiState` struct (status, cursor blink)
- `AppModel` struct composing all state

#### Phase 2: Nested Messages

- `Direction` enum (Up, Down, Left, Right)
- `EditorMsg`, `DocumentMsg`, `UiMsg`, `AppMsg` enums
- Top-level `Msg` enum with sub-message dispatch
- Updated `handle_key()` for nested messages

#### Phase 3: Async Cmd System

- `Cmd::SaveFile` and `Cmd::LoadFile` variants
- `std::thread` + `mpsc` for async operations
- `process_cmd()` and `process_async_messages()` in event loop

#### Phase 4: Theming

- `src/theme.rs` with Color, Theme, YAML parsing
- All hardcoded colors replaced with theme lookups
- 6 new theme tests (96 total at this point)

#### Phase 5: Multi-Cursor Prep

- `Position` and `Selection` types in editor.rs
- `EditorState` uses `Vec<Cursor>` and `Vec<Selection>`
- Accessor methods: `cursor()`, `cursor_mut()`, `selection()`, `selection_mut()`
- ~220 cursor accesses updated across files

#### Phase 6: Performance Monitoring

- `PerfStats` struct with frame timing, cache stats
- `#[cfg(debug_assertions)]` gating
- Rolling 60-frame window for FPS calculation
- Semi-transparent perf overlay
- F2 toggle for overlay visibility

### Changed

- 90 tests passing after Phase 1-2
- 96 tests passing after Phase 4-5
