# File Change Detection

Detect and respond to external file modifications

> **Status:** Planned
> **Priority:** P2
> **Effort:** M
> **Created:** 2025-12-19
> **Milestone:** 3 - File Lifecycle

---

## Table of Contents

1. [Overview](#overview)
2. [Architecture](#architecture)
3. [Data Structures](#data-structures)
4. [User Experience](#user-experience)
5. [Implementation Plan](#implementation-plan)
6. [Testing Strategy](#testing-strategy)
7. [References](#references)

---

## Overview

### Current State

The editor currently:
- Uses `notify` crate for workspace file tree watching (`src/fs_watcher.rs`)
- Refreshes file tree on external changes
- Does NOT detect changes to open documents
- Does NOT warn before overwriting externally modified files

### Goals

1. **Detect external modifications**: Watch open files for changes made outside the editor
2. **User prompt on conflict**: "File changed on disk. Reload / Keep Mine / Compare"
3. **Automatic reload option**: Configurable silent reload for unmodified buffers
4. **Deleted file handling**: Warn when an open file is deleted externally
5. **Integration with auto-save**: Prevent auto-save from overwriting external changes

### Non-Goals

- Full 3-way merge conflict resolution
- Git-aware conflict detection (separate LSP feature)
- Real-time collaborative editing
- Network file system polling (rely on `notify` events)

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              Main Thread                                     │
│                                                                              │
│  ┌──────────────────────────────────────────────────────────────────────┐   │
│  │                          AppModel                                     │   │
│  │  ┌─────────────────────────────────────────────────────────────────┐ │   │
│  │  │ documents: HashMap<DocumentId, Document>                        │ │   │
│  │  │   └── file_state: Option<FileState>  ◄─── NEW                   │ │   │
│  │  │         ├── last_mtime: SystemTime                              │ │   │
│  │  │         ├── last_size: u64                                      │ │   │
│  │  │         └── external_change: Option<ExternalChange>             │ │   │
│  │  └─────────────────────────────────────────────────────────────────┘ │   │
│  └──────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
│  ┌──────────────────────┐      ┌──────────────────────────────────────┐     │
│  │ Msg::FileChange(     │      │          update_file_change()        │     │
│  │   FileChangeMsg::    │─────▶│                                      │     │
│  │   ExternalModified   │      │  - Update document.file_state        │     │
│  │ )                    │      │  - Show conflict modal if buffer     │     │
│  └──────────────────────┘      │    is modified                       │     │
│          ▲                     │  - Auto-reload if buffer is clean    │     │
│          │                     └──────────────────────────────────────┘     │
│          │                                                                   │
└──────────┼───────────────────────────────────────────────────────────────────┘
           │
           │ mpsc channel
           │
┌──────────┴───────────────────────────────────────────────────────────────────┐
│                        Document Watcher Thread                               │
│                                                                              │
│  ┌──────────────────────────────────────────────────────────────────────┐   │
│  │  DocumentWatcher                                                      │   │
│  │    ├── watcher: RecommendedWatcher                                    │   │
│  │    ├── watched_paths: HashMap<PathBuf, DocumentId>                    │   │
│  │    └── debouncer: Debouncer<Duration>                                 │   │
│  │                                                                       │   │
│  │  Events:                                                              │   │
│  │    notify::EventKind::Modify(_) ──▶ Check mtime ──▶ Send message     │   │
│  │    notify::EventKind::Remove(_) ──▶ Mark as deleted ──▶ Send message │   │
│  │    notify::EventKind::Create(_) ──▶ Recreated after delete           │   │
│  └──────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
└──────────────────────────────────────────────────────────────────────────────┘
```

### Module Structure

```
src/
├── file_watcher.rs           # NEW: DocumentWatcher (separate from workspace watcher)
├── model/
│   └── document.rs           # Add FileState field
├── messages.rs               # Add FileChangeMsg
├── update/
│   ├── mod.rs                # Route FileChangeMsg
│   └── file_change.rs        # NEW: update_file_change()
└── app.rs                    # Create DocumentWatcher, handle events
```

### Relationship to Existing Watcher

The workspace file tree uses `notify-debouncer-mini` in `src/fs_watcher.rs`. This watches directories for tree updates. Document watching is different:

| Aspect | Workspace Watcher | Document Watcher |
|--------|-------------------|------------------|
| Scope | Entire workspace directory | Only open files with paths |
| Purpose | Update file tree UI | Detect conflicts |
| Debounce | 500ms (coarse) | 100ms (responsive) |
| Events | Create/Delete for tree | Modify/Delete for prompts |

Both can coexist. For simplicity, we could extend the existing watcher, but separate concerns are cleaner.

---

## Data Structures

### FileState

```rust
// In src/model/document.rs

use std::time::SystemTime;

/// External file state tracking for conflict detection
#[derive(Debug, Clone)]
pub struct FileState {
    /// Last known modification time from disk
    pub last_mtime: SystemTime,

    /// Last known file size in bytes
    pub last_size: u64,

    /// Pending external change (set by watcher, cleared by user action)
    pub external_change: Option<ExternalChange>,

    /// Whether the file exists on disk (for delete detection)
    pub exists_on_disk: bool,
}

/// Type of external change detected
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalChange {
    /// File was modified by another process
    Modified,

    /// File was deleted
    Deleted,

    /// File was recreated after being deleted
    Recreated,
}

impl FileState {
    /// Create from current file metadata
    pub fn from_path(path: &std::path::Path) -> std::io::Result<Self> {
        let metadata = std::fs::metadata(path)?;
        Ok(Self {
            last_mtime: metadata.modified()?,
            last_size: metadata.len(),
            external_change: None,
            exists_on_disk: true,
        })
    }

    /// Check if file has changed since last known state
    pub fn has_changed(&self, path: &std::path::Path) -> std::io::Result<bool> {
        let metadata = std::fs::metadata(path)?;
        let current_mtime = metadata.modified()?;
        let current_size = metadata.len();

        Ok(current_mtime != self.last_mtime || current_size != self.last_size)
    }

    /// Update from current file metadata
    pub fn refresh(&mut self, path: &std::path::Path) -> std::io::Result<()> {
        let metadata = std::fs::metadata(path)?;
        self.last_mtime = metadata.modified()?;
        self.last_size = metadata.len();
        self.external_change = None;
        self.exists_on_disk = true;
        Ok(())
    }
}

// Update Document struct
pub struct Document {
    // ... existing fields ...

    /// External file state (None for new/unsaved documents)
    pub file_state: Option<FileState>,
}
```

### DocumentWatcher

```rust
// In src/file_watcher.rs

use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use notify_debouncer_mini::{new_debouncer, DebouncedEvent, Debouncer};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

use crate::messages::{FileChangeMsg, Msg};
use crate::model::editor_area::DocumentId;

/// Watches open documents for external changes
pub struct DocumentWatcher {
    /// The underlying file watcher
    debouncer: Debouncer<RecommendedWatcher>,

    /// Map of watched paths to document IDs
    watched_paths: HashMap<PathBuf, DocumentId>,

    /// Receiver for debounced events (polled by main thread)
    event_rx: mpsc::Receiver<Result<Vec<DebouncedEvent>, notify::Error>>,
}

impl DocumentWatcher {
    /// Debounce duration for file change events
    const DEBOUNCE_MS: u64 = 100;

    pub fn new() -> Result<Self, notify::Error> {
        let (tx, rx) = mpsc::channel();

        let debouncer = new_debouncer(
            Duration::from_millis(Self::DEBOUNCE_MS),
            move |res| {
                let _ = tx.send(res);
            },
        )?;

        Ok(Self {
            debouncer,
            watched_paths: HashMap::new(),
            event_rx: rx,
        })
    }

    /// Start watching a document's file
    pub fn watch_document(
        &mut self,
        doc_id: DocumentId,
        path: PathBuf,
    ) -> Result<(), notify::Error> {
        // Don't re-watch if already watching
        if self.watched_paths.contains_key(&path) {
            return Ok(());
        }

        self.debouncer
            .watcher()
            .watch(&path, RecursiveMode::NonRecursive)?;

        self.watched_paths.insert(path, doc_id);
        Ok(())
    }

    /// Stop watching a document's file
    pub fn unwatch_document(&mut self, path: &PathBuf) {
        if self.watched_paths.remove(path).is_some() {
            let _ = self.debouncer.watcher().unwatch(path);
        }
    }

    /// Stop watching all files for a document (by document ID)
    pub fn unwatch_by_document_id(&mut self, doc_id: DocumentId) {
        let paths_to_remove: Vec<_> = self
            .watched_paths
            .iter()
            .filter(|(_, &id)| id == doc_id)
            .map(|(path, _)| path.clone())
            .collect();

        for path in paths_to_remove {
            let _ = self.debouncer.watcher().unwatch(&path);
            self.watched_paths.remove(&path);
        }
    }

    /// Poll for pending file change events (non-blocking)
    pub fn poll_events(&self) -> Vec<FileChangeMsg> {
        let mut messages = Vec::new();

        while let Ok(Ok(events)) = self.event_rx.try_recv() {
            for event in events {
                if let Some(&doc_id) = self.watched_paths.get(&event.path) {
                    let msg = self.event_to_message(doc_id, &event.path);
                    if let Some(m) = msg {
                        messages.push(m);
                    }
                }
            }
        }

        messages
    }

    /// Convert a notify event to a FileChangeMsg
    fn event_to_message(&self, doc_id: DocumentId, path: &PathBuf) -> Option<FileChangeMsg> {
        // Check if file still exists
        if !path.exists() {
            return Some(FileChangeMsg::ExternalDeleted {
                document_id: doc_id,
                path: path.clone(),
            });
        }

        // File exists - it was modified (or recreated)
        Some(FileChangeMsg::ExternalModified {
            document_id: doc_id,
            path: path.clone(),
        })
    }
}
```

### Messages

```rust
// In src/messages.rs

use std::path::PathBuf;
use crate::model::editor_area::DocumentId;

/// File change detection messages
#[derive(Debug, Clone)]
pub enum FileChangeMsg {
    /// File was modified externally
    ExternalModified {
        document_id: DocumentId,
        path: PathBuf,
    },

    /// File was deleted externally
    ExternalDeleted {
        document_id: DocumentId,
        path: PathBuf,
    },

    /// User chose to reload the file
    Reload { document_id: DocumentId },

    /// User chose to keep their local version
    KeepLocal { document_id: DocumentId },

    /// User chose to save (overwrite external changes)
    OverwriteExternal { document_id: DocumentId },

    /// User requested diff view
    ShowDiff { document_id: DocumentId },

    /// Dismiss the conflict notification (for deleted files)
    DismissDeleted { document_id: DocumentId },

    /// Save as new file (for deleted files)
    SaveAs { document_id: DocumentId },
}

// Add to Msg enum
pub enum Msg {
    // ... existing variants ...
    FileChange(FileChangeMsg),
}
```

---

## User Experience

### Conflict Modal (Modified File)

When a file is modified externally AND the buffer has local changes:

```
┌─────────────────────────────────────────────────────────────────┐
│  File Changed on Disk                                      [X]  │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  "main.rs" has been modified outside the editor.                │
│                                                                 │
│  Your local changes will be lost if you reload.                 │
│                                                                 │
│                                                                 │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐          │
│  │    Reload    │  │  Keep Mine   │  │   Compare    │          │
│  └──────────────┘  └──────────────┘  └──────────────┘          │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### Conflict Modal (Deleted File)

When an open file is deleted externally:

```
┌─────────────────────────────────────────────────────────────────┐
│  File Deleted                                              [X]  │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  "main.rs" has been deleted from disk.                          │
│                                                                 │
│  You can save your version to recreate the file.                │
│                                                                 │
│                                                                 │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐          │
│  │     Save     │  │   Save As    │  │    Close     │          │
│  └──────────────┘  └──────────────┘  └──────────────┘          │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### Automatic Reload (Clean Buffer)

If the buffer has no local changes, reload automatically:

```
Status bar flash:
┌────────────────────────────────────────────────────────────────────┐
│  main.rs                          │ Reloaded from disk │ Ln 42    │
└────────────────────────────────────────────────────────────────────┘
```

This behavior is configurable:

```yaml
# config.yaml
file_change_detection:
  # always_prompt: Always show modal, even for clean buffers
  # auto_reload: Reload clean buffers silently
  # ignore: Ignore external changes entirely
  mode: auto_reload
```

### Tab Indicator

Show indicator in tab when external change is pending:

```
┌─────────────────────────────────────────────────────────────┐
│  main.rs *  │  lib.rs !  │  mod.rs  │                       │
└─────────────────────────────────────────────────────────────┘
              ▲
              └── "!" indicates external change pending
```

---

## Implementation Plan

### Phase 1: FileState Tracking

**Estimated effort: 1-2 days**

1. [ ] Add `FileState` struct to `src/model/document.rs`
2. [ ] Add `file_state: Option<FileState>` to `Document`
3. [ ] Initialize `file_state` when loading from file
4. [ ] Update `file_state` after saving
5. [ ] Clear `file_state` for new/untitled documents

**Test:** Load file, verify FileState populated with correct mtime

### Phase 2: DocumentWatcher

**Estimated effort: 2-3 days**

1. [ ] Create `src/file_watcher.rs` with `DocumentWatcher`
2. [ ] Add `document_watcher: DocumentWatcher` to `App`
3. [ ] Call `watch_document()` when opening files
4. [ ] Call `unwatch_document()` when closing tabs
5. [ ] Poll events in `about_to_wait()` hook
6. [ ] Dispatch `Msg::FileChange(...)` for each event

**Test:** Open file, modify externally, verify message dispatched

### Phase 3: Update Handler

**Estimated effort: 2-3 days**

1. [ ] Create `src/update/file_change.rs`
2. [ ] Handle `ExternalModified`: set `document.file_state.external_change`
3. [ ] Handle `ExternalDeleted`: set change type, mark non-existent
4. [ ] If buffer is clean AND mode is auto_reload, trigger reload
5. [ ] If buffer is dirty, show conflict modal

**Test:** Modify file externally with dirty buffer, verify modal appears

### Phase 4: Conflict Modal

**Estimated effort: 2 days**

1. [ ] Add `ModalId::FileConflict` to modal system
2. [ ] Create modal UI with buttons (Reload / Keep / Compare)
3. [ ] Handle `Reload`: re-read file, update buffer, clear undo
4. [ ] Handle `KeepLocal`: dismiss modal, clear `external_change`
5. [ ] Handle `ShowDiff`: (defer to future diff feature)

**Test:** Complete flow from external change to user choice

### Phase 5: Auto-Save Integration

**Estimated effort: 1 day**

1. [ ] In auto-save, check `file_state.external_change` before saving
2. [ ] If external change pending, skip auto-save
3. [ ] Log/warn that auto-save was skipped due to conflict

**Test:** Enable auto-save, modify file externally, verify no overwrite

### Phase 6: Polish

**Estimated effort: 1-2 days**

1. [ ] Add tab indicator for pending external changes
2. [ ] Add configuration for behavior mode
3. [ ] Add status bar transient message for auto-reload
4. [ ] Handle edge cases (file moved, permission changed)

**Test:** Visual indicators work, config respected

---

## Testing Strategy

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_file_state_detects_change() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.txt");

        // Create file
        File::create(&path).unwrap().write_all(b"hello").unwrap();

        // Record state
        let state = FileState::from_path(&path).unwrap();

        // Modify file
        std::thread::sleep(std::time::Duration::from_millis(10));
        File::create(&path).unwrap().write_all(b"world").unwrap();

        // Should detect change
        assert!(state.has_changed(&path).unwrap());
    }

    #[test]
    fn test_file_state_no_change() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.txt");

        File::create(&path).unwrap().write_all(b"hello").unwrap();

        let state = FileState::from_path(&path).unwrap();

        // No modification
        assert!(!state.has_changed(&path).unwrap());
    }

    #[test]
    fn test_file_state_deleted() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.txt");

        File::create(&path).unwrap().write_all(b"hello").unwrap();
        let state = FileState::from_path(&path).unwrap();

        std::fs::remove_file(&path).unwrap();

        assert!(state.has_changed(&path).is_err());
    }
}
```

### Integration Tests

```rust
#[test]
fn test_external_modification_triggers_modal() {
    let mut app = test_app();
    let path = temp_file("test.rs", "fn main() {}");

    // Open file
    app.dispatch(Msg::Layout(LayoutMsg::OpenFileInNewTab(path.clone())));

    // Modify locally (make buffer dirty)
    app.dispatch(Msg::Document(DocumentMsg::InsertChar('x')));

    // Simulate external modification
    std::fs::write(&path, "fn main() { println!(\"changed\"); }").unwrap();

    // Trigger file check
    app.dispatch(Msg::FileChange(FileChangeMsg::ExternalModified {
        document_id: app.focused_document_id(),
        path: path.clone(),
    }));

    // Modal should be visible
    assert!(app.model.ui.modal_state.is_some());
}

#[test]
fn test_clean_buffer_auto_reloads() {
    let mut app = test_app_with_config(FileChangeConfig {
        mode: FileChangeMode::AutoReload,
    });

    let path = temp_file("test.rs", "original");
    app.dispatch(Msg::Layout(LayoutMsg::OpenFileInNewTab(path.clone())));

    // No local changes - buffer is clean

    // External modification
    std::fs::write(&path, "modified").unwrap();
    app.dispatch(Msg::FileChange(FileChangeMsg::ExternalModified {
        document_id: app.focused_document_id(),
        path: path.clone(),
    }));

    // Should auto-reload without modal
    assert!(app.model.ui.modal_state.is_none());
    assert_eq!(app.model.document().buffer.to_string(), "modified");
}
```

### Manual Testing Checklist

- [ ] Open file, edit externally, verify modal appears
- [ ] Open file (no local changes), edit externally, verify auto-reload
- [ ] Open file, delete externally, verify delete modal
- [ ] Reload from modal, verify buffer updated
- [ ] Keep Mine from modal, verify local changes preserved
- [ ] Tab shows indicator for pending external change
- [ ] Auto-save skips files with external changes
- [ ] Closing tab unwatches file (no orphaned watches)

---

## Edge Cases and Invariants

### Edge Cases

1. **Rapid changes**: Multiple external edits should coalesce (debouncing)
2. **Save during external change**: Save should warn or prompt
3. **File moved**: Treat as delete + create at new path
4. **Permission denied**: Handle read errors gracefully
5. **Symbolic links**: Watch the link, not the target
6. **Network files**: `notify` may not work; degrade gracefully

### Invariants

1. Only documents with `file_path = Some(_)` can have `FileState`
2. `file_state.external_change` cleared after user action (Reload/Keep/etc.)
3. Watcher must be synchronized with document open/close lifecycle
4. Debouncer prevents event flood from rapid saves

---

## Configuration

```rust
// In src/config.rs

/// File change detection configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct FileChangeConfig {
    /// How to handle external changes
    #[serde(default)]
    pub mode: FileChangeMode,

    /// Whether to check for changes when window gains focus
    #[serde(default = "default_true")]
    pub check_on_focus: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileChangeMode {
    /// Always prompt, even for clean buffers
    AlwaysPrompt,

    /// Auto-reload clean buffers, prompt for dirty
    #[default]
    AutoReload,

    /// Ignore external changes entirely
    Ignore,
}
```

```yaml
# config.yaml
file_change_detection:
  mode: auto_reload
  check_on_focus: true
```

---

## References

- VS Code file watcher: https://code.visualstudio.com/docs/editor/codebasics#_hot-exit
- notify crate: https://docs.rs/notify/latest/notify/
- notify-debouncer-mini: https://docs.rs/notify-debouncer-mini/latest/
- Existing workspace watcher: `src/fs_watcher.rs`
- Auto-save integration: `docs/feature/auto-save.md` (F-100)
