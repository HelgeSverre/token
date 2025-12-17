# Workspace Management & CLI Arguments

A comprehensive workspace management system including CLI argument handling, file tree sidebar, and workspace tracking.

---

## Overview

### Current State

The editor currently has:

- ✅ CLI argument parsing with clap (v0.3.0)
- ✅ `EditorArea` with split views and tabs (v0.3.0)
- ✅ `ScaledMetrics` for HiDPI support (v0.3.4)
- No workspace concept
- No file tree sidebar

### Goals

1. ~~**CLI Argument Parsing**: Support multiple files, directories, flags~~ ✅ Done (v0.3.0)
2. **Workspace Concept**: Track root directory, manage open files
3. **File Tree Sidebar**: VS Code-style tree with expand/collapse, icons
4. **File System Watching**: React to external file changes
5. **Integration with Split View**: Files open in tabs within groups

### HiDPI Considerations (v0.3.4+)

All layout constants must use `ScaledMetrics` to work correctly on Retina/HiDPI displays. See [UI-SCALING-REVIEW.md](../archived/UI-SCALING-REVIEW.md) for background.

**New constants needed in `ScaledMetrics`:**
```rust
// File tree layout (base values at scale factor 1.0)
const BASE_FILE_TREE_ROW_HEIGHT: f64 = 22.0;
const BASE_FILE_TREE_INDENT: f64 = 16.0;
const BASE_SIDEBAR_DEFAULT_WIDTH: f64 = 250.0;
const BASE_SIDEBAR_MIN_WIDTH: f64 = 150.0;
const BASE_SIDEBAR_MAX_WIDTH: f64 = 500.0;
const BASE_RESIZE_HANDLE_ZONE: f64 = 4.0;
```

---

## Part 1: CLI Argument Handling

### CLI Specification

```
USAGE:
    red [OPTIONS] [PATHS]...

ARGS:
    [PATHS]...    Files or directories to open

OPTIONS:
    -n, --new              Start with empty buffer (ignore session)
    -w, --wait             Wait for all files to close (for git/svn)
    --line <N>             Go to line N in first file
    --column <N>           Go to column N (with --line)
    -h, --help             Print help information
    -V, --version          Print version information

EXAMPLES:
    red file.rs            # Open single file
    red src/lib.rs main.rs # Open multiple files as tabs
    red ./src              # Open directory as workspace
    red --new              # Start with empty buffer
    red -w COMMIT_MSG      # Wait mode for git commit
```

### Data Structures

Create `src/cli.rs`:

```rust
use clap::Parser;
use std::path::PathBuf;

/// Rust text editor
#[derive(Parser, Debug)]
#[command(name = "red", version, about)]
pub struct CliArgs {
    /// Files or directories to open
    #[arg(value_name = "PATHS")]
    pub paths: Vec<PathBuf>,

    /// Start with empty buffer (ignore session)
    #[arg(short = 'n', long)]
    pub new: bool,

    /// Wait for all files to close (for git/svn integration)
    #[arg(short = 'w', long)]
    pub wait: bool,

    /// Go to line N in first file
    #[arg(long)]
    pub line: Option<usize>,

    /// Go to column N (with --line)
    #[arg(long)]
    pub column: Option<usize>,
}

/// Parsed and validated startup configuration
#[derive(Debug, Clone)]
pub struct StartupConfig {
    /// Mode to start the editor in
    pub mode: StartupMode,
    /// Initial cursor position (if specified)
    pub initial_position: Option<(usize, usize)>, // (line, column)
    /// Whether to wait for close (git integration)
    pub wait_mode: bool,
}

#[derive(Debug, Clone)]
pub enum StartupMode {
    /// Start with empty buffer
    Empty,
    /// Open single file
    SingleFile(PathBuf),
    /// Open multiple files as tabs
    MultipleFiles(Vec<PathBuf>),
    /// Open directory as workspace
    Workspace {
        root: PathBuf,
        initial_files: Vec<PathBuf>,
    },
}

impl CliArgs {
    /// Validate and convert to StartupConfig
    pub fn into_config(self) -> Result<StartupConfig, String> {
        let mode = if self.new || self.paths.is_empty() {
            StartupMode::Empty
        } else if self.paths.len() == 1 {
            let path = &self.paths[0];
            if path.is_dir() {
                StartupMode::Workspace {
                    root: path.clone(),
                    initial_files: vec![],
                }
            } else {
                StartupMode::SingleFile(path.clone())
            }
        } else {
            // Multiple paths - check for directory
            let (dirs, files): (Vec<_>, Vec<_>) = self.paths
                .iter()
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

/// Check path status and handle gracefully
pub fn validate_path(path: &PathBuf) -> PathStatus {
    if path.exists() {
        if path.is_dir() {
            PathStatus::Directory
        } else {
            PathStatus::File
        }
    } else if path.parent().map(|p| p.exists()).unwrap_or(true) {
        // Parent exists, can create new file
        PathStatus::NewFile
    } else {
        PathStatus::Invalid("Parent directory does not exist".to_string())
    }
}

pub enum PathStatus {
    File,
    Directory,
    NewFile,
    Invalid(String),
}
```

