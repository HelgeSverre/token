# Editor surface

The editor surface is Token's domain composition for viewing and editing a
document in one group. It shares editor anatomy and geometry; it is not a
reusable multiline-field implementation and must never reimplement the text
renderer for a gallery or feature.

## Current composition — verified

One EditorArea owns documents, per-view EditorState values, editor groups and a
layout tree. A group has tabs and an active editor; multiple editor views can
share a document while retaining their own cursors, selections, viewport, soft
wrap, folds and inline ghost projection. Rendering is orchestrated only by
[Renderer](../../src/view/mod.rs).

| Layer                                                    | Existing responsibility                                                                           |
| -------------------------------------------------------- | ------------------------------------------------------------------------------------------------- |
| [editor-area model](../../src/model/editor_area.rs)      | documents, views, tab groups, split/preview tree and focused group                                |
| [editor state](../../src/model/editor.rs)                | cursors/selections, viewport, wrapping, folds, ghost text and view mode                           |
| [editor layout](../../src/layout/editor.rs)              | tab strip flow, tab clipping/scroll and preview chrome                                            |
| [editor geometry](../../src/view/geometry.rs)            | group content rectangle, find-bar inset, gutter lanes, cursor/row transforms and scrollbar tracks |
| [editor text renderer](../../src/view/editor_text.rs)    | visible text rows, selections, diagnostics, carets, gutter and clipping                           |
| [editor scrollbars](../../src/view/editor_scrollbars.rs) | pixel scroll metrics, thumbs and overview marks                                                   |
| [runtime mouse](../../src/runtime/mouse.rs)              | typed hit dispatch, selection drags, tabs, gutter and scrollbar capture                           |
| [editor update](../../src/update/editor.rs)              | deterministic cursor, selection, folding, wrap and scroll transitions                             |

A GroupLayout consists of group rect, tab bar, optional find-bar rect, remaining
content rect, gutter lanes/border and text start. Both rendering and pointer
mapping construct it. Content uses a clip; the gutter has a different clip. The
tab bar is a separate Clay snapshot so its scrolling and title clipping do not
invent editor-content geometry.

## Shared anatomy

### Group and chrome

1. **Tab strip**: each group owns an ordered tab collection and horizontal pixel
   tab scroll. EditorTabBarLayout provides identical flow, clipping and hit
   rectangles for drawing/dragging/input. Tab focus changes the focused group.
2. **Docked find bar**: exists only for the matching focused editor and consumes
   space below tabs. It is not overlaid inside the text viewport.
3. **Content rect**: establishes the clipping boundary for the active tab
   content and the overlay scrollbar edge.
4. **Special content boundary**: the group can render Text, CSV, Image or
   BinaryPlaceholder. The latter three use separate painters/input paths; text
   behaviors must be gated by EditorState::is_plain_text_mode().

### Text editor anatomy

For Text mode, the surface is: gutter lanes; gutter border/padding; text
viewport; selection and occurrence layers; syntax/decorations; carets; fold
disclosures; ghost/inlay projection; optional find/diagnostic overview marks;
and vertical/horizontal overlay scrollbars.

