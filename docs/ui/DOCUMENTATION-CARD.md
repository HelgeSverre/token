# Documentation card — Token implementation reference

## Boundary and representation

Token displays readable documentation in two feature projections: a completion side card and an LSP hover Zones panel. They share documentation viewport mechanics, but content identity, async lifecycle, and anchor belong to Completion or Hover. There is no standalone card model that fetches, parses, pins, or owns focus.

```rust
// Current excerpt.
pub struct DocumentationState {
    pub scroll: usize,  // wrapped rendered-line index; not bytes or pixels
    pub expanded: bool,
}

pub struct DocumentationViewport {
    pub scroll: usize,
    pub visible: usize, // planned wrapped lines that fit
    pub total: usize,   // planned wrapped lines
}

impl DocumentationViewport {
    pub fn max_scroll(self) -> usize {
        self.total.saturating_sub(self.visible)
    }
}
```

DocumentationState lives in CursorOverlayState, so its lifetime is the surrounding Completion or Hover overlay. Completion content is CompletionMenuState data selected by overlay.selected. Hover content is HoverCardState with optional StyledText and optional text-cell anchor (line,column). StyledText/spans are content, not geometry. Renderer-owned derived data is the wrapped TextZonePlan, code plan, viewport, scrollbar geometry, and panel rectangles.

UiState.has_documentation is the event gate: it rejects an active modal, non-editor focus, non-documenting popup kinds, and a completion row without docs. Hover has a reading surface whenever its overlay is live, including diagnostics-only text. This is the reducer repair boundary: an old drag message must not change scroll after the owner vanished.

The two layouts use different panel relationships. Completion obtains selected StyledText through menu.selected_documentation(selected), which follows filtered[selected] to items[item_index], requires an LSP insertion payload, and rejects blank documentation, then passes it as OverlaySpec.docs beside Body::List. Hover combines live diagnostics at captured dwell/caret position with HoverCardState.content, then projects banner, leading fenced code, and prose into Body::Zones. Hover's card location is re-derived from stored text-cell identity when present; completion's anchor is re-derived from query_start. Stored identities are semantic positions; caret rectangles are derived physical geometry.

## Lifecycle, focus, scrolling, async results

| Event                        | Guard                                                     | State/effect                                                            |
| ---------------------------- | --------------------------------------------------------- | ----------------------------------------------------------------------- |
| Completion selection changes | selected index in current filtered order                  | reset docs to scroll=0, expanded=false; resolve selected item if needed |
| Completion resolve reply     | current session/document/revision/item owner matches      | attach docs to that item; redraw                                        |
| Hover reply                  | live hover request owns document/revision/position/anchor | install HoverCardState and Hover overlay                                |
| Wheel/thumb/page             | has_documentation and measured destination exists         | store clamped wrapped-line scroll                                       |
| Toggle expansion             | live documentation owner                                  | flip expanded; clear scrollbar drag; redraw/re-measure                  |
| Caret/edit/focus loss/close  | hover policy                                              | clear request/card/overlay; reject old reply                            |
| Modal opens/kind changes     | no docs owner                                             | input is inert; stale messages no-op                                    |

PageDocumentation deliberately does not calculate in update: only real glyph measurement knows current visible lines. Runtime handles Cmd::PageDocumentation by building the same measured cursor-overlay layout, reading its viewport, calculating viewport.scrolled(plus or minus visible), then dispatching UiMsg::DocumentationScrolled. Update stores an already-measured destination. This prevents PageDown wrapping differently from paint.

Hover dwell work is cancellation/revision guarded in [runtime/hover.rs](../../src/runtime/hover.rs) and LSP update. HoverResolved must match HoverRequest.anchor document id/revision and position; reconciliation additionally requires a freshly captured HoverAnchor equal to the request and rejects a competing overlay. It invalidates on movement, edit, focus loss, resize, and dismissal; pointer entry into a shown card gets grace time. Keyboard hover ignores incidental pointer movement.

Completion resolving is narrower than “A's reply can never enrich A.” CompletionItemResolved carries document id, revision, selected filtered-row ordinal, detail, documentation, and additional edits. merge_resolved_item requires a live completion menu whose document_id and revision equal the reply and whose current filtered vector still has selected; it then follows filtered[selected] into items and marks that LSP item resolved. It does **not** itself compare query text or resolve purpose. A valid late resolve can enrich the old underlying item if it still belongs to that valid menu; the side card is always projected from the _currently selected_ filtered row. Selecting B resets DocumentationState, so A cannot become B's card. Deferred acceptance is a separate post-merge path: only pending_resolve == Some(selected) proceeds to accept; otherwise the docs-purpose reply only redraws.

Scrollbar drag is a capture state outside DocumentationState because it represents transient physical pointer input. The drag target identifies which overlay viewport is being dragged; release, focus loss, resize, modal change, and expansion terminate it. Keeping capture out of content state prevents a new completion selection from inheriting an old thumb position. Wheel and page operations change only documentation.scroll, never list selection or CursorOverlayState.scroll.

## Measurement, placement, clipping

Completion uses OverlaySpec.docs beside its list. Preferred docs width is 360 logical px, scaled and limited by the larger side space around the primary panel. If resulting width is zero, or post-padding content width is nonpositive, plan_docs returns its default plan: no card, viewport, or scrollbar is projected. This is suppression, not a 0px clipped card. Hover uses Body::Zones: optional diagnostic banner, optional leading code block, then prose in the cursor-anchored parent. plan_docs and plan_zones wrap using PainterMeasure; render and hit testing reuse these plans.

