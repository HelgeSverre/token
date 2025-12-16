# File Operations & Workspace Management

Comprehensive file handling including dialogs, drag-and-drop, CLI arguments, and workspace support.

---

## Overview

### Current State (as of 2025-12-15)

**Implemented:**
- `Document::from_file(path)` - Synchronous file loading
- `AppMsg::SaveFile` / `AppMsg::LoadFile` - File operations via messages
- `Cmd::SaveFile` / `Cmd::LoadFile` - Async commands for file I/O
- `LayoutMsg::OpenFileInNewTab(path)` - Opens file as new tab
- `WindowEvent::DroppedFile` handling - Basic drag-and-drop (opens in new tab)
- `EditorConfig` persistence to `~/.config/token-editor/config.yaml`
- Theme loading with user → builtin fallback
- Multiple files via CLI args open as tabs
- ✅ **Phase 1:** `AppModel::new()` refactored with `ViewportGeometry`, `load_config_and_theme()`, `create_initial_session()` helpers
- ✅ **Phase 2:** Native file dialogs via `rfd` crate (⌘O Open File, ⇧⌘O Open Folder, ⇧⌘S Save As)
- ✅ **Phase 3:** Visual feedback during file drag-hover (`DropState`, overlay rendering)
- ✅ **Phase 4:** File validation (`FileOpenError`, binary detection, 50MB size limit) in `src/util/file_validation.rs`
- ✅ **Phase 5:** CLI arguments with `clap` (`--new`, `--wait`, `--line`, `--column`) in `src/cli.rs`
- ✅ **Duplicate file detection:** Already-open files focus existing tab instead of opening again

**Not Implemented:**
- Workspace concept with file tree sidebar (Phase 6)

### Architecture

The editor follows Elm Architecture: `Message → Update → Command → Render`

```
User Action → WindowEvent/Keypress
    ↓
Message (Msg::App, Msg::Layout, etc.)
    ↓
update() → mutates AppModel, returns Option<Cmd>
    ↓
Command execution (async I/O, dialogs)
    ↓
Result message → back to update()
```

---

## Phase 1: Refactor AppModel::new() ✅

**Status:** Completed 2025-12-15

**Goal:** Separate concerns - config/theme loading, file loading, UI initialization.

### New Helper Types

```rust
// src/model/mod.rs or src/model/geometry.rs

/// Viewport geometry calculations (pure, no I/O)
#[derive(Debug, Clone, Copy)]
pub struct ViewportGeometry {
    pub window_width: u32,
    pub window_height: u32,
    pub line_height: usize,
    pub char_width: f32,
    pub visible_lines: usize,
    pub visible_columns: usize,
}

impl ViewportGeometry {
    pub fn new(window_width: u32, window_height: u32) -> Self {
        let line_height = 20;
        let char_width: f32 = 10.0;
        let text_x = text_start_x(char_width).round();
        let visible_columns = ((window_width as f32 - text_x) / char_width).floor() as usize;
        let status_bar_height = line_height;
        let visible_lines = (window_height as usize)
            .saturating_sub(status_bar_height) / line_height;

        Self { window_width, window_height, line_height, char_width, visible_lines, visible_columns }
    }
}
```

### Helper Functions

```rust
// src/model/mod.rs

/// Load configuration and theme
fn load_config_and_theme() -> (EditorConfig, Theme) {
    EditorConfig::ensure_config_dirs();
    let config = EditorConfig::load();
    let theme = load_theme(&config.theme).unwrap_or_else(|e| {
        tracing::warn!("Failed to load theme '{}': {}, using default", config.theme, e);
        Theme::default()
    });
    (config, theme)
}

/// Initial session with documents and editor area
pub struct InitialSession {
    pub editor_area: EditorArea,
    pub status_message: String,
    pub workspace_root: Option<PathBuf>,
}

fn create_initial_session(file_paths: Vec<PathBuf>, geom: &ViewportGeometry) -> InitialSession {
    // Move existing file loading logic here
    // ...
}
```

### Refactored AppModel::new()

