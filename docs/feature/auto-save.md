# Auto-Save

Automatic document saving with configurable triggers and modes

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
4. [Configuration](#configuration)
5. [Implementation Plan](#implementation-plan)
6. [Testing Strategy](#testing-strategy)
7. [References](#references)

---

## Overview

### Current State

The editor currently:
- Tracks dirty state via `Document.is_modified`
- Saves on explicit `Cmd+S` (or `Ctrl+S` on Windows/Linux)
- Shows modified indicator (`*`) in tab title
- Prompts on close if document has unsaved changes (not yet implemented)

### Goals

1. **Configurable auto-save modes**: off, on focus loss, after idle timeout
2. **Per-document override**: Allow disabling auto-save for specific files
3. **Visual feedback**: Status bar indicator when auto-save is active/triggered
4. **Conflict detection**: Integrate with file-change-detection (F-110) to prevent overwriting external changes
5. **Undo preservation**: Auto-save should not clear undo stack

### Non-Goals

- Cloud sync / remote storage
- Backup/snapshot versions (defer to session-restore F-120)
- Auto-save to temp file only (VS Code's "hot exit" pattern)
- Format-on-save (separate feature)

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              Main Thread                                     │
│                                                                              │
│  ┌──────────────┐                                                            │
│  │ Window Focus │───▶ WindowEvent::Focused(false)                            │
│  │    Lost      │                          │                                 │
│  └──────────────┘                          ▼                                 │
│                           ┌────────────────────────────────────┐             │
│                           │ Msg::App(AppMsg::WindowFocusLost)  │             │
│                           └────────────────┬───────────────────┘             │
│                                            │                                 │
│  ┌──────────────┐                          ▼                                 │
│  │   Document   │──────────▶ ┌──────────────────────────────┐                │
│  │    Edit      │            │  update_auto_save()          │                │
│  └──────────────┘            │                              │                │
│         │                    │  - Check auto_save_mode      │                │
│         ▼                    │  - Check document.is_modified│                │
│  ┌──────────────┐            │  - Check file_path exists    │                │
│  │Reset Idle    │            │  - Check no external changes │                │
│  │   Timer      │            └──────────────┬───────────────┘                │
│  └──────────────┘                           │                                │
│                                             ▼                                │
│  ┌──────────────┐            ┌──────────────────────────────┐                │
│  │ Idle Timer   │───────────▶│ Cmd::SaveFile { path, ... }  │                │
│  │   Fires      │            │                              │                │
│  └──────────────┘            │ (Same as manual save)        │                │
│                              └──────────────────────────────┘                │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────────────┐
│                          Idle Timer Thread                                   │
│                                                                              │
│  ┌──────────────────────────────────────────────────────────────────────┐   │
│  │  loop {                                                               │   │
│  │      rx.recv_timeout(remaining_time)?;  // Reset or timeout           │   │
│  │      if timeout => send Msg::AutoSave(AutoSaveMsg::IdleTimerFired)    │   │
│  │  }                                                                    │   │
│  └──────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Module Structure

```
src/
├── auto_save.rs              # NEW: AutoSaveState, timer logic
├── config.rs                 # Add AutoSaveConfig
├── messages.rs               # Add AutoSaveMsg
├── commands.rs               # (reuse existing Cmd::SaveFile)
├── update/
│   ├── mod.rs                # Route AutoSaveMsg
│   └── auto_save.rs          # NEW: update_auto_save()
└── app.rs                    # Window focus event handling
```

### Message Flow

1. **Focus Loss Trigger**:
   ```
   WindowEvent::Focused(false)
     → Msg::App(AppMsg::WindowFocusLost)
     → update_app() checks auto_save_mode
     → if FocusLoss: iterate modified documents
     → Cmd::Batch([SaveFile, SaveFile, ...])
   ```

2. **Idle Timeout Trigger**:
   ```
   Document edit
     → Msg::AutoSave(AutoSaveMsg::ResetIdleTimer { doc_id })
     → Timer thread resets countdown

   Timeout expires
     → Msg::AutoSave(AutoSaveMsg::IdleTimerFired { doc_id })
     → update_auto_save() checks document still modified
     → Cmd::SaveFile { ... }
   ```

---

## Data Structures

### Configuration

```rust
// In src/config.rs

/// Auto-save configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct AutoSaveConfig {
    /// Auto-save mode
    #[serde(default)]
    pub mode: AutoSaveMode,

    /// Idle delay in milliseconds (used when mode is AfterDelay)
    /// Default: 1000ms (1 second)
    #[serde(default = "default_auto_save_delay")]
    pub delay_ms: u64,

    /// File patterns to exclude from auto-save (glob patterns)
    /// Example: ["*.tmp", "*.log", "COMMIT_EDITMSG"]
    #[serde(default)]
    pub exclude_patterns: Vec<String>,
}

fn default_auto_save_delay() -> u64 {
    1000
}

impl Default for AutoSaveConfig {
    fn default() -> Self {
        Self {
            mode: AutoSaveMode::Off,
            delay_ms: default_auto_save_delay(),
            exclude_patterns: vec![
                "COMMIT_EDITMSG".to_string(),
                "*.tmp".to_string(),
            ],
        }
    }
}

/// Auto-save mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AutoSaveMode {
    /// Auto-save disabled (default)
    #[default]
    Off,

    /// Save when window loses focus
    OnFocusLoss,

    /// Save after idle delay (no edits for N ms)
    AfterDelay,

    /// Save on both focus loss AND after delay
    OnFocusLossAndDelay,
}
```

### Runtime State

```rust
// In src/auto_save.rs

use std::sync::mpsc;
use std::time::{Duration, Instant};
use std::collections::HashMap;

use crate::model::editor_area::DocumentId;
use crate::messages::Msg;

/// Per-document auto-save state
#[derive(Debug, Clone)]
pub struct DocumentAutoSaveState {
    /// When the document was last modified
    pub last_modified: Instant,

    /// Whether auto-save is disabled for this specific document
    pub disabled: bool,

    /// Last known external modification time (for conflict detection)
    pub last_external_mtime: Option<std::time::SystemTime>,
}

impl Default for DocumentAutoSaveState {
    fn default() -> Self {
        Self {
            last_modified: Instant::now(),
            disabled: false,
            last_external_mtime: None,
        }
    }
}

/// Global auto-save state
#[derive(Debug)]
pub struct AutoSaveState {
    /// Per-document state
    pub documents: HashMap<DocumentId, DocumentAutoSaveState>,

    /// Channel to reset/stop the idle timer
    timer_tx: Option<mpsc::Sender<TimerCommand>>,

    /// Currently pending auto-save (document being waited on)
    pending_doc_id: Option<DocumentId>,
}

#[derive(Debug, Clone)]
enum TimerCommand {
    /// Reset the timer with a new duration
    Reset { doc_id: DocumentId, delay_ms: u64 },
    /// Stop the timer entirely
    Stop,
}

impl AutoSaveState {
    pub fn new() -> Self {
        Self {
            documents: HashMap::new(),
            timer_tx: None,
            pending_doc_id: None,
        }
    }

    /// Start or restart the idle timer for a document
    pub fn schedule_idle_save(
        &mut self,
        doc_id: DocumentId,
        delay_ms: u64,
        msg_tx: mpsc::Sender<Msg>,
    ) {
        // Update last modified time
        self.documents
            .entry(doc_id)
            .or_default()
            .last_modified = Instant::now();

        // If timer thread exists, send reset command
        if let Some(tx) = &self.timer_tx {
            let _ = tx.send(TimerCommand::Reset { doc_id, delay_ms });
            self.pending_doc_id = Some(doc_id);
            return;
        }

        // Otherwise, spawn timer thread
        let (tx, rx) = mpsc::channel();
        self.timer_tx = Some(tx);
        self.pending_doc_id = Some(doc_id);

        std::thread::spawn(move || {
            idle_timer_loop(rx, msg_tx, doc_id, delay_ms);
        });
    }

    /// Stop any pending auto-save timer
    pub fn cancel_pending(&mut self) {
        if let Some(tx) = &self.timer_tx {
            let _ = tx.send(TimerCommand::Stop);
        }
        self.pending_doc_id = None;
    }

    /// Check if a document should be excluded from auto-save
    pub fn should_exclude(&self, path: &std::path::Path, patterns: &[String]) -> bool {
        let file_name = path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");

        for pattern in patterns {
            if pattern.starts_with('*') {
                // Simple suffix match (e.g., "*.tmp")
                let suffix = &pattern[1..];
                if file_name.ends_with(suffix) {
                    return true;
                }
            } else if file_name == pattern {
                // Exact match
                return true;
            }
        }

        false
    }
}

/// Idle timer thread loop
fn idle_timer_loop(
    rx: mpsc::Receiver<TimerCommand>,
    msg_tx: mpsc::Sender<Msg>,
    initial_doc_id: DocumentId,
    initial_delay_ms: u64,
) {
    let mut current_doc_id = initial_doc_id;
    let mut delay = Duration::from_millis(initial_delay_ms);
    let mut deadline = Instant::now() + delay;

    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());

        match rx.recv_timeout(remaining) {
            Ok(TimerCommand::Reset { doc_id, delay_ms }) => {
                // Reset timer for (possibly different) document
                current_doc_id = doc_id;
                delay = Duration::from_millis(delay_ms);
                deadline = Instant::now() + delay;
            }
            Ok(TimerCommand::Stop) => {
                // Exit the timer thread
                return;
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                // Timer fired - send auto-save message
                let _ = msg_tx.send(Msg::AutoSave(AutoSaveMsg::IdleTimerFired {
                    document_id: current_doc_id,
                }));

                // Wait for next reset or stop
                match rx.recv() {
                    Ok(TimerCommand::Reset { doc_id, delay_ms }) => {
                        current_doc_id = doc_id;
                        delay = Duration::from_millis(delay_ms);
                        deadline = Instant::now() + delay;
                    }
                    Ok(TimerCommand::Stop) | Err(_) => return,
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                return;
            }
        }
    }
}
```

### Messages

```rust
// In src/messages.rs

/// Auto-save messages
#[derive(Debug, Clone)]
pub enum AutoSaveMsg {
    /// Idle timer fired for a document
    IdleTimerFired { document_id: DocumentId },

    /// Toggle auto-save for a specific document
    ToggleDocumentAutoSave { document_id: DocumentId },

    /// Save completed (for status feedback)
    SaveCompleted { document_id: DocumentId, success: bool },
}

// Update top-level Msg enum
pub enum Msg {
    Editor(EditorMsg),
    Document(DocumentMsg),
    Ui(UiMsg),
    Layout(LayoutMsg),
    App(AppMsg),
    Syntax(SyntaxMsg),
    Csv(CsvMsg),
    Workspace(WorkspaceMsg),
    AutoSave(AutoSaveMsg),  // NEW
}
```

### AppMsg Extension

```rust
// In src/messages.rs, add to AppMsg

pub enum AppMsg {
    // ... existing variants ...

    /// Window gained focus
    WindowFocusGained,

    /// Window lost focus (triggers auto-save if configured)
    WindowFocusLost,
}
```

---

## Configuration

### YAML Configuration

```yaml
# ~/.config/token-editor/config.yaml

auto_save:
  # Options: off, onFocusLoss, afterDelay, onFocusLossAndDelay
  mode: afterDelay

  # Delay in milliseconds (only used for afterDelay modes)
  delay_ms: 1000

  # Files to exclude from auto-save
  exclude_patterns:
    - COMMIT_EDITMSG
    - "*.tmp"
    - "*.log"
```

### Status Bar Integration

When auto-save is active, show in status bar:

```
┌────────────────────────────────────────────────────────────────────┐
│  main.rs                                │ AutoSave: On │ Ln 42    │
└────────────────────────────────────────────────────────────────────┘
```

Brief flash when auto-save triggers:

```
┌────────────────────────────────────────────────────────────────────┐
│  main.rs                                │   Saving...  │ Ln 42    │
└────────────────────────────────────────────────────────────────────┘
```

---

## Implementation Plan

### Phase 1: Core Infrastructure

**Estimated effort: 2-3 days**

1. [ ] Add `AutoSaveConfig` to `EditorConfig`
2. [ ] Add `auto_save` section to default config.yaml
3. [ ] Create `src/auto_save.rs` with `AutoSaveState`
4. [ ] Add `AutoSaveMsg` to messages.rs
5. [ ] Add `auto_save: AutoSaveState` field to `App` struct
6. [ ] Create `src/update/auto_save.rs` with update handler

**Test:** Config loads correctly, `AutoSaveState` initializes

### Phase 2: Focus Loss Trigger

**Estimated effort: 1-2 days**

1. [ ] Handle `WindowEvent::Focused(false)` in `App::window_event()`
2. [ ] Dispatch `Msg::App(AppMsg::WindowFocusLost)`
3. [ ] In `update_app()`, check `auto_save.mode` includes focus loss
4. [ ] Iterate all modified documents with file paths
5. [ ] Filter by exclude patterns
6. [ ] Generate `Cmd::Batch([SaveFile, ...])` for each

**Test:** Modify document, click away from window, verify file saved

### Phase 3: Idle Timer Trigger

**Estimated effort: 2-3 days**

1. [ ] Wire document edit messages to `AutoSaveState::schedule_idle_save()`
2. [ ] Implement timer thread with reset/stop commands
3. [ ] Handle `AutoSaveMsg::IdleTimerFired` in update
4. [ ] Verify document still modified before saving
5. [ ] Handle case where document was closed before timer fired

**Test:** Edit document, wait for delay, verify file saved

### Phase 4: Conflict Detection Integration

**Estimated effort: 1-2 days**

1. [ ] Store `last_external_mtime` in `DocumentAutoSaveState`
2. [ ] Before auto-save, check file mtime hasn't changed
3. [ ] If conflict detected, skip auto-save and show warning
4. [ ] Integrate with file-change-detection (F-110) when available

**Test:** Modify file externally during idle delay, verify no overwrite

### Phase 5: Status Bar & Polish

**Estimated effort: 1 day**

1. [ ] Add auto-save status segment to status bar
2. [ ] Show transient "Saving..." message during save
3. [ ] Add command palette entry to toggle auto-save mode
4. [ ] Add per-document toggle via context menu / command

**Test:** Status bar updates correctly, mode toggle works

---

## Testing Strategy

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exclude_pattern_suffix() {
        let state = AutoSaveState::new();
        let patterns = vec!["*.tmp".to_string()];

        assert!(state.should_exclude(Path::new("foo.tmp"), &patterns));
        assert!(!state.should_exclude(Path::new("foo.txt"), &patterns));
    }

    #[test]
    fn test_exclude_pattern_exact() {
        let state = AutoSaveState::new();
        let patterns = vec!["COMMIT_EDITMSG".to_string()];

        assert!(state.should_exclude(Path::new("COMMIT_EDITMSG"), &patterns));
        assert!(!state.should_exclude(Path::new("README.md"), &patterns));
    }

    #[test]
    fn test_auto_save_mode_default() {
        let config = AutoSaveConfig::default();
        assert_eq!(config.mode, AutoSaveMode::Off);
        assert_eq!(config.delay_ms, 1000);
    }

    #[test]
    fn test_config_serialization() {
        let config = AutoSaveConfig {
            mode: AutoSaveMode::AfterDelay,
            delay_ms: 500,
            exclude_patterns: vec!["*.log".to_string()],
        };

        let yaml = serde_yaml::to_string(&config).unwrap();
        let parsed: AutoSaveConfig = serde_yaml::from_str(&yaml).unwrap();

        assert_eq!(parsed.mode, AutoSaveMode::AfterDelay);
        assert_eq!(parsed.delay_ms, 500);
    }
}
```

### Integration Tests

```rust
#[test]
fn test_focus_loss_saves_modified_documents() {
    let mut app = test_app_with_config(AutoSaveConfig {
        mode: AutoSaveMode::OnFocusLoss,
        ..Default::default()
    });

    // Create and modify a document
    app.dispatch(Msg::Layout(LayoutMsg::NewTab));
    app.dispatch(Msg::Document(DocumentMsg::InsertChar('x')));

    // Simulate focus loss
    app.dispatch(Msg::App(AppMsg::WindowFocusLost));

    // Verify save command was generated
    let cmds = app.pending_commands();
    assert!(cmds.iter().any(|c| matches!(c, Cmd::SaveFile { .. })));
}

#[test]
fn test_idle_timer_triggers_save() {
    // This test needs async/threading support
    // Use test harness that can advance time
}

#[test]
fn test_excluded_files_not_saved() {
    let mut app = test_app_with_config(AutoSaveConfig {
        mode: AutoSaveMode::OnFocusLoss,
        exclude_patterns: vec!["COMMIT_EDITMSG".to_string()],
        ..Default::default()
    });

    // Open COMMIT_EDITMSG and modify
    app.open_file(Path::new("/tmp/COMMIT_EDITMSG"));
    app.dispatch(Msg::Document(DocumentMsg::InsertChar('x')));

    // Simulate focus loss
    app.dispatch(Msg::App(AppMsg::WindowFocusLost));

    // Verify NO save command for this file
    let cmds = app.pending_commands();
    assert!(cmds.iter().all(|c| !matches!(c, Cmd::SaveFile { path, .. } if path.ends_with("COMMIT_EDITMSG"))));
}
```

### Manual Testing Checklist

- [ ] Focus loss saves all modified documents (mode: onFocusLoss)
- [ ] Idle timer triggers after delay (mode: afterDelay)
- [ ] Timer resets on continued editing
- [ ] Excluded files are not auto-saved
- [ ] New/unsaved files (no path) are not auto-saved
- [ ] Status bar shows "Saving..." during save
- [ ] Undo stack preserved after auto-save
- [ ] Auto-save disabled per-document works
- [ ] Config changes take effect without restart

---

## Edge Cases and Invariants

### Edge Cases

1. **No file path**: Documents without a file path (new/untitled) cannot be auto-saved
2. **Read-only files**: Auto-save should not attempt to save read-only files
3. **Network files**: Save may fail; handle gracefully with retry/warning
4. **Rapid edits**: Timer should reset, not accumulate saves
5. **Document closed**: Timer should be cancelled when document is closed
6. **Focus bounce**: Quick focus loss/gain should still trigger if mode is focus-based

### Invariants

1. `is_modified` must be `true` for auto-save to trigger
2. `file_path` must be `Some(_)` for auto-save to trigger
3. Timer thread must exit when `AutoSaveState` is dropped (no orphaned threads)
4. Undo stack must not be affected by auto-save
5. Only one timer thread should exist at a time

---

## References

- VS Code auto-save: https://code.visualstudio.com/docs/editor/codebasics#_save-auto-save
- JetBrains auto-save: https://www.jetbrains.com/help/idea/saving-and-reverting-changes.html
- Existing save implementation: `src/update/app.rs` (SaveFile handling)
- File watcher integration: `docs/feature/file-change-detection.md` (F-110)
