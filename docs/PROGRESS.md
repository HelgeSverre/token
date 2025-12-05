# Architecture Refactoring Progress Tracker

**Goal:** Evolve the Elm-style architecture following the recommendations in CODEBASE_REVIEW.md

**Status:** Complete - All 6 Phases Done

---

## Overview

This document tracks the refactoring of the rust-editor from a monolithic Model/Msg structure to a well-factored, async-ready architecture with theming support.

### Current State Analysis

| Component | Location                   | Issues                                            |
| --------- | -------------------------- | ------------------------------------------------- |
| Model     | `main.rs:31-60`            | Monolithic - mixes document, editor, and UI state |
| Msg       | `main.rs:355-400`          | Flat enum with 25 variants, will grow unwieldy    |
| Cmd       | `main.rs:1008-1010`        | Only `Redraw` variant, no async support           |
| File I/O  | `main.rs:100-118, 911-930` | Synchronous blocking calls                        |
| Colors    | Hardcoded constants        | No theming support                                |

### Target Architecture

```
┌───────────────────────────────────────────────────────────────────────────┐
│  AppModel                                                                 │
│  ┌────────────┐ ┌──────────────┐ ┌─────────┐ ┌────────────┐ ┌───────────┐│
│  │ Document   │ │ EditorState  │ │ UiState │ │ DebugState │ │   Theme   ││
│  │            │ │              │ │         │ │ (optional) │ │           ││
│  │ - buffer   │ │ - cursors[]  │ │ - status│ │ - perf     │ │ - ui      ││
│  │ - undo/redo│ │ - selections │ │ - blink │ │ - overlay  │ │ - syntax  ││
│  │ - file_path│ │ - viewport   │ │         │ │            │ │           ││
│  └────────────┘ └──────────────┘ └─────────┘ └────────────┘ └───────────┘│
│                                                                           │
│  Layout: window_size, line_height, char_width                             │
└───────────────────────────────────────────────────────────────────────────┘
```

### Feature Design Documents

Detailed design docs for implemented and planned features:

| Feature | Status | Design Doc |
|---------|--------|------------|
| Theming System | Complete | [feature/THEMING.md](feature/THEMING.md) |
| Selection & Multi-Cursor | Phase 8/9 Complete | [feature/SELECTION_MULTICURSOR.md](feature/SELECTION_MULTICURSOR.md) |
| Status Bar | Planned | [feature/STATUS_BAR.md](feature/STATUS_BAR.md) |
| Split View | Planned | [feature/SPLIT_VIEW.md](feature/SPLIT_VIEW.md) |

### Message Flow

```
KeyEvent/MouseEvent
        │
        ▼
┌───────────────────────────────────────────────────────────┐
│  Msg                                                      │
│  ├── Editor(EditorMsg)    → update_editor()               │
│  ├── Document(DocumentMsg)→ update_document()             │
│  ├── Ui(UiMsg)            → update_ui()                   │
│  ├── Theme(ThemeMsg)      → update_theme()                │
│  ├── Debug(DebugMsg)      → update_debug()  [cfg(perf)]   │
│  └── App(AppMsg)          → update_app()                  │
└───────────────────────────────────────────────────────────┘
        │
        ▼
┌───────────────────────────────────────────────────────────┐
│  Cmd                                                      │
│  ├── Redraw                                               │
│  ├── SaveFile { path, content }                           │
│  ├── LoadFile { path }                                    │
│  ├── LoadTheme { path }                                   │
│  ├── StartPerfSampler { interval_ms }  [cfg(perf)]        │
│  └── Multiple(Vec<Cmd>)                                   │
└───────────────────────────────────────────────────────────┘
        │
        ▼
    process_cmd() → spawns threads → sends Msg back via channel
```

---

## Phase 1: Split Model

**Status:** [x] Complete

**Goal:** Extract domain types from the monolithic Model struct into focused components.

### Tasks

- [x] **1.1** Create `Document` struct

  ```rust
  struct Document {
      buffer: Rope,
      file_path: Option<PathBuf>,
      is_modified: bool,
      undo_stack: Vec<EditOperation>,
      redo_stack: Vec<EditOperation>,
  }
  ```

- [x] **1.2** Create `EditorState` struct

  ```rust
  struct EditorState {
      cursor: Cursor,              // Single cursor for now
      viewport: Viewport,
      scroll_padding: usize,
  }
  ```

  _Note: Will expand to `cursors: Vec<Cursor>` and add `selections: Vec<Selection>` in Phase 5._

- [x] **1.3** Create `UiState` struct

  ```rust
  struct UiState {
      status_message: String,
      cursor_visible: bool,
      last_cursor_blink: Instant,
  }
  ```

