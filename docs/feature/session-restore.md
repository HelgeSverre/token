# Session Restore

Persist and restore editor state across restarts

> **Status:** Planned
> **Priority:** P2
> **Effort:** L
> **Created:** 2025-12-19
> **Milestone:** 3 - File Lifecycle

---

## Table of Contents

1. [Overview](#overview)
2. [Architecture](#architecture)
3. [Data Structures](#data-structures)
4. [Session File Format](#session-file-format)
5. [Implementation Plan](#implementation-plan)
6. [Testing Strategy](#testing-strategy)
7. [References](#references)

---

## Overview

### Current State

The editor currently:
- Opens files specified on command line
- Has `EditorArea` with split views, tabs, and groups
- Tracks cursor position per editor (`EditorState`)
- Has workspace concept with sidebar state
- Does NOT persist any state between sessions

### Goals

1. **Persist open files**: Remember which files were open
2. **Restore split layout**: Recreate horizontal/vertical splits with ratios
3. **Restore cursor positions**: Return to exact line/column in each file
4. **Restore scroll positions**: Return to same viewport
5. **Restore selection state**: Preserve selections and multi-cursors
6. **Workspace-aware**: Separate sessions per workspace
7. **Graceful degradation**: Handle missing files, changed files, moved workspaces

### Non-Goals

- Undo history persistence (complex, large, fragile)
- Unsaved file content persistence ("hot exit" pattern - separate feature)
- Recent files list across sessions (simpler feature, could be part of this)
- Project-level settings (separate config feature)

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              Application Lifecycle                           │
│                                                                              │
│  ┌──────────────────────────────────────────────────────────────────────┐   │
│  │                         Startup                                       │   │
│  │                                                                       │   │
│  │   1. Parse CLI args                                                   │   │
│  │   2. Determine workspace root (if any)                                │   │
│  │   3. Load session file:                                               │   │
│  │        ~/.config/token-editor/sessions/<workspace-hash>.json          │   │
│  │      OR ~/.config/token-editor/sessions/default.json                  │   │
│  │   4. Validate session (files exist, etc.)                             │   │
│  │   5. Merge CLI args with session (CLI takes precedence)               │   │
│  │   6. Build AppModel with restored state                               │   │
│  └──────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
│  ┌──────────────────────────────────────────────────────────────────────┐   │
│  │                         Shutdown                                      │   │
│  │                                                                       │   │
│  │   1. Collect current session state from AppModel                      │   │
│  │   2. Serialize to SessionData                                         │   │
│  │   3. Write to session file (atomic write with temp file)              │   │
│  │   4. Handle write errors gracefully (log, don't crash)                │   │
│  └──────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
│  ┌──────────────────────────────────────────────────────────────────────┐   │
│  │                      Periodic Autosave (Optional)                     │   │
│  │                                                                       │   │
│  │   Every 60 seconds (configurable), snapshot session state             │   │
│  │   Protects against crashes losing session data                        │   │
│  └──────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Module Structure

```
src/
├── session.rs                # NEW: SessionData, load/save functions
├── cli.rs                    # Integrate session loading
├── main.rs                   # Call session restore
├── app.rs                    # Save session on close
└── model/
    ├── mod.rs                # AppModel::from_session()
    ├── editor_area.rs        # EditorArea::from_session()
    └── editor.rs             # EditorState::from_session()

~/.config/token-editor/
├── config.yaml               # User configuration
├── keymap.yaml               # Key bindings
└── sessions/                 # NEW: Session storage
    ├── default.json          # Session for no-workspace mode
    ├── a1b2c3d4.json         # Session for workspace (hashed path)
    └── ...
```

### Session Storage Location

```
Workspace mode:
  ~/.config/token-editor/sessions/<hash>.json
  where hash = blake3(canonical_workspace_path)[0..8]

No workspace mode:
  ~/.config/token-editor/sessions/default.json

Session filename example:
  /home/user/projects/my-app  →  sessions/a1b2c3d4.json
```

---

## Data Structures

### SessionData (Persisted)

```rust
// In src/session.rs

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Complete session state for persistence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionData {
    /// Session format version (for migration)
    pub version: u32,

    /// When this session was last saved
    pub timestamp: chrono::DateTime<chrono::Utc>,

    /// Workspace root (None for no-workspace mode)
    pub workspace_root: Option<PathBuf>,

    /// Editor layout
    pub layout: LayoutData,

    /// All open documents with their state
    pub documents: Vec<DocumentSessionData>,

    /// Sidebar state
    pub sidebar: Option<SidebarData>,

    /// Window geometry (optional, for multi-monitor support)
    pub window: Option<WindowData>,
}

/// Current session format version
pub const SESSION_VERSION: u32 = 1;

impl SessionData {
    pub fn new() -> Self {
        Self {
            version: SESSION_VERSION,
            timestamp: chrono::Utc::now(),
            workspace_root: None,
            layout: LayoutData::Single { group: GroupData::empty() },
            documents: Vec::new(),
            sidebar: None,
            window: None,
        }
    }
}

/// Editor layout tree (mirrors LayoutNode)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LayoutData {
    /// Single group (no splits)
    Single { group: GroupData },

    /// Split container with children
    Split {
        direction: SplitDirectionData,
        children: Vec<LayoutData>,
        ratios: Vec<f32>,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SplitDirectionData {
    Horizontal,
    Vertical,
}

/// Editor group (pane with tabs)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupData {
    /// Document indices (into SessionData.documents)
    pub tab_document_indices: Vec<usize>,

    /// Active tab index
    pub active_tab: usize,
}

impl GroupData {
    pub fn empty() -> Self {
        Self {
            tab_document_indices: Vec::new(),
            active_tab: 0,
        }
    }
}

/// Document state for session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentSessionData {
    /// File path (absolute or relative to workspace)
    pub path: PathBuf,

    /// Whether path is relative to workspace root
    pub path_is_relative: bool,

    /// Editor state for this document
    pub editor_state: EditorSessionData,

    /// Whether document had unsaved changes (warning on restore)
    pub was_modified: bool,
}

/// Editor state for session (view-specific)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorSessionData {
    /// Primary cursor position
    pub cursor: CursorData,

    /// Additional cursors (for multi-cursor)
    pub additional_cursors: Vec<CursorData>,

    /// Selection state (parallel to cursors)
    pub selections: Vec<SelectionData>,

    /// Viewport scroll position
    pub viewport: ViewportData,

    /// View mode (text/csv)
    pub view_mode: ViewModeData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CursorData {
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectionData {
    pub anchor_line: usize,
    pub anchor_column: usize,
    pub head_line: usize,
    pub head_column: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewportData {
    pub top_line: usize,
    pub left_column: usize,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewModeData {
    Text,
    Csv,
}

/// Sidebar state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SidebarData {
    /// Whether sidebar is visible
    pub visible: bool,

    /// Width in logical pixels
    pub width: f32,

    /// Expanded folder paths (relative to workspace)
    pub expanded_folders: Vec<PathBuf>,

    /// Selected item path (relative to workspace)
    pub selected_item: Option<PathBuf>,

    /// Scroll position
    pub scroll_offset: usize,
}

/// Window geometry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowData {
    /// Window position (if available)
    pub position: Option<(i32, i32)>,

    /// Window size
    pub size: (u32, u32),

    /// Whether window was maximized
    pub maximized: bool,
}
```

### Session Manager

```rust
// In src/session.rs

use std::fs;
use std::path::PathBuf;

use crate::config_paths;

/// Manages session loading and saving
pub struct SessionManager {
    /// Path to the session file
    session_path: PathBuf,

    /// Last saved session (for dirty checking)
    last_saved: Option<SessionData>,
}

impl SessionManager {
    /// Create session manager for a workspace
    pub fn for_workspace(workspace_root: Option<&PathBuf>) -> Self {
        let session_path = Self::session_file_path(workspace_root);
        Self {
            session_path,
            last_saved: None,
        }
    }

    /// Get the session file path for a workspace
    fn session_file_path(workspace_root: Option<&PathBuf>) -> PathBuf {
        let sessions_dir = config_paths::sessions_dir();

        match workspace_root {
            Some(root) => {
                let hash = Self::hash_path(root);
                sessions_dir.join(format!("{}.json", hash))
            }
            None => sessions_dir.join("default.json"),
        }
    }

    /// Hash a path for use as filename
    fn hash_path(path: &PathBuf) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let canonical = path.canonicalize().unwrap_or_else(|_| path.clone());
        let mut hasher = DefaultHasher::new();
        canonical.hash(&mut hasher);
        format!("{:016x}", hasher.finish())[..8].to_string()
    }

    /// Load session from disk
    pub fn load(&mut self) -> Result<SessionData, SessionLoadError> {
        if !self.session_path.exists() {
            return Err(SessionLoadError::NotFound);
        }

        let content = fs::read_to_string(&self.session_path)
            .map_err(SessionLoadError::Io)?;

        let session: SessionData = serde_json::from_str(&content)
            .map_err(SessionLoadError::Parse)?;

        // Check version compatibility
        if session.version > SESSION_VERSION {
            return Err(SessionLoadError::FutureVersion {
                file_version: session.version,
                supported_version: SESSION_VERSION,
            });
        }

        // TODO: Migrate older versions if needed

        self.last_saved = Some(session.clone());
        Ok(session)
    }

    /// Save session to disk (atomic write)
    pub fn save(&mut self, session: &SessionData) -> Result<(), SessionSaveError> {
        // Ensure sessions directory exists
        let sessions_dir = self.session_path.parent()
            .ok_or_else(|| SessionSaveError::InvalidPath)?;

        fs::create_dir_all(sessions_dir)
            .map_err(SessionSaveError::Io)?;

        // Serialize to JSON
        let content = serde_json::to_string_pretty(session)
            .map_err(SessionSaveError::Serialize)?;

        // Atomic write via temp file
        let temp_path = self.session_path.with_extension("json.tmp");
        fs::write(&temp_path, &content)
            .map_err(SessionSaveError::Io)?;

        fs::rename(&temp_path, &self.session_path)
            .map_err(SessionSaveError::Io)?;

        self.last_saved = Some(session.clone());
        Ok(())
    }

    /// Check if session has changed since last save
    pub fn has_changes(&self, current: &SessionData) -> bool {
        match &self.last_saved {
            Some(saved) => {
                // Compare relevant fields (ignore timestamp)
                saved.documents.len() != current.documents.len()
                    || !matches!(&saved.layout, l if format!("{:?}", l) == format!("{:?}", &current.layout))
            }
            None => true,
        }
    }
}

#[derive(Debug)]
pub enum SessionLoadError {
    NotFound,
    Io(std::io::Error),
    Parse(serde_json::Error),
    FutureVersion { file_version: u32, supported_version: u32 },
}

#[derive(Debug)]
pub enum SessionSaveError {
    InvalidPath,
    Io(std::io::Error),
    Serialize(serde_json::Error),
}
```

### Conversion Traits

```rust
// In src/session.rs

use crate::model::{
    AppModel, EditorArea, EditorState, EditorGroup, Document,
    Cursor, Selection, Position, Viewport, LayoutNode, SplitDirection,
};

impl SessionData {
    /// Create session data from current app state
    pub fn from_app_model(model: &AppModel) -> Self {
        let mut session = SessionData::new();

        session.workspace_root = model.workspace.as_ref().map(|w| w.root.clone());

        // Collect all documents with their paths
        let mut doc_index_map = std::collections::HashMap::new();
        for (doc_id, doc) in &model.editor_area.documents {
            if let Some(path) = &doc.file_path {
                let index = session.documents.len();
                doc_index_map.insert(*doc_id, index);

                // Find editor state for this document
                let editor_state = model.editor_area.editors.values()
                    .find(|e| e.document_id == Some(*doc_id))
                    .map(EditorSessionData::from_editor_state)
                    .unwrap_or_default();

                session.documents.push(DocumentSessionData {
                    path: Self::make_path_relative(path, &session.workspace_root),
                    path_is_relative: session.workspace_root.is_some(),
                    editor_state,
                    was_modified: doc.is_modified,
                });
            }
        }

        // Convert layout tree
        session.layout = LayoutData::from_layout_node(
            &model.editor_area.layout,
            &model.editor_area,
            &doc_index_map,
        );

        // Sidebar state
        if let Some(workspace) = &model.workspace {
            session.sidebar = Some(SidebarData {
                visible: workspace.sidebar_visible,
                width: workspace.sidebar_width_logical,
                expanded_folders: workspace.expanded_folders.iter().cloned().collect(),
                selected_item: workspace.selected_item.clone(),
                scroll_offset: workspace.scroll_offset,
            });
        }

        // Window state
        session.window = Some(WindowData {
            position: None, // TODO: Get from window
            size: model.window_size,
            maximized: false, // TODO: Get from window
        });

        session.timestamp = chrono::Utc::now();
        session
    }

    fn make_path_relative(path: &PathBuf, workspace: &Option<PathBuf>) -> PathBuf {
        if let Some(root) = workspace {
            if let Ok(relative) = path.strip_prefix(root) {
                return relative.to_path_buf();
            }
        }
        path.clone()
    }
}

impl EditorSessionData {
    fn from_editor_state(state: &EditorState) -> Self {
        Self {
            cursor: CursorData {
                line: state.active_cursor().line,
                column: state.active_cursor().column,
            },
            additional_cursors: state.cursors.iter()
                .skip(1)
                .map(|c| CursorData { line: c.line, column: c.column })
                .collect(),
            selections: state.selections.iter()
                .map(|s| SelectionData {
                    anchor_line: s.anchor.line,
                    anchor_column: s.anchor.column,
                    head_line: s.head.line,
                    head_column: s.head.column,
                })
                .collect(),
            viewport: ViewportData {
                top_line: state.viewport.top_line,
                left_column: state.viewport.left_column,
            },
            view_mode: if state.view_mode.is_csv() {
                ViewModeData::Csv
            } else {
                ViewModeData::Text
            },
        }
    }
}

impl Default for EditorSessionData {
    fn default() -> Self {
        Self {
            cursor: CursorData { line: 0, column: 0 },
            additional_cursors: Vec::new(),
            selections: Vec::new(),
            viewport: ViewportData { top_line: 0, left_column: 0 },
            view_mode: ViewModeData::Text,
        }
    }
}

impl LayoutData {
    fn from_layout_node(
        node: &LayoutNode,
        area: &EditorArea,
        doc_index_map: &std::collections::HashMap<crate::model::DocumentId, usize>,
    ) -> Self {
        match node {
            LayoutNode::Group(group_id) => {
                if let Some(group) = area.groups.get(group_id) {
                    LayoutData::Single {
                        group: GroupData::from_group(group, area, doc_index_map),
                    }
                } else {
                    LayoutData::Single { group: GroupData::empty() }
                }
            }
            LayoutNode::Split(container) => {
                LayoutData::Split {
                    direction: match container.direction {
                        SplitDirection::Horizontal => SplitDirectionData::Horizontal,
                        SplitDirection::Vertical => SplitDirectionData::Vertical,
                    },
                    children: container.children.iter()
                        .map(|child| Self::from_layout_node(child, area, doc_index_map))
                        .collect(),
                    ratios: container.ratios.clone(),
                }
            }
        }
    }
}

impl GroupData {
    fn from_group(
        group: &EditorGroup,
        area: &EditorArea,
        doc_index_map: &std::collections::HashMap<crate::model::DocumentId, usize>,
    ) -> Self {
        let tab_document_indices: Vec<usize> = group.tabs.iter()
            .filter_map(|tab| {
                let editor = area.editors.get(&tab.editor_id)?;
                let doc_id = editor.document_id?;
                doc_index_map.get(&doc_id).copied()
            })
            .collect();

        Self {
            tab_document_indices,
            active_tab: group.active_tab_index.min(tab_document_indices.len().saturating_sub(1)),
        }
    }
}
```

---

## Session File Format

### Example Session File

```json
{
  "version": 1,
  "timestamp": "2025-12-19T10:30:00Z",
  "workspace_root": "/home/user/projects/my-app",
  "layout": {
    "type": "split",
    "direction": "horizontal",
    "ratios": [0.5, 0.5],
    "children": [
      {
        "type": "single",
        "group": {
          "tab_document_indices": [0, 1],
          "active_tab": 0
        }
      },
      {
        "type": "single",
        "group": {
          "tab_document_indices": [2],
          "active_tab": 0
        }
      }
    ]
  },
  "documents": [
    {
      "path": "src/main.rs",
      "path_is_relative": true,
      "editor_state": {
        "cursor": { "line": 42, "column": 15 },
        "additional_cursors": [],
        "selections": [
          {
            "anchor_line": 42, "anchor_column": 15,
            "head_line": 42, "head_column": 15
          }
        ],
        "viewport": { "top_line": 30, "left_column": 0 },
        "view_mode": "text"
      },
      "was_modified": false
    },
    {
      "path": "src/lib.rs",
      "path_is_relative": true,
      "editor_state": {
        "cursor": { "line": 0, "column": 0 },
        "additional_cursors": [],
        "selections": [],
        "viewport": { "top_line": 0, "left_column": 0 },
        "view_mode": "text"
      },
      "was_modified": true
    },
    {
      "path": "README.md",
      "path_is_relative": true,
      "editor_state": {
        "cursor": { "line": 10, "column": 0 },
        "additional_cursors": [],
        "selections": [],
        "viewport": { "top_line": 0, "left_column": 0 },
        "view_mode": "text"
      },
      "was_modified": false
    }
  ],
  "sidebar": {
    "visible": true,
    "width": 250.0,
    "expanded_folders": ["src", "docs"],
    "selected_item": "src/main.rs",
    "scroll_offset": 0
  },
  "window": {
    "position": [100, 50],
    "size": [1200, 800],
    "maximized": false
  }
}
```

---

## Implementation Plan

### Phase 1: Session Infrastructure

**Estimated effort: 2-3 days**

1. [ ] Add `chrono` and `serde_json` dependencies
2. [ ] Create `src/session.rs` with data structures
3. [ ] Add `sessions_dir()` to `config_paths.rs`
4. [ ] Implement `SessionManager::load()` and `save()`
5. [ ] Add session file version handling

**Test:** Create/load/save session file manually

### Phase 2: Session Capture

**Estimated effort: 2 days**

1. [ ] Implement `SessionData::from_app_model()`
2. [ ] Implement `LayoutData::from_layout_node()`
3. [ ] Implement `GroupData::from_group()`
4. [ ] Implement `EditorSessionData::from_editor_state()`
5. [ ] Handle workspace-relative paths

**Test:** Capture session from running editor, verify JSON

### Phase 3: Session Restore

**Estimated effort: 3-4 days**

1. [ ] Add `AppModel::from_session()` constructor
2. [ ] Load documents from session paths (handle missing files)
3. [ ] Restore layout tree from `LayoutData`
4. [ ] Restore cursor/selection/viewport per editor
5. [ ] Restore sidebar state (expanded folders, selection)

**Test:** Save session, restart, verify layout matches

### Phase 4: CLI Integration

**Estimated effort: 1-2 days**

1. [ ] Add `--no-session` / `-n` flag to skip restore
2. [ ] In `main()`, determine workspace from args
3. [ ] Load session before creating `AppModel`
4. [ ] Merge CLI file args with session (CLI wins)
5. [ ] If session load fails, fall back to clean state

**Test:** Various CLI scenarios with/without session

### Phase 5: Shutdown Save

**Estimated effort: 1 day**

1. [ ] Add `session_manager` to `App`
2. [ ] In `App::exiting()`, call `session_manager.save()`
3. [ ] Handle save errors gracefully (log, don't crash)
4. [ ] Ensure atomic write (temp file + rename)

**Test:** Close editor, verify session file updated

### Phase 6: Validation and Recovery

**Estimated effort: 2 days**

1. [ ] Validate paths before restoring (skip missing files)
2. [ ] Warn about files that were modified externally
3. [ ] Warn about files that had unsaved changes
4. [ ] Add status bar message for session restore
5. [ ] Handle corrupted session files gracefully

**Test:** Delete file from session, restart, verify recovery

### Phase 7: Configuration

**Estimated effort: 1 day**

1. [ ] Add session config to `config.yaml`
2. [ ] Option to disable session restore
3. [ ] Option for periodic session auto-save
4. [ ] Option to limit session history

**Test:** Config options respected

---

## Configuration

```rust
// In src/config.rs

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct SessionConfig {
    /// Whether to restore sessions on startup
    #[serde(default = "default_true")]
    pub restore_session: bool,

    /// Whether to save session on close
    #[serde(default = "default_true")]
    pub save_session: bool,

    /// Auto-save session every N seconds (0 = disabled)
    #[serde(default)]
    pub auto_save_interval_secs: u64,

    /// Maximum number of session files to keep
    #[serde(default = "default_max_sessions")]
    pub max_sessions: usize,
}

fn default_max_sessions() -> usize {
    50
}
```

```yaml
# config.yaml
session:
  restore_session: true
  save_session: true
  auto_save_interval_secs: 60  # 0 to disable
  max_sessions: 50
```

---

## Testing Strategy

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_serialization_roundtrip() {
        let session = SessionData {
            version: SESSION_VERSION,
            timestamp: chrono::Utc::now(),
            workspace_root: Some(PathBuf::from("/home/user/project")),
            layout: LayoutData::Single { group: GroupData::empty() },
            documents: vec![
                DocumentSessionData {
                    path: PathBuf::from("src/main.rs"),
                    path_is_relative: true,
                    editor_state: EditorSessionData::default(),
                    was_modified: false,
                }
            ],
            sidebar: None,
            window: None,
        };

        let json = serde_json::to_string(&session).unwrap();
        let parsed: SessionData = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.version, SESSION_VERSION);
        assert_eq!(parsed.documents.len(), 1);
    }

    #[test]
    fn test_path_hashing_consistent() {
        let path = PathBuf::from("/home/user/project");
        let hash1 = SessionManager::hash_path(&path);
        let hash2 = SessionManager::hash_path(&path);
        assert_eq!(hash1, hash2);
        assert_eq!(hash1.len(), 8);
    }

    #[test]
    fn test_path_hashing_different() {
        let path1 = PathBuf::from("/home/user/project1");
        let path2 = PathBuf::from("/home/user/project2");
        let hash1 = SessionManager::hash_path(&path1);
        let hash2 = SessionManager::hash_path(&path2);
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_relative_path_conversion() {
        let workspace = Some(PathBuf::from("/home/user/project"));
        let file = PathBuf::from("/home/user/project/src/main.rs");

        let relative = SessionData::make_path_relative(&file, &workspace);
        assert_eq!(relative, PathBuf::from("src/main.rs"));
    }
}
```

### Integration Tests

```rust
#[test]
fn test_session_save_and_load() {
    let temp_dir = tempfile::tempdir().unwrap();
    let workspace = temp_dir.path().to_path_buf();

    // Create test files
    std::fs::create_dir_all(workspace.join("src")).unwrap();
    std::fs::write(workspace.join("src/main.rs"), "fn main() {}").unwrap();

    // Create app with open files
    let mut app = test_app_with_workspace(&workspace);
    app.open_file(&workspace.join("src/main.rs"));

    // Save session
    let session = SessionData::from_app_model(&app.model);
    let mut manager = SessionManager::for_workspace(Some(&workspace));
    manager.save(&session).unwrap();

    // Load session
    let loaded = manager.load().unwrap();

    assert_eq!(loaded.documents.len(), 1);
    assert_eq!(loaded.documents[0].path, PathBuf::from("src/main.rs"));
}

#[test]
fn test_session_handles_missing_files() {
    let temp_dir = tempfile::tempdir().unwrap();
    let workspace = temp_dir.path().to_path_buf();

    // Create session referencing non-existent file
    let session = SessionData {
        documents: vec![
            DocumentSessionData {
                path: PathBuf::from("deleted.rs"),
                path_is_relative: true,
                editor_state: EditorSessionData::default(),
                was_modified: false,
            }
        ],
        ..SessionData::new()
    };

    // Restore should skip missing file
    let model = AppModel::from_session(session, &workspace, 800, 600, 1.0);

    assert!(model.editor_area.documents.is_empty()
        || model.editor_area.documents.values().all(|d| d.file_path.is_none()));
}
```

### Manual Testing Checklist

- [ ] Open multiple files, split view, close editor, reopen
- [ ] Verify cursor positions restored
- [ ] Verify scroll positions restored
- [ ] Verify split layout restored with correct ratios
- [ ] Verify sidebar expanded folders restored
- [ ] Delete file from session, reopen, verify graceful handling
- [ ] `--no-session` flag works
- [ ] CLI files override session
- [ ] Multiple workspaces have separate sessions
- [ ] No-workspace mode has default session

---

## Edge Cases and Invariants

### Edge Cases

1. **File moved/renamed**: Path no longer valid; skip file, log warning
2. **File deleted**: Skip file, potentially warn user
3. **Workspace moved**: Session hash changes; effectively new workspace
4. **Permissions changed**: Handle read errors gracefully
5. **Corrupted session**: Fall back to clean state with warning
6. **Future version**: Refuse to load, suggest upgrade
7. **Very old session**: Consider staleness; maybe don't restore

### Invariants

1. Session file is always valid JSON (or missing)
2. Session version <= `SESSION_VERSION`
3. Cursor positions are validated against actual document length
4. Layout tree always has at least one group
5. Tab indices are within bounds of tab list

---

## References

- VS Code workspace state: https://code.visualstudio.com/docs/getstarted/settings#_workspace-settings
- JetBrains project state: `.idea/` directory structure
- Existing EditorArea: `src/model/editor_area.rs`
- CLI handling: `src/cli.rs`
- Config paths: `src/config_paths.rs`
