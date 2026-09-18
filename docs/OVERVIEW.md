# Developer Overview

Quick reference for navigating the codebase. Press F7 in debug builds to dump app state to JSON.

## Architecture: Elm Pattern

`AppModel::new(width, height, scale)` creates an empty model with in-memory
defaults; `AppModel::with_document` accepts already prepared text. Neither reads
configuration, histories or file paths. Runtime application preparation loads
configuration/history and routes startup files through the same file preparation
and message-based tab installation used by later opens. Initial syntax/LSP work
is dispatched after the startup session is prepared.

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
│  Msg::Image / Msg::Preview  - Image tabs, markdown/HTML preview         │
│  Msg::Workspace(WorkspaceMsg) - File tree, sidebar operations           │
│  Msg::Dock / Outline / Problems / Usages / Terminal - Dock panels       │
│  Msg::Completion / Lsp / Formatting - Language tooling                  │
│  Msg::ContextMenu(ContextMenuMsg) - Right-click menus                   │
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
│    ├── image.rs / preview.rs / dock.rs / outline.rs / problems.rs /      │
│    │   usages.rs / terminal.rs / completion.rs / lsp.rs /                │
│    │   formatting.rs / context_menu.rs → one handler per Msg variant     │
│    └── text_edits.rs → shared edit transactions (typing, completion,     │
│                        Find, LSP TextEdit/WorkspaceEdit)                 │
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
│    │     ├── layout: LayoutNode (tree of splits/groups/previews)         │
│    │     └── focused_group_id: GroupId                                   │
│    ├── workspace: Option<Workspace> (workspace.rs)                       │
│    │     ├── root: PathBuf                                               │
│    │     ├── file_tree: FileTree                                         │
│    │     ├── expanded_folders / selected_item                            │
│    │     ├── sidebar_visible: bool, sidebar_width_logical: f32           │
│    │     └── scroll_offset: usize                                        │
│    ├── ui: UiState                (ui.rs)                                │
│    │     ├── keymap: Keymap                                              │
│    │     ├── status_bar: StatusBar                                       │
│    │     ├── active_modal: Option<ModalState>                            │
│    │     ├── find_bar: Option<FindReplaceState>                          │
│    │     ├── hover: HoverRegion, focus: FocusTarget                      │
│    │     ├── completion, hover_card, context_menu, signature_help        │
│    │     └── cursor_visible: bool                                        │
│    ├── dock_layout, terminal, outline_panel, problems_panel,             │
│    │   usages_panel                (dock panels)                         │
│    ├── recent_files, command_history, jump_history, lsp                  │
│    ├── theme: Theme               (../theme.rs)                          │
│    ├── config: EditorConfig       (../config.rs)                         │
│    └── metrics: ScaledMetrics     (mod.rs)                               │
└─────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                         COMMANDS (src/commands.rs)                       │
│  Cmd::None / Redraw / RedrawAreas / Batch   - Redraw + damage tracking  │
│  Cmd::SaveFile / LoadFile / ObserveFile /   - Async file I/O            │
│      PrepareFileOpen / ResolveFilePolicy                                 │
│  Cmd::ScheduleAutoSave / CancelAutoSave     - Auto-save timers          │
│  Cmd::DebouncedSyntaxParse / RunSyntaxParse - Syntax worker             │
│  Cmd::Lsp* / RunFormatter / RunFindSearch / - Language + search workers │
│      WorkspaceSymbols / *Inline* / CompletePaths                         │
│  Cmd::SpawnTerminal / CloseTerminal, dialogs, clipboard, config, Quit   │
└─────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                         VIEW (src/view/)                                 │
│  Renderer::render() → render_with_preview_snapshots() → present         │
│  ├── frame.rs       - Frame, TextPainter abstractions                    │
│  ├── geometry.rs    - Layout calculations, GroupLayout                   │
│  ├── editor_text.rs - render_text_area(), render_gutter()                │
│  ├── modal.rs       - render_modals(), render_drop_overlay()             │
│  ├── panels.rs      - Sidebar and dock panels                            │
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
├── config/              Formatter and language-server config
├── config_paths.rs      Config directory management
├── settings.rs          Settings model; settings/ has catalog, forms, pages
├── overlay.rs           Overlay rendering utilities
├── cli.rs               CLI argument parsing
├── launcher.rs          Startup/session launch
├── session.rs           Session persistence
├── search.rs            Find/search engine
├── wrap.rs              Soft wrap
├── editorconfig.rs      .editorconfig file policy
├── recent_files.rs      Recent files list
├── command_history.rs   Command palette history
├── mcp.rs / automation.rs  MCP and automation hooks
├── perf.rs              PerfStats, performance monitoring
├── tracing.rs           Debug tracing, cursor snapshots
├── debug_dump.rs        State dump to JSON (F7, debug only)
├── debug_overlay.rs     Debug overlay rendering (F8, debug only)
├── fs_watcher.rs        File system watcher for workspace
│
├── runtime/
│   ├── mod.rs           Runtime module exports
│   ├── app.rs           App struct, ApplicationHandler, event handling
│   ├── input.rs         handle_key() - keyboard shortcuts → Msg
│   ├── mouse.rs         Mouse handling
│   ├── auto_save.rs, file_io.rs, file_watch.rs, configuration.rs
│   ├── find_worker.rs, formatting.rs, lsp_slot.rs, inline_*.rs
│   └── webview.rs       Native preview webview
│
├── view/
│   ├── mod.rs           Renderer, render phases, GlyphCache
│   ├── frame.rs         Frame, TextPainter abstractions
│   ├── geometry.rs      Layout calculations, GroupLayout, helpers
│   ├── editor_text.rs   Text area and gutter drawing
│   ├── document_tabs.rs Tab bar
│   ├── modal.rs         Modals, drop overlay, cursor overlay
│   ├── find_bar.rs      Docked find/replace bar
│   ├── panels.rs        Sidebar and dock panels
│   ├── overlay_surface.rs  Command palette / search-everywhere surface
│   ├── helpers.rs       Rendering helper functions
│   └── text_field.rs    Text field rendering for modals
│
├── model/
│   ├── mod.rs           AppModel, ViewportGeometry, ScaledMetrics
│   ├── document.rs      Document, Rope buffer, EditOperation, undo/redo
│   ├── editor.rs        EditorState, Cursor, Selection, Viewport, ViewMode
│   ├── editor_area.rs   EditorArea, EditorGroup, Tab, LayoutNode, splits
│   ├── status_bar.rs    StatusBar, segments, sync_status_bar()
│   ├── ui.rs            UiState, ModalState, HoverRegion
│   ├── workspace.rs     Workspace, FileTree, FileNode
│   └── hover.rs, usages.rs, save.rs, file_io.rs, ghost_text.rs, ...
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
│   ├── text_edits.rs    Shared edit transactions (typing, completion, LSP edits)
│   └── lsp.rs, completion.rs, terminal.rs, dock.rs, folding.rs, hover.rs,
│       navigation.rs, preview.rs, image.rs, outline.rs, problems.rs, ...
│
├── keymap/              Configurable keybindings system
│   ├── mod.rs           Module exports
│   ├── keymap.rs        Keymap, KeyAction
│   ├── command.rs       Command enum
│   ├── defaults.rs      Default keybinding definitions
│   ├── binding.rs / types.rs / config.rs / preferences.rs
│   ├── winit_adapter.rs winit key events → Keystroke
│   └── context.rs       KeyContext for conditional bindings
│
├── syntax/              Tree-sitter syntax highlighting
│   ├── mod.rs           Language detection, parser management
│   ├── registry.rs      LanguageId + grammar registry
│   ├── languages.rs     Extension → language mapping
│   ├── highlights.rs    Syntax highlight types and colors
│   ├── parser.rs        Parsing and highlight extraction
│   └── folding.rs / selection.rs  Syntax-driven folds and selection
│
├── lsp/                 Language server client, transport, sync
├── completion/          Menu + inline completion (providers, sessions)
├── terminal/            PTY sessions, integrated terminal
├── panel/               Dock layout; panels/ has panel views
├── layout/              Chrome/editor layout algorithm and snapshots
├── markdown/            Markdown preview rendering
├── image/               Image tab rendering
├── folding/             Fold state and persistence
├── outline/             Symbol outline extraction
├── context_menu/        Context menu builders
├── tooling/             Tool presets
├── bin/                 Helper binaries (screenshot, ui_gallery, fake_lsp_server)
│
├── csv/                 CSV viewer/editor
│   ├── mod.rs           CsvState, grid operations
│   ├── parser.rs        CSV parsing utilities
│   └── model.rs / navigation.rs / render.rs / viewport.rs
│
├── editable/            Shared primitives and small-field editing
│   ├── cursor.rs        Cursor and Position (also used by document editors)
│   ├── selection.rs     Shared Selection behavior
│   ├── buffer.rs        Small-field buffer traits and StringBuffer
│   ├── state.rs         EditableState operations
│   ├── history.rs       EditOperation, EditHistory
│   ├── constraints.rs   EditConstraints
│   └── messages.rs      MoveTarget
│
└── util/                Utilities
    ├── mod.rs           char_type, word boundary helpers
    ├── file_validation.rs  File validation, binary detection
    ├── file_identity.rs, byte_size.rs, tree.rs
    └── text.rs          Text manipulation helpers
