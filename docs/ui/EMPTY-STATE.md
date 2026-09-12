# Empty state

## Purpose

An **empty state** explains why a scoped surface has no content and, only when
useful, tells the user the next relevant action. It is not a generic blank
panel, loading indicator, error notification, or a promise that a placeholder
feature exists.

## Current implementation — high confidence

Token has several feature-local empty renderings and no shared `EmptyState`
model/painter:

| Consumer                  | Current empty result                                                   |
| ------------------------- | ---------------------------------------------------------------------- |
| Problems panel            | centered `problems_empty_text(model)` when derived rows are empty      |
| Outline panel             | centered “No outline available”                                        |
| Usages panel              | `UsagesPanelState` starts with “Run Find Usages at a symbol to search” |
| placeholder panels        | static provisional strings, e.g. “Terminal panel coming soon…”         |
| binary/special editor tab | separate `BinaryPlaceholderLayout`/button, not a panel empty state     |

The Problems panel gets its rows and message from the diagnostics/workspace
model; it draws the message centered inside the dock’s clipped content rect.
Outline checks its parsed outline. Usages owns a domain state that distinguishes
its initial instruction from populated results/loading. The gallery includes
`panel.bottom-empty` (Problems) and `panel.right-empty` (Outline), making those
two the only demonstrated shared-chrome empty states.

| Current state source                               | Empty decision                                       | Event/lifecycle owner                                  |
| -------------------------------------------------- | ---------------------------------------------------- | ------------------------------------------------------ |
| `problems_rows(model)` + diagnostic/workspace data | rows empty → `problems_empty_text(model)`            | diagnostics/update refresh re-renders panel            |
| outline model/parser output                        | no outline → “No outline available”                  | syntax/outline update changes source data              |
| `UsagesPanelState` rows/status                     | initial query instruction or feature-specific status | usages request/result state owns transition            |
| `PlaceholderPanel::message()`                      | fixed provisional message for enum id                | no async/loading lifecycle; not a reusable empty model |

## Proposed contract

Do not replace the domain state with an English string. A reusable _view data_
shape may be extracted when three consumers genuinely converge:

```text
EmptyState { kind: Empty | FilteredOut | Unavailable | Error, title?, detail?, primary_action? }
```

| Proposed event                       | Owner                 | Constraint                                                  |
| ------------------------------------ | --------------------- | ----------------------------------------------------------- |
| source result becomes empty/nonempty | consumer update       | projection changes; painter retains no prior text/selection |
| filter changes                       | consumer query state  | distinguish `FilteredOut` from no data                      |
| retry/primary action                 | consumer message      | action exists only when current scope can perform it        |
| scope closes                         | dock/editor lifecycle | action/focus state is discarded with scope                  |

The consumer owns `kind`, data query and action message; the shared painter only
lays it out. `Loading` and `Error` remain distinct states, with retry only when
the consumer can safely issue it. Action text must describe an available action,
not advertise a coming-soon feature. An empty document editor is not an empty
state—it is a valid editing surface.

### Interaction/accessibility

Current centered text is passive; no common keyboard target, action button, live
announcement, illustration, or accessibility role is implemented. A proposed
action must be keyboard-focusable, have a visible focus state and match a real
message. Screen readers need the reason and action in the same logical order.

### Geometry/theme

Current panel messages calculate text width and center within the supplied
content rectangle, inherited `Theme::sidebar`/overlay text colors, and dock
clip. They use current painter line height, not a fixed pixel baseline. Any
shared version must use the consumer’s `LayoutSnapshot` rect, `TextPainter`
measurement, logical padding/scaling and truncate/wrap safely on small panels.

Current transitions are feature-local: Problems repaints when derived
diagnostics rows go empty/nonempty; Outline follows parsed-outline availability;
Usages replaces its instruction/loading/status with its own result model. A
future common painter must receive the _current_ projection every frame rather
than retaining messages or initiating queries itself.

## Gallery, gaps, acceptance

Existing gallery: `panel.bottom-empty`, `panel.right-empty`. Needed before a
shared empty state: no-data vs filter-no-match vs unavailable vs error; short
and wrapped detail/action; narrow/short dock clipping; selected/focused action;
light/dark/HiDPI. Acceptance is context-specific explanation, no false action,
no painting outside the content clip, and state transition from empty to loaded
without stale selection or scroll.

## Evidence

- [Problems empty rendering](../../src/view/panels.rs), [panel states](../../src/model/ui.rs), [Usages model](../../src/model/usages.rs)
- [Placeholder implementation](../../src/panels/placeholder.rs), [binary layout](../../src/view/geometry.rs), [gallery](../../src/model/gallery.rs)
- [IntelliJ components catalog](https://plugins.jetbrains.com/docs/intellij/components.html)
