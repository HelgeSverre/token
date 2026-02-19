# Tree Navigation Abstraction

Unified keyboard/mouse navigation for tree-style panels: outline, file explorer, todo list, and future tree views.

> **Status:** Planned
> **Priority:** P2
> **Effort:** M (3–5 days)
> **Created:** 2026-02-19
> **Updated:** 2026-02-19
> **Milestone:** 3 - Workspace Features

---

## Overview

### Current State

The editor has multiple tree-style panels that each implement navigation independently:

- **File explorer** (left dock): Full arrow key navigation (Up/Down to move selection, Left/Right to collapse/expand, Enter to open). Implemented in `src/update/workspace.rs` with `WorkspaceMsg::SelectPrevious`, `SelectNext`, `OpenOrToggle`, etc.
- **Outline panel** (right dock): Mouse click and double-click working. Keyboard navigation messages exist (`OutlineMsg::SelectPrevious/Next`, `ExpandSelected`, `CollapseSelected`, `OpenSelected`) but **no keybinding dispatch** connects arrow keys to these messages when the outline panel is focused.
- **Future panels** (todo list, bookmarks, search results): Will need the same tree navigation pattern.

### Problem

Each panel re-implements the same navigation logic:
- `count_visible_items()` / `node_at_index()` tree flattening
- Scroll-into-view when selection moves past viewport edges
- Expand/collapse toggle on Left/Right arrows
- Enter to activate/open
- Escape to return focus to editor

This creates duplication and inconsistency. The outline panel currently has no keyboard navigation because the key dispatch in `src/runtime/input.rs` doesn't route arrow keys to `OutlineMsg` when `FocusTarget::Dock(Right)` is active.

### Goals

1. **Immediate**: Wire arrow key dispatch to existing `OutlineMsg` handlers when outline panel is focused
2. **Short-term**: Extract a shared `TreeNavState` and `TreeNavMsg` abstraction
3. **Long-term**: New tree panels get navigation "for free" by implementing a `TreeDataSource` trait

### Non-Goals

- Drag-and-drop reordering within trees
- Multi-select in trees (future consideration)
- Virtualized rendering (current approach of skipping off-screen items is sufficient)

---

## Architecture

### Proposed Trait

```rust
/// Data source for a navigable tree view
trait TreeDataSource {
    type NodeId: Clone + Eq + Hash;

    fn root_count(&self) -> usize;
    fn root_ids(&self) -> Vec<Self::NodeId>;
    fn children(&self, id: &Self::NodeId) -> &[Self::NodeId];
    fn is_expandable(&self, id: &Self::NodeId) -> bool;
    fn node_label(&self, id: &Self::NodeId) -> &str;
}
```

### Shared Navigation State

```rust
/// Reusable state for any tree-navigable panel
struct TreeNavState {
    selected_index: Option<usize>,
    scroll_offset: usize,
    collapsed: HashSet<NodeKey>,
}
```

### Key Dispatch

The immediate fix (Phase 1) wires `FocusTarget::Dock(Right)` arrow keys to outline messages in `src/runtime/input.rs`:

| Key | Outline Action |
|-----|---------------|
| `↑` | `OutlineMsg::SelectPrevious` |
| `↓` | `OutlineMsg::SelectNext` |
| `→` | `OutlineMsg::ExpandSelected` |
| `←` | `OutlineMsg::CollapseSelected` |
| `Enter` | `OutlineMsg::OpenSelected` |
| `Escape` | Focus editor |

---

## Implementation Plan

### Phase 1: Wire Outline Arrow Keys (0.5 day)

Connect existing `OutlineMsg` handlers to keyboard input when the outline panel has focus.

**Files:**
- `src/runtime/input.rs` — Add key dispatch for `FocusTarget::Dock(Right)` when active panel is `PanelId::Outline`

### Phase 2: Extract TreeNavState (1–2 days)

Factor the shared navigation pattern out of `OutlinePanelState` and sidebar's `Workspace`:

- `src/tree_nav.rs` — `TreeNavState`, `TreeNavMsg` enum, `update_tree_nav()` pure function
- Update `OutlinePanelState` to wrap `TreeNavState`
- Update `Workspace` selection/scroll to use `TreeNavState`

### Phase 3: TreeDataSource Trait (1–2 days)

Define the trait and implement it for:
- `OutlineData` (outline nodes)
- `FileTree` (file explorer nodes)
- Future: `TodoList`, `SearchResults`, `Bookmarks`

### Phase 4: Unified Tree Renderer (optional, future)

Extract `render_tree_panel()` that takes a `TreeDataSource` + `TreeNavState` and handles:
- Indented rows with expand/collapse indicators
- Selection highlighting
- Scroll offset
- Hit-testing for clicks

---

## Dependencies

- Outline panel (✅ implemented)
- Dock focus system (✅ implemented)
- File explorer sidebar (✅ existing, to be refactored)

## References

- Current outline update: `src/update/outline.rs`
- Current sidebar update: `src/update/workspace.rs`
- Sidebar rendering context: `SidebarRenderContext` in `src/view/mod.rs`
- Outline rendering context: `OutlineRenderContext` in `src/view/mod.rs`
- Panel UI abstraction doc: `docs/feature/panel-ui-abstraction.md`
