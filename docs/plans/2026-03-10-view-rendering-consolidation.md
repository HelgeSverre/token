# View Rendering Consolidation Plan

**Status:** In Progress
**Created:** 2026-03-10
**Last Updated:** 2026-03-10

## Summary

This plan is the next step after the balanced refactor slices that already landed. It keeps the renderer monolithic where that is useful, but changes what that monolith is responsible for.

The goal is not "smaller files" and not a generic UI framework. The goal is:

- a top-level renderer that mostly does orchestration
- domain renderers that own repeated state and become the natural place to add features
- shared layout types only where render, update, and hit-test must agree
- fewer places where new editor features require copying geometry or draw-order logic
- avoid baking current no-wrap assumptions so future soft wrap can slot into the text pipeline without another broad rewrite

This plan is intentionally conservative. It treats the renderer as an editor UI, not a reusable widget toolkit.

## Implementation Status

### Current Snapshot

- `Phase 1` is complete.
- `Phase 2` is complete.
- `Phase 3` is complete.
- `Phase 4` is in progress.
- `Phase 4.5` through `Phase 5` have not started.

### What Has Landed

- `RenderPlan` and `RenderSession` now drive the top-level render pipeline in `src/view/mod.rs`.
- `EditorGroupScene` and `EditorContentKind` now own editor-group content resolution and local render order.
- `PreviewPaneScene` now owns preview content resolution for hosted vs native preview rendering.
- `DockPaneScene` and `DockContentKind` now own active dock-content resolution for outline vs placeholder dock panels.
- `TabBarLayout` now owns group-tab geometry for both rendering and hit-testing.
- `PreviewPaneLayout` now owns preview header/content geometry for rendering, hit-testing, and hosted webview placement.
- `DockHeaderLayout` now owns dock header/content geometry for rendering, hit-testing, and outline interaction.
- `TextEditorRenderer` now has an explicit visible-line pipeline with staged background, decoration, glyph, and cursor-line redraw paths.

### What Still Remains

- popup geometry contracts are still missing if context-menu work begins before a narrower overlay contract is enough.
- the text renderer still needs future text-viewport seam work for soft wrap readiness and more feature-specific decoration inputs.
- older docs and transitional seams still need a cleanup pass once the architecture settles.

## Decision

### Keep

- `Renderer` as the top-level owner of surface lifecycle, back buffer, damage handling, and draw order
- immediate-mode rendering with `Frame` + `TextPainter`
- editor-specific rendering paths that stay specialized for text, CSV, image, and preview content
- the recent domain abstractions that already proved useful:
  - `WindowLayout`
  - `GroupLayout`
  - `OutlinePanelLayout`
  - `TextEditorRenderer`
  - visible-tree utilities and shared tree traversal
  - `TextFieldRenderer::render_modal_input()`
  - `render_selectable_list()`

### Do Not Do

- no generic widget tree
- no broad trait-heavy GUI framework
- no "split this file because it is large" work by itself
- no new abstraction unless it either:
  - becomes a shared source of truth for layout/ordering/interaction, or
  - owns repeated render state and becomes the obvious place to add real features

## Why A New Plan

The balanced refactor improved some real things:

- tree traversal and visible-row semantics are now shared across render, update, and mouse handling
- outline geometry is shared across render, update, and hit-test
- text rendering now has a real stateful renderer object
- modal input/list rendering has some shared primitives

But the main rendering flow is still weak structurally:

- `src/view/mod.rs` still mixes backend lifecycle, frame setup, damage decisions, layer orchestration, and feature-specific rendering
- full-frame rendering still rebuilds the same `Frame`/`TextPainter` setup for each layer block
- `render_editor_group()` still resolves model state, chooses content kind, and handles draw order inline
- new interactive geometry still tends to start as feature-local math instead of a shared layout contract

The next work should target those problems directly.

## Architectural Principles

### 1. Monolith At The Top, Strong Seams Below

`Renderer` should remain the place where render order is obvious. The top-level file does not need to be tiny.

But it should mostly answer:

- what layers render, and in what order
- which regions are dirty
- which scene/layout objects are needed for this frame

It should not remain the main home for feature-local rendering details.

### 2. Prefer Domain Types Over Generic Framework Types

Good examples in the current codebase:

- `OutlinePanelLayout`
- `TextEditorRenderer`
- shared tree traversal utilities

These help because they match real UI concepts in this editor.

Weak examples would be overly generic "widget" or "component" layers that do not clearly reduce feature work.

### 3. Shared Geometry Must Be A Contract

If render, update, and hit-test all need the same shape or ordering, the geometry must be computed once and reused.

If only one renderer needs a piece of math, keep it local.

