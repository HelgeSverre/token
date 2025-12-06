# Changelog

All notable changes to rust-editor are documented in this file.

---

## 2025-12-06 (Latest)

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
