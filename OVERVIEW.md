# Developer Overview

Quick reference for navigating the codebase. Press F7 in debug builds to dump app state to JSON.

## Architecture: Elm Pattern

```
┌─────────────────────────────────────────────────────────────────────────┐
│                     EVENT LOOP (src/runtime/app.rs)                      │
│  ApplicationHandler::window_event() → handle_event() → process_cmd()    │
└─────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                       INPUT (src/runtime/input.rs)                       │
│  handle_key() - Maps keyboard/mouse events → Msg types                  │
│  Keymap system routes most keys; handle_key() for special cases         │
└─────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                         MESSAGES (src/messages.rs)                       │
│  Msg::Editor(EditorMsg)     - Cursor, viewport, selection               │
│  Msg::Document(DocumentMsg)  - Text edits, undo/redo, clipboard         │
│  Msg::Layout(LayoutMsg)     - Splits, tabs, groups                      │
│  Msg::Ui(UiMsg)             - Status bar, modals, cursor blink          │
│  Msg::App(AppMsg)           - File I/O, resize, quit                    │
│  Msg::Syntax(SyntaxMsg)     - Tree-sitter syntax highlighting           │
│  Msg::Csv(CsvMsg)           - CSV viewer/editor operations              │
│  Msg::Workspace(WorkspaceMsg) - File tree, sidebar operations           │
│  Msg::TextEdit(...)         - Unified text editing (editable system)    │
└─────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                         UPDATE (src/update/)                             │
│  update() in mod.rs dispatches to:                                       │
│    ├── editor.rs    → EditorMsg handlers                                 │
│    ├── document.rs  → DocumentMsg handlers                               │
│    ├── layout.rs    → LayoutMsg handlers (splits, tabs, focus)           │
│    ├── ui.rs        → UiMsg handlers (status bar, modals)                │
│    ├── app.rs       → AppMsg handlers (file I/O, window)                 │
│    ├── syntax.rs    → SyntaxMsg handlers (highlighting)                  │
│    ├── csv.rs       → CsvMsg handlers (CSV operations)                   │
│    ├── workspace.rs → WorkspaceMsg handlers (file tree)                  │
│    └── text_edit.rs → TextEdit handlers (editable system)                │
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
│    ├── workspace: Option<Workspace> (workspace.rs)                       │
│    │     ├── root_path: PathBuf                                          │
│    │     ├── file_tree: FileTree                                         │
│    │     ├── sidebar_visible: bool                                       │
│    │     └── scroll_offset: usize                                        │
│    ├── ui: UiState                (ui.rs)                                │
│    │     ├── status_bar: StatusBar                                       │
│    │     ├── active_modal: Option<ModalState>                            │
│    │     ├── hover: HoverRegion                                          │
│    │     └── cursor_visible: bool                                        │
│    ├── theme: Theme               (../theme.rs)                          │
│    ├── config: EditorConfig       (../config.rs)                         │
│    └── metrics: UiMetrics         (mod.rs)                               │
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
│  + Damage tracking for partial redraws                                   │
└─────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                         VIEW (src/view/)                                 │
│  Renderer::render() → render_impl() → buffer.present()                   │
│  ├── frame.rs       - Frame, TextPainter abstractions                    │
│  ├── geometry.rs    - Layout calculations, GroupLayout                   │
│  ├── helpers.rs     - Rendering utilities                                │
│  └── text_field.rs  - Text field rendering for modals                    │
└─────────────────────────────────────────────────────────────────────────┘
```

## File Map

