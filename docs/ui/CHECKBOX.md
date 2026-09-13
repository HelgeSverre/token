# Checkbox

## What exists

Token has a checkbox **mark painter**, not a semantic Checkbox widget.
render_checkbox in [controls.rs:9](../../src/view/controls.rs#L9) draws a caller
supplied square. Settings recognizes exactly labels ["Off", "On"] as a
checkbox-like form accessory ([settings_page.rs:114](../../src/view/settings_page.rs#L114)).
The gallery fixtures checkbox.off and checkbox.on are static paint samples.

The actual Settings flow is:

```text
SettingsForm.enabled / FormChoice.active (durable draft)
 -> overlay Settings Row Accessory::Choices (borrowed label/active state)
 -> checkbox_rect + render_checkbox
 -> shared overlay hit test -> OverlayHit::Choice
 -> form-choice/FormEnabled update -> form.changed -> Cmd::Redraw
```

The painter never owns label, help, hover, press, focus, disabled, mixed,
accessibility, persistence, or input state.

## Existing representation, ownership, and geometry

**Current painter signature**:

```rust
pub fn render_checkbox(
    frame: &mut Frame, painter: &mut TextPainter, theme: &Theme,
    rect: WidgetRect, checked: bool, scale: f64)
```

rect x/y/w/h are physical usize pixels, supplied by the caller. checked is a
borrowed visual Boolean. scale is physical pixels per logical pixel and makes
the check mark 12*scale physical px. The painter fills rect using
overlay.accent when checked, overlay.recessed_wash otherwise, always borders
with overlay.hairline, and when checked truncates/draws a check glyph using
overlay.text_bright. It clips no outer mark geometry itself; the caller owns
rect validity and any parent clipping. It does not force a UI or code font.

Settings defines the actual 1x physical mark box:

```text
size = round(14*scale)
x = row.x + saturating_sub(row.w, size + round(4*scale))
y = row.y + round(10*scale)
w = h = size
```

For row=(100,200,300,56), scale=1, box=(382,210,14,14). At 1.25,
size=18, right inset=5, so box=(377,213,18,18). This right-aligned square is
the only checkbox hit target: a label click is a row hit, not a toggle.

A FormChoice owns a raw usize active index; enabled is a separate Boolean on
SettingsForm. Neither is passed directly to the painter. Settings first projects
the current row into Accessory::Choices { labels, active: Option<usize> }:
the checkbox render expression is *active == Some(1). Thus Some(1) means the
projected On choice, Some(0) Off, and None means no projected choice and paints
off. This distinction matters when inspecting the source: FormChoice.active is
not an Option and cannot itself be compared to Some(1). The collection master
uses SettingsCollectionAction::ToggleMaster and may run LSP/config effects; a
form enabled row changes only its draft. Their visual resemblance must not
merge their ownership.

## Current state × event behaviour

| Event                           | Preconditions                                                   | Current state/effect                                        |
| ------------------------------- | --------------------------------------------------------------- | ----------------------------------------------------------- |
| render checked                  | projected Accessory::Choices.active==Some(1), or master enabled | accent square + check                                       |
| pointer inside box              | labels exactly Off/On                                           | OverlayHit::Choice with opposite index                      |
| pointer elsewhere in Off/On row | same row                                                        | OverlayHit::Row, not toggle                                 |
| choice accepted                 | FormEnabled: choice 0/1                                         | enabled=choice==1; clear field focus; mark draft changed    |
| choice accepted                 | FormChoice                                                      | validate choice<label count; set active; mark draft changed |
| collection master click         | collection form                                                 | ToggleMaster; LSP/provider owner chooses command/effect     |
| saving form                     | collection action                                               | update rejects mutation and redraws                         |
| press/release/cancel/focus loss | no checkbox state exists                                        | no checkbox transition                                      |
| disabled/read-only/mixed        | no representation                                               | no semantic behaviour                                       |

The update path is [update/settings.rs:137](../../src/update/settings.rs#L137)
for form choices/enabled and [update/settings.rs:544](../../src/update/settings.rs#L544)
for collection master. Existing visual fixtures do not establish keyboard or
assistive semantics.

## Proposed semantic checkbox (not current API)

```rust
// Proposed API — not implemented.
enum CheckValue { Off, On, Mixed }
struct CheckboxModel<Id> {
    id: Id, label: String, help: Option<String>,
    value: CheckValue, enabled: bool, read_only: bool, invalid: Option<String>,
    focused: bool, pressed: Option<u64>,
}
enum CheckboxMsg<Id> { Toggle(Id), CancelPress }
enum Event {
    Press { pointer: u64 }, Release { pointer: u64 }, Cancel { pointer: u64 },
    FocusLost, Activate,
}
```

Mixed must have a defined aggregate/loading meaning, not be a vague third
Boolean. Durable value, validation, persistence, and effects remain in the
consumer update owner. The label uses UI font; the existing mark painter can
remain the final fill/glyph step.

**Proposed layout algorithm** — WidgetRect has physical usize x/y/w/h, and
px(n)=max(1,round(n*scale)). measure_ui is a supplied UI-font measurement, not
an unstated fixed character width:

```rust
// Proposed algorithm — no CheckboxLayout currently exists.
struct CheckboxLayout { box_rect: WidgetRect, label: WidgetRect, hit: WidgetRect }

fn checkbox_layout(row: WidgetRect, label_w: usize, scale: f64) -> CheckboxLayout {
    let requested_side = px(14.0, scale);
    let side = requested_side.min(row.w).min(row.h);
    let gap = px(8.0, scale);
    let box_rect = WidgetRect { x: row.x, y: row.y + row.h.saturating_sub(side)/2,
                                w: side, h: side };
    let label_x = box_rect.x.saturating_add(side).saturating_add(gap);
    let right = row.x.saturating_add(row.w);
    let label = WidgetRect { x: label_x, y: row.y,
        w: label_w.min(right.saturating_sub(label_x)), h: row.h };
    CheckboxLayout { box_rect, label,
        hit: WidgetRect { x: row.x, y: row.y, w: row.w, h: row.h } }
}
```

For row=(100,200,180,24), scale=1, requested_side=side=14 and gap=8,
box=(100,205,14,14).
If measured label width is 210, label is clipped to x=122,w=158 and the hit
rect remains (100,200,180,24). A row narrower than side+gap produces label
width zero but never an overflowing unsigned rectangle. Parent layout decides
whether that degenerate control is displayed; hit testing never invents a
larger label rectangle.

**Proposed reducer algorithm**:

```rust
fn checkbox_event<Id: Clone>(m: &mut CheckboxModel<Id>, e: Event, hit: bool)
    -> Option<CheckboxMsg<Id>> {
    match e {
        Event::Press { pointer } if hit && m.enabled && !m.read_only => {
            m.pressed = Some(pointer); m.focused = true; None
        }
        Event::Release { pointer } if m.pressed == Some(pointer) => {
            m.pressed = None;
            hit.then(|| CheckboxMsg::Toggle(m.id.clone()))
        }
        Event::Release { .. } => None,
        Event::Cancel { pointer } if m.pressed == Some(pointer) => { m.pressed = None; None }
        Event::Cancel { .. } => None,
        Event::FocusLost => { m.pressed = None; None }
        Event::Activate if m.focused && m.enabled && !m.read_only =>
            Some(CheckboxMsg::Toggle(m.id.clone())),
        _ => None,
    }
}
```

Event and Id: Clone are dependencies of this proposed sketch. The owner maps
Toggle to Off<->On (or its declared Mixed resolution) and redraws from the new
durable value; the reducer never changes value optimistically.

| Proposed event                    | Preconditions              | Reducer result                                       |
| --------------------------------- | -------------------------- | ---------------------------------------------------- |
| press box or label                | enabled and writable       | capture pointer; pressed=true                        |
| release on same combined target   | capture valid              | clear press; Toggle(id) once; focus remains          |
| release outside/cancel/focus loss | captured                   | clear press; no toggle                               |
| Space/Enter                       | focused, enabled, writable | Toggle(id) once                                      |
| Tab                               | enabled                    | one focus stop; disabled is skipped                  |
| read-only activation              | read-only                  | report/value remains, no Toggle                      |
| removal/replacement               | ID absent                  | clear focus/capture; owner selects next focus target |

For a binary setting, Toggle maps Off<->On. For Mixed, the owner must state
whether activation goes to On or an aggregate resolution; test it explicitly.

## Cost, invalidation, and verification

Paint is O(1) plus one glyph truncation when checked. The current layout
depends on row rect, scale, checked state, theme, painter metrics, and parent
clip. Invalidate derived hit rectangles on any of those; a Settings row
replacement also invalidates its raw active-index interpretation. No interaction
cache or timing claim exists.

Existing coverage is static gallery paint only
([model/gallery.rs:304](../../src/model/gallery.rs#L304)). Add or preserve these
tests at the owner level:

| Initial condition             | Action                                       | Expected                                |
| ----------------------------- | -------------------------------------------- | --------------------------------------- |
| row=(100,200,300,56), scale=1 | derive box                                   | (382,210,14,14)                         |
| Off/On active=0               | box click                                    | Choice(1), then checked render          |
| Off/On active=1               | label-side click                             | Row, no toggle                          |
| FormEnabled=true              | choice=0                                     | enabled=false, focused=None, dirty=true |
| saving=true                   | master/choice activation                     | no model mutation                       |
| proposed checked capture      | press then release outside/cancel/focus loss | no Toggle, pressed clears               |
| proposed Mixed aggregate      | activate                                     | documented deterministic resolution     |
| proposed disabled             | Tab/click/Space                              | no focus and no Toggle                  |

Role checkbox, accessible name from label, checked/mixed/disabled/invalid state,
and error description are future semantic-bridge obligations, not current facts.
