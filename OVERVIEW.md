# Developer Overview

Quick reference for navigating the codebase. Press F7 in debug builds to dump app state to JSON.

## Architecture: Elm Pattern

```
┌─────────────────────────────────────────────────────────────────────────┐
│                           EVENT LOOP (src/app.rs)                        │
│  ApplicationHandler::window_event() → handle_event() → process_cmd()    │
└─────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                         INPUT (src/input.rs)                             │
│  handle_key() - Maps keyboard/mouse events → Msg types                  │
└─────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                         MESSAGES (src/messages.rs)                       │
│  Msg::Editor(EditorMsg)    - Cursor, viewport, selection                │
│  Msg::Document(DocumentMsg) - Text edits, undo/redo, clipboard          │
│  Msg::Layout(LayoutMsg)    - Splits, tabs, groups                       │
│  Msg::Ui(UiMsg)            - Status bar, cursor blink                   │
│  Msg::App(AppMsg)          - File I/O, resize, quit                     │
└─────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                         UPDATE (src/update/)                             │
│  update() in mod.rs dispatches to:                                       │
│    ├── editor.rs   → EditorMsg handlers                                  │
│    ├── document.rs → DocumentMsg handlers                                │
│    ├── layout.rs   → LayoutMsg handlers (splits, tabs, focus)            │
│    ├── ui.rs       → UiMsg handlers                                      │
│    └── app.rs      → AppMsg handlers                                     │
└─────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                         MODEL (src/model/)                               │
│  AppModel (mod.rs)                                                       │
│    ├── editor_area: EditorArea    (editor_area.rs)                       │
│    │     ├── documents: HashMap<DocumentId, Document>                    │
│    │     ├── editors: HashMap<EditorId, EditorState>                     │
│    │     ├── groups: HashMap<GroupId, EditorGroup>                       │
│    │     ├── layout: LayoutNode (tree of splits/groups)                  │
│    │     └── focused_group_id: GroupId                                   │
│    ├── ui: UiState                (ui.rs)                                │
│    │     ├── status_bar: StatusBar                                       │
│    │     └── cursor_visible: bool                                        │
│    └── theme: Theme               (../theme.rs)                          │
└─────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                         COMMANDS (src/commands.rs)                       │
│  Cmd::None      - No action                                              │
│  Cmd::Redraw    - Request UI refresh                                     │
│  Cmd::SaveFile  - Async file save                                        │
│  Cmd::LoadFile  - Async file load                                        │
│  Cmd::Batch     - Multiple commands                                      │
└─────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                         VIEW (src/view.rs)                               │
│  Renderer::render() → render_impl() → buffer.present()                   │
└─────────────────────────────────────────────────────────────────────────┘
```

## File Map

```
src/
├── main.rs              Entry point, event loop setup
├── app.rs               App struct, ApplicationHandler, event handling
├── input.rs             handle_key() - keyboard shortcuts → Msg
├── view.rs              Renderer, all drawing code
├── messages.rs          Msg, EditorMsg, DocumentMsg, LayoutMsg, UiMsg, AppMsg
├── commands.rs          Cmd enum (side effects)
├── lib.rs               Library exports
├── theme.rs             Theme, Color, YAML parsing
├── overlay.rs           Overlay rendering utilities
├── util.rs              char_type, word boundary helpers
├── perf.rs              Debug performance overlay (F2)
├── debug_dump.rs        State dump to JSON (F7, debug only)
│
├── model/
│   ├── mod.rs           AppModel, layout constants
│   ├── document.rs      Document, Rope buffer, EditOperation, undo/redo
│   ├── editor.rs        EditorState, Cursor, Selection, Viewport
│   ├── editor_area.rs   EditorArea, EditorGroup, Tab, LayoutNode, splits
│   ├── status_bar.rs    StatusBar, segments, sync_status_bar()
│   └── ui.rs            UiState
│
└── update/
    ├── mod.rs           update() dispatcher
    ├── editor.rs        Cursor movement, selection, multi-cursor
    ├── document.rs      Text edits, undo/redo, clipboard
    ├── layout.rs        Split, close, focus groups/tabs
    ├── ui.rs            Status bar, cursor blink
    └── app.rs           Resize, file save/load
```

## Rendering Pipeline

```
render()
  │
  ├─► compute_layout(available_rect)     # Calculate group rects, splitter positions
  │         └─► stored in group.rect
  │
  ├─► buffer.fill(bg_color)              # Clear background
  │
  ├─► render_all_groups_static()         # For each group:
  │       │
  │       ├─► render_tab_bar_static()    # Tab bar at top of group
  │       │
  │       ├─► render_editor_group_static()
  │       │       ├─► Line numbers (gutter)
  │       │       ├─► Gutter border
  │       │       ├─► Current line highlight
  │       │       ├─► Selections (for ALL cursors)
  │       │       ├─► Text content (with syntax/tab expansion)
  │       │       ├─► Cursors (blinking)
  │       │       └─► Focus border (if focused && multiple groups)  ◄── LINE 471-500
  │       │
  │       └─► render_splitters_static()  # Draggable split bars
  │
  └─► Status bar rendering               # At bottom of window
```

## Key Locations

### Finding Specific Rendering

