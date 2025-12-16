# Debug Tracing & Instrumentation

> **Status**: ✅ Complete  
> **Priority**: High (debugging multi-cursor issues)  
> **Related**: Multi-cursor system, Elm architecture  
> **Last Updated**: 2025-12-08

## Problem Statement

Debugging multi-cursor positioning and selection state is difficult because:

1. State changes happen rapidly across multiple cursors
2. Invariant violations may occur several messages after the root cause
3. No visibility into message flow and state transitions
4. Hard to correlate user input with resulting state changes

## Goals

1. **Trace message flow** through the Elm architecture (Message → Update → Command)
2. **Capture before/after state** for cursor and selection operations
3. **Scoped filtering** to focus on specific subsystems (cursor._, selection._, etc.)
4. **In-editor debug overlay** for real-time state visibility
5. **Enhanced invariant assertions** with context about what triggered the failure

## Non-Goals

- Production logging/telemetry (this is dev-only instrumentation)
- Performance profiling (separate concern, see `perf.rs`)
- Distributed tracing

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                        Input Layer                               │
│                      (winit Event)                               │
└─────────────────────────────┬───────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                     Tracing Layer                                │
│  ┌─────────────┐  ┌──────────────┐  ┌────────────────────────┐  │
│  │ Msg Tracer  │  │ State Snap   │  │ Invariant Checker      │  │
│  │             │  │              │  │                        │  │
│  │ - log msg   │  │ - before     │  │ - assert after update  │  │
│  │ - span ctx  │  │ - after      │  │ - context annotation   │  │
│  └─────────────┘  └──────────────┘  └────────────────────────┘  │
└─────────────────────────────┬───────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Elm Architecture                              │
│                                                                  │
│     Msg ───────► update() ───────► Model ───────► Cmd           │
│                                                                  │
└─────────────────────────────┬───────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                     Debug Output                                 │
│  ┌─────────────┐  ┌──────────────┐  ┌────────────────────────┐  │
│  │ In-Editor   │  │ Structured   │  │ Console                │  │
│  │ Overlay     │  │ Log File     │  │ (RUST_LOG)             │  │
│  └─────────────┘  └──────────────┘  └────────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
```

---

## Implementation Plan

### Phase 1: Replace `log` with `tracing`

**Dependencies to add:**

```toml
# Cargo.toml
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "fmt"] }
```

**New file: `src/tracing.rs`**

```rust
//! Debug tracing infrastructure for development diagnostics
//!
//! Provides structured logging with scoped filtering for debugging
//! multi-cursor, selection, and state transition issues.

use tracing_subscriber::{fmt, prelude::*, EnvFilter};

/// Initialize tracing subscriber
///
/// Configure via RUST_LOG env var:
/// - `RUST_LOG=debug` - all debug logs
/// - `RUST_LOG=cursor=trace,selection=debug` - scoped filtering
/// - `RUST_LOG=token::update=debug` - module-level filtering
pub fn init() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("warn"));

    tracing_subscriber::registry()
        .with(fmt::layer().with_target(true).with_line_number(true))
        .with(filter)
        .init();
}
```

**Migration**: Replace `log::debug!` → `tracing::debug!` etc.

---

### Phase 2: State Snapshots

**Add to `src/tracing.rs`:**

```rust
use crate::model::EditorState;

/// Lightweight snapshot of cursor/selection state for diffing
#[derive(Debug, Clone)]
pub struct CursorSnapshot {
    pub cursor_count: usize,
    pub active_idx: usize,
    pub cursors: Vec<CursorInfo>,
}

#[derive(Debug, Clone)]
pub struct CursorInfo {
    pub line: usize,
    pub column: usize,
    pub anchor: (usize, usize),
    pub head: (usize, usize),
    pub selection_empty: bool,
}

impl CursorSnapshot {
    pub fn from_editor(editor: &EditorState) -> Self {
        Self {
            cursor_count: editor.cursors.len(),
            active_idx: editor.active_cursor_index,
            cursors: editor.cursors.iter()
                .zip(&editor.selections)
                .map(|(c, s)| CursorInfo {
                    line: c.line,
                    column: c.column,
                    anchor: (s.anchor.line, s.anchor.column),
                    head: (s.head.line, s.head.column),
                    selection_empty: s.is_empty(),
                })
                .collect(),
        }
    }

