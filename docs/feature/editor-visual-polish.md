# Editor visual-polish plan

## Purpose and decision

The performance-panel study is a useful visual reference, not a specification
to reproduce mechanically. It made the document area feel calmer and easier to
scan, but it also introduced controls and chrome that Token does not currently
have. This plan identifies the changes that can improve the native editor while
keeping Token's editing geometry, theme system, and existing interaction
contracts reliable.

The intended direction is:

- keep source text legible at the user's chosen editor size;
- give persistent application chrome a quieter, proportional UI voice where it
  improves scanning;
- make the document tabs, document context, editor viewport, and status bar
  read as deliberate, separate regions;
- introduce new chrome only through the documented primitives, rather than by
  baking a performance-panel mockup into the editor renderer; and
- measure each visual change in real themes and real editing states before
  making it a default.

This is a plan only. It authorizes neither a native implementation nor a change
to a user's font, theme, or layout configuration.

Related documents define the proposed chrome primitives:

- [breadcrumbs](../ui/BREADCRUMBS.md) describes the optional context bar above
  an editor viewport.
- [pane chrome](../ui/PANE-CHROME.md) describes the shared top and bottom
  surfaces that carry document context or pane-local information.
- [activity rail](../ui/ACTIVITY-RAIL.md) describes the future slim rail that
  can appear on either workspace edge.
- [dockable panel](../ui/DOCKABLE-PANEL.md) describes a panel that can move
  between a dock and an in-window floating panel.
- [performance panel](performance-panel.md) contains the feature plan for the
  prototype's dock/floating diagnostic content.

None of those documents make breadcrumbs, a lower pane bar, rails, or floating
panels part of every editor by default. The first native visual work can improve
the existing editor without them.

## Evidence and limits

The comparison in [the prototype fidelity investigation](../../prototypes/debug-performance-fidelity.md)
used an isolated Token window and a repeatable production-painter screenshot:
Default Dark, JetBrains Mono and Inter, the same `src/layout/chrome.rs` source,
an explorer, and a right-side Outline dock. The screen was 1398 × 874 logical
pixels at 2× scale; its explorer was 184 logical pixels and its dock was 250.
The original study was measured at 1440 × 1060 CSS pixels, DPR 2.

It establishes the following facts:

| Surface | Current native default at 2× | Original study | Implication |
| --- | --- | --- | --- |
| Editor source | JetBrains Mono regular, 14 logical px, 18.5 logical-px line pitch | JetBrains Mono regular, 11.52 CSS px, 22.464 CSS-px pitch | The study's calmness comes from a smaller glyph-to-line-pitch ratio. It is not evidence that source should become 11.52 px. |
| Explorer | JetBrains Mono, 14 px, 22 px rows | Inter, 12 px | Interface labels can use a separate role, but switching face changes real width and truncation behavior. |
| Document tabs | JetBrains Mono, 14 px | Inter, 11.04 px; nominal `37px` strip minimum | A tab's visual hierarchy and its tab-reveal math must change together. |
| Dock heading | JetBrains Mono regular, 14 px | Inter, 12.8 px requested at 600 | The bundled Inter asset has only a regular 400 face; the browser synthesized 600. Native must deliberately provide a real weight or use size/color/spacing hierarchy instead. |
| Status text | Inter, 12 px | Inter, 9.76 px | The text differs, but that does not tell us the bar's height. |

The study's full status-bar track is **26 CSS pixels**: `.app` has grid rows
`38px minmax(0, 1fr) 26px`. Its `22px` action minimum fits inside that track.
Current native status height is derived rather than hardcoded:

```text
status height = status-text line height + 2 × small padding
```

With bundled Inter at 12 logical px, the current source calculation is 19
logical pixels at both 1× and 2×: at 1×, a 15 physical-pixel text line height
plus `2 × 2` physical pixels of padding; at 2×, a 30 physical-pixel line height
plus `2 × 4` physical pixels of padding, divided by two. `padding_small` is
scaled per side in `ScaledMetrics::new`, so the 2× calculation has 8 physical
padding pixels in total, not 4. The study therefore has a visibly larger lower
bar; its smaller text does not make it a smaller bar. Any proposal to enlarge
Token's status bar must be evaluated as a layout/viewport change, independently
of status typography.

The study declares a nominal `min-block-size: 37px` tab strip. That is not its
actual occupied height: under the original study typography an active tab has
20 px of vertical padding, a 16.56 px inherited line box, and a 1 px top border;
the containing strip also has its bottom border. Its visual outer height is
therefore roughly 38.6 CSS pixels after the child establishes the minimum. The
native tab height is derived as:

```text
tab-bar height = editor line height + 2 × medium padding
```

At 2×, the current 37 physical-pixel source line height plus 16 physical pixels
of padding is 53 physical pixels, or 26.5 logical pixels. The mock's tab strip
is therefore about 10.5 logical pixels taller. That is a valid design option,
not a fidelity defect to erase.

The investigation did not isolate browser text layout from Token's `fontdue`
bitmap rasterization, physical-origin rounding, or per-character advances.
Changing the rasterizer is out of scope for this plan. The first experiments
should vary font roles, sizes, spacing, and surface boundaries independently.

The mock also uses a curated short file tree, simplified syntax highlighting,
less deeply indented code, and an invented breadcrumb/footer pair. Those choices
can make a screenshot look more spacious without improving real dense work. A
native decision must be made with long paths, deep nesting, wrapped text, marks,
multiple splits, and special editor tabs.

## Current native seams

Token already separates some of the work needed for visual refinement:

| Concern | Current authority | Why it matters to this plan |
| --- | --- | --- |
| Embedded/configured faces | `src/view/fonts.rs`, `src/config.rs` | Code and UI have independent font roles and glyph caches. Current defaults deliberately use the code face for explorer, document tabs, terminal tabs, and inputs. |
| Source metrics | `src/view/mod.rs` (`Renderer`), `src/view/editor_text.rs` | Code size is currently 14 logical px. `line_height()` derives a physical metric and rounds up. Text, cursor, selections, gutter, marks, and scrollbars consume those metrics. |
| Tab geometry | `src/layout/editor.rs` (`EditorTabBarLayout`) | One solved layout supplies document-tab paint, hit testing, clipping, wheel targeting, drag geometry, and reveal spans. Its present width policy is character count × monospace advance. |
| Editor viewport | `src/view/geometry.rs`, `src/model/editor.rs`, `src/model/mod.rs` | Viewport rows and columns depend on group bounds, gutter, code metrics, wrapping, find-bar inset, and pixel scroll offsets. `AppModel::resync_viewports` repairs all editor projections after a metric-affecting change. |
| Shell | `src/layout/chrome.rs` | The shared shell reserves editor, dock, sidebar, bottom dock, and status rectangles. It must be extended before a new pane surface claims space. |
| Status bar | `src/model/status_bar.rs`, `src/view/mod.rs` | Its schema and segment layout already use measured native pixels, while the shell owns its rectangle. It is passive today. |
| Theme roles | `src/theme.rs`, `themes/*.yaml` | Tab, sidebar, status, editor, and overlay colors are semantic theme inputs. A polish pass must not replace them with prototype literals. |

The component docs should remain truthful about this distribution. Token does
not have a universal widget trait, a generic breadcrumbs control, or a generic
footer today. `LayoutSnapshot` is the common solved-layout/hit-test mechanism;
the editor viewport and document-tab flow have specialized geometry authorities.

## Visual language to preserve

### Source text is an editing surface

Keep the 14 logical-pixel code default through the first polish phases. Some
users may select another editor font; source geometry must continue to follow
that font's measured metrics. A calmer view should come first from surface
contrast, spacing around chrome, and a separately tested line-pitch choice—not
from silently shrinking all document text to the mock's 11.52 px.

If a later experiment tests a roomier code line pitch, use a proposed trial
range of **20–21 logical pixels at 1×/2× after physical snapping**, not a
browser CSS value copied from the prototype. It should remain a feature branch
or an explicit preview until dense editing checks show a benefit. The eventual
target must be derived from source font metrics and scale factor, not stored as
an unscaled magic number.

### UI chrome may use a different role

Inter is already loaded as Token's UI face and used by the status bar. The
prototype suggests it may also suit labels whose job is navigation rather than
code reading: explorer entries, document-tab labels, dock headings, pane
context bars, and restrained metadata.

That is a proposed role change, not an assumption that every text label belongs
to Inter. Terminal grids and editable text fields need a monospace contract.
The first real-weight hierarchy must use one of these deliberate choices:

1. regular Inter with size, color, casing, and spacing contrast only;
2. add and license/package a true weight asset, then load/cache it explicitly;
3. keep selected headings in JetBrains Mono where a monospaced dashboard voice
   is more useful.

Do not rely on synthetic 600 because browser synthesis in the prototype does
not correspond to the native `fontdue` path.