```
src/
├── main.rs              Entry point, event loop setup
├── lib.rs               Library exports
├── messages.rs          All Msg types (Editor, Document, Layout, Ui, App, etc.)
├── commands.rs          Cmd enum (side effects + damage tracking)
├── theme.rs             Theme, Color, YAML parsing
├── config.rs            EditorConfig, user settings
├── config_paths.rs      Config directory management
├── overlay.rs           Overlay rendering utilities
├── cli.rs               CLI argument parsing
├── tracing.rs           Debug tracing, cursor snapshots
├── debug_dump.rs        State dump to JSON (F7, debug only)
├── debug_overlay.rs     Debug overlay rendering (F2, debug only)
├── fs_watcher.rs        File system watcher for workspace
│
├── runtime/
│   ├── mod.rs           Runtime module exports
│   ├── app.rs           App struct, ApplicationHandler, event handling
│   ├── input.rs         handle_key() - keyboard shortcuts → Msg
│   └── perf.rs          PerfStats, performance monitoring
│
├── view/
│   ├── mod.rs           Renderer, all drawing code, GlyphCache
│   ├── frame.rs         Frame, TextPainter abstractions
│   ├── geometry.rs      Layout calculations, GroupLayout, helpers
│   ├── helpers.rs       Rendering helper functions
│   └── text_field.rs    Text field rendering for modals
│
├── model/
│   ├── mod.rs           AppModel, ViewportGeometry, UiMetrics
│   ├── document.rs      Document, Rope buffer, EditOperation, undo/redo
│   ├── editor.rs        EditorState, Cursor, Selection, Viewport, ViewMode
│   ├── editor_area.rs   EditorArea, EditorGroup, Tab, LayoutNode, splits
│   ├── status_bar.rs    StatusBar, segments, sync_status_bar()
│   ├── ui.rs            UiState, ModalState, HoverRegion
│   └── workspace.rs     Workspace, FileTree, FileNode
│
├── update/
│   ├── mod.rs           update() dispatcher with tracing
│   ├── editor.rs        Cursor movement, selection, multi-cursor
│   ├── document.rs      Text edits, undo/redo, clipboard
│   ├── layout.rs        Split, close, focus groups/tabs
│   ├── ui.rs            Status bar, cursor blink, modals
│   ├── app.rs           Resize, file save/load, dialogs
│   ├── syntax.rs        Syntax highlighting parse scheduling
│   ├── csv.rs           CSV grid navigation, cell editing
│   ├── workspace.rs     File tree operations, sidebar
│   └── text_edit.rs     Unified text editing dispatch (editable system)
│
├── keymap/              Configurable keybindings system
│   ├── mod.rs           Keymap, KeyAction, Command enum
│   ├── bindings.rs      Default keybinding definitions
│   └── context.rs       KeyContext for conditional bindings
│
├── syntax/              Tree-sitter syntax highlighting
│   ├── mod.rs           Language detection, parser management
│   ├── highlights.rs    Syntax highlight types and colors
│   └── parsers/         Language-specific parsers (Rust, JS, etc.)
│
├── csv/                 CSV viewer/editor
│   ├── mod.rs           CsvState, grid operations
│   └── parser.rs        CSV parsing utilities
│
├── editable/            Unified text editing system
│   ├── mod.rs           EditContext, TextEditMsg
│   └── handlers.rs      Text editing operations
│
└── util/                Utilities
    ├── mod.rs           char_type, word boundary helpers
    ├── file.rs          File validation, binary detection
    └── text.rs          Text manipulation helpers
```

## Rendering Pipeline

```
render()
  │
  ├─► compute_effective_damage()        # Determine what needs redrawing
  │
  ├─► compute_layout_scaled()           # Calculate group rects, splitter positions
  │         └─► stored in group.rect
  │
  ├─► buffer.clear() or partial clear   # Clear based on damage
  │
  ├─► render_sidebar()                  # File tree (if workspace open)
  │       ├─► Background, border
  │       └─► Tree nodes (folders, files)
  │
  ├─► render_editor_area()              # All editor groups
  │       │
  │       └─► For each group:
  │           │
  │           ├─► render_tab_bar()      # Tabs at top of group
  │           │
  │           ├─► render_editor_group()
  │           │       │
  │           │       ├─► Check view mode (text vs CSV)
  │           │       │
  │           │       ├─► TEXT MODE:
  │           │       │   ├─► render_text_area()
  │           │       │   │   ├─► Current line highlight
  │           │       │   │   ├─► Selections (all cursors)
  │           │       │   │   ├─► Syntax-highlighted text
  │           │       │   │   └─► Cursors (blinking)
  │           │       │   └─► render_gutter()
  │           │       │       ├─► Line numbers
  │           │       │       └─► Gutter border
  │           │       │
  │           │       └─► CSV MODE:
  │           │           ├─► render_csv_grid()
  │           │           │   ├─► Grid lines
  │           │           │   ├─► Cell backgrounds
  │           │           │   ├─► Cell text
  │           │           │   └─► Selection highlight
  │           │           └─► render_csv_cell_editor()
  │           │
  │           └─► Dim non-focused groups (4% black overlay)
  │
  ├─► render_splitters()                # Draggable split bars
  │
  ├─► render_status_bar()               # At bottom of window
  │       ├─► Background
  │       ├─► Left segments (mode, file, line/col)
  │       └─► Right segments (encoding, language)
  │
  ├─► render_modals()                   # Modal overlays (if active)
  │       ├─► Dim background (40% black)
  │       ├─► Modal dialog box
  │       ├─► Command palette / Goto line / Find-Replace
  │       └─► Fuzzy file finder
  │
  └─► render_drop_overlay()             # File drag-and-drop indicator
```

