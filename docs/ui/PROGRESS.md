# Progress

## Purpose and current boundary

A **progress indicator** communicates an operation that is still running. It
needs a concrete scope, state transition and cancellation/error story; it is not
synonymous with “we wrote a status message.” IntelliJ’s component catalog
separates loader, progress bar and progress text
([Components](https://plugins.jetbrains.com/docs/intellij/components.html)).

**Token has no reusable progress component or `ProgressState`.** It does have
feature-local lifecycle state: `UiState::{is_loading,is_saving}`, LSP server
states such as starting/indexing, an inline request status segment, asynchronous
find status, usages loading/status, and transient messages. These are distinct
model fields and some produce text in status bar/panels; none establish a shared
spinner, determinate bar, cancellation widget, progress percent, queue, or
progress gallery specimen. This is intentional documentation of absence, not a
request to implement one.

## Existing consumers and behavior — high confidence

| Scope                    | Current visual/data path                                        | What is not supplied           |
| ------------------------ | --------------------------------------------------------------- | ------------------------------ |
| file load/save           | `UiState` booleans, file I/O/runtime and status synchronization | determinate progress/cancel UI |
| LSP                      | server state + transient/status feedback                        | common progress surface        |
| inline completion        | `InlineSuggestion` status-bar segment while request is active   | spinner/percent                |
| find                     | `FindStatus::Searching`/result state in find bar                | shared loader                  |
| usages                   | `UsagesPanelState` loading/status and panel rows                | determinate panel bar          |
| external background work | commands/runtime completion messages                            | common task manager            |

| Current state field/identity      | Lifecycle owner                  | Visibility rule                                                          |
| --------------------------------- | -------------------------------- | ------------------------------------------------------------------------ |
| `UiState::{is_loading,is_saving}` | file I/O/update/runtime          | file lifecycle signal, not a determinate widget model                    |
| LSP server/request records        | LSP update/runtime correlation   | server text/status is feature-derived; request guards drop stale replies |
| `FindStatus` and find request     | find model/update worker         | find-bar-local searching/result rendering                                |
| `UsagesPanelState` status/loading | usages update/LSP request        | panel-local state drives rows/status                                     |
| inline request/in-flight state    | inline completion update/runtime | derives `InlineSuggestion` status segment only while relevant            |

All current async paths should retain their feature-owned request identity and
stale-result guards. A display component must never become the authority for
whether background I/O continues or may be cancelled.

The implemented state machine is therefore fragmented by design: a feature
starts work, records pending identity/status in its own model, receives a
correlated runtime result, then either installs content or drops a stale result.
The shared status bar merely renders derived text. There is no common
`Start → Tick → Complete` transition table to document as implemented.

## Proposed contract

Only introduce a shared projection when one operation has a real consumer and
its scope is clear:

```text
ProgressState { scope, phase, label, value: Indeterminate | Fraction(0..=1), cancellable }
ProgressEvent = Start | Update | Complete | Fail | Cancel
```

| Proposed event    | Feature owner                                         | View constraint                                                                 |
| ----------------- | ----------------------------------------------------- | ------------------------------------------------------------------------------- |
| `Start`           | creates correlation id and scoped progress projection | show only in declared scope; no global default                                  |
| `Update`          | validates current id/value/phase                      | clamp fraction; ignore stale/nonmonotonic policy violations explicitly          |
| `Complete`/`Fail` | consumes matching operation result                    | remove/projection-transition once; report outcome through chosen scoped surface |
| `Cancel`          | feature issues cancellable command                    | disabled/pending state is truthful; completion race cannot revive progress      |

`scope` must select placement: inline/local (find or panel row), scoped banner
(operation affects that editor/panel), or low-priority status text. A modal
progress dialog is reserved for an operation that truly cannot safely continue
without the user waiting; ordinary background work must not block editing. The
feature state owns correlation ID, completion/failure, and cancellation command;
the shared view receives a snapshot only. Do not invent global activity because
there are many unrelated async operations.

### Interaction, visual and accessibility requirements

Indeterminate progress must say what is running; determinate progress must
avoid false precision. Cancel is shown only when it is meaningful, routes to a
feature-specific message, remains disabled/changes label while cancellation is
pending, and never implies an operation was undone. Use theme semantic roles,
UI font, scaled geometry and non-color text. A future accessibility bridge must
announce start, meaningful changes, completion/failure and cancel result without
spamming every frame. Exact animation, sizes and colors are deliberately
unimplemented.

## Gallery, gaps, acceptance

No progress gallery state exists. Any first implementation needs determinate,
indeterminate, queued/pending, cancellation, success/failure, narrow clipped
panel, status-bar overflow, light/dark and HiDPI specimens. Acceptance: renderer
does no I/O; stale completion cannot update a later operation; percent clamps
and monotonic rules are explicit; cancellation behavior is testable; and no
decorative loader obscures editor interaction.

## Evidence

- [UI loading/saving and LSP state](../../src/model/ui.rs), [find state](../../src/model/ui.rs), [usages model](../../src/model/usages.rs)
- [status synchronization](../../src/model/status_bar.rs), [messages/commands](../../src/messages.rs), [runtime](../../src/runtime/app.rs)
- [IntelliJ component catalog](https://plugins.jetbrains.com/docs/intellij/components.html)
