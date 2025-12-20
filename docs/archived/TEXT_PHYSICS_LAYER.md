# Text Physics Layer: Architectural Concept

**Status:** Conceptual / Future consideration  
**Related:** [EDITOR_UI_REFERENCE.md](../EDITOR_UI_REFERENCE.md)

---

## Overview

The "text physics" layer is the pure, buffer-agnostic core that handles cursor movement, selection semantics, and viewport scrolling. It's the behavioral heart of an editor—everything that makes cursor movement "feel right."

Token already has this implicitly spread across `model/editor.rs`, `update/editor.rs`, and the tests. This document explores what a clean separation would look like.

---

## What "Text Physics" Includes

### State

```rust
pub struct TextViewState {
    // Multi-cursor support
    pub cursors: Vec<Cursor>,
    pub selections: Vec<Selection>,
    pub active_cursor: usize,
    
    // Viewport (in line/column space, not pixels)
    pub viewport: Viewport,  // top_line, left_column, visible_lines, visible_columns
    
    // Behavioral state
    pub scroll_mode: ScrollMode,  // CursorLocked, FreeBrowse, RevealPending
    pub rectangle_selection: Option<RectangleSelectionState>,
    pub selection_history: Vec<SelectionSnapshot>,  // for expand/shrink
}
```

### Invariants (Must Always Hold)

```rust
// CRITICAL: Parallel array invariant
assert!(cursors.len() == selections.len());

// CRITICAL: Cursor/selection head correspondence  
for i in 0..cursors.len() {
    assert!(cursors[i].to_position() == selections[i].head);
}

// Viewport: cursor should be in safe zone after any movement
assert!(cursor_in_safe_zone(primary_cursor, viewport, margins));

// Selection normalization: start <= end
for sel in &selections {
    assert!(sel.start() <= sel.end());
}
```

### Operations (Commands)

```rust
pub enum Motion {
    CharLeft, CharRight,
    LineUp, LineDown,
    WordLeft, WordRight,
    LineStart, LineEnd,      // Home/End
    LineStartSmart,          // Toggle between column 0 and first non-whitespace
    PageUp, PageDown,
    DocumentStart, DocumentEnd,
    // With wrapping: VisualLineUp, VisualLineDown
}

pub enum SelectionMode {
    Move,       // Clear selection, move cursor
    Extend,     // Shift+movement: extend selection
    AddCursor,  // Add new cursor (Cmd+Click, Option+Option+Arrow)
}

pub enum SelectionCommand {
    SelectWord,
    SelectLine,
    SelectAll,
    ExpandSelection,   // word → brackets → block → all
    ShrinkSelection,   // reverse via history stack
    SelectNextOccurrence,
    MergeOverlappingSelections,
}
```

---

## What's Outside the Physics Layer

| Layer | Responsibility | Example |
|-------|---------------|---------|
| **Buffer** | Text storage, edits, undo/redo | `Document`, ropey `Rope` |
| **Input** | Raw events → commands | winit → `Motion` + `SelectionMode` |
| **Render** | State → pixels | Glyphs, themes, GPU |
| **App** | Tabs, splits, files | `EditorArea`, workspace |

---

## The Buffer Abstraction

The physics layer needs to query the buffer but shouldn't know its implementation:

```rust
pub trait TextBuffer {
    fn line_count(&self) -> usize;
    fn line_length(&self, line: usize) -> usize;
    
    // For word movement
    fn char_type_at(&self, line: usize, column: usize) -> Option<CharType>;
    
    // For smart home
    fn first_non_whitespace_column(&self, line: usize) -> usize;
    
    // Optional: for advanced features
    fn line_text(&self, line: usize) -> Option<&str>;
}
```

Token's `Document` would implement this trait.

---

## The Physics API

```rust
impl TextViewState {
    /// Apply a motion command, updating cursors, selections, and viewport
    pub fn apply_motion<B: TextBuffer>(
        &mut self,
        buffer: &B,
        motion: Motion,
        selection_mode: SelectionMode,
        reveal_mode: ScrollRevealMode,
        metrics: &ViewMetrics,
    ) {
        // 1. Move each cursor according to motion
        // 2. Update selections according to selection_mode
        // 3. Deduplicate colliding cursors
        // 4. Scroll viewport to keep primary cursor visible
    }
    
    /// Handle mouse click (single, double, triple)
    pub fn handle_click<B: TextBuffer>(
        &mut self,
        buffer: &B,
        position: Position,
        click_count: u8,
        modifiers: Modifiers,
    );
    
    /// Handle mouse drag for selection extension
    pub fn handle_drag<B: TextBuffer>(
        &mut self,
        buffer: &B,
        position: Position,
        metrics: &ViewMetrics,
    );
    
    /// Free scroll (mouse wheel, scrollbar)
    pub fn scroll(&mut self, delta_lines: isize, delta_columns: isize);
    
    /// Compute visible line range for rendering
    pub fn visible_lines(&self) -> Range<usize>;
}
```

---

## Current Token Implementation

Token already has most of this, but distributed:

| Concept | Current Location |
|---------|-----------------|
| Cursor/Selection state | `model/editor.rs` |
| Viewport state | `model/editor.rs` (embedded in `EditorState`) |
| Movement primitives | `model/editor.rs` (`move_cursor_*_at`) |
| All-cursors wrappers | `model/editor.rs` (`move_all_cursors_*`) |
| Selection commands | `update/editor.rs` |
| Scroll logic | `model/editor.rs` (`ensure_cursor_visible_*`) |
| Invariant spec | `EDITOR_UI_REFERENCE.md` (chapters 4, 5) |
| Conformance tests | `tests/cursor_movement.rs`, `tests/selection.rs` |

---

## When to Formalize This Separation

Consider extracting a true "text physics" crate if:

1. **Multiple frontends** — TUI version, web version, or embedding in other apps
2. **Reuse requests** — Others want Token's cursor "feel" in their projects  
3. **Complexity growth** — BiDi, complex folding, or structural selections that benefit from isolation
4. **Testing friction** — Physics tests need too much app scaffolding

For now, the conceptual separation (knowing what belongs where) is sufficient.

---

## Appendix: Proposed Addition to EDITOR_UI_REFERENCE.md

Consider adding a new chapter:

### Chapter 15: System Invariants and Constraints

**15.1 Cursor-Selection Invariants**
```
cursors.len() == selections.len()
cursors[i].to_position() == selections[i].head
```

**15.2 Selection Normalization**
- `start()` always returns the earlier position
- `end()` always returns the later position  
- `is_reversed()` indicates if anchor > head

**15.3 Viewport Safety Zone**
- After any cursor movement, primary cursor must be within `(margins.top, viewport.height - margins.bottom)`
- Horizontal equivalent for horizontal scrolling

**15.4 Multi-Cursor Deduplication**  
- After any operation, no two cursors may occupy the same `(line, column)`
- When cursors collide, keep the one with lower index

**15.5 Selection Merging**
- Overlapping selections `[a, b)` and `[c, d)` where `c <= b` merge to `[a, max(b, d))`
- Adjacent selections (touching but not overlapping) also merge
