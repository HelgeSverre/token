# Section navigation

## Purpose and boundary

Section navigation chooses one category/section of a bounded local surface.
Call it **section navigation**, not tabs, when it is vertical or wraps into a
grid and when the owner retains selection. It is currently shared by Preferences
and UI Gallery; it owns neither their content, focus policy, nor effects.

## Verified implementation and consumers

`SectionNavigation` accepts already-resolved rows, optional divider, selected
index, scale, and labels; it only paints. `section_at` hit-tests those exact
rectangles. `section_rects` produces vertical or row-major grid cells with a
33 logical-pixel step and 28 logical-pixel height (`src/view/section_navigation.rs:1-128`).
Selected state paints a rounded `overlay.keycap_bg` wash; end-ellipsized labels
use `text_bright` or `text_dim`.

| Consumer         | Owner-held state                        | Events / outcome                        | Shared helper use                        |
| ---------------- | --------------------------------------- | --------------------------------------- | ---------------------------------------- |
| UI Gallery       | category, compact choice, focus, scroll | click category resets scroll/focus      | `section_at` for category and width rows |
| Settings overlay | active modal tab and form state         | `OverlayHit::Tab(index)` → modal update | rectangles, hit, and painter             |

The Gallery's `App::click` calls `section_at`, changes its own category or width
state and does not delegate selection to the painter (`src/bin/ui_gallery.rs:92-157`;
categories at `src/model/gallery.rs:4-13`). Settings derives tab rects using
`section_rects`, resolves the hit with `section_at`, and renders the same
component (`src/view/settings_page.rs:376-399`, `:626-632`, `:883-909`).

## Data ownership and state transitions

**Verified data contract:** geometry and labels are positional parallel inputs;
the component has no persistent data model. Its selected index is only a
paint input. That is why the caller owns activation and can reset scroll or
preserve form state without coupling it to navigation paint.

**Proposed contract:**

| Input/output                                     | Responsibility                                                   |
| ------------------------------------------------ | ---------------------------------------------------------------- |
| stable `SectionId`, label, visible/enabled state | owner supplies ordered items                                     |
| selected `SectionId`                             | owner validates after item changes                               |
| bounds, columns, scale                           | layout owner supplies; helper returns one rect/visible item      |
| pointer location                                 | helper returns `Option<SectionId>` from the same rects it paints |
| `ActivateSection(id)`                            | owner update changes selection and returns any command           |

State transition: replace/filter items → retain selected ID if visible,
otherwise choose first enabled → layout once → paint/hit from that result →
pointer/keyboard emits ID → deterministic update changes selection → owner may
scroll/reload detail. Never store a stale index as durable selection.

## Keyboard, pointer, focus, accessibility

**Verified:** Gallery pointer clicks select. Its Tab order is between filter,
width and theme controls, not section-row roving focus. Settings turns a tab
hit into `OverlayHit::Tab`; no common arrow-key selection, disabled item,
focus ring, or accessibility semantics are verified.

**Proposed:** one container focus stop; arrows move by visual grid position,
Home/End reach first/last enabled, Enter/Space activate; outside/gap clicks do
nothing. Define `tablist/tab` versus `navigation/link` role only after intent is
chosen. Expose accessible label/current/disabled state and keyboard instruction,
and paint focus separately from selected/hover colors.

## Geometry, theme, font, and edge cases

The helper rounds logical values at scale and clamps columns to one, so callers
must reuse it rather than derive an independent grid. It truncates against the
resolved rectangle and uses caller's painter at 12×scale (`src/view/section_navigation.rs:34-128`).
Current roles come from `Theme.overlay`, not `tab_bar`/`sidebar`; use UI font
for application navigation. Test empty items, invalid selected index, long
localized label, narrow multi-column bounds, divider edge, high scale, and gaps
between 28px rows/33px steps.

## Gallery, priorities, acceptance

Gallery is a consumer but has no named section-navigation specimen. Priority 1:
add vertical, two-column, selected, keyboard focus, long label, gap miss, and
narrow/HiDPI specimens. Priority 2: ID/disabled support once a third consumer
needs it. Acceptance: one geometry collection drives paint and hit test; no gap
activates; labels cannot cross column/divider; selection survives reorder by ID;
all proposed keyboard paths have focused tests.

## Sources

Primary context: [IntelliJ UI overview](https://plugins.jetbrains.com/docs/intellij/ui-overview.html).
The local secondary reference, `temporary-docs/intellij-platform-sdk/references/ui-settings-and-toolwindows.md`, distinguishes settings/form binding from
general tool-window controls. It is not evidence of Token's implementation.
