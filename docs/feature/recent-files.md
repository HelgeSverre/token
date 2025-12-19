# Recent Files

Persistent recent files list with quick access menu and integration with Quick Open.

> **Status:** Planned
> **Priority:** P1
> **Effort:** M
> **Created:** 2025-12-19
> **Milestone:** 1 - Navigation

---

## Table of Contents

1. [Overview](#overview)
2. [Architecture](#architecture)
3. [Data Structures](#data-structures)
4. [Keybindings](#keybindings)
5. [Implementation Plan](#implementation-plan)
6. [Testing Strategy](#testing-strategy)
7. [References](#references)

---

## Overview

### Current State

The editor has no memory of previously opened files. Each session starts fresh with only CLI-provided files. Users must navigate the file tree or use file dialog to reopen frequently used files.

### Goals

1. **Track recently opened files** - Remember files opened via any method
2. **Persistent storage** - Save list to `~/.config/token-editor/recent.json`
3. **Quick access menu** - Dedicated "Open Recent" submenu or modal
4. **Quick Open integration** - Show recent files first in Cmd+P results
5. **Workspace-aware** - Track both workspace-relative and absolute paths
6. **Cross-session** - Recent files persist across editor restarts

### Non-Goals

- Session restore (reopening all tabs from last session)
- File pinning/favorites (handled in command palette enhancement)
- Recent workspaces/folders (separate tracking)
- Undo close tab (separate feature)

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                        Recent Files System                                   │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ┌────────────────────────────────────────────────────────────────────────┐ │
│  │                          File Open Events                               │ │
│  │                                                                         │ │
│  │  ┌───────────┐  ┌───────────┐  ┌───────────┐  ┌───────────────────┐   │ │
│  │  │ CLI args  │  │  File     │  │  Quick    │  │  Drag & Drop      │   │ │
│  │  │           │  │  Dialog   │  │  Open     │  │                   │   │ │
│  │  └─────┬─────┘  └─────┬─────┘  └─────┬─────┘  └─────────┬─────────┘   │ │
│  │        │              │              │                  │             │ │
│  │        └──────────────┴──────────────┴──────────────────┘             │ │
│  │                              │                                         │ │
│  │                              ▼                                         │ │
│  │                   ┌────────────────────┐                               │ │
│  │                   │  RecentFiles.add() │                               │ │
│  │                   └──────────┬─────────┘                               │ │
│  │                              │                                         │ │
│  └──────────────────────────────┼─────────────────────────────────────────┘ │
│                                 │                                            │
│                                 ▼                                            │
│  ┌────────────────────────────────────────────────────────────────────────┐ │
│  │                         RecentFiles State                               │ │
│  │                                                                         │ │
│  │  ┌─────────────────────────────────────────────────────────────────┐   │ │
│  │  │  entries: Vec<RecentEntry>                                      │   │ │
│  │  │  ─────────────────────────────────────────────────────────────  │   │ │
│  │  │  [0] /Users/dev/project/src/main.rs    (2 mins ago)            │   │ │
│  │  │  [1] /Users/dev/project/Cargo.toml     (1 hour ago)            │   │ │
│  │  │  [2] /Users/dev/other/README.md        (yesterday)             │   │ │
│  │  │  ...                                                            │   │ │
│  │  └─────────────────────────────────────────────────────────────────┘   │ │
│  │                                                                         │ │
│  │  Capacity: 50 files    │    Auto-prune missing files                   │ │
│  │                                                                         │ │
│  └────────────────────────────────────────────────────────────────────────┘ │
│                                 │                                            │
│                                 ▼                                            │
│  ┌────────────────────────────────────────────────────────────────────────┐ │
│  │                    ~/.config/token-editor/recent.json                   │ │
│  │                                                                         │ │
│  │  {                                                                      │ │
│  │    "version": 1,                                                        │ │
│  │    "entries": [                                                         │ │
│  │      { "path": "/Users/dev/project/src/main.rs",                       │ │
│  │        "opened_at": 1702987654,                                         │ │
│  │        "workspace": "/Users/dev/project" },                             │ │
│  │      ...                                                                │ │
│  │    ]                                                                    │ │
│  │  }                                                                      │ │
│  └────────────────────────────────────────────────────────────────────────┘ │
│                                                                              │
└──────────────────────────────────────────────────────────────────────────────┘
```

### Integration Points

```
┌─────────────────────────────────────────────────────────────────┐
│                     Consumer Integrations                        │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌──────────────────┐    ┌──────────────────┐                   │
│  │   Quick Open     │    │   Recent Files   │                   │
│  │   (Cmd+P)        │    │   Modal (Cmd+E)  │                   │
│  │                  │    │                  │                   │
│  │  Shows recent    │    │  Dedicated list  │                   │
│  │  files first     │    │  with previews   │                   │
│  │  when empty      │    │                  │                   │
│  └────────┬─────────┘    └────────┬─────────┘                   │
│           │                       │                              │
│           └───────────┬───────────┘                              │
│                       │                                          │
│                       ▼                                          │
│           ┌──────────────────────┐                               │
│           │    RecentFiles       │                               │
│           │    .entries()        │                               │
│           └──────────────────────┘                               │
│                                                                  │
└──────────────────────────────────────────────────────────────────┘
```

---

## Data Structures

### Recent Entry

```rust
// src/recent_files.rs

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::SystemTime;

/// A single entry in the recent files list
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentEntry {
    /// Absolute path to the file
    pub path: PathBuf,
    /// Timestamp when last opened (Unix epoch seconds)
    pub opened_at: u64,
    /// Workspace root when file was opened (if any)
    /// Used to show relative paths in the same workspace
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace: Option<PathBuf>,
    /// Number of times file has been opened (for ranking)
    #[serde(default)]
    pub open_count: u32,
}

impl RecentEntry {
    /// Create a new entry for the current time
    pub fn new(path: PathBuf, workspace: Option<PathBuf>) -> Self {
        Self {
            path,
            opened_at: now_epoch_secs(),
            workspace,
            open_count: 1,
        }
    }

    /// Update entry for re-opening
    pub fn touch(&mut self) {
        self.opened_at = now_epoch_secs();
        self.open_count += 1;
    }

    /// Get display path (relative to workspace if available, otherwise filename)
    pub fn display_path(&self) -> String {
        if let Some(ws) = &self.workspace {
            if let Ok(relative) = self.path.strip_prefix(ws) {
                return relative.to_string_lossy().to_string();
            }
        }
        self.path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| self.path.to_string_lossy().to_string())
    }

    /// Get human-readable time since opened
    pub fn time_ago(&self) -> String {
        let now = now_epoch_secs();
        let diff = now.saturating_sub(self.opened_at);

        if diff < 60 {
            "just now".to_string()
        } else if diff < 3600 {
            let mins = diff / 60;
            format!("{} min{} ago", mins, if mins == 1 { "" } else { "s" })
        } else if diff < 86400 {
            let hours = diff / 3600;
            format!("{} hour{} ago", hours, if hours == 1 { "" } else { "s" })
        } else if diff < 604800 {
            let days = diff / 86400;
            format!("{} day{} ago", days, if days == 1 { "" } else { "s" })
        } else {
            let weeks = diff / 604800;
            format!("{} week{} ago", weeks, if weeks == 1 { "" } else { "s" })
        }
    }

    /// Check if file still exists
    pub fn exists(&self) -> bool {
        self.path.exists()
    }
}

fn now_epoch_secs() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
```

### Recent Files Manager

```rust
// src/recent_files.rs

use std::collections::HashSet;

/// Maximum number of entries to keep
const MAX_ENTRIES: usize = 50;

/// Persistent recent files list
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RecentFiles {
    /// Schema version for forward compatibility
    #[serde(default)]
    pub version: u32,
    /// Recent file entries, most recent first
    pub entries: Vec<RecentEntry>,
}

impl RecentFiles {
    pub const CURRENT_VERSION: u32 = 1;

    /// Load recent files from disk
    pub fn load() -> Self {
        let path = crate::config_paths::recent_files_path();
        match std::fs::read_to_string(&path) {
            Ok(contents) => {
                let mut recent: Self = serde_json::from_str(&contents).unwrap_or_default();
                recent.prune_missing();
                recent
            }
            Err(_) => Self::default(),
        }
    }

    /// Save recent files to disk
    pub fn save(&self) -> std::io::Result<()> {
        let path = crate::config_paths::recent_files_path();
        crate::config_paths::ensure_all_config_dirs();
        let contents = serde_json::to_string_pretty(self)?;
        std::fs::write(path, contents)
    }

    /// Add a file to recent list (or update if already present)
    pub fn add(&mut self, path: PathBuf, workspace: Option<PathBuf>) {
        // Canonicalize path for consistent matching
        let canonical = path.canonicalize().unwrap_or(path);

        // Check if already in list
        if let Some(idx) = self.find_index(&canonical) {
            // Update existing entry and move to front
            self.entries[idx].touch();
            if let Some(ws) = workspace {
                self.entries[idx].workspace = Some(ws);
            }
            let entry = self.entries.remove(idx);
            self.entries.insert(0, entry);
        } else {
            // Add new entry at front
            let entry = RecentEntry::new(canonical, workspace);
            self.entries.insert(0, entry);
        }

        // Enforce capacity limit
        self.entries.truncate(MAX_ENTRIES);
    }

    /// Remove a file from recent list
    pub fn remove(&mut self, path: &PathBuf) {
        let canonical = path.canonicalize().unwrap_or_else(|_| path.clone());
        self.entries.retain(|e| e.path != canonical);
    }

    /// Clear all recent files
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Get recent files for a specific workspace
    pub fn for_workspace(&self, workspace: &PathBuf) -> Vec<&RecentEntry> {
        self.entries
            .iter()
            .filter(|e| {
                e.workspace.as_ref() == Some(workspace)
                    || e.path.starts_with(workspace)
            })
            .collect()
    }

    /// Get entries as paths (for Quick Open integration)
    pub fn paths(&self) -> Vec<PathBuf> {
        self.entries.iter().map(|e| e.path.clone()).collect()
    }

    /// Prune entries for files that no longer exist
    pub fn prune_missing(&mut self) {
        let original_len = self.entries.len();
        self.entries.retain(|e| e.exists());
        if self.entries.len() != original_len {
            tracing::debug!(
                "Pruned {} missing files from recent list",
                original_len - self.entries.len()
            );
        }
    }

    /// Find index of entry by path
    fn find_index(&self, path: &PathBuf) -> Option<usize> {
        self.entries.iter().position(|e| &e.path == path)
    }
}
```

### Config Paths Extension

```rust
// Add to src/config_paths.rs

/// Path to recent files list
pub fn recent_files_path() -> PathBuf {
    config_dir().join("recent.json")
}
```

### AppModel Integration

```rust
// Updates to src/model/mod.rs

pub struct AppModel {
    // ... existing fields ...

    /// Recent files list
    pub recent_files: RecentFiles,
}

impl AppModel {
    pub fn new(...) -> Self {
        // Load recent files on startup
        let recent_files = RecentFiles::load();

        Self {
            // ... existing fields ...
            recent_files,
        }
    }

    /// Record that a file was opened
    pub fn record_file_opened(&mut self, path: PathBuf) {
        let workspace = self.workspace.as_ref().map(|ws| ws.root.clone());
        self.recent_files.add(path, workspace);

        // Save asynchronously (fire and forget)
        let recent = self.recent_files.clone();
        std::thread::spawn(move || {
            if let Err(e) = recent.save() {
                tracing::warn!("Failed to save recent files: {}", e);
            }
        });
    }
}
```

### Recent Files Modal State

```rust
// Add to src/model/ui.rs

/// State for the recent files modal
#[derive(Debug, Clone)]
pub struct RecentFilesState {
    /// Index of selected file
    pub selected_index: usize,
    /// Cached entries for display
    pub entries: Vec<RecentEntry>,
    /// Filter query (optional)
    pub filter: String,
}

impl RecentFilesState {
    pub fn new(recent: &RecentFiles) -> Self {
        Self {
            selected_index: 0,
            entries: recent.entries.clone(),
            filter: String::new(),
        }
    }

    /// Get filtered entries
    pub fn filtered_entries(&self) -> Vec<&RecentEntry> {
        if self.filter.is_empty() {
            self.entries.iter().collect()
        } else {
            let filter_lower = self.filter.to_lowercase();
            self.entries
                .iter()
                .filter(|e| e.display_path().to_lowercase().contains(&filter_lower))
                .collect()
        }
    }

    /// Get selected file path
    pub fn selected_path(&self) -> Option<&PathBuf> {
        let filtered = self.filtered_entries();
        filtered.get(self.selected_index).map(|e| &e.path)
    }
}

/// Add to ModalId
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModalId {
    // ... existing variants ...
    RecentFiles, // NEW
}

/// Add to ModalState
pub enum ModalState {
    // ... existing variants ...
    RecentFiles(RecentFilesState), // NEW
}
```

---

## Keybindings

| Action | Mac | Windows/Linux | Notes |
|--------|-----|---------------|-------|
| Open Recent Files | Cmd+E | Ctrl+E | Dedicated recent modal |
| Navigate up | Up Arrow | Up Arrow | Select previous |
| Navigate down | Down Arrow | Down Arrow | Select next |
| Open file | Enter | Enter | Open selected file |
| Open in split | Cmd+Enter | Ctrl+Enter | Open in new split |
| Remove from list | Delete | Delete | Remove selected entry |
| Clear all | Cmd+Shift+Delete | Ctrl+Shift+Delete | Clear entire list |
| Close | Escape | Escape | Close modal |

Note: Cmd+P (Quick Open) also shows recent files when query is empty.

---

## Implementation Plan

### Phase 1: Core Data Structures

**Files:** `src/recent_files.rs`, `src/config_paths.rs`

- [ ] Create `RecentEntry` struct with serialization
- [ ] Create `RecentFiles` struct with add/remove/clear methods
- [ ] Implement `load()` and `save()` to config directory
- [ ] Add `recent_files_path()` to config paths
- [ ] Add `prune_missing()` for cleanup

**Test:** Save and load recent files round-trips correctly.

### Phase 2: AppModel Integration

**Files:** `src/model/mod.rs`

- [ ] Add `recent_files` field to `AppModel`
- [ ] Load recent files in `AppModel::new()`
- [ ] Add `record_file_opened()` method
- [ ] Save asynchronously after updates

**Test:** Opening a file adds it to recent list.

### Phase 3: File Open Hooks

**Files:** `src/update/app.rs`, `src/update/layout.rs`

- [ ] Hook `AppMsg::FileLoaded` to record file
- [ ] Hook `LayoutMsg::OpenFileInNewTab` to record file
- [ ] Hook drag-and-drop file open
- [ ] Hook CLI file arguments

**Test:** Files opened via any method appear in recent list.

### Phase 4: Recent Files Modal

**Files:** `src/model/ui.rs`, `src/update/modal.rs`

- [ ] Add `RecentFilesState` struct
- [ ] Add `ModalId::RecentFiles` variant
- [ ] Handle modal open with Cmd+E
- [ ] Implement navigation and selection
- [ ] Implement remove entry (Delete key)

**Test:** Cmd+E opens recent files list.

### Phase 5: Rendering

**Files:** `src/view/modal.rs`

- [ ] Render recent files modal
- [ ] Show file path and time ago
- [ ] Highlight selected entry
- [ ] Show "exists" indicator (dim missing files)
- [ ] Optional filter input

**Test:** Modal shows recent files with relative paths.

### Phase 6: Quick Open Integration

**Files:** `src/model/ui.rs` (QuickOpenState), `src/file_index.rs`

- [ ] Pass recent files to `QuickOpenState::update_results()`
- [ ] Prioritize recent files in empty query results
- [ ] Show recent indicator (star) in results
- [ ] Boost scores for recent files in fuzzy matching

**Test:** Quick Open shows recent files first when query empty.

### Phase 7: Polish

- [ ] Add "Clear Recent" command to command palette
- [ ] Add workspace-scoped recent files view
- [ ] Handle concurrent access (file locking or ignore errors)
- [ ] Prune files older than 30 days
- [ ] Show file preview on hover (optional)

---

## Testing Strategy

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_add_and_retrieve() {
        let mut recent = RecentFiles::default();
        let path = PathBuf::from("/test/file.rs");

        recent.add(path.clone(), None);

        assert_eq!(recent.entries.len(), 1);
        assert_eq!(recent.entries[0].path, path);
    }

    #[test]
    fn test_mru_ordering() {
        let mut recent = RecentFiles::default();

        recent.add(PathBuf::from("/first.rs"), None);
        std::thread::sleep(std::time::Duration::from_millis(10));
        recent.add(PathBuf::from("/second.rs"), None);

        // Most recent first
        assert_eq!(recent.entries[0].path, PathBuf::from("/second.rs"));
        assert_eq!(recent.entries[1].path, PathBuf::from("/first.rs"));
    }

    #[test]
    fn test_reopening_moves_to_front() {
        let mut recent = RecentFiles::default();

        recent.add(PathBuf::from("/first.rs"), None);
        recent.add(PathBuf::from("/second.rs"), None);
        recent.add(PathBuf::from("/first.rs"), None); // Reopen first

        assert_eq!(recent.entries[0].path, PathBuf::from("/first.rs"));
        assert_eq!(recent.entries.len(), 2); // No duplicate
    }

    #[test]
    fn test_capacity_limit() {
        let mut recent = RecentFiles::default();

        for i in 0..100 {
            recent.add(PathBuf::from(format!("/file{}.rs", i)), None);
        }

        assert_eq!(recent.entries.len(), MAX_ENTRIES);
    }

    #[test]
    fn test_time_ago() {
        let entry = RecentEntry::new(PathBuf::from("/test.rs"), None);
        assert_eq!(entry.time_ago(), "just now");
    }

    #[test]
    fn test_display_path_with_workspace() {
        let entry = RecentEntry {
            path: PathBuf::from("/project/src/main.rs"),
            opened_at: 0,
            workspace: Some(PathBuf::from("/project")),
            open_count: 1,
        };

        assert_eq!(entry.display_path(), "src/main.rs");
    }

    #[test]
    fn test_persistence() {
        let dir = tempdir().unwrap();
        // Set config dir to temp
        // Add files, save, load, verify
    }
}
```

### Integration Tests

```rust
// tests/recent_files_tests.rs

#[test]
fn test_recent_modal_flow() {
    // Open several files
    // Open recent files modal
    // Verify files appear in order
    // Select and open
    // Verify file opened
}

#[test]
fn test_quick_open_recent_priority() {
    // Open file A
    // Open Quick Open with empty query
    // Verify file A appears first
}

#[test]
fn test_remove_from_recent() {
    // Open file
    // Open recent modal
    // Press Delete on entry
    // Verify removed from list
}
```

---

## References

- **Config paths:** `src/config_paths.rs` - Configuration directory helpers
- **Quick Open:** F-020 feature for integration
- **Modal system:** `src/model/ui.rs` - Modal state patterns
- **VS Code:** "Open Recent" menu and Ctrl+R shortcut
- **Sublime Text:** Recent files in project switcher
- **IntelliJ:** Recent Files dialog (Cmd+E)
