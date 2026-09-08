# Pixel-scrolling CPU workloads — 2026-09-09

Measured implementation: `61e8eea` on `feat/pixel-smooth-scrolling`, with the
`profile_render` pixel/easing workload extension committed alongside this report.
These are optimized CPU update/render measurements, not native input latency or
display frame rates.

## Results

Each cell is the median milliseconds from a separate 1,000-frame process.
Three independent synthetic Rust panes, approximately 10,000 lines each:

| Repeat | Row-step workload | Continuous pixels | Eased wheel |
| --- | ---: | ---: | ---: |
| 1 | 3.93 | 3.99 | 4.20 |
| 2 | 4.17 | 4.03 | 4.07 |
| 3 | 3.93 | 3.78 | 3.86 |
| Median of process medians | 3.93 | 3.99 | 4.07 |

The three-pane workloads remain around 4 ms median CPU time. Tail variation is
noticeable: pixel p95 spans 4.04–5.82 ms; eased p95 spans 4.15–6.48 ms. The row-step
control itself spans 4.29–7.66 ms p95. Some p99 values exceed an 8.33 ms budget,
before adding native presentation costs. This is not evidence of guaranteed
120 Hz presentation.

The largest individual frames were 118.36 ms in the row-step control, 19.57 ms
for continuous pixels and 10.81 ms for easing. These outliers are retained in
the raw summaries; this run did not attribute them to specific CPU stacks.

A separate one-pane, real-source check (`src/model/editor.rs`, continuous pixels)
measured **0.65 ms median, 0.74 ms p95, 0.80 ms p99**. It has substantially less
visible text than the dense synthetic fixture; it is a different workload, not
a scaling ratio.

[Recorded timing summaries](data/2026-09-09/pixel-scrolling.txt).

## Workloads and interpretation

- `--scroll` retains the existing workload: move to a different integral row
  every ten frames, cycling through the first hundred rows.
- `--pixel-scroll` sends a 0.75 px horizontal / 3.25 px vertical displacement
  through the production update path on every frame, reversing every 240 frames.
- `--eased-scroll` sends three-row wheel steps every twelve frames and advances
  the production animation with 1/120 second elapsed time each frame, reversing
  every 240 frames. Time is simulated; the headless loop does not wait for a
  display refresh. Platform trackpad pixels do not use this easing layer.
- Both new workloads exercise all active text panes. They include message
  reconciliation, continuous bounds, framebuffer clearing, gutter/decorations,
  scrollbar calculation and editor-area CPU painting through the existing
  renderer. No additional stage timer or performance overlay was introduced.
- Different rows, glyphs and update cadences mean these numbers **do not isolate
  the overhead of pixel scrolling**. They are current-state workload checks, not
  a before/after comparison with the parent revision or the historical baseline.
- Review removed eager line-width measurement from ordinary pointer hit testing.
  Tab-free widths use Rope metadata after a chunk scan; tabbed widths use the
  existing tab-stop helper without allocating a whole long line. These are
  structural observations, not separately quantified speedup claims.

## Reproduction

```sh
just profile-render --frames 1000 --splits 3 --scroll --stats
just profile-render --frames 1000 --splits 3 --pixel-scroll --stats
just profile-render --frames 1000 --splits 3 --eased-scroll --stats
just profile-render --frames 1000 --splits 1 --pixel-scroll --files src/model/editor.rs --stats
```

Apple M2 Max, macOS 15.6, rustc 1.98.0. Cargo `release`, opt-level 3, thin LTO;
1920×1080 physical pixels at 2×; ASCII glyph cache prewarmed. Compilation and
test jobs completed before the recorded measurements. Runs were serial; the
last eased/source pair was repeated serially after a potentially overlapping
launch, and only its replacement measurements are retained. Ordinary desktop
background activity was not controlled. All build and transient verification
output stayed under the repository's normal `target/`.

Excluded: window acquisition/presentation, actual input delivery, live language
servers, webviews, the remainder of application chrome and hardware trackpad
feel. No F2-overlay or debug-build numbers are presented as release performance.
The optimized build retained the pre-existing release-only unused-`revision`
warning in `src/update/syntax.rs`; strict all-target/all-feature lint passed.

## Verification boundary

The implementation's full suite passed 2,611 tests and both active doctests;
strict lint passed. Tests cover fractional geometry, full/dirty repaint
agreement, animation retargeting/cancellation, the idle wake deadline, split
targeting and session fractions across changed viewport metrics.

The native macOS app launched and rendered the isolated fixture, and automation
state queries worked. Injected pointer/wheel events did not reliably reach it;
the test app was confirmed foreground and CoreGraphics reported event-posting
access enabled, so the injection failure is not explained by a confirmed lock
or missing permission. No host security settings were changed. Native
gesture/scrollbar interaction and fractional restart are **not verified** yet.
The [active implementation checklist](../feature/pixel-scrolling.md) remains
unarchived until the native checks are closed. The extended CPU pixel tests
also compare wrapped/unwrapped partial rows with tabs, diagnostics, visible
carets and ghost text against full and incremental repainting.

A subsequent inline-dismissal edge fix clamps horizontal offsets when a wide
ghost suggestion disappears. The scrolling workloads above contain no inline
suggestions and were not rerun to claim any performance effect from that fix.

One later full-suite run hit the unchanged managed-server fixture's one-second
startup deadline before its startup marker appeared. An isolated retry and the
final full-suite confirmation passed; no timeout or host-policy changes were made.
