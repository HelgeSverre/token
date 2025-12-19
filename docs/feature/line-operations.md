# Line Operations

Join lines, duplicate lines/selections, and trim trailing whitespace.

> **Status:** Planned
> **Priority:** P2
> **Effort:** S
> **Created:** 2025-12-19
> **Milestone:** 2 - Search & Editing

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

The editor has some line operations implemented. In `src/keymap/command.rs`:

```rust
Command::Duplicate,         // Exists (Cmd+D style duplicate)
Command::DeleteLine,        // Exists (Cmd+Backspace)
// MoveLineUp / MoveLineDown do NOT exist yet
```

Missing operations:
- **Join Lines** - Merge current line with next line
- **Split Line** - Insert newline at cursor without moving cursor
- **Trim Trailing Whitespace** - Remove trailing spaces from lines

### Goals

1. **Join Lines** - Merge line with next, replacing newline with space
2. **Duplicate Line/Selection** - Copy line(s) or selection below cursor
3. **Trim Trailing Whitespace** - Remove trailing whitespace from:
   - Current line only
   - Selected lines
   - Entire document
4. **Multi-cursor support** - All operations work with multiple cursors
5. **Undo support** - Each operation is a single undo unit

### Non-Goals

- Sort lines (separate feature)
- Reverse lines (separate feature)
- Convert case (separate feature)
- Line filtering/grep (separate feature)

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         Line Operations System                               │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ┌────────────────────────────────────────────────────────────────────────┐ │
│  │                          Join Lines (Cmd+J)                             │ │
│  │                                                                         │ │
│  │  Before:    Line one                                                   │ │
│  │             Line two                                                   │ │
│  │                                                                         │ │
│  │  After:     Line one Line two                                          │ │
│  │                     ^                                                   │ │
│  │                     cursor here                                         │ │
│  │                                                                         │ │
│  │  Algorithm:                                                            │ │
│  │  1. Find newline at end of current line                                │ │
│  │  2. Find leading whitespace of next line                               │ │
│  │  3. Replace (newline + whitespace) with single space                   │ │
│  │  4. Position cursor at join point                                      │ │
│  └────────────────────────────────────────────────────────────────────────┘ │
│                                                                              │
│  ┌────────────────────────────────────────────────────────────────────────┐ │
│  │                     Duplicate Line/Selection (Cmd+Shift+D)              │ │
│  │                                                                         │ │
│  │  No Selection (duplicate line):                                        │ │
│  │  Before:    The quick brown fox    After:    The quick brown fox       │ │
│  │                  ^                                The quick brown fox   │ │
│  │             cursor                                    ^                 │ │
│  │                                                  cursor on duplicate   │ │
│  │                                                                         │ │
│  │  With Selection (duplicate selection):                                 │ │
│  │  Before:    Hello [world]          After:    Hello [worldworld]        │ │
│  │             selection                         cursor after             │ │
│  └────────────────────────────────────────────────────────────────────────┘ │
│                                                                              │
│  ┌────────────────────────────────────────────────────────────────────────┐ │
│  │                     Trim Trailing Whitespace                            │ │
│  │                                                                         │ │
│  │  Before:    def foo():   ␣␣␣                                           │ │
│  │                 pass  ␣                                                 │ │
│  │             return    ␣␣                                                │ │
│  │                                                                         │ │
│  │  After:     def foo():                                                 │ │
│  │                 pass                                                    │ │
│  │             return                                                      │ │
│  │                                                                         │ │
│  │  Modes:                                                                │ │
│  │  - Current line only (Cmd+Option+T)                                    │ │
│  │  - Selected lines                                                      │ │
│  │  - Entire document (via command palette)                               │ │
│  └────────────────────────────────────────────────────────────────────────┘ │
│                                                                              │
└──────────────────────────────────────────────────────────────────────────────┘
```

### Multi-Cursor Handling

```
Operation with multiple cursors:

  Line 1: Hello world
              ^
  Line 2: Foo bar
              ^
  Line 3: Baz qux

