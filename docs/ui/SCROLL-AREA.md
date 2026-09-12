# Scroll area

A scroll area exposes a bounded viewport onto content. Its owner controls content
and offset; the shared component owns only the relationship between content,
viewport, track and thumb. This is intentionally not a generic data-list API.

## Current implementation — verified

### Shared scrollbar primitive

[Scrollbar primitive](../../src/view/scrollbar.rs) is reusable.

- ScrollbarState { total, visible, position } describes one axis in a common
  unit and clamps its maximum to total.saturating_sub(visible).
- ScrollbarGeometry vertical/horizontal derives track and thumb rectangles, hit
  tests them, and maps track click/drag back to a position.
- No thumb is needed when content fits; track geometry remains available.
- Thumb length is proportional to visible / total, with a 20 physical-pixel
  minimum. Its travel is track length minus thumb length.
- Track click centers the thumb under the pointer; drag retains the pointer's
  press offset inside the thumb. Non-finite values or zero travel map safely to
  zero.
- Painter inputs are ScrollbarColors { track, thumb, thumb_hover }, resolved
  from Theme::scrollbar.

This geometry is used by editor scrollbars, modal/Settings/documentation lists
and gallery. Its mapping equation is:

```text
max_scroll = max(0, total - visible)
travel     = max(0, track_length - thumb_length)
position   = round(clamp(pointer - grab - track_start, 0, travel)
                   / travel * max_scroll)
```

The equation is valid only after the owner selects consistent units. It is not a
license to mix rows, columns and physical pixels.

### Editor viewport

EditorState::viewport stores the integral first visual row/column
(top_line, left_column) plus PixelViewport sub-cell offsets. A PixelAxis
normalizes a continuous physical-pixel position into that integral anchor and
offset, clamps against content extent, preserves within-cell position on
font/DPI resize, and draws both partially clipped edge cells.

The editor vertical content unit is physical pixel height over
viewport_map(document).row_count(); this map accounts for soft wrapping,
folding and ghost/inlay projection. Horizontal units are physical pixels over
scrollable_columns(document). view/editor_scrollbars.rs::scrollbar_states is
the shared conversion used by editor paint and pointer hit testing. Horizontal
text scrolling is omitted with soft wrap.

EditorState::scroll_pixels is pane-local and rejects non-text special modes.
Discrete wheel input can animate for 140ms; raw trackpad pixels apply directly.
Navigation/editing cancels animation where appropriate. Cursor reveal works in
visual rows and honors scroll_padding; wheel scrolling intentionally leaves the
caret where it is. [Scrolling tests](../../tests/scrolling.rs) cover fractional scroll, clamping,
reveal and animation retargeting.

### Other current owners

| Surface              | Owner-held offset/unit            | Shared part                                 |
| -------------------- | --------------------------------- | ------------------------------------------- |
| Settings records     | form record scroll position       | geometry/pointer mapping                    |
| command/picker modal | selected list row offset          | overlay list/scrollbar geometry             |
| cursor documentation | documentation viewport offset     | overlay geometry/pointer mapping            |
| gallery              | pixel scroll over specimen rows   | shared vertical scrollbar geometry          |
| tab strip            | EditorGroup::tab_scroll in pixels | EditorTabBarLayout, not a scrollbar control |
| sidebar/panels       | feature-specific row offsets      | solved RowListView where adopted            |

These are not yet one common retained ScrollAreaState; do not flatten their
different selection, virtualization and side-effect semantics just to share a
type.

## Anatomy and geometry contract

A scroll area has content extent, viewport rectangle, offset, optional track,
optional thumb, and an explicit clipping rectangle. Content origin is translated
by offset; input maps through the inverse transform. The scrollbar may overlay
editor content: GroupLayout places vertical track at the content right edge and
horizontal track at its bottom edge, reserving the corner.

**Proposed reusable contract:**

```text
ScrollMetrics { total, visible, position }  // one declared unit per axis
ScrollAreaLayout { viewport_rect, clip_rect, vertical?, horizontal? }
ScrollInput { wheel, track_click, thumb_press, drag, release, cancel }
```