    /// Generate a diff description between two snapshots
    pub fn diff(&self, other: &CursorSnapshot) -> Option<String> {
        if self.cursor_count != other.cursor_count {
            return Some(format!(
                "cursor count: {} → {}",
                self.cursor_count, other.cursor_count
            ));
        }

        let mut changes = Vec::new();
        for (i, (before, after)) in self.cursors.iter().zip(&other.cursors).enumerate() {
            if before.line != after.line || before.column != after.column {
                changes.push(format!(
                    "#{}: ({},{}) → ({},{})",
                    i, before.line, before.column, after.line, after.column
                ));
            }
        }

        if changes.is_empty() {
            None
        } else {
            Some(changes.join("; "))
        }
    }
}
```

---

### Phase 3: Instrumented Update Wrapper

**Modify `src/update/mod.rs`:**

```rust
use tracing::{debug, span, Level};
use crate::tracing::CursorSnapshot;

/// Wrapper that traces message processing with before/after state
pub fn update_traced(model: &mut AppModel, msg: Msg) -> Cmd {
    let _span = span!(
        Level::DEBUG,
        "update",
        msg_type = ?std::mem::discriminant(&msg)
    ).entered();

    // Capture before state
    let before = model.focused_editor().map(CursorSnapshot::from_editor);

    debug!(target: "message", ?msg, "processing");

    // Actual update
    let cmd = update(model, msg.clone());

    // Capture after state and diff
    if let (Some(before), Some(editor)) = (before, model.focused_editor()) {
        let after = CursorSnapshot::from_editor(editor);
        if let Some(diff) = before.diff(&after) {
            debug!(target: "cursor", %diff, "state changed");
        }
    }

    // Assert invariants after update
    #[cfg(debug_assertions)]
    if let Some(editor) = model.focused_editor() {
        editor.assert_invariants_with_context(&format!("{:?}", msg));
    }

    cmd
}
```

---

### Phase 4: Scoped Tracing Helpers

**Add to update functions:**

```rust
// src/update/editor.rs
use tracing::{debug, span, Level};

fn handle_move_cursor(editor: &mut EditorState, doc: &Document, dir: Direction) {
    let _span = span!(Level::DEBUG, "cursor.move", ?dir).entered();

    let before = (editor.active_cursor().line, editor.active_cursor().column);

    match dir {
        Direction::Up => editor.move_all_cursors_up(doc),
        Direction::Down => editor.move_all_cursors_down(doc),
        Direction::Left => editor.move_all_cursors_left(doc),
        Direction::Right => editor.move_all_cursors_right(doc),
    }

    let after = editor.active_cursor();
    debug!(
        target: "cursor",
        from_line = before.0,
        from_col = before.1,
        to_line = after.line,
        to_col = after.column,
        cursor_count = editor.cursor_count(),
        "moved"
    );
}

fn handle_add_cursor(editor: &mut EditorState, line: usize, column: usize) {
    let _span = span!(Level::DEBUG, "cursor.add", line, column).entered();

    let before_count = editor.cursor_count();
    editor.add_cursor_at(line, column);
    let after_count = editor.cursor_count();

    debug!(
        target: "cursor",
        before_count,
        after_count,
        active_idx = editor.active_cursor_index,
        "cursor added"
    );
}
```

**Target naming convention:**

- `cursor` - cursor position changes
- `cursor.add` / `cursor.remove` - multi-cursor operations
- `selection` - selection state changes
- `selection.extend` / `selection.collapse` - selection modifications
- `viewport` - scroll and viewport changes
- `document` - text edits
- `message` - raw message logging

---

### Phase 5: Enhanced Invariant Assertions

**Modify `src/model/editor.rs`:**

```rust
impl EditorState {
    /// Assert cursor/selection invariants with detailed context
    #[cfg(debug_assertions)]
    pub fn assert_invariants_with_context(&self, context: &str) {
        // Must have at least one cursor
        assert!(
            !self.cursors.is_empty(),
            "[{context}] Editor must have at least one cursor"
        );

        // Cursor and selection counts must match
        assert_eq!(
            self.cursors.len(),
            self.selections.len(),
            "[{context}] Cursor count ({}) != selection count ({})",
            self.cursors.len(),
            self.selections.len()
        );

        // Active cursor index must be valid
        assert!(
            self.active_cursor_index < self.cursors.len(),
            "[{context}] Active cursor index {} out of bounds (count: {})",
            self.active_cursor_index,
            self.cursors.len()
        );

        // Each cursor position must match its selection head
        for (i, (cursor, selection)) in self.cursors.iter().zip(&self.selections).enumerate() {
            let cursor_pos = cursor.to_position();
            assert_eq!(
                cursor_pos,
                selection.head,
                "[{context}] Cursor {i} position {cursor_pos:?} != selection head {:?}",
                selection.head
            );
        }

        // Cursors must be sorted by position (no duplicates)
        for (i, window) in self.cursors.windows(2).enumerate() {
            let prev = (window[0].line, window[0].column);
            let curr = (window[1].line, window[1].column);
            assert!(
                prev < curr,
                "[{context}] Cursors not sorted at index {i}: {prev:?} >= {curr:?}"
            );
        }
    }

