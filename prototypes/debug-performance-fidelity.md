# Performance prototype: visual fidelity investigation

Compared on 2026-09-16, before adding the theme switcher. The study's lighter,
more spacious appearance comes primarily from typography roles, text size,
line spacing, and quieter colors. It does not use a different font family.

## Evidence and comparison setup

- Launched an isolated native Token window with `src/layout/chrome.rs`, Default
  Dark, JetBrains Mono, Inter, the file explorer, and an Outline dock. The user's
  configuration was not changed. The window reported a 2× scale through its
  physical geometry and measured line pitch.
- Inspected the live native window, then captured a repeatable baseline through
  `target/debug/screenshot`, which uses the production CPU painters. That image
  is 2796 × 1748 physical pixels: 1398 × 874 logical pixels at 2×. The explorer
  is 184 logical pixels wide; the native right dock is 250.
- Measured the original prototype's computed browser styles at 1440 × 1060 CSS
  pixels, DPR 2. Saved the original before modifications. A separate temporary
  browser capture uses the exact native source excerpt (lines 114–158), Default
  Dark colors, the same editor/sidebar/dock widths, and the native gutter width
  to inspect text density. The normal prototype retains its abridged excerpt.
- Native Token does not yet have a performance dock, so its Outline dock is a
  structural reference. The native screenshot tool does not render the F2
  overlay. This is not a pixel-diff claim about the proposed performance panel.

Local evidence is under `target/verification/perf-fidelity/`:
`prototype-original.webp`, `prototype-matched-source.webp`,
`screenshot-native-default-dark.png`, `native-default-dark.yaml`, and native
automation state/configuration fixtures. These generated files are not shipped.

## What differs

| Element | Original study | Native defaults at 2× | Visual effect |
| --- | --- | --- | --- |
| Code face | Bundled JetBrains Mono, regular | Same bundled face, regular | Family is already faithful. |
| Code size | 11.52 CSS px | 14 logical px | Study glyphs are about 18% smaller. |
| Code line pitch | 22.464 CSS px | 18.5 logical px | Study has about 21% more vertical space despite smaller glyphs. |
| Explorer | Inter, 12 px | JetBrains Mono, 14 px; 22 px rows | Proportional labels look less mechanical and occupy less width. |
| Editor tabs | Inter, 11.04 px | JetBrains Mono, 14 px | The study separates interface typography from code. |
| Dock heading | Inter, 12.8 px, weight 600 | JetBrains Mono, regular | Study adds a stronger hierarchy and a distinct UI voice. |
| Status text | Inter, 9.76 px | Inter, 12 px | Study de-emphasizes persistent chrome. |
| Colors | Custom cool charcoal and muted pastels | Theme-defined; Default Dark includes a bright blue status bar | The custom palette draws less attention to surrounding chrome. |

JetBrainsMono.ttf contains a regular 400-weight face, units-per-em 1000, ascender
1020, descender −300, and zero line gap. That gives a nominal 18.48 px line box
at 14 px. Native `line_height()` rounds **physical** pixels up: 37 physical px at
2×, or 18.5 logical px. At 1× it is 19 px. Inter-Regular.ttf is also a regular,
non-variable 400-weight face. The study requests 600 without supplying that face,
so its browser headings use synthesized weight.

The native renderer uses fontdue glyph bitmaps, rounds glyph origins to physical
pixels, advances characters individually, and alpha-blends into its CPU buffer.
The prototype uses browser text layout and requests macOS antialiased text with
`-webkit-font-smoothing`. Those pipelines can differ in glyph edges and spacing.
This comparison does **not** isolate rasterization from size, spacing, and weight,
so it does not establish that changing the rasterizer would improve Token.

Other fidelity differences contribute to the impression: the original mock has
a shorter curated file tree, simplified highlighting, breadcrumbs, a taller tab
strip, and less deeply indented sample code. Native Token shows real syntax
captures, folding indicators, indentation guides, and the full workspace tree.

## Source references

- `src/view/fonts.rs`: embedded faces and configured font roles.
- `src/config.rs`: editor/UI font defaults and 12 px status text.
- `src/view/mod.rs`: 14 px editor size, physical scaling, and line-height rounding.
- `src/view/frame.rs`: `TextPainter::draw`, glyph origins/advances, and alpha blend.
- `src/view/panels.rs`: code-font dock headers and sidebar theme colors.
- `src/theme.rs` and `themes/*.yaml`: theme registry, raw colors, overlay fallbacks.

## Controls added for further comparison

The Theme selector offers all 15 repository themes, including the native
`themes/study.yaml`, plus the preserved original study palette. `?theme=study`
continues to mean the original visual experiment; `?theme=builtin-study` means
the registered theme so comparisons are explicit. Colors are generated from the
built-in registry and YAML, with each source file's SHA-256 recorded in
`debug-performance-themes.js`. Optional overlay colors are resolved with the
same formulas as `src/theme.rs`; explicit YAML colors always win. Chart series
use theme syntax colors. Dock surfaces use sidebar colors, following the native
dock renderer. Color choices for the new chart controls remain a design proposal,
since there is no native performance-dock counterpart yet.

Typography is independent: Study preserves the compact proportional UI;
Token defaults applies the relevant native font roles/sizes and DPI-rounded code
line pitch. It keeps the proposed panel structure and browser rendering, so it
is a typography experiment rather than an exact native screenshot renderer.

Suggested next design experiment: keep code at the user's preferred size and
try Inter for explorer/tab/dock labels, then tune line spacing separately. That
tests the clearest source of the perceived improvement before considering any
change to the text rasterizer. No native appearance settings were changed here.

The follow-up [component decisions](../docs/ui/PROTOTYPE-COMPONENTS.md),
[editor polish plan](../docs/feature/editor-visual-polish.md), and
[performance-panel plan](../docs/feature/performance-panel.md) turn this evidence
into proposed native work. The polish plan separately derives current tab/status
bar heights; font size alone does not describe their occupied vertical space.
