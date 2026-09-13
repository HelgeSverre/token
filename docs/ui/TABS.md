# Tabs implementation manual

Token has four visually similar but mechanically different tab families. They
must not converge into a generic `Tabs` state machine: they select different
resources, use different geometry authorities, and have different effects.

| Family   | Selects              | Durable owner                | Geometry / hit authority | Font |
| -------- | -------------------- | ---------------------------- | ------------------------ | ---- |
| document | an editor in a group | `EditorGroup` / `EditorArea` | `EditorTabBarLayout`     | Code |
| dock     | one panel in a dock  | `DockLayout`                 | chrome `LayoutSnapshot`  | Code |
| terminal | a PTY session        | terminal model               | chrome terminal viewport | Code |
| overlay  | a search category    | modal/search state           | `OverlayLayout`          | UI   |

Document tabs are navigation between editor views. Dock tabs choose a container
panel; terminal tabs select a running session and can spawn/close; overlay tabs
are a layout input for a modal category. A keyboard or drag rule for one family
is not evidence for the others.

## 1. Document tabs: current representation and invariants

```rust
// current excerpt — src/model/editor_area.rs:66-105
pub struct Tab {
    pub id: TabId,            // stable tab identity; never use current index externally
    pub editor_id: EditorId,  // joins tab → editor → document
    pub is_pinned: bool,      // reserved; not implemented behavior
    pub is_preview: bool,     // reserved; not implemented behavior
}
pub struct EditorGroup {
    pub id: GroupId,
    pub tabs: Vec<Tab>,
    pub active_tab_index: usize,
    pub rect: Rect,
    pub tab_scroll: usize,    // physical px from left edge
}
```

`EditorArea` owns documents/editors/groups and resolves a tab title centrally,
including the external-change/save-error suffix (`src/model/editor_area.rs:216-271`).
The durable order is the group's `Vec<Tab>`; an index is only valid while that
vector remains unchanged. The owner must maintain:

```text
tabs.is_empty() ⇒ active_tab_index is never dereferenced
!tabs.is_empty() ⇒ active_tab_index < tabs.len()
tab_scroll ≤ max(0, total_tabs_width - bar_width)
every Tab.editor_id resolves while the tab survives
```

Closing/reordering/moving code is in `update::layout`; action messages carry a
stable `TabId` for move/reorder/close, while current pointer selection supplies
an index only after it was resolved against the current frame
(`src/messages.rs:412-485`, `src/update/layout.rs:36-189`). A removal repair
must choose the prescribed neighbouring tab before exposing the group to render,
then re-clamp/reveal. Never retain the clicked index through an asynchronous
close confirmation or a reorder.

## 2. Document tab geometry, clipping, and scroll repair

`EditorTabBarLayout` builds a small `UiTree` for one group and is the one source
for paint, hit, wheel targeting, title clipping, drag ghost sizing, spans, and
total width (`src/layout/editor.rs:18-180`). It rounds group bounds, applies
the bar's clip chain, flows a horizontally scrolled row, and assigns
`UiKey::EditorTab(group_id, tab_id)` to each tab.

```text
title_width = round(char_count(title) × char_width) + 2 × padding_large
tab_x_screen = bar_x + padding_medium + Σ(previous widths + gaps) - tab_scroll
visible_rect = intersection(solved tab rect, its inherited clip)
total_width = unscrolled end of last tab + trailing padding_medium
max_scroll = saturating_sub(total_width, round(bar_width))
```

Titles are intentionally measured in `chars().count()` for this monospace
chrome policy. They are not byte counts. Paint clips glyphs to `visible_rect`
without shifting their title origin; input calls `snapshot.hit`, not a second
`x / width` formula.

The active-reveal algorithm computes an unscrolled `(start,end)` span. Given
current `s`, bar width `W`, and margin `p=padding_medium`:

```text
if start - p < s:              s' = start - p (saturating at 0)
else if W > 0 and end + p > s + W: s' = end + p - W
else:                          s' = s
tab_scroll = min(s', max_scroll)
```

This runs after implemented tab actions such as selection, new, move, and
reorder (`src/update/layout.rs:36-189`, `:260-306`). It is not currently
invoked as a general resize/font-change repair. Example: `W=300`, `p=8`,
`start=410`, `end=500`, `s=120` produces `s'=208`, placing the tab at x=202
through x=292. If width later shrinks to 100, recomputation produces `408`;
retaining `208` would clip the active tab. Empty tabs yield total width zero and
scroll zero rather than underflowing.

## 3. Document tab input and drag state

Pointer press on a solved tab focuses its group, switches the current index,
and records `TabDragState { tab_id, press, current, active }`; press in empty
bar space focuses only (`src/runtime/mouse.rs:2228-2255`,
`src/model/ui.rs:1345-1365`). After the 4px threshold, runtime drag handling
uses the stable ID to emit reorder/move. Wheel finds the same layout and sends
`ScrollTabBar { group_id, delta_px }` (`src/runtime/mouse.rs:3198-3220`).

| Drag state   | Event                     | Required result                                                                           |
| ------------ | ------------------------- | ----------------------------------------------------------------------------------------- |
| idle         | press visible tab         | focus group, select current tab, capture `TabId`, arm drag                                |
| armed        | move ≤ threshold          | selection remains; no reorder                                                             |
| armed        | move > threshold          | mark active; target derives from current solved tab geometry                              |
| active       | hover tab bar/pane target | live `MoveTab`/`ReorderTab` using stable ID; target order immediately becomes model order |
| active       | release                   | only clear drag state and redraw; tab is already at its live destination                  |
| armed/active | source removed            | subsequent live lookup fails safely; release clears state                                 |

