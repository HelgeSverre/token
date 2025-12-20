# Code Folding - Advanced (Syntax-Based)

Syntax-aware folding, region markers, and fold persistence

> **Status:** 📋 Planned (Future)
> **Priority:** P3 (Nice-to-have)
> **Effort:** L (1-2 weeks)
> **Created:** 2025-12-20
> **Milestone:** 6 - Productivity
> **Feature ID:** F-150b
> **Prerequisite:** [folding-basic.md](folding-basic.md) must be complete

---

## Overview

This document covers **advanced folding features** that build on the basic indentation-based folding. These features require more complex infrastructure and language-specific knowledge.

### Prerequisites

Before implementing this phase:
- ✅ Basic indentation-based folding working
- ✅ Fold regions, toggle, and rendering complete
- ✅ Visual line mapping for collapsed folds

### Goals (This Phase)

1. **Syntax-based detection** - Use tree-sitter to find functions, classes, etc.
2. **Region markers** - Support `#region`/`#endregion` comments
3. **Fold level commands** - Fold to level 1, 2, 3, etc.
4. **Fold persistence** - Remember fold state across sessions
5. **Manual folds** - Create custom fold from selection

---

## Features

### 1. Syntax-Based Detection

Use tree-sitter parse tree to detect language constructs:

```rust
// Rust: functions, impls, structs, enums, mods, match arms
fn detect_rust_folds(tree: &Tree) -> Vec<FoldRegion> {
    let foldable = ["function_item", "impl_item", "struct_item",
                    "enum_item", "mod_item", "match_expression"];
    // Walk tree, find nodes...
}

// JavaScript: functions, classes, if/for/while blocks
fn detect_js_folds(tree: &Tree) -> Vec<FoldRegion> {
    let foldable = ["function_declaration", "class_declaration",
                    "method_definition", "if_statement", "for_statement"];
    // Walk tree, find nodes...
}
```

**Integration:** Hook into existing syntax highlighting worker to compute folds after parse.

### 2. Region Markers

Support explicit fold regions via comments:

```rust
// #region Helper Functions
fn helper_one() { }
fn helper_two() { }
// #endregion

// Also support:
// <!-- region Name --> / <!-- endregion --> (HTML)
// # region Name / # endregion (Python)
```

```rust
fn detect_region_markers(document: &Document) -> Vec<FoldRegion> {
    let patterns = [
        (r"//\s*#region\b", r"//\s*#endregion\b"),   // C-style
        (r"<!--\s*region\b", r"<!--\s*endregion"),   // HTML
        (r"#\s*region\b", r"#\s*endregion\b"),       // Python/Ruby
    ];
    // Match and pair markers...
}
```

### 3. Fold Level Commands

Fold to a specific nesting depth:

| Command | Behavior |
|---------|----------|
| Fold Level 1 | Fold only top-level regions |
| Fold Level 2 | Fold level 1 + level 2 regions |
| Fold Level 3 | Fold level 1, 2, and 3 |

```rust
pub fn fold_to_level(&mut self, max_level: usize) {
    for region in &mut self.regions {
        region.collapsed = region.level < max_level;
    }
}
```

### 4. Fold Persistence

Save fold state per file:

```json
// ~/.config/token-editor/fold-state.json
{
    "/path/to/file.rs": {
        "version": 1,
        "collapsed_lines": [10, 45, 120],
        "manual_folds": [
            { "start": 50, "end": 55 }
        ],
        "timestamp": "2025-12-20T10:00:00Z"
    }
}
```

**Restore logic:**
1. Load fold state on file open
2. Match collapsed lines to current fold regions
3. Apply collapsed state
4. Save on file close or explicit save

### 5. Manual Folds

Create fold from selection:

```rust
pub fn create_manual_fold(&mut self, start_line: usize, end_line: usize) {
    if end_line > start_line {
        self.regions.push(FoldRegion {
            start_line,
            end_line,
            level: 0,
            collapsed: true,
            source: FoldSource::Manual,
        });
        self.regions.sort_by_key(|r| r.start_line);
    }
}
```

---

## Data Structure Extensions

```rust
/// Extended FoldRegion with source tracking
#[derive(Debug, Clone)]
pub struct FoldRegion {
    pub start_line: usize,
    pub end_line: usize,
    pub level: usize,
    pub collapsed: bool,
    pub source: FoldSource,          // NEW
    pub summary: Option<String>,     // NEW: e.g., "fn main()" 
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FoldSource {
    Indentation,
    Syntax,
    RegionMarker,
    Manual,
}
```

---

## Keybindings

| Action | Mac | Windows/Linux | Command |
|--------|-----|---------------|---------|
| Fold Level 1 | `Cmd+K Cmd+1` | `Ctrl+K Ctrl+1` | `FoldLevel(1)` |
| Fold Level 2 | `Cmd+K Cmd+2` | `Ctrl+K Ctrl+2` | `FoldLevel(2)` |
| Fold Level 3 | `Cmd+K Cmd+3` | `Ctrl+K Ctrl+3` | `FoldLevel(3)` |
| Fold Selection | `Cmd+K Cmd+[` | `Ctrl+K Ctrl+[` | `FoldSelection` |

---

## Implementation Plan

### Phase 1: Syntax-Based Detection

**Effort:** M (3-4 days)

- [ ] Add `FoldSource` enum to `FoldRegion`
- [ ] Implement `detect_syntax_folds()` for each language
- [ ] Hook into syntax worker to compute folds after parse
- [ ] Merge syntax folds with indentation folds (prefer syntax)

### Phase 2: Region Markers

**Effort:** S (1-2 days)

- [ ] Implement `detect_region_markers()` with regex patterns
- [ ] Support C-style, HTML, and Python/Ruby comment markers
- [ ] Add to fold detection pipeline

### Phase 3: Fold Level Commands

**Effort:** S (1 day)

- [ ] Implement `fold_to_level()` method
- [ ] Add `FoldLevel(n)` command and keybindings
- [ ] Add to command palette

### Phase 4: Fold Persistence

**Effort:** M (2-3 days)

- [ ] Design fold state JSON schema
- [ ] Implement save/load functions
- [ ] Save fold state on file close
- [ ] Restore fold state on file open
- [ ] Handle stale state (file changed)

### Phase 5: Manual Folds

**Effort:** S (1 day)

- [ ] Implement `create_manual_fold()` from selection
- [ ] Add `FoldSelection` command
- [ ] Persist manual folds separately

---

## Dependencies

- Requires [folding-basic.md](folding-basic.md) to be complete
- Reuses existing tree-sitter infrastructure from syntax highlighting

---

## References

- [VS Code Folding Regions](https://code.visualstudio.com/docs/editor/codebasics#_folding)
- [JetBrains Custom Folding](https://www.jetbrains.com/help/idea/working-with-source-code.html#folding_comments)
- [folding-basic.md](folding-basic.md) - Prerequisite document
