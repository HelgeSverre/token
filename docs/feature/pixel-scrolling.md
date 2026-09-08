# Pixel scrolling and animated easing

Status: implementation in progress on `feat/pixel-smooth-scrolling`.

## Requested outcome

Plain-text editor panes support vertical and horizontal pixel scrolling, clipped
partial rows/glyphs, and animated easing for discrete wheel input. Trackpad pixel
events and scrollbar dragging remain direct. Existing Settings, list, CSV,
terminal and image input semantics are preserved. No new rendering backend.

## Implementation and verification gates

- [x] Shared per-pane scroll state, pixel bounds and coordinate conversions.
- [ ] Full and cursor-only painting agree for text, gutter, selections, guides,
      diagnostics, ghost text and partial rows/columns.
- [ ] Mouse/gutter hits, rectangle selection, caret/IME and popup anchors agree
      with the painted viewport.
- [ ] Pixel wheel dispatch and scrollbar dragging/clicking on both axes.
- [x] Deterministic easing, retargeting, interruption and event-loop scheduling
      that stops waking once settled. No second inertia layer on trackpads.
- [x] Navigation/reveal, wrapping, resize/DPI, split panes, external reload and
      session persistence preserve or deliberately reset fractional positions.
- [x] Focused geometry/pixel/animation regressions, full suite and strict lint.
- [ ] Native macOS pointer/trackpad/scrollbar and restart checks, with explicit
      limits on cross-platform claims.
- [ ] Release scrolling profile through shared performance stages, recorded in
      `docs/benchmark/`; no debug-frame-rate claims.
- [ ] Changelog, logically grouped commits and final diff review. Archive this
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
- `just lint` passed. The final full `just test --no-fail-fast` run passed 2,610
  tests and both active doctests (5 nextest skips, 6 ignored doctests). The earlier
  targeted run had one process-exit handle warning; the final full run was clean.
- Native macOS launch/render and automation queries succeeded, but injected
  pointer/wheel events did not reliably reach the test window. A frontmost-process
  query reported `loginwindow`; desktop accessibility/lock state is not confirmed.
  Do not claim native gesture, scrollbar or fractional-restart verification from
  those attempts. The isolated fixture/config and logs are under
  `target/verification/native-pixel/`. No host security settings were changed.
