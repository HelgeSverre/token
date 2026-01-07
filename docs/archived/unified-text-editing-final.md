# Unified Text Editing System: Final Implementation Plan

A comprehensive guide to consolidating text editing functionality across all input contexts in the Token editor, combining the best architectural decisions with complete implementation details.

> **Target Audience:** AI agents and developers implementing this refactoring.
> This document provides the complete architectural vision, data structures, implementation details, and migration strategy.

---

## Implementation Progress (Updated 2025-12-19)

| Phase | Description | Status |
|-------|-------------|--------|
| **Phase 1** | Core Data Model (`src/editable/` module) | ✅ Complete |
| **Phase 2** | Message and Update System (`TextEditMsg`, `update_text_edit.rs`) | ✅ Complete |
| **Phase 3** | Rendering Consolidation (`TextFieldRenderer`) | ✅ Complete |
| **Phase 4a-c** | Modal States Migration (CommandPalette, GotoLine, FindReplace) | ✅ Complete |
| **Phase 4d** | Wire `update_text_edit()` to modal EditableStates | ✅ Complete |
| **Phase 4e** | Modal rendering with TextFieldRenderer | ✅ Complete |
| **Phase 4f** | Full keyboard navigation for modals | ✅ Complete |
| **Milestone 3** | CSV Cell Migration to EditableState | ✅ Complete |
| **Milestone 6** | Main Editor Bridge (`bridge_text_edit_to_editor()`) | ✅ Complete |
| **Milestone 8** | Cleanup and Documentation | ✅ Complete |

---

## Table of Contents

