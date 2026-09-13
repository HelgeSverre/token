# Cursor popup — Token implementation reference

## Boundary and current representation

Token has a cursor-overlay family, not a generic Popup widget. A visible popup combines shared interaction state with feature-owned payload. OverlaySpec is a borrowed render/hit-test projection, never a store. Current kinds are Completion, Hover, References, Code Actions, and Context Menu, plus debug fixtures ([model](../../src/model/ui.rs)). Signature help is a sibling float and may coexist with completion.

```rust
// Current excerpt; selected/scroll are row indices, not physical px.
pub struct CursorOverlayState {
    pub kind: CursorOverlayKind,
    pub selected: usize,
    pub scroll: usize,
    pub hover_row: Option<usize>,
    pub documentation: DocumentationState,
}

pub struct ContextMenuState {
    pub items: Vec<MenuItem>,
    pub anchor: (usize, usize, usize), // physical x, y, height
    pub region: ContextMenuRegion,
}
```

UiState.cursor_overlay is optional, giving the one-popup invariant. Completion rows live in completion_menu; hover text/dwell anchor in hover_card; references, actions, and context-menu rows in their dedicated fields. Context-menu separators have display slots but no FlatIndex. selected must remain valid for the current feature vector; feature update repairs/removes state when payload changes. View projection clamps before access.

The selected-to-payload mapping is not uniform:

| kind        | acceptance predicate                                                     | selected indexes                                                           | activation validity                                                                     |
| ----------- | ------------------------------------------------------------------------ | -------------------------------------------------------------------------- | --------------------------------------------------------------------------------------- |
| Completion  | completion_menu exists, filtered is nonempty, overlay kind is Completion | filtered[selected].1, then items[item_index]                               | menu document/revision/query snapshot remains current; pending resolve may block accept |
| Hover       | hover_request/card plus Hover overlay                                    | no selectable collection                                                   | docs-only; response requires exact HoverRequest owner                                   |
| References  | reference_list exists with References overlay                            | sorted LocationItem vector                                                 | current document/revision/caret response guard passed before open                       |
| CodeActions | code_action_list exists with CodeActions overlay                         | preferred-first CodeActionItem vector                                      | code_action_origin document/revision remains nonstale at activation                     |
| ContextMenu | context_menu exists with ContextMenu overlay                             | selectable_items(items), excluding separators but retaining disabled items | captured region anchor; disabled activation is a no-op                                  |

CompletionMenuState itself has document_id, revision, query_start, query, items, filtered triples, is_incomplete, selection_changed, and pending_resolve. The triple's second value is the identity bridge from visual filtered position to stable items storage. HoverRequest is HoverAnchor plus Position plus Mouse/Keyboard origin; HoverAnchor captures document id, revision, editor id, caret, selection, and pixel-scroll bits. HoverCardState carries only accepted content plus optional line/column visual anchor.

Durable data is feature payload. Transient input is selection, scroll, pointer row, and docs viewport. Borrowed presentation is OverlaySpec, Row, and Section. Derived layout/cache is OverlayLayout, glyph plans, scrollbar geometry, and frame masks. Completion is visible only when the filtered list is nonempty and its overlay kind is Completion; a pending request owns neither keys nor screen space.

The projection boundary is concrete. view/modal constructs an OverlaySpec with Anchor, optional tabs/header/footer, a Body::List, pointer FlatIndex, and optional Documentation. Its rows borrow strings and precomputed vectors for exactly the closure call to with_cursor_overlay_spec; this is why callers cannot retain a spec in UiState. layout_measured consumes the spec, physical window size, scale factor, and TextMeasure and returns OverlayLayout with panel, row, docs viewport, and scrollbar rectangles. Both render and hit test must call that same composition path.

## Lifecycle, focus, and async ownership

