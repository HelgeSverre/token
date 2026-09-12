# Combo box

## Purpose and name

A **combo box** accepts a selected value or custom text. A finite non-editable choice is **Select** (the IntelliJ catalog calls it a drop-down list); a large known universe is an input with completion. Do not call either a combo box merely because it has an arrow.

IntelliJ defines a combo box as an input plus drop-down list, recommends it for custom values and revisiting prior values, and recommends a drop-down/radio control for a finite list. [Combo Box](https://plugins.jetbrains.com/docs/intellij/combo-box.html) is primary guidance, not a Swing implementation requirement.

## Current Token contract — high confidence

**Token has no editable combo box.** `src/model/select.rs` and `src/view/select.rs` provide a non-editable `Select`; Settings also has a feature-local anchor/option painter in `src/view/controls.rs`. The Gallery exercises `select.closed`, `select.open-anchor`, `select.open-options`, and a theme Select, never arbitrary text or history.

| Concern      | Current behavior                                                                                                                                                            |
| ------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Data/owner   | A caller supplies `labels` and selected index. `SelectState` owns transient `open`, preview `active`, and `scroll`; it intentionally does not own or commit selected value. |
| Event/commit | Gallery pointer/Enter commits its active theme; `move_by` only changes preview. Settings owns setting-specific selection/events.                                            |
| Popup/layout | `Select::render_popup` composes `overlay_surface::OverlaySpec`, anchors a 1–10-row list, and returns its measured row rectangles for pointer input.                         |
| Chrome       | Anchor uses `render_select`: recessed fill, hairline/accent border, clipped 12px UI label, and disclosure glyph.                                                            |
| Editing      | `TextFieldRenderer` supports editable text/caret/selection, but no production caller combines it with a prior-value list.                                                   |
| Focus/a11y   | Gallery supports pointer, Escape, Enter, arrows, Home/End and wheel. Select has a focused paint input but no reusable focus/accessibility interface.                        |

## Proposed Token contract — proposed, not implemented

Add this only for a concrete consumer needing **both** arbitrary entry and reusable history (for example, a confirmed endpoint/path). No such consumer is identified, so implementation is deferred. Rebranding Select would incorrectly promise custom input and history.

```text
ComboBoxModel<T> { editable: EditableState<StringBuffer>, options: &[ComboOption<T>], selected: Option<T>, popup: ComboPopupState { open, active, scroll }, validation: Valid | Invalid { message }, enabled: bool }
events: Edit(text) | Open | Preview(id) | CommitOption(id) | CommitText | Clear | Cancel
```

The owning feature owns durable value/history and maps events to `Msg`; it persists historic text only after successful form commit. Rendering receives immutable state/theme/geometry and returns hit geometry. It does not search, write a setting, or perform I/O.

### Anatomy, geometry, font, and theme

1. Optional [Label](LABEL.md) and [inline help](LABEL.md#inline-help-and-description-text).
2. Field surface, editable content/placeholder/caret/selection from [Text Field](TEXT-FIELD.md).
3. Independently measured trailing popup affordance and optional built-in action.
4. Popup list made from `overlay_surface::Row`/`Section`.
5. Optional validation supplied by [Form](FORM.md), not invented by the painter.

The field outer rectangle is the control bound. Reserve trailing slots before measuring editable text, so Clear/Browse/arrow never overlap or cause options to jump. Popup width is at least anchor width; it shares scale, clips to window bounds, prefers below and flips above when more usable rows fit. One measured layout must drive paint, hit test, clip, scroll and accessibility bounds; no `label.len()` geometry.

Reuse `overlay.recessed_wash`, `hairline`, `accent`, `text_primary`, `text_dim` and overlay-list selection roles before adding theme keys. Use UI font for non-editable labels/options/accessories and Code for all editable text, consistent with Token's text-input policy; this is not limited to code-valued fields. Existing optional overlay roles retain old-theme fallbacks; preserve that behavior. Error needs a semantic error role, and focus needs a non-color-only outline.

### Event transitions, input, focus, and accessibility

- Field focus edits; it neither clears text nor opens a popup. Open by arrow, pointer, or Down. Moving with arrows/Home/End/Page/wheel previews and scrolls into view without committing.
- Click/Enter commits preview; Escape, outside click, Tab/Shift+Tab, or app switch dismisses without changing typed text. Clear restores the owner's documented default. Each built-in button is separately named/focusable.
- The intended behavior follows IntelliJ's documented open/current-selection, hover-preview/click-or-Enter commit, and dismissal rules. [Combo Box](https://plugins.jetbrains.com/docs/intellij/combo-box.html)
- Proposed platform semantics: editable-combo role; accessible name from Label; field value, expanded state, active option and invalid message exposed. Token has no accessibility tree now, so this is an acceptance requirement, not a claim of implementation.

## Consumers, gallery, acceptance

There is no production consumer. Keep finite Settings choices as Select and large known sets as input+completion. On a justified implementation, add one combo-box gallery family: empty/no options, custom unmatched text, open selected+previewed rows/scroll/flip, committed history, invalid, disabled, and narrow/scaled clipping. It must use the production painter.

Acceptance requires a named owner of the data/events above; one layout for paint/pointer; tests proving navigation does not commit, every dismissal path, history timing, custom text and invalid state; correct 1x/scaled narrow geometry; and no application effects in the component.

## Evidence

- Token: [select model](../../src/model/select.rs), [select view](../../src/view/select.rs), [controls](../../src/view/controls.rs), [text field](../../src/view/text_field.rs), [gallery catalog](../../src/model/gallery.rs), [gallery runtime](../../src/bin/ui_gallery.rs).
- Primary: [Components catalog](https://plugins.jetbrains.com/docs/intellij/components.html), [Combo Box](https://plugins.jetbrains.com/docs/intellij/combo-box.html), [Input Field](https://plugins.jetbrains.com/docs/intellij/input-field.html), [Built-In Button](https://plugins.jetbrains.com/docs/intellij/built-in-button.html).
- Secondary local SDK: [UI/settings reference](../../temporary-docs/intellij-platform-sdk/references/ui-settings-and-toolwindows.md) illustrates bound combo-box use only; it does not specify Token.
