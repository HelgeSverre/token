# Pixel scrolling and animated easing

Status: archived — implemented and verified on `feat/pixel-smooth-scrolling`.
Native acceptance covers synthetic macOS input and process restart; physical
trackpad feel and Linux/Windows interaction are not claimed.

## Requested outcome

Plain-text editor panes support vertical and horizontal pixel scrolling, clipped
partial rows/glyphs, and animated easing for discrete wheel input. Trackpad pixel
events and scrollbar dragging remain direct. Existing Settings, list, CSV,
terminal and image input semantics are preserved. No new rendering backend.

## Implementation and verification gates

- [x] Shared per-pane scroll state, pixel bounds and coordinate conversions.
- [x] Full and cursor-only painting agree for text, gutter, selections, guides,
      diagnostics, ghost text and partial rows/columns.
- [x] Mouse/gutter hits, rectangle selection, caret/IME and popup anchors agree
      with the painted viewport.
- [x] Pixel wheel dispatch and scrollbar dragging/clicking on both axes.
- [x] Deterministic easing, retargeting, interruption and event-loop scheduling
      that stops waking once settled. No second inertia layer on trackpads.
- [x] Navigation/reveal, wrapping, resize/DPI, split panes, external reload and
      session persistence preserve or deliberately reset fractional positions.
- [x] Focused geometry/pixel/animation regressions, full suite and strict lint.
- [x] Native macOS pointer/pixel-wheel/scrollbar and restart checks, with explicit
      limits on hardware-gesture and cross-platform claims.
- [x] Release scrolling profile through shared performance stages, recorded in
      `docs/benchmark/`; no debug-frame-rate claims.
- [x] Changelog, logically grouped commits and final diff review. Archive this
      checklist only after the requested outcome and its gates are verified.

All build/verification output stays in the normal repository `target/` directory.

## Implementation checkpoint — 2026-09-09

- `PixelAxis` owns continuous positions, bounds, physical-pixel geometry and
  reveal calculations. Integral row/column anchors still address document data;
  offsets retain the partial first cell. The obsolete row-reveal implementation
  was removed. Horizontal bounds include the caret's reveal margin and use the
  shared tab-expansion helper without allocating entire long lines.
- Full/dirty text passes clip partial rows and columns. Gutter, text, range
  decorations and caret/hit-test mapping consume the same offsets. Scrollbar
  rendering and pointer handling share physical-pixel extents; ordinary hover
  does not measure horizontal line widths.
- Runtime preserves `PixelDelta` directly. `LineDelta` uses a 140 ms cubic
  ease-out, advances with elapsed time and wakes at most every 8 ms while active.
  Same-direction input accumulates toward the target; reversal starts from the
  displayed position. The update path never reads a clock.
- Session files store within-cell fractions with backward-compatible defaults.
  Linked Markdown preview synchronization retains its existing logical-line
  protocol; the webview continues to handle its own smooth scrolling.
- `just lint` passed. The final full `just test --no-fail-fast` run passed 2,611
  tests and both active doctests (5 nextest skips, 6 ignored doctests). The earlier
  targeted run had one process-exit handle warning. Another full run hit the
  unchanged managed-server fixture's one-second startup deadline before its
  startup marker appeared; an isolated retry and the final full confirmation
  passed. No timeouts were changed.
- Native macOS acceptance passed on the latest debug application, using an
  isolated 300-line fixture and config. Session-tap pixel events moved both
  axes directly; discrete wheel input produced intermediate animated positions
  and settled exactly. Native drags moved both scrollbar thumbs to fractional
  row/column positions. Clicking the partial first row selected the expected
  text, and wrapped text also accepted direct pixel movement.
- Orderly window close and process restart restored exactly `x=6 px, y=2900 px`
  and cursor `(79, 8)` (zero-based), including serialized within-cell fractions.
  The test window was closed afterward. No host security settings were changed.
  The local fixture/config, state samples and screenshot remain under
  `target/verification/native-pixel/`; see the durable native record in the
  [benchmark report](../benchmark/2026-09-09-pixel-scrolling.md#native-macos-acceptance).
- Optimized workload results are recorded in the
  [pixel-scrolling CPU report](../benchmark/2026-09-09-pixel-scrolling.md).
- Core implementation: `61e8eea`. Follow-up `989b84a` fixes horizontal bounds after
  inline dismissal and expands full/dirty pixel comparisons to wrapped tabs,
  diagnostics, visible carets and fractional ghost rows. Diff review found no
  remaining code defects. Native acceptance closed without additional
  application changes; the CPU workload/report extension is `37d9335`.