### Boundaries should say what region the user is in

The prototype's appeal also comes from clear, quiet boundaries:

- tabs state which document view is active;
- a context bar says where the current symbol/file sits;
- the editor viewport is the uncluttered working area;
- a lower pane bar can carry pane-local information without becoming a second
  global status bar; and
- the global status bar remains the shell's persistent application summary.

Use a one-physical-pixel, theme-semantic bottom border on a future breadcrumb
bar. It separates context from source without placing a heavy card around the
editor. It belongs to the context bar's bottom edge, is included in the bar's
reserved height, and is painted/hit-tested from the same solved rectangle.

The phrase **“Keep the document in view. Inspect rendering alongside it.”** in
the performance prototype is a description of docked usage, not an editor
command. In real use, a developer opens the performance panel in the right dock
while the document remains visible; source edits, scrolling, and the panel's
measurements can be correlated side by side. A floating version keeps the same
panel visible above the document when the dock is hidden or too narrow. It does
not mean auto-scrolling the source to keep a specific line in view, and it does
not impose a persistent breadcrumb bar.

## Work sequence

Each phase is intentionally shippable only after its stated evidence is
available. Later phases must not block the first typography/surface experiments.

### Phase 0 — Baseline and evaluation harness

Create a controlled visual matrix before changing native paint:

| Dimension | Required cases |
| --- | --- |
| Theme | Default Dark, a light built-in theme, one high-contrast dark theme, then all built-ins before defaulting a change |
| Scale | 1×, 1.5×, 2× |
| Document | current source excerpt, a deeply indented file, a long-path workspace, a document with many digits/marks, and a wrapped line sample |
| Layout | no workspace, explorer, right dock, bottom dock, one split, and two editor groups |
| State | inactive/active/modified tabs, tab overflow/reveal, find bar, selections/cursors, diagnostics, special image/CSV/binary tabs |

Capture the baseline with the production CPU screenshot path and an isolated
configuration. For every capture record theme ID, scale, window physical and
logical size, dock/sidebar sizes, file and scroll/caret position, font names,
and line/tab/status metrics. The curated prototype source remains a reference;
it is not the only acceptance fixture.

**Acceptance evidence:** the matrix has named, reproducible fixtures under
`target/verification/`, a documented capture command, and no reliance on a
user's live configuration. Before/after images show the same file, source
position, window geometry, and theme.

### Phase 1 — Surface rhythm without new components

Refine existing document tabs, editor background/gutter contrast, sidebar
selection, dock heading, and status-bar colors through existing semantic theme
roles. Start with spacing and hierarchy that can be expressed by current
surfaces; do not add breadcrumbs, a footer, or a rail in this phase.

The explicit study values are reference points only:

| Surface | Current native basis | Proposed experiment |
| --- | --- | --- |
| Document tab strip | 26.5 logical px at 2× from source line height + padding | Trial a taller, deliberately specified chrome height around 32–37 logical px, with its own metrics rather than coupling directly to source line pitch. |
| Status bar | 19 logical px at 1× and 2× | Trial a 22–26 logical-px global bar only if the added vertical cost is justified; keep status text and padding separately tunable. |
| Chrome contrast | each theme's current tab/sidebar/status roles | Adjust tokens/fallback resolution only where each built-in theme maintains readable active/inactive state and borders. |
| Editor code | 14 logical px, metric-derived pitch | Preserve size; do not alter line pitch in this phase. |

A taller tab bar must not be implemented by changing code `line_height`. Give
the tab strip a chrome-specific vertical metric and update the solved tab layout,
its background/border painting, its drag ghost, and all group content origins
together. A taller global status bar must flow through `solve_chrome`, viewport
resizing, and the existing status damage rect.

**Acceptance evidence:** screenshots demonstrate clear boundaries in the
matrix; active/inactive/modified tabs remain readable in all themes; status text
is vertically centred; no scroll/selection/caret rectangle shifts without a
matching updated viewport; existing tab clipping, drag, and status layout tests
remain valid or are deliberately updated with numeric geometry expectations.

### Phase 2 — Typography-role trial

On a separate implementation branch, test Inter for non-editable explorer,
document-tab, and dock labels. Keep source, terminal grids, and editable
controls on their current code-font contract unless a dedicated input design
says otherwise.

This is not a simple font substitution. Current document tabs use character
count × code advance for width. Proportional UI text requires a shared measured
title width and truncation policy. The same measurement must feed:

```text
tab layout → visible clip → paint origin → hit target → drag ghost → active reveal
```

Never count characters to estimate Inter width, and never make paint choose a
different truncation from the solved/hit layout. A long Unicode file name,
combining mark sequence, emoji, external-change suffix, and save-error suffix
must be measured and clipped by the same renderer role. The active-tab reveal
calculation must re-run after title, font, scale, group width, or tab order
changes.

The file tree needs the same discipline: row geometry remains its existing
physical metric, but selection hit rectangles stay full-row regardless of label
width. The proposed label face must not make nesting, disclosure controls,
icons, clipping, or keyboard auto-reveal derive independent x coordinates.

Use real asset availability as a gate for hierarchy. Phase 2 may use regular
Inter and visual contrast; it may not request a synthetic semibold. If a true
weight is chosen, first add an explicit asset/source/licensing decision and
separate cache/role support, then remeasure every affected role.

**Acceptance evidence:** measured width tests cover mixed-width labels; document
tab click/wheel/drag/reorder/reveal uses one geometry source; explorer selection
and auto-reveal remain correct after font/scale change; native captures compare
regular Inter hierarchy with current monospace hierarchy across the matrix.

### Phase 3 — Code line-pitch trial

Only after the chrome role trial is evaluated, test a separate source line-pitch
configuration. The purpose is to decide whether Token benefits from some of the
prototype's vertical calm without losing the user's preferred code size.

The candidate must be represented as a **physical, scale-aware line box**. It
cannot be a CSS-style multiplier applied in one paint loop. It changes all of
the following shared calculations:

- viewport visible-row count and row/pixel conversion;
- vertical scrolling, wheel accumulation, drag autoscroll, scrollbar thumb and
  track mapping;
- wrapped visual rows, folded ranges, ghost text, inline suggestions, find
  matches, diagnostics, and indentation guides;
- gutter line-number and marks alignment;
- selection rectangles, cursor height/blink damage, IME/text-input caret
  rectangle, hover placement, and hit testing; and
- every plain-text fast path gated by `EditorState::is_plain_text_mode()`.

Special tabs (image, CSV, binary, Markdown/HTML preview) must stay outside the
plain-text code path. A change that works only for an unwrapped Rust file is not
ready. `AppModel::resync_viewports` must run at every font/scale/line-pitch
transition, and scroll position must preserve the logical visible content as
defined by the chosen transition policy rather than jumping because a stale
pixel offset was reused.

**Acceptance evidence:** targeted tests and manual captures cover wrapped and
unwrapped selection/caret/hit testing, IME rects, folds, ghost text, find bar,
diagnostics/marks, scrollbar position, high DPI rounding, horizontal scroll,
and all special tabs. A controlled reading/editing review records why the
selected pitch is better than both current default and the 20–21 logical-px
trial bounds.

### Phase 4 — Adopt pane context and lower-pane chrome

After the component docs and gallery specimens are accepted, implement the
optional pane surfaces rather than inventing feature-local strips:

- a breadcrumb/context bar above a compatible text editor, with a thin bottom
  border and optional leading icon;
- a lower **pane footer** for compact, pane-local passive context; and
- a clearly separate global status bar for application/document-wide segments.

The context bar is appropriate for a path plus current symbol, such as
`src › layout › chrome.rs › solve_chrome`. It answers “where is this text?” and
is not a duplicate tab strip, a command palette, or a mandatory line of chrome
for every editor. An adopting feature must choose one documented behavior:
passive labels with no input targets, focus, or capture; or full keyboard and
pointer navigation as specified by Breadcrumbs. Do not ship a hidden middle
state where crumbs are clickable but cannot be reached or understood by keyboard.

The lower pane footer has a defined, borrowed passive-content contract:
`PaneFooter` has independent leading and trailing slots composed from
`PaneFooterRun` values. `HintFooter` and `StatusFooter` are simple adapters for
common leading/trailing projections; they do not own state, timers, or status
messages. A pane footer can therefore show a preview mode or narrow contextual
hint without absorbing global status-bar notifications.

Both bars reserve their own layout rectangles. Viewport, tab, find-bar, and
split geometry must derive from those rectangles rather than subtracting
independently guessed heights. The same snapshot/geometry must control paint,
clipping, and hit testing. Passive initial versions have no focus/capture state;
interactive crumbs or actions need a separately specified input contract.

