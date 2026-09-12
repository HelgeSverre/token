# Segmented control

## Purpose and implementation

A segmented control is a compact, single-choice selector whose short options
remain visible. It is value selection—not a toolbar of independent actions.
`segment_rects` divides a supplied rectangle into equal adjoining rectangles;
the final segment receives rounding remainder
([segmented_control.rs](../../src/view/segmented_control.rs#L8)).
`SegmentedControl` receives those rectangles, labels, selected index, focus and
scale; it paints each with the shared button primitive, applying `Selected` and
a ring only to the selected segment ([segmented_control.rs](../../src/view/segmented_control.rs#L26)).
There is no stored segment state, hit-test, keyboard handler, or disabled state.
The UI Gallery is its direct current consumer for the Narrow/Wide preview choice
([gallery.rs](../../src/view/gallery.rs#L283)); its input handling still belongs
to the gallery app, not this painter.

Settings' `preset_rects`/choice buttons and gallery's `choice-group.selected`
look related but are not this component: their rectangles wrap and use
feature-specific choices ([controls.rs](../../src/view/controls.rs#L119),
[gallery.rs](../../src/model/gallery.rs#L377)). Preserve that distinction until
their interaction contracts converge.

### Current data/geometry ownership

| Input                | Owner and current invariant                                                                                                                                                                                                                                                |
| -------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Bounds / segments    | Owner calls `segment_rects(bounds, count)`. For `i` in `0..count`, `left = x + width*i/count`, `right = x + width*(i+1)/count`; `w = right-left`, so adjoining segments cover the truncated bounds and the final one receives remainder. Zero count returns no rectangles. |
| Labels / selected    | Owner supplies parallel slices and an index. Render uses `segments.iter().zip(labels)`: mismatched lengths silently paint only the shorter length; an out-of-range selected index paints no selected segment.                                                              |
| Focus / scale        | Owner supplies one group-focus boolean and scale. Only selected segment gets the inherited Button focus ring.                                                                                                                                                              |
| Hit testing / events | No implementation in `SegmentedControl`; owner must reuse the segment rectangles.                                                                                                                                                                                          |

The gallery holds `GalleryState.compact`; pointer hit testing uses the same
`layout.width_segments`, sets the boolean and gallery focus, while Left/Home and
Right/End update it when Width is focused ([ui_gallery.rs](../../src/bin/ui_gallery.rs#L147),
[ui_gallery.rs](../../src/bin/ui_gallery.rs#L220)). This is a concrete consumer
example, not generic segmented-control keyboard support.

| Gallery trigger                                 | Current result                                                                                   |
| ----------------------------------------------- | ------------------------------------------------------------------------------------------------ |
| Pointer hits a width segment                    | Index 0 means Narrow/`compact = true`; index 1 means Wide/false; focus becomes Width.            |
| Tab / Shift+Tab                                 | Gallery cycles its Filter, Theme and Width focus targets; it does not focus individual segments. |
| Left/Home or Right/End with Width focus         | Sets compact true or false respectively.                                                         |
| Other keys, disabled option, option replacement | No component-level behaviour exists.                                                             |

## Anatomy and visual contract

Implemented anatomy is N adjoining labelled button surfaces, exactly one
selected visual segment, and optional focus ring. It inherits button roles:
`background`, `background_hover`, `background_pressed`, optional
`background_selected`, `foreground`, `border`, and `focus_ring`. It has no
segment separator, disabled, overflow, badge or icon-specific roles. Labels are
UI text at `12 * scale`; button clipping protects the individual segment, so
callers must choose labels that stay comprehensible when narrow.

The gallery gives this control a fixture width based on its global scale and
uses 12×scale UI text through the Button painter. `segment_rects` itself takes
already-scaled `Rect` pixels and performs integer division; callers must not
independently round hit-test bounds. The selected segment has no separator or
dedicated foreground token, so focus remains an outset `button.focus_ring` and
selection uses `button.background_selected` (falling back to pressed).

## Proposed contract and accessibility

**Proposed:** require 2–5 stable-ID, short-label options and exactly one
committed value (or an explicit unavailable state); emit a value-change message
from the owner. Pointer click commits that segment. A focused group exposes
role `radiogroup`; segments expose radio name, checked and disabled state.
Arrow keys move and commit according to platform policy, Home/End choose ends,
Space commits focused segment, and Tab enters/leaves the group once. Roving
focus must not make every segment a Tab stop. Selected and focused must remain
distinguishable visually.

| Proposed transition           | Required result                                                                                                                                               |
| ----------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Options supplied/replaced     | Reject duplicate IDs; reconcile selected/focused ID atomically. If selected disappears, use declared fallback or unavailable state—never raw-index neighbour. |
| Pointer click enabled segment | Commit that option; owner receives one value-change message.                                                                                                  |
| Enter/Tab into group          | One roving-focus stop enters selected (or first enabled) segment; Tab exits group.                                                                            |
| Arrow / Home / End / Space    | Move roving focus and commit according to documented policy; never activate disabled option.                                                                  |
| Disabled vs read-only         | Disabled is skipped/inert; a read-only selection stays discoverable and announces value but emits no change.                                                  |

IntelliJ's radio guidance recommends a segmented button for short labels in
small visible choice sets, otherwise radio buttons or a drop-down
([Radio button](https://plugins.jetbrains.com/docs/intellij/radio-button.html)).
This is guidance, not evidence that Token currently implements those events.

## Acceptance

Add gallery fixtures for all normal/selected/focused/disabled states, 2/3/5
segments, fractional-width scaling, long-label clipping, pointer selection and
keyboard traversal, empty/mismatched inputs (assert rather than silently zip),
and replacement/removal reconciliation. Keep `segment_rects` as shared geometry
for painting and hit testing. Do not use it for tabs, multi-select filters, or
an on/off setting.
