# Syntax-Aware Expand Selection

Use tree-sitter structure to expand selections through meaningful language scopes while preserving the existing plaintext fallback.

> **Status:** ✅ Complete (base-language phase)  
> **Priority:** P2 (Important)  
> **Effort:** M (3–5 days)  
> **Created:** 2026-08-11  
> **Updated:** 2026-08-11  
> **Milestone:** 2 - Search & Editing

---

## Overview

### Current State

`ExpandSelection` currently follows a fixed progression:

```text
cursor → word → line → document
```

Word detection is character based. An empty selection without a word expands directly to the line, any arbitrary single-line selection expands to the line, and any arbitrary multi-line selection expands to the document. The update layer cannot inspect tree-sitter nodes because authoritative parse trees live only in the syntax worker.

Expansion iterates all cursors, but history is a flat `Vec<Selection>` and `ShrinkSelection` restores only the active selection. This does not represent a complete multi-cursor expansion step.

### Goals

- Expand through strings, expressions, delimiter pairs, blocks, declarations, and other grammar-defined ancestors.
- Keep expansion synchronous, deterministic, and proportional to syntax-tree depth rather than document size.
- Preserve current behavior for plaintext, unsupported languages, stale parses, and unusable syntax trees.
- Make expand/shrink a lossless round trip for single and multiple cursors.
- Provide a foundation for injected-language selection without requiring it in the first implementation.

### Non-Goals

- Changing the `ExpandSelection` or `ShrinkSelection` commands or their default shortcuts.
- Adding language-specific selection query files in the first implementation.
- Retaining or traversing Markdown fenced-code and HTML/Vue script/style injection trees in the first implementation.
- Using highlight captures as a substitute for the syntax hierarchy.
- Running a synchronous parse on the UI thread when the current tree is stale.

---

## Behavior

### Candidate Order

For each selection, choose the first candidate that is a strict superset of its current normalized range:

1. Existing lexical word selection when starting from an empty cursor in a word.
2. The smallest useful named syntax node containing the current range.
3. A delimiter interior or exterior range when it adds a distinct step.
4. Successive useful named ancestors, excluding a root range equivalent to
   the whole document.
5. The current line when the syntax tree has no larger useful candidate.
6. The whole document.

Candidates with empty or duplicate ranges are removed. Nodes marked missing are ignored. `ERROR` nodes may participate when they provide a valid containing range, allowing incomplete code to degrade naturally.

Example for a cursor inside `bar`:

```rust
let result = calculate(foo.bar(), "hello world");
```

An appropriate grammar may produce:

```text
bar
→ foo.bar
→ foo.bar()
→ calculate(foo.bar(), "hello world")
→ let result = calculate(foo.bar(), "hello world");
→ containing block
→ containing declaration
→ document
```

Exact intermediate steps are grammar-defined. The implementation guarantees containment and monotonic growth, not identical node names across languages.

Some grammars expose ranges that include formatting or attach trivia to a
neighboring node. Candidate normalization corrects these cases without changing
the parse tree: YAML collection scopes exclude comments attached at their outer
boundaries, INI values exclude horizontal padding around `=`, and Rust items
gain a distinct owning range containing contiguous documentation comments and
outer attributes.

An empty cursor at the end of a code line uses boundary affinity before the
normal ancestor walk. Trailing horizontal whitespace is ignored, a completed
named node has left affinity, and an opening delimiter retains right affinity to
the contents that follow it. If the terminal token ends no completed node, as
with an ambiguous separator or continuation boundary, expansion falls back to
the current line instead of jumping to a broader containing node.

### Delimiter Ranges

For nodes whose source range is grammar-confirmed and wrapped by matching delimiters, synthesize separate inner and outer candidates:

- `(content)`
- `[content]`
- `{content}`
- quoted string nodes, including grammar-specific multiline or raw strings

Do not infer strings from quote characters alone. The node kind must establish that the range is a string so escaping and language-specific literal forms remain correct. Angle brackets are excluded from generic delimiter detection because comparisons, generics, and markup make them ambiguous.

### Fallback and Freshness

Use syntax-aware candidates only when the tree snapshot language matches the document and its revision equals `Document::revision`. Never apply a stale tree range to newer text.

