# Performance panel

> **Status:** Planned. This document specifies a replacement for the debug-only
> F2 floating overlay; it does not authorize a native implementation yet.
>
> **Default presentation:** a tab in the right dock. **Second presentation:** a
> floating panel inside Token's window, using the same data projection and body.
>
> **Scope boundary:** use actual renderer instrumentation only. The user has
> asked to omit the study's top pause/live/reload controls and decorative header
> icon. Synthetic workload/seed controls have no native data source; a target
> budget is deferred until its source and semantics are specified.

## Purpose

The current F2 surface is useful for answering a narrow diagnostic question:
which work happened during recent rendered frames, and where did time go? It is
not a release-performance benchmark. It is a manually positioned 500-pixel
debug overlay drawn after normal content, so it hides the document a developer
is trying to inspect and makes all timing include the cost of showing itself.

The new Performance panel keeps the document visible alongside a right-docked
inspection surface. Its floating form is for temporarily comparing a local
area of the editor with the measurements without moving the panel to another
window. Both forms render one domain-owned projection; switching presentation
must never create a second timer, history, cache counter, or stage taxonomy.

The first release is a diagnostic panel for debug builds. It reports *rendered
frame duration* and instrumented render/runtime work, with its units and
limitations visible in the empty/summary states. It does not claim wall-clock
application FPS, display refresh rate, input latency, CPU utilization, or
release-equivalent performance.

## Current evidence and constraints

### Instrumentation that exists today

`src/perf.rs` owns `PerfStats` in debug builds and makes its measurement/history
methods no-ops in ordinary release builds. The `profile-tracing` feature is the
exception: its release-shaped methods still enter tracing spans. Its fixed
`PerfStage::ALL` array contains 30 ordered stages,
including layout/build-plan, editor passes, all three dock phases, overlays,
surface acquire/copy/present, and webview work. `PerfStageSpec` currently owns
the human label, short label, and visual color for each stage.

At the start of each `App::render`, `src/runtime/app.rs` records an `Instant`.
The runtime calls the renderer, webview visibility work, text-input rectangle
synchronization, and webview synchronization; it then calls
`record_frame_time()` and `record_render_history()`. The latter appends one
sample for every `PerfStage` and untracked time, retaining at most
`PERF_HISTORY_SIZE = 60` samples. A stage sample is a `Duration` accumulated
within that rendered frame. A missing/zero stage is still represented as a zero
in history; `visible_stages()` only hides stages that are zero both now and
throughout retained history.

The existing summary values are available with precise meanings:

| Value | Current source | Correct display meaning |
| --- | --- | --- |
| Latest frame | `last_frame_time` | elapsed time from the runtime's `start_frame()` to just before history recording for the latest rendered frame, in ms |
| Mean rendered frame | `avg_frame_time()` | arithmetic mean of retained rendered-frame durations, in ms; denominator is `frame_times.len()`, not 60 when history is warm-up/empty |
| Render throughput | `1 / avg_frame_time()` | reciprocal of that mean in renders/s; a derived throughput estimate, not monitor FPS |
| Stage latest / mean | `stage_time()` / `stage_history()` | accumulated duration for one named stage in the latest frame / mean over that stage's actual retained history length, in µs or ms |
| Tracked / untracked | sum of current stages / `last_frame_time - tracked` | instrumented time and the non-negative remainder of the latest frame; they are not guaranteed to be mutually exclusive work categories outside the measured boundaries |
| Cache hits/misses | `TextPainter` cache statistics supplied each frame | glyph-cache lookup counters; the existing percentage uses process-lifetime `total_cache_hits / (total_cache_hits + total_cache_misses)`, not a per-frame ratio |
| Cache entries | `TextPainter::glyph_cache_size()` | current glyph-cache entry count, sampled while rendering; it has no historical series today |

There is no timestamped wall-clock sample, budget value, live/paused capture
state, user-selected history duration, per-stage cache, or per-frame cache-rate
history. Median and p95 *can* be derived from the existing bounded
`frame_times` window; they need documented sorted-window conventions, not new
instrumentation. The prototype's over-budget, wall-time distribution, and gauge
views cannot be copied as facts until a separate instrumentation phase records
the required inputs and defines their denominators.

