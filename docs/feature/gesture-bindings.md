# Double-Tap Modifier Keybindings

Support repeated modifiers in key specifications (`alt+alt+up`)

> **Status:** 📋 Planned
> **Priority:** P2 (Important)
> **Effort:** S (1-2 days)
> **Created:** 2025-12-19
> **Updated:** 2026-09-17
> **Milestone:** 3 - Keybinding Enhancements

---

## Overview

### Current State

The editor currently has a hardcoded implementation for `Option+Option+Arrow` (double-tap Option, then arrow key) to add cursors above/below. The gesture is tracked by `OptionKeyGesture` in `src/runtime/input.rs`, `should_skip_non_global_keymap()` in `src/runtime/app.rs` bypasses the keymap while it is active, and the arrow arms are matched directly in `src/runtime/input.rs`. This works but:

- Cannot be reconfigured or disabled
- Not expressed in keymap.yaml
- Requires special handling that bypasses the keymap system

Alt-containing chord shortcuts no longer seed the double-tap (`OptionKeyGesture::on_other_key`); the generalization below must preserve that guard.

### Goal

Allow keybindings like `alt+alt+up` in keymap.yaml where repeating a modifier means "double-tap that modifier":

```yaml
# keymap.yaml
- key: alt+alt+up
  command: AddCursorAbove

- key: alt+alt+down
  command: AddCursorBelow
```

### Non-Goals

- Configurable timing window (hardcode 300ms)
- Triple-tap or more
- Modifier-only bindings (must end with a key)
- Complex gesture sequences

---

## Design

### Core Insight

The key insight is simple: when parsing a key specification like `alt+alt+up`, if we see the same modifier twice, it means "that modifier was double-tapped."

At match time, we check if the modifier is currently held AND was double-tapped (based on timing).

### Keystroke Extension

```rust
// src/keymap/types.rs

/// A parsed keystroke from config
pub struct Keystroke {
    pub key: KeyCode,
    pub mods: Modifiers,
    pub double_tap: Option<ModifierKey>,  // NEW: Which modifier must be double-tapped
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModifierKey {
    Alt,
    Ctrl,
    Shift,
    Cmd,
}
```

### Gesture State (Already Exists)

The existing code already tracks double-tap state:

```rust
// src/runtime/input.rs (existing)
#[derive(Default)]
pub struct OptionKeyGesture {
    last_press: Option<Instant>,
    pub double_tapped: bool,
}

impl OptionKeyGesture {
    pub fn on_press(&mut self) { /* 300ms window */ }
    pub fn on_release(&mut self) { /* clears double_tapped */ }
    pub fn on_other_key(&mut self) { /* chord guard: clears last_press */ }
}

// src/runtime/app.rs (existing)
pub struct App {
    option_gesture: OptionKeyGesture,
    // ...
}
```

`OptionKeyGesture` already has the shape of the `ModifierGesture` below. We just need to:
1. Generalize to track all modifiers (keeping `on_other_key`)
2. Expose this state to the keymap matching

### Generalized Gesture Tracking

```rust
// src/runtime/app.rs

const DOUBLE_TAP_WINDOW_MS: u64 = 300;

#[derive(Default)]
struct ModifierGesture {
    last_press: Option<Instant>,
    double_tapped: bool,
}

impl ModifierGesture {
    fn on_press(&mut self) {
        let now = Instant::now();
        if let Some(last) = self.last_press {
            if now.duration_since(last) < Duration::from_millis(DOUBLE_TAP_WINDOW_MS) {
                self.double_tapped = true;
            }
        }
        self.last_press = Some(now);
    }

    fn on_release(&mut self) {
        self.double_tapped = false;
    }

    /// A shortcut is not a bare tap; don't let it seed another double-tap.
    fn on_other_key(&mut self) {
        self.last_press = None;
    }
}

pub struct App {
    alt_gesture: ModifierGesture,
    ctrl_gesture: ModifierGesture,
    shift_gesture: ModifierGesture,
    cmd_gesture: ModifierGesture,
    // ...
}

impl App {
    /// Get current double-tap states for keymap matching
    pub fn gesture_state(&self) -> GestureState {
        GestureState {
            alt_double_tap: self.alt_gesture.double_tapped,
            ctrl_double_tap: self.ctrl_gesture.double_tapped,
            shift_double_tap: self.shift_gesture.double_tapped,
            cmd_double_tap: self.cmd_gesture.double_tapped,
        }
    }
}

/// Passed to keymap for matching
#[derive(Debug, Clone, Copy, Default)]
pub struct GestureState {
    pub alt_double_tap: bool,
    pub ctrl_double_tap: bool,
    pub shift_double_tap: bool,
    pub cmd_double_tap: bool,
}
```

