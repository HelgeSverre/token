# Codebase Architecture Review: Elm-Style Architecture Scalability

**Date:** December 2024  
**Reviewed by:** Oracle + Librarian analysis  
**Scope:** Will the current Elm-style architecture scale for a production-quality code editor?

---

## Executive Summary

**Verdict: Keep the architecture, but refactor the structure.**

The Elm-style unidirectional data flow (`Msg → update → Model → view`) is fundamentally sound and used successfully by production editors. The current implementation needs **better factoring** (split Model into sub-models) and an **upgraded Cmd layer** for async operations—not a wholesale redesign.

| Aspect | Current State | Recommendation |
|--------|--------------|----------------|
| Core pattern | ✅ Solid | Keep `Msg + update + Cmd` |
| Model structure | ⚠️ Monolithic | Split into Document/EditorState/UiState |
| Cmd system | ⚠️ Underpowered | Expand for async (LSP, file I/O) |
| Performance | ✅ Fine for now | Address O(n) algorithms later |

**Estimated refactor effort:** M–L (1–2 days for structure, incremental after)

---

## 1. Current Architecture Analysis

### What You Have

```
┌─────────────────────────────────────────────────────────────────┐
│                           Model                                 │
│  ┌──────────┐ ┌────────┐ ┌──────────┐ ┌─────────────────────┐  │
│  │  buffer  │ │ cursor │ │ viewport │ │ undo/redo, file,    │  │
│  │  (Rope)  │ │        │ │          │ │ status, blink, etc  │  │
│  └──────────┘ └────────┘ └──────────┘ └─────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  Msg (flat enum: ~25 variants)                                  │
│  MoveCursorUp | InsertChar | SaveFile | ScrollViewport | ...    │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  update(&mut Model, Msg) -> Option<Cmd>                         │
│  - Mutates model in place (hybrid Elm)                          │
│  - Returns Cmd::Redraw or None                                  │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  Cmd::Redraw  (only option currently)                           │
└─────────────────────────────────────────────────────────────────┘
```

### Strengths

1. **Unidirectional data flow** — All state changes go through `update`, making behavior predictable
2. **Single source of truth** — One `Model` owns all state
3. **Explicit events** — `Msg` enum documents every possible state transition
4. **Rust-friendly** — Mutable `&mut Model` avoids expensive cloning while keeping the pattern
5. **Testable** — Pure `update` function is easy to unit test (you already have 50+ tests)

### Weaknesses

1. **Monolithic Model** — Document, view, and UI state are intermingled
2. **Flat Msg enum** — Will grow unwieldy (VS Code has 1000+ commands)
3. **Underpowered Cmd** — Only `Redraw`; no async support
4. **Missing abstractions** — No Selection type, no visual line index, no coordinate systems
5. **Blocking I/O** — `SaveFile` does sync I/O inside `update`

---

## 2. Comparison with Production Editors

### Helix (Rust, ~100k LOC)

**Architecture:** Event-driven with clear crate separation

```
helix-core     → Pure text operations (Rope, Transaction, Selection)
helix-view     → Document, View, Editor state
helix-term     → Terminal UI, event loop
helix-event    → Hook/event dispatch system
```

**Key patterns:**
- Selections stored per `(Document, View)` tuple
- Async via job queue that returns callbacks
- Immutable selection transformations (Kakoune-style)

### Zed (Rust, ~300k LOC)

**Architecture:** Entity-based with GPUI framework

```
text           → Buffer, Anchor (timestamped positions)
multi_buffer   → Multi-file editing abstraction
editor         → Editor entity (1200+ line struct)
gpui           → UI framework with actions
```

**Key patterns:**
- Anchors use Lamport timestamps for CRDT collaboration
- Task-based async with `cx.spawn()` and `entity.update()`
- Tighter coupling between buffer and view

### Relevance to Your Architecture

| Pattern | Helix | Zed | Your Editor | Recommendation |
|---------|-------|-----|-------------|----------------|
| State mutation | Callbacks from jobs | Entity.update() | `&mut Model` | ✅ Keep current |
| Document/View split | Separate crates | Multi-buffer abstraction | Monolithic | ⚠️ Split Model |
| Async model | Job queue + channels | Task spawning | None | ⚠️ Add Cmd variants |
| Selection storage | Per (Doc, View) | Anchor-based | Single cursor | ⚠️ Add multi-cursor |

