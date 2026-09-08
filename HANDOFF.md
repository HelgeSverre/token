# Handoff

Written 2026-09-04, at `9d34829` (7 commits past the `v0.6.0` tag, none pushed).

This file is temporary: delete it once **all** remaining work below is implemented
and verified. Durable findings belong in the audit docs and changelog.

## Settings scrollbar consistency — 2026-09-08

Commit `2a2eba6` keeps the dedicated Settings page and replaces its paint-only
three-pixel indicator with the shared editor scrollbar geometry, theme colors,
renderer and pointer mapping. Settings and list modals now support thumb dragging
and track clicks. Header-aware display-row mapping reaches the final setting
without changing selection. Capture ends on release, focus loss, modal changes
and resizing. Vertical/horizontal interaction messages and width constants are
consolidated.

The wheel path previously discarded delta magnitude and converted trackpad
pixels using the editor's much shorter text lines. Settings now preserves the
delta and uses its shared painted row height; fractional remainders reset when
the scroll units change.

The isolated commit passed 2,176 tests, two doctests, strict lint and formatting
(seven tests skipped, six doctests ignored). Run
`1af5499d-9f04-4161-b5fb-78e2a5923f0b`. Scoped diff review approved; staged patch
matched the independently verified source and preserved all working-file hashes.
The temporary verification checkout is removed. Nothing pushed or published.
This does not complete the other remaining handoff work below.

The final working-tree suite passed 2,568 tests and two doctests (seven tests
skipped, six doctests ignored), with no leaked-process warnings in this run:
`5b61ed4a-9dca-4f3d-a241-1e503f7aab2f`.
Final strict lint, formatting and debug build passed. Headless Settings screenshots
at 1100×720 and 400×750 were rendered and inspected; scrollbar and controls fit.
Pointer behavior was verified through tests, not a new native-app manual session.

## Settings design correction — 2026-09-07

The user explicitly wants the original **separate Settings page**, not a
command-palette-style replacement. Preserve its category sidebar, spacious form
rows, descriptions, right-aligned controls, boolean switches and compact-window
category navigation. The working tree restores `src/view/settings_page.rs` and
its shared paint/hit-test path, adapted to the current settings metadata. Keymap
controls belong in this page's Keymap category. Do not collapse this presentation
into the command palette during API consolidation.

The prior keymap core (`b6bad14`) and modal pointer-effect fix (`dbe3c63`) remain
intact. This correction does not complete the other remaining handoff scope.

Verification: 2,561 tests and two doctests passed (seven tests and six doctests
remain skipped/ignored), strict lint and formatting passed. Wide, 400-pixel
compact and short/high-DPI Settings screenshots were rendered and inspected.
The page-specific tests cover category and switch/action hit geometry; Settings
integration tests cover category filtering and forward/backward cycling.

Commit `5e702c8` records the page safeguards, shared Theme action hit geometry,
page keycap rendering and regression tests against the committed architecture.
Its isolated 24 Settings tests and strict lint pass. The current metadata/UI
integration remains with the other pending source groups; the working tree
preserves the restored page rather than the discarded palette presentation.

## Keymap registry and chord-context checkpoint — 2026-09-07

Commit `e017bd6` unifies bindable command names with the enum and default keymaps
with embedded YAML. The old parser omitted ToggleUsages and RestartLanguageServer;
the new regression fails before the fix and a generated test covers every variant.
Cached defaults produce independent override snapshots; the duplicate hardcoded
list is gone, with only Save/Open/Quit kept for emergency recovery. Merge and
Unbound behavior are unchanged and tested.

Commit `4a00fa5` shares context eligibility across single bindings, complete
chords and partial prefixes. Two regressions fail before the fix: inactive
branches wrongly return AwaitMore. They now return NoMatch/reset pending state;
eligible alternatives and successful completion remain intact. The documented
sidebar_focused condition and aliases now parse. Changelog and guide changes are
in each source group.

Independent suites passed **2,156** then **2,160 tests**, each with two doctests,
strict lint and formatting. Runs `14a21f1f-e01b-4a59-9357-6b9af7115b3f` and
`edafa848-0d58-4573-a3d7-c7146e0c3fbd`. Both red/green regressions and exact scope
are in the durable audit. Final main: **2,538 tests + two doctests passed**, strict lint/formatting clean,
run `1a87a6bd-9596-4f05-bc52-e2f0b0dff8b5`. Seven tests and six doctests remain
skipped/ignored; no final exit warnings.

Scoped self-review approved. Each staged/committed group matched its isolated
patch and preserved working-file hashes; the verification checkout is removed.
The independent context group retains the old one-keystroke dispatch API; the
main tree's multi-interpretation path uses the same shared eligibility check.
Command metadata/hints, chord YAML/runtime dispatch and new completion actions
still need their separate source groups.

These foundations do not complete Settings keymap: merged/searchable rows,
context/platform/prefix conflict analysis, capture with explicit save/cancel,
override persistence, base presets and native/platform gates remain. No additional
whole plan is complete. No new native or benchmark claim; earlier process-warning
debt remains. Continue all other remaining requirements below and keep this
temporary handoff until everything is implemented and verified. Nothing pushed
or published.

## Persistent usages panel checkpoint — 2026-09-07

Commit `7c1d55a` adds the persistent, file-grouped Usages dock panel. Find Usages
opens it; Show Usages keeps the cursor popup. Both share the existing references,
bounded preview and navigation pipeline. One row projection drives interaction,
rendering and automation. Loading, cancellation, unavailable/indexing servers,
timeouts, empty results and the 200-location limit are explicit. Query tokens
reject superseded replies; source edits/closure settle loading; caret/focus/dock
changes do not invalidate a search or let its reply steal focus/reopen the dock.
Completed results persist as snapshots until another Find Usages request.

Seventeen new regressions include real-stdio fake-server flow, server/root
cleanup, keyboard/mouse navigation, grouping, request guards, scrolling/paging
and clipped selection painting. Independent commit: **2,150 tests + two doctests
passed**, strict lint/formatting clean, run
`4ae7be60-393c-4f19-83f6-58c6e71e37a6`. Final main: **2,530 tests + two doctests
passed**, strict lint/formatting clean, run
`a373ab62-2f4d-4f67-8eef-64cfd9c2ae51`. Seven tests and six doctests remain
skipped/ignored. No final exit warnings; older warning debt remains open.

Scoped self-review approved; committed patch matches the independently checked
group, with working-file hashes preserved by staging. Temporary verification
checkout removed after comparison. Changelog/user guide are in the source group;
the durable audit and archived LSP checklist record the follow-up separately.
No new native-panel, live rust-analyzer or benchmark claim. Existing bottom/right
generic dock geometry is covered; left-dock focus routing is not generic painting.

The older checkpoints saying a usages panel remains are superseded here. Continue
dependency-ordered source commits and all remaining completion, Settings, Find,
theme, profiling and platform requirements below. Workspace-symbol runtime/UI still
needs its source group with the file-opening prerequisites. No additional whole
plan is complete, nothing pushed/published. Keep this temporary file until all
remaining requirements are implemented and verified.

## Usages-preview prerequisite checkpoint — 2026-09-07

Commit `50f3a68` moves usages previews off the event loop onto the shared
replaceable worker. Reads and previews are bounded; a 250 ms preview deadline
preserves navigable locations when preparation is slow. Unsaved buffers win,
identical targets deduplicate before the 200-row cap, and generation tokens plus
post-intent polling reject superseded replies. Eight reference regressions and
three worker regressions cover the boundaries, lifecycle and request ordering.

Independent group: **2,133 tests + two doctests passed**, strict lint/formatting
clean, run `0e816902-96ea-4571-b7a9-f0623f15bd7a`. Final main: **2,513 tests + two
doctests passed**, strict lint/formatting clean, run
`20638065-87be-48e3-a4d7-81947496810f`. No new exit warnings; older debt remains.
Review approved; durable details are in the audit. No native/performance claim.

The usages dock panel is still next; this fixes a blocking-I/O prerequisite,
not the panel itself. Continue all remaining work below and dependency-ordered
source commits. No whole plan newly complete, nothing pushed/published. Keep
this temporary file until all requirements are done.

## File identity prerequisite checkpoint — 2026-09-07

Commit `3c4ca44` shares document identity across LSP runtime/diagnostics and
Problems. It fixes lossy Unix filename URI collisions and root/parent handling.
The worker's native-path comparison/regression remains with async file I/O.
Independent group: 2,122 tests + two doctests passed, strict lint/formatting clean.
Final main: **2,504 tests + two doctests passed**, strict lint/formatting clean,
run `5f004685-3318-4c0b-9897-242d415374a9`. No new exit warnings; earlier debt
remains. Review approved; see the durable audit for scope and run IDs.

Continue file-I/O/startup/UI prerequisite commits and all remaining work below.
No whole plan newly complete, no new native/platform/performance claim, nothing
pushed/published. Keep this temporary file until everything is done.

## Workspace symbols checkpoint — 2026-09-07

Search Everywhere now has workspace symbols in the working tree: existing
capable servers, debounced/cancelled fanout, generation/query guards, bounded
deduplicated rows, visible partial failures, and shared navigation/geometry.
Unsaved changes flush before queries; cancellation cannot abandon a replacement
server's reused request ID. Changelog and user configuration guide are updated.

Logical commit `ebf2add` contains the standalone protocol foundation and its
typed-error dependency, verified independently: **2,115 tests passed**, seven
skipped; **two doctests passed**, six ignored, plus strict lint/formatting.
Runtime/UI source remains with the dirty file-opening/runtime prerequisite
groups. Do not sweep unrelated changes into a feature commit.

Final main suite: **2,498 passed**, seven skipped; **two doctests passed**, six
ignored, with 19 new workspace-symbol regressions. Run
`8315d727-cebf-41da-b099-8dc856018130`; strict lint, formatting and diff checks
passed. Review: Approve, no unresolved critical/high findings. No exit warnings
in these final runs; earlier startup/process warning debt remains unresolved.
See the durable audit for isolation/targeted run IDs and exact scope.

Next: dependency-ordered source commits and the remaining full scope below,
including a usages dock panel. No additional whole plan is complete. No new
native/platform/performance claim, and nothing pushed/published. Keep this
temporary handoff until all requirements are implemented and verified.

## Startup preparation checkpoint — 2026-09-07

Startup now shares runtime file preparation and message-based tab installation
with ordinary opens. Images/binaries use their normal special tabs, aliases reuse
documents, CLI order and first-success focus are preserved, and errors do not
discard successful tabs. Initial cursor positions clamp safely, including Unicode.
Workspace setup precedes recent-file recording; records reuse captured identities
instead of canonicalizing paths inside update handlers.

`AppModel::new(width, height, scale)` is now in-memory, with `with_document` for
prepared text. Configuration/theme/history loading moved to runtime preparation.
The duplicate startup loader and unused `InitialSession` surface are removed;
constructor consumers are migrated, with explicit file preparation in fixtures.
One fixture now drops its temporary directory after preparing its document.
Changelog and overview document the API and user-visible changes. Startup remains
on its existing preparation thread/fallback; no new persistence or service policy.

Six new regressions plus extended multi-file coverage pass. Final main suite:
**2,479 passed**, 7 skipped; **2 doctests passed**, 6 ignored, nextest run
`bdc8d49d-5db0-4cea-bb29-69deb813c7a6`, with no leak warning. Strict lint,
formatting and diff checks passed. Review: Approve, no critical/high findings.

Exit-warning debt is now named, not resolved: the six-test targeted run marked
`startup_files_report_failures_without_discarding_successful_tabs` leaky. A
20-iteration stress run passed all 120 executions but warned for
`multiple_startup_files_open_as_distinct_tabs` and
`startup_workspace_files_record_the_workspace_and_keep_an_empty_workspace_usable`.
No matching test process remained afterward. See the durable audit for run IDs
and exact scope; no timeout increases or warning suppression were used.

Source remains uncommitted with the file-open/identity/runtime prerequisites;
keep the constructor API migration and consumers together in dependency-ordered
groups. Continue the full autocomplete/LSP/Settings/Find/profiling/theme/platform
scope below. No new performance or native/platform evidence, no additional whole
plan ready to archive, and no push/publication. Keep this temporary handoff.

## Update-handler API commit checkpoint — 2026-09-07

Logical source commit `80e5538` narrows public message handlers and internal
LSP/syntax scheduling exports, retaining runtime/view helpers that still have
callers. Handler bodies are unchanged; modal tests use `update(model, Msg)` and
two compile-fail doctests protect the boundary. The changelog is included.

The exact patch passed independently against `87779d5`: **2,109 tests passed**,
7 skipped; **2 doctests passed**, 6 ignored, plus strict lint and formatting.
Nextest run `7fb86c7a-92cf-475a-903f-cc758bf85b47`. Review approved the group
after restoring a private LSP save-helper import needed by committed callers.
The temporary checkout was removed after matching its patch to the commit;
working files, other worktrees and native windows were preserved.

Full main verification: **2,473 tests passed**, 7 skipped; **2 doctests passed**,
6 ignored, with no leak warning, nextest run
`d14a8016-ad0e-4bc6-9916-1dc417ba97d1`. Strict lint, formatting and diff checks
passed. The earlier transient cleanup warning remains unattributed. No new
performance or native/platform claim. Durable details are in the audit.

Continue dependency-ordered source commits and all remaining requirements below.
Completion, file-I/O, settings and rendering work remains outside this commit.
No additional whole plan is ready to archive; keep this temporary handoff until
everything is implemented and verified. Nothing was pushed or published.

## Unused editing API commit checkpoint — 2026-09-07

Logical source commit `96c3399` removes the unused `Msg::TextEdit` dispatcher,
`EditContext`/`TextEditMsg` bridge and `RopeBuffer` wrapper. Committed-code searches
found no producers/consumers outside those definitions, exports and their tests.
Existing document/modal/CSV handlers remain unchanged. The shared `editable/`
foundation is now entirely committed, following `7552b7e`'s primitive unification.

The group includes its changelog entry, corrected overview and historical labels
for synthetic rope-line benchmarks. Review preserved the retained `StringBuffer`
clear test that the broader deletion had accidentally removed. No new performance
or native/platform claim, dependency or external service change.

The exact patch passed independently against `7d7d9d2`: **2,109 tests passed**,
7 skipped; doctest target succeeded with six ignored examples. Nextest run
`a6ca2fb5-7401-4247-86b1-42aa40c23e41`; strict lint, formatting and diff checks
passed. The 24-test reduction is limited to tests of the removed API. Staging
preserved working-file hashes. The temporary checkout was removed after its diff
matched the source commit and it contained no extra files; Git preserves all
removed source. Other worktrees and native windows were not touched.

Full main verification: **2,473 tests passed**, 7 skipped; **2 doctests passed**,
6 ignored, with strict lint and formatting clean. First run
`08770a77-7349-424e-9c46-d35653dc365a` had one unidentified passing-but-leaky test;
a full rerun with leak-level output passed without warnings
(`0b8b09bc-808b-41de-b597-129006de3467`). The transient was not attributed or
fixed. Use `--status-level leak --final-status-level fail` in future full runs
to retain the affected test name if it recurs. Durable audit records both runs.

Continue dependency-ordered commits for the remaining editor/runtime/completion
work, then the full open feature scope. Tabby and menu configuration remain
implemented but uncommitted with those prerequisites. No additional whole plan is
ready to archive, and this temporary handoff must remain until everything is done.

## TabbyML transport checkpoint — 2026-09-07

The missing TabbyML transport is implemented as `transport: tabby`, reusing
`FimProvider`, shared routing/choices parsing and the existing worker pipeline.
Its native segments request, optional language IDs, explicit bearer credentials,
gateway paths and server-side model selection follow the checked upstream API.
No model downloads, automatic startup, telemetry or absolute file paths were added.
Recent-buffer context retains the shared opt-in commented-prefix fallback;
workspace-relative metadata and declaration/search attachment remain retrieval work.

An empty Tabby choices array now means no suggestion, not a failure contributing
to automatic-request backoff. Other transports retain their validation. Eight new
regressions cover wire/configuration, bounded alternatives, deadlines/body limits,
real socket cancellation, worker partial acceptance/Undo and empty-result state.
Existing recency, capability and response-error tables also include Tabby.

