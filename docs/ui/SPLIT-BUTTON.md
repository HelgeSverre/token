# Split button — proposed implementation contract

## Status and boundary

Token has **no split-button implementation**. This is intentionally a future
contract, grounded in the existing button painter and context-menu overlay;
none of the types in the proposed sections exist. Do not relabel Settings
Select, Find toggles, terminal tabs, or a menu trigger as a split button.

Use one only where an owner has a frequent default immediate action and a
small, related set of alternatives. The main action must not be duplicated in
the menu. A split button is not a value picker: the default does work, while a
Select changes a represented value. It is also not a toolbar requirement; the
text and icon presentations are the same semantic component.

## Existing pieces it may compose

**Current excerpt.** The only reusable half today is the pure painter:

```rust
pub enum ButtonState { Normal, Hovered, Pressed, Selected, Disabled }
pub struct ButtonStyle { pub state: ButtonState, pub focused: bool, pub text_size: Option<f32> }
pub fn render_button(frame: &mut Frame, painter: &mut TextPainter,
                     theme: &Theme, rect: Rect, label: &str, style: ButtonStyle);
```

It stores neither ID nor interaction. [Button](BUTTON.md) describes its
rounding and outset-ring behavior. Context-menu machinery is the reusable
popup precedent: `ContextMenuState` owns a `Vec<MenuItem>` and an open-time
pixel anchor, `CursorOverlayState` owns selection/hover, and
[overlay-surface](../../src/view/overlay_surface.rs) derives exactly the same
layout for paint and hit test. That code is **not** a split-button API and has
no way to identify a particular disclosure trigger.

Consequently there is no current source representation, consumer lifetime, or
existing test to document beyond those composition points. Any claim that a
split button currently has capture or accessibility behavior would be false.

## Proposed representation, identities, and lifetimes

**Proposed API (not implemented).** `ActionId`, `PointerId`, and `OwnerMsg`
are intentionally owner-defined stable types; an array index is never an action
identity. For the reducer below, `ActionId: Copy + Eq`; `PointerId: Copy + Eq`
names one pointer sequence; `OwnerMsg::Invoke(id)` means “send this stable ID
through the owner's existing message enum.” `Event` is defined in the sketch
below, not an unexplained runtime type: the owner converts its shared layout
hit result to `part`/menu `id`, and keyboard dispatch supplies the named
navigation events. `enabled`, `first_enabled`, `next_enabled`,
`open_menu`, `close_menu`, and `commit_menu` are also defined below and
read only the displayed fields.

```rust
enum SplitPart { Main, Disclosure }

struct SplitAction<ActionId> {
    id: ActionId,
    label: String,
    enabled: bool,
    destructive: bool,
}

struct SplitButton<ActionId> {
    main: SplitAction<ActionId>,
    alternatives: Vec<SplitAction<ActionId>>,
    open: bool,
    active_menu: Option<ActionId>,
    focused: SplitPart,
    press: Option<(PointerId, SplitPart)>,
    presentation: SplitPresentation,
}
enum SplitPresentation { Text, Icon }
struct SplitLayout { outer: Rect, main: Rect, disclosure: Rect, divider_x: f32, popup: OverlayLayout }
```

`alternatives` belongs to the feature model for the menu’s visible lifetime;
the render layer borrows it only during a frame. `SplitLayout` is derived and
must be rebuilt when outer bounds, scale/font/text, main label, alternative
labels/shortcuts, or window bounds change. `active_menu` must be `None` when
closed and name an enabled action when one exists. `open ⇒ alternatives` is
nonempty and all IDs are unique across main/alternatives; removal/reload that
breaks either closes the popup, clears capture, and repairs focus. Call
`reconcile_split` after any enablement/alternative mutation: it clears a
capture whose half became unavailable and either keeps an enabled active ID,
moves to the first enabled ID, or closes the menu. Revalidate availability at
activation because async work can change it after opening.

## Geometry and hit testing

Let `R=(x,y,W,H)` be the physical-pixel outer rectangle and `D` a measured,
scaled disclosure-cell width. Require `W ≥ D`; otherwise omit the control or
give disclosure `min(W,D)` and main width zero — never overlap targets.

