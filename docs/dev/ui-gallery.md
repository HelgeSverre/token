# Native UI Gallery

Run `just ui-gallery` from the repository. This opens an isolated native
development window: no documents, language servers, session restore, or settings
writes. Normal builds do not include it; the launcher requires the `ui-gallery`
Cargo feature. All build artifacts stay under `target/`.

## Current slice

The catalog currently contains 29 labelled visual specimens:

- Buttons: normal, hovered, pressed, focused, selected, disabled, long label.
- Icon button: a glyph-label close action, using the standard button painter.
- Fields: single-line unfocused/focused/selection, multiline editing, and a
  focused overlay search header.
- Checkboxes: off/on. Select anchors: closed/open and an open option list.
- Settings choices: selected choice group and collapsed/expanded disclosures.
- Form fields: a validation message paired with its input geometry.
- Menus and rows: selected/hovered menu rows with shortcut keycaps and a
  separator; a selected completion row with a kind badge.
- Surface swatches: panel, secondary, recessed.

These are explicitly **static visual states**, not pretend interactive controls.
Use their stable names when requesting changes, for example
“`button.selected`: make its fill less saturated”. The gallery shell is
interactive: type to filter, select a category, cycle themes, change preview
width, scroll by pixels, or drag the scrollbar. Escape clears the filter;
Cmd/Ctrl+A selects its text. Native scaling follows the display.

The select popup and choice group share Settings geometry, and menu/list examples
render through the production overlay surface. The gallery is not a replacement
for production interaction testing.

Specimen dimensions are independent of row spacing: single-line fields stay
34 logical pixels high, Settings buttons are 22 pixels high, and icon buttons
are square. Narrow mode changes width, not scale. Each row is painted into a
reused local surface and clipped into the scrolling viewport, so popup anchoring
does not flip or shift when a row reaches the window edge. Metadata and previews
use consistent columns rather than opposite edges of a wide window.

This remains an increment of the [component inventory](ui-component-inventory.md),
not coverage of every editor component. Documentation cards, complex pickers,
interactive specimen sandboxes, live theme editing, and editor compositions
remain subsequent slices.

## Screenshots

Headless PNG output uses exactly the same gallery painter and bundled code/UI
fonts as the native window:

```sh
just ui-gallery --screenshot target/verification/ui-gallery/buttons.png --filter button --height 1100
just ui-gallery --screenshot target/verification/ui-gallery/fields.png --filter field --scale 2
just ui-gallery --screenshot target/verification/ui-gallery/light.png --theme github-light
```

`--width` and `--height` are logical dimensions. `--scale` controls PNG density;
it does not override native display scaling. `--theme` accepts the same theme IDs
as Token. Run `just ui-gallery --help` for options.

## Where to change things

- `src/model/gallery.rs`: explicit specimen metadata, typed preview variants,
  category/filter state. Names do not determine rendering behavior.
- `src/view/gallery.rs`: one layout snapshot for drawing and hit testing, plus
  gallery composition. Reuses production font loading and raster caches.
- `src/bin/ui_gallery.rs`: native window, input translation, theme loading, PNG
  output. No editor-state persistence.
- `src/view/button.rs`: shared button painter, used by Settings and Find.
- `src/view/controls.rs`: checkbox, select-anchor, field-surface, disclosure,
  choice-group and select-popup geometry used by Settings and the gallery.
  Production callers retain interaction state and hit geometry; this is not a
  widget framework.
- `src/view/overlay_surface.rs`: production menu/list and form-label rendering,
  used directly by the gallery's static menu, row, search, and validation
  specimens.
- `src/view/text_field.rs`: shared code-font field content/selection/caret.
- `src/view/scrollbar.rs`: gallery and application scrollbar geometry/painting.

Theme colors come from the current resolved palette. New optional YAML keys
`ui.button.background_selected` and `ui.button.foreground_disabled` separate
selected and disabled styling without requiring existing themes to change.
The former falls back to the pressed fill; the latter to dim overlay text.

When adding a specimen, call its production painter rather than reproducing its
appearance. Add a typed preview variant, a stable ID, source helper and palette
roles, and label whether it is static or interactive. Extract a helper only when
both production and the gallery will actually use it.