Let T be total planned wrapped lines, v visible lines, s current scroll:

```
max_scroll = max(0, T - v)
s_next = min(max_scroll, max(0, s + delta_lines))
```

Scrollbar geometry derives from exactly T, v, s and drag emits a destination line index. Current documentation does not retain arbitrary per-line heights or translate one continuous document by a free y offset. It wraps measured glyph runs into StyledLine vectors, slices code/prose vectors to the [scroll, scroll+visible) row window, and computes visible sub-heights with scaled ZONE_LINE_H. Frame clips rounded panel chrome; reading viewport separately clips text/code so no line paints over chrome or scrollbar.

Normal trace: T=42, v=10, s=0. PageDown yields min(32,0+10)=10; dragging to 100 yields 32. Toggle expanded clears drag capture before remeasurement; if v becomes 28, the old 32 clamps to 14.

Pathological trace: completion panel is x=500..800 in a 900px window: left side is 500 and right is 100, so docs width=min(360,500)=360. At a 300px window the primary popup can consume all horizontal space, giving side space 0; plan_docs returns default and no documentation card is rendered or hit-tested.

For a text plan whose first code block occupies 3 wrapped lines and prose occupies 39, total=42 includes both areas in reading order. With a 10-line viewport and scroll 8, indices 8 through 17 slice from prose; code is absent and has no independent offset. Glyph measurement decides wrap points, but every resulting row uses scaled ZONE_LINE_H. The 17 logical-px row metric is therefore current layout behavior, not merely a painterless fallback.

### Scroll reducer algorithm

This is an **algorithm sketch** for the state-only half of scrolling. The runtime must obtain total and visible from the measured layout first; update must not rewrap StyledText to discover them.

```rust
fn set_documentation_scroll(
    state: &mut DocumentationState,
    requested_line: usize,
) {
    // Current update_ui trusts a destination measured by runtime.
    state.scroll = requested_line;
}

fn toggle_documentation(
    state: &mut DocumentationState,
    scrollbar_drag: &mut Option<ScrollbarDragState>,
) {
    *scrollbar_drag = None; // old thumb geometry is now invalid
    state.expanded = !state.expanded;
}
```

Current update_ui deliberately does not own a viewport or clamp: it stores the runtime-supplied measured destination after has_documentation succeeds. window_documentation clamps the value for the current plan with min(state.scroll, total-visible), and the next Page/wheel/thumb destination normalizes persistent state. A direct synthetic DocumentationScrolled message can therefore temporarily store a large value; tests should distinguish that current trust boundary from a proposed defensive clamp. scroll remains a line-index coordinate even at HiDPI; scrollbar pointer arithmetic is physical pixels only until converted to this destination.

### Content identity matrix

| Consumer       | content identity                                 | anchor identity           | late result must match                            | dismissal owner      |
| -------------- | ------------------------------------------------ | ------------------------- | ------------------------------------------------- | -------------------- |
| Completion     | selected menu item in current completion session | query-start text position | document, revision, session/item resolve identity | completion update    |
| Mouse hover    | hover request at text position                   | stored line,column cell   | document, revision, position, request/origin      | hover runtime/update |
| Keyboard hover | hover request at active caret                    | live caret rect           | document, revision, caret request                 | hover runtime/update |

This matrix is why DocumentationState alone cannot be the API boundary: the same scroll value has no authority to decide whether content or an async response belongs to it.

## Integration, invalidation, verification

```
LSP reply -> feature update validates owner -> UiState + Cmd::Redraw
 -> view::modal builds borrowed OverlaySpec
 -> overlay_surface::layout_measured makes plans/viewport/scrollbar
 -> render and hit test consume one OverlayLayout
 -> wheel/drag/page -> UiMsg::DocumentationScrolled
```

Invalidate on text/spans, completion selection identity, scroll/expanded value, diagnostics at hover position, caret/dwell anchor, window/scale/font/theme, and parent width/side space. Wrapping is O(glyphs plus planned lines); visible paint is O(visible glyphs). Renderer neither fetches nor mutates StyledText.

**Existing tests** cover docs drag/wheel/page/toggle/capture in [runtime/mouse.rs](../../src/runtime/mouse.rs) and hover ownership in [update/lsp.rs](../../src/update/lsp.rs). **Proposed tests:**

| Setup                                    | Action                                     | Expected                                             |
| ---------------------------------------- | ------------------------------------------ | ---------------------------------------------------- |
| T=42, v=10, s=0                          | PageDown                                   | reducer stores 10                                    |
| T=42, v=10, s=32                         | runtime thumb mapping produces destination | mapped destination is 32 before UiMsg is sent        |
| direct UiMsg::DocumentationScrolled(100) | reducer then render                        | state stores 100; projection clamps to max scroll 32 |
| scrolled docs A                          | select B, then reply A                     | B remains selected; viewport reset                   |
| expansion while dragged                  | Toggle                                     | capture None before geometry changes                 |
| dwell hover bottom-right                 | move caret, inject old reply               | card cannot reopen                                   |
| narrow 2× window                         | docs scrollbar hit test                    | snapped viewport matches paint                       |

## Proposed extraction boundary

Do not extract a DocumentationCard until a third real consumer shares semantics. A future reusable projection may contain only borrowed StyledText plus DocumentationState; feature code must retain anchor, async identity, request cancellation, and dismissal policy. Pinning, copy/select/link activation, loading/error UI, semantic reading order, and rich markdown remain unimplemented product decisions.
