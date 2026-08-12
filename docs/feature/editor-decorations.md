# Editor Decorations & Gutter Lanes

A shared contract for per-line gutter marks, in-text range decorations, and scrollbar overview marks — built once, consumed by find enhancements, diff gutter, code folding, and LSP diagnostics instead of each reinventing gutter pixels.

> **Status:** 📋 Planned
> **Priority:** P2 (Important) — prerequisite for [find-enhancements](find-enhancements.md), [diff-gutter](diff-gutter.md), [folding-basic](folding-basic.md), and [LSP diagnostics](lsp-integration.md)
> **Effort:** L (Phase 1 alone is most of an M; see plan)
> **Created:** 2026-08-11
> **Updated:** 2026-08-11 (revised after 3-reviewer pass: damage model reality, `Copy` geometry, `TextViewportMap`, consumer ordering)
> **Milestone:** 4 - Hard Problems (Phase 1 standalone, earlier — see plan)

---

## Overview

### Why

Four planned features independently need to draw in or around the gutter and text:

- [find-enhancements.md](find-enhancements.md) (Milestone 2) wants match highlights "through the text decoration pipeline" — which doesn't exist yet.
- [diff-gutter.md](diff-gutter.md) wants a 3–4 px changed/added/deleted bar and speaks of "`GutterLayout` for marker lanes and hit targets".
- [folding-basic.md](folding-basic.md) wants ▶/▼ chevrons and "shared gutter hit targets".
- [lsp-integration.md](lsp-integration.md) Phase 2 wants severity marks per line plus underlines in the text area.

All four reference shared machinery that does not exist. This document is that contract: one lane system, one per-frame mark collection step, one hit-test extension, one decoration overdraw pass. Each feature keeps its own state and messages; only geometry, rendering order, and click routing are shared. Visual conventions (severity glyphs/colors, `draw_wavy_underline`) are owned by [overlay-surface.md](overlay-surface.md) so gutter dots, underlines, status bar, and hover cards read as one system — this doc consumes them.

### Current State

- The gutter is a fixed-width strip: `LINE_NUMBER_GUTTER_CHARS = 5` in `src/model/mod.rs` (`" 123 "` + `gutter_padding`). It cannot widen for marks — and already misrenders past 99,999 lines (min width 5 chars fits 5 digits since `gutter_padding == padding_medium == 4`). Crucially, the constant has **model-side consumers with no view layout in scope**: `compute_visible_columns` (`src/model/mod.rs:108`), cursor-visibility/viewport sync (`src/model/mod.rs:606,643`), and `sync_viewports` (`src/model/editor_area.rs:719`). Dynamic width must therefore be computable model-side from `(document, active lanes, metrics)` — a view-only `GutterLayout` cannot be the sole source of truth.
- `render_gutter` (`src/view/editor_text.rs`) draws background, line numbers, border — nothing else. Actual pass order is **text area first, then gutter** (`src/view/mod.rs:373-389`); the gutter overpaints the text area's left edge.
- Frame primitives cover every mark we need: `fill_rect_px`, `blend_rect_px`, `set_pixel`, `Frame::blend_pixel` (`src/view/frame.rs`; nb. a *different* free `blend_pixel(src, dst)` exists in `src/overlay.rs` — name collision to watch).
- Hit-testing resolves gutter clicks to a line: `HitTarget::EditorGutter { group_id, editor_id, line }`. But the gutter is not inert today: press **focuses the group** (`src/runtime/mouse.rs:588`), and **drag maps into the text area and drives selection** (`src/runtime/mouse.rs:988-1000`). Lane routing must actively suppress these for interactive lanes.
- `GroupLayout` (`src/view/geometry.rs:578`) is `#[derive(Clone, Copy)]` and constructed **ad hoc** at many sites (`view/mod.rs`, `hit_test.rs`, `caret.rs`, `editor_text.rs`) — including the per-mouse-move hit-test path. Anything added to it must stay `Copy` and allocation-free.
- `TextViewportMap` (`src/model/editor.rs:231`) is the established visible-row ↔ document-line seam, already consumed by `editor_text.rs`, `caret.rs`, `hit_test.rs`, `geometry.rs`; [soft-wrap.md](soft-wrap.md) and [folding-basic.md](folding-basic.md) declare it mandatory. Decorations go through it too.
- The damage system (`src/commands.rs:473`) has exactly three areas: `EditorArea`, `StatusBar`, `CursorLines` — the last being a focused-group, plain-text, cursors-only blink fast path. There is **no group-scoped or general line-range damage**; this contract does not pretend otherwise (see Damage).
- The per-line pixel helpers a decoration pass needs (`EditorRenderContext::line_y`/`pixel_x`/…) are private to `editor_text.rs` — the passes live there, not in a new module, unless/until those helpers are worth exporting.

