# UI component inventory — 2026-09-11

Inventory and design discussion only. A component gallery, new theme fields,
and a component extraction are **not implemented or approved by this document**.

## Research brief

Identify the visual elements Token already renders so a developer gallery can
eventually show the real components with stable names and explicit states.
Compare Zed/GPUI and Lapce/Floem for naming, theme roles, and preview structure,
without changing Token's renderer or adopting another UI framework.

Questions:

1. Which elements are reusable today, and which are feature-local compositions?
2. Can hover, pressed, selected, focused, disabled, and error states be named
   independently, or are their appearances currently conflated?
3. Which colors are configurable, and which geometry/style values are in code?
4. How do other editors expose a searchable, labelled component/state gallery?
5. What is the smallest useful development surface built from Token's real UI?

## Current architecture

Token has a CPU renderer, not a retained widget toolkit. `Renderer` orchestrates
the frame. Pure paint helpers take geometry, theme/style, and caller-owned state;
update/runtime code owns interaction. The existing layout engine (`UiTree`,
`LayoutSnapshot`, `RowListView`) and feature layouts provide shared rectangles
for drawing and hit testing. A gallery should retain these boundaries.

The strongest reusable pieces are:

- [Button](../../src/view/button.rs): `render_button`, `ButtonStyle`, `ButtonState`.
- [Text field](../../src/view/text_field.rs): `TextFieldRenderer`,
  `TextFieldOptions`, shared editable content, caret and selection rendering.
- [Scrollbar](../../src/view/scrollbar.rs): geometry, track/drag mapping, paint.
- [Overlay surface](../../src/view/overlay_surface.rs): common layout and paint
  for palettes, menus, documentation, forms, headers, rows, and accessories.
- [Tree traversal](../../src/view/tree_view.rs): shared traversal and row metadata;
  the caller still paints the particular kind of row.
- [Frame/text painter](../../src/view/frame.rs): clipping, shapes, rounded masks,
  text measurement, font roles, keycaps and symbol drawing.

There is no central component catalog or component-state browser. The
[screenshot generator](../../src/bin/screenshot.rs) already renders real
application scenarios with theme/size/scale overrides and deterministic LSP and
terminal fixtures. That is useful existing infrastructure, not a kitchen sink.
The [HTML Settings prototype](../../prototypes/settings.html) is a design
reference, not a substitute for native component previews.

## Inventory

Names below are **proposed feedback IDs**, not claims that matching Rust types
already exist. Shared = reusable helper today; composed = shared primitives
assembled for a feature; local = paint/layout still specific to that feature.
States describe what exists today. Missing states are called out afterward.

### Controls and content

| Feedback ID     | Current element and visible states                                                      | Implementation / sharing                                                        |
| --------------- | --------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------- |
| `button`        | Text action; normal, hovered, pressed, independent focus ring; optional text size       | Shared `ButtonState` / `ButtonStyle`, `view/button.rs`                          |
| `icon-button`   | Close, arrows, expand, Find options; normal/hovered, some toggled                       | Usually glyph labels passed to Button; no separate icon-button type             |
| `choice-group`  | Settings presets; selected/unselected, hovered, wrapped narrow layout, off-preset value | Composed in `view/settings_page.rs` from `Accessory::Choices` and buttons       |
| `checkbox`      | Boolean preference, checked/unchecked                                                   | Local `draw_checkbox` in Settings; not a shared widget                          |
| `select`        | Closed/open dropdown trigger, current value, option rows                                | Local `select_button` / `select_options`; shares computed option hit rectangles |
| `disclosure`    | Advanced section expanded/collapsed                                                     | Settings interprets Show/Hide choices; not a named disclosure component         |
| `text-field`    | Empty/content, caret/blink, selection, horizontal scrolling, focus                      | Shared text renderer/editable state; caller paints field chrome                 |
| `text-area`     | Multiline editing, selection, visible line window, focus                                | Same renderer via `for_text_area`; not a second editing engine                  |
| `search-field`  | Query, empty hint, selection, status; palette and Find variants                         | Shared text content rendering; surrounding chrome is composed                   |
| `form-field`    | Label, help text, input/value, Browse/action, focused row marker                        | Settings `SettingInput` / `SettingValue`, responsive label/input geometry       |
| `keycap`        | Single key, modifier sequence, chord steps; subdued menu scale                          | Shared `draw_keycap`; `Chip` / `Accessory::Keycaps` assembly                    |
| `label`         | UI text, code text, metadata, truncated and highlighted matches                         | `TextPainter`, `FontRole`, shared ellipsis helpers; no generic Label widget     |
| `kind-badge`    | Function/method/variable/type/keyword/field/module/file/folder/constant/other           | Overlay `RowIcon::KindBadge(MenuItemKind)`                                      |
| `severity-icon` | Error, warning, information, hint                                                       | Shared symbol helpers, used in Problems and documentation/diagnostic surfaces   |
| `theme-swatch`  | Color strip, current-theme check                                                        | Overlay `Accessory::Swatches`                                                   |
| `status-label`  | Saved/dirty/conflict, LSP state, hints and errors                                       | Multiple composed labels; no common Badge/Status component                      |