```rust
impl AppModel {
    pub fn new(window_width: u32, window_height: u32, file_paths: Vec<PathBuf>) -> Self {
        let geom = ViewportGeometry::new(window_width, window_height);
        let (config, theme) = load_config_and_theme();
        let InitialSession { editor_area, status_message, workspace_root } = 
            create_initial_session(file_paths, &geom);

        Self {
            editor_area,
            ui: UiState::with_status(status_message),
            theme,
            config,
            window_size: (window_width, window_height),
            line_height: geom.line_height,
            char_width: geom.char_width,
            workspace_root, // New field: Option<PathBuf>
            #[cfg(debug_assertions)]
            debug_overlay: Some(DebugOverlay::new()),
        }
    }
}
```

### Tasks

- [x] Add `ViewportGeometry` struct
- [x] Extract `load_config_and_theme()` helper
- [x] Extract `create_initial_session()` helper  
- [x] Add `workspace_root: Option<PathBuf>` to `AppModel`
- [x] Refactor `AppModel::new()` to use helpers

---

## Phase 2: File Dialogs with rfd ✅

**Status:** Completed 2025-12-15

**Goal:** Native Save As, Open File, and Open Folder dialogs.

### Dependencies

```toml
# Cargo.toml
rfd = "0.15"
```

### New Command Variants

```rust
// src/commands.rs

pub enum Cmd {
    // ... existing
    
    /// Show native open file dialog
    ShowOpenFileDialog {
        allow_multi: bool,
        start_dir: Option<PathBuf>,
    },
    /// Show native save file dialog
    ShowSaveFileDialog {
        suggested_path: Option<PathBuf>,
    },
    /// Show native open folder dialog
    ShowOpenFolderDialog {
        start_dir: Option<PathBuf>,
    },
}
```

### New App Messages

```rust
// src/messages.rs

pub enum AppMsg {
    // ... existing
    
    /// User requested "Save As..."
    SaveFileAs,
    /// Dialog returned a path (or None if cancelled)
    SaveFileAsDialogResult { path: Option<PathBuf> },
    
    /// User requested "Open File..."
    OpenFileDialog,
    /// Dialog returned paths (empty if cancelled)
    OpenFileDialogResult { paths: Vec<PathBuf> },
    
    /// User requested "Open Folder..."
    OpenFolderDialog,
    /// Dialog returned folder (or None if cancelled)
    OpenFolderDialogResult { folder: Option<PathBuf> },
}
```

### New Command IDs

```rust
// src/commands.rs

pub enum CommandId {
    // ... existing
    SaveFileAs,
    OpenFile,
    OpenFolder,
}

// Add to COMMANDS array with keybindings:
// SaveFileAs → "⇧⌘S"
// OpenFile → "⌘O"  
// OpenFolder → "⇧⌘O"
```

### Update Handler

```rust
// src/update/app.rs

AppMsg::SaveFileAs => {
    let suggested = model.document().file_path.clone();
    Some(Cmd::ShowSaveFileDialog { suggested_path: suggested })
}

AppMsg::SaveFileAsDialogResult { path } => {
    if let Some(path) = path {
        model.document_mut().file_path = Some(path.clone());
        let content = model.document().buffer.to_string();
        model.ui.is_saving = true;
        model.ui.set_status("Saving...");
        Some(Cmd::SaveFile { path, content })
    } else {
        model.ui.set_status("Save cancelled");
        Some(Cmd::Redraw)
    }
}

AppMsg::OpenFileDialog => {
    let start_dir = model.document().file_path
        .as_ref()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()));
    Some(Cmd::ShowOpenFileDialog { allow_multi: true, start_dir })
}

AppMsg::OpenFileDialogResult { paths } => {
    if paths.is_empty() {
        model.ui.set_status("Open cancelled");
        return Some(Cmd::Redraw);
    }
    // Open each file via existing LayoutMsg::OpenFileInNewTab
    let cmds: Vec<Cmd> = paths.into_iter()
        .map(|path| update_layout(model, LayoutMsg::OpenFileInNewTab(path)).into())
        .collect();
    Some(Cmd::batch(cmds))
}

AppMsg::OpenFolderDialog => {
    Some(Cmd::ShowOpenFolderDialog { start_dir: model.workspace_root.clone() })
}

AppMsg::OpenFolderDialogResult { folder } => {
    if let Some(root) = folder {
        model.workspace_root = Some(root.clone());
        model.ui.set_status(format!("Workspace: {}", root.display()));
    } else {
        model.ui.set_status("Open folder cancelled");
    }
    Some(Cmd::Redraw)
}
```

