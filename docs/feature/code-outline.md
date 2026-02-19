# Code Outline Panel

Collapsible tree view showing document structure — headings for markdown, classes/methods/functions for code files.

> **Status:** Planned
> **Priority:** P2
> **Effort:** M (3–5 days)
> **Created:** 2026-02-19
> **Updated:** 2026-02-19
> **Milestone:** 3 - Workspace Features

---

## Table of Contents

1. [Overview](#overview)
2. [Architecture](#architecture)
3. [Data Structures](#data-structures)
4. [Messages](#messages)
5. [Keybindings](#keybindings)
6. [Implementation Plan](#implementation-plan)
7. [Testing Strategy](#testing-strategy)
8. [Dependencies](#dependencies)
9. [References](#references)

---

## Overview

### Current State

The editor has **full dock panel infrastructure** already built:

- ✅ `DockLayout` with Left/Right/Bottom docks (`src/panel/dock.rs`)
- ✅ `PanelId::Outline` registered in right dock, `Cmd+7` keybinding wired
- ✅ Dock resize handles with mouse drag
- ✅ `DockRects` in `geometry.rs` for layout calculation
- ✅ `FocusTarget::Dock(DockPosition)` for keyboard focus routing
- ✅ `HitTarget` variants for dock resize, tabs, and content
- ✅ Sidebar file tree with recursive `render_node` pattern (expand/collapse, scroll, selection)
- ✅ Tree-sitter parsing on background worker thread with cached `Tree` per document
- ✅ 20+ languages with tree-sitter grammars already initialized
- ✅ `SyntaxHighlights` stored per-document with revision-based staleness checking
- ⏳ Currently renders `PlaceholderPanel` with "Code outline coming soon..."

### Goals

1. **Structural overview**: Show document symbols in a collapsible tree view
2. **Markdown headings**: Show `# H1 > ## H2 > ### H3` hierarchy
3. **Code symbols**: Show classes, structs, functions, methods, properties hierarchically
4. **Click-to-jump**: Click a symbol to navigate to its definition
5. **Follow cursor**: Optionally highlight the symbol containing the cursor
6. **Incremental updates**: Outline updates piggyback on existing syntax parse debounce

### Non-Goals (MVP)

- Symbol search / filtering within the outline
- Drag-and-drop symbol reordering
- Symbol rename from outline
- Symbol type signatures or parameter info
- Custom icons per symbol kind (use text indicators)
- Outline for multiple documents simultaneously (only focused doc)

---

## Architecture

### Integration with Existing Infrastructure

The outline panel reuses the existing dock and syntax worker infrastructure. **No new threads, no new panel chrome code.**

```
┌─────────────────────────────────────────────────┐
│  Existing Infrastructure (already built)         │
│  ┌────────────┐  ┌──────────┐  ┌─────────────┐ │
│  │ DockLayout │  │ DockRects│  │  HitTarget  │ │
│  │ + Dock     │  │ geometry │  │  hit_test   │ │
│  └────────────┘  └──────────┘  └─────────────┘ │
│  ┌────────────┐  ┌──────────┐  ┌─────────────┐ │
│  │ FocusTarget│  │ DockMsg  │  │ Resize drag │ │
│  │ + routing  │  │ handling │  │  + borders  │ │
│  └────────────┘  └──────────┘  └─────────────┘ │
│  ┌────────────┐  ┌──────────┐                   │
│  │ Syntax     │  │ParserSta-│                   │
│  │ Worker     │  │te + Tree │                   │
│  │ Thread     │  │ Cache    │                   │
│  └────────────┘  └──────────┘                   │
└─────────────────────────────────────────────────┘
                      │
          ┌───────────▼───────────┐
          │  New: Outline Engine  │
          │  ┌─────────────────┐  │
          │  │ outline.scm     │  │
          │  │ queries (per    │  │
          │  │ language)       │  │
          │  └────────┬────────┘  │
          │           │           │
          │  ┌────────▼────────┐  │
          │  │ OutlineData     │  │
          │  │ (returned with  │  │
          │  │  highlights)    │  │
          │  └─────────────────┘  │
          └───────────────────────┘
```

### Event Flow

Matches Token's existing Elm architecture. Outline extraction runs on the **same syntax worker thread** that already caches tree-sitter `Tree` per document:

```
Document Edit
    │
    ▼
Cmd::DebouncedSyntaxParse (existing 30ms debounce)
    │
    ▼
Syntax Worker Thread:
    1. Parse / incremental re-parse → Tree
    2. Extract highlights (existing)
    3. Extract outline (NEW) → OutlineData
    │
    ▼
Msg::Syntax(ParseCompleted { highlights, outline })
    │
    ▼
update_syntax():
    1. Store highlights on Document (existing)
    2. Store outline on Document (NEW)
    → Cmd::Redraw
    │
    ▼
render_dock(): iterate OutlineData tree
    → fontdue glyph rendering with expand/collapse
```

### Module Structure

```
src/
├── outline/
│   ├── mod.rs              # OutlineData, OutlineNode, OutlineKind exports
│   ├── extract.rs          # extract_outline() — runs on worker thread
│   └── queries.rs          # Embedded outline.scm queries per language
├── panels/
│   ├── outline.rs          # Outline panel rendering + interaction (replaces placeholder)
│   └── ...
├── update/
│   └── outline.rs          # OutlineMsg handler
├── messages.rs             # OutlineMsg variants
queries/
├── rust/outline.scm        # Rust symbol queries
├── typescript/outline.scm  # TypeScript symbol queries
├── python/outline.scm      # Python symbol queries
├── go/outline.scm          # Go symbol queries
├── markdown/outline.scm    # Markdown heading queries
└── ...                     # Added per language as needed
```

---

## Data Structures

### OutlineData (per Document)

Derived data stored on `Document` alongside `syntax_highlights`. Computed by the syntax worker thread.

```rust
// In src/outline/mod.rs

/// Symbol kind for display and icon selection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutlineKind {
    // Markdown
    Heading { level: u8 },   // H1=1, H2=2, ..., H6=6

    // Code symbols
    Module,
    Class,
    Struct,
    Enum,
    Interface,
    Trait,
    Function,
    Method,
    Property,
    Field,
    Constant,
    Variable,
    EnumVariant,
    Constructor,
    Impl,                    // Rust impl blocks
    Namespace,
}

/// A range in the document (char-based, not byte-based)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OutlineRange {
    pub start_line: usize,
    pub start_col: usize,
    pub end_line: usize,
    pub end_col: usize,
}

/// A single node in the outline tree
#[derive(Debug, Clone)]
pub struct OutlineNode {
    /// What kind of symbol this is
    pub kind: OutlineKind,
    /// Display name (e.g., "main", "User", "## Installation")
    pub name: String,
    /// Location in the document
    pub range: OutlineRange,
    /// Child symbols (methods inside class, etc.)
    pub children: Vec<OutlineNode>,
}

/// Complete outline for a document
#[derive(Debug, Clone)]
pub struct OutlineData {
    /// Document revision this outline was computed from
    pub revision: u64,
    /// Root-level symbols
    pub roots: Vec<OutlineNode>,
}
```

### OutlinePanelState (UI state, on AppModel)

Separate from document data. Tracks expand/collapse, selection, and scroll for the outline panel.

```rust
// In src/model/ui.rs or src/outline/mod.rs

/// UI state for the outline panel
#[derive(Debug, Clone, Default)]
pub struct OutlinePanelState {
    /// Collapsed node keys — nodes are expanded by default
    /// Key: (kind discriminant, name, start_line) for stable identity
    pub collapsed: HashSet<(u8, String, usize)>,
    /// Index of selected item in flattened visible list
    pub selected_index: Option<usize>,
    /// Scroll offset (in items)
    pub scroll_offset: usize,
    /// Whether to auto-highlight the symbol at cursor position
    pub follow_cursor: bool,
}

impl OutlinePanelState {
    pub fn node_key(node: &OutlineNode) -> (u8, String, usize) {
        let kind_disc = match node.kind {
            OutlineKind::Heading { .. } => 0,
            OutlineKind::Module => 1,
            OutlineKind::Class => 2,
            OutlineKind::Struct => 3,
            OutlineKind::Function => 5,
            OutlineKind::Method => 6,
            // ... etc
            _ => 255,
        };
        (kind_disc, node.name.clone(), node.range.start_line)
    }

    pub fn is_collapsed(&self, node: &OutlineNode) -> bool {
        self.collapsed.contains(&Self::node_key(node))
    }

    pub fn toggle_collapsed(&mut self, node: &OutlineNode) {
        let key = Self::node_key(node);
        if !self.collapsed.remove(&key) {
            self.collapsed.insert(key);
        }
    }
}
```

### Storage Location

```rust
// In Document (src/model/document.rs) — add alongside syntax_highlights
pub outline: Option<OutlineData>,

// In AppModel (src/model/mod.rs) — add UI state
pub outline_panel: OutlinePanelState,
```

---

## Outline Extraction

### Approach: Tree-Sitter Queries + Range Containment Nesting

Symbol extraction uses **tree-sitter queries** (one `outline.scm` per language) for declarative, maintainable language support. Hierarchy is built by **range containment** — a generic algorithm that works across all languages.

### Query Format

Each language gets an `outline.scm` with patterns that capture `@sym.def` (the full scope node) and `@sym.name` (the display name). The `OutlineKind` is inferred from the pattern index.

Example — `queries/rust/outline.scm`:
```scm
; Functions
(function_item
  name: (identifier) @sym.name) @sym.def

; Structs
(struct_item
  name: (type_identifier) @sym.name) @sym.def

; Enums
(enum_item
  name: (type_identifier) @sym.name) @sym.def

; Enum variants
(enum_variant
  name: (identifier) @sym.name) @sym.def

; Impl blocks
(impl_item
  type: (type_identifier) @sym.name) @sym.def

; Traits
(trait_item
  name: (type_identifier) @sym.name) @sym.def

; Constants
(const_item
  name: (identifier) @sym.name) @sym.def

; Statics
(static_item
  name: (identifier) @sym.name) @sym.def

; Modules
(mod_item
  name: (identifier) @sym.name) @sym.def
```

Example — `queries/markdown/outline.scm`:
```scm
(atx_heading) @sym.def
```

### Range Containment Algorithm

After collecting a flat list of symbols from query matches, build hierarchy:

```rust
fn build_tree(mut symbols: Vec<FlatSymbol>) -> Vec<OutlineNode> {
    // Sort by (start_byte asc, end_byte desc) — parents before children
    symbols.sort_by_key(|s| (s.start_byte, std::cmp::Reverse(s.end_byte)));

    let mut roots: Vec<OutlineNode> = Vec::new();
    let mut stack: Vec<OutlineNode> = Vec::new();

    for sym in symbols {
        let node = OutlineNode {
            kind: sym.kind,
            name: sym.name,
            range: sym.range,
            children: Vec::new(),
        };

        // Pop items off stack that don't contain this node
        while let Some(top) = stack.last() {
            if top.range.end_line >= node.range.start_line
               && top.range.start_line <= node.range.start_line {
                break; // current top contains this node
            }
            let finished = stack.pop().unwrap();
            if let Some(parent) = stack.last_mut() {
                parent.children.push(finished);
            } else {
                roots.push(finished);
            }
        }

        stack.push(node);
    }

    // Flush remaining stack
    while let Some(finished) = stack.pop() {
        if let Some(parent) = stack.last_mut() {
            parent.children.push(finished);
        } else {
            roots.push(finished);
        }
    }

    roots
}
```

### Markdown Heading Hierarchy

Markdown headings use level-based nesting rather than range containment, since headings don't have explicit scope ranges:

```rust
fn build_heading_tree(headings: Vec<(u8, String, usize)>) -> Vec<OutlineNode> {
    // Stack-based: H1 contains H2, H2 contains H3, etc.
    // A heading at level N closes all open headings at level >= N
}
```

### Language Support Priority

| Phase | Languages | Notes |
|-------|-----------|-------|
| MVP | Markdown, Rust | Core languages for dogfooding |
| Fast-follow | TypeScript, JavaScript, Python | Most popular |
| Extended | Go, Java, C, C++, PHP | Remaining supported languages |
| Fallback | All others | No outline (panel shows "No outline available") |

---

## Messages

```rust
// In src/messages.rs — add to existing Msg enum

pub enum OutlineMsg {
    /// Click on outline node → jump to symbol
    JumpToSymbol { line: usize, col: usize },

    /// Toggle expand/collapse of a node
    ToggleNode { line: usize, name: String },

    /// Keyboard navigation within outline panel
    SelectPrevious,
    SelectNext,

    /// Expand selected node (ArrowRight)
    ExpandSelected,
    /// Collapse selected node (ArrowLeft)
    CollapseSelected,

    /// Open/jump to selected node (Enter)
    OpenSelected,

    /// Scroll the outline panel
    Scroll { lines: i32 },
}
```

**Note**: Panel toggle/focus is handled by existing `DockMsg::FocusOrTogglePanel(PanelId::Outline)` — not duplicated here.

---

## Keybindings

| Action | Key (when outline focused) | Handler |
|--------|---------------------------|---------|
| Toggle outline | Cmd+7 | `DockMsg::FocusOrTogglePanel(PanelId::Outline)` (existing) |
| Navigate up | Arrow Up | `OutlineMsg::SelectPrevious` |
| Navigate down | Arrow Down | `OutlineMsg::SelectNext` |
| Expand node | Arrow Right | `OutlineMsg::ExpandSelected` |
| Collapse node | Arrow Left | `OutlineMsg::CollapseSelected` |
| Jump to symbol | Enter | `OutlineMsg::OpenSelected` |
| Return to editor | Escape | `UiMsg::FocusEditor` (existing) |

All keybindings only active when `FocusTarget::Dock(DockPosition::Right)` and active panel is Outline.

---

## Implementation Plan

### Phase 1: Data Model + Extraction Pipeline (1–2 days)

**Goal**: Extract outline data on the syntax worker thread and store it on `Document`.

1. [ ] Create `src/outline/mod.rs` with `OutlineData`, `OutlineNode`, `OutlineKind`, `OutlineRange`
2. [ ] Create `src/outline/extract.rs` with `extract_outline()` function
3. [ ] Create `queries/rust/outline.scm` and `queries/markdown/outline.scm`
4. [ ] Create `src/outline/queries.rs` — embed and compile outline queries per language
5. [ ] Add `outline: Option<OutlineData>` to `Document`
6. [ ] Extend syntax worker to run outline extraction after highlight extraction
7. [ ] Extend `SyntaxMsg::ParseCompleted` to include `Option<OutlineData>`
8. [ ] Update `update_syntax()` to store outline on `Document`
9. [ ] Implement markdown heading hierarchy (level-based nesting)
10. [ ] Implement range-containment nesting for code languages

**Verification**: `make build` compiles. Unit test parses a Rust file and extracts expected symbols. Unit test parses markdown and extracts heading hierarchy.

### Phase 2: Outline Panel Rendering (1–2 days)

**Goal**: See the outline tree in the right dock panel.

1. [ ] Add `OutlinePanelState` to `AppModel`
2. [ ] Create `src/panels/outline.rs` — outline panel renderer
3. [ ] Replace `PlaceholderPanel` rendering for `PanelId::Outline` in `render_dock()`
4. [ ] Render outline tree using recursive `render_node` pattern (same as sidebar file tree):
   - Indentation based on tree depth
   - `+`/`-` expand/collapse indicators for nodes with children
   - Symbol kind indicator (e.g., `fn`, `struct`, `mod`, `#` for headings)
   - Symbol name
   - Selection highlight for selected node
5. [ ] Compute visible items from dock rect + row height
6. [ ] Handle scroll offset for long outlines
7. [ ] Show "No outline available" when document has no outline data

**Verification**: Open a Rust file, press Cmd+7, see outline tree with functions/structs. Open a markdown file, see heading hierarchy.

### Phase 3: Interaction (1 day)

**Goal**: Click symbols to jump, keyboard navigation, expand/collapse.

1. [ ] Add `OutlineMsg` to `src/messages.rs` and `Msg::Outline` variant
2. [ ] Create `src/update/outline.rs` — OutlineMsg handler
3. [ ] Implement click-to-jump: click a symbol → move cursor to that line, focus editor
4. [ ] Implement keyboard navigation (Up/Down/Left/Right/Enter) when outline panel is focused
5. [ ] Implement expand/collapse toggle on click or keyboard
6. [ ] Add hit-testing for outline panel items in dock content area

**Verification**: Click a function in outline → cursor jumps to it. Arrow keys navigate the tree. Enter jumps to selected symbol.

### Phase 4: Follow Cursor + More Languages (1 day)

**Goal**: Outline highlights current symbol, add more language queries.

1. [ ] Implement follow-cursor: when cursor moves, find the containing symbol and highlight it in outline
2. [ ] Auto-scroll outline to keep highlighted symbol visible
3. [ ] Add `queries/typescript/outline.scm`
4. [ ] Add `queries/javascript/outline.scm`
5. [ ] Add `queries/python/outline.scm`
6. [ ] Add `queries/go/outline.scm`

**Verification**: Type in a function body, see that function highlighted in outline. Switch between TypeScript/Python files, see correct outlines.

### Post-MVP: Extended Language Coverage

Not part of initial implementation. Add as needed:

1. [ ] Java, C, C++, PHP outline queries
2. [ ] Outline filtering / search
3. [ ] Sort by name vs position toggle
4. [ ] Symbol kind icons (when icon rendering is available)

---

## Testing Strategy

### Unit Tests

```rust
#[test]
fn test_rust_outline_extraction() {
    let source = r#"
struct User { name: String }
impl User {
    fn new(name: String) -> Self { User { name } }
    fn greet(&self) -> String { format!("Hi, {}", self.name) }
}
fn main() { let u = User::new("Alice".into()); }
"#;
    let outline = extract_outline(source, LanguageId::Rust);
    assert_eq!(outline.roots.len(), 3); // struct User, impl User, fn main
    let impl_node = &outline.roots[1];
    assert_eq!(impl_node.children.len(), 2); // fn new, fn greet
}

#[test]
fn test_markdown_heading_hierarchy() {
    let source = "# Title\n## Section 1\n### Sub 1.1\n## Section 2\n### Sub 2.1\n";
    let outline = extract_outline(source, LanguageId::Markdown);
    assert_eq!(outline.roots.len(), 1); // # Title
    assert_eq!(outline.roots[0].children.len(), 2); // ## Section 1, ## Section 2
    assert_eq!(outline.roots[0].children[0].children.len(), 1); // ### Sub 1.1
}

#[test]
fn test_range_containment_nesting() {
    // Flat list → correctly nested tree
    let symbols = vec![
        flat_sym("MyClass", 0, 20),  // lines 0-20
        flat_sym("method_a", 2, 8),  // lines 2-8 (inside MyClass)
        flat_sym("method_b", 10, 18), // lines 10-18 (inside MyClass)
    ];
    let tree = build_tree(symbols);
    assert_eq!(tree.len(), 1);
    assert_eq!(tree[0].children.len(), 2);
}

#[test]
fn test_outline_panel_state_collapse() {
    let mut state = OutlinePanelState::default();
    let node = OutlineNode { /* ... */ };
    assert!(!state.is_collapsed(&node));
    state.toggle_collapsed(&node);
    assert!(state.is_collapsed(&node));
    state.toggle_collapsed(&node);
    assert!(!state.is_collapsed(&node));
}
```

### Manual Testing Checklist

- [ ] Cmd+7 opens outline panel in right dock
- [ ] Rust file shows structs, functions, impl blocks hierarchically
- [ ] Markdown file shows headings as `# > ## > ###` hierarchy
- [ ] Click symbol → cursor jumps to definition line
- [ ] Arrow keys navigate outline items
- [ ] Left/Right expand/collapse nodes
- [ ] Escape returns focus to editor
- [ ] Scrolling works for files with many symbols
- [ ] Switching tabs updates outline for new document
- [ ] Empty/unsupported files show "No outline available"
- [ ] Cursor position highlights containing symbol in outline

---

## Platform Considerations

No platform-specific code needed. The outline panel uses the same fontdue text rendering and softbuffer pixel buffer as the rest of the editor.

---

## Dependencies

No new crate dependencies needed. The outline panel uses:

- `tree-sitter` (already a dependency) — for query compilation and cursor iteration
- Existing tree-sitter grammar crates (already dependencies) — for AST access
- Existing fontdue renderer — for text rendering
- Existing dock panel infrastructure — for panel chrome

The only new files are outline query files (`queries/<lang>/outline.scm`) which are plain text embedded at compile time via `include_str!`.

---

## References

- Tree-sitter query syntax: https://tree-sitter.github.io/tree-sitter/using-parsers/queries
- Tree-sitter tags queries (prior art): https://tree-sitter.github.io/tree-sitter/code-navigation-systems
- VS Code outline view: https://code.visualstudio.com/docs/getstarted/userinterface#_outline-view
- Zed outline implementation: uses tree-sitter queries for symbol extraction
- Token sidebar rendering: `src/view/mod.rs` `render_sidebar()` — pattern to follow for tree rendering
