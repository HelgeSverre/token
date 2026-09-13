# Docked panels: implementation reference

This chapter documents Token's persistent docked work surfaces. A **dock** is
the left, right, or bottom container; a **panel** is the selected `PanelId`
inside it. This is not the editor-tab or terminal-session-tab model: those have
different owners and lifetimes.

## Current representation and invariants

`DockLayout` is serializable durable layout state. Pointer drag state and
keyboard focus are deliberately elsewhere. This is an abridged current excerpt
from [src/panel/dock.rs](../../src/panel/dock.rs):

```rust
// current excerpt — `size_logical` is logical px, not framebuffer px.
pub struct Dock {
    pub position: DockPosition,       // Left | Right | Bottom
    pub panel_ids: Vec<PanelId>,      // unique identities, tab order
    pub active_index: Option<usize>,  // index into `panel_ids`, if present
    pub is_open: bool,                // visibility, independent of focus
    pub size_logical: f32,            // side width or bottom height
}
pub struct DockLayout { pub left: Dock, pub right: Dock, pub bottom: Dock }
```

`PanelId` is an enum identity, not a per-instance key. The live location is
`DockLayout::find_panel`; `default_position()` is registration advice only.
`active_panel_position` is the authority for input/effects because a panel must
not be assumed to still occupy its default dock. Defaults register Explorer at
left, Outline at right, and Terminal/Problems/Usages at bottom; only left is
open. Side docks start at 250 logical px, bottom at 200.

| State kind              | Owner                                   | Field/unit/identity and invariant                                                                                                                                                                    |
| ----------------------- | --------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| durable placement       | `DockLayout`                            | source methods prevent duplicate registration only within one dock; serde/current mutation do **not** normalize adjacent or cross-dock duplicates, and a valid `active_index` addresses its own list |
| transient pointer input | `UiState::{sidebar_resize,dock_resize}` | one captured dock, axis, physical start coordinate, original logical extent                                                                                                                          |
| focus                   | `UiState::focus: FocusTarget`           | `Dock(position)` routes keys; it does not itself open/select a panel                                                                                                                                 |
| derived presentation    | `chrome(model) -> LayoutSnapshot`       | per-frame physical-px rectangles keyed by dock/panel identity; absent means invisible                                                                                                                |
| content model           | panel domain owner                      | rows, selected index, scroll, queries, errors, terminal sessions never belong to `Dock`                                                                                                              |

`register_panel` repairs the empty-list case by selecting index 0 and prevents
an exact duplicate in that one list. `active_panel()` safely returns `None` if
an index has become out of range, but serde/current mutation do not normalize
such a layout, nor do they enforce one `PanelId` across all three docks. There
is no production unregister API; configuration/deserialization that removes IDs
must repair the index and close an empty dock, rather than asking a renderer to
do it.

```rust
// proposed API — not implemented; persistence/configuration owns repair.
fn normalize_layout(layout: &mut DockLayout) {
    let mut seen = std::collections::HashSet::new();
    for dock in [&mut layout.left, &mut layout.right, &mut layout.bottom] {
        let active_id = dock.active_panel(); // capture identity before compaction
        dock.panel_ids.retain(|id| seen.insert(*id)); // stable, global uniqueness
        dock.active_index = active_id
            .and_then(|id| dock.panel_ids.iter().position(|&candidate| candidate == id))
            .or_else(|| (!dock.panel_ids.is_empty()).then_some(0));
        if dock.panel_ids.is_empty() { dock.is_open = false; }
    }
}
```

## Update machine, capture, and effects

`Msg::Dock(DockMsg)` is reduced in [src/update/dock.rs](../../src/update/dock.rs).
The reducer mutates model state, returns `Cmd`, and the runtime performs those
effects before a later message; the renderer never mutates docking state.