- [x] **1.4** Create `AppModel` struct that composes the above

  ```rust
  struct AppModel {
      document: Document,
      editor: EditorState,
      ui: UiState,
      // Layout (shared across components)
      window_size: (u32, u32),
      line_height: usize,
      char_width: f32,
  }
  ```

- [x] **1.5** Move helper methods to appropriate structs

  | Method                    | Move To                  | Notes                         |
  | ------------------------- | ------------------------ | ----------------------------- |
  | `cursor_buffer_position`  | `Document`               | Takes cursor as parameter     |
  | `current_line_length`     | `Document`               | -                             |
  | `line_length`             | `Document`               | -                             |
  | `get_line`                | `Document`               | -                             |
  | `set_cursor_from_position`| `Document` + `Cursor`    | Returns new Cursor            |
  | `ensure_cursor_visible`   | `EditorState`            | Takes document for line count |
  | `reset_cursor_blink`      | `UiState`                | -                             |
  | `first_non_whitespace_column` | `Document`           | -                             |
  | `last_non_whitespace_column`  | `Document`           | -                             |

- [x] **1.6** Update all references in `update()` function

  - Change `model.buffer` → `model.document.buffer`
  - Change `model.cursor` → `model.editor.cursor`
  - Change `model.viewport` → `model.editor.viewport`
  - Change `model.status_message` → `model.ui.status_message`
  - etc.

- [x] **1.7** Update `Renderer::render()` to work with new structure

  - Pass `&AppModel` instead of `&Model`
  - Access nested fields appropriately

- [x] **1.8** Run tests, fix any breakage

### Files to Modify

| File          | Changes                                       |
| ------------- | --------------------------------------------- |
| `src/main.rs` | Extract types, update `update()` and `render()` |

### Estimated Effort: 2-4 hours

---

## Phase 2: Nested Messages

**Status:** [x] Complete

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

- [x] **2.1** Create `Direction` enum

  ```rust
  enum Direction {
      Up,
      Down,
      Left,
      Right,
  }
  ```

- [x] **2.2** Create `EditorMsg` enum

  ```rust
  enum EditorMsg {
      MoveCursor(Direction),
      MoveCursorLineStart,
      MoveCursorLineEnd,
      MoveCursorDocumentStart,
      MoveCursorDocumentEnd,
      MoveCursorWord(Direction),  // Only Left/Right used
      PageUp,
      PageDown,
      SetCursorPosition { line: usize, column: usize },
      Scroll(i32),                 // Vertical scroll delta
      ScrollHorizontal(i32),       // Horizontal scroll delta
  }
  ```

- [x] **2.3** Create `DocumentMsg` enum

  ```rust
  enum DocumentMsg {
      InsertChar(char),
      InsertNewline,
      DeleteBackward,
      DeleteForward,
      Undo,
      Redo,
  }
  ```

- [x] **2.4** Create `UiMsg` enum

  ```rust
  enum UiMsg {
      SetStatus(String),
      BlinkCursor,
  }
  ```

- [x] **2.5** Create `AppMsg` enum

  ```rust
  enum AppMsg {
      Resize(u32, u32),
      SaveFile,
      LoadFile(PathBuf),  // New
      NewFile,            // New
      Quit,               // New
  }
  ```

- [x] **2.6** Create top-level `Msg` enum

  ```rust
  enum Msg {
      Editor(EditorMsg),
      Document(DocumentMsg),
      Ui(UiMsg),
      App(AppMsg),
  }
  ```

- [x] **2.7** Create delegating update functions

  ```rust
  fn update(app: &mut AppModel, msg: Msg) -> Option<Cmd> {
      match msg {
          Msg::Editor(emsg) => update_editor(app, emsg),
          Msg::Document(dmsg) => update_document(app, dmsg),
          Msg::Ui(umsg) => update_ui(app, umsg),
          Msg::App(amsg) => update_app(app, amsg),
      }
  }

  fn update_editor(app: &mut AppModel, msg: EditorMsg) -> Option<Cmd> { ... }
  fn update_document(app: &mut AppModel, msg: DocumentMsg) -> Option<Cmd> { ... }
  fn update_ui(app: &mut AppModel, msg: UiMsg) -> Option<Cmd> { ... }
  fn update_app(app: &mut AppModel, msg: AppMsg) -> Option<Cmd> { ... }
  ```

- [x] **2.8** Update `handle_key()` to emit new message types

  ```rust
  // Before
  Some(Msg::MoveCursorUp)

  // After
  Some(Msg::Editor(EditorMsg::MoveCursor(Direction::Up)))
  ```

