# Architecture Refactoring Progress Tracker

**Goal:** Evolve the Elm-style architecture following the recommendations in CODEBASE_REVIEW.md

**Status:** Planning

---

## Overview

This document tracks the refactoring of the rust-editor from a monolithic Model/Msg structure to a well-factored, async-ready architecture.

### Current State Analysis

| Component | Location                   | Issues                                            |
| --------- | -------------------------- | ------------------------------------------------- |
| Model     | `main.rs:31-60`            | Monolithic - mixes document, editor, and UI state |
| Msg       | `main.rs:355-400`          | Flat enum with 25 variants, will grow unwieldy    |
| Cmd       | `main.rs:1008-1010`        | Only `Redraw` variant, no async support           |
| File I/O  | `main.rs:100-118, 911-930` | Synchronous blocking calls                        |

### Target Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│  AppModel                                                       │
│  ┌────────────┐ ┌──────────────┐ ┌─────────┐ ┌───────────────┐  │
│  │ Document   │ │ EditorState  │ │ UiState │ │ (future:      │  │
│  │            │ │              │ │         │ │  DebugState)  │  │
│  └────────────┘ └──────────────┘ └─────────┘ └───────────────┘  │
└─────────────────────────────────────────────────────────────────┘
```

---

## Phase 1: Split Model

**Status:** [ ] Not Started

**Goal:** Extract domain types from the monolithic Model struct.

### Tasks

- [ ] **1.1** Create `Document` struct
  - `buffer: Rope`
  - `file_path: Option<PathBuf>`
  - `is_modified: bool`
  - `undo_stack: Vec<EditOperation>`
  - `redo_stack: Vec<EditOperation>`

- [ ] **1.2** Create `EditorState` struct
  - `cursor: Cursor`
  - `viewport: Viewport`
  - `scroll_padding: usize`

- [ ] **1.3** Create `UiState` struct
  - `status_message: String`
  - `cursor_visible: bool`
  - `last_cursor_blink: Instant`

- [ ] **1.4** Create `AppModel` struct that composes the above
  - `document: Document`
  - `editor: EditorState`
  - `ui: UiState`
  - `window_size: (u32, u32)`
  - `line_height: usize`
  - `char_width: f32`

- [ ] **1.5** Move helper methods to appropriate structs
  - `cursor_buffer_position()` → Document
  - `set_cursor_from_position()` → Document + EditorState
  - `current_line_length()` → Document
  - `ensure_cursor_visible()` → EditorState
  - `reset_cursor_blink()` → UiState

- [ ] **1.6** Update all references in `update()` function

- [ ] **1.7** Update `Renderer::render()` to work with new structure

- [ ] **1.8** Run tests, fix any breakage

### Files to Modify

| File          | Changes                                     |
| ------------- | ------------------------------------------- |
| `src/main.rs` | Extract types, update update() and render() |

### Estimated Effort: 2-4 hours

---

## Phase 2: Nested Messages

**Status:** [ ] Not Started

**Goal:** Organize the flat Msg enum into nested domain-specific enums.

### Current Msg Variants (25 total)

```
Window: Resize
Cursor: MoveCursorUp/Down/Left/Right, MoveCursorLineStart/End,
        MoveCursorDocumentStart/End, MoveCursorWordLeft/Right