### Rows, navigation, and structure

| Feedback ID         | Current element and visible states                                               | Implementation / sharing                                                         |
| ------------------- | -------------------------------------------------------------------------------- | -------------------------------------------------------------------------------- |
| `list-row`          | Normal/selected, matched text, subtitle/detail, icon and accessory               | Overlay `Row`, `Section`, `FlatIndex`; palette and completion row metrics differ |
| `menu-item`         | Context action, selected/hover-targeted, shortcut, separator                     | Composed overlay list; no independent MenuItem style API                         |
| `tree-row`          | File/folder or outline symbol, expanded/collapsed, selected/hovered, depth       | Shared traversal; sidebar/outline painters in `view/panels.rs`                   |
| `diagnostic-row`    | Severity, message, location; file grouping and navigation                        | Problems panel composition in `view/panels.rs`                                   |
| `usage-row`         | File grouping, source/location and selected result                               | Usages panel composition in `view/panels.rs`                                     |
| `settings-category` | Selected/unselected category and hover                                           | Local Settings navigation                                                        |
| `settings-record`   | Selected/hovered server/provider, secondary status, empty list                   | `SettingsCollection` / `SettingsRecord`, separate list viewport                  |
| `document-tab`      | Active/inactive, dirty/conflict marker, drag ghost/drop target                   | Renderer plus `layout/editor.rs`; not an overlay tab                             |
| `panel-tab`         | Active/inactive Terminal/Problems/Usages and panel navigation                    | Dock/panel composition                                                           |
| `terminal-tab`      | Active/inactive session, title, add and close targets                            | `panels/terminal.rs::render_tabs`                                                |
| `overlay-tab`       | Active/inactive search category, count                                           | Overlay `TabBar`, `TabCount`, underline selection                                |
| `scrollbar`         | Horizontal/vertical, thumb absent/content fits, normal/hovered, dragged position | Shared geometry/paint; no distinct pressed/dragged color token                   |
| `splitter`          | Pane/dock boundary and resize target                                             | Shared layout rectangles, renderer/panel chrome                                  |
| `surface`           | Panel, raised/header/footer, recessed input, backdrop, rounded cursor popup      | Overlay palette and renderer shapes; no general Surface style type               |
| `header-footer`     | Search/title/header, persistent actions, keyboard hints, separators              | Overlay `Header` / `Footer`, specialized Settings chrome                         |
| `empty-state`       | Empty results, empty records, placeholder panels                                 | Feature-local content inside shared surfaces                                     |

### Larger specimens worth showing without making them generic widgets