- [x] **2.9** Run tests, fix any breakage

### Files to Modify

| File          | Changes                                              |
| ------------- | ---------------------------------------------------- |
| `src/main.rs` | New enums, refactored `update()`, updated `handle_key()` |

### Estimated Effort: 2-3 hours

---

## Phase 3: Expand Cmd for Async I/O

**Status:** [x] Complete

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
    Multiple(Vec<Cmd>),
}
```

### Tasks

- [x] **3.1** Add new Cmd variants

  ```rust
  enum Cmd {
      Redraw,
      SaveFile { path: PathBuf, content: String },
      LoadFile { path: PathBuf },
      Multiple(Vec<Cmd>),
  }
  ```

- [x] **3.2** Add async result messages to `AppMsg`

  ```rust
  enum AppMsg {
      // ... existing variants
      SaveCompleted(Result<(), String>),
      FileLoaded { path: PathBuf, result: Result<String, String> },
  }
  ```

- [x] **3.3** Add loading state to `UiState`

  ```rust
  struct UiState {
      // ... existing fields
      is_loading: bool,  // Show loading indicator
      is_saving: bool,   // Show saving indicator
  }
  ```

- [x] **3.4** Refactor `SaveFile` handler

  ```rust
  // Before: Blocking I/O in update()
  AppMsg::SaveFile => {
      std::fs::write(path, content)?;
      // ...
  }

  // After: Return command, handle result
  AppMsg::SaveFile => {
      app.ui.is_saving = true;
      app.ui.status_message = "Saving...".to_string();
      Some(Cmd::SaveFile { path, content })
  }

  AppMsg::SaveCompleted(result) => {
      app.ui.is_saving = false;
      match result {
          Ok(_) => {
              app.document.is_modified = false;
              app.ui.status_message = format!("Saved: {}", path);
          }
          Err(e) => {
              app.ui.status_message = format!("Error: {}", e);
          }
      }
      Some(Cmd::Redraw)
  }
  ```

- [x] **3.5** Add `LoadFile` handler

  ```rust
  AppMsg::LoadFile(path) => {
      app.ui.is_loading = true;
      app.ui.status_message = "Loading...".to_string();
      Some(Cmd::LoadFile { path })
  }

  AppMsg::FileLoaded { path, result } => {
      app.ui.is_loading = false;
      match result {
          Ok(content) => {
              app.document.buffer = Rope::from(content);
              app.document.file_path = Some(path);
              app.document.is_modified = false;
              app.document.undo_stack.clear();
              app.document.redo_stack.clear();
              app.editor.cursor = Cursor::default();
              app.ui.status_message = format!("Loaded: {}", path);
          }
          Err(e) => {
              app.ui.status_message = format!("Error: {}", e);
          }
      }
      Some(Cmd::Redraw)
  }
  ```

- [x] **3.6** Create command processor

  ```rust
  fn process_cmd(cmd: Cmd, tx: Sender<Msg>) {
      match cmd {
          Cmd::Redraw => {
              // Handled by event loop directly
          }
          Cmd::SaveFile { path, content } => {
              std::thread::spawn(move || {
                  let result = std::fs::write(&path, content)
                      .map_err(|e| e.to_string());
                  let _ = tx.send(Msg::App(AppMsg::SaveCompleted(result)));
              });
          }
          Cmd::LoadFile { path } => {
              std::thread::spawn(move || {
                  let result = std::fs::read_to_string(&path)
                      .map_err(|e| e.to_string());
                  let _ = tx.send(Msg::App(AppMsg::FileLoaded { path, result }));
              });
          }
          Cmd::Multiple(cmds) => {
              for cmd in cmds {
                  process_cmd(cmd, tx.clone());
              }
          }
      }
  }
  ```

- [x] **3.7** Integrate with event loop

  ```rust
  struct App {
      // ... existing fields
      msg_rx: Receiver<Msg>,
      msg_tx: Sender<Msg>,
  }

  impl ApplicationHandler for App {
      fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
          // Process pending async results
          while let Ok(msg) = self.msg_rx.try_recv() {
              if let Some(cmd) = update(&mut self.model, msg) {
                  process_cmd(cmd, self.msg_tx.clone());
              }
          }
          // ... existing timer logic
      }
  }
  ```

- [x] **3.8** Run tests, add async I/O tests

### Files to Modify

| File          | Changes                                              |
| ------------- | ---------------------------------------------------- |
| `src/main.rs` | Cmd variants, command processor, channel integration |

### Estimated Effort: 4-6 hours

---

## Phase 4: Theming System

**Status:** [x] Complete
**Design:** [feature/THEMING.md](feature/THEMING.md)

**Goal:** Replace hardcoded colors with a YAML-based theming system.

### Tasks

- [x] **4.1** Add dependencies to Cargo.toml

  ```toml
  [dependencies]
  serde = { version = "1", features = ["derive"] }
  serde_yaml = "0.9"
  ```

- [x] **4.2** Create `src/theme.rs` module

  ```rust
  // Core types
  pub struct Color { pub r: u8, pub g: u8, pub b: u8, pub a: u8 }
  pub struct StatefulColor { pub normal: String, pub hover: Option<String>, ... }
  pub struct SyntaxStyle { pub foreground: String, pub font_style: Vec<String>, ... }

  // Theme structure matching THEMING_SYSTEM.md
  pub struct Theme {
      pub version: u32,
      pub name: String,
      pub ui: UiTheme,
      pub syntax: SyntaxTheme,
  }

  pub struct UiTheme {
      pub window: WindowTheme,
      pub editor: EditorTheme,
      pub gutter: GutterTheme,
      pub scrollbar: ScrollbarTheme,
      pub status_bar: StatusBarTheme,
  }

  // ... nested structs for each component
  ```

- [x] **4.3** Add `Theme` to `AppModel`

  ```rust
  struct AppModel {
      document: Document,
      editor: EditorState,
      ui: UiState,
      theme: Theme,  // ← Add theme here
      // ... layout fields
  }
  ```

- [-] **4.4** Create `ThemeMsg` enum

  ```rust
  enum ThemeMsg {
      Load(PathBuf),
      LoadCompleted(Result<Theme, String>),
      Reload,
  }
  ```

  Add to top-level `Msg`:

  ```rust
  enum Msg {
      // ... existing variants
      Theme(ThemeMsg),
  }
  ```

- [-] **4.5** Add `Cmd::LoadTheme` variant

  ```rust
  enum Cmd {
      // ... existing variants
      LoadTheme { path: PathBuf },
  }
  ```

- [-] **4.6** Implement theme loading in command processor

  ```rust
  Cmd::LoadTheme { path } => {
      std::thread::spawn(move || {
          let result = std::fs::read_to_string(&path)
              .map_err(|e| e.to_string())
              .and_then(|content| {
                  serde_yaml::from_str(&content)
                      .map_err(|e| e.to_string())
              });
          let _ = tx.send(Msg::Theme(ThemeMsg::LoadCompleted(result)));
      });
  }
  ```

- [x] **4.7** Create default dark theme

  - Embed default theme as `Theme::default_dark()` for fallback
  - Create `themes/github-dark.yaml` as example

- [x] **4.8** Update `Renderer` to use theme colors

  ```rust
  // Before
  const BACKGROUND: u32 = 0xFF1E1E1E;
  ctx.fill_rect(0, 0, width, height, BACKGROUND);

  // After
  let bg = app.theme.ui.editor.background.resolve(UiState::Normal);
  ctx.fill_rect(0, 0, width, height, bg.to_argb_u32());
  ```

- [x] **4.9** Remove hardcoded color constants

  - Delete `CURRENT_LINE_HIGHLIGHT` constant
  - Replace all `0xFF...` color literals with theme lookups

- [x] **4.10** Run tests, verify theme loading

### Files to Create/Modify

| File                         | Changes                        |
| ---------------------------- | ------------------------------ |
| `src/theme.rs`               | New - theme types and parsing  |
| `src/main.rs`                | Add Theme to AppModel, ThemeMsg |
| `themes/github-dark.yaml`    | New - example theme file       |
| `Cargo.toml`                 | Add serde, serde_yaml          |

### Estimated Effort: 4-6 hours

---

## Phase 5: Multi-Cursor & Selection Prep

**Status:** [x] Complete

**Goal:** Lay groundwork for multi-cursor and selections without full implementation.

### Tasks

- [x] **5.1** Create `Position` type

  ```rust
  #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
  struct Position {
      line: usize,
      column: usize,
  }
  ```

- [x] **5.2** Create `Selection` type

  ```rust
  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  struct Selection {
      anchor: Position,    // Where selection started
      head: Position,      // Where cursor is (can be before or after anchor)
  }

  impl Selection {
      fn is_empty(&self) -> bool {
          self.anchor == self.head
      }

      fn start(&self) -> Position {
          if self.anchor <= self.head { self.anchor } else { self.head }
      }

      fn end(&self) -> Position {
          if self.anchor <= self.head { self.head } else { self.anchor }
      }
  }
  ```

- [x] **5.3** Update `EditorState` for multi-cursor readiness

  ```rust
  struct EditorState {
      cursors: Vec<Cursor>,           // Primary cursor is cursors[0]
      selections: Vec<Selection>,     // Parallel to cursors
      viewport: Viewport,
      scroll_padding: usize,
  }

  impl EditorState {
      fn cursor(&self) -> &Cursor {
          &self.cursors[0]
      }

      fn cursor_mut(&mut self) -> &mut Cursor {
          &mut self.cursors[0]
      }

      fn selection(&self) -> &Selection {
          &self.selections[0]
      }

      fn selection_mut(&mut self) -> &mut Selection {
          &mut self.selections[0]
      }
  }
  ```

- [x] **5.4** Update all cursor access to use `cursor()` / `cursor_mut()`

  - Single cursor behavior remains default
  - All existing code works unchanged
  - Data structure ready for multi-cursor expansion

- [x] **5.5** Run tests, verify no behavior changes

### Files Modified

| File                   | Changes                                              |
| ---------------------- | ---------------------------------------------------- |
| `src/model/editor.rs`  | Position, Selection types; EditorState with Vec<>    |
| `src/model/mod.rs`     | Export Position, Selection; accessor methods         |
| `src/update.rs`        | All cursor access via accessor methods               |
| `src/main.rs`          | Renderer and tests use accessor methods              |

### Actual Effort: ~1 hour

---

## Phase 6: Performance Monitoring

**Status:** [x] Complete

**Goal:** Add debug-build-only performance monitoring with toggleable overlay.

### Implementation

Used `#[cfg(debug_assertions)]` instead of feature flags for simpler integration:
- Zero overhead in release builds (all perf code stripped)
- No feature flag configuration needed
- Automatic in debug builds, automatic removal in release

