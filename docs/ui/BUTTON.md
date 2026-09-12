# Button

## Purpose and boundary

A Button invokes one immediate, named action. It is not a value picker, a
navigation tab, or an on/off setting. This follows the IntelliJ component
definition ([Components](https://plugins.jetbrains.com/docs/intellij/components.html)). Use a
select for objects/states and a checkbox for a persistent boolean.

## Implemented contract — high confidence

`ButtonState` and `ButtonStyle` in [button.rs](../../src/view/button.rs#L14)
are a stateless paint contract: the owner supplies `Normal`, `Hovered`,
`Pressed`, `Selected`, or `Disabled`, plus focus and optional text size. The
renderer paints and clips a centered text label; it neither hit-tests nor
dispatches a message ([render_button](../../src/view/button.rs#L40)). Disabled
only changes the picture and suppresses the ring: the owner must also suppress
activation. `button_rect` is a convenience for a label-sized, centered rectangle,
not a general layout engine ([button_rect](../../src/view/button.rs#L153)).

Consumers include Settings action buttons ([settings_page.rs](../../src/view/settings_page.rs#L724)),
binary-preview actions ([editor_special_tabs.rs](../../src/view/editor_special_tabs.rs#L84)),
the Find bar, terminal tabs, and gallery fixtures. Those consumers retain their
own hover, press, focus, and command semantics. The gallery covers normal,
hovered, pressed, focused, selected, disabled, clipped long-label, and glyph
(`×`) button states (`button.*` and `icon-button.close` in
[gallery.rs](../../src/model/gallery.rs#L279)).

### Variants and non-variants

- **Text button (implemented painter):** `label + ButtonStyle`, appropriate for
  a named immediate action.
- **Icon button (visual overlap only):** the gallery sends a glyph label through
  the same painter (`icon-button.close`). It has no icon asset slot, tooltip or
  accessible name. **Proposed:** require an accessible action label and tooltip;
  never infer meaning from the glyph.
- **Toggle button (do not conflate):** `ButtonState::Selected` is only a
  persistent-choice visual. It does _not_ supply boolean ownership, pressed
  toggle events, or switch accessibility. See [Toggle switch](TOGGLE-SWITCH.md).
  IntelliJ confines its named toggle-button pattern to search results and uses
  a checkbox in dialogs/menus ([Toggle button](https://plugins.jetbrains.com/docs/intellij/toggle-button.html)).
- **Built-in/trailing input action (not implemented):** a button that helps
  enter one field's value belongs semantically to that field, not to the form
  footer. Token's `Browse…` is presently a separately painted adjacent button;
  it is not inside the field surface. See [Text field](TEXT-FIELD.md).

### Anatomy and visual contract

`rect + label + ButtonStyle` is the entire primitive. It paints a bordered
surface, then an optional one-pixel _outset_ focus ring, then clipped centered
text. The focus ring is intentionally present even for tiny rectangles; a unit
test locks that behaviour ([button.rs](../../src/view/button.rs#L119)).

### Current data, geometry, and event ownership

| Input / concern                                  | Current owner            | Implemented fact                                                                                                                                                     |
| ------------------------------------------------ | ------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Label, rectangle, visual state, focus, text size | Caller → `render_button` | Painter consumes them for one frame; it stores nothing.                                                                                                              |
| Pointer geometry                                 | Caller                   | `rect` is both intended visual/click target by convention, but Button does not hit-test.                                                                             |
| Activation / command                             | Feature update handler   | No Button message exists; a Settings footer hit becomes a row/choice and `form_choice` decides Save/Cancel/etc.                                                      |
| Hover/press                                      | Feature UI state         | Painter merely receives state. Current Settings helper supplies Hovered/Normal and always `focused: false`. [settings_page.rs](../../src/view/settings_page.rs#L724) |
| Disabled enforcement                             | Feature                  | Painter dims foreground and omits focus ring; it cannot block owner activation.                                                                                      |

`render_button` rounds `Rect` coordinates/sizes to pixels, draws background and
border, then when eligible paints a 1-pixel ring exactly one pixel outside all
four edges, then clips the label to the original rect. Text is centered from
measured width/line height; `text_size` changes measurement and draw size,
otherwise the caller's active painter role/size applies
([button.rs](../../src/view/button.rs#L40)). `button_rect` is label-length × a
caller-provided _character width_, so it is safe only where that approximation
matches the font; it is not a substitute for `TextPainter::measure_width` with
proportional UI labels.

| Visual state     | Current background/border/foreground behaviour                                                         |
| ---------------- | ------------------------------------------------------------------------------------------------------ |
| Normal / Hovered | `background` / `background_hover`, `border`, `foreground`.                                             |
| Pressed          | `background_pressed` and `focus_ring` border.                                                          |
| Selected         | `background_selected` if supplied, otherwise pressed; `focus_ring` border.                             |
| Disabled         | Normal background/border and `foreground_disabled`, falling back to `overlay.text_dim`; no focus ring. |

The state table is paint-only. There is no built-in loading, destructive/error,
default/primary, read-only, tooltip, or accessible action state.

Theme roles are `button.background`, `background_hover`, `background_pressed`,
`background_selected`, `foreground`, `foreground_disabled`, `border`, and
`focus_ring` ([theme.rs](../../src/theme.rs#L771)). `Selected` falls back to
pressed colour and disabled foreground falls back to `overlay.text_dim`.
There is no built-in danger, primary/secondary, icon-only, loading, or tooltip
role. UI labels use the painter's active UI font unless a caller deliberately
chooses otherwise; every supplied rectangle is physical-pixel geometry and must
already account for scale.

## Proposed interaction/accessibility contract

Each future semantic Button should expose `id`, accessible name (the visible
label, or a required label for an icon), enabled state, focusability, and an
`on_activate` message. Pointer activation is press-inside/release-inside;
keyboard activation is Space and Enter; Tab/Shift+Tab move focus. Escape must
not activate it. A focused disabled button is skipped. These are **proposed**:
the existing primitive has no event API or accessibility tree. They implement
the keyboard-only baseline in IntelliJ's [Accessibility guidance](https://plugins.jetbrains.com/docs/intellij/accessibility.html).

Use title capitalization for action labels and sentence capitalization for
non-action labels per [Capitalization](https://plugins.jetbrains.com/docs/intellij/capitalization.html).
Do not rely on colour, glyph-only labels, or hover to communicate state.

| Proposed transition        | Contract                                                                                                                                    |
| -------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------- |
| Pointer press → release    | Capture press only while enabled; activate once on release inside. Release outside cancels.                                                 |
| Keyboard                   | Space and Enter activate focused enabled button once; Tab/Shift+Tab traverse; Escape never activates.                                       |
| Owner data changes/removal | Remove it from focus order atomically; if focused, choose the next valid focus target.                                                      |
| Disabled vs read-only      | Buttons have no useful read-only interaction: use disabled for unavailable actions with explanatory context, or a non-button value display. |

## Acceptance and next slice

- Reuse `render_button`; keep hit testing and effects in the owning update
  handler.
- Add a semantic wrapper only when two or more owners need identical focus,
  pointer and activation transitions.
- Add gallery fixtures for a true keyboard-focus traversal, disabled activation
  rejection, icon button with accessible name, selected-versus-toggle semantics,
  and a trailing input action. The current gallery is visual rather than
  assistive-technology verification.

Evidence: Token code above (high confidence); IntelliJ [button/component
guidance](https://plugins.jetbrains.com/docs/intellij/components.html) (high
confidence). The local `temporary-docs/intellij-platform-sdk` UI reference is
secondary corroboration only; it does not describe Token.