### Config Parsing

```rust
// src/keymap/config.rs

pub fn parse_key_string(key_str: &str) -> Result<Keystroke, KeymapError> {
    let parts: Vec<&str> = key_str.split('+').collect();

    let mut mods = Modifiers::NONE;
    let mut double_tap: Option<ModifierKey> = None;
    let mut seen_modifiers: Vec<&str> = Vec::new();

    // Parse all but last part as modifiers
    for part in &parts[..parts.len() - 1] {
        let lower = part.to_lowercase();

        // Check for repeated modifier (double-tap)
        if seen_modifiers.contains(&lower.as_str()) {
            double_tap = Some(match lower.as_str() {
                "alt" | "option" => ModifierKey::Alt,
                "ctrl" | "control" => ModifierKey::Ctrl,
                "shift" => ModifierKey::Shift,
                "cmd" | "meta" | "super" => ModifierKey::Cmd,
                _ => return Err(KeymapError::InvalidKey(format!("Unknown modifier: {}", lower))),
            });
        } else {
            seen_modifiers.push(part);
            match lower.as_str() {
                "alt" | "option" => mods = mods | Modifiers::ALT,
                "ctrl" | "control" => mods = mods | Modifiers::CTRL,
                "shift" => mods = mods | Modifiers::SHIFT,
                "cmd" | "meta" | "super" => mods = mods | Modifiers::META,
                _ => return Err(KeymapError::InvalidKey(format!("Unknown modifier: {}", lower))),
            }
        }
    }

    // Last part is the key
    let key = parse_key_code(parts.last().unwrap())?;

    Ok(Keystroke { key, mods, double_tap })
}
```

### Keymap Matching

```rust
// src/keymap/keymap.rs

impl Keymap {
    pub fn lookup_with_context(
        &self,
        keystroke: &Keystroke,
        context: Option<&KeyContext>,
        gesture: &GestureState,
    ) -> Option<Command> {
        let indices = self.single_lookup.get(keystroke)?;
        self.find_matching_binding(indices, &|binding| {
            binding.matches_single(keystroke, gesture) && binding.is_active(context)
        })
    }
}

// src/keymap/binding.rs

impl Keybinding {
    pub fn matches_single(&self, keystroke: &Keystroke, gesture: &GestureState) -> bool {
        if self.keystrokes.len() != 1 {
            return false;
        }
        let bound = &self.keystrokes[0];

        // Check key and modifiers
        if bound.key != keystroke.key || bound.mods != keystroke.mods {
            return false;
        }

        // Check double-tap requirement
        if let Some(mod_key) = bound.double_tap {
            let is_double_tapped = match mod_key {
                ModifierKey::Alt => gesture.alt_double_tap,
                ModifierKey::Ctrl => gesture.ctrl_double_tap,
                ModifierKey::Shift => gesture.shift_double_tap,
                ModifierKey::Cmd => gesture.cmd_double_tap,
            };
            if !is_double_tapped {
                return false;
            }
        }

        true
    }
}
```

---

## Default Bindings

```yaml
# keymap.yaml - move hardcoded behavior to config

# Double-tap Option + Arrow for multi-cursor
- key: alt+alt+up
  command: AddCursorAbove

- key: alt+alt+down
  command: AddCursorBelow
```

---

## Implementation Plan

### Phase 1: Generalize Gesture Tracking

**Effort:** S (half day)

- [ ] Generalize `OptionKeyGesture` (`src/runtime/input.rs`) into `ModifierGesture`, keeping the `on_other_key` chord guard
- [ ] Add gesture tracking for Alt, Ctrl, Shift, Cmd in `App` (replacing `option_gesture`)
- [ ] Create `GestureState` struct for passing to keymap
- [ ] Update keyboard event handler to track all modifiers

**Test:** Log gesture state, verify double-tap detection works.

### Phase 2: Extend Keystroke Parsing

