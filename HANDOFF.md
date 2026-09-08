# Handoff

Reconciled 2026-09-08. This is a temporary checklist. Delete it only when the
remaining scope below is completed and verified; do not close native/live gates
using fixture tests alone.

## Current checkpoint

Requested feature sequence completed: themed indent guides (`e988572`),
external-file protection (`27645e1`, `f5cd8c5`) and saved-file session restore
(`d3e4269`). Native macOS restart checks, the full 2,605-test suite and strict
lint passed for session restore. The external-file and session proposals are
archived with broader deferred ideas distinguished from their accepted slices.
See the [session record](docs/dev/refactoring-audit-2026-09-06.md#session-restore-implementation-and-native-restarts--2026-09-08).
This closes that feature sequence, not the unrelated remaining scope below.

Context-menu layout: `3c32795` sizes menus to labels/shortcuts, removes the empty
icon column and reduces menu chips by 20%. Native macOS rendering, the full
macOS suite and strict lint passed. The Linux rerun remains incomplete after
statistics-test timeouts and Docker becoming unavailable; see the
[verification record](docs/dev/refactoring-audit-2026-09-06.md#font-roles-and-context-menu-layout--2026-09-08).

Font split: `a06e1e7` adds file-configured `editor_font` (JetBrains Mono) and
`ui_font` (Inter). File explorer, tab text and all text inputs retain the editor
font. Full macOS/Linux suites and strict lint passed. Native macOS Settings
rendering and switching the UI back to the bundled monospace family were checked.

The reported missing editor I-beam remains open: editor hit testing still maps
to `CursorIcon::Text`. Native cursor-state restoration is not yet reproduced or
fixed; do not treat the font or context-menu layout work as resolving it.

Latest implementation: `5b9a502` fixes global shortcut chords across focus
contexts and makes Settings a global action. Linux X11 checks passed from the
terminal, command palette, file explorer and CSV cell editing, including intact
text input and cancelled CSV edits. Full macOS/Linux suites and strict lint
passed. See the [context record](docs/dev/refactoring-audit-2026-09-06.md#global-shortcuts-across-input-contexts--2026-09-08).

Latest diagnosis: an isolated zero-test binary showed a 3.02-second cold launch
and 0.01-second warm launch, with a matching macOS execution-policy delay.
Twenty hover-timeout repeats and five managed-server lifecycle repeats passed
unchanged. See the [launch/exit record](docs/dev/refactoring-audit-2026-09-06.md#macos-launch-policy-and-exit-warning-diagnosis--2026-09-08).
This establishes a host launch-delay mechanism, not the cause of every earlier
timeout or nextest handle warning. No timeouts or host security settings changed.

Latest verification: Linux X11 terminal tab creation, retained scrollback,
independent shell closure, normal-click suppression and Ctrl-click browser
opening passed. Both plain URLs and OSC 8 labels reached the loopback acceptance
server through the default browser. See the
[terminal record](docs/dev/refactoring-audit-2026-09-06.md#linux-terminal-tabs-and-browser-links--2026-09-08).
This completes the requested terminal interaction checks on Linux X11, not
Windows or Wayland. No application code changed in this verification pass.

Completed implementation history and detailed evidence are in the
[refactoring audit](docs/dev/refactoring-audit-2026-09-06.md) and
[benchmark reports](docs/benchmark/README.md), with earlier handoff entries
preserved in Git history.

Latest checkpoint: Linux X11 native verification fixed GNU scanner linking
(`3618108`), clipboard ownership and Alt-chord interception (`c104ed9`), and a
Unix PTY test prompt race (`f973884`). Final macOS/Linux full tests and lint
passed. See the [Linux record](docs/dev/refactoring-audit-2026-09-06.md#linux-native-verification-and-portability-fixes--2026-09-08)
for exact scope and the additional unresolved macOS process-exit warning.

Earlier implementation: `9d9f4fb` adds modifier-click terminal web links with
shared grid geometry, underline/pointer hover cues and a shared browser launcher.
Native macOS normal-click suppression, Cmd-hover and browser opening passed;
full tests and lint passed. Selection/copy (`0020f0e`) and session tabs
(`0620310`) also passed native macOS checks. The requested terminal enhancements
are implemented; see the [terminal verification](docs/dev/refactoring-audit-2026-09-06.md#terminal-modifier-click-links--2026-09-08)
and the updated [archived terminal plan](docs/archived/embedded-terminal.md).

Earlier implementation: `f0e1c4e` reuses coincident caret/selection coordinate
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

Use the repository's normal `target/` directory for all local builds and
verification artifacts. The user explicitly retired the earlier temporary
build-cache approach so `cargo clean` can remove build output in one place.
The stopped, task-owned Docker container `token-handoff-linux-20260908` retains
the Linux `/build` cache and toolchain. Its repository mount is read-only;
raw logs and isolated configuration are in `/tmp/token-linux-native.31IToo`.

## Remaining implementation

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

- Native Windows keymap chips, contexts, capture and persistence, plus the
  remaining Linux focus-context/Wayland matrix. Linux X11 editor-context chips,
  capture/cancel, pointer save, restart persistence and terminal selection/copy
  now have native evidence, as do global Settings chords from the terminal,
  command palette, file explorer and CSV cell editing. Launcher detachment and
  per-instance port files on Windows; managed llama-server startup/teardown on Windows/Linux; terminal
  tabs/selection/modifier-click on Windows and Wayland.
- CJK/emoji positioning outside IME composition and the remaining completion
  native-input matrix. Existing unit/macOS evidence is not a full platform
  certification. IME-specific work is deferred below, not a closeout gate.
- Live hosted model/provider compatibility, retrieval-context relevance,
  and Tabby service/model behavior. Fixture coverage is not live service
  verification.
- Resolve intermittent nextest process-exit warnings. The September 8 theme
  suite flagged `settings_page_keeps_spacious_categories_and_shared_control_hits`
  as leaky. The Find overview full run also flagged
  `supersession_disconnects_old_socket_and_serves_new_request`
  (`ec5fa5f6-f57b-4b4a-baa3-4b7b2436420d`). An intermediate macOS run
  also flagged `a_hover_request_past_its_deadline_is_abandoned_with_no_content`
  (`ce5fea14-e0ef-462e-8214-c0ad315f3540`). Later final macOS/Linux
  runs passed without warnings, but did not establish their cause. Do not
  suppress or call that resolved.
- Investigate the newly reproduced managed-server startup timeout and fake-LSP
  initialization timeouts under the September 8 stress runs. The managed test
  now retains the actual failure reply; ten later runs passed without timeout
  changes. This is distinct from nextest's output-handle warning. Failures,
  clean repeats and environment limits are in the native/exit record above.
  The terminal-tabs run also reproduced ten fake-LSP startup failures; a sampled
  discovery process was still in `_dyld_start` about 25 seconds after launch.
  See the terminal-tabs record for evidence; this is not yet a root-cause fix.
  The later launch-policy record above isolates a delay outside the test body;
  historical per-process policy logs were unavailable, so do not conflate that
  result with a proven explanation for all original failures. The runner's
  leak detector prioritizes reading output before its timer; no speculative
  cleanup patch or timeout suppression is justified by the current evidence.

## Deferred by user — not a closeout gate

Native IME composition, candidate windows and their positioning were explicitly
deprioritized on 2026-09-08 (near-zero priority). Do not resume this investigation
or hold handoff completion for it unless the user asks. The Linux IBus/Anthy
attempt was inconclusive: both Token and a standard GTK entry showed raw Roman
input, so the control environment was not validated. No application fix was
made or verified. The isolated editor/control were closed and the container
stopped; the fixture's unsaved synthetic input was discarded. Host input sources
were never changed. The autocomplete plan retains this as deferred scope.

## Archival and closeout

Completed Settings v1, soft-wrap, LSP baseline, Find and context-menu plans
already live under `docs/archived/`. Settings keymap remains active until its
cross-platform verification gate closes; autocomplete retains unfinished
Phase 5+ scope. Other planned features are not automatically in scope.

Keep changelog entries with changes and add dated benchmark reports under
`docs/benchmark/`. No release or publication is authorized. Once the checklist
above is genuinely closed, archive the eligible plans and delete this file.
