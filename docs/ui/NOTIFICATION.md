# Notifications: current boundary and implementable contract

Token currently has **no notification component**: no toast, banner, queue,
history, severity, action button, or notification center state. The only nearby
implemented mechanism is the one-slot transient status flash in STATUS-BAR.md.
Calling that flash a toast would invent input, geometry, and lifecycle behavior
that source does not contain.

## Current representation: a passive status projection

```rust
// current excerpt — src/model/status_bar.rs
pub struct TransientMessage {
    pub text: String,       // plain display text; no severity or action
    pub expires_at: Instant // monotonic deadline; no request/notification ID
}
// UiState::transient_message: Option<TransientMessage>
```

SetTransientMessage is the canonical path: it stores one record, writes
StatusMessage, and requests status-bar damage. Some direct feature writers
(for example inline failure) store only `transient_message` and request editor
damage; they suppress diagnostic fallback but do not mirror text into the slot.
The event loop includes the current deadline and expiry clears the record and
slot. UpdateSegment(StatusMessage) is an independent explicit write that leaves
an old transient deadline live, so that deadline can later clear its text.

| Concept                | Current state                            | Input/lifetime                                                                    | Geometry                       |
| ---------------------- | ---------------------------------------- | --------------------------------------------------------------------------------- | ------------------------------ |
| canonical status flash | Option<TransientMessage> + StatusMessage | producer installs/replaces; runtime wakes at current deadline; no user dismiss    | StatusBar left segment         |
| direct transient write | Option<TransientMessage> only            | suppresses diagnostic fallback until expiry; no slot mirror or status-only damage | whatever feature damage occurs |
| contextual banner      | absent                                   | no scope identity, action, close, or renderer                                     | absent                         |
| floating toast         | absent                                   | no IDs, queue/coalesce policy, timer generation, hover/focus, action              | absent                         |
| alert/decision         | feature-specific ModalState              | concrete modal reducers own confirm/cancel                                        | modal geometry                 |

It is therefore passive: no mouse/keyboard state machine exists. The owner
invalidates the status bar after a reducer or deadline and the status renderer
projects its text. Do not create fictional click/capture/cancel rules for the
current flash.

## Existing lifecycle and stale deadline behavior

```text
canonical producer → SetTransientMessage(text,duration)
                   → transient = { text, expires_at=now+duration }
                     StatusMessage = text; redraw status bar
direct writer      → transient only; e.g. redraw editor, no StatusMessage write
runtime  → next_wake includes current expires_at
deadline → expire_status_message()
         → only when now >= current expires_at: clear current record/slot
```

There is no callback carrying an old message ID. Replacing a canonical flash is
safe because expiry interrogates the present record: an early wake after a newer
longer flash sees now < new expiry and does nothing. This does **not** protect
an explicit StatusMessage update while the old transient still exists: expiry
will clear that new explicit text. A producer needing correlated completion
keeps request identity in its feature model; display text is not identity.

## Proposed notification model

Only implement this when a concrete consumer requires it. These full types are
proposed, not current Rust APIs. NotificationId is monotonic reducer-owned
identity; ScopeKey resolves live geometry and is never a cached rectangle.

```rust
// proposed API — dependencies are deliberately explicit.
struct NotificationId(u64);
struct ActionId(u64);
enum ScopeKey {
    GlobalWindow,
    EditorView { group: GroupId, document: DocumentId },
    Panel(PanelId),
    Modal(ModalId),
}
enum NoticeSeverity { Info, Success, Warning, Error }
enum NoticeLifetime { UntilDismissed, For(Duration) }
enum NoticeActionRoute { Dispatch(Msg) } // Msg must carry feature request identity
struct NoticeAction { id: ActionId, label: String, route: NoticeActionRoute }
struct NoticeDraft {
    scope: ScopeKey, severity: NoticeSeverity, title: Option<String>, body: String,
    action: Option<NoticeAction>, lifetime: NoticeLifetime, dismissible: bool,
    coalesce_key: Option<String>,
}
enum QueuePolicy {
    ReplaceSameScope,
    KeepNewest { capacity: std::num::NonZeroUsize },
    CoalesceByKey { capacity: std::num::NonZeroUsize },
}
struct Notice {
    id: NotificationId, scope: ScopeKey, severity: NoticeSeverity,
    title: Option<String>, body: String, action: Option<NoticeAction>,
    lifetime: NoticeLifetime, expires_at: Option<Instant>, dismissible: bool, created_at: Instant,
    coalesce_key: Option<String>,
}
enum NoticeTarget { Dismiss(NotificationId), Action { notice: NotificationId, action: ActionId } }
struct NotificationInputState { focused: Option<NoticeTarget> }
struct NotificationState {
    next_id: u64, active: VecDeque<Notice>,
    policy: QueuePolicy, dropped: u64, timer_generation: u64,
    input: NotificationInputState,
}
struct NoticeCardLayout {
    id: NotificationId, rect: Rect,
    action_rect: Option<Rect>, dismiss_rect: Option<Rect>,
}
struct NoticeLayout { cards: Vec<NoticeCardLayout>, hidden: Vec<NotificationId> }
enum NoticeMsg {
    Show(NoticeDraft),
    TimerFired { id: NotificationId, generation: u64 },
    Dismiss { id: NotificationId },
    InvokeAction { id: NotificationId, action: ActionId },
    ScopeRemoved(ScopeKey),
}
enum Msg { /* existing variants */, Notification(NoticeMsg) } // proposed wrapper
enum NotificationEffect { Redraw, WakeAt(Instant) }
```