`Frame::draw_sparkline` already paints duration histories with a clipping/bounds
test, but it is used only by the old overlay. This supports a feature-local
time-series helper in the first implementation; it is not evidence that Token
needs a universal chart framework.

### Why the old overlay changes the thing it observes

The F2 key currently toggles runtime-owned `PerfStats::show_overlay` in debug
builds. `Cmd::TogglePerfOverlay` maps to `Damage::Full`; while visible,
`build_render_plan` cannot take the normal cursor-lines fast path, and the
renderer paints `render_perf_overlay` near the end of the frame while timing it
as `PerfStage::PerfOverlay`. The panel must preserve this caveat in its empty
and summary copy: opening F2 forces full redraw, so visible samples diagnose
render stages but are not comparable to normal damage-limited rendering.

A right dock will itself be measured by the existing `RightDock` stage; a
floating panel will need a named rendering stage before it can report its own
cost. The first rollout must label each sample by the observation mode latched
when that sample completed. Opening the panel first draws a history that may
have been captured while hidden, so it must not relabel those entries “with
Performance panel open.” If a retained window mixes Hidden, DockOpen, and
FloatingOpen samples, its summary says so rather than implying one comparable
condition. No instrumentation should time the panel by starting a second ad-hoc
timer: extend `PerfStage` and retain its one shared stage list.

### Existing layout and ownership seams

Clay-inspired layout is already the geometry authority. `layout/chrome.rs`
declares a root tree and solves it into a `LayoutSnapshot`; `layout/keys.rs`
provides typed identities used consistently by paint, hit test, and update
logic. `DockLayout` owns durable dock placement and active tab state, while
`UiState` owns focus and resize capture. `view/panels.rs` paints right/bottom
generic dock chrome and active content from the same `LayoutSnapshot`.

Today `PerfStats` lives in `runtime::App`, while a normal panel renderer is
called with `AppModel` and layout geometry. That split is intentional. The
implementation must add a borrowed per-frame performance projection at the
render seam (or another explicit read-only handoff); it must not duplicate the
60-frame history into `AppModel` just so a panel can paint it. Dock/floating
presentation state belongs in model/UI state only after its lifetime and
persistence rules are specified by the dockable-panel work.

## First-release content contract

The panel consists of a standard panel title supplied by its host chrome and a
passive, scrollable body. The host supplies the title “Performance” and may
supply standard Dock / Float / Close actions once that generic chrome exists.
For the ordinary single-tab dock form, host chrome owns the one visible title;
the body does not repeat a second “Performance” heading. The performance body
itself supplies no header icon and no pause, live, reload, workload, seed, or
budget controls.

At a useful right-dock width, the body is ordered as follows:

1. **Rendered frame history.** A line/area history of completed rendered-frame
   samples with the current `frame_times` semantics, fixed to the retained
   sample count rather than a fake number of seconds.
   The x-axis must say `N rendered frames` (for example `43 of 60 samples`),
   because samples are not guaranteed to arrive at regular wall-clock
   intervals. The readout shows latest, mean, median and p95 in ms. Mean,
   median and p95 all state `N` as their retained rendered-frame denominator.
2. **Stage breakdown.** A current-composition bar followed by one row per
   completed-view visible stage, in shared `PerfStage::ALL` order (or an
   explicitly documented presentation sort that still retains stable stage
   identity). Each row has the registry-owned label, theme-resolved stage
   color, latest duration, mean duration with the stage history denominator,
   and a compact duration history. “Untracked” is a
   derived extra row when at least one completed sample has a composable
   untracked duration; it is not added to `PerfStage`. Overlap samples are gaps
   in that row's chart, never valid zero-duration points.

   The composition bar is drawn only when latest stage totals are no greater
   than the completed frame duration. Its segments are latest non-zero stage
   durations plus the actual `frame - tracked` remainder. If stage totals
   exceed the frame because stages overlap or nest, the bar is omitted and a
   neutral explanation says that overlapping stage timings cannot be composed
   into one fraction-of-frame bar. Rows retain their truthful durations; no
   clipping, normalization or fractional-pixel remainder is presented as
   untracked time.
3. **Renderer cache.** Current glyph-cache entries, lifetime hits, lifetime
   misses, and the existing lifetime hit rate. Its labels say “lifetime” so a
   user cannot infer a current-frame percentage. With zero lifetime lookups it
   displays `—` and “No lookups yet”, rather than an ambiguous 0% or
   unavailable state.

