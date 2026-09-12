# Form

## Purpose and current Token form

A Form groups labelled inputs, choices, help, validation and actions into a
draft workflow. It is a container and ownership contract, not merely a vertical
stack. Token’s implemented open-ended settings forms are `SettingsForm` and
`FormField` ([settings/forms.rs](../../src/settings/forms.rs#L170)); each field
has label, help, editable draft and optional Browse action. The form owns draft
status, dirty/saving/removal state, focus, popup choice state, advanced state and
records scroll. `SettingsChange` remains typed and applies only after a command
completes ([settings/forms.rs](../../src/settings/forms.rs#L11)).

This preserves Elm flow: pointer/key intent becomes `SettingsMsg`, deterministic
update mutates draft or requests `Cmd`, runtime performs file-picker,
validation/persistence work, then `FormApplied` commits/announces result
([messages.rs](../../src/messages.rs#L283), [update/settings.rs](../../src/update/settings.rs#L520)).
Typing calls `changed`; Save validates `form.change` before `ApplySettingsForm`;
Cancel reconstructs the form from persisted configuration. Saving blocks
interaction. These are implemented behaviour, not a generic form framework.

### State and ownership

| Data                    | Implemented owner                                                                   | Contract                                                                                                                           |
| ----------------------- | ----------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------- |
| Persisted configuration | `AppModel.config`                                                                   | Read to construct form; ordinary field editing does not mutate it. `SettingsChange::apply` runs only after successful persistence. |
| Typed draft             | `SettingsForm { kind, fields, choices, enabled, preset }`                           | Fields hold editable values/cursors/selections; choices hold labels and active index. [forms.rs](../../src/settings/forms.rs#L179) |
| Async identity          | `session: Arc<()>`                                                                  | File choice, executable inspection and apply results are ignored unless their session still matches.                               |
| Transient UI            | `focused`, `dragging`, `open_select`, `select_cursor`, `records_scroll`, `advanced` | Update-only state, never configuration.                                                                                            |
| Submission              | `dirty`, `saving`, `remove_pending`, status strings                                 | Guards unsafe interaction and communicates draft/progress/validation status.                                                       |
| Effects                 | `Cmd` plus runtime                                                                  | Picker, executable inspection, persistence and LSP/inline reconfiguration stay outside the form.                                   |

`FormField` is only `label`, `help`, `EditableState<StringBuffer>`, and `browse`.
`FormChoice` is only label/help/static labels/active index. There is no current
field-level disabled, read-only, placeholder, stable option ID, invalid/error,
or accessible-description data; do not treat those as hidden behaviour.

### Implemented transitions

| Intent/event                                 | Deterministic update                                                        | Effect / outcome                                                                               |
| -------------------------------------------- | --------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------- |
| Open LSP/provider record                     | Construct draft from config; reset scroll/hover                             | May inspect a nonempty executable; config stays unchanged.                                     |
| Field pointer/drag                           | Select form row, focus indexed field, set cursor/selection                  | Reset blink; `EndFieldSelection` ends drag.                                                    |
| Edit, undo/redo, enable/choice/preset change | Mutate draft then `changed()`                                               | Dirty; removal confirmation clears; status says unapplied draft.                               |
| Browse                                       | Choice on a browsable field                                                 | `ChooseSettingsFile`; matching `FileChosen` changes draft.                                     |
| Save / Save & Use                            | `form.change(&config)` validates typed draft                                | Valid: lock `saving`, issue `ApplySettingsForm`; invalid: preserve draft and status error.     |
| Apply result                                 | Matching session clears saving                                              | Success applies change/refreshes entries and requests dependent work; failure preserves draft. |
| Cancel                                       | Reconstruct form from config when not saving                                | Discards draft, no write.                                                                      |
| Remove                                       | First request sets `remove_pending`; confirmation saves typed remove change | Never deletes immediately.                                                                     |

This is the LSP/AI configuration form, not a claim about the Keymap or all
simple Settings rows.

## Anatomy and rendering

Current form anatomy: title/breadcrumb/category navigation; optional master
enable; labelled field plus detail/help and optional Browse; preset/choice or
checkbox control; Advanced disclosure; record selection; draft status; Save,
Cancel and contextual destructive/secondary actions. Settings-page rendering
composes TextField, checkbox, select, button and shared overlay layout
([settings_page.rs](../../src/view/settings_page.rs#L1140)). The gallery has a
visual `form-field.validation` specimen only
([gallery.rs](../../src/model/gallery.rs#L405)); it is not a live validation
workflow.

Theme roles are mostly `overlay`: panel backgrounds/text, `recessed_wash`,
hairline/accent, selection wash and `severity_*`/`severity_*_text`
([theme.rs](../../src/theme.rs#L597)). Buttons have their separate `button.*`
palette. Inputs use Code font today; labels/help use UI painter sizing. All
control rectangles are scale-aware and overlay/form layout is the clipping and
ordering authority. No unified field error-border, disabled, required-marker,
success, or form-spacing theme role exists.

### Geometry, font, focus, and validation facts

`overlay_surface` solves Settings field, input, choice and footer rectangles;
painting and `OverlayHit` use that same layout. Pointer-to-text conversion calls
the field's `TextFieldOptions::position_at`, avoiding a second character-grid
projection ([settings_page.rs](../../src/view/settings_page.rs#L630)). Footer
buttons are right-aligned from their labels, 24×scale high with 6×scale gaps
([settings_page.rs](../../src/view/settings_page.rs#L748)).

Titles are UI text at 14×scale, labels 12×scale, help/actions 11×scale. Field
content is Code font via `TextFieldRenderer`; record IDs explicitly use Code
font ([settings_page.rs](../../src/view/settings_page.rs#L1026)). The rounded
form panel, field, and scrolling parent each clip their relevant paint; do not
rederive row rectangles outside the measured layout.

Focus is a selected settings row and optional `form.focused` field index.
Keyboard traversal includes fields, enable, advanced, presets/choices and footer
actions; a saving form rejects interaction. `saving` is a behavioural lock, not
a themed disabled state. `enabled` is the server/provider setting, not a
renderer-level disabled/read-only condition. Validation returns `FormError` into
form status: current code does not mark an individual invalid surface, move
focus to first invalid field, provide a tooltip, or expose invalid semantics.

## Proposed reusable form contract

**Proposed:** retain domain form models; share descriptors/layout only where
they do not erase typed validation. Each field should expose stable id, visible
label/help, required/disabled/read-only/invalid state, accessible description,
and messages. Form owns draft, focus order, submit/cancel and validation summary;
commands own I/O. Tab traversal follows visual order; Enter submits only when
unambiguous; Escape cancels/dismisses according to container policy; focus moves
to first invalid field on submit. Announce errors, async saving, and completion.

IntelliJ recommends labels aligned as a coherent group and validation on an
appropriate boundary; complex forms keep confirmation available, then highlight
invalid fields on submit ([Layout](https://plugins.jetbrains.com/docs/intellij/layout.html),
[Validation errors](https://plugins.jetbrains.com/docs/intellij/validation-errors.html)).
Its Kotlin UI DSL guidance in the local `temporary-docs/intellij-platform-sdk`
is secondary architectural context, not an implementation dependency.

## Gaps and acceptance

Add live gallery/automation coverage for complete keyboard traversal, field
errors and recovery, async saving/retry, Cancel draft discard, removal
confirmation, narrow/scroll clipping, disabled dependencies and screen-reader
names/status. Do not replace Settings with a command palette or introduce a
widget framework under this work. First useful slice is a shared explicit
validation presentation contract while preserving current typed `SettingsForm`
validation and command boundary.

Acceptance beyond the static `form-field.validation` gallery specimen: automate
initial focus and Tab order, multiline field clipping/selection, Browse return,
dirty navigation rejection, error and recovery, saving lockout, successful and
failed apply, Cancel discard, and two-step removal. Add disabled/read-only and
field-local error specimens only when their semantics/palette roles exist; test
names, descriptions and status announcements when an accessibility adapter is
introduced.