## Key Locations

### Finding Specific Rendering

| What                               | File               | Function / Line                |
| ---------------------------------- | ------------------ | ------------------------------ |
| **Main render loop**               | `src/view/mod.rs`  | `render()` L2034               |
| **Damage computation**             | `src/view/mod.rs`  | `compute_effective_damage()`   |
| **Editor group rendering**         | `src/view/mod.rs`  | `render_editor_group()` L535   |
| **Tab bar**                        | `src/view/mod.rs`  | `render_tab_bar()` L597        |
| **Text area (main editor)**        | `src/view/mod.rs`  | `render_text_area()` L917      |
| **Line numbers/gutter**            | `src/view/mod.rs`  | `render_gutter()` L858         |
| **CSV grid**                       | `src/view/mod.rs`  | `render_csv_grid()` L1216      |
| **CSV cell editor**                | `src/view/mod.rs`  | `render_csv_cell_editor()`     |
| **Splitter bars**                  | `src/view/mod.rs`  | `render_splitters()` L654      |
| **Status bar**                     | `src/view/mod.rs`  | `render_status_bar()` L1517    |
| **Modals**                         | `src/view/mod.rs`  | `render_modals()` L1569        |
| **Sidebar (file tree)**            | `src/view/mod.rs`  | `render_sidebar()` L663        |
| **Drop overlay**                   | `src/view/mod.rs`  | `render_drop_overlay()` L1999  |
| **Geometry calculations**          | `src/view/geometry.rs` | `GroupLayout`, helpers     |
| **Text field (modal input)**       | `src/view/text_field.rs` | `TextFieldRenderer`      |

### Finding Specific Logic

| What                        | File                         | Function                                  |
| --------------------------- | ---------------------------- | ----------------------------------------- |
| **Event handling**          | `src/runtime/app.rs`         | `handle_event()` L393                     |
| **Keyboard input routing**  | `src/runtime/input.rs`       | `handle_key()`                            |
| **Modal key handling**      | `src/runtime/input.rs`       | `handle_modal_key()`                      |
| **CSV key handling**        | `src/runtime/input.rs`       | `handle_csv_edit_key()`                   |
| **Sidebar key handling**    | `src/runtime/input.rs`       | `handle_sidebar_key()`                    |
| **Keymap system**           | `src/keymap/mod.rs`          | `Keymap::handle()`                        |
| **Default keybindings**     | `src/keymap/bindings.rs`     | `load_default_keymap()`                   |
| **Update dispatcher**       | `src/update/mod.rs`          | `update()`                                |
| **Cursor movement**         | `src/update/editor.rs`       | `update_editor()`                         |
| **Text insertion**          | `src/update/document.rs`     | `update_document()`                       |
| **Split/tab operations**    | `src/update/layout.rs`       | `update_layout()`                         |
| **Undo/redo**               | `src/update/document.rs`     | `handle_undo()`, `handle_redo()`          |
| **Multi-cursor logic**      | `src/update/editor.rs`       | `add_cursor_*`, `merge_*`                 |
| **Viewport scrolling**      | `src/update/editor.rs`       | `handle_scroll()`, `ensure_cursor_visible()` |
| **Status bar sync**         | `src/model/status_bar.rs`    | `sync_status_bar()`                       |
| **Syntax highlighting**     | `src/update/syntax.rs`       | `update_syntax()`, `schedule_parse()`     |
| **CSV operations**          | `src/update/csv.rs`          | `update_csv()`                            |
| **Workspace operations**    | `src/update/workspace.rs`    | `update_workspace()`                      |
| **File tree navigation**    | `src/model/workspace.rs`     | `FileTree` methods                        |