### Tasks

- [x] **6.1** Create `PerfStats` struct

  ```rust
  #[cfg(debug_assertions)]
  struct PerfStats {
      frame_start: Option<Instant>,
      last_frame_time: Duration,
      frame_times: VecDeque<Duration>,  // Rolling 60-frame window
      total_cache_hits: usize,
      total_cache_misses: usize,
      show_overlay: bool,
  }
  ```

- [x] **6.2** Add `PerfStats` to `App` struct

  ```rust
  struct App {
      // ... existing fields
      #[cfg(debug_assertions)]
      perf: PerfStats,
  }
  ```

- [x] **6.3** Instrument frame timing in App::render()

- [x] **6.4** Implement perf overlay with render_perf_overlay()

  - Frame time (current + average)
  - FPS display
  - Budget bar (% of 16.67ms target)
  - Glyph cache size
  - Cache hit rate

- [x] **6.5** Add F2 keybinding to toggle overlay

### Overlay Display

```
┌─────────────────────────────┐
│ PERF (F2 to hide)           │
│ Frame: 16.2ms (60 fps)      │
│ [████████░░] 97%            │
│ Avg: 15.8ms                 │
│ Cache: 1,234 glyphs         │
│ Hits: 45,678 (99.8%)        │
│ Miss: 12                    │
└─────────────────────────────┘
```

