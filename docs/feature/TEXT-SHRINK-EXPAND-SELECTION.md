# Expand / Shrink Selection

Progressively expand or contract the selection scope around the cursor position.

---

## Overview

### Keyboard Shortcuts

| Platform | Expand Selection | Shrink Selection |
|----------|------------------|------------------|
| **macOS** | ⌥↑ (Option+Up) | ⌥↓ (Option+Down) |
| **Windows/Linux** | Ctrl+W | Ctrl+Shift+W |

### Behavior Summary

**Expand Selection** progressively widens the selection in semantic scope:
- From cursor → word → line → entire document

**Shrink Selection** reverses the expansion by restoring the previous selection state.

---

## Part 1: Expansion Levels (Plaintext)

For plain text editing without syntax awareness, we implement three expansion levels:

```
Level 0: No selection (cursor only)
   │
   │ ⌥↑ Expand
   ▼
Level 1: Word
   │
   │ ⌥↑ Expand
   ▼
Level 2: Line
   │
   │ ⌥↑ Expand
   ▼
Level 3: All (entire document)
```

### Level Definitions

| Level | Name | Selection Scope |
|-------|------|-----------------|
| 0 | Cursor | Empty selection (anchor == head) |
| 1 | Word | Contiguous characters of same `CharType` around cursor |
| 2 | Line | Entire line including trailing newline |
| 3 | All | From document start (0,0) to document end |

### Shrink Path

Shrinking restores the previous selection from a history stack:

```
All → Line → Word → Cursor
```

---

## Part 2: Data Structures

### Selection History Stack

To support shrinking, we maintain a stack of previous selections:

```rust
// In EditorState (src/model/editor.rs)
pub struct EditorState {
    // ... existing fields

    /// Stack of previous selections for shrink functionality
    /// Push before expanding, pop when shrinking
    pub selection_history: Vec<Selection>,
}
```

### Messages

```rust
// In messages.rs
pub enum EditorMsg {
    // ... existing variants

    /// Expand selection to next semantic level (⌥↑ or Ctrl+W)
    ExpandSelection,

    /// Shrink selection to previous level (⌥↓ or Ctrl+Shift+W)
    ShrinkSelection,
}
```

---

## Part 3: Algorithm

### Expand Selection

```rust
fn expand_selection(model: &mut AppModel) -> Option<Cmd> {
    let current = model.editor.selection().clone();

    // Push current selection to history before expanding
    model.editor.selection_history.push(current.clone());

    if current.is_empty() {
        // Level 0 → 1: Select word
        select_word_at_cursor(model);
    } else if is_word_selection(&current, model) {
        // Level 1 → 2: Select line
        select_current_line(model);
    } else if is_line_selection(&current, model) {
        // Level 2 → 3: Select all
        select_all(model);
    } else {
        // Already at max or custom selection - try to find next logical scope
        // For plaintext: expand to line if partial, else to all
        if is_within_single_line(&current) {
            select_current_line(model);
        } else {
            select_all(model);
        }
    }

    Some(Cmd::Redraw)
}
```

### Shrink Selection

```rust
fn shrink_selection(model: &mut AppModel) -> Option<Cmd> {
    if let Some(previous) = model.editor.selection_history.pop() {
        // Restore previous selection
        *model.editor.selection_mut() = previous.clone();

        // Update cursor to match selection head
        model.editor.cursor_mut().line = previous.head.line;
        model.editor.cursor_mut().column = previous.head.column;
        model.editor.cursor_mut().desired_column = None;

        Some(Cmd::Redraw)
    } else {
        // No history - collapse to cursor
        model.editor.clear_selection();
        Some(Cmd::Redraw)
    }
}
```

### History Management

Clear selection history when any other action modifies the selection:

```rust
fn clear_selection_history(model: &mut AppModel) {
    model.editor.selection_history.clear();
}

// Call this in:
// - Any cursor movement (arrow keys, mouse click)
// - Any edit operation (insert, delete)
// - Manual selection change (shift+arrow, mouse drag)
// - ClearSelection (Escape)
```

### Detection Helpers