### 4. New Features Should Have An Obvious Home

After this refactor:

- new text decorations should land in the text renderer
- new pane content kinds should land in a pane-content scene/renderer path
- new tab interactions should land on a shared tab-strip layout type
- new dock header interactions should land on a shared dock-header layout type

If a new feature still requires editing several unrelated places with copied math, the refactor has not gone far enough.

### 5. Keep Logical Text State Separate From Visual Text Layout

Soft wrap is the clearest future feature that will stress the current rendering model.

Today, much of the editor still assumes:

- one logical document line maps to one rendered row
- `viewport.top_line` is both a scroll position and a render row origin
- pixel-to-cursor conversion can derive line and column directly from row math

New rendering abstractions should avoid hard-coding those assumptions further.

This plan does **not** implement soft wrap now. It does require that new editor-side abstractions leave room for a future shared text viewport model that can own:

- logical-to-visual line mapping
- visual row iteration
- wrapped hit-testing
- wrapped cursor reveal and scrolling
- wrapped gutter presentation
- folded-line visual mapping and reveal behavior

The goal is not to prebuild soft wrap infrastructure everywhere. The goal is to stop making it harder.

## Target Architecture

### 1. Explicit Render Plan In `src/view/mod.rs`

Add an internal frame-level plan/context in `src/view/mod.rs` that computes once:

- window layout
- splitters
- viewport sync
- effective damage
- which layers need redraw
- whether the frame uses the cursor-lines fast path

Possible shape:

```rust
struct RenderPlan {
    window_layout: WindowLayout,
    splitters: Vec<SplitterBar>,
    effective_damage: Damage,
    render_editor: bool,
    render_status_bar: bool,
    cursor_lines_only: Option<Vec<usize>>,
}
```

This is not a new framework object. It is an internal orchestration object for one render pass.

### 2. Single Full-Frame Render Session

For the normal full-render path, construct `Frame` and `TextPainter` once and render layers in order through a single session object.

Possible shape:

```rust
struct RenderSession<'a, 'b> {
    frame: Frame<'a>,
    painter: TextPainter<'b>,
    model: &'a AppModel,
    plan: &'a RenderPlan,
}
```

The cursor-lines fast path can stay separate if that keeps it simpler and faster.

The main win is not performance micro-optimization. The main win is that draw order becomes explicit without repeating setup for sidebar, docks, status bar, modals, and overlays.

#### Damage Ownership By Layer

Phase 1 should also make layer ownership explicit for damage and redraw policy.

Initial rule:

- editor content, editor chrome, sidebar, and standard dock content may participate in future partial redraw
- status bar keeps its own coarse damage region
- modal, popup menu, drop overlay, and debug/perf overlays may continue to force `Damage::Full` until they have stable layout and invalidation rules

The plan does not need a fine-grained damage system now. It does need a stable mapping between render phases and damage ownership so new overlays and high-frequency dock content do not get bolted on ad hoc.

This is especially important for future dock content like an embedded terminal: it is acceptable to start with coarse redraws, but the renderer should leave room for a future `DockArea` or `DockContent` damage bucket without reshaping the layer model.

#### Overlay Policy

Phase 1 should also document a narrow overlay interaction policy so overlay behavior does not become a pile of feature-local exceptions.

Initial rule:

- render order and input precedence for modal, popup menu, drop overlay, and debug/perf overlays are explicit in one place
- modal overlays suppress secondary overlays such as popup menus
- popup menus consume click-away and menu-navigation input before normal hit-test or keymap routing continues
- overlays may intercept input without becoming a new `FocusTarget` unless focus restoration actually becomes a problem

This is intentionally not a generic overlay manager. It is a small editor-specific contract for z-order, consume rules, and keyboard capture.

### 3. Scene Objects For Pane Content

Keep pane rendering editor-specific, but make the content dispatch explicit.

Introduce small scene/state objects such as:

- `EditorGroupScene`
- `PreviewPaneScene`
- `DockPaneScene`

For dock-native content, resolve the active panel content once through a small enum such as `DockContentKind` rather than a trait registry. That keeps terminal, outline, and future dock panels data-first without committing to a generic panel framework.

Pane scenes may also own content that is not painted into the `Frame` directly. For externally hosted child content such as a markdown preview webview, the scene should still own bounds sync, visibility, and lifecycle decisions even if the pixels are not produced by `TextPainter`.

For editor groups, resolve the active content kind once:

```rust
enum EditorContentKind<'a> {
    Text { editor: &'a EditorState, document: &'a Document },
    Csv { state: &'a CsvState },
    Image { state: &'a ImageState },
    BinaryPlaceholder { state: &'a BinaryPlaceholderState },
}
```