| Event                    | Precondition            | Reduction                                                                                    | Intent/effect                                                                                 |
| ------------------------ | ----------------------- | -------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------- |
| `FocusOrTogglePanel(id)` | ID registered           | focused+open+active closes and focuses Editor; otherwise activate/open/focus containing dock | sync left workspace, recompute viewports, redraw, outline refresh/terminal sync if applicable |
| `TogglePanel(id)`        | ID registered           | open active panel closes; all other states activate/open; focus unchanged                    | same geometry work                                                                            |
| `ActivatePanel(id)`      | ID registered           | activate/open and focus dock; active-tab click is a no-op, not close                         | same geometry work                                                                            |
| `CloseFocusedDock`       | focus is `Dock(p)`      | close it, move focus Editor                                                                  | geometry and terminal synchronization                                                         |
| next/previous            | dock focused, count > 1 | modulo index change                                                                          | redraw and possibly terminal sync                                                             |
| resize start             | boundary hit            | capture position/axis/start physical coordinate/original logical size                        | subsequent motion has exclusive drag meaning                                                  |
| resize move              | matching capture        | replace clamped `size_logical`                                                               | recompute viewports; active Terminal may emit spawn/resize                                    |
| pointer release          | capture exists          | clear capture                                                                                | recompute and redraw                                                                          |

Current input sends `EndResize` on pointer release. It does **not** clear dock
or sidebar resize state on focus loss/cancel (unlike some scrollbar paths), so
a stale capture after those events is a current gap and later motion can resize
unexpectedly. A future dispatcher must send `EndResize` for release, cancel,
and focus loss. Dock switching has no generic asynchronous completion. Terminal is the
exceptional consumer: opening/resizing an active Terminal queries current
`PanelContent(Terminal)` geometry and may send `SpawnTerminal` or resize its
grid. It uses live location/visibility, so an old default-bottom assumption
cannot target a relocated/closed terminal.

## Layout, units, clipping, and hit mapping

`layout/chrome.rs` is the sole geometry authority for render, hit test, and
update-layer capacity. It solves a physical-pixel root:

```text
status = [0, window_height - status_bar_height, window_width, status_bar_height]
side_px(d) = is_open(d) && !empty(d) ? size_logical(d) × scale_factor : 0
work = sidebar | (editor + right dock), with bottom dock below that work
```

The result contains `Dock(p)`, `DockHeader(p)`, `DockTab(p,id)`, and
`PanelContent(active)`. Problems/Outline/Usages additionally get
`PanelRows(active)` with physical row height, count, and scroll offset. Missing
keys are not zero-size hit targets: they mean the object is not visible.

The header height is `metrics.tab_bar_height`. It has medium horizontal
padding, small top padding, `(medium-small)` bottom padding, and small tab gaps.
Each tab is Fit-sized by `CellMeasure`: title character count times
`model.char_width`, plus large horizontal/medium vertical tab padding; it does
not use `TextPainter` glyph measurement. The header clips excess tabs. Active content receives the
remaining dock rectangle and also clips. `DockPaneScene` paints chrome/header,
pushes the content clip, then invokes only the active panel renderer. A partial
bottom row may paint/click inside that clip but never escape into header/status.

Resize is logical-pixel arithmetic. For physical pointer `p`, press `p0`,
scale `s=max(scale_factor, ε)`, original extent `o`:

```text
δleft = (p-p0)/s;  δright = δbottom = (p0-p)/s
new = clamp(o+δ, 150, 0.5×window_axis_px/s)
```

Rounding is deferred to `LayoutSnapshot::snap`; retaining a fractional logical
extent avoids DPI drift. Current code should normalize an undersized window
before `clamp`: if `0.5×axis/s < 150`, its bounds invert. That is a boundary
case to fix in the reducer, not a paint-time workaround.

### Geometry traces

At 2x, right dock `o=250`, window width 1600 px, and drag `p0=1200→p=1100`:
`δ=(1200-1100)/2=50`, so state becomes 300 logical px and solved width is 600
physical px. Its maximum is `0.5×1600/2=400`; dragging to 300 produces
`δ=(1200-300)/2=450`, raw `250+450=700`, and clamps to 400 (800 physical px).

