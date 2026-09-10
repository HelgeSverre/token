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

- [Hosted CI](https://github.com/HelgeSverre/token/actions/runs/34491028145)
  passed on non-release revision `235093e`. [Packaging](https://github.com/HelgeSverre/token/actions/runs/34491767089)
  passed on `531a854` for all four targets, including native Windows MSI
  installation, payload/resource checks and uninstall. The packaging blocker is
  closed; see the Windows follow-up below. Recheck the final release revision
  before publishing: these hosted builds exclude the local version bump and
  automation-input changes.
- The input-handler checks below now cover focus-loss saving, scrolling, drag
  capture and Find hit testing through the running app's bridge. OS event
  delivery, perceived trackpad smoothness and native LSP Configure pointer
  interaction remain unverified; native computer-use startup was unavailable.
- Launch the resulting Linux/Windows and Intel macOS packages on those systems.
  Successful cross-compilation/packaging alone is not native interaction coverage.
- Session restore retains saved-file layout, not unsaved-buffer recovery.
  Dirty-close/quit confirmation is covered by the unsaved-change follow-up below;
  crash recovery remains separate from both confirmation and opt-in auto-save.

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

## Windows MSI follow-up

`9c973d9` fixes the two failures identified in the earlier Windows build:

- cargo-bundle 0.9.0 uses resource filenames verbatim as MSI `File` keys and
  Component `KeyPath` identifiers. Rename `Inter-OFL.txt` to `Inter_OFL.txt` in
  both bundle metadata blocks, cargo-dist's includes and the README link. The
  license is byte-identical; it is not removed from distribution.
- Resize the executable's icon to 256 pixels before ICO encoding. Previously
  the oversized image aborted resource creation before `winres` could embed
  either the icon or version metadata.

The first native [verification run](https://github.com/HelgeSverre/token/actions/runs/34487928868)
on non-release revision `9d9ed1f` successfully generated the MSI without the icon
warning. However, its new install smoke check failed with Windows Installer error
1620: the experimental backend's output was not accepted as a valid package.
Successful archive generation was therefore not sufficient to close this blocker.

`606b3f6` replaces that backend with WiX Toolset 3.14, already installed on the
hosted Windows runner. The shared PowerShell packaging script builds only Token,
takes the exact generated ICO path from Cargo's build-script output, and invokes
WiX compilation/linking with validation enabled. The package contains the
executable, Credits, MIT license and both font licenses, plus a standard directory
selection dialog and Start menu shortcut. It has a stable upgrade identity.
WiX linking also caught a redundant `ARPNOMODIFY` property already supplied by
the standard dialog; `78c909a` removes that duplicate.

The Windows Build job installs the MSI to an isolated path containing spaces,
compares all five installed payload files against their source hashes, checks
embedded product/version metadata, and uninstalls the package. Installer logs are
retained as CI artifacts even if the smoke check fails. The smoke script is
intended only for disposable Windows runners.

`just bundle-windows` now calls the same WiX packaging script as CI. The old ZIP
recipe could rename a host-native executable to `token.exe`; it has been removed.

Local verification passed: 2,699 tests, seven skipped; two doctests, six ignored;
strict Clippy, formatting, actionlint, PowerShell syntax parsing and WiX schema
validation using .NET's schema validator. The system libxml validator could not
compile WiX's schema regexes; native WiX linking/validation remained enabled.

Final [packaging verification](https://github.com/HelgeSverre/token/actions/runs/34491767089)
on non-release revision `531a854` passed all four targets. Windows installed the
MSI successfully, verified all five payload hashes and embedded Token 0.6.0
product/version resources, then uninstalled successfully and confirmed the
executable was removed. [Standard CI](https://github.com/HelgeSverre/token/actions/runs/34491028145)
passed on `235093e`; the only subsequent implementation change was removal of the
duplicate WiX property. Interactive installer dialogs, upgrade scenarios and
Windows application interaction were not exercised by this quiet-install check.
WiX emitted ICE61 because same-version replacement is explicitly enabled; this
avoids registering rebuilt packages of the same three-part version side by side.
Upgrade/downgrade behavior still needs its own verification before shipping.

The verified MSI is saved locally at
`target/verification/windows-msi-artifact/x86_64-pc-windows-msvc/release/bundle/msi/Token.msi`.
Its metadata identifies an x64/en-US package built by WiX 3.14.1.8722.

The verification branch excluded the local version bump and automation-input
changes. MSI files remain Build workflow artifacts; the release-publishing
workflow is unchanged. No release tag or publication was made.

## Offline preview follow-up

Markdown now embeds the pinned Mermaid 11.17.2 full browser bundle and
Highlight.js 11.9.0 with its stylesheet. Library files remain byte-identical to
their distributions, with provenance/checksums and licenses under
`vendor/markdown/`. Both licenses are included in archive/app/MSI packaging.
Prose-only documents omit the libraries; diagram-only documents omit Highlight.js.

The production HTML renderer passed a headless Chrome check with DNS resolution
disabled and a restrictive network-blocking CSP. Five diagrams (flowchart,
sequence, state, mindmap, mathematical labels) rendered; one deliberately invalid
diagram retained its source and explanation. Rust highlighting rendered, with no
external script/stylesheet elements or blocked resource attempts. The fixture,
check harness and screenshot are in `target/verification/offline-preview/`.
Document-authored remote images and raw HTML are outside this renderer-asset check.

The 12 targeted renderer tests, strict lint and full suite passed (2,700 tests,
seven skipped; two doctests passed, six ignored). Final release-candidate
verification still needs to include the subsequent safety and Settings work.

## Unsaved-change follow-up

Tab, group, keyboard/menu Quit and native window-close requests now share one
confirmation gate. Save/Save All waits for the existing asynchronous file pipeline,
including untitled Save As dialogs. Failed/cancelled saves preserve tabs; changes
made after confirmation are rechecked. Shared document views avoid unnecessary
prompts, while pane-local CSV drafts are committed before saving or retained when
they cannot be applied. Conflicting drafts for the same CSV cell stop closing.

Verification: 2,709 tests passed, seven skipped; two doctests passed, six ignored;
strict lint and optimized build passed. This includes nine focused safety tests,
one of which reaches the real writer through the native close-event handler.
The confirmation uses the same row/layout primitives as file-conflict dialogs;
its production-renderer screenshot is `target/verification/closing/confirmation.png`.

The updated native-handler smoke passed in
`target/verification/input-smoke/run-dlDXft/`: fractional scrolling with Find and
folding, both scrollbar drags, focus/capture, hit testing, active/background
auto-save, and a cancelled close followed by an ordinary save without exiting.
An initial concurrent run timed out during window startup; the unchanged release
binary passed on retry after the full suite finished. No timeouts or host security
settings were changed. These are synthetic handler checks, not physical trackpad
or OS focus-delivery coverage. Settings and final cross-platform release gates
remain pending; no release was published.