An empty projection contains no invented zero performance claim. Before the
first completed rendered frame it says that no rendered-frame history is
available yet. A zero-duration stage is hidden by the completed-view visibility
rule; an empty list of visible stages still permits a frame history and cache
section.
If the panel is narrower than its chart/readout minimum, its body uses the
shared scroll-area contract or a compact textual fallback; it must not silently
derive a second geometry system.

### Scales and units

Duration rows format exact values with a unit appropriate to magnitude (for
example `340 µs` or `1.4 ms`) and use the same underlying `Duration`. The
frame-history plot has one shared y-scale derived from its retained frame
samples. A stage sparkline is independently scaled to that stage's retained
history and labels it as such; sharing the frame scale would make small but
meaningful stage changes unreadable. It must not make visual height across two
different stage rows look like a quantitative comparison without their printed
values.

The first release has no gauge. A “headroom” gauge is meaningful only after a
defined target (such as an explicitly selected target frame duration) and its
source/persistence/semantics have been implemented. Histogram or wall-time
distribution views remain future work rather than simulated styling.

### Retained-window statistics

For completed samples with a frame duration, retain their actual
oldest-to-newest order for plotting, then sort a copy of only their frame
durations ascending as `s`, with `n = s.len()`, for the summary:

```text
median = s[n / 2]                                      if n is odd
         low + (high - low) / 2                         if n is even,
         where low = s[n / 2 - 1], high = s[n / 2]

p95_rank = min(n - 1, max(0, ceil(0.95 × n) - 1))
p95      = s[p95_rank]
```

Median is conventional: an even pair is averaged without adding the two
durations first, and any half-nanosecond is rounded down. P95 uses nearest-rank
and is deliberately not interpolated. Both use the retained completed-frame
window, not a wall-clock interval. Example: sorted `[1, 3, 5, 9] ms` has
median `4 ms` and p95 `9 ms`; sorted `[2, 8, 10] ms` has median `8 ms` and p95
`10 ms`. Empty history has neither summary and renders the empty state.

## Proposed data boundary and algorithms

The following is an architecture sketch, not a current type. It shows the
single-domain-owner rule and the data needed by paint without handing a vague
`state` bag to the view.

```rust
// proposed API — all durations are sampled renderer/runtime work, not wall time.
struct CompletedPerfView<'a> {
    // Oldest to newest; N <= PERF_HISTORY_SIZE. This is the one source for
    // every plotted/statistical value. `history.back()` is the latest sample.
    history: &'a VecDeque<CompletedFrameSample>,
    stages: Vec<StageProjection<'a>>, // derived from PerfStage::ALL once
    untracked: Option<HistoryProjection<'a>>,
}

struct CompletedFrameSample {
    sample_id: u64, // monotonically increasing; survives VecDeque front removal
    frame: Duration,
    stage_times: [Duration; PerfStage::COUNT],
    untracked: Option<Duration>, // None when stage totals exceed frame
    glyph_cache_entries: usize,
    lifetime_cache_hits: usize,
    lifetime_cache_misses: usize,
    observation: ObservationMode, // Hidden | DockOpen | FloatingOpen at finalization
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ObservationMode { Hidden, DockOpen, FloatingOpen }

struct StageProjection<'a> {
    stage: PerfStage,              // identity; spec owns display label/color
    samples: &'a VecDeque<CompletedFrameSample>,
}

struct HistoryProjection<'a> {
    label: &'static str, samples: &'a VecDeque<CompletedFrameSample>,
}

// The plot does not demand contiguous &[Duration] storage. Every retained
// offset occupies an x-slot; None is a gap. sample_id identifies a hover/readout
// target even after older samples are evicted from the VecDeque front.
trait DurationSeries {
    fn len(&self) -> usize;
    fn sample_at(&self, offset: usize) -> Option<(u64, Option<Duration>)>;
}
```

`PerfStats::completed_view(...)` (or an equivalently named single constructor)
owns the filtering and derives all summaries from its inputs. The collector is
field-split into the mutable current-frame accumulator and one immutable,
bounded completed-sample history owned by that same `PerfStats`; it is not a
second collector. Phase 1 replaces the panel-facing parallel duration queues
with this append-once `VecDeque<CompletedFrameSample>` (or derives legacy
accessors from it), so frame, stage, untracked, cache, and observation values
cannot drift by one sample.

