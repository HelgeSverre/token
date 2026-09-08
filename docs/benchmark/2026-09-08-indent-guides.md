# Indent-guide CPU rendering cost — 2026-09-08

The first guide implementation (`e988572`) adds bounded indentation inference
and guide strokes to the existing row-decoration pass. This compares the same
optimized binary with guides enabled and disabled, not different builds.
The measured code state is `94398a9`.

## Results

CPU editor-area timings in milliseconds, 1,000 frames per separate process:

| Pair | Guides on median | Guides off median | On p95 | Off p95 |
| --- | --- | --- | --- | --- |
| 1 | 3.55 | 3.47 | 3.75 | 3.88 |
| 2 | 3.54 | 3.43 | 3.75 | 3.67 |
| 3 | 3.54 | 3.55 | 3.82 | 3.88 |

The median of the three process medians is 3.54 ms on versus 3.47 ms off:
approximately 0.07 ms (2%) higher. Paired differences range from −0.01 to
+0.11 ms and tail timings vary in both directions. This is a small observed
cost within noticeable desktop-run variation, not a precise overhead estimate
or a claim of statistical significance. No further caching/refactor is justified
by this measurement alone.

[Recorded timing summaries](data/2026-09-08/indent-guides.txt).

## Reproduction and boundaries

```sh
just profile-render --frames 1000 --splits 3 --scroll --stats
just profile-render --frames 1000 --splits 3 --scroll --stats --no-indent-guides
```

- Apple M2 Max, macOS 15.6, rustc 1.98.0; Cargo `release`, opt-level 3, thin LTO.
- 1920×1080 physical pixels at 2×, three independent synthetic Rust documents
  of approximately 10,000 lines. ASCII glyph cache prewarmed; viewport advances
  every ten frames through the existing scrolling workload.
- Includes framebuffer clearing, scrolling updates, indentation inference and
  editor-area CPU painting. Excludes native window presentation, input latency,
  LSP, documentation overlays and the remaining application chrome. This is
  not a documentation-card benchmark or a measured application frame rate.
- Three serial on/off pairs, after the build and test jobs completed. Ordinary
  desktop background activity was not controlled. The initial 600-frame run
  attached to compilation was discarded from this comparison.
- All compilation used the repository's normal `target/`; the first optimized
  build rebuilt dependencies following cleanup. Its unrelated pre-existing
  unused-`revision` warning in `src/update/syntax.rs` was not suppressed.

This supports keeping the simple bounded implementation. Syntax-derived scopes,
blank-line continuation and other richer guide modes would need their own
correctness and performance checks.
