# Native UI Gallery

Run `just ui-gallery` from the repository. This opens an isolated native
development window: no documents, language servers, session restore, or settings
writes. Normal builds do not include it; the launcher requires the `ui-gallery`
Cargo feature. All build artifacts stay under `target/`.

## Current slice

The catalog currently contains 56 labelled visual specimens:

- Buttons: normal, hovered, pressed, focused, selected, disabled, long label.
- Icon button: a glyph-label close action, using the standard button painter.
- Fields: single-line unfocused/focused/selection, multiline editing, and a
  focused overlay search header.
- Checkboxes: off/on. Select anchors: closed/open and an open option list.
- Settings choices: selected choice group and collapsed/expanded disclosures.
- Form fields: a validation message paired with its input geometry.
- A production Settings composition with typed checkbox, select and disclosure
  presentation; visible labels no longer determine the control kind.
- Menus and rows: selected/hovered menu rows with shortcut keycaps and a
  separator; a selected completion row with a kind badge.
- Contextual overlays: completion with documentation, hover documentation and
  signature help with an accented active parameter.
- Search Everywhere: grouped Commands and Files, workspace-symbol loading, and a
  non-selectable empty state rendered by the production modal.
- Surface swatches: panel, secondary, recessed.
- Document tabs: active/inactive, save-error marker, clipped/scrolling title and
  drag ghost. Dock tabs, terminal active/hovered/exited/overflow states, and
  overlay tabs with count/pending/unavailable indicators remain distinct families.
- Bottom Problems and right Outline panels: real headers and borders; Outline is
  populated with nested, selected and truncated production tree rows. Problems
  includes grouped files, mixed severities, selection and a collapsed group.
- Settings records: selected language-server records and the production empty
  collection state, including the narrow two-pane Settings layout.
- Scrollbars: vertical/horizontal, normal/hovered, end position and content fitting,
  shown in small dummy-content viewports so the thumb position has context.
  Splitters: horizontal and vertical boundaries.

These are explicitly **static visual states**, not pretend interactive controls.
Use their stable names when requesting changes, for example
“`button.selected`: make its fill less saturated”. The gallery shell is
interactive: type to filter, select a category, choose a theme, change preview
width, scroll by pixels, or drag the scrollbar. Escape clears the filter;
Cmd/Ctrl+A selects its text. Native scaling follows the display.

### Selection controls and naming

- `select.theme`: a non-editable **select**, also called a **dropdown** or
  Apple's **pop-up button**. It displays the current value and opens a list of
  mutually exclusive choices, with a checkmark on the committed theme.
- `segmented-control.preview-width`: a **single-select segmented control**.
  Narrow and Wide remain visible; clicking the selected segment leaves it selected.
  This is not an on/off switch or a button that cycles through hidden values.

