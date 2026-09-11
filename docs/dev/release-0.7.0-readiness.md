# 0.7.0 release preparation — 2026-09-11

Preparation only. No release tag or publication is authorized.
Exact-candidate cross-platform verification is **deferred at the user's request**,
not passed or waived. This is the current checklist; earlier implementation-stage
reports remain in Git history.

## Implemented

- Language-server code assistance, Go synchronization, and Sema support.
  Sema parameter hints default off; AI inline suggestions remain opt-in.
- Separate Settings with compact controls and two-pane LSP/AI record editors:
  Add, presets, rename, removal, validation, persistence and affected-server
  reconfiguration. Presets create ordinary editable records.
- AI forms cover supported transports, endpoint/model/credential variables,
  generation/prompt/context settings and managed local server options. Saving
  does not silently enable suggestions. Connection testing is not exposed;
  installation of servers/models remains the user's responsibility.
- Offline Mermaid and Markdown highlighting assets, including licenses and
  provenance. No CDN is required for renderer assets; document-authored remote
  content is a separate matter.
- Unsaved-close confirmation, ordered saves, external-change protection,
  auto-save, saved-file session restore, folding and pixel scrolling.
- Windows WiX packaging and executable resources, replacing the experimental
  MSI backend.
- Completion acceptance refreshes bracket decoration after final caret placement,
  including other plain-text panes showing the same document. The reported
  selected-letter effect was stale bracket paint, not a real selection.

## Local verification

- Before the bracket fix, the final two-pane Settings implementation passed
  **2,726 tests**, seven skipped; two doctests passed, six ignored; strict Clippy
  passed. Logs are under
  `target/verification/settings-redesign/collection-verified-tests.log` and
  `collection-verified-lint.log`.
- The PHP bracket regression passes for `cur` and `curr`, both split panes, and
  disabled matching. It uses production LSP-item conversion and acceptance with
  a deterministic item, not a live PHP server.
- Final bracket-fix verification passed: **2,727 tests**, seven skipped; two
  doctests passed, six ignored; strict all-target/all-feature Clippy and formatting
  passed. Nextest run: `b9df500a-0e08-4a5f-91b1-f8d223f76f46`.
- Latest shared-JS native-handler smoke: all eight checks passed against the
  local debug application, report
  `target/verification/input-smoke/run-7bxIH2/report.json`. Coverage includes
  fractional Find/folding scrolling, both scrollbar drags/release, focus/capture,
  Find hit testing, active/background focus-loss saving and cancelled dirty-close
  safety. It predates the bracket fix and is not an optimized-performance result.
- Earlier optimized bundle checks passed Go hover/completion, Sema semantic
  tokens/hints/explicit evaluation, EditorConfig cleanup, folding/splits, Find and
  auto-save. Offline preview rendered five diagram types and highlighted Rust
  with renderer network access blocked. These verify those revisions, not the
  exact final candidate.

Fixtures remain under the regular `target/verification/` tree. Settings captures
use the production renderer, not OS file dialogs or physical pointer delivery.

## Remaining publication gates

1. **Deferred: exact-candidate hosted CI and packaging.** Latest successful
   [standard CI](https://github.com/HelgeSverre/token/actions/runs/34491028145)
   was `235093e`; [four-target packaging](https://github.com/HelgeSverre/token/actions/runs/34491767089)
   was `531a854`. These excluded the local 0.7.0 bump and subsequent Settings,
   offline-preview, unsaved-close and automation changes. Verify the final
   revision before publishing.
2. **Native interaction smoke.** Check final packaged Settings forms, executable
   browsing, trackpad behavior and real OS focus loss. Launch Linux, Windows and
   Intel macOS packages on those systems. Bridge input does not verify physical
   devices or OS event delivery.
3. **Windows upgrade/downgrade and distribution.** The successful MSI check
   installed five payload files, compared hashes/resources, uninstalled and
   confirmed executable removal. That package identified Token 0.6.0.
   Upgrade/downgrade and interactive installer flows remain unverified. MSI
   currently exists as a Build artifact; cargo-dist's release workflow does not
   attach it. Decide whether MSI is a 0.7.0 release download and connect that
   workflow if so.
4. **Final release metadata.** Cargo manifests already use 0.7.0. The changelog
   is consolidated under `v0.7.0 - 2026-09-11`, including later feature work.
   Refresh its date if publication is later and include subsequent application
   changes. Keep release preparation local until publication is authorized.

## Separate or optional work

- UI component gallery/theme changes are at inventory/discussion stage; see
  [UI inventory](ui-component-inventory.md). No showcase is implemented.
- AI connection-test UI, font-family pickers and crash recovery are optional
  future work. Saved-file sessions do not recover unsaved buffers after a crash;
  confirmation and auto-save do not claim otherwise.
- The separate Sema SDK is deferred in the Sema packages repository.

## Repeatable checks when verification resumes

```sh
just test
just lint
just smoke-input
# Optional real-provider checks with installed binaries:
GOPLS_SMOKE_BINARY=/path/to/gopls SEMA_SMOKE_BINARY=/path/to/sema \
  just test '-E "test(gopls_live_hover_and_completion) | test(sema_live_async_document_features_and_run)" --run-ignored only'
```

Follow `AGENTS.md` for release preparation and tagging. Preparation never
authorizes publication.