"Join Lines" with cursors on lines 1 and 2:

  Result:    Hello world Foo bar Baz qux
                        ^       ^
             (cursors at join points)

Algorithm:
  1. Collect all line joins needed
  2. Process in reverse document order (to maintain offsets)
  3. Update cursor positions after each operation
```

---

## Data Structures

### Line Operation Types

```rust
// src/update/document.rs

/// Types of line operations that can be performed
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineOperation {
    /// Join current line with next line
    JoinLines,
    /// Duplicate current line (or selection)
    Duplicate,
    /// Trim trailing whitespace from current line
    TrimTrailingWhitespace,
    /// Trim trailing whitespace from entire document
    TrimAllTrailingWhitespace,
}
```

### Join Line Implementation

```rust
// src/update/document.rs

use crate::model::{Document, EditorState, Position};

impl Document {
    /// Join line at `line_idx` with the next line
    ///
    /// Returns the column position where the join occurred (for cursor placement)
    pub fn join_line(&mut self, line_idx: usize) -> Option<usize> {
        if line_idx >= self.line_count().saturating_sub(1) {
            return None; // Can't join last line
        }

        // Find end of current line (before newline)
        let line_start = self.line_start_offset(line_idx);
        let line_end = self.line_end_offset(line_idx); // Position of \n

        // Find start of content on next line (skip leading whitespace)
        let next_line_start = line_end + 1; // After \n
        let next_line_content_start = self.first_non_whitespace_offset(line_idx + 1);

        // Calculate the join column (end of current line content)
        let current_line_content_end = self.last_non_whitespace_offset(line_idx);
        let join_column = if let Some(end) = current_line_content_end {
            // Has content - join column is after last non-whitespace + 1 (for space)
            end - line_start + 1
        } else {
            // Empty line - join at column 0
            0
        };

        // Replace: [trailing_ws + \n + leading_ws] with single space (or nothing if either line empty)
        let replace_start = current_line_content_end.unwrap_or(line_start);
        let replace_end = next_line_content_start;

        // Determine replacement: space if both lines have content, nothing otherwise
        let has_current_content = current_line_content_end.is_some();
        let has_next_content = next_line_content_start > next_line_start;
        let replacement = if has_current_content && has_next_content {
            " "
        } else {
            ""
        };

        // Perform the edit
        self.replace_range(replace_start, replace_end, replacement);

        Some(join_column)
    }

    /// Get offset of first non-whitespace character on line
    fn first_non_whitespace_offset(&self, line_idx: usize) -> usize {
        let line_start = self.line_start_offset(line_idx);
        let line = self.get_line(line_idx).unwrap_or_default();

        let ws_count = line.chars().take_while(|c| c.is_whitespace()).count();
        line_start + ws_count
    }

    /// Get offset of last non-whitespace character on line (exclusive)
    fn last_non_whitespace_offset(&self, line_idx: usize) -> Option<usize> {
        let line_start = self.line_start_offset(line_idx);
        let line = self.get_line(line_idx).unwrap_or_default();

        // Remove trailing newline for calculation
        let content = line.trim_end_matches('\n');
        if content.is_empty() {
            return None;
        }

        let trimmed = content.trim_end();
        if trimmed.is_empty() {
            return None;
        }

        Some(line_start + trimmed.len())
    }
}
```

### Duplicate Implementation

```rust
// src/update/document.rs

impl Document {
    /// Duplicate the line at `line_idx`
    ///
    /// Returns the new line index (which is line_idx + 1, containing the copy)
    pub fn duplicate_line(&mut self, line_idx: usize) -> usize {
        let line_content = self.get_line(line_idx).unwrap_or_default();

        // Ensure line ends with newline
        let content = if line_content.ends_with('\n') {
            line_content.to_string()
        } else {
            format!("{}\n", line_content)
        };

        // Insert after current line
        let insert_offset = self.line_start_offset(line_idx + 1);
        self.insert_at(insert_offset, &content);

        line_idx + 1
    }

