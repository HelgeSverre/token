# Refactoring audit and CPU profiling — 2026-09-06

## Recency refresh — 2026-09-08

Commit `56a3be8` reuses exact unchanged snapshots while preserving current
capture, strict similarity, ordering and region invalidation. The
[report](../benchmark/2026-09-08-recency-refresh.md) records CPU attribution,
before/after repeats, edited-path costs and the small tracked-peak tradeoff.
All 2,581 tests/two doctests and strict lint passed; no new test functions were
added. Scoped review: **Approve**, no outstanding findings. The recency
investigation is addressed; edited idle costs remain documented, and forward
multi-cursor attribution plus native/live verification gates remain open.
No additional plan became archive-eligible.

## ASCII literal Find — 2026-09-08

Commit `780b13e` uses the existing Aho-Corasick dependency directly for valid
ASCII-only literal queries on ASCII-only text. Regex remains the validation and
Unicode/whole-word engine; construction failure falls back to it. Editor
occurrence selection retains its intentionally different overlapping contract.
No public API, worker, cache or new transitive package was introduced.

Dense 100,000-line cold medians changed from ~7.1 ms to 3.4–3.5 ms; worker
computation from 10.2–10.3 ms to 6.5–6.7 ms. The added matcher costs about 3 KB
in reported peak tracked allocation in the paired engine probe, not an extra
document copy. Existing tests were extended without new test functions; all
2,581 tests/two doctests, strict lint and formatting passed. No exit warnings
occurred in this run, but their historical cause remains open.

Scoped review: **Approve**, no outstanding findings. See the
[full report and raw measurements](../benchmark/2026-09-08-find-literals.md).
The measured cold Find target is addressed; recency/multi-cursor profiling and
native/live gates remain in `HANDOFF.md`. No additional plan became archive-eligible.

## Find overview projection — 2026-09-08

Commit `6974ba1` replaces per-line Rope slice traversal with an advancing chunk
suffix and Ropey's coordinate helpers. No matching semantics, worker lifecycle,
cache policy or public API changed. An initial prefix-rescan prototype regressed
and was discarded; the final dense 100,000-line worker medians were 10.2–10.3 ms
versus 11.1–11.6 ms before. The regex scan remains a separate target.

96 Find-focused tests and the full 2,581-test/two-doctest suite passed, plus
strict lint and formatting. The final full run flagged the existing inline-worker
supersession test as leaky; that investigation remains open. Existing coordinate
coverage was expanded without new test functions. Scoped review: **Approve**, no
outstanding findings. See [measurement, rejected experiment and verification](../benchmark/2026-09-08-find-overview.md).
No additional feature plan became archive-eligible; `HANDOFF.md` retains its
unfinished implementation and native/live verification gates.

## Completion response ownership — 2026-09-08

Commit `68cf62a` addresses the September allocation finding: eager `CompletionItem` → JSON
conversion for every candidate, followed by a deep JSON clone when scheduling
or requesting resolve. Menu presentation already needs separate normalized
snippet/edit fields; that does not require a second JSON representation too.

Rows now share an immutable `Arc<CompletionItem>` through both resolve commands
and the existing debounce. Only the runtime's actual request boundary serializes
it. Opaque `data`, label details, original snippets and commit metadata remain
untouched; display/acceptance fields retain their previous transformations.
Serialization failure follows the existing unavailable-server resolve fallback.
No provider API, worker, scheduler, dependency or menu behavior was added.

The existing conversion, round-trip, metadata, resolve/cancellation and rendering
fixtures were adapted. A pointer-identity assertion checks sharing across menu
clones. The existing asynchronous trace-label test now also covers completion
responses: debug builds log identity/count rather than format the full payload.
No test cases were added.

Verification: **216 completion-focused tests passed**; then **2,581 full-suite
tests passed**, five skipped, and **two doctests passed**, six ignored
(`be248240-c876-4626-a891-d859c89fe6bb`). Strict all-target/all-feature lint passed.
The full run had no process-exit warnings; the historical warning's cause remains
unresolved.

Scoped diff-based self-review checked immutable round trips, snippet/$0
precedence, upfront edits, shared ownership through both resolve paths and
failure fallback.

| Severity | File | Finding / resolution |
| --- | --- | --- |
| Medium | `src/update/mod.rs` | Generic debug tracing would format the entire typed payload. Completion responses now have a metadata-only trace label, checked in the existing test. |

No outstanding findings; verdict: **Approve**. Before/after measurement reduced fresh allocation from
4.398 MB to 838.5 KB per 1,000-item conversion; medians changed from 894.5 µs to
138.5–155.5 µs. The repeat had a 2.084 ms outlier. See the
[report and raw output](../benchmark/2026-09-08-completion-responses.md) for scope
and limits. No other plan became archive-eligible; completed history was trimmed
from `HANDOFF.md`, leaving its unfinished scope and current checkpoint intact.

## Workspace retrieval context — 2026-09-08

Commit `09895c3` adds `workspace_retrieval` as an opt-in alternative to recency context. A single
shared `LatestWorker` performs ignore-aware collection; no new scheduler,
filesystem work on the UI thread, persistent index or provider-specific context
pipeline. The existing grammar/outline registry supplies declaration ranges.
Exact source equality reuses those ranges; each traversal evicts sources that
were deleted, excluded or not observed within its limits. Eligible unsaved
buffers replace saved content. BM25 selection excludes zero-overlap chunks and
deduplicates overlapping regions/identical text before shared serialization.

