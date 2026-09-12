# Status bar

## Purpose

The **status bar** is the always-visible, space-constrained summary of the
focused document and application state. It is not a notification center, a
toolbar, or a substitute for an actionable context menu. IntelliJ similarly
describes status widgets as compact information/settings relevant enough to be
always shown ([Status Bar Widgets](https://plugins.jetbrains.com/docs/intellij/status-bar-widgets.html)).

## Current implementation — high confidence

`StatusBar` is a structured ordered vector of `StatusSegment`s. Each has a
fixed `SegmentId`, left/right position, text-or-empty content, overflow priority
and optional minimum width. Implemented IDs are file name, modified indicator,
status message, diagnostics, LSP server, inline suggestion, caret count, text
policy, selection, cursor position and line count. Center segments are modeled
but the renderer emits none.

`sync_status_bar(model)` derives most values from current model state. The
`StatusMessage` slot is special: a `TransientMessage` (or an explicit segment
write) wins; otherwise the highest-severity diagnostic under the cursor may
occupy it, flattened and capped at 120 characters. `UiMsg::UpdateSegment`,
`SetTransientMessage`, and `ClearTransientMessage` produce status-bar-only
damage. Rendering is isolated as `Renderer::render_status_bar` and has a
dedicated performance/damage stage.

| Model/event                                       | Ownership                                                    |
| ------------------------------------------------- | ------------------------------------------------------------ |
| `StatusBar`, `StatusSegment`, `TransientMessage`  | `model/status_bar.rs` and `UiState`                          |
| derived segment content; stored priority metadata | `sync_status_bar` / `StatusSegment`                          |
| explicit feedback                                 | `UiMsg` → `update/ui.rs`                                     |
| expiry wake                                       | runtime app event loop                                       |
| geometry/draw                                     | `StatusBar::layout_measured` + `Renderer::render_status_bar` |

### Layout/theme

The layout measures real glyph widths. It applies character-unit padding and
spacing converted through measured space width; left items flow left-to-right,
right items are placed from the right edge backwards, and separators appear
between right segments. `StatusSegment::priority` and `min_width` are stored,
but `layout_measured` currently does **not** cull, truncate, or otherwise apply
them on overflow; its saturating placement can compress leftward under narrow
width. Overflow prioritization is therefore proposed, not implemented. Rendering
snaps the `UiKey::StatusBar` rect, paints background/top border, vertically
centers text at a configured clamped status-bar font size, and blends right-side
separators. Theme roles are `Theme::status_bar.{background,foreground,border}`;
text uses the UI face.

No segment has current click/keyboard activation, popup, tooltip, configuration
UI, semantic accessibility role, or center alignment. `HoverRegion::StatusBar`
exists for hit/scroll routing but does not make the bar interactive.

| Transition (implemented)                  | Authority                           | Invariant                                                  |
| ----------------------------------------- | ----------------------------------- | ---------------------------------------------------------- |
| focused document/config/LSP state changes | `sync_status_bar`                   | derived segments recompute from model, never painter cache |
| explicit segment write                    | `UiMsg::UpdateSegment`              | triggers `DamageArea::StatusBar` only                      |
| transient feedback starts/expires         | `UiState` + runtime wake            | transient owns message slot until cleared/replaced         |
| renderer runs                             | measured layout + theme/font config | text/separators use the same measured result               |

## Proposed contract

Preserve fixed segment ownership. A new segment needs a named `SegmentId`,
position, content derivation owner, priority/minimum-width rationale, privacy
review, and narrow `UiMsg`/update API. Do not let arbitrary features append
unbounded strings or write a shared status segment directly.

If an existing segment becomes a setting/action, add an explicit activation
event and a context popup contract with a keyboard equivalent—do not overload
plain display text. Proposed semantic shape:

```text
StatusSegmentSpec { id, position, priority, min_width, text, accessible_label, activate? }
```

| Proposed event                        | Owner                         | Required guard                                                             |
| ------------------------------------- | ----------------------------- | -------------------------------------------------------------------------- |
| derive/update segment projection      | named feature/update function | only declared `SegmentId` may be written; preserve transient priority rule |
| activate a future interactive segment | status-bar hit/input route    | target must be visible, focusable and dispatch a specific message/popup    |
| width/config/theme change             | layout/render                 | rerun measured layout; do not preserve stale x coordinates                 |

It remains a renderer-facing projection; effects stay behind messages/commands.

## Gallery, gaps, acceptance

The gallery has no status-bar specimen. Add one only with the renderer’s real
layout: full/default, diagnostics/LSP/inline values, transient override,
narrow overflow priority, long localized text, light/dark and scaled font.

Acceptance: derived state never overwrites an explicit/transient message;
expiry redraws only the required area; glyph measurement and paint coordinates
match; narrow-width behavior is specified and tested before priority becomes a
claimed feature; and every newly interactive segment has focus, activation and
accessible-description coverage.

## Evidence

- [Status model and layout](../../src/model/status_bar.rs), [UI state](../../src/model/ui.rs), [messages](../../src/messages.rs), [update](../../src/update/ui.rs)
- [Renderer](../../src/view/mod.rs), [chrome layout keys](../../src/layout/keys.rs), [performance stage](../../src/perf.rs)
- [Local IntelliJ SDK status-widget reference](../../temporary-docs/intellij-platform-sdk/references/ui-settings-and-toolwindows.md) (secondary)
- [IntelliJ status widgets](https://plugins.jetbrains.com/docs/intellij/status-bar-widgets.html)