### Command Execution (winit event loop)

```rust
// src/runtime/app.rs or wherever Cmd is processed

fn process_cmd(&self, cmd: Cmd, window: &Window, proxy: &EventLoopProxy<Msg>) {
    match cmd {
        // ... existing
        
        Cmd::ShowOpenFileDialog { allow_multi, start_dir } => {
            let mut dlg = rfd::FileDialog::new();
            if let Some(dir) = start_dir {
                dlg = dlg.set_directory(dir);
            }
            // dlg = dlg.set_parent(window); // if using raw-window-handle
            
            let paths = if allow_multi {
                dlg.pick_files().unwrap_or_default()
            } else {
                dlg.pick_file().into_iter().collect()
            };
            
            let _ = proxy.send_event(Msg::App(AppMsg::OpenFileDialogResult { paths }));
        }
        
        Cmd::ShowSaveFileDialog { suggested_path } => {
            let mut dlg = rfd::FileDialog::new();
            if let Some(path) = suggested_path {
                if let Some(dir) = path.parent() {
                    dlg = dlg.set_directory(dir);
                }
                if let Some(name) = path.file_name() {
                    dlg = dlg.set_file_name(name.to_string_lossy());
                }
            }
            let path = dlg.save_file();
            let _ = proxy.send_event(Msg::App(AppMsg::SaveFileAsDialogResult { path }));
        }
        
        Cmd::ShowOpenFolderDialog { start_dir } => {
            let mut dlg = rfd::FileDialog::new();
            if let Some(dir) = start_dir {
                dlg = dlg.set_directory(dir);
            }
            let folder = dlg.pick_folder();
            let _ = proxy.send_event(Msg::App(AppMsg::OpenFolderDialogResult { folder }));
        }
    }
}
```

### Tasks

- [x] Add `rfd` to Cargo.toml
- [x] Add dialog `Cmd` variants (`ShowOpenFileDialog`, `ShowSaveFileDialog`, `ShowOpenFolderDialog`)
- [x] Add dialog `AppMsg` variants (`SaveFileAs`, `OpenFileDialog`, `OpenFolderDialog` + results)
- [x] Add `CommandId::SaveFileAs`, `OpenFile`, `OpenFolder`
- [x] Implement `update_app` handlers for dialog messages
- [x] Implement `process_cmd` for dialog commands (runs in background thread)
- [x] Add keybindings (⇧⌘S, ⌘O, ⇧⌘O) in `keymap.yaml` and defaults
- [x] Wire up command palette entries

---

## Phase 3: Visual Feedback for File Dropping ✅

**Status:** Completed 2025-12-15

**Goal:** Show overlay when files are dragged over the window.

### Data Structures

```rust
// src/model/ui.rs

/// State for file drop operations
#[derive(Debug, Clone, Default)]
pub struct DropState {
    /// Files currently being hovered over the window
    pub hovered_files: Vec<PathBuf>,
    /// Whether a valid drop target is being hovered
    pub is_hovering: bool,
}

// Add to UiState:
pub drop_state: DropState,
```

### Messages

```rust
// src/messages.rs

/// File drop messages
#[derive(Debug, Clone)]
pub enum DropMsg {
    FileHovered(PathBuf),
    HoverCancelled,
    FilesDropped(Vec<PathBuf>),
}

// Add to Msg enum:
Drop(DropMsg),
```

### Event Handling

```rust
// src/runtime/app.rs

WindowEvent::HoveredFile(path) => {
    update(&mut self.model, Msg::Drop(DropMsg::FileHovered(path.clone())))
}

WindowEvent::HoveredFileCancelled => {
    update(&mut self.model, Msg::Drop(DropMsg::HoverCancelled))
}

WindowEvent::DroppedFile(path) => {
    // Clear hover state and open file
    update(&mut self.model, Msg::Drop(DropMsg::FilesDropped(vec![path.clone()])))
}
```

