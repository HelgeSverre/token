# Keymapping Implementation Plan

> **Status:** Phase 11 Complete (All Phases Done)  
> **Last Updated:** December 2024  
> **Related:** [KEYMAPPING.md](./KEYMAPPING.md)

## Executive Summary

This document outlines a phased migration from the current hardcoded key handling in `input.rs` to the configurable keymap system described in KEYMAPPING.md. The approach prioritizes safety (no behavior regressions) over speed.

## Implementation Progress

### Completed ✅

- **Phase 1-3**: Core keymap module (`src/keymap/`) with types, commands, bindings
- **Phase 4**: Bridge integration - keymap tried first for simple commands
- **Phase 5**: CommandId → KeymapCommand mapping for palette keybinding lookup
- **Phase 6**: Context system with `KeyContext` and `Condition` for conditional bindings
- **Phase 9**: User configuration with layered loading and merge support
- **YAML Config**: `keymap.yaml` at project root with 74 default bindings (embedded at compile time)
- **74 tests** covering keymap functionality including context-aware bindings and merge logic

### User Configuration (Phase 9)

User-configurable keymaps with layered loading:

- **Loading order** (each layer overrides the previous):
  1. Embedded default keymap (compiled into binary)
  2. `keymap.yaml` in current directory (project-local overrides)
  3. User config at `~/.config/token-editor/keymap.yaml`

- **`merge_bindings()`** combines base + user bindings:
  - Same keystroke + conditions → user binding replaces base
  - `command: Unbound` → removes matching default bindings
  - Different conditions → both bindings coexist

- **`get_user_config_path()`** returns platform-appropriate path:
  - Unix: `~/.config/token-editor/keymap.yaml`
  - Windows: `%APPDATA%\token-editor\keymap.yaml`

### Context System (Phase 6)

The context system enables bindings that activate only under certain conditions:
- `KeyContext` struct captures: `has_selection`, `has_multiple_cursors`, `modal_active`, `editor_focused`
- `Condition` enum: `HasSelection`, `NoSelection`, `HasMultipleCursors`, `SingleCursor`, `ModalActive`, `ModalInactive`, `EditorFocused`
- YAML supports `when: ["condition1", "condition2"]` for conditional bindings
- Tab and Escape now use context-aware bindings in `keymap.yaml`

### Architecture Flow

```
┌─────────────────────────────────────────────────────────────────┐
│  WindowEvent::KeyboardInput (app.rs)                            │
│  ├─ Modifier tracking (ctrl/shift/alt/logo)                     │
│  ├─ Option double-tap detection                                 │
│  └─ Keymap dispatch (if not modal and not option double-tap)    │
└─────────────────────────────────────────────────────────────────┘
                              │
              ┌───────────────┴───────────────┐
              ▼                               ▼
┌──────────────────────────┐    ┌──────────────────────────┐
│  Keymap System           │    │  handle_key() fallback   │
│  ├─ keystroke_from_winit │    │  (input.rs)              │
│  ├─ lookup_with_context  │    │  ├─ Modal routing        │
│  └─ Command::to_msgs()   │    │  ├─ Option double-tap    │
│     → dispatch directly  │    │  └─ Complex behaviors    │
└──────────────────────────┘    └──────────────────────────┘
```

### Still in input.rs (complex/special cases)

1. **Option double-tap for multi-cursor**: Temporal pattern (not a chord), bypasses keymap when `option_double_tapped && alt`
2. **Arrow keys with selection collapse**: Up/Down jump to selection start/end, then move
3. **PageUp/Down with selection**: Normalize to selection start/end first
4. **Navigation clears selection**: Home/End, Ctrl+Home/End, Alt+Arrow, Cmd+Arrow clear selection before moving
5. **Modal key handling**: `handle_modal_key()` routes keys when modal is active
6. **Character input**: Regular typing flows through input.rs

### Test Coverage

- 547 total tests passing
- 74 keymap-specific tests (includes 8 merge tests)
- 9 modal isolation tests
- Context-aware Tab and Escape tests

---

## Remaining Work

### Phase 7: Complex Behaviors Migration (Partial)

**Status:** Deferred - Current implementation works correctly

The following behaviors remain in `input.rs` because they require imperative logic that doesn't fit cleanly into the declarative keymap system:

| Behavior | Reason for input.rs |
|----------|---------------------|
| Arrow key selection collapse | Requires checking selection, mutating cursor position, clearing selection, THEN moving |
| PageUp/Down normalization | Similar multi-step mutation before navigation |
| Option double-tap | Temporal pattern detection (300ms window) |
| Navigation clears selection | `clear_selection()` call before dispatching move command |

**Decision:** Keep these in input.rs as fallback. The keymap handles 90%+ of bindings cleanly.

### Phase 8: Modal Integration (Deferred)

**Status:** Not planned

Modal input (`handle_modal_key()`) remains separate. The current architecture properly isolates modal input from editor keybindings. Unifying them would add complexity without clear benefit.

### Phase 9: User Configuration ✅

**Status:** Complete

Implemented user-configurable keymaps with layered loading:
- `get_user_config_path()` returns `~/.config/token-editor/keymap.yaml` (Unix) or `%APPDATA%\token-editor\keymap.yaml` (Windows)
- `merge_bindings()` combines base + user bindings with override semantics
- `command: Unbound` removes matching default bindings
- Layered loading: embedded defaults → project keymap.yaml → user config

**Not implemented:** Hot-reload on file change (deferred - requires file watcher)

