# File Dialogs

Cross-platform native file and folder dialogs with unified API.

> **Status:** Planned
> **Priority:** P2
> **Effort:** M
> **Created:** 2025-01-07
> **Milestone:** 1 - Core

---

## Table of Contents

1. [Overview](#overview)
2. [Architecture](#architecture)
3. [Platform Behavior](#platform-behavior)
4. [Data Structures](#data-structures)
5. [Implementation Plan](#implementation-plan)
6. [Testing Strategy](#testing-strategy)
7. [References](#references)

---

## Overview

### Current State

The editor uses `rfd` (Rusty File Dialogs) for file and folder dialogs. The `rfd` crate only supports:
- `FileDialog::pick_file()` / `pick_files()` - Opens a file picker
- `FileDialog::pick_folder()` - Opens a folder picker

There is no way to have a single dialog that allows selecting **either** a file or a folder.

### Goals

1. **Unified API** - Single `open_file_or_folder()` function that works cross-platform
2. **Native dialogs** - Use OS-native dialogs for best UX
3. **macOS enhancement** - Use `NSOpenPanel` which natively supports file+folder selection
4. **Graceful fallback** - On Windows/Linux, use separate commands since OS doesn't support combined dialogs

### Non-Goals

- Custom in-app file browser (that's a separate feature)
- Remote file systems or cloud storage
- Replacing the existing `rfd` integration for simple file saves

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         File Dialog Architecture                             │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ┌────────────────────────────────────────────────────────────────────────┐ │
│  │                         Platform Abstraction                            │ │
│  │                                                                         │ │
│  │   pub fn open_file_or_folder() -> Option<OpenTarget>                   │ │
│  │                                                                         │ │
│  │   ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐       │ │
│  │   │     macOS       │  │    Windows      │  │     Linux       │       │ │
│  │   │   NSOpenPanel   │  │  rfd fallback   │  │  rfd fallback   │       │ │
│  │   │  (file+folder)  │  │  (two dialogs)  │  │  (two dialogs)  │       │ │
│  │   └─────────────────┘  └─────────────────┘  └─────────────────┘       │ │
│  │                                                                         │ │
│  └────────────────────────────────────────────────────────────────────────┘ │
│                                                                              │
│  ┌────────────────────────────────────────────────────────────────────────┐ │
│  │                          Menu / Commands                                │ │
│  │                                                                         │ │
│  │   macOS:                     Windows/Linux:                            │ │
│  │   ┌─────────────────────┐    ┌─────────────────────┐                   │ │
│  │   │  File > Open...     │    │  File > Open File   │                   │ │
│  │   │  (single dialog)    │    │  File > Open Folder │                   │ │
│  │   └─────────────────────┘    └─────────────────────┘                   │ │
│  │                                                                         │ │
│  └────────────────────────────────────────────────────────────────────────┘ │
│                                                                              │
└──────────────────────────────────────────────────────────────────────────────┘
```

### Flow Diagram

```
User triggers "Open"
        │
        ▼
┌───────────────────┐
│  Platform Check   │
└───────────────────┘
        │
        ├── macOS ──────────────────────────────────┐
        │                                           ▼
        │                              ┌─────────────────────────┐
        │                              │   NSOpenPanel           │
        │                              │   canChooseFiles: YES   │
        │                              │   canChooseDirectories: │
        │                              │   YES                   │
        │                              └─────────────────────────┘
        │                                           │
        │                                           ▼
        │                              ┌─────────────────────────┐
        │                              │  Check if selected      │
        │                              │  path is file or dir    │
        │                              └─────────────────────────┘
        │                                           │
        │                                           ▼
        │                              ┌─────────────────────────┐
        │                              │  Return OpenTarget::    │
        │                              │  File or Folder         │
        │                              └─────────────────────────┘
        │
        └── Windows/Linux ──────────────────────────┐
                                                    ▼
                                       ┌─────────────────────────┐
                                       │  Show in-app prompt:    │
                                       │  "Open File" or         │
                                       │  "Open Folder"?         │
                                       └─────────────────────────┘
                                                    │
                            ┌───────────────────────┴───────────────────────┐
                            ▼                                               ▼
               ┌─────────────────────────┐                     ┌─────────────────────────┐
               │  rfd::FileDialog::      │                     │  rfd::FileDialog::      │
               │  pick_file()            │                     │  pick_folder()          │
               └─────────────────────────┘                     └─────────────────────────┘
                            │                                               │
                            ▼                                               ▼
               ┌─────────────────────────┐                     ┌─────────────────────────┐
               │  Return OpenTarget::    │                     │  Return OpenTarget::    │
               │  File(path)             │                     │  Folder(path)           │
               └─────────────────────────┘                     └─────────────────────────┘
```

### Module Structure

```
src/
├── platform/
│   ├── mod.rs              # Platform abstraction exports
│   ├── dialogs.rs          # Cross-platform dialog API
│   └── macos/
│       └── dialogs.rs      # macOS NSOpenPanel implementation
├── messages.rs             # Add DocumentMsg::OpenFileOrFolder
└── update/
    └── document.rs         # Handle open result
```

---

## Platform Behavior

### macOS

Uses `NSOpenPanel` directly via `objc2` crate:
- `setCanChooseFiles(YES)`
- `setCanChooseDirectories(YES)`
- Single native dialog that allows selecting either files or folders
- User sees unified file browser with both options available

**UX:** Single "Open..." menu item, single dialog.

### Windows

`IFileOpenDialog` does not cleanly support combined file+folder selection:
- `FOS_PICKFOLDERS` flag makes it folder-only
- No official "pick file or folder" mode
- Double-clicking folders navigates into them rather than selecting them

**Fallback approach:**
1. Show a small in-app modal: "What would you like to open?"
   - [Open File] [Open Folder] [Cancel]
2. Based on selection, call `rfd::FileDialog::pick_file()` or `pick_folder()`

**UX:** Either two menu items ("Open File...", "Open Folder...") OR a pre-dialog choice.

### Linux (GTK / XDG Portals)

GTK `GtkFileChooserDialog` modes are mutually exclusive:
- `GTK_FILE_CHOOSER_ACTION_OPEN` - files only
- `GTK_FILE_CHOOSER_ACTION_SELECT_FOLDER` - folders only

XDG Desktop Portal has the same limitation.

**Fallback approach:** Same as Windows - in-app prompt or separate menu items.

**UX:** Either two menu items or a pre-dialog choice.

---

## Data Structures

### OpenTarget Enum

```rust
// src/platform/dialogs.rs

use std::path::PathBuf;

/// Result of a file/folder open dialog
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenTarget {
    /// User selected a file
    File(PathBuf),
    /// User selected a folder (workspace)
    Folder(PathBuf),
}

impl OpenTarget {
    /// Get the path regardless of type
    pub fn path(&self) -> &PathBuf {
        match self {
            OpenTarget::File(p) | OpenTarget::Folder(p) => p,
        }
    }

    /// Check if this is a file
    pub fn is_file(&self) -> bool {
        matches!(self, OpenTarget::File(_))
    }

    /// Check if this is a folder
    pub fn is_folder(&self) -> bool {
        matches!(self, OpenTarget::Folder(_))
    }
}
```

### Dialog Options

```rust
/// Options for opening files/folders
#[derive(Debug, Clone, Default)]
pub struct OpenDialogOptions {
    /// Window title (optional, uses platform default if None)
    pub title: Option<String>,
    /// Starting directory (optional)
    pub default_path: Option<PathBuf>,
    /// Allow multiple selection
    pub multiple: bool,
    /// Parent window handle for proper modal behavior
    pub parent: Option<raw_window_handle::RawWindowHandle>,
}
```

### Platform-Specific Selection Prompt (Windows/Linux)

```rust
/// What the user wants to open (for platforms without combined dialogs)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenKind {
    File,
    Folder,
}
```

---

## API Design

### Public API

```rust
// src/platform/dialogs.rs

/// Open a native dialog to select a file or folder.
///
/// On macOS, shows a single dialog that allows selecting either.
/// On Windows/Linux, requires a pre-selection (via `kind` parameter or UI prompt).
pub fn open_file_or_folder(options: OpenDialogOptions) -> Option<OpenTarget> {
    #[cfg(target_os = "macos")]
    {
        macos::open_file_or_folder_native(&options)
    }

    #[cfg(not(target_os = "macos"))]
    {
        open_file_or_folder_rfd(&options)
    }
}

/// Open a file dialog (cross-platform, uses rfd)
pub fn open_file(options: OpenDialogOptions) -> Option<PathBuf> {
    let mut dialog = rfd::FileDialog::new();
    
    if let Some(title) = &options.title {
        dialog = dialog.set_title(title);
    }
    if let Some(path) = &options.default_path {
        dialog = dialog.set_directory(path);
    }
    if let Some(parent) = options.parent {
        dialog = dialog.set_parent(parent);
    }
    
    dialog.pick_file()
}

/// Open a folder dialog (cross-platform, uses rfd)
pub fn open_folder(options: OpenDialogOptions) -> Option<PathBuf> {
    let mut dialog = rfd::FileDialog::new();
    
    if let Some(title) = &options.title {
        dialog = dialog.set_title(title);
    }
    if let Some(path) = &options.default_path {
        dialog = dialog.set_directory(path);
    }
    if let Some(parent) = options.parent {
        dialog = dialog.set_parent(parent);
    }
    
    dialog.pick_folder()
}
```

### macOS Implementation

```rust
// src/platform/macos/dialogs.rs

#[cfg(target_os = "macos")]
pub fn open_file_or_folder_native(options: &OpenDialogOptions) -> Option<OpenTarget> {
    use objc2::rc::Id;
    use objc2::runtime::Bool;
    use objc2_app_kit::NSOpenPanel;
    use objc2_foundation::{NSString, NSURL};

    unsafe {
        let panel = NSOpenPanel::openPanel();
        
        // Allow both files and directories
        panel.setCanChooseFiles(Bool::YES);
        panel.setCanChooseDirectories(Bool::YES);
        panel.setAllowsMultipleSelection(Bool::NO);
        panel.setResolvesAliases(Bool::YES);
        
        // Set title if provided
        if let Some(title) = &options.title {
            let ns_title = NSString::from_str(title);
            panel.setTitle(&ns_title);
        }
        
        // Set default directory if provided
        if let Some(path) = &options.default_path {
            if let Some(path_str) = path.to_str() {
                let ns_path = NSString::from_str(path_str);
                let url = NSURL::fileURLWithPath(&ns_path);
                panel.setDirectoryURL(Some(&url));
            }
        }
        
        // Run modal
        let response = panel.runModal();
        
        // NSModalResponseOK = 1
        if response.0 != 1 {
            return None;
        }
        
        // Get selected URL
        let url = panel.URL()?;
        let path_str = url.path()?.to_string();
        let path = PathBuf::from(path_str);
        
        // Determine if it's a file or folder
        let metadata = std::fs::metadata(&path).ok()?;
        if metadata.is_dir() {
            Some(OpenTarget::Folder(path))
        } else {
            Some(OpenTarget::File(path))
        }
    }
}
```

### Cross-Platform Fallback (Windows/Linux)

```rust
// src/platform/dialogs.rs

#[cfg(not(target_os = "macos"))]
fn open_file_or_folder_rfd(options: &OpenDialogOptions) -> Option<OpenTarget> {
    // This would be called after the user has chosen File vs Folder
    // via menu items or an in-app prompt.
    //
    // For now, default to file dialog.
    // The actual implementation will integrate with the UI prompt system.
    
    open_file(options.clone()).map(OpenTarget::File)
}

/// Open with explicit kind selection (for Windows/Linux)
pub fn open_with_kind(kind: OpenKind, options: OpenDialogOptions) -> Option<OpenTarget> {
    match kind {
        OpenKind::File => open_file(options).map(OpenTarget::File),
        OpenKind::Folder => open_folder(options).map(OpenTarget::Folder),
    }
}
```

---

## Message Integration

```rust
// src/messages.rs

pub enum DocumentMsg {
    // ... existing variants ...
    
    /// Open file or folder (platform-aware)
    OpenFileOrFolder,
    
    /// Open file specifically
    OpenFile,
    
    /// Open folder specifically  
    OpenFolder,
    
    /// Handle result from open dialog
    HandleOpenResult(Option<OpenTarget>),
}
```

```rust
// src/update/document.rs

fn handle_document_msg(model: &mut AppModel, msg: DocumentMsg) -> Cmd {
    match msg {
        DocumentMsg::OpenFileOrFolder => {
            #[cfg(target_os = "macos")]
            {
                // macOS: directly open combined dialog
                let options = OpenDialogOptions {
                    parent: model.window_handle(),
                    ..Default::default()
                };
                if let Some(target) = open_file_or_folder(options) {
                    return handle_open_target(model, target);
                }
            }
            
            #[cfg(not(target_os = "macos"))]
            {
                // Windows/Linux: show in-app prompt first
                model.show_open_kind_prompt();
            }
            
            Cmd::none()
        }
        
        DocumentMsg::HandleOpenResult(Some(target)) => {
            handle_open_target(model, target)
        }
        
        _ => Cmd::none(),
    }
}

fn handle_open_target(model: &mut AppModel, target: OpenTarget) -> Cmd {
    match target {
        OpenTarget::File(path) => {
            // Open file in editor
            Cmd::LoadFile(path)
        }
        OpenTarget::Folder(path) => {
            // Open as workspace
            Cmd::OpenWorkspace(path)
        }
    }
}
```

---

## Keybindings

| Action | Mac | Windows/Linux | Notes |
|--------|-----|---------------|-------|
| Open (unified) | Cmd+O | - | macOS only, opens combined dialog |
| Open File | - | Ctrl+O | Windows/Linux |
| Open Folder | Shift+Cmd+O | Ctrl+Shift+O | |

---

## Implementation Plan

### Phase 1: Core API & Types

**Files:** `src/platform/mod.rs`, `src/platform/dialogs.rs`

- [ ] Create `OpenTarget` enum
- [ ] Create `OpenDialogOptions` struct
- [ ] Implement `open_file()` using rfd
- [ ] Implement `open_folder()` using rfd
- [ ] Add platform module structure

**Test:** `open_file()` and `open_folder()` work on all platforms.

### Phase 2: macOS NSOpenPanel

**Files:** `src/platform/macos/dialogs.rs`

- [ ] Add `objc2` and `objc2-app-kit` dependencies (macOS only)
- [ ] Implement `open_file_or_folder_native()`
- [ ] Handle window parenting with raw-window-handle
- [ ] Test file vs folder detection

**Test:** On macOS, single dialog allows selecting either file or folder.

### Phase 3: Message Integration

**Files:** `src/messages.rs`, `src/update/document.rs`

- [ ] Add `DocumentMsg::OpenFileOrFolder` variant
- [ ] Add `DocumentMsg::HandleOpenResult` variant
- [ ] Implement message handlers
- [ ] Wire up to menu/keybindings

**Test:** Cmd+O on macOS opens combined dialog and loads result.

### Phase 4: Windows/Linux Fallback

**Files:** `src/platform/dialogs.rs`, `src/model/ui.rs`

- [ ] Add `OpenKind` enum
- [ ] Create in-app "Open File or Folder?" prompt modal
- [ ] Implement `open_with_kind()` function
- [ ] Wire up prompt result to rfd dialogs

**Test:** On Windows/Linux, user is prompted then shown appropriate dialog.

### Phase 5: Menu Integration

**Files:** Menu/command system

- [ ] macOS: Single "Open..." menu item
- [ ] Windows/Linux: "Open File..." and "Open Folder..." menu items
- [ ] Command palette entries for all variants

**Test:** Menu items work correctly per platform.

---

## Testing Strategy

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_open_target_path() {
        let file = OpenTarget::File(PathBuf::from("/test/file.txt"));
        let folder = OpenTarget::Folder(PathBuf::from("/test/folder"));
        
        assert_eq!(file.path(), &PathBuf::from("/test/file.txt"));
        assert_eq!(folder.path(), &PathBuf::from("/test/folder"));
    }

    #[test]
    fn test_open_target_type_checks() {
        let file = OpenTarget::File(PathBuf::from("/test/file.txt"));
        let folder = OpenTarget::Folder(PathBuf::from("/test/folder"));
        
        assert!(file.is_file());
        assert!(!file.is_folder());
        assert!(folder.is_folder());
        assert!(!folder.is_file());
    }
}
```

### Manual Testing Checklist

- [ ] macOS: Cmd+O opens dialog, can select file, file opens in editor
- [ ] macOS: Cmd+O opens dialog, can select folder, folder opens as workspace
- [ ] Windows: Ctrl+O shows prompt, selecting "File" opens file dialog
- [ ] Windows: Ctrl+O shows prompt, selecting "Folder" opens folder dialog
- [ ] Linux: Same as Windows
- [ ] Dialog is modal to main window (doesn't get lost behind)
- [ ] Cancel button works on all platforms
- [ ] Default directory is respected if set

---

## Dependencies

### New Dependencies (macOS only)

```toml
[target.'cfg(target_os = "macos")'.dependencies]
objc2 = "0.5"
objc2-foundation = "0.2"
objc2-app-kit = "0.2"
```

### Existing Dependencies

- `rfd` - Already in use for file dialogs
- `raw-window-handle` - Already in use for window integration

---

## References

- **rfd crate:** https://docs.rs/rfd - Current file dialog solution
- **NSOpenPanel:** https://developer.apple.com/documentation/appkit/nsopenpanel
- **objc2 crate:** https://docs.rs/objc2 - Safe Rust bindings for Objective-C
- **VS Code behavior:** Separate "Open File" and "Open Folder" commands
- **Zed implementation:** Uses GPUI's `PathPromptOptions { files: true, directories: true }`
