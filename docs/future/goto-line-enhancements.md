# Go to Line Enhancements

Column support and enhanced position navigation with `:line:column` format.

> **Status:** Planned
> **Priority:** P2
> **Effort:** S
> **Created:** 2025-12-19
> **Milestone:** 1 - Navigation

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

The Go to Line dialog exists in `src/model/ui.rs` as `GotoLineState`:

```rust
pub struct GotoLineState {
    pub editable: EditableState<StringBuffer>,
}
```

It uses `EditConstraints::goto_line()` which allows numeric input. Current behavior:
- Accepts line number only (e.g., "123")
- Jumps to start of specified line
- No column support
- No preview while typing

### Goals

1. **Column support** - Accept `:line:column` format (e.g., ":123:45")
2. **Colon prefix optional** - Accept "123", ":123", or ":123:45"
3. **Live preview** - Show target line in editor while typing
4. **Bounds validation** - Visual feedback when line/column out of range
5. **Percentage jumps** - Support "50%" to jump to middle of file

### Non-Goals

- Go to symbol/definition (LSP feature)
- Go to character offset (niche use case)
- Named bookmarks (separate feature)

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────┐
│                      Go to Line Modal Flow                               │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│  ┌──────────────────────────────────────────────────────────────────┐   │
│  │  Input: [ :123:45                                           ]    │   │
│  ├──────────────────────────────────────────────────────────────────┤   │
│  │                         Parse Input                               │   │
│  │  ┌────────────────────────────────────────────────────────────┐  │   │
│  │  │  ":123:45" → GotoTarget::LineColumn { line: 123, col: 45 } │  │   │
│  │  │  "123"     → GotoTarget::Line(123)                         │  │   │
│  │  │  "50%"     → GotoTarget::Percentage(50)                    │  │   │
│  │  └────────────────────────────────────────────────────────────┘  │   │
│  └──────────────────────────────────────────────────────────────────┘   │
│                                │                                         │
│                                ▼                                         │
│  ┌──────────────────────────────────────────────────────────────────┐   │
│  │                      Validate Bounds                              │   │
│  │                                                                   │   │
│  │  Line 123 exists?  ────▶  Column 45 exists on line?              │   │
│  │       │                          │                                │   │
│  │       ▼                          ▼                                │   │
│  │  ┌─────────┐              ┌─────────────┐                        │   │
│  │  │ Valid   │              │ Valid       │  ───▶ Preview jump     │   │
│  │  └─────────┘              └─────────────┘       (temporary)      │   │
│  │       │                          │                                │   │
│  │  ┌─────────┐              ┌─────────────┐                        │   │
│  │  │ Invalid │              │ Out of range│  ───▶ Clamp + warning  │   │
│  │  └─────────┘              └─────────────┘                        │   │
│  └──────────────────────────────────────────────────────────────────┘   │
│                                │                                         │
│                                ▼                                         │
│  ┌──────────────────────────────────────────────────────────────────┐   │
│  │  [Enter] Confirm jump   [Esc] Cancel (restore original position) │   │
│  └──────────────────────────────────────────────────────────────────┘   │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

### Message Flow

```
User Action              Msg                              Effect
─────────────────────────────────────────────────────────────────────────
Open dialog          → ModalMsg::OpenGotoLine         → Store original pos
Type input           → ModalMsg::SetInput(":123:45") → Parse, validate, preview
Confirm              → ModalMsg::Confirm              → Jump to position
Cancel               → ModalMsg::Close                → Restore original pos
```

---

## Data Structures

### Go to Target

