# Group header

## Purpose and catalog mapping

A **Group Header** labels a coherent set of controls. It is not an overlay-list section header, a panel title, a tab, or generic bold text. A **collapsible group header** additionally owns an expanded/collapsed disclosure state. IntelliJ maps the visual family to group headers, collapsible group headers, tabs, and master-detail layouts; Token should expose those as variants of one group contract, not files/types for every appearance.

IntelliJ says headers add noise for groups of three controls or fewer, titles should be short/title-cased/non-generic, advanced groups may start collapsed, and groups become tabs or master-detail as count/height grows. [Group Header](https://plugins.jetbrains.com/docs/intellij/group-header.html) and [Groups of Controls](https://plugins.jetbrains.com/docs/intellij/groups-of-controls.html) are primary guidance.

## Current Token contract — high confidence

**There is no generic GroupHeader.** The closest elements deliberately have distinct owners.

| Element                      | Owner/data/behavior                                                                                                                                                                                |
| ---------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Settings disclosure          | `render_disclosure` draws literal `▸/▾ Advanced`; its settings row owns interaction and settings state owns expansion. Gallery has `disclosure.collapsed` and `.expanded`.                         |
| Overlay section header       | `Section { title, rows }` becomes non-selectable uppercase dim text in `overlay_surface`; a title-less later section becomes a separator hairline. It groups palette/menu rows, not form controls. |
| Settings/collection headings | `settings_page.rs` composes feature-specific headings, fields and viewport geometry.                                                                                                               |
| Gallery                      | Categories include Forms/Structure, but no Group Header specimen; it only uses disclosure and overlay menu sections indirectly.                                                                    |

Current disclosure is incomplete as a generic component: its text is hard-coded “Advanced”, no explicit label/id/disabled/focus state exists, it has no keyboard/accessibility contract, and form body geometry does not model a reusable group content region.

## Proposed Token contract — proposed, not implemented

Do not extract a static heading first. Extract a group only with a concrete Settings form that needs labeled grouping and optional collapse.

```text
GroupHeader { id, title, description: Option<Label>, collapsible: Option<GroupDisclosure { expanded, enabled }>, level }
event: Toggle(id)                         // emitted only for enabled collapsible header
GroupLayout { header_rect, content_rect: Option<Rect>, description_rect: Option<Rect> }
```

The form/settings model owns durable expansion (and whether collapse hides content); update maps `Toggle` to a deterministic `SettingsMsg`; renderer derives content visibility and rects from that state. GroupHeader does not validate child controls, scroll a page, or choose Form layout. [Form](FORM.md) owns labels/errors/row relationship, and [Label](LABEL.md) owns text styling.

### Geometry, theme, font and edge cases

Header text and optional disclosure share one clickable/focus rect only when the whole row toggles; otherwise return separate disclosure rect. Collapsed content occupies zero layout height and is absent from hit traversal/tab order; animation is out of scope until layout and damage policy exist. Expanded `content_rect` is the sole child clip/scroll relationship, so paint/hit test cannot leave hidden controls active. Preserve the Settings narrow-mode layout rather than deriving a new group-specific line loop.

Use UI font. Static group title can be a clear UI heading role; overlay list section retains its dim uppercase metadata style and remains separate. Reuse overlay text/hairline/accent/focus roles initially; do not theme a static heading as an action. Scale through caller metrics and measure/truncate with `TextPainter`. A header should not truncate title into ambiguity: require short title, offer help/description below when needed.

### Events, keyboard, pointer, focus and accessibility

- Static headers have no focus/click target. Collapsible headers toggle on pointer click, Space/Enter when focused, and expose expanded/collapsed state; Escape never globally collapses a form section.
- Focus follows normal Form order: header then visible descendants. On collapse while a descendant owns focus, move focus to header; never leave focus in hidden content.
- Pointer hover is not an activation substitute. Disabled collapse affordance neither toggles nor appears interactive.
- Proposed semantics: group name/description and `expanded` state; disclosure role/button name communicates action. Existing Token has no platform accessibility tree.

## Consumers, gallery, acceptance

The existing “Advanced” Settings disclosure is a concrete retrofit candidate only if its literal title and interaction row are replaced without changing Settings category navigation. Static GroupHeader has no independently justified consumer. For 3 or fewer clearly labeled controls use vertical insets, not a new heading; for many/variable groups use existing navigation/tabs/master-detail decisions rather than nested collapsibles.

Gallery states: static form group with description; collapsible expanded; collapsed; focused; disabled; long-title/narrow clipped; content with validation error; and overlay list section separately labelled as non-interactive. Acceptance: explicit model/event ownership; collapsed children paint/hit/focus nowhere; header/content rectangles are shared; title/description follow font/scale/clip rules; keyboard and pointer transitions tested; existing advanced Settings workflow remains intact.

## Evidence

- Token: [controls/disclosure](../../src/view/controls.rs), [overlay Section](../../src/view/overlay_surface.rs), [settings page](../../src/view/settings_page.rs), [gallery catalog](../../src/model/gallery.rs), [settings forms](../../src/settings/forms.rs).
- Primary: [Group Header](https://plugins.jetbrains.com/docs/intellij/group-header.html), [Groups of Controls](https://plugins.jetbrains.com/docs/intellij/groups-of-controls.html), [Components](https://plugins.jetbrains.com/docs/intellij/components.html).
- Secondary local SDK: [UI/settings reference](../../temporary-docs/intellij-platform-sdk/references/ui-settings-and-toolwindows.md) documents `group`/`collapsibleGroup`; it is not Token API evidence.