**Effort:** S (half day)

- [ ] Add `double_tap: Option<ModifierKey>` to `Keystroke`
- [ ] Update `parse_key_string()` to detect repeated modifiers
- [ ] Add unit tests for parsing `alt+alt+up`

**Test:** Parse "alt+alt+up", verify `double_tap = Some(Alt)`.

### Phase 3: Update Keymap Matching

**Effort:** S (half day)

- [ ] Pass `GestureState` to `Keymap::lookup_with_context()`
- [ ] Add double-tap check in `Keybinding::matches_single()`
- [ ] Update all call sites to pass gesture state

**Test:** Binding with `alt+alt+up` only matches when Alt double-tapped.

### Phase 4: Remove Hardcoded Logic

**Effort:** S (half day)

- [ ] Remove the `option_double_tapped && alt_pressed` bypass in `should_skip_non_global_keymap()` (`app.rs`)
- [ ] Remove hardcoded `ArrowUp`/`ArrowDown` double-tap arms (and the `!option_double_tapped` guards on word navigation) in `input.rs`
- [ ] Add default bindings to `keymap.yaml` and document them in `docs/KEYBINDINGS.md`
- [ ] Verify existing behavior preserved

**Test:** Double-tap Option + Arrow still works, now via keymap.

---

## Testing

### Unit Tests

```rust
#[test]
fn test_parse_double_tap() {
    let ks = parse_key_string("alt+alt+up").unwrap();
    assert_eq!(ks.key, KeyCode::ArrowUp);
    assert!(ks.mods.alt());
    assert_eq!(ks.double_tap, Some(ModifierKey::Alt));
}

#[test]
fn test_parse_no_double_tap() {
    let ks = parse_key_string("alt+up").unwrap();
    assert_eq!(ks.key, KeyCode::ArrowUp);
    assert!(ks.mods.alt());
    assert_eq!(ks.double_tap, None);
}

#[test]
fn test_match_requires_double_tap() {
    let binding = Keybinding::new(
        Keystroke {
            key: KeyCode::ArrowUp,
            mods: Modifiers::ALT,
            double_tap: Some(ModifierKey::Alt),
        },
        Command::AddCursorAbove,
    );

    let input = Keystroke {
        key: KeyCode::ArrowUp,
        mods: Modifiers::ALT,
        double_tap: None,
    };

    // Without double-tap state, should NOT match
    let gesture_no = GestureState::default();
    assert!(!binding.matches_single(&input, &gesture_no));

    // With double-tap state, SHOULD match
    let gesture_yes = GestureState { alt_double_tap: true, ..Default::default() };
    assert!(binding.matches_single(&input, &gesture_yes));
}
```

### Manual Testing

- [ ] Double-tap Option + Up/Down adds cursors (existing behavior)
- [ ] Single Option + Up/Down does NOT add cursors
- [ ] Can remap to `ctrl+ctrl+up` in user keymap
- [ ] Can disable with `command: Unbound`
- [ ] Timing window feels right (~300ms)

---

## Migration

Existing users will get the same behavior automatically via the default bindings. No breaking changes.

---

## Files to Modify

| File | Changes |
|------|---------|
| `src/runtime/app.rs` | Track `ModifierGesture` for all modifiers, expose `gesture_state()`, drop the bypass in `should_skip_non_global_keymap()` |
| `src/keymap/types.rs` | Add `double_tap: Option<ModifierKey>` to `Keystroke` |
| `src/keymap/config.rs` | Parse `alt+alt+up` syntax in `parse_key_string()` |
| `src/keymap/keymap.rs` | Pass gesture state through `lookup_with_context()` |
| `src/keymap/binding.rs` | Check gesture state in `matches_single()` |
| `src/runtime/input.rs` | Generalize `OptionKeyGesture`, remove hardcoded double-tap arrow handling |
| `keymap.yaml` | Add `alt+alt+up/down` default bindings |
| `docs/KEYBINDINGS.md` | Document the new bindings |

---

## References

- [Current gesture detection](../../src/runtime/input.rs) - `OptionKeyGesture`
- [Keymap bypass](../../src/runtime/app.rs) - `should_skip_non_global_keymap()`
- [Keymapping System](../archived/KEYMAPPING_IMPLEMENTATION_PLAN.md) - Keymap architecture
