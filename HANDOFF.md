# Handoff

Reconciled 2026-09-08. This is a temporary checklist, not a historical activity
log. Delete it only when the remaining work below is completed and verified.
Earlier checkpoints are preserved in Git history and the
[refactoring audit](docs/dev/refactoring-audit-2026-09-06.md).

## Completed and committed

The previously uncommitted implementation is now recorded as a coordinated
six-commit series, `ab96495` through `2bdbcca`:

- Shared document edits, lossless pane Undo/Redo, soft wrap and viewport rendering.
- Ordered file/configuration effects, captured file identity and startup preparation.
- Context-aware dropdowns, path suggestions, commit characters, documentation
  cards, cancelable inline providers, partial acceptance, alternatives, cache,
  recency context, multi-row ghost text and local statistics.
- Settings/keymap controls, override persistence and live shortcut hints.
- Workspace-symbol runtime/UI, background Find and shared runtime integration.
- Benchmark workloads, fixtures, changelog and implementation/verification records.

These groups share contracts; intermediate commits are not independent build
checkpoints. The combined source passed 2,568 tests and two doctests
(`98402f7e-0048-457c-888a-e28d3ae2d78d`). Staging preserved working-file hashes.
The persistent Usages panel, Mermaid rendering, context-menu hover, restored
Settings design and draggable shared scrollbars were committed previously.
Nothing was pushed or published.

The original eight-theme overlay tuning follow-up is committed in `48a29fa`:
explicit palettes, contrast checks and inspected command-palette/compact Settings
renders. Details are in the refactoring audit; later built-ins and custom-theme
fallbacks are unchanged.

The load-sensitive spawn-test cleanup is complete: PTY tests use a controlled
shell, assert actual output and run shell-exit coverage by default. The ignored
shell-script LSP handshake duplicate was removed; the existing real-process
integration scenario retains that coverage. Twenty repeated runs and the full
suite passed on macOS (2,575 tests, five skipped). See the refactoring audit.

## Settings scrolling correction

Preserve the separate Settings page, category navigation and form controls.
Do not turn it into a command palette. Settings uses physical-pixel scrolling
with clipped partial rows; editor scrolling stays row-based. The shared
`RowListView` owns geometry, clipping ranges, hit mapping and row reveal.
This correction is committed in `67fa676`; profiling and the shared dimmer
follow-up are in `8516dc2`.

The final scrolling/rendering suite passed 2,572 tests and two doctests
(`f462df76-3cca-4c02-8ab7-29836f9ebab2`), strict lint, formatting and debug build.
Seven tests and six doctests remain skipped/ignored. Wide and compact headless
screenshots confirm partial-row clipping; native trackpad presentation is not
measured by those checks.

Debug and optimized CPU profiling is complete. Reusing the shared rectangle
dimmer reduced high-DPI debug modal paint from 101.8 to 47.4 ms; optimized repeats
were about 2.2–2.3 ms, with no established optimized speedup. See the
[Settings report](docs/benchmark/2026-09-08-settings-scroll.md) for raw results,
measurement boundaries and remaining debug rendering costs.

Follow-up `05136be` makes Settings opaque and skips backdrop work hidden by
opaque panels through one shared private helper. The fresh high-DPI debug modal
comparison is 49.5 → 28.2 ms; see the report's opaque-panel follow-up. All 2,574
tests and two doctests, strict lint, formatting and debug build passed; default
Settings/palette screenshot files match their pre-change versions byte-for-byte.
Optimized modal paint measured 0.95–0.99 ms at high DPI (previous snapshot about
2.25 ms); the combined editor/modal probe was 1.45–1.49 ms. These remain CPU-only
measurements, not native presentation checks.

## Remaining implementation

- Autocomplete: edit prediction, workspace retrieval context, and supervised
  local llama-server ownership. See [autocomplete](docs/feature/autocomplete.md).
- Address measured performance targets as warranted: cold explicit Find scans,
  completion-response allocations, larger recency refreshes and forward
  multi-cursor edits. Existing measurements are in
  [the September report](docs/benchmark/2026-09-07-current.md).

## Remaining verification

- Native Windows/Linux keymap chips, contexts, capture and persistence; launcher
  detachment and per-instance port files on Windows.
- Actual IME composition/candidate windows, CJK/emoji positioning and the remaining
  completion native-input matrix. Existing unit/macOS evidence is not a full
  platform certification.
- Live hosted model/provider compatibility, Tabby service/model behavior, and
  workspace-symbol/Usages live-server interaction. Fixture coverage is not live
  service verification.
- Native context-menu pointer acceptance; automated runtime/pixel checks already pass.
- Resolve intermittent nextest process-exit warnings. The September 8 theme
  suite flagged `settings_page_keeps_spacious_categories_and_shared_control_hits`
  as leaky. The subsequent spawn-test cleanup suite passed all 2,575 tests without
  warnings, but did not establish their cause. Do not suppress or call that resolved.

## Archival and closeout

Completed Settings v1, soft-wrap, LSP baseline, Find and context-menu plans
already live under `docs/archived/`. Settings keymap remains active until its
cross-platform verification gate closes; autocomplete retains unfinished
Phase 5+ scope. Other planned features are not automatically in scope.

Keep changelog entries with changes and add dated benchmark reports under
`docs/benchmark/`. No release or publication is authorized. Once the checklist
above is genuinely closed, archive the eligible plans and delete this file.