1. [Executive Summary](#1-executive-summary)
2. [Current State Analysis](#2-current-state-analysis)
3. [Target Architecture](#3-target-architecture)
4. [Phase 1: Core Data Model](#4-phase-1-core-data-model)
5. [Phase 2: Message and Update System](#5-phase-2-message-and-update-system)
6. [Phase 3: Rendering Consolidation](#6-phase-3-rendering-consolidation)
7. [Phase 4: Migration Strategy](#7-phase-4-migration-strategy)
8. [Implementation Order](#8-implementation-order)
9. [File Change Summary](#9-file-change-summary)
10. [Testing Strategy](#10-testing-strategy)

**Appendices:**

- [A: Key Invariants](#appendix-a-key-invariants)
- [B: Word Boundary Detection](#appendix-b-word-boundary-detection)
- [C: Keyboard Shortcut Matrix](#appendix-c-keyboard-shortcut-matrix)
- [D: Message Type Mappings](#appendix-d-message-type-mappings)
- [E: Critical Files Reference](#appendix-e-critical-files-reference)
- [F: Manual Testing Checklist](#appendix-f-manual-testing-checklist)

---

## 1. Executive Summary

### 1.1 Problem Statement

The codebase has **5 separate text editing implementations** with duplicated logic and inconsistent behavior:

| Context | Buffer | Cursor Model | Selection | Undo/Redo | Word Ops | Issues |
|---------|--------|--------------|-----------|-----------|----------|--------|
| Main Editor | `Rope` | Multi-cursor (line/col) | Full | Full history | Yes | Reference impl |
| Command Palette | `String` | End-only implicit | No | No | Partial* | No cursor nav |
| Go to Line | `String` | End-only implicit | No | No | No | Missing features |
| Find/Replace | `String` | End-only implicit | No | No | No | Missing features |
| CSV Cell | `String` | Byte offset | No | Cancel only | No | Different model |

*\* Uses different word boundary logic than main editor*

### 1.2 Solution

Create a unified `src/editable/` module with:

| Component | Purpose |
|-----------|---------|
| **`TextBuffer` trait** | Abstract over `Rope` and `String` backends |
| **`TextBufferMut` trait** | Mutation operations (extends `TextBuffer`) |
| **`EditableState<B>`** | Unified cursor, selection, and history management |
| **`EditConstraints`** | Context-specific restrictions |
| **`TextEditMsg`** | Unified editing message type with `MoveTarget` |
| **`EditContext`** | Identifies which input area is being edited |
| **`TextFieldRenderer`** | Unified text field rendering |

### 1.3 User Requirements

- ✅ Full undo/redo in ALL contexts (configurable)
- ✅ Selection support (Shift+Arrow) in ALL contexts
- ✅ Unified rendering AND editing logic
- ✅ Include Find/Replace and Go to Line inputs
- ✅ Single-cursor constraint in modal/cell contexts
- ✅ Consistent word boundary detection (`CharType`)
- ✅ Proper cursor navigation in all inputs

### 1.4 Key Design Decisions

| Decision | Rationale |
|----------|-----------|
| **Dedicated `src/editable/` module** | Cleaner organization, easier to find/test/maintain |
| **Split `TextBuffer`/`TextBufferMut` traits** | Allows read-only access for rendering |
| **Line/column cursors (not byte offsets)** | Matches existing `EditorState`, avoids conversions |
| **`MoveTarget` enum** | Reduces message duplication (movement + selection share targets) |
| **`EditContext` enum** | Explicit routing, clear message dispatch |
| **Bridge migration for main editor** | Safer incremental approach |

---

## 2. Current State Analysis

### 2.1 Main Editor Implementation

**Files:**
| File | Lines | Purpose |
|------|-------|---------|
| `src/model/editor.rs` | ~1291 | `Cursor`, `Selection`, `EditorState` |
| `src/model/document.rs` | ~299 | `Document`, `EditOperation`, undo stacks |
| `src/update/editor.rs` | ~1263 | Cursor movement, selection operations |
| `src/update/document.rs` | ~2007 | Text editing, clipboard, undo/redo |

**Key Structures:**
```rust
// src/model/editor.rs
pub struct Position {
    pub line: usize,
    pub column: usize,
}

pub struct Cursor {
    pub line: usize,
    pub column: usize,
    pub desired_column: Option<usize>,  // For vertical movement
}

pub struct Selection {
    pub anchor: Position,  // Fixed point
    pub head: Position,    // Moves with cursor
}

pub struct EditorState {
    pub cursors: Vec<Cursor>,
    pub selections: Vec<Selection>,
    pub active_cursor_index: usize,
    pub viewport: Viewport,
    // ...
}

// src/model/document.rs
pub enum EditOperation {
    Insert { position: usize, text: String, cursor_before: Cursor, cursor_after: Cursor },
    Delete { position: usize, text: String, cursor_before: Cursor, cursor_after: Cursor },
    Replace { position: usize, deleted_text: String, inserted_text: String, cursor_before: Cursor, cursor_after: Cursor },
    Batch { operations: Vec<EditOperation>, cursors_before: Vec<Cursor>, cursors_after: Vec<Cursor> },
}
```

### 2.2 Modal Input Implementation

**Files:**
| File | Lines | Purpose |
|------|-------|---------|
| `src/model/ui.rs` | 56-135 | `CommandPaletteState`, `GotoLineState`, `FindReplaceState` |
| `src/update/ui.rs` | 15-25 | `delete_word_backward()` (separate implementation!) |
| `src/update/ui.rs` | 110-349 | `update_modal()` |
| `src/runtime/input.rs` | 202-260 | `handle_modal_key()` |

**Key Structures:**
```rust
pub struct CommandPaletteState {
    pub input: String,           // No cursor tracking!
    pub selected_index: usize,
}

pub struct GotoLineState {
    pub input: String,           // No cursor tracking!
}

pub struct FindReplaceState {
    pub query: String,           // No cursor tracking!
    pub replacement: String,
    pub focused_field: FindReplaceField,
    // ...
}
```

**Supported Operations:**
- `InsertChar` (append only)
- `DeleteBackward` (pop from end)
- `DeleteWordBackward` (custom implementation, different from main editor!)
- ❌ No cursor positioning
- ❌ No selection
- ❌ No word movement (stubbed)

### 2.3 CSV Cell Editor Implementation

**Files:**
| File | Lines | Purpose |
|------|-------|---------|
| `src/csv/model.rs` | 54-154 | `CellEditState` |
| `src/update/csv.rs` | ~548 | Cell editing and navigation |
| `src/runtime/input.rs` | 266-336 | `handle_csv_edit_key()` |

**Key Structures:**
```rust
pub struct CellEditState {
    pub position: CellPosition,
    pub buffer: String,
    pub cursor: usize,        // Byte offset (not character position!)
    pub original: String,     // For cancel/undo
}
```

**Supported Operations:**
- `insert_char`, `delete_backward`, `delete_forward`
- `cursor_left`, `cursor_right`, `cursor_home`, `cursor_end`
- ❌ No selection
- ❌ No word operations
- ❌ No undo (only cancel to original)

### 2.4 Rendering Implementation

**Files:**
| File | Lines | Purpose |
|------|-------|---------|
| `src/view/mod.rs` | ~1835 | Main renderer |
| `src/view/frame.rs` | ~502 | `Frame`, `TextPainter` primitives |

**Separate Rendering Paths:**
| Context | Function | Lines |
|---------|----------|-------|
| Main Editor | `render_text_area()` | 615-905 |
| Modals | `render_modals()` → inline | 1410-1425 |
| CSV Cell | `render_csv_cell_editor()` | 1137-1208 |

### 2.5 Feature Matrix: Current State

| Feature | Editor | Palette | GoToLine | Find/Replace | CSV Cell |
|---------|--------|---------|----------|--------------|----------|
| Multi-line | ✅ | ❌ | ❌ | ❌ | ❌ |
| Multi-cursor | ✅ | ❌ | ❌ | ❌ | ❌ |
| Cursor left/right | ✅ | ❌ | ❌ | ❌ | ✅ |
| Cursor home/end | ✅ | ❌ | ❌ | ❌ | ✅ |
| Word movement | ✅ | ❌ | ❌ | ❌ | ❌ |
| Selection | ✅ | ❌ | ❌ | ❌ | ❌ |
| Select all | ✅ | ❌ | ❌ | ❌ | ❌ |
| Delete backward | ✅ | ✅ | ✅ | ✅ | ✅ |
| Delete forward | ✅ | ❌ | ❌ | ❌ | ✅ |
| Word delete back | ✅ | ✅* | ✅* | ✅* | ❌ |
| Word delete fwd | ✅ | ❌ | ❌ | ❌ | ❌ |
| Undo/Redo | ✅ | ❌ | ❌ | ❌ | ❌ |
| Cut/Copy/Paste | ✅ | ❌ | ❌ | ❌ | ❌ |

*\* Different implementation than main editor*

---

## 3. Target Architecture

### 3.1 Module Structure

```
src/editable/
├── mod.rs              # Module exports and re-exports
├── buffer.rs           # TextBuffer, TextBufferMut traits + implementations
├── cursor.rs           # Position, Cursor structs
├── selection.rs        # Selection struct and operations
├── history.rs          # EditOperation, EditHistory
├── constraints.rs      # EditConstraints, CharFilter
├── state.rs            # EditableState<B> main abstraction
├── context.rs          # EditContext enum
└── messages.rs         # TextEditMsg, MoveTarget enums

src/update/text_edit.rs     # update_text_edit() unified handler
src/view/text_field.rs      # TextFieldRenderer + TextFieldContent trait
```

### 3.2 Architecture Diagram

```
┌─────────────────────────────────────────────────────────────────────────┐
│                         src/editable/                                    │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  ┌─────────────┐ │
│  │ TextBuffer   │  │EditableState │  │ EditHistory  │  │EditConstraints│
│  │ TextBufferMut│  │  <B: Buffer> │  │ (undo/redo)  │  │(restrictions)│
│  └──────────────┘  └──────────────┘  └──────────────┘  └─────────────┘ │
│         │                  │                │                 │         │
│         └──────────────────┴────────────────┴─────────────────┘         │
│                                    │                                     │
│  ┌──────────────┐  ┌──────────────┐                                     │
│  │ EditContext  │  │ TextEditMsg  │                                     │
│  │ (routing ID) │  │ + MoveTarget │                                     │
│  └──────────────┘  └──────────────┘                                     │
└─────────────────────────────────────────────────────────────────────────┘
                                    │
                    ┌───────────────┴───────────────┐
                    ▼                               ▼
┌───────────────────────────────┐   ┌───────────────────────────────────┐
│   src/update/text_edit.rs     │   │      src/view/text_field.rs       │
│  ┌─────────────────────────┐  │   │  ┌─────────────────────────────┐  │
│  │ update_text_edit(       │  │   │  │ TextFieldRenderer           │  │
│  │   model,                │  │   │  │   ::render(frame, painter,  │  │
│  │   context: EditContext, │  │   │  │     content, bounds, opts)  │  │
│  │   msg: TextEditMsg      │  │   │  │                             │  │
│  │ ) -> Option<Cmd>        │  │   │  │ TextFieldContent trait      │  │
│  └─────────────────────────┘  │   │  │   for uniform access        │  │
└───────────────────────────────┘   └───────────────────────────────────┘
```

### 3.3 Data Flow

```
Keyboard Event
      │
      ▼
┌─────────────────────────────────────────────┐
│ src/runtime/input.rs                        │
│   determine_edit_context(model) → EditContext│
│   key_to_text_edit_msg(key, mods) → TextEditMsg│
└─────────────────────┬───────────────────────┘
                      │
                      ▼
┌─────────────────────────────────────────────┐
│ Msg::TextEdit(EditContext, TextEditMsg)     │
└─────────────────────┬───────────────────────┘
                      │
                      ▼
┌─────────────────────────────────────────────┐
│ src/update/text_edit.rs                     │
│   match context {                           │
│     EditContext::Editor(id) => ...,         │
│     EditContext::CommandPalette => ...,     │
│     EditContext::CsvCell(_) => ...,         │
│   }                                         │
│   apply to EditableState, record history    │
└─────────────────────┬───────────────────────┘
                      │
                      ▼
┌─────────────────────────────────────────────┐
│ Cmd::Redraw                                 │
└─────────────────────────────────────────────┘
```

### 3.4 Target Feature Matrix

| Feature | Editor | Palette | GoToLine | Find/Replace | CSV Cell |
|---------|--------|---------|----------|--------------|----------|
| Multi-line | ✅ | ❌ | ❌ | ❌ | ❌ |
| Multi-cursor | ✅ | ❌ | ❌ | ❌ | ❌ |
| Cursor left/right | ✅ | ✅ | ✅ | ✅ | ✅ |
| Cursor home/end | ✅ | ✅ | ✅ | ✅ | ✅ |
| Word movement | ✅ | ✅ | ✅ | ✅ | ✅ |
| Selection | ✅ | ✅ | ✅ | ✅ | ✅ |
| Select all | ✅ | ✅ | ✅ | ✅ | ✅ |
| Delete backward | ✅ | ✅ | ✅ | ✅ | ✅ |
| Delete forward | ✅ | ✅ | ✅ | ✅ | ✅ |
| Word delete back | ✅ | ✅ | ✅ | ✅ | ✅ |
| Word delete fwd | ✅ | ✅ | ✅ | ✅ | ✅ |
| Undo/Redo | ✅ | ✅ | ✅ | ✅ | ✅ |
| Cut/Copy/Paste | ✅ | ✅ | ✅ | ✅ | ✅ |
| Char filter | ❌ | ❌ | digits | ❌ | ❌ |

---

## 4. Phase 1: Core Data Model

### 4.1 TextBuffer Traits

```rust
// src/editable/buffer.rs

use std::borrow::Cow;
use std::ops::Range;

/// Read-only view into a text buffer for cursor navigation and rendering.
/// Abstracts over Rope (large files) and String (small inputs).
pub trait TextBuffer {
    /// Number of lines (always >= 1)
    fn line_count(&self) -> usize;
    
    /// Length of a specific line in characters (excluding newline)
    fn line_length(&self, line: usize) -> usize;
    
    /// Total length in characters
    fn len_chars(&self) -> usize;
    
    /// Total length in bytes
    fn len_bytes(&self) -> usize;
    
    /// Check if buffer is empty
    fn is_empty(&self) -> bool {
        self.len_chars() == 0
    }
    
    /// Get character at position, None if out of bounds
    fn char_at(&self, line: usize, column: usize) -> Option<char>;
    
    /// Get line content (without trailing newline)
    fn line(&self, line: usize) -> Option<Cow<'_, str>>;
    
    /// Convert (line, column) to byte offset
    fn position_to_offset(&self, line: usize, column: usize) -> usize;
    
    /// Convert byte offset to (line, column)
    fn offset_to_position(&self, offset: usize) -> (usize, usize);
    
    /// Get slice of text as String
    fn slice(&self, range: Range<usize>) -> String;
    
    /// Get full content as String (may be expensive for large buffers)
    fn to_string(&self) -> String;
    
    /// Column of first non-whitespace character on line (for smart Home)
    fn first_non_whitespace_column(&self, line: usize) -> usize;
    
    /// Column of last non-whitespace character on line
    fn last_non_whitespace_column(&self, line: usize) -> usize;
}

/// Mutable buffer operations. Extends TextBuffer.
pub trait TextBufferMut: TextBuffer {
    /// Insert text at byte offset
    fn insert(&mut self, offset: usize, text: &str);
    
    /// Insert single character at byte offset
    fn insert_char(&mut self, offset: usize, ch: char);
    
    /// Remove text in byte range
    fn remove(&mut self, range: Range<usize>);
    
    /// Replace text in range with new text (atomic operation)
    fn replace(&mut self, range: Range<usize>, text: &str) {
        self.remove(range.clone());
        self.insert(range.start, text);
    }
}
```

### 4.2 RopeBuffer Implementation

```rust
// src/editable/buffer.rs

use ropey::Rope;

/// TextBuffer implementation wrapping ropey::Rope.
/// Used for multi-line document editing with efficient operations on large files.
pub struct RopeBuffer {
    rope: Rope,
}

impl RopeBuffer {
    pub fn new() -> Self {
        Self { rope: Rope::new() }
    }
    
    pub fn from_str(s: &str) -> Self {
        Self { rope: Rope::from_str(s) }
    }
    
    /// Access the underlying Rope for rope-specific operations
    pub fn rope(&self) -> &Rope {
        &self.rope
    }
    
    pub fn rope_mut(&mut self) -> &mut Rope {
        &mut self.rope
    }
}

impl Default for RopeBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl TextBuffer for RopeBuffer {
    fn line_count(&self) -> usize {
        self.rope.len_lines().max(1)
    }
    
    fn line_length(&self, line: usize) -> usize {
        if line >= self.rope.len_lines() {
            return 0;
        }
        let line_slice = self.rope.line(line);
        let len = line_slice.len_chars();
        // Exclude trailing newline if present
        if len > 0 && line_slice.char(len - 1) == '\n' {
            len - 1
        } else {
            len
        }
    }
    
    fn len_chars(&self) -> usize {
        self.rope.len_chars()
    }
    
    fn len_bytes(&self) -> usize {
        self.rope.len_bytes()
    }
    
    fn char_at(&self, line: usize, column: usize) -> Option<char> {
        if line >= self.rope.len_lines() {
            return None;
        }
        let line_start = self.rope.line_to_char(line);
        let line_len = self.line_length(line);
        if column >= line_len {
            return None;
        }
        Some(self.rope.char(line_start + column))
    }
    
    fn line(&self, line: usize) -> Option<Cow<'_, str>> {
        if line >= self.rope.len_lines() {
            return None;
        }
        let line_slice = self.rope.line(line);
        let s = line_slice.to_string();
        // Strip trailing newline
        let trimmed = s.trim_end_matches(&['\n', '\r'][..]);
        Some(Cow::Owned(trimmed.to_string()))
    }
    
    fn position_to_offset(&self, line: usize, column: usize) -> usize {
        let line = line.min(self.line_count().saturating_sub(1));
        let line_start = self.rope.line_to_byte(line);
        
        // Convert column (chars) to bytes
        if let Some(line_text) = self.line(line) {
            let col_bytes: usize = line_text
                .chars()
                .take(column)
                .map(|c| c.len_utf8())
                .sum();
            line_start + col_bytes
        } else {
            line_start
        }
    }
    
    fn offset_to_position(&self, offset: usize) -> (usize, usize) {
        let offset = offset.min(self.len_bytes());
        let line = self.rope.byte_to_line(offset);
        let line_start = self.rope.line_to_byte(line);
        let col_bytes = offset - line_start;
        
        // Convert byte offset within line to character column
        if let Some(line_text) = self.line(line) {
            let column = line_text
                .char_indices()
                .take_while(|(byte_idx, _)| *byte_idx < col_bytes)
                .count();
            (line, column)
        } else {
            (line, 0)
        }
    }
    
    fn slice(&self, range: Range<usize>) -> String {
        let start = range.start.min(self.len_bytes());
        let end = range.end.min(self.len_bytes());
        let start_char = self.rope.byte_to_char(start);
        let end_char = self.rope.byte_to_char(end);
        self.rope.slice(start_char..end_char).to_string()
    }
    
    fn to_string(&self) -> String {
        self.rope.to_string()
    }
    
    fn first_non_whitespace_column(&self, line: usize) -> usize {
        self.line(line)
            .map(|l| {
                l.chars()
                    .take_while(|c| c.is_whitespace())
                    .count()
            })
            .unwrap_or(0)
    }
    
    fn last_non_whitespace_column(&self, line: usize) -> usize {
        self.line(line)
            .map(|l| {
                let trimmed = l.trim_end();
                trimmed.chars().count()
            })
            .unwrap_or(0)
    }
}

impl TextBufferMut for RopeBuffer {
    fn insert(&mut self, offset: usize, text: &str) {
        let offset = offset.min(self.len_bytes());
        let char_idx = self.rope.byte_to_char(offset);
        self.rope.insert(char_idx, text);
    }
    
    fn insert_char(&mut self, offset: usize, ch: char) {
        let offset = offset.min(self.len_bytes());
        let char_idx = self.rope.byte_to_char(offset);
        self.rope.insert_char(char_idx, ch);
    }
    
    fn remove(&mut self, range: Range<usize>) {
        let start = range.start.min(self.len_bytes());
        let end = range.end.min(self.len_bytes());
        if start >= end {
            return;
        }
        let start_char = self.rope.byte_to_char(start);
        let end_char = self.rope.byte_to_char(end);
        self.rope.remove(start_char..end_char);
    }
}
```

### 4.3 StringBuffer Implementation

```rust
// src/editable/buffer.rs

/// TextBuffer implementation wrapping String.
/// Used for single-line inputs (command palette, dialogs, CSV cells).
/// More efficient than Rope for small text.
pub struct StringBuffer {
    text: String,
}

impl StringBuffer {
    pub fn new() -> Self {
        Self { text: String::new() }
    }
    
    pub fn from_str(s: &str) -> Self {
        Self { text: s.to_string() }
    }
    
    /// Access the underlying String
    pub fn as_str(&self) -> &str {
        &self.text
    }
    
    /// Consume and return the String
    pub fn into_string(self) -> String {
        self.text
    }
}

impl Default for StringBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl TextBuffer for StringBuffer {
    fn line_count(&self) -> usize {
        1  // Single-line buffer
    }
    
    fn line_length(&self, line: usize) -> usize {
        if line == 0 {
            self.text.chars().count()
        } else {
            0
        }
    }
    
    fn len_chars(&self) -> usize {
        self.text.chars().count()
    }
    
    fn len_bytes(&self) -> usize {
        self.text.len()
    }
    
    fn char_at(&self, line: usize, column: usize) -> Option<char> {
        if line != 0 {
            return None;
        }
        self.text.chars().nth(column)
    }
    
    fn line(&self, line: usize) -> Option<Cow<'_, str>> {
        if line == 0 {
            Some(Cow::Borrowed(&self.text))
        } else {
            None
        }
    }
    
    fn position_to_offset(&self, _line: usize, column: usize) -> usize {
        // For single-line buffer, ignore line parameter
        self.text
            .chars()
            .take(column)
            .map(|c| c.len_utf8())
            .sum()
    }
    
    fn offset_to_position(&self, offset: usize) -> (usize, usize) {
        let offset = offset.min(self.text.len());
        let column = self.text[..offset].chars().count();
        (0, column)
    }
    
    fn slice(&self, range: Range<usize>) -> String {
        let start = range.start.min(self.text.len());
        let end = range.end.min(self.text.len());
        self.text[start..end].to_string()
    }
    
    fn to_string(&self) -> String {
        self.text.clone()
    }
    
    fn first_non_whitespace_column(&self, line: usize) -> usize {
        if line != 0 {
            return 0;
        }
        self.text.chars().take_while(|c| c.is_whitespace()).count()
    }
    
    fn last_non_whitespace_column(&self, line: usize) -> usize {
        if line != 0 {
            return 0;
        }
        self.text.trim_end().chars().count()
    }
}

impl TextBufferMut for StringBuffer {
    fn insert(&mut self, offset: usize, text: &str) {
        let offset = offset.min(self.text.len());
        self.text.insert_str(offset, text);
    }
    
    fn insert_char(&mut self, offset: usize, ch: char) {
        let offset = offset.min(self.text.len());
        self.text.insert(offset, ch);
    }
    
    fn remove(&mut self, range: Range<usize>) {
        let start = range.start.min(self.text.len());
        let end = range.end.min(self.text.len());
        if start < end {
            self.text.drain(start..end);
        }
    }
}
```

### 4.4 Position and Cursor

```rust
// src/editable/cursor.rs

/// A position in the text buffer (line and column, both 0-indexed).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Position {
    pub line: usize,
    pub column: usize,
}

impl Position {
    pub fn new(line: usize, column: usize) -> Self {
        Self { line, column }
    }
    
    pub fn zero() -> Self {
        Self { line: 0, column: 0 }
    }
}

/// A cursor in the text buffer with optional desired column for vertical movement.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Cursor {
    pub line: usize,
    pub column: usize,
    /// Desired column for vertical movement.
    /// When moving up/down through lines of varying length, this preserves
    /// the "intended" column position even when a shorter line is traversed.
    pub desired_column: Option<usize>,
}

impl Cursor {
    pub fn new(line: usize, column: usize) -> Self {
        Self {
            line,
            column,
            desired_column: None,
        }
    }
    
    pub fn at_position(pos: Position) -> Self {
        Self::new(pos.line, pos.column)
    }
    
    pub fn to_position(&self) -> Position {
        Position::new(self.line, self.column)
    }
    
    /// Clear desired column (call after horizontal movement)
    pub fn clear_desired_column(&mut self) {
        self.desired_column = None;
    }
    
    /// Set desired column to current column (call before vertical movement)
    pub fn set_desired_column(&mut self) {
        if self.desired_column.is_none() {
            self.desired_column = Some(self.column);
        }
    }
    
    /// Get the effective column for positioning (uses desired_column if set)
    pub fn effective_column(&self) -> usize {
        self.desired_column.unwrap_or(self.column)
    }
}

impl From<Position> for Cursor {
    fn from(pos: Position) -> Self {
        Self::at_position(pos)
    }
}

impl From<Cursor> for Position {
    fn from(cursor: Cursor) -> Self {
        cursor.to_position()
    }
}
```

### 4.5 Selection

```rust
// src/editable/selection.rs

use crate::editable::cursor::Position;
use crate::editable::buffer::TextBuffer;
use std::ops::Range;

/// A text selection with anchor (start point) and head (cursor position).
/// The anchor stays fixed while the head moves during selection extension.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Selection {
    /// Where the selection started (fixed point)
    pub anchor: Position,
    /// Where the cursor is (moving point)
    pub head: Position,
}

impl Selection {
    pub fn new(anchor: Position, head: Position) -> Self {
        Self { anchor, head }
    }
    
    /// Create a collapsed selection (cursor with no selection)
    pub fn collapsed(pos: Position) -> Self {
        Self { anchor: pos, head: pos }
    }
    
    /// Check if selection is empty (anchor == head)
    pub fn is_empty(&self) -> bool {
        self.anchor == self.head
    }
    
    /// Get the start position (minimum of anchor and head)
    pub fn start(&self) -> Position {
        if self.anchor <= self.head {
            self.anchor
        } else {
            self.head
        }
    }
    
    /// Get the end position (maximum of anchor and head)
    pub fn end(&self) -> Position {
        if self.anchor >= self.head {
            self.anchor
        } else {
            self.head
        }
    }
    
    /// Check if selection is reversed (head before anchor)
    pub fn is_reversed(&self) -> bool {
        self.head < self.anchor
    }
    
    /// Extend selection to new head position
    pub fn extend_to(&mut self, pos: Position) {
        self.head = pos;
    }
    
    /// Collapse selection to head position
    pub fn collapse(&mut self) {
        self.anchor = self.head;
    }
    
    /// Collapse selection to start position
    pub fn collapse_to_start(&mut self) {
        let start = self.start();
        self.anchor = start;
        self.head = start;
    }
    
    /// Collapse selection to end position
    pub fn collapse_to_end(&mut self) {
        let end = self.end();
        self.anchor = end;
        self.head = end;
    }
    
    /// Get the byte range of this selection in the buffer
    pub fn byte_range<B: TextBuffer>(&self, buffer: &B) -> Range<usize> {
        let start = buffer.position_to_offset(self.start().line, self.start().column);
        let end = buffer.position_to_offset(self.end().line, self.end().column);
        start..end
    }
    
    /// Get the selected text from the buffer
    pub fn get_text<B: TextBuffer>(&self, buffer: &B) -> String {
        if self.is_empty() {
            return String::new();
        }
        buffer.slice(self.byte_range(buffer))
    }
    
    /// Check if a position is within this selection
    pub fn contains(&self, pos: Position) -> bool {
        pos >= self.start() && pos < self.end()
    }
}
```

### 4.6 EditConstraints

```rust
// src/editable/constraints.rs

/// Character filter function type
pub type CharFilter = fn(char) -> bool;

/// Constraints that limit what operations are allowed in an editing context.
#[derive(Debug, Clone)]
pub struct EditConstraints {
    /// Allow multiple lines (Enter inserts newline vs confirms)
    pub allow_multiline: bool,
    
    /// Allow multiple cursors
    pub allow_multi_cursor: bool,
    
    /// Allow text selection
    pub allow_selection: bool,
    
    /// Enable undo/redo tracking
    pub enable_undo: bool,
    
    /// Maximum length in characters (None = unlimited)
    pub max_length: Option<usize>,
    
    /// Character filter (None = all characters allowed)
    pub char_filter: Option<CharFilter>,
}

impl Default for EditConstraints {
    fn default() -> Self {
        Self {
            allow_multiline: false,
            allow_multi_cursor: false,
            allow_selection: true,
            enable_undo: true,
            max_length: None,
            char_filter: None,
        }
    }
}

impl EditConstraints {
    /// Full editor constraints (all features enabled)
    pub fn editor() -> Self {
        Self {
            allow_multiline: true,
            allow_multi_cursor: true,
            allow_selection: true,
            enable_undo: true,
            max_length: None,
            char_filter: None,
        }
    }
    
    /// Single-line input constraints (command palette, find query)
    pub fn single_line() -> Self {
        Self {
            allow_multiline: false,
            allow_multi_cursor: false,
            allow_selection: true,
            enable_undo: true,
            max_length: Some(10_000),
            char_filter: None,
        }
    }
    
    /// Numeric input constraints (go-to-line)
    pub fn numeric() -> Self {
        Self {
            allow_multiline: false,
            allow_multi_cursor: false,
            allow_selection: true,
            enable_undo: true,
            max_length: Some(10),
            char_filter: Some(|c| c.is_ascii_digit()),
        }
    }
    
    /// CSV cell constraints
    pub fn csv_cell() -> Self {
        Self {
            allow_multiline: false,
            allow_multi_cursor: false,
            allow_selection: true,
            enable_undo: true,
            max_length: None,
            char_filter: None,
        }
    }
    
    /// Check if a character is allowed by the filter
    pub fn is_char_allowed(&self, ch: char) -> bool {
        match self.char_filter {
            Some(filter) => filter(ch),
            None => true,
        }
    }
    
    /// Check if adding more characters would exceed max length
    pub fn would_exceed_length(&self, current_len: usize, add_len: usize) -> bool {
        match self.max_length {
            Some(max) => current_len + add_len > max,
            None => false,
        }
    }
}
```

### 4.7 EditHistory

```rust
// src/editable/history.rs

use crate::editable::cursor::Cursor;

/// A single edit operation that can be undone/redone.
#[derive(Debug, Clone)]
pub struct EditOperation {
    /// Byte offset where the edit occurred
    pub offset: usize,
    /// Text that was deleted (empty for pure insert)
    pub deleted_text: String,
    /// Text that was inserted (empty for pure delete)
    pub inserted_text: String,
    /// Cursor states before the edit (for multi-cursor support)
    pub cursors_before: Vec<Cursor>,
    /// Cursor states after the edit
    pub cursors_after: Vec<Cursor>,
}

impl EditOperation {
    /// Create an insert-only operation
    pub fn insert(offset: usize, text: String, cursors_before: Vec<Cursor>, cursors_after: Vec<Cursor>) -> Self {
        Self {
            offset,
            deleted_text: String::new(),
            inserted_text: text,
            cursors_before,
            cursors_after,
        }
    }
    
    /// Create a delete-only operation
    pub fn delete(offset: usize, text: String, cursors_before: Vec<Cursor>, cursors_after: Vec<Cursor>) -> Self {
        Self {
            offset,
            deleted_text: text,
            inserted_text: String::new(),
            cursors_before,
            cursors_after,
        }
    }
    
    /// Create a replace operation (delete + insert)
    pub fn replace(
        offset: usize,
        deleted: String,
        inserted: String,
        cursors_before: Vec<Cursor>,
        cursors_after: Vec<Cursor>,
    ) -> Self {
        Self {
            offset,
            deleted_text: deleted,
            inserted_text: inserted,
            cursors_before,
            cursors_after,
        }
    }
    
    /// Create the inverse operation (for undo)
    pub fn invert(&self) -> Self {
        Self {
            offset: self.offset,
            deleted_text: self.inserted_text.clone(),
            inserted_text: self.deleted_text.clone(),
            cursors_before: self.cursors_after.clone(),
            cursors_after: self.cursors_before.clone(),
        }
    }
    
    /// Check if this is an insert-only operation
    pub fn is_insert(&self) -> bool {
        self.deleted_text.is_empty() && !self.inserted_text.is_empty()
    }
    
    /// Check if this is a delete-only operation
    pub fn is_delete(&self) -> bool {
        !self.deleted_text.is_empty() && self.inserted_text.is_empty()
    }
    
    /// Check if this is a replace operation
    pub fn is_replace(&self) -> bool {
        !self.deleted_text.is_empty() && !self.inserted_text.is_empty()
    }
}

/// Batched edit operations for multi-cursor edits (atomic undo/redo).
#[derive(Debug, Clone)]
pub struct BatchedEdit {
    pub operations: Vec<EditOperation>,
    pub cursors_before: Vec<Cursor>,
    pub cursors_after: Vec<Cursor>,
}

/// Edit history with undo/redo stacks.
#[derive(Debug, Clone, Default)]
pub struct EditHistory {
    undo_stack: Vec<EditOperation>,
    redo_stack: Vec<EditOperation>,
    /// Maximum number of operations to keep
    max_size: usize,
}

impl EditHistory {
    pub fn new() -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            max_size: 1000,
        }
    }
    
    pub fn with_max_size(max_size: usize) -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            max_size,
        }
    }
    
    /// Push a new operation (clears redo stack)
    pub fn push(&mut self, operation: EditOperation) {
        self.redo_stack.clear();
        self.undo_stack.push(operation);
        
        // Trim if over max size
        if self.undo_stack.len() > self.max_size {
            self.undo_stack.remove(0);
        }
    }
    
    /// Pop an operation for undo (moves to redo stack)
    pub fn pop_undo(&mut self) -> Option<EditOperation> {
        let op = self.undo_stack.pop()?;
        self.redo_stack.push(op.clone());
        Some(op)
    }
    
    /// Pop an operation for redo (moves to undo stack)
    pub fn pop_redo(&mut self) -> Option<EditOperation> {
        let op = self.redo_stack.pop()?;
        self.undo_stack.push(op.clone());
        Some(op)
    }
    
    /// Check if undo is available
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }
    
    /// Check if redo is available
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }
    
    /// Clear all history
    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
    }
    
    /// Get number of undoable operations
    pub fn undo_count(&self) -> usize {
        self.undo_stack.len()
    }
    
    /// Get number of redoable operations
    pub fn redo_count(&self) -> usize {
        self.redo_stack.len()
    }
}
```

### 4.8 EditableState

```rust
// src/editable/state.rs

use crate::editable::buffer::{TextBuffer, TextBufferMut};
use crate::editable::cursor::{Cursor, Position};
use crate::editable::selection::Selection;
use crate::editable::history::{EditHistory, EditOperation};
use crate::editable::constraints::EditConstraints;
use crate::util::text::{char_type, CharType};

/// Main abstraction for editable text with cursors, selections, and history.
/// Generic over the buffer type to support both Rope and String.
pub struct EditableState<B: TextBuffer> {
    /// The text buffer
    pub buffer: B,
    
    /// Cursor positions (always at least one)
    pub cursors: Vec<Cursor>,
    
    /// Selections (parallel to cursors, selection[i].head == cursor[i].to_position())
    pub selections: Vec<Selection>,
    
    /// Index of the active cursor
    pub active_cursor: usize,
    
    /// Editing constraints
    pub constraints: EditConstraints,
    
    /// Edit history (if undo enabled)
    history: EditHistory,
}

impl<B: TextBuffer + TextBufferMut> EditableState<B> {
    pub fn new(buffer: B, constraints: EditConstraints) -> Self {
        let pos = Position::zero();
        Self {
            buffer,
            cursors: vec![Cursor::new(0, 0)],
            selections: vec![Selection::collapsed(pos)],
            active_cursor: 0,
            constraints,
            history: EditHistory::new(),
        }
    }
    
    // === Accessors ===
    
    /// Get the primary cursor
    pub fn cursor(&self) -> &Cursor {
        &self.cursors[self.active_cursor]
    }
    
    /// Get the primary cursor mutably
    pub fn cursor_mut(&mut self) -> &mut Cursor {
        &mut self.cursors[self.active_cursor]
    }
    
    /// Get the primary selection
    pub fn selection(&self) -> &Selection {
        &self.selections[self.active_cursor]
    }
    
    /// Get the primary selection mutably
    pub fn selection_mut(&mut self) -> &mut Selection {
        &mut self.selections[self.active_cursor]
    }
    
    /// Get text content
    pub fn text(&self) -> String {
        self.buffer.to_string()
    }
    
    /// Check if there's any non-empty selection
    pub fn has_selection(&self) -> bool {
        self.selections.iter().any(|s| !s.is_empty())
    }
    
    /// Check if there are multiple cursors
    pub fn has_multiple_cursors(&self) -> bool {
        self.cursors.len() > 1
    }
    
    // === Cursor Movement ===
    
    /// Move cursor left by one character
    pub fn move_left(&mut self, extend_selection: bool) {
        for i in 0..self.cursors.len() {
            let cursor = &mut self.cursors[i];
            
            if cursor.column > 0 {
                cursor.column -= 1;
            } else if cursor.line > 0 && self.constraints.allow_multiline {
                cursor.line -= 1;
                cursor.column = self.buffer.line_length(cursor.line);
            }
            
            cursor.clear_desired_column();
            self.update_selection(i, extend_selection);
        }
    }
    
    /// Move cursor right by one character
    pub fn move_right(&mut self, extend_selection: bool) {
        for i in 0..self.cursors.len() {
            let cursor = &mut self.cursors[i];
            let line_len = self.buffer.line_length(cursor.line);
            
            if cursor.column < line_len {
                cursor.column += 1;
            } else if cursor.line < self.buffer.line_count() - 1 && self.constraints.allow_multiline {
                cursor.line += 1;
                cursor.column = 0;
            }
            
            cursor.clear_desired_column();
            self.update_selection(i, extend_selection);
        }
    }
    
    /// Move cursor up by one line
    pub fn move_up(&mut self, extend_selection: bool) {
        if !self.constraints.allow_multiline {
            return;
        }
        
        for i in 0..self.cursors.len() {
            let cursor = &mut self.cursors[i];
            
            if cursor.line > 0 {
                cursor.set_desired_column();
                cursor.line -= 1;
                let line_len = self.buffer.line_length(cursor.line);
                cursor.column = cursor.effective_column().min(line_len);
            }
            
            self.update_selection(i, extend_selection);
        }
    }
    
    /// Move cursor down by one line
    pub fn move_down(&mut self, extend_selection: bool) {
        if !self.constraints.allow_multiline {
            return;
        }
        
        for i in 0..self.cursors.len() {
            let cursor = &mut self.cursors[i];
            
            if cursor.line < self.buffer.line_count() - 1 {
                cursor.set_desired_column();
                cursor.line += 1;
                let line_len = self.buffer.line_length(cursor.line);
                cursor.column = cursor.effective_column().min(line_len);
            }
            
            self.update_selection(i, extend_selection);
        }
    }
    
    /// Move cursor to start of line (or first non-whitespace with smart home)
    pub fn move_home(&mut self, extend_selection: bool, smart: bool) {
        for i in 0..self.cursors.len() {
            let cursor = &mut self.cursors[i];
            
            if smart {
                let first_non_ws = self.buffer.first_non_whitespace_column(cursor.line);
                cursor.column = if cursor.column == first_non_ws || cursor.column < first_non_ws {
                    0
                } else {
                    first_non_ws
                };
            } else {
                cursor.column = 0;
            }
            
            cursor.clear_desired_column();
            self.update_selection(i, extend_selection);
        }
    }
    
    /// Move cursor to end of line
    pub fn move_end(&mut self, extend_selection: bool) {
        for i in 0..self.cursors.len() {
            let cursor = &mut self.cursors[i];
            cursor.column = self.buffer.line_length(cursor.line);
            cursor.clear_desired_column();
            self.update_selection(i, extend_selection);
        }
    }
    
    /// Move cursor left by one word
    pub fn move_word_left(&mut self, extend_selection: bool) {
        for i in 0..self.cursors.len() {
            self.move_cursor_word_left_at(i);
            self.update_selection(i, extend_selection);
        }
    }
    
    /// Move cursor right by one word
    pub fn move_word_right(&mut self, extend_selection: bool) {
        for i in 0..self.cursors.len() {
            self.move_cursor_word_right_at(i);
            self.update_selection(i, extend_selection);
        }
    }
    
    fn move_cursor_word_left_at(&mut self, idx: usize) {
        let cursor = &mut self.cursors[idx];
        let offset = self.buffer.position_to_offset(cursor.line, cursor.column);
        
        if offset == 0 {
            return;
        }
        
        let text = self.buffer.slice(0..offset);
        let chars: Vec<char> = text.chars().collect();
        let mut i = chars.len();
        
        // Skip current char type
        if i > 0 {
            let current_type = char_type(chars[i - 1]);
            while i > 0 && char_type(chars[i - 1]) == current_type {
                i -= 1;
            }
        }
        
        let new_offset: usize = chars[..i].iter().map(|c| c.len_utf8()).sum();
        let (line, column) = self.buffer.offset_to_position(new_offset);
        cursor.line = line;
        cursor.column = column;
        cursor.clear_desired_column();
    }
    
    fn move_cursor_word_right_at(&mut self, idx: usize) {
        let cursor = &mut self.cursors[idx];
        let offset = self.buffer.position_to_offset(cursor.line, cursor.column);
        let len = self.buffer.len_bytes();
        
        if offset >= len {
            return;
        }
        
        let text = self.buffer.slice(offset..len);
        let chars: Vec<char> = text.chars().collect();
        let mut i = 0;
        
        // Skip current char type
        if !chars.is_empty() {
            let current_type = char_type(chars[0]);
            while i < chars.len() && char_type(chars[i]) == current_type {
                i += 1;
            }
        }
        
        let advance: usize = chars[..i].iter().map(|c| c.len_utf8()).sum();
        let (line, column) = self.buffer.offset_to_position(offset + advance);
        cursor.line = line;
        cursor.column = column;
        cursor.clear_desired_column();
    }
    
    /// Move cursor to start of document
    pub fn move_document_start(&mut self, extend_selection: bool) {
        for i in 0..self.cursors.len() {
            self.cursors[i].line = 0;
            self.cursors[i].column = 0;
            self.cursors[i].clear_desired_column();
            self.update_selection(i, extend_selection);
        }
    }
    
    /// Move cursor to end of document
    pub fn move_document_end(&mut self, extend_selection: bool) {
        for i in 0..self.cursors.len() {
            let last_line = self.buffer.line_count().saturating_sub(1);
            self.cursors[i].line = last_line;
            self.cursors[i].column = self.buffer.line_length(last_line);
            self.cursors[i].clear_desired_column();
            self.update_selection(i, extend_selection);
        }
    }
    
    // === Selection Operations ===
    
    fn update_selection(&mut self, cursor_idx: usize, extend: bool) {
        let cursor_pos = self.cursors[cursor_idx].to_position();
        
        if !self.constraints.allow_selection || !extend {
            // Collapse selection to cursor
            self.selections[cursor_idx] = Selection::collapsed(cursor_pos);
        } else {
            // Extend selection: anchor stays, head moves with cursor
            self.selections[cursor_idx].head = cursor_pos;
        }
    }
    
    /// Select all text
    pub fn select_all(&mut self) {
        if !self.constraints.allow_selection {
            return;
        }
        
        // Collapse to single selection covering all text
        let end_line = self.buffer.line_count().saturating_sub(1);
        let end_col = self.buffer.line_length(end_line);
        
        self.cursors = vec![Cursor::new(end_line, end_col)];
        self.selections = vec![Selection::new(
            Position::zero(),
            Position::new(end_line, end_col),
        )];
        self.active_cursor = 0;
    }
    
    /// Select the current word
    pub fn select_word(&mut self) {
        if !self.constraints.allow_selection {
            return;
        }
        
        // TODO: Implement word selection at cursor
    }
    
    /// Select the current line
    pub fn select_line(&mut self) {
        if !self.constraints.allow_selection {
            return;
        }
        
        for i in 0..self.cursors.len() {
            let line = self.cursors[i].line;
            let line_len = self.buffer.line_length(line);
            
            self.selections[i] = Selection::new(
                Position::new(line, 0),
                Position::new(line, line_len),
            );
            self.cursors[i].column = line_len;
        }
    }
    
    /// Collapse all cursors to the active one
    pub fn collapse_cursors(&mut self) {
        if self.cursors.len() > 1 {
            let cursor = self.cursors[self.active_cursor];
            let selection = self.selections[self.active_cursor];
            self.cursors = vec![cursor];
            self.selections = vec![selection];
            self.active_cursor = 0;
        }
    }
    
    // === Text Editing Operations ===
    
    /// Insert a character at cursor position(s)
    pub fn insert_char(&mut self, ch: char) -> bool {
        // Check constraints
        if !self.constraints.is_char_allowed(ch) {
            return false;
        }
        
        if ch == '\n' && !self.constraints.allow_multiline {
            return false;
        }
        
        // Check max length
        let selection_len: usize = self.selections.iter()
            .filter(|s| !s.is_empty())
            .map(|s| s.get_text(&self.buffer).chars().count())
            .sum();
        let current_len = self.buffer.len_chars() - selection_len;
        if self.constraints.would_exceed_length(current_len, 1) {
            return false;
        }
        
        // Record cursors before
        let cursors_before = self.cursors.clone();
        
        // Process cursors in reverse order to preserve offsets
        let mut indices: Vec<usize> = (0..self.cursors.len()).collect();
        indices.sort_by_key(|&i| {
            let pos = self.cursors[i].to_position();
            std::cmp::Reverse((pos.line, pos.column))
        });
        
        for &i in &indices {
            let selection = &self.selections[i];
            let cursor = &mut self.cursors[i];
            
            if !selection.is_empty() {
                // Delete selection first
                let range = selection.byte_range(&self.buffer);
                self.buffer.remove(range.clone());
                let (line, col) = self.buffer.offset_to_position(range.start);
                cursor.line = line;
                cursor.column = col;
            }
            
            // Insert character
            let offset = self.buffer.position_to_offset(cursor.line, cursor.column);
            self.buffer.insert_char(offset, ch);
            
            // Update cursor position
            if ch == '\n' {
                cursor.line += 1;
                cursor.column = 0;
            } else {
                cursor.column += 1;
            }
            cursor.clear_desired_column();
            
            // Collapse selection
            self.selections[i] = Selection::collapsed(cursor.to_position());
        }
        
        // Record history
        if self.constraints.enable_undo {
            let op = EditOperation::insert(
                0, // Simplified - full impl tracks actual offset
                ch.to_string(),
                cursors_before,
                self.cursors.clone(),
            );
            self.history.push(op);
        }
        
        true
    }
    
    /// Insert a string at cursor position(s)
    pub fn insert_text(&mut self, text: &str) -> bool {
        // Check character filter for all chars
        if !text.chars().all(|c| self.constraints.is_char_allowed(c)) {
            return false;
        }
        
        if text.contains('\n') && !self.constraints.allow_multiline {
            return false;
        }
        
        // Check max length
        let selection_len: usize = self.selections.iter()
            .filter(|s| !s.is_empty())
            .map(|s| s.get_text(&self.buffer).chars().count())
            .sum();
        let current_len = self.buffer.len_chars() - selection_len;
        if self.constraints.would_exceed_length(current_len, text.chars().count()) {
            return false;
        }
        
        let cursors_before = self.cursors.clone();
        
        let mut indices: Vec<usize> = (0..self.cursors.len()).collect();
        indices.sort_by_key(|&i| {
            let pos = self.cursors[i].to_position();
            std::cmp::Reverse((pos.line, pos.column))
        });
        
        for &i in &indices {
            let selection = &self.selections[i];
            let cursor = &mut self.cursors[i];
            
            if !selection.is_empty() {
                let range = selection.byte_range(&self.buffer);
                self.buffer.remove(range.clone());
                let (line, col) = self.buffer.offset_to_position(range.start);
                cursor.line = line;
                cursor.column = col;
            }
            
            let offset = self.buffer.position_to_offset(cursor.line, cursor.column);
            self.buffer.insert(offset, text);
            
            // Update cursor position (count newlines and chars after last newline)
            let newlines = text.chars().filter(|&c| c == '\n').count();
            if newlines > 0 {
                cursor.line += newlines;
                cursor.column = text.rsplit('\n').next().map(|s| s.chars().count()).unwrap_or(0);
            } else {
                cursor.column += text.chars().count();
            }
            cursor.clear_desired_column();
            
            self.selections[i] = Selection::collapsed(cursor.to_position());
        }
        
        if self.constraints.enable_undo {
            let op = EditOperation::insert(
                0,
                text.to_string(),
                cursors_before,
                self.cursors.clone(),
            );
            self.history.push(op);
        }
        
        true
    }
    
    /// Delete backward (backspace)
    pub fn delete_backward(&mut self) {
        let cursors_before = self.cursors.clone();
        
        let mut indices: Vec<usize> = (0..self.cursors.len()).collect();
        indices.sort_by_key(|&i| {
            let pos = self.cursors[i].to_position();
            std::cmp::Reverse((pos.line, pos.column))
        });
        
        for &i in &indices {
            let selection = &self.selections[i];
            let cursor = &mut self.cursors[i];
            
            if !selection.is_empty() {
                // Delete selection
                let range = selection.byte_range(&self.buffer);
                self.buffer.remove(range.clone());
                let (line, col) = self.buffer.offset_to_position(range.start);
                cursor.line = line;
                cursor.column = col;
            } else {
                // Delete character before cursor
                let offset = self.buffer.position_to_offset(cursor.line, cursor.column);
                if offset > 0 {
                    // Find previous character boundary
                    let text = self.buffer.slice(0..offset);
                    if let Some((byte_idx, _)) = text.char_indices().last() {
                        self.buffer.remove(byte_idx..offset);
                        let (line, col) = self.buffer.offset_to_position(byte_idx);
                        cursor.line = line;
                        cursor.column = col;
                    }
                }
            }
            
            cursor.clear_desired_column();
            self.selections[i] = Selection::collapsed(cursor.to_position());
        }
        
        if self.constraints.enable_undo {
            let op = EditOperation::delete(
                0,
                String::new(), // Simplified
                cursors_before,
                self.cursors.clone(),
            );
            self.history.push(op);
        }
    }
    
    /// Delete forward (delete key)
    pub fn delete_forward(&mut self) {
        let cursors_before = self.cursors.clone();
        
        let mut indices: Vec<usize> = (0..self.cursors.len()).collect();
        indices.sort_by_key(|&i| {
            let pos = self.cursors[i].to_position();
            std::cmp::Reverse((pos.line, pos.column))
        });
        
        for &i in &indices {
            let selection = &self.selections[i];
            let cursor = &mut self.cursors[i];
            
            if !selection.is_empty() {
                let range = selection.byte_range(&self.buffer);
                self.buffer.remove(range.clone());
                let (line, col) = self.buffer.offset_to_position(range.start);
                cursor.line = line;
                cursor.column = col;
            } else {
                let offset = self.buffer.position_to_offset(cursor.line, cursor.column);
                if offset < self.buffer.len_bytes() {
                    // Find next character boundary
                    let text = self.buffer.slice(offset..self.buffer.len_bytes());
                    if let Some((_, ch)) = text.char_indices().next() {
                        self.buffer.remove(offset..offset + ch.len_utf8());
                    }
                }
            }
            
            cursor.clear_desired_column();
            self.selections[i] = Selection::collapsed(cursor.to_position());
        }
        
        if self.constraints.enable_undo {
            let op = EditOperation::delete(
                0,
                String::new(),
                cursors_before,
                self.cursors.clone(),
            );
            self.history.push(op);
        }
    }
    
    /// Delete word backward
    pub fn delete_word_backward(&mut self) {
        let cursors_before = self.cursors.clone();
        
        let mut indices: Vec<usize> = (0..self.cursors.len()).collect();
        indices.sort_by_key(|&i| {
            let pos = self.cursors[i].to_position();
            std::cmp::Reverse((pos.line, pos.column))
        });
        
        for &i in &indices {
            let selection = &self.selections[i];
            let cursor = &mut self.cursors[i];
            
            if !selection.is_empty() {
                let range = selection.byte_range(&self.buffer);
                self.buffer.remove(range.clone());
                let (line, col) = self.buffer.offset_to_position(range.start);
                cursor.line = line;
                cursor.column = col;
            } else {
                let offset = self.buffer.position_to_offset(cursor.line, cursor.column);
                let word_start = self.find_word_start_before(offset);
                if word_start < offset {
                    self.buffer.remove(word_start..offset);
                    let (line, col) = self.buffer.offset_to_position(word_start);
                    cursor.line = line;
                    cursor.column = col;
                }
            }
            
            cursor.clear_desired_column();
            self.selections[i] = Selection::collapsed(cursor.to_position());
        }
        
        if self.constraints.enable_undo {
            let op = EditOperation::delete(
                0,
                String::new(),
                cursors_before,
                self.cursors.clone(),
            );
            self.history.push(op);
        }
    }
    
    /// Delete word forward
    pub fn delete_word_forward(&mut self) {
        let cursors_before = self.cursors.clone();
        
        let mut indices: Vec<usize> = (0..self.cursors.len()).collect();
        indices.sort_by_key(|&i| {
            let pos = self.cursors[i].to_position();
            std::cmp::Reverse((pos.line, pos.column))
        });
        
        for &i in &indices {
            let selection = &self.selections[i];
            let cursor = &mut self.cursors[i];
            
            if !selection.is_empty() {
                let range = selection.byte_range(&self.buffer);
                self.buffer.remove(range.clone());
                let (line, col) = self.buffer.offset_to_position(range.start);
                cursor.line = line;
                cursor.column = col;
            } else {
                let offset = self.buffer.position_to_offset(cursor.line, cursor.column);
                let word_end = self.find_word_end_after(offset);
                if word_end > offset {
                    self.buffer.remove(offset..word_end);
                    // Cursor stays at same position
                }
            }
            
            cursor.clear_desired_column();
            self.selections[i] = Selection::collapsed(cursor.to_position());
        }
        
        if self.constraints.enable_undo {
            let op = EditOperation::delete(
                0,
                String::new(),
                cursors_before,
                self.cursors.clone(),
            );
            self.history.push(op);
        }
    }
    
    fn find_word_start_before(&self, offset: usize) -> usize {
        if offset == 0 {
            return 0;
        }
        
        let text = self.buffer.slice(0..offset);
        let chars: Vec<char> = text.chars().collect();
        let mut i = chars.len();
        
        if i > 0 {
            let current_type = char_type(chars[i - 1]);
            while i > 0 && char_type(chars[i - 1]) == current_type {
                i -= 1;
            }
        }
        
        chars[..i].iter().map(|c| c.len_utf8()).sum()
    }
    
    fn find_word_end_after(&self, offset: usize) -> usize {
        let len = self.buffer.len_bytes();
        if offset >= len {
            return len;
        }
        
        let text = self.buffer.slice(offset..len);
        let chars: Vec<char> = text.chars().collect();
        let mut i = 0;
        
        if !chars.is_empty() {
            let current_type = char_type(chars[0]);
            while i < chars.len() && char_type(chars[i]) == current_type {
                i += 1;
            }
        }
        
        offset + chars[..i].iter().map(|c| c.len_utf8()).sum::<usize>()
    }
    
    // === Clipboard Operations ===
    
    /// Get selected text (for copy)
    pub fn selected_text(&self) -> String {
        let sel = self.selection();
        if sel.is_empty() {
            return String::new();
        }
        sel.get_text(&self.buffer)
    }
    
    /// Cut selected text (returns cut text for clipboard)
    pub fn cut(&mut self) -> String {
        let text = self.selected_text();
        if !text.is_empty() {
            self.delete_backward();
        }
        text
    }
    
    /// Copy selected text (returns text for clipboard)
    pub fn copy(&self) -> String {
        self.selected_text()
    }
    
    /// Paste text at cursor
    pub fn paste(&mut self, text: &str) -> bool {
        self.insert_text(text)
    }
    
    // === Undo/Redo ===
    
    /// Undo last operation
    pub fn undo(&mut self) -> bool {
        if !self.constraints.enable_undo {
            return false;
        }
        
        if let Some(op) = self.history.pop_undo() {
            // Apply inverse operation
            if !op.inserted_text.is_empty() {
                let end = op.offset + op.inserted_text.len();
                self.buffer.remove(op.offset..end);
            }
            if !op.deleted_text.is_empty() {
                self.buffer.insert(op.offset, &op.deleted_text);
            }
            
            // Restore cursors
            self.cursors = op.cursors_before;
            self.selections = self.cursors.iter()
                .map(|c| Selection::collapsed(c.to_position()))
                .collect();
            self.active_cursor = 0.min(self.cursors.len().saturating_sub(1));
            
            return true;
        }
        false
    }
    
    /// Redo last undone operation
    pub fn redo(&mut self) -> bool {
        if !self.constraints.enable_undo {
            return false;
        }
        
        if let Some(op) = self.history.pop_redo() {
            // Apply operation
            if !op.deleted_text.is_empty() {
                let end = op.offset + op.deleted_text.len();
                self.buffer.remove(op.offset..end);
            }
            if !op.inserted_text.is_empty() {
                self.buffer.insert(op.offset, &op.inserted_text);
            }
            
            // Restore cursors
            self.cursors = op.cursors_after;
            self.selections = self.cursors.iter()
                .map(|c| Selection::collapsed(c.to_position()))
                .collect();
            self.active_cursor = 0.min(self.cursors.len().saturating_sub(1));
            
            return true;
        }
        false
    }
    
    /// Check if undo is available
    pub fn can_undo(&self) -> bool {
        self.constraints.enable_undo && self.history.can_undo()
    }
    
    /// Check if redo is available
    pub fn can_redo(&self) -> bool {
        self.constraints.enable_undo && self.history.can_redo()
    }
}
```

---

## 5. Phase 2: Message and Update System

### 5.1 EditContext Enum

```rust
// src/editable/context.rs

use crate::model::EditorGroupId;
use crate::csv::model::CellPosition;

/// Identifies which editing context a message is for.
/// Used for routing TextEditMsg to the correct EditableState.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EditContext {
    /// Main document editor in a specific group
    Editor(EditorGroupId),
    /// Command palette input
    CommandPalette,
    /// Go-to-line dialog input
    GotoLine,
    /// Find query input
    FindQuery,
    /// Replace input in find/replace
    ReplaceQuery,
    /// CSV cell being edited
    CsvCell(CellPosition),
}

impl EditContext {
    /// Get the constraints for this context
    pub fn constraints(&self) -> EditConstraints {
        match self {
            EditContext::Editor(_) => EditConstraints::editor(),
            EditContext::CommandPalette => EditConstraints::single_line(),
            EditContext::GotoLine => EditConstraints::numeric(),
            EditContext::FindQuery => EditConstraints::single_line(),
            EditContext::ReplaceQuery => EditConstraints::single_line(),
            EditContext::CsvCell(_) => EditConstraints::csv_cell(),
        }
    }
    
    /// Check if this context is a modal input
    pub fn is_modal(&self) -> bool {
        matches!(
            self,
            EditContext::CommandPalette
                | EditContext::GotoLine
                | EditContext::FindQuery
                | EditContext::ReplaceQuery
        )
    }
}
```

### 5.2 MoveTarget Enum

```rust
// src/editable/messages.rs

/// Target for cursor movement operations.
/// Shared between Move and MoveWithSelection to reduce duplication.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveTarget {
    // Character movement
    Left,
    Right,
    
    // Line movement (multi-line only)
    Up,
    Down,
    
    // Line boundaries
    LineStart,
    LineEnd,
    LineStartSmart,  // First non-whitespace or column 0
    
    // Word movement
    WordLeft,
    WordRight,
    
    // Document boundaries
    DocumentStart,
    DocumentEnd,
    
    // Page movement (multi-line only)
    PageUp,
    PageDown,
}
```

### 5.3 TextEditMsg Enum

```rust
// src/editable/messages.rs

/// Unified message type for all text editing operations.
/// Replaces the fragmented EditorMsg/DocumentMsg/ModalMsg/CsvMsg edit variants.
#[derive(Debug, Clone, PartialEq)]
pub enum TextEditMsg {
    // === Movement ===
    /// Move cursor without affecting selection
    Move(MoveTarget),
    /// Move cursor and extend selection
    MoveWithSelection(MoveTarget),
    
    // === Insertion ===
    /// Insert a single character
    InsertChar(char),
    /// Insert a string (e.g., from paste)
    InsertText(String),
    /// Insert newline (multiline only, else ignored)
    InsertNewline,
    
    // === Deletion ===
    /// Delete character before cursor (Backspace)
    DeleteBackward,
    /// Delete character after cursor (Delete key)
    DeleteForward,
    /// Delete word before cursor (Ctrl/Option+Backspace)
    DeleteWordBackward,
    /// Delete word after cursor (Ctrl/Option+Delete)
    DeleteWordForward,
    /// Delete entire line(s) containing cursor(s)
    DeleteLine,
    
    // === Selection ===
    /// Select all text
    SelectAll,
    /// Select word at cursor
    SelectWord,
    /// Select line at cursor
    SelectLine,
    /// Collapse selection (if any) without moving cursor
    CollapseSelection,
    
    // === Multi-Cursor (editor only) ===
    /// Add cursor above current
    AddCursorAbove,
    /// Add cursor below current
    AddCursorBelow,
    /// Add cursor at next occurrence of selection
    AddCursorAtNextOccurrence,
    /// Add cursors at all occurrences of selection
    AddCursorsAtAllOccurrences,
    /// Collapse to single cursor
    CollapseCursors,
    
    // === Clipboard ===
    /// Copy selection to clipboard
    Copy,
    /// Cut selection to clipboard
    Cut,
    /// Paste from clipboard (text provided by caller)
    Paste(String),
    
    // === Undo/Redo ===
    /// Undo last edit
    Undo,
    /// Redo last undone edit
    Redo,
    
    // === Indentation ===
    /// Indent selected lines
    Indent,
    /// Unindent selected lines
    Unindent,
    
    // === Line Operations ===
    /// Duplicate current line or selection
    Duplicate,
    /// Move line(s) up
    MoveLineUp,
    /// Move line(s) down
    MoveLineDown,
}
```

### 5.4 Adding to Main Msg Enum

```rust
// src/messages.rs

use crate::editable::context::EditContext;
use crate::editable::messages::TextEditMsg;

pub enum Msg {
    // ... existing variants ...
    
    /// Unified text editing message
    TextEdit(EditContext, TextEditMsg),
    
    // ... rest of variants ...
}
```

### 5.5 Update Handler

```rust
// src/update/text_edit.rs

use crate::model::AppModel;
use crate::commands::Cmd;
use crate::editable::context::EditContext;
use crate::editable::messages::{TextEditMsg, MoveTarget};

/// Handle a TextEditMsg by routing to the appropriate EditableState.
pub fn update_text_edit(
    model: &mut AppModel,
    context: EditContext,
    msg: TextEditMsg,
) -> Option<Cmd> {
    match context {
        EditContext::Editor(group_id) => {
            update_editor_text(model, group_id, msg)
        }
        EditContext::CommandPalette => {
            update_modal_text(model, &msg, |ui| {
                ui.active_modal.as_mut()
                    .and_then(|m| m.as_command_palette_mut())
                    .map(|p| &mut p.editable)
            })
        }
        EditContext::GotoLine => {
            update_modal_text(model, &msg, |ui| {
                ui.active_modal.as_mut()
                    .and_then(|m| m.as_goto_line_mut())
                    .map(|g| &mut g.editable)
            })
        }
        EditContext::FindQuery => {
            update_modal_text(model, &msg, |ui| {
                ui.active_modal.as_mut()
                    .and_then(|m| m.as_find_replace_mut())
                    .filter(|f| f.focused_field == FindReplaceField::Query)
                    .map(|f| &mut f.query_editable)
            })
        }
        EditContext::ReplaceQuery => {
            update_modal_text(model, &msg, |ui| {
                ui.active_modal.as_mut()
                    .and_then(|m| m.as_find_replace_mut())
                    .filter(|f| f.focused_field == FindReplaceField::Replace)
                    .map(|f| &mut f.replace_editable)
            })
        }
        EditContext::CsvCell(cell_pos) => {
            update_csv_cell_text(model, cell_pos, msg)
        }
    }
}

fn update_modal_text<F>(
    model: &mut AppModel,
    msg: &TextEditMsg,
    get_editable: F,
) -> Option<Cmd>
where
    F: FnOnce(&mut UiState) -> Option<&mut EditableState<StringBuffer>>,
{
    let editable = get_editable(&mut model.ui)?;
    
    let changed = match msg {
        TextEditMsg::Move(target) => {
            apply_move(editable, *target, false);
            true
        }
        TextEditMsg::MoveWithSelection(target) => {
            apply_move(editable, *target, true);
            true
        }
        TextEditMsg::InsertChar(ch) => editable.insert_char(*ch),
        TextEditMsg::InsertText(text) => editable.insert_text(text),
        TextEditMsg::DeleteBackward => { editable.delete_backward(); true }
        TextEditMsg::DeleteForward => { editable.delete_forward(); true }
        TextEditMsg::DeleteWordBackward => { editable.delete_word_backward(); true }
        TextEditMsg::DeleteWordForward => { editable.delete_word_forward(); true }
        TextEditMsg::SelectAll => { editable.select_all(); true }
        TextEditMsg::Copy => true, // Caller handles clipboard
        TextEditMsg::Cut => { editable.cut(); true }
        TextEditMsg::Paste(text) => editable.paste(text),
        TextEditMsg::Undo => editable.undo(),
        TextEditMsg::Redo => editable.redo(),
        // Multi-cursor operations are no-ops for single-line inputs
        TextEditMsg::AddCursorAbove
        | TextEditMsg::AddCursorBelow
        | TextEditMsg::AddCursorAtNextOccurrence
        | TextEditMsg::AddCursorsAtAllOccurrences
        | TextEditMsg::CollapseCursors => false,
        // Line operations are no-ops for single-line inputs
        TextEditMsg::InsertNewline
        | TextEditMsg::DeleteLine
        | TextEditMsg::Duplicate
        | TextEditMsg::MoveLineUp
        | TextEditMsg::MoveLineDown
        | TextEditMsg::Indent
        | TextEditMsg::Unindent => false,
        _ => false,
    };
    
    if changed {
        Some(Cmd::Redraw)
    } else {
        None
    }
}

fn apply_move<B: TextBuffer + TextBufferMut>(
    editable: &mut EditableState<B>,
    target: MoveTarget,
    extend_selection: bool,
) {
    match target {
        MoveTarget::Left => editable.move_left(extend_selection),
        MoveTarget::Right => editable.move_right(extend_selection),
        MoveTarget::Up => editable.move_up(extend_selection),
        MoveTarget::Down => editable.move_down(extend_selection),
        MoveTarget::LineStart => editable.move_home(extend_selection, false),
        MoveTarget::LineEnd => editable.move_end(extend_selection),
        MoveTarget::LineStartSmart => editable.move_home(extend_selection, true),
        MoveTarget::WordLeft => editable.move_word_left(extend_selection),
        MoveTarget::WordRight => editable.move_word_right(extend_selection),
        MoveTarget::DocumentStart => editable.move_document_start(extend_selection),
        MoveTarget::DocumentEnd => editable.move_document_end(extend_selection),
        MoveTarget::PageUp | MoveTarget::PageDown => {
            // Page movement requires viewport info, handled at higher level
        }
    }
}
```

### 5.6 Keyboard Input Translation

```rust
// src/runtime/input.rs (additions)

use crate::editable::context::EditContext;
use crate::editable::messages::{TextEditMsg, MoveTarget};

/// Determine the current editing context based on model state.
pub fn determine_edit_context(model: &AppModel) -> Option<EditContext> {
    // Priority order:
    // 1. Modal inputs
    if let Some(ref modal) = model.ui.active_modal {
        return match modal {
            ModalState::CommandPalette(_) => Some(EditContext::CommandPalette),
            ModalState::GotoLine(_) => Some(EditContext::GotoLine),
            ModalState::FindReplace(f) => Some(match f.focused_field {
                FindReplaceField::Query => EditContext::FindQuery,
                FindReplaceField::Replace => EditContext::ReplaceQuery,
            }),
            _ => None,
        };
    }
    
    // 2. CSV cell editing
    if model.is_csv_editing() {
        if let Some(cell_pos) = model.csv_editing_cell_position() {
            return Some(EditContext::CsvCell(cell_pos));
        }
    }
    
    // 3. Main editor
    if let Some(group_id) = model.focused_editor_group_id() {
        return Some(EditContext::Editor(group_id));
    }
    
    None
}

/// Convert a key event to TextEditMsg.
/// Returns None if the key is not a text editing operation.
pub fn key_to_text_edit_msg(
    key: Key,
    ctrl: bool,
    shift: bool,
    alt: bool,
    logo: bool,  // Cmd on macOS
) -> Option<TextEditMsg> {
    use TextEditMsg::*;
    
    // macOS uses Cmd (logo) for shortcuts, Windows/Linux use Ctrl
    #[cfg(target_os = "macos")]
    let cmd = logo;
    #[cfg(not(target_os = "macos"))]
    let cmd = ctrl;
    
    // Alt is Option on macOS
    let opt = alt;
    
    match (key, cmd, shift, opt) {
        // === Movement without selection ===
        (Key::ArrowLeft, false, false, false) => Some(Move(MoveTarget::Left)),
        (Key::ArrowRight, false, false, false) => Some(Move(MoveTarget::Right)),
        (Key::ArrowUp, false, false, false) => Some(Move(MoveTarget::Up)),
        (Key::ArrowDown, false, false, false) => Some(Move(MoveTarget::Down)),
        (Key::Home, false, false, false) => Some(Move(MoveTarget::LineStart)),
        (Key::End, false, false, false) => Some(Move(MoveTarget::LineEnd)),
        
        // Word movement (Option+Arrow on macOS, Ctrl+Arrow on others)
        #[cfg(target_os = "macos")]
        (Key::ArrowLeft, false, false, true) => Some(Move(MoveTarget::WordLeft)),
        #[cfg(target_os = "macos")]
        (Key::ArrowRight, false, false, true) => Some(Move(MoveTarget::WordRight)),
        #[cfg(not(target_os = "macos"))]
        (Key::ArrowLeft, true, false, false) => Some(Move(MoveTarget::WordLeft)),
        #[cfg(not(target_os = "macos"))]
        (Key::ArrowRight, true, false, false) => Some(Move(MoveTarget::WordRight)),
        
        // Line start/end with Cmd (macOS)
        #[cfg(target_os = "macos")]
        (Key::ArrowLeft, true, false, false) => Some(Move(MoveTarget::LineStart)),
        #[cfg(target_os = "macos")]
        (Key::ArrowRight, true, false, false) => Some(Move(MoveTarget::LineEnd)),
        
        // Document start/end
        #[cfg(target_os = "macos")]
        (Key::ArrowUp, true, false, false) => Some(Move(MoveTarget::DocumentStart)),
        #[cfg(target_os = "macos")]
        (Key::ArrowDown, true, false, false) => Some(Move(MoveTarget::DocumentEnd)),
        #[cfg(not(target_os = "macos"))]
        (Key::Home, true, false, false) => Some(Move(MoveTarget::DocumentStart)),
        #[cfg(not(target_os = "macos"))]
        (Key::End, true, false, false) => Some(Move(MoveTarget::DocumentEnd)),
        
        // === Movement with selection (Shift held) ===
        (Key::ArrowLeft, false, true, false) => Some(MoveWithSelection(MoveTarget::Left)),
        (Key::ArrowRight, false, true, false) => Some(MoveWithSelection(MoveTarget::Right)),
        (Key::ArrowUp, false, true, false) => Some(MoveWithSelection(MoveTarget::Up)),
        (Key::ArrowDown, false, true, false) => Some(MoveWithSelection(MoveTarget::Down)),
        (Key::Home, false, true, false) => Some(MoveWithSelection(MoveTarget::LineStart)),
        (Key::End, false, true, false) => Some(MoveWithSelection(MoveTarget::LineEnd)),
        
        // Word selection
        #[cfg(target_os = "macos")]
        (Key::ArrowLeft, false, true, true) => Some(MoveWithSelection(MoveTarget::WordLeft)),
        #[cfg(target_os = "macos")]
        (Key::ArrowRight, false, true, true) => Some(MoveWithSelection(MoveTarget::WordRight)),
        #[cfg(not(target_os = "macos"))]
        (Key::ArrowLeft, true, true, false) => Some(MoveWithSelection(MoveTarget::WordLeft)),
        #[cfg(not(target_os = "macos"))]
        (Key::ArrowRight, true, true, false) => Some(MoveWithSelection(MoveTarget::WordRight)),
        
        // === Deletion ===
        (Key::Backspace, false, false, false) => Some(DeleteBackward),
        (Key::Delete, false, false, false) => Some(DeleteForward),
        
        // Word deletion
        #[cfg(target_os = "macos")]
        (Key::Backspace, false, false, true) => Some(DeleteWordBackward),
        #[cfg(target_os = "macos")]
        (Key::Delete, false, false, true) => Some(DeleteWordForward),
        #[cfg(not(target_os = "macos"))]
        (Key::Backspace, true, false, false) => Some(DeleteWordBackward),
        #[cfg(not(target_os = "macos"))]
        (Key::Delete, true, false, false) => Some(DeleteWordForward),
        
        // === Selection ===
        (Key::Character(ref c), true, false, false) if c == "a" => Some(SelectAll),
        
        // === Clipboard ===
        (Key::Character(ref c), true, false, false) if c == "c" => Some(Copy),
        (Key::Character(ref c), true, false, false) if c == "x" => Some(Cut),
        // Paste is special - needs clipboard content from runtime
        
        // === Undo/Redo ===
        (Key::Character(ref c), true, false, false) if c == "z" => Some(Undo),
        (Key::Character(ref c), true, true, false) if c == "z" => Some(Redo),
        #[cfg(not(target_os = "macos"))]
        (Key::Character(ref c), true, false, false) if c == "y" => Some(Redo),
        
        // === Newline ===
        (Key::Enter, false, false, false) => Some(InsertNewline),
        
        _ => None,
    }
}
```

---

## 6. Phase 3: Rendering Consolidation

### 6.1 TextFieldContent Trait

```rust
// src/view/text_field.rs

use crate::editable::buffer::TextBuffer;
use crate::editable::cursor::Cursor;
use crate::editable::selection::Selection;

/// Trait for content that can be rendered as a text field.
/// Abstracts over EditableState, allowing uniform rendering.
pub trait TextFieldContent {
    /// Get the text buffer for reading
    fn buffer(&self) -> &dyn TextBuffer;
    
    /// Get all cursors
    fn cursors(&self) -> &[Cursor];
    
    /// Get all selections
    fn selections(&self) -> &[Selection];
    
    /// Get the active cursor index
    fn active_cursor_index(&self) -> usize;
    
    /// Check if this is a single-line input
    fn is_single_line(&self) -> bool;
}

impl<B: TextBuffer> TextFieldContent for EditableState<B> {
    fn buffer(&self) -> &dyn TextBuffer {
        &self.buffer
    }
    
    fn cursors(&self) -> &[Cursor] {
        &self.cursors
    }
    
    fn selections(&self) -> &[Selection] {
        &self.selections
    }
    
    fn active_cursor_index(&self) -> usize {
        self.active_cursor
    }
    
    fn is_single_line(&self) -> bool {
        !self.constraints.allow_multiline
    }
}
```

### 6.2 TextFieldRenderer

```rust
// src/view/text_field.rs

use crate::view::frame::Frame;
use crate::view::TextPainter;

/// Options for rendering a text field.
pub struct TextFieldOptions {
    /// X position of text area in pixels
    pub x: usize,
    /// Y position of text area in pixels
    pub y: usize,
    /// Width of text area in pixels
    pub width: usize,
    /// Height of text area in pixels
    pub height: usize,
    /// Character width (monospace font)
    pub char_width: f32,
    /// Line height in pixels
    pub line_height: usize,
    /// Text foreground color
    pub text_color: u32,
    /// Cursor color
    pub cursor_color: u32,
    /// Secondary cursor color (for multi-cursor)
    pub secondary_cursor_color: u32,
    /// Selection background color
    pub selection_color: u32,
    /// Whether cursors should be visible (for blinking)
    pub cursor_visible: bool,
    /// Horizontal scroll offset in columns
    pub scroll_x: usize,
    /// Vertical scroll offset in lines (for multi-line)
    pub scroll_y: usize,
    /// Whether to draw a focus border
    pub focused: bool,
    /// Focus border color
    pub focus_color: u32,
}

/// Unified renderer for text fields.
pub struct TextFieldRenderer;

impl TextFieldRenderer {
    /// Render a text field.
    pub fn render(
        frame: &mut Frame,
        painter: &mut TextPainter,
        content: &dyn TextFieldContent,
        opts: &TextFieldOptions,
    ) {
        if content.is_single_line() {
            Self::render_single_line(frame, painter, content, opts);
        } else {
            Self::render_multi_line(frame, painter, content, opts);
        }
    }
    
    fn render_single_line(
        frame: &mut Frame,
        painter: &mut TextPainter,
        content: &dyn TextFieldContent,
        opts: &TextFieldOptions,
    ) {
        let buffer = content.buffer();
        let text = buffer.line(0).unwrap_or_default();
        let text_x = opts.x;
        let text_y = opts.y;
        
        // 1. Render selection backgrounds
        for selection in content.selections() {
            if selection.is_empty() {
                continue;
            }
            
            let start_col = selection.start().column;
            let end_col = selection.end().column;
            
            // Adjust for horizontal scroll
            let visible_start = start_col.saturating_sub(opts.scroll_x);
            let visible_end = end_col.saturating_sub(opts.scroll_x);
            
            if visible_end > visible_start {
                let sel_x = text_x + (visible_start as f32 * opts.char_width).round() as usize;
                let sel_width = ((visible_end - visible_start) as f32 * opts.char_width).round() as usize;
                
                frame.fill_rect_px(
                    sel_x,
                    text_y,
                    sel_width.min(opts.width),
                    opts.line_height,
                    opts.selection_color,
                );
            }
        }
        
        // 2. Render text (with horizontal scroll)
        let visible_text: String = text
            .chars()
            .skip(opts.scroll_x)
            .take((opts.width as f32 / opts.char_width).ceil() as usize + 1)
            .collect();
        
        painter.draw(frame, text_x, text_y, &visible_text, opts.text_color);
        
        // 3. Render cursors
        if opts.cursor_visible {
            for (idx, cursor) in content.cursors().iter().enumerate() {
                let col = cursor.column.saturating_sub(opts.scroll_x);
                let cursor_x = text_x + (col as f32 * opts.char_width).round() as usize;
                
                // Check if cursor is visible in viewport
                if cursor_x >= text_x && cursor_x < text_x + opts.width {
                    let color = if idx == content.active_cursor_index() {
                        opts.cursor_color
                    } else {
                        opts.secondary_cursor_color
                    };
                    
                    // 2px wide cursor bar
                    frame.fill_rect_px(
                        cursor_x,
                        text_y + 1,
                        2,
                        opts.line_height.saturating_sub(2),
                        color,
                    );
                }
            }
        }
        
        // 4. Render focus border if focused
        if opts.focused {
            frame.draw_rect_outline(
                opts.x.saturating_sub(2),
                opts.y.saturating_sub(2),
                opts.width + 4,
                opts.height + 4,
                opts.focus_color,
                1,
            );
        }
    }
    
    fn render_multi_line(
        frame: &mut Frame,
        painter: &mut TextPainter,
        content: &dyn TextFieldContent,
        opts: &TextFieldOptions,
    ) {
        let buffer = content.buffer();
        let visible_lines = opts.height / opts.line_height;
        let start_line = opts.scroll_y;
        let end_line = (start_line + visible_lines + 1).min(buffer.line_count());
        
        for doc_line in start_line..end_line {
            let screen_line = doc_line - start_line;
            let line_y = opts.y + screen_line * opts.line_height;
            
            if let Some(line_text) = buffer.line(doc_line) {
                // 1. Render selections on this line
                for selection in content.selections() {
                    if selection.is_empty() {
                        continue;
                    }
                    
                    Self::render_line_selection(
                        frame,
                        &line_text,
                        doc_line,
                        selection,
                        opts,
                        line_y,
                    );
                }
                
                // 2. Render text
                let visible_text: String = line_text
                    .chars()
                    .skip(opts.scroll_x)
                    .take((opts.width as f32 / opts.char_width).ceil() as usize + 1)
                    .collect();
                
                painter.draw(frame, opts.x, line_y, &visible_text, opts.text_color);
            }
        }
        
        // 3. Render cursors
        if opts.cursor_visible {
            for (idx, cursor) in content.cursors().iter().enumerate() {
                if cursor.line >= start_line && cursor.line < end_line {
                    let screen_line = cursor.line - start_line;
                    let cursor_y = opts.y + screen_line * opts.line_height;
                    let col = cursor.column.saturating_sub(opts.scroll_x);
                    let cursor_x = opts.x + (col as f32 * opts.char_width).round() as usize;
                    
                    if cursor_x >= opts.x && cursor_x < opts.x + opts.width {
                        let color = if idx == content.active_cursor_index() {
                            opts.cursor_color
                        } else {
                            opts.secondary_cursor_color
                        };
                        
                        frame.fill_rect_px(
                            cursor_x,
                            cursor_y + 1,
                            2,
                            opts.line_height.saturating_sub(2),
                            color,
                        );
                    }
                }
            }
        }
    }
    
    fn render_line_selection(
        frame: &mut Frame,
        line_text: &str,
        doc_line: usize,
        selection: &Selection,
        opts: &TextFieldOptions,
        line_y: usize,
    ) {
        let sel_start = selection.start();
        let sel_end = selection.end();
        
        // Check if this line is in selection range
        if doc_line < sel_start.line || doc_line > sel_end.line {
            return;
        }
        
        let line_len = line_text.chars().count();
        
        let start_col = if doc_line == sel_start.line {
            sel_start.column
        } else {
            0
        };
        
        let end_col = if doc_line == sel_end.line {
            sel_end.column
        } else {
            line_len
        };
        
        if start_col >= end_col {
            return;
        }
        
        // Adjust for horizontal scroll
        let visible_start = start_col.saturating_sub(opts.scroll_x);
        let visible_end = end_col.saturating_sub(opts.scroll_x);
        
        if visible_end > visible_start {
            let sel_x = opts.x + (visible_start as f32 * opts.char_width).round() as usize;
            let sel_width = ((visible_end - visible_start) as f32 * opts.char_width).round() as usize;
            
            frame.fill_rect_px(
                sel_x,
                line_y,
                sel_width,
                opts.line_height,
                opts.selection_color,
            );
        }
    }
}
```

### 6.3 Usage in Modal Rendering

```rust
// src/view/mod.rs (updated)

fn render_command_palette(
    frame: &mut Frame,
    painter: &mut TextPainter,
    model: &AppModel,
    state: &CommandPaletteState,
    // ...
) {
    // ... layout calculations ...
    
    // Render input field using unified renderer
    let input_opts = TextFieldOptions {
        x: input_x + padding,
        y: input_y + padding,
        width: input_width - padding * 2,
        height: line_height,
        char_width: painter.char_width(),
        line_height,
        text_color: model.theme.overlay.foreground.to_argb_u32(),
        cursor_color: model.theme.editor.cursor_color.to_argb_u32(),
        secondary_cursor_color: model.theme.editor.secondary_cursor_color.to_argb_u32(),
        selection_color: model.theme.editor.selection_background.to_argb_u32(),
        cursor_visible: model.ui.cursor_visible,
        scroll_x: 0,  // Single-line, no scroll for now
        scroll_y: 0,
        focused: true,
        focus_color: model.theme.overlay.highlight.to_argb_u32(),
    };
    
    TextFieldRenderer::render(frame, painter, &state.editable, &input_opts);
    
    // ... render command list ...
}
```

---

## 7. Phase 4: Migration Strategy

### 7.1 Migration Philosophy

The migration is designed to be **incremental and non-breaking**:
- Each phase can be merged independently
- Old code coexists with new code during transition
- Bridge layers allow gradual migration
- Full test coverage at each phase

### 7.2 Migration Order

```
Phase 1: Foundation (Week 1)
    │   Create src/editable/ module with all types
    │   Add TextEditMsg to messages
    │   Comprehensive unit tests
    ▼
Phase 2: CSV Cell Editor (Week 2)
    │   Simplest context, isolated
    │   Replace CellEditState with EditableState<StringBuffer>
    │   Test: edit lifecycle
    ▼
Phase 3: Modal Inputs (Week 2-3)
    │   Command palette, go-to-line, find/replace
    │   Add EditableState to modal states
    │   Test: all modal inputs
    ▼
Phase 4: Rendering Consolidation (Week 3)
    │   Create TextFieldRenderer
    │   Migrate modal and CSV rendering
    │   Visual regression testing
    ▼
Phase 5: Main Editor Bridge (Week 4)
    │   Create bridge from TextEditMsg to existing EditorMsg/DocumentMsg
    │   No internal changes yet
    │   Test: editor still works
    ▼
Phase 6: Main Editor Migration (Week 4-5)
    │   Incrementally replace EditorState internals
    │   Migrate movement → editing → selection → multi-cursor
    │   Test: all editor functionality
    ▼
Phase 7: Cleanup (Week 6)
    │   Remove deprecated message variants
    │   Remove old CellEditState
    │   Remove bridge code
    │   Final documentation
    ▼
Done!
```

### 7.3 Detailed Migration Steps

#### Step 1: Create Foundation

1. Create `src/editable/mod.rs` with module structure
2. Implement `TextBuffer` trait and both buffer types
3. Implement `Cursor`, `Selection`, `Position`
4. Implement `EditConstraints` with presets
5. Implement `EditHistory`
6. Implement `EditableState` with all operations
7. Add `TextEditMsg`, `MoveTarget`, `EditContext` to messages
8. Create `src/update/text_edit.rs` with routing stub
9. Add comprehensive unit tests for all types

#### Step 2: Migrate CSV Cell Editor

**Files to modify:**
- `src/csv/model.rs` - Replace `CellEditState` internals

```rust
// Before
pub struct CellEditState {
    pub position: CellPosition,
    pub buffer: String,
    pub cursor: usize,
    pub original: String,
}

// After
pub struct CellEditState {
    pub position: CellPosition,
    pub editable: EditableState<StringBuffer>,
    pub original: String,
}
```

- `src/update/csv.rs` - Route edit messages to `update_text_edit()`
- `src/runtime/input.rs` - Update `handle_csv_edit_key()` to emit `TextEditMsg`
- `src/view/mod.rs` - Update `render_csv_cell_editor()` to use `TextFieldRenderer`

**Test checklist:**
- [x] Start editing cell (Enter, F2, typing)
- [x] Type characters
- [x] Backspace, Delete
- [x] Arrow keys move cursor
- [x] Home/End
- [x] Word movement (Option+Arrow)
- [x] Word delete (Option+Backspace)
- [x] Selection (Shift+Arrow, Shift+Home/End, Shift+Option+Arrow)
- [x] Select all (Cmd+A)
- [x] Cut/Copy/Paste (Cmd+C/X/V)
- [x] Undo/Redo
- [x] Confirm edit (Enter)
- [x] Cancel edit (Escape)

#### Step 3: Migrate Modal Inputs

**Files to modify:**
- `src/model/ui.rs` - Add `EditableState` to modal states

```rust
// Before
pub struct CommandPaletteState {
    pub input: String,
    pub selected_index: usize,
}

// After
pub struct CommandPaletteState {
    pub editable: EditableState<StringBuffer>,
    pub selected_index: usize,
}

// Similarly for GotoLineState, FindReplaceState
```

- `src/update/ui.rs` - Route to `update_text_edit()`
- `src/runtime/input.rs` - Update `handle_modal_key()` to emit `TextEditMsg`
- `src/view/mod.rs` - Update modal rendering

**Test checklist:**
- [x] Command palette: Full text editing
- [x] Go-to-line: Only digits accepted
- [x] Find/Replace: Both fields editable
- [x] Tab switches fields in find/replace
- [x] Escape closes modal
- [x] Selection works (Shift+Arrow, Cmd+A)
- [x] Clipboard integration (Cmd+C/X/V)

#### Step 4: Rendering Consolidation

1. Create `src/view/text_field.rs` with `TextFieldRenderer`
2. Implement `TextFieldContent` trait
3. Migrate CSV cell rendering
4. Migrate modal input rendering
5. Visual regression testing

#### Step 5: Main Editor Bridge

Create a bridge that maps `TextEditMsg` to existing `EditorMsg`/`DocumentMsg`:

```rust
// src/update/text_edit.rs

fn update_editor_text_via_bridge(
    model: &mut AppModel,
    group_id: EditorGroupId,
    msg: TextEditMsg,
) -> Option<Cmd> {
    // Map TextEditMsg to existing messages
    let msgs: Vec<Msg> = match msg {
        TextEditMsg::Move(MoveTarget::Left) => 
            vec![Msg::Editor(EditorMsg::MoveCursor(Direction::Left))],
        TextEditMsg::MoveWithSelection(MoveTarget::Left) => 
            vec![Msg::Editor(EditorMsg::MoveCursorWithSelection(Direction::Left))],
        TextEditMsg::InsertChar(ch) => 
            vec![Msg::Document(DocumentMsg::InsertChar(ch))],
        TextEditMsg::DeleteBackward => 
            vec![Msg::Document(DocumentMsg::DeleteBackward)],
        // ... map all variants
        _ => vec![],
    };
    
    // Dispatch through existing handlers
    let mut cmd = None;
    for m in msgs {
        if let Some(c) = update_inner(model, m) {
            cmd = Some(c);
        }
    }
    cmd
}
```

#### Step 6: Main Editor Migration

This is the largest phase. Migrate incrementally:

1. **Movement operations**: Replace cursor movement in `EditorState` with calls to `EditableState`
2. **Editing operations**: Replace insert/delete in `Document` with `EditableState`
3. **Selection operations**: Replace selection handling
4. **Multi-cursor**: Ensure all multi-cursor operations work

**Key consideration**: The main editor has additional complexity:
- Viewport scrolling
- Syntax highlighting interaction
- Rectangle selection
- Occurrence highlighting

These may need to remain separate or be adapted.

#### Step 7: Cleanup

1. Remove deprecated `EditorMsg` variants (movement, selection)
2. Remove deprecated `DocumentMsg` variants (basic editing)
3. Remove deprecated `ModalMsg` editing variants
4. Remove deprecated `CsvMsg::Edit*` variants
5. Remove old `delete_word_backward()` from `update_ui.rs`
6. Remove bridge code
7. Update AGENTS.md with new patterns
8. Update CHANGELOG.md

---

## 8. Implementation Order

### Milestone 1: Foundation (Week 1)

- [ ] Create `src/editable/mod.rs`
- [ ] Create `src/editable/buffer.rs` with `TextBuffer`, `TextBufferMut` traits
- [ ] Implement `RopeBuffer`
- [ ] Implement `StringBuffer`
- [ ] Create `src/editable/cursor.rs` with `Position`, `Cursor`
- [ ] Create `src/editable/selection.rs` with `Selection`
- [ ] Create `src/editable/history.rs` with `EditOperation`, `EditHistory`
- [ ] Create `src/editable/constraints.rs` with `EditConstraints`
- [ ] Create `src/editable/state.rs` with `EditableState`
- [ ] Create `src/editable/context.rs` with `EditContext`
- [ ] Create `src/editable/messages.rs` with `TextEditMsg`, `MoveTarget`
- [ ] Add unit tests for all buffer operations
- [ ] Add unit tests for cursor movement
- [ ] Add unit tests for text editing
- [ ] Add unit tests for selection
- [ ] Add unit tests for undo/redo

### Milestone 2: Message System (Week 1-2)

- [ ] Add `Msg::TextEdit(EditContext, TextEditMsg)` to `src/messages.rs`
- [ ] Create `src/update/text_edit.rs` with `update_text_edit()`
- [ ] Add dispatch in `src/update/mod.rs`
- [ ] Add `determine_edit_context()` to `src/runtime/input.rs`
- [ ] Add `key_to_text_edit_msg()` to `src/runtime/input.rs`

### Milestone 3: CSV Cell Migration (Week 2)

- [ ] Update `CellEditState` to use `EditableState<StringBuffer>`
- [ ] Update `CsvMsg` editing variants to delegate
- [ ] Update `handle_csv_edit_key()` to emit `TextEditMsg`
- [ ] Update `update_csv()` to route to `update_text_edit()`
- [ ] Test cell editing lifecycle

### Milestone 4: Modal Migration (Week 2-3)

- [ ] Add `EditableState` to `CommandPaletteState`
- [ ] Add `EditableState` to `GotoLineState`
- [ ] Add `EditableState` to `FindReplaceState`
- [ ] Update `handle_modal_key()` to emit `TextEditMsg`
- [ ] Update `update_modal()` to delegate
- [ ] Test all modal inputs

### Milestone 5: Rendering (Week 3)

- [ ] Create `src/view/text_field.rs`
- [ ] Implement `TextFieldContent` trait
- [ ] Implement `TextFieldRenderer`
- [ ] Implement `TextFieldOptions`
- [ ] Migrate CSV cell rendering
- [ ] Migrate modal input rendering
- [ ] Visual regression testing

### Milestone 6: Main Editor Bridge (Week 4)

- [ ] Create bridge in `update_text_edit.rs`
- [ ] Map all `TextEditMsg` variants to existing messages
- [ ] Test editor functionality unchanged

### Milestone 7: Main Editor Migration (Week 4-5)

- [ ] Migrate cursor movement operations
- [ ] Migrate insert/delete operations
- [ ] Migrate selection operations
- [ ] Migrate multi-cursor operations
- [ ] Verify all editor features work

### Milestone 8: Cleanup (Week 6)

- [ ] Remove deprecated message variants
- [ ] Remove old `CellEditState`
- [ ] Remove `delete_word_backward()` duplicate
- [ ] Remove bridge code
- [ ] Update documentation
- [ ] Final test pass

---

## 9. File Change Summary

### New Files

| File | Purpose | Lines (est.) |
|------|---------|--------------|
| `src/editable/mod.rs` | Module exports | 30 |
| `src/editable/buffer.rs` | TextBuffer traits + implementations | 400 |
| `src/editable/cursor.rs` | Position, Cursor types | 80 |
| `src/editable/selection.rs` | Selection type | 100 |
| `src/editable/history.rs` | EditOperation, EditHistory | 150 |
| `src/editable/constraints.rs` | EditConstraints, CharFilter | 100 |
| `src/editable/state.rs` | EditableState main abstraction | 600 |
| `src/editable/context.rs` | EditContext enum | 50 |
| `src/editable/messages.rs` | TextEditMsg, MoveTarget | 100 |
| `src/update/text_edit.rs` | Unified update handler | 300 |
| `src/view/text_field.rs` | TextFieldRenderer | 300 |

**Total new code:** ~2,200 lines

### Modified Files

| File | Changes |
|------|---------|
| `src/lib.rs` | Add `editable` module |
| `src/messages.rs` | Add `Msg::TextEdit` variant |
| `src/update/mod.rs` | Add dispatch for `TextEdit` |
| `src/runtime/input.rs` | Add context detection, key translation |
| `src/model/ui.rs` | Update modal states with `EditableState` |
| `src/csv/model.rs` | Update `CellEditState` |
| `src/update/ui.rs` | Delegate editing to `update_text_edit()` |
| `src/update/csv.rs` | Delegate editing to `update_text_edit()` |
| `src/view/mod.rs` | Use `TextFieldRenderer` |

### Files to Deprecate (Phase 7)

| Location | What to Remove |
|----------|----------------|
| `src/update/ui.rs:15-25` | `delete_word_backward()` function |
| `src/messages.rs` | `ModalMsg::InsertChar`, `DeleteBackward`, etc. |
| `src/messages.rs` | `CsvMsg::EditInsertChar`, `EditDeleteBackward`, etc. |
| `src/csv/model.rs` | Old `CellEditState` methods |

---

## 10. Testing Strategy

### 10.1 Unit Tests

Create test modules for each new file:

```rust
// src/editable/buffer.rs

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_string_buffer_insert() {
        let mut buf = StringBuffer::from_str("hello");
        buf.insert(5, " world");
        assert_eq!(buf.to_string(), "hello world");
    }
    
    #[test]
    fn test_string_buffer_remove() {
        let mut buf = StringBuffer::from_str("hello world");
        buf.remove(5..11);
        assert_eq!(buf.to_string(), "hello");
    }
    
    #[test]
    fn test_string_buffer_position_conversion() {
        let buf = StringBuffer::from_str("hello");
        assert_eq!(buf.offset_to_position(3), (0, 3));
        assert_eq!(buf.position_to_offset(0, 3), 3);
    }
    
    #[test]
    fn test_string_buffer_utf8() {
        let buf = StringBuffer::from_str("héllo");
        assert_eq!(buf.len_chars(), 5);
        assert_eq!(buf.len_bytes(), 6);  // é is 2 bytes
    }
    
    #[test]
    fn test_rope_buffer_multiline() {
        let buf = RopeBuffer::from_str("line1\nline2\nline3");
        assert_eq!(buf.line_count(), 3);
        assert_eq!(buf.line(1).unwrap(), "line2");
    }
    
    #[test]
    fn test_rope_buffer_position_conversion() {
        let buf = RopeBuffer::from_str("hello\nworld");
        assert_eq!(buf.offset_to_position(6), (1, 0));
        assert_eq!(buf.position_to_offset(1, 0), 6);
    }
}
```

```rust
// src/editable/state.rs

#[cfg(test)]
mod tests {
    use super::*;
    
    fn create_test_state(text: &str) -> EditableState<StringBuffer> {
        EditableState::new(
            StringBuffer::from_str(text),
            EditConstraints::single_line(),
        )
    }
    
    #[test]
    fn test_cursor_movement() {
        let mut state = create_test_state("hello");
        state.cursors[0] = Cursor::new(0, 2);
        
        state.move_left(false);
        assert_eq!(state.cursor().column, 1);
        
        state.move_right(false);
        assert_eq!(state.cursor().column, 2);
    }
    
    #[test]
    fn test_word_movement() {
        let mut state = create_test_state("hello world");
        state.cursors[0] = Cursor::new(0, 0);
        
        state.move_word_right(false);
        assert_eq!(state.cursor().column, 5);
        
        state.move_word_right(false);
        assert_eq!(state.cursor().column, 6);  // After space
    }
    
    #[test]
    fn test_selection() {
        let mut state = create_test_state("hello world");
        state.cursors[0] = Cursor::new(0, 0);
        
        state.move_word_right(true);  // Select "hello"
        
        assert_eq!(state.selection().anchor, Position::new(0, 0));
        assert_eq!(state.selection().head, Position::new(0, 5));
        assert_eq!(state.selected_text(), "hello");
    }
    
    #[test]
    fn test_insert_replaces_selection() {
        let mut state = create_test_state("hello world");
        state.cursors[0] = Cursor::new(0, 5);
        state.selections[0] = Selection::new(
            Position::new(0, 0),
            Position::new(0, 5),
        );
        
        state.insert_char('X');
        
        assert_eq!(state.text(), "X world");
        assert_eq!(state.cursor().column, 1);
    }
    
    #[test]
    fn test_char_filter() {
        let mut state = EditableState::new(
            StringBuffer::new(),
            EditConstraints::numeric(),
        );
        
        assert!(state.insert_char('5'));
        assert!(!state.insert_char('a'));
        assert_eq!(state.text(), "5");
    }
    
    #[test]
    fn test_undo_redo() {
        let mut state = create_test_state("");
        
        state.insert_char('a');
        state.insert_char('b');
        assert_eq!(state.text(), "ab");
        
        assert!(state.undo());
        assert_eq!(state.text(), "a");
        
        assert!(state.undo());
        assert_eq!(state.text(), "");
        
        assert!(state.redo());
        assert_eq!(state.text(), "a");
    }
}
```

### 10.2 Integration Tests

```rust
// tests/text_editing_integration.rs

#[test]
fn test_command_palette_workflow() {
    let mut model = create_test_model();
    
    // Open palette
    update(&mut model, Msg::Ui(UiMsg::OpenCommandPalette));
    
    // Type a query
    let context = EditContext::CommandPalette;
    update(&mut model, Msg::TextEdit(context, TextEditMsg::InsertText("format".into())));
    
    // Check text
    assert_eq!(model.ui.active_modal.as_ref().unwrap()
        .as_command_palette().unwrap()
        .editable.text(), "format");
    
    // Move cursor and edit
    update(&mut model, Msg::TextEdit(context, TextEditMsg::Move(MoveTarget::LineStart)));
    update(&mut model, Msg::TextEdit(context, TextEditMsg::DeleteWordForward));
    
    assert_eq!(model.ui.active_modal.as_ref().unwrap()
        .as_command_palette().unwrap()
        .editable.text(), "");
}

#[test]
fn test_csv_cell_editing() {
    let mut model = create_test_model_with_csv();
    
    // Start editing
    update(&mut model, Msg::Csv(CsvMsg::StartEditing));
    
    // Type in cell
    let cell_pos = model.csv_editing_cell_position().unwrap();
    let context = EditContext::CsvCell(cell_pos);
    update(&mut model, Msg::TextEdit(context, TextEditMsg::InsertText("hello".into())));
    
    // Select all and replace
    update(&mut model, Msg::TextEdit(context, TextEditMsg::SelectAll));
    update(&mut model, Msg::TextEdit(context, TextEditMsg::InsertText("world".into())));
    
    // Undo
    update(&mut model, Msg::TextEdit(context, TextEditMsg::Undo));
    
    // Verify
    let cell_state = model.csv_editing_state().unwrap();
    assert_eq!(cell_state.editable.text(), "hello");
}
```

### 10.3 Regression Tests

- Run existing test suite at each phase
- Visual comparison: Before/after screenshots of all input areas
- Multi-cursor: Verify no regressions in complex scenarios
- Performance: Ensure no degradation in large file editing

---

## Appendix A: Key Invariants

These invariants must be maintained at all times:

1. **Cursor-Selection Parallel**
   ```
   cursors.len() == selections.len()
   ```

2. **Cursor-Head Match**
   ```
   ∀i: cursors[i].to_position() == selections[i].head
   ```

3. **Sorted Cursors** (for multi-cursor)
   ```
   cursors are sorted by (line, column)
   ```

4. **Active Index Valid**
   ```
   active_cursor_index < cursors.len()
   ```

5. **Non-Empty Cursors**
   ```
   cursors.len() >= 1
   ```

6. **Redo Stack Cleared on Edit**
   ```
   Any mutation operation clears the redo stack
   ```

7. **Constraint Enforcement**
   ```
   Operations that violate constraints are rejected (return false)
   ```

8. **UTF-8 Validity**
   ```
   All byte offsets must be at valid UTF-8 character boundaries
   ```

---

## Appendix B: Word Boundary Detection

Use the existing `CharType` from `src/util/text.rs` for consistent word boundaries:

```rust
/// Character classification for word operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CharType {
    Whitespace,    // ch.is_whitespace()
    WordChar,      // Alphanumeric (letters, digits, underscore)
    Punctuation,   // /, :, ., -, (, ), {, }, etc.
}

pub fn char_type(ch: char) -> CharType {
    if ch.is_whitespace() {
        CharType::Whitespace
    } else if is_punctuation(ch) {
        CharType::Punctuation
    } else {
        CharType::WordChar
    }
}

fn is_punctuation(ch: char) -> bool {
    matches!(ch,
        '/' | '\\' | ':' | '.' | ',' | ';' | '-' | '_' |
        '(' | ')' | '[' | ']' | '{' | '}' | '<' | '>' |
        '"' | '\'' | '`' | '!' | '?' | '@' | '#' | '$' |
        '%' | '^' | '&' | '*' | '+' | '=' | '|' | '~'
    )
}
```

**Word boundaries** occur at `CharType` transitions (IntelliJ-style):
- `hello|World` - camelCase boundary (if enabled)
- `hello| |world` - whitespace boundaries
- `path|/|to|/|file` - punctuation boundaries

---

## Appendix C: Keyboard Shortcut Matrix

### Expected Shortcuts After Unification

| Action | macOS | Windows/Linux | TextEditMsg |
|--------|-------|---------------|-------------|
| **Movement** |
| Cursor left | ← | ← | `Move(Left)` |
| Cursor right | → | → | `Move(Right)` |
| Cursor up | ↑ | ↑ | `Move(Up)` |
| Cursor down | ↓ | ↓ | `Move(Down)` |
| Word left | ⌥← | Ctrl+← | `Move(WordLeft)` |
| Word right | ⌥→ | Ctrl+→ | `Move(WordRight)` |
| Line start | ⌘← or Home | Home | `Move(LineStart)` |
| Line end | ⌘→ or End | End | `Move(LineEnd)` |
| Doc start | ⌘↑ | Ctrl+Home | `Move(DocumentStart)` |
| Doc end | ⌘↓ | Ctrl+End | `Move(DocumentEnd)` |
| **Selection** |
| Select left | ⇧← | Shift+← | `MoveWithSelection(Left)` |
| Select right | ⇧→ | Shift+→ | `MoveWithSelection(Right)` |
| Select up | ⇧↑ | Shift+↑ | `MoveWithSelection(Up)` |
| Select down | ⇧↓ | Shift+↓ | `MoveWithSelection(Down)` |
| Select word left | ⌥⇧← | Ctrl+Shift+← | `MoveWithSelection(WordLeft)` |
| Select word right | ⌥⇧→ | Ctrl+Shift+→ | `MoveWithSelection(WordRight)` |
| Select all | ⌘A | Ctrl+A | `SelectAll` |
| **Deletion** |
| Delete backward | ⌫ | Backspace | `DeleteBackward` |
| Delete forward | ⌦ or Fn+⌫ | Delete | `DeleteForward` |
| Delete word back | ⌥⌫ | Ctrl+Backspace | `DeleteWordBackward` |
| Delete word fwd | ⌥⌦ | Ctrl+Delete | `DeleteWordForward` |
| **Clipboard** |
| Cut | ⌘X | Ctrl+X | `Cut` |
| Copy | ⌘C | Ctrl+C | `Copy` |
| Paste | ⌘V | Ctrl+V | `Paste(text)` |
| **Undo/Redo** |
| Undo | ⌘Z | Ctrl+Z | `Undo` |
| Redo | ⌘⇧Z | Ctrl+Y or Ctrl+Shift+Z | `Redo` |

---

## Appendix D: Message Type Mappings

### Current → Unified Mapping

| Old Message | New Message |
|-------------|-------------|
| **Editor Movement** |
| `EditorMsg::MoveCursor(Left)` | `TextEditMsg::Move(MoveTarget::Left)` |
| `EditorMsg::MoveCursor(Right)` | `TextEditMsg::Move(MoveTarget::Right)` |
| `EditorMsg::MoveCursor(Up)` | `TextEditMsg::Move(MoveTarget::Up)` |
| `EditorMsg::MoveCursor(Down)` | `TextEditMsg::Move(MoveTarget::Down)` |
| `EditorMsg::MoveCursorWord(Left)` | `TextEditMsg::Move(MoveTarget::WordLeft)` |
| `EditorMsg::MoveCursorWord(Right)` | `TextEditMsg::Move(MoveTarget::WordRight)` |
| `EditorMsg::MoveCursorWithSelection(Left)` | `TextEditMsg::MoveWithSelection(MoveTarget::Left)` |
| `EditorMsg::SelectAll` | `TextEditMsg::SelectAll` |
| **Document Editing** |
| `DocumentMsg::InsertChar(ch)` | `TextEditMsg::InsertChar(ch)` |
| `DocumentMsg::InsertNewline` | `TextEditMsg::InsertNewline` |
| `DocumentMsg::DeleteBackward` | `TextEditMsg::DeleteBackward` |
| `DocumentMsg::DeleteForward` | `TextEditMsg::DeleteForward` |
| `DocumentMsg::DeleteWordBackward` | `TextEditMsg::DeleteWordBackward` |
| `DocumentMsg::DeleteWordForward` | `TextEditMsg::DeleteWordForward` |
| `DocumentMsg::Undo` | `TextEditMsg::Undo` |
| `DocumentMsg::Redo` | `TextEditMsg::Redo` |
| `DocumentMsg::Copy` | `TextEditMsg::Copy` |
| `DocumentMsg::Cut` | `TextEditMsg::Cut` |
| `DocumentMsg::Paste` | `TextEditMsg::Paste(text)` |
| **Modal Editing** |
| `ModalMsg::InsertChar(ch)` | `TextEditMsg::InsertChar(ch)` |
| `ModalMsg::DeleteBackward` | `TextEditMsg::DeleteBackward` |
| `ModalMsg::DeleteWordBackward` | `TextEditMsg::DeleteWordBackward` |
| **CSV Cell Editing** |
| `CsvMsg::EditInsertChar(ch)` | `TextEditMsg::InsertChar(ch)` |
| `CsvMsg::EditDeleteBackward` | `TextEditMsg::DeleteBackward` |
| `CsvMsg::EditDeleteForward` | `TextEditMsg::DeleteForward` |
| `CsvMsg::EditCursorLeft` | `TextEditMsg::Move(MoveTarget::Left)` |
| `CsvMsg::EditCursorRight` | `TextEditMsg::Move(MoveTarget::Right)` |
| `CsvMsg::EditCursorHome` | `TextEditMsg::Move(MoveTarget::LineStart)` |
| `CsvMsg::EditCursorEnd` | `TextEditMsg::Move(MoveTarget::LineEnd)` |

---

## Appendix E: Critical Files Reference

### Current Implementation Files

| File | Lines | Purpose |
|------|-------|---------|
| `src/model/editor.rs` | ~1291 | Cursor, Selection, EditorState |
| `src/model/document.rs` | ~299 | Document, EditOperation, undo stacks |
| `src/update/editor.rs` | ~1263 | Cursor movement, selection operations |
| `src/update/document.rs` | ~2007 | Text editing, clipboard, undo/redo |
| `src/model/ui.rs` | ~135 | Modal state structs |
| `src/update/ui.rs` | ~350 | Modal update handlers |
| `src/csv/model.rs` | ~300 | CellEditState, CsvState |
| `src/update/csv.rs` | ~548 | CSV editing handlers |
| `src/runtime/input.rs` | ~350 | Keyboard event handling |
| `src/view/mod.rs` | ~1835 | All rendering |
| `src/util/text.rs` | ~100 | CharType, word boundaries |

### Key Locations

| What | File:Line |
|------|-----------|
| Cursor struct | `src/model/editor.rs:50-55` |
| Selection struct | `src/model/editor.rs:60-70` |
| EditorState struct | `src/model/editor.rs:200-250` |
| EditOperation enum | `src/model/document.rs:20-50` |
| CommandPaletteState | `src/model/ui.rs:56-60` |
| CellEditState | `src/csv/model.rs:54-65` |
| delete_word_backward (modal) | `src/update/ui.rs:15-25` |
| handle_modal_key | `src/runtime/input.rs:202-260` |
| handle_csv_edit_key | `src/runtime/input.rs:266-336` |
| render_text_area | `src/view/mod.rs:615-905` |
| render_csv_cell_editor | `src/view/mod.rs:1137-1208` |

---

## Appendix F: Manual Testing Checklist

### Command Palette
- [ ] Opens with ⌘P / Ctrl+P
- [ ] Cursor visible and blinking
- [ ] Type characters
- [ ] Backspace deletes character
- [ ] ⌥⌫ / Ctrl+Backspace deletes word
- [ ] ← / → moves cursor
- [ ] ⌥← / Ctrl+← moves by word
- [ ] Home/End moves to start/end
- [ ] ⇧+Arrow selects text
- [ ] ⌘A / Ctrl+A selects all
- [ ] ⌘X / Ctrl+X cuts selection
- [ ] ⌘C / Ctrl+C copies selection
- [ ] ⌘V / Ctrl+V pastes
- [ ] ⌘Z / Ctrl+Z undoes
- [ ] ⌘⇧Z / Ctrl+Y redoes
- [ ] Escape closes (or collapses selection first)
- [ ] Enter confirms selection

### Go to Line
- [ ] Opens with ⌘G / Ctrl+G
- [ ] Only accepts digits
- [ ] All text editing works
- [ ] Enter navigates to line
- [ ] Escape closes

### Find/Replace
- [ ] Opens with ⌘F / Ctrl+F
- [ ] Tab switches between fields
- [ ] Both fields fully editable
- [ ] Enter finds next
- [ ] All text editing works

### CSV Cell Editor
- [ ] Enter starts editing
- [ ] Typing starts editing with that character
- [ ] All cursor movement works
- [ ] Selection works
- [ ] Word operations work
- [ ] Undo/Redo works
- [ ] Enter confirms and moves down
- [ ] Tab confirms and moves right
- [ ] Escape cancels edit

### Main Editor
- [ ] All existing functionality preserved
- [ ] Multi-cursor still works
- [ ] Rectangle selection still works
- [ ] All keyboard shortcuts work
- [ ] Performance unchanged for large files

---

*End of document. This plan provides a complete roadmap for unifying text editing across all input contexts in the Token editor.*