## Data Flow Example: Typing a Character

```
1. WindowEvent::KeyboardInput { key: 'a', ... }
   └─► src/runtime/app.rs: handle_event()

2. Keymap::handle(...) or handle_key(..., Key::Character("a"), ...)
   └─► src/runtime/input.rs: returns Msg::Document(DocumentMsg::InsertChar('a'))

3. update(model, Msg::Document(DocumentMsg::InsertChar('a')))
   └─► src/update/mod.rs: dispatches to document::update_document()

4. update_document(model, DocumentMsg::InsertChar('a'))
   └─► src/update/document.rs:
       - Deletes selection (if any)
       - Inserts char at cursor position
       - Updates undo stack
       - Moves cursor
       - Returns Some(Cmd::Redraw with damage info)

5. sync_status_bar(model)
   └─► src/model/status_bar.rs: updates line/col, modified indicator

6. process_cmd(Cmd::Redraw)
   └─► src/runtime/app.rs: window.request_redraw()
       - Accumulates damage for partial redraw

7. WindowEvent::RedrawRequested
   └─► render() → compute_effective_damage() → selective redraw → buffer.present()
```

## Data Flow Example: Opening Command Palette

```
1. WindowEvent::KeyboardInput { key: 'P', shift: true, logo: true }
   └─► src/runtime/app.rs: handle_event()

2. Keymap matches Cmd+Shift+P → Command::OpenCommandPalette
   └─► Converts to Msg::Ui(UiMsg::Modal(ModalMsg::OpenCommandPalette))

3. update(model, Msg::Ui(UiMsg::Modal(ModalMsg::OpenCommandPalette)))
   └─► src/update/ui.rs: sets model.ui.active_modal = Some(ModalState::CommandPalette)

4. Subsequent keys route to handle_modal_key()
   └─► src/runtime/input.rs: handles modal input separately

5. render() includes render_modals()
   └─► Draws dimmed background + modal dialog + command list
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
│       ├── view_mode: ViewMode              # Text or Csv(CsvState)
│       ├── cursors: Vec<Cursor>             # Multi-cursor support
│       ├── selections: Vec<Selection>
│       └── viewport: Viewport
│
└── documents: HashMap<DocumentId, Document>
    └── Document
        ├── buffer: Rope                     # Text content (ropey crate)
        ├── file_path: Option<PathBuf>
        ├── language: Option<LanguageId>     # For syntax highlighting
        ├── syntax_highlights: Option<SyntaxHighlights>
        ├── undo_stack: Vec<EditOperation>
        └── redo_stack: Vec<EditOperation>
```

## Workspace Structure

```
Workspace
├── root_path: PathBuf                       # Workspace root directory
├── file_tree: FileTree                      # Tree of files/folders
├── sidebar_visible: bool                    # Sidebar toggle state
├── scroll_offset: usize                     # Vertical scroll position
└── selected_path: Option<PathBuf>           # Currently selected file

FileTree
└── root: FileNode                           # Root directory node

FileNode
├── name: String                             # File/folder name
├── path: PathBuf                            # Full path
├── is_dir: bool                             # Directory vs file
├── expanded: bool                           # Folder expansion state (dirs only)
└── children: Vec<FileNode>                  # Child nodes (dirs only)
```

## Modal System

```
ModalState (enum)
├── CommandPalette(CommandPaletteState)      # Cmd+Shift+P
│   ├── input: String                        # Search query
│   ├── cursor: usize                        # Input cursor position
│   ├── selection: Option<Range>             # Input selection
│   ├── filtered_commands: Vec<Command>      # Matching commands
│   └── selected_index: usize                # List selection
│
├── GotoLine(GotoLineState)                  # Cmd+G
│   ├── input: String                        # Line number input
│   └── cursor: usize
│
├── FindReplace(FindReplaceState)            # Cmd+F
│   ├── query: String                        # Search query
│   ├── replace: String                      # Replacement text
│   ├── active_field: FindReplaceField       # Query or Replace
│   ├── case_sensitive: bool
│   └── cursor positions, selections
│
├── FileFinder(FileFinderState)              # Cmd+Shift+O
│   ├── input: String                        # Fuzzy search query
│   ├── filtered_files: Vec<FileMatch>       # Matching workspace files
│   └── selected_index: usize
│
└── ThemePicker(ThemePickerState)            # Theme selection modal
    ├── filtered_themes: Vec<String>
    └── selected_index: usize
```

