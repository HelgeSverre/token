# Refactoring and production-path profiling — 2026-09-05

This pass preserves the uncommitted soft-wrap/inline/config work described in
`HANDOFF.md`. Nothing was committed or published. The older feature roadmap is
not part of this completion claim.

## Changes implemented

- **Navigation:** one `EditorArea::focused_document_and_editor_mut` borrow seam
  replaces document/history clones in movement and selection handlers.
- **Find:** a shared memoized result set serves navigation, replacement, status,
  decorations and overview marks. Its key checks rope identity, revision, query,
  case/word/regex options and effective selection scope. Rope identity handles
  reloads and direct buffer replacement even at the same revision. Results and
  regex errors are shared; scrollbar line projections are calculated once.
  Decorations binary-search the offset range before converting visible matches
  to positions, including matches that span the entire viewport.
- **Wrapped rendering:** visual-row text and column conversion share
  `VisibleTextLine`; selections, brackets, glyphs, cursors and range decorations
  no longer materialize the whole logical line for every wrapped segment. Ghost
  visibility is checked only on the cursor's visual row.
- **Viewport ownership:** resize/font changes compute actual split geometry and
  delegate capacity calculation to `sync_all_viewports`. Removed unused,
  misleading line/visual-row conversions and the no-wrap `usize::MAX` segment
  length sentinel.
- **Editing API:** `Cursor`, `Position` and `Selection` have one implementation
  in `editable`, re-exported by the model. Removed the unused `Msg::TextEdit`
  router, `EditContext`, `TextEditMsg` and unused `RopeBuffer`. Real modal/CSV
  `EditableState<StringBuffer>` editing remains. Document-specific text
  extraction stays in the model.
- **Actions:** palette metadata owns its keyboard action mapping; common
  palette actions use keyboard message dispatch. Bindable command declarations
  generate their string parser, replacing a second 123-name registry. Registry
  lookup no longer allocates a temporary vector. Context-menu and panel-toggle
  differences remain explicit; effects (`Cmd`) and user keybindings remain
  separate concepts.
- **Edit effects:** document editing, completion, inline acceptance and planned
  LSP edits share syntax/LSP/redraw finalization. Feature planners still own
  mutation, undo batching and cursor policy.
- **Configuration effects:** saves, reloads, and theme loading now run in the
  runtime and return result messages. Save errors reach the status bar. Saves
  execute in order rather than competing writer threads. This moves I/O out of
  these update handlers; it does not make disk operations asynchronous.
- **Profiling:** `profile_render` and loop benchmarks now use the production
  renderer, replacing copied/fake drawing code. Reports label CPU scope rather
  than application FPS. The completion keystroke benchmark times the insert,
  not insert/backspace/dismiss/reopen. `editor_workloads` preserves the audit
  workloads for future regressions.

The shared primitives preserve the document-facing constructors. Internal
editable callers now use `Cursor::at`, `Selection::new` for a collapsed range,
and `Selection::from_anchor_head` for two positions. Removed APIs had no runtime
producers/consumers; their tracked source remains recoverable in Git.

## Measurements

Apple M2 Max, macOS 15.6; optimized release build, no F2 overlay. Both versions
used the same production editor-group harness, not the former simulated render
profiler. Window buffer: 1920×1080; group rect: 1920×1060; JetBrains Mono 14,
8.4px cell width, 20px row height. Completion/bracket matching disabled for the
navigation workload. Font/model/cache construction is excluded; 10 warmups,
500 movement samples, 120 rendering samples, 40 Find samples.

Numbers below are ranges of the medians from two runs, not confidence intervals.

| Workload | Before | After |
|---|---:|---:|
| Cursor update, empty undo history | 0.92–0.96 µs | 0.96–1.00 µs |
| Cursor update, 10,000 history entries | 245–260 µs | 1.00 µs |
| Render 10,000 ordinary short lines | 0.303–0.305 ms | 0.306–0.309 ms |
| Render 100,000 ordinary short lines | 0.305–0.307 ms | 0.303–0.305 ms |
| Render wrapped 100,000-character line | 2.26–2.40 ms | 1.69–1.76 ms |
| Render wrapped 1,000,000-character line | 6.23–7.15 ms | 1.70–1.86 ms |
| Render unwrapped 1,000,000-character line | 0.296–0.326 ms | 0.191–0.192 ms |
| Find render, 10,000 matches | 9.02–9.07 ms | 1.37–1.47 ms |
| Find render, 100,000 matches | 89.41–89.49 ms | 3.07–3.15 ms |
| Find result retrieval, 100,000 matches | 6.79–6.98 ms | 0.083 µs, cache hit |