```rust
/// Check if selection exactly covers a word boundary
fn is_word_selection(selection: &Selection, model: &AppModel) -> bool {
    if selection.start().line != selection.end().line {
        return false;  // Multi-line is not word
    }

    let line = selection.start().line;
    if let Some(line_text) = model.document.get_line(line) {
        let chars: Vec<char> = line_text.chars().collect();
        let start_col = selection.start().column;
        let end_col = selection.end().column;

        if start_col >= chars.len() || end_col > chars.len() {
            return false;
        }

        // Check all chars in selection are same type
        let first_type = char_type(chars[start_col]);
        let all_same = (start_col..end_col)
            .all(|i| char_type(chars[i]) == first_type);

        if !all_same {
            return false;
        }

        // Check boundaries are at type transitions
        let at_word_start = start_col == 0
            || char_type(chars[start_col - 1]) != first_type;
        let at_word_end = end_col >= chars.len()
            || char_type(chars[end_col]) != first_type
            || chars[end_col] == '\n';

        at_word_start && at_word_end
    } else {
        false
    }
}

/// Check if selection covers exactly one line (including newline)
fn is_line_selection(selection: &Selection, model: &AppModel) -> bool {
    let start = selection.start();
    let end = selection.end();

    // Must start at column 0
    if start.column != 0 {
        return false;
    }

    // Single line: must end at line length or start of next line
    if start.line == end.line {
        if let Some(line_text) = model.document.get_line(start.line) {
            return end.column == line_text.chars().count();
        }
    } else if end.line == start.line + 1 && end.column == 0 {
        // Selection ends at start of next line (includes newline)
        return true;
    }

    false
}

/// Check if selection is entirely within a single line
fn is_within_single_line(selection: &Selection) -> bool {
    selection.start().line == selection.end().line
}
```

---

## Part 4: Edge Cases

| Scenario | Behavior |
|----------|----------|
| **Cursor at word boundary** | Expand to the word the cursor is "inside" (prefer right/forward) |
| **Cursor on whitespace** | Select whitespace run as a "word", then line |
| **Empty line** | Level 1 (word) selects nothing, skip to line |
| **Cursor at end of line** | Select previous word if at boundary |
| **Last line without newline** | Line selection extends to end of content |
| **Empty document** | All levels result in empty selection at (0,0) |
| **Shrink with empty history** | Collapse selection to cursor position |
| **Manual selection then expand** | Push manual selection to history, expand to next level |

### Word Boundary Special Cases

Using modified `CharType` classification (underscore treated as word character):

```rust
// Text: "hello_world   func()"
//       ^^^^^^^^^^^ one word (underscore is word char)
//                  ^^^ whitespace
//                     ^^^^ word
//                         ^^ punctuation

// Cursor positions and expand behavior:
// "hel|lo_world" → expand selects "hello_world" (whole identifier)
// "hello  |  world" → expand selects whitespace run "   "
// "func|()" → expand selects "func", then line
```

**Implementation note:** Modify `is_punctuation()` in `src/util.rs` to exclude underscore from punctuation, making `_` behave as a word character (Python/Rust identifier style).

---

## Part 5: Test Cases

### Basic Expansion Flow

```
Test: expand_from_cursor_selects_word
Given: text "hello world" with cursor at position (0, 2) (inside "hello")
When: ExpandSelection
Then: Selection covers (0,0) to (0,5) - "hello"

Test: expand_from_word_selects_line
Given: text "hello world\n" with selection (0,0)-(0,5) covering "hello"
When: ExpandSelection
Then: Selection covers (0,0) to (1,0) - entire line including newline

Test: expand_from_line_selects_all
Given: text "hello\nworld\n" with selection covering line 0
When: ExpandSelection
Then: Selection covers (0,0) to (2,0) - entire document
```

### Shrink Flow

```
Test: shrink_restores_previous_selection
Given: expanded from word to line (history contains word selection)
When: ShrinkSelection
Then: Selection restored to word boundaries

Test: shrink_from_word_to_cursor
Given: expanded from cursor to word (history contains empty selection)
When: ShrinkSelection
Then: Selection becomes empty, cursor at original position

Test: shrink_with_empty_history_clears
Given: selection exists but history is empty
When: ShrinkSelection
Then: Selection cleared, cursor at head position
```

### Edge Cases

```
Test: expand_on_whitespace
Given: text "hello   world" with cursor at (0, 6) (in whitespace)
When: ExpandSelection
Then: Selection covers whitespace "   "

Test: expand_on_empty_line
Given: text "hello\n\nworld" with cursor on line 1 (empty)
When: ExpandSelection
Then: Skip word level, select empty line (just newline)

Test: expand_at_word_boundary
Given: text "hello world" with cursor at (0, 5) (between words)
When: ExpandSelection
Then: Select "hello" (word to the left) or whitespace (if cursor on space)

Test: history_cleared_on_edit
Given: expanded to word, history = [empty selection]
When: Insert character
Then: history = [] (cleared)

Test: history_cleared_on_movement
Given: expanded to word, history = [empty selection]
When: Move cursor left
Then: history = [] (cleared)

Test: underscore_is_word_char
Given: text "hello_world" with cursor at (0, 3)
When: ExpandSelection
Then: Selection covers (0,0) to (0,11) - entire "hello_world"
```

