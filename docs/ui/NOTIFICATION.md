# Notification

## Purpose and taxonomy

A **notification** reports an event or changed state without becoming a hidden
transport for required decisions. IntelliJ explicitly separates balloons,
alerts, and context-bound banners ([UI overview](https://plugins.jetbrains.com/docs/intellij/ui-overview.html)).
Token must preserve that distinction:

| Name                   | Intended use                                                       | Token status                                            |
| ---------------------- | ------------------------------------------------------------------ | ------------------------------------------------------- |
| status message         | brief, low-priority local feedback in bottom bar                   | implemented                                             |
| banner                 | non-blocking, actionable state tied to one editor/tab/panel/dialog | not implemented                                         |
| balloon / toast        | transient unanchored event feedback                                | not implemented                                         |
| alert / modal decision | crucial confirmation or block before proceeding                    | implemented as specific dialogs, not a notification API |

“Toast” is Token vocabulary for an unanchored balloon-style transient; it is
not an existing UI component. IntelliJ calls its equivalent a notification
balloon; its banners must be tied to a context, while a main-window balloon is
appropriate when there is none ([Banner](https://plugins.jetbrains.com/docs/intellij/banner.html),
[Notification balloons](https://plugins.jetbrains.com/docs/intellij/notification-balloons.html)).

## Current implementation — high confidence

`UiMsg::SetTransientMessage` installs `TransientMessage { text, expires_at }`
and mirrors it to the status bar `StatusMessage` segment. `App` schedules a
wake at expiration; update clears it and draws status-bar damage. The current
common paths use brief messages such as save/LSP/inline feedback. This is not
a toast: it has no floating card, severity/icon/action, queue, hover pause,
dismiss button, history, banner placement or notification center.

| Current field/event                                    | Writer                                             | Scope/lifecycle                                                 |
| ------------------------------------------------------ | -------------------------------------------------- | --------------------------------------------------------------- |
| `UiState::transient_message: Option<TransientMessage>` | `UiMsg::SetTransientMessage` via update            | one plain-text status flash, deadline held by runtime scheduler |
| `TransientMessage::{text, expires_at}`                 | `UiState::set_status_for` / direct feature updates | expired message is removed; no severity/actions/queue           |
| `StatusBar::StatusMessage` segment                     | explicit update and transient mirror               | display slot; diagnostic fallback is lower priority             |
| `UiMsg::ClearTransientMessage`                         | update                                             | clears transient ownership and status message content           |

Explicit `UiMsg::UpdateSegment` can independently set status-bar content.
Diagnostics under the cursor provide a fallback only when no transient/explicit
message owns that segment. See [model status](../../src/model/status_bar.rs),
[UI update](../../src/update/ui.rs), [runtime scheduling](../../src/runtime/app.rs),
and [status renderer](../../src/view/mod.rs).

| Transition (implemented status feedback) | Owner                  | Result                                                         |
| ---------------------------------------- | ---------------------- | -------------------------------------------------------------- |
| `SetTransientMessage`                    | update/UI state        | install expiry and write `StatusMessage`                       |
| deadline wake                            | runtime then update    | clear transient and status slot unless a newer message owns it |
| `UpdateSegment(StatusMessage)`           | explicit caller/update | prevent diagnostic fallback from overwriting caller text       |
| diagnostic cursor change                 | `sync_status_bar`      | write fallback only while segment is empty/diagnostic-owned    |

## Proposed contracts

Keep status feedback on its current path. If a concrete need appears, choose
exactly one separate model:

```text
BannerState { scope: EditorTab | Panel(PanelId) | Dialog(ModalId), severity, text, action?, dismissible }
ToastState  { id, severity, title?, text, action?, expiry, dismissal }
Alert       = existing specific ModalState with an explicit confirm/cancel effect
```

| Proposed event         | Owner                               | Constraint                                                                     |
| ---------------------- | ----------------------------------- | ------------------------------------------------------------------------------ |
| enqueue/coalesce toast | notification update                 | bounded policy; must not replace a required alert                              |
| timeout/dismiss/action | notification update + runtime timer | id-correlated; action emits normal message; late timer cannot close newer item |
| show/hide banner       | scoped feature update               | scope key must still resolve to live editor/panel/dialog geometry              |
| alert confirm/cancel   | existing modal feature update       | no auto-expiry or outside-loss semantics                                       |

The banner’s scope must be live layout identity and it must disappear when the
scope/document does; never use it for a global event. A toast needs bounded
queue/coalescing, severity policy, animation/timer ownership, user dismissal,
and keyboard/screen-reader announcement before implementation. Alerts remain
specific modal workflows and never auto-expire. The state model owns no I/O;
update emits commands/actions.

### Presentation/accessibility

Current status messages are plain text and do not expose severity. Proposed
banner/toast tokens need information/warning/error/success roles, readable
contrast, UI font, icon label, non-color status cue, short sentence-case text,
focusable action/dismiss affordance, and an announcement policy. Notification
headers/body use sentence capitalization in IntelliJ guidance
([Capitalization](https://plugins.jetbrains.com/docs/intellij/capitalization.html)).

## Gallery, gaps, acceptance

No notification specimen exists. Status bar is exercised only through the
application renderer, not a gallery state. Do not add a visually attractive
toast merely for the gallery. A concrete banner needs scoped editor/panel and
dialog cases; a toast needs success/error, stacked/coalesced, action, expiry,
manual dismiss, small-window, theme/scale and announcement states.

Acceptance: no banner outside its scope; no silent loss of required error;
timeout wake clears current transient feedback; actions are deterministic
messages; toast and banner lifecycle cannot outlive their model target; modal
alerts cannot be mistaken for dismissible notifications.

## Evidence

- [Transient model/status segments](../../src/model/status_bar.rs), [UI messages](../../src/messages.rs), [update](../../src/update/ui.rs), [runtime wake](../../src/runtime/app.rs)
- [Local IntelliJ SDK notification/status reference](../../temporary-docs/intellij-platform-sdk/references/ui-settings-and-toolwindows.md) (secondary)
- [IntelliJ UI overview](https://plugins.jetbrains.com/docs/intellij/ui-overview.html), [banners](https://plugins.jetbrains.com/docs/intellij/banner.html), [notification balloons](https://plugins.jetbrains.com/docs/intellij/notification-balloons.html)
