# Code Folding (F-150)

Collapse and expand regions of code for better navigation and focus.

> **Status:** 📋 Planned
> **Priority:** P3 (Nice-to-have)
> **Effort:** XL (2+ weeks)
> **Created:** 2025-12-19
> **Updated:** 2025-12-19
> **Milestone:** 4 - Hard Problems

---

## Overview

Code folding allows users to collapse code blocks (functions, classes, comments, etc.) to hide their contents and show only a summary. This improves navigation in large files and helps focus on relevant code sections.

---

## Features

### Core Capabilities

| Feature | Description |
|---------|-------------|
| Fold/Unfold | Collapse/expand individual regions |
| Fold All | Collapse all foldable regions |
| Unfold All | Expand all folded regions |
| Fold Level N | Fold to specific nesting depth |
| Fold Selection | Create custom fold from selection |
| Persistent Folds | Remember fold state across sessions |

### Fold Sources

| Source | Description | Priority |
|--------|-------------|----------|
| Syntax-based | Language constructs (functions, classes) | P1 |
| Indentation-based | Indent levels (Python, YAML) | P1 |
| Region markers | `// #region` / `// #endregion` | P2 |
| Manual | User-created folds | P3 |

---

## Data Structures

### FoldRegion

```rust
/// A single foldable region in the document
#[derive(Debug, Clone)]
pub struct FoldRegion {
    /// Starting line (inclusive)
    pub start_line: usize,
    /// Ending line (inclusive)
    pub end_line: usize,
    /// Current fold state
    pub state: FoldState,
    /// Source of fold detection
    pub source: FoldSource,
    /// Summary text to show when folded
    pub summary: Option<String>,
    /// Nesting level (0 = top level)
    pub level: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FoldState {
    Expanded,
    Collapsed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FoldSource {
    /// From syntax tree (functions, classes, etc.)
    Syntax,
    /// From indentation changes
    Indentation,
    /// From region markers
    RegionMarker,
    /// User-created fold
    Manual,
}
```

### FoldingState

```rust
/// Folding state for a document
#[derive(Debug, Clone, Default)]
pub struct FoldingState {
    /// All detected fold regions
    pub regions: Vec<FoldRegion>,
    /// Collapsed regions (by start line)
    pub collapsed: HashSet<usize>,
    /// Document revision when folds were computed
    pub revision: u64,
}

impl FoldingState {
    /// Check if a line is inside a collapsed fold
    pub fn is_hidden(&self, line: usize) -> bool {
        self.regions.iter().any(|r| {
            r.state == FoldState::Collapsed
                && line > r.start_line
                && line <= r.end_line
        })
    }

    /// Get visible line count (total - hidden)
    pub fn visible_line_count(&self, total_lines: usize) -> usize {
        let hidden = self.regions.iter()
            .filter(|r| r.state == FoldState::Collapsed)
            .map(|r| r.end_line - r.start_line)
            .sum::<usize>();
        total_lines - hidden
    }

    /// Map visual line to document line
    pub fn visual_to_document_line(&self, visual: usize) -> usize {
        // Account for collapsed regions above this line
        let mut doc_line = 0;
        let mut vis_line = 0;

        while vis_line < visual {
            if !self.is_hidden(doc_line) {
                vis_line += 1;
            }
            doc_line += 1;
        }
        doc_line
    }
}
```

---

## Fold Detection

### Syntax-Based Detection

Uses tree-sitter syntax tree to find foldable constructs.

```rust
/// Detect fold regions from syntax tree
fn detect_syntax_folds(
    tree: &tree_sitter::Tree,
    source: &str,
    language: LanguageId,
) -> Vec<FoldRegion> {
    let mut regions = Vec::new();
    let mut cursor = tree.walk();

    // Language-specific node types that are foldable
    let foldable_types = match language {
        LanguageId::Rust => &[
            "function_item", "impl_item", "struct_item",
            "enum_item", "mod_item", "match_expression"
        ],
        LanguageId::JavaScript => &[
            "function_declaration", "class_declaration",
            "method_definition", "if_statement", "for_statement"
        ],
        // ... other languages
    };

    visit_tree(&mut cursor, |node| {
        if foldable_types.contains(&node.kind()) {
            let start = node.start_position().row;
            let end = node.end_position().row;

            // Only fold if multi-line
            if end > start {
                regions.push(FoldRegion {
                    start_line: start,
                    end_line: end,
                    state: FoldState::Expanded,
                    source: FoldSource::Syntax,
                    summary: extract_summary(node, source),
                    level: compute_nesting_level(node),
                });
            }
        }
    });

    regions
}
```