After all current-frame timing guards and cache counters are recorded, the
runtime finalizes frame `k` by recording its duration/history and appending one
fully populated `CompletedFrameSample`, including its latched
`ObservationMode`. During paint of frame `k+1`, both docked and floating
renderers receive that immutable completed view. They never pair partially
accumulated stage `k+1` values with frame `k` duration/history, or apply the
current host's mode to hidden history captured before the panel opened.

The cache-entry count and lifetime hit/miss totals are latched into the same
completed sample at finalization. The render seam therefore needs a narrow way
to report the painter's cache-entry count before publishing `k`; it must not
ask the painter for a changing value while the panel body is being painted.
A later snapshot/export API can own copies deliberately, but it cannot become a
second mutable collector.

For a history `h` with `n = h.len()`:

```text
mean(h) = 0                                if n = 0
          floor(sum(h[i].as_nanos()) / n)  otherwise

// Sum no more than PERF_HISTORY_SIZE Duration nanosecond values in u128,
// then reconstruct the result as a Duration. Do not add Durations directly
// or convert to floating point for the mean.

cache_rate = no lookups yet                 if hits + misses = 0
             100 × hits / (hits + misses)  otherwise

untracked = latest_frame - sum(latest_stage_times)
            only when sum(latest_stage_times) <= latest_frame
            otherwise no composable remainder
```

For each `stage` in `PerfStage::ALL`, the completed view includes it when its
latest completed value is non-zero or any value in its completed sample history
is non-zero. This reproduces the useful visibility semantics of today's
`visible_stages()` without consulting the mutable current-frame accumulator.

Every completed sample supplies a stage value, and its untracked field is
`Some(duration)` only for frames whose stage total is composable. That gives
untracked a same-length `Option<Duration>` series: gaps consume their x-slot
and break the plotted line, rather than inheriting today's zero value from
`untracked_history`. Stage/value summaries use the actual count of eligible
values; frame mean/median/p95 use the actual completed-frame count. No row
prints a mean when its eligible denominator is zero: frame rows use `—` and
“No completed samples”; an untracked row uses `—` and “No composable samples.”
All means floor integer nanoseconds as specified above, and their labels state
the applicable denominator. The projection uses `history.len()` rather than
assume 60: startup and clear-history leave it short. Composition uses the
latest completed durations and frame, not an average divided by a selected plot
window.

The feature-local plotting helper receives a solved content rectangle in
physical px, a `DurationSeries` (implemented through `VecDeque::get` or its
two slices), an explicitly selected y-domain, foreground/background colors,
and a clipping rectangle. It preserves stable `sample_id` values for hover and
readout, and treats `None` as a visual gap. It performs no sampling, sorting,
timing, model mutation, or input dispatch. If a second non-performance feature
needs the same contract, extract a documented `TimeSeriesPlot`; until then,
keep it under the performance feature alongside its tests. Metric text reuses
existing label/group-header/status/badge primitives where their documented
semantics fit; no general `Chart` primitive is justified now.

Current `PerfStageSpec` colors are hard-coded ARGB values. The new body must not
copy that registry into a second stage-color list. Its display resolver is
keyed by the existing `PerfStage` identity and derives a contrast-safe series
from the active Token theme palette. The old hard-coded color remains an
implementation detail of the old overlay until that painter is removed; labels
and short labels remain source-owned by the existing stage registry.

## Presentation transitions and ownership

The intended final docking/floating primitive is specified separately in
`docs/ui/DOCKABLE-PANEL.md` and `docs/ui/PANE-CHROME.md`. This feature consumes
that contract rather than inventing another overlay manager.

| State | Event | Reduction/effect | Result |
| --- | --- | --- | --- |
| hidden | debug command/F2 | activate/open `PanelId::Performance` in its remembered/current host; request layout/redraw | first use defaults to right dock; a previously floated panel returns in its remembered host |
| right dock active | generic Float action | move one panel presentation to floating; preserve its performance-domain identity | a floating in-window panel paints the same projection |
| floating visible | generic Dock action | return it to the remembered/current dock | same shared stage history remains visible |
| dock or floating visible | generic Close action or F2 | hide the presentation and return focus according to generic chrome rules, retaining host choice | collector continues; no fake “paused” state appears |
| any | render with no history | projection has `history.is_empty()` | informative empty state, no zero-valued chart |