**Acceptance evidence:** new gallery specimens and component tests exercise
absent/present path, overflow/truncation, optional icon, one/two groups, narrow
split, resize, theme/scale, and non-text/special-tab policy. The bottom border
is exactly one snapped physical pixel at each tested scale and no gap/overlap
appears at the editor edge.

### Phase 5 — Dockable diagnostics and future rails

Implement the performance panel from its feature plan using the dockable-panel
primitive. It may have a title and optional action buttons, dock right or bottom,
and become an in-window floating panel while retaining stable panel identity and content
state. The initial performance implementation deliberately omits the mock's
live/pause/reload header controls and its leading header icon.

An activity rail is deferred. The prototype's right rail is a future navigation
primitive, not a necessary performance-panel dependency. If introduced later,
it should support left or right placement by the same edge/selection semantics
and avoid stealing editor width unexpectedly.

**Acceptance evidence:** dock, float, redock, close/reopen, resize, and focus
transitions preserve panel state by stable identity; title/action chrome is
present only when supplied; no left/right rail is required by the panel; the
document remains visible and independently editable beside a right dock; charts
are derived only from actual completed rendered-frame history.

## Implementation guardrails

1. **Do not replace the theme system with a prototype palette.** New surface
   roles need explicit theme fields or documented fallbacks, with all built-in
   YAML themes verified. Avoid accidental reuse of a bright status color for a
   passive pane footer simply because it is convenient in Default Dark.
2. **Do not conflate logical, physical, and browser CSS pixels.** Metrics enter
   at a named scale boundary. Snap adjacent edges together through the shared
   snapshot helper; do not round origin and width separately.
3. **Solve geometry once.** Document-tab and shell changes must be consumed by
   paint, hit testing, clipping, scrolling, and damage from the same layout
   authority. The performance panel should extend shared stages in `src/perf.rs`
   rather than create a second instrumentation vocabulary.
4. **Preserve stable identity.** Tabs keep `TabId`; dock panels keep `PanelId`;
   a floating/docked view does not re-create its underlying state. Indexes are
   projections that must be repaired after removal/reorder.
5. **Keep current user choices.** A visual default change must not overwrite
   configured `editor_font`, `ui_font`, editor text preferences, or theme.
   Configuration migration requires a separately documented compatibility
   decision.
6. **Keep editing mechanics testable.** Font/metric work must resync viewports
   and validate caret, selection, input method, scroll, wrapping, folding, and
   special tab behavior before screenshot polish is treated as evidence.
7. **Do not turn a passive bar into a hidden interaction surface.** If the
   context bar/footer later gains actions, specify keys, focus, pointer capture,
   disabled state, keyboard navigation, and cancellation in its component doc.

## Verification and release bar

For a small visual-token or painter adjustment, run the directly affected unit
and layout tests, capture the controlled before/after matrix, run `just fmt`,
and inspect `git diff --check`. For changes that alter shared editor metrics,
tab geometry, shell layout, themes, or input/scroll projection, treat them as
substantial: run targeted tests while iterating, then `just fmt`, `just test`,
and `just lint` before handoff.

Each visual review should answer these concrete questions:

| Review question | Evidence |
| --- | --- |
| Is the result calmer without reducing code legibility? | Same text size, same zoom, side-by-side native captures and a short editing pass on dense/wrapped code. |
| Do tabs still behave exactly like tabs? | Solved geometry tests plus click, wheel, drag/reorder, active reveal, close, resize, and font-change cases. |
| Does the status bar have intentional weight? | Measured bar/text heights at 1×, 1.5×, and 2×, plus viewport line-count change. |
| Do all themes retain hierarchy? | Capture all built-in themes, including light themes; inspect active/inactive tabs, borders, gutter, sidebar selection, and status text. |
| Did source editing stay correct? | Cursor/selection/IME/hit/scroll/wrap/fold/diagnostic/special-tab checks against the same layout metrics. |

Do not claim a performance improvement from the debug build or the F2 overlay:
the overlay forces full redraw while visible. Its plan is useful for stage
diagnosis, not release-equivalent frame-rate comparison.

## Explicitly deferred

- Replacing `fontdue`, changing text antialiasing, or drawing conclusions about
  rasterizer quality from browser comparison.
- Enabling synthetic bold as a substitute for a packaged, verified font weight.
- Making breadcrumbs, pane footers, activity rails, or floating panels universal
  editor chrome before their documented component contracts are implemented.
- Copying the mock's live/pause/reload header actions or leading performance
  icon into the first native performance panel.
- Treating the mock's curated source/tree density as an editor-layout benchmark.