Final full suite: **2,472 passed**, 7 skipped; **2 doctests passed**, 6 ignored.
Nextest run `2921ce4a-f9e9-4c95-b5ec-96527c7692e1`; strict lint, formatting and
diff checks passed. Changelog, user guide, active checklist and durable audit were
updated. No live model quality, new native/platform or performance claim.

The adapter source remains uncommitted with the HTTP/provider/editor foundations;
keep grouping those prerequisites before the dependent feature commit. Prior
source commit `7552b7e` and audit `af4fe0d` remain intact. Server supervision,
retrieval/edit prediction, full snippets, LSP symbols/usages, Settings keymap,
Find/file boundaries, theme tuning and native/platform work are still open.
No additional whole plan is ready to archive. Keep this temporary file.

## Shared editor primitives checkpoint — 2026-09-07

Logical source commit `7552b7e` shares cursor, position and selection types between
documents and small editable fields. It removes duplicate definitions, migrates
constructor consumers and includes a Unicode/selection/desired-column regression
and changelog entry. It deliberately excludes unrelated model, viewport, editing
transaction and completion changes in the same working tree.

The exact patch passed independently against `5d4a818`: **2,133 tests passed**,
7 skipped; doctest target succeeded with six ignored examples. Nextest run
`4b42e9b1-87b8-4f99-9083-4844f21a9b67`; strict lint and formatting passed. The
temporary checkout was removed after its patch matched the source commit exactly.
Working files and other worktrees were preserved.

The main working tree then passed **2,464 tests**, 7 skipped; **2 doctests passed**,
6 ignored (`88462a6b-49a3-4f09-8375-d9d4e8f33cb6`), plus strict lint, formatting
and diff checks. Durable audit records both scopes separately. No new performance
or native/platform evidence is claimed; no push or publication occurred.

Continue dependency-ordered source commits and the complete feature scope below.
Menu configuration remains implemented but uncommitted with its foundations.
No additional whole plan is ready to archive; keep this temporary file until all
requirements are actually implemented and verified.

## Menu configuration checkpoint — 2026-09-07

The `completion.menu` gap is implemented: automatic dropdown opening, minimum
candidate word length (default three characters, floor one) and local-word policy.
The existing master `completion.enabled` retains its global semantics. Disabling
automatic menus still allows Ctrl+Space and open-session refinement, manual path
continuation, signature help and configured inline suggestions. The minimum is
for candidate identifiers, not the existing two-character typed-prefix trigger.

Legacy `completion.words` migrates into `menu.words` on save; explicit nested
values win, partial menu blocks retain legacy preferences, unknown settings are
preserved, and invalid configs are not overwritten. One runtime field and one
collector entry point replace the old policy location and hardcoded minimum.
Folder acceptance now retains the path session's explicit-request provenance.

Nine new regressions pass. Full suite: **2,463 passed**, 7 skipped; **2 doctests
passed**, 6 ignored. Nextest run `9bb31d07-8432-4afa-a0bc-afa146ca0a09`; strict
lint, formatting and diff checks passed. Changelog, user guide, active checklist
and durable audit updated. No new native/platform or performance claim.

This source remains uncommitted with the completion/editor foundations; stage
it in dependency order rather than sweeping the dirty tree. Full snippets,
retrieval/edit prediction/transports, LSP symbols/usages, Settings keymap,
Find/file boundaries, remaining theme tuning and native/platform requirements
stay open. No additional whole plan is ready to archive. Keep this temporary file.

## Grouped hover commit checkpoint — 2026-09-07

Popup hover is committed as `e08ecb4`, separate from completion filtering,
documentation scrolling, settings/keymap and editor transactions. The seven-file
patch contains popup-owned hover state, shared hit-index painting, idle and
window-exit repaint, state/pixel regressions, changelog and archived-plan notes.

A temporary detached checkout verified that exact patch against committed code:
**2,132 tests passed**, 7 skipped; doctest target succeeded with six ignored
examples. Strict lint and formatting passed. Nextest run:
`fadc609f-5306-4d68-89f9-294254c73917`. This is the isolated commit's suite, not
the larger working tree's 2,454-test suite. The temporary checkout was removed
after its diff matched the commit exactly; no working-tree source was changed.

The two ignored process-spawn tests were also run serially against the current
main working tree: both the real-child LSP handshake and PTY exit notification
passed on macOS (`83ab2760-a560-4246-9448-e6e85205ef50`). Reproduction and limits
are in the durable audit. Default ignores remain pending load-stability evidence;
this does not establish Windows/Linux or native pointer/IME acceptance.

Continue dependency-ordered source commits and the complete remaining feature
scope below. No additional whole plan is ready to archive. Keep this temporary
file until all requirements are verified. Nothing was pushed or published.

## Documentation viewport checkpoint — 2026-09-07

Completion documentation now scrolls independently across all wrapped code and
prose. The card has a visible-row footer, click/F1 expansion and Alt+PageUp/PageDown
paging. It fits available side space without covering the menu and uses shared
measured geometry for painting and input. New selection resets the card; no-op
navigation and late local-path replies retaining the server item preserve it.
Fresh wheel hit testing and focus/modal dismissal guard stale popup actions.

All ten viewport regressions pass. Final full suite: **2,454 passed**, 7 skipped;
**2 doctests passed**, 6 ignored. Nextest run
`c56440ca-f578-4541-b91f-b03582ee29a9`; strict lint and formatting passed. Tests
include actual rendered pixels, but native keyboard/pointer/IME acceptance is
still open. No native window was driven and no new timing is claimed.

Changelog, user guide, active autocomplete checklist and durable audit updated.
Menu documentation richness is now checked; the whole autocomplete plan remains
unfinished. The source depends on uncommitted completion/overlay foundations;
stage these in logical dependency order, not as an indiscriminate tree commit.

The independent Mermaid implementation is committed as `6c9a9f1`, including
escaped-source tests, the JavaScript renderer, sample spacing, its changelog
entry and archived-plan follow-up. Existing browser/WKWebView evidence remains
in the durable audit; this checkpoint reran the Rust suite and JavaScript syntax
check, not native preview interaction. Nothing was pushed or published.

Next: grouped source commits, full snippet placeholders, remaining retrieval/edit
prediction/transports, LSP symbols/usages, Settings keymap, Find/file boundaries,
theme/configuration and native-platform requirements. Keep this temporary file
until the complete remaining scope is verified; no additional whole plan is
ready to archive.

## Documentation Markdown checkpoint — 2026-09-07

The documentation-formatting part of menu richness is implemented. Completion,
hover and signature cards share the preview's existing CommonMark parser and
native `StyledText` output. Nested formatting, matching fences, literal code,
escapes/reference links, lists/tasks/quotes, tables and footnotes are covered.
Four custom Markdown grammar helpers were removed; diagnostic backtick-only
formatting remains separate. No new dependency, public API or HTML surface.

All **14 converter tests passed**, including seven new regressions. Full suite:
**2,444 passed**, 7 skipped; **2 doctests passed**, 6 ignored. Nextest run
`8e1f074c-82d5-4749-80f3-c2ecd8b666d6`; strict all-target/all-feature lint passed.
The durable audit records representation changes, review and verification scope.
Changelog, user semantics and active autocomplete checklist were updated.
Focused commit `ee9bbb0` contains the converter, its directly related client test,
changelog entry and durable audit. The user-guide/checklist updates remain with
the broader uncommitted completion work. Formatting checks passed; unrelated
capability changes in the same client file were deliberately left unstaged.

Next: long-document scrolling/expansion in the shared card surface, then full
snippet placeholders and the remaining retrieval/edit prediction/transports,
LSP symbols/usages, Settings keymap, Find/file boundaries, theme/configuration
and native-platform requirements. The full menu-richness item stays unchecked.
No native window was driven and no new performance measurement is claimed.
Keep this temporary file: no additional whole feature plan is complete.

## Path-source completion checkpoint — 2026-09-07

Context-aware filesystem completion is implemented on the existing dropdown and
edit transaction. Ordinary quoted strings wait for current syntax; plain text
and Markdown destinations recognize path-shaped prefixes. Relative paths use the
current file or workspace, with absolute/home-relative support. Recognition and
post-decode validation exclude comments, regexes, URLs and unsupported path forms;
Unicode, quoted spaces and percent-encoded Markdown filenames are covered.

Directory reads use a bounded speculative worker, not the ordered save queue.
Find and paths share the latest-request lifecycle: one running/one pending job,
cancellation, nonjoining shutdown and panic-to-failure delivery. Running regex
scans still lack mid-computation interruption. The source caps scans/results/name
bytes and guards document/pane, language/path, workspace, revision, all cursors,
focus/selection and request ownership. Late replies cannot reopen a closed menu.

Server completions, aliases and resolve semantics remain authoritative alongside
local paths; matching insertions are deduplicated. Acceptance replaces the full
component, continues directories and shares one-step Undo/Redo and peer mapping.
Tests cover UTF-16/import edits and resolved/deferred commit characters, including
multiple cursors and a syntax reply arriving during pending acceptance.

Verification: **23 targeted tests passed** on macOS. Final full run: **2,437 passed**,
7 skipped; **2 doctests passed**, 6 ignored. Nextest run:
`15505665-158d-4f79-8334-2bbf246a1df2`. Strict all-target/all-feature lint passed.
An initial full run exposed the existing statistics-writer test's assumption that
a documented 200 ms busy-lock timeout could never occur. Its test now retries only
that uncommitted outcome within a bounded deadline; the exact 32-update assertion,
production timeout and failure behavior remain unchanged. Subsequent full runs pass.
The Linux non-UTF-8-name fixture is present but was not run here; native platform
and IME gates remain open. No new performance measurement is claimed.

Changelog, user semantics, autocomplete checklist, benchmark caveat and durable
path-source audit updated. Next: richer menu documentation and full snippet
placeholders, followed by the remaining retrieval/edit prediction/transports,
LSP symbols/usages, Settings keymap, Find/file boundaries, theme/configuration
and native-platform requirements below. No additional whole plan is ready to
archive. Keep this temporary file until the full remaining scope is verified.

## Commit-character acceptance checkpoint — 2026-09-07

Commit-character input and deferred acceptance are implemented. The client now
advertises `commitCharactersSupport` while full snippet support stays off. Only
server/item-declared characters on an eligible visible LSP row can accept;
ordinary words/snippets have no guessed punctuation set. Single-character keys
commit; paste/`InsertText` and multi-character keyboard payloads do not.

An unresolved item keeps the typed character visible and uses the existing
resolve worker. A private completion lifecycle checks revision, document/pane,
file/language, cursor/selection/menu state, history depth, configuration and focus.
Further input/navigation or changed identity invalidates the reply without losing
typed text. Valid replies reuse the existing pristine-coordinate edit planner:
the literal is retracted only inside one update, then completion/imports/character
are applied, and the three history batches coalesce into one Undo step. No full
document/history clone, new range mapper, transport or input queue. Multi-cursor
acceptance retains the existing plain-text fallback, not repeated absolute imports.

Review fixed two related races: menu navigation withdraws an earlier Enter
acceptance; the literal's effects run before resolve because a missing server can
accept synchronously, otherwise overwriting the final syntax deadline with an
older revision. Follow-up signature/completion requests use the final caret.
Final review also reproduced a stale status-bar position caused by an early
return; commit input now goes through shared status/wrap finalization. Assertions
cover both immediate acceptance and the visible pending character's position.

Fourteen new tests cover resolved/deferred acceptance, CRLF/astral Unicode,
imports and suffix edits, snippet carets, exact multi-cursor/peer Undo/Redo,
cancellation/identity guards, stable pending states, selected-row changes,
trigger positions, keyboard payload dispatch and runtime reply/timeout/missing
server behavior. Targeted: **15 passed** including one existing metadata test.
Full suite: **2,414 passed**, 7 skipped; **2 doctests**, 6 ignored. Strict lint
passed. Final logs: `/tmp/token-commit-character-final-{targeted,full,lint,fmt}.log`.
The finalization regression failed before the fix in
`/tmp/token-commit-character-finalization-before.log` and passed afterwards.
Changelog, user semantics, autocomplete checklist and durable audit updated.
Scoped review: Approve; native IME/platform checks remain open, not claimed tested.

At this checkpoint path completion was next; it is now implemented above.
The remaining autocomplete scope includes
(richer menu docs, full snippet placeholders, retrieval/edit prediction and
provider transports), alongside the existing LSP symbols/usages, Settings keymap,
Find/file-boundary, theme/config and native-platform requirements below. No new
plan is wholly complete; retain this temporary handoff. No native window input,
new performance claim, dependencies, staging, commit or publication in this task.

## Benchmark documentation checkpoint — 2026-09-07

Later refresh, completed by 13:54 CEST: reran the existing optimized completion
keystroke probe after commit-character implementation. All three 100-sample cases
passed: 5.635/24.62/95.07 µs median with 0/200/1,000 carried server items. The dated
report contains a separate addendum and complete measurement tables; source
fingerprints matched before/after. This measures ordinary typing, not acceptance
or native latency, and is not a controlled speedup comparison. Original broad
measurements remain unchanged; no additional feature plan is complete.
Saved output equality, 19 benchmark links/anchors, historical baseline
preservation, scoped Markdown formatting and `just fmt-check` passed. Scoped
documentation review: Approve; no application tests were needed for this increment.

The user's benchmark-documentation addition is done. The guide and historical
August baseline now live under `docs/benchmark/`; `README.md` indexes dated
reports. `2026-09-07-current.md` records fresh optimized edit/history, Find,
cursor/render and completion probes, plus an independent edit/history repeat.
All five runs exited successfully: 121 distinct stage/input cases and 54 repeat
measurements. Their full outputs are retained under `data/2026-09-07/` beside
the report, with hardware/toolchain/dirty-source fingerprints and exclusions.

Current findings: 100,000-line Find cache misses cost 6.8–7.2 ms median versus
0.37 ms for the pending typing frame and 12.24 ms for the synthetic full CPU
roundtrip. Converting 1,000 completion items costs 0.96 ms and 38,002 fresh
allocations / 4.398 MB; near-duplicate idle refresh reaches 4.69 ms at 32 snippets.
Two-pane, 1,000-cursor forward edits cost about 2.2–2.7 ms versus roughly
0.14–0.17 ms for Undo/Redo. These are descriptive stage measurements, not a
controlled speedup claim, native latency measurement or root-cause profile.

An external documentation commit advanced HEAD from `6598017` to `100c0a1`
during the run. Source and lockfile fingerprints stayed identical; Git-version
embedding triggered a rebuild. The external index/conflict resolution was left
alone. No application/benchmark source was changed, no dependencies added, no
editor windows driven, and nothing staged, committed or published by this task.
Changelog and links updated; historical values preserved; formatting, copied
logs and report links checked. Scoped self-review: Approve for the docs increment.

**The broader handoff remains unfinished.** Commit-character input was the next
implementation at this benchmark checkpoint and is completed above. No additional
feature plan became complete from running benchmarks; retain the remaining scope.

## Completion metadata checkpoint — 2026-09-07

Commit-character work exposed a prerequisite correctness bug: conversion dropped
`additionalTextEdits` from the initial LSP response. A new real-update regression
failed with the completion inserted but its import absent. Conversion now keeps
those edits; ordinary single-cursor acceptance and an empty/failed resolve retain
them in the existing transaction, including one-step Undo and caret restoration.
The existing multi-cursor plain-text fallback is unchanged and documented.

The same conversion boundary now receives `Option<&CompletionOptions>` instead
of a pre-derived resolve boolean. It derives resolve support and captures the
effective commit-character set together. Explicit item lists, including empty
ones, override `allCommitCharacters`; missing lists inherit it. Entries must be
one Unicode scalar, are sorted/deduplicated, and inherited sets share an Arc.
The original raw item sent to resolve remains unchanged. Runtime, benchmarks and
test callers use this one options-snapshot path; no parallel capability mirror.

Five added tests pass, including the before/after import regression, item/default
precedence, invalid entries, literal additional edits, raw-payload preservation
and shared sets. Full suite: **2,400 passed**, 7 skipped, plus **2 doctests**,
6 ignored. Strict lint and formatting passed. Logs:
`/tmp/token-completion-metadata-{before,targeted,after,full,lint,fmt}.log`.
Scoped self-review: Approve for this conversion prerequisite, not for the whole
commit-character feature. Changelog, guide and durable audit updated.