### Files Modified

| File          | Changes                                     |
| ------------- | ------------------------------------------- |
| `src/main.rs` | PerfStats, App, render timing, F12 handler, overlay |

### Actual Effort: ~1 hour

---

## Implementation Order

```
Phase 1: Split Model
    ↓
Phase 2: Nested Messages
    ↓
Phase 3: Async Cmd
    ↓
Phase 4: Theming System
    ↓
Phase 5: Multi-Cursor Prep
    ↓
(Optional) Phase 6: Performance Monitoring
```

Each phase should be a separate commit (or PR) with passing tests.

---

## Progress Log

| Date       | Phase | Task                           | Status    | Notes                                               |
| ---------- | ----- | ------------------------------ | --------- | --------------------------------------------------- |
| 2025-12-04 | 1     | Create model/ module hierarchy | Complete  | Document, EditorState, UiState, AppModel            |
| 2025-12-04 | 1     | Create update.rs               | Complete  | Full update() with sub-handlers                     |
| 2025-12-04 | 2     | Create messages.rs             | Complete  | Direction, EditorMsg, DocumentMsg, UiMsg, AppMsg    |
| 2025-12-04 | 2     | Update handle_key()            | Complete  | All key handlers use nested messages                |
| 2025-12-04 | 1+2   | Update tests                   | Complete  | 90 tests pass with new architecture                 |
| 2025-12-04 | 3     | Async Cmd system               | Complete  | SaveFile/LoadFile via std::thread + mpsc            |
| 2025-12-04 | 3     | Event loop integration         | Complete  | process_cmd() + process_async_messages()            |
| 2025-12-04 | 4     | Theme module                   | Complete  | src/theme.rs with Color, Theme, YAML parsing        |
| 2025-12-04 | 4     | Renderer theming               | Complete  | All hardcoded colors replaced with theme lookups    |
| 2025-12-04 | 4     | Tests                          | Complete  | 96 tests pass (6 new theme tests + 90 existing)     |
| 2025-12-04 | 5     | Position & Selection types     | Complete  | Added to editor.rs with full implementations        |
| 2025-12-04 | 5     | Multi-cursor data structures   | Complete  | EditorState uses Vec<Cursor> and Vec<Selection>     |
| 2025-12-04 | 5     | Accessor methods               | Complete  | cursor()/cursor_mut()/selection()/selection_mut()   |
| 2025-12-04 | 5     | Update all cursor access       | Complete  | ~220 cursor accesses updated across files           |
| 2025-12-04 | 5     | Tests                          | Complete  | 96 tests pass - no behavior changes                 |
| 2025-12-04 | 6     | PerfStats struct               | Complete  | Frame timing, cache stats, overlay toggle           |
| 2025-12-04 | 6     | App struct integration         | Complete  | #[cfg(debug_assertions)] gating                     |
| 2025-12-04 | 6     | Frame timing                   | Complete  | Rolling 60-frame window, FPS calculation            |
| 2025-12-04 | 6     | Perf overlay rendering         | Complete  | Semi-transparent panel with all metrics             |
| 2025-12-04 | 6     | F12 toggle                     | Complete  | Toggles overlay visibility                          |
| 2025-12-04 | 6     | Build verification             | Complete  | Debug + Release builds clean, 90 tests pass         |

