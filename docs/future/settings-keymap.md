# Settings Keymap Tab

> **Status:** Future work, not implemented.
> **Extracted:** 2026-09-06 from Phase 4 of the [archived Settings v1 plan](../archived/settings-page.md).
> **Priority:** P3 · **Effort:** M · **Milestone:** 6 - Productivity

Settings v1 provides searchable preset controls and LSP state. Archiving that
completed scope does not complete the separate keybinding editor described here.

## Scope

- [ ] Add a Keymap tab with the merged embedded and user `keymap.yaml` bindings,
  using the existing merge semantics, shared `binding_chips` and fuzzy search.
- [ ] Detect conflicts between bindings, accounting for context, platform and
  chord prefixes rather than comparing display strings alone.
- [ ] Add chord capture for rebinding, with cancellation and an explicit commit.
- [ ] Persist only overrides to the user `keymap.yaml`, not `config.yaml`.
- [ ] Design and add a base-keymap preset choice using the Settings chip pattern.

Reuse shared overlay geometry and one filtered ordering authority for rendering,
keyboard actions, mouse actions and automation. Keep file I/O behind runtime
effects. The preset-only v1 controls are not a substitute for chord capture or
its validation/error handling.

## Verification checkpoints

- [ ] Merged bindings and displayed chips agree across platforms and contexts.
- [ ] Filtered-row actions mutate the displayed binding, including chord sequences.
- [ ] Conflict checks distinguish overlapping from disjoint contexts and test
  exact binding collisions and chord-prefix ambiguity.
- [ ] Capture cancellation does not change the keymap; successful saves survive
  reload. Invalid/unreadable existing files and failed writes are handled safely.
- [ ] Base-keymap changes have defined user-override precedence and do not silently
  discard overrides. Cover the chosen behavior with tests before exposing it.
- [ ] Native keyboard/capture and pointer checks, narrow-window rendering, full
  tests and strict lint pass before archival.

The related [keymap enhancements](keymap-enhancements.md) plan owns timeout,
pending-chord feedback, hot reload and broader routing work. Coordinate those
changes instead of creating a second keymap engine.
