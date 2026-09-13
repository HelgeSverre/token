# Scroll area

This chapter documents Token's current scrolling machinery. It does **not**
claim Token has one retained generic `ScrollArea`: owners choose their own
content unit and semantic policy. The reusable layer is a pure one-axis
scrollbar calculation. This distinction matters because an editor scrolls in
physical pixels over _projected visual rows_, whereas a modal may scroll rows
and the Settings form owns a measured pixel offset.

For conceptual terminology, see [the editor reference's scrollable-region
chapter](../EDITOR_UI_REFERENCE.md#chapter-1-foundational-concepts-the-scrollable-region).
This document specifies Token's real representations and conversions.

## Current representation and ownership

### Reusable axis geometry

**Current excerpt** — [`src/view/scrollbar.rs`](../../src/view/scrollbar.rs)

```rust
pub struct ScrollbarState {
    pub total: usize,
    pub visible: usize,
    pub position: usize,
}

pub struct ScrollbarGeometry {
    pub state: ScrollbarState,
    vertical: bool,
    pub track_rect: Rect,
    pub thumb_rect: Rect,
    pub needed: bool,
}
```

`ScrollbarState` is an ephemeral value passed into a pure calculation; it owns
no model offset. `total`, `visible`, and `position` must use the same unit at a
call site. The type cannot prevent rows being mixed with pixels, so each owner
is responsible for the conversion. `usize` makes the primitive's range
integral: editor callers round their physical position, while a row-list owner
may pass rows directly.

`track_rect`/`thumb_rect` are window-coordinate physical-pixel rectangles;
`vertical` selects Y rather than X. `needed` is `total > visible`. When false,
the geometry retains the track as an initialized `thumb_rect` but the painter
draws no thumb. It still draws the track. The color input is presentation only:

```rust
pub struct ScrollbarColors {
    pub track: u32,
    pub thumb: u32,
    pub thumb_hover: u32,
}
```

It is resolved from `Theme::scrollbar`. Changing colors must not mutate scroll
state. There are idle and hover colors today, not a pressed color.

### Editor durable state, transient state, and borrowed mapping

**Current excerpt** — [`src/model/editor.rs`](../../src/model/editor.rs) and
[`src/model/scroll.rs`](../../src/model/scroll.rs)

```rust
pub struct Viewport {
    pub animation: Option<ScrollAnimation>,
    pub pixels: PixelViewport,
    pub top_line: usize,
    pub left_column: usize,
    pub visible_lines: usize,
    pub visible_columns: usize,
}

pub struct PixelAxis {
    pub offset: f64,
    pub unit: f64,
    pub extent: f64,
}
```

`top_line` and `left_column` are integral anchors, not complete positions. In
text mode `top_line` addresses a projected visual-row sequence; it means a
source line only when wrap, folds, and ghost projection make those sequences
identical. `left_column` is a display column. `offset` is the within-anchor
physical-pixel displacement, `unit` is measured char width or line height
(never below `1.0`), and `extent` is the exact visible pixel length.
`visible_lines`/`visible_columns` are integral command capacities rebuilt by
layout.

`ScrollAnimation` is transient model state: start, target, last displayed
`(x,y)` pixels, elapsed time, document revision, and cursor source position.
Discrete wheel input uses a 0.140-second ease-out; raw trackpad pixels do not.
`PixelAxis::resize` rescales the stored sub-cell offset by `new_unit / old_unit`
then receives a newly measured extent, preserving a fractional scroll location
through font/DPI changes.

The editor view owns `soft_wrap`, `wrap_cache`, `folds`, `ghost_text`, and
`overview_cache`; the `Document` owns buffer/revision and diagnostics.
`EditorState::viewport_map` constructs a `TextViewportMap<'_>` snapshot for the
operation at hand. It **copies** `PixelViewport`, `top_line`, `left_column`,
`visible_lines`, and source `line_count`; it borrows only optional `WrapCache`,
fold projection, and eligible ghost projection. It is derived
presentation/mapping data, not durable state and not a live borrow of
`Viewport`. In wrapped mode its copied X anchor and X offset are forced to zero.

### Input capture

**Current excerpt** — [`src/model/ui.rs`](../../src/model/ui.rs)

```rust
pub struct ScrollbarDragState {
    pub target: ScrollbarTarget,
    pub axis: ScrollbarDragAxis,
    pub grab_offset: f32,
    pub track_start: f32,
    pub track_size: f32,
    pub thumb_size: f32,
    pub max_scroll: usize,
}
```

This is transient `UiState::scrollbar_drag`, not scrollbar geometry state.
`target` is an identity (Settings records, `EditorId`, modal ID, or
documentation overlay kind plus selected row), never a borrowed editor. The
remaining values are a press-time physical-pixel snapshot except `max_scroll`,
which is in the owner's integral unit. It retains the thumb grab point when the
pointer moves outside the track. An updater validates IDs before changing modal
or documentation state; absent editor lookup safely does nothing.

## Projection, geometry, and invariants

Let `i` be the integral anchor, `u` the axis unit in pixels, `o` the fractional
offset, `E` viewport extent, and `N` projected content cells:

```text
p     = i × u + o
p_max = max(0, N × u - E)
0 ≤ p ≤ p_max; 0 ≤ o < u (except the final clamped partial extent)
```

`PixelAxis::set_position` rejects non-finite input, clamps `p`, then repairs
the representation as `i = floor(p / u)` and `o = p - i × u`. It is the only
normalization path for editor pixel scroll. After a resize or changed
projection, `sync_all_viewports` resizes axes, refreshes wraps, and reapplies
the current position with current `N`, preventing an anchor beyond EOF.

For local visual row/cell index `k`, renderer and hit testing use the same
rounded offset boundary:

```text
y(k) = k × line_height - round(y.offset)
x(c) = (c - left_column) × char_width - round(x.offset)
```

`drawn_count` includes partial first/final cells and the clip rect decides which
pixels are visible. `cell_origin` subtracts locally, avoiding large absolute
pixel subtraction on long files. Pointer conversion in `cell_at_pixel` uses the
matching offset rounding; do not replace it with row rounding.

[`scrollbar_states`](../../src/view/editor_scrollbars.rs) converts editor state
to the primitive's integral **physical-pixel** units:

```text
vertical.total    = viewport_map.row_count × line_height
vertical.visible  = GroupLayout.content_h
vertical.position = round(top_line × line_height + y.offset)

horizontal.total    = ceil(scrollable_columns × char_width)
horizontal.visible  = content_right - text_start_x
horizontal.position = round(left_column × char_width + x.offset)
```

Soft wrapping suppresses horizontal painting and hit testing, preserving the
zero-X invariant. The vertical count uses projected rows: folds reduce it and
ghost rows may add/replace it. `Document::line_count()` is therefore wrong for
editor scrollbar total.

For primitive values `T` total, `V` visible, `P` position and track length `L`:

```text
M       = saturating_sub(T, V)
thumb   = min(max((V / T) × L, 20 px), max(L, 0))
travel  = max(0, L - thumb)
thumb_0 = track_start + clamp(P / M × travel, 0, travel)  when M > 0
```

`needed` protects division by zero because no thumb is calculated when `T ≤ V`.
Track clicks center a thumb at pointer coordinate `q`; drag preserves press
distance `g` inside the thumb:

```text
d = clamp(q - g - track_start, 0, travel)
P = min(round(d / travel × M), M)
```

Zero travel, NaN coordinate, or NaN grab offset produces zero safely. The owner
still owns semantic clamping before mutating persistent state.

## Current transition path

```text
winit pointer/wheel → runtime/mouse hit_test_ui
→ Msg::Ui(UiMsg) or Msg::Editor(EditorMsg)
→ update::{ui,editor} → Cmd::Redraw / Cmd::redraw_editor
→ Renderer rebuilds layout/map/geometry and paints
```

Scrollbar hit targets precede editor-content hit targets, so a track press never
places a caret. Renderer and hit testing derive the same editor state through
`scrollbar_states` and `ScrollbarGeometry`; a consumer must not calculate a
lookalike track independently.

| State      | Event/precondition                        | Transition and effect                                                                                                                                         |
| ---------- | ----------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| no capture | thumb press on needed track               | `ScrollbarThumbPressed(snapshot)` cancels editor easing, sets `UiState::scrollbar_drag`, redraws. No caret/focus change.                                      |
| no capture | track press                               | runtime uses `position_from_track_click`; `ScrollbarTrackClicked` routes that position and redraws.                                                           |
| capture    | pointer move                              | `ScrollbarDragUpdate` maps the captured axis coordinate through press-time geometry and routes it.                                                            |
| capture    | pointer release or window focus loss      | runtime dispatches `ScrollbarDragEnd`, which takes capture and redraws.                                                                                       |
| capture    | wrong/removed modal or overlay identity   | `scroll_target` clears capture and does nothing; a missing editor is a no-op.                                                                                 |
| text pane  | wheel pixels                              | `ScrollPixels` rejects special modes/non-finite values. Direct input calls `set_pixel_scroll`; discrete input retargets easing.                               |
| editor     | navigation, editing, resize/metric change | updates cancel/revalidate animation; resize clears captured scrollbar geometry, and layout reapplies clamped current pixel position after rebuilding mapping. |

`scroll_target` turns an editor scrollbar's integer position back into a delta
from `pixel_scroll_position` and invokes `scroll_editor_pixels_by(..., false)`.
That deliberately reuses `set_pixel_scroll` normalization. Track clicks do not
move modal/documentation selection. Wheel routing follows `HoverRegion`; tab
bars instead modify their own `EditorGroup::tab_scroll`.

Release and window focus loss are modeled: runtime dispatches
`ScrollbarDragEnd` for both; `AppModel::resize` also clears capture because its
track snapshot belongs to the old window geometry. There is no distinct pointer
cancel event or rollback semantics: a cancel after a drag would currently retain
the last applied offset. A future consumer should define that transition and
revalidate target/generation after structural mutations. There is no generic
scrollbar keyboard focus today.

## Worked traces

### Ordinary projected editor drag

200 projected rows at `20 px` make `T=4000`; a `300 px` content viewport and
track make `V=300`. At `top_line=10`, `offset=7.5`, `P=round(207.5)=208` and
`M=3700`. Thumb size is `max(300/4000 × 300,20)=22.5 px`; travel is `277.5 px`;
thumb origin is `208/3700 × 277.5=15.6 px` after track start.

The user presses 5 px into it and moves to 190 px after track start. Thus
`d=190-5=185`; `185/277.5 × 3700 = 2466.666…`, which rounds to `2467 px`.
Normalization gives `top_line=floor(2467/20)=123`, `offset=7`. Local row origins
are `-7, 13, 33, …`; clipping paints the partial first row.

### Endpoint, fractional hit, and tiny track

Ten `20 px` rows in `95 px` have `p_max=105`. A request for `1000` clamps to
`105`, repairing to `(top_line=5, offset=5)`. It draws
`ceil((95+round(5))/20)=5` cells. Local Y `11.5` maps to cell 0;
Y `15` maps to cell 1. A whole-row approximation would disagree at that edge.

On a 12 px track the 20 px minimum clamps thumb size to 12, leaving travel zero;
drag mapping returns zero without division. If content fits (`T ≤ V`), no thumb
target is published even though the track can remain painted.

## Invalidation, cost, and verification

`ScrollbarGeometry` is cheap derived data; rebuild it whenever track rectangle,
metrics, total, visible, or position changes. Editor metrics depend on group and
find-bar geometry, scaled scrollbar width, font metrics, document buffer and
revision, wrap width/cache identity, folds, ghost projection, and viewport
position. The overview-mark cache separately keys buffer identity/revision,
wrap/fold/find identity, diagnostic ranges, projected row count, and exact track
height; it rebuilds its bounded pixel-row projection when any changes.

Text rendering traverses visible projected rows, not the full document. Use the
existing shared `PerfStage` entries for evidence; do not add a component-local
timer. Async producers must retain document identity/revision checks before
their results alter cached projection input.

Existing tests are in [`src/model/scroll.rs`](../../src/model/scroll.rs),
[`src/view/scrollbar.rs`](../../src/view/scrollbar.rs),
[`src/view/editor_scrollbars.rs`](../../src/view/editor_scrollbars.rs), and
[`tests/scrolling.rs`](../../tests/scrolling.rs).

| Coverage            | Initial state/action                                              | Required result                                                                                                                  |
| ------------------- | ----------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------- |
| existing            | `unit=20`, `extent=95`, set 47.5 across ten rows                  | `(anchor,offset)=(2,7.5)`, edge cells match conversion, endpoint becomes `(5,5)`.                                                |
| existing            | ease from 7.75 by +60, advance 40 ms, then -10 animated           | reversal targets displayed Y; navigation/resize clear animation.                                                                 |
| existing            | ordinary/tiny thumb grabbed one-third in                          | same coordinate returns current position; bounds and zero travel are safe.                                                       |
| proposed runtime    | press editor thumb, move outside track, release/focus loss/resize | captured `EditorId` receives moves until release; release or focus loss ends capture, and resize clears stale captured geometry. |
| proposed projection | wrapped/folded/ghosted document; vary each                        | vertical total uses map rows; X bar absent while wrapped; overview invalidates for each input.                                   |
| proposed gallery    | production long text at 7.5 px offset                             | first row clips by 8 px and thumb agrees with `scrollbar_states`, with no hand-drawn substitute.                                 |

## Proposed reusable boundary

No `ScrollAreaState` is implemented. If future owners truly share semantics,
share only pure layout and conversion:

**Proposed API — not implemented**

```rust
pub struct ScrollMetrics { pub total: usize, pub visible: usize, pub position: usize }
pub struct ScrollAreaLayout {
    pub viewport_rect: Rect,
    pub clip_rect: Rect,
    pub vertical: Option<ScrollbarGeometry>,
    pub horizontal: Option<ScrollbarGeometry>,
}
```

The owner must still own selection/focus, keyboard commands, effects,
animations, target-generation checks, and accessibility. This API must not
replace `TextViewportMap` or force row lists into editor pixel semantics.
