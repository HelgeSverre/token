# Settings scrolling — 2026-09-08

Settings now scrolls in physical pixels and paints clipped partial rows. This
report investigates the separate complaint that scrolling feels sluggish in
debug builds. It measures production CPU paths, not native input-to-display
latency or a promised frame rate.

## Findings

- Scroll math is not the dominant cost: approximately 9 µs per update in debug
  and 1 µs optimized. The Settings hit/spec/layout path is approximately 27 µs
  debug and 4 µs optimized.
- The old backdrop dimmer repeatedly checked the clip stack through the
  text-pixel path. At 2200×1440, its debug median was 81.0 ms, compared with
  101.8 ms for the full Settings modal paint.
- Reusing `Frame::blend_rect_px` in `Frame::dim` removes that duplicate
  clipping traversal without changing blend arithmetic. The debug backdrop
  median fell to 27.1 ms and modal paint to 47.4 ms in the repeat.
- Debug is still expensive at high DPI: the editor-plus-Settings CPU probe is
  57.8 ms after the change, already beyond a 16.7 ms frame budget before native
  presentation. The optimized baseline was 2.78 ms for the same combined probe.
  Do not infer release behavior from the debug build or from the F2 overlay.

## Debug before/after

Medians in milliseconds. Both runs include the pixel-scrolling correction;
the runtime rendering change between them is the shared dimmer implementation.
The benchmark harness received formatting-only changes.

| Physical window / scale | Backdrop before → after | Modal paint before → after | Editor + modal before → after |
| ----------------------- | ----------------------- | -------------------------- | ----------------------------- |
| 1100×720 / 1×           | 20.196 → 6.769          | 26.044 → 12.355            | 30.706 → 16.825               |
| 400×750 / 1×            | 7.689 → 2.587           | 9.998 → 4.827              | 14.483 → 9.235                |
| 2200×1440 / 2×          | 80.992 → 27.134         | 101.779 → 47.401           | 112.254 → 57.826              |

Full output, including p95 and scroll-plus-paint:
[debug before](data/2026-09-08/settings-debug-before.txt),
[debug after](data/2026-09-08/settings-debug-after.txt).

## Optimized baseline

Before the dimmer consolidation, medians in milliseconds:

| Physical window / scale | Backdrop | Modal paint | Scroll + modal | Editor + modal |
| ----------------------- | -------- | ----------- | -------------- | -------------- |
| 1100×720 / 1×           | 0.370    | 0.627       | 0.628          | 0.859          |
| 400×750 / 1×            | 0.142    | 0.215       | 0.220          | 0.448          |
| 2200×1440 / 2×          | 1.543    | 2.190       | 2.243          | 2.784          |

The high-DPI baseline has noticeable tail variation (modal p95 4.522 ms).
Single desktop runs are not confidence intervals. Full output:
[optimized before](data/2026-09-08/settings-release-before.txt).

## Optimized repeat after consolidation

Medians in milliseconds:

| Physical window / scale | Backdrop | Modal paint | Scroll + modal | Editor + modal |
| ----------------------- | -------- | ----------- | -------------- | -------------- |
| 1100×720 / 1×           | 0.372    | 0.638       | 0.640          | 0.879          |
| 400×750 / 1×            | 0.142    | 0.222       | 0.223          | 0.460          |
| 2200×1440 / 2×          | 1.576    | 2.314       | 2.279          | 2.935          |

This run does **not** establish an optimized speedup: medians were slightly
higher, while tail timings varied in both directions. The clear observed gain
is in debug, where the removed per-pixel clip checks were not optimized away.
The high-DPI modal p95 was 3.854 ms; editor-plus-modal p95 was 5.091 ms.
[Full optimized after output](data/2026-09-08/settings-release-after.txt).

A separate-process repeat with the same optimized binary measured modal medians
of 0.635 / 0.216 / 2.247 ms and editor-plus-modal medians of
0.859 / 0.461 / 2.807 ms (wide / compact / high-DPI). This supports treating the
small optimized differences cautiously rather than claiming a speedup.
[Full repeat output](data/2026-09-08/settings-release-repeat.txt).

## Reproduction and boundaries