    #[cfg(not(debug_assertions))]
    #[inline]
    pub fn assert_invariants_with_context(&self, _context: &str) {}
}
```

---

### Phase 6: Debug Overlay Panel

**New file: `src/debug_overlay.rs`:**

```rust
//! In-editor debug overlay for real-time state visibility
//!
//! Toggle with F8 in debug builds.

use std::collections::VecDeque;
use std::time::Instant;
use crate::model::{AppModel, EditorState};

/// Maximum number of messages to retain in history
const MESSAGE_HISTORY_SIZE: usize = 50;

#[derive(Debug, Default)]
pub struct DebugOverlay {
    /// Whether the overlay is visible
    pub visible: bool,
    /// Show cursor position details
    pub show_cursors: bool,
    /// Show selection ranges
    pub show_selections: bool,
    /// Show recent message history
    pub show_messages: bool,
    /// Recent message history
    pub message_history: VecDeque<MessageEntry>,
}

#[derive(Debug, Clone)]
pub struct MessageEntry {
    pub timestamp: Instant,
    pub msg_type: String,
    pub cursor_diff: Option<String>,
}

impl DebugOverlay {
    pub fn new() -> Self {
        Self {
            visible: false,
            show_cursors: true,
            show_selections: true,
            show_messages: true,
            message_history: VecDeque::with_capacity(MESSAGE_HISTORY_SIZE),
        }
    }

    /// Toggle overlay visibility
    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    /// Record a message in history
    pub fn record_message(&mut self, msg_type: String, cursor_diff: Option<String>) {
        if self.message_history.len() >= MESSAGE_HISTORY_SIZE {
            self.message_history.pop_front();
        }
        self.message_history.push_back(MessageEntry {
            timestamp: Instant::now(),
            msg_type,
            cursor_diff,
        });
    }

    /// Generate overlay text lines for rendering
    pub fn render_lines(&self, model: &AppModel) -> Vec<String> {
        if !self.visible {
            return Vec::new();
        }

        let mut lines = vec!["─── DEBUG OVERLAY (F8 to hide) ───".to_string()];

        if let Some(editor) = model.focused_editor() {
            if self.show_cursors {
                lines.push(String::new());
                lines.extend(self.render_cursor_info(editor));
            }

            if self.show_selections {
                lines.push(String::new());
                lines.extend(self.render_selection_info(editor));
            }
        }

        if self.show_messages && !self.message_history.is_empty() {
            lines.push(String::new());
            lines.push("Recent Messages:".to_string());
            for entry in self.message_history.iter().rev().take(10) {
                let age_ms = entry.timestamp.elapsed().as_millis();
                let diff_str = entry.cursor_diff.as_deref().unwrap_or("-");
                lines.push(format!("  [{:>4}ms] {} → {}", age_ms, entry.msg_type, diff_str));
            }
        }

        lines
    }

    fn render_cursor_info(&self, editor: &EditorState) -> Vec<String> {
        let mut lines = vec![
            format!("Cursors: {} (active: #{})", editor.cursor_count(), editor.active_cursor_index)
        ];

        for (i, cursor) in editor.cursors.iter().enumerate() {
            let marker = if i == editor.active_cursor_index { "→" } else { " " };
            let desired = cursor.desired_column
                .map(|c| format!(" (desired: {})", c))
                .unwrap_or_default();
            lines.push(format!(
                "  {} #{}: L{}:C{}{}",
                marker, i, cursor.line, cursor.column, desired
            ));
        }

        lines
    }

    fn render_selection_info(&self, editor: &EditorState) -> Vec<String> {
        let mut lines = vec!["Selections:".to_string()];

        for (i, sel) in editor.selections.iter().enumerate() {
            let status = if sel.is_empty() { "empty" } else { "active" };
            let reversed = if sel.is_reversed() { " [rev]" } else { "" };
            lines.push(format!(
                "  #{}: ({},{})→({},{}) [{}]{}",
                i,
                sel.anchor.line, sel.anchor.column,
                sel.head.line, sel.head.column,
                status, reversed
            ));
        }

        lines
    }
}
```

---

### Phase 7: Integration

**Modify `src/main.rs`:**

```rust
mod tracing;