The gutter is not merely line numbers. GutterLayout orders Marks, LineNumbers,
Fold and Diff lanes left-to-right, skipping zero-width lanes. Marks activate with
diagnostics; fold lane is enabled for plain text. Folding is an interactive
gutter lane; marks may consume input for their future owner. Line number width
uses one model formula based on line count, avoiding geometry drift. IntelliJ's
editor reference similarly treats the gutter as an area for line numbers,
folding and contextual actions, and inlays as additional editor information:
[UI overview](https://plugins.jetbrains.com/docs/intellij/ui-overview.html).

The current inlay-like mechanism is inline ghost text. It projects rows and
source spans through TextViewportMap; it affects wrapping, hit testing,
scrollbars and overview invalidation, but is not a generic arbitrary inlay API.
A proposed inlay system must extend this projection/mapping source of truth,
not paint foreign text in a second line loop.

## Coordinates and data model

Document positions are source line/column. Visual rows are derived by
TextViewportMap from document plus pane-local wrap cache, folds and ghost
projection. The viewport retains integral top visual row/left column and
fractional physical-pixel offsets. The transformations are:

```text
window pointer
  -> GroupLayout text origin/content clip
  -> TextViewportMap visible row + visual display column
  -> document source Position
source Position
  -> display row/column through wrap/fold/ghost map
  -> viewport pixel offsets
  -> GroupLayout text origin
  -> clipped window pixels
```

Use GroupLayout::pixel_to_cursor or the renderer wrapper, never a logical line
times line-height calculation. Rectangle selection intentionally requests a
visual column helper. Hover requests an actual source glyph cell and rejects
whitespace, gutter, below-EOF and ghost glyphs; this differs correctly from a
caret hit. Cursor, selection and rendering all consult the same visual map.

EditorState owns per-view state. Document owns buffer/revision, diagnostics,
syntax/folds source data and text settings. App UI owns global focus, modal,
hover/capture and overlays. A component cannot move cursor or modify a buffer by
calling a painter; it sends EditorMsg/DocumentMsg and update owns policy.

## Event and focus contract

### Existing pointer behavior

- A group tab focuses its group, selects the tab and arms a thresholded tab drag.
- Text content click uses the production cursor mapping and selection logic,
  including modifiers/click count; drag selection is editor-owned.
- Fold lane click focuses its group and sends Fold Toggle. Other interactive
  gutter lanes consume before ordinary text selection.
- Scrollbar thumb/track hit testing precedes editor content and uses captured
  press-time scrollbar geometry. Track click changes viewport, not caret.
- Find bar, overlays and special modes intercept their own targets first.
- Right/middle click paths have their own contexts; no caller should assume a
  left press definition covers them.
- Scrolling can dismiss completion because an anchored completion window cannot
  remain truthfully attached after text moves.

FocusTarget::Editor is coarse. Per-group focus lives in EditorArea::focused_group_id.
A group click updates that ID; multiple views must not share cursor/viewport state
merely because they share the document.

### Existing keyboard behavior

EditorMsg covers movement, selection extension, word/line/document navigation,
paging, cursor positioning, scrolling, soft wrap, rectangle selection,
occurrence and fold operations. Update first rejects text operations for non-text
tabs and keeps selection/cursor history policy deterministic. Cursor reveal uses
visual rows/padding; wheel scroll does not silently move the cursor.

### Accessibility — gap and proposal

Current native CPU painting has no component accessibility tree, editor text
provider, screen-reader semantics, IME contract, high-contrast mode guarantee or
standard focus traversal across editor subparts. Do not claim accessibility from
visible carets, colored syntax or pointer affordances.

A future adapter should expose one editor document/view, caret/selection,
line/column, read-only/busy state, gutter actions and labelled scroll ranges
without changing the Rust rendering/model boundary. It must retain source versus
visual-row semantics, announce validation/diagnostics without relying solely on
color, and provide keyboard access to any new gutter/inlay action. This is
proposed work, not a precondition for ordinary rendering fixes.

## Font, theme and performance contract

The text grid uses Code role and actual configured monospaced metrics. Editor
tabs and explorer-style text also currently use Code in production; do not
silently switch them to Ui. Proportional Ui role is selected only by painters
whose labels/controls are designed for it. Gutter, text, selection, caret and
syntax pull their semantic colors from resolved editor/gutter/syntax roles.
Scrollbar overview marks map existing diagnostic/find semantics to colors; no
new arbitrary color should be introduced at paint time.

Text rendering is bounded to visible projected rows. Shared PerfStage entries
measure text, glyph, gutter, scrollbar and related stages. Preserve
EditorState::is_plain_text_mode() around text-only fast paths so CSV/image/binary
tabs never enter code-text rendering. Gallery fixtures must call production
Renderer/editor-text paths; a hand-drawn line loop breaks geometry/performance
truth.

## Edge cases

- Soft wrap maps one logical line to many visual rows and disables horizontal
  scrolling; viewport/caret/gutter/folding still target source positions.
- Folds hide source rows; collapsed headers and fold badges use shared geometry.
- Ghost/inlay projection can add rows and width, moves source suffix display and
  must invalidate wrap/overview caches.
- A docked find bar shifts content geometry and every gutter/text hit must see it.
- Fractional pixel scroll clips the first/last row; no integer-only row math.
- Long lines, tabs and Unicode use display/visual-column helpers, not byte count.
- A small group may yield zero visible rows/columns; hit and scroll must clamp.
- Multiple groups can show one document but have distinct view state and focus.
- CSV/image/binary have their own interaction contracts; editor-surface docs
  describe shared chrome, not permission to feed them text messages.

## Gallery coverage and sequencing

Gallery currently samples document tabs (states, overflow, drag ghost), dock/
terminal tabs, panels and static scrollbars/splitters. It deliberately does not
cover editor composition or interaction. Missing specimens include: focused and
unfocused text group; selected and multi-caret text; diagnostic and folding
gutter lanes; wrapped/fractional-scroll viewport; find-bar content inset; ghost
projection; syntax/selection/caret layering; special tab boundaries; scrollbars
with overview marks; and split editor groups.

Recommended first composition is a deterministic production EditorArea fixture
with a single text view, then focused/unfocused pair, then a wrapped/folded/
diagnostic/ghost matrix. Capture native-size light/dark screenshots. Only after
that add interaction-specific tests for selection, gutter and capture; do not
turn gallery into a second editor implementation.

## Acceptance criteria and evidence

An editor-surface change is acceptable when it names document versus view versus
global UI ownership; reuses GroupLayout, TextViewportMap and editor_text;
keeps all content clipping; gates special modes; maps pointer through the same
geometry; preserves tab/focus/capture behavior; adds relevant PerfStage evidence;
and gives gallery composition through production painters.

High-confidence evidence comes from the linked implementation files,
[editor-area tests](../../tests/editor_area.rs),
[scrolling tests](../../tests/scrolling.rs), [folding tests](../../tests/folding.rs) and
[gallery guide](../dev/ui-gallery.md), reviewed 2026-09-12. The accessibility
adapter, generic inlays and new gallery matrix are proposed, not implemented.
