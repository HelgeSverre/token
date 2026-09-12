# Split button

## Purpose and catalog mapping

A **Split Button** presents a common immediate action and a separate adjacent disclosure for less-common _related_ actions. It is neither a Select, a toggle, a menu button, nor an IconButton with an incidental arrow. The **Split Icon Button** is the toolbar/icon variant: main icon plus triangle; catalog it as `SplitButton { presentation: Text | Icon }`, not as a second framework component.

IntelliJ recommends a split button for more than two related actions when space is tight, or an uncommon dangerous action; it prohibits unrelated/duplicated main actions. Its split-icon variant is for crowded horizontal toolbars, requires icons for menu actions, and keeps unavailable actions disabled rather than hidden. [Split Button](https://plugins.jetbrains.com/docs/intellij/split-button.html) and [Split Icon Button](https://plugins.jetbrains.com/docs/intellij/split-icon-button.html) are primary guidance.

## Current Token contract — high confidence

**No Split Button exists.** `view/button.rs` paints one rectangular label/button whose caller supplies Normal/Hovered/Pressed/Selected/Disabled and focus. `overlay_surface` supplies menu/list layout and dismissal patterns. Some controls use glyph labels as icon buttons, but no painter separates main/disclosure hit targets and no feature owns a related secondary action menu.

The Gallery's button and icon-button specimens prove single-button state only. Settings Select is value selection, not split action. Find option buttons are persistent toggles, not split actions. There is therefore no production data/consumer to extract.

## Proposed Token contract — proposed, not implemented

Defer until one action has a demonstrated common default and at least two related secondary actions (or one uncommon destructive action). The data must preserve two targets:

```text
SplitButton { main: ActionPresentation { id, label_or_icon, enabled }, menu: &[MenuAction { id, label, icon?, enabled, danger? }], open, menu_active, focused_part: Main | Disclosure, presentation: Text | Icon }
events: InvokeMain(id) | OpenMenu | PreviewMenu(id) | InvokeMenu(id) | DismissMenu
layout: { outer, main_rect, disclosure_rect, divider_rect, popup_layout }
```

The feature model owns action availability, selection and popup state; update maps invocation to its existing command/message path. Button painter owns neither action effects nor menu content. `overlay_surface` remains the popup list/keyboard/scroll source, while [Button](BUTTON.md) remains the visual base for each half.

### Geometry, theme and font invariants

Outer bounds are one visual unit but `main_rect` and `disclosure_rect` are separately measured/hit-tested/focused. Divider is painted once and never steals a hit. Text variant reserves disclosure width independent of main label; icon variant reserves square main and arrow cells and requires menu icons. Popup anchor is disclosure/outer bottom, clips/flips using shared overlay geometry, and paints last. Long label truncation leaves disclosure usable. Use UI font/text measurement, shared Button colors/focus ring plus overlay menu palette; selected/open disclosure needs explicit state distinct from pressed. No new theme keys before a concrete consumer proves shared roles insufficient.

### State transitions, input, focus, accessibility

- Main click/Space invokes only main. Disclosure click, Down, or documented accelerator opens menu with first enabled row active; opening never invokes main.
- Pointer hover previews menu rows; click/Enter commits row and dismisses. Escape/outside click/Tab dismissal restores focus to disclosure/next control as appropriate. Main action never appears again in menu.
- Tab reaches main then disclosure (or a deliberate grouped equivalent); Arrow Down from main opens menu. Disabled main/menu items never invoke. Dangerous menu item is visually separated and confirmed by its owner if needed.
- Proposed semantics expose grouped split button, separate main/disclosure names, expanded state, active menu item and disabled states. Current CPU renderer has no a11y bridge.

## Consumers, gallery, acceptance

There is no current consumer, so no implementation slice. Do not retrofit Select, Find toggles, terminal tabs or the command palette. When justified, gallery has one family: text normal/hover/pressed/focused/disabled; disclosure open; keyboard-selected popup; dangerous separated action; long/narrow; and icon presentation with all menu icons/disabled item. This is catalog coverage, not separate visual-variant component docs.

Acceptance: actions are genuinely related and non-duplicated; main/disclosure geometry and hit tests differ; every open/dismiss/keyboard route is tested; popup follows overlay clipping; icon variant exposes labels/tooltips; disabled menu stays discoverable; and production gallery uses shared button/menu painters.

## Evidence

- Token: [Button](../../src/view/button.rs), [overlay surface](../../src/view/overlay_surface.rs), [gallery catalog](../../src/model/gallery.rs), [Find controls](../../src/view/find_bar.rs), [Select](../../src/view/select.rs).
- Primary: [Split Button](https://plugins.jetbrains.com/docs/intellij/split-button.html), [Split Icon Button](https://plugins.jetbrains.com/docs/intellij/split-icon-button.html), [Button](https://plugins.jetbrains.com/docs/intellij/button.html), [Components](https://plugins.jetbrains.com/docs/intellij/components.html).