    /// Duplicate text in the given range (for selection duplication)
    ///
    /// Inserts copy immediately after the range.
    /// Returns the new end offset of the duplicated text.
    pub fn duplicate_range(&mut self, start: usize, end: usize) -> usize {
        let text = self.text_range(start, end);
        self.insert_at(end, &text);
        end + text.len()
    }
}
```

### Trim Whitespace Implementation

```rust
// src/update/document.rs

impl Document {
    /// Trim trailing whitespace from a single line
    ///
    /// Returns true if any whitespace was removed
    pub fn trim_trailing_whitespace_line(&mut self, line_idx: usize) -> bool {
        let line_start = self.line_start_offset(line_idx);
        let line = self.get_line(line_idx).unwrap_or_default();

        // Find trailing whitespace before newline
        let has_newline = line.ends_with('\n');
        let content = if has_newline {
            &line[..line.len() - 1]
        } else {
            &line[..]
        };

        let trimmed = content.trim_end();

        if trimmed.len() < content.len() {
            // There is trailing whitespace to remove
            let trim_start = line_start + trimmed.len();
            let trim_end = line_start + content.len();

            self.delete_range(trim_start, trim_end);
            true
        } else {
            false
        }
    }

    /// Trim trailing whitespace from all lines in range
    ///
    /// Returns number of lines modified
    pub fn trim_trailing_whitespace_range(&mut self, start_line: usize, end_line: usize) -> usize {
        let mut modified = 0;

        // Process in reverse order to maintain line indices
        for line_idx in (start_line..=end_line).rev() {
            if self.trim_trailing_whitespace_line(line_idx) {
                modified += 1;
            }
        }

        modified
    }

    /// Trim trailing whitespace from entire document
    ///
    /// Returns number of lines modified
    pub fn trim_all_trailing_whitespace(&mut self) -> usize {
        let line_count = self.line_count();
        self.trim_trailing_whitespace_range(0, line_count.saturating_sub(1))
    }
}
```

### EditorState Integration

```rust
// src/update/document_ops.rs

impl EditorState {
    /// Join current line(s) with next line(s)
    ///
    /// For each cursor, joins its line with the following line.
    /// Multiple cursors are processed in reverse order.
    pub fn join_lines(&mut self, doc: &mut Document) {
        // Collect unique lines to join (avoid joining same line twice)
        let mut lines_to_join: Vec<usize> = self
            .cursors
            .iter()
            .map(|c| c.line)
            .collect();
        lines_to_join.sort_unstable();
        lines_to_join.dedup();

        // Process in reverse order
        for line_idx in lines_to_join.into_iter().rev() {
            if let Some(join_column) = doc.join_line(line_idx) {
                // Update cursors on this line
                for (cursor, selection) in self.cursors.iter_mut().zip(self.selections.iter_mut()) {
                    if cursor.line == line_idx {
                        cursor.column = join_column;
                        cursor.desired_column = None;
                        *selection = Selection::new(cursor.to_position());
                    }
                }

                // Update cursors on lines below (they moved up by one)
                for cursor in &mut self.cursors {
                    if cursor.line > line_idx {
                        cursor.line -= 1;
                    }
                }
            }
        }

        self.deduplicate_cursors();
    }

    /// Duplicate current line(s) or selection(s)
    pub fn duplicate(&mut self, doc: &mut Document) {
        let has_selection = !self.primary_selection().is_empty();

        if has_selection {
            // Duplicate selection
            self.duplicate_selection(doc);
        } else {
            // Duplicate line
            self.duplicate_lines(doc);
        }
    }

    /// Duplicate lines containing cursors
    fn duplicate_lines(&mut self, doc: &mut Document) {
        // Collect unique lines
        let mut lines: Vec<usize> = self.cursors.iter().map(|c| c.line).collect();
        lines.sort_unstable();
        lines.dedup();

        // Process in reverse order
        for line_idx in lines.into_iter().rev() {
            let new_line = doc.duplicate_line(line_idx);

            // Move cursors on this line to the duplicate
            for cursor in &mut self.cursors {
                if cursor.line == line_idx {
                    cursor.line = new_line;
                }
            }

            // Update cursors below
            for cursor in &mut self.cursors {
                if cursor.line > new_line {
                    cursor.line += 1;
                }
            }
        }

        self.collapse_selections_to_cursors();
    }