Commit-character input was pending and unadvertised at this conversion checkpoint.
The acceptance checkpoint above records the completed lifecycle, guards and
transaction/routing tests; it does not skip resolution or lose known imports.
No plan became ready for archival from the conversion prerequisite alone.

## Local inline statistics checkpoint — 2026-09-07

Local acceptance statistics are implemented and verified. Each offered response
records at most one terminal outcome: first explicit full/word/line acceptance,
dismissal, or complete manual typing-through. Alternatives and additional partial
accepts do not double count. Attribution captures the configured provider name at
request time; opt-out discards the current observation. No source, suggestion
text, connection settings or network telemetry enter the aggregate JSON.

The existing ordered file worker merges `inline-statistics.json` under a stable
sidecar lock, writes an exclusively created temporary file, then replaces the
old JSON. Limits: 256 KiB, 256 providers, 256 UTF-8 bytes per provider name;
counters saturate. Lock retries and temporary-file creation retries are bounded.
Malformed/newer-format files and user symlinks are preserved. Failed writes are
not replayed and notify non-modally once until a successful write rearms them.
“Open Inline Completion Statistics” opens a normal file snapshot, not a live
dashboard; existing open buffers are preserved. Configuration and Settings can
disable collection (default on, while inline completion itself defaults off).

Twenty added tests cover outcomes, opt-out, attribution, Unicode, config/action
mapping, concurrent independent file handles, file bounds/preservation, errors
and ordered-worker draining. Final `just test --no-fail-fast`: **2,395 passed**,
7 skipped, plus **2 doctests**, 6 ignored. Strict `just lint` passed. Logs:
`/tmp/token-inline-statistics-{full,lint,fmt}.log`. The first full attempt was
stopped during compilation to isolate old fake-provider tests; those now disable
statistics, and persistence fixtures use temporary directories. Scoped
self-review: Approve after opt-out, retry-bound and test-isolation corrections.
No new dependency, separate worker, native input or performance claim.

Changelog, user guides, autocomplete checklist and durable audit updated. The
autocomplete plan is not ready for archival; keep this temporary handoff until
the remaining edit prediction, retrieval, transports, path/commit/snippet work,
LSP symbols/usages, settings keymap, Find/file boundaries, theme/config debt and
native IME/platform gates are done. Nothing committed or published.

## Indexed forward-mapping checkpoint — 2026-09-07

The measured forward per-edit/per-cursor scan is replaced by an internal
`EditOffsetMap` in the existing transaction module. It counts payload characters
and records final starts once per position set, then uses binary search for old
offsets. Ordinary final carets, LSP completion and shared peer mapping use it.
Duplication resolves each copy's caret from its final start without repeated
suffix scans. Equal-point order and ownership, replacement clipping and undo
snapshots are unchanged. Find's two endpoints and actual-order history for new
panes remain sequential where required. The obsolete scalar batch-scan API is
removed; no new public API/dependency or persistent state was introduced.

The Rust oracle compares 42,601 valid Unicode-payload batches / 426,010 old
offsets plus every inserted-copy offset with sequential mapping. A separate
scope/position oracle covers zero/one/two panes, touching replacements/deletions,
same-point inserts and no-ops. Targeted tests passed, then **2,375 full tests**,
7 skipped, plus **2 doctests**, 6 ignored; strict lint and formatting passed.
Logs: `/tmp/token-forward-map-{targeted,full,lint,fmt-check}.log`. Scoped
self-review: Approve, excluding the rest of the dirty worktree.

Both unchanged optimized `edit-history` probes completed all 54 reports and
fixture assertions. At 1,000 cursors, deletion/duplication edit medians changed
from 2.78–6.99 ms to 0.96–2.51 ms (2.3–3.9× in these CPU fixtures). One-cursor
medians were unchanged or up by at most 0.167 µs; 100-cursor cases improved about
4–8%. Temporary index allocation is a tradeoff, not a universal speedup.
Residual forward cost needs further profiling before attributing it. Undo/Redo
was not optimized again in this checkpoint. Before/after logs:
`/tmp/token-forward-map-{baseline,optimized}.log`; methodology, full high-cursor
table and limits are in the top of `docs/dev/refactoring-audit-2026-09-06.md`.

Changelog, benchmark guide and undo contract updated. No new plan is ready for
archival. Keep this file: inline Phase 5+ (edit prediction, retrieval and remaining
providers/features), LSP symbols/usages, settings keymap, cold Find, theme/config,
file boundaries and the native IME/platform matrix are still outstanding. The
prior isolated native test window was not touched. Nothing committed/published;
no cache or unrelated file deleted. About 20 GiB remain free.

## Diagram/hover fixes and history profiling checkpoint — 2026-09-07

The two newly reported UI issues are implemented: Markdown Mermaid fences render
with theme colors and escaped-source fallback; all cursor-popup lists share
popup-owned hover state and request repaint on row transitions/window exit.
The reported state-diagram label collision reproduced on a blank page without
editor CSS. The sample uses diagram-local rank spacing to avoid it; this does not
claim a general Mermaid layout fix. Mermaid 11.17.2 loads on demand from jsDelivr,
so CDN access is required. Chrome dark/light, invalid-then-valid and offline
checks passed, and macOS WKWebView rendered three diagrams through `token://`.
Context-menu runtime and pixel regressions passed. The guarded native pointer
attempt sent no events because another window obscured the test window; native
pointer acceptance is not claimed. The isolated editor (PID 3636, socket
`/tmp/token-preview-hover.JiI6EZ/editor.sock`) remains open to preserve typing in
its scratch `state` tab. No normal user configuration or source was changed.

Final suite **2,373 passed**, 7 skipped, plus **2 doctests**, 6 ignored; strict
lint, formatting and JS syntax checks passed. Logs:
`/tmp/token-preview-hover-final-reviewed-{full,lint,fmt}.log`. Scoped self-review:
Approve, with the native verification limit above. Changelog and already-archived
preview/context-menu docs updated; durable findings are at the top of
`docs/dev/refactoring-audit-2026-09-06.md`.

Deletion/duplication and separate Undo/Redo release profiling is now complete.
Filtering out panes restored from exact snapshots removes redundant history
mapping while preserving new-pane and Find-scope mapping. At 1,000 cursors the
measured Undo/Redo medians fell from 3.23–6.83 ms to 0.132–0.161 ms; forward edits
remain 2.66–6.99 ms. These are unsaved-scratch CPU stages, not end-to-end latency.
Both 54-result runs completed; the audit records the full comparison, methodology
and passing 2,370-test pre-UI verification. General forward high-cursor mapping
optimization remains separate work.

Keep this file: the remaining scope below is not complete (native IME/platform
checks, autocomplete Phase 5+, LSP symbols/usages, settings keymap, cold Find,
forward mapping, theme/config and file boundaries). No further active plan is
ready to archive. Nothing committed/published; no caches or unrelated files
deleted. About 21 GiB remain free.

## Lossless per-pane undo checkpoint — 2026-09-07

Full per-pane selection/active-index undo snapshots are implemented. Batch
history now records editor-ID-keyed before/after cursor and selection vectors,
including desired columns. Undo/Redo restores each surviving pane's own state
regardless of focus, recovering endpoints clipped by deletion. New panes keep
mapped live positions; closed panes are not recreated. Typing captures original
overlapping selections before normalization. The old cursor-only batch fields
and unconditional post-history selection collapse are removed. No second history
stack, dependency or whole-editor/rope snapshot was added.

Three initial regressions failed before the fix. All five final tests cover
Unicode/CRLF, cursor merging, directional selections, active/desired columns,
focus changes, repeated Undo/Redo, branch replacement and new/closed panes.
Full suite: **2,369 passed**, 7 skipped, plus **2 doctests**, 6 ignored
(`/tmp/token-undo-pane-full.log`); strict lint passed
(`/tmp/token-undo-pane-lint.log`). Scoped self-review: Approve. The undo contract,
Unreleased changelog and durable refactoring audit are updated. History retains
additional per-pane selection/cursor data proportional to edits and cursors;
high-cursor mapping optimization is still separate work.

The existing optimized insertion probe passed all fixture assertions:
`/tmp/token-undo-pane-profile.log`, 500 samples/case, setup/effects/rendering
excluded. Single-cursor medians are 2.708/3.875 µs in one/two panes; 1,000-cursor
medians are 3.669/6.540 ms. The audit records every size and p95. These are
current-state costs including snapshots, not isolated snapshot overhead or an
undo/heap benchmark. The release build took 6m 03s with the pre-existing unused
`revision` warning. About 21 GiB remain; no native UI or user config was changed
in this checkpoint.

Remaining scope is unchanged except for closing this undo correctness gap:
native IME/platform verification, the rest of autocomplete Phase 5+, LSP
symbols/usages, settings keymap, cold Find, high-cursor and deletion/duplication
profiling, theme/config and file-boundary work below. No additional active plan
is ready to archive. Keep this temporary file until **all** work is verified.
Nothing committed or published; no caches or unrelated files deleted.

## Ghost native/profiling checkpoint — 2026-09-07

The existing projection now has isolated macOS keyboard, pointer and resize
checks: alternatives, Tab, Undo, matching type-through with word completion
enabled, partial acceptance, navigation, ghost/suffix hit mapping and narrow
soft wrapping passed. A loopback fixture supplied known candidates; this is not
model-quality validation. Actual IME composition/candidate-window behavior is
still unverified: Option+e inserted a direct accented character on this layout,
and another attempt did not establish composition. Keep that gate open.
The exact scratch source was restored, the isolated editor quit and its server
stopped. No normal user config/input source was changed. Native artifacts remain
in `/tmp/token-ghost-native.E0ElXY/`.

`benches/completion.rs` now profiles arrival, cycling, blink, type-through, width
refresh and warm visible/plain frames through existing production paths. Setup
asserts on-screen projected rows after font/viewport refresh; an initial
offscreen render fixture was corrected. Final release output is
`/tmp/token-ghost-profile-reviewed.log`: ordinary arrival 5.275/6.733 µs at
100/10,000 source lines; cycling 4.463/4.589 µs; 65,536-character anchor arrival
265.7 µs and type-through 414.2 µs. Full width reflow at 10,000 lines is 3.328 ms.
These are local CPU stages, not network latency, a speedup or FPS. Different
plain/ghost glyph workloads cannot establish incremental rendering overhead.
No new runtime optimization or public profiling API was warranted by this pass.

Full suite: **2,364 passed**, 7 skipped, plus **2 doctests**, 6 ignored
(`/tmp/token-ghost-profile-tests.log`); strict lint passed
(`/tmp/token-ghost-profile-lint.log`). Durable methodology and results are in
`docs/dev/refactoring-audit-2026-09-06.md`; benchmark guide, changelog and active
plan/sprint queue are updated. Scoped self-review: Approve, excluding remaining
IME/platform verification. About 21 GiB free; no cache or unrelated file deleted.

Archival recheck found no additional completed active plan. Soft Wrap, Damage
Tracking, Command Palette and Settings v1 remain archived with their deferred
scope active. Keep this file until **all** remaining work is verified. Next:
remaining native IME/platform verification and Phase 5+ implementation, plus all
LSP symbols/usages, settings keymap, cold Find/high-cursor/undo, theme/config and
file-boundary work below. Nothing committed or published.

## Multi-row ghost projection checkpoint — 2026-09-07

Multi-row and mid-line insertion previews are implemented. `model/ghost_text.rs`
reflows only the anchor logical line using shared wrap segmentation and Rope
line boundaries. The focused pane stores an immutable derived projection;
`TextViewportMap` supplies source/display conversion to painting, hit testing,
IME anchors, scrollbars and reveal. Ghost hits map to the insertion anchor.
Source fragments keep syntax, selections, brackets and diagnostics off ghost
text. Suffix glyphs and following lines move visually; source and undo history
remain untouched until acceptance. The obsolete first-line/badge helper is gone.

Projection changes request editor-area damage; steady blink/scroll reuse geometry.
Type-through, acceptance, alternatives, focus/split cleanup and resizing are
covered. Navigation clears the projection before moving; rectangle selection
rebases its display anchor. Review found and fixed automatic dropdown collection
hiding a compatible mid-line remainder, and direct runtime font/gutter refreshes
dropping the preview outside `update()`. Those refreshes now reflow it too.
Automatic suffix gating remains unchanged; explicit requests support mid-line.

Final verification: **2,364 tests passed**, 7 skipped, plus **2 doctests**, 6
ignored (`/tmp/token-ghost-full-reflow.log`); strict all-target/all-feature lint
passed (`/tmp/token-ghost-lint-reflow.log`). Seven tests were added. The geometry
oracle compares against actual insertion across every fixture source position,
wrap widths, tabs, Unicode, CRLF, lone CR, Unicode separators and blank lines.
Pixel tests compare cursor-line blink with full rendering and check source-only
decorations. Additional tests cover scroll/replacement identity and lifecycle.
Earlier `targeted-v2`/`full` failures are superseded: type-through was corrected
and an old expectation that stale state lingered after navigation was updated.
The `full-reviewed` build failed on four test callers after consolidating
`cursor_visual_line` onto the shared map; all callers are fixed in `full-reflow`.

The new `screenshots/scenarios/inline-multiline.yaml` was rendered and inspected
at `/tmp/token-ghost-screenshots/screenshot-inline-multiline.png`. The screenshot
fixture now uses production update/resize synchronization. This is headless,
not a native GUI/IME check or a model-quality claim. The durable implementation
record is in `docs/dev/refactoring-audit-2026-09-06.md`; changelog, user guide,
active plan and sprint queue are updated. No caches or unrelated files deleted,
no user configuration changed, nothing committed or published. About 22 GiB free.

Next: native keyboard/mouse/IME validation and release-stage profiling of the
projection, then the rest of Phase 5+ (edit prediction, retrieval, providers,
acceptance stats, paths, commit characters and full snippets). All other LSP
symbols/usages, settings keymap, cold Find/high-cursor/undo, theme/config,
file-boundary and native platform work below remains active. No additional plan
is ready to archive. Keep this temporary file until **all** work is verified.

## Recency profiling checkpoint — 2026-09-07

Release profiling found a real idle-thread hotspot: full refresh of 32 distinct
8 KiB snippets took median 38.77 ms and reported 121,120 allocations. Runtime now
indexes each captured snippet once using sorted token byte ranges, then performs
exact merge/intersection comparisons with a strict-threshold upper-bound exit.
The same fixture now takes 1.900 ms (about 20x for this stage), with 128 allocations
plus bounded vector growth/shrink work. Default 8-chunk refresh is 2.326 ms to
442.1 us. Ordinary observation and attachment are essentially unchanged.
Near-duplicate inputs that force long comparisons measure 580.1 us / 4.655 ms
for 8 / 32 chunks; no baseline comparison or universal worst-case claim is made
for that added corpus. These are instrumented local release-stage timings, not
network/model latency, KV-cache reuse or GUI frame rates.

`IndexedChunk` is private to `runtime/inline_context.rs`. It owns immutable text
and token ranges; no hashes, source-token copies or public API were added. Indexes
are not transmitted or retained by the completion cache. Retained range metadata
has a conservative 2 MiB upper bound at the maximum configuration on 64-bit
builds, plus bounded capture scratch, source and labels; the guide records this
tradeoff. Similarity, idle scheduling, source scope and payload bounds are unchanged.

Existing `benches/completion.rs` compiles the production private module directly,
with no second algorithm or public profiling seam. `just bench-completion recency`
forwards filters through the existing recipe. Fixtures exclude setup, config I/O,
network and rendering, and assert retained counts/bytes. Final baseline includes
fresh-vector allocation for attachment; the first exploratory run did not, so
use `/tmp/token-recency-profile-baseline-final.log`. Final indexed results including
the harder corpus: `/tmp/token-recency-profile-indexed-final.log`. The audit has
the full table, allocation caveats and environment (M2 Max / 32 GiB / rustc 1.98.0).

Full suite: **2,357 passed**, 7 skipped, plus **2 doctests**, 6 ignored
(`/tmp/token-recency-index-full.log`). Fifteen recency-filtered tests and strict
lint passed; final all-target lint is `/tmp/token-recency-profile-final-lint.log`.
One new differential test checks 1,000 generated Unicode pairs against the old
token-set oracle, plus punctuation/empty cases; existing strict-threshold and
lifecycle tests remain green. Formatting and diff whitespace checks passed.
Scoped review: Approve. Initial harness-only sort/import/dead-code lint errors
were fixed; they are not runtime failures. No native GUI/hosted service used,
no user config changed, nothing committed/published and no cache deleted.
The pre-existing release-only unused `revision` warning remains unchanged.