---

## Part 2: Workspace Concept

### Data Structures

Create `src/model/workspace.rs`:

```rust
use std::path::PathBuf;
use std::collections::HashSet;

/// Workspace configuration and state
#[derive(Debug, Clone)]
pub struct Workspace {
    /// Root directory of the workspace
    pub root: PathBuf,

    /// Expanded folder paths (relative to root)
    pub expanded_folders: HashSet<PathBuf>,

    /// Currently selected item in file tree
    pub selected_item: Option<PathBuf>,

    /// File tree cache (populated by file system scan)
    pub file_tree: FileTree,

    /// Sidebar visibility
    pub sidebar_visible: bool,

    /// Sidebar width in LOGICAL pixels (multiply by scale_factor for physical)
    /// This ensures the width remains consistent when switching displays.
    pub sidebar_width_logical: f32,
}

impl Workspace {
    /// Get sidebar width in physical pixels
    pub fn sidebar_width(&self, scale_factor: f64) -> f32 {
        self.sidebar_width_logical * scale_factor as f32
    }

    /// Set sidebar width from physical pixels
    pub fn set_sidebar_width(&mut self, physical_width: f32, scale_factor: f64) {
        self.sidebar_width_logical = physical_width / scale_factor as f32;
    }

    pub fn new(root: PathBuf, metrics: &ScaledMetrics) -> std::io::Result<Self> {
        let file_tree = FileTree::from_directory(&root)?;
        Ok(Self {
            root,
            expanded_folders: HashSet::new(),
            selected_item: None,
            file_tree,
            sidebar_visible: true,
            // Store logical width; ScaledMetrics has the base values
            sidebar_width_logical: metrics.sidebar_default_width_logical,
        })
    }

    /// Clamp sidebar width to valid range (in logical pixels)
    pub fn clamp_sidebar_width(&mut self, metrics: &ScaledMetrics) {
        self.sidebar_width_logical = self.sidebar_width_logical
            .max(metrics.sidebar_min_width_logical)
            .min(metrics.sidebar_max_width_logical);
    }
}

/// Hierarchical file tree structure
#[derive(Debug, Clone, Default)]
pub struct FileTree {
    pub root: Option<FileNode>,
    pub last_scanned: Option<std::time::Instant>,
}

/// A node in the file tree
#[derive(Debug, Clone)]
pub struct FileNode {
    /// File or directory name
    pub name: String,
    /// Full path (absolute)
    pub path: PathBuf,
    /// Node type
    pub node_type: FileNodeType,
    /// Children (sorted: folders first, then alphabetical)
    pub children: Vec<FileNode>,
    /// Is this node expanded in the UI?
    pub is_expanded: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileNodeType {
    File { extension: FileExtension },
    Directory,
    SymLink,
}

/// Known file extensions for icon mapping
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FileExtension {
    Rust,       // .rs
    Toml,       // .toml, Cargo.toml
    Markdown,   // .md
    Yaml,       // .yaml, .yml
    Json,       // .json
    Git,        // .gitignore, .gitattributes
    License,    // LICENSE*
    Readme,     // README*
    #[default]
    Unknown,
}

impl FileExtension {
    pub fn from_path(path: &PathBuf) -> Self {
        let name = path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");

        // Check special file names first
        if name.starts_with("LICENSE") { return Self::License; }
        if name.starts_with("README") { return Self::Readme; }
        if name == "Cargo.toml" { return Self::Toml; }
        if name.starts_with(".git") { return Self::Git; }

        // Check extension
        match path.extension().and_then(|e| e.to_str()) {
            Some("rs") => Self::Rust,
            Some("toml") => Self::Toml,
            Some("md") => Self::Markdown,
            Some("yaml") | Some("yml") => Self::Yaml,
            Some("json") => Self::Json,
            _ => Self::Unknown,
        }
    }

    /// Get icon character for this file type
    pub fn icon(&self) -> &'static str {
        match self {
            Self::Rust => "rs",
            Self::Toml => "{}",
            Self::Markdown => "md",
            Self::Yaml => "ym",
            Self::Json => "js",
            Self::Git => "gi",
            Self::License => "li",
            Self::Readme => "rd",
            Self::Unknown => "  ",
        }
    }
}
```