EditorView scope deliberately identifies both split/group and document; a bare
DocumentId would ambiguously place a notice when the same document is visible in
multiple groups. Feature state owns operation request ID, result, cancellation and stale-reply
guards; NotificationState owns only notice identity/lifetime. Scope is validated
every frame: a panel notice does not keep a dock open; document/modal notices
are removed when their identity disappears. GlobalWindow is only for feedback
without a local surface. A serious error uses UntilDismissed or durable feature
error state, never a silently lossy bounded toast queue.

| Event         | Guard                                        | Reduction/effect                                                                                                                      |
| ------------- | -------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------- |
| Show          | body nonempty; scope resolves or is global   | allocate ID, apply stored QueuePolicy; insert/drop and increment dropped as policy requires; set deadline and schedule nearest expiry |
| timer         | ID/generation matches active expiring notice | remove exactly that notice; re-arm nearest deadline                                                                                   |
| dismiss       | record exists and dismissible                | remove ID; re-arm timer without touching feature work                                                                                 |
| invoke        | ID/action match current record               | apply retention rule, then dispatch its stored `NoticeActionRoute`                                                                    |
| scope removal | exact scope identity gone                    | remove active notices in scope; late timer becomes no-op                                                                              |

The state explicitly records active records, selected policy, and dropped count;
`NoticeLayout` separately records what physically fits this frame. Timer
generation plus ID prevents a stale deadline for an evicted/replaced toast
from removing a newer notice. Action messages must carry the feature's own
generation/request identity when they start/retry async work; notification ID
cannot authorize a late operation result.

The proposed policies have fixed reducer semantics. `ReplaceSameScope` removes
all active records with exactly matching scope, then appends the new record.
`KeepNewest { capacity }` appends it and, while length exceeds capacity, removes
the oldest active record and increments `dropped`. `CoalesceByKey { capacity }`
replaces an active record only when both nonempty keys match; otherwise it uses
KeepNewest. `For(duration)` sets `expires_at = now + duration` at Show, and all
active records—including cards that do not physically fit—continue to expire.
There is deliberately no hidden-card promotion: the next pure layout pass may
show a previously hidden active record after another leaves or geometry changes.

`layout_notices(active, window, measure)` is pure and processes newest-first
cards with their measured heights using the stack formula below. It appends a
full card/action/dismiss rectangle only while the next card fits above the top margin; all
remaining active IDs go in `hidden`. The renderer draws a compact “+N” summary
only if an implementation adds that summary to `NoticeLayout`; otherwise hidden
records are intentionally unannounced/expiring and the policy must not promise
delivery. This makes variable-height fit, timer behavior, and cache inputs
explicit rather than treating queue membership as visible geometry.

Proposed integration is `Msg::Notification(NoticeMsg) →
update_notification(&mut NotificationState, NoticeMsg) → Vec<NotificationEffect>`.
Runtime converts `WakeAt` to its next event-loop deadline and uses the same
`NoticeLayout` rectangles for hit testing,
Tab focus, and painting: a focused action/dismiss emits the wrapper's
`InvokeAction`/`Dismiss`; the reducer validates current ID/action before routing
the stored action. Focus is cleared when that ID leaves active/hidden layout.

## Proposed placement and math

A banner is scoped content, not a global card. Given validated scope content
rect R=(x,y,w,h), horizontal padding p, and measured/wrapped height b, top
placement is [x+p, y+p, max(0,w-2p), b], clipped to R. If w <= 2p, render no
action row and truncate/omit safely; never generate a negative width. Height is
measured after wrapping at max(0,w-2p-action_width-gap), rounded outward once
to physical px.

A toast stack belongs to GlobalWindow. With rect W, margin m, card heights hᵢ,
and gap g, newest-first bottom anchoring is y₀=W.bottom-m-h₀ and
yᵢ=yᵢ₋₁-g-hᵢ. Stop before y < W.top+m; overflow is coalesced only under an
explicit policy. Every card/hit target clips to W. This is proposed; current
flashes use status-bar placement.

Normal proposed trace: W=1200×800, m=16, g=8, heights 48 and 72: newest y=736,
older y=656. Pathological trace: W=320×90 with three 48px cards: first y=26,
second would be -30, so only one draws; policy preserves/merges the others
rather than painting off-screen.

## Invalidation, cost, and tests

Canonical `SetTransientMessage`/expiry invalidates DamageArea::StatusBar;
direct transient writers may invalidate only their feature (for example editor)
and do not update the status slot. Current cost is one optional record plus
fixed status measurement. Proposed notifications invalidate on active-list
mutation, expiry, scope geometry/window/scale/theme/font changes, and hover
only if pause-on-hover has actual state. Layout is O(visible notices + wrapped
text); timer scheduling scans expiring notices or uses a deadline heap.

| Initial state          | Action                                   | Expected result                                  |
| ---------------------- | ---------------------------------------- | ------------------------------------------------ |
| no transient           | set Saved, 3000 ms                       | one current flash and a status-bar wake deadline |
| Saved until t=3        | set Formatting, 5000 ms at t=2; wake t=3 | newer text remains; old wake cannot clear it     |
| proposed scoped notice | close Panel(Usages) before timer         | removed; later timer is no-op                    |
| proposed action notice | click after dismiss                      | no dispatch; ID/action guard fails               |
| proposed 320×90 stack  | enqueue three 48px notices               | first y=26; overflow policy is observable/tested |

## Evidence

- [transient/status data](../../src/model/status_bar.rs), [UI ownership](../../src/model/ui.rs), [messages/reducer](../../src/messages.rs) and [src/update/ui.rs](../../src/update/ui.rs)
- [runtime wake selection](../../src/runtime/app.rs), [status renderer](../../src/view/mod.rs)