The repeated after-run p95 was 1.08 µs for navigation with 10,000 history
entries, 1.76 ms for the million-character wrapped line, and 3.24 ms for the
100,000-match Find render. The first after-run briefly overlapped a separate
40-frame profiler smoke test; the second ran without that overlap.

Before-change native samples attributed about 75% of the long wrapped-line
sample to `get_line_cow` (about 52% at `memmove`). Find samples showed repeated
regex searches and document-wide offset-to-cursor conversions. The isolated
10,000-entry document clone cost 243–257 µs, explaining nearly all the old
navigation time. The borrowed primitive is about 0.17 µs, but is **not** the
whole update operation.

These are warmed CPU editor-group measurements, **not end-to-end FPS**. They
exclude window scheduling, surface presentation and modal paint. The Find
cache-hit result does not measure the initial search or invalidation cost.
Ordinary-line performance is approximately unchanged; improvements target the
measured pathological workloads.

## Reproduction and validation

```sh
just test
just lint
just profile-workloads
just profile-workloads find
just release
target/release/profile_render --frames 500 --splits 3 --stats
```

`profile-workloads` is an optimized Cargo bench executable with ordinary timing
output; `sample` and `sample-find` hold a workload for 12 seconds for native
sampling. Timing assertions are deliberately not part of the test suite.
The historical two-run comparison used the same source linked directly to the
release library with thin LTO and panic-abort; Cargo's bench profile may differ
in panic/debug settings. Re-run both versions under the same recipe for a new
comparison. The synthetic undo fixture is never undone: its purpose is to vary
the amount of retained history during movement.

Raw original/after timing logs and native samples from this session are in
`/tmp/token-review-profile.sumtod` (temporary, not a durable artifact). The
workload source and this summary are kept in the repository.

Regression coverage includes Find cache reuse/invalidation, Unicode offsets,
invalid regex, viewport-spanning matches, config result handling, registry
identity/parsing and existing full-versus-dirty rendering pixel tests.

Final verification: `just fmt`, `just test` (2,108 passed, 7 skipped;
6 doctests ignored), `just lint` (all targets/features) and `git diff --check`
passed. The reduced test count comes from deleting tests of retired APIs while
adding regression coverage for the replacement seams. Release build and both
repository workload modes were exercised. Diff-based self-review found no
new blocking issues in this pass; verdict: **Approve** with the follow-ups
below, not a review of unrelated pre-existing work.

## Remaining opportunities

1. Find still rebuilds the entire result set after an edit/query change and
   traverses all matching lines to reduce scrollbar marks to track pixels.
   The remaining 100,000-match frame is about 3 ms versus 1.4 ms at 10,000.
   Profile cold search, typing with Find open and track projection before
   adding incremental search or another cache. A result cache retains a rope
   snapshot and match offsets, so memory use should be measured too.
2. Movement wrappers still expose separate selection/non-selection methods.
   A target-plus-selection-policy API can replace them, but must preserve
   horizontal selection collapse, page scrolling and multi-cursor merging.
   Shared primitives and history-free borrowing landed first.
3. Undo/cursor reconciliation remains feature-owned. This pass shares effects,
   not a universal mutation planner. Completion, inline acceptance and replace
   need a separate contract-focused consolidation of multi-pane cursor/undo
   semantics before merging those code paths.
4. Remaining update-time filesystem work includes config-directory/keymap
   preparation and some path/metadata checks. Theme discovery also still
   happens when constructing picker state. These should become scoped runtime
   effects; do not claim the entire update layer is now I/O-free.
5. `benches/hot_paths.rs` still contains copied implementation comparisons;
   treat these as illustrative microbenchmarks, not production baselines.
   Loop benchmarks now use real drawing but include fixture setup/font loading
   in their multi-operation workload scope. Use the warmed workload probes for
   isolated frame comparisons.

No native UI interaction, cross-platform execution or memory profiler was run
in this pass. The existing release-only unused `revision` warning in
`src/update/syntax.rs` remains unrelated to these changes.