The owner supplies metrics, chooses visibility, applies returned position, and
owns effects/selection policy. Geometry/painter functions stay pure. Rendering,
pointer mapping, automation and tests consume the same layout object. Define
whether the track remains visible when content fits; editor currently paints the
vertical strip but no meaningful thumb, while horizontal renders only when needed.

## Event, focus and accessibility contract

### Existing behavior

Scrollbar thumb press creates ui.scrollbar_drag: ScrollbarDragState with target,
axis, grab offset, track metrics and maximum scroll. UiMsg routes updates to its
target, and release clears capture. Pressing a track jumps without changing a
list selection. Beginning a thumb drag cancels editor scroll animation.
runtime/mouse.rs hits editor vertical/horizontal thumbs and tracks before
ordinary editor content, so a scrollbar cannot move the text cursor.

Wheel routing is hover-region based. Editor tab bars scroll horizontally; modal
and cursor-overlay lists scroll their own viewport without moving selection;
editor wheel input scrolls the hovered editor/special mode. A completion popup
is dismissed before editor scroll because its caret anchor would otherwise
detach. Focus is not changed by shared scrollbar messages today.

### Required behavior for a new consumer

- Capture a thumb from press until release/cancel, even outside the track.
- Retain press-time target and axis; reject a stale target after it disappears.
- Keep track clicks and wheel scroll separate from selection activation unless
  the surface explicitly says otherwise.
- Clamp after content/viewport/font/scale changes. Cancel or revalidate an
  animation/drag whose data identity changes.
- Support keyboard scrolling in the owner (arrow/Page/Home/End as appropriate)
  and disclose focus target. Current generic scrollbar has no keyboard focus.
- Expose semantic name/value/range to an accessibility backend when Token gains
  one; never make the only operation an unlabeled thumb gesture.

## Edge cases

- Empty, fitting or zero-size viewport: maximum is zero; never divide by zero
  or expose a stale thumb.
- Minimum thumb can eliminate travel; position remains zero.
- Fractional editor offsets mean first/last rows can be clipped; draw and hit
  tests use pixel extents, not a rounded row approximation.
- Soft wrap changes row count and disables horizontal text scroll. Fold/ghost
  changes also invalidate the editor overview projection.
- Very long documents retain precision: PixelAxis::cell_origin subtracts locally
  instead of subtracting large absolute positions.
- Scrollbars have no pressed/dragging color beyond normal/hover today.
- Do not route text-only fast paths into CSV, image or binary tabs:
  EditorState::is_plain_text_mode() is the boundary.

## Gallery coverage and next slice

Gallery has static scrollbar.vertical, hovered, end, content-fits, horizontal and
horizontal-hovered specimens, plus an interactive gallery-shell scrollbar. It
lacks a real editor viewport with fractional scroll, wrapped rows, folds,
overview marks, drag capture, track-jump, keyboard scrolling, modal/documentation
target differences and narrow group-edge clipping.

Priority after foundational gallery work: add an editor-scroll composition that
calls scrollbar_states and production editor painting with long/wrapped/folded
content; then add an interaction harness or focused native test for thumb capture
and track click. Do not draw lookalike scrollbars.

## Acceptance criteria and evidence

A change is ready when its owner/state unit is explicit; one geometry object
drives paint/hit/automation; clips include partial edge cells; offset clamps
after extent changes; pointer capture/cancel is tested; light/dark scrollbar
roles resolve; and its gallery specimen calls production painting.

High-confidence evidence: [model scroll](../../src/model/scroll.rs),
[editor state](../../src/model/editor.rs), [UI state](../../src/model/ui.rs),
[scrollbar](../../src/view/scrollbar.rs),
[editor scrollbars](../../src/view/editor_scrollbars.rs),
[geometry](../../src/view/geometry.rs), [editor update](../../src/update/editor.rs),
[UI update](../../src/update/ui.rs), [runtime mouse](../../src/runtime/mouse.rs),
[scrolling tests](../../tests/scrolling.rs) and [gallery guide](../dev/ui-gallery.md). The
proposed reusable contract is not implemented. Editor anatomy terminology aligns
with [IntelliJ's editor/gutter description](https://plugins.jetbrains.com/docs/intellij/ui-overview.html).