```

## Rendering Pipeline

```
render()
  │
  ├─► compute_effective_damage()        # Determine what needs redrawing
  │
  ├─► compute_layout_scaled()           # Calculate group rects, splitter positions
  │         └─► stored in group.rect    # (src/model/editor_area.rs)
  │
  ├─► buffer.clear() or partial clear   # Clear based on damage
  │
  ├─► render_sidebar_phase()            # File tree (if workspace open)
  │       ├─► Background, border
  │       └─► Tree nodes (folders, files)
  │
  ├─► render_editor_area_phase()        # All editor groups
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
  │           └─► Dim non-focused groups (render_unfocused_dim)
  │
  ├─► render_splitters()                # Draggable split bars
  │
  ├─► render_right_dock_phase() / render_bottom_dock_phase()  # Dock panels
  │
  ├─► render_status_bar_phase()         # At bottom of window
  │       ├─► Background
  │       ├─► Left segments (mode, file, line/col)
  │       └─► Right segments (encoding, language)
  │
  ├─► render_modal_phase()              # Modal overlays (if active)
  │       ├─► Dim background
  │       ├─► Modal dialog box
  │       ├─► Command palette / Goto line / Settings
  │       └─► Fuzzy file finder
  │
  ├─► render_cursor_overlay_phase()     # Hover cards, completion menus
  │
  └─► render_drop_overlay_phase()       # File drag-and-drop indicator