### Building the File Tree

```rust
impl FileTree {
    /// Scan directory and build file tree
    pub fn from_directory(root: &PathBuf) -> std::io::Result<Self> {
        let root_node = Self::scan_directory(root, 0)?;
        Ok(Self {
            root: Some(root_node),
            last_scanned: Some(std::time::Instant::now()),
        })
    }

    fn scan_directory(path: &PathBuf, depth: usize) -> std::io::Result<FileNode> {
        const MAX_DEPTH: usize = 20; // Prevent infinite recursion

        let name = path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(".")
            .to_string();

        if !path.is_dir() {
            return Ok(FileNode {
                name,
                path: path.clone(),
                node_type: FileNodeType::File {
                    extension: FileExtension::from_path(path),
                },
                children: vec![],
                is_expanded: false,
            });
        }

        let mut children = Vec::new();

        if depth < MAX_DEPTH {
            for entry in std::fs::read_dir(path)? {
                let entry = entry?;
                let child_path = entry.path();

                let child_name = child_path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("");

                // Skip hidden files (except .gitignore)
                if child_name.starts_with('.')
                    && !matches!(child_name, ".gitignore" | ".gitattributes")
                {
                    continue;
                }

                // Skip common ignore patterns
                if matches!(child_name, "target" | "node_modules" | ".git" | "__pycache__") {
                    continue;
                }

                children.push(Self::scan_directory(&child_path, depth + 1)?);
            }
        }

        // Sort: directories first, then alphabetical (case-insensitive)
        children.sort_by(|a, b| {
            match (&a.node_type, &b.node_type) {
                (FileNodeType::Directory, FileNodeType::File { .. }) => std::cmp::Ordering::Less,
                (FileNodeType::File { .. }, FileNodeType::Directory) => std::cmp::Ordering::Greater,
                _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
            }
        });

        Ok(FileNode {
            name,
            path: path.clone(),
            node_type: FileNodeType::Directory,
            children,
            is_expanded: depth == 0, // Root is expanded by default
        })
    }

    /// Refresh the file tree from disk
    pub fn refresh(&mut self, root: &PathBuf) -> std::io::Result<()> {
        *self = Self::from_directory(root)?;
        Ok(())
    }
}
```

---

## Part 3: Messages for Workspace

Add to `src/messages.rs`:

```rust
/// Workspace and file tree messages
#[derive(Debug, Clone)]
pub enum WorkspaceMsg {
    // === Sidebar ===
    /// Toggle sidebar visibility
    ToggleSidebar,
    /// Set sidebar width
    SetSidebarWidth(f32),
    /// Start sidebar resize drag
    StartSidebarResize { initial_x: f64 },
    /// Update sidebar resize
    UpdateSidebarResize { current_x: f64 },
    /// End sidebar resize
    EndSidebarResize,

    // === File Tree Navigation ===
    /// Toggle folder expand/collapse
    ToggleFolder(PathBuf),
    /// Expand all folders
    ExpandAll,
    /// Collapse all folders
    CollapseAll,
    /// Select item in tree
    SelectItem(PathBuf),
    /// Move selection up
    SelectPrevious,
    /// Move selection down
    SelectNext,

    // === File Operations ===
    /// Open file from tree (single click = preview, double click = permanent)
    OpenFile { path: PathBuf, preview: bool },
    /// Open file in split
    OpenFileInSplit { path: PathBuf, direction: SplitDirection },
    /// Reveal current file in tree
    RevealActiveFile,

    // === File System ===
    /// Refresh file tree
    Refresh,
    /// File system change detected
    FileSystemChange(FileSystemEvent),
}

#[derive(Debug, Clone)]
pub enum FileSystemEvent {
    Created(PathBuf),
    Modified(PathBuf),
    Deleted(PathBuf),
    Renamed { from: PathBuf, to: PathBuf },
}

// Update top-level Msg enum
#[derive(Debug, Clone)]
pub enum Msg {
    Editor(EditorMsg),
    Document(DocumentMsg),
    Ui(UiMsg),
    App(AppMsg),
    Layout(LayoutMsg),
    Workspace(WorkspaceMsg), // NEW
}
```

