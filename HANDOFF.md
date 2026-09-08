# Handoff

Reconciled 2026-09-08. This is a temporary checklist. Delete it only when the
remaining scope below is completed and verified; do not close native/live gates
using fixture tests alone.

## Current checkpoint

Completed implementation history and detailed evidence are in the
[refactoring audit](docs/dev/refactoring-audit-2026-09-06.md) and
[benchmark reports](docs/benchmark/README.md), with earlier handoff entries
preserved in Git history.

Latest implementation: `68cf62a` defers completion-item JSON until an actual
resolve request. The full suite passed 2,581 tests and two doctests, strict lint
and formatting. The [response-conversion report](docs/benchmark/2026-09-08-completion-responses.md)
records roughly 81% lower fresh allocation at 1,000 items. This closes the
completion-response allocation investigation, not the other profiling targets.

Preserve the separate opaque Settings page, category navigation and form
controls. Settings scrolls continuously in physical pixels with clipped partial
rows; editor scrolling stays row-based. Shared geometry remains authoritative.

Reuse `CARGO_TARGET_DIR=/tmp/token-managed-server-check.5prvI4` for local checks
if that cache still exists; it contains verified debug and optimized builds.
The default `target/` previously disappeared outside this task.

## Remaining implementation

- Autocomplete: edit prediction (anchored edits, deletion/diff preview and jump
  targets). The plan names candidate providers but no selected backend contract;
  a backend/model preference has been requested. See
  [autocomplete](docs/feature/autocomplete.md).
- Address measured performance targets as warranted: cold explicit Find scans,
  larger recency refreshes and forward multi-cursor edits. Existing measurements are in
  [the September report](docs/benchmark/2026-09-07-current.md).
  [Cold Find sampling](docs/benchmark/2026-09-08-cold-find.md) now identifies dense
  regex matching and overview-line traversal as the two next optimization targets;
  no Find optimization has yet been made for these findings.

## Remaining verification

- Native Windows/Linux keymap chips, contexts, capture and persistence; launcher
  detachment and per-instance port files on Windows; managed llama-server
  startup/teardown on Windows/Linux.
- Actual IME composition/candidate windows, CJK/emoji positioning and the remaining
  completion native-input matrix. Existing unit/macOS evidence is not a full
  platform certification.
- Live hosted model/provider compatibility, retrieval-context relevance,
  Tabby service/model behavior, and
  workspace-symbol/Usages live-server interaction. Fixture coverage is not live
  service verification.
- Native context-menu pointer acceptance; automated runtime/pixel checks already pass.
- Resolve intermittent nextest process-exit warnings. The September 8 theme
  suite flagged `settings_page_keeps_spacious_categories_and_shared_control_hits`
  as leaky. Subsequent spawn-test and managed-server suites passed without
  warnings, but did not establish their cause. Do not suppress or call that resolved.

## Archival and closeout

Completed Settings v1, soft-wrap, LSP baseline, Find and context-menu plans
already live under `docs/archived/`. Settings keymap remains active until its
cross-platform verification gate closes; autocomplete retains unfinished
Phase 5+ scope. Other planned features are not automatically in scope.

Keep changelog entries with changes and add dated benchmark reports under
`docs/benchmark/`. No release or publication is authorized. Once the checklist
above is genuinely closed, archive the eligible plans and delete this file.