Editing: InsertChar, InsertNewline, DeleteBackward, DeleteForward
Navigation: PageUp, PageDown, SetCursorPosition
History: Undo, Redo
Viewport: ScrollViewport, ScrollViewportHorizontal
File: SaveFile
UI: BlinkCursor
```

### Tasks

- [ ] **2.1** Create `EditorMsg` enum
  - `MoveCursor(Direction)` - consolidate Up/Down/Left/Right
  - `MoveCursorLineStart`, `MoveCursorLineEnd`
  - `MoveCursorDocumentStart`, `MoveCursorDocumentEnd`
  - `MoveCursorWord(Direction)` - consolidate WordLeft/WordRight
  - `PageUp`, `PageDown`
  - `SetCursorPosition { line: usize, column: usize }`
  - `Scroll(ScrollDelta)`

- [ ] **2.2** Create `DocumentMsg` enum
  - `InsertChar(char)`
  - `InsertNewline`
  - `DeleteBackward`
  - `DeleteForward`
  - `Undo`
  - `Redo`

- [ ] **2.3** Create `UiMsg` enum
  - `SetStatus(String)`
  - `BlinkCursor`

- [ ] **2.4** Create `AppMsg` enum
  - `Resize(u32, u32)`
  - `SaveFile`
  - `LoadFile(PathBuf)` - new!
  - `NewFile` - new!

- [ ] **2.5** Create `Direction` enum
  - `Up`, `Down`, `Left`, `Right`

- [ ] **2.6** Create top-level `Msg` enum

  ```rust
  enum Msg {
      Editor(EditorMsg),
      Document(DocumentMsg),
      Ui(UiMsg),
      App(AppMsg),
  }
  ```

- [ ] **2.7** Create delegating update functions
  - `update_editor()`
  - `update_document()`
  - `update_ui()`
  - `update_app()`

- [ ] **2.8** Update `handle_key()` to emit new message types

- [ ] **2.9** Run tests, fix any breakage

### Files to Modify

| File          | Changes                                              |
| ------------- | ---------------------------------------------------- |
| `src/main.rs` | New enums, refactored update(), updated handle_key() |

### Estimated Effort: 2-3 hours

---

## Phase 3: Expand Cmd for Async I/O

**Status:** [ ] Not Started

**Goal:** Make file operations non-blocking by processing them as commands.

### Current Cmd

```rust
enum Cmd {
    Redraw,
}
```

### Target Cmd

```rust
enum Cmd {
    Redraw,
    SaveFile { path: PathBuf, content: String },
    LoadFile { path: PathBuf },
    Multiple(Vec<Cmd>),  // For batching
}
```

### Tasks

- [ ] **3.1** Add new Cmd variants
  - `SaveFile { path: PathBuf, content: String }`
  - `LoadFile { path: PathBuf }`
  - `Multiple(Vec<Cmd>)` for combining commands

- [ ] **3.2** Create result message types
  - `AppMsg::SaveCompleted(Result<(), String>)`
  - `AppMsg::FileLoaded { path: PathBuf, result: Result<String, String> }`

- [ ] **3.3** Refactor `SaveFile` handler
  - Instead of doing I/O in update(), return `Cmd::SaveFile`
  - Set status to "Saving..." in model
  - Handle `SaveCompleted` message to update status

- [ ] **3.4** Add `LoadFile` handler
  - Create `AppMsg::LoadFile(PathBuf)` message
  - Return `Cmd::LoadFile` from update()
  - Handle `FileLoaded` message to populate buffer

- [ ] **3.5** Create command processor
  - Add `process_cmd()` function
  - Spawn threads for file I/O
  - Use channel to send result messages back

- [ ] **3.6** Integrate with event loop
  - Add channel receiver to App struct
  - Check for pending messages in `about_to_wait()`
  - Process incoming file I/O results

- [ ] **3.7** Add loading state UI
  - Track "loading" or "saving" state in UiState
  - Show indicator in status bar

- [ ] **3.8** Run tests, add async I/O tests

### Files to Modify

| File          | Changes                                              |
| ------------- | ---------------------------------------------------- |
| `src/main.rs` | Cmd variants, command processor, channel integration |

### Estimated Effort: 4-6 hours

---

## Phase 4: Prepare for Future Features (Optional)

**Status:** [ ] Not Started

**Goal:** Lay groundwork for multi-cursor and selections without full implementation.

### Tasks

- [ ] **4.1** Create `Selection` type (placeholder)

  ```rust
  struct Selection {
      start: Position,
      end: Position,
      direction: SelectionDirection,
  }
  ```

- [ ] **4.2** Change cursor storage to `Vec<Cursor>`
  - Single cursor remains default behavior
  - Data structure ready for multi-cursor

- [ ] **4.3** Add `Position` type
  ```rust
  struct Position {
      line: usize,
      column: usize,
  }
  ```

### Estimated Effort: 1-2 hours (structure only, no behavior changes)

---

## Implementation Order

```
Phase 1: Split Model
    ↓
Phase 2: Nested Messages
    ↓
Phase 3: Async Cmd
    ↓
(Optional) Phase 4: Future Prep
```

Each phase should be a separate commit with passing tests.

---

## Progress Log

| Date | Phase | Task          | Status | Notes |
| ---- | ----- | ------------- | ------ | ----- |
| TBD  | 1     | Start Phase 1 | -      | -     |

---

## Risk Mitigation

| Risk                          | Mitigation                               |
| ----------------------------- | ---------------------------------------- |
| Breaking existing tests       | Run `cargo test` after each major change |
| Render performance regression | Profile before/after with large files    |
| Undo/redo corruption          | Comprehensive undo/redo test coverage    |
| Thread safety issues          | Use channels, avoid shared mutable state |

---

## Success Criteria

- [ ] All 56 existing tests pass
- [ ] Model split into Document/EditorState/UiState
- [ ] Msg enum nested by domain
- [ ] File save/load operations are non-blocking
- [ ] Status bar shows loading/saving indicators
- [ ] No visible performance regression