| What | File | Line/Function |
|------|------|---------------|
| **Focus border (group highlight)** | `src/view.rs` | `render_editor_group_static()` ~L471-500 |
| Tab bar | `src/view.rs` | `render_tab_bar_static()` L504 |
| Line numbers/gutter | `src/view.rs` | `render_editor_group_static()` ~L360-400 |
| Current line highlight | `src/view.rs` | `render_editor_group_static()` ~L310-330 |
| Selection highlight | `src/view.rs` | `render_editor_group_static()` ~L335-360 |
| Cursor drawing | `src/view.rs` | `render_editor_group_static()` ~L420-450 |
| Splitter bars | `src/view.rs` | `render_splitters_static()` L601 |
| Status bar | `src/view.rs` | `render_impl()` ~L689-720 |
| Text/glyphs | `src/view.rs` | `draw_text()` L862 |

### Finding Specific Logic

| What | File | Function |
|------|------|----------|
| Keyboard shortcuts | `src/input.rs` | `handle_key()` |
| Mouse click handling | `src/app.rs` | `handle_event()` ~L225-300 |
| Click to focus group | `src/app.rs` | `handle_event()` ~L244-254 |
| Cursor movement | `src/update/editor.rs` | `update_editor()` |
| Text insertion | `src/update/document.rs` | `update_document()` |
| Split/tab operations | `src/update/layout.rs` | `update_layout()` |
| Undo/redo | `src/update/document.rs` | `handle_undo()`, `handle_redo()` |
| Multi-cursor logic | `src/update/editor.rs` | various `add_cursor_*`, `merge_*` |
| Viewport scrolling | `src/update/editor.rs` | `handle_scroll()`, `ensure_cursor_visible()` |
| Status bar sync | `src/model/status_bar.rs` | `sync_status_bar()` |

## Data Flow Example: Typing a Character

```
1. WindowEvent::KeyboardInput { key: 'a', ... }
   └─► src/app.rs: handle_event()

2. handle_key(..., Key::Character("a"), ...)
   └─► src/input.rs: returns Msg::Document(DocumentMsg::InsertChar('a'))

3. update(model, Msg::Document(DocumentMsg::InsertChar('a')))
   └─► src/update/mod.rs: dispatches to document::update_document()

4. update_document(model, DocumentMsg::InsertChar('a'))
   └─► src/update/document.rs:
       - Deletes selection (if any)
       - Inserts char at cursor position
       - Updates undo stack
       - Moves cursor
       - Returns Some(Cmd::Redraw)

5. sync_status_bar(model)
   └─► src/model/status_bar.rs: updates line/col, modified indicator

6. process_cmd(Cmd::Redraw)
   └─► src/app.rs: window.request_redraw()

7. WindowEvent::RedrawRequested
   └─► render() → buffer.present()
```

## Data Flow Example: Clicking to Focus a Group

```
1. WindowEvent::MouseInput { button: Left, ... } at (x, y)
   └─► src/app.rs: handle_event()

2. model.editor_area.group_at_point(x, y)
   └─► src/model/editor_area.rs: finds GroupId containing point

3. update(model, Msg::Layout(LayoutMsg::FocusGroup(group_id)))
   └─► src/update/layout.rs: sets focused_group_id

4. Continue with cursor positioning in the clicked group...
```

## Layout Tree Structure

```
EditorArea
├── layout: LayoutNode (root of tree)
│   ├── LayoutNode::Group(GroupId)           # Leaf: single editor group
│   └── LayoutNode::Split(SplitContainer)    # Branch: contains children
│         ├── direction: Horizontal | Vertical
│         ├── children: Vec<LayoutNode>
│         └── ratios: Vec<f32>               # How to divide space
│
├── groups: HashMap<GroupId, EditorGroup>
│   └── EditorGroup
│       ├── tabs: Vec<Tab>                   # Each tab → EditorId
│       ├── active_tab_index: usize
│       └── rect: Rect                       # Computed by compute_layout()
│
├── editors: HashMap<EditorId, EditorState>
│   └── EditorState
│       ├── document_id: DocumentId
│       ├── cursors: Vec<Cursor>             # Multi-cursor support
│       ├── selections: Vec<Selection>
│       └── viewport: Viewport
│
└── documents: HashMap<DocumentId, Document>
    └── Document
        ├── buffer: Rope                     # Text content (ropey crate)
        ├── file_path: Option<PathBuf>
        ├── undo_stack: Vec<EditOperation>
        └── redo_stack: Vec<EditOperation>
```

## Debug Tools

| Key | Action | File |
|-----|--------|------|
| F2 | Toggle performance overlay | `src/perf.rs` |
| F7 | Dump state to JSON | `src/debug_dump.rs` |

## Theme Colors (where used)

| Color | Usage | View.rs location |
|-------|-------|------------------|
| `theme.editor.background` | Main background | `buffer.fill()` |
| `theme.editor.foreground` | Text color | `draw_text()` calls |
| `theme.editor.line_number` | Gutter numbers | ~L380 |
| `theme.editor.current_line` | Line highlight | ~L320 |
| `theme.editor.selection` | Selection bg | ~L340 |
| `theme.editor.cursor_color` | Cursor + focus border | ~L440, ~L472 |
| `theme.editor.gutter_border` | Gutter separator | ~L465 |
| `theme.status_bar.background` | Status bar bg | ~L689 |
| `theme.status_bar.foreground` | Status bar text | ~L690 |
| `theme.tab_bar.background` | Tab bar bg | `render_tab_bar_static()` |
| `theme.tab_bar.active_tab` | Active tab color | `render_tab_bar_static()` |
