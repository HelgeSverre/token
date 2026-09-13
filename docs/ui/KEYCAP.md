# Keycap

## Current representation and ownership

A keycap is passive physical-pixel chrome for one displayed key. A binding is an
ordered sequence of chord steps; it is not a button and never participates in
keymap dispatch. Token has no retained `Keycap` state. The keymap/configuration
owner owns the durable binding; render borrows its already formatted `&str`,
creates temporary chips, and paints them in one frame.

**Current excerpt — borrowed presentation data**

```rust
// src/view/overlay_surface.rs
#[derive(Debug, Clone)]
pub struct Chip { pub label: String }
pub enum Accessory<'a> {
    Keycaps(&'a [Vec<Chip>]), // outer: chord step; inner: modifiers then key
    DimText(&'a str),
    // ...
}
```

`Chip::label` owns a `String` because parsing splits the borrowed display string.
`Accessory::Keycaps` borrows the completed vector only for rendering; it cannot
escape into `AppModel`. `⌘K ⌘C` becomes `[[⌘, K], [⌘, C]]`, while `F12` is one
chip. Empty spaces are discarded. Leading macOS modifiers `⌃⌥⇧⌘` split by
character; textual `Ctrl+`, `Alt+`, `Shift+`, `Win+` prefixes are peeled in
order, leaving the final `+` in `Ctrl++` as the key.

The parser does not canonicalize or validate bindings. That belongs to the
keymap/platform formatter. Its only invariant is leading modifier chips plus at
most one nonempty key chip per nonempty step. An all-modifier malformed input is
displayable but must not be created by the model owner.

## Measurement, paint, and clipping

Logical constants become physical pixels as `round(v × scale).max(1)`. Text is
`11 × scale` physical px; text measurement is `f32` physical px, but extents
and origins are integer physical px.

**Current algorithm — shared by reservation and paint**

```rust
fn width(measure: &mut dyn TextMeasure, label: &str, sf: f64) -> usize {
    let px = |v: f32| (v as f64 * sf).round().max(1.0) as usize;
    let text = measure.width(label, TextStyle::sized((11.0 * sf) as f32));
    (text.ceil() as usize + 2 * px(4.0)).max(px(17.0))
}
fn height(p: &TextPainter, sf: f64) -> usize {
    let px = |v: f32| (v as f64 * sf).round().max(1.0) as usize;
    p.line_height_for_size((11.0 * sf) as f32) + 2 * px(2.0)
}
```

At `sf=1`, `measure("⌘")=7.2` yields `max(ceil(7.2)+8,17)=17 px`; a
`Ctrl` measurement of 23.1 yields 32 px. At 1.25x, padding is `round(5)=5`
and minimum width `round(21.25)=21`: never multiply a rounded 1x result.

`draw_keycap` first fills a rounded outer rectangle with radius `round(4×sf)`,
then an inset background. Border is one pixel at top/sides and deliberately two
at bottom:

```
inner = (x+b, y+b, width-2b, height-b-(b+round(1×sf)))
text.x = x + floor((width-ceil(text_width))/2)
text.y = y + floor((height-line_height)/2)
```

The raised lower edge is paint, not extra layout height. `RoundedRectMaskCache`
caches coverage by physical radius; `TextPainter` caches rasters by glyph and
physical size. Measure and draw must use the same scoped font.

Overlay accessories are right-aligned at
`row.x+row.w-inset-text_pad-accessory_width`. Width is all cap widths plus
`CHIP_GAP` within a chord and `CHIP_STEP_GAP` between chords. Caps share
`row.y+floor((row.h-chip_height)/2)` and create no individual clip or hit
rectangle. Crucially, normal `overlay_surface::render_list` does **not** push an
owner/list clip; it relies on the clip already active on `Frame` (often none).
The Settings page is different: it pushes `settings_viewport.rect()` around its
row traversal and separately clips its keycap control rectangle.

## Passive projection, cost, and proposed boundary

Keycaps have no pointer, keyboard, focus, capture, cancel, or accessibility
machine. Their owner invalidates paint when binding, geometry, scale, palette,
or font changes. Glyph/mask caches are derived accelerators, not state. Cost is
O(chips plus glyphs); no retained keycap layout cache exists.

The palette applies `chip_count(steps)>4 → Accessory::DimText(original)` before
paint. This prevents narrow rows from consuming primary-label width; it is a
consumer policy, not wrapping. Any extraction must preserve it:

```rust
// proposed API — not implemented
// WidgetRect is existing view::geometry physical-usize rectangle. `chip_rects`
// would be paint-only geometry, not input targets; Vec is per-layout ephemeral.
struct KeycapSequence<'a> { steps: &'a [KeycapStep<'a>] }
struct KeycapStep<'a> { keys: &'a [Keycap<'a>] }
struct Keycap<'a> { label: &'a str }
struct KeycapLayout { chip_rects: Vec<WidgetRect>, total_width: usize }
```

Geometry may support a future capture UI, but passive display must never become
a tab stop.

## Worked trace, integration, and tests

At 1x let normal overlay row be `(20,100,300,30)`, inset 6/text pad 8, and every cap in
`⌘K ⌘C` be 17px. With gaps 4/6, width is
`17+4+17+6+17+4+17=78`; accessory origin is `20+300-6-8-78=228`; primary
label right is `20+300-6-8-78-8=220`. All four caps use the same centered y,
and primary text is truncated to end at or before x=220. `⇧⌘K ⌘C` is
five caps and therefore must project as dim text. `Ctrl++` must project as
`[Ctrl,+]`, not an empty last cap.

Flow is `keymap/config → modal::palette_accessory → binding_chips →
Accessory::Keycaps → overlay_surface::render_list → frame::draw_keycap`.
Model/update owns mutations; render only projects them. Existing parser and
scale tests are in [overlay_surface.rs](../../src/view/overlay_surface.rs) and
[frame.rs](../../src/view/frame.rs). Add deterministic-font tests for the 78px
trace, 1.25x measure=paint width, and no cap `HitTarget`/focus entry. A clipping
test belongs to Settings' viewport/control path; normal overlay list paint is
not itself a clipping guarantee.
