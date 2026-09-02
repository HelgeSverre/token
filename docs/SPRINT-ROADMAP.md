# Sprint Roadmap

Implementation sequence across the current design docs: [overlay-surface](archived/overlay-surface.md), [editor-decorations](archived/editor-decorations.md), [find-enhancements](feature/find-enhancements.md), [autocomplete](feature/autocomplete.md), [lsp-integration](archived/lsp-integration.md), [soft-wrap](feature/soft-wrap.md), [context-menu](archived/context-menu.md), [settings-page](feature/settings-page.md).

> **Created:** 2026-08-11 · **Updated:** 2026-09-02
> **Status:** steps 1–11 and the whole August queue shipped in v0.6.0; current queue below.

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
| 11 ✅ | LSP completion source into the menu | autocomplete P4 = lsp-integration P5 |
| — ✅ | Problems panel (⌘4, scope switch, next/previous diagnostic) | queue item 1 |
| — ✅ | Show Usages popup + multi-location go-to-definition | queue item 2 |
| — ✅ | Context menu (editor, tabs, file tree) | queue item 3 / [context-menu](archived/context-menu.md) |
| — ✅ | Signature help, Rename Symbol, Show Code Actions, Format Document/Selection | lsp-integration "Phase 6+" |
| — ✅ | CLI detach, single-instance handoff, `--wait`; per-instance automation and MCP instance targeting | v0.6.0 |

Shipped alongside (not in the original sequence): status-bar overhaul (border, font size, centering, expiring flash messages); theme-picker swatches; JetBrains keybinds (⌘B/⇧⌘D/⌘[/⌘]) + ⌘-click + mouse back/forward; Toggle LSP + Language Servers picker modal; Reveal in File Explorer; ZonePlan hover layouting; decoration-preserving cursor fast path; live LSP stress-testing against rust-analyzer / sema / phpantom / laravel-lsp (upstream bugs filed/found: sema#151 cross-file definitions; phpantom 0.9.0 builtin stubs unresolved — repro ready, issue not yet filed).

## Current queue (in order)

1. **Soft wrap** ([soft-wrap.md](feature/soft-wrap.md)) — the XL item; unblocks multi-line ghost text.
2. **Step 12** — inline/FIM ghost text (autocomplete P2–P3).
3. **Settings page** ([settings-page.md](feature/settings-page.md)) — `keep_unknown` config merge is its shippable Phase 1.
4. LSP workspace-symbols → Search Everywhere Symbols tab; usages panel (popup shipped).

## Known debt

8 bundled themes on derivation fallbacks (only default-dark hand-tuned); no `SearchResults` cache (stateless, viewport-bounded); completion config block; snippet source awaits the snippets feature; two `#[ignore]`d load-sensitive process-spawn tests (`--include-ignored`).
