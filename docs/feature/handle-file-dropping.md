# File Dropping & Drag-and-Drop Support

Handle drag-and-drop file operations for opening files in the editor.

---

## Overview

### Current State

The editor uses winit for windowing with an `ApplicationHandler` implementation in `src/main.rs`. The event handling in `App::handle_event()` processes `WindowEvent` variants. Currently, there is no handling for:

- `WindowEvent::DroppedFile` - File dropped onto the window
- `WindowEvent::HoveredFile` - File being dragged over the window
- `WindowEvent::HoveredFileCancelled` - Drag operation cancelled

File loading currently uses:

- `Document::from_file(path)` in `src/model/document.rs` - Synchronous file loading
- `AppMsg::LoadFile(PathBuf)` / `AppMsg::FileLoaded` - Async message pattern
- `Cmd::LoadFile { path }` - Command to trigger async file loading

### Goals

1. Handle file drag-and-drop events from winit
2. Visual feedback during drag-hover (show drop zone indication)
3. Support multiple files dropped at once
4. Integrate with the tabs system (open in new tabs)
5. Avoid opening duplicate tabs for already-open files
6. Handle errors gracefully with user-friendly messages

---

## Architecture

### Data Structures

Add to `src/model/ui.rs`:

```rust
/// State for file drop operations
#[derive(Debug, Clone, Default)]
pub struct DropState {
    /// Files currently being hovered over the window
    pub hovered_files: Vec<PathBuf>,
    /// Whether a valid drop target is being hovered
    pub is_hovering: bool,
}
```

Extend `UiState`:

```rust
pub struct UiState {
    // ... existing fields

    /// File drop state for visual feedback
    pub drop_state: DropState,
}
```

### Message Types

Add to `src/messages.rs`:

```rust
/// File drop related messages
#[derive(Debug, Clone)]
pub enum DropMsg {
    /// A file is being dragged over the window
    FileHovered(PathBuf),
    /// Multiple files are being dragged over the window
    FilesHovered(Vec<PathBuf>),
    /// Drag operation left the window or was cancelled
    HoverCancelled,
    /// File(s) were dropped on the window
    FilesDropped(Vec<PathBuf>),
}
```

Extend `AppMsg` for file operations:

```rust
pub enum AppMsg {
    // ... existing

    /// Open file in new tab (from drop or other sources)
    OpenFileInTab {
        path: PathBuf,
        /// If true, switch to tab if file is already open
        activate_existing: bool,
    },
    /// Open multiple files in tabs
    OpenFilesInTabs(Vec<PathBuf>),
    /// File open completed (result of async load)
    FileOpenedInTab {
        path: PathBuf,
        result: Result<String, FileOpenError>,
    },
}

/// Detailed file open errors for user-friendly messages
#[derive(Debug, Clone)]
pub enum FileOpenError {
    NotFound,
    PermissionDenied,
    IsDirectory,
    BinaryFile,
    TooLarge { size_mb: f64 },
    IoError(String),
}
```

Extend top-level `Msg`:

```rust
pub enum Msg {
    // ... existing
    Drop(DropMsg),
}
```

### Commands

Add to `src/commands.rs`:

```rust
pub enum Cmd {
    // ... existing

    /// Open a file and add to tabs (async)
    OpenFile {
        path: PathBuf,
        activate_existing: bool,
    },
}
```

---

## Event Handler Changes

In `src/main.rs`, extend `App::handle_event()`:

```rust
fn handle_event(&mut self, event: &WindowEvent) -> Option<Cmd> {
    match event {
        // ... existing handlers

        // File hovering - drag is over window
        WindowEvent::HoveredFile(path) => {
            update(&mut self.model, Msg::Drop(DropMsg::FileHovered(path.clone())))
        }

        // Drag left window or cancelled
        WindowEvent::HoveredFileCancelled => {
            update(&mut self.model, Msg::Drop(DropMsg::HoverCancelled))
        }

        // File dropped
        WindowEvent::DroppedFile(path) => {
            // Winit sends DroppedFile once per file
            update(&mut self.model, Msg::Drop(DropMsg::FilesDropped(vec![path.clone()])))
        }

        // ... rest of handlers
    }
}
```

**Note on multi-file drops**: Winit sends separate `DroppedFile` events for each file. Each file is handled as a separate tab open operation.

---

## Update Logic

Add to `src/update.rs`:

