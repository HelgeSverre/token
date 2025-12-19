# Unified Text Editing System: Comprehensive Refactoring Plan

A complete guide to consolidating text editing functionality across all input areas in the Token editor,
enabling consistent behavior, code reuse, and maintainability.

> **Target Audience:** AI agents and developers implementing this refactoring.
> This document provides the complete architectural vision, data structures, implementation phases, and migration strategy.

---

## Table of Contents

1. [Current State Analysis](#chapter-1-current-state-analysis)
2. [Problem Statement](#chapter-2-problem-statement)
3. [Core Abstractions: The TextBuffer Trait](#chapter-3-core-abstractions-the-textbuffer-trait)
4. [The TextInputField Component](#chapter-4-the-textinputfield-component)
5. [Cursor and Selection Unification](#chapter-5-cursor-and-selection-unification)
6. [Edit Operations and Undo/Redo](#chapter-6-edit-operations-and-undoredo)
7. [Message System Consolidation](#chapter-7-message-system-consolidation)
8. [Rendering Pipeline Unification](#chapter-8-rendering-pipeline-unification)
9. [Context-Specific Restrictions](#chapter-9-context-specific-restrictions)
10. [Migration Strategy](#chapter-10-migration-strategy)
11. [File-by-File Implementation Guide](#chapter-11-file-by-file-implementation-guide)
12. [Testing Strategy](#chapter-12-testing-strategy)

**Appendices:**

- [A: Current Implementation Inventory](#appendix-a-current-implementation-inventory)
- [B: Message Type Mappings](#appendix-b-message-type-mappings)
- [C: Keyboard Shortcut Matrix](#appendix-c-keyboard-shortcut-matrix)

---

## Chapter 1: Current State Analysis

### 1.1 Overview of Text Input Areas

The editor currently has **four distinct text input implementations**:

| Input Area | File Location | Text Storage | Cursor Model | Selection Support |
|------------|---------------|--------------|--------------|-------------------|
| **Main Editor** | `src/model/editor.rs` | `Rope` (ropey) | `Cursor` struct with line/column | Full multi-cursor + selections |
| **Command Palette** | `src/model/ui.rs` | `String` | None (end-only) | None |
| **Go-to-Line Dialog** | `src/model/ui.rs` | `String` | None (end-only) | None |
| **CSV Cell Editor** | `src/csv/model.rs` | `String` + `usize` cursor | Byte offset cursor | None |

### 1.2 Main Editor Implementation

Located in `src/model/editor.rs` and `src/model/document.rs`:

```rust
// Current Position representation
pub struct Position {
    pub line: usize,
    pub column: usize,
}

// Current Cursor representation  
pub struct Cursor {
    pub line: usize,
    pub column: usize,
    pub desired_column: Option<usize>,
}

// Current Selection representation
pub struct Selection {
    pub anchor: Position,
    pub head: Position,
}

// Editor state with multi-cursor support
pub struct EditorState {
    pub cursors: Vec<Cursor>,
    pub selections: Vec<Selection>,
    pub active_cursor_index: usize,
    // ... viewport, etc.
}
```

**Capabilities:**
- Multi-line document editing via Rope
- Multiple cursors with parallel selections
- Word-wise movement (Ctrl+Left/Right)
- Word-wise deletion (Ctrl+Backspace/Delete)
- Line operations (duplicate, delete, indent)
- Full undo/redo with `EditOperation` enum
- Rectangle selection mode

### 1.3 Command Palette Implementation

Located in `src/model/ui.rs`:

```rust
pub struct CommandPaletteState {
    pub input: String,           // Just a string
    pub selected_index: usize,   // For list navigation
}
```

**Capabilities:**
- Character insertion (append only)
- Backspace (pop from end)
- Word deletion backward (custom implementation)
- Word movement: **STUBBED** (no-op)

**Missing:**
- Cursor positioning within text
- Arrow key navigation
- Selection
- Delete forward
- Home/End

### 1.4 CSV Cell Editor Implementation

Located in `src/csv/model.rs`:

```rust
pub struct CellEditState {
    pub position: CellPosition,
    pub buffer: String,
    pub cursor: usize,      // Byte offset - at least has cursor!
    pub original: String,
}
```

**Capabilities:**
- Character insertion at cursor
- Cursor movement (left/right/home/end)
- Delete backward/forward
- UTF-8 aware cursor movement

**Missing:**
- Word-wise operations
- Selection
- Undo/redo

### 1.5 Focus and Routing System

Current routing in `src/runtime/input.rs`:

```rust
pub fn handle_key(...) -> Option<Cmd> {
    // Priority order:
    // 1. Modal capture → handle_modal_key()
    // 2. CSV edit capture → handle_csv_edit_key()
    // 3. Sidebar capture → handle_sidebar_key()
    // 4. Normal editor → keymap lookup
}
```

Each handler has its own message types and update functions.

---

## Chapter 2: Problem Statement

### 2.1 Code Duplication

The same text editing operations are implemented multiple times with different behaviors:

| Operation | Main Editor | Command Palette | CSV Cell |
|-----------|-------------|-----------------|----------|
| Insert char | `DocumentMsg::InsertChar` | `ModalMsg::InsertChar` | `CsvMsg::EditInsertChar` |
| Delete backward | `DocumentMsg::DeleteBackward` | `ModalMsg::DeleteBackward` | `CsvMsg::EditDeleteBackward` |
| Word delete | `DocumentMsg::DeleteWordBackward` | Custom `delete_word_backward()` | Not implemented |
| Cursor left | `EditorMsg::MoveCursor(Left)` | Not implemented | `CellEditState::cursor_left()` |

### 2.2 Inconsistent Behavior

Users experience different behaviors depending on context:

1. **Word boundaries**: Main editor uses `CharType` classification; palette uses simple whitespace splitting
2. **Cursor movement**: Works in main editor and CSV cell, but not in palette
3. **Delete forward**: Works in main editor and CSV cell, but not in palette
4. **Home/End**: Works in main editor and CSV cell, but not in palette
5. **Selection**: Only works in main editor

### 2.3 Missing Features by Context

```
┌─────────────────────────────────────────────────────────────────────────┐
│ Feature Matrix                                                          │
├─────────────────────┬───────────┬─────────┬──────────┬─────────────────┤
│ Feature             │ Editor    │ Palette │ GoToLine │ CSV Cell        │
├─────────────────────┼───────────┼─────────┼──────────┼─────────────────┤
│ Insert character    │ ✅        │ ✅      │ ✅       │ ✅              │
│ Delete backward     │ ✅        │ ✅      │ ✅       │ ✅              │
│ Delete forward      │ ✅        │ ❌      │ ❌       │ ✅              │
│ Cursor left/right   │ ✅        │ ❌      │ ❌       │ ✅              │
│ Home/End            │ ✅        │ ❌      │ ❌       │ ✅              │
│ Word left/right     │ ✅        │ ❌      │ ❌       │ ❌              │
│ Word delete back    │ ✅        │ ✅*     │ ✅*      │ ❌              │
│ Word delete fwd     │ ✅        │ ❌      │ ❌       │ ❌              │
│ Select all          │ ✅        │ ❌      │ ❌       │ ❌              │
│ Selection extend    │ ✅        │ ❌      │ ❌       │ ❌              │
│ Cut/Copy/Paste      │ ✅        │ ❌      │ ❌       │ ❌              │
│ Undo/Redo           │ ✅        │ ❌      │ ❌       │ ❌              │
│ Multi-cursor        │ ✅        │ N/A     │ N/A      │ N/A             │
└─────────────────────┴───────────┴─────────┴──────────┴─────────────────┘
* Different implementation than main editor
```

### 2.4 Goals of This Refactoring

1. **Single source of truth** for text editing logic
2. **Consistent behavior** across all input areas
3. **Easy extensibility** for new input areas
4. **Context-specific restrictions** (e.g., no multi-cursor in palette)
5. **Shared rendering code** for cursors and selections
6. **Optional undo/redo** per context

---

## Chapter 3: Core Abstractions: The TextBuffer Trait

### 3.1 Design Philosophy

We need a trait that abstracts over different text storage backends:
- `Rope` for the main editor (efficient for large documents)
- `String` for single-line inputs (simple, efficient for small text)

The trait should provide a unified interface for all text operations.

### 3.2 The TextBuffer Trait

```rust
// src/model/text_buffer.rs (NEW FILE)

/// A trait abstracting over text storage backends.
/// Implementations: RopeBuffer (wraps Rope), StringBuffer (wraps String)
pub trait TextBuffer {
    /// Total length in bytes
    fn len_bytes(&self) -> usize;
    
    /// Total length in characters (grapheme clusters for full Unicode support)
    fn len_chars(&self) -> usize;
    
    /// Number of lines (always >= 1)
    fn line_count(&self) -> usize;
    
    /// Get the text content of a specific line (0-indexed), without trailing newline
    fn line(&self, line_idx: usize) -> Option<Cow<str>>;
    
    /// Get length of a line in characters
    fn line_len_chars(&self, line_idx: usize) -> usize;
    
    /// Convert (line, column) to byte offset
    fn position_to_offset(&self, line: usize, column: usize) -> usize;
    
    /// Convert byte offset to (line, column)
    fn offset_to_position(&self, offset: usize) -> (usize, usize);
    
    /// Insert text at byte offset
    fn insert(&mut self, offset: usize, text: &str);
    
    /// Insert a single character at byte offset
    fn insert_char(&mut self, offset: usize, ch: char);
    
    /// Remove text in byte range
    fn remove(&mut self, range: Range<usize>);
    
    /// Get a slice of text as String
    fn slice(&self, range: Range<usize>) -> String;
    
    /// Get full content as String (may be expensive for large buffers)
    fn to_string(&self) -> String;
    
    /// Check if buffer is empty
    fn is_empty(&self) -> bool {
        self.len_bytes() == 0
    }
}
```

### 3.3 RopeBuffer Implementation

```rust
// src/model/text_buffer.rs

use ropey::Rope;

/// TextBuffer implementation wrapping ropey::Rope
/// Used for multi-line document editing
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

impl TextBuffer for RopeBuffer {
    fn len_bytes(&self) -> usize {
        self.rope.len_bytes()
    }
    
    fn len_chars(&self) -> usize {
        self.rope.len_chars()
    }
    
    fn line_count(&self) -> usize {
        self.rope.len_lines()
    }
    
    fn line(&self, line_idx: usize) -> Option<Cow<str>> {
        if line_idx >= self.rope.len_lines() {
            return None;
        }
        let line = self.rope.line(line_idx);
        // Strip trailing newline if present
        let s = line.to_string();
        let trimmed = s.trim_end_matches(&['\n', '\r'][..]);
        Some(Cow::Owned(trimmed.to_string()))
    }
    
    fn line_len_chars(&self, line_idx: usize) -> usize {
        self.line(line_idx).map(|l| l.chars().count()).unwrap_or(0)
    }
    
    fn position_to_offset(&self, line: usize, column: usize) -> usize {
        let line_start = self.rope.line_to_byte(line.min(self.line_count().saturating_sub(1)));
        let line_text = self.line(line).unwrap_or(Cow::Borrowed(""));
        let col_bytes: usize = line_text.chars().take(column).map(|c| c.len_utf8()).sum();
        line_start + col_bytes
    }
    
    fn offset_to_position(&self, offset: usize) -> (usize, usize) {
        let offset = offset.min(self.len_bytes());
        let line = self.rope.byte_to_line(offset);
        let line_start = self.rope.line_to_byte(line);
        let col_bytes = offset - line_start;
        let line_text = self.line(line).unwrap_or(Cow::Borrowed(""));
        let column = line_text.chars()
            .scan(0usize, |acc, c| {
                let prev = *acc;
                *acc += c.len_utf8();
                Some((prev, c))
            })
            .take_while(|(byte_pos, _)| *byte_pos < col_bytes)
            .count();
        (line, column)
    }
    
    fn insert(&mut self, offset: usize, text: &str) {
        let char_idx = self.rope.byte_to_char(offset.min(self.len_bytes()));
        self.rope.insert(char_idx, text);
    }
    
    fn insert_char(&mut self, offset: usize, ch: char) {
        let char_idx = self.rope.byte_to_char(offset.min(self.len_bytes()));
        self.rope.insert_char(char_idx, ch);
    }
    
    fn remove(&mut self, range: Range<usize>) {
        let start_char = self.rope.byte_to_char(range.start.min(self.len_bytes()));
        let end_char = self.rope.byte_to_char(range.end.min(self.len_bytes()));
        self.rope.remove(start_char..end_char);
    }
    
    fn slice(&self, range: Range<usize>) -> String {
        let start_char = self.rope.byte_to_char(range.start.min(self.len_bytes()));
        let end_char = self.rope.byte_to_char(range.end.min(self.len_bytes()));
        self.rope.slice(start_char..end_char).to_string()
    }
    
    fn to_string(&self) -> String {
        self.rope.to_string()
    }
}
```

### 3.4 StringBuffer Implementation

```rust
// src/model/text_buffer.rs

/// TextBuffer implementation wrapping String
/// Used for single-line inputs (command palette, dialogs, CSV cells)
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
    
    pub fn into_string(self) -> String {
        self.text
    }
}

impl TextBuffer for StringBuffer {
    fn len_bytes(&self) -> usize {
        self.text.len()
    }
    
    fn len_chars(&self) -> usize {
        self.text.chars().count()
    }
    
    fn line_count(&self) -> usize {
        1  // Single-line buffer
    }
    
    fn line(&self, line_idx: usize) -> Option<Cow<str>> {
        if line_idx == 0 {
            Some(Cow::Borrowed(&self.text))
        } else {
            None
        }
    }
    
    fn line_len_chars(&self, line_idx: usize) -> usize {
        if line_idx == 0 {
            self.text.chars().count()
        } else {
            0
        }
    }
    
    fn position_to_offset(&self, _line: usize, column: usize) -> usize {
        // For single-line buffer, ignore line parameter
        self.text.chars()
            .take(column)
            .map(|c| c.len_utf8())
            .sum()
    }
    
    fn offset_to_position(&self, offset: usize) -> (usize, usize) {
        let column = self.text[..offset.min(self.text.len())]
            .chars()
            .count();
        (0, column)
    }
    
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
        self.text.drain(start..end);
    }
    
    fn slice(&self, range: Range<usize>) -> String {
        let start = range.start.min(self.text.len());
        let end = range.end.min(self.text.len());
        self.text[start..end].to_string()
    }
    
    fn to_string(&self) -> String {
        self.text.clone()
    }
}
```

### 3.5 Word Boundary Utilities

```rust
// src/model/text_buffer.rs (or src/util/text.rs)

/// Character classification for word operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CharType {
    Whitespace,
    WordChar,
    Punctuation,
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

/// Find the start of the previous word from offset
pub fn word_start_before<B: TextBuffer>(buffer: &B, offset: usize) -> usize {
    let text = buffer.slice(0..offset);
    let chars: Vec<char> = text.chars().collect();
    let mut i = chars.len();
    
    if i == 0 {
        return 0;
    }
    
    // Skip current char type
    let current_type = char_type(chars[i - 1]);
    while i > 0 && char_type(chars[i - 1]) == current_type {
        i -= 1;
    }
    
    // Convert char index back to byte offset
    chars[..i].iter().map(|c| c.len_utf8()).sum()
}

/// Find the end of the next word from offset
pub fn word_end_after<B: TextBuffer>(buffer: &B, offset: usize) -> usize {
    let text = buffer.slice(offset..buffer.len_bytes());
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    
    if chars.is_empty() {
        return offset;
    }
    
    // Skip current char type
    let current_type = char_type(chars[0]);
    while i < chars.len() && char_type(chars[i]) == current_type {
        i += 1;
    }
    
    // Convert char index to byte offset
    offset + chars[..i].iter().map(|c| c.len_utf8()).sum::<usize>()
}
```

---

## Chapter 4: The TextInputField Component

### 4.1 Design Goals

The `TextInputField` is a reusable component that provides:
- Single-line or multi-line text editing
- Cursor management (single cursor for inputs, multi-cursor optional)
- Selection support (optional)
- Word-wise operations using consistent `CharType` logic
- Undo/redo (optional)

### 4.2 Core Structures

```rust
// src/model/text_input.rs (NEW FILE)

use crate::model::text_buffer::{TextBuffer, StringBuffer, CharType, char_type};

/// A cursor position within a TextInputField
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct InputCursor {
    /// Byte offset in the buffer
    pub offset: usize,
    /// For multi-line: desired column when moving vertically
    pub desired_column: Option<usize>,
}

impl InputCursor {
    pub fn new(offset: usize) -> Self {
        Self { offset, desired_column: None }
    }
    
    pub fn at_end<B: TextBuffer>(buffer: &B) -> Self {
        Self::new(buffer.len_bytes())
    }
}

/// A selection in the input field
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct InputSelection {
    /// Where selection started (anchor)
    pub anchor: usize,
    /// Where cursor is (head) - same as InputCursor.offset
    pub head: usize,
}

impl InputSelection {
    pub fn empty(offset: usize) -> Self {
        Self { anchor: offset, head: offset }
    }
    
    pub fn is_empty(&self) -> bool {
        self.anchor == self.head
    }
    
    pub fn start(&self) -> usize {
        self.anchor.min(self.head)
    }
    
    pub fn end(&self) -> usize {
        self.anchor.max(self.head)
    }
    
    pub fn range(&self) -> Range<usize> {
        self.start()..self.end()
    }
}

/// Configuration for a TextInputField
#[derive(Debug, Clone)]
pub struct InputFieldConfig {
    /// Allow multiple lines (Enter inserts newline vs confirms)
    pub multiline: bool,
    /// Allow multiple cursors
    pub multi_cursor: bool,
    /// Allow text selection
    pub allow_selection: bool,
    /// Enable undo/redo tracking
    pub enable_undo: bool,
    /// Maximum length in characters (None = unlimited)
    pub max_length: Option<usize>,
    /// Allowed character filter (None = all allowed)
    pub char_filter: Option<fn(char) -> bool>,
}

impl Default for InputFieldConfig {
    fn default() -> Self {
        Self {
            multiline: false,
            multi_cursor: false,
            allow_selection: true,
            enable_undo: false,
            max_length: None,
            char_filter: None,
        }
    }
}

impl InputFieldConfig {
    /// Config for command palette / dialogs
    pub fn single_line() -> Self {
        Self::default()
    }
    
    /// Config for go-to-line (numbers only)
    pub fn numeric() -> Self {
        Self {
            char_filter: Some(|c| c.is_ascii_digit()),
            ..Self::default()
        }
    }
    
    /// Config for main editor
    pub fn editor() -> Self {
        Self {
            multiline: true,
            multi_cursor: true,
            allow_selection: true,
            enable_undo: true,
            max_length: None,
            char_filter: None,
        }
    }
    
    /// Config for CSV cell
    pub fn csv_cell() -> Self {
        Self {
            multiline: false,
            multi_cursor: false,
            allow_selection: true,
            enable_undo: false,  // CSV has its own undo via original value
            max_length: None,
            char_filter: None,
        }
    }
}
```

### 4.3 The TextInputField Struct

```rust
// src/model/text_input.rs (continued)

/// A reusable text input field component
pub struct TextInputField<B: TextBuffer = StringBuffer> {
    /// The text buffer
    pub buffer: B,
    /// Cursor position(s) - always at least one
    pub cursors: Vec<InputCursor>,
    /// Selection(s) - parallel to cursors
    pub selections: Vec<InputSelection>,
    /// Active cursor index
    pub active_cursor: usize,
    /// Configuration
    pub config: InputFieldConfig,
    /// Undo stack (if enabled)
    undo_stack: Vec<InputEdit>,
    /// Redo stack (if enabled)
    redo_stack: Vec<InputEdit>,
}

/// An edit operation for undo/redo
#[derive(Debug, Clone)]
pub struct InputEdit {
    /// Byte range that was affected
    pub range: Range<usize>,
    /// Text that was deleted (empty for pure insert)
    pub deleted: String,
    /// Text that was inserted (empty for pure delete)
    pub inserted: String,
    /// Cursor state before edit
    pub cursor_before: usize,
    /// Cursor state after edit
    pub cursor_after: usize,
}

impl<B: TextBuffer> TextInputField<B> {
    pub fn new(buffer: B, config: InputFieldConfig) -> Self {
        Self {
            cursors: vec![InputCursor::at_end(&buffer)],
            selections: vec![InputSelection::empty(buffer.len_bytes())],
            active_cursor: 0,
            buffer,
            config,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }
    
    /// Get the primary cursor
    pub fn cursor(&self) -> &InputCursor {
        &self.cursors[self.active_cursor]
    }
    
    /// Get the primary selection
    pub fn selection(&self) -> &InputSelection {
        &self.selections[self.active_cursor]
    }
    
    /// Get text content
    pub fn text(&self) -> String {
        self.buffer.to_string()
    }
    
    /// Check if there's a non-empty selection
    pub fn has_selection(&self) -> bool {
        self.selections.iter().any(|s| !s.is_empty())
    }
}
```

### 4.4 Cursor Movement Operations

```rust
// src/model/text_input.rs (continued)

impl<B: TextBuffer> TextInputField<B> {
    /// Move cursor left by one character
    pub fn cursor_left(&mut self, extend_selection: bool) {
        for i in 0..self.cursors.len() {
            let offset = self.cursors[i].offset;
            if offset > 0 {
                // Find previous character boundary
                let text = self.buffer.slice(0..offset);
                let new_offset = text.char_indices()
                    .last()
                    .map(|(i, _)| i)
                    .unwrap_or(0);
                self.cursors[i].offset = new_offset;
                self.cursors[i].desired_column = None;
            }
            self.update_selection(i, extend_selection);
        }
    }
    
    /// Move cursor right by one character
    pub fn cursor_right(&mut self, extend_selection: bool) {
        for i in 0..self.cursors.len() {
            let offset = self.cursors[i].offset;
            if offset < self.buffer.len_bytes() {
                let text = self.buffer.slice(offset..self.buffer.len_bytes());
                if let Some((_, ch)) = text.char_indices().next() {
                    self.cursors[i].offset = offset + ch.len_utf8();
                    self.cursors[i].desired_column = None;
                }
            }
            self.update_selection(i, extend_selection);
        }
    }
    
    /// Move cursor to start of line (or buffer for single-line)
    pub fn cursor_home(&mut self, extend_selection: bool) {
        for i in 0..self.cursors.len() {
            let (line, _) = self.buffer.offset_to_position(self.cursors[i].offset);
            self.cursors[i].offset = self.buffer.position_to_offset(line, 0);
            self.cursors[i].desired_column = Some(0);
            self.update_selection(i, extend_selection);
        }
    }
    
    /// Move cursor to end of line (or buffer for single-line)
    pub fn cursor_end(&mut self, extend_selection: bool) {
        for i in 0..self.cursors.len() {
            let (line, _) = self.buffer.offset_to_position(self.cursors[i].offset);
            let line_len = self.buffer.line_len_chars(line);
            self.cursors[i].offset = self.buffer.position_to_offset(line, line_len);
            self.cursors[i].desired_column = Some(usize::MAX);
            self.update_selection(i, extend_selection);
        }
    }
    
    /// Move cursor left by one word
    pub fn cursor_word_left(&mut self, extend_selection: bool) {
        for i in 0..self.cursors.len() {
            let new_offset = word_start_before(&self.buffer, self.cursors[i].offset);
            self.cursors[i].offset = new_offset;
            self.cursors[i].desired_column = None;
            self.update_selection(i, extend_selection);
        }
    }
    
    /// Move cursor right by one word
    pub fn cursor_word_right(&mut self, extend_selection: bool) {
        for i in 0..self.cursors.len() {
            let new_offset = word_end_after(&self.buffer, self.cursors[i].offset);
            self.cursors[i].offset = new_offset;
            self.cursors[i].desired_column = None;
            self.update_selection(i, extend_selection);
        }
    }
    
    /// Helper to update selection based on cursor movement
    fn update_selection(&mut self, cursor_idx: usize, extend: bool) {
        if !self.config.allow_selection {
            // Collapse selection to cursor
            self.selections[cursor_idx] = InputSelection::empty(self.cursors[cursor_idx].offset);
        } else if extend {
            // Extend selection: anchor stays, head moves with cursor
            self.selections[cursor_idx].head = self.cursors[cursor_idx].offset;
        } else {
            // Collapse selection to cursor
            self.selections[cursor_idx] = InputSelection::empty(self.cursors[cursor_idx].offset);
        }
    }
    
    /// Select all text
    pub fn select_all(&mut self) {
        if !self.config.allow_selection {
            return;
        }
        // Collapse to single cursor selecting all
        self.cursors = vec![InputCursor::new(self.buffer.len_bytes())];
        self.selections = vec![InputSelection {
            anchor: 0,
            head: self.buffer.len_bytes(),
        }];
        self.active_cursor = 0;
    }
}
```

### 4.5 Text Editing Operations

```rust
// src/model/text_input.rs (continued)

impl<B: TextBuffer> TextInputField<B> {
    /// Insert a character at cursor position(s)
    pub fn insert_char(&mut self, ch: char) -> bool {
        // Check character filter
        if let Some(filter) = self.config.char_filter {
            if !filter(ch) {
                return false;
            }
        }
        
        // Check max length
        if let Some(max) = self.config.max_length {
            if self.buffer.len_chars() >= max && !self.has_selection() {
                return false;
            }
        }
        
        // Process cursors in reverse order to preserve offsets
        let mut indices: Vec<usize> = (0..self.cursors.len()).collect();
        indices.sort_by_key(|&i| std::cmp::Reverse(self.cursors[i].offset));
        
        for &i in &indices {
            let selection = self.selections[i];
            
            if !selection.is_empty() {
                // Delete selection first
                self.buffer.remove(selection.range());
                self.cursors[i].offset = selection.start();
            }
            
            // Insert character
            self.buffer.insert_char(self.cursors[i].offset, ch);
            self.cursors[i].offset += ch.len_utf8();
            
            // Collapse selection
            self.selections[i] = InputSelection::empty(self.cursors[i].offset);
        }
        
        self.redo_stack.clear();
        true
    }
    
    /// Insert a string at cursor position(s)
    pub fn insert_text(&mut self, text: &str) -> bool {
        // Check character filter for all chars
        if let Some(filter) = self.config.char_filter {
            if !text.chars().all(filter) {
                return false;
            }
        }
        
        // Process cursors in reverse order
        let mut indices: Vec<usize> = (0..self.cursors.len()).collect();
        indices.sort_by_key(|&i| std::cmp::Reverse(self.cursors[i].offset));
        
        for &i in &indices {
            let selection = self.selections[i];
            
            if !selection.is_empty() {
                self.buffer.remove(selection.range());
                self.cursors[i].offset = selection.start();
            }
            
            self.buffer.insert(self.cursors[i].offset, text);
            self.cursors[i].offset += text.len();
            self.selections[i] = InputSelection::empty(self.cursors[i].offset);
        }
        
        self.redo_stack.clear();
        true
    }
    
    /// Delete backward (backspace)
    pub fn delete_backward(&mut self) {
        let mut indices: Vec<usize> = (0..self.cursors.len()).collect();
        indices.sort_by_key(|&i| std::cmp::Reverse(self.cursors[i].offset));
        
        for &i in &indices {
            let selection = self.selections[i];
            
            if !selection.is_empty() {
                // Delete selection
                self.buffer.remove(selection.range());
                self.cursors[i].offset = selection.start();
            } else if self.cursors[i].offset > 0 {
                // Find previous char boundary
                let text = self.buffer.slice(0..self.cursors[i].offset);
                let new_offset = text.char_indices()
                    .last()
                    .map(|(idx, _)| idx)
                    .unwrap_or(0);
                self.buffer.remove(new_offset..self.cursors[i].offset);
                self.cursors[i].offset = new_offset;
            }
            
            self.selections[i] = InputSelection::empty(self.cursors[i].offset);
        }
        
        self.redo_stack.clear();
    }
    
    /// Delete forward (delete key)
    pub fn delete_forward(&mut self) {
        let mut indices: Vec<usize> = (0..self.cursors.len()).collect();
        indices.sort_by_key(|&i| std::cmp::Reverse(self.cursors[i].offset));
        
        for &i in &indices {
            let selection = self.selections[i];
            
            if !selection.is_empty() {
                self.buffer.remove(selection.range());
                self.cursors[i].offset = selection.start();
            } else if self.cursors[i].offset < self.buffer.len_bytes() {
                // Find next char boundary
                let text = self.buffer.slice(self.cursors[i].offset..self.buffer.len_bytes());
                if let Some((_, ch)) = text.char_indices().next() {
                    self.buffer.remove(self.cursors[i].offset..self.cursors[i].offset + ch.len_utf8());
                }
            }
            
            self.selections[i] = InputSelection::empty(self.cursors[i].offset);
        }
        
        self.redo_stack.clear();
    }
    
    /// Delete word backward (Ctrl+Backspace / Option+Backspace)
    pub fn delete_word_backward(&mut self) {
        let mut indices: Vec<usize> = (0..self.cursors.len()).collect();
        indices.sort_by_key(|&i| std::cmp::Reverse(self.cursors[i].offset));
        
        for &i in &indices {
            let selection = self.selections[i];
            
            if !selection.is_empty() {
                self.buffer.remove(selection.range());
                self.cursors[i].offset = selection.start();
            } else {
                let word_start = word_start_before(&self.buffer, self.cursors[i].offset);
                self.buffer.remove(word_start..self.cursors[i].offset);
                self.cursors[i].offset = word_start;
            }
            
            self.selections[i] = InputSelection::empty(self.cursors[i].offset);
        }
        
        self.redo_stack.clear();
    }
    
    /// Delete word forward (Ctrl+Delete / Option+Delete)
    pub fn delete_word_forward(&mut self) {
        let mut indices: Vec<usize> = (0..self.cursors.len()).collect();
        indices.sort_by_key(|&i| std::cmp::Reverse(self.cursors[i].offset));
        
        for &i in &indices {
            let selection = self.selections[i];
            
            if !selection.is_empty() {
                self.buffer.remove(selection.range());
                self.cursors[i].offset = selection.start();
            } else {
                let word_end = word_end_after(&self.buffer, self.cursors[i].offset);
                self.buffer.remove(self.cursors[i].offset..word_end);
            }
            
            self.selections[i] = InputSelection::empty(self.cursors[i].offset);
        }
        
        self.redo_stack.clear();
    }
}
```

---

## Chapter 5: Cursor and Selection Unification

### 5.1 Current State: Dual Representation Problem

The main editor uses `Position { line, column }` and `Cursor { line, column, desired_column }`, while single-line inputs would use byte offsets. We need to bridge these representations.

### 5.2 Position Adapter

```rust
// src/model/text_input.rs

impl<B: TextBuffer> TextInputField<B> {
    /// Get cursor position as (line, column) - useful for rendering
    pub fn cursor_position(&self) -> (usize, usize) {
        self.buffer.offset_to_position(self.cursor().offset)
    }
    
    /// Get cursor position for a specific cursor index
    pub fn cursor_position_at(&self, idx: usize) -> (usize, usize) {
        self.buffer.offset_to_position(self.cursors[idx].offset)
    }
    
    /// Set cursor from (line, column) position
    pub fn set_cursor_position(&mut self, line: usize, column: usize) {
        let offset = self.buffer.position_to_offset(line, column);
        self.cursors[self.active_cursor].offset = offset;
        self.selections[self.active_cursor] = InputSelection::empty(offset);
    }
    
    /// Get character column (for rendering with char_width)
    pub fn cursor_char_column(&self) -> usize {
        let (_, col) = self.cursor_position();
        col
    }
}
```

### 5.3 Multi-Cursor Operations (Editor Only)

```rust
// src/model/text_input.rs

impl<B: TextBuffer> TextInputField<B> {
    /// Add a cursor above the current one (for multi-line only)
    pub fn add_cursor_above(&mut self) {
        if !self.config.multi_cursor || !self.config.multiline {
            return;
        }
        
        let (line, col) = self.cursor_position();
        if line == 0 {
            return;
        }
        
        let new_col = self.cursors[self.active_cursor].desired_column.unwrap_or(col);
        let target_line_len = self.buffer.line_len_chars(line - 1);
        let actual_col = new_col.min(target_line_len);
        let new_offset = self.buffer.position_to_offset(line - 1, actual_col);
        
        // Check for duplicate
        if self.cursors.iter().any(|c| c.offset == new_offset) {
            return;
        }
        
        let new_cursor = InputCursor {
            offset: new_offset,
            desired_column: Some(new_col),
        };
        
        self.cursors.push(new_cursor);
        self.selections.push(InputSelection::empty(new_offset));
        self.sort_and_merge_cursors();
    }
    
    /// Add a cursor below the current one
    pub fn add_cursor_below(&mut self) {
        if !self.config.multi_cursor || !self.config.multiline {
            return;
        }
        
        let (line, col) = self.cursor_position();
        if line >= self.buffer.line_count() - 1 {
            return;
        }
        
        let new_col = self.cursors[self.active_cursor].desired_column.unwrap_or(col);
        let target_line_len = self.buffer.line_len_chars(line + 1);
        let actual_col = new_col.min(target_line_len);
        let new_offset = self.buffer.position_to_offset(line + 1, actual_col);
        
        if self.cursors.iter().any(|c| c.offset == new_offset) {
            return;
        }
        
        let new_cursor = InputCursor {
            offset: new_offset,
            desired_column: Some(new_col),
        };
        
        self.cursors.push(new_cursor);
        self.selections.push(InputSelection::empty(new_offset));
        self.sort_and_merge_cursors();
    }
    
    /// Sort cursors by position and merge overlapping ones
    fn sort_and_merge_cursors(&mut self) {
        // Sort by offset
        let mut combined: Vec<(InputCursor, InputSelection)> = 
            self.cursors.iter().cloned()
                .zip(self.selections.iter().cloned())
                .collect();
        combined.sort_by_key(|(c, _)| c.offset);
        
        // Remove duplicates
        combined.dedup_by(|a, b| a.0.offset == b.0.offset);
        
        self.cursors = combined.iter().map(|(c, _)| *c).collect();
        self.selections = combined.iter().map(|(_, s)| *s).collect();
        
        // Ensure active_cursor is valid
        self.active_cursor = self.active_cursor.min(self.cursors.len().saturating_sub(1));
    }
    
    /// Collapse to single cursor (Escape)
    pub fn collapse_to_single_cursor(&mut self) {
        if self.cursors.len() > 1 {
            let cursor = self.cursors[self.active_cursor];
            let selection = self.selections[self.active_cursor];
            self.cursors = vec![cursor];
            self.selections = vec![selection];
            self.active_cursor = 0;
        }
    }
}
```

### 5.4 Selection Text Operations

```rust
// src/model/text_input.rs

impl<B: TextBuffer> TextInputField<B> {
    /// Get selected text for the primary selection
    pub fn selected_text(&self) -> String {
        let sel = self.selection();
        if sel.is_empty() {
            return String::new();
        }
        self.buffer.slice(sel.range())
    }
    
    /// Get all selected texts (for multi-cursor)
    pub fn all_selected_texts(&self) -> Vec<String> {
        self.selections.iter()
            .map(|sel| {
                if sel.is_empty() {
                    String::new()
                } else {
                    self.buffer.slice(sel.range())
                }
            })
            .collect()
    }
    
    /// Cut selected text (returns cut text for clipboard)
    pub fn cut(&mut self) -> String {
        let text = self.selected_text();
        if !text.is_empty() {
            self.delete_backward();  // Deletes selection
        }
        text
    }
    
    /// Copy selected text (returns copied text for clipboard)
    pub fn copy(&self) -> String {
        self.selected_text()
    }
    
    /// Paste text at cursor (replaces selection if any)
    pub fn paste(&mut self, text: &str) {
        self.insert_text(text);
    }
}
```

---

## Chapter 6: Edit Operations and Undo/Redo

### 6.1 Undo/Redo Architecture

The main editor currently has a complex `EditOperation` enum with multiple variants. For the unified system, we'll use a simpler approach that works for all contexts.

### 6.2 InputEdit Structure

```rust
// src/model/text_input.rs

/// Represents a single atomic edit that can be undone/redone
#[derive(Debug, Clone)]
pub struct InputEdit {
    /// Byte offset where the edit occurred
    pub offset: usize,
    /// Text that was deleted (empty for pure insert)
    pub deleted_text: String,
    /// Text that was inserted (empty for pure delete)
    pub inserted_text: String,
    /// Cursor offset before the edit
    pub cursor_before: usize,
    /// Cursor offset after the edit
    pub cursor_after: usize,
}

impl InputEdit {
    /// Create an insert-only edit
    pub fn insert(offset: usize, text: String, cursor_before: usize, cursor_after: usize) -> Self {
        Self {
            offset,
            deleted_text: String::new(),
            inserted_text: text,
            cursor_before,
            cursor_after,
        }
    }
    
    /// Create a delete-only edit
    pub fn delete(offset: usize, text: String, cursor_before: usize, cursor_after: usize) -> Self {
        Self {
            offset,
            deleted_text: text,
            inserted_text: String::new(),
            cursor_before,
            cursor_after,
        }
    }
    
    /// Create a replace edit (delete + insert)
    pub fn replace(
        offset: usize, 
        deleted: String, 
        inserted: String,
        cursor_before: usize,
        cursor_after: usize
    ) -> Self {
        Self {
            offset,
            deleted_text: deleted,
            inserted_text: inserted,
            cursor_before,
            cursor_after,
        }
    }
    
    /// Invert this edit (for undo)
    pub fn invert(&self) -> Self {
        Self {
            offset: self.offset,
            deleted_text: self.inserted_text.clone(),
            inserted_text: self.deleted_text.clone(),
            cursor_before: self.cursor_after,
            cursor_after: self.cursor_before,
        }
    }
}
```

### 6.3 Undo/Redo Operations

```rust
// src/model/text_input.rs

impl<B: TextBuffer> TextInputField<B> {
    /// Push an edit to the undo stack (clears redo)
    fn push_edit(&mut self, edit: InputEdit) {
        if self.config.enable_undo {
            self.undo_stack.push(edit);
            self.redo_stack.clear();
        }
    }
    
    /// Undo the last edit
    pub fn undo(&mut self) -> bool {
        if !self.config.enable_undo {
            return false;
        }
        
        if let Some(edit) = self.undo_stack.pop() {
            // Apply inverse operation
            if !edit.inserted_text.is_empty() {
                // Undo an insert = delete
                let end = edit.offset + edit.inserted_text.len();
                self.buffer.remove(edit.offset..end);
            }
            if !edit.deleted_text.is_empty() {
                // Undo a delete = insert
                self.buffer.insert(edit.offset, &edit.deleted_text);
            }
            
            // Restore cursor
            self.cursors = vec![InputCursor::new(edit.cursor_before)];
            self.selections = vec![InputSelection::empty(edit.cursor_before)];
            self.active_cursor = 0;
            
            // Move to redo stack
            self.redo_stack.push(edit);
            return true;
        }
        false
    }
    
    /// Redo the last undone edit
    pub fn redo(&mut self) -> bool {
        if !self.config.enable_undo {
            return false;
        }
        
        if let Some(edit) = self.redo_stack.pop() {
            // Apply operation
            if !edit.deleted_text.is_empty() {
                // Redo a delete
                let end = edit.offset + edit.deleted_text.len();
                self.buffer.remove(edit.offset..end);
            }
            if !edit.inserted_text.is_empty() {
                // Redo an insert
                self.buffer.insert(edit.offset, &edit.inserted_text);
            }
            
            // Restore cursor
            self.cursors = vec![InputCursor::new(edit.cursor_after)];
            self.selections = vec![InputSelection::empty(edit.cursor_after)];
            self.active_cursor = 0;
            
            // Move back to undo stack
            self.undo_stack.push(edit);
            return true;
        }
        false
    }
    
    /// Check if undo is available
    pub fn can_undo(&self) -> bool {
        self.config.enable_undo && !self.undo_stack.is_empty()
    }
    
    /// Check if redo is available
    pub fn can_redo(&self) -> bool {
        self.config.enable_undo && !self.redo_stack.is_empty()
    }
    
    /// Clear undo/redo history
    pub fn clear_history(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
    }
}
```

### 6.4 Integration with Main Editor Document

For the main editor, we may want to keep the existing `EditOperation` system for its batch capabilities. The `TextInputField` can be used as the edit engine, with a wrapper that translates to `EditOperation`.

```rust
// src/model/document.rs (updated)

impl Document {
    /// Apply an edit from TextInputField to the document's undo system
    pub fn record_input_edit(&mut self, edit: InputEdit, cursors: &[Cursor]) {
        // Convert InputEdit to our EditOperation format
        let operation = if edit.deleted_text.is_empty() {
            EditOperation::Insert {
                position: edit.offset,
                text: edit.inserted_text,
                cursor_before: cursors[0],  // Simplified
                cursor_after: cursors[0],
            }
        } else if edit.inserted_text.is_empty() {
            EditOperation::Delete {
                position: edit.offset,
                text: edit.deleted_text,
                cursor_before: cursors[0],
                cursor_after: cursors[0],
            }
        } else {
            EditOperation::Replace {
                position: edit.offset,
                deleted_text: edit.deleted_text,
                inserted_text: edit.inserted_text,
                cursor_before: cursors[0],
                cursor_after: cursors[0],
            }
        };
        
        self.push_edit(operation);
    }
}
```

---

## Chapter 7: Message System Consolidation

### 7.1 Current Problem: Message Fragmentation

Currently we have three separate message types for text editing:
- `EditorMsg` + `DocumentMsg` for main editor
- `ModalMsg` for command palette/dialogs
- `CsvMsg::Edit*` for CSV cells

### 7.2 Unified TextInputMsg

```rust
// src/messages.rs (new addition)

/// Unified message type for all text input operations
#[derive(Debug, Clone)]
pub enum TextInputMsg {
    // === Character Input ===
    InsertChar(char),
    InsertText(String),
    
    // === Deletion ===
    DeleteBackward,
    DeleteForward,
    DeleteWordBackward,
    DeleteWordForward,
    DeleteLine,
    
    // === Cursor Movement ===
    CursorLeft,
    CursorRight,
    CursorUp,
    CursorDown,
    CursorHome,
    CursorEnd,
    CursorWordLeft,
    CursorWordRight,
    CursorDocumentStart,
    CursorDocumentEnd,
    
    // === Selection ===
    SelectLeft,
    SelectRight,
    SelectUp,
    SelectDown,
    SelectHome,
    SelectEnd,
    SelectWordLeft,
    SelectWordRight,
    SelectAll,
    SelectLine,
    SelectWord,
    
    // === Multi-Cursor (editor only, ignored by single-line inputs) ===
    AddCursorAbove,
    AddCursorBelow,
    AddCursorAtNextOccurrence,
    AddCursorsAtAllOccurrences,
    CollapseCursors,
    
    // === Clipboard ===
    Cut,
    Copy,
    Paste(String),
    
    // === Undo/Redo ===
    Undo,
    Redo,
    
    // === Newline (context-dependent) ===
    InsertNewline,  // In editor: insert \n, in modal: confirm
}
```

### 7.3 TextInputField Message Handler

```rust
// src/model/text_input.rs

impl<B: TextBuffer> TextInputField<B> {
    /// Handle a TextInputMsg, returning true if handled
    pub fn handle_message(&mut self, msg: TextInputMsg) -> bool {
        match msg {
            // Character input
            TextInputMsg::InsertChar(ch) => self.insert_char(ch),
            TextInputMsg::InsertText(text) => self.insert_text(&text),
            
            // Deletion
            TextInputMsg::DeleteBackward => { self.delete_backward(); true }
            TextInputMsg::DeleteForward => { self.delete_forward(); true }
            TextInputMsg::DeleteWordBackward => { self.delete_word_backward(); true }
            TextInputMsg::DeleteWordForward => { self.delete_word_forward(); true }
            TextInputMsg::DeleteLine => {
                if self.config.multiline { self.delete_line(); true }
                else { false }
            }
            
            // Cursor movement (without selection)
            TextInputMsg::CursorLeft => { self.cursor_left(false); true }
            TextInputMsg::CursorRight => { self.cursor_right(false); true }
            TextInputMsg::CursorUp => {
                if self.config.multiline { self.cursor_up(false); true }
                else { false }
            }
            TextInputMsg::CursorDown => {
                if self.config.multiline { self.cursor_down(false); true }
                else { false }
            }
            TextInputMsg::CursorHome => { self.cursor_home(false); true }
            TextInputMsg::CursorEnd => { self.cursor_end(false); true }
            TextInputMsg::CursorWordLeft => { self.cursor_word_left(false); true }
            TextInputMsg::CursorWordRight => { self.cursor_word_right(false); true }
            TextInputMsg::CursorDocumentStart => { self.cursor_document_start(false); true }
            TextInputMsg::CursorDocumentEnd => { self.cursor_document_end(false); true }
            
            // Selection (cursor movement with extend)
            TextInputMsg::SelectLeft => { self.cursor_left(true); true }
            TextInputMsg::SelectRight => { self.cursor_right(true); true }
            TextInputMsg::SelectUp => {
                if self.config.multiline { self.cursor_up(true); true }
                else { false }
            }
            TextInputMsg::SelectDown => {
                if self.config.multiline { self.cursor_down(true); true }
                else { false }
            }
            TextInputMsg::SelectHome => { self.cursor_home(true); true }
            TextInputMsg::SelectEnd => { self.cursor_end(true); true }
            TextInputMsg::SelectWordLeft => { self.cursor_word_left(true); true }
            TextInputMsg::SelectWordRight => { self.cursor_word_right(true); true }
            TextInputMsg::SelectAll => { self.select_all(); true }
            TextInputMsg::SelectLine => { self.select_line(); true }
            TextInputMsg::SelectWord => { self.select_word(); true }
            
            // Multi-cursor
            TextInputMsg::AddCursorAbove => { self.add_cursor_above(); true }
            TextInputMsg::AddCursorBelow => { self.add_cursor_below(); true }
            TextInputMsg::CollapseCursors => { self.collapse_to_single_cursor(); true }
            TextInputMsg::AddCursorAtNextOccurrence => { self.select_next_occurrence(); true }
            TextInputMsg::AddCursorsAtAllOccurrences => { self.select_all_occurrences(); true }
            
            // Clipboard (caller handles actual clipboard access)
            TextInputMsg::Cut => true,  // Caller should call cut() and set clipboard
            TextInputMsg::Copy => true, // Caller should call copy() and set clipboard
            TextInputMsg::Paste(text) => self.insert_text(&text),
            
            // Undo/Redo
            TextInputMsg::Undo => self.undo(),
            TextInputMsg::Redo => self.redo(),
            
            // Newline
            TextInputMsg::InsertNewline => {
                if self.config.multiline {
                    self.insert_char('\n')
                } else {
                    false  // Let caller handle (confirm action)
                }
            }
        }
    }
}
```

### 7.4 Routing Strategy

```rust
// src/runtime/input.rs (updated concept)

/// Convert keyboard event to TextInputMsg
fn key_to_text_input_msg(key: Key, ctrl: bool, shift: bool, alt: bool) -> Option<TextInputMsg> {
    use TextInputMsg::*;
    
    match (key, ctrl, shift, alt) {
        // Basic movement
        (Key::Left, false, false, false) => Some(CursorLeft),
        (Key::Right, false, false, false) => Some(CursorRight),
        (Key::Up, false, false, false) => Some(CursorUp),
        (Key::Down, false, false, false) => Some(CursorDown),
        (Key::Home, false, false, false) => Some(CursorHome),
        (Key::End, false, false, false) => Some(CursorEnd),
        
        // Word movement (Option on Mac, Ctrl on Windows/Linux)
        (Key::Left, false, false, true) => Some(CursorWordLeft),   // Option+Left
        (Key::Right, false, false, true) => Some(CursorWordRight), // Option+Right
        
        // Selection (Shift held)
        (Key::Left, false, true, false) => Some(SelectLeft),
        (Key::Right, false, true, false) => Some(SelectRight),
        (Key::Up, false, true, false) => Some(SelectUp),
        (Key::Down, false, true, false) => Some(SelectDown),
        (Key::Home, false, true, false) => Some(SelectHome),
        (Key::End, false, true, false) => Some(SelectEnd),
        
        // Word selection
        (Key::Left, false, true, true) => Some(SelectWordLeft),
        (Key::Right, false, true, true) => Some(SelectWordRight),
        
        // Deletion
        (Key::Backspace, false, false, false) => Some(DeleteBackward),
        (Key::Delete, false, false, false) => Some(DeleteForward),
        (Key::Backspace, false, false, true) => Some(DeleteWordBackward), // Option+Backspace
        (Key::Delete, false, false, true) => Some(DeleteWordForward),     // Option+Delete
        
        // Select all
        (Key::A, true, false, false) => Some(SelectAll), // Cmd+A / Ctrl+A
        
        // Undo/Redo
        (Key::Z, true, false, false) => Some(Undo),  // Cmd+Z
        (Key::Z, true, true, false) => Some(Redo),   // Cmd+Shift+Z
        
        // Newline
        (Key::Enter, false, false, false) => Some(InsertNewline),
        
        // Character input handled separately
        _ => None,
    }
}
```

---

## Chapter 8: Rendering Pipeline Unification

### 8.1 Current State

Each input area has its own rendering code:
- Main editor: `render_text_area()` in `src/view/mod.rs`
- Command palette: inline in `render_command_palette()`
- CSV cell: `render_csv_cell_editor()`

### 8.2 Unified Text Rendering Component

```rust
// src/view/text_input_renderer.rs (NEW FILE)

use crate::model::text_input::{TextInputField, InputSelection};
use crate::model::text_buffer::TextBuffer;
use crate::view::frame::Frame;
use crate::view::TextPainter;

/// Configuration for rendering a text input
pub struct TextInputRenderConfig {
    /// X position of text area
    pub x: usize,
    /// Y position of text area
    pub y: usize,
    /// Width of text area in pixels
    pub width: usize,
    /// Height of text area in pixels
    pub height: usize,
    /// Character width (monospace)
    pub char_width: f32,
    /// Line height in pixels
    pub line_height: usize,
    /// Text color
    pub text_color: u32,
    /// Cursor color
    pub cursor_color: u32,
    /// Selection background color
    pub selection_color: u32,
    /// Secondary cursor color (for multi-cursor)
    pub secondary_cursor_color: u32,
    /// Whether cursor should be visible (for blinking)
    pub cursor_visible: bool,
    /// Whether to show line numbers (for multi-line)
    pub show_line_numbers: bool,
    /// Horizontal scroll offset in pixels
    pub scroll_x: usize,
    /// Vertical scroll offset in lines (for multi-line)
    pub scroll_y: usize,
}

/// Render a TextInputField to a Frame
pub fn render_text_input<B: TextBuffer>(
    frame: &mut Frame,
    painter: &mut TextPainter,
    field: &TextInputField<B>,
    config: &TextInputRenderConfig,
) {
    let content_x = config.x;
    let content_y = config.y;
    
    // For single-line inputs
    if field.buffer.line_count() == 1 {
        render_single_line(frame, painter, field, config);
    } else {
        render_multi_line(frame, painter, field, config);
    }
}

fn render_single_line<B: TextBuffer>(
    frame: &mut Frame,
    painter: &mut TextPainter,
    field: &TextInputField<B>,
    config: &TextInputRenderConfig,
) {
    let text = field.buffer.to_string();
    let text_x = config.x;
    let text_y = config.y;
    
    // Render selection background first
    for selection in &field.selections {
        if !selection.is_empty() {
            let start_col = field.buffer.offset_to_position(selection.start()).1;
            let end_col = field.buffer.offset_to_position(selection.end()).1;
            
            let sel_x = text_x + (start_col as f32 * config.char_width).round() as usize;
            let sel_width = ((end_col - start_col) as f32 * config.char_width).round() as usize;
            
            frame.fill_rect_px(
                sel_x.saturating_sub(config.scroll_x),
                text_y,
                sel_width.min(config.width),
                config.line_height,
                config.selection_color,
            );
        }
    }
    
    // Render text
    painter.draw(frame, text_x, text_y, &text, config.text_color);
    
    // Render cursor(s)
    if config.cursor_visible {
        for (idx, cursor) in field.cursors.iter().enumerate() {
            let (_, col) = field.buffer.offset_to_position(cursor.offset);
            let cursor_x = text_x + (col as f32 * config.char_width).round() as usize;
            
            let color = if idx == field.active_cursor {
                config.cursor_color
            } else {
                config.secondary_cursor_color
            };
            
            // 2px wide cursor bar
            frame.fill_rect_px(
                cursor_x.saturating_sub(config.scroll_x),
                text_y + 1,
                2,
                config.line_height.saturating_sub(2),
                color,
            );
        }
    }
}

fn render_multi_line<B: TextBuffer>(
    frame: &mut Frame,
    painter: &mut TextPainter,
    field: &TextInputField<B>,
    config: &TextInputRenderConfig,
) {
    let visible_lines = config.height / config.line_height;
    let start_line = config.scroll_y;
    let end_line = (start_line + visible_lines + 1).min(field.buffer.line_count());
    
    for doc_line in start_line..end_line {
        let screen_line = doc_line - start_line;
        let line_y = config.y + screen_line * config.line_height;
        
        if let Some(line_text) = field.buffer.line(doc_line) {
            // Render selection on this line
            for selection in &field.selections {
                if !selection.is_empty() {
                    render_line_selection(
                        frame,
                        &line_text,
                        doc_line,
                        selection,
                        config,
                        line_y,
                        &field.buffer,
                    );
                }
            }
            
            // Render text
            painter.draw(frame, config.x, line_y, &line_text, config.text_color);
        }
    }
    
    // Render cursors
    if config.cursor_visible {
        for (idx, cursor) in field.cursors.iter().enumerate() {
            let (line, col) = field.buffer.offset_to_position(cursor.offset);
            
            if line >= start_line && line < end_line {
                let screen_line = line - start_line;
                let cursor_y = config.y + screen_line * config.line_height;
                let cursor_x = config.x + (col as f32 * config.char_width).round() as usize;
                
                let color = if idx == field.active_cursor {
                    config.cursor_color
                } else {
                    config.secondary_cursor_color
                };
                
                frame.fill_rect_px(
                    cursor_x.saturating_sub(config.scroll_x),
                    cursor_y + 1,
                    2,
                    config.line_height.saturating_sub(2),
                    color,
                );
            }
        }
    }
}

fn render_line_selection<B: TextBuffer>(
    frame: &mut Frame,
    line_text: &str,
    doc_line: usize,
    selection: &InputSelection,
    config: &TextInputRenderConfig,
    line_y: usize,
    buffer: &B,
) {
    let sel_start_pos = buffer.offset_to_position(selection.start());
    let sel_end_pos = buffer.offset_to_position(selection.end());
    
    // Check if this line is in selection range
    if doc_line < sel_start_pos.0 || doc_line > sel_end_pos.0 {
        return;
    }
    
    let line_len = line_text.chars().count();
    
    let start_col = if doc_line == sel_start_pos.0 { sel_start_pos.1 } else { 0 };
    let end_col = if doc_line == sel_end_pos.0 { sel_end_pos.1 } else { line_len };
    
    if start_col >= end_col {
        return;
    }
    
    let sel_x = config.x + (start_col as f32 * config.char_width).round() as usize;
    let sel_width = ((end_col - start_col) as f32 * config.char_width).round() as usize;
    
    frame.fill_rect_px(
        sel_x.saturating_sub(config.scroll_x),
        line_y,
        sel_width,
        config.line_height,
        config.selection_color,
    );
}
```

---

## Chapter 9: Context-Specific Restrictions

### 9.1 Restriction Model

Different input contexts need different capabilities. The `InputFieldConfig` handles this, but we also need clear documentation of what each context supports.

### 9.2 Context Definitions

```rust
// src/model/input_context.rs (NEW FILE)

/// Predefined input contexts with their configurations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputContext {
    /// Main document editor - full capabilities
    Editor,
    /// Command palette input - single line, no multi-cursor
    CommandPalette,
    /// Go-to-line dialog - single line, numeric only
    GotoLine,
    /// Find/replace query - single line, with history
    FindReplace,
    /// CSV cell editor - single line, no multi-cursor
    CsvCell,
}

impl InputContext {
    /// Get the configuration for this context
    pub fn config(&self) -> InputFieldConfig {
        match self {
            InputContext::Editor => InputFieldConfig {
                multiline: true,
                multi_cursor: true,
                allow_selection: true,
                enable_undo: true,
                max_length: None,
                char_filter: None,
            },
            InputContext::CommandPalette => InputFieldConfig {
                multiline: false,
                multi_cursor: false,
                allow_selection: true,
                enable_undo: false,
                max_length: Some(1000),  // Reasonable limit
                char_filter: None,
            },
            InputContext::GotoLine => InputFieldConfig {
                multiline: false,
                multi_cursor: false,
                allow_selection: true,
                enable_undo: false,
                max_length: Some(10),  // Line numbers
                char_filter: Some(|c| c.is_ascii_digit()),
            },
            InputContext::FindReplace => InputFieldConfig {
                multiline: false,
                multi_cursor: false,
                allow_selection: true,
                enable_undo: false,
                max_length: Some(10000),
                char_filter: None,
            },
            InputContext::CsvCell => InputFieldConfig {
                multiline: false,
                multi_cursor: false,
                allow_selection: true,
                enable_undo: false,
                max_length: None,
                char_filter: None,
            },
        }
    }
    
    /// What happens when Enter is pressed
    pub fn enter_behavior(&self) -> EnterBehavior {
        match self {
            InputContext::Editor => EnterBehavior::InsertNewline,
            InputContext::CommandPalette => EnterBehavior::Confirm,
            InputContext::GotoLine => EnterBehavior::Confirm,
            InputContext::FindReplace => EnterBehavior::FindNext,
            InputContext::CsvCell => EnterBehavior::ConfirmAndMoveDown,
        }
    }
    
    /// What happens when Escape is pressed
    pub fn escape_behavior(&self) -> EscapeBehavior {
        match self {
            InputContext::Editor => EscapeBehavior::CollapseCursors,
            _ => EscapeBehavior::Cancel,
        }
    }
    
    /// What happens when Tab is pressed
    pub fn tab_behavior(&self) -> TabBehavior {
        match self {
            InputContext::Editor => TabBehavior::InsertTab,
            InputContext::CommandPalette => TabBehavior::Ignored,
            InputContext::GotoLine => TabBehavior::Ignored,
            InputContext::FindReplace => TabBehavior::SwitchField,
            InputContext::CsvCell => TabBehavior::ConfirmAndMoveRight,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum EnterBehavior {
    InsertNewline,
    Confirm,
    FindNext,
    ConfirmAndMoveDown,
}

#[derive(Debug, Clone, Copy)]
pub enum EscapeBehavior {
    CollapseCursors,
    Cancel,
}

#[derive(Debug, Clone, Copy)]
pub enum TabBehavior {
    InsertTab,
    Ignored,
    SwitchField,
    ConfirmAndMoveRight,
}
```

### 9.3 Capability Matrix

| Feature | Editor | CommandPalette | GotoLine | FindReplace | CsvCell |
|---------|--------|----------------|----------|-------------|---------|
| Multi-line | ✅ | ❌ | ❌ | ❌ | ❌ |
| Multi-cursor | ✅ | ❌ | ❌ | ❌ | ❌ |
| Selection | ✅ | ✅ | ✅ | ✅ | ✅ |
| Undo/Redo | ✅ | ❌ | ❌ | ❌ | ❌ |
| Word ops | ✅ | ✅ | ✅ | ✅ | ✅ |
| Char filter | ❌ | ❌ | digits | ❌ | ❌ |
| Enter | newline | confirm | confirm | find next | confirm+↓ |
| Tab | insert | ignore | ignore | switch | confirm+→ |
| Escape | collapse | cancel | cancel | cancel | cancel |

---

## Chapter 10: Migration Strategy

### 10.1 Phased Approach

The refactoring should be done in phases to minimize risk and allow incremental testing.

### Phase 1: Create Core Abstractions (Week 1)

**Goal:** Implement `TextBuffer`, `StringBuffer`, and `TextInputField` without changing existing code.

**Files to create:**
- `src/model/text_buffer.rs` - TextBuffer trait, StringBuffer, RopeBuffer
- `src/model/text_input.rs` - TextInputField, InputCursor, InputSelection
- `src/model/input_context.rs` - InputContext enum and configs

**Testing:**
- Unit tests for all buffer operations
- Unit tests for cursor movement
- Unit tests for text editing operations

### Phase 2: Migrate Command Palette (Week 2)

**Goal:** Replace `CommandPaletteState.input: String` with `TextInputField<StringBuffer>`.

**Files to modify:**
- `src/model/ui.rs` - Update CommandPaletteState
- `src/update/ui.rs` - Route ModalMsg to TextInputField
- `src/runtime/input.rs` - Update handle_modal_key
- `src/view/mod.rs` - Use unified renderer

**Breaking changes:** None (internal refactor)

### Phase 3: Migrate Go-to-Line and Find/Replace (Week 2)

**Goal:** Apply same pattern to other modal inputs.

**Files to modify:**
- `src/model/ui.rs` - Update GotoLineState, FindReplaceState
- `src/update/ui.rs` - Route messages
- `src/view/mod.rs` - Use unified renderer

### Phase 4: Migrate CSV Cell Editor (Week 3)

**Goal:** Replace `CellEditState` with `TextInputField<StringBuffer>`.

**Files to modify:**
- `src/csv/model.rs` - Update CellEditState
- `src/update/csv.rs` - Route CsvMsg::Edit* to TextInputField
- `src/view/mod.rs` - Use unified renderer for cells

### Phase 5: Create RopeBuffer Wrapper (Week 3-4)

**Goal:** Create RopeBuffer that wraps Document's Rope for the main editor.

**Files to create/modify:**
- `src/model/text_buffer.rs` - Add RopeBuffer implementation
- `src/model/editor.rs` - Consider using TextInputField for cursor/selection logic

### Phase 6: Unify Main Editor (Week 4-5)

**Goal:** Refactor main editor to use shared cursor/selection logic.

This is the most complex phase. Options:
1. **Full migration:** Replace EditorState internals with TextInputField<RopeBuffer>
2. **Partial migration:** Keep EditorState but delegate to TextInputField for common operations
3. **Shared logic extraction:** Extract common cursor/selection functions, use from both

**Recommended approach:** Option 2 - keeps existing architecture but reuses logic.

### Phase 7: Consolidate Messages (Week 5)

**Goal:** Introduce TextInputMsg and unify keyboard handling.

**Files to modify:**
- `src/messages.rs` - Add TextInputMsg
- `src/runtime/input.rs` - Unify key-to-message conversion
- `src/keymap/command.rs` - Map commands to TextInputMsg

### Phase 8: Cleanup and Documentation (Week 6)

**Goal:** Remove deprecated code, update documentation.

- Remove old ModalMsg variants
- Remove duplicate code in CsvMsg
- Update AGENTS.md with new patterns
- Add integration tests

---

## Chapter 11: File-by-File Implementation Guide

### 11.1 New Files to Create

```
src/model/text_buffer.rs       # TextBuffer trait, StringBuffer, RopeBuffer
src/model/text_input.rs        # TextInputField, InputCursor, InputSelection
src/model/input_context.rs     # InputContext enum and configs
src/view/text_input_renderer.rs # Unified rendering code
src/messages/text_input.rs     # TextInputMsg enum (optional: in messages.rs)
```

### 11.2 Files to Modify

#### `src/model/mod.rs`
```rust
// Add new modules
pub mod text_buffer;
pub mod text_input;
pub mod input_context;
```

#### `src/model/ui.rs`

```rust
// Before
pub struct CommandPaletteState {
    pub input: String,
    pub selected_index: usize,
}

// After
use crate::model::text_input::TextInputField;
use crate::model::text_buffer::StringBuffer;

pub struct CommandPaletteState {
    pub input: TextInputField<StringBuffer>,
    pub selected_index: usize,
}

impl CommandPaletteState {
    pub fn new() -> Self {
        Self {
            input: TextInputField::new(
                StringBuffer::new(),
                InputFieldConfig::single_line(),
            ),
            selected_index: 0,
        }
    }
    
    /// Get the input text (for filtering commands)
    pub fn query(&self) -> String {
        self.input.text()
    }
}
```

#### `src/update/ui.rs`

```rust
// Before
ModalMsg::InsertChar(ch) => {
    if let Some(ModalState::CommandPalette(state)) = &mut model.ui.active_modal {
        state.input.push(ch);
        state.selected_index = 0;
    }
}

// After
ModalMsg::TextInput(msg) => {
    if let Some(ModalState::CommandPalette(state)) = &mut model.ui.active_modal {
        state.input.handle_message(msg);
        state.selected_index = 0;  // Reset on any input change
    }
}

// Or, route through unified handler
fn handle_modal_text_input(model: &mut AppModel, msg: TextInputMsg) -> Option<Cmd> {
    match &mut model.ui.active_modal {
        Some(ModalState::CommandPalette(state)) => {
            if state.input.handle_message(msg) {
                state.selected_index = 0;
                Some(Cmd::Redraw)
            } else {
                None
            }
        }
        Some(ModalState::GotoLine(state)) => {
            state.input.handle_message(msg);
            Some(Cmd::Redraw)
        }
        _ => None,
    }
}
```

#### `src/csv/model.rs`

```rust
// Before
pub struct CellEditState {
    pub position: CellPosition,
    pub buffer: String,
    pub cursor: usize,
    pub original: String,
}

// After
use crate::model::text_input::TextInputField;
use crate::model::text_buffer::StringBuffer;

pub struct CellEditState {
    pub position: CellPosition,
    pub input: TextInputField<StringBuffer>,
    pub original: String,
}

impl CellEditState {
    pub fn new(position: CellPosition, value: String) -> Self {
        let original = value.clone();
        let mut input = TextInputField::new(
            StringBuffer::from_str(&value),
            InputFieldConfig::csv_cell(),
        );
        // Move cursor to end
        input.cursor_end(false);
        
        Self { position, input, original }
    }
    
    pub fn with_char(position: CellPosition, original: String, ch: char) -> Self {
        let mut input = TextInputField::new(
            StringBuffer::new(),
            InputFieldConfig::csv_cell(),
        );
        input.insert_char(ch);
        
        Self { position, input, original }
    }
    
    pub fn value(&self) -> String {
        self.input.text()
    }
    
    pub fn cursor_char_position(&self) -> usize {
        self.input.cursor_char_column()
    }
}
```

### 11.3 Message Type Updates

#### `src/messages.rs`

```rust
// Add new message type
pub enum TextInputMsg {
    InsertChar(char),
    InsertText(String),
    DeleteBackward,
    DeleteForward,
    DeleteWordBackward,
    DeleteWordForward,
    CursorLeft,
    CursorRight,
    CursorHome,
    CursorEnd,
    CursorWordLeft,
    CursorWordRight,
    SelectAll,
    // ... etc
}

// Update ModalMsg to use it
pub enum ModalMsg {
    TextInput(TextInputMsg),  // Replaces individual variants
    Confirm,
    Cancel,
    SelectPrevious,
    SelectNext,
    // ... modal-specific actions
}

// Similarly for CsvMsg
pub enum CsvMsg {
    // Navigation
    Move(Direction),
    // ...
    
    // Cell editing - now uses TextInputMsg
    EditInput(TextInputMsg),
    StartEditing,
    ConfirmEdit,
    CancelEdit,
}
```

---

## Chapter 12: Testing Strategy

### 12.1 Unit Tests for TextBuffer

```rust
// tests/text_buffer_tests.rs

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
```

### 12.2 Unit Tests for TextInputField

```rust
// tests/text_input_tests.rs

#[test]
fn test_cursor_movement() {
    let mut field = TextInputField::new(
        StringBuffer::from_str("hello"),
        InputFieldConfig::single_line(),
    );
    field.cursors[0].offset = 2;  // Start at 'l'
    
    field.cursor_left(false);
    assert_eq!(field.cursor().offset, 1);
    
    field.cursor_right(false);
    assert_eq!(field.cursor().offset, 2);
}

#[test]
fn test_word_movement() {
    let mut field = TextInputField::new(
        StringBuffer::from_str("hello world"),
        InputFieldConfig::single_line(),
    );
    field.cursors[0].offset = 0;
    
    field.cursor_word_right(false);
    assert_eq!(field.cursor().offset, 5);  // After "hello"
    
    field.cursor_word_right(false);
    assert_eq!(field.cursor().offset, 6);  // After space
}

#[test]
fn test_selection() {
    let mut field = TextInputField::new(
        StringBuffer::from_str("hello world"),
        InputFieldConfig::single_line(),
    );
    
    field.cursor_word_right(true);  // Select "hello"
    
    assert_eq!(field.selection().anchor, 0);
    assert_eq!(field.selection().head, 5);
    assert_eq!(field.selected_text(), "hello");
}

#[test]
fn test_insert_replaces_selection() {
    let mut field = TextInputField::new(
        StringBuffer::from_str("hello world"),
        InputFieldConfig::single_line(),
    );
    
    // Select "hello"
    field.selections[0] = InputSelection { anchor: 0, head: 5 };
    field.cursors[0].offset = 5;
    
    field.insert_char('X');
    
    assert_eq!(field.text(), "X world");
    assert_eq!(field.cursor().offset, 1);
}

#[test]
fn test_char_filter() {
    let mut field = TextInputField::new(
        StringBuffer::new(),
        InputFieldConfig::numeric(),
    );
    
    assert!(field.insert_char('5'));
    assert!(!field.insert_char('a'));
    assert_eq!(field.text(), "5");
}

#[test]
fn test_multi_cursor_disabled() {
    let mut field = TextInputField::new(
        StringBuffer::from_str("hello\nworld"),
        InputFieldConfig::single_line(),  // No multi-cursor
    );
    
    field.add_cursor_below();
    
    assert_eq!(field.cursors.len(), 1);  // Should not add
}
```

### 12.3 Integration Tests

```rust
// tests/integration/modal_input_tests.rs

#[test]
fn test_command_palette_full_workflow() {
    let mut model = create_test_model();
    
    // Open palette
    update(&mut model, Msg::Ui(UiMsg::OpenCommandPalette));
    
    // Type a query
    update(&mut model, Msg::Ui(UiMsg::Modal(ModalMsg::TextInput(
        TextInputMsg::InsertText("format".to_string())
    ))));
    
    // Check filtered results
    let palette = model.ui.active_modal.as_ref().unwrap();
    if let ModalState::CommandPalette(state) = palette {
        assert!(state.query().contains("format"));
    }
    
    // Cursor movement
    update(&mut model, Msg::Ui(UiMsg::Modal(ModalMsg::TextInput(
        TextInputMsg::CursorHome
    ))));
    
    // Word delete
    update(&mut model, Msg::Ui(UiMsg::Modal(ModalMsg::TextInput(
        TextInputMsg::DeleteWordForward
    ))));
    
    // Confirm palette is empty
    let palette = model.ui.active_modal.as_ref().unwrap();
    if let ModalState::CommandPalette(state) = palette {
        assert!(state.query().is_empty());
    }
}
```

---

## Appendix A: Current Implementation Inventory

### A.1 Text Editing Code Locations

| Component | File | Lines | Operations Implemented |
|-----------|------|-------|----------------------|
| Main Editor Cursor | src/model/editor.rs | 50-200 | Position, Cursor, move_cursor_* |
| Main Editor Selection | src/model/editor.rs | 200-350 | Selection, extend_to, get_text |
| Document Insert/Delete | src/update/document.rs | 100-400 | InsertChar, DeleteBackward, etc |
| Command Palette Input | src/model/ui.rs | 69-76 | String field only |
| Modal Handlers | src/update/ui.rs | 150-220 | InsertChar, DeleteBackward, DeleteWordBackward |
| CSV Cell Edit | src/csv/model.rs | 56-150 | insert_char, delete_*, cursor_* |
| Word Boundaries | src/util/text.rs | 1-50 | char_type, word_start_before |

### A.2 Rendering Code Locations

| Component | File | Lines |
|-----------|------|-------|
| Editor text+cursor | src/view/mod.rs | 400-700 |
| Editor selection | src/view/mod.rs | 700-850 |
| Command palette | src/view/mod.rs | 1383-1478 |
| CSV cell editor | src/view/mod.rs | 1135-1208 |

### A.3 Message Handlers

| Message Type | Handler Location |
|--------------|-----------------|
| EditorMsg | src/update/editor.rs |
| DocumentMsg | src/update/document.rs |
| ModalMsg | src/update/ui.rs |
| CsvMsg::Edit* | src/update/csv.rs |

---

## Appendix B: Message Type Mappings

### B.1 Current to Unified Mapping

| Old Message | New TextInputMsg |
|-------------|------------------|
| DocumentMsg::InsertChar(c) | InsertChar(c) |
| DocumentMsg::DeleteBackward | DeleteBackward |
| DocumentMsg::DeleteForward | DeleteForward |
| DocumentMsg::DeleteWordBackward | DeleteWordBackward |
| DocumentMsg::DeleteWordForward | DeleteWordForward |
| EditorMsg::MoveCursor(Left) | CursorLeft |
| EditorMsg::MoveCursor(Right) | CursorRight |
| EditorMsg::MoveCursorWord(Left) | CursorWordLeft |
| EditorMsg::MoveCursorWord(Right) | CursorWordRight |
| EditorMsg::MoveCursorWithSelection(Left) | SelectLeft |
| EditorMsg::SelectAll | SelectAll |
| ModalMsg::InsertChar(c) | InsertChar(c) |
| ModalMsg::DeleteBackward | DeleteBackward |
| ModalMsg::DeleteWordBackward | DeleteWordBackward |
| CsvMsg::EditInsertChar(c) | InsertChar(c) |
| CsvMsg::EditDeleteBackward | DeleteBackward |
| CsvMsg::EditCursorLeft | CursorLeft |
| CsvMsg::EditCursorRight | CursorRight |

---

## Appendix C: Keyboard Shortcut Matrix

### C.1 Expected Shortcuts After Unification

| Action | macOS | Windows/Linux | TextInputMsg |
|--------|-------|---------------|--------------|
| Cursor left | ← | ← | CursorLeft |
| Cursor right | → | → | CursorRight |
| Cursor up | ↑ | ↑ | CursorUp |
| Cursor down | ↓ | ↓ | CursorDown |
| Word left | ⌥← | Ctrl+← | CursorWordLeft |
| Word right | ⌥→ | Ctrl+→ | CursorWordRight |
| Line start | ⌘← or Home | Home | CursorHome |
| Line end | ⌘→ or End | End | CursorEnd |
| Doc start | ⌘↑ | Ctrl+Home | CursorDocumentStart |
| Doc end | ⌘↓ | Ctrl+End | CursorDocumentEnd |
| Select left | ⇧← | Shift+← | SelectLeft |
| Select right | ⇧→ | Shift+→ | SelectRight |
| Select word left | ⌥⇧← | Ctrl+Shift+← | SelectWordLeft |
| Select word right | ⌥⇧→ | Ctrl+Shift+→ | SelectWordRight |
| Select all | ⌘A | Ctrl+A | SelectAll |
| Delete backward | ⌫ | Backspace | DeleteBackward |
| Delete forward | ⌦ or Fn+⌫ | Delete | DeleteForward |
| Delete word back | ⌥⌫ | Ctrl+Backspace | DeleteWordBackward |
| Delete word fwd | ⌥⌦ | Ctrl+Delete | DeleteWordForward |
| Undo | ⌘Z | Ctrl+Z | Undo |
| Redo | ⌘⇧Z | Ctrl+Y / Ctrl+Shift+Z | Redo |
| Cut | ⌘X | Ctrl+X | Cut |
| Copy | ⌘C | Ctrl+C | Copy |
| Paste | ⌘V | Ctrl+V | Paste(text) |

---

*End of document. This plan provides a complete roadmap for unifying text editing across all input areas in the Token editor.*
