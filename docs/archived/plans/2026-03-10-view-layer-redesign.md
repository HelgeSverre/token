# View Layer Redesign

**Status:** Superseded by `2026-03-10-view-rendering-consolidation.md`
**Created:** 2026-03-10

> Historical note: this redesign option was considered but not chosen. The active direction is now [`2026-03-10-view-rendering-consolidation.md`](./2026-03-10-view-rendering-consolidation.md), which keeps the renderer monolithic at the top and only adds domain seams where they clearly reduce real feature work.

## Summary

This is the larger redesign option for the editor UI layer. It replaces the current mix of shared primitives and feature-local drawing code with a formal component/layout system that unifies:

- layout
- rendering
- hit-testing
- reusable widgets

This option is for a future phase after the balanced refactor has either plateaued or proven too limiting.

## Why Consider It

The current architecture has good primitive pieces, but it still has these structural limits:

- render and hit-test often recompute the same geometry independently
- `src/view/mod.rs` mixes renderer orchestration, widget code, and feature-specific composition
- modals, lists, and trees repeat the same patterns with slightly different local logic
- some interactive types exist without a matching shared visual abstraction

If the UI surface area keeps growing, small refactors will keep paying off less.

## Target Model

### 1. Layout Tree

Build a lightweight layout tree or component tree where each node owns:

- its computed bounds
- optional children
- render behavior
- hit-test behavior

Possible shape:

```rust
trait Widget {
    fn layout(&self, cx: &UiLayoutCx, bounds: Rect) -> LayoutNode;
    fn render(&self, cx: &mut RenderCx, layout: &LayoutNode);
    fn hit_test(&self, layout: &LayoutNode, pt: Point) -> Option<UiTarget>;
}
```

This does not require retained-mode state for everything. It can still be immediate-mode in rendering, but geometry should become explicit and reusable.

### 2. Shared Screen Vocabulary

Replace ad hoc feature-specific composition with a small stable widget set:

- `PaneShell`
- `TabStrip`
- `SelectableList`
- `TreeView`
- `InputField`
- `LabeledField`
- `StatusSegments`
- `ScrollbarPair`
- `OverlayStack`
- `EmptyStateCard`
- `TextViewport`

### 3. Single Source Of Truth For Interactive Geometry

Every interactive surface should be laid out once and reused by both render and input:

- tab rectangles
- list row rectangles
- tree row/chevron rectangles
- scrollbar track/thumb rectangles
- modal shell/content rectangles
- dock header/content/resize rectangles

This removes a whole class of UI drift bugs.

### 4. Specialized Editor Viewport Layers

The text editor itself should still be optimized, but split into explicit layers:

- background/current-line layer
- selection layer
- text layer
- cursor layer
- scrollbar layer
- chrome layer

This keeps text rendering hot paths special without forcing every other UI surface to stay equally ad hoc.

## Migration Strategy

### Phase 1

- Introduce layout tree data structures
- Rebuild modal shells, pane shells, and lists on top of them

### Phase 2

- Move sidebar and outline to a shared tree widget
- Move docks to shared pane/header/tab abstractions

### Phase 3

- Move tab bars and scrollbars to shared components with shared layout outputs
- Make hit-test consume the same layout results

### Phase 4

- Convert the editor viewport composition to layered widgets while keeping text rasterization specialized

## Benefits

- Strong long-term consistency
- Much cheaper to add new panels and overlays
- Fewer geometry drift bugs
- Better separation between UI primitives and feature modules

## Costs

- Larger migration surface
- More churn in a sensitive subsystem
- Higher short-term regression risk
- Likely damage/invalidation work to match new component boundaries cleanly

## When To Choose This

Choose this if the editor is likely to add several more interactive panels, overlays, inspectors, side tools, inline widgets, or richer dock/tab behavior within the next few cycles.

If the goal is faster improvement with lower risk, use the balanced refactor first and preserve migration hooks for this redesign.