Next: multi-row and mid-line ghost text using the shared `TextViewportMap` in
`src/model/editor.rs`, exposed through `EditorState::viewport_map()` and consumed
by rendering, hit testing and carets. `render_ghost_text_stage` currently paints
only the first line plus a badge. No ghost-row code was changed this turn. All
remaining Phase 5+, LSP symbols/usages, settings keymap, cold Find/high-cursor/undo,
theme/config, file-boundary and native matrix scope stays active. No additional
plan is ready to archive; keep this temporary file until **all** work is verified.
About 22 GiB remained after release builds; recheck before another large build.

## Idle recency context checkpoint — 2026-09-07

Opt-in `context: { strategy: recency_ring, max_chunks: 8, chunk_lines: 64 }` is
implemented. `completion/recency.rs` owns config, bounds, provider-neutral chunks
and comment fallback; private `runtime/inline_context.rs` owns the ring and idle
scheduling. Activation, file switch, successful save and large cursor jumps queue
positions, not whole-document snapshots. After 750 ms without cursor/revision
changes, capture is bounded to 8 KiB per snippet and 1 KiB per filename, with
pending/retained counts capped at 32 (default 8). Strict >0.9 token-set similarity
evicts older duplicates. Revisiting overlapping logical-line ranges replaces old
snapshots even after substantial rewrites or deletion to empty text; actual
clamped ranges are used, not cursor-distance approximations.

The ring stays stable while typing. Provider/workspace changes clear it; closed
or path-changed sources are evicted. Named workspace files require an in-root
boundary-resolved identity; no new filesystem reads or unopened-file scan occurs.
Outside a workspace the scope is this window's open text buffers, including
untitled/unsaved text. The user guide explicitly documents transmission scope.
llama.cpp receives native `input_extra`; other transports use line comments.
Unsupported comment languages fail explicitly (including JSON/HTML/CSS), and raw
FIM control-token checks include snippets. The active prefix and local analysis
snapshot are not rewritten. Context text/order participates in exact/partial
cache equality and the existing 8 MiB payload accounting. No new worker, persistent
index, dependency or cache setting was introduced.

Final verification: **2,356 passed**, 7 skipped, plus **2 doctests**, 6 ignored,
in `/tmp/token-recency-full-ranges.log`. Strict lint passed in
`/tmp/token-recency-lint-ranges.log`. Ten new tests plus existing worker/cache
coverage were added or extended since the raw-FIM checkpoint. The runtime test
verifies idle scheduling, queued extra context, worker arrival, acceptance and
Undo; wire fixtures cover native llama.cpp, Ollama, OpenAI-compatible, Mistral,
and raw Ollama/OpenAI combinations. Earlier `targeted*`, `full*` and `reviewed`
logs predate the final overlap-range correction: use the `ranges` logs. Scoped
code review: Approve. No native GUI, live-model quality or release ring-stage
timing claim. All checks ran without changing user config or hosted services.

Changelog, user guide, active plan, sprint queue and durable audit are updated.
Next: recency-stage release profiling and multi-row/mid-line ghost text on shared
visual-row geometry, then the rest of Phase 5+ and every remaining handoff item.
LSP symbols/usages, settings keymap, cold Find/high-cursor/undo, theme/config,
file-boundary and native platform scope all remain. No additional plan is ready
to archive; keep this file until **all** remaining work is implemented and
verified. Nothing committed/published or unrelated deleted. Last free space:
about 25 GiB; recheck before a large release build.

## Raw FIM formats checkpoint — 2026-09-07

Archival follow-up (2026-09-07): the four completed/superseded plans remain in
`docs/archived/`. The older partial Line Operations archive now explicitly links
its unimplemented join-line and trimming scope from `docs/future/line-operations.md`
and the active index. Changelog updated; local links in nine relevant documents,
`just fmt-check` and `git diff --check` passed. Documentation-only self-review:
Approve; no runtime test rerun or new implementation claim. Recency-ring context
has only been inspected, not implemented. All remaining scope below stays active.

Explicit raw FIM formats are implemented on the existing Ollama/OpenAI-compatible
provider path. One template table owns Qwen, StarCoder, CodeLlama, DeepSeek,
Codestral and Mellum layouts, transport stops and response-cleanup vocabulary.
`prompt_format: native` preserves existing behavior; named formats and conservative
`infer` opt into raw prompting. Unknown/ambiguous inference and raw settings on
llama.cpp `/infill` or Mistral FIM fail explicitly. Ollama bypasses templates;
raw requests omit native suffix fields. Cache equality includes the format.

Byte-exact tests cover Unicode/CRLF/whitespace, canonical prefix/suffix order,
CodeLlama marker spaces and Mellum basename-only metadata. Raw control-token
collisions fail before HTTP; response cleanup includes DeepSeek Unicode/EOS.
Review corrected CodeLlama spacing and confirmed its 7B/13B Instruct inference.
The shared worker test covers native and raw arrival, leaked-token cleanup,
acceptance and Undo. Model weights, hosted credentials and user config were not
used or modified; actual model/server/tokenizer quality still needs validation.

Final rebuilt suite: **2,346 passed**, 7 skipped, plus **2 doctests**, 6 ignored
(`/tmp/token-raw-fim-full.log`). Strict lint passed (`/tmp/token-raw-fim-lint.log`).
Seven tests were added and worker/cache coverage extended. An earlier seven-test
pass predates the final corrections; the following build failed with `errno=28`.
Build directories and formatter caches were cleared externally, restoring space;
this agent did not delete them. The normal formatter recipe restored its tool.
Use the final rebuilt log as evidence, not the failed link or older binaries.
No new release profiling/native GUI/hosted-model claim is made.

The changelog, user guide, active plan and sprint queue are updated. Next is
idle recency-ring context and comment fallback, then Phase 5+; all LSP
symbols/usages, keymap, cold Find/high-cursor/undo, theme/config and platform
scope remains. No additional plan is complete enough to archive. Keep this
temporary file until **all** remaining work is implemented and verified.
Nothing committed/published or unrelated deleted. Free space was about 27 GiB
after the external cleanup and rebuilt checks; recheck before large builds.

## Inline post-cache filters checkpoint — 2026-09-06

Filters 5–6 are implemented in the existing worker: registry-parser bracket
sanity and conservative indentation normalization on every serve, including
cache hits. A local-only Rope snapshot (documents up to 1 MiB) supplies full
lexical context without entering HTTP requests or cache entries. Recognized
literals/comments are opaque; quote/slash-bearing recovery, unsupported/oversize
input and exhausted cooperative budgets preserve the candidate. Rust, Go,
JavaScript, C and C++ indentation follows a tab/space majority while preserving
visual columns; other languages and tied styles remain unchanged. Alternatives
share one cooperative 50 ms parsing/traversal budget, not one each.

Cache entries keep pre-filter and served text within the existing 8 MiB limit.
Exact hits use fresh local context; partial replay consumes normalized served
text. Empty/rejected/oversize replacement invalidates the older exact-context
entry. Review caught and fixed unfinished-literal recovery, indentation-sensitive
tab semantics, per-alternative budget multiplication and stale rejected refreshes.

Final full suite: **2,339 passed**, 7 skipped, plus **2 doctests**, 6 ignored
(`/tmp/token-inline-filters-full-final.log`). Strict lint passed
(`/tmp/token-inline-filters-lint-final.log`). Ten tests were added. Optimized
`just bench-completion` measured median filtering cost **367.6 µs / 3.71 ms** for
one candidate in fixtures with 100 / 1,000 generated Rust statements. A dropdown
keystroke with 1,000 carried LSP items measured **99.8 µs**. Raw output:
`/tmp/token-inline-filters-bench.log`; methodology and limitations are in the
durable audit. These are local stage timings, not end-to-end speedups. No new
native GUI or hosted-provider verification is claimed. The release-only unused
`revision` warning in `src/update/syntax.rs` remains unchanged.

The changelog, guide, active plan and sprint queue are updated. Soft Wrap,
Damage Tracking, Command Palette and Settings v1 remain archived, with deferred
work active; their index links were rechecked. No additional plan is complete.
Raw prompt formats, recency context, Phase 5+, LSP symbols/usages, keymap,
cold Find/high-cursor/undo, theme/config and platform scope remain. Keep this
handoff until **all** remaining work is verified. Nothing committed/published;
no caches or unrelated files deleted. Disk space after the optimized build is
approximately 0.9 GiB; check capacity before another substantial build.

## Inline LRU checkpoint — 2026-09-06

The worker now owns a private LRU cache, at most 256 entries and 8 MiB retained
source/result payload plus bounded metadata. It stores postprocess filters 1–4's
results, checks exact bounded generation context (not hashes alone), and can
replay after backspace/retype or return compatible typed/accepted-prefix remainders.
Unicode, CRLF and sliding prefix windows are covered. Hits receive the current
snapshot and retain existing session/revision/visibility guards. Provider config
and named credentials are validated before lookup; explicit requests bypass
reuse. Errors and empty results are not cached. Nothing persists to disk, and
no dependency, public cache API or configuration surface was added.

Eight new tests cover cache invalidation/eviction/limits and worker integration.
Final targeted tests: **60 passed**. Final full suite: **2,329 passed**, 7 skipped,
plus **2 doctests**, 6 ignored (`/tmp/token-inline-cache-full2.log`). Strict lint
passed (`/tmp/token-inline-cache-lint2.log`). A native isolated one-shot backend
closed after its first answer; Backspace/X recovered cached ghost text at a new
revision with no live backend, and Tab/Undo were verified. Evidence is under
`/tmp/token-inline-cache-native.0aMMzh/`, including inspected `replayed.png`.
The temporary editor and backend exited; all test edits were undone. The audit
records exact limitations, including that an attempted error capture did not
visibly show the transient. No release latency speedup is claimed from this check.

The changelog, user guide, sprint queue and autocomplete plan are updated.
**Post-cache filters 5–6 remain undone**: syntax-aware bracket sanity and
indentation normalization need reliable literal/comment and style handling.
Do not mark their combined old checkbox complete just because the LRU is done.
Raw prompt formats, recency context, Phase 5+, LSP symbols/usages, keymap,
cold Find/high-cursor/undo, theme/config and platform scope remain active.
No additional plan is ready to archive. Keep this handoff until **all** remaining
work is complete; nothing committed/published or unrelated deleted.

## Native rust-analyzer dropdown checkpoint — 2026-09-06

The reported `cc::Build` member-chain case is now natively verified on macOS,
using an isolated build-script fixture and the repository's exact `cc` 1.2.67
dependency. rust-analyzer 1.98.0 returned 77 members after a native `.` key event;
typing `comp` filtered to seven compiler-related methods with `compile` first.
Tab inserted `compile`, one Undo restored `.comp`, and selected signatures/docs
were visually inspected. The screenshot's `ar_flag`, `archiver`, `asm_flag` and
other first rows really are builder methods, not local-word leakage. Fixture
locals were absent. The original project file was not edited; broader language,
pointer, IME and Windows/Linux validation remains unverified.

Methods now retain a distinct `M` badge (functions `f`, modules `m`). Sources and
rows share `MenuItemKind`; the duplicate view enum and conversion table are gone.
Private glyph/color logic remains in view. Ranking, insertion, requests, geometry
and configuration are unchanged. Updated conversion/row tests and one new badge
test cover the refactor. **142 completion tests passed**; full suite **2,321 passed**,
7 skipped, plus **2 doctests**, 6 ignored. Strict lint passed. Logs:
`/tmp/token-ra-dropdown-{targeted,full,lint}.log`. Native artifacts and exact
limitations are in the durable audit; final images/state are under
`/tmp/token-ra-dropdown.wazUue/final-*`. The temporary editor closed cleanly after
undoing test edits. No user config/source changed, cache deleted or work committed.

The changelog, completion user guide and active plan are updated. This supersedes
older statements that the specific builder-chain real-server check remains open,
but not the full platform matrix. All remaining inline formats/context/cache,
Phase 5+, LSP symbols/usages, keymap, cold Find/high-cursor/undo, theme/config and
other handoff scope stays active. Keep this temporary file until **all** remaining
work is implemented and verified; no new plan is ready for archival this turn.

## Inline alternatives and layout-aware shortcuts checkpoint — 2026-09-06