### Update Handler

```rust
// src/update/drop.rs (new file)

pub fn update_drop(model: &mut AppModel, msg: DropMsg) -> Option<Cmd> {
    match msg {
        DropMsg::FileHovered(path) => {
            model.ui.drop_state.hovered_files = vec![path];
            model.ui.drop_state.is_hovering = true;
            Some(Cmd::Redraw)
        }
        
        DropMsg::HoverCancelled => {
            model.ui.drop_state.is_hovering = false;
            model.ui.drop_state.hovered_files.clear();
            Some(Cmd::Redraw)
        }
        
        DropMsg::FilesDropped(paths) => {
            model.ui.drop_state.is_hovering = false;
            model.ui.drop_state.hovered_files.clear();
            
            // Open each file in a new tab
            let cmds: Vec<Cmd> = paths.into_iter()
                .map(|path| {
                    update_layout(model, LayoutMsg::OpenFileInNewTab(path))
                        .unwrap_or(Cmd::None)
                })
                .collect();
            
            Some(Cmd::batch(cmds))
        }
    }
}
```

### Theme Extension

```rust
// src/theme.rs

pub struct DropZoneTheme {
    pub overlay: Color,  // Semi-transparent background
    pub border: Color,   // Border highlight
    pub text: Color,     // "Drop to open" text
}

impl DropZoneTheme {
    pub fn default_dark() -> Self {
        Self {
            overlay: Color::rgba(0x00, 0x00, 0x00, 0xB4),
            border: Color::rgb(0x00, 0x7A, 0xCC),
            text: Color::rgb(0xFF, 0xFF, 0xFF),
        }
    }
}
```

### Rendering

```rust
// In renderer

fn render_drop_overlay(&mut self, model: &AppModel) {
    if !model.ui.drop_state.is_hovering {
        return;
    }
    
    // Draw semi-transparent overlay
    // Draw border
    // Draw centered text: "Drop to open: filename" or "Drop to open N files"
}
```

### Tasks

- [x] Add `DropState` to `UiState`
- [x] Add drop-related messages to `UiMsg` (`FileHovered`, `FileHoverCancelled`)
- [x] Handle `WindowEvent::HoveredFile` and `HoveredFileCancelled`
- [x] Implement drop overlay rendering in `src/view/mod.rs`
- [x] Use existing theme overlay colors for drop feedback

---

## Phase 4: File Validation ✅

**Status:** Completed 2025-12-15

**Goal:** Validate files before opening (binary detection, size limits, permissions).

**Implementation:** Created `src/util/file_validation.rs` with validation utilities.

### Error Types

```rust
// src/model/document.rs or new file

#[derive(Debug, Clone)]
pub enum FileOpenError {
    NotFound,
    PermissionDenied,
    IsDirectory,
    BinaryFile,
    TooLarge { size_mb: f64 },
    IoError(String),
}

impl FileOpenError {
    pub fn user_message(&self, filename: &str) -> String {
        match self {
            Self::NotFound => format!("File not found: {}", filename),
            Self::PermissionDenied => format!("Permission denied: {}", filename),
            Self::IsDirectory => format!("Cannot open directory: {}", filename),
            Self::BinaryFile => format!("Binary file: {}", filename),
            Self::TooLarge { size_mb } => format!("{} is too large ({:.1} MB)", filename, size_mb),
            Self::IoError(msg) => format!("Error opening {}: {}", filename, msg),
        }
    }
}
```

### Validation Functions

```rust
const MAX_FILE_SIZE: u64 = 50 * 1024 * 1024; // 50 MB

pub fn validate_file_for_opening(path: &Path) -> Result<(), FileOpenError> {
    let metadata = std::fs::metadata(path).map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => FileOpenError::NotFound,
        std::io::ErrorKind::PermissionDenied => FileOpenError::PermissionDenied,
        _ => FileOpenError::IoError(e.to_string()),
    })?;

    if metadata.is_dir() {
        return Err(FileOpenError::IsDirectory);
    }

    if metadata.len() > MAX_FILE_SIZE {
        return Err(FileOpenError::TooLarge {
            size_mb: metadata.len() as f64 / (1024.0 * 1024.0)
        });
    }

    Ok(())
}

pub fn is_binary_file(path: &Path) -> bool {
    if let Ok(mut file) = std::fs::File::open(path) {
        let mut buffer = [0u8; 8192];
        if let Ok(n) = std::io::Read::read(&mut file, &mut buffer) {
            return buffer[..n].contains(&0);
        }
    }
    false
}
```

