# Completion refactor progress

## Baseline

- Branch: `refactor/completion-sessions`
- Branch starting SHA: `6ccaae63e2f4605a231251bb1ecaa929e3d8b5be`
  (orb setup committed on top of main)
- Upstream baseline and merge base: `origin/main` at `9102ea0`
- Initial worktree: clean
- The audited implementation was still current: completion state was split
  across `UiState`, menu selection and delayed work used overlay rows or labels,
  completion popup keys were intercepted before the keymap, and multi-cursor
  LSP acceptance could fall back to plain text.
- This orb did not retain a same-configuration pre-refactor benchmark run.
  Historical benchmark documents therefore remain context, not a quantitative
  before/after comparison for this branch.

The shell did not initially inherit Cargo's binary directory. All repository
recipes below were run after sourcing `$HOME/.cargo/env`.

## Phases 0–6

### Phase 0 — characterization

Mapped the existing menu, path, LSP resolve, commit-character, inline, keymap,
pointer, transaction and runtime paths. Existing tests already covered most of
the behavior contract. Added focused coverage where identity, actual keymap
dispatch, strict edit validation and combined multi-cursor plans could otherwise
regress.

### Phase 1 — ownership and identity

- `src/completion/session.rs` owns `CompletionState` and typed, checked
  `SessionId`, `RequestId` and `CandidateId` allocation/correlation.
- `UiState::completion` is the sole storage owner for menu/path/commit and inline
  lifecycle state. The temporary field projection used during migration was
  removed.
- `CompletionMenuState` owns stable candidate selection and scroll identity.
  Filtering keeps IDs; source refreshes conservatively rematch semantic
  candidates, including equal-label candidates with different edits.
- Menu scheduling and runtime replies carry the originating session. Inline
  snapshots carry both session and request identity. Dismissed sessions cannot
  be revived by late replies.

### Phase 2 — interaction policy

- `src/completion/interaction.rs` derives menu visibility, inline visibility,
  pending-session state, acceptance eligibility and explicit/automatic trigger
  admission from model facts.
- Pending menus with no rows do not capture editing keys or suppress inline
  suggestions. Visible menu presentation takes precedence over inline
  presentation.
- Inline visibility includes master/inline enablement and provider/session
  identity rather than trusting ghost or overlay presence.

### Phase 3 — keymap and pointer routing

- Added named menu accept, dismiss, next/previous and page actions to the normal
  command registry, conditions and default `keymap.yaml` bindings.
- Removed completion-specific navigation and acceptance from the imperative
  overlay interceptor while retaining unrelated overlay routing.
- Real dispatch tests cover defaults, a user Tab override, hidden pending
  sessions and modified keys. Pointer selection/acceptance resolves a stable
  candidate ID and uses the same reducer as keyboard acceptance.

### Phase 4 — guarded acceptance boundary

- `src/update/completion/accept.rs` prepares completion edit plans around the
  existing `PlannedEdit`, `EditOffsetMap` and shared transaction machinery.
- Deferred resolve and commit intent identify an exact session/candidate rather
  than a row. Navigating while resolve is in flight cannot accept a different
  candidate.
- Commit characters remain immediate and participate in one undo transaction;
  stale or rejected semantic acceptance preserves the literal exactly once.

### Phase 5 — strict completion edits

- Completion-only LSP range conversion rejects malformed UTF-16 positions and
  out-of-bounds ranges instead of clamping them.
- The complete primary/additional-edit plan is validated before mutation;
  incompatible overlap rejects atomically with a nonmodal status.
- Narrow multi-cursor LSP query replacement replicates only across compatible
  empty selections with matching query text. Shared additional edits are
  applied once. Complex ranges or incompatible cursor sites are rejected rather
  than flattened into plain text.
- Local words, flattened snippets, paths and active-caret-only inline acceptance
  retain their previous policies.

### Phase 6 — consolidation

- Removed the temporary `UiState` completion-field projection; consumers now
  address the explicit `ui.completion` owner.
- Updated the current autocomplete contract and the Unreleased changelog.
- Updated direct-state fixtures, automation, screenshot fixtures and completion
  benchmarks for the new identities and ownership path.

### Review follow-up

- Bound ordinary deferred acceptance to the originating document, editor,
  revision, cursor, selection and history state. Tab or pane changes now dismiss
  the pending menu, and late resolve replies cannot edit the new focus target.
- Preserve each LSP response's request position and source-prefix coordinate
  basis. Carried `textEdit` and additional-edit ranges are translated after
  supported prefix growth or backspace and rejected when translation is
  ambiguous.
- Require a primary LSP edit to be single-line and contain its request position,
  and require every cursor selection to be empty before replicating an LSP edit.
  Rejection remains atomic and preserves commit-character literals exactly once.

## Current source map

| Responsibility | Source of truth |
| --- | --- |
| Session/request/candidate identity and lifecycle storage | `src/completion/session.rs` |
| Candidate data, stable selection and rematching | `src/completion/menu.rs` |
| Pure visibility, acceptance and trigger policy | `src/completion/interaction.rs` |
| Menu lifecycle and acceptance orchestration | `src/update/completion.rs` |
| Guarded edit normalization/application | `src/update/completion/accept.rs` |
| Commit-character staged intent | `src/update/completion/commit.rs` |
| Path lifecycle | `src/update/completion/paths.rs` |
| Inline lifecycle | `src/update/inline.rs` |
| Named actions and conditions | `src/commands.rs`, `src/keymap/`, `keymap.yaml` |
| Existing atomic edit engine (reused) | `src/update/text_edits.rs` |

## Intentional behavior changes

1. Menu completion keys are normally rebindable and unbindable through the
   existing keymap. Defaults remain equivalent.
2. Candidate identity, not label or row index, controls selection, resolution
   and acceptance across filtering and refresh.
3. Invalid, overlapping or unsupported multi-cursor LSP completion plans are
   rejected atomically instead of silently dropping protocol edits. A commit
   character that initiated a rejected plan remains inserted once.

Interactive snippet sessions, replacement prediction, LSP `inlineCompletion`,
streaming and a generic source-plugin registry remain deliberately deferred.

## Verification record

- `cargo check --all-targets --all-features`: passed.
- `just test-one completion`: 231 passed before the final commit-character
  rejection regression; that focused regression also passed.
- `just test-one inline`: 98 passed before the final typed-request cleanup; 96
  focused inline tests passed after that cleanup.
- `just test-one keymap`: 106 passed.
- `just fmt` and `just fmt-check`: passed.
- `just test`: 2823 nextest tests passed, 3 skipped; doctests passed (2 passed,
  6 ignored). The final run used `CARGO_BUILD_JOBS=1` after this 4 GiB orb
  thrashed while compiling the library and test harness concurrently; test
  scope was unchanged.
- `just lint`: passed after replacing one Clippy-suggested early return with
  `?`.
- `just bench-completion`: passed in optimized mode. Full output is at
  `target/verification/completion-refactor-final-benchmark.txt`. No valid
  same-orb baseline exists, so no percentage regression claim is made.
- `just smoke-input`: the release build passed, but native automation could not
  start because this headless Linux orb has neither `WAYLAND_DISPLAY`,
  `WAYLAND_SOCKET` nor `DISPLAY`. Output is at
  `target/verification/completion-refactor-smoke-input.txt`. macOS, IME and a
  display-backed Linux session were not exercised.

The build emits pre-existing signedness/fallthrough warnings from the vendored
tree-sitter Blade C scanner. They do not originate in this refactor.

## Current phase

Phases 0–6 and review findings 1–3 are complete.