```bash
CARGO_BUILD_JOBS=1 just profile-workloads-debug settings
CARGO_BUILD_JOBS=1 just profile-workloads settings
```

- Apple M2 Max, 12 logical CPUs, 32 GiB RAM; macOS 15.6 (24G84).
- Rust 1.98.0 (`88d9e12ae`, 2026-08-18), aarch64-apple-darwin.
- Debug: Cargo `dev`, Token opt-level 0, debug assertions enabled; existing
  fontdue/ttf-parser package optimization overrides remain.
- Optimized: Cargo `bench`/release settings, opt-level 3 and thin LTO,
  debug assertions disabled. No extra features, F2 overlay or runtime timers.
- Compiles and timed suites ran serially. Compilation is excluded. This was an
  active desktop; no unrelated processes or editor windows were terminated.
- Optimized builds reported an existing unused `revision` local in
  `src/update/syntax.rs` under non-debug compilation. All benchmark processes
  exited successfully; the warning was not suppressed.
- The shared workload harness uses ten warmups, then 120 samples for
  update/hit-layout and 80 for painting. It sorts durations and reports elements
  `n/2` and `n*95/100` as median/p95. Raw outputs retain these summaries,
  not every individual sample.
- Default dark theme, embedded JetBrains Mono, 14 logical-pixel font,
  20 logical-pixel line height, 8.4 logical-pixel character width. Settings is
  open on All Settings with a 127-pixel offset; update probes alternate ±1 pixel
  to avoid benchmarking a clamped no-op.
- Painting calls the production `render_modals`, including spec/layout
  construction, dimming, form controls, glyphs and scrollbar. Glyph/mask caches
  are warm. Each paint and backdrop probe includes buffer clearing.
- The combined editor/modal case adds the production editor-group renderer over
  1,000 short background lines. It excludes other window chrome, native event
  scheduling, surface acquire/copy/present, WindowServer and IME.
- This is a Settings-specific snapshot, not a rerun of the broader
  [September 7 baseline](2026-09-07-current.md).

The measured working tree started at `2bdbcca` plus the pixel-scrolling and
harness changes. Pixel scrolling is committed in `67fa676`; that commit retains
the before-version dimmer. Commit `8516dc2` contains the measured harness and
after-version dimmer. To reproduce the before variant, use that harness with
`src/view/frame.rs` from `67fa676` in an isolated checkout. Checking out
`2bdbcca` alone does not reproduce these probes.

| Source                      | SHA-256                                                            |
| --------------------------- | ------------------------------------------------------------------ |
| `src/view/frame.rs`, before | `f285306389bfff610004458f61aa8d0936df8eed1925a37837ba39b59e23b18f` |
| `src/view/frame.rs`, after  | `bed4d99781be411e75a4eaaca2756effa20328f0a88ad68ffec13055457d9391` |
| Harness, before rustfmt     | `295d5b9a04f1c43182eeac6be15d3b82ff46a80a89539343c7bc74afe51b8c21` |
| Harness, after rustfmt      | `489f7bcdf5f3fb5309ea4a5840b56e7d853948894478d3705040ae40772a123f` |

## Remaining limits

Continuous pixel scrolling removes row/section snapping; it does not by itself
make an unoptimized full-window paint cheap. Further work should target measured
painting costs, not the sub-millisecond update/layout path. Native trackpad
latency and presentation remain separate verification work.

## Verification

- Final suite: 2,572 tests passed, seven skipped; two doctests passed, six ignored.
  Nextest run: `f462df76-3cca-4c02-8ab7-29836f9ebab2`.
- Strict `just lint`, `just fmt-check` and `just build` passed.
- Regression coverage includes all 256 dimming alpha values, nested/empty clips,
  pixel-based row ranges, hit mapping, end clamping, keyboard reveal and dragging.
- The `settings-scrolled` screenshot fixture was rendered and visually inspected
  at 1100×720 and 400×750. Partial rows remain clipped inside the body; header,
  categories and footer stay fixed. This is headless evidence, not a native
  trackpad or cross-platform acceptance session.
- Diff-based self-review: **Approve**; no outstanding critical/high findings.