| Event                            | Guard                                                | Transition/effect                                             |
| -------------------------------- | ---------------------------------------------------- | ------------------------------------------------------------- |
| Completion filter nonempty       | active plain-text session                            | install Completion; selected/scroll zero; schedule/refine LSP |
| Accepted hover reply             | document/revision/position/focus/request owner match | install Hover card and overlay                                |
| References/actions reply         | revision and captured caret match                    | build authoritative vector, install kind                      |
| Context trigger                  | builder returns items                                | capture click/caret anchor, install ContextMenu               |
| Up/Down/Page                     | selectable rows                                      | move selection and minimal-reveal scroll                      |
| Enter/click                      | row exists/enabled                                   | activate same indexed payload, clear overlay                  |
| Escape/outside press             | kind policy                                          | dismiss; context non-navigation keys dismiss and consume      |
| Ordinary typing while Completion | no claimed key                                       | editor receives key; completion refreshes/dismisses           |
| Edit/caret/focus/resize          | hover policy                                         | invalidate request/card/overlay                               |

Runtime routes popup keys before normal editor/keymap routing, but this is not modal capture: only documented navigation keys are claimed. Mouse code derives row and outside targets from the same OverlayLayout used for paint. Hover request ownership includes document/revision/position/origin; dismiss_hover clears request and card, so a late reply cannot reopen. Completion is revision guarded and asks runtime to cancel work when dismissed.

Input differs by kind and is intentionally not erased behind a generic reducer. Completion claims navigation/accept/dismiss while normal text still reaches the editor. References and code actions use the stored vector for Up/Down/Enter and dismiss-and-consume ordinary keys. Context-menu FlatIndex excludes separators only: disabled menu rows retain their index for rendering, pointer hover, and navigation; activation of such a row is a no-op rather than an accidental index shift. Hover has no selectable rows; its text reading controls are handled through has_documentation. Pointer hover changes hover_row only: it must never overwrite keyboard selected, even if mouse and keyboard refer to different rows. Pointer release/capture cancellation clears scrollbar drag through the shared UI drag owner.

| kind         | focus ownership                                         | dismissal triggers that are implemented/policy-specific                                                                                                                                             | capture behavior                                                               |
| ------------ | ------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------ |
| Completion   | editor remains focused; popup pre-routes dedicated keys | explicit dismiss, accept, editor edit/session invalidation; ordinary typing passes through                                                                                                          | docs-thumb capture only when docs visible; release/focus/resize ends it        |
| Hover        | editor remains focused                                  | hover reconcile sees changed HoverAnchor, competing overlay, mouse-disabled/signature state; runtime also dismisses on keypress except docs controls, focus loss, resize, and pointer leaving grace | docs scrollbar uses shared capture; hover grace is timing, not pointer capture |
| References   | editor remains focused                                  | activation, Escape, and non-navigation key dismissal/consume                                                                                                                                        | no docs capture; row click uses layout hit result                              |
| Code Actions | editor remains focused                                  | activation, Escape/non-navigation routing and stale activation rejection                                                                                                                            | no docs capture; activation clears list/origin before applying edit            |
| Context Menu | editor remains focused; no modal focus                  | Escape and non-navigation dismiss/consume; outside press follows menu pointer routing                                                                                                               | no scrolling in v1; captured physical open anchor, not pointer capture         |

No row popup transfers FocusTarget to Modal. A focused window loss is a runtime boundary for hover/documentation; consumers must not infer an unimplemented global “all cursor overlays close on blur” rule from Hover's stronger behavior.

## Geometry, index mapping, clipping

Anchor::Cursor and Anchor::Menu receive physical x,y,h. WidthRule values are logical pixels: resolve window_w×pct, clamp to logical min/max and the scaled 32px edge margin, then apply the scaled 200px cursor floor without exceeding the window. Let gap=round(2×scale), panel=pw×ph:

```
px = min(x, window_w - pw)
below = y + h + gap
fits_below = below + ph <= window_h
fits_above = y >= ph + gap
py = below if prefer_below && fits_below
   = y - gap - ph if fits_above
   = below if fits_below
   = window_h - ph otherwise
```

Completion is below-preferred, 240–320 logical px, maximum eight visible rows. Hover is above-preferred and 42% window width clamped to 360–560 logical px. Menus measure content before width finalization. UiTree/Frame clip an overlarge pinned panel.

Display positions include headers and separators; FlatIndex includes selectable rows only. resolve_scroll_for_selection maps flat to display, minimally reveals there, then returns flat scroll. This prevents header-induced click/Enter/render drift.

