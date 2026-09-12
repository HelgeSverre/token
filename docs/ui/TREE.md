# Tree

## Purpose and boundary

A tree displays hierarchical nodes with expanded/collapsed state and a
flattened visible projection. Use it for workspace files and outline hierarchy,
not a flat List or Table. Token shares traversal; node data, expansion,
selection, opening, and context actions remain feature-owned.

## Verified implementation and consumers

Workspace owns `FileTree`, roots, selected item, and expanded folders
(`src/model/workspace.rs:226-285`). Its update paths derive visible count,
selection and scroll after workspace changes (`src/update/workspace.rs:240-420`).
`render_tree(roots, RowListView, is_expanded, render_row)` performs preorder:
it records node/depth/display index/physical row y, walks ancestors above the
viewport, and calls row paint only inside the solved `drawn_range`
(`src/view/tree_view.rs:1-87`). Tests prove visible-window clipping and a nested
row remains reachable when ancestors were skipped (`src/view/tree_view.rs:89-192`).

| Consumer            | Model owner          | Current pointer/keyboard workflow                           |
| ------------------- | -------------------- | ----------------------------------------------------------- |
| Workspace file tree | `Workspace`          | click selects; double-click toggles directory or opens file |
| Outline dock        | outline model/update | domain-specific scroll/select/jump                          |

File-tree pointer behavior is explicit: double click toggles `WorkspaceMsg::ToggleFolder`
for a directory, opens a file otherwise, and focuses the left dock
(`src/runtime/mouse.rs:2180-2213`). With sidebar focus, Up/Down move selection;
Right expands a collapsed directory or moves next (`src/runtime/input.rs:1155-1185`).
The file-tree context target includes path and `is_dir` (`src/context_menu/types.rs:18-47`).

## Data ownership and state transitions

**Verified shared data:** `TreeRow` is borrowed node + depth + display index +
physical y (`src/view/tree_view.rs:8-15`); `TreeNodeLike` only supplies children.
The caller decides expansion. No shared tree owns selected node or effects.

**Proposed contract:**

| Data                                             | Owner             | Invariant                                           |
| ------------------------------------------------ | ----------------- | --------------------------------------------------- |
| stable `NodeId`, children, label/kind/load state | feature model     | every visible projection item has one current ID    |
| expanded IDs and selected ID                     | feature model     | selected node is visible or reconciled              |
| `RowListView`, flattened order                   | layout/projection | paint, hit, scroll, keyboard use exactly this order |
| open/toggle/context messages                     | feature update    | tree helper never does I/O                          |

Transition: data/expanded set changes → make visible projection → retain selected
ID if visible, otherwise closest selectable ancestor/sibling → solve row layout
→ pointer/key yields current `NodeId` → owner update toggles/opens/jumps →
recompute projection and reveal selection. This prevents stale display-index
activation after refresh/filter.

## Keyboard, pointer, focus, accessibility

**Verified:** sidebar focus consumes keys rather than falling through to editor
(`src/runtime/input.rs:253-285`); no verified Left-collapse, Home/End,
type-ahead, multi-select, drag/reorder, semantic tree roles, or focus ring.

**Proposed:** Right expands then first child; Left collapses or moves parent;
Up/Down adjacent visible node; Home/End endpoints; Enter only performs a
declared feature default. Disclosure glyph hit targets must differ from row
activation. Context-click selection policy must be explicit. Supply tree/treeitem
semantics (level, expanded, selected, disabled, accessible label) and focus
feedback independent of selected color.

## Geometry, theme, edge cases

Use solved `RowListView` rows—never `index × guessed height`. File tree row
height/indent are scaled metrics (`src/model/mod.rs:241-302`). Clip names,
preserve surface theme roles, and use Code font only where the owning surface
intentionally does. Test partial first/last row, deep nesting, empty/loading/
error states, duplicate labels, collapsed selected descendant, async refresh,
and cycle/symlink policy.

## Gallery, priorities, acceptance

Gallery has empty dock chrome but no live tree specimen (`src/model/gallery.rs:140-145`).
Priority 1: depth 0–3, expanded/collapsed, selected/hover/focus, long filename,
scroll window, empty/loading/error. Priority 2: ID-based shared projection only
if workspace and outline can consume it without state loss. Acceptance: one
projection drives painter/hit/keyboard; collapse cannot retain invisible
selection; virtualization tests cover scrolled nested nodes; accessible state is
observable.

## Sources

Primary: [IntelliJ list and tree controls](https://plugins.jetbrains.com/docs/intellij/lists-and-trees.html).
Secondary local context: `temporary-docs/intellij-platform-sdk/references/ui-settings-and-toolwindows.md`.
