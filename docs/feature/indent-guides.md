# Indentation Guides

Visual vertical lines showing indentation levels with depth-based coloring

> **Status:** Planned
> **Priority:** P2
> **Effort:** M
> **Created:** 2025-12-20
> **Milestone:** 2 - Refinement

---

## Table of Contents

1. [Overview](#overview)
2. [Architecture](#architecture)
3. [Visual Design](#visual-design)
4. [Data Structures](#data-structures)
5. [Implementation Plan](#implementation-plan)
6. [Testing Strategy](#testing-strategy)
7. [References](#references)

---

## Overview

### Current State

The editor currently:
- Shows whitespace rendering (spaces/tabs as dots/arrows)
- Has no visual indication of indentation hierarchy
- Relies on syntax highlighting for scope detection
- Difficult to track deep nesting, especially in Python/YAML

### Goals

1. **Vertical indent guides**: Faint vertical lines at each indentation level
2. **Depth-based coloring**: Different colors/intensities per nesting depth
3. **Scope-aware highlighting**: Active scope's guide more prominent
4. **Tree-sitter integration**: Use AST for accurate scope detection
5. **Per-language configuration**: Different behavior for Python vs braces
6. **Performance**: Efficient rendering for large files

### Non-Goals

- Rainbow brackets (separate feature)
- Scope folding from guides (use existing folding)
- Custom user color configuration (first iteration)

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         Indentation Guide Rendering                          │
│                                                                              │
│  Visual (with active scope on depth 2):                                      │
│                                                                              │
│  │    fn main() {                                                           │
│  │    ┆   let config = Config::new();                                       │
│  │    ┆   if config.enabled {                                               │
│  │    ┆   ║   for item in items {                      ← cursor here       │
│  │    ┆   ║       process(item);                                            │
│  │    ┆   ║   }                                                              │
│  │    ┆   }                                                                  │
│  │    ┆   cleanup();                                                         │
│  │    }                                                                      │
│                                                                              │
│  Legend:                                                                     │
│  │ = depth 0 (faintest, not shown in braces languages)                      │
│  ┆ = depth 1 (subtle gray)                                                  │
│  ║ = depth 2 (active scope - highlighted)                                   │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Color Gradient Example

```
Depth 0: #404040 (30% opacity) ──────▶ Faintest
Depth 1: #505050 (40% opacity)
Depth 2: #606060 (50% opacity)
Depth 3: #707070 (60% opacity)
Depth 4: #808080 (70% opacity)
Depth 5: #909090 (80% opacity) ──────▶ Most visible
Active:  Theme accent color (full opacity)
```

### Module Structure

```
src/
├── indent_guides/
│   ├── mod.rs           # Module exports
│   ├── detector.rs      # Indentation level detection
│   ├── scope.rs         # Active scope detection (tree-sitter)
│   └── renderer.rs      # Guide line rendering
├── view/
│   └── editor.rs        # Integrate guide rendering
└── model/
    └── editor.rs        # IndentGuideConfig
```

---

## Visual Design

### Guide Appearance

```
Line thickness: 1 pixel (crisp)
Line style: Solid (or optionally dotted for differentiation)
Vertical position: Aligned to character column
Height: Full line height
```

### Depth Coloring Strategies

**Strategy 1: Opacity Gradient (Default)**
All guides same hue, opacity increases with depth
```rust
fn guide_opacity(depth: usize) -> f32 {
    0.2 + (depth as f32 * 0.1).min(0.7)
}
```

**Strategy 2: Hue Rotation (Rainbow)**
Different hues per depth level
```rust
fn guide_hue(depth: usize) -> f32 {
    (depth * 30) % 360 // 30° hue shift per level
}
```

**Strategy 3: Saturation Gradient**
Same hue, increasing saturation with depth
```rust
fn guide_color(depth: usize, base_hue: f32) -> Color {
    let saturation = 0.1 + (depth as f32 * 0.15).min(0.8);
    hsv_to_rgb(base_hue, saturation, 0.5)
}
```

### Active Scope Highlighting

The indent guide containing the cursor is highlighted:
- Uses theme accent color
- Full opacity
- Optionally thicker (2px instead of 1px)
- Spans from scope start to scope end

### Language-Specific Behavior

**Python/YAML (whitespace-significant):**
- Show guides at every indentation level
- Critical for readability
- Don't require tree-sitter (use leading whitespace)

**Rust/JavaScript/etc. (brace-based):**
- Show guides based on block structure
- Use tree-sitter for scope detection
- Skip guides for short single-line blocks

---

## Data Structures

### IndentGuideConfig

```rust
// In src/model/editor.rs

/// Configuration for indent guides
#[derive(Debug, Clone)]
pub struct IndentGuideConfig {
    /// Whether guides are enabled
    pub enabled: bool,
    
    /// Coloring strategy
    pub color_strategy: GuideColorStrategy,
    
    /// Whether to highlight active scope
    pub highlight_active: bool,
    
    /// Minimum indentation level to show (0 = show all)
    pub min_depth: usize,
    
    /// Maximum indentation levels to render
    pub max_depth: usize,
    
    /// Base opacity (0.0 - 1.0)
    pub base_opacity: f32,
    
    /// Opacity increment per depth
    pub opacity_step: f32,
    
    /// Whether to use scope detection (tree-sitter)
    pub use_scopes: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GuideColorStrategy {
    /// Same color, increasing opacity
    OpacityGradient,
    /// Different hues per depth
    Rainbow,
    /// Theme-based with saturation gradient
    Saturation,
}

impl Default for IndentGuideConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            color_strategy: GuideColorStrategy::OpacityGradient,
            highlight_active: true,
            min_depth: 1,
            max_depth: 12,
            base_opacity: 0.2,
            opacity_step: 0.08,
            use_scopes: true,
        }
    }
}
```

### IndentInfo

```rust
// In src/indent_guides/detector.rs

/// Indentation information for a line
#[derive(Debug, Clone, Copy)]
pub struct IndentInfo {
    /// Number of indentation levels
    pub depth: usize,
    
    /// Character column where content starts
    pub content_start: usize,
    
    /// Whether this line continues a scope from above
    pub continues_scope: bool,
    
    /// Whether this line starts a new scope
    pub starts_scope: bool,
    
    /// Whether this line ends a scope
    pub ends_scope: bool,
}

/// Analyze indentation for visible lines
pub fn analyze_indentation(
    document: &Document,
    visible_range: Range<usize>,
    indent_size: usize,
) -> Vec<IndentInfo> {
    let mut result = Vec::with_capacity(visible_range.len());
    
    for line_idx in visible_range {
        let line = document.get_line(line_idx).unwrap_or_default();
        let leading = count_leading_whitespace(&line, indent_size);
        
        result.push(IndentInfo {
            depth: leading / indent_size,
            content_start: leading,
            continues_scope: false, // Filled in by scope analyzer
            starts_scope: false,
            ends_scope: false,
        });
    }
    
    result
}

fn count_leading_whitespace(line: &str, tab_width: usize) -> usize {
    let mut count = 0;
    for ch in line.chars() {
        match ch {
            ' ' => count += 1,
            '\t' => count += tab_width - (count % tab_width),
            _ => break,
        }
    }
    count
}
```

### ScopeInfo

```rust
// In src/indent_guides/scope.rs

use tree_sitter::Tree;

/// Active scope information for indent guide highlighting
#[derive(Debug, Clone)]
pub struct ScopeInfo {
    /// Start line of active scope
    pub start_line: usize,
    
    /// End line of active scope
    pub end_line: usize,
    
    /// Indentation depth of scope
    pub depth: usize,
    
    /// Nested scopes
    pub children: Vec<ScopeInfo>,
}

/// Detect scopes containing the cursor
pub fn find_cursor_scopes(
    tree: &Tree,
    cursor_line: usize,
    cursor_column: usize,
) -> Vec<ScopeInfo> {
    let root = tree.root_node();
    let mut scopes = Vec::new();
    
    find_scopes_recursive(root, cursor_line, cursor_column, 0, &mut scopes);
    
    scopes
}

fn find_scopes_recursive(
    node: tree_sitter::Node,
    cursor_line: usize,
    cursor_column: usize,
    depth: usize,
    scopes: &mut Vec<ScopeInfo>,
) {
    let start = node.start_position();
    let end = node.end_position();
    
    // Check if cursor is within this node
    let cursor_in_node = 
        (cursor_line > start.row || (cursor_line == start.row && cursor_column >= start.column)) &&
        (cursor_line < end.row || (cursor_line == end.row && cursor_column <= end.column));
    
    if !cursor_in_node {
        return;
    }
    
    // Check if this is a scope-creating node
    if is_scope_node(&node) && end.row > start.row {
        scopes.push(ScopeInfo {
            start_line: start.row,
            end_line: end.row,
            depth,
            children: Vec::new(),
        });
    }
    
    // Recurse into children
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            find_scopes_recursive(child, cursor_line, cursor_column, depth + 1, scopes);
        }
    }
}

fn is_scope_node(node: &tree_sitter::Node) -> bool {
    matches!(
        node.kind(),
        "block" | "function_item" | "impl_item" | "if_expression" |
        "for_expression" | "while_expression" | "match_expression" |
        "closure_expression" | "struct_item" | "enum_item" |
        // JavaScript/TypeScript
        "statement_block" | "function_declaration" | "arrow_function" |
        "if_statement" | "for_statement" | "while_statement" |
        "class_declaration" | "method_definition" |
        // Python
        "function_definition" | "class_definition" | "if_statement" |
        "for_statement" | "while_statement" | "with_statement" |
        "try_statement"
    )
}
```

### Guide Renderer

```rust
// In src/indent_guides/renderer.rs

use crate::view::RenderContext;

/// Render indent guides for visible lines
pub fn render_indent_guides(
    ctx: &mut RenderContext,
    config: &IndentGuideConfig,
    indent_infos: &[IndentInfo],
    active_scopes: &[ScopeInfo],
    viewport_top: usize,
    indent_size: usize,
    char_width: f32,
    line_height: f32,
    gutter_width: f32,
) {
    if !config.enabled {
        return;
    }
    
    // Find active scope depth for highlighting
    let active_depth = active_scopes.last().map(|s| s.depth);
    let active_range = active_scopes.last()
        .map(|s| s.start_line..=s.end_line);
    
    for (visual_idx, info) in indent_infos.iter().enumerate() {
        let line_idx = viewport_top + visual_idx;
        let y = visual_idx as f32 * line_height;
        
        // Draw guide for each depth level
        for depth in config.min_depth..=info.depth.min(config.max_depth) {
            let x = gutter_width + (depth * indent_size) as f32 * char_width;
            
            // Determine if this is the active scope guide
            let is_active = config.highlight_active 
                && active_depth == Some(depth)
                && active_range.as_ref()
                    .map(|r| r.contains(&line_idx))
                    .unwrap_or(false);
            
            let color = if is_active {
                ctx.theme.accent_color
            } else {
                guide_color(depth, config)
            };
            
            let thickness = if is_active { 2.0 } else { 1.0 };
            
            // Draw vertical line
            ctx.draw_line(
                x, y,
                x, y + line_height,
                color,
                thickness,
            );
        }
    }
}

fn guide_color(depth: usize, config: &IndentGuideConfig) -> Color {
    match config.color_strategy {
        GuideColorStrategy::OpacityGradient => {
            let opacity = (config.base_opacity + depth as f32 * config.opacity_step)
                .min(0.9);
            Color::rgba(128, 128, 128, (opacity * 255.0) as u8)
        }
        GuideColorStrategy::Rainbow => {
            let hue = ((depth * 50) % 360) as f32;
            hsv_to_rgb(hue, 0.4, 0.6)
        }
        GuideColorStrategy::Saturation => {
            let saturation = 0.1 + (depth as f32 * 0.12).min(0.7);
            hsv_to_rgb(220.0, saturation, 0.5)
        }
    }
}
```

---

## Keybindings

| Action | Mac | Windows/Linux | Message |
|--------|-----|---------------|---------|
| Toggle indent guides | Cmd+Shift+I | Ctrl+Shift+I | `EditorMsg::ToggleIndentGuides` |
| Cycle color strategy | (via settings) | (via settings) | - |

---

## Implementation Plan

### Phase 1: Basic Indentation Detection

**Estimated effort: 1-2 days**

1. [ ] Create `src/indent_guides/mod.rs` module
2. [ ] Implement `count_leading_whitespace()`
3. [ ] Implement `analyze_indentation()` for visible lines
4. [ ] Handle tabs vs spaces correctly
5. [ ] Unit tests for detection

**Test:** Correctly count indentation depth for various lines

### Phase 2: Simple Guide Rendering

**Estimated effort: 2 days**

1. [ ] Add `IndentGuideConfig` to `EditorState`
2. [ ] Implement basic vertical line rendering
3. [ ] Render guides in editor view (after gutter, before text)
4. [ ] Basic opacity gradient coloring
5. [ ] Toggle keybinding

**Test:** Guides render at correct column positions

### Phase 3: Depth-Based Coloring

**Estimated effort: 1-2 days**

1. [ ] Implement `GuideColorStrategy` enum
2. [ ] Implement opacity gradient strategy
3. [ ] Implement rainbow strategy
4. [ ] Implement saturation gradient strategy
5. [ ] Add config option to switch strategies

**Test:** Different depths have visually distinct colors

### Phase 4: Scope Detection (Tree-sitter)

**Estimated effort: 2-3 days**

1. [ ] Implement `find_cursor_scopes()` using tree-sitter
2. [ ] Identify scope-creating node types per language
3. [ ] Cache scope info (invalidate on cursor move)
4. [ ] Handle languages without tree-sitter (fallback to indent)

**Test:** Active scope correctly identified from AST

### Phase 5: Active Scope Highlighting

**Estimated effort: 1-2 days**

1. [ ] Highlight guide for active scope
2. [ ] Use theme accent color
3. [ ] Optionally increase thickness
4. [ ] Highlight entire scope range (not just cursor line)
5. [ ] Smooth transitions (future: animation)

**Test:** Moving cursor updates highlighted scope correctly

### Phase 6: Python/YAML Special Handling

**Estimated effort: 1 day**

1. [ ] Detect whitespace-significant languages
2. [ ] Always use indent-based detection (not scope)
3. [ ] Show guides at every level (no min_depth)
4. [ ] Especially important for long blocks

**Test:** Python files show guides correctly

### Phase 7: Performance Optimization

**Estimated effort: 1-2 days**

1. [ ] Only compute guides for visible lines
2. [ ] Cache indent info per line (invalidate on edit)
3. [ ] Batch guide rendering (single draw call)
4. [ ] Profile with large deeply-nested files

**Test:** No perceptible lag in large files

---

## Testing Strategy

### Unit Tests

```rust
#[test]
fn test_count_leading_whitespace() {
    assert_eq!(count_leading_whitespace("    hello", 4), 4);
    assert_eq!(count_leading_whitespace("\thello", 4), 4);
    assert_eq!(count_leading_whitespace("  \thello", 4), 4);
    assert_eq!(count_leading_whitespace("hello", 4), 0);
}

#[test]
fn test_indent_depth() {
    assert_eq!(IndentInfo::from_line("hello", 4).depth, 0);
    assert_eq!(IndentInfo::from_line("    hello", 4).depth, 1);
    assert_eq!(IndentInfo::from_line("        hello", 4).depth, 2);
}

#[test]
fn test_guide_color_opacity() {
    let config = IndentGuideConfig::default();
    let c1 = guide_color(1, &config);
    let c2 = guide_color(2, &config);
    assert!(c2.a > c1.a); // Higher depth = higher opacity
}
```

### Visual Tests

```rust
#[test]
fn test_guide_rendering() {
    let mut model = test_model_with_text(r#"
fn main() {
    if true {
        for i in 0..10 {
            println!("{}", i);
        }
    }
}
"#);
    
    // Enable guides
    model.editor_mut().indent_guide_config.enabled = true;
    
    // Render and verify guide positions
    let frame = render_frame(&model);
    
    // Check that guides exist at columns 0, 4, 8, 12
    assert!(frame.has_vertical_line_at_column(4));
    assert!(frame.has_vertical_line_at_column(8));
    assert!(frame.has_vertical_line_at_column(12));
}
```

### Manual Testing Checklist

- [ ] Guides appear at each indentation level
- [ ] Deeper indentation has more visible guides
- [ ] Active scope is highlighted with accent color
- [ ] Guides don't overlap with text
- [ ] Toggle shortcut works
- [ ] Works with tabs
- [ ] Works with spaces
- [ ] Works with mixed indentation
- [ ] Python files show all levels
- [ ] Cursor movement updates active scope
- [ ] Large files remain performant
- [ ] Empty lines show appropriate guides

---

## Edge Cases

1. **Empty lines**: Show guides matching surrounding context
2. **Mixed tabs/spaces**: Normalize to visual columns
3. **Very deep nesting**: Cap at `max_depth`, ensure visibility
4. **Cursor on blank line**: Show scope from nearest content line
5. **Multiple cursors**: Highlight scope for primary cursor
6. **Folded regions**: Skip guides for folded content
7. **Soft-wrapped lines**: Guides on first visual line only

---

## References

- VS Code indent guides: Built-in feature with similar behavior
- Sublime Text "indent_guide_options"
- JetBrains IDEs: Scope highlighting feature
- Tree-sitter: https://tree-sitter.github.io/tree-sitter/
- Existing highlighting: `src/view/editor.rs`