```

## Key Locations

### Finding Specific Rendering

| What                         | File                          | Function                       |
| ---------------------------- | ----------------------------- | ------------------------------ |
| **Main render loop**         | `src/view/mod.rs`             | `render()`                     |
| **Damage computation**       | `src/view/mod.rs`             | `compute_effective_damage()`   |
| **Editor group rendering**   | `src/view/mod.rs`             | `render_editor_group()`        |
| **Tab bar**                  | `src/view/document_tabs.rs`   | `render()`                     |
| **Text area (main editor)**  | `src/view/editor_text.rs`     | `render_text_area()`           |
| **Line numbers/gutter**      | `src/view/editor_text.rs`     | `render_gutter()`              |
| **CSV grid**                 | `src/view/mod.rs`             | `render_csv_grid()`            |
| **CSV cell editor**          | `src/view/mod.rs`             | `render_csv_cell_editor()`     |
| **Splitter bars**            | `src/view/mod.rs`             | `render_splitters()`           |
| **Status bar**               | `src/view/mod.rs`             | `render_status_bar()`          |
| **Modals**                   | `src/view/modal.rs`           | `render_modals()`              |
| **Find bar**                 | `src/view/find_bar.rs`        |                                |
| **Sidebar (file tree)**      | `src/view/panels.rs`          | `render_sidebar()`             |
| **Dock panels**              | `src/view/panels.rs`          | `render_dock()`                |
| **Drop overlay**             | `src/view/modal.rs`           | `render_drop_overlay()`        |
| **Geometry calculations**    | `src/view/geometry.rs`        | `GroupLayout`, helpers         |
| **Group layout tree**        | `src/model/editor_area.rs`    | `compute_layout_scaled()`      |
| **Text field (modal input)** | `src/view/text_field.rs`      | `TextFieldRenderer`            |

### Finding Specific Logic

| What                       | File                       | Function                                     |
| -------------------------- | -------------------------- | -------------------------------------------- |
| **Event handling**         | `src/runtime/app.rs`       | `handle_event()`                             |
| **Keyboard input routing** | `src/runtime/input.rs`     | `handle_key()`                               |
| **Modal key handling**     | `src/runtime/input.rs`     | `handle_modal_key()`                         |
| **CSV key handling**       | `src/runtime/input.rs`     | `handle_csv_edit_key()`                      |
| **Sidebar key handling**   | `src/runtime/input.rs`     | `handle_sidebar_key()`                       |
| **Keymap system**          | `src/keymap/keymap.rs`     | `Keymap::handle_keystroke()`                 |
| **Default keybindings**    | `src/keymap/defaults.rs`   | `load_default_keymap()`                      |
| **Update dispatcher**      | `src/update/mod.rs`        | `update()`                                   |
| **Cursor movement**        | `src/update/editor.rs`     | `update_editor()`                            |
| **Text insertion**         | `src/update/document.rs`   | `update_document()`                          |
| **Shared edit transactions** | `src/update/text_edits.rs` | `plan_text_edits()`, `apply_planned_edits()` |
| **Move selected lines**    | `src/update/document.rs`   | `move_lines()`, `moved_line()`               |
| **Split/tab operations**   | `src/update/layout.rs`     | `update_layout()`                            |
| **Undo/redo**              | `src/update/document.rs`   | `update_document()` (Undo/Redo arms)         |
| **Multi-cursor logic**     | `src/model/editor.rs`      | `add_cursor_at()`, `merge_overlapping_selections()` |
| **Viewport scrolling**     | `src/model/mod.rs`         | `ensure_cursor_visible()` and variants       |
| **Status bar sync**        | `src/model/status_bar.rs`  | `sync_status_bar()`                          |
| **Syntax highlighting**    | `src/update/syntax.rs`     | `update_syntax()`, `schedule_syntax_parse()` |
| **CSV operations**         | `src/update/csv.rs`        | `update_csv()`                               |
| **Workspace operations**   | `src/update/workspace.rs`  | `update_workspace()`                         |
| **File tree navigation**   | `src/model/workspace.rs`   | `FileTree` methods                           |

## Data Flow Example: Typing a Character

```
1. WindowEvent::KeyboardInput { key: 'a', ... }
   └─► src/runtime/app.rs: handle_event()