### Indentation-Based Detection

For languages like Python where indentation defines blocks.

```rust
fn detect_indentation_folds(lines: &[&str]) -> Vec<FoldRegion> {
    let mut regions = Vec::new();
    let mut stack: Vec<(usize, usize)> = vec![]; // (start_line, indent_level)

    for (line_num, line) in lines.iter().enumerate() {
        let indent = line.chars().take_while(|c| c.is_whitespace()).count();

        // Close any regions with higher indent
        while let Some(&(start, level)) = stack.last() {
            if indent <= level && line.trim().len() > 0 {
                regions.push(FoldRegion {
                    start_line: start,
                    end_line: line_num - 1,
                    // ...
                });
                stack.pop();
            } else {
                break;
            }
        }

        // Start new region if line ends with colon (Python) or similar
        if should_start_fold(line) {
            stack.push((line_num, indent));
        }
    }

    regions
}
```

### Region Marker Detection

```rust
fn detect_region_markers(lines: &[&str]) -> Vec<FoldRegion> {
    let region_patterns = [
        (r"//\s*#region\s*(.*)", r"//\s*#endregion"),   // C-style
        (r"<!--\s*region\s*(.*)", r"<!--\s*endregion"), // HTML
        (r"#\s*region\s*(.*)", r"#\s*endregion"),      // Python/Ruby
    ];

    // Match start/end markers and create regions
}
```

---

## Messages

```rust
pub enum EditorMsg {
    /// Toggle fold at cursor line
    ToggleFold,
    /// Fold the region at cursor
    Fold,
    /// Unfold the region at cursor
    Unfold,
    /// Fold all regions
    FoldAll,
    /// Unfold all regions
    UnfoldAll,
    /// Fold to specific level (0 = all, 1 = top-level only, etc.)
    FoldLevel(usize),
    /// Fold selection into manual region
    FoldSelection,
    /// Go to parent fold
    GoToParentFold,
    /// Go to next fold
    GoToNextFold,
    /// Go to previous fold
    GoToPrevFold,
}
```

---

## Key Bindings

