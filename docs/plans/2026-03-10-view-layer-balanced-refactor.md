# View Layer Balanced Refactor

**Status:** In progress  
**Created:** 2026-03-10

## Summary

This plan keeps the current immediate-mode renderer (`Frame` + `TextPainter` + feature-specific render functions) and improves consistency by centralizing layout, extracting a small set of shared UI primitives, and removing places where render and hit-test compute different geometry.

The goal is to land this in small, low-risk slices that also fix known UI inconsistencies:

- Splitter hover/cursor mismatch from render and hit-test using different editor-area bounds
- Scrollbar sizing drift between panes because viewport capacity is not always derived from actual group layout
- Repeated modal shell/input/list code
- Repeated tree rendering code between sidebar and outline

## Existing Docs To Fold In

- [`docs/POTENTIAL-REFACTORS.md`](../POTENTIAL-REFACTORS.md)
- [`docs/archived/panel-ui-abstraction.md`](../archived/panel-ui-abstraction.md)
- [`docs/archived/tree-navigation-abstraction.md`](../archived/tree-navigation-abstraction.md)
- [`docs/feature/DAMAGE-TRACKING.md`](../feature/DAMAGE-TRACKING.md)
- [`docs/plans/2026-02-27-sidebar-clipping.md`](./2026-02-27-sidebar-clipping.md)

These already point toward the same direction: layout should be shared, trees should be unified, and render work should be split into reusable building blocks instead of feature-local ad hoc code.

## Target Architecture

### 1. Shared Window And Pane Layout

Add a single layout helper that computes:

- status bar rect
- sidebar rect
- right dock rect
- bottom dock rect
- editor-area rect

Use that helper in:

- render orchestration
- hit-testing
- splitter drag setup
- future overlay positioning

This removes the current drift where render, hit-test, and drag code each reconstruct the screen differently.

### 2. Per-Pane Viewport Sync

Treat group rects as the source of truth for visible rows/columns.

- Re-sync editor viewport capacities from actual group rectangles after layout computation
- Use scaled metrics, not legacy default metrics, for visible column calculation
- Derive scrollbar thumb sizing from actual pane capacity, not stale approximations

This is the first practical fix for the split-pane scrollbar inconsistencies.

### 3. Small Reusable View Primitives

Introduce or strengthen these primitives before any larger widget system:

- `PaneShell`: pane chrome with header, border, content inset
- `InputField`: wraps `TextFieldRenderer` with standard background/padding/focus styling
- `SelectableList`: shared row list for command palette, file finder, recent files, theme picker
- `TreeView`: shared rendering/hit-test/navigation structure for sidebar and outline
- `RenderContext`: frame, painter, theme, metrics, cursor visibility

These should replace repeated setup code, not add another layer of wrappers with no ownership.

### 4. Shared Layout Outputs For Render + Hit-Test

Where interactive geometry matters, compute it once and reuse it:

- tab strip layout
- scrollbar track/thumb layout
- modal content/list row layout
- tree row + chevron layout

This is the main maintainability lever for the current codebase.

## Recommended Implementation Order

### Phase 1: Layout Consistency

- Add shared window/editor-area layout helper
- Use it in render + hit-test
- Sync editor viewport capacities from actual group layout
- Fix splitter hover/cursor and split-pane scrollbar sizing issues

### Phase 2: Modal Cleanup

- Split `render_modals()` into per-modal functions
- Add `InputField`
- Add `SelectableList`
- Move modal shell rendering to shared pane/card helpers

### Phase 3: Tree Cleanup

- Introduce shared tree row layout/render helpers
- Align sidebar and outline rendering
- Align tree hit-testing with tree layout
- Reuse tree navigation abstractions from the existing tree-navigation plan

### Phase 4: Renderer Structure Cleanup

- Split `src/view/mod.rs` into orchestration vs reusable chrome/widgets
- Reduce duplicate editor text rendering paths where possible
- Move more geometry-specific helpers out of feature renderers

## Explicit Non-Goals

- No immediate full widget tree rewrite
- No large input-routing redesign in the first pass
- No broad theme-system rewrite beyond removing hardcoded colors while touching the affected code
- No damage-tracking redesign beyond keeping new layout usage compatible with existing damage regions

## Risks

- Viewport sync changes can expose old assumptions in scrolling code
- Shared layout helpers can accidentally lock in current dock/sidebar compromises if we overfit them to today’s structure
- The editor fast path (`render_cursor_lines_only`) still duplicates some full-render logic and will need a later pass

## First Slice

Start with layout consistency and viewport sync:

1. Shared window/editor-area layout helper
2. Use the helper in render and hit-test
3. Recompute viewport capacities from actual group rects during render
4. Base scrollbar geometry on actual pane capacity

This fixes real bugs immediately and creates the foundation for the rest of the balanced refactor.
