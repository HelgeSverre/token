# Configurable Gesture Bindings

**Status:** Planned
**Created:** 2025-12-19
**Effort:** M (3-5 sessions)

---

## Overview

Enable configurable keybindings for temporal gestures like double-tap modifier keys. Currently, `Option+Option+Arrow` (double-tap Option then Arrow) is hardcoded for adding cursors above/below. This feature makes such gestures configurable through the keymap system.

### Goals

1. **Configurable double-tap bindings** — Express `alt+alt+up` in keymap.yaml
2. **Support all modifiers** — Alt, Ctrl, Shift, Cmd can all have double-tap gestures
3. **Clean syntax** — `key: alt+alt+down` instead of conditions
4. **Backward compatible** — Existing hardcoded behavior becomes default binding

### Non-Goals (This Phase)

- Configurable timing window (hardcode 300ms)
- Triple-tap detection
- Hold-to-repeat gestures
- Modifier-only bindings (double-tap must end with a key)

---

## Current Implementation

### Detection (`src/runtime/app.rs:404-421`)

```rust
let is_option_key = matches!(
    event.physical_key,
    PhysicalKey::Code(KeyCode::AltLeft) | PhysicalKey::Code(KeyCode::AltRight)
);

if is_option_key {
    if event.state == ElementState::Pressed && !event.repeat {
        let now = Instant::now();
        if let Some(last) = self.last_option_press {
            if now.duration_since(last) < Duration::from_millis(300) {
                self.option_double_tapped = true;
            }
        }
        self.last_option_press = Some(now);
    } else if event.state == ElementState::Released {
        self.option_double_tapped = false;
    }
}
```

### Handling (`src/runtime/input.rs:70-75`)

```rust
Key::Named(NamedKey::ArrowUp) if alt && option_double_tapped => {
    update(model, Msg::Editor(EditorMsg::AddCursorAbove))
}
Key::Named(NamedKey::ArrowDown) if alt && option_double_tapped => {
    update(model, Msg::Editor(EditorMsg::AddCursorBelow))
}
```

### Why It Bypasses the Keymap

The keymap system is declarative and cannot express temporal constraints. The current code explicitly skips the keymap when `option_double_tapped && alt` is true.

---

## Proposed Design

### YAML Syntax

```yaml
# Double-tap modifier expressed in key specification
- key: alt+alt+up
  command: AddCursorAbove

- key: alt+alt+down
  command: AddCursorBelow

# Works with all modifiers
- key: ctrl+ctrl+d
  command: DuplicateLine

- key: cmd+cmd+k
  command: ToggleBookmark
```

The parser recognizes `modifier+modifier` (same modifier twice) as a double-tap pattern.

### Extended Keystroke

```rust
// src/keymap/types.rs

pub enum ModifierKey {
    Alt,
    Ctrl,
    Shift,
    Cmd,
}

pub struct Keystroke {
    pub key: KeyCode,
    pub mods: Modifiers,
    pub double_tap_mod: Option<ModifierKey>,  // NEW
}
```

### Extended KeyContext

```rust
// src/keymap/context.rs

pub struct KeyContext {
    // Existing...
    pub has_selection: bool,
    pub modal_active: bool,

    // Gesture states (NEW)
    pub alt_double_tap: bool,
    pub ctrl_double_tap: bool,
    pub shift_double_tap: bool,
    pub cmd_double_tap: bool,
}
```

### Gesture State Tracking

```rust
// src/runtime/app.rs

#[derive(Default)]
struct GestureState {
    last_press: Option<Instant>,
    double_tapped: bool,
}

struct App {
    // Replace single field with per-modifier tracking
    alt_gesture: GestureState,
    ctrl_gesture: GestureState,
    shift_gesture: GestureState,
    cmd_gesture: GestureState,
}
```

---

## Implementation Plan

### Phase 1: Generalize Gesture Detection
**File:** `src/runtime/app.rs`

- Add `GestureState` struct with `last_press` and `double_tapped`
- Track all 4 modifiers: Alt, Ctrl, Shift, Cmd
- Update keyboard event handler to detect double-tap for each

### Phase 2: Extend KeyContext
**File:** `src/keymap/context.rs`

- Add `alt_double_tap`, `ctrl_double_tap`, `shift_double_tap`, `cmd_double_tap` fields
- Wire up from App gesture state when creating context

### Phase 3: Extend Keystroke
**File:** `src/keymap/types.rs`

- Add `ModifierKey` enum
- Add `double_tap_mod: Option<ModifierKey>` to Keystroke
- Update `Keystroke::new()` and related constructors

### Phase 4: Update Config Parser
**File:** `src/keymap/config.rs`

- Parse `alt+alt+key` syntax
- Detect repeated modifier (e.g., `alt+alt`) and set `double_tap_mod`
- Handle all combinations: `ctrl+ctrl`, `cmd+cmd`, `shift+shift`

### Phase 5: Update Keymap Matching
**File:** `src/keymap/keymap.rs`

- When matching, check if `double_tap_mod` requirement is satisfied
- Compare against corresponding `KeyContext` gesture field

### Phase 6: Remove Hardcoded Handling
**Files:** `src/runtime/app.rs`, `src/runtime/input.rs`

- Remove `skip_keymap` check for `option_double_tapped`
- Remove hardcoded ArrowUp/ArrowDown handling in input.rs
- Let keymap handle it naturally

### Phase 7: Add Default Bindings
**File:** `keymap.yaml`

```yaml
- key: alt+alt+up
  command: AddCursorAbove
- key: alt+alt+down
  command: AddCursorBelow
```

### Phase 8: Testing

- Unit tests for gesture detection timing
- Unit tests for YAML parsing of double-modifier syntax
- Integration tests for keymap matching with gesture context
- Verify backward compatibility (default behavior unchanged)

---

## Edge Cases

| Scenario | Behavior |
|----------|----------|
| Alt+Alt+Alt+Up | Third press within window keeps double_tapped true |
| Alt release before arrow | double_tapped becomes false, binding doesn't match |
| Alt+Ctrl+Up during alt_double_tap | Only matches if binding requires both (unlikely) |
| Very fast typing 301ms+ | Gesture expires, treated as normal Alt+Up |

---

## Files to Modify

| File | Changes |
|------|---------|
| `src/runtime/app.rs` | Generalize gesture tracking to all modifiers |
| `src/keymap/types.rs` | Add `ModifierKey`, `double_tap_mod` to Keystroke |
| `src/keymap/context.rs` | Add gesture state fields to KeyContext |
| `src/keymap/config.rs` | Parse `alt+alt+key` syntax |
| `src/keymap/keymap.rs` | Match double-tap bindings against gesture state |
| `src/runtime/input.rs` | Remove hardcoded gesture handling |
| `keymap.yaml` | Add `alt+alt+up/down` default bindings |

---

## Success Criteria

1. `alt+alt+up` and `alt+alt+down` work as before (default bindings)
2. Users can remap to different modifiers: `ctrl+ctrl+up`
3. Users can disable by binding to `Unbound`
4. No timing window configuration needed (300ms hardcoded)
5. All existing tests pass
