# Toggle switch

## Status and naming

Token has **no switch painter or switch component**. The Settings form has a
master enable action (`ToggleMaster`) and uses checkbox-like visual treatment in
other boolean rows ([messages.rs](../../src/messages.rs#L327),
[update/settings.rs](../../src/update/settings.rs#L544)); that is not an
implemented toggle switch. Do not rename a checkbox or selected button “switch”
without a distinct visual and event contract.

| Existing overlap        | Current owner/behaviour                                                       | Why it is not a switch                                                                                   |
| ----------------------- | ----------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------- |
| `ButtonState::Selected` | Caller paints a persistent choice colour.                                     | No boolean value, thumb/track, toggle message, keyboard or switch accessibility.                         |
| Settings `Off`/`On` row | Checkbox painter plus feature-owned active index.                             | Semantically a checkbox-like boolean; no switch geometry/state.                                          |
| `ToggleMaster`          | Settings update toggles LSP/provider master setting and may save/reconfigure. | A domain message, not a reusable visual control. [update/settings.rs](../../src/update/settings.rs#L544) |

No switch fields, painter, hover/focus/disabled palette, hit-test geometry,
gallery specimen, or accessibility mapping are implemented.

IntelliJ calls an on/off control in search results a _toggle button_ and
explicitly recommends checkboxes in dialogs and menus instead
([Toggle button](https://plugins.jetbrains.com/docs/intellij/toggle-button.html)).
For Token Settings, checkbox is therefore the default component; a future switch
needs a concrete non-dialog consumer before implementation.

## Proposed contract if a real switch is justified

State is `checked`, enabled, focus, label/help and an owner `Toggle(id)` message;
the switch owns no storage/effects. Click track/thumb, Space, and Enter toggle;
Tab traverses; disabled does neither activate nor receive focus. It exposes role
`switch`, visible-label name, checked, disabled and invalid/error description.
The label must name the setting, never append “On”/“Off”; the state is announced
separately. Pointer drag must either be deliberately supported with cancel-on-
release-outside semantics or omitted—click semantics are safer for the first
slice.

There are no switch theme roles today. A future painter needs normal/hover/
pressed/focused/disabled track and thumb, on/off contrast, focus ring and error
roles, plus UI-label font, scaling and clipping. Do not inherit only button
selected colour: it cannot represent both thumb and track or accessible focus.

| Proposed data                   | Required ownership/invariant                                                                                                |
| ------------------------------- | --------------------------------------------------------------------------------------------------------------------------- |
| Stable id, label, help, checked | Owner persists/updates typed boolean; label names setting, state is separate.                                               |
| enabled/read-only/invalid/focus | Explicit state. Disabled is inert/skipped; read-only is focusable and reports state but does not toggle.                    |
| track/thumb geometry            | One shared layout plan drives paint and pointer hit testing; do not hit-test a visual thumb with separately derived bounds. |
| `Toggle(id)` message            | Update commits one state transition; runtime owns any persistence/effect.                                                   |

| Proposed transition                | Required result                                                          |
| ---------------------------------- | ------------------------------------------------------------------------ |
| Click/Space/Enter enabled switch   | Toggle exactly once and retain focus.                                    |
| Pointer press then release outside | Cancel, unless a documented drag contract is introduced.                 |
| Tab/Shift+Tab                      | Traverses once; no focus for disabled switch.                            |
| Model replacement/removal          | Remove safely from focus order; announce resulting value/unavailability. |

Acceptance: gallery specimens for off/on, focus, hover/pressed, disabled,
error, high contrast and long label; automated focus/keyboard and no-activation
disabled tests; confirm Settings still uses checkbox. Until then this document
is a deliberate exclusion, not a widget request.