| Feedback ID           | Existing composition and useful scenarios                                                                              | Source                                                           |
| --------------------- | ---------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------- |
| `settings-page`       | Ordinary preferences; LSP/AI list+form; add/preset/empty; advanced open; validation failure; narrow layout             | `view/settings_page.rs`, `settings.rs`, `settings/forms/`        |
| `command-palette`     | Query/results/empty, selected command, shortcut chips                                                                  | `view/modal.rs`, overlay List body                               |
| `picker`              | File/recent/theme/language/server, selected/current and grouped results                                                | `view/modal.rs`, shared List body                                |
| `confirmation-dialog` | Unsaved changes, external-file conflict, remove-record confirmation                                                    | Modal/overlay list and field compositions                        |
| `completion-menu`     | Kind badges, matched prefix, selected candidate, attached documentation                                                | `view/modal.rs`, overlay cursor anchor                           |
| `documentation-card`  | Full-bleed code header, themed snippets, prose/inline code, diagnostics banner, short/scrollable                       | Overlay Zones/Documentation and measured text plans              |
| `signature-help`      | Signature and active parameter, optional documentation                                                                 | Cursor overlay using code font                                   |
| `find-bar`            | Find/Replace, option toggles, no matches/invalid regex, expanded/narrow layout                                         | `view/find_bar.rs`, shared buttons/text fields                   |
| `status-bar`          | File state, encoding/line endings, cursor location, LSP state, transient error/message                                 | `model/status_bar.rs`, renderer                                  |
| `terminal`            | ANSI colors, cursor forms, selection, modifier-link cue, scrollback indicator                                          | `panels/terminal.rs`                                             |
| `editor-decorations`  | Caret/multicursor, selection, current line, brackets, indent guides, fold markers, diagnostics, Find marks, ghost text | `view/editor_text.rs`, shared viewport and gutter/scrollbar code |
| `csv-grid`            | Headers/grid, selected cell, cell editing, numeric text                                                                | Renderer CSV composition and shared text-field editor            |
| `image-preview`       | Checkerboard and scaled image                                                                                          | `view/editor_special_tabs.rs`                                    |
| `binary-placeholder`  | Unsupported binary message                                                                                             | `view/editor_special_tabs.rs`                                    |
| `preview-pane`        | Markdown/HTML, syntax-highlighted blocks, Mermaid/fallback                                                             | Renderer + webview/Markdown HTML; separate styling boundary      |
| `drop-overlay`        | File drop target and explanatory content                                                                               | `view/modal.rs`, overlay zones                                   |
| `debug-overlay`       | Performance and damage/layout indicators                                                                               | Renderer/perf debug paths; development-only specimens            |

OS file dialogs, macOS titlebar/window controls, and the HTML/webview preview are
not CPU-drawn Token controls. A native gallery cannot restyle them through Button.
Record these as external surfaces rather than displaying misleading duplicates.

## Why component-wide polish is difficult today

These are concrete design limitations, not hypothetical runtime bugs:

1. `ButtonState` has only Normal/Hovered/Pressed. Find toggles and Settings preset
   selections both use **Pressed** for a persistent selected value. Pressing and
   selection cannot be styled independently. Disabled/destructive/primary/quiet
   roles are not part of the shared button API.
2. Settings chooses checkbox/disclosure presentation by matching literal labels
   (`["Off", "On"]`, `["Show"]`, `["Hide"]`). The visual kind is implicit in
   `Accessory::Choices`, rather than explicit semantic metadata.
3. Input text/caret/selection behavior is shared, but borders, fill, labels,
   focus and validation presentation are owned by the surrounding surface.
4. Tabs and rows have several legitimate families. Sharing text and rectangles
   does not mean document tabs, terminal tabs, and search tabs have one style.
5. Padding, heights, corner radii and type sizes are mostly Rust constants,
   including overlay `dims` and Settings-specific geometry. Themes mainly
   control colors. Changing one constant does not change every family.

## Theme inventory and gaps

[Theme schema and fallbacks](../../src/theme.rs) have 12 UI groups: editor,
gutter, status_bar, overlay, splitter, sidebar, tab_bar, csv, button,
image_preview, syntax, scrollbar. Older themes can omit optional fields and use
resolved defaults. Preserve that compatibility if the schema grows.

| Existing group          | Configurable coverage                                                                                                                            | Missing general-purpose distinction                                                       |
| ----------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------ | ----------------------------------------------------------------------------------------- |
| `ui.button`             | background, background_hover, background_pressed, foreground, border, focus_ring                                                                 | selected vs pressed, disabled, primary/quiet/destructive roles                            |
| `ui.overlay`            | accent, panel/secondary/recessed surfaces, hairline, selection wash, several text levels, keycaps, severity fills/text, legacy input/list colors | Roles exist but are named/owned as overlays, even when Settings and other UI consume them |
| `ui.scrollbar`          | track, thumb, thumb_hover                                                                                                                        | separate dragging/pressed appearance                                                      |
| `ui.sidebar`            | background/foreground, selected colors, hover, file/folder icons, border                                                                         | Not a reusable row-state palette outside sidebar                                          |
| `ui.tab_bar`            | active/inactive backgrounds/foregrounds, border, modified marker                                                                                 | Independent hover/focus/disabled state roles                                              |
| Text/font configuration | `editor_font` vs `ui_font`, metrics and painter font roles                                                                                       | No common component size/density/radius scale in theme YAML                               |

Theme expansion candidates for discussion: shared `surface`, `text`, `border`,
`control` state roles; explicit selected and disabled styles; semantic intent
(accent/danger); then component exceptions only where useful. Geometry tokens
may belong to a separate style/density specification rather than color themes.
Do not add every possible field before the first component family is agreed.