The additional command/message stage reflects an actual asynchronous boundary:
prepare context, revalidate document/cursor/provider/workspace identity, then
submit to the existing provider worker. Recency remains synchronous and keeps
its idle behavior. Disable/strategy changes drop the retrieval worker and cache;
ordinary cancellation retains the cache but supersedes pending work.
See [configuration and resource/transmission limits](../user/config-editor.md#workspace-retrieval-context).

Four focused tests cover declaration relevance and Unicode bounds; ignore,
buffer and cache refresh policy; stale preparation replies; and the actual
background preparation-to-provider flow. Existing config and wire fixtures were
extended rather than duplicating the transport acceptance suite.

Final full-suite verification passed **2,581 tests**, five skipped, plus **two
doctests**, six ignored (`93feaa76-18f1-422e-9137-b18083c10d84`), without
process-exit warnings. This includes the review corrections below and
closest-to-cursor query priority. Strict all-target/all-feature lint, formatting
and diff checks also passed. The optimized ranking probe and independent
repeat measured **139–143 µs** at 32 files and **1.15–1.19 ms** at 256 files.
See the [benchmark report](../benchmark/2026-09-08-workspace-retrieval.md) for
allocations, raw output and exclusions; this is not total collection latency.

Scoped self-review corrected the following issues:

| Severity | File | Finding / resolution |
| --- | --- | --- |
| High | `src/update/mod.rs` | Generic debug tracing would include active source/provider configuration. The new message's trace label contains only its request ID, checked by the lifecycle test. |
| Medium | `src/runtime/inline_retrieval.rs` | Failed/invalid reads initially did not consume the budget. All attempted read bytes now count. |
| Medium | `src/completion/retrieval.rs` | Hash-map iteration could vary floating-point score summation. Term-frequency maps now sum in lexical order. |

No outstanding critical/high findings. Verdict: **Approve** for the implementation; native/live model relevance
and end-to-end latency are not established by fixture tests.

Reference APIs: [`ignore` WalkBuilder](https://docs.rs/ignore/0.4.32/ignore/struct.WalkBuilder.html)
and [BM25 term-frequency/length normalization](https://nlp.stanford.edu/IR-book/html/htmledition/okapi-bm25-a-non-binary-model-1.html).
The implementation uses the [positive IDF variant and defaults documented by Lucene](https://lucene.apache.org/core/9_12_3/core/org/apache/lucene/search/similarities/BM25Similarity.html),
`k1=1.2`, `b=0.75`.

Native checks remain open. The isolated context-menu window received edits and
palette input not sent by this task, then exited cleanly before pointer testing;
that uncontrolled session is not acceptance evidence. During process-exit
diagnosis, PID 38373 looked like an old watcher-test invocation, but its sampled
stack showed a normal winit/App event loop, not a stuck test. It was left untouched.
Neither observation identifies the intermittent nextest warning's cause.

## Managed local llama-server — 2026-09-08

Commit `31e3ff4` implements opt-in child ownership on the existing inline worker.
`local_server` supplies literal executable/model paths, a startup deadline and
context/GPU limits. Startup is demand-driven and loopback-only. The server uses
offline mode; inherited `LLAMA_*` settings are removed except the explicitly
referenced credential, passed through the child environment. No downloads,
shell command expansion, new generation thread or second HTTP provider path.

Configuration lifetime is separate from request cancellation: typing can cancel
HTTP work without unloading the model. Disable/configuration changes and window
exit kill and reap the owned child; startup timeout and crash handling require
explicit retry instead of a restart loop. The worker owner joins cleanup during
shutdown. The port preflight rejects an existing listener; externally managed
providers retain their existing behavior. Details and constraints are in the
[user guide](../user/config-editor.md#managed-local-llama-server).

Two focused tests cover configuration boundaries and a real-child lifecycle.
The latter shares the existing HTTP fixture parser and verifies startup,
generation, reuse, cancellation during loading, deadline enforcement, explicit
retry, reaping and occupied-port refusal. Review caught and corrected a fixture
issue: closing a health probe mid-header must not crash the fixture and falsely
appear to prove timeout cleanup. No extra test was added for that correction.

Fresh verification, with `CARGO_BUILD_JOBS=1`:

- **2,577 tests passed**, five skipped; **two doctests passed**, six ignored
  (`e082ece7-0af1-4241-8e31-24af02178241`). No process-exit warnings.
- Five repetitions of both managed-server tests passed
  (`1608feec-42d3-4a02-b7a9-47a28b91a92c`).
- Strict all-target/all-feature lint, debug build, formatting and diff checks
  passed. Scoped code review: **Approve**, no outstanding findings.

The initial full suite also passed, but its following doctest build failed when
the repository's entire `target/` directory disappeared. No cleanup command was
run by this task. All final checks above used a fresh isolated `CARGO_TARGET_DIR`;
the missing-artifact failure was not treated as a test or code failure.

An isolated native macOS window then used llama-server 0.3.0, build 10621
(`c1d0e7a00`) and the already-cached Qwen2.5-Coder-1.5B-Instruct Q8_0 GGUF.
Temporary configuration selected CPU execution, a 4,096-token context and a
24-token response cap. No server was started merely by opening the window.
An explicit request spawned a child whose parent was the test editor and
returned a suggestion. Accept Line inserted `a + b`; Undo restored the original
buffer. The same child remained loaded. Native Enter on Reload Configuration
applied an inline-disable change and removed the child/listener; re-enabling
and requesting again started a new child, and Quit removed both editor and child.
The pre-existing external llama-server remained alive. Normal user configuration
and documents were not changed.

This is a local compatibility/lifecycle check, not a model-quality or performance
benchmark: the suggestion continued beyond `a + b` into an unnecessary example
program. Windows/Linux process behavior and broader provider/model quality remain
open. No other plan became archive-eligible, and `HANDOFF.md` remains required.

## Spawn-test cleanup — 2026-09-08

Removed the ignored shell-script LSP handshake test: the existing
`full_lifecycle_including_workspace_configuration_mid_init` integration test
already exercises the real child, framing, initialization and Ready notification,
including the more demanding configuration request during initialization.

The two existing PTY tests now select a known shell without personal startup
files (`/bin/sh` with `ENV` removed; `cmd.exe /D /Q` on Windows). One private
command-taking helper keeps them on the production PTY worker path; the public
API and normal shell selection are unchanged. The echo assertion requires a
complete output line instead of accepting the terminal's echo of typed input.
Both tests explicitly terminate the child before their final assertion, and
the shell-exit test is no longer ignored. No new test or dependency was added.

Verification on macOS:

- Twenty repetitions of both PTY tests plus the existing LSP lifecycle test:
  **60 executions passed**, no warnings (`c61b2a06-0d37-408f-9e06-6bae834bc2b2`).
- Full suite: **2,575 tests passed**, five skipped; **two doctests passed**, six
  ignored (`d15ead82-6a6a-4f24-b278-43eeb3cfeb94`). No process-exit warnings.
- Strict lint, formatting and diff checks passed. Scoped self-review:
  **Approve**; no outstanding findings. Windows/Linux execution remains unverified.

This closes the two ignored spawn-test entries, not the intermittent nextest
warning investigation. The earlier Settings/startup warnings are still
unattributed; no runner timeout was increased or warning suppressed. Nextest's
[output-handle detection](https://nexte.st/docs/features/leaky-tests/) does not
measure heap leaks, and clean reruns alone do not establish a fix. No performance
or native UI claim is made by this test cleanup.

## Bundled overlay theme tuning — 2026-09-08

Commit `48a29fa` closed the original eight-theme follow-up: Fleet Dark, GitHub Dark/Light,
Dracula, Mocha, Nord, Tokyo Night and Gruvbox Dark now specify all 23 overlay
palette keys. Values were selected to retain each theme's palette identity,
with distinct text ramps and readable selection, shortcut and diagnostic colors.
Existing legacy overlay alpha is preserved; Settings remains opaque. The five
later built-ins and custom-theme fallback resolver were not changed.

An independent contrast calculation read the final YAML values, composited
translucent panels over both black and white, and evaluated text, keycaps,
selected matches and 15% severity-banner grounds. Ratios below are the worst
values in each group; banner values also include the conservative composited
panel cases. GitHub Light's dim/error text was darkened after this check exposed
insufficient contrast over black. The existing Rust theme tests additionally
check every registered built-in (currently 14); no new tests or harness were added.

| Theme | Panel text minimum | Keycap text | Selected match | Banner text minimum |
| --- | --- | --- | --- | --- |
| fleet-dark | 4.56 | 8.38 | 8.52 | 5.35 |
| github-dark | 4.90 | 9.86 | 9.09 | 5.55 |
| github-light | 4.71 | 7.71 | 10.61 | 4.53 |
| dracula | 5.94 | 8.28 | 7.89 | 6.14 |
| mocha | 7.37 | 8.69 | 8.70 | 7.00 |
| nord | 5.14 | 7.49 | 7.30 | 4.61 |
| tokyo-night | 6.44 | 8.09 | 9.43 | 7.40 |
| gruvbox-dark | 6.77 | 8.45 | 8.64 | 6.41 |

All eight command-palette screenshots (1400×1000 physical, 2×) and compact
scrolled Settings screenshots (400×750, 1×) were rendered from explicit YAML
paths and inspected. Selection, match highlighting, shortcut chips and text
hierarchy remain distinct. The existing command-palette fixture was corrected
from `theme_picker` to `command_palette`; its previous live-preview selection
would have overwritten the requested theme.

Reproduce for each theme ID with the existing screenshot tool:

```bash
target/debug/screenshot --scenario screenshots/scenarios/showcase-command-palette.yaml --theme themes/nord.yaml --width 1400 --height 1000 --out-dir /tmp/token-theme-palette
target/debug/screenshot --scenario screenshots/scenarios/settings-scrolled.yaml --theme themes/nord.yaml --width 400 --height 750 --out-dir /tmp/token-theme-settings
```

Final suite: 2,574 tests passed, seven skipped; two doctests passed,
six ignored. Nextest run `0d4fc96e-5401-41c0-a8dc-a618066499d8` flagged
`settings_page_keeps_spacious_categories_and_shared_control_hits` as leaky.
That process-exit issue remains open; it is not an assertion failure or proof
of a theme regression. Diff-based self-review: **Approve** for theme tuning
after the contrast correction. This is headless/macOS evidence, not a new
native-platform certification. Strict lint, formatting and a debug build passed.

## Source consolidation and Settings scrolling — 2026-09-08

The implementation backlog is committed as a coordinated series:

| Commit    | Scope                                                            |
| --------- | ---------------------------------------------------------------- |
| `ab96495` | Document edits, pane state, soft wrap and rendering              |
| `d4fd1a4` | Ordered file/configuration effects and startup preparation       |
| `6531ecf` | Contextual dropdown and inline completion pipelines              |
| `c2a1ee5` | Settings/keymap controls, override saves and live shortcut hints |
| `ac362d4` | Shared runtime contracts, dispatch, Find and workspace symbols   |
| `2bdbcca` | Benchmarks, fixtures, changelog and verification records         |

Working-file hashes matched before/after staging. These source groups share
contracts and are verified as a complete series, not as independently buildable
intermediate snapshots. The combined source passed 2,568 tests and two doctests,
run `98402f7e-0048-457c-888a-e28d3ae2d78d`.

Settings' first draggable scrollbar still converted positions to selectable
rows and skipped section headings. The correction uses physical-pixel offsets
and extends the shared `RowListView` to expose clipped partial rows, hit
coordinates and minimal pixel-based selection reveal. Editor/list row snapping
is preserved. Trackpad subpixel remainders accumulate at pixel granularity.
The corrected suite passed 2,571 tests and two doctests, run
`8da31a5a-edc7-42a2-b5d6-893f377bbfac`.

The handoff is now a short current checklist. Historical checkpoints below
describe their dates, not present-day source-group status. Completed baseline
plans are already archived; Settings keymap's cross-platform gate and the
autocomplete follow-ups remain open. The archived Settings proposal is
explicitly marked superseded where it conflicts with the user's separate-page
design. No additional incomplete plan was archived to make the checklist empty.

Settings CPU measurements and measurement limits are recorded separately in
[the September 8 report](../benchmark/2026-09-08-settings-scroll.md).
Pixel scrolling is committed in `67fa676`; the shared dimmer, benchmark harness
and raw measurements are in `8516dc2`.
The final scrolling/dimmer suite passed 2,572 tests and two doctests, run
`f462df76-3cca-4c02-8ab7-29836f9ebab2`, plus strict lint, formatting and a debug
build. Wide/compact scrolled screenshots were inspected for body clipping and
fixed chrome. Diff-based self-review found no outstanding critical/high issues
(Approve). Native presentation and the handoff's platform gates remain open.

## Keymap registries and contextual chord eligibility — 2026-09-07

Commit `e017bd6` removes two competing registries ahead of the Settings keymap
work. Bindable commands and their case-sensitive YAML names now come from the
same enum declaration. The independent snapshot has 126 variants; comparing its
old parser found two omissions, `ToggleUsages` and `RestartLanguageServer`.
The new regression failed on `ToggleUsages` before the fix (run
`3fa666fb-8ebf-4485-adf5-bc2ff2fcd007`). A macro-generated test exercises both
`FromStr` and YAML parsing for every variant, including future additions;
unknown names still produce `InvalidCommand`.

Default construction and runtime loading now share the embedded YAML, parsed
once with independent cloned snapshots for overrides. The large hardcoded
default list is removed; only Save/Open/Quit remain as deliberate emergency
controls if embedded YAML is invalid. User-keymap load/merge behavior stays the
same. Regressions cover equality with parsed embedded defaults, independent
snapshots, exact/contextual replacements, chord overrides, unconditional
sequence-wide `Unbound` removal and the minimal emergency controls. This does
not add presets, change override precedence or implement hot reload.

The registry-only group passed **2,156 tests**, seven skipped, and **two doctests**,
six ignored, plus strict lint and formatting. Run
`14a21f1f-e01b-4a59-9357-6b9af7115b3f`. It contains five explicit files, with
177 inserted and 406 deleted lines including tests, changelog and usage notes.
The broader command metadata, shortcut hints, new completion actions and chord
YAML/runtime routing remain separate source groups.

### Chord context correction

Commit `4a00fa5` fixes a concrete context mismatch: completed bindings checked
conditions, but starting or extending a chord only checked its key sequence.
An inactive conditional branch could therefore consume input or prolong pending
state. Two new regressions failed against the old engine with `AwaitMore` instead
of `NoMatch` (run `1148cc5a-42a0-4b61-981e-a8b97db9b27d`).

One keymap-private binding-eligibility helper now governs conditional single
bindings, complete chords and every partial prefix. Conditional bindings require
a context, all predicates must match, and eligibility is reevaluated on each
stroke. Eligible alternative branches remain usable; context changes or missing
context cannot keep an ineligible branch pending. Existing single-stroke and
complete-chord precedence is unchanged. The documented `sidebar_focused`
condition and its case-insensitive aliases now parse instead of rejecting the
entire user keymap.

Four new regressions cover the shared predicate, missing/changed contexts,
initial and intermediate prefixes, eligible alternatives, successful completion,
pending-state reset and sidebar-condition parsing. The independent check keeps
the committed single-keystroke input API; the working tree's multi-interpretation
routing uses the same predicate without pulling that migration into this commit.

The context-correction group passed **2,160 tests**, seven skipped; **two
doctests passed**, six ignored, with strict lint and formatting. Run
`edafa848-0d58-4573-a3d7-c7146e0c3fbd`.

Final full working-tree integration passed **2,538 tests**, seven skipped; **two
doctests passed**, six ignored, plus strict lint, formatting and diff checks.
Run `1a87a6bd-9596-4f05-bc52-e2f0b0dff8b5`. The final suites reported no exit
warnings; earlier startup/process warning debt remains unresolved.

### Review and remaining scope

Scoped self-review: **Approve**; no unresolved critical/high findings. Each
staged source group matched its independently checked patch byte-for-byte and
preserved the affected working-file hashes. Changelog and user guide changes
are grouped with their code; this audit and the future Settings plan are separate.
The temporary verification checkout was removed only after matching the committed
source.

These are verified foundations, not completion of the Settings keymap tab.
Merged/searchable rows, conflict analysis, chord capture with explicit save/cancel,
override persistence, base presets and native/platform verification remain open.
No new benchmark or native keyboard/pointer run was performed; existing performance
reports retain their snapshot boundaries. No additional whole plan is ready to
archive, the temporary handoff remains necessary, and nothing was pushed or
published.

## Persistent grouped usages panel — 2026-09-07

Commit `7c1d55a` separates Find Usages from Show Usages: Find opens persistent
results in the bottom Usages dock; Show retains the transient cursor popup.
Results are sorted by native path, line and UTF-16 column, deduplicated and
grouped by file. The panel reports its source position, loading, cancellation,
unavailable/indexing servers, empty results, timeouts and the 200-location cap.
Closing the dock or navigating away retains completed results; reopening it
does not issue a new query. Results are snapshots, not a live-updating index.

The existing LSP references request and bounded preview worker remain the only
effect pipeline. A captured popup/panel destination follows the request through
gating, timeout, preview preparation and reply. Allocation-identity query tokens
reject old and duplicate panel replies, including repeated requests from the
same cursor. Source document/revision checks invalidate pending searches on edit
or close; moving the caret or changing focus does not. Show Usages cancels a
pending panel search explicitly. Server/root cleanup now emits a terminal outcome
before discarding request deadlines, preventing an indefinitely loading panel.
Receiving results neither changes focus nor reopens a closed dock.

A single row projection and the shared `RowListView` drive labels, layout,
rendering, hit testing, scroll limits and paging. Selection and file chevrons use
the existing tree geometry. Left/Right collapse/expand; Enter toggles a group or
navigates, single clicks select and double clicks navigate. Mouse activation
preserves the returned navigation command. Panel keyboard focus captures text
input before CSV/sidebar dispatch; Escape returns to the editor. Popup and panel
activation share one route-hint/history/UTF-16 navigation helper. Automation uses
the same row labels and reports loading, selection and scroll state.

### Verification and limits

Seventeen new regressions cover grouping/order/deduplication, the cap, distinct
request destinations, supersession, duplicate/foreign tokens, source changes and
closure, closed-panel/focus behavior, terminal outcomes, keyboard/mouse actions,
collapse/expand, paging, extreme scroll deltas, automation and clipped selection
painting. A real-stdio fake-server test exercises the panel request and preview
pipeline alongside the existing popup test; a runtime test covers server/root
cleanup. Geometry tests cover the existing bottom/right generic dock layouts.
The left-dock focus-routing test is not a claim of generic left-dock rendering.

The independent feature group passed **2,150 tests**, seven skipped; **two
doctests passed**, six ignored, plus strict lint and formatting. Run
`4ae7be60-393c-4f19-83f6-58c6e71e37a6`. Isolation exposed a navigation fixture
that used a nonexistent path accepted by the pending file-opening refactor;
a real temporary file now tests both committed and working-tree navigation.
The isolated group keeps the committed constructor and command-registry shapes,
leaving their unrelated migrations in the working tree.

Final working-tree integration passed **2,530 tests**, seven skipped; **two
doctests passed**, six ignored, with strict lint and formatting clean. Run
`a373ab62-2f4d-4f67-8eef-64cfd9c2ae51`. Neither final suite reported exit
warnings; the older startup/process warning debt is not thereby resolved.

Scoped self-review verdict: **Approve**, with no unresolved critical/high findings.
The staged and committed source patch matched the independently checked patch
byte-for-byte; all 29 working-file hashes were preserved by staging. The temporary
verification checkout was removed after this comparison. Changelog and user guide
are included with the feature; this audit and the archived LSP checklist are a
separate documentation group.

No native panel screenshot, live rust-analyzer quality run or new benchmark was
performed for this group. Existing performance reports retain their explicit
snapshot boundaries. No additional whole plan is complete: remaining source
groups, full completion/Settings/Find scope and platform verification still keep
the temporary handoff open. Nothing was pushed or published.

## Bounded usages-preview prerequisite — 2026-09-07

Commit `50f3a68` removes unopened-file preview reads from the event loop before
extending usages into a persistent dock panel. The previous implementation read
whole files and allocated every line on the event-loop thread, up to 200 files
per response. Target resolution now uses a bounded ordered set, deduplicating
identical path/line/UTF-16-column targets before the existing 200-location cap.
Open-document lookup reuses the pure file-identity snapshot, without filesystem
canonicalization; unsaved rope snapshots take precedence over disk text.

The shared replaceable worker now supports typed replies as well as ordinary
messages. It retains one active job and one replaceable pending job, signals
cancellation on replacement/drop, and does not join blocked speculative work on
the event-loop thread. References read at most 1 MiB per file and 4 MiB per
response, retain one file's text at a time, traverse ordered lines once, and cap
preview text at 240 Unicode scalar values. Control characters become spaces.
Only regular opened handles are read; Unix opens use nonblocking mode so a FIFO
does not stall the open. No new dependencies or language-server capability were
introduced. Find/path workers remain in their later source groups but share this
now-committed worker in the working tree.

A 250 ms preview deadline is scheduled in the existing runtime wake calculation.
When processed, it returns navigable locations without previews if the worker
has not replied. Missing/unreadable files, exhausted read budgets, worker startup
failure and worker panics likewise retain locations. A blocked filesystem call
may outlive cancellation; the deadline does not promise an interruptible OS read
or hard real-time rendering. Lines beyond the bounded prefix have no preview.
Navigation coordinates and route hints remain unchanged.

Allocation-identity tokens span both the LSP request and preview job, rejecting
superseded network replies and already-queued worker replies even when the same
document/revision/cursor is reused. Review caught an ordering gap: ready previews
must be polled after queued user intents have been processed. That correction is
covered by a runtime regression; existing document/revision/cursor guards still
run when the resolved message reaches update.

### Verification and review

The exact independent patch passed **2,133 tests**, seven skipped, and **two
doctests**, six ignored, plus strict lint and formatting. Final independent run:
`0e816902-96ea-4571-b7a9-f0623f15bd7a`. Eight reference regressions cover Unicode,
unsaved buffers, unreadable files, read budgets, cancellation, queued replies,
deadline fallback, wake/origin metadata, row ordering/deduplication and routing.
Three shared-worker tests cover replacement, panic recovery and nonblocking drop.
Existing real-stdio reference-popup and stale-revision tests also pass.

Final main integration: **2,513 tests passed**, seven skipped; **two doctests
passed**, six ignored, with strict lint, formatting and diff checks clean. Run
`20638065-87be-48e3-a4d7-81947496810f`. No exit warnings appeared in either final
suite; earlier startup/process warning debt is not thereby resolved.

Review verdict: **Approve**, with no unresolved critical/high findings in this
group. Selective staging matched the isolated patch byte-for-byte and preserved
working-file hashes. The usages dock panel itself remains unimplemented. No new
native/platform, live-server quality or release-performance claim is made; these
are functional bounds and scheduling tests, not a new benchmark report. No whole
plan is newly complete, and nothing was pushed or published.

## Shared file identity prerequisite — 2026-09-07

Commit `3c4ca44` gives loaded documents one immutable original/resolved-path
snapshot. LSP document opening and diagnostic clearing capture or reuse it at the
runtime boundary; diagnostics and current-file Problems share pure document
lookup. The duplicated filename-gated canonicalization scans are removed. Path
changes invalidate old aliases instead of silently reusing the previous identity.
The tab-opening lookup and async save/reload integration remain in later groups.

Review found that lossy filename-to-URI conversion could merge distinct Unix
native filenames. Encoding now preserves native bytes, decoding preserves them on
Unix, and prepared identities retain their supplied resolved path directly.
Root and parent components survive conversion; Unix paths resembling Windows
drive paths are not rewritten, and NUL/malformed percent encodings are rejected.
The working-tree file worker also compares resolved native paths and has a
regression proving that a raw-byte filename cannot reuse the distinct Unicode
replacement-character filename. That worker change remains with the async-I/O
group, outside this commit.

This is a Unix-local filename convention, not a claim of portable non-UTF-8
language-server interoperability. [RFC 8089 §4](https://www.rfc-editor.org/rfc/rfc8089.html#section-4)
distinguishes filesystem encodings and leaves non-UTF-8 choices outside its
general encoding recommendation. Windows/platform execution and hard-link/
watcher identity questions remain open.

### Verification and review

The independent group passed **2,122 tests**, seven skipped, and **two doctests**,
six ignored; strict lint and formatting passed. Nextest run
`d8a74de8-4133-46ce-a6c0-98182919295a`. Seven identity regressions cover snapshot
invalidation, parent/root/drive-like paths, native bytes, malformed encoding,
diagnostic/Problems agreement, and runtime identity refresh. Two existing
working-tree tests moved into their owning modules without dropping coverage.

Final main verification: **2,504 tests passed**, seven skipped; **two doctests
passed**, six ignored, plus strict lint, formatting and diff checks. Nextest run
`5f004685-3318-4c0b-9897-242d415374a9`. Earlier targeted identity run:
13 passed (`a53c676e-8189-4011-9a76-d534a1b201ee`), before the final runtime and
worker regressions were added. No exit-warning output appeared in these final
runs; older load-sensitive startup/process warnings are not thereby resolved.

Review verdict: **Approve**, no unresolved critical/high findings in this group.
Selective staging matched the independent patch and preserved working-file hashes.
The temporary checkout was removed after checking its patch and extra-file list;
Git retains the committed source. No new performance measurement or native visual
claim, no additional whole-plan archive, and no push/publication. Continue the
remaining file-I/O/startup/UI prerequisite commits and the full handoff scope.

## Workspace symbols: protocol foundation and Search Everywhere — 2026-09-07

Workspace-symbol search is implemented in the working tree: existing capable
language-server instances overlapping the workspace feed the Symbols tab and a
five-row All-tab summary. An empty-query `@` switches to Symbols when available.
One debounced query owns the multi-server fanout; it does not start servers.
Pending document changes flush before `workspace/symbol`. Query identity,
workspace/provider identity and server generations reject stale replies;
cancellation also checks generation so reused IDs on replacement servers survive.

The protocol decoder accepts legacy SymbolInformation and complete
WorkspaceSymbol locations, preserves UTF-16 positions and rejects malformed or
unusable locations. Retained rows are capped at 2,000, display strings are
sanitized/bounded, and URI lengths are bounded. Ranking and deduplication are
deterministic across server arrival order. Queries are limited to 256 characters,
debounced for 150 ms and timed out after five seconds per server. Partial failure
and result-limit states remain visible alongside usable results. Lazy symbol
resolution is not advertised or implemented.

The existing section-ordering helper drives rendering, keyboard/page/wheel
navigation, hit testing and automation. Symbol activation uses the shared
file-opening/navigation path, including UTF-16 conversion after asynchronous
opening and Back/Forward history. No separate source scanner, navigation service,
or document-keyed LSP request-map trio was introduced: this workspace query has
one owner and multiple server replies, unlike document FeatureSlot requests.

### Logical commit and review

`ebf2add` commits only the independent protocol foundation, typed response,
capability support, bounded rows/ranking, unit and real-stdio tests, and changelog.
It adds a direct `thiserror = "2"` dependency using the already-locked 2.0.17
package; unrelated dependency/lockfile changes remain unstaged. The runtime/UI
layer remains outside that commit with its file-opening/runtime prerequisites.

The exact protocol patch passed independently against `04d3140`: **2,115 tests
passed**, seven skipped; **two doctests passed**, six ignored. Nextest run
`33ca8bd9-e541-4a70-a399-a8cda65b3f6d`. Strict lint, formatting and diff checks
passed, with no exit-warning output. Initial isolation exposed the missing direct
error-type dependency; the final patch includes it. Working-file hashes matched
before/after selective staging. The temporary checkout was removed only after
matching its staged patch and confirming no extra files; the source is in Git.

Review verdict: **Approve** for that independent commit. The working-tree review
also fixed a generation-blind cancellation race and added a regression asserting
that replacement-server requests are not abandoned. Tests cover wire formats,
invalid/error/null responses, bounds, Unicode, ranking/deduplication, cancellation,
timeout, fanout, query ABA, provider restarts, navigation, geometry and automation.

### Working-tree verification and limits

Final main verification: **2,498 tests passed**, seven skipped; **two doctests
passed**, six ignored. Nextest run `8315d727-cebf-41da-b099-8dc856018130`.
Strict lint, formatting and diff checks passed. Nineteen new regressions include
per-server enablement/workspace scoping and explicit didChange-before-query
ordering, in addition to the earlier 18-test targeted run
`c790f22a-c58a-4f89-ac8e-afa8b4052d12`. Review verdict: **Approve** for the
working-tree workspace-symbol implementation, with no unresolved critical/high
findings. No exit warnings appeared in either final full run; the earlier
load-sensitive startup/process warnings are not thereby resolved.

There is no new native visual check, real-language-server quality evaluation,
cross-platform execution, or performance measurement in this checkpoint. Existing
benchmark reports retain their stated snapshot boundaries. A usages dock panel,
the remaining autocomplete/snippet/retrieval/prediction/provider work, Settings
keymap follow-up, Find/file-boundary and profiling/theme/platform debt remain.
Continue dependency-ordered source commits; do not archive another whole plan or
delete the temporary handoff yet. Nothing was pushed or published.

## Startup preparation and pure model construction — 2026-09-07

Startup now uses the normal runtime file-preparation and message-based tab
installation path. The separate model-side loader and its first-file/additional-
file branches are removed. CLI files retain their input order, with the first
successful document focused; duplicate paths and symlink aliases reuse one tab.
Images use image tabs and initial pane-fit geometry, binaries use placeholders,
and missing paths remain unsaved documents without creating files. Failed paths
are counted in the status message without discarding successful tabs; an entirely
failed startup retains the empty document.

`AppModel::new(width, height, scale)` now creates an empty in-memory model.
`with_document` accepts prepared text and preserves its identity/unsaved state
without opening the path. Configuration, theme and history loading moved to
runtime preparation. Workspace setup precedes file installation so recent-file
entries carry their workspace. CLI positions clamp against the first successful
document's logical line and character column, including Unicode.

Review found an additional boundary violation: recent-file insertion
canonicalized paths from update handlers. It now uses the document's captured
identity; the recording helper is crate-private and takes a document ID.
`RecentFiles::add` accepts a boundary-resolved path without I/O. Other recent-file
operations and workspace/legacy I/O helpers are not claimed to be pure.
Constructor consumers were migrated, with file-backed tests explicitly preparing
their documents. One cursor fixture no longer leaks its temporary directory:
the prepared document survives its removal. The overview and changelog describe
the changed Rust API and startup behavior.

Startup still completes on the preparation thread (with the existing synchronous
fallback if that thread fails). Before installing the model into the runtime,
preparation consumes file-open effects and installs replies; the final startup
pass dispatches syntax/LSP work for the completed session. Startup does not replay
redraw/notification/history-save commands. No new server, dependency, download,
native-window action or history-persistence policy was introduced.

Six new regressions cover special tabs/order/identity/recent entries, partial and
total failures, aliases, Unicode position clamping, workspace association and
pure prepared-model construction. Existing multi-file startup coverage now also
asserts the first file stays selected. Initial fixture assertions incorrectly
equated macOS display and canonical paths; those were corrected to distinguish
`/var` from `/private/var`, without weakening tab or identity assertions.

Final main suite: **2,479 tests passed**, 7 skipped; **2 doctests passed**, 6
ignored, nextest run `bdc8d49d-5db0-4cea-bb29-69deb813c7a6`. No leak warning
in that full run. Strict all-target/all-feature lint, formatting and diff checks
passed. Scoped implementation review: **Approve**, no outstanding critical/high
findings. No new performance measurement or native/platform verification claim.

Cleanup verification remains open. The targeted run
`fe8933d0-69cc-4420-8661-14ea0095b29e` passed six startup tests but marked
`startup_files_report_failures_without_discarding_successful_tabs` leaky.
A bounded 20-iteration stress run passed all **120 test executions** plus both
doctests (`7354d843-8c4f-4251-a8ec-5449c7e4e741`), with two exit warnings:
`multiple_startup_files_open_as_distinct_tabs` in iteration 1 and
`startup_workspace_files_record_the_workspace_and_keep_an_empty_workspace_usable`
in iteration 7. Nextest 0.9.118 was used without changing timeouts or suppressing
warnings. Its [leak detection](https://nexte.st/docs/features/leaky-tests/) concerns
output handles remaining open after a test exits, not a heap-leak measurement.
No matching test process remained afterward; the underlying cause is not
attributed or fixed, and clean reruns do not close that debt.

This source group depends on the still-uncommitted file-open/identity and runtime
foundations. Keep its API migration and consumers together when staging those
prerequisites; do not claim the current HEAD alone contains this implementation.
The audit checkpoint is committed separately. All remaining autocomplete, LSP,
Settings/keymap, Find, profiling, theme and platform requirements stay active.
No additional whole plan is ready to archive; retain the temporary handoff.

## Update-handler API commit — 2026-09-07

Commit `80e5538` keeps message handlers behind `update(model, Msg)`, narrowing
17 handler functions to their parent module and three navigation helpers to the
crate. Layout is private, outline is crate-visible, and internal LSP/syntax
scheduling helpers are no longer re-exported publicly. Handler bodies are
unchanged; three calls in modal test code now use the main dispatch entry point.

The 21-file group includes its changelog entry and two compile-fail doctests
protecting the external API boundary. Runtime effect helpers and read-only view
projections with existing callers remain public. In particular,
`create_default_keymap_file` remains available in this commit because the
committed runtime still calls it; moving that I/O is a separate pending group.
Private imports needed by existing internal callers are retained. An initial
isolated compile caught an omitted `save_lsp_document` import; restoring that
private import fixed the compile without changing behavior.

The exact patch passed independently against `87779d5`: **2,109 tests passed**,
7 skipped; **2 doctests passed**, 6 ignored. Nextest run
`7fb86c7a-92cf-475a-903f-cc758bf85b47`; strict all-target/all-feature lint,
formatting and diff checks passed. Scoped review: **Approve**, no outstanding
findings. No dependencies, handler algorithms or external effects changed.
Staged and isolated patches matched exactly, and staging preserved working-file
hashes. The temporary checkout was removed after checking its diff matched the
source commit and that it contained no extra files; committed source remains
recoverable through Git. Other worktrees and native windows were untouched.

The main working tree then passed **2,473 tests**, 7 skipped, and **2 doctests**,
6 ignored, with no leak warning: nextest run
`d14a8016-ad0e-4bc6-9916-1dc417ba97d1`. Strict lint, formatting and diff checks
also passed. These broader results include uncommitted features and must not be
confused with the isolated commit suite. No new performance or native/platform
claim is made, and the earlier unidentified transient cleanup warning remains
unattributed.

Continue dependency-ordered source commits and the full remaining feature scope.
No additional whole plan is ready to archive; the temporary handoff must remain.

## Unused editing API commit — 2026-09-07

Commit `96c3399` removes `Msg::TextEdit`, `EditContext`, `TextEditMsg`, their
dispatcher/bridge and the unused `RopeBuffer` wrapper. Searches against committed
code found no message producers or wrapper consumers outside those definitions,
their exports and their own tests. Document, modal and CSV input already use the
remaining handlers and shared editing primitives; no replacement dispatcher was
added. This intentionally narrows the Rust API exposed by the application crate.

The ten-file group removes 1,055 lines and adds 23, including its changelog entry
and corrected overview. Review caught a `StringBuffer::clear` test accidentally
included in the broader RopeBuffer deletion: that retained-type test was restored.
Historical rope-line microbenchmarks still compare the same synthetic algorithms;
their comments now distinguish those fixtures from the actual editor path. No
benchmark names, measurements or performance claims changed.

The exact source patch was independently verified in a detached checkout at
`7d7d9d2`: **2,109 tests passed**, 7 skipped, nextest run
`a6ca2fb5-7401-4247-86b1-42aa40c23e41`. Its doctest target succeeded with six
ignored examples. The count is 24 below the previous isolated baseline: six
RopeBuffer, four EditContext, four TextEditMsg and ten bridge tests were removed
with their obsolete subjects. Existing production-path tests remain. Strict
all-target/all-feature lint, formatting and diff checks passed.

Scoped review: **Approve**, no outstanding findings after preserving the retained
buffer test. Staged and temporary patches matched exactly; working-file hashes
confirmed partial staging did not overwrite the wider changes. The temporary
checkout was removed only after its diff matched the source commit and it had no
extra files. All source removals are recoverable through Git history. No unrelated
worktree, native editor window, dependency or external service was changed.

The current main working tree passed **2,473 tests**, 7 skipped, and **2 doctests**,
6 ignored; strict lint and formatting passed. The first run
`08770a77-7349-424e-9c46-d35653dc365a` reported one passing-but-leaky test in its
aggregate summary. Its fail-only status output did not identify that test, and no
test child remained when processes were inspected afterward. A fresh full run
with `--status-level leak` passed without warnings:
`0b8b09bc-808b-41de-b597-129006de3467`. This does not identify or fix the transient
cleanup warning; future full runs should retain leak-level output so a recurrence
can be attributed. Default ignored tests and platform gates remain unchanged.

These working-tree results include uncommitted features, unlike the isolated
2,109-test commit suite. Continue dependency-ordered source grouping and all open
handoff requirements; no additional whole plan is ready to archive.

## TabbyML transport — 2026-09-07

The missing TabbyML adapter now uses `transport: tabby` through the existing
`FimProvider`. The [official API](https://tabby.tabbyml.com/api/completion/) and
[connection example](https://tabby.tabbyml.com/docs/extensions/troubleshooting/)
were checked before implementation, with Context7 used to locate primary docs.
Native prefix/suffix segments and optional language IDs are serialized without
OpenAI prompt/model parameters. Gateway prefixes and trailing `/v1` handling
share the existing route logic; response texts share the bounded choices parser.
The server selects its model and generation length. Raw prompt formats and
requesting more than one alternative remain capability errors for this transport.

Credentials use the existing explicit environment reference, sensitive header,
HTTPS/non-loopback rule and redirect refusal. No ambient key discovery, clipboard,
Git URL, user identifier, acceptance telemetry or absolute current-file path is
added. Optional workspace-relative path/declaration attachment remains retrieval
work; the current provider-neutral request does not carry a workspace root.
Opt-in recent-buffer context keeps the same validated commented-prefix fallback
as other non-llama.cpp transports, including unsupported-language errors.

Review identified that an empty Tabby choices array must not trigger backend
failure/backoff. The shared parser now accepts empty lists for Tabby while keeping
the existing hosted transports' validation. A full worker/update regression
checks the no-suggestion result clears in-flight state without incrementing
failures or editing the document. Other new coverage includes Unicode segments,
gateway paths, credentials, language IDs, configuration round trips, alternative
bounds, deadlines, chunked body limits, socket cancellation and partial acceptance
with Undo. Existing recency and error tables also exercise Tabby. Shared test
helpers avoid duplicating the worker acceptance and cancellation fixtures.

The adapter does not start a server or download a model. No live Tabby model,
native input or new performance measurement is claimed. Server supervision,
structured retrieval and the rest of the autocomplete plan remain open.

Final verification: **2,472 tests passed**, 7 skipped; **2 doctests passed**,
6 ignored. Nextest run `2921ce4a-f9e9-4c95-b5ec-96527c7692e1` includes all eight
new Tabby regressions and the expanded shared tables. Strict all-target/all-feature
lint, formatting and diff checks passed. Scoped self-review: **Approve**, with the
empty-result issue fixed and no outstanding findings. No dependency was added.

Changelog, provider guide and active checklist were updated. This source remains
uncommitted with its HTTP/provider/editor prerequisites; the verification report
does not imply an independently buildable Tabby source commit. Continue staging
those prerequisites in logical groups. No whole feature plan is newly complete,
so the temporary handoff remains and no additional plan was archived.

## Shared editor primitives commit — 2026-09-07

Commit `7552b7e` removes the document editor's duplicate cursor, position and
selection definitions. Documents and small editable fields now use the same
types, with the existing model API re-exported from `editable`. Document-specific
text extraction stays with the model; generic selection and desired-column
behavior have one implementation. Constructor consumers were migrated together.
The seven-file group includes its changelog entry and a regression asserting type
identity, reversed Unicode extraction, exclusive range ends and desired columns.

The exact patch was tested in a detached checkout at `5d4a818`, independent of
the remaining dirty editor/completion foundations. Full nextest run
`4b42e9b1-87b8-4f99-9083-4844f21a9b67`: **2,133 passed**, 7 skipped. The doctest
target succeeded with all six examples ignored. The new targeted regression also
passed (`85d740ee-31fa-4bd0-b9ff-6297b1a436fd`). Strict all-target/all-feature lint,
formatting and diff checks passed. No new performance or native-input claim.

Scoped self-review: **Approve**, no outstanding findings. The shared methods
preserve ordering, half-open selection ranges and first-remembered desired-column
semantics. No dependency, I/O or rendering-loop changes were included. Staged and
temporary patches matched exactly; working-file hashes remained unchanged. The
temporary checkout was removed only after its patch matched the committed diff
and it contained no extra files. All removed checkout content is recoverable from
the commit. Other worktrees and running editor windows were left untouched.

After committing, the current main working tree also passed its full suite:
**2,464 tests passed**, 7 skipped; **2 doctests passed**, 6 ignored. Nextest run
`88462a6b-49a3-4f09-8375-d9d4e8f33cb6`; strict lint, formatting and diff checks
passed. These broader results include still-uncommitted features and must not be
confused with the isolated commit's 2,133-test suite. Remaining source groups,
whole-plan completion and native/platform verification are still open.

## Completion menu configuration — 2026-09-07

`completion.menu` now groups automatic dropdown opening, minimum candidate word
length and local-word policy. The existing top-level `completion.enabled` remains
the master switch for menu and inline requests, including explicit requests.
`menu.enabled: false` prevents new automatic dropdowns on word typing, server
trigger characters and paths. Ctrl+Space and refinement of an open session still
work, without disabling signature help or configured inline suggestions.

`menu.min_word_length` replaces the local source's hardcoded three-character
candidate minimum, preserving three as the default. It counts Unicode scalar
characters, has an effective floor of one, and does not change the two-character
typed-prefix trigger, snippets, filenames or server results. Syntax restrictions,
nearest-first scanning and scan/result caps remain shared and unchanged. The
collector has one explicit minimum argument, not parallel configurable/default
entry points; its benchmark retains the default-three fixture without a new
timing claim.

Legacy `completion.words` is accepted only at deserialization. Runtime policy has
one nested field; an explicitly supplied nested value takes precedence, while a
partial menu block retains the legacy word preference. Saving emits the canonical
nested key and removes the recognized legacy key before unknown-key preservation.
Tests cover repeated saves, unknown nested settings, invalid-file preservation,
defaulting and YAML/JSON round trips. No dependencies or configuration I/O were
added to update handlers.

Nine new regressions also cover character counts and identifier restrictions,
candidate versus typed-prefix length, manual refinement/dismissal, member requests,
signature help, inline acceptance/Undo and manual path continuation. The path test
exposed that folder acceptance reset the request's explicit flag; continuation
now preserves that provenance. No acceptance/history machinery was duplicated.

Final verification: **2,463 tests passed**, 7 skipped; **2 doctests passed**,
6 ignored. Full nextest run `9bb31d07-8432-4afa-a0bc-afa146ca0a09`; targeted run
`f933f35d-e371-4064-8c8d-0bb5d80999b2` passed all nine configuration regressions.
Strict all-target/all-feature lint, formatting and diff checks passed. Scoped
review: **Approve**, with no outstanding critical/high findings. Native input,
cross-platform behavior and performance were not newly measured.

Changelog, user semantics and active autocomplete checklist were updated. This
closes the menu-configuration gap, not the complete autocomplete plan. Source
changes remain with the uncommitted completion/editor foundations and require
dependency-ordered grouping; this is not a claim that the feature is committed.

## Isolated hover commit and process checks — 2026-09-07

Commit `e08ecb4` isolates popup hover from the broader dirty tree: popup-owned
pointer state replaces the completion-only field, shared row hit indices feed
the existing hover painter, and row transitions/window exit request repaint.
It includes state-transition and rendered-pixel regressions, its changelog entry
and the archived context-menu plan's follow-up. Completion filtering, documentation
scrolling, keymap, settings and edit-transaction changes are deliberately excluded.

Verification used a temporary detached checkout at `2bdd393` with exactly that
patch, sharing only the build cache with the main checkout. The isolated full
suite passed **2,132 tests**, 7 skipped (nextest run
`fadc609f-5306-4d68-89f9-294254c73917`). Its doctest target succeeded with all six
examples ignored; it does not contain the dirty tree's two compile-fail doctests.
Strict all-target/all-feature lint and formatting checks passed. The targeted
popup-hover test also passed independently. File hashes confirmed staging left
the working files unchanged. The temporary patch matched the committed diff
byte-for-byte before its checkout was removed; the commit preserves all of it.

Scoped code review: **Approve**, no outstanding findings. Checks covered hover
versus keyboard selection, separators, stable-row no-op, window-exit repaint,
popup lifetime and shared rendering indices. This proves the commit does not
depend on unrelated uncommitted source, not native pointer/IME acceptance.

The two carried ignored process-spawn tests were then explicitly run against
the **current main working tree**, serially, after the isolated build completed:

```sh
CARGO_BUILD_JOBS=1 just test-one "--run-ignored only --test-threads 1 -E 'test(spawn_server_completes_the_handshake_against_a_real_child) | test(process_exited_is_sent_after_shell_quits)'"
```

Both passed on macOS (run `83ab2760-a560-4246-9448-e6e85205ef50`), covering the
real-child LSP handshake and PTY shell-exit notification. This is a focused run,
not a fresh full working-tree suite or cross-platform/load-stability proof.
Their default ignore annotations remain: one serial pass does not establish
that the previously load-sensitive fixtures are safe for ordinary parallel CI.
No performance measurement, native window input or publication occurred.

## Completion documentation viewport — 2026-09-07

Completion documentation now uses an independently scrollable viewport instead
of discarding prose after twelve wrapped rows or allowing unbounded code height.
Code and prose share one row range, with a separator when both are present. The
collapsed card shows up to twelve rows; expansion uses the available window
height. The footer reports the visible range and accepts a click or F1 to toggle
expansion. Alt+PageUp/PageDown pages by the measured visible row count; ordinary
PageUp/PageDown still navigates completion items.

Painting, pointer hits and keyboard paging consume the shared measured overlay
layout. Wrapping uses the solver's anchor helpers to fit the larger side of the
menu without covering it. Cards disappear when no usable text area remains and
return after resizing. This changes completion side cards only, not standalone
hover or signature popups. No dependency, transport or configuration was added.

Wheel input rechecks current hit geometry so an unmoved pointer cannot scroll a
stale target after resizing or a new reply. Card interaction leaves buffer,
carets and list selection unchanged. Selecting another item resets the card;
no-op navigation and a late path result retaining the same server item preserve
its position. Focus/modal transitions dismiss the session through the existing
update lifecycle; delayed documentation actions cannot reopen it.

Ten regressions cover row reachability, long code followed by styled prose,
resize/DPI bounds, narrow-window separation, footer hits, scrolled text pixels,
keyboard routing, fresh wheel targets, selection/focus lifecycle and late path
arrival. The pixel test compares actual font-rendered buffers and checks that
all changed pixels stay inside the documentation card. It is not a native input
or screenshot acceptance test.

Final verification: **2,454 tests passed**, 7 skipped; **2 doctests passed**,
6 ignored. Nextest run `c56440ca-f578-4541-b91f-b03582ee29a9` used
`CARGO_BUILD_JOBS=1 just test '--no-fail-fast --status-level fail --final-status-level fail'`.
Strict all-target/all-feature `just lint`, `just fmt-check` and `git diff --check`
passed. The earlier targeted run passed all ten viewport tests; the full run
also includes the final stronger modal-dismissal assertions.

Scoped review: **Approve**, with no outstanding critical/high code findings.
Native keyboard, pointer and IME acceptance remain open; command-routing tests
do not establish end-to-end window input behavior. No new performance result is
claimed. This source remains coupled to the uncommitted completion/overlay
foundation and must be staged in dependency order, not swept into a broad commit.
The active autocomplete checklist records menu documentation richness complete,
but full snippets and other handoff requirements keep the whole plan active.

## Shared documentation Markdown — 2026-09-07

Completion, hover and signature cards now reduce Markdown through the existing
`pulldown-cmark` 0.12.2 dependency used by preview. A private event consumer emits
the existing `StyledText`; there is no new public API, renderer, dependency or
transport. The diagnostic-only backtick formatter stays separate because its
input is not Markdown. Four hand-written Markdown helpers were removed.

Formatting now preserves nested emphasis/code, matching fence types and lengths,
escaped punctuation and entities, reference-link labels, numbered/nested lists,
task markers, quotes, footnotes and separated table cells with emphasized headers.
Paragraph separation and code indentation are retained. Equal adjacent styles
coalesce, and span ranges remain ordered, non-overlapping and UTF-8 aligned.
This is native text reduction, not a full rich-document renderer: links are
labels, images are alt text, HTML is literal text and strikeout is dimmed.
There is no resource fetching or HTML execution. Plain-text protocol content and
completion acceptance/resolve lifecycles are unchanged.

| Severity | Finding addressed | Evidence |
| --- | --- | --- |
| Medium | Link stripping altered inline code examples before code parsing | Literal-link/entity code regression |
| Medium | Any three-backtick/tilde prefix toggled fence state, corrupting nested examples | Four-backtick and mismatched-tilde fence regression |
| Medium | Hand-written parsing lost reference links and nested block structure | Escapes/nested styles, lists/tasks/quotes, tables and footnote tests |

Verification: **14 converter tests passed**, including seven new regressions.
Full `CARGO_BUILD_JOBS=1 just test '--no-fail-fast --status-level fail --final-status-level fail'`:
**2,444 passed**, 7 skipped; **2 doctests passed**, 6 ignored. Nextest run
`8e1f074c-82d5-4749-80f3-c2ecd8b666d6`. Strict all-target/all-feature `just lint`
passed. The suite includes existing completion conversion/resolution, hover,
signature-help and styled-card layout/painting tests.

Initial test iterations exposed intended representation changes: normalized
list bullets; CommonMark setext headings distinguished from thematic dividers;
and fenced-code spans including their parser-provided line feed. Expectations
were updated explicitly, with separate heading/divider coverage. No production
behavior was weakened to preserve the old parser's incorrect interpretation.
Dependency API usage was checked against the locked crate source; the Context7
guide was supplementary, not authority for a newer API shape.

Scoped review: **Approve**. No critical/high findings remain for this converter.
No native-window check or new performance measurement is claimed. Long-document
scrolling/expansion and the rest of menu-panel richness remain unfinished, as do
the remaining autocomplete and broader handoff requirements. No whole feature
plan becomes ready for archival from this increment.

## Context-aware path completion — 2026-09-07

The dropdown now includes a filesystem source for path-shaped quoted strings,
plain text and Markdown destinations. Ctrl+Space additionally supports empty or
slash-free quoted/link prefixes. Relative paths use the current file's parent or
the workspace root; absolute and home-relative paths do not need a saved buffer.
Code literals wait for current ordinary-string highlighting; comments, regexes,
URLs and Markdown fragments/query strings do not trigger directory reads.

The implementation keeps feature boundaries explicit:

- `completion/path.rs` recognizes bounded contexts and formats insertions, without
  filesystem access. Requests snapshot document/pane, revision, language, path,
  workspace and every cursor; response ownership uses the request's `Arc` identity.
- `runtime/path_completion.rs` reads one directory on a speculative worker, with
  cancellation, 500-result / 1 MiB filename limits, a 20,000-entry scan limit and
  a cooperative 50 ms budget. Blocking OS calls may exceed the budget; neither
  typing nor shutdown joins them. Reads never enter the ordered save queue.
- `runtime/latest_worker.rs` shares one-running/one-pending replacement, shutdown,
  cancellation and panic-to-failure delivery with Find. The existing Find worker
  tests still exercise coalescing and nonjoining shutdown. This does not interrupt
  an already-running regex scan; that remains separate work.
- `update/completion/paths.rs` owns request/merge/acceptance lifecycle, reusing the
  existing menu, badges, filtering, LSP source and planned-edit/Undo transaction.
  No separate popup geometry, document clone, position mapper or public worker API.

Language-server items keep their priority and resolve semantics. A local-directory
failure leaves an LSP-capable session alive for import aliases. Both arrival
orders work; identical visible label/insertion pairs suppress the local duplicate.
Accented filesystem prefixes use exact prefix matching, avoiding fuzzy-normalizer
needle mismatches. Already-complete local filenames are omitted when accepting
would make no change. File/folder kinds also preserve the server's distinction.

Filesystem acceptance replaces the complete current component, including text
after the caret. A directory consumes any existing separator once and requests
its children. Compatible multi-cursor edits and peer-pane state use the shared
transaction. LSP path fallback uses the component range, while explicit UTF-16
edits retain their end range and additional edits. Deferred commit characters
remain visible, resolve safely and Undo once, including multi-cursor acceptance.

### Review findings addressed

| Severity | Finding | Verification |
| --- | --- | --- |
| High | A fast directory failure could suppress a later server import-alias result | `path_completion_failed_directory_keeps_late_server_alias_results` |
| High | Fresh syntax could withdraw an Enter acceptance waiting for LSP resolution | `path_completion_parse_cannot_withdraw_pending_server_acceptance` |
| High | Path lifecycle validation could cancel a staged commit character, or fallback could leave the filename suffix behind | Single/multi-cursor deferred-commit and UTF-16/import/Undo tests |
| Medium | An exact accented filename prefix could disappear in fuzzy normalization | Context/insertion, directory and dedup/filter regression tests |

Twenty-three targeted tests pass on macOS: context/source boundaries, root
resolution, Unicode/encoding, hidden names, symlinks, bounds/cancellation,
stale/reopened sessions, identity/focus/config guards, shared worker failure
recovery, server/local arrival races, acceptance/Undo and a headless runtime
directory → child-file → two Undo flow. The Linux-only non-UTF-8 filename test is
present but not exercised on macOS; this host rejects such names at creation.

Final verification: **2,437 tests passed**, 7 skipped; **2 doctests passed**,
6 ignored. Nextest run `15505665-158d-4f79-8334-2bbf246a1df2`, using
`CARGO_BUILD_JOBS=1 just test '--no-fail-fast --status-level fail --final-status-level fail'`.
Strict `CARGO_BUILD_JOBS=1 just lint` passed on the final implementation.
An initial final-gate run passed 2,436 tests but failed the existing independent
statistics-writer test when its 200 ms advisory-lock timeout expired under load.
That test incorrectly unwrapped an explicitly permitted `WouldBlock` result.
It now retries only this known-uncommitted result within a five-second writer
deadline, retaining the exact 32-successful-update assertion and failing on all
other errors. The targeted writer test passes. Production persistence, timeout,
failure notifications and no-replay behavior are unchanged; the separate busy-lock
test still verifies the error and recovery contract. This is not a guarantee that
all production statistics events persist under contention.

Native keyboard/pointer, IME, Windows/Linux and live server quality are not proven
by these fixtures. Supported forms and explicit limits are documented in
[File path suggestions](../user/config-editor.md#file-path-suggestions). Existing
benchmark snapshots precede this change; no new timing or speedup is claimed.

The path-source implementation does not complete the autocomplete plan: richer
documentation, full snippet placeholders, retrieval/edit prediction, provider
transports and native verification remain active. Retain `HANDOFF.md` and the
active plan; no additional feature plan is ready to archive.

## Commit-character acceptance — 2026-09-07

The metadata prerequisite below now has an input/acceptance implementation.
The client advertises `commitCharactersSupport`; full snippet support remains
unadvertised. Per the [LSP 3.17 completion contract](https://github.com/microsoft/language-server-protocol/blob/gh-pages/_specifications/lsp/3.17/language/completion.md),
declared commit characters accept the item and then enter the character; item
lists override server defaults. There is no guessed punctuation set for local
words or snippets and no resolve-time rewrite of primary insertion metadata.

`update/completion/commit.rs` owns this private lifecycle. Resolved items apply
the completion and character in the existing planned-edit transaction. Pending
items first insert the literal character and use the existing resolve worker.
Revision, document/pane identity, file path/language, cursors, collapsed selections,
active cursor, menu selection, history depth, configuration and focus guard the
reply. Further typing/navigation withdraws acceptance and preserves the literal
text. Empty replies, timeout and missing-server fallback retain initial imports.

A valid deferred accept retracts only the staged characters inside one update,
then applies the existing pristine-coordinate completion plan with the character
at the snippet caret. No intermediate frame or retraction worker effect is sent.
This deliberately reuses UTF-16 conversion, overlap handling and peer mapping
instead of cloning a document/history or implementing a second LSP range mapper.
The three history batches coalesce using the first before-state and final
after-state, preserving one-step Undo/Redo and exact peer selections. The existing
multi-cursor plain-text fallback still does not repeat absolute import edits.

Ordinary `InsertChar`/one-character keyboard events can commit. Paste/`InsertText`
messages and multi-character keyboard payloads cannot; the latter now enter as
one text transaction, not a loop of acceptance-capable keystrokes. Native IME
composition/candidate-window behavior remains a separate verification gate.
Signature/completion follow-ups are scheduled at the final accepted caret.

Fourteen added tests cover immediate/deferred acceptance, one-step Undo/Redo,
CRLF and astral Unicode, initial/resolved imports and suffix edits, snippet caret
placement, exact multi-cursor/peer history, selection/visibility/config/focus/file
guards, stale replies, additional typing, Copy/blink/repeat Enter, Enter-resolve
reuse, menu navigation, final trigger positions, keyboard payload routing and
runtime reply/timeout/missing-server paths. Fifteen targeted tests pass (including
one existing metadata test); full suite: **2,414 passed**, 7 skipped, plus
**2 doctests**, 6 ignored. Strict lint and final formatting passed.
Final logs: `/tmp/token-commit-character-final-{targeted,full,lint,fmt}.log`.
The first attempt
had two old private test calls missing the new optional argument; they were fixed
before the successful runs. No red/green behavioral claim is made from that run.

Scoped self-review found and corrected three ordering/lifecycle issues:

| Severity | Location | Finding and resolution |
| --- | --- | --- |
| High | `update/completion.rs` | Navigation must withdraw an earlier Enter accept; otherwise its late resolve accepts the old row. Clear that pending acceptance and test old/new reply ordering. |
| High | `update/completion/commit.rs` | Missing-server resolve can accept synchronously. Process the literal's effects before resolve so they cannot overwrite the accepted revision's syntax deadline; verify through the runtime. |
| Medium | `update/mod.rs` | An early return skipped shared status/wrap finalization. A new assertion reproduced `Ln 2, Col 3` after the caret reached column 8; keep the branch inside normal finalization and assert both accepted/pending status updates. |

Verdict: **Approve** for this acceptance increment after those corrections.
Rust and review guidance kept the lifecycle private and the transaction/resolve
paths shared; the protocol check verified the capability and acceptance order.
The finalization regression failed before the fix
(`/tmp/token-commit-character-finalization-before.log`) and passed in the final
targeted run; the full suite and strict lint were rerun after that correction.

No dependency, new provider transport, input queue, performance claim, native
window interaction, staging, commit or publication. The benchmark report remains
the earlier fingerprinted snapshot, not a measurement of these later changes.
Path completion, richer menu docs, full snippet placeholders, retrieval, edit
prediction and native/platform work remain; the autocomplete plan is not ready
for archival.

## Completion conversion preserves acceptance metadata — 2026-09-07

At this earlier checkpoint, commit-character input was not complete (implemented
in the acceptance checkpoint above). Its first
prerequisite exposed a real bug: `completion_item_to_menu_item` initialized
`additional_text_edits` to an empty vector regardless of the initial response.
Non-resolvable completions therefore inserted a name without the supplied import;
an empty resolve fallback also had nothing to retain. A new test reproduces the
missing import before the fix (`/tmp/token-completion-metadata-before.log`).

Conversion now moves initial additional edits into `LspInsert`, preserving their
order, UTF-16 ranges and literal text. The existing acceptance transaction and
resolve merger apply them without a new edit path. The real-update regression
passes for non-resolvable items and empty resolve replies, asserting final text,
caret, and one-step Undo. Multi-cursor acceptance still uses its existing
plain-text fallback, rather than repeating absolute edits at every cursor.

The public conversion entry now takes the server's optional `CompletionOptions`
snapshot instead of an extracted boolean. Resolve support and commit-character
inheritance are owned by this boundary. Per the
[LSP 3.17 completion contract](https://github.com/microsoft/language-server-protocol/blob/gh-pages/_specifications/lsp/3.17/language/completion.md),
an explicit per-item list wins over server defaults, including an empty list.
Single-scalar entries are sorted/deduplicated; empty/multi-scalar strings are
ignored. Inherited sets are shared across items and carried clones, and no
inherited/normalized fields are injected into the original resolve payload.
Runtime conversion, view fixtures and benchmarks consume the same boundary.
No additional runtime capability mirror, transport or dependency was added.

Five added tests cover the import regression, explicit/inherited/empty lists,
malformed entries, missing options, literal additional edit contents, unchanged
raw payloads and shared sets. Targeted tests passed; full suite **2,400 passed**,
7 skipped, plus **2 doctests**, 6 ignored. Strict lint and formatting passed.
Logs: `/tmp/token-completion-metadata-{targeted,after,full,lint,fmt}.log`.
No performance measurements or native input were performed for this increment.

Scoped review, excluding unrelated dirty-tree work:

| Severity | Location | Finding and resolution |
| --- | --- | --- |
| High | `completion/lsp.rs` | Initial additional edits were discarded; retain them and cover conversion → acceptance → Undo, including empty resolve fallback. |

Verdict: **Approve** for this conversion prerequisite. The Rust/review pass kept
capability interpretation in one boundary and checked raw-payload preservation.
Commit-character typing was still unimplemented and unadvertised at this
conversion checkpoint. The subsequent acceptance checkpoint above records the
character-preserving lifecycle, guards, transactional Undo and routing tests.
No further plan became ready for archival from the conversion prerequisite.

## Local inline completion outcomes — 2026-09-07

The remaining acceptance-statistics item is implemented through the existing
inline lifecycle and file queue, rather than a new worker or telemetry subsystem.
A crate-private observation tracks pending/offered/resolved state per response;
the only persistence message contains a configured provider name and an outcome.
The first explicit full/word/line acceptance wins over later dismissal or manual
typing. Cycling alternatives and accepting additional portions do not inflate
counts. Full manual typing-through has its own counter. Pending, failed, empty
and stale replies are excluded; undo does not reverse an acceptance. Attribution
uses the request-time name, even if configuration later chooses an equivalent
endpoint under another name. Opting out discards the current observation.

`completion.inline.statistics` defaults on, with a Settings control; inline
completion itself still defaults off. “Open Inline Completion Statistics”
dismisses first, then queues the JSON resource through the existing file opener.
This is a normal buffer snapshot, preserving existing unsaved edits on reuse,
not a live dashboard. Close/reopen to read newer counts. The
[user guide](../user/config-editor.md#local-completion-statistics) explains the
denominator, reset procedure, opt-out and loss-on-write-failure policy.

Persistence reads/merges under a stable `inline-statistics.lock` sidecar, using
[`File::try_lock`](https://doc.rust-lang.org/std/fs/struct.File.html#method.try_lock)
with a 200 ms retry window. An exclusively created sibling temporary file is
written and synced before [`rename`](https://doc.rust-lang.org/std/fs/fn.rename.html).
The old JSON is never truncated by this path. Temporary creation retries stop
after 16 collisions. Version/field validation, a 256 KiB file cap, 256-provider
cap and 256-byte UTF-8 name limit bound stored aggregates; counters saturate.
Malformed/newer-format files and user-created symlinks are left untouched.
These are local aggregate counts, not provider connection settings, source,
suggestions, paths or network telemetry. A failed write is reported non-modally
once until a subsequent success rearms notification, and is not replayed.
No power-loss durability, hostile-directory protection, controlled quality score,
or cross-platform native acceptance claim is made.

Twenty added tests cover pure and real-update lifecycle behavior, Unicode
type-through/backspace, partial/full acceptance, alternatives, navigation/edit
dismissal, original provider attribution, opt-out, error notification recovery,
config/action mapping, limits, malformed files, symlinks, independent concurrent
writers, lock contention and file-worker queue order/draining after receiver loss.
All persistence fixtures use temporary directories. Existing runtime fake-provider
fixtures explicitly disable collection, avoiding writes to normal statistics.
The full suite passed **2,395 tests**, 7 skipped, plus **2 doctests**, 6 ignored;
strict all-target/all-feature lint passed. Logs:
`/tmp/token-inline-statistics-{full,lint,fmt}.log`. The earlier full attempt was
interrupted during compilation to correct test isolation; it is not test evidence.

Scoped self-review findings, all corrected before the final full run:

| Severity | Location | Correction |
| --- | --- | --- |
| Medium | `update/inline.rs` | Discard the active observation on opt-out, preventing later opt-in from counting it. |
| Medium | `runtime/app_tests.rs` | Disable collection in fake-provider fixtures; persistence tests use temporary directories. |
| Low | `runtime/inline_statistics.rs` | Bound temporary-name collision retries. |

Verdict: **Approve** for this statistics increment, excluding unrelated dirty-tree
work. The Rust/review passes kept observation state private and reused the shared
file worker. No new dependency or performance claim; no additional plan is fully
implemented and ready for archival. The remaining autocomplete features and native
matrix are still active.

## Indexed forward-edit positions — 2026-09-07

The forward mapper no longer walks every edit for every cursor/selection
endpoint. `EditOffsetMap`, internal to the existing update module, counts edit
lengths and records final starts in ascending pristine order. Mapping selects
the last edit starting at/before an offset by binary search, then applies the
same relative-offset/clipping rule as the old sequential mapper. Equal-start
entries retain reverse application order, including replacement-before-insert
and several inserts at one point. Construction is linear in edit count plus
payload character counting; each old-offset lookup is logarithmic in edit count.

Ordinary final-caret placement, LSP completion placement and shared peer-pane
mapping use this index. Duplication obtains an offset inside its own inserted
copy directly from that copy's final start, replacing repeated suffix scans.
The old `offset_after_edits` helper is removed. Author-caret indexes are dropped
before the shared transaction creates a peer index, and a transaction with no
live positions or Find scope skips mapping entirely. Temporary storage scales
with edit count; no persistent document/history state or dependency was added.

Find's two scope endpoints retain actual sequential order: its start has left
affinity, including when a deletion brings it onto a subsequent insertion.
History also retains its actual-order mapper for new panes lacking snapshots;
this is not a claim that arbitrary history sequences are indexed. Snapshot
capture, buffer mutation order, effects and restoration policies are unchanged.

The exhaustive Rust oracle checks **42,601** valid batches of up to three edits,
with starts 0–5, removed lengths 0–2, inserted lengths 0–3 and Unicode payloads.
It compares **426,010** old-offset mappings and every in-insertion caret offset
with sequential application. A second regression compares all ordered scopes
with endpoints 0–9 across zero/one/two panes, touching replacement/deletion/
insertion chains, equal-point inserts, empty and no-op batches. Existing ordinary
editing, completion, Find replacement and lossless pane-history tests also pass.

Verification: **2,375 passed**, 7 skipped, plus **2 doctests**, 6 ignored
(`/tmp/token-forward-map-full.log`); strict lint passed
(`/tmp/token-forward-map-lint.log`). The targeted seven-test run also passed
(`/tmp/token-forward-map-targeted.log`). Scoped self-review checked descending
non-overlap preconditions at all planners, equal-point ownership, Find affinity,
empty-batch navigation state, snapshot/effect order and temporary allocation
lifetime. No outstanding findings; verdict: **Approve** for this refactor, not
the entire dirty worktree.

### Controlled release comparison

The unchanged `just profile-workloads edit-history` fixture ran before and after
the refactor on Apple M2 Max / 32 GiB, rustc 1.98.0, optimized bench profile.
Both runs completed all **54** reports and every text/pane/history assertion.
There are 10 warmups and 100 samples per case; setup and returned runtime effects
are excluded. These are the same unsaved-scratch CPU stages described below,
not loaded-file saved-content checks, rendering, end-to-end latency or FPS.

At **1,000 cursors**, times in milliseconds:

| Operation | Panes | Before median | After median | After p95 | Median ratio |
| --- | ---: | ---: | ---: | ---: | ---: |
| Delete | 1 | 3.723 | 0.961 | 1.006 | 3.87× |
| Delete | 2 | 6.986 | 2.123 | 2.236 | 3.29× |
| Duplicate selection | 1 | 2.781 | 1.228 | 1.333 | 2.26× |
| Duplicate selection | 2 | 6.076 | 2.508 | 2.582 | 2.42× |
| Duplicate lines | 1 | 3.176 | 1.181 | 1.266 | 2.69× |
| Duplicate lines | 2 | 6.573 | 2.479 | 2.601 | 2.65× |

At 100 cursors, edit medians improved by roughly 4–8%. At one cursor, they were
unchanged or increased by 0.083–0.167 µs in this run: the temporary index is not
free, and this is not a universal speedup claim. No Undo/Redo optimization was
made here; that already-filtered history path remains unchanged, with observed
1,000-cursor medians of 0.138–0.181 ms. Forward updates still cost 0.96–2.51 ms at
1,000 cursors after removing the repeated mapping scans. Attribute that residual
cost only after further stage/stack profiling, not from these aggregate timings.

Raw results: `/tmp/token-forward-map-baseline.log` and
`/tmp/token-forward-map-optimized.log`. Release builds took 6m 02s and 5m 54s;
both emitted the existing release-only unused `revision` warning in
`src/update/syntax.rs`. No native application or user buffer/configuration was
changed during this checkpoint. About 20 GiB remain free; no cache was deleted.

## Markdown diagrams and popup hover — 2026-09-07

Markdown previously sent every fence through code highlighting, with no diagram
renderer. The preview now passes escaped `language-mermaid` fence `textContent`
to pinned Mermaid 11.17.2, loaded only when needed from jsDelivr. It uses strict
security mode, theme-derived colors and the supported asynchronous
[`initialize`/`run({ nodes })` API](https://mermaid.js.org/config/usage.html).
Success replaces only the source block; source line markers remain. Invalid
syntax or renderer loading failure leaves the source and an explanatory status
message, and one invalid diagram does not prevent later diagrams. Ordinary code
highlighting and raw HTML preview remain separate. CDN access is required;
this is not bundled offline support.

Browser checks passed for flowchart, sequence and state diagrams in dark/light
themes, invalid-then-valid fences, ordinary Rust highlighting, and offline
fallback (all three sources retained). A standalone macOS WKWebView probe using
the same `token://preview-1/index.html` custom scheme rendered all three SVGs
without errors (`/tmp/token-preview-hover.JiI6EZ/webkit.log`). This is an engine
integration check, not a full native editor interaction test.

The reported state-transition label collision reproduces with Mermaid 11.17.2
on a blank page without editor CSS. Increasing
[`state.rankSpacing`](https://mermaid.js.org/config/schema-docs/config-defs-state-diagram-config.html)
to 120 in the sample's own configuration separates the labels (measured vertical
boxes no longer intersect). Spacing is not forced globally, nor is this a general
fix for upstream graph layout. Dark-theme borders and sequence lifelines now
use readable theme colors.

Context-menu hover had two missing connections: runtime row tracking handled
completion only, and non-modal idle pointer changes did not request redraw.
`CursorOverlayState` now owns `hover_row`, replacing the completion-specific UI
field. Context menus, completion, code actions and references use that one state
and the existing shared hit-test indices/hover painter. Row transitions and
window exit request repaint without changing keyboard selection; separators
clear hover. Regression coverage checks state transitions, stable-row no-op,
leave/reset, selection independence and actual rendered pixels for the hovered
row, selected row and separator. A guarded native pointer attempt sent no events
because Chrome obscured the fixture window; do not count it as native acceptance.

Final suite: **2,373 passed**, 7 skipped, plus **2 doctests**, 6 ignored
(`/tmp/token-preview-hover-final-reviewed-full.log`); strict lint passed
(`/tmp/token-preview-hover-final-reviewed-lint.log`). Formatting and JavaScript
syntax checks passed. Scoped self-review covered escaped-source handling,
strict renderer configuration, independent failures, shared hit indices and
hover-vs-keyboard state. No outstanding code findings; verdict: **Approve** for
these changes, with native pointer/platform acceptance still explicitly limited.

The isolated editor window (PID 3636, socket
`/tmp/token-preview-hover.JiI6EZ/editor.sock`) is left open to preserve typing
that reached its scratch `state` tab. No user input was discarded. An incorrect
initial automation CLI spelling opened two scratch tabs in that isolated editor;
subsequent checks used `token automate`. No normal editor configuration changed.

## High-cursor edit/history profiling — 2026-09-07

The new `just profile-workloads edit-history` probe measures deletion, selection
duplication and line duplication, then Undo and Redo separately. It uses the real
update path with 1, 100 and 1,000 cursors in one/two panes, 10 warmups and 100
samples per case. Fixture reset, assertions and returned effects/rendering are
excluded. Text, cursor/selection state, active index and history stack transitions
are asserted after every stage. These are unsaved scratch buffers: restoring a
loaded file's saved-content equality is not part of this fixture.

The baseline exposed redundant work: history mapped every saved pane through
every atomic edit, immediately replacing the result with its exact snapshot.
`EditPositions::capture` now takes a pane predicate shared by ordinary edits and
history. Undo/Redo excludes panes with saved snapshots, while newly opened panes
retain mapping and selection-only Find scopes are captured independently. A
regression covers Find-scope mapping when all panes are excluded.

M2 Max / 32 GiB, rustc 1.98.0, optimized profile, 1,000 cursors; medians in µs:

| Operation | Panes | Edit before → after | Undo before → after | Redo before → after |
| --- | ---: | ---: | ---: | ---: |
| Delete | 1 | 3,663.833 → 3,755.208 | 3,250.833 → 140.500 | 3,232.958 → 132.625 |
| Delete | 2 | 6,865.625 → 6,989.459 | 6,357.917 → 142.750 | 6,363.125 → 131.500 |
| Duplicate selection | 1 | 2,619.625 → 2,660.958 | 3,442.833 → 131.875 | 3,426.542 → 135.000 |
| Duplicate selection | 2 | 6,027.125 → 6,118.292 | 6,738.708 → 137.792 | 6,733.666 → 138.750 |
| Duplicate lines | 1 | 3,170.833 → 3,180.416 | 3,515.791 → 155.125 | 3,496.667 → 153.625 |
| Duplicate lines | 2 | 6,489.917 → 6,632.708 | 6,830.834 → 161.250 | 6,760.709 → 155.500 |

Optimized history p95 across these cases is 147–193 µs. Forward edit cost is
essentially unchanged at 2.66–6.99 ms; the general per-edit/per-position mapping
cost remains. These are stage measurements, not FPS, network latency, heap use,
or a universal speedup. Before/after logs with all 54 results are
`/tmp/token-edit-history-baseline-fixed.log` and
`/tmp/token-edit-history-optimized.log`; both completed successfully with all
fixture assertions. The earlier `baseline.log` was a harness compile failure,
not a usable measurement. The optimized executable was built before the later
preview/hover changes.

Verification before those UI changes: **2,370 passed**, 7 skipped, plus **2
doctests**, 6 ignored (`/tmp/token-edit-history-final-full.log`), and strict lint
passed (`/tmp/token-edit-history-final-lint.log`). Scoped self-review checked
saved-pane exclusion, new/closed pane behavior, atomic compatibility and Find
scope capture; no outstanding findings. Verdict: **Approve** for this checkpoint,
not the whole dirty worktree. No new public profiling API, dependency or history
representation was introduced.

## Lossless per-pane undo selections — 2026-09-07

The remaining per-pane history gap is fixed. Previously a batch retained only
one pane's cursor vectors; Undo copied those into the currently focused pane and
collapsed its selections. Peer endpoints were mapped through inverse mutations,
which cannot recover positions clipped inside a deletion. Three new integration
regressions reproduced these failures before implementation
(`/tmp/token-undo-pane-before.log`).

`EditOperation::Batch` now owns before/after `EditorEditState` snapshots keyed by
editor ID. Each contains cursors (including desired columns), directional
selections and active cursor index. `apply_planned_edits` captures all panes
showing the document around mutation and final caret placement/deduplication.
Typing's overlapping-selection normalization explicitly supplies the original
state to that same transaction; otherwise the record would already have lost
the original cursor order and selections. This replaces the old cursor-only
batch fields rather than adding parallel history stacks.

Undo/Redo first applies the buffer operations and maps live positions, then
restores each surviving recorded editor's own snapshot. Current focus does not
choose the snapshot owner. New panes have no saved state and retain their mapped
live positions; closed panes are not recreated. Restoration checks document ID,
clears derived occurrence/selection-expansion state, and does not restore layout,
viewport, speculative text or pointer gestures. Existing panes intentionally
return to the recorded before/after selections even if their carets moved later.
Legacy/synthetic atomic records keep their original single-cursor behavior.

Five tests in `tests/undo_pane_state.rs` cover Unicode, CRLF, clipped peer ranges,
reversed/overlapping selections, active-index and desired-column restoration,
focus changes, repeated Undo/Redo, nonempty redo selections, a new edit branch,
pre-normalization capture, and panes created/closed after an edit. The final full
suite passed **2,369 tests**, 7 skipped, plus **2 doctests**, 6 ignored
(`/tmp/token-undo-pane-full.log`). Strict all-target/all-feature lint passed
(`/tmp/token-undo-pane-lint.log`). The earlier targeted pass contains only the
initial three tests; the full log covers all five.

The memory tradeoff is explicit: history now retains two selection-state vectors
per pane as well as cursor vectors and bounded per-pane metadata. Cost scales
with the number of retained edits and saved cursors across panes. No document
rope, renderer/cache state or whole editor is cloned into these snapshots, and
no persistent history format or dependency was introduced. High-cursor mapping
optimization and dedicated deletion/duplication profiling remain separate work.

The unchanged production insertion probe was rerun with
`CARGO_BUILD_JOBS=1 just profile-workloads insertions` on M2 Max / 32 GiB,
rustc 1.98.0, optimized profile. Each case has 10 warmups and 500 samples;
fixture reset and assertions are outside timing. It inserts one Unicode
character per cursor through the real update path, with completion/LSP/bracket
matching disabled. Returned runtime effects and rendering are not executed.

| Cursors | One pane median / p95 (µs) | Two panes median / p95 (µs) |
| --- | ---: | ---: |
| 1 | 2.708 / 2.834 | 3.875 / 4.209 |
| 10 | 18.042 / 18.750 | 34.792 / 36.000 |
| 100 | 373.208 / 393.667 | 838.042 / 892.000 |
| 1,000 | 3,668.834 / 3,808.208 | 6,539.709 / 6,873.542 |

These current-state measurements include snapshot capture. They are not a
controlled before/after attribution of its overhead, an undo traversal benchmark,
a heap profile or end-to-end latency. High cursor counts still carry the known
per-edit/per-position cost. All fixture assertions passed; raw output is
`/tmp/token-undo-pane-profile.log`. The build took 6m 03s and emitted only the
existing release-only unused `revision` warning in `src/update/syntax.rs`.

Scoped code-review verdict: **Approve**. The review specifically checked capture
before normalization, snapshot/document identity, restoration after focus or pane
lifecycle changes, redo branching and the absence of unconditional selection
collapse. No outstanding findings in this change; this is not approval of the
entire dirty worktree. The undo contract and Unreleased changelog are updated.

## Ghost native checks and release profiling — 2026-09-07

### Isolated macOS checks

A debug editor instance used an isolated configuration, an unsaved scratch Rust
buffer and a loopback-only fixture server returning two known insertions. LSP
was disabled; buffer-word completion was enabled to exercise interference with
inline type-through. No hosted model, credentials or normal user configuration
were used. This verifies editor interaction, not generated-code quality.

The source fixture contained eight logical lines, with the cursor at line 1,
column 4 (zero-based), before `println!("Existing suffix: {name}");`. The first
candidate was `let greeting = format!("Hello, {name}");` followed by a newline,
an indented `println!("{greeting}");`, another newline and indentation. Its tab
was normalized to spaces by the existing provider pipeline.

- The preview occupied ten visual rows without modifying the eight-line source.
  Native Option+9 selected the second alternative on the current keyboard layout;
  Tab accepted it ahead of the suffix. One Command+Z restored the exact source.
- Native pointer events on the second ghost row mapped to the insertion anchor
  (1:4); a click on the shifted suffix mapped to its original source column
  (1:8). Both dismissed the preview without modifying the document. The helper
  checked that the target point belonged to the isolated editor window before
  each event and restored the pointer afterward. Earlier PID-targeted mouse
  attempts did not deliver effective clicks and are not passing evidence.
- Resizing that window from 800×600 to 360×322 content pixels with soft wrap
  enabled yielded twelve visual rows. Inspected window captures showed the
  wrapped preview and shifted/wrapped suffix, with the caret before the ghost.
- Typing native `l`, `e`, `t` consumed the matching prefix, preserved the remaining
  projection and did not open the automatic word dropdown. Named
  `AcceptInlineLine` accepted one line; native Down dismissed the remainder and
  navigated the actual wrapped source. Undo restored the exact fixture.
- IME composition is **not verified**. Option+e inserted `é` directly on this
  layout, so that was not a dead-key test. A further composition attempt did not
  establish a composed result or a candidate window. Do not count either as an
  IME pass. No system input-source change was made.

Artifacts are temporarily under `/tmp/token-ghost-native.E0ElXY/`: `visible.png`,
`narrow.png`, `cycled.json`, `accepted.json`, `ghost-click-verified.json`,
`suffix-click-verified.json`, `typed.json`, `partial.json` and `down.json`.
`final-restored.json` confirms exact original text and `modified: false` before
quitting the isolated editor. Its fixture server was stopped; artifacts remain.
The full native language/platform matrix remains open.

### Release-stage measurements

`CARGO_BUILD_JOBS=1 just bench-completion ghost` on Apple M2 Max / 32 GiB,
rustc 1.98.0 (88d9e12ae, 2026-08-18), optimized bench profile. Each case uses
100 samples and the existing Divan allocation profiler. Fixture construction
is excluded; no provider request, parser, user-config I/O or window presentation
is timed. These are current-state CPU measurements, not a before/after speedup.

| Stage | Fixture | Median |
| --- | --- | ---: |
| Arrival / projection | 100 / 10,000 source lines | 5.275 / 6.733 µs |
| Arrival / long anchor | 80 / 4,096 / 65,536 characters | 5.316 / 23.16 / 265.7 µs |
| Cycle alternatives | 100 / 10,000 source lines | 4.463 / 4.589 µs |
| Blink update, no rendering | 100 / 10,000 source lines | 0.892 / 0.985 µs |
| Matching type-through | 80 / 4,096 / 65,536 anchor characters | 12.70 / 35.89 / 414.2 µs |
| Width reflow, 80 → 40 columns | 100 / 10,000 source lines | 33.61 µs / 3.328 ms |
| Warm CPU frame, plain source | 100 / 10,000 source lines | 289.8 / 306.0 µs |
| Warm CPU frame, visible ghost | 100 / 10,000 source lines | 269.7 / 279.9 µs |

The insertion has eight newlines and two alternatives. Arrival/cycling use the
production update path; type-through also includes the ordinary edit transaction,
source wrap refresh and remainder projection, but returned runtime effects are
not executed. Width changes rewrap the whole source document, explaining the
largest measured stage (3.328 ms median, 3.714 ms maximum here). Ordinary arrival
and cycling remain nearly flat over the two tested document sizes. Long anchors
scale with their content; these finite fixtures do not establish a worst-case
bound or justify truncating suggestions.

Rendering uses the shared 1100×720 CPU renderer. Setup warms metrics/glyphs and
asserts the anchor is visible at row three, the following source line is visible,
and all projected rows survived font/width refresh. An initial exploratory run
placed the ghost offscreen after resizing; its render results must not be used.
Plain and ghost frames contain different glyphs and lengths: the lower ghost
number does **not** show a rendering speedup or measure incremental overhead.
No new public profiling API, dependency, one-off timer or runtime optimization
was added on the strength of these measurements.

Final reviewed output: `/tmp/token-ghost-profile-reviewed.log`. Earlier
`baseline`, `visible` and `final` logs predate the last fixture assertions.
Full verification: **2,364 tests passed**, 7 skipped, plus **2 doctests**, 6
ignored (`/tmp/token-ghost-profile-tests.log`); strict all-target/all-feature
lint passed (`/tmp/token-ghost-profile-lint.log`). The existing release-only
unused `revision` warning in `src/update/syntax.rs` is unchanged.

Scoped self-review covered benchmark mutation/reset boundaries, excluded effects,
on-screen assertions and documentation claims. No outstanding findings in this
benchmark/documentation change; verdict: **Approve**. This does not close the
unverified IME gate or approve every unrelated dirty-tree change.

Archival recheck: Soft Wrap, Damage Tracking, Command Palette and Settings v1
remain archived with active follow-ups. No additional active plan is complete
enough to archive. Autocomplete and the temporary handoff remain active.

## Implementation follow-up

The subsequent goal continuation fixed the two cursor defects in finding 1.
Word and LSP completion now use `apply_planned_edits`, with feature-owned final
caret offsets. `EditPositions` maps cursors and both selection endpoints for
planned edits, inline acceptance, and undo/redo. It replaces inline acceptance's
selection-restoration loop and consolidates undo/redo buffer traversal. Tests
cover the original repros plus Unicode, overlaps, reversed selections, replacement
clipping and snippet/boundary-insert behavior. The original observations below
are retained as historical evidence, not claims that those defects remain open.

The broader edit-system migration is not complete: ordinary forward document
commands and Find replacement now use the shared transaction (follow-ups below).
Undo records still store caret vectors,
not full per-pane selection/active-index snapshots; peer positions inside deleted
text follow a clipping policy, not lossless restoration. Findings 2–8 remain
open except findings 3–8, addressed below; finding 2 now has cached results and
background large-file display scans, with explicit cold actions still synchronous.
The remaining scan cost is measured below. Finding 7
now has document-targeted save/reload replies, asynchronous ordinary new-tab and
configuration-resource preparation, and shared boundary-resolved file identity.
Native validation and optimized profiling limits are recorded below. No new
performance improvement is claimed for the edit consolidation.

Follow-up verification: `just test` passed **2,136 tests**, with 7 skipped and
6 ignored doctests; `just fmt`, `just lint`, `just fmt-check` and
`git diff --check` passed. Scoped diff-based self-review: **Approve** for this
edit-position change, not completion of the remaining audit/feature roadmap.

### Multi-row and mid-line ghost projection — 2026-09-07

The first-line-only overlay has been replaced by a per-pane insertion projection
in `model/ghost_text.rs`. It reflows only the anchor logical line using the
existing wrap segmentation and the document's Rope line definition. The shared
`TextViewportMap` adds its row displacement to the ordinary document map. Cursor
and IME positions stay before the insertion; suffix glyphs are shifted after it.
Ghost hit tests map to the source anchor; following rows retain source identity.
No speculative text enters the document, syntax tree or undo history.

`VisibleTextLine` exposes up to two source fragments, so syntax, selections,
brackets and range decorations cannot accidentally include a same-row ghost.
Glyph painting leaves blank cells for the ghost pass instead of overdrawing
bright glyphs, avoiding antialiased fringes. Changed projections redraw the
editor area; unchanged projections use the existing cursor-line blink path.
The obsolete first-line/badge helper was removed rather than retained as a
second presentation API.

The update lifecycle reuses immutable geometry for scroll/blink, reprojects
after type-through, acceptance and cycling, and clears peers on focus changes.
Navigation dismisses before source movement; rectangle selection converts its
display anchor before dismissal. Direct runtime font/gutter refreshes also
reflow a current projection. Source Rope identity protects against replacement
buffers with equal revision numbers. Overview caches invalidate with projection
changes. Automatic suffix gating remains policy; explicit requests work mid-line.

A new regression exposed automatic dropdown collection racing type-through in
the same deterministic update: the current word's suffix could produce a menu
before the inline remainder consumed its matching character. Consumption now
runs first, and a still-visible remainder suppresses automatic menu collection.
Explicit menu requests and unrelated typing retain their existing paths.

Verification includes a real-insertion geometry oracle over source positions,
wrap widths, tabs, Unicode, CRLF, lone CR, Unicode separators and blank lines;
source-fragment, scroll/replacement, lifecycle and pixel-equivalent blink tests.
The headless `inline-multiline` scenario was generated and visually inspected at
`/tmp/token-ghost-screenshots/screenshot-inline-multiline.png`. The screenshot
fixture now goes through the production resize/update lifecycle.

Final verification: **2,364 tests passed**, 7 skipped, plus **2 doctests**, 6
ignored, in `/tmp/token-ghost-full-reflow.log`. Strict all-target/all-feature
lint passed in `/tmp/token-ghost-lint-reflow.log`. Seven tests were added; the
final run supersedes earlier type-through and stale-state-expectation failures
and an intermediate build with outdated test callers of `cursor_visual_line`.

Scoped diff-based self-review:

| Severity | Finding | Resolution |
|----------|---------|------------|
| High | Automatic dropdown collection could hide a compatible mid-line remainder before consumption | Reordered edit reconciliation; type-through/accept/Undo regression passes |
| High | Direct runtime font/gutter refreshes could drop a current projection outside `update()` | Reflowed from current source identity in the shared cache refresh; direct-refresh regression passes |
| Medium | Painting source glyphs underneath ghost glyphs would brighten antialiased edges | Source pass leaves ghost cells blank; ghost glyphs are painted once |

Verdict: **Approve** for this implementation scope. Native
interaction/IME checks and release-stage projection profiling remain next;
headless rendering is not native GUI validation or evidence of model quality.
All other handoff scope remains active; this does not complete Phase 5+.

### Recency release profiling and token-index consolidation — 2026-09-07

`just bench-completion recency` now measures the actual private runtime ring,
compiled into the existing completion benchmark via a source-module reference.
No public profiling API, separate algorithm, dependency or additional benchmark
recipe was introduced; the existing recipe now forwards optional Divan filters.
`with_inputs`/`bench_local_refs` exclude fixture generation and call each generated
fixture once, verified against the installed Divan 0.1.21 source. Headless fixtures
use default model construction without reading user config. Network, model
inference, window presentation and setup are outside these measurements.

Environment: Apple M2 Max, 32 GiB RAM, rustc 1.98.0, repository release/bench
profile (`opt-level = 3`, thin LTO), 100 samples per case, existing Divan Rust
allocation instrumentation. Buffers contain distinct identifier-heavy 8 KiB
snippets, with 8 or 32 open buffers and matching ring limits. Refresh queues saves
for every buffer after filling the ring; setup asserts the expected retained
chunk count and source byte total. Attachment starts with an unallocated context
vector, matching a new production request. The first exploratory log predates
that vector correction; use the final baseline below for comparisons.

| Stage / snippets | Baseline median | Indexed median |
| --- | --- | --- |
| First idle fill / 8 | 1.244 ms | 434.7 µs |
| First idle fill / 32 | 19.82 ms | 1.813 ms |
| Full idle refresh / 8 | 2.326 ms | 442.1 µs |
| Full idle refresh / 32 | 38.77 ms | 1.900 ms |
| Stable observation / 8 | 231.4 ns | 249.7 ns |
| Stable observation / 32 | 1.030 µs | 1.062 µs |
| Request attachment / 8 | 2.058 µs | 2.048 µs |
| Request attachment / 32 | 7.244 µs | 7.495 µs |

The measured hotspot was repeated construction of both `BTreeSet`s for every
snippet pair. A 32-snippet refresh reported 121,120 allocations plus 15,872 growth
operations. `IndexedChunk` now retains sorted/deduplicated byte ranges into its
own immutable text; a merge comparison counts exact intersections and rejects
only when the maximum possible remaining intersection cannot exceed the strict
threshold. No hashes, case folding, approximate matching or changed ranking.
The same refresh reports 128 allocations, 256 growth operations and 32 shrinks.
This is approximately **20× faster for that idle-refresh fixture**, not an
application-wide speedup; ordinary observation and attachment are essentially
unchanged at this measurement resolution.

A second corpus shares about 94% of token identities, placing shared tokens first
in sort order. Jaccard similarity remains just below 90%, so all chunks survive
and comparisons visit most common tokens before rejecting. Indexed refresh
medians are **580.1 µs / 4.655 ms** for 8 / 32 chunks (32-chunk maximum observed
sample: 6.430 ms). There is no baseline speedup claim for this added corpus.
Neither corpus proves a universal worst-case time or end-to-end responsiveness.

Tradeoff: retained range storage can occupy up to 2 MiB at 32 snippets on a
64-bit build, plus bounded transient capture scratch, source text and labels.
The bound follows from at most 4,096 nonempty separated tokens per 8 KiB snippet,
with 16-byte ranges. Indexes remain runtime-local and disappear with their
snippets; only `ContextChunk` payloads enter HTTP requests or the existing cache.
Allocation-profiler “max alloc” rows are not total process memory. No new native
GUI, hosted model, server KV-cache or frame-rate result is claimed. The unrelated
release-only unused `revision` warning in `src/update/syntax.rs` remains unchanged.

Final evidence:

- Baseline: `/tmp/token-recency-profile-baseline-final.log`.
- Indexed, including near-duplicate corpus: `/tmp/token-recency-profile-indexed-final.log`.
- Full suite: **2,357 passed**, 7 skipped; **2 doctests**, 6 ignored
  (`/tmp/token-recency-index-full.log`). One new differential test compares 1,000
  generated Unicode pairs, symmetry and empty/punctuation cases against the old
  token-set oracle; strict-threshold and lifecycle tests also pass.
- Final strict lint: `/tmp/token-recency-profile-final-lint.log`.
- Formatting/whitespace checks: `/tmp/token-recency-profile-fmt-check.log`.

Scoped review: **Approve**. The measured performance issue is addressed; fixture
review also removed accidental setup-time preallocation from attachment timing.
The full handoff remains open. Next is multi-row/mid-line ghost text through
`EditorState::viewport_map()` / `TextViewportMap`, shared with caret placement,
hit testing, IME and rendering. The current `render_ghost_text_stage` only paints
a first line and continuation badge; painting additional independent rows would
not satisfy the required shared geometry. No ghost-row implementation was made
in this profiling pass.

### Idle recency context — 2026-09-07

The existing request/worker/cache path now carries provider-neutral
`ContextChunk` values. `completion/recency.rs` owns opt-in configuration, bounds
and comment fallback; private `runtime/inline_context.rs` owns observation,
pending positions, idle deadlines and the ring. No parallel completion worker,
filesystem scan, retrieval index or persistence path was added. Configuration
defaults to `context: { strategy: none }`; enabling recency permits transmission
of other open buffers' snippets, including unsaved text. Workspace-root checks
use boundary-resolved identities, not querying syscalls or lexical symlink paths.
Outside a workspace, the scope is the current window's open text buffers.

Activation, file switches, successful save replies and jumps of at least
`chunk_lines` queue positions. Cursor/revision changes postpone the 750 ms idle
deadline, which participates in `App::next_wake`; syntax replies and blinking
do not postpone it. Idle copies at most 8 KiB per snippet and 1 KiB per filename,
with both pending and committed counts bounded by `max_chunks` (default 8,
maximum 32). Token-set Jaccard similarity strictly above 0.9 evicts the older
chunk; there is no per-keystroke ranking. Committed snapshots stay stable during
typing. Close/path invalidation removes old ring entries, and provider/workspace
changes clear the ring. Already-sent requests cannot be retracted from a backend;
the worker's bounded cache may retain prior payloads until eviction.

llama.cpp's documented `input_extra` array has `{filename, text}` objects and is
placed before the FIM prefix. Verified through Context7 and the
[primary server documentation](https://github.com/ggml-org/llama.cpp/blob/master/tools/server/README.md#post-infill-for-code-infilling).
Other transports prepend comment-formatted snippets to their HTTP/FIM prefix,
without rewriting the request's active-buffer prefix or its cursor bookkeeping.
Unsupported comment languages fail explicitly; JSON/HTML/CSS are not silently
treated as C-family source. Raw FIM token checks include extra text. Context order
and contents participate in exact/partial cache equality, and retained capacities
count toward the existing 8 MiB payload bound. Chunk Debug shows sizes, not text.

Scoped review corrected physical-line comment boundaries (CR-only, Unicode line
separators and C-family backslash continuation), avoided duplicate raw-prefix
formatting, and checked the newly retained cache payload accounting. Revisiting
an overlapping region now replaces its old snapshot even when the new text falls
below the token-similarity threshold or the region is empty; this avoids retaining
contradictory old code after a substantial rewrite. Regression
coverage includes defaults/bounds, strict similarity, pending limits, Unicode,
idle postponement, jumps/switches/saves, close/provider invalidation, resolved-path
workspace exclusion, native/raw wire shapes across all four transports, changed
partial replay and runtime worker arrival/acceptance/Undo. No native GUI or live
model/provider quality check was performed in this pass; no release timing or
KV-cache speedup is claimed. Existing profiling checkpoints below are historical,
not measurements of this new ring's capture/deduplication cost.

Final verification: **2,356 tests passed**, 7 skipped, plus **2 doctests**,
6 ignored (`/tmp/token-recency-full-ranges.log`); strict lint passed
(`/tmp/token-recency-lint-ranges.log`). Ten tests were added since the raw-FIM
checkpoint, with existing worker/cache coverage extended. The earlier full-suite
logs predate the final overlap-range regression and are not the final evidence.
Formatting, whitespace and updated guide/plan local-link checks passed.

| Scoped review concern | Resolution |
| --- | --- |
| Medium: comment boundaries could consume or expose following text | Physical separators are commented; an empty separator terminates backslash continuation. Regression-tested. |
| Medium: rewritten regions could retain contradictory old snapshots | Overlapping captured ranges replace older text, including empty replacements and clamped first-line ranges. Regression-tested. |
| Medium: extra context needs cache accounting and replay guards | Capacities count toward 8 MiB; changed/reordered snippets reject partial replay. Regression-tested. |

Scoped review verdict: **Approve** for this implementation, not completion of the
remaining feature plan or the full handoff. `HANDOFF.md` remains temporary and
active; Phase 5+, profiling and the broader verification matrix remain open.

### Explicit raw FIM formats — 2026-09-07

`completion/prompt.rs` provides one public `PromptFormat` enum and a private
template table. The existing `FimProvider` resolves it before credentials/network
work. `native` is the backward-compatible default, rather than applying the old
plan's unconditional `Infer` default to already-working native-suffix backends.
Explicit raw formats and `infer` are supported on Ollama and OpenAI-compatible
completions only. llama.cpp `/infill` and Mistral FIM retain server-side formatting
and reject raw mode rather than receiving a doubly formatted prompt.

The template table owns marker order and up to four transport stop strings.
Response cleanup derives its vocabulary from that table, retaining the previous
sentinels and adding missing DeepSeek Unicode/EOS, filename and padding tokens.
Ollama raw requests use `raw: true`, per the
[Ollama API](https://github.com/ollama/ollama/blob/main/docs/api.md), with stops in
`options`; OpenAI-compatible stops are top-level. Neither raw shape sends a native
`suffix` field. Other HTTP/authentication/cancellation/alternative handling is
unchanged. No new provider, worker, client dependency or persistent model storage
was added. Provider configuration equality already includes the new field, with
an explicit cache-invalidation regression.

Checked format authorities:

| Family | Authority and layout |
| --- | --- |
| Qwen | [Qwen2.5-Coder tokenizer](https://huggingface.co/Qwen/Qwen2.5-Coder-7B/resolve/main/tokenizer_config.json): pipe-delimited FIM markers, prefix/suffix/middle |
| StarCoder | [BigCode model card](https://huggingface.co/bigcode/starcoder#fill-in-the-middle): prefix/suffix/middle |
| CodeLlama | [Meta infilling implementation](https://github.com/meta-llama/codellama/blob/main/llama/generation.py) and [tokenizer](https://github.com/meta-llama/codellama/blob/main/llama/tokenizer.py): prefix-first, including marker word-boundary spaces |
| DeepSeek | [Publisher code-insertion example](https://huggingface.co/deepseek-ai/deepseek-coder-33b-base#2code-insertion): full-width-bar/lower-block Unicode markers, begin/prefix/hole/suffix/end |
| Codestral | [Mistral FIM encoder](https://github.com/mistralai/mistral-common/blob/main/src/mistral_common/tokens/tokenizers/instruct.py): suffix then prefix |
| Mellum | [JetBrains model card](https://huggingface.co/JetBrains/Mellum-4b-base#fill-in-the-middle-with-additional-files-as-context-generation): optional filename, suffix/prefix/middle |

Context7 verified Ollama's raw/template contract. Browser access to some tokenizer
JSON pages failed; direct read-only public downloads checked their token spellings
and EOS metadata instead. The moving Qwen repository README now discusses Qwen3,
so it was not treated as a version-specific Qwen2.5 authority. No model weights,
hosted credentials or user configuration were downloaded, read or modified.

`infer` examines a case-insensitive, family-bounded model basename. It rejects
unknown aliases and ambiguous variants rather than falling back to a guessed
family. CodeLlama inference is restricted to its documented 7B/13B infilling
variants, including Instruct but excluding Python variants; Mellum's Python SFT
name is recognized. Explicit formats support custom serving aliases. This is
name mapping, not model capability detection. The server must tokenize special
markers and handle BOS normally; no duplicate textual BOS or chat wrapper is
added. Tokenizer/server/model quality is not proven by wire-format tests.

Raw input containing the selected format's control markers fails before HTTP,
without echoing source. Mellum uses only a basename and omits filenames containing
controls or angle brackets; it does not disclose the absolute directory path.
Prefix/suffix Unicode, CRLF and whitespace remain byte-exact within the format's
own separators. No local post-filter Rope context is added to these prompts.

Seven new tests cover golden strings for all six families, inference/errors,
config defaults/round-trip, metadata privacy, control-token collisions and response
cleanup, and actual loopback HTTP bodies for both raw transports. Existing worker
accept/Undo coverage now runs native and raw DeepSeek modes with a leaked Unicode
sentinel. Review corrected CodeLlama marker spacing and preserved its confirmed
Instruct support rather than guessing from a generic model-name substring.
The first seven-test run passed, but predates those final corrections. The next
build failed at linking with `errno=28`; its output is not a passing verification.
Build directories and the formatter cache were subsequently cleared externally,
and free space recovered. No cleanup was performed by this agent. No new release
performance or native GUI/hosted-model claim is made for this change.

Final rebuilt verification after the spacing/inference corrections and expanded
worker test: **2,346 passed**, 7 skipped, plus **2 doctests**, 6 ignored
(`/tmp/token-raw-fim-full.log`). Strict all-target/all-feature lint passed
(`/tmp/token-raw-fim-lint.log`). The offline formatting recipe initially stopped
because its cache had disappeared; the normal recipe restored the formatter and
passed. No project dependency or user configuration was changed. Scoped review
verdict: **Approve** for raw format serialization and integration, not completion
of live model validation, recency context, Phase 5+ or the remaining handoff.

### Context-sensitive inline filters — 2026-09-06

Post-cache filters 5–6 now live in `completion/postprocess.rs`, shared by every
provider and cache hit through the existing worker. The registry supplies the
grammar; the worker reuses one parser and never changes the editor's syntax cache.
`InlineJob` carries a local-only Rope snapshot capped at 1 MiB. Providers still
receive only `InlineRequest`; full local source does not enter HTTP requests or
the result cache. Context Debug output contains metadata, not source text.

The parser sees the full bounded local document with the candidate inserted,
so openers and literals preceding the 4,000-character prompt window remain known.
Recognized literals/comments are opaque. A mismatched closing bracket truncates
the candidate; incomplete opening constructs remain usable. Parser recovery
containing quotes or slashes is deliberately inconclusive and
preserves the whole candidate. This can also leave a bad bracket unfiltered when
an error contains a valid quote or division operator; it is not semantic validation. Unsupported syntax,
documents/candidates over 1 MiB and budget exhaustion also preserve input.

Indentation normalization is limited to Rust, Go, JavaScript, C and C++.
Nonblank, nonliteral/comment code lines supply a tab-versus-space majority;
ties leave whitespace unchanged. Shared editor tab-stop helpers preserve visual
columns, including a partial first-line indent. Literal/comment contents and
first-line spacing after code are protected. Other languages retain their original
indentation because visual tab stops need not match language semantics.

Alternatives share a cooperative 50 ms parsing/traversal budget, rather than
50 ms each. Snapshot conversion and bounded indentation work are not hard-real-time
operations; this is not a strict wall-clock latency guarantee. The parser resets
after each parse, including cancellation, per the installed
[Tree-sitter 0.25.10 API](https://docs.rs/tree-sitter/0.25.10/tree_sitter/struct.Parser.html#method.reset).

Cache entries now explicitly own only request/configuration and raw/served
results, never the local Rope snapshot. Exact hits reapply filters with fresh
local context. Partial replay consumes served text, so normalized tabs/spaces
match what the user actually accepted. Both result vectors count against the
existing 8 MiB budget; fully rejected results are not inserted. Replacement removes
an older exact-context entry even when the new result is empty/rejected/oversized,
so an automatic request cannot resurrect that superseded answer.

Diff-based self-review found and fixed these issues before handoff:

| Severity | Finding | Resolution |
| --- | --- | --- |
| High | Recovery exposed unfinished-string brackets as code | Conservative quote/comment-error fallback; Rust, JavaScript and Python regressions |
| High | Visual tab conversion could change indentation-sensitive syntax | Restrict normalization to covered brace-based languages; Python preservation regression |
| Medium | Eight alternatives could each spend a full parse budget | Share one deadline across alternatives and check traversal as well as parsing |
| Medium | Rejected refresh could leave an old exact-context cache answer | Invalidate that entry before deciding whether the new result can be retained |

Ten added tests cover grammar/literal/bracket behavior, long-prefix context,
indentation/CRLF, bounds/privacy, exhausted budgets and fresh-context/normalized
cache replay. No new native GUI or hosted-provider verification is claimed for
these filters. The earlier native dropdown and cache checks below are separate.
Autocomplete remains active: raw prompt formats, recency context and Phase 5+
are unfinished. The four already-implemented plans remain archived with their
deferred follow-ups active; `HANDOFF.md` is not ready for deletion.

Release measurements from `CARGO_BUILD_JOBS=1 just bench-completion`:

| Measured stage | Fixture | Median |
| --- | --- | --- |
| Inline syntax/indentation filters | One candidate, 100 generated Rust statements | 367.6 µs |
| Inline syntax/indentation filters | One candidate, 1,000 generated Rust statements | 3.71 ms |
| Dropdown keystroke, pending parse | 1,000 carried LSP items | 99.8 µs |
| Filter/sort LSP items | 1,000 items | 75.52 µs |
| Convert server response | 1,000 items | 886.9 µs |
| Fresh syntax-filtered word fallback | 2,000 identifier/comment lines | 300 µs |

Each case used 100 samples. The inline benchmark reuses the worker-style parser
and asserts normalization occurs before timing; snapshot capture/fixture setup
and network time are excluded, but source conversion/parsing/filtering are included.
The measured tenfold document-size increase costs roughly tenfold filtering time;
this is now a worker cost on cache hits too. It runs on the existing worker,
rather than in the editor's update/render handlers.
These are absolute local stage measurements, not before/after speedups, frame
rates, worst-case cancellation latency or a hosted completion latency claim.
Multi-alternative/near-limit/pathological syntax workloads were not benchmarked.
Divan's allocation figures cover Rust allocations, not all native parser memory.

The optimized build took 12m 41s with one build job, including dependency builds;
that is build time, not benchmark latency. It emitted the existing release-only
unused `revision` warning at `src/update/syntax.rs:196`, left unchanged.
Raw output: `/tmp/token-inline-filters-bench.log`.

Final verification after the conservative language/recovery guards and cache
replacement fix: **2,339 tests passed**, 7 skipped, plus **2 doctests**, 6 ignored
(`/tmp/token-inline-filters-full-final.log`). Strict all-target/all-feature lint
passed (`/tmp/token-inline-filters-lint-final.log`). Scoped self-review verdict:
**Approve** for this filter/cache change; no unresolved high-severity finding.
This does not complete the remaining feature roadmap or native/platform matrix.

### Bounded inline-result reuse — 2026-09-06

`runtime/inline_cache.rs` is private to the existing worker, with two operations:
lookup/promote and insert/evict. The LRU holds at most 256 entries and 8 MiB of
owned source/result capacities; fixed entry metadata is separately count-bounded.
Limits use the repository's `ByteSize`. No dependency, public cache API, persisted
setting, model field or disk storage was added. Cache entries have no Debug
implementation that could leak retained source into logs.

Exact reuse checks document identity, cursor, full bounded prefix, suffix,
language, file path and the entire provider configuration. The prefix hash only
accelerates candidate selection; equality is checked even on a matching hash.
Revision/request ID deliberately do not key reusable content. Each served reply
gets the new request's snapshot and still passes the model's current session,
visibility and revision checks. The worker constructs/validates the provider
before lookup, so cache hits do not bypass configuration or named-credential
validation. Explicit requests bypass reuse and run the provider; they do not
flush unrelated entries. Provider failures and empty results are not stored.

Typed/accepted-prefix replay matches character cursor movement, then checks the
whole rolling prefix window against original prefix plus consumed candidate text.
The comparison streams backwards over at most 4,000 characters without building
a temporary concatenated prompt. UTF-8 boundaries and CRLF row movement are
covered. Only compatible, nonempty remainders are returned, in provider order;
backspacing to the original context can recover all original alternatives. Full
acceptance of a shorter alternative can reuse a longer alternative's remainder;
an entirely consumed set misses and requests new generation normally. The
ordinary debounce and rendering pipeline are unchanged.

Existing postprocess filters 1–4 run once before caching. At this historical
checkpoint, post-cache bracket sanity and indentation normalization were unfinished: a character
stack must not interpret literal/comment braces as syntax, and indentation
needs a reliable style signal. Future extra-context or prompt-format inputs
must participate in cache identity when those request fields are introduced.
This checkpoint does not claim that full inline Phase 3 is complete.

Eight tests were added: five cache tests and three worker tests cover exact
context mismatches, forced hash collision, alternative replay, Unicode/CRLF,
sliding windows, consumed choices, hit promotion, replacement, byte/count limits,
explicit refresh, failure retry and new snapshots. A one-shot loopback server
test proves a second request receives a cached remainder after the backend has
closed. Provider-count tests prove compatible replay skips generation and
explicit/config-changed requests do not. These are evidence of avoided provider
calls, not a measured release latency or frame-rate speedup.

Native macOS verification used isolated config/source files under
`/tmp/token-inline-cache-native.0aMMzh/`. A one-shot `/infill` fixture answered
the prefix `x` with `hello_world` plus a sentinel/leaked tail, then exited with
`backend closed`. After Escape and native Backspace/X, the ghost returned at
revision 3 without a live backend; another replay at revision 5 was captured and
visually inspected. Native Tab produced `xhello_world\n`; Undo restored `x\n`.
Explicit refresh cleared the ghost while the backend was unavailable; the
capture named `refresh-error.png` does not visibly show the transient error and
is not evidence of its visual presentation. The unit provider-count test is the
direct explicit-refresh network-bypass guard. Artifacts include `first.json`,
`replayed.json`, `replayed-again.json`, `replayed.png`, `accepted.json`, `undo.json`
and `restored.json`. All test edits were undone and the editor closed cleanly.
No hosted service, real credentials or user files/configuration were changed.

Initial compilation caught a mistaken external `bytesize` import and two
test-only `DocumentId` integer conversions; these were fixed to use the local
type and u64 IDs, without adding a dependency. Final targeted suite: **60 inline
tests passed** (`/tmp/token-inline-cache-targeted2.log`). After the streaming
comparison cleanup, final full suite: **2,329 passed**, 7 skipped, plus **2
doctests**, 6 ignored (`/tmp/token-inline-cache-full2.log`). Strict lint passed
(`/tmp/token-inline-cache-lint2.log`).

Scoped diff-based review checked source retention, exact context validation,
snapshot ownership, cancellation, limits and replay before maintainability.
No unresolved correctness finding in this cache change. Verdict: **Approve**;
filters 5–6, other inline work and the broader handoff remain active. The
changelog, user guide, active plan and sprint queue distinguish those scopes.

### Real rust-analyzer dropdown validation — 2026-09-06

The screenshot's first rows (`ar_flag`, `archiver`, `asm_flag`, `cargo_debug`,
`cargo_metadata`, `cargo_output`, `cargo_warnings`, `ccbin`) are actual receiver
methods on `cc::Build`, not unrelated local words. This was checked against the
installed `cc` 1.2.67 source and the crate's [Build reference](https://docs.rs/cc/latest/cc/struct.Build.html).
The versioned documentation URL was unavailable through the browser and Context7
returned unrelated packages; version-specific evidence comes from local source
and the exact-version native fixture, not those failed lookups. Alphabetical
server order and terse labels explain why this particular list looked arbitrary.
Other previously fixed local-source/ranking defects remain separate findings.

The final native check used rust-analyzer **1.98.0 (88d9e12a, 2026-08-18)** and
an isolated Cargo project with `cc = "=1.2.67"` as a build dependency. Its
`build.rs` copies `compile_fennel_grammar`'s builder chain, but `main()` is empty
so the helper never compiles C sources. Cargo was offline and all project/config
changes stayed under `/tmp/token-ra-dropdown.wazUue/`. The repository source and
user configuration were untouched. No hosted model was involved.

Reproduction: open the fixture's `build.rs`, wait for the server to report Ready,
insert a new indented line after `.file(scanner)`, then type `.` using the native
keyboard path. The dot automatically opened **77** server results. The first
eight labels match the supplied screenshot; `parser`, `scanner` and
`compile_fennel_grammar` were absent. Typing `comp` filtered to exactly
`compile`, `compile_intermediates`, `compiler`, `get_compiler`, `try_compile`,
`try_compile_intermediates`, `try_get_compiler`, with `compile` selected.
Native Tab inserted `compile`; one Undo restored `.comp`. This preserves the
server's plain insertion behavior: full snippet/tabstop support is not advertised,
so this does not claim automatic argument placeholders. Remaining fixture edits
were undone to the unmodified state and the editor closed cleanly.

The presentation check exposed an avoidable loss of semantic information:
`CompletionItemKind::METHOD` was mapped to Function. The model now preserves
Method, painted as `M` (Function remains `f`, Module remains `m`). The duplicate
view `CompletionKind` enum and exhaustive translation table were removed;
sources and rows use `MenuItemKind`, while private badge glyph/color methods stay
in the view module. No new dependency, ranking rule, completion request, insertion
policy, geometry or persisted setting was added. Debug fixture rows share the
same semantic kinds. The selected signature/documentation and final method badges
were visually inspected in the native window.

Artifacts: `final-member-menu.{json,png}`, `final-filtered-menu.{json,png}`,
`final-accepted.json`, `final-undo.json` and `final-restored.json` in the temporary
directory above. Earlier images without the `final-` prefix predate the badge
fix. JSON assertions checked the exact filtered list, absence of fixture locals,
and accepted document text. The screenshot's exact original project/path was not
edited; this verifies its receiver/chain shape with the same dependency version.
Other languages, IME, pointer input and Windows/Linux native checks remain open.
This debug native check makes no release performance claim.

Scoped diff-based review checked enum exhaustiveness, kind conversion, preserved
insertable labels and rendering dependencies. Existing conversion and row tests
now assert Method. A new exhaustive badge test locks down all glyphs and the
shared method/function palette. Completion-targeted tests: **142 passed**
(`/tmp/token-ra-dropdown-targeted.log`). Full verification: **2,321 passed**,
7 skipped, plus **2 doctests**, 6 ignored (`/tmp/token-ra-dropdown-full.log`).
Strict lint passed (`/tmp/token-ra-dropdown-lint.log`).

| Severity | Finding | Disposition |
| --- | --- | --- |
| LOW | Method/function collapse makes member results less recognizable | Fixed; source and view now share one semantic kind |

Scoped verdict: **Approve**. The specific real-server builder-chain validation
is complete; older notes calling it unverified are superseded here. The broader
inline, LSP symbols/usages and platform scope is not complete. The active plan,
user guide and changelog are updated; no additional feature plan is ready to archive.

### Inline alternatives and Option-key dispatch — 2026-09-06

The provider contract now returns up to eight candidates through the existing
worker. Postprocessing stays shared; private model choice storage deduplicates
results and preserves provider order. OpenAI-compatible `n` defaults to one and
is deliberately capped at eight, below the endpoint's wider limit documented in
the [OpenAI Completions reference](https://developers.openai.com/api/reference/resources/completions/methods/create).
Unsupported transports reject multiple choices before credential lookup/network
I/O, rather than issuing multiple requests. Existing single-result config remains
valid through a serde default. No new dependencies or provider control APIs.

Cycling requires a current visible suggestion and exact agreement with already
consumed text. It changes only the selected choice, not buffer revision, undo
history, pending requests or deadlines. Backspace can make choices eligible again.
The shared ghost stage and automation consume one compatible position/count
method, keeping full and cursor-line redraws consistent. The screenshot scenario
is `screenshots/scenarios/inline-alternatives.yaml`.

Scoped review covered response bounds, deduplication, Unicode/CRLF prefix handling,
empty/invisible/stale states, acceptance/undo, action wiring and rendering. Native
validation found a **MEDIUM correctness issue** in Option shortcut dispatch:
logical-only lookup misses composed-key shortcuts, but unmodified-only lookup
breaks layouts that need Option to type the binding character. The fix tries the
logical character first and the layout's unmodified key second. Both interpretations
resolve against the same pending chord; state advances once. Dispatch uses the
existing context method with ordered candidates, removing the mutating chord
completion helper rather than adding another public resolver. Popup priority and
focus/global-command gates remain unchanged. Normal unbound text receives the
original character. The platform conversion uses the installed winit 0.30.13
`KeyEventExtModifierSupplement::key_without_modifiers`; no US physical-key table
or global Option-as-Alt mode was added.

| Severity | Finding | Disposition |
| --- | --- | --- |
| MEDIUM | Option composition and non-US bracket keys bypass cycling | Fixed with logical-first fallback and single chord-state transition |

Eleven new tests since the hosted checkpoint cover provider count/capability,
bounded results, deduplication, compatible-prefix cycling, acceptance/undo,
automation, keymap conditions, fallback precedence and chord state. Final full
suite: **2,320 passed**, 7 skipped, plus **2 doctests**, 6 ignored
(`/tmp/token-inline-alternatives-fallback-tests.log`). Strict lint passed
(`/tmp/token-inline-alternatives-fallback-lint.log`). Scoped verdict: **Approve**
for this change, not for the unfinished feature roadmap or platform matrix.

A temporary native macOS window used a loopback provider and isolated config.
With the actual Norwegian layout, Option+9 (`]`) selected result 2 and Option+8
(`[`) returned to result 1, with revision zero and an unmodified buffer. Tab
accepted `hello_two();`; undo restored the original newline. An unbound Option
character still inserted `˙`, also undone. The inspected final capture shows
`hello_two(); [2/3]`. Evidence is in
`/tmp/token-inline-alternatives-native.s1CyGl/native-fallback-next.png` and
`fallback-{prev,next,accepted,composed}.json`; the initial probes used incorrect
US physical bracket keys for this layout and are not successful verification.
The editor closed cleanly and its fixture server was stopped. No real credentials,
hosted service, user config/source edit or global keyboard-layout change was used.
US fallback has unit coverage, not native verification; broader IME/dead-key and
Windows/Linux checks remain. Headless rendering was also inspected. Debug frame
samples are presentation diagnostics, not new release performance measurements.
The real rust-analyzer dropdown repro remains separate and unverified natively.

Archival recheck: implemented Soft Wrap, Damage Tracking, Command Palette and
Settings v1 plans live in `docs/archived/`, with active deferred-work links.
Active documentation has no stale links to their previous locations. The
incomplete autocomplete plan and temporary handoff remain active. Earlier
single-choice pipeline descriptions below are historical and superseded here.

### Hosted native-suffix inline transports — 2026-09-06

OpenAI-compatible `/v1/completions` and Mistral `/v1/fim/completions` now use the
same `FimProvider`, HTTP/TLS client, cancelable worker, deadline and 1 MiB body
limit as llama.cpp and Ollama. Request shaping is transport-specific; prefix and
suffix remain provider-neutral. Mistral's message content and OpenAI-compatible
choice text converge to the existing single-string postprocess/ghost/accept path.
Mistral text chunks are joined; nontext or malformed replies are rejected without
echoing backend content. Base URLs accept a trailing `/v1` and preserve gateway
prefixes. No dependency or second worker/control API was introduced.

The wire contracts were checked against the [OpenAI Completions reference](https://developers.openai.com/api/reference/resources/completions/methods/create)
and [Mistral FIM reference](https://docs.mistral.ai/api/endpoint/fim). These endpoints
have different reply shapes. Mistral FIM has no `n` request field in the fetched
schema; OpenAI-compatible currently requests one choice. Native suffix insertion
requires server/model support, not merely a chat-compatible endpoint. Raw
sentinel `PromptFormat` rendering/inference and multiple-choice cycling remain
unfinished and are explicitly distinguished in the active plan and user guide.

`ProviderConfig.api_key_env` stores an environment-variable name, never a resolved
secret. The worker resolves only that named variable when constructing the
provider, with no automatic ambient key discovery. Mistral requires the reference;
other transports can omit it. Invalid names, absent/empty/non-Unicode values,
non-graphic token bytes and credentials over 4 KiB fail before network I/O.
Authenticated non-loopback endpoints require HTTPS. Authorization headers are
marked sensitive and attached per request, not retained on the shared client.
Redirects remain disabled. The environment lookup is injectable privately for
deterministic tests, not exposed as another provider API. Existing session equality
includes the new reference, so changing it invalidates an outstanding session.

Scoped diff-based review checked credential persistence/logging, endpoint routing,
redirect behavior, error redaction, response bounds, state reconciliation and
shared cancellation before maintainability. New wire fixtures cover both routes
with root/versioned/gateway bases, native suffix fields, headers, malformed replies,
status errors and string/text-chunk content. Tests verify no ambient lookup for
unauthenticated endpoints and reject invalid credentials before lookup/network.
Runtime tests exercise OpenAI-compatible postprocessing, partial/full acceptance
and undo; a separate test process provides a synthetic Mistral credential to the
real environment lookup and worker, then accepts and undoes the result. The parent
test does not mutate global environment or inspect user credentials. A guard that
requires the child test to actually run caught and corrected an initial module-path
typo; that failed run is not the final verification result.

Final verification: **2,309 tests passed**, 7 skipped, plus **2 doctests**, 6 ignored
(`/tmp/token-hosted-inline-full3.log`). The isolated Mistral worker test passed
both separately (`/tmp/token-hosted-inline-targeted.log`) and in that full run.
Strict lint passed (`/tmp/token-hosted-inline-final-lint3.log`). Ten new tests were
added; the last code change was the corrected child-test path, included in these
results. Formatting and diff checks passed.

| Severity | Scope | Unresolved findings |
| --- | --- | --- |
| None | Native-suffix transports and credential reference | None found in the scoped review. |

Verdict: **Approve** for this implementation, not completion of inline maturity
or verification of a live hosted service.

No hosted API request, billing, real credential lookup, native GUI validation or
new performance measurement was performed for this change. The completion plan
and temporary handoff remain active for raw prompt formats, recency context,
alternatives, cache/filters, Phase 5+ and the other outstanding goal requirements.

### Settings LSP section and v1 archival — 2026-09-06

Settings Phase 3 generates one master row and three rows per registered server:
enabled, read-only command and read-only process state. `src/settings.rs` holds
the metadata alongside general presets; `SettingsState` owns one filtered order
shared by rendering, actions and automation. Config values and live labels are
resolved against the current model, not cached with search metadata. Searches
cover stable names, descriptions, sections and YAML keys, not changing status
or command values. No new registry, I/O path, dependency or public control API
was added.

Changes reuse `toggle_lsp_enabled` / `toggle_lsp_server_enabled`, including their
ordered persistence and lifecycle effects. Per-server preferences stay independent
of the master flag, with absent overrides defaulting to enabled. Disabling stops
the affected servers; enabling allows lazy startup on the next matching open/edit.
Unchanged choices do not save, and read-only rows have no choices or mutation
path. Commands occupy clipped detail text with a bounded “Read-only” accessory.
Status uses the existing server-ID mirror, not a per-root aggregate. No restart
control or alternative process-management surface was introduced.

Scoped diff-based review checked mutation/persistence boundaries first, then
current-value behavior, filtered order, redraw damage and shared layout. Resolved
findings:

| Severity | Location | Resolution |
| --- | --- | --- |
| Medium | `src/update/lsp.rs`, `ServerStateChanged` | Status-bar-only damage left open modal labels stale. Settings and Language Servers now request a full redraw; tests cover all seven states and stable query/selection/order. |
| Low | `src/view/overlay_surface.rs`, list text bounds | Native capture exposed text touching the right accessory. Shared rendering now reserves a text-pad gap; the final headless screenshot was visually checked. |

Six new tests cover registry-derived rows, current/default/empty command values,
master/per-server effects and no-ops, preservation of other overrides, read-only
keyboard/direct actions, long command clipping and live state rendering. Final
source verification: **2,299 tests passed**, 7 skipped, plus **2 doctests**, 6 ignored
(`/tmp/token-lsp-settings-full3.log`); strict lint passed
(`/tmp/token-lsp-settings-final-lint3.log`). A test-only Clippy suggestion was
corrected before this final full-suite run. Verdict: **Approve** for this scope.

Native validation used an isolated macOS editor, temporary config/source files
and the repository's fake LSP server. The real event loop and presentation path
showed Settings filtered to rust-analyzer during Indexing → Ready. Targeted native
keyboard events verified `Cmd+,`, Left/Right enabled-switch changes and persistence.
Command-row Left/Right/Enter actions left config SHA-256 unchanged. Saves retained
`cursor_blink_ms: 777`, an unknown YAML key and the configured command/arguments.
The source remained revision 0/unmodified, and the native window exited cleanly.
Evidence is in `/tmp/token-lsp-settings-native.14FPqZ/`: state snapshots, config
hashes, server transcript and `native-settings.png`. The final headless image is
`headless/screenshot-settings-lsp.png`; it includes the accessory-spacing fix made
after the native capture. `screenshots/scenarios/settings-lsp.yaml` preserves the
headless fixture.

This verifies native macOS keyboard/persistence and live modal rendering, not
physical mouse interaction, Windows/Linux GUI behavior or the separate real
rust-analyzer completion dropdown report. The 20-frame debug presentation check
is not release profiling and supports no speedup claim. Settings v1 is archived
at [settings-page.md](../archived/settings-page.md), while its unimplemented Phase 4
remains active in [Settings Keymap Tab](../future/settings-keymap.md). Earlier
Phase 2 limitations below describe that historical checkpoint, not current status.
No cache, user config or unrelated file was deleted.

### Settings preset UI follow-up — 2026-09-06

Settings Phase 2 now has one descriptor table (`src/settings.rs`) and one cached
filtered ordering authority. Appearance, Editor and Status Bar rows cover the
six originally named settings plus mouse hover/delay and format-on-save. The
Theme row opens the existing picker; it does not duplicate theme discovery or
preview behavior. `Cmd+,` and the palette share the new keymap action.

The existing overlay now supports explicit choice accessories. Chip rectangles
are solved from visible row geometry once, then used for both drawing and hit
testing. Rendering, mouse/keyboard actions, scrolling, caret placement and
automation share the settings order. Off-preset values have no active chip;
opening/closing or clicking a label cannot normalize them. Unchanged choices
do not write. Explicit changes emit the existing ordered `SaveConfiguration`
runtime effect, and status-font changes also synchronize font metrics. No new
I/O is performed by the settings update handler and no dependency was added.

Blink Off now restores a steady caret. A positive 250 ms maintenance interval
keeps the runtime from spinning on a zero-delay wake while preserving other
deadline handling. Shared section-window logic now includes the first heading
when there is room for it and a selectable row; selection reveal uses the same
start rule. Choice layout reuses the already-flattened overlay list rather than
adding another full-list allocation. Narrow footers fit their leading navigation
hint and hide the secondary hint when the two would overlap.

Scoped diff-based review and rendered inspection found and fixed:

| Severity | Location | Resolution |
| --- | --- | --- |
| High | `src/update/ui.rs`, settings row activation | Fixed: label clicks select only; only explicit preset actions mutate config. Regression verifies no save or normalization. |
| Medium | `src/runtime/app.rs` / `src/model/ui.rs`, zero blink interval | Fixed: steady caret plus positive maintenance interval. Runtime and modal tests cover both halves. |
| Low | `src/view/overlay_surface.rs`, top section and narrow footer | Fixed: shared heading/reveal bounds and footer text fitting. Exhaustive section-window tests and headless inspection cover the correction. |

Twelve new tests cover descriptor preset roundtrips, invalid choices, off-preset
values, filtering/empty sections, cross-section view/action order, save effects,
theme handoff, label-only selection, steady caret, automation rows and chip
geometry across scales/widths/scroll. The shared section-window regression sweeps
all selections and previous scroll positions for windows of 1–14 slots. The old
blink-damage test now uses an elapsed positive interval, since zero means Off.
The initial fuzzy fixture matched an extra row across metadata; its query was
made specific to the intended bracket rows. Early builds also exposed missing
exhaustive modal/command arms and a test import; these were corrected.

Final verification: **2,293 tests passed**, 7 skipped, plus **2 doctests**,
6 ignored (`/tmp/token-settings-full6.log`). Strict lint passed
(`/tmp/token-settings-final-lint4.log`). This final run includes the footer's
minimum-width guard, not just the earlier green checkpoint. Formatting and
diff checks pass. The debug screenshot executable rendered the checked-in
Settings scenario at 1100 px and 360 px widths. Both were visually inspected;
the first heading is present, chips remain distinct, and the narrow footer no
longer overlaps. Images: `/tmp/token-settings-visual.wXdhrR/final/screenshot-settings.png`
and `/tmp/token-settings-visual.wXdhrR/narrow/screenshot-settings.png`; logs:
`/tmp/token-settings-screenshot-final.log` and
`/tmp/token-settings-screenshot-narrow.log`.

Verdict: **Approve** for this scoped implementation. This is the general preset
UI, not the remaining LSP settings section or future keymap tab. Headless renders
and model/runtime tests are not native interactive GUI validation. Settings
therefore stays an active plan; no handoff scope is removed and no new performance
speedup is claimed for this feature. No cache or unrelated files were deleted.

### Background Find display follow-up — 2026-09-06

Large cold display searches (at least 256 KiB) now use one runtime worker with
one running request and one replaceable pending snapshot. Update schedules the
effect after query, option, scope, document and focus changes. Rendering never
starts a large scan: it shows “Searching…” and omits stale highlights/overview
marks until current results arrive. Small searches retain synchronous behavior.

Synchronous and background paths share immutable `FindSearchRequest` inputs and
`FindResults` output. Replies require pending-request identity, document identity,
revision, rope-instance identity, query, flags and effective selection scope to
match. Result ownership is checked separately. Closing/reopening resets the
pending session; matching worker-start failures are displayed without retrying on
every blink. Background computation also prepares logical overview lines; the
explicit-action fallback keeps that projection lazy. Worker shutdown does not
join a running regex scan on the UI thread.

Twelve new tests cover scheduling/deduplication, stale query/edit/buffer replies,
document identity, close/reopen, options/scope, mismatched result ownership,
failure/regex status, returning to small synchronous searches, explicit Replace
All freshness, overview preparation, bounded debug output, off-thread execution,
pending coalescing and nonblocking shutdown. Worker tests use channels to control
ordering instead of assuming sleeps establish it.

Scoped diff-based review checked these guards, model/thread ownership, redraw
damage, worker lifecycle and all display readers. It found and fixed an otherwise
unbounded `Debug` representation: debug update tracing formats messages even
without a subscriber. Requests/results now print sizes and counts, never document
or query text or the match array.

| Severity | Location | Resolution |
| --- | --- | --- |
| Medium | `src/model/ui.rs`, Find request/result debug formatting | Fixed: bounded metadata; regression test excludes snapshot text and large output. |

Final verification: **2,281 tests passed**, 7 skipped, plus **2 doctests**,
6 ignored (`/tmp/token-find-async-full3.log`). Strict lint passed
(`/tmp/token-find-async-final-lint.log`). Earlier full-build attempts exhausted
disk space; the new integration cases were consolidated into the existing modal
test executable. A close/reopen fixture was corrected to use the existing Find
action persistence boundary. The final suite includes all corrected fixtures and
bounded debug changes. No cache was deleted.

Verdict: **Approve** for this scoped change, not the entire dirty worktree.
Explicit Find navigation/replacement still computes fresh results synchronously
when cold; it never applies stale offsets. Those handlers clone state, so a cold
result computed in the clone need not populate the active display cache. Running
regex scans are not interrupted mid-computation, and worker panics are not
supervised. A submission/shutdown racing the final publication check can still
enqueue a reply; model guards, not worker queue timing, establish acceptance.
Scrollbar pixel-row projection and painting still happen on the UI thread.
Native GUI latency and the broader inline/settings/LSP scope remain unverified.

The optimized build completed in 5m 47s. Both fresh Find runs completed; the
repeat-run median / p95 milliseconds (40 samples after ten warmups) were:

| Workload | 10,000 lines | 100,000 lines |
| --- | --- | --- |
| Cold matches after edit | 0.739 / 0.830 | 6.895 / 7.253 |
| Cold matches after query change | 0.696 / 0.841 | 6.580 / 7.040 |
| Warm production Find render | 0.389 / 0.425 | 0.406 / 0.425 |
| Edit update + pending-search render | 0.355 / 0.388 | 0.360 / 0.375 |
| Edit + synthetic search/reply + completed render | 1.584 / 1.678 | 11.672 / 12.142 |

The warm fixture explicitly primes real matches outside timing: it does not
silently benchmark an empty pending scene. Pending-frame timing excludes search
effects, while the synthetic roundtrip executes computation and reply application
inline for deterministic total CPU accounting. Neither measures worker scheduling,
contention or native presentation latency. Both fixtures exceed the 256 KiB
threshold (290,000 and 2,900,000 bytes).

The previous synchronous 100,000-line edit/render median was 11.745 ms. The new
pending frame is 0.360 ms, but has no search marks yet; that is a different
completion boundary, not a 33-fold completed-search speedup. Total CPU work remains
similar (11.672 ms synthetic roundtrip), and cold matching remains about 7 ms.
The first run was consistent: 0.373 ms pending, 11.867 ms synthetic roundtrip and
0.407 ms warm rendering at 100,000 lines. The earlier decoration improvement is
retained. Logs: `/tmp/token-find-async-profile.log` and
`/tmp/token-find-async-profile-repeat.log`; preceding synchronous baseline:
`/tmp/token-decoration-profile-repeat.log`.

Formatting and diff checks passed. All verification/profile processes completed;
about 901 MiB free space remained, with no cache deletion. The existing
release-only unused `revision` warning remains in `src/update/syntax.rs:196`.
Next: address explicit cold Find action latency and remaining high-cursor edit
costs without applying stale ranges, then continue the broader handoff scope.

### Decoration traversal follow-up — 2026-09-06

The range-decoration pass now prepares visible-row geometry once, then uses
monotonic logical-line bounds to visit only rows intersecting each decoration.
Repeated logical lines from soft wrap remain separate visual rows. Each row
materializes its text lazily at most once for the pass. The scratch vector and
text caches are local to that invocation, so there is no cross-frame invalidation
policy or retained document state. An empty decoration list keeps its early return.

The per-decoration row-preparation method is removed. Tint-first/stroke-second
ordering, source order within each category, span clipping and painting primitives
are unchanged. Both full and cursor-line redraws use the same range-decoration
stage. The existing debug `TextDecorations` performance stage now includes range
overdraw, which previously sat outside its accumulated elapsed time; no new timer
registry or overlay-specific stage was introduced.

A test-only copy of the preceding traversal is the pixel oracle. The new matrix
compares all output pixels across 48 combinations of text, wrapping, top-row and
horizontal scroll, with 64 mixed-order ranges each. It covers all five decoration
kinds, overlapping alpha tints, tabs, Unicode, CRLF, long wrapped lines, empty
documents and stale/reversed ranges. Span and paint primitives are intentionally
shared: this proves traversal equivalence, not their independent correctness.
The reference test passed before the production optimization
(`/tmp/token-decoration-baseline.log`). An initial fixture used a nonexistent
context width field; it was corrected to use the existing group bounds before
that baseline run.

Scoped diff-based review checked monotonic row ordering, inclusive end-line bounds,
repeated wrapped rows, lazy text lifetimes, unchanged paint order, empty-list fast
return and the existing stage accounting. No public API or dependency was added.

| Severity | Location | Resolution |
| --- | --- | --- |
| — | Scoped range-decoration traversal | No unresolved critical/high finding. |

Verification: **2,269 tests passed**, 7 skipped, plus **2 doctests**, 6 ignored
(`/tmp/token-decoration-full.log`). Strict lint passed
(`/tmp/token-decoration-lint.log`).

Verdict: **Approve** for this scoped change, not the entire dirty worktree.
This does not address synchronous cold Find matching, high-cursor edit mapping,
inline/settings/LSP roadmap work or native GUI validation.

The optimized build completed in 5m 51s, with only the existing release-only
unused `revision` warning in `src/update/syntax.rs:196`. The Find workload was run
twice. Repeat-run median / p95 milliseconds (40 samples after ten warmups):

| Workload | 10,000 lines | 100,000 lines |
| --- | --- | --- |
| Match results after edit | 0.829 / 0.956 | 6.891 / 7.143 |
| Match results after query change | 0.701 / 0.837 | 6.595 / 7.174 |
| Warm production Find render | 0.387 / 0.535 | 0.399 / 0.416 |
| Edit update + production Find render | 1.582 / 1.742 | 11.745 / 12.318 |

The prior same-fixture repeat measured 1.172 ms warm rendering and 13.064 ms
typing/render at 100,000 lines. This repeat measured 0.399 and 11.745 ms, roughly
66% and 10% lower medians respectively. The first post-change run was consistent:
0.399 ms warm rendering and 11.958 ms typing/render. These are exploratory
same-machine CPU measurements, not claims about native input/presentation latency.
Cold matching remains roughly 7 ms and was not optimized by this change.
Logs: `/tmp/token-decoration-profile.log`,
`/tmp/token-decoration-profile-repeat.log`; before:
`/tmp/token-find-refresh-repeat.log`.

The refreshed five-second native sample collected 3,833 main-thread samples.
Summing disjoint call-graph branches for `prepare_visible_line` gives 584 inclusive
samples (15.2%), versus 2,721 of 3,854 (70.6%) before. This includes preparation in
other text/gutter passes, which this increment intentionally does not consolidate.
The repeated per-decoration preparation is no longer dominant. Raw sample:
`/tmp/token-decoration.sample.txt`; workload:
`/tmp/token-decoration-sample-run.log`. As usual, sample shares are diagnostic,
not exact elapsed-time attribution or frame-rate measurements.

The general production workload also completed: 100,000 short-line rendering
measured 0.303 / 0.335 ms; a wrapped million-character line 1.737 / 1.885 ms;
the same unwrapped line 0.190 / 0.214 ms (median / p95, 120 samples). Cursor update
with 10,000 undo entries measured 1.042 / 1.125 us (500 samples). These are sanity
checks, not controlled speedup comparisons. Log:
`/tmp/token-decoration-general-profile.log`.

Formatting and diff checks passed. All build/profile processes completed normally,
and approximately 776 MiB disk space remained. No caches or unrelated files were
deleted. Next: the synchronous cold Find result construction, while retaining
query/document/revision guards and safe replacement semantics. The broader
remaining feature and native/manual verification scope is unchanged.

### Find replacement follow-up — 2026-09-06

Both Find replacement paths now plan pristine character-offset edits and use
`apply_planned_edits`, removing direct rope/revision mutation and duplicated
syntax/LSP scheduling. Replace All records one undo batch. Only the primary
caret has feature-owned placement; secondary carets and both endpoints of peer
selections follow shared mapping. Its final position comes from the actual first
replacement, not a hard-coded line zero. Identical replacements are excluded from
the mutation plan, preserving dirty state, revision and redo history.

The shared live-position snapshot also captures an active selection-only Find
scope for the edited, focused document. Its start has left insertion affinity and
its end has right affinity, so boundary insertions stay inside the scope. The
scope follows forward edits and undo/redo, including deletion to an empty range;
an empty range does not turn into a document-wide search. Replace-and-Find uses
the updated scope and includes an adjacent match at the new caret position.
Explicit Find Next retains its established strictly-after navigation behavior.
Inactive remembered scopes are still recaptured from the live selection when
reopening Find. This is not persistent, document-associated search-session state.

Eight new integration tests cover multiline Unicode placement, peer and secondary
selections, undo/redo, adjacent matches, shrinking/growing/empty scopes, regex
zero-width boundary insertions, no-op history and non-text guards. The initial
five all failed on the old implementation (`/tmp/token-find-replace-before.log`)
and passed after migration (`/tmp/token-find-replace-after.log`). An additional
test fixture initially referred to an absent external byte-size crate; it now
uses the repository's `ByteSize`, without adding a dependency.

The two existing effect tests now recursively count exactly one LSP change
command, matching runtime batch execution rather than assuming flat batches.
Scoped diff-based review checked pristine ordering, unchanged-replacement
filtering, partial primary placement, shared effects, active-document scope
guards, insertion affinity, undo mapping and repeated scoped replacement.
No new I/O, dependencies or public API were added to the mutation path.

| Severity | Location | Resolution |
| --- | --- | --- |
| — | Scoped Find replacement migration | No unresolved critical/high finding. |

Verification: `just test '--no-fail-fast'` passed **2,268 tests**, 7 skipped,
plus **2 doctests**, 6 ignored (`/tmp/token-find-replace-full2.log`). Strict lint
passed (`/tmp/token-find-replace-lint.log`).

Verdict: **Approve** for this scoped migration, not the entire dirty tree.
Full per-pane selection/active-index undo snapshots and native/manual validation
remain open. Regex replacement text retains existing literal semantics; this
change does not implement capture-group expansion or single zero-width-match UI
selection. Cold Find/decoration performance work remains in scope.

`just profile-workloads replacements` adds a production-update probe at
1/100/10,000 matches in one/two panes. Each case uses ten warmups and 200 timed
samples. It restores buffers, cursors and undo stacks outside timing and creates
a fresh Find state/cache for every sample. Timed work includes the cold match
scan, planning, mutation, history capture and effect construction. Postconditions
check final text, the primary/peer carets and exactly one undo record outside the
measurement. LSP/completion/bracket matching are disabled; rendering, command
execution and native input latency are excluded. No speedup is inferred from
these current-state probes alone.

The optimized build completed in six minutes despite the limited free space.
It emitted the existing release-only unused `revision` warning in
`src/update/syntax.rs:196`; strict all-feature lint remained clean. No caches or
unrelated files were deleted. Replace All results, median / p95 microseconds:

| Matches | One pane | Two panes |
| --- | --- | --- |
| 1 | 4.208 / 4.333 | 5.667 / 5.875 |
| 100 | 128.542 / 144.834 | 134.459 / 155.292 |
| 10,000 | 5,436.292 / 5,749.792 | 5,332.667 / 5,761.166 |

The small inversion between one/two-pane large cases is not evidence that adding
a pane makes replacement faster. These are exploratory current-state timings,
not a controlled speedup comparison with the old, non-undoable implementation.
Log: `/tmp/token-find-replace-profile.log`.

The existing Find probe was rerun twice on this same optimized build. Repeat-run
median / p95 milliseconds (40 samples after ten warmups):

| Workload | 10,000 lines | 100,000 lines |
| --- | --- | --- |
| Match results after edit | 0.756 / 0.869 | 7.058 / 7.541 |
| Match results after query change | 0.726 / 0.773 | 6.771 / 7.494 |
| Warm production Find render | 1.113 / 1.180 | 1.172 / 1.504 |
| Edit update + production Find render | 2.509 / 2.708 | 13.064 / 14.444 |

Warm match/status lookups remained around 0.08 us median. The first run's
100,000-line medians were 7.485, 7.276, 1.214 and 13.797 ms respectively, but
query-change and typing/render p95 values reached 52.038 and 49.976 ms. Those
large tails did not recur in the repeat, so neither a tail regression nor a
speedup is attributed to this correctness refactor. Logs:
`/tmp/token-find-refresh-profile.log`, `/tmp/token-find-refresh-repeat.log`.
The ~7 ms synchronous cold-result construction remains substantive work; cached
lookups are not the bottleneck. These are CPU fixture timings, not GUI latency.

A five-second native sample of the warmed `sample-find` workload collected
3,854 main-thread samples. Summing the disjoint `render_one_decoration` call-graph
branches gives 2,744 inclusive samples (about 71%). Repeated
`prepare_visible_line` / `Document::line_length` / rope line lookup remains
prominent. Source inspection confirms every decoration prepares every visible
row again before testing whether its logical line intersects. The next concrete
optimization is shared prepared-row reuse and narrowed decoration traversal in
`src/view/editor_text.rs`, preserving tint/stroke ordering and soft-wrap geometry.
This is a target backed by current sampling, not an implemented optimization.
Raw sample: `/tmp/token-find-refresh.sample.txt`; workload log:
`/tmp/token-find-refresh-sample-run.log`.

The existing insertion probe also passed its postconditions on this release
build. Median / p95 microseconds (500 samples per case):

| Cursors | One pane | Two panes |
| --- | --- | --- |
| 1 | 2.334 / 2.458 | 3.291 / 3.500 |
| 10 | 18.292 / 22.667 | 35.291 / 38.208 |
| 100 | 384.958 / 437.042 | 855.750 / 902.750 |
| 1,000 | 3,749.584 / 7,252.917 | 6,629.375 / 6,799.125 |

Medians are broadly consistent with the prior insertion checkpoint; this single
run does not establish tail-latency stability or a performance improvement.
Log: `/tmp/token-insert-refresh-profile.log`. Dedicated deletion/duplication
measurements remain absent. All build/profile processes completed normally;
about 941 MiB disk space remained afterward. Formatting and diff checks passed.

### Ordinary forward-edit follow-up — 2026-09-06

The remaining ordinary forward `DocumentMsg` mutations now plan pristine edits
and use `apply_planned_edits`: character/word deletion, cut, whole-line deletion,
duplication and indent/unindent. This removes `shift_sibling_cursors`,
`delete_selection`, the record-time peer-mapping bridge and its operation adapter.
The document handler shrank from about 1,790 to 695 lines in this increment,
without expanding the public API or adding another caret-placement mode.

Deletion unions touching/overlapping physical ranges, then maps every original
caret through the descending plan. Clipboard capture remains in original
selection order, including overlapping payloads; Copy does not collapse selections.
Status counts Unicode characters. Backspace/delete join CRLF atomically, and
empty deletion/indentation plans do not create undo records or mark the file dirty.
Indentation shares selection-endpoint mapping, including reversed selections;
unindent examines at most four leading characters rather than copying whole lines.

Whole-line deletion groups contiguous runs before choosing their physical ranges.
A trailing run includes its preceding line ending, avoiding a stray empty final
line. Caret placement chooses a surviving pristine line and maps its offset,
retaining preferred columns without a preview buffer. Noncontiguous runs preserve
the active caret; a single contiguous run collapses carets as before. The existing
single-cursor policy still deletes the current line rather than all selected lines.

Duplication captures every payload before mutation, retaining one copy per cursor
on a shared line. Stable descending insertion order makes equal-point copies
deterministic. Each caret is placed relative to its own copy, then mapped only
through subsequent edits; this avoids shifting it through its own insertion twice.

Thirteen added regressions cover same-line sibling deletion, overlapping words
and selections, clipboard order with unselected carets, newline/CRLF joins, no-op
history, pristine duplicate sources, equal-point duplicate ownership, trailing
line runs, noncontiguous active-caret preservation and reversed unindent selections.
The initial six deletion regressions all failed before implementation
(`/tmp/token-delete-before.log`). Two duplication regressions also failed in a
later baseline run; fail-fast cancelled other cases, so that run is not evidence
that every added test reproduced a defect (`/tmp/token-lines-before.log`).

Verification: **2,260 tests passed**, 7 skipped, plus **2 doctests passed**,
6 ignored, including the final lint-adjusted-source rerun
(`/tmp/token-delete-full.log`, `/tmp/token-delete-final-full.log`). Strict lint passed after its suggested
equivalent stable `sort_by_key(Reverse(...))` change
(`/tmp/token-delete-lint2.log`). Formatting and diff checks passed.
Scoped diff-based review checked deletion unions,
Unicode offsets, CRLF boundaries, original clipboard order, equal-point insertion
ownership, EOF placement, shared effects, peer mapping and no-op history.

| Severity | Location | Resolution |
| --- | --- | --- |
| — | Scoped ordinary forward-edit migration | No unresolved critical/high finding. |

Verdict: **Approve** for this scoped migration, not the entire dirty worktree.
The Rust/review skill baseline guided private helpers, stable ordering, regression
coverage and strict lint; no new dependencies or external APIs were introduced.

No fresh optimized deletion/duplication measurement is claimed. The insertion and
file-identity tables below predate this increment and remain historical evidence.
Approximately 1.6 GiB disk space remained after verification; no release rebuild
or cache deletion was attempted. Fresh optimized probes are still needed for these
paths, particularly high cursor counts with per-edit/per-position mapping.

Remaining: Find/Replace's direct buffer mutations and position mapping; cold Find
and decoration work; complete per-pane selection/active-index undo snapshots.
Peer positions inside deleted text still clip rather than restore losslessly.
The inline/settings/LSP roadmap and native completion/GUI validation remain open.
Implemented plans stay archived, but the temporary handoff cannot yet be deleted.

### Ordinary insertion follow-up — 2026-09-06

After available disk space recovered to 4.8 GiB, the peer-position checkpoint
passed the full suite: **2,240 tests**, 7 skipped, plus **2 doctests**, 6 ignored.
Strict lint passed (`/tmp/token-peer-full-test.log`,
`/tmp/token-peer-full-lint.log`). This closes the earlier ENOSPC verification gap
for that checkpoint without deleting caches or unrelated files.

Typing, newline insertion and paste now share a private pristine-buffer planner
and `apply_planned_edits`. The three independent mutation/sibling-adjustment
paths are removed. Each selection is replaced, paste lines are distributed in
document order when their count matches the normalized cursor count, and every
caret is placed after its insertion through the shared offset policy. Disjoint
carets retain their original order/active index. Overlapping/touching selections
are merged before planning so text is neither replaced twice nor corrupted.

Surround is represented by two boundary insertions, not a whole-selection
replacement. Peer positions follow surviving text, and undo removes only the
new delimiters. Character offsets are used throughout, fixing Unicode surround
caret placement; paste status also counts characters instead of UTF-8 bytes.
Shared planned-edit effects retain the earliest affected line and old/new line
counts for provisional highlight shifting.

Four new regressions failed before implementation (ignored selections, stale
same-line sibling columns, newline replacement and Unicode surround). They pass
afterward. Three further tests cover full multiline paste with a reversed
selection, overlapping replacements and mixed surround/plain carets with the
setting enabled/disabled. The targeted filter passes all 12 ordinary-edit tests
plus one unrelated matching syntax test. The first post-change full suite and
strict lint passed before those three final tests were added.

`just profile-workloads insertions` adds an optimized production-update probe at
1/10/100/1,000 cursors in one/two panes. It uses 10 warmups and 500 samples, resets
rope/cursor/history fixtures outside timing, and verifies final text, every caret,
selection collapse and one undo record outside timing. Completion, LSP and bracket
matching are disabled. This measures synchronous update work, not command
execution, rendering, presentation or native input latency.

Final optimized insertion measurements (microseconds, 500 samples each):

| Cursors | One pane median / p95 | Two panes median / p95 |
| --- | --- | --- |
| 1 | 2.500 / 2.625 | 3.250 / 3.500 |
| 10 | 18.334 / 18.792 | 34.416 / 35.917 |
| 100 | 375.583 / 446.958 | 839.291 / 911.250 |
| 1,000 | 3,666.125 / 3,813.959 | 6,506.292 / 6,879.292 |

The initial insertion-refactor probe, before skipping redundant accepting-pane
mapping, measured 6,522.291 us at 1,000 cursors in one pane and 9,333.209 us in
two panes. The final probe measured 3,666.125 and 6,506.292 us respectively
(about 44% and 30% lower medians in this fixture). These are exploratory
same-machine measurements, not a comparison against the old, incorrect insertion
semantics or a claim about end-to-end latency. The final source also bounds paste
line splitting, which this character-insertion fixture does not exercise.
High cursor counts still have nontrivial cost; the mapper still performs
per-edit/per-position work. These timings alone do not attribute all of that
cost to one function. Logs: `/tmp/token-insert-profile.log` (initial) and
`/tmp/token-insert-profile-final.log` (final).

The optimized recipe successfully rebuilt after the earlier ENOSPC failures.
It also builds application binaries; the clean/recovered build took about nine
minutes and the final incremental rebuild about six. An existing release-only
unused `revision` warning in `src/update/syntax.rs:196` remains; it was not
introduced by this insertion patch.

Previously blocked file probes also completed on this final optimized build.
Values below are median / p95 microseconds, 500 samples per case:

| Probe | 1 open document | 100 open documents | 1,000 open documents |
| --- | --- | --- | --- |
| Canonical identity hit | 0.083 / 0.125 | 7.666 / 7.917 | 81.458 / 90.791 |
| Identity miss | 0.208 / 0.209 | 7.833 / 11.416 | 81.250 / 91.625 |
| Current-file Problems scope | 0.125 / 0.167 | 10.667 / 11.583 | 106.292 / 121.667 |
| Open request plus synthetic failure reply | 1.542 / 1.667 | 11.500 / 12.583 | 114.875 / 127.042 |
| Exact-path tab reuse update | 1.959 / 2.042 | 35.625 / 40.792 | 371.916 / 408.459 |

The identity fixture supplies resolved identities; at one document the hit is
focused, while larger cases hit an unfocused document. The open fixture measures
update handling, not file-worker preparation or disk reads. Lookup and Problems
scope still scale with document/diagnostic count; moving resolution off the
update path does not imply constant-time lookup. These are current-state
measurements, not before/after speedup claims for file I/O.

At 100,000 lines, save-snapshot preparation plus synthetic success completion
measured **1.791 / 1.875 us** (500 samples). An undo+redo pair including
saved-content comparison measured **209.917 / 222.083 us** (200 samples), with
an edit at the document end. Actual writes, background execution, rendering and
native input latency are excluded. Reproduce with `just profile-workloads
file-identity`, `file-open` and `file-io`. Logs are
`/tmp/token-identity-profile-final.log`, `/tmp/token-open-profile-final.log` and
`/tmp/token-file-io-profile-final.log`.

Final correctness verification: **2,247 tests passed**, 7 skipped, plus
**2 doctests passed**, 6 ignored. Strict `just lint` passed after the final
production changes (`/tmp/token-insert-final-full.log`,
`/tmp/token-insert-final-lint2.log`).

Scoped diff-based self-review checked pristine ordering, selection normalization,
Unicode offsets, explicit caret placement, peer/undo mapping, no-op behavior and
highlight effect routing. It found and fixed a newly introduced allocation issue:
single-cursor paste unnecessarily collected all clipboard lines. Distribution now
collects at most cursor-count plus one lines and is skipped for a single caret.
The shared transaction also skips mapping the accepting pane when every caret
has an explicit final offset, retaining general mapping for partial placement.
Explicit placement still clears occurrence/selection history and deduplicates.

| Severity | Location | Resolution |
| --- | --- | --- |
| Medium | `src/update/document.rs::insert_at_cursors` | Bounded clipboard-line scratch allocation; single-cursor paste does not allocate that vector. Fixed. |

Verdict: **Approve** for the scoped insertion change; no unresolved critical/high
finding. This is not a review/approval of the entire dirty worktree.

Remaining migration: deletion, cut, duplicate and indentation still own focused
caret algorithms. `shift_sibling_cursors` remains for backspace and duplication.
Undo still does not retain complete per-pane selection/active-index snapshots;
positions inside actual deletions follow the existing clipping policy. No claim
of full edit-system or roadmap completion is made here.

### Ordinary peer-position follow-up — 2026-09-06

Ordinary typing, paste, cut, deletion, duplication and indentation now map other
panes using the same character-offset policy as completion, inline acceptance
and undo/redo. Positions are captured before the document handler; its private
`record_edit` boundary feeds the exact new operation into `EditPositions` before
pushing history. Batches retain recorded application order, not pristine-offset
sorting. No history traversal/cloning is used to infer mutations. The focused
pane's caret/selection algorithms are deliberately unchanged in this increment.

This removes five `sync_other_editor_cursors*` helpers, the public
`EditorArea::adjust_other_editors_cursors` method and its independent
`adjust_position_for_edit` arithmetic. It also removes peer-only indent maps and
an unreachable single-paste fallback that bypassed undo recording. Single-pane
editing captures no offset vectors and skips operation text counting for peers.
No measured speedup is claimed.

Initial regressions reproduced incorrect Unicode multiline-paste columns and
selection-replacement peer lines. Five final tests cover those cases plus
newline joins, word/line deletions, duplication, reversed selections, multi-cursor
newline batches, indentation/unindentation, undo/redo and no-op preservation.
Fixtures keep each cursor aligned with its selection head; independent peer
caret and selection assertions use two cursors. An initially inconsistent focused
fixture was corrected after a debug invariant failure.

Verification:

- Five tests passed in the freshly built `ordinary_edit_positions` integration
  executable (`/tmp/token-ordinary-direct-final.log`). Normal nextest builds
  failed linking other application executables with `errno=28`; serialized builds
  and a test-target-only Cargo fallback could not avoid those binary builds.
- Strict `just lint` passed for all production changes
  (`/tmp/token-ordinary-final-lint.log`). Its final rerun after fixture corrections
  failed due to ENOSPC, not a reported Rust diagnostic.
- `just fmt` and `git diff --check` passed. The full suite and optimized profiling
  remain unverified for this checkpoint. Prior green counts are historical.
- No caches or unrelated files were deleted. About 500 MiB remained after failed
  build temporaries were released; free space before further build retries.

Scoped diff-based self-review checked removed call sites, operation coverage,
Unicode counts, batch order, excluded invoking/unrelated panes, no-op state and
history's separate mapper. Review caught that revision changes alone could reset
peer navigation state for an empty batch; `EditPositions` now records whether an
actual atom was mapped before restoring. No critical/high defect remains in this
scoped review. The verification gap prevents a merge-ready verdict.

| Severity | Location | Remaining concern |
| --- | --- | --- |
| Medium | `src/update/text_edits.rs`, `src/update/document.rs` | Full regression-suite verification is blocked by disk space. Run `just test` and `just lint` after freeing space. |

Verdict: **Comment — verification pending**, not approval of the entire dirty
worktree or completion of finding 1's broader migration. Same-pane sibling
placement still uses legacy algorithms; general replacement offsets are clipped,
not semantic tracking of surviving text inside surrounds. Other roadmap work and
native GUI/Windows verification remain outstanding.

### Shared file-identity follow-up — 2026-09-06

Finding 7's implementation is in place. Each document owns a source-bound
`FileIdentity` snapshot containing the original spelling, resolved path and URI.
The immutable data is shared cheaply with worker requests. Ordinary opens and
successful writes/reloads resolve identity at their I/O boundary; synchronous
startup loaders and the runtime compatibility boundary cover legacy callers.
Lookup itself does not touch the filesystem or environment.

Tab reuse, navigation's location display, LSP URI lookup and current-file
Problems all use the same document predicate. This replaces the per-open
canonical-path map and the inconsistent LSP/Problems canonicalizing scans.
Problems no longer filters out symlinks whose basename differs from their target.
Known original/canonical spellings reuse directly; arbitrary unknown aliases
still require worker resolution. `find_open_file` is now a pure query, not an
implicit resolver, and deterministically prefers the focused group. Existing
buffers remain authoritative when the worker encounters an alias.

Source-path checks prevent retaining old aliases after a path change. Save and
reload replies carry refreshed identity only after successful I/O; stale/failing
replies cannot install it. A pending open retries a worker snapshot whose URI
changed despite retaining its display path. A same-path identity change also
closes/reopens LSP synchronization. LSP open and diagnostic-clear operations reuse
the stored URI; only an unresolved legacy document invokes the runtime fallback.
Snapshots are released with the document rather than an independently pruned map.

Review found a missing-path edge case made consequential by shared lookup: URI
encoding discarded `..`, so a nonexistent `a/../b` could be identified as `a/b`.
The fallback now preserves parent components. It does not pretend unresolved
paths are fully canonical or guess unknown alias equivalence.

Eight new regressions cover source-path invalidation, differently named symlinks
across tabs/LSP/Problems after the disk target is moved, known-path group preference,
same-path stale-open identity, successful/failed Save As, stale reloads, worker
reuse after a symlink changes, and missing parent components. Existing real worker
write/read tests now assert the returned canonical identity. Update-only fixtures
explicitly install identity when simulating a loaded document. Two fixtures that
queried an unknown raw alias after opening its canonical LSP URI now query that
known URI spelling; they no longer rely on hidden filesystem work in lookup.

Verification: the final `just test '--no-fail-fast'` passed **2,235 tests**,
7 skipped, plus **2 doctests**, 6 ignored. Strict `just lint` passed. The earlier
full run exposed seven fixture failures, corrected before this green run; logs
are `/tmp/token-identity-test2.log` and `/tmp/token-identity-lint2.log`.

`just profile-workloads file-identity` adds reproducible canonical-hit, miss and
current-file Problems-scope probes at 1, 100 and 1,000 documents/diagnostic groups.
Identity fixtures are supplied without filesystem resolution, with hit/miss and
scope assertions outside timing. The hit is focused at one document and unfocused
at larger counts. Each probe uses 10 warmups and 500 samples, excluding model/config
setup, worker work and rendering.
The existing `file-open` workload still measures request/reply bookkeeping.
Neither is an end-to-end latency test. **No new timings were produced:** release
profiling remains unverified after the earlier disk-space failure, with about
740 MiB available after this test run. No caches or unrelated files were deleted.

Scoped diff-based self-review checked pure lookup call sites, source/URI guards,
saved/reloaded identity ordering, content-safe reply tracing and deterministic
group preference. No unresolved high/critical findings remain in this increment.

| Severity | Area | Resolution |
| --- | --- | --- |
| HIGH | Missing-path identity | Preserve parent components instead of identifying a different child |
| MEDIUM | Stale same-path open snapshot | Compare captured/current URI and retry before reuse |
| MEDIUM | Problems symlink scope | Use the shared predicate without a basename gate |

Verdict: **Approve** for this implementation. This is not proof of complete
cross-platform/native behavior or a measured speedup. Startup and other runtime
I/O, queue/teardown limits and Windows/GUI validation remain. Identity refresh is
boundary-based, not a live alias watcher, and does not add hard-link matching.
The remaining edit, Find/decoration, inline, settings and LSP roadmap is intact.

### Asynchronous configuration-opening follow-up — 2026-09-06

Configuration-directory preparation, default-keymap creation and log selection
now execute on the same ordered file worker as normal file opening. Environment
discovery also occurs on that thread. `FileOpenSource` carries either a direct
path or a configuration-resource request; the worker then prepares a normal
file or returns a directory for the runtime to reveal. This removes the separate
`Cmd::OpenConfigResource` effect and unkeyed `AppMsg::ConfigResourcePrepared`
reply instead of adding another pending-request map or worker.

The target group, active tab and stale-input snapshot are captured before config
preparation begins. A delayed resource reply therefore cannot treat a new tab
choice as its original destination. Ordinary and config-file opens share alias
reuse, live-buffer preservation and post-load installation. Directory requests
do not supersede pending tab opens, do not consume their LSP route hint, and
produce no tab/document. Their tokens are consumed once; a closed group rejects
the reply without launching the explorer. Existing exclusive keymap creation,
symlink preservation and log-name selection are unchanged.

Six new regressions cover original-group installation, stale tab choices,
directory-versus-file ordering, duplicate/closed-group directory replies, actual
ordered worker preparation alongside writes/reads, and missing-root errors
returned with the original request token. Worker fixtures inject isolated config
roots without changing process environment and assert discovery runs on the
`file-io` thread. The seven prior config tests still cover default creation,
no-clobber files/symlinks, log selection, action routing and unsaved-buffer reuse.

Verification: `just test-one config_resource` passed **13 tests**; the full
`just test '--no-fail-fast'` run passed **2,227 tests**, 7 skipped, plus
**2 doctests**, 6 ignored. Strict `just lint` passed. Logs:
`/tmp/token-config-async-{targeted,test,lint}.log`.

Scoped diff-based self-review checked the removed effect/reply call sites,
token consumption, failure/redraw paths, directory side-effect gating, queue
ordering and content-safe debug formatting. Existing no-clobber preparation
continues to own filesystem errors; update performs no resource discovery.

| Severity | Finding |
| --- | --- |
| — | No unresolved findings in the scoped configuration-opening change. |

Verdict: **Approve** for this increment, not completion of finding 7. General
resolved file/URI identity and its open/Save As/rename invalidation remain to be
shared across tab reuse, navigation, LSP lookup and Problems. Startup loading,
configuration YAML save/reload, theme loading and other runtime I/O remain outside
this asynchronous-opening change. Slow jobs still delay later jobs and worker
teardown still drains the queue without a bound. Native GUI/Windows checks remain
unperformed. No latency improvement is claimed: the prior optimized profiling
build failed for disk space, only about 1.4 GiB remained, and it was not retried
or replaced with debug timings. No caches or unrelated files were deleted.

### Asynchronous new-tab open follow-up — 2026-09-06

Ordinary new-tab opening now emits a preparation request to the existing ordered
file worker. Validation, canonical alias lookup, text reading, binary detection
and image decoding no longer run in the layout update handler. Exact-path reuse
needs no filesystem work; worker-resolved aliases reuse live documents instead
of rereading unsaved buffers. Concurrent loads reuse an already-installed live
document, and closed/renamed worker snapshots retry against current documents.

The model owns target group, originating tab/cursor/selection/revision, current
focus and a typed post-open position. Reply installation consumes its token once,
rejects closed groups and respects newer input. LSP UTF-16 conversion and CLI
character-position clamping use the loaded destination, not whichever document
is subsequently focused. Native dialogs carry their requesting group, and
CLI/automation waiters register before command execution so both immediate reuse
and delayed installation are accounted for. `--wait` then tracks document closure.

One private tab installer replaces duplicated loading/reuse/split setup. Review
found that splitting special tabs previously created plain-text views, which
could pollute subsequent reuse. The installer now retains image/binary modes.
Decoded image pixels are shared with `Arc<[u8]>`; pan and zoom remain per view.
The obsolete synchronous navigation wrappers were removed; runtime keeps one
coordinate-aware open adapter.

Workspace edits also depended on synchronous opening. Rename, code actions and
server apply-edit requests now retain their continuation until unopened targets
are prepared. Failures and changed/closed target snapshots reject that deferred
operation before its edits run. Code-action follow-up commands and successful
server acknowledgements wait for application. Already-open buffers are retained;
missing and non-text disk targets are rejected rather than created as new files.
This does not add atomic resource operations or change the existing handling of
unsupported/overlapping edits into a fully atomic workspace transaction.

Verification: **2,221 tests passed**, 7 skipped, plus **2 doctests passed**,
6 ignored, via `just test '--no-fail-fast'`. Strict `just lint` passed. The
19 new tests comprise 16 update/continuation regressions and three real worker
tests. Existing navigation, file-dialog, fake-LSP, code-action and CLI wait
fixtures now explicitly complete deferred effects; production loading has no
test-only synchronous bypass. Worker tests cover images, binary content, alias
reuse without rereading, directories, invalid UTF-8, malformed images, oversized
files, missing files and ExistingText policy. Native GUI/Windows checks and the
live rust-analyzer completion scenario remain unverified here.

Profiling: `just profile-workloads file-open` adds two main-update workloads at
1, 100 and 1,000 open documents: request plus simulated failure reply, and
exact-path reuse. Each uses 10 warmups and 500 samples, excluding model setup,
worker/disk I/O, parsing and rendering. It measures bookkeeping scaling, not
end-to-end open latency. **No measurements were produced:** the optimized build
failed with `No space left on device (os error 28)`. A debug application build
also hit a linker disk-space error earlier; the full test and lint runs above
subsequently completed. No build caches or unrelated files were deleted. Retry
the optimized recipe after disk space is available; earlier timings elsewhere in
this audit are historical measurements, not measurements of this checkpoint.

Finding 7 remains incomplete. Config directory/keymap/log preparation is still
synchronous in runtime; startup constructors, general tab identity, LSP URI
lookup and Problems scope still contain synchronous filesystem work. The new
open cache is not yet their shared identity authority. The ordered worker has
no cancellation/preemption: slow opens can delay saves behind them, and shutdown
drains the queue without a filesystem time bound. No atomic-write/fsync or
end-to-end responsiveness guarantee was added.

Scoped diff-based self-review checked content-safe debug formatting, error
replies, stale target handling, effect ordering, mode preservation and CLI waiter
completion. The following issues were fixed and covered before the green run:

| Severity | Area | Resolution |
| --- | --- | --- |
| HIGH | Workspace edits | Wait for preparation; reject failed/changed targets before deferred mutation/acknowledgement |
| HIGH | Special-tab installation | Shared installer preserves modes through splits and reuse |
| MEDIUM | Image sharing | Split views share decoded pixels; regression asserts shared allocation |
| MEDIUM | Rejected opens | Closed groups and missing source views still notify waiters and redraw loading state |

Verdict: **Approve** for the scoped asynchronous-open change; profiling and
manual validation remain outstanding, and this is not completion of finding 7
or the roadmap. Test/lint logs are `/tmp/token-open-final3-{test,lint}.log`;
the failed profiling build is `/tmp/token-open-profile.log`.

### Plan archival follow-up — 2026-09-06

The implemented damage-tracking plan and the command-palette proposal superseded
by OverlaySurface Phase 4 moved to `docs/archived/`. Implementation notes name
the current code/test evidence without marking historical manual checks or
optional ideas complete. Deferred history work remains in
[command-history follow-ups](../future/command-history-followups.md). Soft wrap
was already archived; the index and sprint queue now reflect its implemented
status. Autocomplete and settings remain active. The temporary handoff remains
until all its work is implemented and verified.

Scoped diff-based self-review checked archive contents, status claims and inbound
links, including the source documentation link. No behavior changed. All 71 local
links checked in the moved plans, follow-up note, index and sprint roadmap resolve.
Targeted damage (15), command-history (8) and palette (25) tests passed; formatting,
strict lint and `git diff --check` passed. The previous full suite remains 2,202
tests plus two doctests; native manual checks were not repeated.

| Severity | Finding |
| --- | --- |
| — | No unresolved findings in the archival change. |

Verdict: **Approve** for this documentation cleanup, not completion of the roadmap.
Logs: `/tmp/token-plan-archive-{fmt,fmt-check,damage,history,palette,lint}.log`.

### Profiler fixture follow-up — 2026-09-06

Finding 4 is fixed. `profile_render` now allocates a distinct document for each
split instead of overwriting the document shared by `SplitFocused`. Explicit
file inputs cycle in the supplied order, including independent copies of repeated
paths. Missing/unreadable/non-UTF-8 inputs fail setup instead of silently rendering
an error comment. CSV/TSV inputs activate the real grid renderer and its viewport;
`--include-csv` explicitly selects the final synthetic pane, including a one-pane
run. Text fixtures carry filename/language metadata and fresh syntax highlights.
Synthetic Rust no longer consists of thousands of unclosed function bodies.

Scrolling now uses each mode's own clamped viewport helpers. Font and shell
metrics share the same 2× scale. All setup/parsing stays outside render timing.
Five regression tests assert document identity/content independence, file cycling,
CSV/TSV cells and mode, bad inputs, explicit synthetic mode selection and scrolling.
`just test`: **2,158 passed**, 7 skipped, 6 ignored doctests. Strict all-target,
all-feature Clippy passed. Diff-based self-review: **Approve** for this repair.

The new repository recipe is:

```sh
just profile-render --frames 500 --splits 3 --lines 200 --include-csv --scroll --stats
just profile-render --frames 20 --splits 3 --files build.rs --files samples/large_data.csv --stats
```

The first command, rerun after builds/tests finished, confirmed three distinct
document IDs, two highlighted Rust panes and a CSV grid. CPU median was **3.33 ms**,
p95 **11.30 ms**, p99 **74.43 ms** (500 frames; 1920×1080, 2× font scale).
The high tail remains visible and should not be represented as stable frame rate.
This is a corrected mixed-mode workload, **not** a before/after speedup or a change
to the single-group Find measurements below. The real-file smoke run confirms
the two supplied paths cycle across three independent documents. Logs are
`/tmp/token-profiler-fixtures-{quiet-profile,files,full-test,lint}.log`.

The next Find/overview follow-up is recorded below. Findings 6–7 and the broader
handoff feature roadmap remain.

### Find and overview follow-up — 2026-09-06

Finding 3 is fixed; finding 2's first-stage consolidation is implemented.
`FindResults` still owns the complete, shared match list used by navigation,
replacement, status and rendering. Its line projection is now lazy and bound to
the immutable result's rope snapshot. Dense results advance a rope line iterator;
sparse gaps seek directly. This preserves Ropey's newline/Unicode/EOF coordinates
without performing a tree lookup for every adjacent match.

Both full-group and caret-only scrollbar rendering now enter the same projection
path directly. Find and diagnostic marks reduce into a track-height-sized row
buffer, with the existing highest-priority-wins rule. One cached projection per
pane replaces per-frame Find tick vectors and tree reduction. Cache keys cover
rope identity/revision, Find result identity (including query/options/scope and
focus), actual wrap-layout identity, total visual rows and fractional track height.
Diagnostic ranges/severity are compared because their public vector can change
in place without a document revision. Diagnostic message changes do not invalidate
geometry. Colors and track origin/width are applied at paint time.

The cache is derived state only: no new runtime effects, search limits or regex
semantics. The public editor field uses an opaque cache type; projection details
remain crate-private. Search still copies/scans the full buffer synchronously on
a cache miss. This is not an incremental or asynchronous search implementation.

Eight new regression tests cover lazy snapshot results; dense/sparse, Unicode,
CRLF and EOF projection; search input/focus invalidation; same-revision buffer
replacement; fractional resize; in-place/vanished diagnostics; combined producer
priority; actual wrap replacement; reference reducer parity and paint-time colors.
Self-review also corrected the older collision test so its two marks really share
a pixel row and a later lower-priority mark cannot overwrite the winner.

`just profile-workloads find` now permanently includes mutation/query cache misses
and edit-update-plus-production-render workloads, not only warmed cache hits.
Both measurements use 10 warmups and 40 samples, 10,000/100,000 repeated short
lines, a 1920×1080 CPU buffer, 14-pixel font, Find query `ordinary`, and completion
and bracket matching disabled. Edits alternate one character at EOF; query misses
alternate `ordinary` and `text`. Setup, OS input dispatch and window presentation
are excluded. These are CPU timings, not application FPS or LSP latency.

The quiet repeated optimized run, after all builds/tests finished:

| 100,000-line workload | Before median / p95 | After median / p95 |
| --- | --- | --- |
| Match results after edit | 21.612 / 24.309 ms | 7.361 / 7.523 ms |
| Match results after query change | 21.476 / 22.482 ms | 7.096 / 7.843 ms |
| Warm production Find render | 3.295 / 6.395 ms | 1.248 / 1.757 ms |
| Edit update + production Find render | 25.029 / 25.478 ms | 13.560 / 13.968 ms |

At 10,000 lines, corresponding medians were **2.029 → 0.753 ms**,
**1.978 → 0.709 ms**, **1.381 → 1.146 ms**, and **3.770 → 2.702 ms**.
The first after-build run was consistent: 100,000-line medians 7.192, 6.687,
1.223 and 13.358 ms respectively. Ordinary 100,000-short-line rendering measured
0.325 ms and navigation with 10,000 undo entries 1.000 µs in the separate general
workload run; those are sanity checks, not controlled before/after claims.

A five-second native sample of the warmed `sample-find` workload collected
4,048 main-thread samples. `render_one_decoration` accounted for 2,539 inclusive
samples (about 63%), with visible-line preparation/rope line lookup prominent.
The prior document-wide overview reduction is no longer the dominant stack.
**Next profiling opportunity:** reuse/narrow visible-line preparation across
decorations in the shared text renderer. The remaining ~7 ms synchronous match
scan also still warrants consideration for guarded background work on very large
documents. Neither follow-up is implemented in this checkpoint.

Verification: `just test` **2,166 passed**, 7 skipped and 6 ignored doctests;
`just lint` passed all targets/features; formatting and diff checks passed.
Scoped diff-based self-review: **Approve**, no outstanding findings in this
change. The known optimized-only unused `revision` warning in `update/syntax.rs`
remains unrelated. No native GUI interaction was exercised. Nothing was committed
or published, and the larger handoff is still incomplete.

Logs: `/tmp/token-find-overview-{before,after,after-quiet,other-workloads,full-test,lint}.log`
and `/tmp/token-find-overview-sample.txt`. Reproduce with `just profile-workloads find`,
`just profile-workloads`, and `just profile-workloads sample-find` plus the native
sampler attached to that benchmark process.

### Movement and update-surface follow-up — 2026-09-06

Findings 5 and 8 are implemented. Twenty-four all-cursor movement wrappers and
their two iteration helpers have become one internal `move_cursors` operation
with an explicit target and Move/Extend policy. Per-cursor primitives retain
their logical/wrapped behavior. Horizontal arrows collapse nonempty selections
to the relevant edge; extending keeps anchors and updates heads; deduplication
runs once. The unsupported vertical-word message remains a no-op (or collapses
selection for non-extending movement), preserving the previous message contract.

The update layer now shares borrowing, document/page viewport adjustment,
directional reveal, blink reset and redraw. Page selection retains its previous
minimal reveal policy; ordinary paging retains directional reveal. Occurrence
and selection-history rules remain in the existing message gate. The model and
editor-update files together are 195 lines shorter than the prior checkpoint.
User-facing messages, action identities and keybindings are unchanged; this does
not merge modal text editing into document movement.

All 17 leaf message handlers are restricted to the update module tree. Their
root re-exports and unused public LSP/syntax scheduling exports are removed;
layout is private and outline is crate-private. The main `update(model, Msg)`
entry point retains special-tab routing and lifecycle cleanup. Existing
runtime/view consumers still have command execution, default-keymap creation,
palette projections, inline visibility, context-menu opening, navigation and
Problems helpers. This deliberately does not claim every mutation helper is
private or that remaining runtime/file-identity boundaries are consolidated.

New integration tests exercise all 24 target/selection combinations, forward
and reversed horizontal collapse, unsupported vertical-word policy, and desired
visual column through short wrapped rows via the main dispatcher.
Two compile-fail doctests prevent reintroducing the direct message-handler
imports. Modal rendering tests now also drive the main dispatcher. Existing
wrapped movement, multi-cursor, paging/desired-column, special-tab and lifecycle
tests provide broader regression coverage. The obsolete direct-wrapper
microbenchmark is removed; production message/update timing remains.

Verification: `just test` **2,169 passed**, 7 skipped; **2 compile-fail doctests
passed**, 6 existing doctests ignored. Strict all-target/all-feature `just lint`,
formatting and diff checks passed. Scoped diff-based self-review: **Approve**,
no outstanding findings. This is an intentional Rust API reduction, not a
user-facing command migration.

The optimized `just profile-workloads` sanity run measured cursor-update medians
of **1.042, 1.042, 1.000 and 0.958 µs** for 0, 100, 1,000 and 10,000 undo entries
(500 samples each). Ordinary 100,000-short-line rendering measured **0.303 ms**
(120 samples). The previous checkpoint was about 1 µs for navigation; no speedup
is claimed from this structural refactor. Setup/window presentation are excluded.
The pre-existing optimized-only unused `revision` warning remains. Logs:
`/tmp/token-movement-{targeted,matrix,full-test,lint,profile,fmt-check}.log`.
No native GUI interaction was exercised and nothing was committed or published.

The remaining audit priorities are live-keymap hints and runtime-owned file
identity/effects (findings 6–7), ordinary forward-edit position synchronization,
and the Find/decoration profiling opportunities above. The separate feature
roadmap and carried verification debt in `HANDOFF.md` remain unfinished.

### Shortcut-hint and keymap follow-up — 2026-09-06

Finding 6 is implemented. The loaded `Keymap`, including pending chord state,
now belongs to `UiState`: runtime dispatch, automation's binding inventory,
palette rendering and context-menu builders share the same object. User-file
loading/merging stays in runtime preparation; model defaults only parse the
embedded YAML. There is no separately synchronized hint registry or keymap copy.
All 69 static shortcut metadata entries and the static-fallback lookup are removed
from the command definitions.

Hint resolution tests the complete sequence against the same single/chord
resolution rules as dispatch. False conditions, shadowing by another command,
single-stroke prefix bindings and shorter complete chords cannot produce
misleading hints. Looking up hints does not consume a pending chord. Palette
hints use the editor context after closing the palette; context menus use their
underlying target and captured selection. Temporary popup/inline state is
excluded. Keycaps support macOS glyphs and Windows/Linux textual modifiers,
including multiple steps and a literal plus key.

The embedded YAML is the only full default-binding registry. Its parsed defaults
are cached; a deliberately minimal Save/Open/Quit emergency list handles an
invalid embedded file, with a regression test requiring the shipped YAML to
parse. User overrides retain the existing merge and `Unbound` rules. Changing
the keymap file still requires a restart; this is not hot reload.

The runtime integration test exposed two older chord defects: the YAML loader
only parsed one stroke, and the runtime's global probe consumed non-global chord
completions before its second dispatch attempt. Space-separated chord strings
now parse, and the actual keyboard path resolves each event once, then applies
its global/editor eligibility gate. Chords start in editor routing; modal/dock
special handling and single-stroke global commands retain their existing policy.
Timeout/status feedback and broader chord focus routing remain separate future
work, not claims of this checkpoint.

Eleven new regressions cover registry parity/minimal fallback, conditional and
shadowed hints, unbinding, chord prefix collisions, pending-state preservation,
palette/menu rows, platform keycaps, YAML validation and runtime user override,
chord and focus behavior. `just test`: **2,180 passed**, 7 skipped;
**2 doctests passed**, 6 ignored. Strict all-target/all-feature `just lint`
passed. Scoped diff-based self-review: **Approve**, with no outstanding findings.
No native GUI or Windows runtime was exercised. Nothing was committed/published.

The new `just profile-workloads shortcuts` workload measures resolution for every
palette row with a keyboard action, using the embedded defaults and default
editor context (10 warmups, 500 samples). It excludes model setup, keycap
construction, rendering and OS events; it is not an end-to-end latency measure.
Optimized result: **8.75 µs median / 9.25 µs p95** for the full palette's
shortcut resolution, including action-ID mapping. No before/after speedup is
claimed. The optimized build retains the pre-existing unused `revision` warning
in `src/update/syntax.rs`; all-feature strict lint passes.
Logs: `/tmp/token-shortcuts-{targeted,full-test,lint,profile,fmt-check}.log`.

Next: runtime-owned file identity/effects (finding 7), ordinary forward-edit
position migration, and remaining Find/decoration profiling. The broader inline,
settings, LSP and verification roadmap remains in `HANDOFF.md`.

### Configuration-effect follow-up — 2026-09-06

Finding 7 is **partially implemented**, not closed. `OpenConfigResource` replaces
the separate keymap-creation and duplicate focused-buffer loader effects. A
runtime-private configuration module owns path discovery/preparation and log
selection; keymap translation and request handlers no longer inspect the
filesystem. Keyboard and palette log actions agree. The public update-layer
writer and environment-dependent `config_paths::log_file` discovery API are gone.

Default keymaps use exclusive creation rather than exists-then-truncate, preserving
existing files, empty keymaps and symlink targets. Log selection uses one maximum
over matching daily-rotation/bare filenames, excluding backups and directories.
Missing logs and preparation failures produce a status error rather than a
silent no-op or an empty replacement buffer. Resource files now use ordinary
tab reuse/opening and retain unsaved content in both the origin and existing
resource documents.

Seven new regressions replace two environment-dependent tests: explicit-root
runtime fixtures cover creation, preservation, symlinks, directory errors and
log selection; dispatcher tests cover keyboard/palette routing, error visibility,
redraw requirements and unsaved-buffer/tab reuse. `just test`: **2,185 passed**,
7 skipped; **2 doctests passed**, 6 ignored. Strict `just lint` passed. Scoped
diff-based self-review: **Approve** for this incremental ownership and
buffer-preservation change; full finding 7 remains open. No native GUI or Windows
runtime was exercised. Nothing was committed or published.

No filesystem-latency improvement is claimed or benchmarked. Preparation runs in
runtime command order, synchronously, avoiding delayed focus changes. Opening
the resulting path still enters the existing synchronous tab loader; palette
log reads therefore no longer use the old background buffer-replacement loader.
This limitation must be addressed by asynchronous target-aware loading, not
treated as the completed I/O boundary. Rendering, navigation, LSP lookup and
Problems still need shared resolved identity. Post-open cursor jumps and
document/group/revision guards must survive that migration; the existing
unkeyed save/load replies also require attention.

Logs: `/tmp/token-config-resource-{targeted,test,lint,fmt,fmt-check}.log`.
The complete remaining roadmap is retained in `HANDOFF.md`.

### File-reply and saved-state follow-up — 2026-09-06

Continuing finding 7 exposed three concrete data-loss risks in the old file
effects: an unkeyed save reply marked whichever document was focused clean; an
unkeyed load reply replaced that focused buffer; Save As carried a document ID
but marked edits made during its write as saved. Its dialog itself was also
untargeted. These contracts are now document-targeted and regression-tested.

`FileRequest` carries document/revision/source-path context and a one-use token
from document-owned pending state. Save and Save As share one `SaveFile` command
and completion; path/language/LSP identity changes wait for a successful write.
Dialogs continue to target their original document across focus changes. Reloads
reject intervening edits, changed identity, superseded requests and duplicate
replies, updating/clamping all panes sharing the target without stealing focus.
A save issued after a pending read supersedes that read: accepting both could
otherwise leave a clean loaded buffer disagreeing with the subsequently written
bytes. Replacing or closing a document makes its old replies inert.

One lazily started runtime worker orders writes and explicit reads **within the
window**, streams immutable rope chunks, and drains accepted jobs on teardown
even if the UI reply receiver disappears. This removes per-write thread races
and the full-buffer String materialization from save request construction. It
does not add cross-process locking, atomic file replacement or crash durability;
file creation/write errors remain errors, and a failed write can be partial.
Filesystem stalls can delay the queue and its shutdown drain.

Save completion and undo/redo compare against the saved rope snapshot, not an
undo-stack depth that another branch can reuse. Identical rope instances have a
cheap fast path; equal-length changed buffers may require content comparison.
Ordinary edits still set the dirty flag eagerly. `didSave`'s optional text is
the saved snapshot, not a newer editor buffer. File-reply trace labels exclude
buffer contents. The shared save entry also rejects image/binary placeholders
instead of allowing their text buffer to overwrite the file; CSV's text-backed
save path remains supported.

Verification: **17 new regressions**, plus adapted existing reload/save tests
and a strengthened fake-server notification assertion. Coverage includes focus
changes, edits during save, Save As dialog/write separation, failed overlapping
saves, history branching, stale/reordered/duplicate replies, shared-pane clamps,
closed/replaced documents, read/write ordering, worker failure continuation,
teardown draining, text-free trace labels and special-tab save gating.
`just test`: **2,202 passed**, 7 skipped; **2 doctests passed**, 6 ignored.
Strict all-target/all-feature `just lint` passed.

Scoped diff-based self-review checked the changed data/command boundaries,
filesystem error paths, redraws, snapshot ownership, ordering and regression
coverage. Findings discovered during review were fixed before handoff:

| Severity | Changed path | Finding / resolution |
| --- | --- | --- |
| High | `src/model/file_io.rs` | Save-after-read could leave clean divergent bytes; saves now supersede pending reads, with a regression. |
| Medium | `src/update/mod.rs` | Debug formatting a save reply could materialize/log its buffer; trace labels now contain metadata only, with a regression. |

Verdict: **Approve** for this incremental change; no unresolved critical/high
findings in the reviewed patch. No native GUI or Windows runtime was exercised.
Nothing was committed or published.

`just profile-workloads file-io` measures two production-update CPU workloads on
a 100,000-line buffer: save snapshot + successful completion (500 samples), and
an undo/redo pair for a same-length edit near EOF (200 samples), each after ten
warmups. Model/setup work is excluded; commands are not executed, so these
exclude disk I/O, worker scheduling, LSP transport, parsing and rendering. They
are not end-to-end save latency or a before/after speedup claim.

Optimized results: save snapshot + completion **1.834 µs median / 1.917 µs p95**;
undo/redo saved-state comparison **219.250 µs median / 243.667 µs p95**.

Logs: `/tmp/token-file-io-{test,lint,profile-final,fmt,fmt-check}.log`.
The optimized build retains the pre-existing release-only unused `revision`
warning in `src/update/syntax.rs`; strict all-feature lint passes.

Still open: asynchronous config preparation and ordinary new-tab loading,
target-group/navigation continuations, and a shared resolved identity authority
for navigation, tab reuse, LSP and Problems. The worker currently handles explicit
reloads and writes, not general new-file opening or image decoding. Finding 7
and the full remaining handoff roadmap are not complete.

## Original audit scope and outcome

Reviewed the current dirty working tree, starting with its diff and the handoff,
then following editing, action dispatch, geometry, search, runtime effects and
profiling code. This is a repository audit, not a claim that every finding was
introduced by the uncommitted changes. No application code was changed, and
nothing was committed or published. The older feature roadmap was not resumed.

The existing consolidation of editing primitives, action dispatch, viewport
mapping and Find results is useful. The highest-value next consolidation is an
edit-position transformation shared by completion, ordinary edits and undo.
There are reproducible cursor bugs at that boundary, not just duplicated code.

### Refresh after the cancelable-provider checkpoint

Rechecked the current sources and reran optimized workloads after the newer
inline cancellation/Ollama changes. Both completion cursor defects below still
reproduce against the freshly built library. The structural priorities remain
unchanged; an additional public update-handler surface is documented in finding
8. No application code was changed during this refresh.

The provider recommendation at the end now reflects the implemented transports:
the small cancelable provider boundary has a concrete purpose. Hosted transports
and the other handoff features are separate from this audit.

## Ranked findings

### 1. HIGH — Completion mutations leave other cursors behind

Category: correctness and consolidation.

Sources: `src/update/completion.rs:631`, `:671`, `:742`;
`src/update/text_edits.rs:132`; `src/update/inline.rs:301`;
`src/update/editor.rs:1109`; `src/update/document.rs:2071`.

Plain completion applies replacements in reverse order and immediately records
each resulting cursor. A later replacement earlier on the same line shifts text
under already-recorded cursors without shifting those cursors. It also updates
only the focused editor, unlike the shared planned-edit path.

Two probes through the public message/update API reproduced this:

- Buffer `valueA\nval val\n`, cursors at `(1, 3)` and `(1, 7)`, trigger the
  completion menu and accept: text correctly becomes `valueA\nvalueA valueA\n`,
  but cursor columns are **6 and 10**, instead of **6 and 13**.
- Two panes share `val\nvalueA\n`, each with a cursor after `val`. Accepting
  `valueA` in the focused pane leaves the other pane's cursor at column **3**,
  instead of shifting it to **6**.

The current multi-cursor completion test uses different lines and asserts text
and undo, so it does not catch the same-line cursor defect.

**Suggested consolidation:** plan ranges against the pristine buffer, then use
one offset transformation to reconcile all cursor and selection endpoints.
Make insertion affinity, positions inside replaced ranges, active-caret placement
and selection retention explicit policies. Keep snippet caret placement and
completion query selection feature-owned. Do not simply redirect every caller
to `apply_planned_edits`: it currently collapses every selection, and inline
acceptance already has to snapshot and restore those selections around it.
Include undo/redo, peer panes, reversed selections, Unicode and overlapping
multi-cursor ranges in the contract tests.

### 2. MEDIUM — Find cache misses remain a synchronous large-document cost

Category: performance.

Source: `src/model/ui.rs:415`.

Any edit or query change rebuilds all matches and converts every match start to
a document line before caching the result. The existing workload's warmups hide
this cost: it measures cache hits, not typing with Find open.

The supplemental probe measured **21.5–21.9 ms** after a one-character edit on a
100,000-match document, and **20.9–21.1 ms** after a query change. These are search
result costs alone, not complete editor frames.

**Suggestion:** add cache-miss and typing-with-Find workloads permanently. First
separate matching from the eager overview-line projection so callers that only
need matches do not pay for both. If the remaining matching cost warrants it,
move full searches behind revision/query-guarded runtime work; preserve one
shared result contract for navigation, replacement, status and rendering.
Incremental regex search needs an explicit cross-line correctness policy.

### 3. MEDIUM — Overview marks redo document-wide work every frame

Category: performance and consolidation.

Sources: `src/view/mod.rs:400`, `src/view/editor_scrollbars.rs:16`.

Find caches matching lines, but every frame converts them to a fresh tick vector
and reduces all ticks into a new `BTreeMap` of track-pixel rows. At 100,000
matches, warm rendering is **3.08–3.12 ms**. A diagnostic build of the same
workload with `show_scrollbar=false` measured **1.16 ms**, versus **3.08 ms** with
scrollbars enabled. Native sampling independently put roughly half the sampled
CPU stacks in `render_editor_scrollbars`, plus about 5% in Find tick-vector
construction. The toggle removes all scrollbar work and changes available width
slightly, so the difference is not an exact isolated cost for the map alone.

**Suggestion:** make one overview projection consume both Find and diagnostic
marks, with a cache keyed by result/diagnostic identity, wrap generation, total
visual rows and track geometry. A track-height-sized row buffer is a simpler
reduction structure than a tree. Preserve highest-severity-wins behavior and
test resize, soft wrap, theme changes and diagnostic updates. Avoid a separate
Find-only scrollbar API.

### 4. MEDIUM — The multi-split profiler does not create independent documents

Category: validation correctness.

Sources: `src/bin/profile_render.rs:263`, `src/update/layout.rs:592`.

`SplitFocused` creates another editor for the same document. `create_model`
then overwrites that shared document's buffer for each intended input file.
All panes therefore end up showing the final buffer. Supplying CSV text also
does not activate CSV view mode. This does not invalidate the single-group
`editor_workloads` measurements, but the multi-file/mixed-mode profiler cannot
support those workload claims yet.

**Suggestion:** explicitly create and attach a document per intended file, set
special view modes through their real initialization path, and assert document
IDs and modes in a fixture smoke test. Retain same-document splits as a separate
named workload. Reuse small fixture setup helpers where they have actual shared
consumers; keep the production renderer as the drawing authority.

### 5. LOW — Movement has a 24-method combinatorial wrapper surface

Category: maintainability.

Sources: `src/model/editor.rs:1545`, `:1638`, `src/update/editor.rs:78`.

Twelve movement targets each expose selection/non-selection wrappers, while
update handlers repeat borrowing, cursor visibility, blink reset and redraw.

**Suggestion:** one internal movement entry point taking a target and selection
policy can replace the wrapper pairs. Preserve horizontal selection collapse,
smart Home, desired visual column, page scrolling and cursor deduplication as
explicit behavior. Keep existing user-facing action names compatible. Do not
force the single-line modal editor and wrapped document editor into an identical
movement engine merely because both have Left/Right operations.

### 6. MEDIUM — Shortcut hints are still a separate, drifting registry

Category: correctness and maintainability.

Sources: `src/commands.rs:173`, `:696`, `src/view/modal.rs:247`,
`src/context_menu/types.rs:81`, `src/keymap/defaults.rs:27`.

Action identity is now shared, but palette and context-menu rows still consume
static macOS-style shortcut strings. The live-keymap lookup helper has no source
callers. Consequently a user rebind/unbind is not reflected by those hints.
Embedded YAML also has a separately maintained hardcoded fallback binding list.

**Suggestion:** resolve display hints from the live keymap and action metadata
once at the runtime/model boundary, preserving conditions and platform display.
Choose either a deliberately minimal emergency fallback or a generated fallback
from one declaration; do not hand-maintain another full binding registry.

### 7. MEDIUM — File identity and filesystem effects have multiple owners

Category: architecture and maintainability; latency not measured here.

Sources: `src/update/lsp.rs:1065`, `src/update/navigation.rs:127`,
`src/update/problems.rs:64`, `src/update/app.rs:475`.

Navigation, LSP lookup and Problems scope independently compare/canonicalize
paths. Their policies differ: LSP lookup has a fallback for differently named
symlink targets, while Problems rejects different basenames before attempting
canonicalization. Config-directory/log preparation also still performs I/O in
update handlers, despite configuration save/reload having moved to commands.

**Suggestion:** establish document identity at open/save-as/rename boundaries,
with a runtime-owned canonical-path/URI mapping and explicit invalidation.
Share that identity in navigation and diagnostic scope checks. Move the remaining
directory preparation behind commands. Do not merely move filesystem calls into
a helper still invoked synchronously by every update.

### 8. LOW — Leaf update handlers expose a bypass around lifecycle guards

Category: API surface and maintainability.

Sources: `src/update/mod.rs:38`, `:63`, `:81`.

The public update module re-exports individual message handlers such as
`update_editor`, `update_document`, `update_completion` and `update_ui` alongside
the main `update` function. Those leaf entry points skip outer responsibilities:
special-tab routing, wrap refresh, menu invalidation, and inline-session
reconciliation. Repository runtime, automation and benchmark callers use the
main dispatcher for these messages; the leaf handler callers found are internal
to the library, including unit tests. This is an unnecessary public contract,
not evidence of a current runtime bypass bug.

**Suggested consolidation:** make leaf handler exports crate-private and expose
`update(model, Msg)` as the normal mutation entry point. Preserve genuinely
consumed cross-crate helpers such as palette row resolution, inline visibility,
and runtime navigation until their callers are deliberately migrated. Keep the
internal feature modules; reducing public entry points does not require merging
their implementations into one file.

## Performance measurements

Apple M2 Max, macOS 15.6; optimized Cargo bench build, no F2 overlay.
No benchmark/test build was deliberately run concurrently with timing samples.
Numbers are ranges of medians from two runs, not confidence intervals or a new
before/after comparison. Scheduling noise is visible in render p95 values.

| Workload | Current median |
| --- | ---: |
| Cursor update, empty history | 0.96–1.13 µs |
| Cursor update, 10,000 undo entries | 0.96–1.08 µs |
| Render 100,000 short lines | 0.327–0.356 ms |
| Render wrapped 1,000,000-character line | 1.85–1.98 ms |
| Render unwrapped 1,000,000-character line | 0.205–0.214 ms |
| Warm Find render, 10,000 matches | 1.37–1.40 ms |
| Warm Find render, 100,000 matches | 3.08–3.12 ms |
| Find cache hit, 100,000 matches | under 0.2 µs |
| Find after edit, 10,000 matches | 2.06–2.08 ms |
| Find after edit, 100,000 matches | 21.5–21.9 ms |
| Find after query change, 100,000 matches | 20.9–21.1 ms |

The second-run p95 was 1.00 µs for navigation with 10,000 history entries,
2.65 ms for the wrapped million-character line, 3.43 ms for warm Find rendering
at 100,000 matches, and 23.1 ms for Find after an edit.

Rendering uses the real editor-group renderer, a 1920×1080 pixel buffer,
1920×1060 group, JetBrains Mono 14, 8.4-pixel character width and 20-pixel rows.
The repository harness excludes model/font/cache construction and performs
10 warmups, 500 movement samples, 120 render samples and 40 Find samples.
The supplemental search probe uses 5 warmups and 40 samples, alternating a
one-character insertion/removal at EOF or the queries `ordinary` and `text`.
It includes cache rebuilding and result retrieval but excludes rendering.
It is linked to the current optimized bench library with opt-level 3/thin LTO.

These are CPU measurements, not end-to-end FPS. Window scheduling, presentation,
native UI interaction, cross-platform behavior and heap profiling were not
measured. No new claim is made about startup or provider/network latency.

### Refreshed measurements on the current provider checkpoint

Same machine and workload setup; one fresh run of each workload, with builds and
tests excluded from the timing intervals. These are current-state measurements,
not a controlled before/after comparison or evidence of a provider regression.

| Workload | Median | p95 |
| --- | ---: | ---: |
| Cursor update, empty history | 0.958 µs | 1.125 µs |
| Cursor update, 10,000 undo entries | 1.000 µs | 1.166 µs |
| Render 100,000 short lines | 0.313 ms | 0.357 ms |
| Render wrapped 1,000,000-character line | 1.851 ms | 2.091 ms |
| Render unwrapped 1,000,000-character line | 0.197 ms | 0.222 ms |
| Warm Find render, 10,000 matches | 1.408 ms | 1.682 ms |
| Warm Find render, 100,000 matches | 3.303 ms | 6.178 ms |
| Find after edit, 10,000 matches | 2.108 ms | 2.213 ms |
| Find after edit, 100,000 matches | 21.806 ms | 22.628 ms |
| Find after query change, 100,000 matches | 21.212 ms | 22.042 ms |

Fresh native sampling (5 seconds, requested 1 ms interval) again puts roughly
55% of main-thread samples under scrollbar rendering. The cold search cost is
still larger than a 16.7 ms frame budget before rendering is included. The
warm-cache numbers alone should not be used to predict typing latency.

The supplemental probe was rebuilt from the existing `probe.rs` against
`target/release/deps/libtoken-18ed21e9da416c95.rlib` using Rust 2021, opt-level 3
and thin LTO. It reconfirmed cursor columns `[6, 10]` instead of `[6, 13]`, and
the peer cursor remaining at column 3 instead of 6. Temporary refresh logs are
`/tmp/token-audit-refresh-{workloads,find,probe}.log`; native stacks are in
`/tmp/token-audit-refresh-find.sample.txt`. Exact timing reproduction is subject
to machine load; these temporary artifacts are not durable regression tests.

## Reproduction and recommended sequence

```sh
just profile-workloads
just profile-workloads find
just profile-workloads sample-find
just test
```

Temporary raw logs, native sample and supplemental Rust probes are at
`/tmp/token-audit-20260906.6Is5mr/`. The probes are outside the repository; their
fixtures and important outcomes are described above because temporary files
are not durable regression coverage. The release build emitted the existing
unused `revision` warning at `src/update/syntax.rs:192`.

Verification in the original audit: `just test` passed **2,113 tests**, with
7 skipped and 6 ignored doctests. `just lint`, `just fmt-check` and
`git diff --check` passed. The completion defects above were reproduced by
supplemental probes despite the existing suite being green; regression tests
for those exact scenarios still need to be added with the fix.

Refresh verification: `just test` passed **2,127 tests**, with 7 skipped and
6 ignored doctests. `just lint`, `just fmt-check` and `git diff --check` passed.
The suite is green despite the two independently reproduced cursor defects;
this is not approval of those code paths.

Recommended order: lock down edit/cursor invariants and fix the reproduced
completion defects; repair profiler fixtures; optimize cold Find and overview
projection; then simplify movement, shortcut metadata and file identity.
The handoff's inline cancellation/provider work remains separate feature work.

Keep `Renderer` as orchestrator and keep messages, effects and user actions
distinct. Large files alone do not justify splitting them. The implemented
llama.cpp/Ollama shared HTTP client and cancelable provider contract should stay
small; they do not warrant another routing or plugin framework.

| Priority | Severity | Area | Disposition |
| --- | --- | --- | --- |
| 1 | HIGH | Completion reconciliation | Reproduced defects fixed; ordinary-edit migration remains |
| 2 | MEDIUM | Cold Find | Preserve measured miss workload; optimize shared results |
| 3 | MEDIUM | Overview marks | Consolidate and cache track projection |
| 4 | MEDIUM | Profiler fixtures | Fixed and verified with independent documents and CSV/TSV modes |
| 5 | LOW | Movement wrappers | Consolidate target plus selection policy |
| 6 | MEDIUM | Shortcut hints | Derive from live keymap |
| 7 | MEDIUM | File identity/effects | Share identity and move I/O behind commands |
| 8 | LOW | Public leaf update handlers | Keep one normal public mutation entry point |

Original review verdict: **Request changes** for the reproduced completion
correctness defects, now addressed by the implementation follow-up above. The
other items are scoped follow-up recommendations, not a request for a
repository-wide rewrite.