```rust
// src/model/ui.rs

/// Parsed goto target from user input
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GotoTarget {
    /// Jump to line only (column 0)
    Line(usize),
    /// Jump to specific line and column
    LineColumn { line: usize, column: usize },
    /// Jump to percentage through file (0-100)
    Percentage(u8),
    /// Invalid input
    Invalid,
}

impl GotoTarget {
    /// Parse input string into a GotoTarget
    ///
    /// Supported formats:
    /// - "123"       → Line 123 (1-indexed input, 0-indexed result)
    /// - ":123"      → Line 123
    /// - ":123:45"   → Line 123, Column 45
    /// - "123:45"    → Line 123, Column 45
    /// - "50%"       → 50% through file
    pub fn parse(input: &str) -> Self {
        let input = input.trim();

        if input.is_empty() {
            return GotoTarget::Invalid;
        }

        // Handle percentage
        if input.ends_with('%') {
            let num_str = input.trim_end_matches('%');
            if let Ok(pct) = num_str.parse::<u8>() {
                if pct <= 100 {
                    return GotoTarget::Percentage(pct);
                }
            }
            return GotoTarget::Invalid;
        }

        // Remove leading colon if present
        let input = input.trim_start_matches(':');

        // Split on colon for line:column
        let parts: Vec<&str> = input.split(':').collect();

        match parts.as_slice() {
            [line_str] => {
                if let Ok(line) = line_str.parse::<usize>() {
                    if line > 0 {
                        // Convert 1-indexed input to 0-indexed
                        return GotoTarget::Line(line - 1);
                    }
                }
                GotoTarget::Invalid
            }
            [line_str, col_str] => {
                if let (Ok(line), Ok(col)) = (line_str.parse::<usize>(), col_str.parse::<usize>()) {
                    if line > 0 {
                        // Convert 1-indexed to 0-indexed
                        return GotoTarget::LineColumn {
                            line: line - 1,
                            column: col.saturating_sub(1), // Column can be 0 or 1-indexed
                        };
                    }
                }
                GotoTarget::Invalid
            }
            _ => GotoTarget::Invalid,
        }
    }

    /// Resolve to actual (line, column) given document info
    pub fn resolve(self, line_count: usize, line_length_fn: impl Fn(usize) -> usize) -> Option<(usize, usize)> {
        match self {
            GotoTarget::Line(line) => {
                let clamped_line = line.min(line_count.saturating_sub(1));
                Some((clamped_line, 0))
            }
            GotoTarget::LineColumn { line, column } => {
                let clamped_line = line.min(line_count.saturating_sub(1));
                let max_col = line_length_fn(clamped_line);
                let clamped_col = column.min(max_col);
                Some((clamped_line, clamped_col))
            }
            GotoTarget::Percentage(pct) => {
                if line_count == 0 {
                    return Some((0, 0));
                }
                let target_line = (line_count as f64 * pct as f64 / 100.0).floor() as usize;
                let clamped_line = target_line.min(line_count.saturating_sub(1));
                Some((clamped_line, 0))
            }
            GotoTarget::Invalid => None,
        }
    }
}
```

### Enhanced GotoLineState

