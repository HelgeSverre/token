# Text field

## Purpose and current implementation

A text field owns editable text, cursor(s), selections and edit constraints;
the shared renderer only projects that model into a rectangle. Token uses
`EditableState<StringBuffer>` through [TextFieldContent](../../src/view/text_field.rs#L154),
not a generic field widget. It is used by modal prompts, Find, CSV editing,
Settings-form fields, and the gallery. The input model/update paths own typing,
clipboard, undo and effects.

### Current data and ownership

| Concern                                                | Implemented owner                 | Contract                                                                                                             |
| ------------------------------------------------------ | --------------------------------- | -------------------------------------------------------------------------------------------------------------------- |
| Text, edit constraints, cursor(s), selections, history | `EditableState<StringBuffer>`     | Passed through `TextFieldContent`; renderer reads but never mutates it.                                              |
| Active cursor and multiline capability                 | `TextFieldContent`                | Content provides active index and line policy; renderer has no independent focus model.                              |
| Viewport/colour/blink geometry                         | `TextFieldOptions`                | Caller computes physical-pixel projection and supplies text/cursor/selection colours.                                |
| Field surface / focus border                           | Caller via `render_field_surface` | Text renderer intentionally paints no border, placeholder, error or disabled state.                                  |
| Pointer/key messages                                   | Owning feature                    | Settings maps pointer to `Position`, edits in update; gallery owns its filter's input; renderer has no event method. |

`TextFieldOptions::for_text_box` derives visible characters as
`ceil(inner_width / char_width) + 1` and calls `calculate_scroll` to retain a
two-column margin around the active cursor. `for_modal` applies horizontal modal
padding; `for_text_area` derives rows from `rect.height / line_height`, projects
a negatively scrolled source y into the visible frame, and adjusts `scroll_y`
([text_field.rs](../../src/view/text_field.rs#L61)). Both painting and caret/hit
testing must receive the same options instance/equivalent calculation.

`TextFieldOptions` provides exact paint/interaction geometry: text area,
single-row or multiline viewport, character-grid metrics, colours, cursor blink,
and horizontal/vertical scrolling ([text_field.rs](../../src/view/text_field.rs#L16)).
`for_modal`, `for_text_box`, and `for_text_area` centralise projection; both
caret and pointer must use those options. `TextFieldRenderer` paints selections,
visible text and cursors using `FontRole::Code` and gives the matching
`caret_rect` ([text_field.rs](../../src/view/text_field.rs#L200)). It clips only
when its caller supplies a clip. It converts tabs to spaces and uses character
columns; it is therefore unsuitable for proportional text input.

The surrounding surface is separately painted by
[render_field_surface](../../src/view/controls.rs#L100). Settings uses the same
options to paint, hit test, selection-drag and caret
([settings_page.rs](../../src/view/settings_page.rs#L1200),
[update/settings.rs](../../src/update/settings.rs#L594)). Gallery specimens:
`field.unfocused`, `field.focused`, `field.selection`, `field.multiline`, and
`search-field.focused` ([gallery.rs](../../src/model/gallery.rs#L290)).

## Anatomy, state and visual roles

Implemented anatomy: optional external label/help; field surface; clipped code
text; selection(s); one or more carets; optional adjacent Browse button. Focus
is held by the owning modal/form, not `TextFieldRenderer`. The active caret is
bright; secondary carets are alpha-dimmed. `calculate_scroll` retains a margin
around the active cursor ([text_field.rs](../../src/view/text_field.rs#L367)).

### Current and proposed transitions

| Situation                | Current implementation                                                                                                             | Gap / proposed requirement                                                                                                                     |
| ------------------------ | ---------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------- |
| Focus                    | Owner decides focused field and whether cursor blinks; renderer receives only `cursor_visible`.                                    | Semantic field should expose focus, label/name and focus reason.                                                                               |
| Pointer placement/drag   | Settings resolves pointer through the same options and sends `FieldPointer`; update sets editable cursor/selection and `dragging`. | Every consumer must reuse `position_at`; no shared field pointer dispatcher exists.                                                            |
| Typing/edit/undo         | Owning editable/update path changes `EditableState`; renderer reflects next frame.                                                 | Keep input messages deterministic and side-effect free.                                                                                        |
| Cursor leaves viewport   | `for_text_box` horizontally scrolls active cursor; `for_text_area` calculates a vertical logical-line window.                      | Add explicit narrow/empty/IME/grapheme test cases.                                                                                             |
| Disabled/read-only/error | No renderer state.                                                                                                                 | Proposed: disabled is unfocusable/inert; read-only remains focusable/selectable; error has explicit visual and accessible invalid description. |
| Submit/Escape/Tab        | Container-specific, not `TextFieldRenderer`.                                                                                       | Proposed field reports editing intent; container decides submit/dismiss/focus traversal.                                                       |

The gallery's filter is an editable field fixture, while its `field.*` and
`search-field.focused` specimens exercise static paint state. Settings is the
concrete form consumer: field options are derived from measured layout, cursor
colours are assigned by the Settings painter, and `SettingsMsg::FieldPointer`,
`MoveFieldCursor`, and `UndoField` perform update work
([settings_page.rs](../../src/view/settings_page.rs#L1200),
[update/settings.rs](../../src/update/settings.rs#L602)).

Surface roles are `overlay.recessed_wash`, `hairline`, and focused `accent`;
content currently receives `overlay.text_primary`, `accent_bright`, and
`selection_wash` from its caller. The cursor/selection in other uses may instead
come from editor theme fields (`editor.cursor_color`, `selection_background`).
The font roles are explicitly split: code/editor is monospaced and UI is
proportional ([fonts.rs](../../src/view/fonts.rs#L10)). There are no dedicated
placeholder, disabled, error-border, error-icon, password, autocomplete, or
field-font theme fields.

### Trailing / built-in actions

`FormField::browse` marks fields that need a file picker
([settings/forms.rs](../../src/settings/forms.rs#L170)). Settings draws
`Browse…` through the shared Button painter in an **adjacent** `action_rect`,
not embedded in the `render_field_surface` rectangle
([settings_page.rs](../../src/view/settings_page.rs#L1200)). The action dispatches
the owner's `ChooseSettingsFile` command path, so it is intentionally not a
text-edit operation. It is current, feature-specific composition—not a generic
input-button API.

**Proposed:** a semantically field-owned trailing action has its own accessible
name and tooltip, preserves the field label/value relationship, is after the
text entry in focus order (unless its platform convention requires otherwise),
and reports/returns focus deliberately after its dialog closes. Do not use a
footer button for a single-field helper. This follows IntelliJ's direction to
use built-in controls for field-entry help such as browsing
([Input field](https://plugins.jetbrains.com/docs/intellij/input-field.html)).

## Proposed semantic contract

**Proposed:** a reusable field descriptor should carry `id`, label/help,
placeholder, required/disabled/read-only, validation state, `EditableState`,
and messages for edit/focus/submit rather than owning commands. Focus gains
caret-at-end or select-all only when the consumer specifies it. Keyboard must
support text editing, Shift selection, undo/redo, Tab traversal and Escape
delegated to its container; pointer press/drag uses the same `position_at`
projection. Expose role `textbox`, value, name from visible label, multiline
state, invalid state and error description to accessibility infrastructure.

Use a one-line field only when likely values cannot be enumerated; use a text
area for meaningful newlines, and a select/combobox for constrained choices.
That distinction, labels, placeholders, and label placement are primary-source
IntelliJ guidance ([Input field](https://plugins.jetbrains.com/docs/intellij/input-field.html),
[Text area](https://plugins.jetbrains.com/docs/intellij/text-area.html)).

## Edges, gaps, acceptance

Long content scrolls horizontally and multiline content vertically, but there
is no placeholder or inactive-value policy. `for_text_area` handles a parent
viewport scrolled above zero; callers must still clip field and parent. Empty,
narrow, and zero-height geometry are defensively clamped, yet grapheme-cluster,
IME, bidi, and screen-reader behaviour are unverified.

Next slice: make surface status (`normal/focused/disabled/error`) explicit,
then add semantic naming/invalid descriptions and gallery fixtures for
placeholder, disabled/read-only, validation recovery, keyboard focus, narrow
scrolling, multiline clipping, and Browse dialog focus return. IntelliJ recommends validation at an
appropriate commit boundary rather than disruptive per-keystroke validation
([Validation errors](https://plugins.jetbrains.com/docs/intellij/validation-errors.html)).