### Integration

Update `open_file_in_new_tab()` in `src/update/layout.rs`:

```rust
fn open_file_in_new_tab(model: &mut AppModel, path: PathBuf) {
    // Validate first
    if let Err(e) = validate_file_for_opening(&path) {
        let filename = path.file_name().unwrap_or_default().to_string_lossy();
        model.ui.set_status(e.user_message(&filename));
        return;
    }
    
    if is_binary_file(&path) {
        let filename = path.file_name().unwrap_or_default().to_string_lossy();
        model.ui.set_status(format!("Binary file: {}", filename));
        return;
    }
    
    // ... existing loading logic
}
```

### Tasks

- [x] Add `FileOpenError` enum with `NotFound`, `PermissionDenied`, `IsDirectory`, `BinaryFile`, `TooLarge`, `IoError`
- [x] Implement `validate_file_for_opening()` - checks existence, permissions, size (50MB limit)
- [x] Implement `is_likely_binary()` - scans first 8KB for null bytes
- [x] Integrate validation into `open_file_in_new_tab()` in `src/update/layout.rs`
- [x] Integrate validation into `create_initial_session()` in `src/model/mod.rs`
- [x] Add `filename_for_display()` helper for error messages
- [x] User-friendly error messages shown in status bar
- [x] 6 unit tests for validation functions

---

## Phase 5: CLI Arguments with clap ✅

**Status:** Completed 2025-12-15

**Goal:** Parse command-line arguments with flags for startup behavior.

### Dependencies

```toml
# Cargo.toml
clap = { version = "4", features = ["derive"] }
```

### CLI Specification

```
USAGE:
    token [OPTIONS] [PATHS]...

ARGS:
    [PATHS]...    Files or directories to open

OPTIONS:
    -n, --new              Start with empty buffer (ignore session)
    -w, --wait             Wait for all files to close (for git/svn)
    --line <N>             Go to line N in first file
    --column <N>           Go to column N (with --line)
    -h, --help             Print help information
    -V, --version          Print version information
```

### Data Structures

```rust
// src/cli.rs (new file)

use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "token", version, about = "A fast text editor")]
pub struct CliArgs {
    /// Files or directories to open
    #[arg(value_name = "PATHS")]
    pub paths: Vec<PathBuf>,

    /// Start with empty buffer
    #[arg(short = 'n', long)]
    pub new: bool,

    /// Wait for files to close (git integration)
    #[arg(short = 'w', long)]
    pub wait: bool,

    /// Go to line N in first file
    #[arg(long)]
    pub line: Option<usize>,

    /// Go to column N (with --line)
    #[arg(long)]
    pub column: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct StartupConfig {
    pub mode: StartupMode,
    pub initial_position: Option<(usize, usize)>,
    pub wait_mode: bool,
}

#[derive(Debug, Clone)]
pub enum StartupMode {
    Empty,
    SingleFile(PathBuf),
    MultipleFiles(Vec<PathBuf>),
    Workspace { root: PathBuf, initial_files: Vec<PathBuf> },
}

impl CliArgs {
    pub fn into_config(self) -> Result<StartupConfig, String> {
        let mode = if self.new || self.paths.is_empty() {
            StartupMode::Empty
        } else if self.paths.len() == 1 {
            let path = &self.paths[0];
            if path.is_dir() {
                StartupMode::Workspace { root: path.clone(), initial_files: vec![] }
            } else {
                StartupMode::SingleFile(path.clone())
            }
        } else {
            let (dirs, files): (Vec<_>, Vec<_>) = self.paths.iter()
                .partition(|p| p.is_dir());
            
            if dirs.len() > 1 {
                return Err("Cannot open multiple directories".to_string());
            }
            
            if let Some(dir) = dirs.first() {
                StartupMode::Workspace {
                    root: (*dir).clone(),
                    initial_files: files.into_iter().cloned().collect(),
                }
            } else {
                StartupMode::MultipleFiles(files.into_iter().cloned().collect())
            }
        };

        Ok(StartupConfig {
            mode,
            initial_position: self.line.map(|l| (l, self.column.unwrap_or(0))),
            wait_mode: self.wait,
        })
    }
}
```