---

## Part 4: AppModel Integration

Update `src/model/mod.rs`:

```rust
pub mod workspace;
pub use workspace::{Workspace, FileTree, FileNode, FileNodeType, FileExtension};

/// The complete application model
pub struct AppModel {
    /// Editor area with all documents, editors, groups, and layout
    pub editor_area: EditorArea,

    /// Workspace state (optional - None when no workspace open)
    pub workspace: Option<Workspace>,

    /// UI state
    pub ui: UiState,

    /// Theme
    pub theme: Theme,

    /// Window dimensions
    pub window_size: (u32, u32),

    /// Font metrics
    pub line_height: usize,
    pub char_width: f32,

    /// Sidebar resize state
    pub sidebar_resize: Option<SidebarResizeState>,
}

#[derive(Debug, Clone)]
pub struct SidebarResizeState {
    pub initial_width: f32,
    pub initial_x: f64,
}

impl AppModel {
    /// Create from startup config
    pub fn from_startup_config(
        config: &StartupConfig,
        window_width: u32,
        window_height: u32,
    ) -> Self {
        match &config.mode {
            StartupMode::Empty => Self::new(window_width, window_height, None),
            StartupMode::SingleFile(path) => {
                Self::new(window_width, window_height, Some(path.clone()))
            }
            StartupMode::MultipleFiles(paths) => {
                Self::with_multiple_files(window_width, window_height, paths)
            }
            StartupMode::Workspace { root, initial_files } => {
                Self::with_workspace(window_width, window_height, root, initial_files)
            }
        }
    }

    /// Get the editor area x-offset (after sidebar)
    pub fn editor_area_x(&self) -> f32 {
        if let Some(workspace) = &self.workspace {
            if workspace.sidebar_visible {
                return workspace.sidebar_width;
            }
        }
        0.0
    }

    /// Get sidebar rect if visible
    pub fn sidebar_rect(&self) -> Option<Rect> {
        self.workspace.as_ref().and_then(|ws| {
            if ws.sidebar_visible {
                Some(Rect::new(
                    0.0,
                    0.0,
                    ws.sidebar_width,
                    self.window_size.1 as f32,
                ))
            } else {
                None
            }
        })
    }
}
```

---

## Part 5: Rendering the File Tree

### Theme Extensions

Add to `src/theme.rs`:

```rust
pub struct Theme {
    // ... existing fields
    pub sidebar: SidebarTheme,
    pub file_tree: FileTreeTheme,
}

pub struct SidebarTheme {
    pub background: Color,
    pub border: Color,
    pub resize_handle: Color,
    pub resize_handle_hover: Color,
}

pub struct FileTreeTheme {
    pub background: Color,
    pub foreground: Color,
    pub selected_background: Color,
    pub selected_foreground: Color,
    pub hover_background: Color,
    pub indent_guide: Color,
    pub folder_icon: Color,
    pub file_icon: Color,
}

impl Default for SidebarTheme {
    fn default() -> Self {
        Self {
            background: Color::rgb(0x1E, 0x1E, 0x1E),
            border: Color::rgb(0x3C, 0x3C, 0x3C),
            resize_handle: Color::rgb(0x00, 0x7A, 0xCC),
            resize_handle_hover: Color::rgb(0x00, 0x9A, 0xCC),
        }
    }
}

impl Default for FileTreeTheme {
    fn default() -> Self {
        Self {
            background: Color::rgb(0x1E, 0x1E, 0x1E),
            foreground: Color::rgb(0xCC, 0xCC, 0xCC),
            selected_background: Color::rgb(0x04, 0x39, 0x5E),
            selected_foreground: Color::rgb(0xFF, 0xFF, 0xFF),
            hover_background: Color::rgb(0x2A, 0x2D, 0x2E),
            indent_guide: Color::rgb(0x40, 0x40, 0x40),
            folder_icon: Color::rgb(0xDC, 0xDC, 0xAA),
            file_icon: Color::rgb(0x9C, 0xDC, 0xFE),
        }
    }
}
```

