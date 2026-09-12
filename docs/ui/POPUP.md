# Popup

## Purpose and boundary

A **popup** is a lightweight, cursor- or pointer-anchored transient surface for
choosing or reading in the current editor context. It is not a dialog: it has
no backdrop or title-bar close affordance, and it must not become a second
settings page. This matches IntelliJ's distinction between lightweight popups
and action-blocking dialogs ([UI overview](https://plugins.jetbrains.com/docs/intellij/ui-overview.html),
[Popups](https://plugins.jetbrains.com/docs/intellij/popups.html)). **High confidence** for
the terminology; Token's component contract below is proposed where noted.

## Current implementation — high confidence

Token implements a single cursor-overlay family, not a generic `Popup` type.
`UiState::cursor_overlay: Option<CursorOverlayState>` owns exactly one kind,
selection, list scroll, pointer-hover row, and independent documentation state
([model](../../src/model/ui.rs)). `CursorOverlayKind` currently covers real
completion, hover, references/multiple definitions, code actions, and context
menu; its two debug variants are fixtures, not product components. Signature
help is a separate floating surface so that it can coexist with completion.

`view::overlay_surface::OverlaySpec` and its shared `layout()` are the painter
and hit-test authority. `view::modal::with_cursor_overlay_spec` maps model data
to that spec, while `runtime/input.rs`, `runtime/mouse.rs`, and update modules
own dispatch. This is a genuine reusable surface, but it currently supports
only the named cursor/menu cases—there is no application-wide popup registry.

| Concern               | Implemented authority                                                                                                                    |
| --------------------- | ---------------------------------------------------------------------------------------------------------------------------------------- |
| state                 | `CursorOverlayState`, plus feature data such as `completion_menu`, `hover_card`, `reference_list`, `code_action_list`, or `context_menu` |
| placement/hit testing | `OverlaySpec`/`OverlayLayout`, `Anchor::Cursor` or `Anchor::Menu`, and `view::hit_test`                                                  |
| transitions           | `LspMsg`, `CompletionMsg`, `ContextMenuMsg`, `UiMsg::DocumentationScrolled`; feature update modules                                      |
| paint/theme           | `view/modal.rs`, `view/overlay_surface.rs`, `Theme::overlay`                                                                             |

### Implemented data ownership (field-level)

| Field/symbol                                                                       | Owner and writer                                       | Reader / invariant                                                                         |
| ---------------------------------------------------------------------------------- | ------------------------------------------------------ | ------------------------------------------------------------------------------------------ |
| [`UiState::cursor_overlay`](../../src/model/ui.rs)                                 | feature update opens/replaces it; dismissal clears it  | renderer and input dispatcher branch on `kind`; exactly one cursor overlay                 |
| [`CursorOverlayState::{kind, selected, scroll, hover_row}`](../../src/model/ui.rs) | feature update and pointer routing                     | `selected` is keyboard authority; `hover_row` is visual pointer wash; scroll is list-local |
| `CursorOverlayState::documentation`                                                | `UiMsg::DocumentationScrolled` / `ToggleDocumentation` | independent from list scroll; valid only through `UiState::has_documentation()`            |
| [`completion_menu`](../../src/model/ui.rs)                                         | completion update                                      | Completion is visible only for nonempty filtered results; menu owns item order/docs        |
| [`hover_card`](../../src/model/ui.rs) + `hover_request`                            | hover/LSP update                                       | payload/request identity clear on dismiss; response matches request/document/revision      |
| `reference_list`, `code_action_list`, `context_menu`                               | navigation/LSP/context-menu update                     | vector/`items` is render, click and Enter order; context menu owns captured anchor         |

`OverlaySpec` borrows feature payload to render; it is not a second store for
rows, selection, or asynchronous requests. `SignatureHelpState` is outside the
table because it is a separate simultaneous float, not a `CursorOverlayKind`.

### Implemented interaction and lifecycle

Opening a feature creates a `CursorOverlayState`; the feature payload is the
ordering/content authority and is cleared with dismissal. Lists route Up/Down,
Enter, Escape and sometimes Tab before editor input. The documented exception
is important: completion accepts its dedicated keys but ordinary typing flows
to the editor; references/code actions/context menu consume their documented
keys and dismiss otherwise. Hover is invalidated by cursor/edit/focus changes;
mouse dwell requests are cancellation and revision guarded. See
[`model/ui.rs`](../../src/model/ui.rs),
[`update/completion.rs`](../../src/update/completion.rs),
[`update/hover.rs`](../../src/update/hover.rs), and
[`runtime/input.rs`](../../src/runtime/input.rs).

Pointer interaction uses the same layout for row, tab, documentation-scrollbar,
and outside-press decisions. A popup is clipped to its own panel and its anchor
is captured when it cannot be derived again (notably a right-click context menu).
No accessibility tree, screen-reader announcement, or focus-ring model is
implemented for cursor popups; that is a known gap, not an implied capability.

| Transition (implemented)                                           | State owner / invariant                                                     | Result                                            |
| ------------------------------------------------------------------ | --------------------------------------------------------------------------- | ------------------------------------------------- |
| completion result becomes nonempty                                 | completion update; selected row and menu order stay paired                  | open `Completion`, anchor at caret                |
| LSP hover resolves for current document/revision/request           | hover update; `HoverCardState` and overlay are installed together           | open `Hover` at dwell cell or caret               |
| references/code actions/context menu open                          | their update module builds one authoritative row vector                     | open matching kind with `selected = 0`            |
| key/pointer activation                                             | runtime routes supported navigation; feature update indexes the same vector | command/edit/navigation or no-op for disabled row |
| Escape/outside press/caret edit/focus loss (policy varies by kind) | dismissal clears overlay and payload; hover invalidates request ownership   | no stale overlay resurrection                     |

### Geometry, type, and theme

`Anchor::Cursor` uses a physical caret/click rect, prefers a side, flips when
space is insufficient, and clamps to window edges. `layout::anchor` owns this
geometry. Overlay logical constants scale through `scale_factor`: cursor radius
8, cursor rows 24 high with 4 inset and radius 5; regular overlay rows are 30
high. The type scale is input 14, row 13, metadata 11 logical px.
`Theme::overlay` supplies panel/background, borders, text, selection and chrome
roles; painting uses UI and code font roles rather than assuming editor text
metrics. Shadows, rounded masks, and clipping are implemented by `Frame`.
([overlay surface](../../src/view/overlay_surface.rs),
[theme](../../src/theme.rs), [frame](../../src/view/frame.rs)).

#### Measured-layout invariants — implemented

- [`OverlaySpec`](../../src/view/overlay_surface.rs) declares anchor, optional
  tabs/header/footer, list/fields/zones body, pointer row and optional docs;
  [`layout_measured`](../../src/view/overlay_surface.rs) produces the one
  `OverlayLayout` consumed by painting and hit testing.
- Real paths pass glyph measurement to `layout_measured`; `layout()` is only a
  monospace-cell fallback for tests/painterless contexts. Do not use it for
  interactive production placement.
- `Anchor::Cursor`/`Menu` receive physical `(x, y, h)`, add caret gap, prefer a
  side, flip when it cannot fit, then clamp to the window. Cursor/menu surfaces
  do not dim the backdrop; centered/settings anchors follow separate rules.
- `OverlayLayout` retains snapped panel/row/tab/docs/scrollbar rects and
  premeasured wrapped-zone/docs plans. Painting consumes those plans; hit tests
  rebuild the same spec/layout; `Frame` clips panel/documentation content.

## Proposed public contract

Do not introduce a widget object that owns I/O. Add a declarative `PopupSpec`
only if a new concrete consumer cannot be expressed by `OverlaySpec`:

```text
PopupSpec { anchor, body, selected, scroll, dismiss_policy, a11y_label }
PopupEvent = Move | Activate | Dismiss | HoverRow | Scroll | ToggleDocs
```

The feature model owns payload and effect-specific validation; update maps a
`PopupEvent` into the feature's messages/commands. Required policies are
explicit: outside pointer press, Escape, focus loss, document/caret change,
and whether non-navigation keys pass through or are consumed. Never use a
popup for required confirmation, a persistent workflow, or unanchored global
feedback.

## Gallery, gaps, and acceptance

The gallery proves only shared pieces: `overlay-tabs.counts`, menu rows, and
scrollbars ([catalog](../../src/model/gallery.rs)). It has no end-to-end
completion, hover, context-menu, placement-flip, or keyboard specimen.

Before calling a new popup complete, prove: below/above and all-edge clamping;
one authoritative item order for render/click/Enter; disabled rows skipped;
outside/Escape policy; HiDPI and light/dark themes; a clipped scrollable body;
and no leaked late async result after dismissal. Make this measurable with a
headless update test plus gallery screenshots at 1x and 2x. Priority: reuse
this surface for a concrete editor-context consumer before creating a general
API.

Concrete workflow acceptance: open completion at a bottom-edge caret and make
the list flip above; move selection across a documentation item; wheel docs
without changing selection; click a row and press Enter in separate runs; type
ordinary text while completion is open; dismiss and deliver a late LSP reply.
The final model must contain neither a visible popup nor payload from the
abandoned request.

## Evidence

- [Token overlay model](../../src/model/ui.rs), [spec/layout/painter](../../src/view/overlay_surface.rs), [consumer mapping](../../src/view/modal.rs)
- [Token gallery catalog](../../src/model/gallery.rs)
- [IntelliJ UI overview](https://plugins.jetbrains.com/docs/intellij/ui-overview.html) and [popup guidance](https://plugins.jetbrains.com/docs/intellij/popups.html)