When no current tree is available, retain the existing behavior:

```text
cursor → word (when present) → line → document
```

This fallback also applies to plaintext, unsupported languages, parser failure, and the debounce interval immediately after an edit.

Zero-sized or unchanged fallback ranges are skipped. In particular, expansion
on an empty line proceeds to the document instead of repeatedly selecting the
same empty range; this corrects the current degenerate no-op while preserving
the normal fallback sequence.

### Multiple Cursors

Each cursor expands independently to its next candidate. Reaching a syntax root does not force unrelated cursors to skip their remaining scopes. When every selection has no candidate smaller than the document, expansion collapses to the existing single full-document selection.

Each invocation records one complete snapshot. Shrink restores all cursors, selections, their direction, and the active cursor index from that snapshot.

---

## Architecture

### Tree Snapshot

Publish the worker's current base-language tree as revision-tagged document state:

```rust
#[derive(Debug, Clone)]
pub struct SyntaxTreeSnapshot {
    pub revision: u64,
    pub language: LanguageId,
    pub tree: tree_sitter::Tree,
}
```

Add `syntax_tree: Option<SyntaxTreeSnapshot>` to `Document`. `SyntaxMsg::ParseCompleted` carries the snapshot alongside highlights and outline data. The update handler stores all three only after its existing document/revision validation. Clear the snapshot on language changes and document teardown.

Tree cloning is used to share the immutable parsed result with the model; parsing remains owned by the worker. No parser or query object crosses the worker boundary.

### Selection API

Add a pure syntax-selection module with an interface equivalent to:

```rust
pub fn expansion_candidates(
    document: &Document,
    snapshot: &SyntaxTreeSnapshot,
    selection: Selection,
) -> Vec<Selection>;
```

The module:

- Converts editor line/character columns through Rope character offsets to UTF-8 byte offsets.
- Finds the smallest node containing the complete half-open selection range.
- Walks parents and emits useful named-node ranges.
- Adds validated delimiter-interior ranges.
- Converts tree byte endpoints back through Rope byte/character offsets to editor positions.
- Preserves the original anchor/head direction when constructing expanded selections.
- Sorts by containment size and removes identical ranges.

Do not scan the entire tree or execute highlight queries. Candidate lookup should be a descendant lookup followed by a parent walk.

### Selection History

Replace the flat history entry with a whole-editor snapshot:

```rust
#[derive(Debug, Clone)]
pub struct SelectionSnapshot {
    pub cursors: Vec<Cursor>,
    pub selections: Vec<Selection>,
    pub active_cursor_index: usize,
}
```

`EditorState::selection_history` becomes `Vec<SelectionSnapshot>`. Expansion pushes once before applying changes. Shrink pops once and restores the complete snapshot. Existing operations that clear expansion history continue to clear the new stack.

### Message Flow

```text
syntax worker parses revision N
    → ParseCompleted(tree snapshot, highlights, outline)
    → revision validation
    → Document stores snapshot

ExpandSelection
    → validate snapshot revision/language
    → calculate next candidate per selection
    → push one SelectionSnapshot
    → apply candidates and merge only genuinely overlapping results
    → redraw editor
```

### Injected Languages: Deferred Phase

The first phase exposes only the cached base tree. Markdown inline/fenced parsers and HTML/Vue embedded parsers currently create additional trees temporarily for highlighting.

A later phase may retain injected snapshots with absolute base byte offsets. Selection inside an injection will walk the injected tree first and then continue through containing outer-language nodes. The base snapshot type must therefore remain extensible rather than implying that one tree is permanently sufficient.

---

## Implementation Plan

### Phase 1: Tree Availability and Coordinate Safety

- [x] Add `SyntaxTreeSnapshot` and store it on `Document`.
- [x] Return the current base tree with successful parse completion.
- [x] Store and clear snapshots with the same revision/language lifecycle as syntax highlights.
- [x] Add tested helpers for editor character positions ↔ tree-sitter UTF-8 byte offsets, including CRLF and Unicode.

### Phase 2: Structural Expansion