### Rendering Logic

**Note:** All layout values come from `ScaledMetrics`, not hardcoded constants.

```rust
impl Renderer {
    /// Render the file tree sidebar
    pub fn render_sidebar(
        &mut self,
        workspace: &Workspace,
        open_files: &[PathBuf],
        model: &AppModel,  // Access metrics and scale_factor
        theme: &Theme,
    ) -> Result<()> {
        if !workspace.sidebar_visible {
            return Ok(());
        }

        let metrics = &model.metrics;
        let sidebar_width = workspace.sidebar_width(model.scale_factor);
        let rect = Rect::new(0.0, 0.0, sidebar_width, self.height as f32);

        // Background
        self.fill_rect(&rect, theme.sidebar.background.to_argb_u32());

        // Render file tree
        if let Some(root) = &workspace.file_tree.root {
            let mut y_offset = 0.0;
            self.render_file_node(
                root,
                0,
                &mut y_offset,
                workspace,
                open_files,
                &rect,
                metrics,
                theme,
            )?;
        }

        // Render resize border
        let border_x = sidebar_width - 1.0;
        self.draw_vertical_line(
            border_x,
            0.0,
            self.height as f32,
            theme.sidebar.border.to_argb_u32(),
        );

        Ok(())
    }

    fn render_file_node(
        &mut self,
        node: &FileNode,
        depth: usize,
        y_offset: &mut f32,
        workspace: &Workspace,
        open_files: &[PathBuf],
        sidebar_rect: &Rect,
        metrics: &ScaledMetrics,  // Use scaled values
        theme: &Theme,
    ) -> Result<()> {
        let row_height = metrics.file_tree_row_height as f32;
        let indent = metrics.file_tree_indent;
        let padding = metrics.padding_small as f32;

        let x = indent * depth as f32;
        let y = *y_offset;

        // Skip if below visible area
        if y > sidebar_rect.height {
            return Ok(());
        }

        // Row background (selection)
        let is_selected = workspace.selected_item.as_ref() == Some(&node.path);
        let _is_open = open_files.contains(&node.path);

        let row_rect = Rect::new(0.0, y, sidebar_rect.width, row_height);

        if is_selected {
            self.fill_rect(&row_rect, theme.file_tree.selected_background.to_argb_u32());
        }

        // Expand/collapse arrow for directories
        if matches!(node.node_type, FileNodeType::Directory) {
            let arrow = if node.is_expanded { "v" } else { ">" };
            self.draw_text(x + padding, y + padding, arrow,
                theme.file_tree.folder_icon.to_argb_u32())?;
        }

        // Icon (based on file type)
        let icon_x = x + indent;
        let icon = match &node.node_type {
            FileNodeType::Directory => "D ",
            FileNodeType::File { extension } => extension.icon(),
            FileNodeType::SymLink => "@ ",
        };
        let icon_color = match node.node_type {
            FileNodeType::Directory => theme.file_tree.folder_icon,
            _ => theme.file_tree.file_icon,
        };
        self.draw_text(icon_x, y + padding, icon, icon_color.to_argb_u32())?;

        // File name
        let text_x = icon_x + indent + padding;
        let text_color = if is_selected {
            theme.file_tree.selected_foreground
        } else {
            theme.file_tree.foreground
        };
        self.draw_text(text_x, y + padding, &node.name, text_color.to_argb_u32())?;

        *y_offset += row_height;

        // Render children if expanded
        if node.is_expanded && matches!(node.node_type, FileNodeType::Directory) {
            for child in &node.children {
                self.render_file_node(
                    child,
                    depth + 1,
                    y_offset,
                    workspace,
                    open_files,
                    sidebar_rect,
                    metrics,
                    theme,
                )?;
            }
        }

        Ok(())
    }
}
```

---

## Part 6: File System Watching

Using the `notify` crate for cross-platform file system watching.

### Dependencies

```toml
# Cargo.toml
notify = "6.1"
```

### Watcher Module

Create `src/fs_watcher.rs`:

```rust
use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::PathBuf;
use std::sync::mpsc;

pub struct FileSystemWatcher {
    watcher: RecommendedWatcher,
    rx: mpsc::Receiver<Result<Event, notify::Error>>,
}

impl FileSystemWatcher {
    pub fn new() -> Result<Self, notify::Error> {
        let (tx, rx) = mpsc::channel();

        let watcher = RecommendedWatcher::new(
            move |res| {
                let _ = tx.send(res);
            },
            Config::default(),
        )?;

        Ok(Self { watcher, rx })
    }

    /// Watch a workspace directory
    pub fn watch_workspace(&mut self, path: &PathBuf) -> Result<(), notify::Error> {
        self.watcher.watch(path, RecursiveMode::Recursive)
    }

    /// Check for pending events (non-blocking)
    pub fn poll_events(&self) -> Vec<FileSystemEvent> {
        let mut events = Vec::new();

        while let Ok(Ok(event)) = self.rx.try_recv() {
            for path in event.paths {
                let fs_event = match event.kind {
                    notify::EventKind::Create(_) => FileSystemEvent::Created(path),
                    notify::EventKind::Modify(_) => FileSystemEvent::Modified(path),
                    notify::EventKind::Remove(_) => FileSystemEvent::Deleted(path),
                    _ => continue,
                };
                events.push(fs_event);
            }
        }

        events
    }
}
```

---

## Part 7: Mouse Interaction

### Hit Testing for Sidebar

**Note:** All hit testing uses `ScaledMetrics` for proper HiDPI support.

```rust
impl App {
    fn handle_mouse_click(&mut self, x: f64, y: f64, button: MouseButton) {
        let metrics = &self.model.metrics;
        let scale_factor = self.model.scale_factor;

        // Check if click is in sidebar
        if let Some(workspace) = &self.model.workspace {
            let sidebar_width = workspace.sidebar_width(scale_factor) as f64;
            let resize_zone = metrics.resize_handle_zone as f64;

            if workspace.sidebar_visible && x < sidebar_width {
                // Check for resize border click (scaled zone)
                if (x - sidebar_width).abs() < resize_zone {
                    self.dispatch(Msg::Workspace(WorkspaceMsg::StartSidebarResize {
                        initial_x: x,
                    }));
                    return;
                }

                // File tree click
                if let Some(path) = self.file_tree_hit_test(x, y, workspace, metrics) {
                    let is_double = self.is_double_click();
                    self.dispatch(Msg::Workspace(WorkspaceMsg::OpenFile {
                        path,
                        preview: !is_double,
                    }));
                }
                return;
            }
        }

        // Otherwise, delegate to editor area
    }

    fn file_tree_hit_test(
        &self,
        _x: f64,
        y: f64,
        workspace: &Workspace,
        metrics: &ScaledMetrics,  // Use scaled row height
    ) -> Option<PathBuf> {
        let row_height = metrics.file_tree_row_height as f64;

        // Calculate which row was clicked
        let row = (y / row_height) as usize;

        // Walk the tree to find the nth visible item
        self.find_nth_visible_item(&workspace.file_tree, row)
    }
}
```

---

## Part 8: Keyboard Shortcuts

| Action          | Shortcut             | Message                                     |
| --------------- | -------------------- | ------------------------------------------- |
| Toggle sidebar  | Cmd+B                | `WorkspaceMsg::ToggleSidebar`               |
| Focus file tree | Cmd+Shift+E          | (Focus management)                          |
| Navigate up     | Arrow Up (in tree)   | `WorkspaceMsg::SelectPrevious`              |
| Navigate down   | Arrow Down (in tree) | `WorkspaceMsg::SelectNext`                  |
| Expand folder   | Right Arrow / Enter  | `WorkspaceMsg::ToggleFolder`                |
| Collapse folder | Left Arrow           | `WorkspaceMsg::ToggleFolder`                |
| Open file       | Enter                | `WorkspaceMsg::OpenFile { preview: false }` |
| Reveal in tree  | Cmd+Shift+R          | `WorkspaceMsg::RevealActiveFile`            |

---

## Dependencies to Add

```toml
# Cargo.toml additions
clap = { version = "4.4", features = ["derive"] }
notify = "6.1"
```

---

## Implementation Plan

### Phase 0: ScaledMetrics Extension ✅ COMPLETE

- [x] Add file tree constants to `ScaledMetrics` in `src/model/mod.rs`:
  - `file_tree_row_height: usize`
  - `file_tree_indent: f32`
  - `sidebar_default_width_logical: f32`
  - `sidebar_min_width_logical: f32`
  - `sidebar_max_width_logical: f32`
  - `resize_handle_zone: usize`

