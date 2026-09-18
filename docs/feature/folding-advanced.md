# Code Folding - Advanced (Follow-ups)

> **Implementation plan updated 2026-09-09:** Syntax-aware folding and saved fold
> state shipped in v0.7.0 via the
> [coordinated save, EditorConfig, and folding plan](../archived/file-policy-and-folding-plan.md#syntax-aware-folding-and-saved-state).
> This document now tracks only the ideas that plan explicitly deferred.

Region markers, fold level commands, and manual folds

> **Status:** Follow-ups after v0.7.0; nothing below is implemented
> **Priority:** P3 (Nice-to-have)
> **Effort:** M (a few days)
> **Created:** 2025-12-20
> **Milestone:** Deferred scope
> **Feature ID:** F-150b
> **Prerequisite:** [folding-basic.md](../archived/folding-basic.md) (complete)

---

## Overview

Basic and syntax folding are done. What exists today:

- `src/folding/mod.rs` - `FoldRegion { header, end, kind, offsets, fingerprint, context }` and per-pane collapse state
- `src/syntax/folding.rs` - tree-sitter fold providers for Rust, JavaScript/TypeScript, Python, JSON/YAML, HTML/CSS, Markdown, and Sema; indentation folding elsewhere
- `src/folding/persistence.rs` - fold metadata stored on saved tabs in the existing session store (no separate `fold-state.json`)
- Commands: `ToggleFold`, `CollapseFold`, `ExpandFold`, `CollapseAllFolds`, `ExpandAllFolds` (`src/keymap/command.rs`)

User docs live in [user/folding.md](../user/folding.md).

The coordinated plan named custom region markers, manual selection folds, and
fold-to-level commands as follow-ups. They remain open, along with collapsed-header
summary text. Any of them should build on the shared text viewport / visual-line
mapping already used by soft wrap and folding, not introduce a second mapping path.

### Goals (This Phase)

1. **Region markers** - Support `#region`/`#endregion` comments
2. **Fold level commands** - Fold to level 1, 2, 3, etc.
3. **Manual folds** - Create custom fold from selection
4. **Fold summaries** - Show e.g. `fn main()` on a collapsed header

---

## Features

### 1. Region Markers

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

Marker regions would be emitted as `FoldRegion`s with their own `kind` and merged
with the syntax/indentation candidates for the document.

### 2. Fold Level Commands

Fold to a specific nesting depth:

| Command      | Behavior                       |
| ------------ | ------------------------------ |
| Fold Level 1 | Fold only top-level regions    |
| Fold Level 2 | Fold level 1 + level 2 regions |
| Fold Level 3 | Fold level 1, 2, and 3         |

`FoldRegion` has no `level` field today; nesting depth would be derived from
region containment when the command runs.

### 3. Manual Folds

Create a fold from the current selection. Manual regions are not derived from the
document, so they need their own `kind` and must survive edits via the existing
`offsets`/`fingerprint` anchoring, and persist alongside saved folds.

### 4. Fold Summaries

Optional `summary` text on collapsed headers (e.g. `fn main()`), most useful for
syntax regions where the provider already knows the node.

---

## Keybindings (proposed)

| Action         | Mac           | Windows/Linux   | Command         |
| -------------- | ------------- | --------------- | --------------- |
| Fold Level 1   | `Cmd+K Cmd+1` | `Ctrl+K Ctrl+1` | `FoldLevel(1)`  |
| Fold Level 2   | `Cmd+K Cmd+2` | `Ctrl+K Ctrl+2` | `FoldLevel(2)`  |
| Fold Level 3   | `Cmd+K Cmd+3` | `Ctrl+K Ctrl+3` | `FoldLevel(3)`  |
| Fold Selection | `Cmd+K Cmd+[` | `Ctrl+K Ctrl+[` | `FoldSelection` |

None of these commands exist yet; see `src/keymap/command.rs` for the shipped set.

---

## Implementation Plan

### Phase 1: Region Markers

**Effort:** S (1-2 days)

- [ ] Implement `detect_region_markers()` with regex patterns
- [ ] Support C-style, HTML, and Python/Ruby comment markers
- [ ] Add to fold detection pipeline

### Phase 2: Fold Level Commands

**Effort:** S (1 day)

- [ ] Implement `fold_to_level()` method
- [ ] Add `FoldLevel(n)` command and keybindings
- [ ] Add to command palette

### Phase 3: Manual Folds

**Effort:** S (1 day)

- [ ] Implement `create_manual_fold()` from selection
- [ ] Add `FoldSelection` command
- [ ] Persist manual folds with the session fold state

### Phase 4: Fold Summaries

**Effort:** S (1 day)

- [ ] Add optional summary text to `FoldRegion`
- [ ] Render it on collapsed headers

---

## Dependencies

- Builds on the shipped folding in `src/folding/` and `src/syntax/folding.rs`
- Reuses existing tree-sitter infrastructure from syntax highlighting

---

## References

- [VS Code Folding Regions](https://code.visualstudio.com/docs/editor/codebasics#_folding)
- [JetBrains Custom Folding](https://www.jetbrains.com/help/idea/working-with-source-code.html#folding_comments)
- [file-policy-and-folding-plan.md](../archived/file-policy-and-folding-plan.md) - Shipped folding design
- [folding-basic.md](../archived/folding-basic.md) - Historical prerequisite document
