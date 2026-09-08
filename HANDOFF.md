# Handoff

Reconciled 2026-09-08. This is a temporary checklist. Delete it only when the
remaining scope below is completed and verified; do not close native/live gates
using fixture tests alone.

## Current checkpoint

Completed implementation history and detailed evidence are in the
[refactoring audit](docs/dev/refactoring-audit-2026-09-06.md) and
[benchmark reports](docs/benchmark/README.md), with earlier handoff entries
preserved in Git history.

Latest implementation: `f0e1c4e` reuses coincident caret/selection coordinate
conversions. The [multi-cursor report](docs/benchmark/2026-09-08-multicursor.md)
attributes forward-edit costs and records before/after repeats, full tests and
lint. This addresses the multi-cursor performance investigation; remaining
forward-edit/Undo asymmetry is documented, not treated as a correctness defect.

Earlier implementation: `56a3be8` reuses exact unchanged recency snapshots. The
[recency report](docs/benchmark/2026-09-08-recency-refresh.md) records CPU sampling,
before/after repeats, edited-path costs and allocation tradeoffs. Its full tests
and lint passed. This addresses the recency profiling target without claiming
that genuinely edited refreshes are faster.

Earlier implementation: `780b13e` adds gated ASCII literal Find matching after
the overview improvement (`6974ba1`). The
[literal report](docs/benchmark/2026-09-08-find-literals.md) records before/after
CPU and allocation measurements, full-suite/lint verification and preserved
Unicode/regex semantics. Earlier completion response ownership work
(`68cf62a`) reduced fresh allocation by roughly 81% at 1,000 items; see its
[report](docs/benchmark/2026-09-08-completion-responses.md). The measured cold Find
and completion-response investigations are addressed as well.

Preserve the separate opaque Settings page, category navigation and form
controls. Settings scrolls continuously in physical pixels with clipped partial
rows; editor scrolling stays row-based. Shared geometry remains authoritative.

Reuse `CARGO_TARGET_DIR=/tmp/token-managed-server-check.5prvI4` for local checks
if that cache still exists; it contains verified debug and optimized builds.
The default `target/` previously disappeared outside this task.

## Remaining implementation

- Terminal enhancements (requested 2026-09-08):
  - Add terminal tabs with create, switch and close controls. Build on the
    existing session collection and dock; preserve each session's running
    process and scrollback when switching, and clean up only the closed session.
  - Support mouse text selection and copying to the system clipboard, including
    scrollback, wrapped lines and Unicode. Preserve normal shell interrupt keys.
  - Make links clickable while a modifier is held, with a visible hover cue.
    Follow the app's platform modifier conventions; ordinary clicks must not
    open links or interfere with selection. Reuse existing URL-opening effects.
  - Share terminal geometry between rendering and pointer interaction; verify
    these behaviors with live PTYs before marking them complete. Keep the shipped
    [terminal MVP plan](docs/archived/embedded-terminal.md) archived; these are
    follow-on features, not already delivered functionality.
- Autocomplete: edit prediction (anchored edits, deletion/diff preview and jump
  targets). The plan names candidate providers but no selected backend contract;
  a backend/model preference has been requested. See
  [autocomplete](docs/feature/autocomplete.md).

## Remaining verification

The macOS file-tree context-menu pointer check is now verified, including hover,
separator clearing and click-to-open; see the
[native record](docs/dev/refactoring-audit-2026-09-06.md#native-context-menu-acceptance-and-exit-diagnosis--2026-09-08).

Workspace-symbol search and both Usages surfaces are now verified against a real
rust-analyzer on macOS, including Unicode navigation and retained dock results;
see the [live-server record](docs/dev/refactoring-audit-2026-09-06.md#live-workspace-symbols-and-usages--2026-09-08).

- Native Windows/Linux keymap chips, contexts, capture and persistence; launcher
  detachment and per-instance port files on Windows; managed llama-server
  startup/teardown on Windows/Linux.
- Actual IME composition/candidate windows, CJK/emoji positioning and the remaining
  completion native-input matrix. Existing unit/macOS evidence is not a full
  platform certification.
- Live hosted model/provider compatibility, retrieval-context relevance,
  and Tabby service/model behavior. Fixture coverage is not live service
  verification.
- Resolve intermittent nextest process-exit warnings. The September 8 theme
  suite flagged `settings_page_keeps_spacious_categories_and_shared_control_hits`
  as leaky. The Find overview full run also flagged
  `supersession_disconnects_old_socket_and_serves_new_request`
  (`ec5fa5f6-f57b-4b4a-baa3-4b7b2436420d`). Other spawn-test and managed-server
  runs passed without warnings, but did not establish their cause. Do not
  suppress or call that resolved.
- Investigate the newly reproduced managed-server startup timeout and fake-LSP
  initialization timeouts under the September 8 stress runs. The managed test
  now retains the actual failure reply; ten later runs passed without timeout
  changes. This is distinct from nextest's output-handle warning. Failures,
  clean repeats and environment limits are in the native/exit record above.

## Archival and closeout

Completed Settings v1, soft-wrap, LSP baseline, Find and context-menu plans
already live under `docs/archived/`. Settings keymap remains active until its
cross-platform verification gate closes; autocomplete retains unfinished
Phase 5+ scope. Other planned features are not automatically in scope.

Keep changelog entries with changes and add dated benchmark reports under
`docs/benchmark/`. No release or publication is authorized. Once the checklist
above is genuinely closed, archive the eligible plans and delete this file.