Multiple-result inline suggestions are implemented. The provider contract returns
bounded candidates; the existing worker postprocesses them and the model removes
empty/duplicate results while preserving order. OpenAI-compatible `n` is 1–8,
default 1; other transports reject `n > 1` before credential lookup/network I/O.
Alt+]/Alt+[ and named actions cycle compatible choices without edits or requests.
Typed/partially accepted prefixes must match exactly, including Unicode and CRLF;
backspace can restore choice eligibility. Ghost painting and automation share the
compatible position/count. The screenshot fixture accepts strings or alternatives.

Native verification exposed a macOS Option shortcut issue. Keymap dispatch now
tries the logical character first, then the unmodified layout key, against the
same pending chord, advancing state once. This supports layouts needing Option
to type brackets without globally disabling composed text. The existing context
API accepts ordered interpretations; no parallel resolver or feature-local
physical-key mapping was added. Regression tests cover precedence, conditional
fallback, unbound input and multi-step chord completion/reset.

Final verification: **2,320 tests passed**, 7 skipped, plus **2 doctests**, 6 ignored
(`/tmp/token-inline-alternatives-fallback-tests.log`). Strict lint passed
(`/tmp/token-inline-alternatives-fallback-lint.log`). Eleven new tests since the
hosted checkpoint cover alternatives and keyboard resolution. Earlier failed
test authoring attempts and incorrect physical-key native probes are superseded
by these results, not counted as verification.

An isolated native editor and loopback provider verified Norwegian-layout Option+9
(`]`) / Option+8 (`[`) cycling, unchanged document revision/dirty state during
cycling, Tab accepting the selected second result, undo, and unbound composed
`˙` input. The final native capture was visually inspected and shows `[2/3]`.
Artifacts: `/tmp/token-inline-alternatives-native.s1CyGl/`, specifically
`native-fallback-next.png` and `fallback-{prev,next,accepted,composed}.json`.
The earlier `native-next.png` does not show a successful next-choice action.
The temporary editor closed cleanly after undoing test edits; its fixture server
was stopped. No user config/source file, keyboard layout or hosted service was
changed. US-layout fallback is regression-tested but not natively exercised;
IME/dead-key combinations and Windows/Linux native behavior remain unverified.
This is not the real rust-analyzer dropdown repro or release performance profiling.

The changelog, user guides, active autocomplete plan and durable audit are updated.
Soft Wrap, Damage Tracking, Command Palette and Settings v1 plans are archived;
their deferred work stays active. Autocomplete is not ready for archival. Older
checkpoints below are historical; references to a single-result-only pipeline or
unimplemented alternative cycling are superseded here. Next inline work is raw
`PromptFormat` rendering/inference, idle recency context, LRU/filters, then Phase 5+.
All other remaining handoff scope is unchanged, including LSP symbols/usages,
keymap follow-ups, cold explicit Find/high-cursor/undo debt, theme/config debt and
native completion/platform verification. Keep this file until **all** scope is
implemented and verified. Nothing committed or published; no cache was deleted.

## Hosted native-suffix inline checkpoint — 2026-09-06

OpenAI-compatible (`open_ai_compat`) and Mistral FIM (`mistral_fim`) now share the
existing `FimProvider`, HTTP/TLS client, cancellation, deadlines and bounded body
reads. Both send native prefix/suffix fields, with their own response decoders.
OpenAI-compatible requests one choice; Mistral has no `n` field in the checked
API schema. The existing single-string postprocess/ghost/accept pipeline remains.
Raw sentinel prompt formats/inference and alternative cycling are still unfinished;
do not claim arbitrary chat models support this native-suffix transport.

`ProviderConfig.api_key_env` is an explicit environment-variable name, resolved
only in the worker. It is required for Mistral and optional for other transports;
no ambient credential discovery occurs. Names and token values are validated,
authenticated remote endpoints require HTTPS, and redirects remain disabled.
Resolved keys stay out of model/config serialization and error messages. Base URLs
accept a trailing `/v1` and retain gateway prefixes. No new dependency, worker,
public control API or user configuration mutation was introduced.

Ten new tests cover request/response shapes, authentication references and safe
errors, native suffixes, root/versioned/gateway routes, redirects, text chunks,
partial/full acceptance and undo. An isolated child test sets a synthetic key and
exercises the real Mistral environment lookup and runtime worker against loopback;
the parent never mutates its global environment or reads user credentials. Its
initial wrong module path was caught by the guard against running zero tests,
then fixed. Final verification includes that correction: **2,309 tests passed**,
7 skipped, plus **2 doctests**, 6 ignored (`/tmp/token-hosted-inline-full3.log`).
The isolated test also passed separately (`/tmp/token-hosted-inline-targeted.log`).
Strict lint passed (`/tmp/token-hosted-inline-final-lint3.log`). Formatting and
diff checks passed. Scoped review and source references are in the durable audit.

No hosted service was called, no billing or real credentials were used, and no
new native GUI or performance validation is claimed. The changelog, user guide,
active autocomplete plan and sprint queue now distinguish implemented native-suffix
transports from unimplemented raw prompt formats. Next inline work: `PromptFormat`
rendering/inference, multiple-result provider/state/cycling, idle recency context,
LRU and filters 5–6, then Phase 5+. The remaining LSP symbols/usages, keymap,
cold Find/high-cursor/undo, theme/config and native completion/platform requirements
remain in scope. Keep this file until **all** work is implemented and verified.
Nothing committed or published; no cache or unrelated files were deleted.

## Settings LSP and v1 archival checkpoint — 2026-09-06

Settings Phase 3 is implemented: registry-derived master/per-server switches,
read-only command values with YAML keys, and live process states. General presets
and LSP rows share cached metadata/filter order; values are read from the current
model. Switches reuse the existing save/lifecycle handlers. No-op choices and
read-only keyboard actions cannot save or manage servers. Status changes repaint
an open Settings or Language Servers modal without resetting query/selection.
Long commands use clipped detail text; the shared renderer leaves a gap before
right-hand accessories. The model's existing per-server-ID status mirror is used,
not a new per-root aggregate.

Final source verification: **2,299 tests passed**, 7 skipped, plus **2 doctests**,
6 ignored (`/tmp/token-lsp-settings-full3.log`); strict lint passed
(`/tmp/token-lsp-settings-final-lint3.log`). Six new tests cover registry metadata,
current command values, switch effects/no-ops, override preservation, read-only
actions, long paths and all seven process-state labels/redraw behavior.

An isolated native macOS editor used only temporary config/source files and a
fake LSP server. `Cmd+,`, Left/Right switch changes, persistence, read-only command
row actions and Indexing → Ready with Settings open were verified. Read-only
actions left the config byte-for-byte identical; saves preserved an off-preset
blink value and an unknown YAML key. The window was captured and closed cleanly.
Artifacts: `/tmp/token-lsp-settings-native.14FPqZ/` (native state snapshots,
`native-settings.png`, config hashes, server transcript). The final shared-spacing
fix was then rendered and visually checked in `headless/screenshot-settings-lsp.png`;
the earlier native capture predates that spacing fix. The debug 20-frame native
presentation check is not release performance profiling.

Settings v1 is now archived at `docs/archived/settings-page.md`. Unfinished Phase 4
is preserved in `docs/future/settings-keymap.md`: merged bindings, conflicts,
chord capture/rebinding, override persistence and base-keymap choice. The changelog,
documentation index, sprint queue, user guide and durable audit are updated.
The older checkpoints below are historical; their statements that the LSP section
or Settings native keyboard validation remains are superseded by this checkpoint.

The full remaining scope is unchanged: inline/provider maturity, LSP workspace
symbols/usages, keymap follow-ups, cold explicit Find, high-cursor editing/undo,
theme/config debt and native completion/platform validation. Physical-pointer and
Windows/Linux GUI checks were not performed; this fake-server Settings check is
not the real rust-analyzer completion dropdown repro. Keep this temporary handoff
until **all** remaining work is implemented and verified. Nothing committed or
published, and no cache or unrelated files were deleted.

## Settings preset UI checkpoint — 2026-09-06

Settings Phase 2 is implemented on the shared overlay: `Cmd+,` and “Open Settings”
open a searchable preset list. `src/settings.rs` owns descriptors and filtered
order; view, actions and automation consume it. Appearance/Editor/Status Bar
cover the original six fields plus mouse hover/delay and format-on-save. Theme
opens the existing picker. Off-preset values show no active chip and are not
normalized by opening/closing or clicking a row label. Left/Right, Enter or an
explicit chip click commits; unchanged choices do not save. Persistence uses
the existing ordered `SaveConfiguration` runtime command; font changes refresh
status metrics. LSP settings rows and the future keymap tab remain unfinished.

Shared `Accessory::Choices` rectangles drive painting and hit testing; the
already-flattened row list is reused. Top-of-list headings now participate in
the common visible-window/selection-reveal policy. Narrow footers fit navigation
hints and hide secondary text rather than overlap. Blink Off restores a steady
caret and uses a positive 250 ms runtime maintenance interval instead of a
zero-delay wake loop. The screenshot generator accepts a settings modal; the
reusable scenario is `screenshots/scenarios/settings.yaml`.

Scoped review fixed accidental config changes on label clicks, zero-interval
wakeups, hidden first headings and narrow footer overlap. Twelve new tests cover
presets, off-preset/no-op behavior, filtered order, effects, automation, geometry
and runtime scheduling. Final verification, including the last footer guard:
**2,293 tests passed**, 7 skipped, plus **2 doctests**, 6 ignored
(`/tmp/token-settings-full6.log`). Strict lint passed
(`/tmp/token-settings-final-lint4.log`). Formatting and diff checks pass.
The newly built debug screenshot executable rendered and verified the scenario
at 1100 px and 360 px widths: `/tmp/token-settings-visual.wXdhrR/final/screenshot-settings.png`
and `/tmp/token-settings-visual.wXdhrR/narrow/screenshot-settings.png`.
Headless inspection confirmed headings, distinct chips and non-overlapping
footer hints; it does not validate native interactive behavior. Scoped review
and exact limitations are in the durable audit. All build/test/render processes
completed. About 5.9 GiB free space remained; no cache was deleted by this agent.

The changelog, user settings reference, active feature plan and durable audit
are updated. Do not archive Settings yet: Phase 3 LSP rows and native/manual
validation remain. The full inline/provider, LSP workspace-symbol/usages,
high-cursor edit/undo, cold explicit Find and native completion/GUI scope below
is unchanged. Keep this temporary handoff until all remaining work is complete.
Nothing committed or published; no cache was deleted by this agent.

## Background Find display checkpoint — 2026-09-06

Large cold Find display scans (at least 256 KiB) now run on one coalescing worker
with one replaceable pending snapshot. Update owns scheduling; rendering shows
“Searching…” and hides stale marks instead of scanning. Shared immutable search
inputs/results validate document identity, revision, rope identity, query, flags,
effective scope and request/result ownership. Close/reopen resets pending identity.
Background computation prepares logical overview lines; pixel-row projection
remains on the UI thread. Small searches stay synchronous.

Explicit Find navigation/replacement still uses a fresh synchronous fallback
when results are cold. It cannot apply stale offsets, but it can block; state
cloning can also leave the active display cache cold after the action. Running
regex scans are not interrupted mid-computation. Worker panics were not supervised
at this checkpoint; the later path-source checkpoint above adds shared
panic-to-failure delivery. Worker shutdown is nonjoining; model guards remain
authoritative if a reply races submission/shutdown. Mid-scan interruption remains
a follow-up, not completed work.

Twelve new tests cover stale replies, ownership, flags/scope, close/reopen,
failure/regex status, small-file fallback, fresh replacement, overview preparation,
bounded debug output and deterministic worker scheduling/shutdown. Scoped review
fixed a Medium issue: derived request/result debug output would format entire
snapshots/results in debug update tracing. Custom formatting emits only bounded
metadata. Final full suite: **2,281 tests passed**, 7 skipped, plus **2 doctests**,
6 ignored (`/tmp/token-find-async-full3.log`). Strict lint passed
(`/tmp/token-find-async-final-lint.log`). Earlier disk-failed attempts are not the
final verification result. No cache was deleted; tests share the existing modal
integration executable. The changelog and durable audit are updated.

The fresh optimized Find build completed in 5m 47s; both probe runs passed.
Its warm-render fixture explicitly
primes results so it cannot accidentally time an empty pending display. Separate
`typing_and_render_find_pending` and `typing_find_synthetic_roundtrip_render`
cases distinguish the immediate pending frame from total CPU work, including
computation and reply application. The latter runs computation inline for
deterministic accounting; neither measures native worker scheduling/UI latency.
At 100,000 lines, repeat-run median/p95 was **0.360/0.375 ms** for the pending
frame, **11.672/12.142 ms** for the synthetic completed roundtrip and
**0.406/0.425 ms** for warm rendering. Cold matching remains about 7 ms. The
previous synchronous edit/render median was 11.745 ms: total CPU work is similar,
but it no longer all sits before the pending display frame. The pending frame
has no search marks yet, so this is not a completed-search speedup claim.
Full tables/caveats are in the audit. Logs: `/tmp/token-find-async-profile.log`,
`/tmp/token-find-async-profile-repeat.log`.

Formatting and diff checks passed. All build/profile processes completed; about
901 MiB disk space remained and no cache was deleted. The earlier cache-cleanup
question is no longer blocking this checkpoint; no approval was received or
assumed. The existing release-only unused `revision` warning remains in
`src/update/syntax.rs:196`; strict all-feature lint is clean.

The remaining scope below is unchanged: explicit cold Find actions, high-cursor
mapping/deletion/duplication profiling, full per-pane undo selection snapshots,
inline/settings/LSP work and native/manual validation. Implemented plans remain
archived; unfinished plans remain active. Keep this temporary file until all
remaining work is implemented and verified. Nothing committed or published.

## Decoration traversal checkpoint — 2026-09-06

The measured range-decoration hotspot now prepares visible-row geometry once per
pass, uses logical-line bounds to visit only intersecting rows (including repeated
wrapped rows), and lazily materializes text once per touched row. The separate
`render_one_decoration` traversal is removed. There is no cross-frame cache or
invalidation policy: scratch state belongs to this render pass. Tint-first and
stroke-second ordering, within-category source order and span/paint primitives
are unchanged. Full and cursor-line redraws use the same stage. Range overdraw
is now included in the existing debug `TextDecorations` timing stage.

A frozen test-only copy of the prior traversal verifies exact pixel equality
across 48 text/wrap/scroll configurations with 64 mixed-order ranges each. It
covers every decoration kind, alpha overlap, tabs, Unicode, CRLF, long wrapped
lines, empty documents and stale/reversed ranges. The reference matrix passed
before optimization (`/tmp/token-decoration-baseline.log`) and in the final full
suite: **2,269 tests passed**, 7 skipped, plus **2 doctests**, 6 ignored
(`/tmp/token-decoration-full.log`). Strict lint passed
(`/tmp/token-decoration-lint.log`). Scoped review and limits are in the audit;
the changelog is updated. Formatting and diff checks passed.

The optimized build completed in 5m 51s. Two fresh Find runs confirm the reduction:
at 100,000 lines, repeat-run warm render median/p95 is **0.399/0.416 ms**, versus
**1.172/1.504 ms** before (about 66% lower median). Typing plus rendering is
**11.745/12.318 ms**, versus **13.064/14.444 ms** before (about 10% lower median).
Cold matching still takes about 7 ms and is the next substantial typing-path cost.
These are CPU fixture measurements, not native GUI latency. Full tables and
caveats are in the audit. Logs: `/tmp/token-decoration-profile.log`,
`/tmp/token-decoration-profile-repeat.log`; baseline:
`/tmp/token-find-refresh-repeat.log`.

The native sample now attributes 584 of 3,833 main-thread samples (15.2%) to
`prepare_visible_line` across all text/gutter passes, down from 2,721 of 3,854
(70.6%) before. The per-decoration row preparation is no longer dominant.
Raw sample: `/tmp/token-decoration.sample.txt`; workload:
`/tmp/token-decoration-sample-run.log`. Other passes still prepare their own rows.
The general workload passed as a sanity check
(`/tmp/token-decoration-general-profile.log`), including long-line and history
fixtures; no additional speedup claim is made from that run.

All build/profile processes completed. About 776 MiB free space remained; no
caches or unrelated files were deleted. The existing release-only unused
`revision` warning remains in `src/update/syntax.rs:196`; strict lint is clean.

Remaining: synchronous cold Find matching; high-cursor edit mapping and dedicated
deletion/duplication probes; full per-pane selection/active-index undo snapshots;
the inline/settings/LSP scope and native/manual completion/GUI checks below.
Implemented plans stay archived. Keep this temporary handoff until the full scope
is implemented and verified. Nothing committed or published; no caches deleted.

## Find replacement checkpoint — 2026-09-06

Find's single/all replacement paths now use the shared planned-edit transaction.
They no longer bypass undo history or peer position mapping. Replace All is one
undo step and places the primary caret after the actual first replacement, not
on line zero. Secondary selections remain mapped; unchanged replacements leave
revision, dirty state and redo history untouched. Non-text tabs cannot enter
these mutation paths. Replace-and-Find includes an adjacent next match.

`EditPositions` maps an active selection-only Find scope for the edited focused
document, including ordinary edits and history traversal. Boundary insertions
stay in scope; an entirely deleted scope remains an empty range instead of
becoming document-wide. Repeated length-changing replacements read the mapped
scope before finding the next match. Inactive Find state still recaptures its
scope from the current selection on reopening; persistent per-document search
sessions and full per-pane selection/active-index undo snapshots are not added.

Eight new regressions cover these cases; the initial five failed before the fix.
Full `just test '--no-fail-fast'`: **2,268 passed**, 7 skipped, plus **2 doctests**,
6 ignored (`/tmp/token-find-replace-full2.log`). Strict `just lint` passed
(`/tmp/token-find-replace-lint.log`). The scoped review is in the durable audit;
the changelog is updated. No new dependencies or public mutation API were added.

The new `just profile-workloads replacements` probe measures cold search,
planning, mutation and undo capture at 1/100/10,000 matches in one/two panes,
excluding fixture reset, rendering and command execution. The release build
completed in six minutes with no cache deletion. One-pane median/p95 was
**4.208/4.333 us** for one match and **5.436/5.750 ms** for 10,000. Two-pane
10,000-match median/p95 was **5.333/5.761 ms**. These are current-state timings,
not a speedup comparison to the old non-undoable path. Full tables and caveats
are in the audit; log: `/tmp/token-find-replace-profile.log`.

Fresh Find profiling on the same build confirms the remaining cold-scan cost.
At 100,000 lines, the repeat-run median/p95 was **7.058/7.541 ms** after an edit,
**6.771/7.494 ms** after a query change, **1.172/1.504 ms** for warm Find rendering,
and **13.064/14.444 ms** for typing plus rendering. The first run had much larger
query/typing tails, which did not recur; do not infer a regression or speedup.
Logs: `/tmp/token-find-refresh-profile.log`, `/tmp/token-find-refresh-repeat.log`.
Insertion probe medians remain broadly consistent with the earlier checkpoint
(`/tmp/token-insert-refresh-profile.log`); full tables and caveats are in the audit.

A five-second native CPU sample collected 3,854 main-thread samples; 2,744
(about 71%) were inclusively inside `render_one_decoration`, with repeated
visible-line preparation/rope line lookup prominent. The current source still
prepares all visible rows for every decoration. Raw sample:
`/tmp/token-find-refresh.sample.txt`; workload log:
`/tmp/token-find-refresh-sample-run.log`. Next concrete change: reuse prepared
rows and narrow decoration traversal in `src/view/editor_text.rs`, retaining
tint/stroke pass order and shared soft-wrap geometry; test pixel equivalence.

All build/profile processes finished normally. Formatting and diff checks pass.
About 941 MiB free space remained; no caches or unrelated files were deleted.
Dedicated deletion/duplication profiling is still absent. Native CPU sampling is
not validation of native completion-menu behavior or interactive GUI correctness.

Next: cold Find/decoration optimization, then the full remaining
inline/settings/LSP scope and native/manual validation below. Regex replacement
text remains literal; capture-group expansion and single zero-width-match UI
selection were not implemented by this consolidation. Implemented plans remain
archived. Keep this temporary file until all remaining scope is implemented and
verified. Nothing committed or published.

## Ordinary forward-edit checkpoint — 2026-09-06

All ordinary forward `DocumentMsg` mutations now use `apply_planned_edits`.
Character/word deletion, cut, whole-line deletion, duplication and indentation
join the previously migrated insertion paths. Physical deletion ranges are
merged before mutation; clipboard selections keep their original order/payload.
Duplicate sources are captured from the pristine buffer, including multiple
copies inserted at the same point. CRLF joins remove both characters, trailing
line deletion leaves no extra newline, and noncontiguous deletion retains the
active caret. Unindent maps reversed selections through the shared policy.
No-op deletion/indentation does not create history or dirty the document.

Removed `shift_sibling_cursors`, `delete_selection`, the private `record_edit`
bridge and its `EditPositions::transform_operation` adapter. The document handler
is now about 695 lines, down from about 1,790 before this increment. No new public
API or caret-placement variant was needed. Undo/redo retain their shared history
traversal rather than pretending recorded intermediate offsets are pristine.

Thirteen new regressions were added; the initial six failed before the fix.
`just test '--no-fail-fast'`: **2,260 passed**, 7 skipped; **2 doctests passed**,
6 ignored in both the initial and final rerun (`/tmp/token-delete-final-full.log`).
Strict lint passed after replacing
the equivalent stable descending comparator with `sort_by_key(Reverse(...))`
(`/tmp/token-delete-lint2.log`). Formatting and diff checks passed. Scoped review and details are in the durable
audit. The changelog is updated. Earlier profiling results below predate this
increment: no new deletion/duplication timing or speedup is claimed. Only about
1.6 GiB remained after verification; no release rebuild or cache deletion was
attempted in this increment.

Next: audit/consolidate Find replacement's direct buffer mutation and position
mapping, then cold Find/decoration work and fresh optimized profiling. Inspection
of `src/update/ui.rs::replace_and_find_next` / `replace_all` shows both mutate
without pushing undo history or mapping peer positions; Replace All also assigns
the primary caret to line zero regardless of the first match's location. Start
with regression tests for those cases, preserving selection-only search scope.
Complete
per-pane selection/active-index undo snapshots, inline/settings/LSP features and
native/manual validation remain in scope. This migration covers ordinary forward
document commands, not every buffer mutation source. Implemented plans remain
archived; unfinished plans remain active. Keep this temporary handoff until the
entire remaining scope is implemented and verified. Nothing committed/published.

## Ordinary insertion / profiling checkpoint — 2026-09-06

Disk space recovered without this agent deleting caches. The previous peer-map
checkpoint passed the full suite (2,240 tests plus two doctests) and strict lint.
The verification gap recorded below is historical, not a current disk blocker.

Typing, newline insertion and paste now plan against the pristine buffer and
share `apply_planned_edits`. Their three independent mutation/sibling loops are
removed. Selections are replaced, disjoint carets retain order/active index,
overlapping/touching ranges merge before mutation, and all final carets follow
the shared mapper. Surround uses two boundary insertions, preserving peer
positions inside surviving Unicode text and undoing only the delimiters.
Paste distribution is checked with bounded scratch space; single-cursor paste
does not allocate a line vector. Shared transactions avoid mapping positions
that will immediately be overwritten by complete explicit caret placement.

Seven new regressions cover ignored selections, same-line siblings, newline
replacement, Unicode surround, multiline paste/reversed selections, overlapping
ranges and mixed surround/plain carets. Final `just test '--no-fail-fast'`:
**2,247 passed**, 7 skipped; **2 doctests passed**, 6 ignored. Strict `just lint`
passed. Logs: `/tmp/token-insert-final-full.log` and
`/tmp/token-insert-final-lint2.log`. Scoped diff-based review is recorded in the
durable audit, including the scratch-allocation issue fixed during review.

Optimized profiling now works. The new `just profile-workloads insertions` probe
measured a one-cursor median of **2.500 us**. At 1,000 cursors, removing redundant
accepting-pane mapping reduced the initial refactor's median from **6.522 ms to
3.666 ms** in one pane, and **9.333 ms to 6.506 ms** in two panes. This is warm
synchronous update work, not rendering or end-to-end latency. High cursor counts
still have nontrivial cost; the mapper still does per-edit/per-position work.

The previously blocked `file-identity`, `file-open` and `file-io` probes also ran.
At 1,000 open documents, canonical identity lookup was **81.458 us** median and
exact-path tab reuse **371.916 us**. Save snapshot plus synthetic acknowledgement
at 100,000 lines was **1.791 us**; an undo+redo pair with saved-content comparison
was **209.917 us**. These exclude worker/disk effects. Full tables, p95 values,
fixture caveats and log paths are in `docs/dev/refactoring-audit-2026-09-06.md`.
The optimized build emits an existing unused debug-overlay `revision` warning in
`src/update/syntax.rs:196`; the debug/all-feature lint is clean.

Next: migrate deletion, cut, duplication and indentation's focused-caret
algorithms; `shift_sibling_cursors` still serves backspace and duplication.
Complete per-pane selection/active-index undo snapshots are still absent.
Then continue cold Find/decoration work and the inline/settings/LSP features and
native/manual verification below. No goal scope was removed. Implemented plans
remain archived, but unfinished plans remain active. Nothing committed or
published; no live build/profile sessions remain from this pass. Keep this
temporary handoff until all remaining work is implemented and verified.

## Ordinary peer-position checkpoint — 2026-09-06

Ordinary forward edits now capture peer positions before mutation and map them
from newly recorded insert/delete/replace operations, including batches in actual
application order. This shares `EditPositions` with completion/inline/history;
it does not scan or clone undo history. The invoking pane keeps its feature-owned
caret placement. Five peer-sync helpers, the public editor-area adjustment helper
and its divergent line/column arithmetic are removed. Empty operations preserve
peer navigation state, and single-pane edits allocate no peer offset vectors.

Five new regressions pass when running the freshly built integration executable
directly. They cover ordinary insertion/deletion, Unicode multiline paste,
selection replacement/cut, newline joins, duplication, batched newlines,
indent/unindent, no-op navigation state and undo/redo without double mapping.
The initial tests reproduced the paste-column and replacement-line bugs. A later
fixture violated cursor/selection invariants; it was corrected before the green
five-test run (`/tmp/token-ordinary-direct-final.log`).

Verification is incomplete: normal nextest builds and serialized retries failed
linking application executables with ENOSPC. The regression executable did build
and run successfully. Strict `just lint` passed for the production changes
(`/tmp/token-ordinary-final-lint.log`); after test-fixture corrections, its rerun
also hit ENOSPC. Formatting and diff checks passed. The audit records a scoped
review with verification pending; do not reuse the earlier 2,235-test result as
evidence for this checkpoint. No new performance timings are claimed.

Free sufficient disk space before full-suite/lint/release profiling retries;
roughly 500 MiB remained after failed build temporaries were released. No caches
or unrelated files were deleted. There are no live build sessions from this pass.
Next: verify this checkpoint, then migrate same-pane sibling/multi-cursor edit
placement (the `shift_sibling_cursors` helper remains). Generic replacement
mapping clips relative offsets; it does not track surviving substrings through
surround operations. Cold Find/decoration work, inline/settings/LSP features and
all carried native/manual verification remain in scope. Implemented feature
plans stay archived; this temporary handoff is not ready to delete.

## Shared file-identity checkpoint — 2026-09-06

Finding 7's shared identity/update-I/O implementation is in place. Documents
carry an immutable, cheaply shared `FileIdentity` snapshot resolved by opening,
successful writes/reloads or the startup/runtime compatibility boundary. Tab
reuse, location display, LSP URI lookup and Problems scope now use the same pure
document lookup. The per-open identity map and Problems' divergent basename gate
are removed. Differently named symlinks retain current-file diagnostics even when
their disk target becomes unavailable.

Successful Save As/reload replaces identity; source-path guards reject obsolete
aliases, and a pending open retries if a known document's URI changed without a
display-path change. LSP opens and clears reuse the document snapshot. Known
original/canonical spellings reuse directly; unknown aliases must pass through
the worker, not `find_open_file` as an implicit filesystem resolver. Opening an
unknown alias still preserves the live buffer. Missing-path URI encoding now
preserves `..` rather than identifying a different child.

Eight new regressions and strengthened actual worker tests cover these cases.
`just test '--no-fail-fast'`: **2,235 passed**, 7 skipped; **2 doctests passed**,
6 ignored. Strict `just lint` passed. The audit describes the scoped review,
fixture migration, remaining limits and the new `file-identity` profiling mode.
No new timing numbers are claimed: release profiling remains blocked by disk
space (about 740 MiB available after verification). No caches were deleted.

Next: ordinary forward-edit position mapping, then the outstanding cold Find /
decoration work and optimized profiling, plus the inline/settings/LSP scope and
manual verification debt below. Startup loading and some other runtime I/O remain
synchronous; this is not a claim that all filesystem work is asynchronous.
There is no live filesystem alias watcher or hard-link identity policy; snapshots
refresh at explicit boundaries. Native GUI/Windows validation remains outstanding.
Nothing committed/published. Keep this temporary handoff until the full scope is
implemented and verified.

## Asynchronous configuration-opening checkpoint — 2026-09-06

Config directory/theme-directory preparation, no-clobber default-keymap creation
and log selection now run on the ordered file worker, followed there by normal
file preparation. `FileOpenSource` distinguishes a known path from a resource
that needs discovery. The separate `Cmd::OpenConfigResource` and unkeyed
`AppMsg::ConfigResourcePrepared` reply are removed; keyboard/palette actions
still share the same user request.

The original group/tab/cursor/revision is captured before any preparation, not
after the async gap. Config opens respect newer tab choices and closed groups.
Directory replies are consumed once, do not create tabs, and do not supersede a
pending normal file open. Existing keymaps, logs and unsaved buffers retain their
previous preservation behavior.

Six new regressions plus the seven existing configuration tests passed.
`just test '--no-fail-fast'`: **2,227 passed**, 7 skipped; **2 doctests passed**,
6 ignored. Strict `just lint` passed. Worker tests use isolated explicit config
roots, check off-thread discovery and ordered preparation/read/write behavior,
and deliver errors through the original request tokens. The audit records the
scoped review; the changelog records the user-visible changes.

Finding 7's next work is the shared resolved path/URI identity across tab reuse,
navigation, LSP lookup and Problems scope, with open/Save As/rename invalidation.
Startup loading and other runtime filesystem work remain; configuration YAML
save/reload and theme loading were not made asynchronous in this increment.
The file worker still has the queue/teardown limits noted below. Optimized
profiling remains unverified after the previous disk-space failure (about
1.4 GiB available at this checkpoint); no release rebuild was retried and no
caches were deleted. All remaining scope below is intact. Keep this temporary
handoff; nothing was committed or published.

## Asynchronous file-open checkpoint — 2026-09-06

Ordinary new-tab validation, alias resolution, text reads and image decoding now
run on the ordered file worker. Request tokens retain the originating group,
tab/cursor/revision and typed navigation position. Replies cannot switch to a
different split, overwrite a reused live buffer, or resurrect a closed group.
Native dialogs retain their original group; CLI/automation acknowledgements wait
for installation, and `--wait` tracks the resulting documents through closure.

Tab installation is shared by normal opens, reuse and splits, preserving image
and binary modes. Image views share immutable decoded pixels while keeping local
pan/zoom. Workspace edits prepare unopened text targets before mutation and
acknowledgement, reject failed/stale preparation, and defer code-action commands.
This is not general atomic support for resource operations or arbitrary edits.

Nineteen new regressions cover update continuations and actual worker behavior;
existing navigation, native-dialog, fake-LSP and CLI wait fixtures were migrated.
`just test '--no-fail-fast'`: **2,221 passed**, 7 skipped; **2 doctests passed**,
6 ignored. Strict `just lint` passed. Scoped self-review and limits are in the
asynchronous new-tab open follow-up in `docs/dev/refactoring-audit-2026-09-06.md`.

The new optimized `just profile-workloads file-open` probe could not run: its
release build exhausted disk space (`os error 28`). No timing result or latency
improvement is claimed. No caches or unrelated files were deleted. Free adequate
disk space before retrying; do not treat an older executable as this checkpoint.

Finding 7 remains open. Next: async config directory/keymap/log preparation using
the target-aware open boundary, then one resolved identity shared by tab reuse,
navigation, LSP lookup and Problems scope. Their remaining canonicalization and
startup loading still need work. The single worker preserves ordering but slow
opens can delay queued saves; teardown drains the queue and has no bounded wait.
All other remaining scope below is intact. Nothing committed or published; keep
this temporary handoff until the full scope is implemented and verified.

## Plan archival checkpoint — 2026-09-06

Implemented damage tracking and the palette proposal superseded by OverlaySurface
Phase 4 are now in `docs/archived/`, with implementation evidence and explicit
verification limits. Deferred history migration/pruning/polish ideas remain in
`docs/future/command-history-followups.md`. Soft wrap was already archived; it,
column selection and select-next-occurrence now appear under completed features
in the docs index rather than planned work. Soft wrap is no longer an active
sprint queue item. Autocomplete, settings and other unfinished plans remain active.
Archival does not complete the remaining scope below or authorize deleting this
temporary handoff yet.

Archival verification: 71 local links resolved; targeted damage (15), history
(8) and palette (25) tests passed, as did formatting, strict lint and
`git diff --check`. No application behavior changed in this cleanup.

## File-reply checkpoint — 2026-09-06

The unsafe unkeyed save/load replies identified in the previous checkpoint are
replaced. Document-owned tokens bind Save, Save As dialogs/writes and explicit
reloads to their initiating document. Save/Save As share one ordered background
writer, and explicit reads use that same queue. Saved rope snapshots preserve
edits made during a write and distinguish undo branches at equal history depth.
Stale reads, replaced/closed documents, duplicate replies, and save-after-read
ordering are guarded. LSP didSave includes the actual saved text; tracing omits
file contents. Image/binary placeholder saves are rejected; CSV remains savable.

Seventeen new regressions plus strengthened existing tests cover update, actual
runtime dispatch, ordered worker I/O/teardown and fake-server save notifications.
`just test`: **2,202 passed**, 7 skipped; **2 doctests passed**, 6 ignored.
Strict `just lint` passed. Optimized CPU profiling and scoped self-review are in
the file-reply follow-up in `docs/dev/refactoring-audit-2026-09-06.md`.

Finding 7 remains incomplete: config preparation and ordinary new-tab opening
are still synchronous. Next, move normal open validation/loading/image decoding
behind a target-group-aware effect with post-open navigation continuations, then
share runtime-resolved identity across tab reuse, navigation, LSP and Problems.
Reuse the file worker rather than reintroducing detached per-write threads.
Do not substitute its document-targeted reload request for a group-targeted new
open. All other remaining scope below is intact. Nothing committed/published.

## Configuration-effect checkpoint — 2026-09-06

Finding 7 has a first implementation step, not full closure. Configuration
directory/keymap/log actions now emit one `OpenConfigResource` effect; runtime
owns environment lookup, directory preparation, exclusive default-keymap creation
and log selection. Keyboard and palette log actions share this path. The public
update-layer keymap writer, `config_paths::log_file`, duplicate
`OpenFileInEditor` loader and keymap-specific effect/reply are removed.

Keybindings/logs open through ordinary tab opening, preserving the prior buffer
and unsaved edits in an already-open resource. Existing keymaps, including empty
files and symlinks, are never replaced. Preparation errors are visible; empty
log directories do not create blank logs, and backup names/directories are not
selected as logs. `docs/CHANGELOG.md` records these user-visible changes.

Seven regressions replace two environment-dependent log tests. `just test`:
**2,185 passed**, 7 skipped; **2 doctests passed**, 6 ignored. Strict `just lint`
passed. Scoped diff-based self-review checked error handling, no-clobber creation,
buffer preservation, shared command routing and redraw accounting.

Important remaining work: preparation currently executes synchronously in runtime
command order to avoid a delayed reply opening in a different focused group.
The normal tab loader still performs synchronous I/O/canonicalization in updates;
palette log loading now shares that path rather than the old background
focused-buffer replacement path. This is not a latency improvement or completion
of the deterministic-update boundary. The next step must make file opening an
asynchronous, target-aware effect with post-open navigation continuations, then
share resolved identity across tab reuse, navigation, LSP and Problems. Preserve
group/focus and document/revision identity across replies; audit the existing
unkeyed `SaveCompleted`/`FileLoaded` replies as part of that migration. The runtime
already retains LSP `OpenDocState` URIs, but only for LSP-synced documents, so it
cannot by itself be the general file-identity authority.

Ordinary forward-edit migration, Find/decoration profiling, inline Phase 3/5+,
settings UI, remaining LSP features and carried verification debt remain in
scope. Nothing was committed or published. Keep this handoff: it is not complete.

## Shortcut/keymap checkpoint — 2026-09-06

Audit finding 6 is implemented. Runtime dispatch, palette hints, context-menu
hints and automation share the model-owned loaded keymap. All 69 static shortcut
metadata entries are gone. Hints respect user overrides, unbinding, conditions
and sequence shadowing without consuming pending input. Embedded YAML is the
only full default registry, with a minimal Save/Open/Quit emergency fallback.

Runtime regression testing also exposed and fixed two chord defects: YAML now
accepts space-separated sequences, and keyboard dispatch resolves each event
once so its global-command probe cannot consume a non-global chord. Chords start
in editor routing; timeout/status UI and broader focus routing remain future
work. Keymap file changes still require restarting the application.

Eleven new regressions cover defaults, sequence resolution, palette/menu hints,
platform keycaps, configuration and actual runtime dispatch. `just test`:
**2,180 passed**, 7 skipped; **2 doctests passed**, 6 ignored. Strict `just lint`
and scoped diff-based self-review passed. Optimized full-palette shortcut
resolution: **8.75 µs median / 9.25 µs p95**, 500 samples, excluding setup,
keycap construction and rendering. Details are in the shortcut follow-up in
`docs/dev/refactoring-audit-2026-09-06.md`. Nothing was committed or published.

Next audit priority: runtime-owned file identity/effects (finding 7), then
ordinary forward-edit position migration and remaining Find/decoration profiling.
Inline Phase 3/5+, settings UI, remaining LSP features and carried verification
debt remain in scope. Keep this handoff: its full scope is not complete.

## Movement/update-surface checkpoint — 2026-09-06

Audit findings 5 and 8 are implemented. The 24 all-cursor wrappers now share one
internal target/Move-or-Extend operation, and editor updates share viewport,
blink and redraw handling. Existing selection collapse, smart Home, wrapped
desired column, deduplication and page-reveal policies are preserved. All 17
leaf message handlers are restricted to the update module tree; direct handler
re-exports and unused public LSP/syntax scheduling exports are gone. Existing
runtime/view helper consumers remain supported. User-facing commands are unchanged.

Three new integration regressions cover the 24 target/selection combinations,
reversed collapse/no-op word policy, and wrapped short-row movement through the
main dispatcher. Two compile-fail doctests guard the public message boundary.
`just test`: **2,169 passed**, 7 skipped; **2 doctests passed**, 6 ignored.
Strict `just lint` and scoped diff-based self-review passed. Details and the
optimized production-update sanity check are in the movement follow-up in
`docs/dev/refactoring-audit-2026-09-06.md`. Nothing was committed or published.

Next audit priorities: live-keymap shortcut hints and runtime-owned file
identity/effects (findings 6–7), then ordinary forward-edit position migration
and the remaining Find/decoration profiling opportunities. Inline Phase 3/5+,
settings UI, remaining LSP features and all carried verification debt remain
in scope. The full handoff is not complete; keep this file.

## Find/overview checkpoint — 2026-09-06

The next audit priority is implemented and verified: Find results now defer
overview lines until needed, using their immutable rope snapshot and an adaptive
line walk. Full and caret-only redraws share a per-pane Find/diagnostic pixel-row
projection, replacing repeated tick vectors and tree reduction. Cache validation
covers result identity, document identity/revision, actual wrap mapping, track
height and in-place diagnostic ranges/severity. Painting uses current colors.

Eight new regressions cover lazy/snapshot search, Unicode/newlines/EOF, cache
invalidation, wrapping, producer priority and reference-projection parity.
`just test`: **2,166 passed**, 7 skipped, 6 ignored doctests; strict `just lint`
and diff-based self-review passed. Optimized repeated 100,000-line medians:
Find results after edit **21.61 → 7.36 ms**, warm Find render **3.29 → 1.25 ms**,
and edit update + render **25.03 → 13.56 ms**. Permanent miss/typing workloads,
full measurements and limitations are in the Find/overview follow-up in
`docs/dev/refactoring-audit-2026-09-06.md`.

Find still scans the whole buffer synchronously on a miss. Native sampling now
points to repeated visible-line preparation in decoration rendering (~63% of
warmed main-thread samples), a separate next profiling opportunity. Movement,
shortcut, file-identity and public update-surface consolidation remain, as do
ordinary forward-edit position migration, inline Phase 3/5+, settings UI, remaining
LSP features and carried verification debt. Keep this handoff: its full scope is
not complete. Nothing was committed or published.

## Profiler fixture checkpoint — 2026-09-06

The next audit priority is complete: multi-split profiling now uses independent
documents and cycles real file inputs correctly. CSV/TSV inputs use grid mode;
code inputs have language metadata and fresh highlights. Missing/non-UTF-8 inputs
fail setup. Mode-specific scrolling uses shared clamping helpers, and font/shell
scale is consistent. `--include-csv` explicitly reserves the final synthetic pane.

Five new regression tests cover these contracts. `just test`: **2,158 passed**,
7 skipped, 6 ignored doctests; `just lint` passed. The optimized mixed-mode run
and real-file cycling smoke test use the new `just profile-render` recipe.
Measurements, limitations and commands are in the profiler follow-up section of
`docs/dev/refactoring-audit-2026-09-06.md`; no before/after speedup is claimed.

Next audit priority: cold Find result construction and cached overview projection,
then movement/shortcut/file-identity/public-update-surface consolidation. All
inline Phase 3/5+, settings UI, remaining LSP features and carried verification
debt remain in scope. Nothing was committed or published. The user permits
deleting this handoff only once its entire remaining scope is implemented and
verified; that condition is not yet met.

## Dropdown context checkpoint — 2026-09-06

The latest request targeted misleading dropdown completions in the `build.rs`
`cc::Build` chain. The screenshot entries are genuine methods in locked cc
1.2.67, but missing signature metadata made them look arbitrary. Independently,
word/snippet fallback polluted member lists, and empty local results prevented
LSP requests. Those client-side defects are now addressed:

- Code member access (`.`, `::`, `->`, including multiline chains) is LSP-only.
  Local words require prefixes, exclude syntax-highlighted comments/strings and
  non-identifier captures, reject numeric/symbol noise, and retain proximity.
  Uncolored identifier-shaped code words remain eligible; this is not a scope
  resolver. Fresh syntax is required for code fallback.
- Empty visible results no longer cancel the request session. Pending sessions
  do not claim editing keys, suppress inline suggestions, or appear as an open
  popup in automation. Escape cancels them; stale replies cannot reopen them.
  A fresh parse refreshes locals without restarting the LSP debounce.
- Server relevance/preselection and structured method parameters/return metadata
  are retained. User navigation wins over later server preference.
- Regression coverage includes the real update/input path, existing fake-server
  runtime, renderer rows, syntax, Unicode and selection preservation. The live
  rust-analyzer GUI check remains manual; nothing was committed or published.

Implementation notes and the current policy are consolidated at the top of
`docs/feature/autocomplete.md`. `just bench-completion` now covers fresh code-word
fallback as well as server conversion, refiltering and typing. Final verification:
`just test` **2,153 passed**, 7 skipped, 6 ignored doctests; `just lint`,
`just fmt-check`, `git diff --check` and diff-based self-review passed. Optimized
CPU medians: typing with 1,000 carried items **103.7 µs**, refiltering those items
**79.62 µs**, converting a 1,000-item response **971.8 µs**, and collecting/filtering
500 fresh code words **310.7 µs**. These exclude LSP latency, parser setup and
rendering; no end-to-end speedup is claimed. Logs: `/tmp/token-completion-final-`
`{test,lint,fmt,bench}.log`. The optimized build still reports the pre-existing
release-only unused `revision` warning in `src/update/syntax.rs`.

The broader
refactoring/profile opportunities, inline Phase 3/5+, settings UI and remaining
LSP features below are still unfinished; this checkpoint does not narrow that
roadmap or mark it complete.

## Edit-position consolidation — 2026-09-06

Continued the remaining handoff work by fixing the audit's highest-priority
completion cursor defects. Word and LSP completion now use the shared planned
edit transaction. Character-offset mapping preserves cursor/selection endpoints
across panes, clips positions inside replacements, and gives insertions right
affinity. Overlapping word prefixes complete once; snippet caret offsets remain
feature-owned. Inline acceptance no longer needs a separate selection-restoration
loop. Undo and redo share one buffer traversal and transform peer pane positions
in actual history order. LSP boundary inserts are no longer swallowed.

The two reproduced defects have durable regression tests, with additional
coverage for unsorted Unicode cursors, overlapping prefixes, reversed selections,
replacement clipping, snippet redo and equal-point LSP insert ordering.
Verification: `just fmt`, `just test` (**2,136 passed**, 7 skipped; 6 ignored
doctests), `just lint`, `just fmt-check` and diff-based self-review passed.
History retains the existing invoking-pane caret restoration convention; this
does not add complete per-pane selection/active-cursor snapshots to undo records.
Ordinary forward typing/deletion still has legacy position-sync helpers and can
be migrated separately; do not claim all edit APIs are consolidated.

Next audit work: repair the profiler's independent-document/mode fixtures, then
Find cache misses and shared overview projection. The entire inline Phase 3 and
5+ remainder, settings UI, remaining LSP surfaces and carried debt below remain
in scope. Changes are uncommitted and nothing was published.

## Refactoring audit refresh — 2026-09-06

The latest requested pass reviewed deduplication/API consolidation opportunities,
then profiled the current optimized code. Findings and measurements are in
`docs/dev/refactoring-audit-2026-09-06.md`. No application code changed in this
pass. Both ordinary-completion cursor defects still reproduce; prioritize a
shared edit-position contract, profiler fixture repair, and Find/overview work.
Also reduce redundant movement wrappers and public leaf update entry points,
derive shortcut hints from the live keymap, and consolidate file identity.

Fresh medians: navigation with 10,000 undo entries 1.0 µs; rendering 100,000
short lines 0.313 ms; warm Find rendering with 100,000 matches 3.303 ms;
Find cache rebuild after editing 21.806 ms. Native sampling attributes roughly
55% of warm Find samples to scrollbar rendering. These are CPU timings, not FPS.
The feature roadmap below remains separate and unfinished; cancellation and
Ollama are already implemented, so do not repeat that work.

## Cancelable provider checkpoint — 2026-09-06

This checkpoint takes precedence over the historical entries below. Inline
Phase 3 now has real cancellation and its second transport, Ollama:

- `InlineProvider` returns a cancelable future; `InlineRequest` contains no
  HTTP/config fields. `InlineJob` carries the provider configuration separately.
  A non-HTTP heuristic test exercises the same response pipeline.
- A single latest-value watch slot replaces queued per-document jobs. Dropping
  the provider future cancels socket waits; one generation-tagged debounce and
  a pane/cursor/revision/config session guard cover supersession, dismissal,
  edits, selections, pane/focus changes, provider changes and shutdown.
- llama.cpp and Ollama share a reqwest/rustls HTTP(S) client with total deadlines,
  a 1 MiB response limit, no redirects or ambient proxies, and sanitized errors.
  Ollama accepts `model` and `keep_alive` (seconds, default `-1`), and reports
  suffix-capability rejection without silently degrading to prefix-only input.

Verification: `just fmt`, `just test` (**2,127 passed**, 7 skipped; 6 doctests
ignored), `just lint` and diff-based self-review passed. Fake-server tests prove
actual TCP disconnection on cancellation/shutdown, latest-request completion,
Ollama acceptance/undo, bounded chunked bodies, timeouts and stale lifecycle
guards. No live Ollama model, hosted HTTPS endpoint or native UI checklist was
exercised. Server-side GPU cancellation depends on the server honoring disconnects.

**Next:** OpenAI-compatible and Mistral FIM with environment-based credentials,
prompt formats/inference, then context ring, alternatives, cache/filters. All
Phase 5+ work, settings UI, remaining LSP surfaces and carried debt remain in
scope; the full goal is not complete. The preceding repository audit is in
`docs/dev/refactoring-audit-2026-09-06.md`; its completion-cursor defects and
other recommendations remain open. Changes are uncommitted; nothing was published.

## Goal continuation — 2026-09-06

The user resumed the full remaining handoff goal after the refactoring pass.
Inline Phase 3 now has word/line acceptance, conditional word acceptance keys,
palette/automation actions, per-portion undo and retained remainder. Acceptance
uses the active cursor and preserves peer cursors/selections through the shared
edit planner. A fake-backend automation test exercises word → line → full
acceptance; Unicode, stale reply and split-pane regressions are covered too.

Verification: `just fmt`, `just test` (2,113 passed, 7 skipped; 6 doctests
ignored), `just lint` (all targets/features), and diff-based self-review passed.
No native interactive checklist or additional provider was exercised this turn.

The goal remains incomplete: cancellation/provider boundary and the additional
transports, prompt formats, context ring, alternative cycling, cache/filters,
all Phase 5+ work, settings UI, remaining LSP surfaces and carried debt remain
in scope. Next is the cancelable provider boundary together with the second
transport. Changes remain uncommitted; nothing was published.

## Continuation checkpoint — 2026-09-05

The original history below is retained; this checkpoint takes precedence for
current implementation status. Changes are uncommitted and nothing was published.
The full handoff scope is **not complete**.

**Latest request:** the user switched to a repository refactoring/performance
pass. Its implementation, measurements and remaining opportunities are in
`docs/dev/refactoring-profile-2026-09-05.md`. Shared editing primitives and
action routing, Find caching, history-free navigation, segment-local rendering,
viewport sizing, config runtime effects and production profiling tools landed
uncommitted. Verification: 2,108 tests passed, 7 skipped; 6 doctests ignored;
strict all-target/all-feature Clippy clean. Do not confuse this pass with
completion of the original feature roadmap below.

- **Soft wrap: all eight implementation phases complete.** Per-pane Alt+Z,
  word-boundary cache, shared `TextViewportMap`, rendering/gutter, visual
  navigation, selections/rectangle selections, mouse/IME anchors, scrollbars,
  incremental edit/reload updates, automation, release benchmark and screenshot.
  The plan is archived at `docs/archived/soft-wrap.md`, with implementation and
  verification notes. Unicode follows the existing renderer's character-cell
  model; this does not add wide-glyph typography, folding, or virtual ghost rows.
- **Inline Phase 2 gaps 1–2 addressed.** A shared visibility guard prevents
  explicit long-tail suggestions from painting or intercepting Tab. Added
  status-bar progress and automation `inline_in_flight`, fixed missing ghost
  paint in debug full redraws, and request-ID guards against superseded replies.
  Cancellation and the additional transports remain undone.
- **Settings Phase 1 complete.** Config saves preserve unknown nested YAML keys,
  distinguish removed known optional fields/map entries, and refuse to overwrite
  invalid/unreadable files. Comments and formatting are not preserved. The
  settings modal and LSP section remain undone.
- **Verification:** `just fmt`, `just test`, `just lint`: 2,128 tests passed,
  7 skipped; 6 doctests remain ignored. `just bench-wrap` and the release
  `screenshots/scenarios/soft-wrap.yaml` screenshot were run and inspected.
  The broader interactive manual checklists remain unrun. Release builds still
  report the pre-existing unused `revision` warning in `src/update/syntax.rs`.

**Next:** inline Phase 3 (cancelable provider boundary with second transport,
Ollama/OpenAI-compatible/Mistral, prompt formats, context ring, partial accept,
alternatives and cache), then Phase 5+ virtual rows/edit predictions/retrieval/
other providers; settings UI; remaining LSP surfaces and carried debt. Keep these
items in scope rather than treating the soft-wrap checkpoint as the whole goal.

Read `AGENTS.md` first — it is the canonical instruction file and this document
does not repeat it. The short version: Elm-style `Message -> Update -> Command ->
Render`, update handlers stay deterministic and I/O-free, `just fmt && just test
&& just lint` before handing off, record user-visible changes in
`docs/CHANGELOG.md` under `Unreleased`.

---

## 1. Where things stand

**v0.6.0 shipped 2026-09-02** (tag pushed, cargo-dist release workflow ran). It
carried the whole August LSP wave plus two new subsystems:

- **CLI launcher** (`src/launcher.rs`): `token file` returns to the shell
  immediately and hands paths to a running editor; a detached child starts one
  when none is running. `-w/--wait` blocks until the opened documents close or
  the editor exits (the git `core.editor` contract). Directories and
  `--new-window` always get their own process. Also `path:line:col`, `token -`
  for stdin, `--foreground`, and macOS Finder/`open -a` delivery via
  `application:openURLs:` (`src/macos_open.rs`).
- **Per-instance automation** (`src/automation.rs`): every editor process listens
  on `$TMPDIR/token-<uid>/instances/<pid>.sock` (a loopback port file on
  Windows). `token automate instances` lists them, `--instance <pid>` targets
  one, the MCP bridge gained `list_instances` and an optional `instance` argument
  on every tool. The snapshot field is `instance_id` (renamed from `process_id`)
  plus `workspace_root` and `focused_at_ms`.

**Since the tag** (7 unpushed commits):

| Commit                                  | What                                                                                      |
| --------------------------------------- | ----------------------------------------------------------------------------------------- |
| `0416ed0`                               | Archived seven shipped feature plans into `docs/archived/`, refreshed the roadmap indexes |
| `ea19fd7` `df6615f` `0b9f539` `06c97a7` | find-enhancements Phases 5 + 7 (below)                                                    |
| `76e5b94` `9d34829`                     | autocomplete Phase 2, inline ghost text (below)                                           |

Suite is 2102 tests, green. `just lint` and `just fmt-check` clean.

### find-enhancements: done, doc archived

`docs/archived/find-enhancements.md`. The Find label row carries a status
("3 of 42" / "No matches" / "Invalid regex: unclosed group" in the error colour)
through a new `Field::trailing` slot; the footer legend lists the four options
with ⌥⌘C/W/R/L and a check when on. Selection scope captures the primary
selection as char offsets and `FindReplaceState::matches()` is the single
filtered list navigation, replace, replace-all, and the highlight decorations all
read.

Deliberately **not** done: mouse-clickable toggle chips (needs a new
`OverlayHit` variant and a `UiKey`; keyboard reaches everything today), and the
`SearchResults` cache the original doc specified (search is stateless and
viewport-bounded — see the doc's Phase 1/3 notes).

### autocomplete Phase 2: inline ghost text, shipped with gaps

`docs/feature/autocomplete.md` (still active — Phases 3 and 5+ remain). What
landed:

- `src/completion/inline.rs` — `InlineSuggestionState`, `RequestSnapshot`,
  `InlineRequest`, the `postprocess` filter chain (filters 1–4), prefix
  consumption, the end-of-line trigger rule.
- `src/completion/fim.rs` — llama.cpp `/infill` over a ~100-line `std::net`
  HTTP/1.1 client. **No HTTP dependency was added** and none should be until a
  TLS/hosted transport actually needs one.
- `src/runtime/inline_worker.rs` — worker thread cloned from the syntax worker's
  shape; newest request per document wins.
- `src/update/inline.rs` — triggering, revision-guarded arrival, accept as one
  undo step, failure/backoff policy.
- Paint: `render_ghost_text_stage` in `src/view/editor_text.rs`, inside
  `render_line_content_stages` so the cursor-lines-only damage path draws it too.
- Config `completion.inline` + `completion.providers` (`src/config.rs`), theme
  key `editor.ghost_text`, `inline_suggestion` in the automation snapshot and in
  screenshot scenarios, keymap `inline_suggestion_visible` condition with Tab /
  Escape / ⌥\.

Verified live against `llama-server -hf
Qwen/Qwen2.5-Coder-1.5B-Instruct-GGUF:Q8_0 --port 8012`: ghost text in ~500 ms,
type-through, Tab-accept, single-step undo, and a responsive editor after the
server was killed mid-request. The model is cached on this machine; the server
does **not** auto-start.

---

## 2. What remains

Ordered as I would do it. Items 1 and 2 are the user's stated priorities.

### Priority 1 — Soft wrap (`docs/archived/soft-wrap.md`, implemented)

Completed in the continuation checkpoint above. The original brief is retained in §3.

### Priority 2 — Finish inline completion (autocomplete Phases 3 and 5+)

See §4. Phase 3 is M-sized and independent of soft wrap; the multi-row half of
Phase 5+ is blocked on it.

### Priority 3 — Settings keymap follow-up (`docs/future/settings-keymap.md`)

Settings v1 (preserving save, general preset UI and LSP section) is implemented
and archived at `docs/archived/settings-page.md`. The separate future keymap tab
remains; see the latest checkpoint above for verification and platform limits.

### Priority 4 — Remaining LSP surface

Workspace symbols are implemented and tested in the current working tree;
protocol foundation committed as `ebf2add`. Runtime/UI source remains with its
file-opening/runtime prerequisite groups. The persistent usages panel is now
committed as `7c1d55a`, alongside the existing popup. Native/live-server
verification and the remaining source grouping stay open; do not treat the
entire LSP follow-up as complete.

### Known debt carried forward

Eight bundled themes still ride derivation fallbacks (only `default-dark` is
hand-tuned). The `completion.menu` config block is implemented in the latest
checkpoint; scan-window and result caps intentionally remain internal. Two
`#[ignore]`d load-sensitive process-spawn tests (both explicitly passed serially
on macOS on 2026-09-07; parallel-load stability and default unignore remain open).

Also: Windows is compiled-by-reasoning only for the launcher (`DETACHED_PROCESS`,
always-detach) and the per-instance port file. Neither has been run on Windows.

---

## 3. Soft wrap — the brief

**Status:** Implemented 2026-09-05; original brief below is historical. See the
archived plan's implementation notes for the actual cache and viewport APIs.

### Why it is a prerequisite

Three separate features are parked behind it, all for the same reason: they need
a mapping between _logical_ positions (line, column in the buffer) and _visual_
rows (what the renderer draws), and today those are the same thing.

1. **Multi-row ghost text.** Today a multi-line suggestion draws its first line
   and collapses the rest into a `⏎ +2 lines` badge
   (`src/view/editor_text.rs:render_ghost_text_stage`). Drawing the real
   remaining rows means inserting visual rows that do not exist in the document,
   which is exactly what a wrap cache does. `autocomplete.md` Phase 5+ says so
   explicitly: _"Multi-row ghost text + mid-line suggestions — after soft-wrap's
   `logical_to_visual` mapping exists."_
2. **Mid-line suggestions.** Same reason: ghost text mid-line has to shift the
   following text right (Neovim's `inline` vs `overlay` distinction), which is a
   visual-row layout problem.
3. **Code folding** (`docs/feature/folding-basic.md`) needs the identical seam —
   a fold hides logical lines and renumbers visual rows.

### The one architectural decision that matters

**Do not build a render-only wrap cache bolted onto the draw loop.** The doc was
written before `TextViewportMap` existed and its own Data Structures section now
says this in a preamble.

`TextViewportMap` (`src/model/editor.rs`, struct at the `/// No-wrap mapping
between viewport rows/columns and logical document positions` comment) is already
the single seam every consumer asks "which logical line/column does this visible
row or pixel map to?". Its API:

```
doc_line_for_visible_row   visible_row_for_doc_line   contains_doc_line
doc_line_for_pixel_y       visual_column_for_x_offset
reveal_line_no_padding     reveal_line_with_mode      reveal_column
clamp_top_line  max_top_line  end_line  bottom_line   scroll_vertical_by
```

Consumers, all of which get wrap support for free if the map becomes wrap-aware:

- `src/view/editor_text.rs:118` (the renderer)
- `src/view/hit_test.rs:740` (clicks)
- `src/view/geometry.rs:247` and `:285` (layout)
- `src/view/caret.rs:65` (IME rect, completion popup anchor)

So: build `WrapCache` as the doc specifies (Phases 1–2), then make
`TextViewportMap` wrap-aware — or add a wrap-aware variant answering the same
queries in visual-row terms — and let the four consumers follow. Phase 3's
implementation note says the same thing.

### Phase order from the doc

| Phase | Work                                                                                                        | Est.  |
| ----- | ----------------------------------------------------------------------------------------------------------- | ----- |
| 1     | `src/wrap.rs`: `WrapCache`, `WrapSegment`, `compute_line_wraps`, `logical_to_visual` / `visual_to_logical`  | 3–4 d |
| 2     | `EditorState.soft_wrap` + cache, toggle message, invalidate on `push_edit`                                  | 2 d   |
| 3     | Wrap-aware viewport map; renderer iterates visual rows; gutter shows logical numbers + continuation markers | 4–5 d |
| 4     | Cursor Up/Down by visual line, `desired_column` across wraps, Home/End stay logical                         | 3–4 d |
| 5     | Selection rects per visual row                                                                              | 2–3 d |
| 6     | Mouse: click Y → visual row → logical position; double/triple click across wraps                            | 2 d   |
| 7     | Viewport tracks visual lines; scroll, reveal, PageUp/Down in visual rows                                    | 2 d   |
| 8     | Incremental cache updates on edit; benchmark large files                                                    | 3–4 d |

Phases 1–3 are the vertical slice that first shows something on screen. Ship
that, then 4, then the rest.

### Traps the doc already flags

- **Tab expansion must match the renderer.** `compute_line_wraps` takes a
  `tab_width` parameter in the sketch; reuse `TABULATOR_WIDTH` /
  `expand_tabs_for_display` from `src/view/geometry.rs` instead of a free
  parameter, or wrapping and painting will disagree.
- Wide Unicode is a `// TODO` in the sketch (`char_width = 1`). Decide
  deliberately; the editor is char-column based throughout.
- An empty line still occupies one visual row.
- Cache invalidation triggers: content edit, viewport width change (resize _and_
  split), and toggling wrap on.
- Non-goals, per the doc: hard wrap, configurable wrap width (use viewport
  width), character-level-only wrapping, bidi.
- `EditorState` is per-pane, so `soft_wrap` is per-pane — the doc's goal #1.
  Two splits on the same document can disagree, which means the wrap cache
  cannot live on `Document`.

---

## 4. Inline completion — what "fully complete" means

### Gaps in what shipped (fix these before Phase 3)

1. **Fixed 2026-09-05 — mid-line paint was unguarded.** `line_tail_is_short`
   (`src/completion/inline.rs`) gates _auto_-trigger only — it sits in the
   `if !explicit` branch of `schedule()` in `src/update/inline.rs`. An explicit
   ⌥\ trigger mid-line therefore produces ghost text that paints _over_ the
   text following the cursor, because `render_ghost_text_stage` overdraws.
   Either apply the same tail rule to the paint stage (cheap, matches the doc's
   v1 scope) or accept it only once soft wrap can shift the trailing text.
2. **Fixed 2026-09-05 — no in-flight status glyph.** `ui.inline_in_flight` is maintained but nothing
   draws it. `docs/feature/autocomplete.md` Configuration asks for a spinner
   segment; `SegmentId` lives in `src/model/status_bar.rs`.
3. **Fixed 2026-09-06 — cancellation.** Originally an in-flight `std` socket read could not be aborted, so
   supersession only drops _queued_ requests and the revision guard discards late
   replies. Fine against localhost; a slow remote transport will make the wait
   visible. Documented as a deviation in the doc's Phase 2 list.
4. **Fixed 2026-09-06 — provider trait.** Originally one implementation did not earn one
   — add it with the second transport, as the doc's deviation note says.

### Phase 3 (M, unblocked, no soft wrap needed)

- Transports: Ollama (`keep_alive`, per-model suffix capability surfaced as a
  clear error), OpenAI-compatible, Mistral FIM. This is where `PromptFormat`
  (sentinels, PSM/SPM, `Infer` from model name) becomes real, because those
  transports do not build the FIM prompt server-side the way `/infill` does.
  **This is also the point where an HTTP client dependency may finally be
  justified** (TLS for hosted APIs); until then keep `src/completion/fim.rs`.
  **2026-09-06 checkpoint:** Ollama and both hosted native-suffix shapes are now
  implemented on the shared reqwest/rustls client. Raw `PromptFormat` rendering
  and hosted service/model validation remain. See the latest checkpoint above.
- `RecencyRing` context strategy — llama.vim's design: no ranking, pure recency,
  so the prompt prefix stays stable and the server's KV cache is reused. Idle-only
  ring updates. Inline-as-comments fallback for transports without `input_extra`.
- Partial accept (Word / Line granularity, Zed's leading-run rule) and
  alternative cycling when the provider returns more than one.
- LRU cache (~256 entries) keyed on `(document, cursor, prefix tail hash)`, with
  the pre/post-cache filter split; postprocess filters 5–6 (bracket sanity,
  indentation normalisation).
- Automation coverage: suggestion visible → partial accept → full accept.

### Phase 5+ (after soft wrap)

Multi-row ghost text and mid-line suggestions first. Then, in rough order of
value: edit prediction (anchor-based edit lists, the Zed model), retrieval
context (BM25 / tree-sitter declaration extraction), TabbyML transport, a
supervised local llama-server child process, local acceptance stats, path
completion, commit characters.

### The manual checklist nobody has run

`docs/feature/autocomplete.md` → Testing Strategy → Manual checklist. Seven
items, none ticked. The two most likely to find real bugs:

- CJK/emoji around the cursor: popup anchor and ghost x-position.
- Blink fast path over visible ghost text (the `CursorLines` damage route) — the
  stage is inside `render_line_content_stages` specifically so this works, but it
  has not been eyeballed.

---

## 5. Working notes for whoever picks this up

**Build and verify**

```bash
just build        # debug
just test         # nextest + doctests, 2102 tests today
just lint         # clippy, same strictness as CI
just fmt          # rust + markdown
just release && just install   # ~/.local/bin/token
```

**Driving the real editor.** The automation socket is the fastest way to check
behaviour without a human at the keyboard:

```bash
export TOKEN_AUTOMATION_SOCKET=/tmp/token-dev.sock     # isolates from your own editor
target/release/token --foreground file.rs &
target/release/token automate state | jq .state
target/release/token automate cursor 10 4
target/release/token automate text "x"
target/release/token automate action AcceptInlineSuggestion
```

`docs/AUTOMATION.md` has the full command list. `EditorSnapshot`
(`src/automation.rs`) is the assertion surface — add a field there rather than
scraping pixels.

**Headless screenshots** for visual checks:
`cargo run --release --bin screenshot -- --scenario <yaml> --out-dir <dir>`.
Scenarios live in `screenshots/scenarios/`; the config struct in
`src/bin/screenshot.rs` supports `modal`, `inline_suggestion`, view modes,
workspaces and splits.

**Inline suggestions need a server**:

```bash
llama-server -hf Qwen/Qwen2.5-Coder-1.5B-Instruct-GGUF:Q8_0 --port 8012
```

The model is already cached. `~/.config/token-editor/config.yaml` has
`completion.inline.enabled: true` pointing at `http://127.0.0.1:8012`. With the
server down the editor shows one transient per failure and pauses auto-trigger
after three (`MAX_CONSECUTIVE_FAILURES`).

**Test conventions.** Unit tests drive `update()` directly with real `Msg`s
(`src/update/completion.rs` and `src/update/inline.rs` are good models). Runtime
tests build a headless `App::new(800, 600, cfg, None, None, None)` and push
through `automation_tx` + `process_automation_requests`
(`src/runtime/app_tests.rs`); `pump_until` drives `process_async_messages` for
worker replies. Fake servers bind `127.0.0.1:0` in-process — see
`fake_infill_server` in `app_tests.rs` and `fake_server` in
`src/completion/fim.rs`.

**Docs discipline.** Feature plans in `docs/feature/` are active; move one to
`docs/archived/` with a dated `> **Status:**` line when it ships, and fix the
inbound links such as `docs/README.md`.
`0416ed0` is the pattern.

**Release process** is in `AGENTS.md` §Releases. Preparing a release does not
authorize publishing it.
