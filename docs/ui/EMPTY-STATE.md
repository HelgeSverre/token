# Empty states: feature-owned passive projections

An empty state explains why a particular surface has no rows. It is not a shared Token model, a loading spinner, an error notification, or an editor containing zero bytes. Token renders independent empty projections; no EmptyState struct, generic action, or common input machine exists.

## Current data sources and ownership

| Surface                | Current condition/data authority                                             | Exact passive projection                    |
| ---------------------- | ---------------------------------------------------------------------------- | ------------------------------------------- |
| Problems               | problems_rows(model) is empty; scope is ProblemsPanelState.current_file_only | “No problems in this file” or “No problems” |
| Outline                | focused document outline is None or empty                                    | “No outline available”                      |
| Usages                 | UsagesPanelState items/status/query                                          | summary-row label from domain status        |
| placeholders           | PlaceholderPanel panel ID                                                    | fixed provisional text                      |
| binary/special editors | BinaryPlaceholderLayout/view-mode data                                       | separate editor presentation                |

```rust
// current excerpt — src/update/problems.rs
pub fn problems_empty_text(model: &AppModel) -> &'static str {
    if model.problems_panel.current_file_only {
        "No problems in this file"
    } else { "No problems" }
}
```

Problems’ condition is derived, not stored. problems_rows traverses diagnostic groups for current-file/workspace scope, observes collapsed paths, and returns no rows only when scoped groups are absent. The text is borrowed static presentation: no ID, expiry, action, selection, or retained geometry.

Outline is a direct render branch: it queries focused document outline and when absent/empty measures and centers a literal. Focused-document or parsed-outline changes invalidate it; renderer does not remember an outline or emit a request. Usages is durable feature data:

```rust
// current excerpt — src/model/usages.rs
pub struct UsagesPanelState {
    pub items: Vec<LocationItem>, pub selected_index: Option<usize>,
    pub scroll_offset: usize, pub collapsed: HashSet<PathBuf>,
    pub source: String, pub status: String,
    pub(crate) query: Option<UsagesQuery>,
}
```

items is result collection; selection indexes the projection returned by rows(), not items directly. scroll_offset is row units; collapsed is keyed by file path; query carries Arc token, document ID, revision. Its initial/searching/no-result wording is summary-row content, not generic state.

## Projection and renderer geometry

Empty renderers are passive: no pointer capture, button hit test, focus target, keyboard event, timer, or cancellation exists. Owners invalidate panel after source state changes; renderer takes current content rect/text painter:

```text
text_width_px = painter.measure_width(message)
text_x = content.x + (content.width - text_width_px) / 2
text_y = content.y + (content.height - painter.line_height()) / 2
```

Coordinates are physical floating layout px until painter casts x/y to usize. In Rust, a negative float cast to usize saturates to zero; it does not wrap. Rect is UiKey::PanelContent(id), created by chrome and clipped by DockPaneScene. Theme foreground is sidebar text color. No current branch has padding, wrapping, or an intentional narrow-layout policy.

Normal trace: Problems R=(100,80,400,240), text width 96, line height 20 gives x=252, y=190. Pathological: Outline R=(0,0,40,10), text width 150, line 20 computes x=-55, y=-5, then current casts yield (0,0); the oversize text is painted from content origin and clipped, not wrapped. A shared painter must truncate/wrap before placement, not rely on dock minimum.

A nonempty row surface uses same PanelRows declaration for layout, paint and input. Empty Problems has row count zero, so row hit mapping cannot select stale rows. Domain reducers repair selection when data/collapse removes a row; empty painter cannot repair it.

## Consumer lifecycle and async staleness

```text
diagnostics publish/focus/scope/collapse → Problems projection → rows OR text
syntax outline/focused document change   → current outline     → rows OR text
Usages start → clear items/selection/scroll; query=token; status=Searching
reply/reconcile → matching token + document revision → items/status projection
```

