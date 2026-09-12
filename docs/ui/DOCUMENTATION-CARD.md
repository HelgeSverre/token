# Documentation card

## Purpose

A **documentation card** is the readable, scrollable detail associated with a
completion selection or hover result. It is not a tooltip: it may contain
multi-line styled documentation, diagnostic context, code zones, scrolling, and
an expand state. IntelliJ lists documentation popups as a popup use case
([UI overview](https://plugins.jetbrains.com/docs/intellij/ui-overview.html)).

## Current implementation — high confidence

The current shared reading state is `DocumentationState { scroll, expanded }`
inside `CursorOverlayState`. `UiState::has_documentation()` exposes only (a)
an open hover card or (b) an open completion popup whose selected item has
documentation, with editor focus and no modal. The update layer accepts
`UiMsg::PageDocumentation`, measured `DocumentationScrolled`, and
`ToggleDocumentation`; it clears a captured scrollbar drag when expansion
changes geometry ([model](../../src/model/ui.rs), [update](../../src/update/ui.rs)).

Two consumers render through `OverlaySurface`:

- Completion supplies a list plus an optional side `Documentation` panel,
  resolved asynchronously for the selected item.
- Hover maps LSP styled text plus diagnostics/related information into a
  cursor-anchored `Zones` card. Mouse-dwell cards preserve their text-cell
  anchor; keyboard hover falls back to the caret.

The card content is deliberately feature-owned: completion holds its selected
documentation; `HoverCardState` owns optional `StyledText` and an optional
anchor. The view does not fetch or parse LSP data. Late responses are guarded
by document/revision/request ownership and a dismissed hover cannot reopen.

### Implemented data ownership (field-level)

| Field/symbol                                                      | Writer                                    | Reader / invariant                                                                      |
| ----------------------------------------------------------------- | ----------------------------------------- | --------------------------------------------------------------------------------------- |
| [`DocumentationState::{scroll, expanded}`](../../src/model/ui.rs) | `update_ui` documentation messages        | copied into `overlay_surface::Documentation`; independent viewport, not document offset |
| [`HoverCardState::{content, anchor}`](../../src/model/ui.rs)      | `update::hover` after accepted LSP result | spec builds zones; dwell cell anchor is optional, otherwise caret anchor                |
| `completion_menu` selected item/docs                              | completion provider/update/resolve        | card exists only for current selected item; selected index is `CursorOverlayState`      |
| `CursorOverlayState::documentation`                               | page/wheel/toggle messages                | `UiState::has_documentation()` rejects non-editor/modal/unsupported cases               |
| `hover_request`                                                   | hover request/update/runtime              | dismissal/cursor/document mutation invalidates ownership before response writes card    |

`StyledText`/spans are content, not geometry. The view may make wrapped plans
but must not reparse or mutate LSP payload.

### Interaction/lifecycle

Page controls are valid only while `has_documentation()` is true. The
documentation viewport scroll is independent from its associated list’s
selection/scroll. Hover cards close on edit/caret/focus changes and respect a
pointer grace interval when the user enters the card; keyboard hover ignores
incidental pointer movement until blur. Completion documentation follows the
currently selected completion row and is reset/re-resolved when selection
changes. These are implemented feature rules, not generic card behavior.

Keyboard accessibility, semantic reading order, copy/link activation, pinned
documentation, and markdown-rich rendering are not established general
capabilities. Current LSP markdown is reduced to Token `StyledText`/spans;
do not promise browser-like documentation support.

| Transition (implemented)        | Authority                                                | Guard/visible result                                           |
| ------------------------------- | -------------------------------------------------------- | -------------------------------------------------------------- |
| selected completion changes     | completion update + menu ordering                        | docs viewport resets; resolve is associated with that item     |
| docs reply arrives              | completion/LSP update                                    | only matching request/session fills the selected item          |
| hover reply arrives             | hover update                                             | document/revision/request and live hover ownership must match  |
| page/wheel/scrollbar            | measured overlay layout → `UiMsg::DocumentationScrolled` | only documented surfaces update `DocumentationState.scroll`    |
| expansion                       | `UiMsg::ToggleDocumentation`                             | clears scrollbar capture and recalculates layout               |
| dismiss/edit/caret/focus change | feature update/runtime                                   | card and pending ownership are removed; late result is ignored |

### Layout/theme

Overlay dimensions define a 360 logical-px completion docs width and 17 logical
px zone line stack; placement, edge clamp, scrollbar and clipping are shared
with the parent overlay. Banner/code/text zones have their own layout and use
the overlay theme; code vs prose uses the appropriate `TextPainter` font role.
The renderer—not the model—measures wrapped content, then reports a measured
scroll destination. All logical metrics are scaled by `model.metrics.scale_factor`.

#### Measured-layout invariants — implemented

- A completion card is `OverlaySpec::docs`; a hover card is `Body::Zones` with
  optional severity banner/code/styled text. One `layout_measured` call creates
  `OverlayLayout::{docs_panel,docs_text,docs_code,docs_scrollbar,docs_viewport}`
  or a zone plan respectively.
- Wrapped docs lines/truncation/height are built once in `docs_plan`; hover-zone
  wrapped lines/heights are built once in `zone_plan`. Painting consumes those
  plans and hit testing returns the same viewport/scrollbar geometry.
- Existing completion docs width is 360 logical px before scaling/available
  constraints; it is not a universal window minimum. Parent cursor anchoring
  still flip/clamps and clips at the window edge.
- `DocumentationViewport::max_scroll()` clamps to `total - visible`. Update
  receives measured destinations rather than wrapping text, preventing
  painter/hit-test divergence.

## Proposed contract

When a third consumer appears, extract only a data-neutral payload:

```text
DocumentationContent { styled_text, sections, source_label? }
DocumentationViewport { scroll, expanded }
DocumentationEvent = Page | ScrollToMeasuredOffset | ToggleExpanded | Dismiss
```

The owning feature remains responsible for content identity, anchor, loading,
and stale-result cancellation. The card needs accessible name/source, a
visible loading/empty/error treatment, focus policy, selectable/copyable text
decision, bounded wrap/height, and an explicit pin policy before it can be
called reusable.

## Gallery, gaps, acceptance

No documentation-card fixture exists. `overlay-tabs.counts`, list rows and
scrollbar specimens exercise chrome only. Add gallery states for short/long
wrapped docs, code plus diagnostic banner, narrow edge flip, scroll thumb,
expanded/collapsed, and light theme before changing card layout.

Acceptance: the same layout drives draw, hit test and scrollbar; selected
completion and docs cannot drift; scroll survives redraw but resets on changed
content; narrow windows clip safely; hover’s stale response cannot reopen; and
all zones remain legible with UI/code font and theme variation.

Concrete workflow acceptance: resolve documentation for completion item A,
scroll it, select B, then deliver A’s delayed resolve; B must remain selected
and A cannot replace B’s card. For mouse hover, dwell on a bottom-right word,
enter the card during grace, scroll diagnostic/text, move caret, inject the old
reply: card placement must flip/clamp while open and remain closed afterward.

## Evidence

- [Documentation state and hover payload](../../src/model/ui.rs)
- [documentation messages/update](../../src/messages.rs), [update](../../src/update/ui.rs)
- [completion integration](../../src/update/completion.rs), [hover update](../../src/update/hover.rs), [overlay specs](../../src/view/modal.rs), [surface](../../src/view/overlay_surface.rs)
- [IntelliJ UI overview](https://plugins.jetbrains.com/docs/intellij/ui-overview.html)