## Zed/GPUI and Lapce/Floem comparison

Primary-source observations are high confidence; recommendations below are
inferences for Token, not assertions of API compatibility.

| Question             | Zed/GPUI                                                                          | Lapce/Floem                                                                | Implication for Token                                                 |
| -------------------- | --------------------------------------------------------------------------------- | -------------------------------------------------------------------------- | --------------------------------------------------------------------- |
| Component vocabulary | Button, IconButton, ButtonLike, DropdownMenu, InputField                          | ButtonClass, TextInputClass, CheckboxClass, ToggleButtonClass              | Use conventional names; distinguish visual kind from state            |
| State vocabulary     | Disabled/selectable/toggle traits plus button styles/sizes                        | hover, active, focus_visible, disabled, selected selectors                 | Keep selected, pressed and focus separate; name only supported states |
| Gallery structure    | In-app Component Preview; metadata registry, categories/search, labelled examples | Floem widget-gallery example with named sections and per-section demos     | Real shared rendering plus labels, not an HTML imitation              |
| Theme organization   | Semantic surfaces, element/ghost states, text/icon/border roles                   | Floem semantic base/derived palette; Lapce semantic UI keys with fallbacks | Shared role defaults with explicit theme overrides                    |

Zed's `Component` metadata includes name, ID, scope, description, status and
preview. Its examples provide named groups/variants. Source:
[Component contract](https://github.com/zed-industries/zed/blob/1c3d902fbf1cbd513f5d989d96a8c18694e4c287/crates/component/src/component.rs),
[example layout](https://github.com/zed-industries/zed/blob/1c3d902fbf1cbd513f5d989d96a8c18694e4c287/crates/component/src/component_layout.rs),
[preview browser](https://github.com/zed-industries/zed/blob/1c3d902fbf1cbd513f5d989d96a8c18694e4c287/crates/component_preview/src/component_preview.rs),
[button states/styles](https://github.com/zed-industries/zed/blob/1c3d902fbf1cbd513f5d989d96a8c18694e4c287/crates/ui/src/components/button/button_like.rs),
[theme roles](https://github.com/zed-industries/zed/blob/1c3d902fbf1cbd513f5d989d96a8c18694e4c287/crates/theme/src/styles/colors.rs).

Floem's gallery is a toolkit example, **not evidence of a gallery inside Lapce**.
No equivalent Lapce-app catalog was found in the inspected paths; that is not
proof none exists elsewhere. Sources:
[widget gallery](https://github.com/lapce/floem/blob/778bb5f2aa08429e579ee2e6ac97e84fbf18b618/examples/widget-gallery/src/main.rs),
[button demos](https://github.com/lapce/floem/blob/778bb5f2aa08429e579ee2e6ac97e84fbf18b618/examples/widget-gallery/src/buttons.rs),
[Floem theme and state styles](https://github.com/lapce/floem/blob/778bb5f2aa08429e579ee2e6ac97e84fbf18b618/src/style/theme.rs),
[Lapce UI color names](https://github.com/lapce/lapce/blob/b604d57de4a820006d335a3be0d7583eb8fab558/lapce-app/src/config/color.rs),
[Lapce theme fallback](https://github.com/lapce/lapce/blob/b604d57de4a820006d335a3be0d7583eb8fab558/lapce-app/src/config/color_theme.rs).

## Recommendation to discuss, not implement yet

A development-only **UI Gallery** inside Token is the smallest useful first
step: searchable family navigation, a vertically scrollable specimen area, theme
and scale controls, and stable labels such as `button / hovered / focused` or
`select / open / selected-option`. Keep a future standalone launcher possible,
but reuse the same renderer and fixture catalog if one is added.

Each entry should identify its source helper, current consumers, relevant theme
keys, and whether its state is simulated for comparison or genuinely interactive.
Show missing states as unsupported, not invented styling. An explicit small
catalog is sufficient; Zed's macro registry and a new widget framework are not
required. Existing screenshot tooling can later capture the same specimens.

Start discussion with buttons, inputs, checkbox/select/disclosure, rows, and
surface/header/footer styles. Extract only the pieces needed to render those
**same live controls** in both their current callers and the gallery. Keep
layout/hit geometry shared. A gallery does not replace interaction tests or prove
OS event delivery. A free-pan/zoom whiteboard and live theme-property editor can
be considered later; neither is needed to establish a precise visual vocabulary.