fn main() -> anyhow::Result<()> {
    // Initialize tracing (respects RUST_LOG env var)
    tracing::init();

    // ... rest of main
}
```

**Add to `AppModel`:**

```rust
pub struct AppModel {
    // ... existing fields

    /// Debug overlay state (debug builds only)
    #[cfg(debug_assertions)]
    pub debug_overlay: DebugOverlay,
}
```

**Add hotkey handling in input.rs:**

```rust
// F8 - Toggle debug overlay
KeyCode::F8 if cfg!(debug_assertions) => {
    model.debug_overlay.toggle();
    Cmd::Redraw
}
```

---

## File Structure

```
src/
├── tracing.rs          # NEW: Tracing setup, CursorSnapshot, instrumentation
├── debug_overlay.rs    # NEW: In-editor debug panel
├── debug_dump.rs       # EXISTING: F7 state dump (keep as-is)
├── model/
│   └── editor.rs       # MODIFY: Add assert_invariants_with_context
├── update/
│   ├── mod.rs          # MODIFY: Add update_traced wrapper
│   └── editor.rs       # MODIFY: Add span instrumentation
├── input.rs            # MODIFY: Add F8 hotkey
└── main.rs             # MODIFY: Call tracing::init()
```

---

## Usage Examples

### Console Filtering

```bash
# All debug output
RUST_LOG=debug cargo run

# Only cursor operations
RUST_LOG=cursor=debug cargo run

# Cursor + selection, trace level
RUST_LOG=cursor=trace,selection=trace cargo run

# Only message flow
RUST_LOG=message=debug cargo run

# Module-level filtering
RUST_LOG=token::update::editor=debug cargo run
```

### Debug Hotkeys

| Key | Action                                  |
| --- | --------------------------------------- |
| F7  | Dump full state to JSON file (existing) |
| F8  | Toggle debug overlay panel              |
| F9  | (future) Toggle message history only    |

---

## Success Criteria

1. ✅ Can filter logs by subsystem (cursor, selection, etc.)
2. ✅ Before/after state visible for cursor operations
3. ✅ Invariant violations include context about triggering message
4. ✅ Real-time cursor/selection state visible in overlay
5. ✅ Zero runtime cost in release builds

---

## Resolved Issues

### ~~Message Type Names Show Discriminant Instead of Variant Name~~

**Status**: ✅ Resolved (2025-12-08)  
**Location**: `src/update/mod.rs:93-109` (`msg_type_name()` function)  
**Solution**: Option A (Debug format)

**Problem** (was): The implementation used `std::mem::discriminant()` which output opaque values like `Ui::Discriminant(1)`.

**Solution implemented**: Changed to use `{:?}` Debug formatting on the inner enum directly.

```rust
#[cfg(debug_assertions)]
fn msg_type_name(msg: &Msg) -> String {
    match msg {
        Msg::Editor(m) => format!("Editor::{:?}", m),
        Msg::Document(m) => format!("Document::{:?}", m),
        Msg::Ui(m) => format!("Ui::{:?}", m),
        Msg::Layout(m) => format!("Layout::{:?}", m),
        Msg::App(m) => format!("App::{:?}", m),
    }
}
```

**Output examples:**
```
msg=App::Resize(800, 600)
msg=Document::InsertChar('a')
msg=Editor::MoveCursor(Up)
msg=Layout::SplitFocused(Horizontal)
```

Note: Noisy periodic messages like `Ui::BlinkCursor` are automatically filtered from logs.

**Why Option A over alternatives:**
- Zero dependencies (vs strum crate)
- Zero maintenance (vs ~85 manual match arms)
- Includes variant arguments which are helpful for debugging multi-cursor/selection issues
- Greppable: `rg "Editor::MoveCursor"` works regardless of argument values

---

## References

- [Zed's zlog crate](https://github.com/zed-industries/zed/tree/main/crates/zlog) - Scoped logging with hierarchical context
- [Helix event hooks](https://github.com/helix-editor/helix/blob/master/docs/architecture.md) - Functional composition for debugging
- [Lapce tracing](https://github.com/lapce/lapce/blob/master/lapce-app/src/tracing.rs) - `#[instrument]` macro usage
- [tracing crate docs](https://docs.rs/tracing/latest/tracing/)
