# Toggle switch

## Current status and boundary

Token has no toggle-switch painter or semantic switch. A Settings collection
master action named ToggleMaster and Off/On checkbox-like rows are domain
behaviour, not a switch implementation:

| Related current item                   | Owner                             | Why it is not a switch                                       |
| -------------------------------------- | --------------------------------- | ------------------------------------------------------------ |
| ButtonState::Selected                  | button caller                     | persistent button paint, no Boolean/thumb/track              |
| Off/On form row                        | SettingsForm choice/enabled owner | square checkbox painter, no switch geometry                  |
| SettingsCollectionAction::ToggleMaster | settings update                   | domain action that can save/reconfigure, no visual component |

The meaningful current flow is
SettingsCollectionAction::ToggleMaster -> update/settings -> either LSP toggle
command or inline-provider config mutation plus SaveConfiguration/Redraw
([update/settings.rs:544](../../src/update/settings.rs#L544)). It must remain
separate from any future visual control.

## Actual current representations

The only generic visual state that resembles a persistent toggle is the button
style:

```rust
pub enum ButtonState {
    Normal, Hovered, Pressed, Selected, Disabled,
}
pub struct ButtonStyle {
    pub state: ButtonState,
    pub focused: bool,
    pub text_size: Option<f32>,
}
```

This is borrowed paint input. Selected chooses
button.background_selected (falling back to background_pressed) and focus ring;
it has no durable bool, no identity, no input capture, and no accessible switch
state. Disabled changes paint text but callers must suppress activation.

The Settings message enum has:

```rust
pub enum SettingsCollectionAction {
    ToggleMaster,
    ToggleSelect(usize),
    CloseSelect,
    Select(usize),
    Add,
}
```

ToggleMaster has no target ID because it is scoped by the current SettingsForm
kind. For LanguageServer, update delegates to toggle_lsp_enabled; for
InlineProvider, it flips config.completion.inline.enabled and emits a save
command. The value belongs to those domain models, not a button/switch object.
It can be asynchronous or persisted, so a generic renderer cannot correctly
speculate or repair it.

No current type has track width/height, thumb bounds, checked field, switch
label/help, pressed pointer, hover, focus order, disabled/read-only, error,
gallery specimen, or switch hit test. Therefore all press/release/cancel and
focus/disabled transitions are absent rather than undocumented.

## Current state × event matrix

| Event                                | Existing result                                             |
| ------------------------------------ | ----------------------------------------------------------- |
| render selected button               | caller chooses Selected; button paints selected surface     |
| click Off/On Settings row square     | checkbox-like Choice, not ToggleMaster                      |
| click collection master hit area     | emits ToggleMaster                                          |
| ToggleMaster, LanguageServer         | update returns LSP enable/disable command plus redraw       |
| ToggleMaster, InlineProvider         | flips config Boolean, returns SaveConfiguration plus redraw |
| saving Settings form                 | collection action returns redraw without mutation           |
| pointer press/release outside/cancel | no switch state/capture exists                              |
| Space/Enter/arrows/Tab/focus loss    | no switch keyboard/focus reducer exists                     |
| disabled/read-only/mixed/replacement | no switch model exists                                      |

Calling a master action a “switch” in user documentation would falsely claim
visual and accessibility behaviour that the source does not implement.

## Proposed switch contract (not current API)

A switch is justified only when a live, immediate setting needs a distinct
track-and-thumb affordance. Dialog/form booleans should continue to use the
checkbox primitive. A future model needs clear ownership:

```rust
// Proposed API — not implemented.
struct SwitchModel<Id> {
    id: Id,
    label: String, help: Option<String>,
    checked: bool,              // durable value owned by consumer
    enabled: bool, read_only: bool,
    invalid: Option<String>,
    focused: bool,
    pressed: Option<u64>,       // input transient only during capture
}
struct SwitchLayout {
    label: WidgetRect, track: WidgetRect, thumb: WidgetRect,
    thumb_diameter: usize,     // physical px; equals thumb.w and thumb.h
    hit: WidgetRect,            // union/expanded target in physical px
}
enum SwitchMsg<Id> { Toggle(Id), CancelPress }
```

checked must be supplied from the owner’s committed or deliberately staged
value. A renderer never writes it. Layout owns all physical-pixel geometry:
paint and hit test use exactly the same track/thumb/hit plan. thumb movement is
derived from checked and track, not stored independently. A minimum-thumb rule
must avoid negative travel. The proposed constructor defines requested_d as a
logical size rounded through px(n)=max(1,round(n*scale)), then uses
d=min(requested_d,track.w,track.h). This is an explicit oversize policy: a
thumb never exceeds either track dimension; if either dimension is zero, layout
returns no interactive SwitchLayout. The complete physical-pixel calculation is:

```text
d      = min(requested_d, track.w, track.h)
travel = track.w - d                             // safe because d <= track.w
thumb.x = track.x + (checked ? travel : 0)       // LTR example
thumb.y = track.y + track.h.saturating_sub(d)/2
thumb   = WidgetRect { x: thumb.x, y: thumb.y, w: d, h: d }
union.l = min(track.left, label.left);  union.t = min(track.top, label.top)
union.r = max(track.right, label.right); union.b = max(track.bottom, label.bottom)
hit.l   = max(parent.left, union.l); hit.t = max(parent.top, union.t)
hit.r   = min(parent.right, union.r); hit.b = min(parent.bottom, union.b)
hit     = hit.l < hit.r && hit.t < hit.b
        ? Some((hit.l, hit.t, hit.r-hit.l, hit.b-hit.t)) : None
```

Here left/top are x/y and right/bottom are x+w/y+h with checked/saturating
construction; the parent supplies a physical clip rectangle. Thus all four
intersection sides are clamped and an entirely clipped union becomes an empty,
non-interactive result rather than an underflowing rectangle.
The proposed constructor signature is
switch_layout(label, track, requested_d, checked, parent_clip) -> Option<SwitchLayout>;
it returns None before the equations when track.w==0 or track.h==0.

Worked trace: track=(300,100,36,20), requested_d=16 gives d=16, travel=20 and
thumb=(300,102,16,16) off or (320,102,16,16) on. For track=(300,100,12,20),
requested_d=16 gives d=12, travel=0, y=104, and both states are
(300,104,12,12): they remain inside the track rather than clipping or
underflowing. The layout constructor, not input code, rejects zero-size tracks.

| Proposed event                                    | Preconditions              | Result                                            |
| ------------------------------------------------- | -------------------------- | ------------------------------------------------- |
| press track/hit target                            | enabled and writable       | capture pointer; pressed=true                     |
| release on same target                            | capture valid              | clear press; emit Toggle(id) once; keep focus     |
| release outside/other, pointer cancel, focus loss | capture                    | clear press; no Toggle                            |
| Space/Enter                                       | focused, enabled, writable | emit Toggle(id) once                              |
| Tab/Shift+Tab                                     | enabled                    | one focus stop; disabled skipped                  |
| read-only activation                              | read-only                  | focus/report checked value; no Toggle             |
| model replacement/removal                         | ID gone                    | clear focus/capture; owner chooses next focus     |
| async commit returns stale                        | generation/owner mismatch  | ignore result; do not overwrite replacement value |

First implementation should deliberately omit drag behaviour. If drag is later
added, store pointer ID and starting checked value, define a threshold/travel
mapping, and make cancel restore the original visual value without emitting a
domain mutation.

## Integration, invalidation, and verification

A future consumer follows the existing architecture:

```text
runtime pointer/key -> Msg::Settings or feature Msg
 -> deterministic update toggles durable/staged Boolean
 -> Cmd::SaveConfiguration / LSP effect if owner requires it
 -> renderer receives derived SwitchModel/Layout
```

A settings consumer must retain a session/generation for async saves and ignore
a reply for an obsolete form, the same owner-check reason SettingsForm has
session identity. ToggleMaster should not be mechanically replaced: its LSP and
inline-provider effects are different contracts.

Proposed layout/paint cost is O(1) plus label glyph work. Invalidate when checked,
enabled/read-only/invalid/focused/pressed state, bounds, scale, label/font
metrics, theme, layout direction, or parent clipping changes. No current cache,
benchmark, or performance claim exists.

| Initial condition                                     | Action                            | Expected                                                      |
| ----------------------------------------------------- | --------------------------------- | ------------------------------------------------------------- |
| current language-server master                        | ToggleMaster                      | LSP toggle command + redraw; no Switch instance               |
| current inline-provider master=false                  | ToggleMaster                      | true then SaveConfiguration + redraw                          |
| Settings form saving=true                             | ToggleMaster                      | no mutation                                                   |
| proposed track=(300,100,36,20), diameter=16           | off/on layout                     | thumb x=300/320, y=102                                        |
| proposed track=(300,100,12,20), requested diameter=16 | derive                            | d=12, travel=0, thumb=(300,104,12,12) for both states         |
| proposed zero-width or zero-height track              | derive                            | no interactive layout/hit target                              |
| proposed enabled switch                               | press/release same target         | exactly one Toggle and focus retained                         |
| proposed capture                                      | release outside/cancel/focus loss | no Toggle, pressed clears                                     |
| proposed disabled/read-only                           | Tab/click/Space                   | disabled inert/skipped; read-only reports but does not toggle |
| proposed stale async reply                            | owner generation differs          | ignored                                                       |

Role switch, visible-label accessible name, checked/disabled/read-only/invalid
state, and error description are future semantic bridge requirements. They are
not supplied by ButtonState or SettingsCollectionAction today.