The selected policy is to continue the existing debug collector while the panel
is hidden, so opening it can show the latest retained frames. It must not
schedule frames merely to collect data. F2's old `Damage::Full` behavior is
removed only after the runtime no longer requests it for a hidden/closed panel.

`PanelId::Performance` is a candidate new enum identity with default
`DockPosition::Right`. It consumes the agreed [Dockable panel](../ui/DOCKABLE-PANEL.md)
contract: docked membership means it occurs exactly once in `DockLayout`; a
floating or hidden entry occurs exactly once in `NonDockedPanels` and in no
dock, carrying its return slot/last presentation. The shared contract owns
focus, drag/resize capture, logical bounds, persistence and restoration. This
feature supplies only its domain projection and placement intents; it does not
ship a bespoke floating overlay.

## Implementation sequence

### Phase 0 — confirm the agreed contract and add static chrome helpers

- Confirm the agreed [Dockable panel](../ui/DOCKABLE-PANEL.md) and
  [Pane chrome](../ui/PANE-CHROME.md) contracts at implementation start; do not
  reopen their membership, persistence, focus or Escape product decisions.
- Add production chrome helpers and gallery tiles for docked title/actions and
  static pane-header/footer packing. Full floating layout/painter specimens
  belong to Phase 3, with the real generic floating implementation.
- Activity rails remain deferred and are not a dependency for F2 command
  placement.

### Phase 1 — make the existing collector renderable without changing meaning

- Keep `PerfStats` as the sole collector and `PerfStage` as the sole stage
  registry. Field-split current accumulation from a published completed-frame
  latch/history, then extract a read-only `CompletedPerfView`. Unit-test its
  empty, partial-history, zero-cache, untracked, percentile and warm-60
  behavior.
- Move only reusable duration/statistical formatting and feature-local plot
  geometry out of `render_perf_overlay`; do not preserve its fixed top-right
  geometry or duplicate its bespoke layout.
- Decide and add a named stage for panel/floating chrome painting if the
  measurements are expected to explain their own observer cost. Do not create
  overlay-only timers.

### Phase 2 — right-dock Performance panel

- Add the panel enum registration/default position and Clay layout declaration
  through the generic dock path. Pass the borrowed projection at the render
  seam to `view::panels` (or its feature-local renderer); leave domain data in
  `PerfStats`.
- Render the three first-release sections with `LayoutSnapshot` rectangles,
  proper clips, registry-owned stage labels/theme-resolved colors and the real
  60-frame history.
- Add the debug command/keybinding route through normal command/update flow so
  it opens/focuses the panel rather than toggling an independently drawn F2
  overlay. Remove the old painter only after equivalent real data is visible.

### Phase 3 — implement generic floating presentation

- Make Performance the first consumer of the generic dock/float/close title
  affordances. The floating renderer invokes exactly the Phase 2 body renderer
  with another solved content rect and the same projection.
- Verify move/dock/close, pointer capture cancellation, clamped geometry,
  focus return, resize and scale-factor changes. There is still no
  Performance-specific title icon or toolbar.

### Phase 4 — only after real use reveals a need

- Consider an explicitly configured target/budget, histogram/wall-time
  distributions, cache history, exports, or a reusable time-series primitive
  only with a real data-source and denominator design.
- Do not add activity rails as a side effect. They are a separate shell feature
  whose placement on one/both sides remains deferred.

## Risks and mitigations

| Risk | Mitigation |
| --- | --- |
| The observer changes frame cost | Time the panel through the shared stage list; state `DockOpen`/`FloatingOpen` observation mode and retain the F2/full-redraw limitation in visible copy. |
| Samples look time-uniform but are render-triggered | Label the x-axis by rendered-frame count, not seconds, until timestamped samples exist. |
| A warmed-up cache/lifetime rate is mistaken for current health | Label cache counters/rate “lifetime”; do not draw a cache-rate chart without per-frame history. |
| Docked and floating panels drift | One `CompletedPerfView` constructor and one body renderer accept a host-provided content rect. |
| The feature recreates manual geometry | Use Clay declarations/`LayoutSnapshot` keys for body and generic pane chrome; share those rects with input. |
| Performance instrumentation becomes invasive | Instrument only named renderer/runtime boundaries already staged; no per-glyph/row timers and no collection-driven redraw scheduler. |
| Debug-only types leak into release behavior | Preserve the existing ordinary-release no-op `PerfStats` measurement contract and compile gates, while retaining the intentional `profile-tracing` span exception; ensure release command/panel registration follows the chosen availability policy. |