### Phase 10: Chord Support (Future)

**Status:** Infrastructure complete, no default chords defined

The keymap system supports multi-key chords:
- `KeyAction::AwaitMore` returned when chord prefix detected
- Status bar can show pending chord via `pending_chord_display()`
- Timeout handling needed for chord abandonment

No default chords are currently defined. Could add:
- `Cmd+K Cmd+C` for comment
- `Cmd+K Cmd+U` for uncomment

### Phase 11: Cleanup ✅

**Status:** Complete

Cleaned up `input.rs` to remove redundant code now handled by keymap:

- **Removed 350+ lines** of redundant match arms from input.rs
- Removed: numpad shortcuts, undo/redo, save, clipboard, select all, duplicate, select next/unselect occurrence, modals (command palette, goto line, find), layout (splits, tabs, focus groups), tab/shift-tab indent, escape, expand/shrink selection, all selection navigation (shift+arrows)
- **Kept** only:
  - Modal input routing (`handle_modal_key`)
  - Option double-tap for multi-cursor creation (temporal gesture)
  - Navigation with selection collapse (Home, End, Ctrl+Home/End, Alt+Arrow, PageUp/Down, Arrow Up/Down with selection)
  - Character input (regular typing)
- Updated tests in `main.rs` to use message system instead of `handle_key` directly
- `input.rs` reduced from ~477 lines to ~220 lines (54% reduction)
- 546 tests passing

---

## Keymap YAML Reference

### File Location

- **Default:** `keymap.yaml` at project root (embedded at compile time)
- **Project-local:** `keymap.yaml` in current working directory (overrides defaults)
- **User config:** `~/.config/token-editor/keymap.yaml` (highest priority)

### Modifier Keys

| Modifier | Description |
|----------|-------------|
| `cmd` | Platform command key (Cmd on macOS, Ctrl on Windows/Linux) |
| `ctrl` | Control key |
| `shift` | Shift key |
| `alt` | Alt/Option key |
| `meta` | Explicit meta key (Cmd on macOS, Win on Windows) |

### Conditional Bindings

```yaml
- key: "tab"
  command: IndentLines
  when: ["has_selection"]

- key: "tab"
  command: InsertTab
  when: ["no_selection"]
```

### Available Conditions

| Condition | Description |
|-----------|-------------|
| `has_selection` | Text is selected |
| `no_selection` | No text selected |
| `has_multiple_cursors` | Multiple cursors active |
| `single_cursor` | Only one cursor |
| `modal_active` | A modal dialog is open |
| `modal_inactive` | No modal dialog |
| `editor_focused` | Editor has focus (not modal) |

### Platform-Specific Bindings

```yaml
- key: "meta+left"
  command: MoveCursorLineStart
  platform: macos
```

---

## Command Reference

### Navigation Commands

| Command | Description |
|---------|-------------|
| `MoveCursorUp/Down/Left/Right` | Basic cursor movement |
| `MoveCursor*WithSelection` | Movement extending selection |
| `MoveCursorLineStart/End` | Line navigation |
| `MoveCursorDocumentStart/End` | Document navigation |
| `MoveCursorWordLeft/Right` | Word navigation |
| `PageUp/Down` | Page navigation |
| `PageUp/DownWithSelection` | Page navigation with selection |

### Editing Commands

| Command | Description |
|---------|-------------|
| `InsertNewline` | Insert line break |
| `InsertTab` | Insert tab character |
| `DeleteBackward/Forward` | Delete single character |
| `DeleteWordBackward/Forward` | Delete word (Option+Backspace/Delete) |
| `DeleteLine` | Delete entire line |
| `IndentLines/UnindentLines` | Adjust indentation |
| `Duplicate` | Duplicate line/selection |

### Selection Commands

| Command | Description |
|---------|-------------|
| `SelectAll` | Select entire document |
| `SelectWord/Line` | Select at cursor |
| `ClearSelection` | Collapse selection to cursor |
| `ExpandSelection/ShrinkSelection` | Progressive selection |
| `SelectNextOccurrence` | Multi-occurrence selection |
| `CollapseToSingleCursor` | Remove secondary cursors |

### File/UI Commands

| Command | Description |
|---------|-------------|
| `SaveFile` | Save current file |
| `Undo/Redo` | History navigation |
| `Copy/Cut/Paste` | Clipboard operations |
| `ToggleCommandPalette` | Open command palette |
| `ToggleGotoLine` | Open goto line dialog |
| `ToggleFindReplace` | Open find/replace |

### Layout Commands

| Command | Description |
|---------|-------------|
| `NewTab/CloseTab` | Tab management |
| `NextTab/PrevTab` | Tab navigation |
| `SplitHorizontal/Vertical` | Create split |
| `FocusNextGroup/PrevGroup` | Navigate splits |
| `FocusGroup1-4` | Direct split focus |

---

## Success Criteria

### Current Status ✅

- [x] 90%+ shortcuts via keymap
- [x] Palette shows correct bindings from keymap
- [x] All 546 tests pass
- [x] Context-aware Tab and Escape work correctly
- [x] Option double-tap still works for multi-cursor
- [x] Modal input properly isolated
- [x] User-configurable keymap file with layered loading
- [x] `command: Unbound` to disable default bindings
- [x] Redundant input.rs code removed (54% line reduction)

### Future Milestones

- [ ] Hot-reload keymap on file change
- [ ] Chord sequences (Ctrl+K Ctrl+C style)
- [ ] Move remaining selection-clearing logic into editor movement handlers
