# Splitter

A splitter is the adjustable boundary between adjacent children of one layout-tree
container. It is a layout control, not a visual separator. Token's current
splitter resizes editor groups and preview panes through the editor-area tree.

## Current implementation — verified

[Editor area model](../../src/model/editor_area.rs) contains a tree of
LayoutNode values: Empty, Group, Split and Preview. A SplitContainer has a
direction, ordered children, ratios and min_sizes. Horizontal means children
run left-to-right; vertical means top-to-bottom. Its child_rects helper is the
single traversal for group layout, splitter placement and splitter hit tests.

compute_layout_scaled stores the shell rectangle, recursively assigns each
group/preview rect, and derives a SplitterBar after every non-final child. A bar
has direction, rectangle and local boundary index. Its rectangle is centered on
the child boundary, using ScaledMetrics::splitter_width (base 6 logical pixels).
The render pass supplies this exact splitter list to Renderer::render_splitters;
a read-only compute_splitters supports input without cloning editor documents.

[Layout update](../../src/update/layout.rs) begins a drag by resolving the
same shell rectangle and splitter list, then records SplitterDragState:

| State field               | Meaning                              |
| ------------------------- | ------------------------------------ |
| splitter_index            | global depth-first splitter identity |
| local_index               | boundary index in owning container   |
| start_position            | physical-pixel press position        |
| original_ratios           | cancellation baseline                |
| direction, container_size | active axis and ratio denominator    |
| active                    | true only after the drag threshold   |

Updates modify only the two adjacent original ratios. The combined share remains
constant; a minimum target is calculated from MIN_PANE_SIZE_PIXELS, with an
epsilon fallback when the container cannot physically satisfy both minimums.
End drops the drag state. Escape/cancel restores the entire original ratio vector.
Tests in [tests/editor_area.rs](../../tests/editor_area.rs) prove tree placement
and boundary hit testing.

## Anatomy and ownership

A splitter has a visual bar, a hit/capture region, a preceding child, following
child, orientation and drag affordance. The current bar uses one rectangle for
painting and hit testing; its apparent background comes from Theme::splitter.

| Concern                      | Current owner                    | Reuse rule                      |
| ---------------------------- | -------------------------------- | ------------------------------- |
| child ordering and ratios    | EditorArea / SplitContainer      | never duplicate in view/runtime |
| group and preview rectangles | EditorArea layout traversal      | derive from same tree           |
| bar geometry                 | SplitContainer::splitter_after   | use it for render and hit test  |
| press/update/cancel state    | UiState::splitter_drag           | state remains model-owned       |
| ratio transition             | update/layout.rs                 | deterministic, no platform work |
| pointer dispatch/cursor      | runtime/mouse.rs and hit_test.rs | dispatch typed Splitter target  |
| bar painter                  | Renderer::render_splitters       | no lookalike local divider      |
| colors/scaling               | Theme::splitter, ScaledMetrics   | no hard-coded width/color       |

The app does not currently provide a generic splitter usable by arbitrary
containers. That is appropriate: dock/sidebar resize handles have their own
configuration units, persistence and constraints. Share mechanics only after
their state and geometry semantics genuinely align.

## Event transitions — verified

1. Pointer press on typed Splitter target sends BeginSplitterDrag and records
   original ratios. It does not move a boundary immediately.
2. Pointer move computes Euclidean distance. Below DRAG_THRESHOLD_PIXELS, the
   drag remains inactive; after threshold, only the divider's axis contributes
   to ratio delta.
3. Each active update recomputes the two shares from the press-time ratio vector,
   not accumulated floating-point mutation, then redraws.
4. Pointer release sends EndSplitterDrag and commits the current in-memory
   ratios. There is no disk/session persistence effect at this boundary today.
5. Escape sends CancelSplitterDrag and restores originals. An invalid/mutated
   tree makes an update a safe no-op.

The model state is capture, but this is not yet a fully documented OS-level
pointer-capture integration. The runtime must keep routing movement/release
while splitter_drag exists; this needs focused interaction coverage.

## Focus, keyboard and accessibility

A splitter press is consumed; current left-click handling does not explicitly
move FocusTarget, so editor keyboard focus normally remains intact. Hover
identifies HoverRegion::Splitter and changes pointer affordance through
[hit testing](../../src/view/hit_test.rs). Splitters have no current keyboard
focus, arrow-key resize, semantic range/value, screen-reader role, double-click
reset, or focus-loss cancellation policy.

**Proposed contract:** a keyboard-accessible splitter is a focusable separator
with orientation, accessible name and current proportional/physical value. Arrow
keys adjust the active axis by a documented scaled step; Shift or Page variants
may use a larger step; Home/End reset to documented bounds; Escape restores the
press baseline. Pointer and keyboard must use the same ratio/minimum formula.
Until these are implemented, state the control as pointer-only and preserve a
non-pointer path to all essential layouts.

## Geometry constraints and edge cases

- Ratios are intended to sum to one, but missing values use equal-share fallback.
  A normalizing/validation strategy is proposed before arbitrary nesting grows.
- Splitter identity is traversal-order index; changing the tree during a drag
  can invalidate it. Current update defensively ignores invalid indices.
- The model stores min_sizes but compute_layout_node does not enforce them; drag
  has an independent 100px-style safeguard. This discrepancy is an existing
  gap and the highest correctness risk in this component.
- Tiny containers cannot satisfy both adjacent minima. Current epsilon prevents
  zero/negative children, but does not make panes usable.
- The bar overlays the boundary; child ratios still consume the full container
  extent. Do not subtract bar width in another geometry path.
- Nested splitters are depth-first and each bar is local to its owning container.
- Display-scale changes rebuild ScaledMetrics. Starting a drag with old physical
  metrics and continuing after scale change is not explicitly reconciled today.
- Preview nodes share this layout tree but are not editor groups; group focus
  lookup must not treat a preview as editable content.

## Gallery coverage and recommended implementation slice

Gallery provides static splitter.horizontal and splitter.vertical specimens,
painted as boundaries. It lacks nested ratios, hover/cursor state, threshold
before activation, drag result, cancellation rollback, constrained minimum size,
light/dark scale snapshots and a group composition that visibly changes size.

The next useful slice is not a new widget framework: extract/verify a
SplitLayout fixture from the real EditorArea traversal, then add production
tests for nested drag, scale transition and the min_sizes discrepancy. Add a
native gallery interaction only after the same production hit geometry can drive
it. Do not encode a visual splitter as a fresh local rectangle.

## Acceptance criteria and evidence

A change is acceptable when child rects, bar paint and hit testing share the
tree traversal; press-time state can cancel exactly; active updates preserve
adjacent combined share and non-negative sizes; all direction/scale/tiny-layout
paths are tested; theme splitter roles render in light/dark; and gallery uses
the production bar painter.

Implementation claims are high confidence from
[editor_area.rs](../../src/model/editor_area.rs),
[ui.rs](../../src/model/ui.rs), [layout update](../../src/update/layout.rs),
[runtime mouse dispatch](../../src/runtime/mouse.rs),
[renderer](../../src/view/mod.rs), [theme](../../src/theme.rs), and
[editor-area tests](../../tests/editor_area.rs), reviewed 2026-09-12. Proposed
keyboard/accessibility behavior and normalized min-size policy are not present.
