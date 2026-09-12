# Token UI foundations

This is the shared contract for the UI component research. It records current
Token behavior and constraints for new components; it does not authorize a
widget toolkit or replace the separate Settings page.

## Vocabulary

| Name              | Meaning                                                        | Not this                              |
| ----------------- | -------------------------------------------------------------- | ------------------------------------- |
| action control    | invokes a command, with no selected value                      | select or tab                         |
| select            | shows one committed value and opens exclusive choices          | cycling button                        |
| navigation        | changes a current place/item                                   | a command that happens to open a view |
| container/surface | establishes bounds, clipping and composition                   | owner of unrelated effects            |
| editor surface    | tabs, optional find bar, gutter, content, overlays, scrollbars | generic multiline field               |
| scroll area       | viewport over content with an offset                           | its document/list model               |
| splitter          | adjustable boundary between sibling layout nodes               | arbitrary divider                     |

IntelliJ is UX vocabulary evidence, not an instruction to adopt Swing: its
editor area has tabs, gutter and inlays; tool windows surround it; popups
normally dismiss on outside click. [UI overview](https://plugins.jetbrains.com/docs/intellij/ui-overview.html)

## Verified architecture

Token uses `Msg -> update -> Cmd -> runtime -> Renderer`.

| Layer                                                            | Authority                               | Component rule                                     |
| ---------------------------------------------------------------- | --------------------------------------- | -------------------------------------------------- |
| [model](../../src/model/)                                        | durable document, editor and UI state   | owners retain state by stable domain IDs           |
| [messages](../../src/messages.rs)                                | state-change requests                   | messages express intent/result, not drawing        |
| [update](../../src/update/)                                      | deterministic transitions and commands  | no I/O, native focus or rendering effects          |
| [commands](../../src/commands.rs), [runtime](../../src/runtime/) | winit input and platform/I/O effects    | translate physical input and execute commands      |
| [view](../../src/view/)                                          | CPU painting and hit testing            | read state; do not become another state machine    |
| [layout](../../src/layout/)                                      | pure chrome trees and `LayoutSnapshot`s | use its solved typed geometry where it owns chrome |

[`layout/mod.rs`](../../src/layout/mod.rs) makes a queryable `LayoutSnapshot` the output for its surfaces:
paint and hit testing query the same rect and clip chain. It owns shell/chrome,
tab strips, previews, overlays and uniform virtual row lists.
[`view/geometry.rs`](../../src/view/geometry.rs)'s `GroupLayout` is the editor-content geometry authority;
`GutterLayout` and `pixel_to_cursor` preserve editor coordinate semantics. Do
not add a feature-local line loop, gutter formula or alternate text hit test.

## Geometry, clipping and scaling

**Verified.** Renderer/window `Rect` values and runtime input are physical
pixels. `AppModel::metrics: ScaledMetrics` derives tab, splitter, padding,
border and scrollbar metrics from display scale. The renderer derives code-font
size, line height and character advance at that scale. Convert a logical design
value once with `round(value * scale_factor)` and retain a one-pixel stroke
floor. Sidebar widths are deliberately logical configuration values and convert
at that boundary.

**Required.** A new component declares coordinate space, rectangle owner, clip
rectangle and rounding point. Solve layout before both paint and hit testing;
do not mix logical/physical values or independently derive a box in the
runtime. `Frame` clip stacks are mandatory for overflow. Floating surfaces use
the existing anchoring helpers (edge clamp and caret flip), not copied math.
Scroll units must match their surface: editor scrollbar state is physical
pixels; row lists/settings records use their own row/record units.

## State, focus and input

### Ownership — verified

Component owners keep committed and transient state in the model. Shared code
may calculate rectangles, paint an explicit visual state, map pointer to value,
or build a typed drag payload. It does not save settings, load themes, mutate a
document, open a file or decide policy. The gallery proves the boundary:
[`model/gallery.rs`](../../src/model/gallery.rs) owns isolated state and
[`bin/ui_gallery.rs`](../../src/bin/ui_gallery.rs) has no document,
session or settings persistence.

### Dispatch/capture — verified

[`view/hit_test.rs`](../../src/view/hit_test.rs) returns typed `HitTarget`;
[`runtime/mouse.rs`](../../src/runtime/mouse.rs) dispatches it
and calls update. Priority prevents an overlay, scrollbar, tab, gutter lane or
splitter becoming a text click. `FocusTarget` is currently coarse (`Editor`,
`Dock`, `FindBar`, `Modal`); `HoverRegion` separately drives cursor/wheel
routing. Runtime owns native focus/cursor; model focus owns keyboard routing.

Pointer capture is model drag state: `ScrollbarDragState`, `SplitterDragState`,
tab drag, sidebar resize and dock resize retain press-time identity/geometry.
Scrollbar release ends capture. Escape cancels a splitter and restores original
ratios. A changing layout causes the splitter update to ignore an invalid frame
rather than use stale indices.

### Baseline for proposals

Every interactive component must state press target/capture payload (or none),
release/cancel/window-focus-loss behavior, focus target, keyboard traversal and
activation, Escape behavior, and disabled/unavailable consumption. A painted
focus ring does not make a control keyboard accessible. Token has no general
accessibility tree or screen-reader adapter today: expose semantic role/name and
keyboard-equivalent action in new contracts, but call it a gap until implemented.

## Fonts, theme roles and icons

### Fonts — verified

[`view/fonts.rs`](../../src/view/fonts.rs) loads a configured monospaced editor face and proportional UI
face, falling back to embedded JetBrains Mono and Inter; a non-monospace editor
font is rejected. `TextPainter::FontRole::{Code,Ui}` switches font plus an
independent glyph cache/ascent. Editable text uses `Code`; Token also currently
uses `Code` for editor/explorer text, document/dock/terminal tabs and gallery
chrome fixtures; overlay tabs and section navigation use `Ui`. Use `Ui`
only where the production painter selects it (for example UI labels and
proportional chrome), and make role choice explicit for every surface. Scoped
`with_font` prevents a temporary role from leaking. Never use code-cell width to
truncate or position a `Ui` label: measure its glyph advances.

### Themes — verified

[`theme.rs`](../../src/theme.rs) parses YAML and resolves compatibility fallbacks into `Theme`.
Existing semantic families are `editor`, `gutter`, `status_bar`, `overlay`,
`tab_bar`, `splitter`, `sidebar`, `csv`, `button`, `image_preview`, `syntax`,
and `scrollbar`. Select the surface's semantic role—not a hard-coded ARGB value
or a similar-looking role from another family. Gallery renders the current
resolved palette. Theme loading is a runtime effect returned by messages.

**Proposed:** add a YAML role only for a concrete visual distinction and
production consumer, with a fallback for existing themes. Geometry, type size
and semantics do not belong in theme keys. Verify a new role in a bundled light
and dark theme; fallback behavior is part of compatibility.

### Icons — verified/proposed

Token mostly paints UI icons as glyphs/badges; [workspace](../../src/model/workspace.rs)
and [panel](../../src/panels/mod.rs) helpers explicitly
use Nerd Font code points. It has no general SVG registry, canonical size table,
semantic accessible name or icon gallery. Before broad icon work, define an
`Icon` asset/semantic contract. An icon-only action needs a text label/tooltip
and keyboard equivalent; status needs shape/text as well as color. Prefer simple
scalable assets or an explicitly supported glyph font with a known fallback.
IntelliJ similarly distinguishes action/noun/status icons and says status shape
must not rely on red/green; its 12/13/16px values are reference sizes, not Token
constants. [Icon style guidance](https://plugins.jetbrains.com/docs/intellij/icons-style.html)

## Reuse and acceptance

Primary seams are [scrollbar](../../src/view/scrollbar.rs),
[button](../../src/view/button.rs), [controls](../../src/view/controls.rs),
[text field](../../src/view/text_field.rs),
[overlay surface](../../src/view/overlay_surface.rs),
[editor layout](../../src/layout/editor.rs), [chrome layout](../../src/layout/chrome.rs),
[editor text](../../src/view/editor_text.rs), and [geometry](../../src/view/geometry.rs).
Extract only
when production and gallery, or two production consumers, share true semantics
and geometry.

### Ownership and implementation-overlap matrix

| Surface/mechanism   | Model owner                                       | Geometry/painter authority                                                              | Existing consumers                     | Do not generalize past                                 |
| ------------------- | ------------------------------------------------- | --------------------------------------------------------------------------------------- | -------------------------------------- | ------------------------------------------------------ |
| scrollbar           | target surface plus `ui.scrollbar_drag`           | [scrollbar](../../src/view/scrollbar.rs) and target layout                              | editor, overlays, Settings, gallery    | target-specific row selection and scroll units         |
| editor group        | `EditorArea`, `EditorGroup`, `EditorState`        | [GroupLayout](../../src/view/geometry.rs), [editor text](../../src/view/editor_text.rs) | text, CSV, image, binary tabs          | generic text field or special-tab rendering            |
| tab strip           | `EditorGroup::tabs`, active index, tab scroll     | [EditorTabBarLayout](../../src/layout/editor.rs)                                        | editor tabs, gallery chrome            | dock and terminal tab policies                         |
| chrome row list     | feature panel/modal state                         | [layout snapshot](../../src/layout/snapshot.rs)                                         | Problems, Outline, Usages and overlays | editor's visual-row mapping                            |
| splitter            | `EditorArea` layout ratios and `ui.splitter_drag` | [editor-area traversal](../../src/model/editor_area.rs)                                 | editor/preview tree, gallery swatch    | dock/sidebar resize persistence/units                  |
| field/select/button | caller owns value/focus/commit                    | [controls](../../src/view/controls.rs), [button](../../src/view/button.rs)              | Settings, Find, gallery                | theme loading, validation or form persistence          |
| icon-like output    | feature/domain owner                              | current glyph/badge painter                                                             | file rows, overlays, gutter, buttons   | a registry until semantic IDs and fallback are defined |

This is deliberately a matrix of seams, not a class hierarchy. Where an entry
does not share owner semantics, only share pure geometry/paining primitives.

A slice is acceptable when it has a named owner and typed messages; one geometry
authority for paint/hit/clip/scroll; scale/font/theme roles with light/dark
fallback checks; documented pointer/keyboard/focus/cancel behavior; no helper
effect; and a gallery specimen using its production painter where feasible.

## Evidence and confidence

High-confidence implementation evidence: [model](../../src/model/),
[messages](../../src/messages.rs), [update](../../src/update/),
[runtime mouse](../../src/runtime/mouse.rs), [layout](../../src/layout/),
[view](../../src/view/), [theme](../../src/theme.rs) and
[gallery guide](../dev/ui-gallery.md), reviewed 2026-09-12. **Proposed** sections are not
implemented. The local IntelliJ SDK material was a secondary cross-check;
official pages above are primary UX reference.
