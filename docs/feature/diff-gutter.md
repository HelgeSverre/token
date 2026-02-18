# Diff Gutter

Mini diff markers in the gutter showing changed/added/deleted lines vs disk

> **Status:** Planning
> **Priority:** P2
> **Effort:** M
> **Created:** 2025-12-19
> **Milestone:** 5 - Insight Tools
> **Feature ID:** F-160

---

## Table of Contents

1. [Overview](#overview)
2. [Architecture](#architecture)
3. [Data Structures](#data-structures)
4. [Rendering](#rendering)
5. [Keybindings](#keybindings)
6. [Implementation Plan](#implementation-plan)
7. [Testing Strategy](#testing-strategy)
8. [References](#references)

---

## Overview

### Current State

The editor currently has:

- Line number gutter rendered alongside the text area
- File modification tracking (`Document.is_modified` flag)
- File system watcher for external changes (`src/fs_watcher.rs`)
- Rope-based buffer with efficient line access

However, there is no visual indication of *which* lines have changed since the file was last saved or loaded. Users must rely on the modification indicator in the tab title or status bar.

### Goals

1. **Visual diff indicators** - Show colored markers in the gutter for:
   - Added lines (green)
   - Modified lines (yellow/orange)
   - Deleted lines (red triangle/marker)

2. **Efficient diffing** - Compute diff incrementally when possible, avoid full diff on every keystroke

3. **Disk-based comparison** - Compare current buffer against last saved/loaded content

4. **Navigation** - Jump to next/previous change

5. **Inline diff preview** - Option to show what changed on hover or via command

### Non-Goals (This Phase)

- Git integration (comparing against HEAD, branches, etc.)
- Blame annotations
- Multi-file diff views
- Merge conflict resolution UI
- External diff tool integration

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           Diff Gutter Flow                                  │
│                                                                             │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────────────────────┐  │
│  │   Document   │───▶│  DiffEngine  │───▶│       LineDiffState          │  │
│  │   Changed    │    │  (on edit)   │    │  Vec<LineChange> per doc     │  │
│  └──────────────┘    └──────────────┘    └───────────────┬──────────────┘  │
│         │                   │                             │                 │
│         │                   │                             │                 │
│         ▼                   ▼                             ▼                 │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────────────────────┐  │
│  │ Save/Load    │───▶│  Baseline    │    │        Renderer              │  │
│  │ Operation    │    │  Snapshot    │    │  (draws gutter markers)      │  │
│  └──────────────┘    └──────────────┘    └──────────────────────────────┘  │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘

     Baseline Update              Incremental Diff               Render
        on Save                    on Each Edit                 Each Frame
```

### Module Structure

```
src/
├── diff/                        # NEW MODULE
│   ├── mod.rs                   # Public exports
│   ├── engine.rs                # DiffEngine, Myers diff algorithm wrapper
│   ├── state.rs                 # LineDiffState, per-document diff cache
│   └── gutter.rs                # Gutter marker types, rendering helpers
├── model/
│   └── document.rs              # + baseline_content: Option<String>
├── update/
│   └── document.rs              # + trigger diff recompute after edits
└── view/
    └── mod.rs                   # + render_diff_gutter()
```

### Message Flow

```
                         ┌──────────────────┐
                         │   User Types     │
                         └────────┬─────────┘
                                  │
                                  ▼
                         ┌──────────────────┐
                         │ DocumentMsg::    │
                         │ InsertChar(ch)   │
                         └────────┬─────────┘
                                  │
                                  ▼
                    ┌─────────────────────────────┐
                    │  update_document()          │
                    │  - Apply edit to buffer     │
                    │  - Increment revision       │
                    └─────────────┬───────────────┘
                                  │
                                  ▼
                    ┌─────────────────────────────┐
                    │  Cmd::DebouncedDiffCompute  │
                    │  { doc_id, revision, 100ms }│
                    └─────────────┬───────────────┘
                                  │
                                  ▼ (after debounce)
                    ┌─────────────────────────────┐
                    │  Cmd::RunDiffCompute        │
                    │  { doc_id, revision,        │
                    │    current, baseline }      │
                    └─────────────┬───────────────┘
                                  │
                                  ▼ (background thread)
                    ┌─────────────────────────────┐
                    │  Msg::DiffComputed          │
                    │  { doc_id, revision,        │
                    │    line_changes }           │
                    └─────────────┬───────────────┘
                                  │
                                  ▼
                    ┌─────────────────────────────┐
                    │  Store in Document.         │
                    │  diff_state                 │
                    │  Cmd::Redraw                │
                    └─────────────────────────────┘
```

---

## Data Structures

### Core Types

```rust
// src/diff/mod.rs

/// Type of change for a line relative to baseline
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineChangeKind {
    /// Line was added (not in baseline)
    Added,
    /// Line content was modified
    Modified,
    /// Line(s) were deleted at this position
    /// The count indicates how many baseline lines were removed
    Deleted { count: usize },
}

/// A single line change marker
#[derive(Debug, Clone)]
pub struct LineChange {
    /// Line number in current buffer (0-indexed)
    pub line: usize,
    /// Type of change
    pub kind: LineChangeKind,
}

/// Diff state for a single document
#[derive(Debug, Clone, Default)]
pub struct LineDiffState {
    /// All line changes relative to baseline
    pub changes: Vec<LineChange>,
    /// Revision this diff corresponds to
    pub revision: u64,
    /// Whether diff is currently being computed
    pub computing: bool,
}

impl LineDiffState {
    /// Get the change kind for a specific line, if any
    pub fn change_at(&self, line: usize) -> Option<LineChangeKind> {
        self.changes
            .iter()
            .find(|c| c.line == line)
            .map(|c| c.kind)
    }

    /// Get indices of all changed lines for navigation
    pub fn changed_lines(&self) -> Vec<usize> {
        self.changes.iter().map(|c| c.line).collect()
    }

    /// Find next change after given line (wraps around)
    pub fn next_change(&self, after_line: usize) -> Option<usize> {
        let lines = self.changed_lines();
        if lines.is_empty() {
            return None;
        }

        // Find first change after current line
        if let Some(&line) = lines.iter().find(|&&l| l > after_line) {
            return Some(line);
        }

        // Wrap to first change
        Some(lines[0])
    }

    /// Find previous change before given line (wraps around)
    pub fn prev_change(&self, before_line: usize) -> Option<usize> {
        let lines = self.changed_lines();
        if lines.is_empty() {
            return None;
        }

        // Find last change before current line
        if let Some(&line) = lines.iter().rev().find(|&&l| l < before_line) {
            return Some(line);
        }

        // Wrap to last change
        lines.last().copied()
    }
}
```

### Document Extension

```rust
// In src/model/document.rs

pub struct Document {
    // ... existing fields ...

    /// Baseline content for diff comparison
    /// Set on load/save, cleared on close
    pub baseline_content: Option<String>,

    /// Current diff state (computed asynchronously)
    pub diff_state: LineDiffState,
}

impl Document {
    /// Update baseline to current buffer content
    /// Called after successful save
    pub fn update_baseline(&mut self) {
        self.baseline_content = Some(self.buffer.to_string());
        self.diff_state = LineDiffState::default(); // No changes after save
    }

    /// Set baseline from loaded content
    /// Called when file is loaded from disk
    pub fn set_baseline(&mut self, content: String) {
        self.baseline_content = Some(content);
        self.diff_state = LineDiffState::default();
    }

    /// Check if we have a baseline to diff against
    pub fn has_baseline(&self) -> bool {
        self.baseline_content.is_some()
    }
}
```

### Diff Engine

```rust
// src/diff/engine.rs

use similar::{ChangeTag, TextDiff};

/// Compute line-level diff between baseline and current content
pub fn compute_line_diff(baseline: &str, current: &str) -> Vec<LineChange> {
    let diff = TextDiff::from_lines(baseline, current);
    let mut changes = Vec::new();
    let mut current_line = 0;
    let mut pending_deletes = 0;

    for change in diff.iter_all_changes() {
        match change.tag() {
            ChangeTag::Equal => {
                // Flush any pending deletes at this position
                if pending_deletes > 0 {
                    changes.push(LineChange {
                        line: current_line,
                        kind: LineChangeKind::Deleted { count: pending_deletes },
                    });
                    pending_deletes = 0;
                }
                current_line += 1;
            }
            ChangeTag::Delete => {
                // Accumulate consecutive deletes
                pending_deletes += 1;
            }
            ChangeTag::Insert => {
                // Check if this replaces deleted lines (modification)
                if pending_deletes > 0 {
                    changes.push(LineChange {
                        line: current_line,
                        kind: LineChangeKind::Modified,
                    });
                    pending_deletes -= 1;
                } else {
                    changes.push(LineChange {
                        line: current_line,
                        kind: LineChangeKind::Added,
                    });
                }
                current_line += 1;
            }
        }
    }

    // Handle trailing deletes (at end of file)
    if pending_deletes > 0 {
        changes.push(LineChange {
            line: current_line.saturating_sub(1),
            kind: LineChangeKind::Deleted { count: pending_deletes },
        });
    }

    changes
}

/// Background diff computation task
pub fn compute_diff_async(
    baseline: String,
    current: String,
    doc_id: DocumentId,
    revision: u64,
    tx: std::sync::mpsc::Sender<Msg>,
) {
    std::thread::spawn(move || {
        let changes = compute_line_diff(&baseline, &current);
        let _ = tx.send(Msg::Diff(DiffMsg::Computed {
            document_id: doc_id,
            revision,
            changes,
        }));
    });
}
```

### Messages and Commands

```rust
// In src/messages.rs

/// Diff-related messages
#[derive(Debug, Clone)]
pub enum DiffMsg {
    /// Diff computation completed
    Computed {
        document_id: DocumentId,
        revision: u64,
        changes: Vec<LineChange>,
    },
    /// Navigate to next change
    GotoNextChange,
    /// Navigate to previous change
    GotoPrevChange,
    /// Show inline diff for current line
    ShowLineDiff,
    /// Hide inline diff
    HideLineDiff,
}

// Add to Msg enum:
pub enum Msg {
    // ... existing variants ...
    Diff(DiffMsg),
}
```

```rust
// In src/commands.rs

pub enum Cmd {
    // ... existing variants ...

    /// Start debounce timer for diff computation
    DebouncedDiffCompute {
        document_id: DocumentId,
        revision: u64,
        delay_ms: u64,
    },

    /// Run diff computation in background
    RunDiffCompute {
        document_id: DocumentId,
        revision: u64,
        baseline: String,
        current: String,
    },
}
```

### Theme Extension

```rust
// In src/theme.rs

pub struct EditorTheme {
    // ... existing fields ...

    /// Diff gutter colors
    pub diff_gutter: DiffGutterTheme,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DiffGutterTheme {
    /// Color for added lines (default: green)
    pub added: Color,
    /// Color for modified lines (default: yellow/orange)
    pub modified: Color,
    /// Color for deleted line markers (default: red)
    pub deleted: Color,
    /// Width of the diff marker in pixels (default: 3)
    pub marker_width: u32,
}

impl Default for DiffGutterTheme {
    fn default() -> Self {
        Self {
            added: Color::rgb(0x28, 0xA7, 0x45),      // GitHub green
            modified: Color::rgb(0xD2, 0x9D, 0x22),    // Yellow/gold
            deleted: Color::rgb(0xCB, 0x24, 0x31),     // Red
            marker_width: 3,
        }
    }
}
```

---

## Rendering

### Gutter Marker Rendering

```
┌───┬────┬────────────────────────────────────────────┐
│ ▌ │  1 │ fn main() {                                │  <- Modified line (yellow bar)
│   │  2 │     let x = 1;                             │  <- Unchanged
│ ▌ │  3 │     let y = 2;  // new comment             │  <- Modified line (yellow bar)
│ █ │  4 │     let z = 3;                             │  <- Added line (green bar)
│ █ │  5 │     let w = 4;                             │  <- Added line (green bar)
│   │  6 │     println!("{}", x + y);                 │  <- Unchanged
│ ◀ │  7 │ }                                          │  <- Deleted after this (red triangle)
└───┴────┴────────────────────────────────────────────┘
  │    │
  │    └── Line number gutter (existing)
  └─────── Diff marker gutter (new, 3-4px wide)
```

### Rendering Logic

```rust
// src/diff/gutter.rs

use crate::view::Frame;
use crate::theme::DiffGutterTheme;

/// Render diff markers for visible lines
pub fn render_diff_gutter(
    frame: &mut Frame,
    diff_state: &LineDiffState,
    visible_start_line: usize,
    visible_end_line: usize,
    gutter_x: usize,           // X position of diff gutter
    line_height: usize,
    y_offset: usize,           // Y offset (after tab bar)
    theme: &DiffGutterTheme,
) {
    let marker_width = theme.marker_width as usize;

    for line in visible_start_line..=visible_end_line {
        let Some(change_kind) = diff_state.change_at(line) else {
            continue;
        };

        let line_y = y_offset + (line - visible_start_line) * line_height;

        match change_kind {
            LineChangeKind::Added => {
                // Green vertical bar
                frame.fill_rect(
                    gutter_x,
                    line_y,
                    marker_width,
                    line_height,
                    theme.added.to_argb_u32(),
                );
            }
            LineChangeKind::Modified => {
                // Yellow/orange vertical bar
                frame.fill_rect(
                    gutter_x,
                    line_y,
                    marker_width,
                    line_height,
                    theme.modified.to_argb_u32(),
                );
            }
            LineChangeKind::Deleted { count } => {
                // Red triangle/arrow pointing left
                // Indicates lines were deleted at this position
                render_delete_marker(
                    frame,
                    gutter_x,
                    line_y,
                    marker_width,
                    line_height,
                    count,
                    theme.deleted.to_argb_u32(),
                );
            }
        }
    }
}

/// Render a small triangle marker for deleted lines
fn render_delete_marker(
    frame: &mut Frame,
    x: usize,
    y: usize,
    width: usize,
    height: usize,
    _count: usize,
    color: u32,
) {
    // Draw a small left-pointing triangle at the line boundary
    // Positioned at the bottom of the line to indicate "deleted below"
    let triangle_height = height.min(8);
    let triangle_y = y + height - triangle_height;

    for dy in 0..triangle_height {
        let row_width = (width * (triangle_height - dy)) / triangle_height;
        if row_width > 0 {
            frame.fill_rect(
                x,
                triangle_y + dy,
                row_width,
                1,
                color,
            );
        }
    }
}
```

---

## Keybindings

| Action | Mac | Windows/Linux | Command |
|--------|-----|---------------|---------|
| Next change | `Option+]` | `Alt+]` | `DiffGotoNext` |
| Previous change | `Option+[` | `Alt+[` | `DiffGotoPrev` |
| Show line diff | `Option+D` | `Alt+D` | `DiffShowLine` |
| Revert line | `Option+Shift+D` | `Alt+Shift+D` | `DiffRevertLine` |

### Keymap Configuration

```yaml
# keymap.yaml additions

# Diff navigation
- key: "alt+]"
  command: DiffGotoNext

- key: "alt+["
  command: DiffGotoPrev

- key: "alt+d"
  command: DiffShowLine

- key: "alt+shift+d"
  command: DiffRevertLine
```

---

## Implementation Plan

### Phase 1: Core Data Structures

**Effort:** S (1-2 days)

- [ ] Create `src/diff/mod.rs` with module structure
- [ ] Implement `LineChange`, `LineChangeKind`, `LineDiffState`
- [ ] Add `baseline_content` and `diff_state` to `Document`
- [ ] Add `DiffGutterTheme` to theme system
- [ ] Update YAML theme files with diff colors

**Test:** `Document::has_baseline()` returns true after load.

### Phase 2: Diff Engine

**Effort:** S (1-2 days)

- [ ] Add `similar` crate dependency for diffing
- [ ] Implement `compute_line_diff()` function
- [ ] Add unit tests for various diff scenarios
- [ ] Implement async diff computation with thread spawn

**Test:** `compute_line_diff("a\nb\nc", "a\nX\nc")` returns Modified for line 1.

### Phase 3: Message Flow Integration

**Effort:** M (2-3 days)

- [ ] Add `DiffMsg` to messages.rs
- [ ] Add `DebouncedDiffCompute` and `RunDiffCompute` to commands.rs
- [ ] Create `src/update/diff.rs` update handler
- [ ] Trigger diff compute after document edits
- [ ] Update baseline on save/load operations
- [ ] Implement debouncing (100ms default)

**Test:** Editing document triggers `DiffComputed` message after debounce.

### Phase 4: Gutter Rendering

**Effort:** M (2-3 days)

- [ ] Implement `render_diff_gutter()` in diff/gutter.rs
- [ ] Integrate into main render loop
- [ ] Adjust gutter width calculations to include diff marker
- [ ] Handle scrolling and viewport correctly
- [ ] Test with various theme colors

**Test:** Modified line shows yellow marker in gutter.

### Phase 5: Navigation

**Effort:** S (1 day)

- [ ] Add `DiffGotoNext` and `DiffGotoPrev` commands to keymap
- [ ] Implement navigation in update handler
- [ ] Scroll to change and center cursor
- [ ] Handle wrap-around behavior

**Test:** `]` on last change wraps to first change.

### Phase 6: Polish

**Effort:** S (1 day)

- [ ] Add status bar segment showing change count
- [ ] Improve delete marker visual (consider alternatives)
- [ ] Performance optimization for large files
- [ ] Documentation

**Test:** Status bar shows "3 changes" when document has 3 modified lines.

---

## Testing Strategy

### Unit Tests

```rust
// tests/diff.rs

#[test]
fn test_diff_added_lines() {
    let baseline = "line1\nline2\n";
    let current = "line1\nNEW\nline2\n";
    let changes = compute_line_diff(baseline, current);

    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].line, 1);
    assert_eq!(changes[0].kind, LineChangeKind::Added);
}

#[test]
fn test_diff_modified_lines() {
    let baseline = "line1\nline2\nline3\n";
    let current = "line1\nMODIFIED\nline3\n";
    let changes = compute_line_diff(baseline, current);

    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].line, 1);
    assert_eq!(changes[0].kind, LineChangeKind::Modified);
}

#[test]
fn test_diff_deleted_lines() {
    let baseline = "line1\nline2\nline3\n";
    let current = "line1\nline3\n";
    let changes = compute_line_diff(baseline, current);

    assert_eq!(changes.len(), 1);
    assert!(matches!(changes[0].kind, LineChangeKind::Deleted { count: 1 }));
}

#[test]
fn test_diff_complex_changes() {
    let baseline = "a\nb\nc\nd\ne\n";
    let current = "a\nX\nY\nd\n";  // Modified b->X, added Y after, deleted c, deleted e
    let changes = compute_line_diff(baseline, current);

    // Verify multiple change types detected
    assert!(changes.iter().any(|c| c.kind == LineChangeKind::Modified));
    assert!(changes.iter().any(|c| c.kind == LineChangeKind::Added));
    assert!(changes.iter().any(|c| matches!(c.kind, LineChangeKind::Deleted { .. })));
}

#[test]
fn test_diff_state_navigation() {
    let state = LineDiffState {
        changes: vec![
            LineChange { line: 5, kind: LineChangeKind::Added },
            LineChange { line: 10, kind: LineChangeKind::Modified },
            LineChange { line: 20, kind: LineChangeKind::Added },
        ],
        revision: 1,
        computing: false,
    };

    assert_eq!(state.next_change(0), Some(5));
    assert_eq!(state.next_change(5), Some(10));
    assert_eq!(state.next_change(20), Some(5)); // Wraps

    assert_eq!(state.prev_change(20), Some(10));
    assert_eq!(state.prev_change(5), Some(20)); // Wraps
}

#[test]
fn test_baseline_update_on_save() {
    let mut doc = Document::with_text("initial content");
    doc.set_baseline("initial content".to_string());

    // Simulate edit
    // ... modify buffer ...

    // After save
    doc.update_baseline();
    assert!(doc.diff_state.changes.is_empty());
}
```

### Integration Tests

```rust
// tests/diff_integration.rs

#[test]
fn test_edit_triggers_diff_recompute() {
    let mut model = test_model_with_file("test.txt", "line1\nline2\n");

    // Edit: insert character
    update(&mut model, Msg::Document(DocumentMsg::InsertChar('X')));

    // After debounce, diff should be computed
    // (In real test, we'd mock the timer and check the command)
}

#[test]
fn test_save_clears_diff() {
    let mut model = test_model_with_file("test.txt", "content");

    // Make changes
    update(&mut model, Msg::Document(DocumentMsg::InsertChar('!')));

    // Verify diff shows changes
    // ...

    // Save
    update(&mut model, Msg::App(AppMsg::SaveFile));

    // Diff should be empty after save
    let doc = model.focused_document().unwrap();
    assert!(doc.diff_state.changes.is_empty());
}
```

---

## Dependencies

```toml
# Cargo.toml additions

[dependencies]
similar = "2.4"  # Text diffing library (fast, pure Rust)
```

### Why `similar`?

- Pure Rust implementation (no C dependencies)
- Fast Myers diff algorithm
- Line-level and character-level diffing
- Well-maintained, used by insta (snapshot testing)
- Small footprint (~50KB)

---

## Performance Considerations

### Debouncing

Diff computation is debounced (100ms default) to avoid recomputing on every keystroke:

```rust
const DIFF_DEBOUNCE_MS: u64 = 100;
```

### Large File Handling

For files > 10,000 lines:

1. Consider chunked diffing (diff visible region + context)
2. Increase debounce time
3. Show "computing..." indicator

### Memory

Baseline content doubles memory usage per document. For very large files:

- Consider storing baseline as compressed chunks
- Or compute baseline hash and re-read from disk on demand

---

## Future Enhancements

### Phase 2: Git Integration

- Compare against last commit instead of disk
- Show branch/commit in status
- Stage/unstage individual lines

### Phase 3: Inline Diff

- Show deleted content inline (strikethrough)
- Character-level highlighting within changed lines
- Mini diff tooltip on hover

### Phase 4: Revert Operations

- Revert single line to baseline
- Revert selection to baseline
- Undo revert

---

## References

- [similar crate](https://crates.io/crates/similar) - Diffing library
- [VS Code SCM Decorations](https://code.visualstudio.com/docs/sourcecontrol/overview#_scm-decorations) - Visual reference
- [Myers Diff Algorithm](https://blog.jcoglan.com/2017/02/12/the-myers-diff-algorithm-part-1/) - Algorithm explanation
- [Feature: Workspace Management](../archived/workspace-management.md) - File system watching dependency