### Goals

- A `Copy`, allocation-free `GutterLayout` giving lane x-ranges and a dynamic total width (active lanes + actual digit count), with the width formula also callable from model code.
- A per-visible-row mark collection step aggregating from feature-owned state in O(viewport), routed through `TextViewportMap`.
- A range-decoration overdraw pass in the text area (underline/wavy/tint/strikethrough) that changes no layout.
- Scrollbar overview marks (severity/match ticks).
- Lane-aware gutter click routing that suppresses focus-steal/drag-select on interactive lanes.
- Ship Phase 1 (dynamic width) standalone — it fixes the live >99,999-line rendering bug and de-risks every consumer.

### Non-Goals

- **Virtual text** — inlay hints, code lens, inline blame. These change the document-position ↔ screen-position mapping (cursor math, hit-testing, selection, horizontal scroll). Explicitly out of scope; a future design must not be shoehorned into `RangeDecoration`.
- **Fine-grained damage.** Decoration changes emit `DamageArea::EditorArea`. Diagnostics publishes, diff refreshes, and fold toggles are sub-Hz events; building group-scoped/line-range damage variants plus a new partial-render path to optimize them is real renderer work with no measured need. Revisit only if profiling ever shows it.
- A provider/plugin registry, trait objects, or dynamic decoration sources — the source set is small, known, static.
- The features themselves (find, diff, folds, diagnostics) and the visual conventions (owned by overlay-surface.md).
- Indent guides and whitespace rendering — per-column drawing with their own docs.
- Themeable/user-configurable lane ordering.

---

## Design

### Lane Layout

Lanes are fixed-width vertical columns inside the gutter, left to right:

```text
┌───┬────────┬───┬──┬─┐
│ ● │   123  │ ▼ │▌ │ │  ← marks · line numbers · fold chevron · diff bar · border
└───┴────────┴───┴──┴─┘
  A      B      C   D
```

- **A — Marks lane** (~1 char): diagnostic severity glyph (conventions from overlay-surface.md), bookmark, future breakpoint. One glyph per line; priority within this lane only: breakpoint > error > warning > info > bookmark.
- **B — Line numbers**: width from `max(LINE_NUMBER_GUTTER_CHARS_MIN, digits(line_count))` where the minimum is **5** — matching today's width exactly for files under 100,000 lines (pixel-identity criterion), growing beyond.
- **C — Fold lane** (~1 char): chevron, present only when folding ships.
- **D — Diff lane** (3–4 px): colored bar at the border, per diff-gutter.md.

A lane is allocated only when its feature has state for the document. Geometry is a `Copy` struct of widths — no `Vec`, no allocation on the hit-test path:

```rust
// src/view/geometry.rs — embedded in GroupLayout, which stays Copy
#[derive(Debug, Clone, Copy, Default)]
pub struct GutterLayout {
    pub marks_w: u16,
    pub numbers_w: u16,
    pub fold_w: u16,
    pub diff_w: u16,
}
impl GutterLayout {
    pub fn total_width(&self) -> usize { /* sum + padding */ }
    pub fn lane_at(&self, x_in_gutter: usize) -> Option<LaneId> { /* range arithmetic */ }
}
pub enum LaneId { Marks, LineNumbers, Fold, Diff }
```

**Width is model-derivable.** The formula `gutter_width(document_lines, active_lanes, metrics)` lives where model code can call it, replacing `LINE_NUMBER_GUTTER_CHARS` at *all* consumers — including `compute_visible_columns` and `sync_viewports`. Because `sync_viewports` runs only on layout changes, any width change (99,999 → 100,000 lines; a lane activating on first diagnostic) must trigger a viewport-column recompute, or horizontal scroll geometry goes silently stale. That trigger is Phase 1 work with a test at the digit boundary and on first-lane activation.

### Mark Collection

Feature state stays where it naturally lives — `Document.diagnostics` (a projection; see lsp-integration.md), future `Document.diff_state`, fold state on `EditorState`. Collection is per **visible row**, through `TextViewportMap`:

```rust
// gathered per visible row during the gutter pass
#[derive(Debug, Clone, Copy, Default)]
pub struct LineMarks {
    pub mark: Option<Mark>,           // marks lane; priority already resolved
    pub fold: Option<FoldIndicator>,
    pub diff: Option<DiffKind>,
}

fn collect_line_marks(model: &AppModel, doc: &Document, editor: &EditorState, doc_line: usize) -> LineMarks
```

One lane, one slot — lanes are separate columns and never compete, so a heterogeneous `Vec<GutterMark>` with cross-lane priority was the wrong shape; priority resolution exists only inside `mark`. Each source must answer "marks for line N?" in O(log n) or better (sorted-Vec binary search or per-line map — the source's problem). A plain function with one branch per feature; abstract at the 4th–5th source if ever.

**Folding/wrap rule (stated now, needed at Milestone 4):** iterate visible rows, `doc_line = viewport_map.doc_line_for_row(row)`. A mark inside a *collapsed* range renders on the fold header row with the range's highest priority. Under soft wrap, gutter marks render on the first visual row of the logical line (matching diff-gutter.md's rule). Range decorations clip through the same map.

### Range Decorations (text area)

An overdraw pass after text rendering, before cursors, inside `editor_text.rs` (where the private per-line pixel helpers live), iterating only decorations intersecting the viewport:

```rust
pub struct RangeDecoration {
    pub start: (usize, usize),   // (line, char-col), half-open
    pub end: (usize, usize),
    pub kind: DecorationKind,
}

pub enum DecorationKind {
    Underline(u32),
    Wavy(u32),           // draw_wavy_underline from overlay-surface.md — defined once there
    BackgroundTint(u32), // find matches, documentHighlight, bracket match
    Faded,               // diagnostic tag Unnecessary
    Strikethrough(u32),  // diagnostic tag Deprecated
}
```

Rules:

- Decorations never move text — pure pixels on positions the text pass computed. Char-cols go through `char_col_to_visual_col` for tab expansion, exactly as cursors do.
- Ranges are clamped against the current buffer at render time; a vanished line is skipped, never a panic.
- Tints draw first (blended over via `blend_rect_px` — restructuring the text pass for true under-text z-order isn't warranted), then underlines/wavy/strikethrough.

### Scrollbar Overview Marks

Ticks on the vertical scrollbar track: diagnostics, search matches, future diff hunks. Two pieces of new work `editor_scrollbars.rs` doesn't have: a line → track-y mapping (`ScrollbarGeometry` only maps thumb extent today), and a `needs_scroll` guard for the vertical bar (it currently draws whenever the rect exists; ticks must not appear on documents that fit the viewport — or the track rendering gains the guard the horizontal bar already has). One tick per track pixel row, highest priority wins.

### Hit-Testing & Interaction

`HitTarget::EditorGutter` gains `lane: Option<LaneId>`, resolved via `GutterLayout::lane_at`. **Interactive lanes suppress default gutter behavior**: today a gutter press focuses the group and a drag starts text selection — a chevron click must not move the cursor or begin a selection, so `Fold` and `Marks` lane hits consume the press/drag instead of falling through (`runtime/mouse.rs` press and drag branches).

| Lane clicked | Dispatch |
| --- | --- |
| Fold | `FoldMsg::Toggle { line }` (folding doc) |
| Marks (diagnostic) | show the diagnostic hover card (LSP Phase 4); no-op before that |
| Diff | future: hunk actions (diff doc) |
| LineNumbers / `None` | today's behavior (focus, drag-select) |

### Rendering Order & Damage

Within the real pass order (text area, then gutter, per `view/mod.rs`):

```text
text:   current-line highlight → selections → text → range decorations → cursors
gutter: background → lane marks (A, C, D) → line numbers (B) → border
scroll: track (guarded) → overview marks → thumb
```

Damage: decoration changes emit `DamageArea::EditorArea`, full stop (see Non-Goals). Gutter *width* changes additionally trigger the viewport-column recompute. The only fine-grained damage in play remains the existing cursor-blink fast path, untouched.

---

## Consumers

| Feature | Uses | Status |
| --- | --- | --- |
| Find enhancements ([find-enhancements.md](find-enhancements.md), Milestone 2) | `BackgroundTint` + overview marks (no lanes needed) | **Likely first consumer** — needs only Phases 2–3 |
| LSP diagnostics ([lsp-integration.md](lsp-integration.md) Phase 2, Milestone 4) | Marks lane, `Wavy`/`Faded`/`Strikethrough`, overview marks, mark-click → hover | Consumer; drives lanes |
| Diff gutter ([diff-gutter.md](diff-gutter.md)) | Diff lane, overview marks | Planned; replaces its private `GutterLayout` sketch |
| Code folding ([folding-basic.md](folding-basic.md)) | Fold lane + suppressed-default click routing + collapsed-mark hoisting | Planned |
| LSP documentHighlight / bracket match | `BackgroundTint` | Future |
| Bookmarks | Marks lane, overview marks | Future (no design doc) |
| Breakpoints | Marks lane (top priority) | Speculative — supported by the priority scheme, not built for it |

Not consumers: inlay hints / code lens / inline blame (virtual text), indent guides, whitespace rendering. Visual conventions come *from* overlay-surface.md.

---

## Implementation Plan

### Phase 1: Dynamic Gutter Width — *standalone, ships first*

Ships ahead of all consumers as its own change: it fixes the live >99,999-line rendering bug, touches no feature code, and de-risks everything downstream. Most of an M by itself.

- [x] `GutterLayout` (`Copy`, widths-only) in `GroupLayout`; `gutter_width(...)` formula callable from model code.
- [x] Replace **all** `LINE_NUMBER_GUTTER_CHARS` consumers — view-side (`text_start_x_scaled`, `gutter_border_x_scaled`, `geometry.rs`) *and* model-side (`compute_visible_columns`, cursor-visibility sync, `sync_viewports`).
- [x] Viewport-column recompute on width change; tests at 99,999→100,000 (the real digit boundary given min width 5) and (later) first-lane activation.
- [x] Digit-count *width* verified at 1 / 999 / 10,000 / 100,000 lines (formula-level, no frame-render harness exists yet); pixel-identity vs. today for <100,000-line files (min width 5 chars).

### Phase 2: Marks + Decoration Passes — *with first consumer (find or LSP diagnostics)*

- [x] `LineMarks` + `collect_line_marks` through `TextViewportMap`; priority within the marks slot. Built against synthetic state: no feature (bookmarks, LSP diagnostics) has a real mark source yet, so `collect_line_marks` always returns `LineMarks::default()` today — it's wired into the real gutter render loop (called per visible row with the `doc_line` `TextViewportMap` already resolved) so the next consumer adds one candidate-gathering branch, not new plumbing. Priority resolution (`best_mark`, breakpoint > error > warning > info > bookmark via derived `Ord`) is real and tested.
- [x] Gutter mark pass; range-decoration overdraw pass in `editor_text.rs` with clamping and tab-expansion. `render_gutter_mark` draws in the marks lane (a no-op today since `marks_w` stays 0 until a consumer activates it — see Phase 1's `GutterLayout` note). `RangeDecoration`/`DecorationKind` and the overdraw pass are real, threaded through `render_text_area(..., decorations: &[RangeDecoration], ...)`; the production call site in `view/mod.rs` passes `&[]` until a producer exists.
- [x] `Wavy`/severity rendering wired to overlay-surface.md's primitives. `DecorationKind::Wavy` calls `Frame::draw_wavy_underline`; both the marks-lane glyph color and `Wavy`'s caller-supplied color are expected to come from `model.theme.overlay.severity_*`.
- [x] Automation: per-line gutter marks exposed in the editor snapshot (**this phase**, not later — the first consumer's integration tests assert on it). `EditorSnapshot.gutter_marks: Vec<GutterMarkSnapshot>`, populated via `collect_line_marks` over the visible viewport range — empty today, real once a producer lands.
- [~] Unit tests: slot priority, clamping against shrunk documents, collapsed-range hoisting rule (behind folding, when it exists). Slot priority (`src/model/decorations.rs`) and clamping — including a stale-range fuzz sweep and a full `render_text_area` pass with decorations referencing vanished lines (`src/view/editor_text.rs`) — are done. Collapsed-range hoisting is explicitly deferred: folding doesn't exist yet.

### Phase 3: Interaction + Overview Marks

- [x] `lane` on `EditorGutter`; `lane_at` resolution; press/drag suppression for interactive lanes; dispatch table (no-ops until owners ship). `HitTarget::EditorGutter` gained `lane: Option<LaneId>` resolved via `GutterLayout::lane_at`; `runtime/mouse.rs`'s press (left + middle) and the drag-arming check in `handle_mouse_press` all consume/suppress for `Fold`/`Marks` lanes instead of falling through to focus/drag-select. Inert in production today since no lane is ever active (`marks_w`/`fold_w` stay 0), but the suppression logic itself is exercised via synthetic `GutterLayout`s in `src/view/geometry.rs` tests.
- [x] Scrollbar: line→track-y mapping, `needs_scroll` guard, ticks with per-pixel-row priority. `scrollbar::track_row_for_position` maps a line to a track pixel row; `editor_scrollbars::render_overview_marks` draws one tick per occupied row (highest-priority mark wins on collisions), gated by `v_state.needs_scroll()` so ticks never appear on documents that fit the viewport. Called with an empty tick iterator today (no producer).

---

## Testing Strategy

- **Geometry:** lane x-ranges for every active-lane combination; width at digit boundaries; model-side and view-side width agree; hit-test/`text_start_x` consistency at scale factors 1.0 / 2.0; hit-test path allocation-free (`GutterLayout` stays `Copy`).
- **Collection:** slot priority; absent sources → `LineMarks::default()`; O(viewport) by construction.
- **Clamping:** stale ranges skip without panic (fuzz random edits against stale decorations).
- **Interaction:** chevron-lane click neither moves the cursor nor starts a selection; line-number click/drag behaves exactly as today.
- **Manual:** 100k-line file (numbers fit, h-scroll correct); with the first consumer: marks/tints appear, clear, and survive splits; ticks absent when the document fits the viewport.

---

## Acceptance Criteria

- Documents with no decoration state render pixel-identical to today, except line numbers no longer overflow past 99,999 lines.
- Gutter width adapts to lanes and digit count; model-side viewport math (visible columns, cursor visibility) stays consistent, including across width changes mid-session.
- A feature adds a mark or decoration kind by extending an enum and one collection branch — no layout, hit-test, or damage changes.
- Stale ranges clamp or skip; never panic, never draw outside the viewport.
- Interactive-lane clicks do not steal focus into a selection drag.

---

## Design Decisions

| Decision | Options | Chosen | Rationale |
| --- | --- | --- | --- |
| Extensibility | provider registry / plain enums + match | plain enums | Sources are few, known, static |
| Geometry shape | `Vec<(LaneId, Range)>` / `Copy` widths struct | `Copy` widths | `GroupLayout` is `Copy`, rebuilt ad hoc incl. per-mouse-move; no allocation there |
| Collection shape | heterogeneous `Vec<GutterMark>` / `LineMarks` slots | `LineMarks` | Lanes never compete; priority is a marks-slot concern only |
| Damage | new line/group-scoped variants / `EditorArea` | `EditorArea` | Sub-Hz events; the fine-grained variants + render path are unbudgeted renderer work with no measured need |
| Width source of truth | view `GroupLayout` only / model-callable formula | model-callable | `sync_viewports`/`compute_visible_columns` need it with no view in scope |
| Row mapping | bare line indices / `TextViewportMap` | `TextViewportMap` | Already the mandated seam (soft-wrap, folding); bare lines diverge under both |
| Visual conventions | define here / consume overlay-surface.md | consume | One system across gutter, status bar, hover, panel |
| Build timing | with LSP diagnostics / with first consumer, Phase 1 standalone now | Phase 1 now; 2–3 with find-enhancements or LSP, whichever lands first | Phase 1 fixes a live bug consumer-free; find (Milestone 2) must not wait on Milestone 4 |

## Open Questions

1. Marks lane left of line numbers (VS Code style, chosen) vs. right — decide with screenshots at implementation.
2. Diff lane inside vs. outside the fold lane when both are active.

## References

- Consumers: [find-enhancements.md](find-enhancements.md) · [lsp-integration.md](lsp-integration.md) · [diff-gutter.md](diff-gutter.md) · [folding-basic.md](folding-basic.md)
- Conventions owner: [overlay-surface.md](overlay-surface.md) (`draw_wavy_underline`, severity glyphs/colors)
- Viewport seam: [soft-wrap.md](soft-wrap.md) (`TextViewportMap`)
- Code seams: `src/view/geometry.rs` (`GroupLayout`) · `src/view/editor_text.rs` (passes, private pixel helpers) · `src/view/hit_test.rs` + `src/runtime/mouse.rs` (gutter press/drag) · `src/view/frame.rs` · `src/view/editor_scrollbars.rs` · `src/model/mod.rs` (`LINE_NUMBER_GUTTER_CHARS` and its model-side consumers, to be replaced)