2. Keymap::handle_keystroke(...) or handle_key(..., Key::Character("a"), ...)
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
1. WindowEvent::KeyboardInput { key: 'A', shift: true, logo: true }
   └─► src/runtime/app.rs: handle_event()

2. Keymap matches Cmd+Shift+A → Command::ToggleCommandPalette
   └─► Converts to Msg::Ui(UiMsg::ToggleModal(ModalId::CommandPalette))

3. update(model, Msg::Ui(UiMsg::ToggleModal(ModalId::CommandPalette)))
   └─► src/update/ui.rs: sets model.ui.active_modal = Some(ModalState::CommandPalette(..))

4. Subsequent keys route to handle_modal_key()
   └─► src/runtime/input.rs: handles modal input separately

5. render() includes render_modals()
   └─► Draws dimmed background + modal dialog + command list
```

## Layout Tree Structure

```
EditorArea
├── layout: LayoutNode (root of tree)
│   ├── LayoutNode::Empty                    # No groups yet
│   ├── LayoutNode::Group(GroupId)           # Leaf: single editor group
│   ├── LayoutNode::Preview(PreviewId)       # Leaf: markdown/HTML preview pane
│   └── LayoutNode::Split(SplitContainer)    # Branch: contains children
│         ├── direction: Horizontal | Vertical
│         ├── children: Vec<LayoutNode>
│         └── ratios: Vec<f32>               # How to divide space
│
├── groups: HashMap<GroupId, EditorGroup>
│   └── EditorGroup
│       ├── tabs: Vec<Tab>                   # Each tab → EditorId
│       ├── active_tab_index: usize
│       ├── attached_preview: Option<PreviewId>
│       └── rect: Rect                       # Computed by compute_layout_scaled()
│
├── editors: HashMap<EditorId, EditorState>
│   └── EditorState
│       ├── document_id: Option<DocumentId>
│       ├── view_mode: ViewMode              # Text or Csv(CsvState)
│       ├── cursors: Vec<Cursor>             # Multi-cursor support
│       ├── selections: Vec<Selection>
│       ├── viewport: Viewport
│       ├── folds: FoldState, soft_wrap: bool, wrap_cache: WrapCache
│       └── ghost_text: GhostText            # Inline completion preview
│
└── documents: HashMap<DocumentId, Document>
    └── Document
        ├── buffer: Rope                     # Text content (ropey crate)
        ├── file_path: Option<PathBuf>
        ├── language: LanguageId             # For syntax highlighting (PlainText default)
        ├── syntax_highlights: Option<SyntaxHighlights>
        ├── syntax_tree, outline, diagnostics, lsp_features
        ├── undo_stack: Vec<EditOperation>
        └── redo_stack: Vec<EditOperation>