`render_editor_group()` should then read more like:

1. build group scene
2. render tab strip
3. render content
4. render scrollbars if applicable
5. apply unfocused dim

That is a real orchestration improvement. It makes new editor view modes cheaper to add and easier to review.

### 4. Promote Shared Layout Types Only Where They Pay Off

Keep the current pattern used by:

- `WindowLayout`
- `GroupLayout`
- `OutlinePanelLayout`

Next candidates should be chosen narrowly:

- `TabBarLayout`
  - tab rects
  - text rects
  - future close-button or hover rects
  - shared by render and hit-test
- `PreviewPaneLayout`
  - header/content split
  - scroll capacity
  - future preview hit targets
- `DockHeaderLayout`
  - title area
  - tab strip region
  - future panel actions
- `GutterLayout`
  - line number area
  - fold indicator lane
  - diff marker lane
  - future gutter hit targets
- `PopupMenuLayout`
  - menu anchor rect
  - menu bounds
  - item row rects
  - shared by render, hit-test, and click-away behavior

This work should not create a generic layout tree. It should create a few strong editor-specific contracts.

### 5. Turn The Text Renderer Into The Home For Text-Layer Features

`TextEditorRenderer` is already the right seam. Build on it.

The next internal cleanup should be about feature extension, not file size:

- define the order of text-layer passes explicitly
  - gutter/chrome decorations
  - line background
  - selection/search/diff/diagnostic background decorations
  - structural guides and bracket or token overlays
  - text glyphs
  - fold placeholders / end-of-line affordances
  - cursors and preview cursors
- make line decoration hooks obvious
- keep coordinate conversion and visibility checks inside the renderer/context
- prefer decoration inputs that come from cached editor/document state rather than direct tree-sitter work in the view layer

This is the path that will make future editor features cheaper:

- search result highlights
- diagnostics/inline problem markers
- indent guides
- whitespace rendering
- current symbol highlight

As part of that work, prefer APIs that can eventually render from a visual-line provider rather than assuming direct `doc_line` iteration is the only model.

### 6. Keep Modal And Panel Code Consolidated, But Less Ad Hoc

`modal.rs` and `panels.rs` do not need to be turned into mini-frameworks.

The rule should be:

- keep the top-level modal/dock dispatch readable in one place
- extract only the pieces that are reused across multiple flows
- do not create one-line wrapper layers

For modals, the shared pieces should stay narrow:

- shell
- input field
- selectable list
- empty-state row / footer row if those repeat

For panels, keep using shared tree and outline geometry where it already pays off.

For tree-style panels, keep sharing visible-tree/query helpers and only grow a shared `TreeNavState` if another real panel needs the same selection, scroll, and collapse semantics.

## Implementation Phases

### Phase 1: Render Plan And Session

**Status:** Completed

**Goal:** make the top-level render path readable and stop rebuilding the same rendering setup for every layer block.

**Files:**

- `src/view/mod.rs`

**Changes:**

- add `RenderPlan`
- add `RenderSession`
- move full-frame render order into explicit phase methods
- define damage ownership by layer and document which layers still force `Damage::Full`
- document overlay policy: z-order, modal suppression, click-away consume rules, and keyboard capture precedence
- keep cursor-lines fast path separate
- do not change feature behavior in this phase

**Expected benefit:**

- clearer orchestration
- easier to add/remove layers
- less boilerplate in the hottest review surface of the renderer
- future overlays and dock content have a clear invalidation story instead of widening `Damage::Full` by accident
- popup menus and future overlays have explicit interaction rules instead of imperative special cases scattered across runtime and view code

### Phase 2: Editor Group Scene

**Status:** Completed

**Goal:** make group rendering data-first and remove inline content dispatch complexity.

**Files:**

- `src/view/mod.rs`
- `src/view/editor_text.rs`
- `src/view/editor_scrollbars.rs`
- `src/view/editor_special_tabs.rs`

**Changes:**

- introduce `EditorGroupScene`
- introduce `EditorContentKind`
- introduce `DockPaneScene` / `DockContentKind` for dock-native content
- resolve group/editor/document/layout once
- define the pane-content seam so content may be software-rendered or externally hosted with scene-owned bounds/lifecycle sync
- render content through explicit scene dispatch

**Expected benefit:**

- one obvious place to add a new editor content type
- less repeated model lookup and branching
- better separation between orchestration and feature renderers
- terminal, preview, and future hosted panes get a concrete home without a generic panel trait system

### Phase 3: Shared Layout Contracts For Tabs And Preview/Dock Chrome

**Status:** Complete