Current drag handling performs move/reorder _during hover_ and `end_tab_drag`
only clears its state (`src/runtime/mouse.rs:1641-1709`). Escape/focus-loss
cancellation is not current behavior and must not be represented as one. It is
a proposed hardening rule: cancel transient drag state on either event and
never apply a deferred drop. Close glyph/pin/preview behavior, tab keyboard
roving focus, and accessible tab semantics also do not exist. Proposed document
keyboard navigation should produce a `TabId`/current-order choice in the update
layer and reveal the result; it must not reuse terminal cycling by implication.

## 4. Dock tabs: panel selection, not document tabs

Dock layout owns panel ID order and active index. `DockPaneScene::resolve`
reads that state, resolves each `UiKey::DockTab`, and renders only the active
panel content from the shared chrome snapshot (`src/view/panels.rs:45-124`).
Dock tabs should carry `PanelId` across a reorder/removal; an active index is
repaired by the dock owner after a panel is removed. They do not acquire editor
drag, document close, or terminal process side effects.

The layout snapshot is rebuilt on dock bounds/active panel/metrics changes. Hit
testing and paint must query its `UiKey`s from that same snapshot. When a dock
tab activates, its update owns focus and any panel-specific scroll reset/reveal;
the tab component does not reset arbitrary panel state.

## 5. Terminal tabs: session selection with reserved controls

Terminal declaration reserves fixed action squares for Previous, Next, New,
and Close, clips the intervening viewport, then lays sessions at fixed
`22 × char_width` widths under `terminal.tab_scroll`
(`src/panels/terminal.rs:17-76`). This solves the classic failure where
overflowing tabs cover the close/new buttons.

`TabAction::Select(session.id)` contains the stable session identity. Update
maps it to the current index, cycles modulo the nonempty session count, focuses
the owning dock, and emits spawn/close commands only for New/Close
(`src/update/terminal.rs:16-96`). `reveal_active_tab` compares solved session
and viewport rects after dock/font resize, adding the needed delta and clamping
at zero (`src/panels/terminal.rs:79-97`). An exited session remains selectable;
the painter marks it rather than silently removing it.

Terminal close is asynchronous: retain the session ID in the close command and
ignore subsequent PTY/title events if `session_mut(id)` no longer exists. A
late event must not change whichever session happened to reuse the active index.

## 6. Overlay tabs: declarative category geometry

`overlay_surface::TabBar { tabs, active }` is input to a newly solved overlay,
not persistent widget state. `TabCount::{Hidden,N,Pending,Unavailable}` defines
the count/status label; `Unavailable` is non-clickable and is skipped by its
overlay navigation (`src/view/overlay_surface.rs:285-311`). The layout publishes
`tab_bar` and one `tab_rect` per input tab in `OverlayLayout`
(`src/view/overlay_surface.rs:850-880`, `:1354-1450`).

The search/modal owner must retain a stable category identity or generation
alongside active index. On category removal or changed availability, keep the
same category if available; otherwise select the first available category;
discard result batches whose category/query generation no longer matches.
Overlay input has priority before normal editor input, and focus loss/dismissal
cancels its transient press state (`src/runtime/input.rs:220-285`).

## 7. Invalidation and verification

| Family   | Recompute geometry when                                | Identity repair / stale work                                                                   |
| -------- | ------------------------------------------------------ | ---------------------------------------------------------------------------------------------- |
| document | order/title/char width/metrics, group rect, tab scroll | tab ID after close/move/reorder; missing drag source yields no live move and release clears it |
| dock     | panel order/active state, dock rect, scale             | `PanelId` after panel removal                                                                  |
| terminal | session order/title, scroll, dock rect/font            | session ID after close; ignore late PTY events                                                 |
| overlay  | category/count/availability, overlay bounds/font/query | category ID + query generation after replacement/dismiss                                       |

Document layout tests already cover hit/empty space, clipping, scroll and drag
scenarios (`src/layout/editor.rs:301-381`; `tests/layout.rs:1827-1960`). Add
or retain these executable cases:

| Case                   | Action                                        | Expected                                                           |
| ---------------------- | --------------------------------------------- | ------------------------------------------------------------------ |
| active reveal          | active span 410..500, bar 300, margin 8       | offset 208; title clipped only at viewport                         |
| proposed resize reveal | same tab, bar narrows to 100                  | future resize repair recomputes offset 408; not current behavior   |
| remove active          | `[a,b,c]`, active `b`, close completes        | valid neighbor ID active, index in bounds, scroll clamped          |
| live reorder           | press `b`, cross threshold, hover target slot | `b` moves by ID during move; release only clears ghost/state       |
| terminal overflow      | viewport narrower than five sessions          | action buttons hitable; session paint clipped inside viewport      |
| terminal late event    | close session ID 7 then `TitleChanged(7,...)` | no state mutation/panic                                            |
| overlay unavailable    | active category becomes unavailable           | first available becomes active; click unavailable emits no message |

Static gallery specimens should show document edge clipping/drag, active dock,
terminal overflow/exited, and overlay pending/unavailable. They are not a
substitute for lifecycle, focus, and stale-identity tests.