```rust
/// Handle drop messages
pub fn update_drop(model: &mut AppModel, msg: DropMsg) -> Option<Cmd> {
    match msg {
        DropMsg::FileHovered(path) => {
            model.ui.drop_state.hovered_files = vec![path];
            model.ui.drop_state.is_hovering = true;
            Some(Cmd::Redraw)
        }

        DropMsg::FilesHovered(paths) => {
            model.ui.drop_state.hovered_files = paths;
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

            // Open each file
            let cmds: Vec<Cmd> = paths.into_iter()
                .map(|path| Cmd::OpenFile {
                    path,
                    activate_existing: true
                })
                .collect();

            Some(Cmd::batch(cmds))
        }
    }
}

/// Main update - add drop handling
pub fn update(model: &mut AppModel, msg: Msg) -> Option<Cmd> {
    let result = match msg {
        // ... existing
        Msg::Drop(m) => update_drop(model, m),
    };
    sync_status_bar(model);
    result
}
```

### File Validation

```rust
/// Validate file before attempting to open
fn validate_file_for_opening(path: &Path) -> Result<(), FileOpenError> {
    use std::fs;

    let metadata = fs::metadata(path).map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => FileOpenError::NotFound,
        std::io::ErrorKind::PermissionDenied => FileOpenError::PermissionDenied,
        _ => FileOpenError::IoError(e.to_string()),
    })?;

    if metadata.is_dir() {
        return Err(FileOpenError::IsDirectory);
    }

    // Size check (50MB limit)
    const MAX_SIZE: u64 = 50 * 1024 * 1024;
    if metadata.len() > MAX_SIZE {
        return Err(FileOpenError::TooLarge {
            size_mb: metadata.len() as f64 / (1024.0 * 1024.0)
        });
    }

    Ok(())
}

/// Check if file appears to be binary
fn is_binary_file(path: &Path) -> bool {
    use std::fs::File;
    use std::io::Read;

    if let Ok(mut file) = File::open(path) {
        let mut buffer = [0u8; 8192];
        if let Ok(n) = file.read(&mut buffer) {
            // Check for null bytes (common in binary files)
            return buffer[..n].contains(&0);
        }
    }
    false
}

fn format_error(path: &Path, error: &FileOpenError) -> String {
    let name = path.file_name()
        .unwrap_or_default()
        .to_string_lossy();

    match error {
        FileOpenError::NotFound => format!("File not found: {}", name),
        FileOpenError::PermissionDenied => format!("Permission denied: {}", name),
        FileOpenError::IsDirectory => format!("Cannot open directory: {}", name),
        FileOpenError::BinaryFile => format!("Binary file: {}", name),
        FileOpenError::TooLarge { size_mb } => format!("{} is too large ({:.1} MB)", name, size_mb),
        FileOpenError::IoError(msg) => format!("Error opening {}: {}", name, msg),
    }
}
```

---

## Visual Feedback

### Drop Zone Overlay

Add rendering for the drop zone indicator in `src/main.rs` (Renderer):

```rust
fn render_drop_overlay(&mut self, model: &AppModel) {
    if !model.ui.drop_state.is_hovering {
        return;
    }

    let overlay_color = model.theme.drop_zone.overlay.to_argb_u32();
    let border_color = model.theme.drop_zone.border.to_argb_u32();

    // Draw semi-transparent overlay
    self.fill_rect_alpha(0, 0, self.width, self.height, overlay_color);

    // Draw border (4px wide)
    let border_width = 4;
    self.draw_rect_border(
        border_width, border_width,
        self.width - border_width * 2,
        self.height - border_width * 2,
        border_color, border_width
    );

    // Draw file count/names in center
    let text = if model.ui.drop_state.hovered_files.len() == 1 {
        format!("Drop to open: {}",
            model.ui.drop_state.hovered_files[0]
                .file_name()
                .unwrap_or_default()
                .to_string_lossy())
    } else {
        format!("Drop to open {} files", model.ui.drop_state.hovered_files.len())
    };

    // Center text
    let text_x = (self.width - text.len() * self.char_width as usize) / 2;
    let text_y = self.height / 2;

    self.draw_text(text_x, text_y, &text, model.theme.drop_zone.text.to_argb_u32());
}
```

### Theme Extension

Add to `src/theme.rs`:

```rust
pub struct Theme {
    // ... existing fields
    pub drop_zone: DropZoneTheme,
}

pub struct DropZoneTheme {
    /// Background overlay color (semi-transparent)
    pub overlay: Color,
    /// Border color
    pub border: Color,
    /// Text color for "Drop to open" message
    pub text: Color,
}

impl Default for DropZoneTheme {
    fn default() -> Self {
        Self {
            overlay: Color::new(0x00, 0x00, 0x00, 0xB4),  // Semi-transparent black
            border: Color::new(0x00, 0x7A, 0xCC, 0xFF),   // Blue accent
            text: Color::new(0xFF, 0xFF, 0xFF, 0xFF),     // White
        }
    }
}
```

---

## Integration with EditorArea (Tabs)

When tabs are implemented, extend `src/model/editor_area.rs`:

```rust
impl EditorArea {
    /// Find a document by its file path
    pub fn find_document_by_path(&self, path: &Path) -> Option<DocumentId> {
        self.documents.iter()
            .find(|(_, doc)| doc.file_path.as_ref() == Some(path))
            .map(|(id, _)| *id)
    }

    /// Activate a tab showing the given document
    pub fn activate_document(&mut self, doc_id: DocumentId) -> bool {
        if let Some((group_id, tab_id)) = self.find_tabs_for_document(doc_id).first() {
            self.focused_group_id = *group_id;

            if let Some(group) = self.groups.get_mut(group_id) {
                if let Some(idx) = group.tabs.iter().position(|t| t.id == *tab_id) {
                    group.active_tab_index = idx;
                    return true;
                }
            }
        }
        false
    }
}
```

---

## Edge Cases

| Case               | Handling                                             |
| ------------------ | ---------------------------------------------------- |
| File doesn't exist | Show error in status bar: "File not found: filename" |
| Permission denied  | Show error: "Permission denied: filename"            |
| Binary file        | Detect via null bytes, show "Binary file: filename"  |
| File too large     | Check size before loading (50MB limit)               |
| Directory dropped  | Show "Cannot open directory: dirname"                |
| File already open  | Switch to existing tab, show "Switched to: filename" |
| Multiple files     | Each opens in a new tab; last file becomes active    |
| Symlinks           | `fs::metadata()` follows symlinks; regular handling  |
| Drag cancelled     | `HoveredFileCancelled` clears the drop state         |
| Rapid drops        | Each event processed independently                   |

---

## Implementation Plan

### Phase 1: Basic Drop Handling (No Tabs)

- [ ] Add `DropState` to `UiState`
- [ ] Add `DropMsg` message type
- [ ] Handle `WindowEvent::DroppedFile` and `HoveredFile` in event handler
- [ ] Add `update_drop()` function
- [ ] Open dropped file in current editor (replace document)
- [ ] Basic error handling for file read failures

**Test:** Drop a file onto the editor window; file content replaces current buffer.

### Phase 2: Visual Feedback

- [ ] Add `DropZoneTheme` to theme
- [ ] Implement `render_drop_overlay()` in renderer
- [ ] Show file name(s) during hover
- [ ] Add border effect
- [ ] Update YAML theme with drop zone colors

**Test:** Drag file over window; semi-transparent overlay with border appears.

### Phase 3: File Validation

- [ ] Add `FileOpenError` enum
- [ ] Implement `validate_file_for_opening()`
- [ ] Add binary file detection
- [ ] Add file size limits
- [ ] User-friendly error messages via transient status

**Test:** Drop binary file; shows error message. Drop large file; shows size error.

### Phase 4: Tab Integration

- [ ] Add `find_document_by_path()` to EditorArea
- [ ] Add `activate_document()` to EditorArea
- [ ] Detect already-open files and switch to existing tab
- [ ] Open new files in new tabs in focused group

**Test:** Drop file that's already open; tab switches instead of duplicating.

### Phase 5: Multiple File Support

- [ ] Handle multiple `DroppedFile` events
- [ ] Open each in separate tab
- [ ] Activate last dropped file's tab
- [ ] Show count in hover overlay ("Drop to open 3 files")

**Test:** Drop folder selection with 3 files; all open as tabs.

---

## Success Criteria

- [ ] Dragging files over window shows visual overlay
- [ ] Dropping files opens them in tabs
- [ ] Already-open files switch to existing tab
- [ ] Binary files show error message
- [ ] Large files show error with size
- [ ] Multiple files dropped all open as tabs