---

## Risk Mitigation

| Risk                          | Mitigation                                   |
| ----------------------------- | -------------------------------------------- |
| Breaking existing tests       | Run `cargo test` after each major change     |
| Render performance regression | Profile before/after with large files        |
| Undo/redo corruption          | Comprehensive undo/redo test coverage        |
| Thread safety issues          | Use channels, avoid shared mutable state     |
| Theme parsing errors          | Provide default fallback, validate on load   |
| Feature flag complexity       | Keep `#[cfg]` blocks minimal and isolated    |

---

## Success Criteria

- [x] All 119 tests pass (10 theme + 8 keyboard + 101 integration)
- [x] Model split into Document/EditorState/UiState
- [x] Msg enum nested by domain (Editor, Document, Ui, App)
- [x] File save/load operations are non-blocking (Phase 3)
- [x] Status bar shows loading/saving indicators (Phase 3)
- [x] Theming system loads YAML themes (Phase 4)
- [x] All hardcoded colors replaced with theme lookups (Phase 4)
- [x] EditorState uses `Vec<Cursor>` (ready for multi-cursor) (Phase 5)
- [x] Selection type defined (ready for visual selections) (Phase 5)
- [x] F2 toggles perf overlay in debug builds (Phase 6)
- [x] Frame time and FPS displayed accurately (Phase 6)
- [x] Cache stats show hit rate and size (Phase 6)
- [x] No perf overhead in release builds (Phase 6)
- [x] No visible performance regression
- [x] Tests organized in separate tests/ folder (refactoring)

---

## Module Structure (After Refactoring)

**Current structure (Phase 1-8 complete):**

```
src/
├── main.rs              # Entry point, event loop, App struct, Renderer
├── lib.rs               # Library root with module exports
├── model/
│   ├── mod.rs           # AppModel struct (includes Theme), re-exports
│   ├── document.rs      # Document struct (buffer, undo/redo, file_path)
│   ├── editor.rs        # EditorState, Cursor, Selection, Viewport
│   └── ui.rs            # UiState (status, cursor blink, loading states)
├── messages.rs          # Msg, EditorMsg, DocumentMsg, UiMsg, AppMsg, Direction
├── commands.rs          # Cmd enum (Redraw, SaveFile, LoadFile, Batch)
├── update.rs            # update() dispatcher + update_editor/document/ui/app
├── theme.rs             # Theme, Color, YAML theme loading
└── util.rs              # CharType enum, is_punctuation, char_type

tests/
├── common/
│   └── mod.rs           # Shared test helpers (test_model, test_model_with_selection)
├── cursor_movement.rs   # 38 tests - cursor position, movement, smart home/end, word nav
├── text_editing.rs      # 21 tests - insert/delete, undo/redo
├── selection.rs         # 11 tests - selection helpers, rectangle, multi-cursor
├── scrolling.rs         # 22 tests - vertical, horizontal, page navigation
└── edge_cases.rs        # 9 tests - regression tests, boundaries
```