**Goal:** stop adding new interactive geometry as feature-local math.

**Files:**

- `src/view/geometry.rs`
- `src/view/mod.rs`
- `src/view/hit_test.rs`
- dock/panel input paths as needed

**Changes:**

- add `TabBarLayout`
- add `PreviewPaneLayout` if preview interaction needs it
- add `DockHeaderLayout` only if shared by render and input
- add `GutterLayout` once fold indicators or diff markers land
- add `PopupMenuLayout` if context menu work starts before a more general overlay abstraction is justified
- move existing tab-strip math onto the layout type

**Expected benefit:**

- new tab interactions do not duplicate rect math
- render and input stay aligned
- geometry additions become more disciplined without a framework rewrite
- fold controls, diff markers, and popup menus have a single geometry contract instead of feature-local hit math

### Phase 4: Text Decoration Pipeline

**Status:** In Progress

**Goal:** make the text renderer the clear home for future editor features.

**Files:**

- `src/view/editor_text.rs`

**Changes:**

- formalize text-layer pass ordering
- group current line, gutter/chrome elements, selections, structural guides, fold affordances, text, and cursors as deliberate stages
- add internal hooks/helpers for future decorations
- define where cached per-line metadata enters the renderer

**Expected benefit:**

- future features land in one renderer instead of scattering across passes
- less risk of draw-order regressions
- lower change cost for editor-focused work

### Phase 4.5: Text Viewport Readiness

**Status:** Not Started

**Goal:** prepare the text rendering path for future soft wrap and folding without implementing either feature here.

**Files:**

- `src/view/editor_text.rs`
- `src/view/geometry.rs`
- `src/model/editor.rs`
- text hit-test / cursor conversion paths as needed

**Changes:**

- identify and isolate the assumptions that treat logical lines as rendered rows
- introduce a narrow future seam for a `TextViewportModel`, `VisualLineMap`, or equivalent editor-specific abstraction
- keep new text-renderer helpers expressed in terms that could later come from wrapped visual rows
- avoid spreading new direct uses of `viewport.top_line + screen_line -> doc_line`
- leave room for folded visual-line mapping, gutter continuation markers, and reveal logic to share the same viewport abstraction

**Expected benefit:**

- soft wrap and folding become text-viewport projects instead of renderer-wide rewrites
- future text features added before soft wrap are more likely to survive that transition cleanly
- mouse hit-testing and cursor reveal logic have a clearer future integration point

### Phase 5: Cleanup Old Seams And Update Docs

**Status:** Not Started

**Goal:** retire transitional abstractions that no longer earn their keep.

**Files:**

- `src/view/mod.rs`
- `docs/plans/2026-03-10-view-layer-balanced-refactor.md`
- `docs/plans/2026-03-10-view-layer-redesign.md`
- feature docs that still point at stale rendering seams

**Changes:**

- remove thin wrappers that no longer help orchestration
- update older plan docs to reflect what was completed and what this plan supersedes
- update feature docs that still assume old render entry points or broader abstraction directions than this plan intends to support

**Expected benefit:**

- fewer misleading layers
- docs match the actual direction of the codebase

## Explicit Non-Goals

- No generic retained-mode component system
- No "everything is a widget" rewrite
- No splitting `mod.rs` or `geometry.rs` only because of line count
- No large trait-based panel system unless several real panels need the same behavior
- No premature attempt to unify text, CSV, image, and preview rendering behind one abstract renderer trait
- No soft-wrap or folding implementation in this plan; only seam design that keeps those features from forcing another broad text-renderer rewrite

## Success Criteria

This plan is successful if the codebase ends up with these properties:

- `Renderer::render()` reads like a short ordered rendering pipeline rather than a long series of repeated setup blocks
- adding a new editor content type requires editing one scene enum and one renderer path, not several unrelated branches
- adding a new interactive tab/dock geometry uses a shared layout type instead of copied math
- adding fold controls, diff markers, or popup menus uses a shared geometry contract instead of feature-local hit-testing math
- adding a new text-layer feature has an obvious home inside `TextEditorRenderer`
- a future soft-wrap or folding implementation can be introduced through a text viewport abstraction instead of rewriting orchestration, overlays, and hit-testing from scratch
- we preserve a practical monolith at the top without turning the view layer into a framework

## First Slice

Start with Phase 1.

That is the highest-value orchestration improvement and it does not require another architectural bet. It should reduce complexity immediately without forcing more module churn.

## Relation To Existing Plans

- This plan builds on the useful parts of the balanced refactor.
- This plan should replace the redesign doc as the immediate next step.
- The redesign doc can remain as a long-term option only if this consolidation approach stops paying off.