### Multi-Line Selection

```
Test: expand_partial_selection_to_lines
Given: text "hello\nworld" with selection (0,2)-(1,2) (partial multi-line)
When: ExpandSelection
Then: Selection expands to cover both full lines

Test: expand_already_all_does_nothing
Given: entire document selected
When: ExpandSelection
Then: Selection unchanged (already at maximum)
```

---

## Part 6: Future - Semantic Expansion

> **Status: Future Feature** - Requires syntax tree integration (tree-sitter or similar)

When syntax awareness is available, the expansion levels become:

```
Level 0: Cursor (no selection)
   │
   ▼
Level 1: Word / Identifier
   │
   ▼
Level 2: Expression (string literal, function call args, etc.)
   │
   ▼
Level 3: Statement
   │
   ▼
Level 4: Block / Scope (braces, brackets, indentation block)
   │
   ▼
Level 5: Function / Method body
   │
   ▼
Level 6: Class / Module / Top-level item
   │
   ▼
Level 7: File (all)
```

### Example: Rust Code

```rust
fn process(data: &[u8]) {
    let result = data.iter().map(|x| x + 1).collect::<Vec<_>>();
    //                            ^cursor
}
```

Expand sequence from cursor on `x`:
1. `x` (identifier)
2. `x + 1` (expression)
3. `|x| x + 1` (closure)
4. `.map(|x| x + 1)` (method call)
5. `data.iter().map(|x| x + 1).collect::<Vec<_>>()` (full expression)
6. `let result = ...;` (statement)
7. `{ let result = ...; }` (function body block)
8. `fn process(...) { ... }` (function)
9. Entire file

### Implementation Notes (Future)

```rust
// Hypothetical interface
trait SyntaxExpander {
    /// Returns the smallest syntax node containing the selection
    fn containing_node(&self, selection: &Selection) -> Option<SyntaxNode>;

    /// Returns the parent node for expansion
    fn parent_node(&self, node: &SyntaxNode) -> Option<SyntaxNode>;

    /// Get selection range for a syntax node
    fn node_range(&self, node: &SyntaxNode) -> Selection;
}

fn expand_semantic(model: &mut AppModel, syntax: &impl SyntaxExpander) {
    let current = model.editor.selection().clone();
    model.editor.selection_history.push(current.clone());

    if let Some(node) = syntax.containing_node(&current) {
        if let Some(parent) = syntax.parent_node(&node) {
            let new_selection = syntax.node_range(&parent);
            *model.editor.selection_mut() = new_selection;
        }
    }
}
```

---

## Part 7: Implementation Checklist

### Phase 1: Core Feature
- [ ] Add `selection_history: Vec<Selection>` to `EditorState`
- [ ] Add `ExpandSelection` and `ShrinkSelection` to `EditorMsg`
- [ ] Implement `expand_selection()` in `src/update.rs`
- [ ] Implement `shrink_selection()` in `src/update.rs`
- [ ] Add keyboard handling for ⌥↑/⌥↓ (macOS) and Ctrl+W/Ctrl+Shift+W

### Phase 2: History Management
- [ ] Clear history on cursor movement
- [ ] Clear history on text edits
- [ ] Clear history on explicit selection changes
- [ ] Clear history on ClearSelection (Escape)

### Phase 3: Detection Functions
- [ ] Implement `is_word_selection()`
- [ ] Implement `is_line_selection()`
- [ ] Handle boundary edge cases

### Phase 4: Testing
- [ ] Unit tests for expand/shrink logic
- [ ] Unit tests for detection helpers
- [ ] Integration tests for keyboard shortcuts
- [ ] Edge case tests

---

## Files to Modify

| File | Changes |
|------|---------|
| `src/util.rs` | Remove `_` from punctuation list in `is_punctuation()` |
| `src/model/editor.rs` | Add `selection_history: Vec<Selection>` to `EditorState` |
| `src/messages.rs` | Add `ExpandSelection`, `ShrinkSelection` variants |
| `src/update.rs` | Implement expand/shrink logic and detection helpers |
| `src/main.rs` | Add keyboard handling for ⌥↑/⌥↓ and Ctrl+W/Ctrl+Shift+W |

---

## References

- JetBrains IDEs: Extend Selection (Ctrl+W / Cmd+W)
- VS Code: Expand Selection (Shift+Alt+Right)
- Sublime Text: Expand Selection to Scope (Ctrl+Shift+Space)