```text
main       = [x,     y, max(0, W-D), H]
disclosure = [x+W-D, y, min(W,D),    H]
divider_x  = disclosure.x
hit(p) = Main if main.contains(p), Disclosure if disclosure.contains(p), Outside otherwise
```

The divider is paint only. Paint one joined outer surface and one divider, not
two independent focus rings. Anchor the popup to `(disclosure.x, outer.y,
outer.h)` and use `Anchor::Menu` flip/clamp geometry; it paints after normal
content, clipped by the window rather than `outer`.

**Worked trace.** Outer `(100,40,148,28)`, `D=28` gives main
`(100,40,120,28)`, disclosure `(220,40,28,28)`, divider x `220`.
`(219,54)` is main and `(220,54)` disclosure. At outer width 18, main is zero
and disclosure `(100,40,18,28)` remains reachable; a clipped main label never
steals the only affordance. Text must use measured UI font text and clip to
main. Icon form uses square cells but still needs accessible labels/tooltips.

## Reducer and input state machine

**Algorithm sketch (not compiling integration).** The following is complete
pseudocode in Rust notation: `MenuPointerActivate` is a pointer row click,
`MenuCommit` is Enter/Space in an open popup, and `KeyDown` means Down Arrow
on the focused main half. The owner supplies `part` from the geometry above.

```rust
enum Event<ActionId> {
    Down { pointer: PointerId, part: SplitPart },
    Up { pointer: PointerId, part: SplitPart, inside: bool },
    Cancel { pointer: PointerId },
    FocusLost,
    Escape,
    OutsidePress,
    Tab,
    KeyDown,
    KeyActivate,
    MenuMove { forward: bool },
    MenuPointerActivate(ActionId),
    MenuCommit,
}

fn enabled<ActionId>(s: &SplitButton<ActionId>, part: SplitPart) -> bool {
    match part { SplitPart::Main => s.main.enabled, SplitPart::Disclosure => s.alternatives.iter().any(|a| a.enabled) }
}
fn first_enabled<ActionId: Copy>(s: &SplitButton<ActionId>) -> Option<ActionId> {
    s.alternatives.iter().find(|a| a.enabled).map(|a| a.id)
}
fn next_enabled<ActionId: Copy + Eq>(s: &SplitButton<ActionId>, current: Option<ActionId>, forward: bool) -> Option<ActionId> {
    let enabled: Vec<_> = s.alternatives.iter().filter(|a| a.enabled).map(|a| a.id).collect();
    if enabled.is_empty() { return None; }
    let at = current.and_then(|id| enabled.iter().position(|candidate| *candidate == id));
    Some(match at { Some(i) => enabled[(i + if forward { 1 } else { enabled.len() - 1 }) % enabled.len()], None => enabled[0] })
}
fn open_menu<ActionId: Copy>(s: &mut SplitButton<ActionId>) {
    s.active_menu = first_enabled(s);
    s.open = s.active_menu.is_some();
}
fn close_menu<ActionId>(s: &mut SplitButton<ActionId>) { s.open = false; s.active_menu = None; }
fn reconcile_split<ActionId: Copy + Eq>(s: &mut SplitButton<ActionId>) {
    if s.press.is_some_and(|(_, part)| !enabled(s, part)) { s.press = None; }
    if s.open {
        s.active_menu = s.active_menu
            .filter(|id| s.alternatives.iter().any(|a| a.id == *id && a.enabled))
            .or_else(|| first_enabled(s));
        s.open = s.active_menu.is_some();
    }
}
fn commit_menu<ActionId: Copy + Eq>(s: &mut SplitButton<ActionId>) -> Option<OwnerMsg<ActionId>> {
    if !s.open { return None; }
    let id = s.active_menu?;
    if !s.alternatives.iter().any(|a| a.id == id && a.enabled) { return None; }
    close_menu(s);
    Some(OwnerMsg::Invoke(id))
}

fn reduce_split<ActionId: Copy + Eq>(state: &mut SplitButton<ActionId>, event: Event<ActionId>)
    -> Option<OwnerMsg<ActionId>>
{
    match event {
        Event::Down { pointer, part } if enabled(state, part) => { state.press = Some((pointer, part)); None }
        Event::Up { pointer, part, inside } if state.press == Some((pointer, part)) => {
            state.press = None;
            if !inside { return None; }
            match part {
                SplitPart::Main => state.main.enabled.then(|| OwnerMsg::Invoke(state.main.id)),
                SplitPart::Disclosure => { open_menu(state); None }
            }
        }
        Event::Cancel { pointer } if state.press.is_some_and(|(captured, _)| captured == pointer) => {
            state.press = None; None
        }
        Event::MenuMove { forward } if state.open => {
            state.active_menu = next_enabled(state, state.active_menu, forward); None
        }
        Event::MenuPointerActivate(id) if state.open => {
            if state.alternatives.iter().any(|a| a.id == id && a.enabled) {
                state.active_menu = Some(id);
                commit_menu(state)
            } else { None }
        }
        Event::MenuCommit | Event::KeyActivate if state.open => commit_menu(state),
        Event::Escape if state.open => {
            state.press = None; close_menu(state); state.focused = SplitPart::Disclosure; None
        }
        Event::OutsidePress | Event::FocusLost | Event::Tab => {
            state.press = None; close_menu(state); None
        }
        Event::KeyDown if state.focused == SplitPart::Main => { open_menu(state); None }
        Event::KeyActivate if state.focused == SplitPart::Main && state.main.enabled => Some(OwnerMsg::Invoke(state.main.id)),
        Event::KeyActivate if state.focused == SplitPart::Disclosure => { open_menu(state); None }
        _ => None,
    }
}
```

