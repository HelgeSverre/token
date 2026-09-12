# Panel

## Purpose and naming

A **panel** is persistent, docked task content within the main Token window.
Code calls its outer container a _dock_ and its content identity a `PanelId`.
Use “panel” for the tabbed task surface and “dock” for the left/right/bottom
container. This maps to IntelliJ tool windows—panes supporting work alongside
the editor—without importing the IntelliJ platform model
([UI overview](https://plugins.jetbrains.com/docs/intellij/ui-overview.html),
[Tool windows](https://plugins.jetbrains.com/docs/intellij/tool-windows.html)).

## Current implementation — high confidence

`DockLayout` owns three `Dock`s (`Left`, `Right`, `Bottom`). A dock holds its
registered `PanelId`s, active index, open bit and logical size; focus lives
separately in `UiState::focus: FocusTarget::Dock(DockPosition)`. Defaults are
open Explorer on the left, Outline on the right, and Terminal/Problems/Usages
in the bottom dock. A dock has 150 logical-px minimum and a maximum half-window
fraction; it is scaled by `metrics.scale_factor`.

| Implemented panel content | State/consumer                                      |
| ------------------------- | --------------------------------------------------- |
| Explorer                  | left dock/tree state and shared tree render path    |
| Outline                   | `OutlinePanelState`, collapsible hierarchy          |
| Terminal                  | terminal sessions/grid/tab painter                  |
| Problems                  | diagnostics-derived, collapsible rows and selection |
| Usages                    | `UsagesPanelState`, result rows/loading/status      |
| Tasks, Chat, TODOs        | `PlaceholderPanel`; not feature-complete panels     |

### Implemented data ownership (field-level)

| Field/symbol                                                                     | Writer                                | Reader / invariant                                                                          |
| -------------------------------------------------------------------------------- | ------------------------------------- | ------------------------------------------------------------------------------------------- |
| [`DockLayout::{left,right,bottom}`](../../src/panel/dock.rs)                     | dock update/config persistence        | chrome, hit test and renderer read same dock position                                       |
| [`Dock::{panel_ids,active_index,is_open,size_logical}`](../../src/panel/dock.rs) | `Dock` methods and `DockMsg` handlers | active index addresses registered IDs; closed dock reports physical size zero               |
| [`UiState::focus`](../../src/model/ui.rs)                                        | input/dock update                     | `FocusTarget::Dock(position)` is intentionally outside persisted `Dock`                     |
| [`DockResizeState`](../../src/model/ui.rs)                                       | pointer press/drag/release            | captures dock, axis, start coordinate/original logical size; renderer holds no resize state |
| Outline/Problems/Usages/terminal models                                          | their feature updates                 | selection, scroll, loading/empty/session state remain domain-owned                          |

`DockLayout::active_panel_position(panel_id)` is input/side-effect authority
because a panel must not be assumed to occupy its default dock.

`view/panels.rs` resolves a `DockPaneScene` from the layout snapshot and
renders chrome, tab header, a clipped content rectangle, and selected content.
`layout/chrome.rs` supplies `UiKey::Dock`, header/tab/content/row-list geometry;
this avoids independent render and hit-test math. `DockMsg` handles toggling,
activation, focus/close, cycling, and resizing in update/runtime paths.

The left Explorer is a deliberate current special case: left-dock state is
synchronized to workspace sidebar state and rendered by `render_sidebar`; the
generic `render_dock` path is called for right and bottom only. Treat it as a
panel-placement consumer, not evidence that every dock side shares one painter.

### Anatomy, interaction, lifecycle

An implemented dock has a resize boundary, header/tab strip, active tab,
optional terminal sub-tabs, and clipped content. Tab activation opens its dock;
panel focus is distinct from visibility. Pointer/keyboard routing resolves the
active panel’s _current_ dock rather than assuming its default position.
`DockResizeState` captures direction, starting coordinate and original logical
size. Tree/Problems/Usages own their own selections and scroll; terminal owns
terminal-specific interaction. Those domain behaviors must not be forced into a
generic “panel list” API.

| Transition (implemented)    | Owner                                      | Invariant                                                  |
| --------------------------- | ------------------------------------------ | ---------------------------------------------------------- |
| register/activate           | `Dock`/`DockLayout`                        | panel occurs once; activation opens containing dock        |
| focus-or-toggle/close/cycle | `DockMsg` update path                      | focus follows active panel’s actual dock, not default dock |
| resize press/drag/release   | `DockResizeState` + shared chrome/hit test | logical size is clamped; no unscaled duplicate geometry    |
| panel row select/scroll     | corresponding domain model                 | row list/layout is authority for draw, click and keys      |
| unavailable/pending content | feature model                              | paint scoped empty/loading/status, not global notification |

The general panel system has no plugin registration, tear-off, move-between-
docks action, per-tab close affordance, arbitrary panel factory or accessibility
tree. The enum includes placeholders; do not advertise them as implemented
tools. IntelliJ’s content/tab closing options are useful reference only
([Tool windows](https://plugins.jetbrains.com/docs/intellij/tool-windows.html)).

### Geometry/theme/accessibility

`Dock::size_logical` converts to physical pixels only when open; geometry comes
from the shared chrome snapshot. Dock chrome uses `Theme::sidebar` background,
border, foreground and selection roles, and panel content is clipped with
`Frame::push_clip`. Header text uses a code font in the existing renderer;
individual content supplies appropriate font/metrics. There is no verified
screen-reader role, roving tab focus or focus indicator across dock tabs.

#### Measured-layout and clipping invariants — implemented

- `layout::chrome` emits `UiKey::Dock`, `DockHeader`, `DockTab`,
  `PanelContent` and panel row-list keys. `DockPaneScene::resolve` reads them
  into a render scene; it only falls back if active content geometry is absent.
- `DockPaneScene::render` paints chrome/header then `Frame::push_clip`s content
  before dispatching the selected renderer. Header painting separately clips to
  `header_rect`; terminal tabs follow their dedicated terminal renderer.
- `Dock::size_logical` becomes physical in `Dock::size(scale_factor)` only when
  open. `set_size` converts physical drag coordinates back; min is 150 logical
  px and max is half window fraction. Resize limits belong to update/layout,
  never a painter.
- Active tab rect/text positions are `LayoutSnapshot` node/content rects; wash
  alpha blends sidebar selection over dock background. Do not independently
  center or measure tab titles.

## Proposed contract

Keep `PanelId`/`Dock` as placement and persistence authority. A new real panel
should declare, rather than inherit unrelated behavior:

```text
PanelState (domain data, selection, scroll, loading/error/empty state)
PanelEvent (activate, focus, select, scroll, command-specific action)
PanelView (header title, content layout key, content renderer/hit mapping)
```

The update layer owns mutations and commands; renderer receives immutable
state and a shared layout rect. A new panel must provide an empty/loading/error
view, focus and Escape policy, selection order shared by render/click/keyboard,
and resize/clip behavior. Do not create a generic tab abstraction for editor
tabs, terminal sessions and dock tabs: their lifetime and close semantics differ.

## Gallery, gaps, acceptance

Gallery specimens `dock-tabs.active`, `panel.bottom-empty`, and
`panel.right-empty` exercise real dock chrome. Terminal and document-tab
specimens cover adjacent but different tab systems. Missing coverage: resizing,
overflowed dock tabs, focus, all dock sides, populated/selected/problems rows,
and light/HiDPI.

Acceptance: dock content uses shared `UiKey` geometry in render/hit test;
opening/selecting/focusing a panel has deterministic effects; logical size
persists and clamps; content cannot paint outside the dock; every real consumer
handles empty/loading/error; and placeholders remain visibly provisional.

Concrete workflow acceptance: with bottom dock closed, Focus-or-Toggle Problems
must open/activate/focus it; invoking again closes it and restores editor focus;
activate Terminal and cycle back; resize at 1x/2x; then select Problems rows.
Current dock, tab rect, focus target, clipped content and row activation must
agree throughout.

## Evidence

- [Dock model](../../src/panel/dock.rs), [panel names/placeholders](../../src/panels/mod.rs), [placeholder text](../../src/panels/placeholder.rs)
- [Dock painter](../../src/view/panels.rs), [layout keys](../../src/layout/keys.rs), [messages](../../src/messages.rs)
- [Gallery catalog](../../src/model/gallery.rs)
- [Local IntelliJ SDK tool-window reference](../../temporary-docs/intellij-platform-sdk/references/ui-settings-and-toolwindows.md) (secondary)
- [IntelliJ tool-window overview](https://plugins.jetbrains.com/docs/intellij/tool-windows.html) and [UI overview](https://plugins.jetbrains.com/docs/intellij/ui-overview.html)