## Verification plan

The following are proposed implementation acceptance cases, not claims about
tests that already exist. This documentation-only change adds no Rust test code.

1. **Projection units:** construct histories with 0, 1, 3 and 60 samples;
   assert mean uses actual `len`, no division by zero, duration formatting is
   correct around the µs/ms boundary, and no history yields the empty state.
2. **Median and p95:** sorted `[1,3,5,9] ms` produces conventional median
   `4 ms` and nearest-rank p95 `9 ms`; `[2,8,10] ms` produces `8 ms` and
   `10 ms`. An empty window produces neither readout. An even-duration pair
   verifies `low + (high - low) / 2`, and a near-maximum duration history
   verifies the mean uses bounded `u128` nanoseconds rather than direct
   `Duration` addition or floats.
3. **Completed-frame coherence and mode:** after hidden frame `k` finalizes
   with frame `10 ms`, stage `A=2 ms`, cache entries 100 and 9/1 lifetime
   lookups, opening and painting frame `k+1` reads exactly those hidden-mode
   values even while its own DockOpen stage timer advances. After `k+1`
   finalizes, paint of `k+2` first sees its DockOpen sample. The retained
   sample sequence preserves both modes rather than relabelling `k`.
4. **Composition:** latest frame `10 ms`, stages `2 ms` and `3 ms` produces
   20%/30% stage segments and `5 ms` untracked remainder. A `10 ms` frame with
   stage totals `6 ms + 7 ms` renders no composition bar and the overlap
   explanation; its untracked series has `None` at that sample's x-slot, never
   a negative/zero fake remainder. A zero latest frame produces no division and
   a textual no-sample/zero-frame form.
5. **Stage authority:** a stage newly added to `PerfStage::ALL` appears in the
   projection/body without a second panel list; an all-zero stage remains
   hidden until its current or retained history becomes non-zero.
6. **Cache semantics:** `(hits, misses)=(0,0)` displays `—` and “No lookups
   yet”; `(99,1)` displays 99.0% lifetime rate and does not label it
   “this frame.”
7. **Clay geometry:** at 1x and 2x, assert `PanelContent(Performance)` and its
   chart/list clips remain within the right dock; narrow widths select the
   documented compact/scroll form. Reuse `tests/chrome_layout.rs` style numeric
   vectors rather than hand-derived view geometry.
8. **Interaction:** first command/F2 opens and focuses the right dock; after a
   Float transition, close/F2 preserves the remembered floating host;
   close/dock/float transitions preserve history, repair focus and clear
   pointer capture on release, cancel and focus loss. A closed panel must not
   force a collection redraw.
9. **Visual evidence:** add a labelled native gallery specimen for empty,
   partial, populated and narrow states, plus docked/floating pane chrome.
   Capture deterministic native screenshots under `target/verification/` in
   Default Dark and GitHub Light. The gallery proves paint; interaction tests
   cover pointer and keyboard transitions.
10. **Instrumentation regression:** compare debug traces before/after under a
   fixed interaction script, documenting that visible panel samples include
   panel cost. Do not call `just workspace` or an F2-open session a
   release-performance benchmark.

## Source map

- `src/perf.rs` — collector, stage registry, history semantics, old overlay.
- `src/runtime/app.rs` — runtime frame boundary and current F2 handling.
- `src/view/mod.rs` — stage timing/order, render plan and full-redraw overlay
  consequence.
- `src/view/frame.rs` — existing bounded sparkline painter.
- `src/panel/dock.rs`, `src/model/ui.rs`, `src/messages.rs`,
  `src/update/dock.rs` — dock identity/state/focus/update boundaries.
- `src/layout/chrome.rs`, `src/layout/keys.rs`, `src/view/panels.rs` — shared
  Clay geometry, typed keys, and dock paint integration.
- `docs/ui/DOCKABLE-PANEL.md`, `docs/ui/PANE-CHROME.md` — planned generic host
  contract consumed by this feature.