- [x] Add pure candidate generation using the smallest containing node and named ancestors.
- [x] Add strict containment, missing-node filtering, deduplication, and direction preservation.
- [x] Add grammar-confirmed string and generic `()`, `[]`, `{}` inner/outer ranges.
- [x] Normalize YAML boundary comments and INI value padding, and synthesize
      documented/attributed Rust item ranges.
- [x] Add reusable markup boundary profiles for completed elements and element
      interiors across XML, HTML, Vue, Svelte, JSX, and TSX.
- [x] Integrate candidates into `ExpandSelection` with the unchanged plaintext fallback.
- [x] Keep multi-cursor expansion independent until the final document scope.

### Phase 3: History and Verification

- [x] Replace flat history entries with complete selection snapshots.
- [x] Preserve existing history-clearing behavior for unrelated cursor and selection operations.
- [x] Verify CLI/MCP automation can invoke expand/shrink by action name and inspect resulting selections.
- [x] Update the behavior contract, user documentation, roadmap status, and changelog.

### Future: Injected Trees

- [ ] Retain Markdown inline/fenced-code and HTML/Vue script/style trees with absolute offsets.
- [ ] Select the innermost injected tree at the cursor and bridge its root to the outer tree.
- [ ] Add mixed-language expansion tests at injection boundaries.

---

## Testing Strategy

### Core Behavior

- Cursor to lexical word, then syntax leaf and successive ancestors.
- Arbitrary selection to the smallest strict containing syntax scope.
- String content to complete literal, including escaped, raw, and multiline strings.
- Parenthesis, bracket, and brace interior/exterior stages.
- Calls, member chains, expressions, statements, blocks, and declarations.
- No duplicate or unchanged expansion steps; every step strictly contains the previous range.
- Shrink restores the exact anchor/head direction and cursor state.

### Languages

- Rust: nested calls, method chains, closures, blocks, comments, and raw strings.
- Rust attributed items: bare item, then attached documentation/attributes,
  stopping at blank lines or ordinary comments.
- JavaScript/TypeScript: object literals, template strings, arrow functions, and member calls.
- Python: calls, collections, indentation blocks, and multiline strings.
- HTML: nested elements and attributes using the base HTML tree.
- Markdown: headings, lists, block quotes, and fenced-block containers using
  the base block tree. Inline emphasis/link scopes are deferred with retained
  injected/inline trees.
- YAML nested mappings/sequences do not absorb a following sibling comment.
- INI values behave consistently with or without whitespace around `=`.

### Edge Cases

- Unicode before and inside selected nodes.
- CRLF input and multiline node endpoints.
- Cursor exactly at either edge of a node.
- Empty documents and empty lines.
- Unsupported and plaintext documents.
- A document edit newer than the tree snapshot.
- Incomplete syntax, missing nodes, and `ERROR` nodes.
- Multiple cursors at different syntax depths, including overlapping expansions.
- Full expand/shrink round trips after cursor sorting or selection merging.

### Performance and Automation

- Benchmark candidate lookup in small and large documents at equivalent syntax depth; runtime should not scale with total line count.
- Add automated action tests that set cursor/selection state, invoke `ExpandSelection`/`ShrinkSelection`, and assert returned selections.
- Keep tree publication out of renderer timings and verify that syntax worker profiling remains unchanged apart from the inexpensive tree clone/message transfer.

Current release-benchmark medians for `syntax_selection_candidates` are
3.27 µs at 100 lines, 2.86 µs at 1,000 lines, 4.94 µs at 5,000 lines,
and 5.02 µs at 10,000 lines. Candidate generation performs 3–4 allocations
(268–412 bytes) per invocation. CLI automation also verified the sequence
`lines → lines.iter → lines` through named expand/shrink actions.

---

## Acceptance Criteria

- Supported, freshly parsed code expands through meaningful syntax ancestors before line/document fallback.
- Strings and `()`, `[]`, `{}` constructs expose useful inner and outer ranges without quote-based misclassification.
- Plaintext and stale-tree behavior matches the current implementation except
  that zero-sized/unchanged fallback steps are skipped.
- All expansion steps are deterministic strict supersets.
- Multi-cursor shrink restores the complete preceding expansion state.
- Expansion performs no synchronous parsing and no whole-tree scan on the UI thread.
- Existing expand/shrink command names and shortcuts remain compatible.