```

## Workspace Structure

```
Workspace
├── root: PathBuf                            # Workspace root directory
├── file_tree: FileTree                      # Tree of files/folders
├── expanded_folders: HashSet<PathBuf>       # Folder expansion state
├── selected_item: Option<PathBuf>           # Currently selected file/folder
├── sidebar_visible: bool                    # Sidebar toggle state
├── sidebar_width_logical: f32               # Sidebar width
└── scroll_offset: usize                     # Vertical scroll position

FileTree
└── roots: Vec<FileNode>                     # Root directory nodes

FileNode
├── name: String                             # File/folder name
├── path: PathBuf                            # Full path
├── is_dir: bool                             # Directory vs file
├── extension: FileExtension                 # For icons/language
└── children: Vec<FileNode>                  # Child nodes (dirs only)
```

## Modal System

```
ModalState (enum)
├── CommandPalette(CommandPaletteState)      # Cmd+Shift+A (search everywhere)
│   ├── editable: EditableState<StringBuffer>  # Query input
│   ├── matches: Vec<CommandMatch>           # Filtered, ranked commands
│   ├── selected_index: usize                # List selection
│   ├── active_tab: SearchTab                # Commands / Files / Symbols / All
│   └── files: Option<FileFinderState>, symbols: WorkspaceSymbolsState
│
├── GotoLine(GotoLineState)                  # Cmd+L
├── FileFinder(FileFinderState)              # Fuzzy file finder
├── RecentFiles(RecentFilesState)
├── ThemePicker(ThemePickerState)
├── LanguagePicker(LanguagePickerState)
├── LspServers(LspServersState)
├── RenameSymbol(RenameSymbolState)
├── Settings(SettingsState)
├── UnsavedChanges(UnsavedChangesState)
└── FileConflict(FileConflictState)
```

Find/Replace is not a modal: it lives in `UiState.find_bar:
Option<FindReplaceState>` as a docked bar (`src/view/find_bar.rs`).

## Debug Tools

| Key | Action                     | File                   |
| --- | -------------------------- | ---------------------- |
| F2  | Toggle performance overlay | `src/perf.rs`          |
| F7  | Dump state to JSON         | `src/debug_dump.rs`    |
| F8  | Toggle debug overlay       | `src/debug_overlay.rs` |

All three are handled in `src/runtime/app.rs` under `#[cfg(debug_assertions)]`.

## Theme Colors (where used)

| Color                                | Usage             | View location         |
| ------------------------------------ | ----------------- | --------------------- |
| `theme.editor.background`            | Main background   | `render()` clear      |
| `theme.editor.foreground`            | Text color        | `render_text_area()`  |
| `theme.editor.line_number`           | Gutter numbers    | `render_gutter()`     |
| `theme.editor.current_line`          | Line highlight    | `render_text_area()`  |
| `theme.editor.selection`             | Selection bg      | `render_text_area()`  |
| `theme.editor.cursor_color`          | Cursor            | `render_text_area()`  |
| `theme.editor.gutter_border`         | Gutter separator  | `render_gutter()`     |
| `theme.status_bar.background`        | Status bar bg     | `render_status_bar()` |
| `theme.status_bar.foreground`        | Status bar text   | `render_status_bar()` |
| `theme.tab_bar.background`           | Tab bar bg        | `document_tabs.rs`    |
| `theme.tab_bar.active_background`    | Active tab bg     | `document_tabs.rs`    |
| `theme.tab_bar.active_foreground`    | Active tab text   | `document_tabs.rs`    |
| `theme.overlay.background`           | Modal dialog bg   | `render_modals()`     |
| `theme.overlay.foreground`           | Modal text        | `render_modals()`     |
| `theme.overlay.highlight`            | Modal highlights  | `render_modals()`     |
| `theme.sidebar.background`           | Sidebar bg        | `render_sidebar()`    |
| `theme.sidebar.foreground`           | File tree text    | `render_sidebar()`    |
| `theme.sidebar.selection_background` | Selected file bg  | `render_sidebar()`    |
| `theme.splitter.background`          | Split bars        | `render_splitters()`  |
| `theme.syntax.*`                     | Code highlighting | `render_text_area()`  |