| Action | Mac | Standard |
|--------|-----|----------|
| Toggle Fold | Cmd+Option+[ | Ctrl+Shift+[ |
| Fold | Cmd+Option+[ | Ctrl+Shift+[ |
| Unfold | Cmd+Option+] | Ctrl+Shift+] |
| Fold All | Cmd+K Cmd+0 | Ctrl+K Ctrl+0 |
| Unfold All | Cmd+K Cmd+J | Ctrl+K Ctrl+J |
| Fold Level 1 | Cmd+K Cmd+1 | Ctrl+K Ctrl+1 |
| Fold Level 2 | Cmd+K Cmd+2 | Ctrl+K Ctrl+2 |

---

## Rendering

### Fold Indicators in Gutter

```rust
fn render_fold_indicators(
    frame: &mut Frame,
    gutter_x: usize,
    folding: &FoldingState,
    visible_lines: Range<usize>,
    theme: &Theme,
) {
    for region in &folding.regions {
        if !visible_lines.contains(&region.start_line) {
            continue;
        }

        let y = line_to_y(region.start_line);
        let icon = match region.state {
            FoldState::Expanded => "▼",   // or "⌄"
            FoldState::Collapsed => "▶",  // or "›"
        };

        frame.draw_text(gutter_x, y, icon, theme.fold_indicator);
    }
}
```

### Collapsed Region Display

When a region is collapsed, show:
1. First line of the region
2. Fold indicator (`...` or `⋯`)
3. Optional summary (e.g., `{ 15 lines }`)

```rust
fn render_collapsed_fold(
    frame: &mut Frame,
    line_num: usize,
    region: &FoldRegion,
    theme: &Theme,
) {
    // Render first line normally
    render_line(frame, line_num);

    // Add fold placeholder at end
    let placeholder = format!(
        " ⋯ {} lines",
        region.end_line - region.start_line
    );
    frame.draw_text(
        line_end_x,
        line_y,
        &placeholder,
        theme.fold_placeholder,
    );
}
```

---

## Integration with Other Features

### Soft-Wrap (F-140)

Folding must coordinate with soft-wrap for visual line calculations:

```rust
// Visual line = document line accounting for:
// 1. Collapsed folds (subtract hidden lines)
// 2. Soft-wrapped lines (add extra visual lines)

fn document_to_visual_line(
    doc_line: usize,
    folding: &FoldingState,
    wrap_info: &WrapInfo,
) -> usize {
    let mut visual = 0;

    for line in 0..doc_line {
        if !folding.is_hidden(line) {
            visual += wrap_info.wrapped_line_count(line);
        }
    }

    visual
}
```

### Syntax Highlighting

Reuse existing tree-sitter infrastructure:

```rust
// src/syntax/worker.rs already parses documents
// Add fold detection to parse results

struct ParseResult {
    highlights: SyntaxHighlights,
    fold_regions: Vec<FoldRegion>,  // NEW
}
```

### Find/Replace

When searching:
- Auto-unfold regions containing matches
- Or show match count in collapsed regions

### Multi-Cursor

When a cursor moves into a collapsed region:
- Auto-unfold that region
- Or move cursor to region boundary

---

## Persistence

Save fold state per file:

```rust
// ~/.config/token-editor/fold-state.json
{
    "/path/to/file.rs": {
        "collapsed": [10, 45, 120],  // Start lines of collapsed regions
        "manual": [                   // User-created folds
            {"start": 50, "end": 55}
        ]
    }
}
```

---

## Performance Considerations

### Lazy Detection

Don't detect all folds upfront for large files:

```rust
struct LazyFoldingState {
    /// Fully computed regions
    computed_ranges: Vec<Range<usize>>,
    /// Ranges not yet computed
    pending_ranges: Vec<Range<usize>>,
}

impl LazyFoldingState {
    fn ensure_computed(&mut self, lines: Range<usize>) {
        // Only compute folds in viewport + buffer
    }
}
```

### Incremental Updates

When document changes:
- Invalidate fold regions that overlap with edit
- Shift line numbers for regions after edit
- Recompute only affected regions

```rust
fn update_folds_after_edit(
    folding: &mut FoldingState,
    edit_line: usize,
    lines_delta: isize,
) {
    // Remove regions that overlap with edit
    folding.regions.retain(|r| {
        !(r.start_line <= edit_line && r.end_line >= edit_line)
    });

    // Shift regions after edit
    for region in &mut folding.regions {
        if region.start_line > edit_line {
            region.start_line = (region.start_line as isize + lines_delta) as usize;
            region.end_line = (region.end_line as isize + lines_delta) as usize;
        }
    }
}
```

---

## Testing

### Unit Tests

```rust
#[test]
fn test_detect_rust_function_folds() {
    let source = r#"
fn main() {
    println!("hello");
}
"#;
    let folds = detect_syntax_folds(source, LanguageId::Rust);
    assert_eq!(folds.len(), 1);
    assert_eq!(folds[0].start_line, 1);
    assert_eq!(folds[0].end_line, 3);
}

#[test]
fn test_visual_line_mapping() {
    // Fold lines 5-10 (6 lines hidden)
    // Line 15 in doc should be visual line 9
}

#[test]
fn test_fold_toggle() {
    // Toggle fold, verify state changes
}
```

### Manual Testing Checklist

- [ ] Fold indicators appear in gutter
- [ ] Click indicator toggles fold
- [ ] Keyboard shortcuts work
- [ ] Collapsed regions show placeholder
- [ ] Cursor in collapsed region unfolds it
- [ ] Fold state persists across restart
- [ ] Works with soft-wrap enabled
- [ ] Find highlights visible in collapsed regions

---

## Implementation Plan

### Phase 1: Core Infrastructure (1 week)

1. Define `FoldRegion`, `FoldingState` types
2. Integrate fold state into `EditorState`
3. Implement visual/document line mapping
4. Basic gutter rendering (indicators)

### Phase 2: Fold Detection (1 week)

1. Indentation-based detection
2. Syntax-based detection (extend syntax worker)
3. Region marker detection
4. Incremental updates

### Phase 3: User Interaction (3 days)

1. Message handlers (Toggle, FoldAll, etc.)
2. Key bindings
3. Mouse interaction (click gutter)
4. Command palette integration

### Phase 4: Visual Polish (3 days)

1. Collapsed region placeholder
2. Fold summary extraction
3. Theme support
4. Animation (optional)

### Phase 5: Integration (3 days)

1. Soft-wrap coordination
2. Find integration
3. Multi-cursor handling
4. Persistence

---

## Dependencies

- **Soft-wrap (F-140)**: Required for visual line mapping
- **Syntax Highlighting**: Reuse tree-sitter infrastructure
- **Gutter Rendering**: Need clickable gutter regions

---

## References

- [VS Code Folding](https://code.visualstudio.com/docs/editor/codebasics#_folding)
- [JetBrains Folding](https://www.jetbrains.com/help/idea/code-folding.html)
- [Neovim foldexpr](https://neovim.io/doc/user/fold.html)
- Tree-sitter node traversal for fold detection