**Test distribution:**
- `tests/` folder: 101 integration tests (library API)
- `src/main.rs`: 8 keyboard handling tests (require handle_key)
- `src/theme.rs`: 10 theme tests (inline, module-specific)
- **Total: 119 tests**

---

## Phase 7: Selection & Multi-Cursor Implementation

**Status:** In Progress - Phase 8 Complete (1 phase remaining)
**Design:** [feature/SELECTION_MULTICURSOR.md](feature/SELECTION_MULTICURSOR.md)

**Goal:** Implement the selection and multi-cursor system.

### Implementation Phases

- [x] **Phase 1: Basic Selection** (Foundation)
  - [x] Add `selection_background` and `secondary_cursor_color` to theme
  - [x] Add ~25 new EditorMsg variants for selection/multi-cursor
  - [x] Implement handlers in update.rs
  - [x] Update `handle_key()` for Shift+Arrow detection
  - [x] Render selections before text
  - [x] Handle Shift+Click for `ExtendSelectionToPosition`
  - [x] Implement selection collapse on non-shift movement
  - [x] Escape clears selection

- [x] **Phase 2: Selection Editing**
  - [x] Delete selection on Backspace/Delete
  - [x] Replace selection when typing (InsertChar)
  - [x] Replace selection on InsertNewline

- [x] **Phase 3: Word & Line Selection**
  - [x] Implement `SelectWord` (double-click)
  - [x] Implement `SelectLine` (triple-click)
  - [x] Add double/triple click detection in App

- [x] **Phase 4: Multi-Cursor Basics**
  - [x] Implement `ToggleCursorAtPosition` for Cmd+Click
  - [x] Render all cursors (with secondary color)
  - [x] Implement `CollapseToSingleCursor` on Escape (done in Phase 1)

- [x] **Phase 5: Multi-Cursor Editing**
  - [x] Implement reverse-order editing for all cursors
  - [x] InsertChar at all cursors
  - [x] InsertNewline at all cursors
  - [x] DeleteBackward at all cursors
  - [x] DeleteForward at all cursors

- [x] **Phase 6: Clipboard**
  - [x] Add arboard dependency for clipboard support
  - [x] Implement Copy (Cmd+C) - copies selection or entire line
  - [x] Implement Cut (Cmd+X) - copies and deletes selection
  - [x] Implement Paste (Cmd+V) - multi-cursor aware pasting
  - [x] Handle empty selection copy (copy line)

- [x] **Phase 7: Rectangle Selection**
  - [x] Add RectangleSelectionState to EditorState
  - [x] Handle middle mouse down (StartRectangleSelection)
  - [x] Handle mouse drag (UpdateRectangleSelection)
  - [x] Handle middle mouse up (FinishRectangleSelection)
  - [x] Create cursors/selections for each line in rectangle
  - [x] Render rectangle overlay during drag

- [x] **Phase 8: AddCursorAbove/Below**
  - [x] Add `last_option_press` and `option_double_tapped` to App
  - [x] Implement Option key double-tap detection (300ms threshold)
  - [x] Implement `AddCursorAbove` / `AddCursorBelow` handlers
  - [x] Wire Option+Option+Arrow keyboard shortcuts
  - [x] Add `deduplicate_cursors()` to EditorState
  - [x] Add Selection helper methods (extend_to, collapse_to_start/end, contains)
  - [x] Add `assert_invariants()` for debug builds

- [ ] **Phase 9: Occurrence Selection (JetBrains-style)**
  - [ ] Implement `AddSelectionForNextOccurrence` (Cmd+J)
  - [ ] Implement `UnselectOccurrence` (Shift+Cmd+J)
  - [ ] Track occurrence history for unselect
  - [ ] Word-under-cursor detection (reuse `char_type()`)

### Progress Log