**Test:** `ScaledMetrics::new(2.0).file_tree_row_height == 44` (22 * 2)

### Phase 1: CLI Argument Parsing ✅ COMPLETE (v0.3.0)

- [x] Add `clap` dependency to `Cargo.toml`
- [x] Create `src/cli.rs` module
- [x] Implement `CliArgs` struct with derive macros
- [x] Implement `StartupConfig` with validation
- [x] Update `main()` to use new CLI parsing
- [x] Handle non-existent paths gracefully
- [x] Add `--help` and `--version` output

**Status:** Implemented in v0.3.0. See CHANGELOG.md.

### Phase 2: Workspace Data Structures ✅ COMPLETE

- [x] Create `src/model/workspace.rs` module
- [x] Implement `Workspace`, `FileTree`, `FileNode` structs
- [x] Implement `FileExtension` classification
- [x] Add directory scanning with filtering
- [x] Add sorting (folders first, alphabetical)
- [x] Integrate `Workspace` into `AppModel`

**Test:** Opening directory creates valid `FileTree` with sorted entries.

### Phase 3: Messages and Update Logic ✅ COMPLETE

- [x] Add `WorkspaceMsg` enum to `messages.rs`
- [x] Add `update_workspace()` function
- [x] Implement folder expand/collapse
- [x] Implement file selection
- [x] Implement sidebar toggle
- [x] Implement sidebar resize

**Test:** Cmd+1 toggles sidebar; clicking folders expands/collapses.

### Phase 4: Sidebar Rendering ✅ COMPLETE

- [x] Add `SidebarTheme` to theme
- [x] Implement `render_sidebar()` in Renderer
- [x] Implement file tree rendering with indentation
- [x] Add expand/collapse indicators
- [x] Add selection highlighting
- [x] Render resize border

**Test:** Sidebar renders with file tree; selection is highlighted.

### Phase 5: Mouse Interaction ✅ COMPLETE

- [x] Implement sidebar hit testing
- [x] Implement file tree row hit testing
- [x] Handle click on folder (toggle expand)
- [x] Handle click on file (open in tab)
- [x] Implement resize drag
- [x] Update cursor on resize hover (ColResize on sidebar edge)

**Test:** Clicking file opens it; clicking folder toggles expansion.

### Phase 6: Keyboard Navigation ✅ COMPLETE

- [x] Add keyboard shortcuts for sidebar (Cmd+1)
- [x] Add reveal in sidebar shortcut (Cmd+Shift+R)
- [x] Implement arrow key navigation in tree (Up/Down select items)
- [x] Implement expand/collapse with Enter/arrows (Left collapses, Right expands)
- [x] Enter opens files or toggles folders
- [x] Space toggles folder expansion
- [x] Escape returns focus to editor
- [x] Focus management system with `FocusTarget` enum
  - Click on sidebar transfers focus to sidebar
  - Click outside sidebar returns focus to editor
  - Modals capture/release focus on open/close
  - Hiding sidebar returns focus to editor if focused

**Test:** Arrow keys navigate tree; Enter opens selected file.

### Phase 7: File System Watching

- [ ] Add `notify` dependency
- [ ] Create `src/fs_watcher.rs` module
- [ ] Integrate watcher into event loop
- [ ] Handle create/modify/delete events
- [ ] Refresh tree on changes

**Test:** Create file externally; it appears in tree automatically.

### Phase 8: Tab Integration

- [ ] Wire `OpenFile` to `EditorArea.open_document()`
- [ ] Support preview tabs (single-click opens preview)
- [ ] Support opening in new split pane
- [ ] Highlight open files in tree
- [ ] Sync tree selection with active tab

**Test:** Opening file from tree creates tab; switching tabs updates tree selection.

---

## Success Criteria

- [x] CLI supports `red file1 file2` to open multiple files
- [x] CLI supports `red ./src` to open directory as workspace
- [x] Sidebar shows file tree with folders first
- [x] Folders expand/collapse on click
- [ ] Single-click opens file in preview mode (currently opens permanently)
- [x] Double-click opens file permanently
- [x] Cmd+B toggles sidebar (Cmd+1 in current implementation)
- [ ] File changes detected and tree updates (Phase 7)
- [ ] Open files highlighted in tree (Phase 8)