Pathological: bottom `o=200` at 1x, 600 px-high window, drag `700→1100`:
`δ=-400`, raw -200, result 150. At height 240 px, right/bottom current code
calls `clamp(150,120)` and panics because max is below min; left separately uses
`new_width.max(150).min(max_width)` and instead yields 120, violating its own
150 minimum. A future reducer must use `max(150, 0.5×axis/s)` before either form.

## Assembly, invalidation, and cost

```text
pointer/key → DockMsg → update_dock(DockLayout, UiState)
            → Cmd::Redraw / terminal command → runtime effect
            → RenderPlan stores chrome(model) once for the frame
            → render_sidebar(left) or render_dock(right/bottom)
            → active domain renderer consumes PanelContent/PanelRows
```

Explorer is deliberately special: left-dock openness/width mirrors workspace
sidebar state and `render_sidebar` paints it. Generic `render_dock` is used for
right/bottom. Problems, Outline, and Usages share row geometry but not rows:
their domain functions (`problems_rows`, outline traversal, `UsagesPanelState::rows`)
are the ordering authority for layout, painting, keyboard, and clicks.

`chrome(model)` is pure and currently cheap enough to recompute; `RenderPlan`
only caches within one frame. Inputs include window/scale/status dimensions,
sidebar state, each dock's open/size/IDs/active ID, tab titles, metrics, and
active row count/scroll. A panel update, collapse, scroll, resize, scale/window
change, or tab activation invalidates derived geometry. Row painters use
`RowListView::drawn_range()`, although producing a row projection can still be
O(number of groups/items); there is no persistent layout cache with stale keys.

## Verification cases and proposed boundary

Existing numeric layout tests live in `tests/chrome_layout.rs`; dock reducer
tests live beside the reducer. Keep these concrete vectors:

| Initial state                                          | Action                    | Expected output                                                                                    |
| ------------------------------------------------------ | ------------------------- | -------------------------------------------------------------------------------------------------- |
| bottom closed, Problems registered, focus Editor       | Focus-or-toggle Problems  | bottom open, active Problems, focus `Dock(Bottom)`, content key exists                             |
| resulting state                                        | same event                | bottom closed, focus Editor, Problems content key absent                                           |
| right 250 logical, 2x, 1600 px window                  | resize 1200→1100          | 300 logical persisted; right rect width 600 px                                                     |
| resize capture active, then focus loss without release | later pointer move        | current gap: capture remains and move can resize; future input test must assert capture is cleared |
| right/bottom axis below 300 logical px                 | resize move               | current `clamp(150,max<150)` panic; left instead violates min; future normalization returns 150    |
| selected row 12 disappears on collapse                 | collapse group            | domain owner selects a surviving group row and reveal uses current `RowListView`                   |
| Terminal closes during synchronization                 | terminal effect selection | no spawn/resize derived for a non-active Terminal                                                  |

A new panel should provide a borrowed per-frame projection, not a framework that
pretends all panels share content semantics:

```rust
// proposed API — types are illustrative and not present today.
struct PanelProjection<'a> {
    id: PanelId, title: std::borrow::Cow<'a, str>,
    body: PanelBody<'a>, content_key: UiKey,
    input: PanelInputPolicy,
}
enum PanelBody<'a> { Message(&'a str), Rows { count: usize } }
enum PanelInputPolicy { Passive, Rows { selected: Option<usize> }, DomainSpecific }
```

It cannot own selection, timers, I/O, or an async request. The domain model
must state its own identity, cancellation/stale-reply guard, empty/loading/error
projection, selection repair, Escape/focus policy, and clipping tests. Tasks,
Chat, and TODOs remain placeholders, not implementations of this contract.

## Evidence

- [Dock model](../../src/panel/dock.rs), [dock reducer](../../src/update/dock.rs), [messages](../../src/messages.rs)
- [shared chrome solver](../../src/layout/chrome.rs), [keys](../../src/layout/keys.rs), [dock renderers](../../src/view/panels.rs)
- [Problems row authority](../../src/update/problems.rs), [Usages model](../../src/model/usages.rs), [Usages reducer](../../src/update/usages.rs)
