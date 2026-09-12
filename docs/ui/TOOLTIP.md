# Tooltip

## Purpose and current boundary

A **tooltip** is brief, nonessential explanatory text for a target; it must not
contain a workflow, action list, or long documentation. IntelliJ’s component
guidance distinguishes a tooltip and a “Got It” teaching tooltip from popups
and dialogs ([Components](https://plugins.jetbrains.com/docs/intellij/components.html)).

**Token has no implemented generic tooltip component.** `HoverRegion` only
records what the pointer is over for cursor choice, visual hover, and scroll
routing; it is not tooltip state. The LSP hover card is a documentation popup,
not a tooltip. The status bar shows text but has no hover-description protocol.
This distinction is high-confidence from [`model/ui.rs`](../../src/model/ui.rs),
[`runtime/hover.rs`](../../src/runtime/hover.rs), and the absence of a tooltip
renderer/model/gallery specimen.

| Current related field                    | Actual owner                       | Why it is not tooltip state                                                            |
| ---------------------------------------- | ---------------------------------- | -------------------------------------------------------------------------------------- |
| `UiState::hover: HoverRegion`            | runtime hit/hover routing          | identifies scroll/cursor/visual region only; carries no text, target lifetime or timer |
| hover dwell/request/card fields          | LSP hover feature                  | operate on editor text/LSP documentation, not control descriptions                     |
| `modal_hover_*` / gallery hover previews | their specific input/fixture state | no reusable target id, delay or explanatory content                                    |

There is no implemented tooltip event transition to reuse. Delayed hover,
screen-reader description, and focus tooltip behavior are all **proposed**.

## Proposed Token contract

Add one only for a concrete unlabeled/icon-only consumer after confirming that
the label cannot be always visible. It should be input-neutral and owned by UI
state, never by a painter:

```text
TooltipState { target: TooltipTarget, text, opened_at, placement }
TooltipEvent = PointerEntered(target) | PointerLeft(target) | DelayElapsed | Dismiss
```

| Proposed field/event        | Ownership rule                                            | Required transition                                                            |
| --------------------------- | --------------------------------------------------------- | ------------------------------------------------------------------------------ |
| `target`                    | stable `UiKey`/control identity, never copied coordinates | target disappears/changes → immediate dismiss                                  |
| `opened_at`/scheduled delay | runtime timer correlated to target                        | late timer opens only if target remains hovered and no blocking surface exists |
| `placement`                 | derived from current shared layout rect                   | view flips/clamps; state never caches stale pixel rect                         |
| `Dismiss`                   | update clears state                                       | pointer leave, Escape, focus loss, drag or modal open cancels it               |

`TooltipTarget` must identify a stable, live geometry key rather than copied
coordinates. Update starts/cancels a dwell timer; runtime schedules it; view
uses the target’s layout rect, flips and clamps as needed. Do not open a
tooltip while a modal is active, during a drag, or for a target that disappeared.
Escape and focus loss dismiss it; it never captures editor keyboard focus or
prevents pointer activation of its target. A keyboard-focused control needs an
equivalent discoverable description before tooltip-only text is acceptable.

## Proposed presentation/accessibility

Use one or two short sentences, sentence capitalization, no clickable action,
and no critical error/status information. The official capitalization guidance
places tooltip body text in sentence case
([Capitalization](https://plugins.jetbrains.com/docs/intellij/capitalization.html)).
Apply overlay/panel background, border and text roles (not editor syntax
colors), UI font, scaled logical padding/radius, window-edge clamping and
clipping via shared anchor/frame helpers. The exact delay, max width, arrow,
and fade are **unimplemented choices**; do not bake values into feature-local
painters. When Token gains an accessibility bridge, expose the text as the
focused control’s description and announce it without stealing focus.

## Gallery and acceptance

There is no tooltip gallery coverage. A future specimen needs: delayed open,
cancel before delay, keyboard-focused counterpart, all four edge placements,
long localized text/wrap, light/dark/HiDPI, and overlap with popup/modal. It
must prove a tooltip never blocks a click, never outlives its target, and never
duplicates the LSP documentation card.

### Explicitly deferred: Got It / onboarding tooltip

IntelliJ’s **Got It tooltip** is a distinct onboarding component for a new or
changed feature, not a longer ordinary tooltip. Token currently has no
onboarding model, persistent “seen” state, feature flag/eligibility policy,
sequencing, coachmark anchor, analytics, or dismissal affordance. It is
therefore explicitly deferred. If authorized by a concrete onboarding feature,
model it separately (`OnboardingTip { feature_id, anchor, body, action?, seen }`)
with persistent, versioned dismissal and keyboard/screen-reader equivalents;
never smuggle it into the ordinary hover-delay implementation.

## Evidence

- [Pointer-region model](../../src/model/ui.rs), [hover runtime](../../src/runtime/hover.rs), [shared placement surface](../../src/view/overlay_surface.rs)
- [IntelliJ components](https://plugins.jetbrains.com/docs/intellij/components.html) and [capitalization](https://plugins.jetbrains.com/docs/intellij/capitalization.html)