    /// Duplicate selections (insert copy after each selection)
    fn duplicate_selection(&mut self, doc: &mut Document) {
        // Process selections in reverse order by position
        let mut indexed: Vec<(usize, &Selection)> = self
            .selections
            .iter()
            .enumerate()
            .collect();
        indexed.sort_by(|a, b| b.1.end().cmp(&a.1.end()));

        for (idx, selection) in indexed {
            if selection.is_empty() {
                continue;
            }

            let start = doc.cursor_to_offset(selection.start().line, selection.start().column);
            let end = doc.cursor_to_offset(selection.end().line, selection.end().column);

            let new_end = doc.duplicate_range(start, end);

            // Move cursor to end of duplicated text
            let (line, column) = doc.offset_to_cursor(new_end);
            self.cursors[idx] = Cursor::at(line, column);
            self.selections[idx] = Selection::new(Position::new(line, column));
        }

        self.sort_cursors();
        self.deduplicate_cursors();
    }

    /// Trim trailing whitespace from lines with cursors
    pub fn trim_trailing_whitespace_at_cursors(&mut self, doc: &mut Document) {
        let mut lines: Vec<usize> = self.cursors.iter().map(|c| c.line).collect();
        lines.sort_unstable();
        lines.dedup();

        for line_idx in lines.into_iter().rev() {
            doc.trim_trailing_whitespace_line(line_idx);
        }

        // Adjust cursor columns if they were in trimmed whitespace
        for cursor in &mut self.cursors {
            let line_len = doc.line_length(cursor.line);
            if cursor.column > line_len {
                cursor.column = line_len;
            }
        }

        self.collapse_selections_to_cursors();
    }
}
```

---

## Keybindings

| Action | Mac | Windows/Linux | Notes |
|--------|-----|---------------|-------|
| Join Lines | Ctrl+J | Ctrl+J | Join with next line |
| Duplicate Line/Selection | Cmd+Shift+D | Ctrl+Shift+D | NEW: Different from Cmd+D |
| Trim Line Whitespace | Cmd+Option+T | Ctrl+Alt+T | NEW: Current line(s) |
| Trim Document Whitespace | - | - | Via command palette |
| Move Line Up | Option+Up | Alt+Up | Existing |
| Move Line Down | Option+Down | Alt+Down | Existing |
| Delete Line | Cmd+Backspace | Ctrl+Shift+K | Existing |

Note: Cmd+D is reserved for Select Next Occurrence (F-070).

---

## Implementation Plan

### Phase 1: Document Methods

**Files:** `src/model/document.rs`

- [ ] Add `join_line()` method
- [ ] Add helper methods for whitespace detection
- [ ] Add `duplicate_line()` method
- [ ] Add `duplicate_range()` method
- [ ] Add `trim_trailing_whitespace_line()` method
- [ ] Add `trim_trailing_whitespace_range()` method
- [ ] Add `trim_all_trailing_whitespace()` method

**Test:** Join line removes newline and leading whitespace.

### Phase 2: EditorState Methods

**Files:** `src/update/document.rs` or new `src/update/line_ops.rs`

- [ ] Implement `join_lines()` with multi-cursor support
- [ ] Implement `duplicate()` dispatcher
- [ ] Implement `duplicate_lines()` for no selection
- [ ] Implement `duplicate_selection()` for selections
- [ ] Implement `trim_trailing_whitespace_at_cursors()`

**Test:** Join lines with 2 cursors joins 2 separate lines.

### Phase 3: Commands

**Files:** `src/keymap/command.rs`

- [ ] Add `Command::JoinLines`
- [ ] Add `Command::DuplicateLine` (distinct from existing Duplicate)
- [ ] Add `Command::TrimTrailingWhitespace`
- [ ] Add `Command::TrimAllTrailingWhitespace`

**Test:** Commands defined and available in command palette.

### Phase 4: Message Handling

**Files:** `src/messages.rs`, `src/update/document.rs`

- [ ] Add corresponding DocumentMsg variants
- [ ] Handle messages in update function
- [ ] Ensure proper undo grouping

**Test:** Each operation is a single undo.

### Phase 5: Keybindings

**Files:** `src/keymap/defaults.rs`

- [ ] Bind Ctrl+J to JoinLines
- [ ] Bind Cmd+Shift+D to DuplicateLine
- [ ] Bind Cmd+Option+T to TrimTrailingWhitespace

**Test:** Keyboard shortcuts trigger correct operations.

### Phase 6: Command Palette

**Files:** `src/keymap/command.rs` (Command::display_name)

- [ ] Add display names for new commands
- [ ] Ensure commands appear in palette
- [ ] Add "Trim All Trailing Whitespace" command

**Test:** Commands accessible via Cmd+Shift+A.

---

## Testing Strategy

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_join_lines_basic() {
        let mut doc = Document::from_string("Hello\nWorld");
        doc.join_line(0);
        assert_eq!(doc.to_string(), "Hello World");
    }

    #[test]
    fn test_join_lines_with_leading_whitespace() {
        let mut doc = Document::from_string("Hello\n    World");
        doc.join_line(0);
        assert_eq!(doc.to_string(), "Hello World");
    }

    #[test]
    fn test_join_lines_empty_first() {
        let mut doc = Document::from_string("\nWorld");
        doc.join_line(0);
        assert_eq!(doc.to_string(), "World");
    }

    #[test]
    fn test_join_lines_empty_second() {
        let mut doc = Document::from_string("Hello\n");
        doc.join_line(0);
        assert_eq!(doc.to_string(), "Hello");
    }

    #[test]
    fn test_duplicate_line() {
        let mut doc = Document::from_string("Hello\nWorld");
        doc.duplicate_line(0);
        assert_eq!(doc.to_string(), "Hello\nHello\nWorld");
    }

    #[test]
    fn test_duplicate_range() {
        let mut doc = Document::from_string("Hello World");
        doc.duplicate_range(0, 5);
        assert_eq!(doc.to_string(), "HelloHello World");
    }

    #[test]
    fn test_trim_trailing_whitespace() {
        let mut doc = Document::from_string("Hello   \nWorld  \n");
        let count = doc.trim_all_trailing_whitespace();
        assert_eq!(doc.to_string(), "Hello\nWorld\n");
        assert_eq!(count, 2);
    }

    #[test]
    fn test_trim_no_whitespace() {
        let mut doc = Document::from_string("Hello\nWorld\n");
        let count = doc.trim_all_trailing_whitespace();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_join_lines_multicursor() {
        let mut doc = Document::from_string("A\nB\nC\nD");
        let mut editor = EditorState::new();
        editor.add_cursor_at(0, 0); // Line A
        editor.add_cursor_at(2, 0); // Line C

        editor.join_lines(&mut doc);

        // A joins with B, C joins with D
        assert_eq!(doc.to_string(), "A B\nC D");
    }
}
```

### Integration Tests

```rust
// tests/line_ops_tests.rs

#[test]
fn test_join_lines_undo() {
    // Join lines
    // Undo
    // Verify original restored
}

#[test]
fn test_duplicate_line_with_cursor() {
    // Place cursor in middle of line
    // Duplicate line
    // Verify cursor on new line at same column
}

#[test]
fn test_duplicate_selection_inline() {
    // Select word "hello"
    // Duplicate
    // Verify "hellohello"
}

#[test]
fn test_trim_whitespace_preserves_cursor() {
    // Cursor at end of line with trailing whitespace
    // Trim
    // Verify cursor moved to new line end
}
```

---

## References

- **Existing code:** `src/update/document.rs` - Document editing
- **Multi-cursor:** `src/model/editor.rs` - Cursor management
- **Commands:** `src/keymap/command.rs` - Existing line operations
- **VS Code:** Ctrl+J join lines behavior
- **Sublime Text:** Ctrl+Shift+D duplicate line
- **IntelliJ:** Ctrl+D duplicate line, Ctrl+Shift+J join lines
