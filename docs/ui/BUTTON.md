# Button — implementation reference

## Scope and reading path

A Token button is presently a **stateless paint primitive**, not a widget. It
turns a caller-owned rectangle, label, and visual state into pixels. It does
not retain identity, participate in focus traversal, receive pointer events, or
emit a message. Read this chapter when adding a visual action to a feature;
read [Menu](MENU.md) when the action reveals a transient choice list, and
[Split button](SPLIT-BUTTON.md) only when one action and its alternatives must
be independently operable.

The important boundary is `Message → Update → Command → Render`: feature
runtime code resolves a hit target and sends the feature message; its reducer
checks availability and returns a command; `render_button` only projects the
already-derived visual state. Calling the painter must never change the model.

## Current representation and ownership

**Current excerpt** — [src/view/button.rs](../../src/view/button.rs) is the
complete shared button contract.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ButtonState {
    #[default] Normal,
    Hovered,
    Pressed,
    Selected,
    Disabled,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ButtonStyle {
    pub state: ButtonState,
    pub focused: bool,
    pub text_size: Option<f32>,
}

pub fn render_button(
    frame: &mut Frame,
    painter: &mut TextPainter,
    theme: &Theme,
    rect: Rect,
    label: &str,
    style: ButtonStyle,
)
```

`rect` is in physical framebuffer pixels (`f32` until `render_button` rounds
each origin and extent); its borrowed lifetime is one render call. `label` is
also borrowed for that call. `ButtonStyle` is a copied presentation value,
not durable interaction state. `Frame` and `TextPainter` are mutable render
services; `Theme` is read-only. The feature owns every durable fact behind
these arguments: action identity, enabled condition, current selection,
hover region, press/capture state, and keyboard focus.

The painter's state mapping is precise:

| State      | Fill                                         | border          | text                                                    | focus ring   |
| ---------- | -------------------------------------------- | --------------- | ------------------------------------------------------- | ------------ |
| `Normal`   | `button.background`                          | `button.border` | `button.foreground`                                     | if `focused` |
| `Hovered`  | `background_hover`                           | `border`        | `foreground`                                            | if `focused` |
| `Pressed`  | `background_pressed`                         | `focus_ring`    | `foreground`                                            | if `focused` |
| `Selected` | optional `background_selected`, else pressed | `focus_ring`    | `foreground`                                            | if `focused` |
| `Disabled` | normal fill                                  | normal border   | optional `foreground_disabled`, else `overlay.text_dim` | never        |

`Selected` means only “paint this persistent choice as selected.” It does not
make a toggle, add a boolean, or change activation behavior. Likewise,
`Disabled` is visual only: an owner that emits an action for a disabled visual
button has a real bug. The two current consumers make this division visible:
Settings passes hover-derived styles to [its form action painter](../../src/view/settings_page.rs), while terminal chrome derives its hover from
`TerminalState::hovered_tab` before calling the same painter in
[src/panels/terminal.rs](../../src/panels/terminal.rs).

### Existing geometry and its invariants

Before drawing, the painter computes

```text
x = round(rect.x), y = round(rect.y)
w = round(rect.width), h = round(rect.height)       // converted to usize
line_h = text_size.map(line_height_for_size).unwrap_or(line_height)
text_w = round(measure(label, text_size))
text_x = x + saturating_sub(w, text_w) / 2
text_y = y + saturating_sub(h, line_h) / 2
```

It draws the bordered rectangle, then (when focused and not disabled) a
one-physical-pixel outset ring at `[x-1, y-1, w+2, h+2]`, with saturating
origins. It clips text to the original unrounded `rect`; consequently a label
may be clipped, but paint cannot escape the button surface. The ring is
intentionally drawn even for `0×0`, `2×2`, and `3×3` input rectangles; the
regression tests in [button.rs](../../src/view/button.rs) lock this down.

`button_rect(center_x, y, label, char_width, line_height, padding_h,
padding_v)` is a convenience derivation, not text layout:

```text
estimated_text_w = round(label.len() × char_width)  // UTF-8 bytes, not glyphs
w = estimated_text_w + 2 × padding_h
h = line_height + 2 × padding_v
x = saturating_sub(center_x, floor(w / 2))
```

It is safe only where the supplied average width intentionally represents the
font. Do not use it for proportional UI text, emoji, or non-ASCII labels;
measure through `TextPainter` once and use that measurement for both visual
placement and hit geometry.

**Worked trace.** For `rect=(100.4, 40.6, 63.2, 22.0)`, measured label width
`39.6`, and line height `14`, paint uses `(x,y,w,h)=(100,41,63,22)`,
`text_w=40`, then `(text_x,text_y)=(111,45)`. The focus ring occupies x
`99..=163`, y `40..=63`; its integer draw bounds come from the rounded
`63×22` size. The label is clipped by the original floating `Rect`, rather
than an asserted inclusive integer interval. With a
`2×2` rect at `(10,10)`, the ring still writes the four corners around
`(9,9)..(12,12)` even though the text has no room. That is required behavior,
not a layout recommendation.

### Current event path and failure boundaries

There is no `ButtonMsg`, `ButtonId`, or common button hit target. A feature
must derive its own `Rect` once, use the same rectangle in render and its
feature hit-test, and route its own message. This is an existing pattern, not
an implied component framework:

```text
winit pointer → runtime mouse dispatch → feature HitTarget
  → Msg::<Feature>(Activate(...)) → update::<feature>
  → availability check / Cmd → redraw → feature derives ButtonStyle
  → render_button(frame, painter, theme, rect, label, style)
```

The painter has no capture, release, cancellation, focus-loss, keyboard, or
accessibility machine. Therefore there is currently no general guarantee that
“press inside, release inside” is respected; whether it is correct depends on
the specific owner. It is also wrong to infer hover from a previous frame:
the runtime updates hover state from the current `HitTarget`, and a resize or
feature removal must make the stale target impossible before the next paint.

## Proposed semantic wrapper (not implemented)

Use this only after two real owners need identical transitions. It is a sketch,
not a type that exists in Token, and omitted names (`ActionId`, `UiMsg`) must be
defined by the owning feature rather than smuggled in as a global framework.

For this sketch, `ActionId: Copy + Eq` is a stable owner key (not an index),
`PointerId: Copy + Eq` identifies one OS pointer sequence, and the returned
`ActionId` is immediately wrapped by the owner in its existing `Msg`.
The only required event source is the owner's hit-test/layout pair, which
supplies `inside` from the same rectangle it rendered.

**Proposed API / algorithm sketch**

```rust
// Proposed only. ActionId is an owner-defined stable identity, not an index.
struct ButtonModel<ActionId> {
    id: ActionId,                 // durable identity while the owner contains it
    label: String,                // durable accessible/visible name
    enabled: bool,                // authoritative reducer precondition
    focused: bool,                // projection of the owner's focus manager
    press: Option<PointerId>,     // transient capture, cleared on all exits
}

enum ButtonEvent<ActionId> {
    PointerDown { id: ActionId, pointer: PointerId, inside: bool },
    PointerUp { pointer: PointerId, inside: bool },
    Cancel { pointer: PointerId },
    FocusLost,
    KeyActivate,
}

// Returns the owner message; it does not execute an effect.
fn reduce_button<ActionId: Copy + Eq>(model: &mut ButtonModel<ActionId>, event: ButtonEvent<ActionId>)
    -> Option<ActionId>
{
    if !model.enabled { model.press = None; return None; }
    match event {
        ButtonEvent::PointerDown { id, pointer, inside } if id == model.id && inside => {
            model.press = Some(pointer); None
        }
        ButtonEvent::PointerUp { pointer, inside } => {
            let captured = model.press.take() == Some(pointer);
            (captured && inside).then_some(model.id)
        }
        ButtonEvent::Cancel { pointer } if model.press == Some(pointer) => { model.press = None; None }
        ButtonEvent::FocusLost => { model.press = None; None }
        ButtonEvent::KeyActivate if model.focused => Some(model.id),
        _ => None,
    }
}
```

Required invariants are `press.is_some() ⇒ enabled`, captured pointer identity
must equal the releasing/cancelling pointer, and removing a focused/captured
button clears both before the next event. `Pressed` is a projection of a live
capture and pointer-inside test, never a durable “last clicked” value. The
owner repairs focus after removal by choosing its next enabled focus target,
or `None`; disabled targets are excluded from traversal and from activation.

The intended event table is: pointer down inside enabled button captures;
pointer move out keeps capture but paints non-pressed; release inside activates
once; release outside, OS cancel, window deactivation, removal, and focus loss
clear capture without activation. Space and Enter activate a focused enabled
button once on the documented key edge; Escape never activates; Tab and
Shift+Tab belong to the owner’s focus traversal. “Read-only button” is not a
useful state: expose a read-only value or disable an unavailable action with
an explanation.

## Layout, invalidation, and integration cost

The current painter has O(1) rectangle work plus label measurement/glyph
lookup and clipped glyph drawing. It owns no cache. `TextPainter`/glyph cache
owns measurement/render reuse; callers must invalidate derived geometry when
label, text size, UI font/scale factor, padding, or available rectangle
changes. Theme and state changes invalidate paint colors but do not normally
change geometry. A focus ring can draw one pixel outside `rect`, so adjacent
layouts need a one-pixel paint allowance or an enclosing clip policy.

For a future wrapper, retain only durable action data in the feature model;
derive `ButtonStyle` and rectangles each render or cache them behind exactly
the inputs above. Never cache a raw index as identity: a settings row removed
between pointer-down and pointer-up must cancel, not activate whatever shifted
into that index.

**Consumer assembly sketch.** A Settings-like owner would calculate
`save_rect` via measured UI text, use it in both its `HitTarget::SettingsAction`
construction and `render_button`, send `SettingsMsg::Save`, and have Update
recheck `can_save` before returning its command. The shared painter remains
below that reducer; it must not call `Cmd` or inspect Settings state.

## Verification vectors

Existing coverage in [button.rs](../../src/view/button.rs) verifies selected
fallback, disabled focus suppression, and tiny/zero-size ring behavior. Add
owner-level interaction tests before claiming semantic button behavior:

| Initial state                              | action                                       | expected result                                                |
| ------------------------------------------ | -------------------------------------------- | -------------------------------------------------------------- |
| enabled, `rect=(10,10,40,20)`, no capture  | down `(20,15)`, up `(20,15)`                 | exactly one owner activation                                   |
| same                                       | down `(20,15)`, move `(55,15)`, up `(55,15)` | capture cleared; zero activations                              |
| disabled but hit geometry overlaps pointer | down/up inside or Enter                      | zero activation and no focus ring                              |
| capture pointer 7                          | release pointer 8, then cancel pointer 7     | no activation; capture cleared only by 7’s cancellation        |
| label `"é"`, proportional UI font          | layout and click                             | measured glyph width defines both; never `label.len() × width` |
| `rect=(0,0,0,0)`, focused                  | render                                       | no panic; outset ring is safely clipped by frame bounds        |

The gallery’s existing button samples are visual fixtures, not proof of
pointer capture, keyboard focus, or accessibility. A future semantic wrapper
needs automation coverage for the table above plus a real focus traversal.
