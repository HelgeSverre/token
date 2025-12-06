# Changelog

All notable changes to rust-editor are documented in this file.

---

## 2025-12-06

### Added
- Direction-aware scroll reveal with `ScrollRevealMode` enum
- `ensure_cursor_visible_with_mode()` primitive for scroll behavior
- Arrow key viewport snap-back behavior
- 11 new tests for scroll reveal modes

### Fixed
- MoveCursor now properly calls ensure_cursor_visible()
- Directional reveal: Up→TopAligned, Down→BottomAligned for natural UX

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
