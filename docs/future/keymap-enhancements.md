# Keymap Enhancements

> **Status:** Future work  
> **Prerequisite:** Configurable Keymapping System (completed December 2024)  
> **Related:** [archived/KEYMAPPING_IMPLEMENTATION_PLAN.md](../archived/KEYMAPPING_IMPLEMENTATION_PLAN.md)

## Overview

The configurable keymapping system is complete and working. This document tracks potential enhancements for future releases.

---

## Hot-Reload Keymap Files

**Priority:** Medium  
**Effort:** Medium (requires file watcher integration)

Automatically reload keymap when the configuration file changes:

- Watch `~/.config/token-editor/keymap.yaml` for modifications
- Watch project-local `keymap.yaml` if present (new feature: only `~/.config/token-editor/keymap.yaml` is loaded today)
- Reload and re-merge bindings without restart
- Show status bar notification on reload

**Dependencies:**
- `notify` crate for file system watching (already a dependency; extend the existing watcher in `src/runtime/file_watch.rs` / `src/fs_watcher.rs`)

**Implementation notes:**
- Add file watcher to `App` struct
- On change event, re-run `load_keymap_file` + `merge_bindings` (mirroring `KeymapSnapshot::parse` in `src/keymap/preferences.rs`, the Settings→Keymap reload path fed by `src/runtime/keymap_settings.rs`) and rebuild `Keymap`
- Handle parse errors gracefully (keep old keymap, show error)

---

## Chord Sequences

**2026-09-06 checkpoint:** user-defined space-separated chord strings now parse
and dispatch in editor routing, and shortcut hints resolve complete sequences.
Example: `key: "ctrl+k ctrl+c"`. Global-command chords now also route from
dialogs, docks and CSV cell editing (`5b9a502`, 2026-09-08). Command eligibility
is checked before matching each prefix, so editor-only bindings cannot capture
input in those surfaces. The proposed default chord set, timeout and status-bar
feedback below remain future work; this does not make every editor command a
global action.

**Priority:** Low  
**Effort:** Medium (chord infrastructure exists, but none of the commands below do yet)

Define default multi-key chord sequences (all four commands must be added to `src/keymap/command.rs` first; none exist today):

| Chord | Command | Description |
|-------|---------|-------------|
| `Cmd+K Cmd+C` | CommentLines | Toggle line comments |
| `Cmd+K Cmd+U` | UncommentLines | Remove line comments |
| `Cmd+K Cmd+D` | CompareWithClipboard | Diff selection with clipboard |
| `Cmd+K Cmd+K` | ToggleBookmark | Toggle bookmark at cursor |

**Implementation notes:**
- Infrastructure already exists: `KeyAction::AwaitMore`, `pending_chord_display()`
- Add chord timeout (e.g., 1.5s) to abandon incomplete sequences
- Show pending chord in status bar (`pending_chord_display()` exists but is not called by the view yet)
- Add the proposed default sequences to `keymap.yaml`

**YAML syntax example:**
```yaml
- key: "cmd+k cmd+c"
  command: CommentLines
```

---

## Chord Timeout Handling

**Priority:** Low  
**Effort:** Low

Handle abandoned chord sequences gracefully:

- After pressing `Cmd+K`, if no follow-up key within timeout, reset chord state
- Configurable timeout (default: 1500ms)
- Visual feedback in status bar showing countdown or pending state

**Implementation notes:**
- Add `chord_started_at: Option<Instant>` to `Keymap`
- Check timeout in `about_to_wait()` event loop
- Call `keymap.reset()` on timeout

---

## Selection-Clearing Refactor

**Priority:** Low  
**Effort:** Medium

Move selection-clearing logic from `src/runtime/input.rs` into editor movement handlers:

**Current state:**
- Navigation commands (Home, End, PageUp/Down, etc.) clear selection in `src/runtime/input.rs` before dispatching the movement message
- This creates a split between keymap (binding) and `src/runtime/input.rs` (behavior)

**Proposed:**
- Add `clear_selection` flag to movement `EditorMsg` variants, OR
- Create "smart" movement commands that handle selection collapse internally
- Benefits: Single source of truth, testable without `handle_key`

**Affected commands:**
- `MoveCursorLineStart` / `MoveCursorLineEnd` (Home/End)
- `MoveCursorDocumentStart` / `MoveCursorDocumentEnd` (Ctrl+Home/End)
- `MoveCursorWordLeft` / `MoveCursorWordRight` (Alt+Arrow)
- `PageUp` / `PageDown`
- `MoveCursor(Up)` / `MoveCursor(Down)` when selection exists

---

## Additional Ideas

### Keymap Profiles
- Support named profiles (e.g., "vim", "emacs", "vscode")
- Switch profiles via command palette
- Load from `~/.config/token-editor/keymaps/vim.yaml`

### Keymap Editor UI

Tracked in [Settings Keymap Tab](../archived/settings-keymap.md): merged binding search,
chord capture/rebinding, conflict detection, override persistence and base-keymap
selection. That plan remains active after Settings v1 archival.

### Platform-Specific User Configs
- Support `keymap.macos.yaml`, `keymap.linux.yaml`, etc.
- Auto-select based on platform

---

## See Also

- [Configurable Keymapping (archived)](../archived/KEYMAPPING_IMPLEMENTATION_PLAN.md) - Original implementation plan
- [KEYBINDINGS.md](../KEYBINDINGS.md) - Current user-facing keybinding reference
- [KEYMAPPING.md](../archived/KEYMAPPING.md) - Original user-facing keymapping documentation