These names follow [Apple's pop-up button guidance](https://developer.apple.com/design/human-interface-guidelines/pop-up-buttons)
and [segmented controls](https://developer.apple.com/design/human-interface-guidelines/segmented-controls).
Material uses [single-select segmented button](https://developer.android.com/develop/ui/compose/components/segmented-button)
for the latter pattern.

Tab/Shift+Tab moves between the filter, theme select, and width control. On the
theme control, Enter/Space or an arrow opens the dropdown; Up/Down and Home/End
move the active option, Enter/Space commits, and Escape cancels. Clicking outside
or losing window focus dismisses it. The list supports pointer hover, wheel
scrolling, and scrollbar dragging. Width uses Left/Right or Home/End.

The select popup and choice group share Settings geometry, and menu/list examples
render through the production overlay surface. The gallery is not a replacement
for production interaction testing.

Specimen dimensions are independent of row spacing: single-line fields stay
34 logical pixels high, Settings buttons are 22 pixels high, and icon buttons
are square. Narrow mode changes width, not scale. Each row is painted into a
reused local surface and clipped into the scrolling viewport, so popup anchoring
does not flip or shift when a row reaches the window edge. Metadata and previews
use consistent columns rather than opposite edges of a wide window. Every
specimen sits inside a padded, darker canvas with a dotted boundary; that boundary
also makes the specimen's hard clipping area visible. List-like fixtures show at
least five entries. Metadata for full-width Settings and completion/documentation
compositions is stacked above the canvas instead of competing for horizontal room.

This remains an increment of the [component inventory](ui-component-inventory.md),
not coverage of every editor component. Complex pickers, interactive specimen
sandboxes, live theme editing, Problems/Usages collections and editor compositions
remain subsequent slices.

Chrome specimens build an isolated `AppModel` and use the real editor/dock/terminal
layout and painters. Terminal sessions use headless PTY handles: no shell is
started. Crops preserve native pixel size, not scaled screenshots. The fixtures
are static visual examples; they do not provide editor or terminal interaction.

Unsupported styling is not invented: the document-tab fixture includes a dirty
document, but the current production tab title has no distinct dirty marker.
The `!` marker represents a save error/external change. Scrollbars likewise have
no separate pressed/dragging color beyond their existing normal/hover styling.

## Screenshots

Headless PNG output uses exactly the same gallery painter and bundled code/UI
fonts as the native window:

```sh
just ui-gallery --screenshot target/verification/ui-gallery/buttons.png --filter button --height 1100
just ui-gallery --screenshot target/verification/ui-gallery/fields.png --filter field --scale 2
just ui-gallery --screenshot target/verification/ui-gallery/light.png --theme github-light
just ui-gallery --screenshot target/verification/ui-gallery/theme-menu.png --theme-menu
just ui-gallery --screenshot target/verification/ui-gallery/narrow.png --narrow
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
- `src/view/section_navigation.rs`: Settings and gallery category navigation;
  shared row/grid spacing, active state, label truncation and optional divider.
  The gallery sidebar is itself a live use of this component.
- `src/view/select.rs` and `src/model/select.rs`: select anchor, menu composition,
  measured popup hit geometry, and transient selection/scroll state. Owners commit
  values and perform effects; the shared control does not load themes or settings.
- `src/view/segmented_control.rs`: equal-width segment layout and themed painting.
  The owner supplies labels, selected index, and focus; painting and clicks use
  the same rectangles. Both selection components are available outside the gallery.
- `src/view/document_tabs.rs`: production document-tab painter; geometry stays in
  `src/layout/editor.rs`. `Renderer` remains the application orchestrator.
- `src/view/gallery_chrome.rs`: isolated chrome fixtures and native-size cropping;
  calls the production dock/terminal painters and document-tab layout.

Theme colors come from the current resolved palette. New optional YAML keys
`ui.button.background_selected` and `ui.button.foreground_disabled` separate
selected and disabled styling without requiring existing themes to change.
The former falls back to the pressed fill; the latter to dim overlay text.

When adding a specimen, call its production painter rather than reproducing its
appearance. Add a typed preview variant, a stable ID, source helper and palette
roles, and label whether it is static or interactive. Extract a helper only when
both production and the gallery will actually use it.

## Planned performance-study coverage

The [prototype component decision record](../ui/PROTOTYPE-COMPONENTS.md) defines
new proposed families. They are **not** part of the current 56 specimens and
should only enter the gallery with production layout/painters:

- `pane-header.*`: title-only, optional icon/actions, truncation and overflow.
- `pane-footer.*`: absent, hint with/without an icon, and status plus trailing
  metadata. Footer presence must not be inferred from its label.
- `breadcrumbs.*`: file/path/symbol, optional icons, narrow overflow and
  independent editor-group context.
- `dockable-panel.*`: docked and floating hosts with the same content, title
  shown once, constrained geometry, and action states.
- `performance.*`: empty/partial/full completed-frame data, narrow layout,
  zero cache lookups, and stage-accounting mismatch.
- `activity-rail.*`: left/right/both-edge cases, deferred along with the rail
  implementation.

Use the [editor polish plan](../feature/editor-visual-polish.md) for controlled
font/spacing comparisons and the [performance feature plan](../feature/performance-panel.md)
for metrics and scope. The initial Performance panel excludes the prototype's
live/pause/reload row and decorative header icon. Gallery fixtures do not prove
keyboard navigation, capture cancellation, docking lifecycle, or frame-snapshot
coherence; those require the real owners' layout/reducer/runtime checks.