**Conclusion:** Both production editors use targeted mutability (not pure Elm), but they maintain clear separation of concerns. Your architecture is compatible with either approach.

---

## 3. Recommended Evolution Path

### Phase 1: Split Model (Small, 2-4 hours)

Extract domain types that match `EDITOR_UI_REFERENCE.md`:

```rust
// Before: Everything in Model
struct Model {
    buffer: Rope,
    cursor: Cursor,
    viewport: Viewport,
    undo_stack: Vec<EditOperation>,
    file_path: Option<PathBuf>,
    is_modified: bool,
    status_message: String,
    cursor_visible: bool,
    // ... etc
}

// After: Clear separation
struct Document {
    id: DocumentId,
    buffer: Rope,
    file_path: Option<PathBuf>,
    is_modified: bool,
    undo_stack: Vec<EditOperation>,
    redo_stack: Vec<EditOperation>,
}

struct EditorState {
    document_id: DocumentId,
    cursors: Vec<Cursor>,           // Ready for multi-cursor
    selections: Vec<Selection>,     // Ready for selections
    scroll_offset: ScrollOffset,    // Pixels, not lines
    viewport_size: ViewportSize,
}

struct UiState {
    status_message: String,
    cursor_visible: bool,
    last_cursor_blink: Instant,
    // Later: overlays, popups
}

struct AppModel {
    documents: HashMap<DocumentId, Document>,
    editors: HashMap<EditorId, EditorState>,
    focused_editor: EditorId,
    ui: UiState,
}
```

### Phase 2: Nested Messages (Small, 1-2 hours)

Prevent `Msg` enum explosion:

```rust
// Before: Flat enum
enum Msg {
    MoveCursorUp,
    MoveCursorDown,
    InsertChar(char),
    SaveFile,
    ScrollViewport(i32),
    // ... 25+ variants
}

// After: Nested by domain
enum Msg {
    Editor(EditorId, EditorMsg),
    Document(DocumentId, DocumentMsg),
    Ui(UiMsg),
    App(AppMsg),
}

enum EditorMsg {
    MoveCursor(Direction),
    Scroll(ScrollDelta),
    SetCursorPosition { line: usize, column: usize },
    // Editor-specific actions
}

enum DocumentMsg {
    Insert { pos: usize, text: String },
    Delete { range: Range },
    Undo,
    Redo,
}

enum UiMsg {
    SetStatus(String),
    BlinkCursor,
}

enum AppMsg {
    OpenFile(PathBuf),
    SaveFile(DocumentId),
    NewFile,
}
```

Then delegate in `update`:

```rust
fn update(app: &mut AppModel, msg: Msg) -> Option<Cmd> {
    match msg {
        Msg::Editor(id, emsg) => update_editor(app, id, emsg),
        Msg::Document(id, dmsg) => update_document(app, id, dmsg),
        Msg::Ui(umsg) => update_ui(&mut app.ui, umsg),
        Msg::App(amsg) => update_app(app, amsg),
    }
}
```

### Phase 3: Expand Cmd for Async (Medium, 4-8 hours)

```rust
// Before
enum Cmd {
    Redraw,
}

// After
enum Cmd {
    Redraw,
    SaveFile { document_id: DocumentId },
    LoadFile { path: PathBuf },
    SpawnTask(Task),
    // Future: LSP requests, background tokenization
}

// Event loop handles commands
fn process_cmd(cmd: Cmd, tx: Sender<Msg>, app: &AppModel) {
    match cmd {
        Cmd::Redraw => { /* trigger repaint */ }
        Cmd::SaveFile { document_id } => {
            let doc = app.documents[&document_id].clone();
            std::thread::spawn(move || {
                let result = std::fs::write(&doc.file_path.unwrap(), doc.buffer.to_string());
                tx.send(Msg::Document(document_id, DocumentMsg::SaveCompleted(result))).ok();
            });
        }
        Cmd::LoadFile { path } => {
            std::thread::spawn(move || {
                let result = std::fs::read_to_string(&path);
                tx.send(Msg::App(AppMsg::FileLoaded { path, result })).ok();
            });
        }
    }
}
```

