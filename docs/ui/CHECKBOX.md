# Checkbox

## Purpose and implemented overlap

A checkbox expresses an independent yes/no choice; a group permits multiple
choices. It does not currently exist as a semantic Token widget. The implemented
primitive is `render_checkbox(frame, painter, theme, rect, checked, scale)`
([controls.rs](../../src/view/controls.rs#L9)): it draws a square with either
`overlay.recessed_wash` or `overlay.accent`, a `hairline`, and a clipped ✓ in
`text_bright`. It accepts no label, hover, pressed, focused, disabled,
indeterminate, event or accessible-name state.

Settings recognizes a two-label `Off`/`On` convention as a checkbox-like
accessory and owns commitment in its update layer; form rows paint the primitive
([settings_page.rs](../../src/view/settings_page.rs#L1275)). The gallery covers
`checkbox.off` and `checkbox.on` ([gallery.rs](../../src/model/gallery.rs#L308)).
The current Settings rendering supplies visual state only; do not claim keyboard
or assistive-technology semantics from it.

### Current ownership and transitions

| Layer                | Current data / job                                                                                                                                              | What it does not own                                                        |
| -------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------- |
| `render_checkbox`    | `rect`, `checked`, `scale`, painter/theme                                                                                                                       | Label, focus, pointer hit target, enabledness, update message, persistence. |
| Settings row adapter | Recognises exactly `labels == ["Off", "On"]`; maps active index 1 to checked. [settings_page.rs](../../src/view/settings_page.rs#L114)                          | A reusable checkbox model or tri-state semantics.                           |
| Settings form        | `SettingsForm.enabled` for a form enable row                                                                                                                    | Master collection enable semantics, which use `ToggleMaster` separately.    |
| Update/commands      | Converts a row choice to `form.enabled`, marks draft dirty; master action can issue LSP/config effects. [update/settings.rs](../../src/update/settings.rs#L155) | Checkbox paint state.                                                       |

| Current trigger                                            | Result                                                                                                                                                                                                 |
| ---------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Pointer hits checkbox square in an `Off`/`On` Settings row | Hit testing returns choice `0` or `1`, the opposite of the currently active value; clicking elsewhere on that row is a row hit, not a toggle. [settings_page.rs](../../src/view/settings_page.rs#L676) |
| Form enable choice accepted                                | `form.enabled = choice == 1` (or toggles when invoked without explicit choice), focus clears, draft becomes dirty.                                                                                     |
| Collection master checkbox                                 | Emits `SettingsCollectionAction::ToggleMaster`; LSP/provider owner decides persistence/effect. [update/settings.rs](../../src/update/settings.rs#L544)                                                 |
| Form is saving                                             | Settings action/update rejects interaction.                                                                                                                                                            |

The visible master checkbox and a form's `enabled` row therefore look alike but
do not share a commit path. The gallery specimens are static paint fixtures; no
generic Checkbox type has focus or keyboard transitions.

### Implemented geometry, theme, and font facts

In a standard Settings row the square is `14 × scale`, positioned `10 × scale`
from the row top and `4 × scale` from its right edge
([settings_page.rs](../../src/view/settings_page.rs#L122)). The painter draws the
whole supplied rectangle; it has no intrinsic minimum size or clipping scope.
Its check glyph is measured/truncated at `12 × scale` with the caller's current
font, so it does not mandate UI or Code font. The adjacent master label is
separately UI text at 12×scale ([settings_page.rs](../../src/view/settings_page.rs#L911)).

Implemented roles are only `overlay.recessed_wash` (off), `overlay.accent` (on),
`overlay.hairline`, and `overlay.text_bright`; focus, hover, pressed, disabled,
mixed and error roles are absent. A semantic checkbox cannot communicate a
disabled/read-only distinction today: both must be introduced as data and paint
states, and read-only must remain focusable/announced if its value is useful to
inspect while disabled is skipped and inert.

## Proposed semantic checkbox

**Proposed:** a checkbox descriptor contains stable id, visible label/help,
`checked: bool` or deliberate `mixed`, enabled, focus and `Toggle(id)` message.
The hit target includes box and label. Click, Space and Enter toggle if enabled;
Tab traverses; disabled is skipped and cannot emit. It must expose role
`checkbox`, name from label, checked/mixed/disabled/invalid state, and error
description. A parent control that disables dependent rows must make that
relationship programmatic as well as visual.

Use an imperative, short, non-negated sentence-case label on the right of the
box. IntelliJ recommends checkbox for yes/no or multiple independent choices,
radio buttons for exclusive alternatives, and supports an indeterminate state
where it actually represents a mixed/loading aggregate
([Checkbox](https://plugins.jetbrains.com/docs/intellij/checkbox.html)).

## Theme/layout/acceptance

The only implemented tokens are the overlay roles above; there are no
checkbox-specific hover, focus, disabled or error tokens, and no font role for
the missing label. Scale the square and its check consistently; its label should
use UI font and wrap/clip according to the parent form, not the painter.

Add gallery specimens for hover/pressed/focus/disabled/mixed/error, a long
label, label-side hit target, high scale and keyboard toggling. First reusable
slice: add semantic state and focus outline while retaining `render_checkbox`
as its mark painter; avoid converting all Settings `Off`/`On` rows until their
existing selection and draft behaviour is preserved.

Acceptance should include a live Settings form enable-row transition and a
collection-master transition as separate cases, plus render fixtures for
off/on/focus/hover/pressed/disabled/mixed/error, label-side hit target, long
label wrapping, fractional scale and keyboard activation. Verify that proposed
mixed is not used as a vague third boolean: its model must state the aggregate or
loading meaning and its accessible state.