## Debug Tools

| Key | Action                     | File                        |
| --- | -------------------------- | --------------------------- |
| F2  | Toggle performance overlay | `src/debug_overlay.rs`      |
| F7  | Dump state to JSON         | `src/debug_dump.rs`         |

## Theme Colors (where used)

| Color                              | Usage                 | View.rs location     |
| ---------------------------------- | --------------------- | -------------------- |
| `theme.editor.background`          | Main background       | `render()` clear     |
| `theme.editor.foreground`          | Text color            | `render_text_area()` |
| `theme.editor.line_number`         | Gutter numbers        | `render_gutter()`    |
| `theme.editor.current_line`        | Line highlight        | `render_text_area()` |
| `theme.editor.selection`           | Selection bg          | `render_text_area()` |
| `theme.editor.cursor_color`        | Cursor                | `render_text_area()` |
| `theme.editor.gutter_border`       | Gutter separator      | `render_gutter()`    |
| `theme.status_bar.background`      | Status bar bg         | `render_status_bar()` |
| `theme.status_bar.foreground`      | Status bar text       | `render_status_bar()` |
| `theme.tab_bar.background`         | Tab bar bg            | `render_tab_bar()`   |
| `theme.tab_bar.active_background`  | Active tab bg         | `render_tab_bar()`   |
| `theme.tab_bar.active_foreground`  | Active tab text       | `render_tab_bar()`   |
| `theme.overlay.background`         | Modal dialog bg       | `render_modals()`    |
| `theme.overlay.foreground`         | Modal text            | `render_modals()`    |
| `theme.overlay.highlight`          | Modal highlights      | `render_modals()`    |
| `theme.sidebar.background`         | Sidebar bg            | `render_sidebar()`   |
| `theme.sidebar.foreground`         | File tree text        | `render_sidebar()`   |
| `theme.sidebar.selection_background` | Selected file bg    | `render_sidebar()`   |
| `theme.splitter.background`        | Split bars            | `render_splitters()` |
| `theme.syntax.*`                   | Code highlighting     | `render_text_area()` |

## Key Subsystems

### Keymap System (`src/keymap/`)

- **Purpose**: Configurable keybindings without hardcoding
- **Key Types**:
  - `KeyAction`: Keystroke with modifiers
  - `Command`: High-level editor command (enum with ~100 variants)
  - `KeyContext`: Conditions for when binding is active
  - `Keymap`: Maps KeyAction → Command
- **Flow**: Keystroke → Keymap::handle() → Command → Msg conversion → update()

### Syntax Highlighting (`src/syntax/`)

- **Purpose**: Tree-sitter based syntax highlighting
- **Architecture**: Worker thread parses in background, sends results via channel
- **Supports**: ~20 languages (Rust, JavaScript, Python, etc.)
- **Integration**: Document stores `syntax_highlights`, renderer uses for colors

### CSV Viewer (`src/csv/`)

- **Purpose**: Spreadsheet-like CSV viewing/editing
- **ViewMode**: Editor switches between Text and Csv modes
- **Features**: Grid navigation, cell editing, column resizing
- **Rendering**: Separate `render_csv_grid()` pipeline

### Editable System (`src/editable/`)

- **Purpose**: Unified text editing across contexts (editor, modals, CSV cells)
- **Messages**: `TextEdit(EditContext, TextEditMsg)`
- **Goal**: Phase 2 refactoring to consolidate all text operations

### Workspace (`src/workspace/`)

- **Purpose**: Project-level file management
- **Features**: File tree sidebar, fuzzy file finder, file watcher
- **Integration**: Optional sidebar on left, watch for file changes

## Configuration

- **Config file**: `~/.config/token/config.toml`
- **Themes**: `~/.config/token/themes/*.yaml`
- **Structure**: `EditorConfig` in `src/config.rs`
- **Hot reload**: `Cmd+Shift+R` reloads config and theme