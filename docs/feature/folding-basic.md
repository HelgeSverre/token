# Code Folding - Basic (Indentation-Based)

Collapse and expand code regions based on indentation levels

> **Status:** 📋 Planned
> **Priority:** P2 (Important)
> **Effort:** M (3-5 days)
> **Created:** 2025-12-20
> **Milestone:** 4 - Hard Problems
> **Feature ID:** F-150a
> **Next Phase:** [folding-advanced.md](folding-advanced.md)

---

## Table of Contents

1. [Overview](#overview)
2. [Architecture](#architecture)
3. [Data Structures](#data-structures)
4. [Fold Detection](#fold-detection)
5. [Rendering](#rendering)
6. [Keybindings](#keybindings)
7. [Implementation Plan](#implementation-plan)
8. [Testing Strategy](#testing-strategy)

---

## Overview

### Current State

The editor has:
- Line number gutter rendering
- Syntax highlighting (tree-sitter)
- Viewport scrolling with line tracking
- Multi-cursor support

No code folding exists—users see all lines at all times.

### Goals (This Phase)

1. **Indentation-based folding** - Detect foldable regions from indentation changes
2. **Fold/unfold operations** - Toggle individual regions
3. **Gutter indicators** - Show fold controls in gutter (▶/▼)
4. **Viewport adjustment** - Hidden lines don't render
5. **Cursor handling** - Cursor in folded region expands it

### Non-Goals (This Phase → See folding-advanced.md)

- Syntax-based detection (functions, classes)
- Region markers (`#region`/`#endregion`)
- Fold persistence across sessions
- Fold level commands (fold to level N)
- Manual/custom folds

### Why Indentation First?

1. **Works for all languages** - No language-specific logic needed
2. **Simple algorithm** - Just track indent level changes
3. **Low risk** - Doesn't depend on syntax parsing correctness
4. **Quick win** - Delivers 80% of folding value with 20% of complexity

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                       Indentation-Based Folding                              │
│                                                                              │
│  Document:                                                                   │
│  ┌──────────────────────────────────────────────────────────────────────┐   │
│  │  0: fn main() {           ◄─ indent=0, starts fold (ends line 5)    │   │
│  │  1:     let x = 1;        ◄─ indent=4                               │   │
│  │  2:     if true {         ◄─ indent=4, starts fold (ends line 4)    │   │
│  │  3:         do_thing();   ◄─ indent=8                               │   │
│  │  4:     }                 ◄─ indent=4, ends fold                    │   │
│  │  5: }                     ◄─ indent=0, ends fold                    │   │
│  └──────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
│  FoldingState.regions:                                                       │
│  ┌──────────────────────────────────────────────────────────────────────┐   │
│  │  [FoldRegion { start: 0, end: 5, level: 0, collapsed: false }]      │   │
│  │  [FoldRegion { start: 2, end: 4, level: 1, collapsed: false }]      │   │
│  └──────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
│  Rendering (if line 0-5 collapsed):                                          │
│  ┌──────────────────────────────────────────────────────────────────────┐   │
│  │  ▶ 1 │ fn main() { ⋯ 5 lines                                        │   │
│  │    7 │ // next code                                                  │   │
│  └──────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Module Structure

```
src/
├── folding/                     # NEW MODULE
│   ├── mod.rs                   # Public exports
│   ├── region.rs                # FoldRegion, FoldingState
│   ├── indent.rs                # Indentation-based detection
│   └── gutter.rs                # Fold indicator rendering
├── model/
│   └── editor.rs                # + folding_state: FoldingState
├── update/
│   └── folding.rs               # NEW: Fold message handler
└── view/
    └── mod.rs                   # + render fold indicators, skip hidden lines
```

---

## Data Structures

### FoldRegion

```rust
// src/folding/region.rs

/// A single foldable region
#[derive(Debug, Clone)]
pub struct FoldRegion {
    /// Starting line (inclusive, the line with fold indicator)
    pub start_line: usize,

    /// Ending line (inclusive)
    pub end_line: usize,

    /// Nesting level (0 = outermost)
    pub level: usize,

    /// Current state
    pub collapsed: bool,
}

impl FoldRegion {
    /// Number of lines hidden when collapsed
    pub fn hidden_line_count(&self) -> usize {
        if self.collapsed {
            self.end_line - self.start_line
        } else {
            0
        }
    }

    /// Check if a line is inside this region (not the start line)
    pub fn contains_line(&self, line: usize) -> bool {
        line > self.start_line && line <= self.end_line
    }
}
```

### FoldingState

```rust
// src/folding/region.rs

/// Folding state for a document
#[derive(Debug, Clone, Default)]
pub struct FoldingState {
    /// All fold regions (sorted by start_line)
    pub regions: Vec<FoldRegion>,

    /// Document revision when computed
    pub revision: u64,

    /// Whether state needs recomputation
    pub dirty: bool,
}

impl FoldingState {
    /// Check if a line is hidden (inside a collapsed fold)
    pub fn is_hidden(&self, line: usize) -> bool {
        self.regions.iter().any(|r| r.collapsed && r.contains_line(line))
    }

    /// Get fold region starting at a line (if any)
    pub fn region_at(&self, line: usize) -> Option<&FoldRegion> {
        self.regions.iter().find(|r| r.start_line == line)
    }

    /// Get mutable fold region starting at a line
    pub fn region_at_mut(&mut self, line: usize) -> Option<&mut FoldRegion> {
        self.regions.iter_mut().find(|r| r.start_line == line)
    }

    /// Toggle fold at line (if a fold starts there)
    pub fn toggle(&mut self, line: usize) {
        if let Some(region) = self.region_at_mut(line) {
            region.collapsed = !region.collapsed;
        }
    }

    /// Expand any fold containing this line
    pub fn expand_containing(&mut self, line: usize) {
        for region in &mut self.regions {
            if region.collapsed && region.contains_line(line) {
                region.collapsed = false;
            }
        }
    }

    /// Collapse all folds
    pub fn collapse_all(&mut self) {
        for region in &mut self.regions {
            region.collapsed = true;
        }
    }

    /// Expand all folds
    pub fn expand_all(&mut self) {
        for region in &mut self.regions {
            region.collapsed = false;
        }
    }

    /// Map document line to visual line (accounting for collapsed folds)
    pub fn doc_to_visual(&self, doc_line: usize) -> usize {
        let mut visual = 0;
        for line in 0..doc_line {
            if !self.is_hidden(line) {
                visual += 1;
            }
        }
        visual
    }

    /// Map visual line to document line
    pub fn visual_to_doc(&self, visual_line: usize) -> usize {
        let mut visual = 0;
        let mut doc = 0;

        while visual < visual_line {
            if !self.is_hidden(doc) {
                visual += 1;
            }
            doc += 1;
        }

        // Skip any hidden lines at target
        while self.is_hidden(doc) {
            doc += 1;
        }

        doc
    }

    /// Total visible line count
    pub fn visible_line_count(&self, total_lines: usize) -> usize {
        (0..total_lines).filter(|&l| !self.is_hidden(l)).count()
    }
}
```

---

## Fold Detection

### Indentation-Based Algorithm

```rust
// src/folding/indent.rs

use crate::model::Document;
use super::region::FoldRegion;

/// Minimum indent change to create a fold (usually 1 level = tab_width spaces)
const MIN_FOLD_LINES: usize = 1;

/// Detect fold regions based on indentation
pub fn detect_indent_folds(document: &Document, tab_width: usize) -> Vec<FoldRegion> {
    let mut regions = Vec::new();
    let mut stack: Vec<(usize, usize)> = vec![]; // (start_line, indent_level)
    let line_count = document.line_count();

    for line_idx in 0..line_count {
        let line_text = document.get_line(line_idx).unwrap_or_default();
        let trimmed = line_text.trim_end();

        // Skip empty lines for indent calculation
        if trimmed.is_empty() {
            continue;
        }

        let indent = measure_indent(trimmed, tab_width);

        // Close any regions with higher indent
        while let Some(&(start, level)) = stack.last() {
            if indent <= level {
                // Region ends at previous non-empty line
                let end_line = find_last_content_line(document, start, line_idx - 1);
                if end_line > start + MIN_FOLD_LINES {
                    regions.push(FoldRegion {
                        start_line: start,
                        end_line,
                        level: stack.len() - 1,
                        collapsed: false,
                    });
                }
                stack.pop();
            } else {
                break;
            }
        }

        // Check if this line starts a new fold (ends with fold-starting char)
        if should_start_fold(trimmed) {
            stack.push((line_idx, indent));
        }
    }

    // Close remaining open regions
    for (start, _) in stack.into_iter().rev() {
        let end_line = find_last_content_line(document, start, line_count - 1);
        if end_line > start + MIN_FOLD_LINES {
            regions.push(FoldRegion {
                start_line: start,
                end_line,
                level: 0,
                collapsed: false,
            });
        }
    }

    // Sort by start line for efficient lookup
    regions.sort_by_key(|r| r.start_line);
    regions
}

/// Measure indentation level (handles tabs and spaces)
fn measure_indent(line: &str, tab_width: usize) -> usize {
    let mut indent = 0;
    for ch in line.chars() {
        match ch {
            ' ' => indent += 1,
            '\t' => indent += tab_width - (indent % tab_width),
            _ => break,
        }
    }
    indent
}

/// Check if line should start a fold region
fn should_start_fold(line: &str) -> bool {
    // Lines ending with these characters typically start blocks
    let fold_starters = ['{', ':', '[', '('];

    line.chars()
        .rev()
        .find(|c| !c.is_whitespace())
        .map(|c| fold_starters.contains(&c))
        .unwrap_or(false)
}

/// Find last line with content between start and end
fn find_last_content_line(document: &Document, start: usize, end: usize) -> usize {
    for line in (start..=end).rev() {
        if let Some(text) = document.get_line(line) {
            if !text.trim().is_empty() {
                return line;
            }
        }
    }
    end
}
```

---

## Rendering

### Fold Indicators in Gutter

```rust
// src/folding/gutter.rs

use crate::view::Frame;
use super::region::FoldingState;

/// Render fold indicators in the gutter
pub fn render_fold_indicators(
    frame: &mut Frame,
    folding: &FoldingState,
    visible_start: usize,
    visible_count: usize,
    gutter_x: usize,
    gutter_width: usize,
    line_height: usize,
    y_offset: usize,
    theme: &FoldIndicatorTheme,
    painter: &mut TextPainter,
) {
    let indicator_x = gutter_x + gutter_width - 12; // Right side of gutter

    for visual_line in 0..visible_count {
        let doc_line = folding.visual_to_doc(visible_start + visual_line);
        let y = y_offset + visual_line * line_height;

        if let Some(region) = folding.region_at(doc_line) {
            let icon = if region.collapsed { "▶" } else { "▼" };
            painter.draw_text(
                frame,
                indicator_x,
                y,
                icon,
                theme.indicator_color,
                None,
            );
        }
    }
}

/// Render collapsed fold placeholder
pub fn render_fold_placeholder(
    frame: &mut Frame,
    region: &FoldRegion,
    x: usize,
    y: usize,
    theme: &FoldIndicatorTheme,
    painter: &mut TextPainter,
) {
    let hidden_count = region.end_line - region.start_line;
    let placeholder = format!(" ⋯ {} lines", hidden_count);

    painter.draw_text(
        frame,
        x,
        y,
        &placeholder,
        theme.placeholder_color,
        Some(theme.placeholder_bg),
    );
}

#[derive(Debug, Clone)]
pub struct FoldIndicatorTheme {
    pub indicator_color: u32,
    pub placeholder_color: u32,
    pub placeholder_bg: u32,
}
```

### Skip Hidden Lines in Render

```rust
// In view/mod.rs - modify render_document()

fn render_document(/* ... */) {
    // ...existing setup...

    let folding = &editor_state.folding_state;
    let mut visual_line = 0;

    for doc_line in viewport.top_line..viewport.bottom_line() {
        // Skip hidden lines
        if folding.is_hidden(doc_line) {
            continue;
        }

        let y = y_offset + visual_line * line_height;

        // Render line content
        render_line(frame, document, doc_line, x, y, /* ... */);

        // If this line starts a collapsed fold, add placeholder
        if let Some(region) = folding.region_at(doc_line) {
            if region.collapsed {
                let line_end_x = /* calculate end of line text */;
                render_fold_placeholder(frame, region, line_end_x, y, theme, painter);
            }
        }

        visual_line += 1;
    }
}
```

---

## Keybindings

| Action | Mac | Windows/Linux | Command |
|--------|-----|---------------|---------|
| Toggle fold at cursor | `Cmd+Option+[` | `Ctrl+Shift+[` | `ToggleFold` |
| Fold at cursor | `Cmd+Option+[` | `Ctrl+Shift+[` | `Fold` |
| Unfold at cursor | `Cmd+Option+]` | `Ctrl+Shift+]` | `Unfold` |
| Fold all | `Cmd+K Cmd+0` | `Ctrl+K Ctrl+0` | `FoldAll` |
| Unfold all | `Cmd+K Cmd+J` | `Ctrl+K Ctrl+J` | `UnfoldAll` |

```yaml
# keymap.yaml additions
- key: "cmd+option+["
  command: ToggleFold

- key: "cmd+option+]"
  command: Unfold

- key: "cmd+k cmd+0"
  command: FoldAll

- key: "cmd+k cmd+j"
  command: UnfoldAll
```

---

## Messages

```rust
// src/messages.rs additions

#[derive(Debug, Clone)]
pub enum FoldMsg {
    /// Toggle fold at current cursor line
    Toggle,

    /// Fold region at cursor (if any)
    Fold,

    /// Unfold region at cursor (if any)
    Unfold,

    /// Fold all regions
    FoldAll,

    /// Unfold all regions
    UnfoldAll,

    /// Recompute fold regions (after document change)
    Recompute,
}
```

---

## Implementation Plan

### Phase 1: Core Data Structures

**Effort:** S (1 day)

- [ ] Create `src/folding/mod.rs` module structure
- [ ] Implement `FoldRegion` and `FoldingState`
- [ ] Add `folding_state: FoldingState` to `EditorState`
- [ ] Add `FoldIndicatorTheme` to theme system

**Test:** Create `FoldingState`, add regions, verify `is_hidden()`.

### Phase 2: Fold Detection

**Effort:** S (1-2 days)

- [ ] Implement `measure_indent()` function
- [ ] Implement `detect_indent_folds()` algorithm
- [ ] Trigger detection on document load
- [ ] Trigger re-detection on document change (debounced)

**Test:** Parse Python/YAML file, verify correct regions detected.

### Phase 3: Toggle Operations

**Effort:** S (1 day)

- [ ] Add `FoldMsg` to messages.rs
- [ ] Implement `update_fold()` handler
- [ ] Wire up `toggle()`, `fold()`, `unfold()`
- [ ] Wire up `fold_all()`, `unfold_all()`
- [ ] Add keybindings

**Test:** Toggle fold via keyboard, verify state changes.

### Phase 4: Rendering

**Effort:** M (2-3 days)

- [ ] Implement `render_fold_indicators()` in gutter
- [ ] Modify `render_document()` to skip hidden lines
- [ ] Implement `render_fold_placeholder()`
- [ ] Update viewport calculations for visual lines
- [ ] Handle scrolling with collapsed folds

**Test:** Collapse fold, verify hidden lines don't render, placeholder shown.

### Phase 5: Cursor Integration

**Effort:** S (1 day)

- [ ] When cursor moves into hidden line, expand containing fold
- [ ] When clicking gutter indicator, toggle fold
- [ ] Update cursor position after fold/unfold

**Test:** Arrow down into collapsed region, verify it expands.

---

## Testing Strategy

### Unit Tests

```rust
#[test]
fn test_measure_indent() {
    assert_eq!(measure_indent("hello", 4), 0);
    assert_eq!(measure_indent("    hello", 4), 4);
    assert_eq!(measure_indent("\thello", 4), 4);
    assert_eq!(measure_indent("  \thello", 4), 4); // 2 spaces + tab to 4
}

#[test]
fn test_detect_folds_python() {
    let doc = Document::with_text(
        "def foo():\n    x = 1\n    y = 2\n\nz = 3"
    );
    let regions = detect_indent_folds(&doc, 4);

    assert_eq!(regions.len(), 1);
    assert_eq!(regions[0].start_line, 0);
    assert_eq!(regions[0].end_line, 2);
}

#[test]
fn test_is_hidden() {
    let mut state = FoldingState::default();
    state.regions.push(FoldRegion {
        start_line: 5,
        end_line: 10,
        level: 0,
        collapsed: true,
    });

    assert!(!state.is_hidden(5));  // Start line visible
    assert!(state.is_hidden(6));   // Inside = hidden
    assert!(state.is_hidden(10));  // End line hidden
    assert!(!state.is_hidden(11)); // After = visible
}

#[test]
fn test_visual_line_mapping() {
    let mut state = FoldingState::default();
    state.regions.push(FoldRegion {
        start_line: 2,
        end_line: 5,
        level: 0,
        collapsed: true,
    });

    // Lines: 0, 1, 2, [3,4,5 hidden], 6, 7
    // Visual: 0, 1, 2, 3, 4
    assert_eq!(state.doc_to_visual(0), 0);
    assert_eq!(state.doc_to_visual(2), 2);
    assert_eq!(state.doc_to_visual(6), 3);
    assert_eq!(state.doc_to_visual(7), 4);

    assert_eq!(state.visual_to_doc(0), 0);
    assert_eq!(state.visual_to_doc(2), 2);
    assert_eq!(state.visual_to_doc(3), 6);
}
```

### Manual Testing

- [ ] Fold indicator appears for indented blocks
- [ ] Click indicator toggles fold
- [ ] Keyboard toggle works
- [ ] Fold all/unfold all work
- [ ] Scrolling works with collapsed folds
- [ ] Cursor navigation expands folds
- [ ] Line numbers remain correct
- [ ] Works with Python, YAML, Rust, JavaScript

---

## References

- [VS Code Folding](https://code.visualstudio.com/docs/editor/codebasics#_folding)
- [folding-advanced.md](folding-advanced.md) - Next phase: syntax-based folding
