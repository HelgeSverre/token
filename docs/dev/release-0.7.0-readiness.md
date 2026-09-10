# 0.7.0 release preparation — 2026-09-10

Preparation only: no release tag or publication is authorized. The version and
changelog release commit stays local while non-release changes run through CI.

## Changes from this check

- CI, platform builds and benchmarks now target `main`, the actual default
  branch, instead of `master`. CI also supports manual dispatch.
- The native file-watcher test waits for a real setup event before exercising
  replacement, checks exact event paths, and identifies the failing phase. The
  replacement operations remain single-shot with the same five-second deadline.
  Ten hardened isolated runs passed. The earlier timeout was not reproduced;
  this hardens setup and assertions, rather than proving a production watcher
  failure.
- Sema parameter hints default off, with an Inlay hints switch under Settings →
  Editor. Explicit Run output is independent. Completion, diagnostics, semantic
  colors and the existing mouse-hover timing retain their defaults.
- Settings → LSP executable rows offer Configure buttons. They open config.yaml
  through the existing ordered file worker, creating defaults only for a missing
  file. Existing files, including empty ones, are preserved.
- The [language-server guide](../user/language-servers.md) covers installation
  expectations, paths, arguments, server options, reload/restart and quieter
  preferences. It distinguishes server features from inline AI providers.
- A live Go check found that `gopls` could report Ready without receiving
  `didOpen`: Go was missing from the LSP language-ID map. The mapping is fixed,
  with a registry consistency assertion and an opt-in real-provider hover and
  completion test. The test failed before the fix and passed afterward.

## Verification

- Final local verification after the Go fix: 2,698 tests passed, seven opt-in
  tests skipped; two doctests passed, six ignored; strict Clippy passed. Both
  selected opt-in real-provider tests also passed in a separate run.
- Both baseline watcher stress suites passed (2,696 tests each); the hardened
  watcher also passed in the later full preference suite.
- `actionlint` passed for CI, Build and Bench workflows.
- An optimized macOS ARM build and an unsigned cargo-bundle application were
  produced under the normal `target/` tree. The bundled executable launched with
  isolated configuration and fixture files.
- In that running bundle, rust-analyzer initialized and supplied the expected
  error for an intentionally invalid fixture. Folding reduced visible rows,
  soft wrap retained folds, split panes retained independent collapse state,
  and docked Find reported four expected matches.
- Idle auto-save wrote both the active and background fixture documents.
  Filesystem readback confirmed EditorConfig trailing-whitespace cleanup and
  final newlines.
- The live Sema test passed semantic tokens, parameter hints and an explicit
  evaluation result. The live Go hover/completion test passed after the sync fix
  with gopls v0.18.1 and Go 1.26.3 on macOS ARM.
- The macOS bundle was rebuilt after the fix at source revision `38e94bf`.
  Its native automation API returned `fmt.Println` documentation and exactly
  `Print`, `Printf` and `Println` for the `fmt.Pr` completion prefix. The isolated
  application then quit normally.
- The LSP Settings renderer was inspected with Configure actions and a long
  executable path. This was a headless render, not a physical pointer test.

Repeat the real-provider checks using installed executables:

```sh
GOPLS_SMOKE_BINARY=/path/to/gopls SEMA_SMOKE_BINARY=/path/to/sema \
  just test '-E "test(gopls_live_hover_and_completion) | test(sema_live_async_document_features_and_run)" --run-ignored only'
just test
just lint
```

## Before publishing

- Obtain green hosted CI and all four packaging jobs on the final non-release
  source revision; then recheck the exact release candidate if source changes.
  The source checks are [CI](https://github.com/HelgeSverre/token/actions/runs/34483137098)
  and [Build](https://github.com/HelgeSverre/token/actions/runs/34483137003) at
  `38e94bf`; they were started by pushing the non-release commits to `main`.
- Perform a controlled physical focus-loss auto-save check, trackpad scrolling
  with folding/Find, scrollbar dragging, and LSP Configure pointer interaction.
  Native computer-use startup failed for both Token and Finder in this session;
  semantic automation does not substitute for these OS/pointer checks.
- Launch the resulting Linux/Windows and Intel macOS packages on those systems.
  Successful cross-compilation/packaging alone is not native interaction coverage.
- Session restore retains saved-file layout, not unsaved-buffer recovery. The
  pre-existing lack of a dirty-close/quit confirmation remains a separate safety
  limitation; opt-in auto-save is not a replacement for either feature.

Local fixtures and rendered checks are under
`target/verification/release-0.7.0/`; they are not release assets or committed
source. No temporary Cargo target directory was used.
