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
- Watch project-local `keymap.yaml` if present
- Reload and re-merge bindings without restart
- Show status bar notification on reload

**Dependencies:**
- `notify` crate for file system watching (already planned for workspace feature)

**Implementation notes:**
- Add file watcher to `App` struct
- On change event, call `load_default_keymap()` and rebuild `Keymap`
- Handle parse errors gracefully (keep old keymap, show error)

---

## Chord Sequences

**Priority:** Low  
**Effort:** Low (infrastructure exists)

Define default multi-key chord sequences:

| Chord | Command | Description |
|-------|---------|-------------|
| `Cmd+K Cmd+C` | CommentLines | Toggle line comments |
| `Cmd+K Cmd+U` | UncommentLines | Remove line comments |
| `Cmd+K Cmd+D` | CompareWithClipboard | Diff selection with clipboard |
| `Cmd+K Cmd+K` | ToggleBookmark | Toggle bookmark at cursor |

**Implementation notes:**
- Infrastructure already exists: `KeyAction::AwaitMore`, `pending_chord_display()`
- Add chord timeout (e.g., 1.5s) to abandon incomplete sequences
- Show pending chord in status bar
- Add to `keymap.yaml` with chord syntax

**YAML syntax example:**
```yaml
- key: ["cmd+k", "cmd+c"]
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

Move selection-clearing logic from `input.rs` into editor movement handlers:

**Current state:**
- Navigation commands (Home, End, PageUp/Down, etc.) clear selection in `input.rs` before dispatching the movement message
- This creates a split between keymap (binding) and input.rs (behavior)

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
- In-app keybinding editor
- Search for commands, see current binding, rebind
- Conflict detection

### Platform-Specific User Configs
- Support `keymap.macos.yaml`, `keymap.linux.yaml`, etc.
- Auto-select based on platform

---

## See Also

- [Configurable Keymapping (archived)](../archived/KEYMAPPING_IMPLEMENTATION_PLAN.md) - Original implementation plan
- [KEYMAPPING.md](../feature/KEYMAPPING.md) - User-facing keymapping documentation (if exists)