**Worked input trace.** With alternatives in durable order `A(enabled)`,
`B(disabled)`, `C(enabled)`, disclosure pointer release calls `open_menu`
and sets `open=true, active_menu=A`. Down calls `next_enabled(A, true)=C`;
another Down wraps to A; Up wraps to C. Enter/Space calls `commit_menu(C)`,
rechecks C remains enabled, closes the popup, and returns exactly
`OwnerMsg::Invoke(C)`. If an async refresh disables C after Down, the owner
first calls `reconcile_split`; it replaces invalid `active_menu=C` with the
first enabled alternative A while keeping the menu open. The subsequent
Enter/Space therefore invokes A, never stale C. A consumer that instead wants
to close on refresh must state and implement a different reconciliation policy.

Opening never invokes main. Matching-pointer release outside, window/capture
cancellation, removal, and focus loss clear `press` without an action; a
wrong-pointer release/cancel leaves the original capture intact. When open,
Up/Down uses `next_enabled` to wrap in durable display order, and Enter/Space
uses `commit_menu` to recheck the active ID before one invocation. Escape
restores focus to disclosure; Tab dismisses then lets owner traversal continue.
Disabled entries stay visible but cannot
capture or invoke. Dangerous alternatives are separated and their owner, not
this control, handles confirmation.

## Integration, invalidation, and verification

The first consumer builds alternatives from current model state and maps
`OwnerMsg::Invoke(id)` through its normal Update/Command path. Async results
retain owner ID plus generation; accept only matching replies, then close or
repair if active ID vanished. Never execute a stale row because its old index
is still in range. Projection/measurement is O(alternatives); invalidate
derived popup geometry on menu strings/shortcuts, availability if selection
changes, anchor, font/scale, theme, and window bounds.

| Setup                                                 | event sequence            | assertion                                           |
| ----------------------------------------------------- | ------------------------- | --------------------------------------------------- |
| main `Run`; alternatives `Run Tests`, `Run with Args` | press/release main        | one `Invoke(Run)`, popup closed                     |
| same, first alternative disabled                      | disclosure, Down, Enter   | opens, skips disabled, invokes only alternative     |
| capture main pointer 2                                | pointer 2 release outside | no invocation; capture cleared                      |
| open active `B`; matching refresh removes `B`         | apply reply               | popup closes/repairs by policy; never shifted index |
| outer width 18, `D=28`                                | pointer x=109             | disclosure hit, never main                          |
| anchor `(799,599,0)` in `800×600`                     | layout/hit                | popup clamped/flipped; rendered rect is hit rect    |

There are no existing split-button tests. A future static gallery needs
text/icon, focused halves, disabled action, long narrow label, keyboard row,
separated destructive row, and edge-clamped popup; automation must cover the
table’s actual transitions.
