# Segmented control

## Current boundary

Token has one paint-only segmented control, used by the component gallery’s
Preview width selector. It is not used for Settings choices. The actual path is:

```text
GalleryState.compact (durable bool) + GalleryFocus::Width
 -> GalleryLayout.width_segments (derived physical rectangles)
 -> SegmentedControl::render -> render_button per segment
```

There is no control message, hit test, IDs, hover/pressed state, disabled
state, pointer capture, or keyboard reducer. Those would belong to a consumer.
Settings choice groups use choice_group_rects and standard buttons instead.

## Existing representation and ownership

**Current excerpt** — [segmented_control.rs:8](../../src/view/segmented_control.rs#L8).

```rust
pub fn segment_rects(bounds: Rect, count: usize) -> Vec<WidgetRect> { /* ... */ }

pub struct SegmentedControl<'a> {
    pub segments: &'a [WidgetRect],
    pub labels: &'a [&'a str],
    pub selected: usize,
    pub focused: bool,
    pub scale: f64,
}
```

bounds is a physical-pixel Rect with f32 components. segment_rects truncates it
to usize physical pixels. count is an item count. The returned Vec is owned by
the layout owner; each WidgetRect has physical-pixel x/y/w/h. The control
borrows that Vec and labels only for one paint call.

selected is a raw zero-based index into both slices. focused only requests a
focus ring around the selected segment; it does not store or manage focus.
scale changes label size to 12*scale physical pixels. No assertion establishes
segments.len()==labels.len(): render uses zip, silently dropping trailing input.
That is memory-safe but not an acceptable semantic invariant for a reusable
interactive component.

The gallery owns durable value and focus:

```rust
pub struct GalleryState {
    pub compact: bool,       // false selects index 1, Wide
    pub focus: GalleryFocus, // Width maps to focused=true
}
```

GalleryLayout creates its two rectangles from fixed header geometry on each
layout ([gallery.rs:134](../../src/view/gallery.rs#L134)). The component owns no
persistence, command, cache, identity, or collection-repair policy.

## Geometry and paint derivation

### Equal-width partition

The current calculation is:

```rust
if count == 0 { return vec![]; }
for i in 0..count {
    let left  = bounds.x as usize + bounds.width as usize * i / count;
    let right = bounds.x as usize + bounds.width as usize * (i + 1) / count;
    rects.push(WidgetRect { x:left, y:bounds.y as usize,
                            w:right-left, h:bounds.height as usize });
}
```

For nonnegative truncated bounds, adjacent edges are equal and total output
width is floor(bounds.width). Integer-division remainder goes to later segments.
At bounds=(10.8,20.4,164,29), count=2: (10,20,82,29), (92,20,82,29).
At width=101,count=3: x/w are (10,33), (43,34), (77,34). At width=2,count=3:
(10,0), (10,1), (11,1). The zero-width segment is safe but unusable.

### Projection to standard buttons

```rust
for (i, (rect, label)) in self.segments.iter().zip(self.labels).enumerate() {
    render_button(...,
       state: if i == self.selected { ButtonState::Selected }
              else { ButtonState::Normal },
       focused: self.focused && i == self.selected,
       text_size: Some((12.0 * self.scale) as f32));
}
```

Selected maps to button.background_selected when available, otherwise pressed
background, with focus_ring border. Normal maps to button.background/border.
Each segment is an independent standard button: adjoining geometry does not
make one joined surface. The button painter centers and clips its label to the
same rect.

With labels Narrow/Wide, selected=1, focused=true, Wide is Selected with an
outset one-physical-pixel ring and Narrow is Normal. With selected=9 but two
paired entries, neither is selected and no ring appears. That is present
fail-soft painting, not selection repair.

## Current state × event behaviour

The component is passive; it has no event method.

| Input/model event                    | Current result                                       | Owner responsibility                         |
| ------------------------------------ | ---------------------------------------------------- | -------------------------------------------- |
| pointer press/release/outside/cancel | no transition                                        | resolve shared rectangles and update compact |
| arrows, Space, Enter, Tab            | no transition                                        | dispatch focus/selection in update layer     |
| focus loss                           | focused false only if caller supplies it next render | owner                                        |
| disabled/read-only                   | no representation; normal/selected still paint       | do not use it for inert control              |
| count zero                           | empty rectangle list                                 | omit interaction                             |
| labels/rect mismatch                 | zip paints min length                                | assert/equalize before semantic use          |
| selected stale after replacement     | no selected paint                                    | reconcile durable state                      |
| scale/theme/bounds changed           | repaints arguments                                   | invalidate old layout/hit plan               |

The existing gallery test passes centers of the _same_ width_segments vector to
section_navigation::section_at and verifies the matching index
([gallery.rs:973](../../src/view/gallery.rs#L973)). It proves shared geometry for
this gallery owner, not generic pointer or keyboard semantics.

## Proposed semantic control (not current API)

```rust
// Proposed API — not implemented.
struct Segment<Id> { id: Id, label: String, enabled: bool }
struct SegmentedModel<Id> {
    selected: Option<Id>,       // durable owner value
    focused: Option<Id>,        // transient roving focus
    pressed: Option<(u64, Id)>, // captured pointer identity and target
}
enum SegmentMsg<Id> { Focus(Id), Select(Id), CancelPress }
```

IDs must survive reorder. A consumer validates unique IDs and equal option/layout
count, derives geometry once, and owns persistence/effects. On replacement,
retain selected/focused only if their enabled IDs remain; otherwise use explicit
fallback or None, clear removed capture, and invalidate layout. Never interpret
a former raw index as an identity.

| Proposed state × event                      | Preconditions                  | Result                                    |
| ------------------------------------------- | ------------------------------ | ----------------------------------------- |
| press id                                    | enabled, not read-only         | capture pointer/id; paint Pressed         |
| release same id                             | capture still valid            | clear capture, focus id, emit Select once |
| release another/outside, cancel, focus loss | capture exists                 | clear capture, no Select                  |
| Left/Right, Home/End                        | control focus, enabled options | move roving focus                         |
| Space/Enter                                 | focused enabled id             | emit Select, retain focus                 |
| Tab/Shift+Tab                               | control focus                  | enter/leave once, not per segment         |
| disabled                                    | any activation                 | skipped/inert and Disabled paint          |
| read-only                                   | activation                     | may report/focus; never Select            |

Whether arrows commit immediately is an owner policy. Immediate commit suits the
gallery’s reversible compact setting; a side-effecting setting may preview then
commit on Enter.

## Invalidation, integration, and verification

segment_rects allocates O(count); paint is O(min(labels,rectangles)) plus glyph
work. Recompute when physical bounds, count/order/labels, selected/focused/
pressed/disabled state, scale, font metrics, or theme changes. The component
has no cache or supported timing claim.

Current integration:

```rust
let segments = segment_rects(
    Rect::new(w - 188.0*s, 76.0*s, 164.0*s, 29.0*s), 2);
SegmentedControl {
    segments: &segments,
    labels: &["Narrow", "Wide"],
    selected: usize::from(!state.compact),
    focused: state.focus == GalleryFocus::Width,
    scale,
}.render(frame, painter, theme);
```

If input is added, keep that exact Vec in the layout snapshot and use it for
hit testing. Independent logical geometry will disagree at truncation/remainder
boundaries.

| Initial input              | Action                      | Expected result                                  |
| -------------------------- | --------------------------- | ------------------------------------------------ |
| (10,20,101,29), count=3    | partition                   | (10,33),(43,34),(77,34); right=111               |
| width=2,count=3            | partition                   | widths 0,1,1; no overlap/out-of-bounds           |
| count=0                    | partition                   | empty Vec/no hit target                          |
| two rects, one label       | current render              | only first paints; proposed construction rejects |
| selected=9, two segments   | current render              | no selected/focus paint; proposed owner repairs  |
| compact=false, focus=Width | render                      | Wide selected/focused, Narrow normal             |
| proposed capture A         | release B/cancel/focus loss | no Select; capture clears                        |
| proposed selected B        | replace options A/C         | explicit fallback/None; B focus/capture clear    |

Accessibility group/radio roles and selected/disabled announcements are
proposed semantic-layer work, not capabilities of the present button painter.