| Date       | Phase | Task                           | Status    | Notes                                               |
| ---------- | ----- | ------------------------------ | --------- | --------------------------------------------------- |
| 2025-12-05 | 7.1   | Theme selection colors         | Complete  | selection_background, secondary_cursor_color        |
| 2025-12-05 | 7.1   | EditorMsg variants             | Complete  | ~25 new messages for selection/multi-cursor         |
| 2025-12-05 | 7.1   | Update handlers                | Complete  | All new handlers in update.rs                       |
| 2025-12-05 | 7.1   | Keyboard handling              | Complete  | Shift+Arrow, Shift+Home/End, etc.                   |
| 2025-12-05 | 7.1   | Selection rendering            | Complete  | Blue highlight behind selected text                 |
| 2025-12-05 | 7.1   | Shift+Click                    | Complete  | ExtendSelectionToPosition                           |
| 2025-12-05 | 7.1   | Selection collapse             | Complete  | Clear on movement without shift                     |
| 2025-12-05 | 7.1   | Escape handling                | Complete  | Clear selection or collapse multi-cursor            |
| 2025-12-05 | 7.2   | delete_selection helper        | Complete  | Helper fn to delete selection range                 |
| 2025-12-05 | 7.2   | InsertChar with selection      | Complete  | Deletes selection before inserting                  |
| 2025-12-05 | 7.2   | InsertNewline with selection   | Complete  | Deletes selection before inserting newline          |
| 2025-12-05 | 7.2   | DeleteBackward with selection  | Complete  | Deletes selection instead of char                   |
| 2025-12-05 | 7.2   | DeleteForward with selection   | Complete  | Deletes selection instead of char                   |
| 2025-12-05 | 7.3   | SelectWord handler             | Complete  | Uses char_type for word boundaries                  |
| 2025-12-05 | 7.3   | SelectLine handler             | Complete  | Selects entire line including newline               |
| 2025-12-05 | 7.3   | Click detection in App         | Complete  | Tracks click_count, last_click_time/position        |
| 2025-12-05 | 7.3   | Double/triple click dispatch   | Complete  | 2=SelectWord, 3=SelectLine, wraps at 4              |
| 2025-12-05 | 7.4   | EditorState toggle_cursor_at   | Complete  | Add/remove cursors, sort by position                |
| 2025-12-05 | 7.4   | ToggleCursorAtPosition handler | Complete  | Cmd+Click toggles cursor                            |
| 2025-12-05 | 7.4   | Multi-cursor rendering         | Complete  | Primary=white, secondary=semi-transparent           |
| 2025-12-05 | 7.5   | cursors_in_reverse_order       | Complete  | Helper to sort cursor indices descending            |
| 2025-12-05 | 7.5   | InsertChar multi-cursor        | Complete  | Insert at all cursors in reverse order              |
| 2025-12-05 | 7.5   | InsertNewline multi-cursor     | Complete  | Insert newline at all cursors                       |
| 2025-12-05 | 7.5   | DeleteBackward multi-cursor    | Complete  | Delete before each cursor in reverse order          |
| 2025-12-05 | 7.5   | DeleteForward multi-cursor     | Complete  | Delete at each cursor in reverse order              |
| 2025-12-05 | 7.6   | arboard dependency             | Complete  | Added clipboard support via arboard crate           |
| 2025-12-05 | 7.6   | Copy (Cmd+C)                   | Complete  | Copy selection or line if no selection              |
| 2025-12-05 | 7.6   | Cut (Cmd+X)                    | Complete  | Copy then delete selection                          |
| 2025-12-05 | 7.6   | Paste (Cmd+V)                  | Complete  | Multi-cursor aware, line-per-cursor distribution    |
| 2025-12-05 | 7.7   | RectangleSelectionState        | Complete  | Tracks active, start, current positions             |
| 2025-12-05 | 7.7   | StartRectangleSelection        | Complete  | Middle mouse down starts rectangle mode             |
| 2025-12-05 | 7.7   | UpdateRectangleSelection       | Complete  | Mouse drag updates current position                 |
| 2025-12-05 | 7.7   | FinishRectangleSelection       | Complete  | Creates cursors/selections for each line            |
| 2025-12-05 | 7.7   | Rectangle overlay rendering    | Complete  | Shows selection rectangle during drag               |
| 2025-12-05 | 7.7   | Ghost cursor preview           | Complete  | Preview cursors shown during rectangle drag         |
| 2025-12-05 | 7.8   | Selection helper methods       | Complete  | extend_to, collapse_to_start/end, contains          |
| 2025-12-05 | 7.8   | deduplicate_cursors()          | Complete  | Remove duplicate cursor positions                   |
| 2025-12-05 | 7.8   | assert_invariants()            | Complete  | Debug-only cursor/selection invariant checks        |
| 2025-12-05 | 7.8   | AddCursorAbove/Below handlers  | Complete  | Add cursor on adjacent line, preserve column        |
| 2025-12-05 | 7.8   | Double-tap Option detection    | Complete  | 300ms threshold for Option+Option+Arrow             |
| 2025-12-05 | 7.8   | Tests for multi-cursor         | Complete  | 9 new tests (109 total)                             |
| 2025-12-05 | -     | Test refactoring               | Complete  | Moved 101 tests to tests/ folder, 8 in main.rs      |