```rust
// Updated in src/model/ui.rs

/// State for the goto line modal
#[derive(Debug, Clone)]
pub struct GotoLineState {
    /// Editable state for the input field
    pub editable: EditableState<StringBuffer>,
    /// Original cursor position (for cancel restore)
    pub original_position: Position,
    /// Original viewport top line (for cancel restore)
    pub original_viewport_top: usize,
    /// Parsed target from current input
    pub parsed_target: GotoTarget,
    /// Validation status
    pub validation: GotoValidation,
}

/// Validation result for goto input
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GotoValidation {
    /// Input is valid and in bounds
    Valid,
    /// Line number is out of bounds (clamped)
    LineOutOfBounds { requested: usize, max: usize },
    /// Column is out of bounds (clamped)
    ColumnOutOfBounds { requested: usize, max: usize },
    /// Input format is invalid
    InvalidFormat,
    /// Input is empty (show placeholder)
    Empty,
}

impl GotoValidation {
    /// Get a message for the validation status
    pub fn message(&self) -> Option<&'static str> {
        match self {
            GotoValidation::Valid => None,
            GotoValidation::LineOutOfBounds { .. } => Some("Line out of range (clamped)"),
            GotoValidation::ColumnOutOfBounds { .. } => Some("Column out of range (clamped)"),
            GotoValidation::InvalidFormat => Some("Invalid format: use line or line:column"),
            GotoValidation::Empty => None,
        }
    }

    /// Check if we should show a warning indicator
    pub fn is_warning(&self) -> bool {
        matches!(
            self,
            GotoValidation::LineOutOfBounds { .. }
                | GotoValidation::ColumnOutOfBounds { .. }
                | GotoValidation::InvalidFormat
        )
    }
}

impl GotoLineState {
    /// Create new state, storing current position for restore
    pub fn new(current_position: Position, viewport_top: usize) -> Self {
        Self {
            editable: EditableState::new(StringBuffer::new(), EditConstraints::goto_line_enhanced()),
            original_position: current_position,
            original_viewport_top: viewport_top,
            parsed_target: GotoTarget::Invalid,
            validation: GotoValidation::Empty,
        }
    }

    /// Update parsed target from current input
    pub fn update_target(&mut self, line_count: usize, line_length_fn: impl Fn(usize) -> usize) {
        let input = self.editable.text();

        if input.is_empty() {
            self.parsed_target = GotoTarget::Invalid;
            self.validation = GotoValidation::Empty;
            return;
        }

        self.parsed_target = GotoTarget::parse(&input);

        // Validate bounds
        self.validation = match self.parsed_target {
            GotoTarget::Invalid => GotoValidation::InvalidFormat,
            GotoTarget::Line(line) => {
                if line >= line_count {
                    GotoValidation::LineOutOfBounds {
                        requested: line + 1, // 1-indexed for display
                        max: line_count,
                    }
                } else {
                    GotoValidation::Valid
                }
            }
            GotoTarget::LineColumn { line, column } => {
                if line >= line_count {
                    GotoValidation::LineOutOfBounds {
                        requested: line + 1,
                        max: line_count,
                    }
                } else {
                    let max_col = line_length_fn(line);
                    if column > max_col {
                        GotoValidation::ColumnOutOfBounds {
                            requested: column + 1,
                            max: max_col + 1,
                        }
                    } else {
                        GotoValidation::Valid
                    }
                }
            }
            GotoTarget::Percentage(_) => GotoValidation::Valid,
        };
    }

    /// Get the preview position (clamped to valid range)
    pub fn preview_position(
        &self,
        line_count: usize,
        line_length_fn: impl Fn(usize) -> usize,
    ) -> Option<(usize, usize)> {
        self.parsed_target.resolve(line_count, line_length_fn)
    }
}
```

### EditConstraints Extension

```rust
// Add to src/editable/mod.rs or appropriate location

impl EditConstraints {
    /// Constraints for enhanced goto line input
    ///
    /// Allows: digits, colon, percent sign
    pub fn goto_line_enhanced() -> Self {
        Self {
            max_length: Some(20),
            single_line: true,
            allowed_chars: Some(Box::new(|c| c.is_ascii_digit() || c == ':' || c == '%')),
            numeric_only: false, // We handle validation ourselves
        }
    }
}
```

---

## Keybindings

| Action | Mac | Windows/Linux | Notes |
|--------|-----|---------------|-------|
| Open Go to Line | Cmd+L | Ctrl+G | Main shortcut |
| Confirm jump | Enter | Enter | Jump to position |
| Cancel | Escape | Escape | Restore original position |

---

## Implementation Plan

### Phase 1: Input Parsing

**Files:** `src/model/ui.rs`

- [ ] Add `GotoTarget` enum with parse method
- [ ] Support line-only format ("123", ":123")
- [ ] Support line:column format (":123:45", "123:45")
- [ ] Support percentage format ("50%")
- [ ] Add comprehensive parsing tests

**Test:** `GotoTarget::parse(":123:45")` returns `LineColumn { line: 122, column: 44 }`.

### Phase 2: Enhanced State

**Files:** `src/model/ui.rs`

- [ ] Add `GotoValidation` enum
- [ ] Extend `GotoLineState` with original position storage
- [ ] Add `update_target()` method for parsing + validation
- [ ] Add `preview_position()` for live preview

**Test:** Opening modal stores current position for cancel restore.

### Phase 3: Edit Constraints

**Files:** `src/editable/mod.rs`

- [ ] Add `EditConstraints::goto_line_enhanced()`
- [ ] Allow digits, colons, and percent sign
- [ ] Update `GotoLineState` to use new constraints

**Test:** Input "abc" is rejected, ":123:45" is accepted.

### Phase 4: Preview Integration

**Files:** `src/update/modal.rs`

- [ ] On input change, parse and validate target
- [ ] Temporarily move viewport to preview position
- [ ] On confirm, finalize the jump
- [ ] On cancel, restore original position and viewport

