# Unified Text Editing System - Implementation Plan

A comprehensive refactoring plan to unify text editing behavior across the main editor, command palette, CSV cell editor, find/replace, and go-to-line inputs.

> **Goal:** Create a single, reusable text editing abstraction with consistent behavior (cursor movement, word navigation, selection, undo/redo) across all editing contexts, with configurable constraints per context.

---

## Table of Contents

1. [Executive Summary](#1-executive-summary)
2. [Current State Analysis](#2-current-state-analysis)
3. [Target Architecture Overview](#3-target-architecture-overview)
4. [Phase 1: Core Data Model](#4-phase-1-core-data-model)
5. [Phase 2: Message and Update System](#5-phase-2-message-and-update-system)
6. [Phase 3: Rendering Consolidation](#6-phase-3-rendering-consolidation)
7. [Phase 4: Migration Strategy](#7-phase-4-migration-strategy)
8. [Implementation Order](#8-implementation-order)
9. [File Change Summary](#9-file-change-summary)
10. [Testing Strategy](#10-testing-strategy)

---

## 1. Executive Summary

### Problem

The codebase has **5 separate text editing implementations** with duplicated logic and inconsistent behavior:

| Context | Buffer | Cursor | Selection | Undo/Redo | Word Ops |
|---------|--------|--------|-----------|-----------|----------|
| Main Editor | Rope | Multi-cursor with `desired_column` | Full | Full history | Yes |
| Command Palette | String | End-only implicit | No | No | Partial |
| Go to Line | String | End-only implicit | No | No | No |
| Find/Replace | String | End-only implicit | No | No | No |
| CSV Cell | String | Byte offset | No | Cancel only | No |

### Solution

Create a unified `src/editable/` module with:
- **TextBuffer trait** - Abstract over Rope and String
- **EditableState** - Unified cursor, selection, and history management
- **EditConstraints** - Context-specific restrictions (single-line, single-cursor, char filtering)
- **TextEditMsg** - Unified editing message type
- **TextFieldRenderer** - Unified text field rendering

### User Requirements (Confirmed)

- Full undo/redo in ALL contexts
- Selection support (shift+arrow) in ALL contexts
- Unified rendering AND editing logic
- Include Find/Replace and Go to Line inputs
- Single-cursor constraint in modal/cell contexts

---

## 2. Current State Analysis

### 2.1 Main Editor Implementation

**Files:**
- `src/model/editor.rs` (1291 lines) - Cursor, Selection, EditorState
- `src/model/document.rs` (299 lines) - Document, EditOperation, undo stacks
- `src/update/editor.rs` (1263 lines) - Cursor movement, selection operations
- `src/update/document.rs` (2007 lines) - Text editing, clipboard, undo/redo

**Key Structures:**
```rust
// src/model/editor.rs
pub struct Cursor {
    pub line: usize,
    pub column: usize,
    pub desired_column: Option<usize>,
}

pub struct Selection {
    pub anchor: Position,
    pub head: Position,
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
    Insert { position, text, cursor_before, cursor_after },
    Delete { position, text, cursor_before, cursor_after },
    Replace { position, deleted_text, inserted_text, cursor_before, cursor_after },
    Batch { operations, cursors_before, cursors_after },
}
```

### 2.2 Modal Input Implementation

**Files:**
- `src/model/ui.rs:56-135` - CommandPaletteState, GotoLineState, FindReplaceState
- `src/update/ui.rs:15-25` - `delete_word_backward()` (separate implementation)
- `src/update/ui.rs:110-349` - `update_modal()`

**Key Structures:**
```rust
pub struct CommandPaletteState {
    pub input: String,           // Simple String, cursor implicit at end
    pub selected_index: usize,
}

pub struct GotoLineState {
    pub input: String,
}

pub struct FindReplaceState {
    pub query: String,
    pub replacement: String,
    pub focused_field: FindReplaceField,
}
```

**Supported Operations:**
- InsertChar, DeleteBackward, DeleteWordBackward (custom implementation)
- No cursor positioning, no selection

### 2.3 CSV Cell Editor Implementation

**Files:**
- `src/csv/model.rs:54-154` - CellEditState
- `src/update/csv.rs` (548 lines) - Cell editing and navigation

**Key Structures:**
```rust
pub struct CellEditState {
    pub position: CellPosition,
    pub buffer: String,
    pub cursor: usize,        // Byte offset, not character position
    pub original: String,     // For cancel
}
```

**Supported Operations:**
- InsertChar, DeleteBackward, DeleteForward
- CursorLeft, CursorRight, CursorHome, CursorEnd
- No selection, no undo (just cancel to original)

### 2.4 Rendering Implementation

**Files:**
- `src/view/mod.rs` (1835 lines) - Main renderer
- `src/view/frame.rs` (502 lines) - Frame/TextPainter primitives

**Separate Rendering Paths:**
1. `render_text_area()` (lines 615-905) - Full editor with syntax highlighting
2. `render_modals()` (lines 1410-1425) - Modal input fields
3. `render_csv_cell_editor()` (lines 1137-1208) - CSV cell overlay

---

## 3. Target Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────┐
│                        src/editable/                                 │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌────────────┐ │
│  │ TextBuffer  │  │EditableState│  │  EditHistory │  │EditConstraints│
│  │   (trait)   │  │  (cursors,  │  │  (undo/redo) │  │(single-line,│
│  │             │  │  selections)│  │              │  │ char filter)│
│  └─────────────┘  └─────────────┘  └─────────────┘  └────────────┘ │
└─────────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────────┐
│                      src/messages/text_edit.rs                       │
│  ┌─────────────────────────────────────────────────────────────────┐│
│  │ TextEditMsg: Move, MoveWithSelection, InsertChar, DeleteBackward││
│  │              Undo, Redo, Copy, Cut, Paste, SelectAll, ...       ││
│  └─────────────────────────────────────────────────────────────────┘│
│  ┌─────────────────────────────────────────────────────────────────┐│
│  │ EditContext: Editor(GroupId), CommandPalette, GotoLine,         ││
│  │              FindQuery, FindReplace, CsvCell(CellPosition)      ││
│  └─────────────────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────────┐
│                      src/update/text_edit.rs                         │
│  ┌─────────────────────────────────────────────────────────────────┐│
│  │ update_text_edit(model, context, msg) -> Option<Cmd>            ││
│  │   - Routes to appropriate buffer based on context               ││
│  │   - Applies constraints                                          ││
│  │   - Records undo history                                         ││
│  └─────────────────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────────┐
│                      src/view/text_field.rs                          │
│  ┌─────────────────────────────────────────────────────────────────┐│
│  │ TextFieldRenderer::render(frame, painter, content, bounds, opts)││
│  │   - Unified cursor, selection, text rendering                   ││
│  │   - Works for all contexts via TextFieldContent trait           ││
│  └─────────────────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────────────────┘
```

---

## 4. Phase 1: Core Data Model

### 4.1 New Module Structure

Create `src/editable/` with the following files:

```
src/editable/
├── mod.rs           # Module exports
├── buffer.rs        # TextBuffer trait + implementations
├── cursor.rs        # Position, Cursor structs
├── selection.rs     # Selection struct
├── history.rs       # EditOperation, EditHistory
├── constraints.rs   # EditConstraints, char filters
└── state.rs         # EditableState (combines all above)
```

### 4.2 TextBuffer Trait

```rust
// src/editable/buffer.rs

/// Read-only view into a text buffer for cursor navigation.
/// Abstracts over Rope (large files) and String (small inputs).
pub trait TextBuffer {
    fn line_count(&self) -> usize;
    fn line_length(&self, line: usize) -> usize;
    fn len_chars(&self) -> usize;
    fn is_empty(&self) -> bool { self.len_chars() == 0 }
    fn char_at(&self, line: usize, column: usize) -> Option<char>;
    fn get_line(&self, line: usize) -> Option<String>;
    fn cursor_to_offset(&self, line: usize, column: usize) -> usize;
    fn offset_to_cursor(&self, offset: usize) -> (usize, usize);
    fn first_non_whitespace_column(&self, line: usize) -> usize;
    fn last_non_whitespace_column(&self, line: usize) -> usize;
}

/// Mutable buffer operations.
pub trait TextBufferMut: TextBuffer {
    fn insert(&mut self, offset: usize, text: &str);
    fn remove(&mut self, range: std::ops::Range<usize>);
    fn insert_char(&mut self, offset: usize, ch: char);
    fn slice(&self, range: std::ops::Range<usize>) -> String;
}
```

**Implementations:**
- `impl TextBuffer for ropey::Rope` - For main editor
- `impl TextBuffer for String` - For modals and CSV cells

### 4.3 Cursor and Selection

```rust
// src/editable/cursor.rs

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Position {
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Cursor {
    pub line: usize,
    pub column: usize,
    pub desired_column: Option<usize>,  // For vertical movement
}

// src/editable/selection.rs

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Selection {
    pub anchor: Position,  // Fixed point where selection started
    pub head: Position,    // Moving point (cursor position)
}

impl Selection {
    pub fn is_empty(&self) -> bool { self.anchor == self.head }
    pub fn start(&self) -> Position { /* min of anchor/head */ }
    pub fn end(&self) -> Position { /* max of anchor/head */ }
    pub fn extend_to(&mut self, pos: Position) { self.head = pos; }
    pub fn collapse(&mut self) { self.anchor = self.head; }
}
```

### 4.4 Edit History

```rust
// src/editable/history.rs

#[derive(Debug, Clone)]
pub enum EditOperation {
    Insert { offset: usize, text: String, cursor_before: Cursor, cursor_after: Cursor },
    Delete { offset: usize, text: String, cursor_before: Cursor, cursor_after: Cursor },
    Replace { offset: usize, deleted_text: String, inserted_text: String, cursor_before: Cursor, cursor_after: Cursor },
    Batch { operations: Vec<EditOperation>, cursors_before: Vec<Cursor>, cursors_after: Vec<Cursor> },
}

#[derive(Debug, Clone)]
pub struct EditHistory {
    undo_stack: Vec<EditOperation>,
    redo_stack: Vec<EditOperation>,
    max_entries: usize,  // 0 = unlimited
    is_modified: bool,
}

impl EditHistory {
    pub fn new() -> Self { /* unlimited */ }
    pub fn with_limit(max: usize) -> Self { /* for modals */ }
    pub fn push(&mut self, op: EditOperation);
    pub fn pop_undo(&mut self) -> Option<EditOperation>;
    pub fn pop_redo(&mut self) -> Option<EditOperation>;
    pub fn can_undo(&self) -> bool;
    pub fn can_redo(&self) -> bool;
}
```

### 4.5 Edit Constraints

```rust
// src/editable/constraints.rs

pub type CharFilter = fn(char) -> bool;

pub mod filters {
    pub fn digits_only(ch: char) -> bool { ch.is_ascii_digit() }
    pub fn line_col_format(ch: char) -> bool { ch.is_ascii_digit() || ch == ':' }
    pub fn single_line(ch: char) -> bool { !ch.is_control() && ch != '\n' }
}

#[derive(Debug, Clone)]
pub struct EditConstraints {
    pub multi_line: bool,
    pub multi_cursor: bool,
    pub editable: bool,
    pub selectable: bool,
    pub char_filter: Option<CharFilter>,
    pub max_length: usize,        // 0 = unlimited
    pub history_limit: usize,     // 0 = unlimited
}

impl EditConstraints {
    pub fn full_editor() -> Self { /* all features */ }
    pub fn single_line_input() -> Self { /* no newlines, single cursor, selection enabled */ }
    pub fn numeric_input() -> Self { /* digits only */ }
    pub fn csv_cell() -> Self { /* single line, no newlines */ }
}
```

### 4.6 EditableState (Main Abstraction)

```rust
// src/editable/state.rs

#[derive(Debug, Clone)]
pub struct EditableState {
    pub cursors: Vec<Cursor>,
    pub selections: Vec<Selection>,
    pub active_cursor_index: usize,
    pub history: EditHistory,
    pub constraints: EditConstraints,
    pub selection_history: Vec<Selection>,  // For expand/shrink
}

impl EditableState {
    // Constructors
    pub fn new() -> Self { /* full editor */ }
    pub fn single_line() -> Self { /* command palette, find/replace */ }
    pub fn numeric() -> Self { /* goto line */ }
    pub fn csv_cell() -> Self { /* CSV cell */ }

    // Cursor accessors
    pub fn primary_cursor(&self) -> &Cursor;
    pub fn primary_cursor_mut(&mut self) -> &mut Cursor;
    pub fn primary_selection(&self) -> &Selection;

    // Invariant maintenance
    pub fn collapse_selections(&mut self);
    pub fn collapse_to_primary(&mut self);
    pub fn deduplicate_cursors(&mut self);

    // Editing operations (work with any TextBufferMut)
    pub fn insert_char<B: TextBufferMut>(&mut self, buffer: &mut B, ch: char) -> bool;
    pub fn insert_text<B: TextBufferMut>(&mut self, buffer: &mut B, text: &str) -> bool;
    pub fn delete_backward<B: TextBufferMut>(&mut self, buffer: &mut B) -> bool;
    pub fn delete_forward<B: TextBufferMut>(&mut self, buffer: &mut B) -> bool;
    pub fn delete_word_backward<B: TextBufferMut>(&mut self, buffer: &mut B) -> bool;
    pub fn delete_word_forward<B: TextBufferMut>(&mut self, buffer: &mut B) -> bool;
    pub fn delete_selection<B: TextBufferMut>(&mut self, buffer: &mut B) -> bool;

    // Movement operations
    pub fn move_cursor_left<B: TextBuffer>(&mut self, buffer: &B);
    pub fn move_cursor_right<B: TextBuffer>(&mut self, buffer: &B);
    pub fn move_cursor_up<B: TextBuffer>(&mut self, buffer: &B);
    pub fn move_cursor_down<B: TextBuffer>(&mut self, buffer: &B);
    pub fn move_cursor_word_left<B: TextBuffer>(&mut self, buffer: &B);
    pub fn move_cursor_word_right<B: TextBuffer>(&mut self, buffer: &B);
    pub fn move_cursor_line_start<B: TextBuffer>(&mut self, buffer: &B);
    pub fn move_cursor_line_end<B: TextBuffer>(&mut self, buffer: &B);
    pub fn move_cursor_document_start(&mut self);
    pub fn move_cursor_document_end<B: TextBuffer>(&mut self, buffer: &B);

    // Selection operations
    pub fn select_all<B: TextBuffer>(&mut self, buffer: &B);
    pub fn select_word<B: TextBuffer>(&mut self, buffer: &B);
    pub fn select_line<B: TextBuffer>(&mut self, buffer: &B);
    pub fn extend_selection_left<B: TextBuffer>(&mut self, buffer: &B);
    // ... (with_selection variants for all movement)

    // History operations
    pub fn undo<B: TextBufferMut>(&mut self, buffer: &mut B) -> bool;
    pub fn redo<B: TextBufferMut>(&mut self, buffer: &mut B) -> bool;

    // Clipboard operations
    pub fn copy<B: TextBuffer>(&self, buffer: &B) -> Option<String>;
    pub fn cut<B: TextBufferMut>(&mut self, buffer: &mut B) -> Option<String>;
    pub fn paste<B: TextBufferMut>(&mut self, buffer: &mut B, text: &str) -> bool;
}
```

---

## 5. Phase 2: Message and Update System

### 5.1 Unified TextEditMsg

```rust
// src/messages/text_edit.rs (new file)

#[derive(Debug, Clone)]
pub enum TextEditMsg {
    // Movement
    Move(MoveTarget),
    MoveWithSelection(MoveTarget),

    // Editing
    InsertChar(char),
    InsertText(String),
    InsertNewline,
    DeleteBackward,
    DeleteForward,
    DeleteWordBackward,
    DeleteWordForward,
    DeleteLine,

    // Selection
    SelectAll,
    SelectWord,
    SelectLine,
    ClearSelection,
    ExpandSelection,
    ShrinkSelection,

    // Clipboard
    Copy,
    Cut,
    Paste,

    // History
    Undo,
    Redo,

    // Multi-cursor (editor-only)
    AddCursor { line: usize, column: usize },
    AddCursorAbove,
    AddCursorBelow,
    CollapseToSingleCursor,

    // Line operations (editor-only)
    Duplicate,
    IndentLines,
    UnindentLines,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveTarget {
    Left, Right, Up, Down,
    WordLeft, WordRight,
    LineStart, LineEnd,
    DocumentStart, DocumentEnd,
    PageUp, PageDown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditContext {
    Editor(GroupId),
    CommandPalette,
    GotoLine,
    FindQuery,
    FindReplace,
    CsvCell(CellPosition),
}
```

### 5.2 Update to Top-Level Msg

```rust
// src/messages.rs (add to existing)

pub enum Msg {
    // ... existing variants ...

    /// Unified text editing operations with explicit context
    TextEdit(EditContext, TextEditMsg),
}
```

### 5.3 Unified Update Handler

```rust
// src/update/text_edit.rs (new file)

pub fn update_text_edit(
    model: &mut AppModel,
    context: EditContext,
    msg: TextEditMsg,
) -> Option<Cmd> {
    // Check if operation is supported by context
    let caps = context.capabilities();
    if !is_operation_supported(&msg, &caps) {
        return None;
    }

    match context {
        EditContext::Editor(group_id) => update_editor_text(model, group_id, msg),
        EditContext::CommandPalette => update_modal_text(model, ModalId::CommandPalette, msg),
        EditContext::GotoLine => update_modal_text(model, ModalId::GotoLine, msg),
        EditContext::FindQuery | EditContext::FindReplace => {
            update_modal_text(model, ModalId::FindReplace, msg)
        }
        EditContext::CsvCell(position) => update_csv_cell_text(model, position, msg),
    }
}
```

### 5.4 Context Determination (Input Layer)

```rust
// src/runtime/input.rs (update)

fn determine_edit_context(model: &AppModel) -> Option<EditContext> {
    // Modal has highest priority
    if let Some(ref modal) = model.ui.active_modal {
        return Some(match modal.id() {
            ModalId::CommandPalette => EditContext::CommandPalette,
            ModalId::GotoLine => EditContext::GotoLine,
            ModalId::FindReplace => EditContext::FindQuery,
            _ => return None,
        });
    }

    // CSV cell editing
    if let Some(editor) = model.editor_area.focused_editor() {
        if let Some(csv) = editor.view_mode.as_csv() {
            if csv.is_editing() {
                return Some(EditContext::CsvCell(csv.selected_cell));
            }
        }
    }

    // Default to editor
    if model.ui.focus == FocusTarget::Editor {
        Some(EditContext::Editor(model.editor_area.focused_group_id))
    } else {
        None
    }
}
```

---

## 6. Phase 3: Rendering Consolidation

### 6.1 New Text Field Module

```rust
// src/view/text_field.rs (new file)

#[derive(Debug, Clone, Copy, Default)]
pub enum CursorStyle {
    #[default]
    Pipe,      // |
    Block,     // block
    Underline, // _
}

pub trait TextFieldContent {
    fn text(&self) -> &str;
    fn cursor_char_position(&self) -> usize;
    fn selection(&self) -> Option<&Selection>;
    fn highlights(&self) -> &[HighlightToken];
}

#[derive(Debug, Clone)]
pub struct TextFieldRenderOptions {
    pub background_color: u32,
    pub text_color: u32,
    pub cursor_color: u32,
    pub secondary_cursor_color: Option<u32>,
    pub cursor_style: CursorStyle,
    pub selection_color: Option<u32>,
    pub cursor_visible: bool,
    pub syntax_highlighting: bool,
    pub scroll_offset_chars: usize,
    pub padding: TextFieldPadding,
    pub single_line: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct TextFieldBounds {
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
}

pub struct TextFieldRenderer;

impl TextFieldRenderer {
    pub fn render<C: TextFieldContent>(
        frame: &mut Frame,
        painter: &mut TextPainter,
        content: &C,
        bounds: TextFieldBounds,
        options: &TextFieldRenderOptions,
        char_width: f32,
        line_height: usize,
    ) {
        // 1. Fill background
        // 2. Render selection (if any)
        // 3. Render text (with or without syntax highlighting)
        // 4. Render cursor(s) if visible
    }

    fn render_selection(...) { /* selection background */ }
    fn render_cursor(...) { /* cursor based on style */ }
}
```

### 6.2 Content Adapters

```rust
// Single-line input (modals)
pub struct SingleLineContent<'a> {
    text: &'a str,
    cursor_pos: usize,
    selection: Option<&'a Selection>,
}

// CSV cell
pub struct CsvCellContent<'a> {
    edit_state: &'a CellEditState,
}

// Editor line (for incremental migration)
pub struct EditorLineContent<'a> {
    line_text: &'a str,
    cursor_column: Option<usize>,
    selection: Option<Selection>,
    highlights: &'a [HighlightToken],
}
```

### 6.3 Factory Methods

```rust
impl TextFieldRenderOptions {
    pub fn for_modal_input(theme: &Theme, cursor_visible: bool) -> Self;
    pub fn for_csv_cell(theme: &Theme, cursor_visible: bool) -> Self;
    pub fn for_editor_line(theme: &Theme, cursor_visible: bool, is_current_line: bool) -> Self;
}
```

---

## 7. Phase 4: Migration Strategy

### 7.1 Overview

The migration is designed to be **incremental and non-breaking**. Each phase can be merged independently, and old code coexists with new code during transition.

### 7.2 Migration Order

```
Phase 1: Core Data Model (foundation)
    |
Phase 2: CSV Cell Editor (simplest context)
    |
Phase 3: Modal Inputs (command palette, goto line, find/replace)
    |
Phase 4: Rendering Consolidation
    |
Phase 5: Main Editor (largest, most complex)
    |
Phase 6: Cleanup (remove deprecated code)
```

### 7.3 Detailed Migration Steps

#### Step 1: Create Foundation

1. Create `src/editable/` module with all types
2. Implement `TextBuffer` for `Rope` and `String`
3. Add comprehensive unit tests
4. Add `TextEditMsg` and `EditContext` to messages
5. Create `update_text_edit()` stub

#### Step 2: Migrate CSV Cell Editor

1. Replace `CellEditState` buffer management with `EditableState::csv_cell()`
2. Update `CsvMsg` to delegate editing to `TextEditMsg`
3. Keep `CsvMsg` for cell navigation
4. Test: Start editing, type, backspace, cursor movement, confirm, cancel

#### Step 3: Migrate Modal Inputs

1. Add `EditableState` field to `CommandPaletteState`, `GotoLineState`, `FindReplaceState`
2. Route `ModalMsg::InsertChar`, `DeleteBackward`, etc. to `update_text_edit()`
3. Keep `ModalMsg` for list navigation (SelectPrevious, SelectNext, Confirm, Close)
4. Test: Command palette input, goto line, find/replace query

#### Step 4: Rendering Consolidation

1. Create `src/view/text_field.rs`
2. Migrate modal input rendering to use `TextFieldRenderer`
3. Migrate CSV cell editor rendering
4. Extract cursor/selection rendering from main editor (for reuse)
5. Test: Visual appearance matches before/after

#### Step 5: Main Editor Migration

This is the largest change and should be done incrementally:

1. **Bridge phase**: Map `TextEditMsg` to existing `EditorMsg`/`DocumentMsg`
2. **Movement phase**: Migrate cursor movement to use `EditableState`
3. **Editing phase**: Migrate insert/delete operations
4. **Selection phase**: Migrate selection operations
5. **Multi-cursor phase**: Ensure multi-cursor still works
6. **Full migration**: Remove bridge, use `EditableState` directly

#### Step 6: Cleanup

1. Remove deprecated `EditorMsg` variants
2. Remove deprecated `DocumentMsg` variants
3. Remove old `CellEditState` (replaced by `EditableState`)
4. Remove `delete_word_backward()` from `update_ui.rs`
5. Simplify message routing in `update_inner()`
6. Update documentation

---

## 8. Implementation Order

### Milestone 1: Foundation

- [ ] Create `src/editable/mod.rs`
- [ ] Create `src/editable/buffer.rs` with `TextBuffer` trait
- [ ] Implement `TextBuffer` for `ropey::Rope`
- [ ] Implement `TextBuffer` for `String`
- [ ] Create `src/editable/cursor.rs` with `Position`, `Cursor`
- [ ] Create `src/editable/selection.rs` with `Selection`
- [ ] Create `src/editable/history.rs` with `EditOperation`, `EditHistory`
- [ ] Create `src/editable/constraints.rs` with `EditConstraints`, filters
- [ ] Create `src/editable/state.rs` with `EditableState`
- [ ] Add unit tests for all new types

### Milestone 2: Message System

- [ ] Create `src/messages/text_edit.rs` with `TextEditMsg`, `MoveTarget`, `EditContext`
- [ ] Add `Msg::TextEdit(EditContext, TextEditMsg)` to `src/messages.rs`
- [ ] Create `src/update/text_edit.rs` with `update_text_edit()`
- [ ] Add dispatch in `src/update/mod.rs` for new message type
- [ ] Add `determine_edit_context()` helper

### Milestone 3: CSV Cell Migration

- [ ] Update `CellEditState` to use `EditableState`
- [ ] Update `CsvMsg` editing variants to delegate to `TextEditMsg`
- [ ] Update `update_csv()` to use unified handler
- [ ] Test cell editing lifecycle

### Milestone 4: Modal Migration

- [ ] Add `EditableState` to `CommandPaletteState`
- [ ] Add `EditableState` to `GotoLineState`
- [ ] Add `EditableState` to `FindReplaceState`
- [ ] Update `handle_modal_key()` to emit `TextEditMsg`
- [ ] Update `update_modal()` to delegate editing
- [ ] Test all modal inputs

### Milestone 5: Rendering

- [ ] Create `src/view/text_field.rs`
- [ ] Implement `TextFieldRenderer`
- [ ] Create content adapters
- [ ] Migrate modal input rendering
- [ ] Migrate CSV cell rendering
- [ ] Visual regression testing

### Milestone 6: Main Editor

- [ ] Create bridge from `TextEditMsg` to `EditorMsg`/`DocumentMsg`
- [ ] Incrementally migrate movement operations
- [ ] Incrementally migrate editing operations
- [ ] Incrementally migrate selection operations
- [ ] Verify multi-cursor support
- [ ] Remove bridge, use `EditableState` directly
- [ ] Full test suite pass

### Milestone 7: Cleanup

- [ ] Remove deprecated message variants
- [ ] Remove old `CellEditState`
- [ ] Remove duplicated code
- [ ] Update documentation
- [ ] Final test pass

---

## 9. File Change Summary

### New Files

| File | Purpose |
|------|---------|
| `src/editable/mod.rs` | Module exports |
| `src/editable/buffer.rs` | TextBuffer trait + implementations |
| `src/editable/cursor.rs` | Position, Cursor types |
| `src/editable/selection.rs` | Selection type |
| `src/editable/history.rs` | EditOperation, EditHistory |
| `src/editable/constraints.rs` | EditConstraints, char filters |
| `src/editable/state.rs` | EditableState main abstraction |
| `src/messages/text_edit.rs` | TextEditMsg, MoveTarget, EditContext |
| `src/update/text_edit.rs` | Unified update handler |
| `src/view/text_field.rs` | TextFieldRenderer |

### Modified Files

| File | Changes |
|------|---------|
| `src/lib.rs` | Add `editable` module |
| `src/messages.rs` | Add `Msg::TextEdit` variant |
| `src/update/mod.rs` | Add dispatch for `TextEdit` |
| `src/runtime/input.rs` | Add `determine_edit_context()` |
| `src/model/ui.rs` | Update modal states with `EditableState` |
| `src/csv/model.rs` | Update `CellEditState` |
| `src/update/ui.rs` | Delegate to `update_text_edit()` |
| `src/update/csv.rs` | Delegate to `update_text_edit()` |
| `src/view/mod.rs` | Use `TextFieldRenderer` |

### Files to Deprecate (Phase 6)

| File | Deprecation |
|------|-------------|
| `src/update/ui.rs:15-25` | Remove `delete_word_backward()` |
| Parts of `src/update/editor.rs` | Remove duplicated movement code |
| Parts of `src/update/document.rs` | Remove after full migration |

---

## 10. Testing Strategy

### Unit Tests

For each new type in `src/editable/`:
- `TextBuffer` trait implementations (Rope and String)
- `Cursor` movement operations
- `Selection` manipulation
- `EditHistory` push/pop/undo/redo
- `EditConstraints` character filtering
- `EditableState` editing operations

### Integration Tests

- Modal input: Type, backspace, word delete, undo, redo
- CSV cell: Full edit lifecycle
- Cross-context: Same operations work consistently
- Clipboard: Copy/cut/paste across contexts

### Regression Tests

- Existing test suite must pass throughout migration
- Visual comparison: Before/after screenshots
- Multi-cursor: Verify no regressions in complex scenarios

### Manual Testing Checklist

- [ ] Command palette: Type, backspace, Option+Backspace, cursor visible
- [ ] Go to line: Only digits accepted, Enter confirms
- [ ] Find/Replace: Type in both fields, Tab switches focus
- [ ] CSV cell: Start edit, type, arrow keys move cursor, Enter confirms
- [ ] Main editor: All existing functionality preserved

---

## Appendix A: Key Invariants

1. **Cursor-Selection Parallel**: `cursors.len() == selections.len()` always
2. **Cursor-Head Match**: `cursors[i].to_position() == selections[i].head` always
3. **Sorted Cursors**: Cursors kept sorted by (line, column)
4. **Active Index Valid**: `active_cursor_index < cursors.len()`
5. **Non-Empty Cursors**: At least one cursor always exists
6. **Redo Stack Cleared**: New edits clear redo stack
7. **Constraint Enforcement**: Operations rejected if they violate constraints

## Appendix B: Word Boundary Detection

Use existing `CharType` from `src/util/text.rs`:

```rust
pub enum CharType {
    Whitespace,
    WordChar,     // a-z, A-Z, 0-9, _
    Punctuation,
}

pub fn char_type(ch: char) -> CharType;
```

Word boundaries occur at `CharType` transitions (IntelliJ-style).

## Appendix C: Critical Files Reference

| File | Lines | Purpose |
|------|-------|---------|
| `src/model/editor.rs` | 1291 | Existing Cursor, Selection, EditorState |
| `src/model/document.rs` | 299 | Existing EditOperation, Document |
| `src/update/editor.rs` | 1263 | Cursor movement to study |
| `src/update/document.rs` | 2007 | Text editing to study |
| `src/model/ui.rs` | 135 | Modal states to update |
| `src/csv/model.rs` | 603 | CellEditState to replace |
| `src/view/mod.rs` | 1835 | Rendering to consolidate |
| `src/util/text.rs` | 114 | CharType for word boundaries |