### Phase 4: Add Core Types (Medium, ongoing)

Add types from `EDITOR_UI_REFERENCE.md` as needed:

```rust
// Selection (Chapter 2)
struct Selection {
    start: Position,
    end: Position,
    direction: SelectionDirection,
}

// Visual line index for soft wrapping (Chapter 6)
struct VisualLineIndex {
    document_to_visual: Vec<VisualLineMapping>,
    visual_to_document: Vec<DocumentPosition>,
    total_visual_lines: usize,
}

// Scroll offset in pixels (Chapter 4)
struct ScrollOffset {
    x: f32,
    y: f32,
}
```

---

## 4. Performance Considerations

### Current Bottlenecks (Minor)

1. **O(n) line scanning** in `cursor_buffer_position` and `set_cursor_from_position`
   - Fix: Use ropey's `line_to_char` / `char_to_line` (O(log n))

2. **String allocations** in word navigation
   - Fix: Iterate over rope slices directly

3. **Full redraw on every change**
   - Acceptable for now; modern GPUs handle this easily
   - Future: Track dirty lines for partial repaint

### Not a Problem

- **Mutable update pattern** — This is idiomatic Rust and performant
- **Single-threaded event loop** — Standard for GUI apps
- **Rope operations** — Ropey is highly optimized

---

## 5. What's Missing vs Production Editors

| Feature | Status | Priority | Effort |
|---------|--------|----------|--------|
| Multi-cursor | Missing | High | M |
| Selections | Missing | High | M |
| Visual line index | Missing | Medium | M |
| Pixel-based scroll | Missing | Medium | S |
| Syntax highlighting | Missing | Medium | L |
| LSP integration | Missing | Low | L |
| Overlays (autocomplete) | Missing | Low | M |
| Split views | Missing | Low | L |

---

## 6. Risks of NOT Refactoring

1. **Monolithic enums grow** — `Msg` and `Model` become 2000+ lines, hard to reason about
2. **Hidden O(n²)** — Ad-hoc line calculations scattered through codebase
3. **Async bolted on** — Blocking I/O causes UI jank
4. **Feature coupling** — Adding selections affects unrelated code

---

## 7. When to Consider Different Architecture

Revisit the design if:

- You add a **plugin system** requiring strong isolation
- **Compile times** become problematic from large enums
- You hit **performance issues** after addressing obvious bottlenecks
- You need **real-time collaboration** (consider CRDT-based anchors like Zed)

At that point, consider:
- Command bus / event bus for decoupling
- Reactive state (signals) for some subsystems
- ECS for very dynamic editor components

---

## 8. Conclusion

**The Elm-style architecture is a solid foundation.** Both Helix and Zed use variations of message-passing with targeted mutability. Your current implementation needs:

1. ✅ **Keep** the `Msg → update → Model → view` pattern
2. ⚠️ **Split** Model into Document/EditorState/UiState
3. ⚠️ **Nest** Msg enum by domain
4. ⚠️ **Expand** Cmd for async operations
5. ⚠️ **Add** Selection and multi-cursor support

This is incremental work, not a rewrite. The architecture will scale to a capable editor with multi-cursor, soft wrap, overlays, and even LSP.

---

## Appendix: Quick Reference

### Current Architecture (keep)
```
KeyEvent → Msg → update(&mut Model) → Cmd::Redraw → render()
```

### Target Architecture (evolve to)
```
KeyEvent → Msg::Editor(id, EditorMsg) → update_editor() → Cmd::Redraw
                                                        → Cmd::SaveFile
                                                        → Cmd::SpawnTask
         ↓
     AppModel {
         documents: HashMap<DocumentId, Document>,
         editors: HashMap<EditorId, EditorState>,
         ui: UiState,
     }
```

### Files to Create/Modify

| File | Purpose |
|------|---------|
| `src/document.rs` | Document struct (buffer, undo, file metadata) |
| `src/editor_state.rs` | EditorState (cursors, selections, viewport) |
| `src/selection.rs` | Selection type and operations |
| `src/messages.rs` | Nested Msg enums |
| `src/commands.rs` | Expanded Cmd enum and processing |