The Usages reducer's resolve compares Arc token identity with current query, then
verifies that the **response** document ID still exists and its **response**
revision is current. It does not compare response document/revision back to the
stored query fields. Source change/close clears query and says run again. Late
reply cannot turn a newer token/closed or changed response document into stale
rows. Empty presentation owns no async work/cancellation.

## Proposed reusable view data

Extract only a passive projection when consumers converge, supplied by feature rather than generic reducer:

```rust
// proposed API — presentation only; not implemented.
enum EmptyScope {
    Panel(PanelId),
    EditorView { group: GroupId, document: DocumentId },
}
struct EmptyActionId(u64);
struct RequestId(u64);
enum EmptyActionRoute { Retry { owner: EmptyOwner, request: RequestId } }
enum EmptyOwner { Problems, Outline, Usages }
struct EmptyInputState { focused_action: Option<EmptyActionId>, retry_pending: bool }
enum EmptyReason {
    NoData,
    FilteredOut { filter: String },
    Unavailable { detail: String },
    Error { detail: String },
}
struct EmptyProjection<'a> {
    scope: EmptyScope, title: &'a str, detail: Option<&'a str>,
    reason: EmptyReason, action: Option<EmptyAction>, input: Option<&'a EmptyInputState>,
}
struct EmptyAction { id: EmptyActionId, label: String, route: EmptyActionRoute }
enum EmptyMsg { Activate { scope: EmptyScope, action: EmptyActionId } }
enum Msg { /* existing variants */, Empty(EmptyMsg) } // proposed app-message wrapper
enum EmptyEffect { Redraw, Retry { owner: EmptyOwner, request: RequestId } }
```

EditorView identifies both group and document because one DocumentId may appear in
multiple splits. Scope establishes current clip rect and never caches a
rectangle. Reason distinguishes no-data, filtered-out, unavailable. Painter
receives projection each frame and owns neither collection nor retry. This
proposed action is a real contract: layout returns an action rect only when
`retry_pending` is false; hit test and Tab focus use that exact rect; Enter or
pointer release emits `Msg::Empty(EmptyMsg::Activate { .. })`; the proposed
`update_empty` reducer checks scope/action, sets retry_pending, and returns
`EmptyEffect::Retry`. Runtime routes that effect to the named feature request.
Completion carries `RequestId` and only the owning feature clears
retry_pending. Focus is cleared when the projection/action disappears. Until
those pieces exist, current empty states remain passive.

Safe proposed layout: inner=inset(content,p) after nonnegative clamp, wrap detail at inner width, clamp origin to content. At p=12 and R=(0,0,40,10), inner width is 16; truncate/clip rather than generate negative coordinates.

## Invalidation, cost, and tests

Decision cost follows sources: Problems traversal O(files plus diagnostics in scope); Outline availability O(1) over parsed reference; Usages rows O(items/groups). Text adds glyph measure/draw. Inputs: data/filter/query/collapse, focused document, theme/font/scale, content rect/dock/window. No cached empty rectangle.

| Initial state                                | Action         | Expected result                          |
| -------------------------------------------- | -------------- | ---------------------------------------- |
| current-file Problems; no diagnostics        | render         | “No problems in this file”; row count 0  |
| workspace Problems; no diagnostics           | scope/render   | “No problems”                            |
| Outline R=(100,80,400,240), text 150/line 20 | render         | x=225, y=190, clipped                    |
| Usages query A then B                        | late A resolve | token guard ignores A; B stays searching |
| Usages source revision changes               | reconcile      | query cleared; source-changed status     |
| proposed R=(0,0,40,10), p=12                 | layout         | width 16; clipped/truncated block        |

Gallery panel.bottom-empty and panel.right-empty cover static chrome only. Add filtered/error/action/narrow/scale/theme/stale-retry coverage only with a shared/actionable implementation.

## Evidence

- [Problems derivation](../../src/update/problems.rs), [render branches](../../src/view/panels.rs), [chrome rows](../../src/layout/chrome.rs)
- [Usages data](../../src/model/usages.rs), [async reducer](../../src/update/usages.rs), [placeholder source](../../src/panels/placeholder.rs)
