# Tabs

## Scope and naming

Tabs select one peer-owned surface from a peer set. They are navigation, not a
generic row control and not an action toolbar. **Verified:** Token has four
incompatible tab families. Keep their state, layout, and behavior distinct.

| Family        | Selects                   | State owner        | Layout/hit-test authority       | Consumer                |
| ------------- | ------------------------- | ------------------ | ------------------------------- | ----------------------- |
| Document tabs | one editor in one group   | `EditorGroup`      | `EditorTabBarLayout`            | split editor            |
| Dock tabs     | one `PanelId` in one dock | `DockLayout`       | chrome `LayoutSnapshot`         | left/right/bottom docks |
| Terminal tabs | one terminal session      | terminal state     | terminal chrome viewport        | Terminal dock           |
| Overlay tabs  | one search category       | modal/search state | `OverlaySpec` → `OverlayLayout` | Search Everywhere       |

This distinction parallels IntelliJ's editor tabs, tool-window content, and
popup navigation; it does not authorize adopting Swing/Kotlin infrastructure.
IntelliJ's editor policy describes scrolling, squeezing, and multi-row choices;
Token currently implements only horizontal scrolling. [Editor Tabs](https://www.jetbrains.com/help/idea/settings-editor-tabs.html)

## Verified implementation

### Document tabs

`EditorGroup` owns ordered `Vec<Tab>`, `active_tab_index`, group rectangle, and
physical-pixel `tab_scroll`; a tab identifies an editor (`src/model/editor_area.rs:66-103`).
`EditorArea::tab_display_name` centrally joins tab → editor → document so layout
and painter cannot disagree; it suffixes ` !` for external change/save error
and otherwise falls back to `Untitled` (`src/model/editor_area.rs:252-271`).

| Verified field/state      | Meaning                                  | Mutation route             |
| ------------------------- | ---------------------------------------- | -------------------------- |
| `Tab.id`, `editor_id`     | stable tab/editor identity               | layout/open/close handlers |
| `is_pinned`, `is_preview` | reserved flags; unused today             | none (explicit TODO)       |
| `active_tab_index`        | selected peer in that group              | `LayoutMsg::SwitchToTab`   |
| `tab_scroll`              | horizontal display offset in physical px | `LayoutMsg::ScrollTabBar`  |
| `focused_group_id`        | which group receives editor input        | `LayoutMsg::FocusGroup`    |

`EditorTabBarLayout` is the sole authority for tab flow, gap, scroll offset,
clip chain, visible tab rectangle, title origin, total width, and hit testing
(`src/layout/editor.rs:32-180`). Width is code-character count plus metrics
padding (`:20-30`); it is intentionally shared with drag-ghost sizing. The
painter consumes the solved layout, paints `theme.tab_bar` active/inactive
roles, and clips every title to its visible tab rect (`src/view/document_tabs.rs:1-54`).

Pointer press on a tab focuses its group, switches index, and arms a drag;
empty bar press only focuses its group (`src/runtime/mouse.rs:2228-2255`). Wheel
input finds the bar through the same layout and emits `ScrollTabBar`
(`src/runtime/mouse.rs:3198-3220`). Tests in `src/layout/editor.rs:301-381`
cover hit/empty space, clipping at a group edge, and scrolling.

### Dock tabs

Dock tab order/selection are owned by the dock model, not an editor group.
`DockPaneScene::resolve` reads `panel_ids` and `active_index`, resolves each
`UiKey::DockTab`, then chooses the active panel's content (`src/view/panels.rs:45-124`).
Header/content rects are from the shared chrome snapshot; painting uses sidebar
selection, foreground, background, and border roles. A dock tab is a container
selector: it must not acquire document drag, terminal cycling, or overlay counts.

### Terminal tabs

Terminal tabs select sessions and coexist with Previous, Next, New, Close
actions. `declare_tabs` reserves fixed action squares around a clipped,
horizontally-scrolled viewport and keys each session by `TabAction::Select(id)`
(`src/panels/terminal.rs:17-76`). `reveal_active_tab` uses the solved viewport
and active-tab bounds after resize/font changes (`:79-97`). The painter clips
the strip and viewport, sanitizes/truncates titles, marks exited sessions, and
uses sidebar selection/hover roles (`:99-180`). `TerminalMsg::Tab` performs
select/cycle/new/close, with spawn/close emitted as commands (`src/update/terminal.rs:16-82`).

### Overlay tabs

`overlay_surface::TabBar { tabs, active }` is an input to layout/render, not
persistent widget state. `TabCount::{Hidden,N,Pending,Unavailable}` provides
the small count/status label (`src/view/overlay_surface.rs:285-311`). The
overlay layout reserves its optional tab bar, declares a `UiKey` for each tab,
and publishes the exact rectangles in `OverlayLayout`
(`src/view/overlay_surface.rs:850-872`, `:1018-1040`, `:1354-1450`). The
caller maps those hits to its modal message. Current use is search categories,
not document/dock selection.

## Geometry, events, focus, accessibility

**Verified geometry invariant:** document paint, title clipping, tab hit test,
wheel target, and drag sizing derive from `EditorTabBarLayout`; dock and terminal
from chrome snapshot; overlay from `OverlayLayout`. A new feature must not
recompute tab boxes or ordering in its event handler.

**Verified focus:** document pointer interaction returns `FocusTarget::Editor`.
Dock/terminal input has domain focus routes; modal overlays receive key priority
before normal editor input (`src/runtime/input.rs:220-285`, `:1155-1185`).
No semantic accessibility tree, role/name/value API, document-tab focus ring,
close glyph, tooltip, pin behavior, or preview replacement is verified.

**Proposed common minimum:** each family exposes stable ID, label, selected,
enabled/unavailable status, accessible name, and a family-local activate message.
The owner validates selected identity after remove/reorder, maps pointer and
keyboard through current order, and owns effects. Selection must have a visible
non-colour cue. Do not define universal Left/Right, Ctrl+Tab, Home/End, close,
or drag rules: specify them separately per family when implemented.

## Layout, theme, typography

Document and terminal strip height derive from `ScaledMetrics`; document and
terminal labels deliberately use `FontRole::Code`. Overlay/section UI can use
UI font. `TextPainter` keeps Code/UI metrics and glyph caches independent
(`src/view/frame.rs:832-930`; `src/view/fonts.rs:85-128`). Family palettes are
not interchangeable: document `tab_bar`, dock/terminal `sidebar`, overlay
`overlay` (`src/theme.rs:637-755`). All new geometry must scale via existing
metrics and clip overflow at the solved viewport.

## Gallery, gaps, and acceptance

The gallery contains document state/overflow/drag, dock active, terminal
normal/overflow/exited, and overlay counts specimens
(`src/model/gallery.rs:82-138`); it is isolated from editor settings (`:1-2`).

Priority 1: add focused/keyboard/pointer activation specimens for every family,
and a selected document tab clipped at each edge. Priority 2: implement then
document close/pin/preview/accessibility contracts. Acceptance requires: shared
geometry for paint/hit tests; valid selection through remove/reorder; clipped
text never crosses panes; family-local keyboard tests; and gallery states for
selected/inactive/overflow/hover-or-focus/disabled-or-unavailable/HiDPI.

## Sources

Token evidence is cited inline. Primary UX references: [IntelliJ editor-tab policy](https://www.jetbrains.com/help/idea/settings-editor-tabs.html)
and [Platform UI overview](https://plugins.jetbrains.com/docs/intellij/ui-overview.html).
The secondary local cross-check is `temporary-docs/intellij-platform-sdk/references/ui-settings-and-toolwindows.md`.