### Integration

```rust
// src/main.rs

fn main() {
    let args = CliArgs::parse();
    let startup = args.into_config().expect("Invalid arguments");
    
    // Use startup.mode to determine what to open
    // Use startup.initial_position to set cursor after loading
    // Use startup.wait_mode for git integration behavior
}
```

### Tasks

- [x] Add `clap` to Cargo.toml
- [x] Create `src/cli.rs` module
- [x] Implement `CliArgs` with derive macros
- [x] Implement `StartupConfig` and `StartupMode`
- [x] Update `main()` to use CLI parsing
- [x] Handle `--line` and `--column` flags (1-indexed to 0-indexed conversion)
- [ ] Handle `--wait` mode for git integration (flag parsed but not yet implemented)
- [x] 7 unit tests for CLI parsing

---

## Duplicate File Detection ✅

**Status:** Completed 2025-12-15

**Implementation:** Added `find_open_file()` and `is_file_open()` methods to `EditorArea` in `src/model/editor_area.rs`.

When opening a file that's already open:
- The existing tab is focused instead of creating a new one
- Status bar shows "Switched to: filename"
- Works with canonicalized paths to handle symlinks/relative paths

---

## Phase 6: Workspace & FileTree Sidebar

**Goal:** Full workspace support with file tree sidebar.

See [workspace-management.md](workspace-management.md) for detailed specification.

### Summary

- `Workspace` struct with `FileTree`, expanded folders, sidebar state
- `WorkspaceMsg` for sidebar interactions
- `update_workspace()` handler
- Sidebar rendering with file icons
- File system watching with `notify` crate
- Mouse and keyboard navigation

### Dependencies

```toml
# Cargo.toml
notify = "6.1"
```

### Tasks

- [ ] Create `src/model/workspace.rs`
- [ ] Implement `Workspace`, `FileTree`, `FileNode`
- [ ] Add `WorkspaceMsg` to messages
- [ ] Implement `update_workspace()`
- [ ] Add sidebar rendering
- [ ] Implement mouse hit-testing for sidebar
- [ ] Add keyboard navigation (arrows, enter)
- [ ] Integrate `notify` for file watching
- [ ] Add Cmd+B to toggle sidebar

---

## Success Criteria

### Phase 1 (Refactor) ✅
- [x] `AppModel::new()` is clean with separated concerns
- [x] `ViewportGeometry` calculates dimensions
- [x] `workspace_root` field exists for future use

### Phase 2 (Dialogs) ✅
- [x] ⌘O opens native Open File dialog
- [x] ⇧⌘S opens native Save As dialog
- [x] ⇧⌘O opens native Open Folder dialog
- [x] Dialogs work on macOS, Linux, Windows (via rfd)
- [x] Commands appear in command palette

### Phase 3 (Drop Feedback) ✅
- [x] Dragging files shows overlay with file names
- [x] Overlay disappears when drag leaves window
- [x] Dropping opens files (existing behavior preserved)

### Phase 4 (Validation) ✅
- [x] Binary files show error message
- [x] Large files (>50MB) show error with size
- [x] Missing files show "not found" error
- [x] Permission errors are caught

### Phase 5 (CLI) ✅
- [x] `token file.rs` opens file
- [x] `token --new` starts with empty buffer
- [x] `token --line 42 file.rs` opens at line 42
- [x] `token ./src` opens directory as workspace (sets workspace_root)

### Duplicate File Detection ✅
- [x] Opening already-open file focuses existing tab
- [x] Canonicalized path comparison handles symlinks

### Phase 6 (Workspace)
- [ ] Sidebar shows file tree
- [ ] Folders expand/collapse
- [ ] Clicking file opens in tab
- [ ] Cmd+B toggles sidebar
- [ ] External file changes update tree
