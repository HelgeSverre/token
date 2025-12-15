# Changelog

All notable changes to rust-editor are documented in this file.

---

## v0.3.0 - 2025-12-15 (Latest)

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
