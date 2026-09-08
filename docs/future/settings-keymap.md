# Settings Keymap Tab

> **Status:** Implementation committed 2026-09-08; cross-platform validation remains.
> **Extracted:** 2026-09-06 from Phase 4 of the [archived Settings v1 plan](../archived/settings-page.md).
> **Priority:** P3 · **Effort:** M · **Milestone:** 6 - Productivity

Settings v1 provides searchable preset controls and LSP state. Archiving that
completed scope does not complete the separate keybinding editor described here.

The keymap foundations were consolidated on 2026-09-07: `e017bd6` gives bindable
names and defaults one source of truth; `4a00fa5` shares binding eligibility across
single strokes, complete chords and prefixes, and accepts the documented sidebar
condition. Reuse these implementations and existing merge semantics. These
commits do not implement any of the Settings-tab UI or persistence scope below.

## Scope

- [x] Add a Keymap tab with the merged embedded and user `keymap.yaml` bindings,
  using the existing merge semantics, shared `binding_chips` and fuzzy search.
- [x] Detect conflicts between bindings, accounting for context, platform and
  chord prefixes rather than comparing display strings alone.
- [x] Add chord capture for rebinding, with cancellation and an explicit commit.
- [x] Persist only overrides to the user `keymap.yaml`, not `config.yaml`.
- [x] Design and add a base-keymap preset choice using the Settings chip pattern.

Reuse shared overlay geometry and one filtered ordering authority for rendering,
keyboard actions, mouse actions and automation. Keep file I/O behind runtime
effects. The preset-only v1 controls are not a substitute for chord capture or
its validation/error handling.

## Verification checkpoints

- [ ] Merged bindings and displayed chips agree across platforms and contexts.
- [x] Filtered-row actions mutate the displayed binding, including chord sequences.
- [x] Conflict checks distinguish overlapping from disjoint contexts and test
  exact binding collisions and chord-prefix ambiguity.
- [x] Capture cancellation does not change the keymap; successful saves survive
  reload. Invalid/unreadable existing files and failed writes are handled safely.
- [x] Base-keymap changes have defined user-override precedence and do not silently
  discard overrides. Cover the chosen behavior with tests before exposing it.
- [x] Native keyboard/capture and pointer checks, narrow-window rendering, full
  tests and strict lint pass before archival.

## 2026-09-07 implementation checkpoint

`b6bad14` independently commits the pure preferences layer: canonical command
enumeration, shared sequence parsing, optional base selection, conflict masks,
override transformations and bounded startup reads. Its independent suite passed
2,167 tests and two doctests, strict lint and formatting. It does **not** include
the Settings UI or file-worker persistence; those still depend on uncommitted
Settings v1, configuration/file-opening and runtime work.

The working-tree UI keeps the separate Settings page and adds a Keymap category
alongside the original category navigation. It uses shared filtered rows/keycaps,
four-stroke capture, explicit Save/Cancel and a Literal-next control. Recorded
keys occupy the header so narrow-row clipping cannot hide them. The Token/Common
base chips preserve user overrides; Common (`conventional`) changes only three
shortcuts, not a complete third-party keymap. Disk effects remain on the ordered
file worker, with exact-text stale checks, advisory locking and atomic replacement.
Comments/formatting are not preserved; linked/read-only/invalid files are refused.

macOS native checks covered tab clicks, capture/cancel, reserved Cmd+Q rejection,
a 400-pixel window, a pointer Save, immediate use of the new Ctrl+K Ctrl+S binding,
restart persistence and native-menu restoration. They found and fixed two gaps:
AppKit menu accelerators bypassed winit capture, and modal pointer handlers
discarded runtime commands. The latter now has a shared effect-preserving path.
Windows/Linux native checks and their platform-specific merged/chip behavior are
not established by this macOS run. Keep this plan active until those platform
checks are complete. The dependency-ordered source
grouping was closed by `ab96495`–`2bdbcca` on 2026-09-08; UI/override code is
in `c2a1ee5`, with shared runtime wiring in `ac362d4`.

The related [keymap enhancements](keymap-enhancements.md) plan owns timeout,
pending-chord feedback, hot reload and broader routing work. Coordinate those
changes instead of creating a second keymap engine.

## 2026-09-08 Linux verification checkpoint

An isolated Debian ARM64 X11 editor verified Ctrl keycaps, capture cancellation,
pointer Save, override persistence after restart, and opaque 400-pixel Settings
layout. Native checks exposed Alt-containing chords being mistaken for a bare
Alt double-tap; `c104ed9` fixes that without changing the gesture timing.
The saved Ctrl+Alt+K Ctrl+Alt+S sequence opened Settings from the editor both
immediately and after restart. Final Linux/macOS full tests and strict lint passed.
See the [native record](../dev/refactoring-audit-2026-09-06.md#linux-native-verification-and-portability-fixes--2026-09-08).
This does not verify every focus context, Wayland or Windows. Keep the plan active.

The follow-up context check exposed global chords being discarded outside the
editor and Settings missing global classification. `5b9a502` fixes both through
one filtered keymap resolver. Native Linux X11 checks now cover opening Settings
from terminal, command palette, file explorer and CSV cell editing; a temporary
editor-only text chord did not swallow ordinary field/terminal input. Full
macOS/Linux tests and strict lint passed. See the
[context record](../dev/refactoring-audit-2026-09-06.md#global-shortcuts-across-input-contexts--2026-09-08).
This plan still awaits the remaining platform/context verification.
