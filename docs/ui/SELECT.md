# Select (drop-down list)

## Purpose, name, and boundary

Token's `Select` is a non-editable, single-value drop-down: select one known
object or state from a known label list. It is not a combo box (custom values),
an action menu, multi-select, or a navigation tab. That distinction follows
IntelliJ's [Drop-down list](https://plugins.jetbrains.com/docs/intellij/drop-down.html):
use it for one object/state, a checkbox group for multiple independent values,
a menu/split button for actions, and a combo box for a value outside the list.

The current abstraction is deliberately split: the feature owns its committed
value and behaviour; `SelectState` owns only transient popup navigation; the
view paints and returns measured hit geometry. It must not mutate config or
perform effects.

## Current implementation — high confidence

### Data ownership

| Item               | Fields / authority                               | Current responsibility                                                                                                                     |
| ------------------ | ------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------ |
| Committed value    | `Select.selected: usize`                         | Owner supplies index on every render; the view displays its label and checkmark.                                                           |
| Option data        | `Select.labels: &[&str]`                         | Owner owns ordering and replacement. Labels are display-only; there is no option ID, disabled state, detail, grouping or validation field. |
| Transient state    | `SelectState { open, active, scroll }`           | Owner stores it; helper methods mutate it but never commit `selected`. [model/select.rs](../../src/model/select.rs#L3)                     |
| Anchor/focus/scale | `Select { anchor, focused, scale }`              | Owner supplies physical-pixel `Rect`, focus boolean, and scale. [view/select.rs](../../src/view/select.rs#L26)                             |
| Paint/hit geometry | `SelectLayout { panel, rows, first, scrollbar }` | View returns it after `render_popup`; caller must use `option_at`, not rederive rows. [view/select.rs](../../src/view/select.rs#L13)       |
| Command/effect     | none                                             | Intentionally absent. The feature maps a chosen index to its own message/effect.                                                           |

`SelectState::open(selected)` sets `open`, copies the supplied selected index to
`active`, and starts scroll at `selected - 4`; it **does not know option count or
clamp the index**. `move_by(delta, count)` saturates into `0..count`, while
`scroll_to(first, visible, count)` limits scroll and active to the visible range
and is safe for count zero ([model/select.rs](../../src/model/select.rs#L9)).
Do not call these helpers as though they validate a replaced option list.

`render_anchor` resolves `labels[selected]`, otherwise prints `No options`; its
border is accent when open _or focused_. `render_popup` returns `None` when
closed or `labels.is_empty`; otherwise it delegates visible/selected/scroll
normalisation and overlay rows to `SelectableListViewport` and `overlay_surface`
([view/select.rs](../../src/view/select.rs#L35)). It paints last, so the menu
overlays owner content. No generic select input dispatcher exists.

### Geometry and sizing invariants

The anchor is the caller's `Rect` cast to non-negative unsigned pixel geometry.
The anchor painter uses UI text at `12 × scale` pixels, left inset `8 × scale`,
disclosure at right inset `20 × scale`, and truncates the label to
`anchor.width - 32 × scale`; it clips all text to anchor bounds
([controls.rs](../../src/view/controls.rs#L47)). Its surface is
`overlay.recessed_wash` with `overlay.hairline` or `overlay.accent` border and
uses `overlay.text_primary`/`text_dim`.

For a window height `H`, anchor y `y`, anchor height `h`, scale `s`, the current
maximum visible count is:

```
visible = clamp(usize((H - y - h - 24s) / (28s)), 1, 10)
```

The popup asks `overlay_surface` for an anchor width of `anchor.width / s` logical
units and `prefer_below: true`; that shared layout owns final clipping, row
rectangles, upward fallback and scrollbar geometry
([view/select.rs](../../src/view/select.rs#L69)). The returned `first` is the
viewport scroll offset, so `option_at(pointer) = first + row_hit`. These facts
are implemented; exact row height/placement after overlay measurement must be
read from the returned layout, not assumed from the 28-scale estimate.

Theme fields are currently only generic overlay roles above plus overlay-list
roles from `overlay_surface`; there are no select-specific hover, disabled,
error, option-icon or option-detail tokens. FontRole is whatever the surrounding
`TextPainter` has selected (the gallery uses the UI role for labels); `Select`
does not impose an explicit font role. This is a gap to preserve rather than a
license to use Code font indiscriminately.

## Current transitions and concrete consumer

The native UI gallery is the complete direct consumer. It owns
`GalleryState.selected_theme` and `theme_select`; it renders the select at the
Theme control ([gallery.rs](../../src/view/gallery.rs#L260)).

| Trigger in gallery                                   | State mutation                      | Commit/dismiss result                                                                                                    |
| ---------------------------------------------------- | ----------------------------------- | ------------------------------------------------------------------------------------------------------------------------ |
| Click closed anchor; focused Theme + Enter/Space/↑/↓ | `open(selected_theme)`, focus Theme | Opens; no theme change. [ui_gallery.rs](../../src/bin/ui_gallery.rs#L159)                                                |
| ↑ / ↓ while open                                     | `move_by(±1, themes.len())`         | Preview index changes only.                                                                                              |
| Home / End while open                                | set `active` to first / `len - 1`   | No commit. Empty list gives zero through `saturating_sub`.                                                               |
| Popup-row click or Enter/Space                       | `apply_theme(active)`               | If `themes[active]` loads, set committed selected theme; always close. [ui_gallery.rs](../../src/bin/ui_gallery.rs#L165) |
| Click outside popup, Tab, Escape, window loses focus | `open = false`                      | Dismisses without commit. [ui_gallery.rs](../../src/bin/ui_gallery.rs#L178)                                              |
| Scrollbar track/thumb/wheel                          | `scroll_to(...)`                    | Moves visible window and clamps active; no commit. [ui_gallery.rs](../../src/bin/ui_gallery.rs#L97)                      |

The gallery assures `selected_theme` is constructed from its available theme
list. The generic `Select` itself does **not** assure that. Settings uses a
separate feature-local collection selector, `open_select` and
`SettingsCollectionAction`, not `Select` ([update/settings.rs](../../src/update/settings.rs#L520)).
The gallery fixtures are `select.closed`, `select.open-anchor`, and
`select.open-options` ([model/gallery.rs](../../src/model/gallery.rs#L326));
they demonstrate paint only, not all transitions.

## Option-list changes, empty cases, and validation

| Situation                                  | Current behaviour                                                                         | Required owner action / proposed policy                                                                                                                                |
| ------------------------------------------ | ----------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Empty labels while closed                  | Anchor says `No options`; no popup.                                                       | Prefer an explicit disabled “No … available”/Add action instead of a false selectable value.                                                                           |
| Labels become empty while open             | Popup returns `None`, but `state.open` remains true.                                      | **Proposed:** close, clear active preview, expose unavailable state, and transfer focus predictably.                                                                   |
| Selected index removed/reordered           | Anchor falls back to `No options`; checkmark none; helpers do not repair committed index. | **Proposed:** identify choices by stable ID; retain selected ID if present, otherwise choose declared safe fallback or enter explicit no-selection/error state.        |
| Active option removed/reordered while open | Viewport may normalise rendering, but transient `active` is index-based.                  | **Proposed:** remap active by ID, otherwise reset it to committed option/fallback, reset scroll, and do not commit an accidental neighbour.                            |
| Invalid / unavailable option               | No representation.                                                                        | **Proposed:** either omit unavailable options with explanation or represent `disabled` and block commit; use field invalid/error state for an invalid committed value. |

There is no select validation today. A form owner should validate the committed
typed value at its normal submit/focus boundary and provide an invalid description
in its surface rather than encode meaning in an index. IntelliJ advises controls
that constrain choices to prevent invalid entry and defines validation timing for
forms ([Validation errors](https://plugins.jetbrains.com/docs/intellij/validation-errors.html)).

## Proposed semantic contract

**Proposed:** `Select<T>` should take stable option IDs, `value: Option<Id>`,
option label/detail/icon/disabled metadata, field label/help, enabled/invalid
state and owner messages (`Open`, `Preview`, `Commit(Id)`, `Dismiss`). It still
does not own effects. Opening sets active to committed ID, not raw index. Pointer
hover changes preview only; click/Enter commits; Escape/outside click/focus loss
dismiss; Down opens a focused closed select; Tab dismisses then moves to the next
control. Apply a list replacement atomically with state reconciliation from the
table above.

Expose accessible `combobox` role/name from its visible label, current value,
expanded and disabled/invalid states; expose a listbox/options with selected and
active state. Keyboard-only operation and visible focus are baseline requirements
under IntelliJ [Accessibility](https://plugins.jetbrains.com/docs/intellij/accessibility.html).

## Acceptance and next slice

- Preserve `SelectLayout` as painting and hit-test authority; never duplicate
  row math in update/runtime code.
- Test out-of-range selected/active indices, empty/open replacement, removal and
  reordering, 1/10/11 options, scroll/drag, upward placement, narrow label,
  fractional scale and option commit versus cancellation.
- Add gallery/automation specimens for disabled/error/no-options, focus and
  screen-reader naming. Do not migrate Settings until its draft-record semantics
  fit the stable-ID contract.

IntelliJ recommends a drop-down especially when choices exceed four or space is
limited; use radio/segmented selection for a small visible set. This guidance is
high-confidence primary-source UX evidence, not a claim that Token implements
all suggested behaviour.