**Test:** Typing ":100" scrolls to line 100 preview.

### Phase 5: Rendering

**Files:** `src/view/modal.rs`

- [ ] Show placeholder text ":line or :line:column"
- [ ] Show validation message below input
- [ ] Highlight input in red for invalid format
- [ ] Highlight in yellow for out-of-bounds (clamped)
- [ ] Show resolved position (e.g., "Line 123, Column 45")

**Test:** Invalid input shows error message.

### Phase 6: Polish

- [ ] Add status bar hint while modal is open
- [ ] Show document line count next to input
- [ ] Support Ctrl+G as alternative shortcut (common in Windows)
- [ ] Remember last goto value in session

---

## Testing Strategy

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_line_only() {
        assert_eq!(GotoTarget::parse("123"), GotoTarget::Line(122));
        assert_eq!(GotoTarget::parse(":123"), GotoTarget::Line(122));
    }

    #[test]
    fn test_parse_line_column() {
        assert_eq!(
            GotoTarget::parse(":123:45"),
            GotoTarget::LineColumn { line: 122, column: 44 }
        );
        assert_eq!(
            GotoTarget::parse("123:45"),
            GotoTarget::LineColumn { line: 122, column: 44 }
        );
    }

    #[test]
    fn test_parse_percentage() {
        assert_eq!(GotoTarget::parse("50%"), GotoTarget::Percentage(50));
        assert_eq!(GotoTarget::parse("0%"), GotoTarget::Percentage(0));
        assert_eq!(GotoTarget::parse("100%"), GotoTarget::Percentage(100));
    }

    #[test]
    fn test_parse_invalid() {
        assert_eq!(GotoTarget::parse(""), GotoTarget::Invalid);
        assert_eq!(GotoTarget::parse("abc"), GotoTarget::Invalid);
        assert_eq!(GotoTarget::parse("0"), GotoTarget::Invalid); // Line 0 invalid
        assert_eq!(GotoTarget::parse("150%"), GotoTarget::Invalid);
    }

    #[test]
    fn test_resolve_clamping() {
        let line_count = 100;
        let line_length = |_| 80;

        // Line out of bounds
        let target = GotoTarget::Line(200);
        assert_eq!(target.resolve(line_count, line_length), Some((99, 0)));

        // Column out of bounds
        let target = GotoTarget::LineColumn { line: 50, column: 200 };
        assert_eq!(target.resolve(line_count, line_length), Some((50, 80)));
    }

    #[test]
    fn test_percentage_resolution() {
        let line_count = 100;
        let line_length = |_| 80;

        assert_eq!(
            GotoTarget::Percentage(0).resolve(line_count, line_length),
            Some((0, 0))
        );
        assert_eq!(
            GotoTarget::Percentage(50).resolve(line_count, line_length),
            Some((50, 0))
        );
        assert_eq!(
            GotoTarget::Percentage(100).resolve(line_count, line_length),
            Some((99, 0)) // Clamped to last line
        );
    }

    #[test]
    fn test_validation_messages() {
        assert!(GotoValidation::Valid.message().is_none());
        assert!(GotoValidation::LineOutOfBounds { requested: 200, max: 100 }
            .message()
            .is_some());
    }
}
```

### Integration Tests

```rust
// tests/goto_line_tests.rs

#[test]
fn test_goto_line_preview() {
    // Open goto line modal
    // Type line number
    // Verify viewport moved to preview position
    // Cancel
    // Verify viewport restored
}

#[test]
fn test_goto_line_confirm() {
    // Open goto line modal
    // Type ":50:10"
    // Press Enter
    // Verify cursor at line 50, column 10
}

#[test]
fn test_goto_line_percentage() {
    // Open 100-line document
    // Goto "50%"
    // Verify cursor at line 50
}
```

---

## References

- **Existing code:** `src/model/ui.rs` - `GotoLineState`
- **Editable system:** `src/editable/` - Input constraints
- **VS Code:** "Go to Line" (Ctrl+G) with `:line:column` support
- **Sublime Text:** Goto Anything with `:line` syntax
- **Modal patterns:** `src/update/modal.rs` - Modal message handling
