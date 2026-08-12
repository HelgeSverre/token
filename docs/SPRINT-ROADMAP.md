# Sprint Roadmap

Implementation sequence across the current design docs: [overlay-surface](feature/overlay-surface.md), [editor-decorations](feature/editor-decorations.md), [find-enhancements](feature/find-enhancements.md), [autocomplete](feature/autocomplete.md), [lsp-integration](feature/lsp-integration.md), [soft-wrap](feature/soft-wrap.md), [context-menu](feature/context-menu.md), [settings-page](feature/settings-page.md).

> **Created:** 2026-08-11 · **Updated:** 2026-08-13
> **Status:** steps 1–10 shipped; current queue below.

## Shipped

| # | Work | Doc / phase |
| --- | --- | --- |
| 1 ✅ | Dynamic gutter width | editor-decorations P1 |
| 2 ✅ | Painter primitives + palette on OverlaySurface | overlay-surface P1–P2 |
| 3 ✅ | All modals migrated, old shell deleted | overlay-surface P3 |
| 4 ✅ | Find enhancements + decoration pipeline | find-enhancements + editor-decorations P2–P3 |
| 5 ✅ | Search Everywhere (Symbols tab disabled) | overlay-surface P4 |
| 6 ✅ | Cursor-anchored mode + offline menu completion | overlay-surface P5 + autocomplete P1 |
| 7 ✅ | LSP transport / lifecycle / document sync | lsp-integration P1 |
| 8 ✅ | Diagnostics (gutter, squiggles, status, overview) | lsp-integration P2 |
| 9 ✅ | Go to definition + jump history (+ forward stack) | lsp-integration P3 |
| 10 ✅ | Hover card (keyboard ⇧⌘D + mouse dwell) | lsp-integration P4 |

Shipped alongside (not in the original sequence): status-bar overhaul (border, font size, centering, expiring flash messages); theme-picker swatches; JetBrains keybinds (⌘B/⇧⌘D/⌘[/⌘]) + ⌘-click + mouse back/forward; Toggle LSP + Language Servers picker modal; Reveal in File Explorer; ZonePlan hover layouting; decoration-preserving cursor fast path; live LSP stress-testing against rust-analyzer / sema / phpantom / laravel-lsp (upstream bugs filed/found: sema#151 cross-file definitions; phpantom 0.9.0 builtin stubs unresolved — repro ready, issue not yet filed).

## Current queue (in order)

1. **Problems panel** — ⌘4, bottom-dock tab next to Terminal, per the C4 mockup; model-side diagnostics mirror; rows jump via navigation + jump history. (Investigated, seams verified, not started.)
2. **Show Usages** — ⌥F7/⌥⌘F7 cursor popup (panel later), built on two new abstractions extracted with three consumers each: `LspFeatureSlot` (generic per-feature request plumbing) and `LocationList` (path/range/preview rows → jump). Also upgrades multi-location go-to-definition.
3. **Context menu** — implement per the revised [context-menu.md](feature/context-menu.md) (OverlaySurface-based; editor + tabs + file tree in v1).
4. Adversarial review pass over the zone/hover layout work (folded into the next wave's review stage).

## Later (specs ready, unscheduled)

- **Step 11** — LSP completion source into the menu (autocomplete P4 = lsp-integration P5).
- **Step 12** — inline/FIM ghost text (autocomplete P2–P3; multi-line waits on soft-wrap).
- **Settings page** ([settings-page.md](feature/settings-page.md)) — `keep_unknown` config merge is its shippable Phase 1.
- Soft-wrap → multi-line ghost text; LSP workspace-symbols → Symbols tab; code actions (shell exists); usages panel.

## Known debt

8 bundled themes on derivation fallbacks (only default-dark hand-tuned); no `SearchResults` cache (stateless, viewport-bounded); completion config block; snippet source awaits the snippets feature; two `#[ignore]`d load-sensitive process-spawn tests (`--include-ignored`).
