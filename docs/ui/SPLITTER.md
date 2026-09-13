# Splitter

A splitter is the movable boundary of an editor-area `SplitContainer`. It is not
a generic divider: it owns neither panes, focus, nor persistence. This chapter
specifies the current layout-tree implementation and its real constraint gap.
See [the editor reference's split model](../EDITOR_UI_REFERENCE.md#chapter-2-structural-hierarchy-tabs-splits-and-editor-groups) for shared terminology.

## Representation and ownership

**Current excerpt** — [`src/model/editor_area.rs`](../../src/model/editor_area.rs)

```rust
pub enum LayoutNode {
    Empty,
    Group(GroupId),
    Split(SplitContainer),
    Preview(PreviewId),
}

pub struct SplitContainer {
    pub direction: SplitDirection,
    pub children: Vec<LayoutNode>,
    pub ratios: Vec<f32>,
    pub min_sizes: Vec<f32>,
}

pub struct SplitterBar {
    pub direction: SplitDirection,
    pub rect: Rect,
    pub index: usize,
}
```

`EditorArea::layout` durably owns the tree. `GroupId` and `PreviewId` resolve to
separately owned panes. `Horizontal` lays children left-to-right and uses X;
`Vertical` lays them top-to-bottom and uses Y. `children` is visual order.
`ratios` are intended shares of the full active-axis physical-pixel length.
`min_sizes` are physical pixels and nominally parallel `children`.

`SplitterBar` is derived presentation/hit geometry. Its `index` is a _local_
boundary: child `index` precedes it. The returned bar vector also has a global
depth-first emission index used by drag messages. Neither is durable identity:
editing an ancestor or sibling changes global order. More importantly, that
global index is not currently a sound drag identity for arbitrary nesting; the
emission and lookup traversals disagree (documented under "Traversal mismatch").

`Rect` is `f32` physical pixels, lower-inclusive and upper-exclusive. The
mutating layout traversal stores group/preview rects and `last_layout_rect`; the
read-only traversal derives bars for input without cloning documents/undo data.

**Current transient capture** — [`src/model/ui.rs`](../../src/model/ui.rs)

```rust
pub struct SplitterDragState {
    pub splitter_index: usize,
    pub local_index: usize,
    pub start_position: (f32, f32),
    pub original_ratios: Vec<f32>,
    pub direction: SplitDirection,
    pub container_size: f32,
    pub active: bool,
}
```

`UiState::splitter_drag` is a press-time transaction record. `original_ratios`
is the full cancellation baseline, `container_size` is the parent active-axis
denominator, `start_position` is physical pixels, and `active=false` means the
press has not exceeded threshold. It does not borrow the tree; stale lookups can
therefore be ignored rather than dereferenced.

## Layout algorithm and invariants

`SplitContainer::child_rects` is the single traversal used by layout, bar
placement, hit testing, and drag-container discovery. For parent origin `a`,
active-axis length `S`, and ratio `r_i`:

```text
offset_0 = 0
size_i = S × r_i
child_i = [a + offset_i, a + offset_i + size_i)
offset_(i+1) = offset_i + size_i
```

Missing ratios use `1 / children.len()`; an empty split has no children. The
desired durable invariant is:

```text
children.len() = ratios.len() = min_sizes.len()
all r_i finite and r_i ≥ 0
abs(sum(r_i) - 1) ≤ epsilon
```

Current layout does not validate/repair this. A supplied vector not summing to
one leaves space unused or runs past the parent. Debug `assert_invariants`
checks group/tab/editor/document references, not ratio normalization.

After each non-final child, `splitter_after` centers a scaled bar of width `w`
over the mathematical boundary:

```text
horizontal: Rect(boundary - w/2, parent.y, w, parent.height)
vertical:   Rect(parent.x, boundary - w/2, parent.width, w)
```

The bar overlays the boundary; children retain the full parent extent. Never
subtract `w` in a separate calculation. `compute_layout_scaled` loops each
child in order: it emits that child's following bar and immediately recurses
into the child subtree before moving to the next sibling. It stores rectangles;
the renderer then calls `sync_all_viewports`. `compute_splitters` repeats this
same interleaved emission read-only for `hit_test_ui` using the solved editor
shell.

### Traversal mismatch: current nested-drag constraint

The update-side `find_container_for_splitter` and
`visit_splitter_container_mut` instead reserve all `N - 1` boundaries of a
container as one contiguous global-index range _before_ recursing into its
children. That does not match bar emission above. For a root whose first child
is a two-child nested split and whose remaining children are B and C, bar
emission is root boundary 0, nested boundary, root boundary 1. Lookup treats
root boundaries 0 and 1 as contiguous positions before it looks into the nested
container. Consequently nested splitter drags can resolve/mutate an outer
container, and some combinations of captured global/local indices become a
defensive no-op. The outer boundary after the nested child can also be associated
with a different logical traversal position than its painted bar.

This is an existing correctness constraint, not a hypothetical concern. Current
documentation must not claim that every global index re-resolves its owning
container. A code fix would make emission and lookup share one traversal/identity
source; none is made by this documentation update.

### Drag calculation

[`src/update/layout.rs`](../../src/update/layout.rs) sets a 4 physical-pixel
Euclidean threshold and `MIN_PANE_SIZE_PIXELS = 100.0`. Once active, only the
split axis contributes:

```text
delta = pointer.x - press.x (horizontal), or pointer.y - press.y (vertical)
q = delta / container_size
combined = original[left] + original[right]
raw_min = 100 / container_size
effective_min = raw_min if raw_min ≤ combined/2 else 0.01
new_left = clamp(original[left] + q, effective_min, combined - effective_min)
new_right = combined - new_left
```

When the captured index resolves to the intended container, every move derives
from `original_ratios`, never a previous move; adjacent sum is preserved and
unrelated entries are unchanged. Invalid local/vector lookup is a safe no-op.
For nested layouts, however, the traversal mismatch above can resolve a _wrong_
container rather than merely fail, so those preservation claims apply only to the
container actually reached, not necessarily to the painted boundary's owner.

There are two limits. First, current code assumes a finite positive
`container_size`; future geometry boundaries should reject invalid values before
division. Second, `min_sizes` is **not enforced** in `child_rects` or ordinary
layout. The drag path's separate hard-coded 100 px guard can disagree with the
model vector. In a too-small container, `0.01` avoids zero panes, not unusable
panes. This is a current correctness gap, not a completed constraint system.

## Transitions, focus, and capture

```text
pointer → hit_test_ui (splitters before groups)
→ LayoutMsg::{Begin,Update,End,Cancel}SplitterDrag
→ update/layout changes UiState/layout tree → Cmd::Redraw
→ Renderer resolves rects/bars and viewport sizes
```

| State                 | Event/precondition                   | Deterministic result                                                                                                                                           |
| --------------------- | ------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| none                  | left press hits `Splitter { index }` | begin resolves current scaled shell/bar; records transaction with `active=false`; press is consumed.                                                           |
| armed                 | move distance `< 4 px`               | no ratio change; diagonal distance still decides activation.                                                                                                   |
| armed                 | move distance `≥ 4 px`               | marks active, then applies active-axis formula.                                                                                                                |
| active, flat layout   | move                                 | resolves the intended container and replaces only adjacent ratios; redraw.                                                                                     |
| active, nested layout | move                                 | current global-index mismatch can update a different container or no-op; do not rely on arbitrary nested splitter drag.                                        |
| armed/active          | release                              | drops capture and retains current in-memory ratios; no immediate persistence command. Ratios are captured/restored by session persistence in `src/session.rs`. |
| armed/active          | Escape/cancel                        | restores the complete captured vector if current global target resolves; redraw.                                                                               |
| any                   | tree/index/vector invalid            | safe no-op; capture waits for end/cancel.                                                                                                                      |

Splitter presses do not explicitly change `FocusTarget`; editor keyboard focus
normally remains. Hover drives the splitter cursor affordance. Current splitters
have no keyboard focus, keyboard resize, reset gesture, accessible role/value,
or explicit OS focus-loss/pointer-cancel transition.

**Proposed behavior — not implemented:** model a focusable separator with
orientation/name/value, arrow/Home/End commands using this same constraint
formula, and Escape rollback. Pointer cancel/application focus loss should send
`CancelSplitterDrag`. Those claims require a real normalized min-size solver.

## Worked traces

### Normal horizontal drag

At parent `x=0,width=800`, ratios `[.50,.50]` make children `[0,400)` and
`[400,800)`. A 6 px bar is `[397,403)`. Press `(400,200)` then move
`(402,200)`: below threshold, ratio remains `.50`. Move `(520,201)`:
distance exceeds 4, axis `delta=120`, `q=.15`, `raw_min=.125`,
`combined=1`; result `[.65,.35]`. Re-layout gives widths 520 and 280 with
bar `[517,523)`. Perpendicular 1 px has no ratio contribution.

### Tiny and nested edge case

At `S=150`, `[.5,.5]` has `raw_min=.667 > combined/2`, so current fallback
is `.01`. An extreme drag may yield `[.99,.01]`, sizes `148.5` and
`1.5 px`. Even `min_sizes=[300,300]` currently changes nothing: this is the
documented gap.

For a root split whose first child is nested, emission is not "all root bars,
then nested bars": root boundary 0 is emitted, then the nested bars, then root
boundary 1. Update lookup assumes the former ordering. Thus the nested bar's
emitted global index can target root ratios. Adding a root sibling further shifts
indices. Capture/restore is reliable only while the current traversal happens to
resolve the intended container; arbitrary nesting is a current known gap.

## Invalidation, cost, and verification

Bars/rectangles change with shell size, tree topology/order, ratios, direction,
preview presence, and DPI-scaled splitter width. Computing bars/traversing the
layout tree is O(layout nodes). A full drag redraw has broader cost: the renderer
calls `sync_all_viewports` for every tab, a changed group width can trigger
`ensure_wrap_cache` reflow, and it paints visible projected text. Thus only the
tree/bar phase is independent of document text; a resize drag's total work is
not. Read-only hit testing avoids cloning the editor area.

Display-scale change mid-drag is not currently rebased against press-time
physical geometry. A future persistence/sync feature must use structural or
generation identity before applying asynchronous ratios; there is no such async
owner today.

Existing structural tests in [`tests/editor_area.rs`](../../tests/editor_area.rs)
verify single/horizontal/vertical placement and edge containment. Static gallery
specimens only test paint.

| Coverage            | Initial state/action                                                        | Expected result                                                                                  |
| ------------------- | --------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------ |
| existing            | 800×600 root with `.5/.5` horizontal                                        | two 400 px groups and one horizontal-split bar.                                                  |
| existing            | 800×600 root with `.5/.5` vertical                                          | two 300 px groups and a vertical bar.                                                            |
| proposed unit       | trace above: 2 px then 120 px move                                          | no mutation below threshold; then `[.65,.35]`, sum preserved.                                    |
| proposed unit       | `[.3,.4,.3]`, modify boundary, cancel                                       | entire original vector restores, not only adjacent entries.                                      |
| proposed regression | `min_sizes=[300,300]`, 400 px parent                                        | records current non-enforcement until a solver intentionally changes contract.                   |
| proposed regression | root with first child a two-child split, plus B/C siblings; drag nested bar | demonstrates current index mismatch, then after a future fix verifies only nested ratios change. |
| proposed runtime    | active drag plus focus loss/tree replacement/DPI change                     | cancel/rebase deterministically; no write reaches a different container.                         |
| proposed gallery    | real nested area, hover, threshold, cancel at 1×/2×                         | production traversal/bar painter visibly resizes groups.                                         |

## Boundary

Dock/sidebar resizers have different state units, persistence, and constraints;
do not extract a generic widget from visual similarity. Once a correct constraint
solver exists, its pure boundary should accept original ratios, adjacent index,
parent axis size, constraints, and delta, then return a complete validated vector.
It must not borrow unspecified application state or duplicate group geometry.