Trace: at 1× anchor=(950,200,20), window=1000×250, panel=300×100 gives px=700; below 222 fails; py=98 above. For sections Recent rows 0–2 then All rows 3–5, flat positions map to display [1,2,3,5,6,7]; a three-slot window selecting flat 3 starts at display 5, not raw display 3.

At scale 2×, a logical 240px completion minimum is 480 physical px and the 2px logical gap is 4 physical px. In a 450px window, resolve_width applies the cursor floor but its final min(window_width) produces 450px, then px=0. This degradation is expected: horizontal panel clipping is preferable to unsigned underflow or off-window geometry.

### Selection and scrolling algorithm

The following is an **algorithm sketch** of the invariant that existing update helpers implement. SectionShape is the current shape-only dependency used by update code: has_title denotes a titled header, len is the number of selectable rows in that section. The layout helper also inserts a display separator before every non-first untitled section; a caller must represent that boundary consistently rather than treating all untitled sections as one list. FlatIndex is a newtype over a selectable-row ordinal, so display slots are never passed to activation.

```rust
fn move_popup_selection(
    selected: FlatIndex,
    scroll: usize,
    selectable_count: usize,
    delta: isize,
    shapes: &[SectionShape],
    max_visible: usize,
) -> (FlatIndex, usize) {
    if selectable_count == 0 || max_visible == 0 {
        return (FlatIndex(0), 0);
    }
    let next = selected.0
        .saturating_add_signed(delta)
        .min(selectable_count - 1);
    let next_scroll =
        resolve_scroll_for_selection(shapes, next, max_visible, scroll);
    (FlatIndex(next), next_scroll)
}
```

The real list navigation may wrap for a particular popup kind; a caller must state that policy rather than silently applying it. max_visible must be nonzero before reveal math; the current callers use fixed caps such as eight or ten. An empty list returns FlatIndex(0),0 only as a harmless stored coordinate: it does not make row zero valid. A filter replacement must preserve selected identity only if it can map that identity into the new ordering authority; otherwise reset to zero and discard docs/hover that named the removed item.

Conceptually, section_positions first produces flat_to_display. For each shape at section ordinal i, increment display by one if has_title or i is nonzero (the latter is the separator), then append the current display position once for every selectable row and increment it. resolve_scroll_for_selection looks up flat_to_display[selected], computes reveal in display space, then uses a partition search back to a FlatIndex-space scroll. This two-way mapping is the missing dependency in any algorithm that claims headers are harmless.

### Hit-test consistency contract

For each frame, view/modal creates rows in feature-vector order, layout_measured snaps their rectangles once, and hit testing returns a FlatIndex from that layout. Pointer action then indexes the original feature vector by that returned index. The forbidden implementation is:

```
paint: sorted(filtered_items)
click: original_items[index]
Enter: filtered_items[selected]
```

Those three arrays agree only by accident. Separators make the failure visible sooner because display row 3 can have no selectable payload. The actual design uses flattened selectable rows for FlatIndex and keeps section headings only in the display layout.

## Integration, invalidation, tests

```
feature or LSP event -> update validates owner, changes UiState -> Cmd::Redraw
 -> view::modal::with_cursor_overlay_spec borrows payload
 -> overlay_surface::layout_measured creates OverlayLayout
 -> renderer and hit_test_cursor_overlay consume same layout
 -> pointer/key result -> feature message/update
```

Production paths use glyph-backed layout_measured; monospace layout is fallback/test-only. Invalidate on payload/order, selection/scroll/docs state, caret/dwell anchor, window/viewport, scale, theme/font, and side space. Projection is O(rows); layout/paint is O(visible rows plus measured docs glyphs).

**Existing tests** cover flip/clamp/spec in [view/modal.rs](../../src/view/modal.rs), drag/click/wheel in [runtime/mouse.rs](../../src/runtime/mouse.rs), and stale LSP paths in [update/lsp.rs](../../src/update/lsp.rs). **Proposed regressions:** numeric trace at 1×/2×; context menu keeps captured anchor after caret mutation; filtering sectioned rows preserves painted/clicked/Enter identity; dismiss each async kind then deliver old reply and assert nothing reappears.
