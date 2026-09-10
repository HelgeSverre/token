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

- [Hosted CI](https://github.com/HelgeSverre/token/actions/runs/34483137098)
  passed on `38e94bf`. [Packaging](https://github.com/HelgeSverre/token/actions/runs/34483137003)
  passed for Linux and both macOS targets. Windows compiled, but MSI generation
  failed because cargo-bundle rejected `Inter-OFL.txt` as a Component `KeyPath`.
  Windows also warned that the 512-pixel icon exceeded the ICO encoder's
  256-pixel limit. Resolve these packaging issues and recheck the final source
  revision before publishing.
- The input-handler checks below now cover focus-loss saving, scrolling, drag
  capture and Find hit testing through the running app's bridge. OS event
  delivery, perceived trackpad smoothness and native LSP Configure pointer
  interaction remain unverified; native computer-use startup was unavailable.
- Launch the resulting Linux/Windows and Intel macOS packages on those systems.
  Successful cross-compilation/packaging alone is not native interaction coverage.
- Session restore retains saved-file layout, not unsaved-buffer recovery. The
  pre-existing lack of a dirty-close/quit confirmation remains a separate safety
  limitation; opt-in auto-save is not a replacement for either feature.

Local fixtures and rendered checks are under
`target/verification/release-0.7.0/`; they are not release assets or committed
source. No temporary Cargo target directory was used.

## Automation input follow-up

The [input bridge](automation-input.md) now accepts bounded event sequences and
uses the actual window-event handler plus shared editor/scrollbar geometry.
An initial single-event API allowed real mouse movement to replace the pointer
between automation calls: the trace showed a move to Find at `(525, 55.5)` followed
by a wheel event at the native position `(304.39, 309.11)` over editor text.
Grouping move+wheel/press in one dispatch addresses that demonstrated race
without suppressing native input or bypassing hit testing.

`just smoke-input` passed, followed by three consecutive successful repeats.
Each verified fractional pixel scrolling with Find/folding, Find-bar wheel
routing, both scrollbar drags and release, capture cancellation on focus loss,
text hit testing below Find, and active/background focus-loss saves with disk
readback. The existing save test now enters through the bridge; one additional
test covers pixel/line behavior, invalid-sequence rejection and bounds. Final
local tests: **2,699 passed**, seven skipped; two doctests passed, six ignored;
strict Clippy passed. MCP initialization and tools/list exposed the new typed
`input` schema successfully. These additions remain local with the release
preparation; hosted results above predate them.

The rebuilt macOS `.app` also passed the same complete smoke check. Its local
JSON report is `target/verification/input-smoke/run-k7tiNB/report.json`. All five
final-API runs exited normally; the native mouse-interleaving failures above
were from the superseded single-event API, not retried into a passing result.
