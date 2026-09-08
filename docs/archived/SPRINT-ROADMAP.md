# Sprint Roadmap

Implementation sequence across the current design docs: [overlay-surface](overlay-surface.md), [editor-decorations](editor-decorations.md), [find-enhancements](find-enhancements.md), [autocomplete](../feature/autocomplete.md), [lsp-integration](lsp-integration.md), [soft-wrap](soft-wrap.md), [context-menu](context-menu.md), [settings-page](settings-page.md).

> **Created:** 2026-08-11 · **Updated:** 2026-09-06
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
| — ✅ | Context menu (editor, tabs, file tree) | queue item 3 / [context-menu](context-menu.md) |
| — ✅ | Signature help, Rename Symbol, Show Code Actions, Format Document/Selection | lsp-integration "Phase 6+" |
| — ✅ | CLI detach, single-instance handoff, `--wait`; per-instance automation and MCP instance targeting | v0.6.0 |
| — ✅ | Find options/status chrome + selection scope | find-enhancements P5+P7 |
| — ✅ | Inline ghost text via llama.cpp `/infill` | autocomplete P2 |

Shipped alongside (not in the original sequence): status-bar overhaul (border, font size, centering, expiring flash messages); theme-picker swatches; JetBrains keybinds (⌘B/⇧⌘D/⌘[/⌘]) + ⌘-click + mouse back/forward; Toggle LSP + Language Servers picker modal; Reveal in File Explorer; ZonePlan hover layouting; decoration-preserving cursor fast path; live LSP stress-testing against rust-analyzer / sema / phpantom / laravel-lsp (upstream bugs filed/found: sema#151 cross-file definitions; phpantom 0.9.0 builtin stubs unresolved — repro ready, issue not yet filed).

## Implemented, unreleased

**Soft wrap** ([archived plan](soft-wrap.md)) — all eight phases complete;
the shared mapping now also supports the implemented multi-line ghost projection.

**Settings v1** ([archived plan](settings-page.md)) — preserving saves,
searchable preset controls and the LSP section are implemented and verified,
including isolated macOS native keyboard/persistence checks. The keymap tab is
separate future work; Windows/Linux GUI and physical-pointer checks remain.

## Current queue (in order)

1. **Inline completion Phase 5+** — [autocomplete](../feature/autocomplete.md#phase-5-future): complete the remaining native IME/platform verification, then edit prediction, retrieval/provider and completion work. Multi-row/mid-line ghost projection has automated geometry/lifecycle/pixel coverage, isolated macOS keyboard/pointer/resize checks and release-stage profiling recorded in the audit. Phase 3's opt-in idle recency context, raw FIM formats/inference, partial acceptance, alternative cycling, bounded LRU reuse, conservative post-cache filters and cancelable transports are implemented. Recency-stage profiling identified the implemented token-index optimization. Live model/server validation and the full native language/platform matrix remain unverified.
2. **Settings Keymap Tab** ([follow-up plan](../future/settings-keymap.md)) — merged binding list, conflict detection, chord capture/rebinding, override persistence and base-keymap choice remain future work.
3. LSP workspace-symbols → Search Everywhere Symbols tab; usages panel (popup shipped).

The [refactoring audit](../dev/refactoring-audit-2026-09-06.md) tracks the parallel
consolidation/performance work and remaining file-effect boundary. Completed
damage tracking and palette history plans are in the [documentation index](../README.md#completed-features);
deferred history ideas remain in [command-history follow-ups](../future/command-history-followups.md).

## Known debt

8 bundled themes on derivation fallbacks (only default-dark hand-tuned); no `SearchResults` cache (stateless, viewport-bounded); completion config block; snippet source awaits the snippets feature; two `#[ignore]`d load-sensitive process-spawn tests (`--include-ignored`).