## Key Subsystems

### Keymap System (`src/keymap/`)

- **Purpose**: Configurable keybindings without hardcoding
- **Key Types**:
  - `Keystroke`: Key with modifiers (`types.rs`)
  - `Command`: High-level editor command (`command.rs`, ~150 variants)
  - `KeyContext`: Conditions for when binding is active (`context.rs`)
  - `Keymap`: Maps Keystroke → Command, returns `KeyAction` (`keymap.rs`)
- **Flow**: Keystroke → Keymap::handle_keystroke() → Command → Msg conversion → update()
- **User overrides**: `~/.config/token-editor/keymap.yaml` (loaded by `defaults.rs`)

### Syntax Highlighting (`src/syntax/`)

- **Purpose**: Tree-sitter based syntax highlighting
- **Architecture**: Worker thread (`syntax_worker_loop` in `src/runtime/app.rs`) parses in background, sends results via channel
- **Supports**: 70+ tree-sitter grammars registered in `registry.rs` (Rust, JavaScript, Python, etc.)
- **Integration**: Document stores `syntax_highlights`, renderer uses for colors; also drives folding, outline and syntax-aware selection

### Language Tooling (`src/lsp/`, `src/completion/`, `src/runtime/formatting.rs`)

- **LSP**: Client/transport per server, document sync, hover, definition, references, rename, code actions, diagnostics (Problems panel)
- **Completion**: Menu completion (LSP, path, recency sources) and inline/ghost-text completion with its own worker
- **Formatting**: LSP formatting or external formatter presets (`src/tooling/`)

### Dock Panels and Terminal (`src/panel/`, `src/panels/`, `src/terminal/`)

- **Purpose**: Right/bottom docks hosting Outline, Problems, Usages and Terminal panels
- **Terminal**: PTY-backed sessions with key translation and link detection

### CSV Viewer (`src/csv/`)

- **Purpose**: Spreadsheet-like CSV viewing/editing
- **ViewMode**: Editor switches between Text and Csv modes
- **Features**: Grid navigation, cell editing, column resizing
- **Rendering**: Separate `render_csv_grid()` pipeline

### Editable System (`src/editable/`)

- **Purpose**: Shared cursor/position/selection primitives, plus editing state for modal inputs, find bar and CSV cells
- **Routing**: Existing document, modal and CSV messages; document-wide edits go through `src/update/text_edits.rs`
- **Storage**: Small fields use `EditableState<StringBuffer>`; documents retain their rope and document-level history

### Workspace (`src/model/workspace.rs`, `src/update/workspace.rs`)

- **Purpose**: Project-level file management
- **Features**: File tree sidebar, fuzzy file finder, file watcher (`src/fs_watcher.rs`)
- **Integration**: Optional sidebar on left, watch for file changes

### Preview, Folding and Settings

- **Markdown/HTML preview** (`src/markdown/`, `src/runtime/webview.rs`): preview panes attached to a group
- **Folding** (`src/folding/`): fold state per editor, persisted across sessions
- **Settings** (`src/settings/`, `src/settings.rs`): Settings modal with catalog, forms and keymap editing

## Configuration

- **Config file**: `~/.config/token-editor/config.yaml`
- **Keymap**: `~/.config/token-editor/keymap.yaml`
- **Themes**: `~/.config/token-editor/themes/*.yaml`
- **Structure**: `EditorConfig` in `src/config.rs`; paths in `src/config_paths.rs`
- **Reload**: `Reload Configuration` command from the command palette (`AppMsg::ReloadConfiguration` → `Cmd::ReloadConfiguration`)
