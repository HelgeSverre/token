# Macros

Record and replay sequences of editor actions for repetitive tasks

> **Status:** 📋 Planned
> **Priority:** P2 (Important)
> **Effort:** L (1-2 weeks)
> **Created:** 2025-12-20
> **Milestone:** 7 - Productivity
> **Feature ID:** F-190

---

## Table of Contents

1. [Overview](#overview)
2. [Architecture](#architecture)
3. [Data Structures](#data-structures)
4. [Recording Engine](#recording-engine)
5. [Playback Engine](#playback-engine)
6. [Keybindings](#keybindings)
7. [Implementation Plan](#implementation-plan)
8. [Testing Strategy](#testing-strategy)
9. [References](#references)

---

## Overview

### Current State

The editor currently has:

- Configurable keybindings via YAML
- Comprehensive command system (`Command` enum with 70+ commands)
- Message-based architecture (`Msg` → `update()` → `Cmd`)
- Multi-cursor support for parallel edits
- Undo/redo with atomic operations

However, there is no way to record a sequence of actions and replay them later.

### Goals

1. **Quick recording** - Single key to start/stop recording (no prompts)
2. **Instant replay** - Single key to replay last recorded macro
3. **Multiple slots** - Store macros in numbered slots (1-9)
4. **Persistent macros** - Save/load macros across sessions
5. **Visual feedback** - Clear indication when recording is active
6. **Composable** - Macros can invoke other macros (with recursion limit)
7. **Multi-cursor aware** - Macros work correctly with multiple cursors

### Non-Goals (This Phase)

- Macro editing/viewing the recorded actions
- Conditional logic within macros (if/else, loops)
- Macro palette or searchable macro list
- Sharing macros between users/machines
- Complex scripting language (Lua, etc.)

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           Macro System Flow                                  │
│                                                                              │
│  ┌──────────────────┐                                                        │
│  │  User presses    │                                                        │
│  │  Cmd+Shift+R     │    START RECORDING                                     │
│  └────────┬─────────┘                                                        │
│           │                                                                  │
│           ▼                                                                  │
│  ┌──────────────────────────────────────────────────────────────────────┐   │
│  │                      MacroRecorder (Active)                           │   │
│  │  - Intercepts commands before normal processing                       │   │
│  │  - Filters out non-recordable commands (RecordMacro, PlayMacro)      │   │
│  │  - Appends recordable commands to buffer                              │   │
│  └────────────────────────────────────────────────────────────────────┬─┘   │
│           │                                                           │      │
│           │  User presses Cmd+Shift+R again                          │      │
│           ▼                                                           ▼      │
│  ┌──────────────────┐                                    ┌──────────────────┐│
│  │  STOP RECORDING  │                                    │  Command logged  ││
│  │  Save to slot    │                                    │  [MoveCursor Up] ││
│  └────────┬─────────┘                                    │  [InsertText "x"]││
│           │                                              │  [DeleteChar]    ││
│           ▼                                              └──────────────────┘│
│  ┌──────────────────────────────────────────────────────────────────────┐   │
│  │                         MacroRegistry                                 │   │
│  │  - Slot 0: Last recorded (always available)                          │   │
│  │  - Slots 1-9: Named/numbered slots                                   │   │
│  │  - Persistence to ~/.config/token-editor/macros.yaml                 │   │
│  └────────────────────────────────────────────────────────────────────┬─┘   │
│                                                                        │     │
│           User presses Cmd+Shift+E (or Cmd+1..9)                      │     │
│           │                                                            │     │
│           ▼                                                            ▼     │
│  ┌──────────────────────────────────────────────────────────────────────┐   │
│  │                        MacroPlayer                                    │   │
│  │  - Retrieves macro from registry                                     │   │
│  │  - Dispatches each command in sequence                               │   │
│  │  - Tracks recursion depth (limit: 100)                               │   │
│  │  - Stops on error or user interrupt (Escape)                         │   │
│  └──────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Module Structure

```
src/
├── macros/                      # NEW MODULE
│   ├── mod.rs                   # Public exports
│   ├── types.rs                 # Macro, MacroAction, MacroSlot
│   ├── recorder.rs              # MacroRecorder, recording state
│   ├── player.rs                # MacroPlayer, playback logic
│   ├── registry.rs              # MacroRegistry, storage, persistence
│   └── filter.rs                # Which commands are recordable
├── model/
│   └── mod.rs                   # + macro_state: MacroState
├── update/
│   └── macros.rs                # NEW: Macro message handler
├── messages.rs                  # + MacroMsg enum
└── keymap/
    └── command.rs               # + RecordMacro, PlayMacro, etc.

~/.config/token-editor/
└── macros.yaml                  # Persisted macros
```

### Message Flow

1. **Start Recording:**
   - `Msg::Macro(MacroMsg::ToggleRecord)` received
   - If not recording: Create new `MacroRecorder`, set `macro_state.recording = true`
   - If recording: Finalize recording, store in registry, clear recorder

2. **During Recording:**
   - Commands flow through `update()` normally
   - `MacroRecorder::record()` intercepts recordable commands
   - Non-recordable commands (file dialogs, macros) are filtered

3. **Playback:**
   - `Msg::Macro(MacroMsg::Play(slot))` received
   - `MacroPlayer` retrieves macro from registry
   - Each action dispatched via `Cmd::Batch` of messages
   - Recursion depth tracked to prevent infinite loops

---

## Data Structures

### MacroAction

```rust
// src/macros/types.rs

use crate::keymap::command::Command;

/// A single recordable action in a macro
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum MacroAction {
    /// A command from the keymap system
    Command(Command),

    /// Raw text insertion (batched for efficiency)
    InsertText(String),

    /// Delay between actions (for debugging/visualization)
    Delay(std::time::Duration),
}

impl MacroAction {
    /// Create from a command, batching consecutive InsertChar
    pub fn from_command(cmd: Command) -> Self {
        MacroAction::Command(cmd)
    }
}
```

### Macro

```rust
// src/macros/types.rs

/// A recorded macro (sequence of actions)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Macro {
    /// Unique identifier (slot number or generated)
    pub id: MacroId,

    /// Optional user-provided name
    pub name: Option<String>,

    /// Sequence of recorded actions
    pub actions: Vec<MacroAction>,

    /// When this macro was recorded
    pub recorded_at: chrono::DateTime<chrono::Utc>,

    /// Number of times this macro has been played
    pub play_count: u32,
}

/// Macro identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum MacroId {
    /// The last recorded macro (slot 0, always overwritten)
    Last,

    /// Numbered slot 1-9
    Slot(u8),
}

impl Macro {
    /// Create a new macro from recorded actions
    pub fn new(id: MacroId, actions: Vec<MacroAction>) -> Self {
        Self {
            id,
            name: None,
            actions,
            recorded_at: chrono::Utc::now(),
            play_count: 0,
        }
    }

    /// Check if macro is empty
    pub fn is_empty(&self) -> bool {
        self.actions.is_empty()
    }

    /// Get action count
    pub fn len(&self) -> usize {
        self.actions.len()
    }
}
```

### MacroState

```rust
// src/macros/types.rs

/// Runtime state for the macro system
#[derive(Debug, Default)]
pub struct MacroState {
    /// Currently recording
    pub recorder: Option<MacroRecorder>,

    /// Currently playing (with recursion depth)
    pub player: Option<MacroPlayer>,

    /// Stored macros
    pub registry: MacroRegistry,
}

impl MacroState {
    /// Check if currently recording
    pub fn is_recording(&self) -> bool {
        self.recorder.is_some()
    }

    /// Check if currently playing
    pub fn is_playing(&self) -> bool {
        self.player.is_some()
    }
}
```

### MacroRecorder

```rust
// src/macros/recorder.rs

use super::types::{MacroAction, MacroId};
use crate::keymap::command::Command;

/// Active macro recording session
#[derive(Debug, Clone)]
pub struct MacroRecorder {
    /// Target slot for this recording
    pub target_slot: MacroId,

    /// Recorded actions so far
    pub actions: Vec<MacroAction>,

    /// Buffer for batching consecutive text insertions
    text_buffer: String,

    /// Actions recorded (for status display)
    pub action_count: usize,
}

impl MacroRecorder {
    /// Start a new recording session
    pub fn new(target_slot: MacroId) -> Self {
        Self {
            target_slot,
            actions: Vec::new(),
            text_buffer: String::new(),
            action_count: 0,
        }
    }

    /// Record a command (returns true if recorded, false if filtered)
    pub fn record(&mut self, command: &Command) -> bool {
        // Filter non-recordable commands
        if !is_recordable(command) {
            return false;
        }

        // Batch consecutive text insertions
        if let Command::InsertChar(c) = command {
            self.text_buffer.push(*c);
            self.action_count += 1;
            return true;
        }

        // Flush text buffer before recording other commands
        self.flush_text_buffer();

        self.actions.push(MacroAction::Command(command.clone()));
        self.action_count += 1;
        true
    }

    /// Flush any buffered text insertions
    fn flush_text_buffer(&mut self) {
        if !self.text_buffer.is_empty() {
            let text = std::mem::take(&mut self.text_buffer);
            self.actions.push(MacroAction::InsertText(text));
        }
    }

    /// Finalize recording and return the macro
    pub fn finalize(mut self) -> Macro {
        self.flush_text_buffer();
        Macro::new(self.target_slot, self.actions)
    }
}

/// Check if a command should be recorded
fn is_recordable(command: &Command) -> bool {
    use Command::*;

    match command {
        // Macro commands are never recorded (prevent recursion issues)
        RecordMacro | RecordMacroToSlot(_) | PlayMacro | PlayMacroFromSlot(_) |
        StopMacro | SaveMacroToSlot(_) => false,

        // File dialogs are not recordable (user interaction)
        OpenFile | SaveFileAs | OpenFolder => false,

        // Window management not recordable
        Quit | ForceQuit => false,

        // Theme/config changes not recordable
        OpenThemePicker | OpenCommandPalette => false,

        // Everything else is recordable
        _ => true,
    }
}
```

### MacroPlayer

```rust
// src/macros/player.rs

use super::types::{Macro, MacroAction, MacroId};
use crate::messages::Msg;

/// Maximum recursion depth for macro playback
pub const MAX_RECURSION_DEPTH: u32 = 100;

/// Active macro playback session
#[derive(Debug)]
pub struct MacroPlayer {
    /// The macro being played
    pub macro_ref: Macro,

    /// Current action index
    pub current_index: usize,

    /// Recursion depth (for nested macro calls)
    pub recursion_depth: u32,

    /// Whether playback should stop
    pub cancelled: bool,
}

impl MacroPlayer {
    /// Start playing a macro
    pub fn new(macro_ref: Macro, recursion_depth: u32) -> Result<Self, MacroPlayError> {
        if recursion_depth >= MAX_RECURSION_DEPTH {
            return Err(MacroPlayError::RecursionLimit);
        }

        if macro_ref.is_empty() {
            return Err(MacroPlayError::EmptyMacro);
        }

        Ok(Self {
            macro_ref,
            current_index: 0,
            recursion_depth,
            cancelled: false,
        })
    }

    /// Get the next action to execute
    pub fn next_action(&mut self) -> Option<&MacroAction> {
        if self.cancelled {
            return None;
        }

        if self.current_index >= self.macro_ref.actions.len() {
            return None;
        }

        let action = &self.macro_ref.actions[self.current_index];
        self.current_index += 1;
        Some(action)
    }

    /// Cancel playback
    pub fn cancel(&mut self) {
        self.cancelled = true;
    }

    /// Check if playback is complete
    pub fn is_complete(&self) -> bool {
        self.cancelled || self.current_index >= self.macro_ref.actions.len()
    }

    /// Progress as percentage
    pub fn progress(&self) -> f32 {
        if self.macro_ref.actions.is_empty() {
            return 100.0;
        }
        (self.current_index as f32 / self.macro_ref.actions.len() as f32) * 100.0
    }
}

/// Errors that can occur during macro playback
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MacroPlayError {
    /// Macro not found in registry
    NotFound(MacroId),

    /// Recursion limit exceeded
    RecursionLimit,

    /// Macro has no actions
    EmptyMacro,

    /// Playback was cancelled by user
    Cancelled,
}
```

### MacroRegistry

```rust
// src/macros/registry.rs

use std::collections::HashMap;
use std::path::PathBuf;
use super::types::{Macro, MacroId};

/// Storage for all macros
#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct MacroRegistry {
    /// Stored macros by ID
    #[serde(flatten)]
    macros: HashMap<MacroId, Macro>,
}

impl MacroRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Store a macro (overwrites existing)
    pub fn store(&mut self, macro_data: Macro) {
        self.macros.insert(macro_data.id, macro_data);
    }

    /// Get a macro by ID
    pub fn get(&self, id: MacroId) -> Option<&Macro> {
        self.macros.get(&id)
    }

    /// Get a mutable reference to a macro
    pub fn get_mut(&mut self, id: MacroId) -> Option<&mut Macro> {
        self.macros.get_mut(&id)
    }

    /// Check if a slot has a macro
    pub fn has(&self, id: MacroId) -> bool {
        self.macros.contains_key(&id)
    }

    /// Get all stored macros
    pub fn all(&self) -> impl Iterator<Item = &Macro> {
        self.macros.values()
    }

    /// Clear a specific slot
    pub fn clear(&mut self, id: MacroId) {
        self.macros.remove(&id);
    }

    /// Clear all macros
    pub fn clear_all(&mut self) {
        self.macros.clear();
    }

    /// Load from config file
    pub fn load() -> Result<Self, String> {
        let path = Self::config_path()
            .ok_or("Could not find config directory")?;

        if !path.exists() {
            return Ok(Self::default());
        }

        let content = std::fs::read_to_string(&path)
            .map_err(|e| format!("Failed to read macros: {}", e))?;

        serde_yaml::from_str(&content)
            .map_err(|e| format!("Failed to parse macros: {}", e))
    }

    /// Save to config file
    pub fn save(&self) -> Result<(), String> {
        let path = Self::config_path()
            .ok_or("Could not find config directory")?;

        // Create parent directory if needed
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create config directory: {}", e))?;
        }

        let content = serde_yaml::to_string(self)
            .map_err(|e| format!("Failed to serialize macros: {}", e))?;

        std::fs::write(&path, content)
            .map_err(|e| format!("Failed to write macros: {}", e))
    }

    fn config_path() -> Option<PathBuf> {
        crate::config_paths::config_dir()
            .map(|dir| dir.join("macros.yaml"))
    }
}
```

### MacroMsg

```rust
// src/messages.rs (additions)

/// Messages for the macro system
#[derive(Debug, Clone)]
pub enum MacroMsg {
    /// Toggle recording (start if not recording, stop if recording)
    ToggleRecord,

    /// Start recording to a specific slot
    RecordToSlot(MacroId),

    /// Stop recording and save
    StopRecord,

    /// Play the last recorded macro
    PlayLast,

    /// Play macro from a specific slot
    Play(MacroId),

    /// Play macro N times
    PlayRepeat { slot: MacroId, count: u32 },

    /// Cancel current playback
    CancelPlayback,

    /// Save last recorded to a numbered slot
    SaveToSlot(u8),

    /// Clear a macro slot
    Clear(MacroId),

    /// Clear all macros
    ClearAll,

    /// Internal: Execute next action during playback
    ExecuteNext,
}
```

---

## Recording Engine

### Command Interception

Recording happens at the `update()` layer, after keymap resolution:

```rust
// src/update/macros.rs

pub fn update_macro(model: &mut AppModel, msg: MacroMsg) -> Cmd {
    match msg {
        MacroMsg::ToggleRecord => {
            if model.macro_state.is_recording() {
                stop_recording(model)
            } else {
                start_recording(model, MacroId::Last)
            }
        }

        MacroMsg::RecordToSlot(slot) => {
            if model.macro_state.is_recording() {
                // Already recording - ignore or switch slots?
                Cmd::None
            } else {
                start_recording(model, slot)
            }
        }

        MacroMsg::StopRecord => stop_recording(model),

        MacroMsg::PlayLast => play_macro(model, MacroId::Last),

        MacroMsg::Play(slot) => play_macro(model, slot),

        MacroMsg::PlayRepeat { slot, count } => {
            play_macro_repeat(model, slot, count)
        }

        MacroMsg::CancelPlayback => {
            if let Some(player) = &mut model.macro_state.player {
                player.cancel();
            }
            model.macro_state.player = None;
            Cmd::Redraw
        }

        MacroMsg::SaveToSlot(n) => {
            if let Some(last) = model.macro_state.registry.get(MacroId::Last).cloned() {
                let mut saved = last;
                saved.id = MacroId::Slot(n);
                model.macro_state.registry.store(saved);
                let _ = model.macro_state.registry.save();
            }
            Cmd::None
        }

        MacroMsg::ExecuteNext => execute_next_action(model),

        _ => Cmd::None,
    }
}

fn start_recording(model: &mut AppModel, slot: MacroId) -> Cmd {
    model.macro_state.recorder = Some(MacroRecorder::new(slot));
    Cmd::Redraw // Update status bar
}

fn stop_recording(model: &mut AppModel) -> Cmd {
    if let Some(recorder) = model.macro_state.recorder.take() {
        let macro_data = recorder.finalize();
        if !macro_data.is_empty() {
            model.macro_state.registry.store(macro_data);
            let _ = model.macro_state.registry.save();
        }
    }
    Cmd::Redraw
}
```

### Recording During Normal Updates

```rust
// src/update/mod.rs (modification)

pub fn update(model: &mut AppModel, msg: Msg) -> Cmd {
    // Record the command if we're recording
    if let Some(recorder) = &mut model.macro_state.recorder {
        if let Some(command) = msg.as_command() {
            recorder.record(&command);
        }
    }

    // Normal update processing
    match msg {
        Msg::Macro(macro_msg) => macros::update_macro(model, macro_msg),
        Msg::Editor(editor_msg) => editor::update_editor(model, editor_msg),
        // ... etc
    }
}
```

---

## Playback Engine

### Sequential Execution

```rust
// src/update/macros.rs

fn play_macro(model: &mut AppModel, slot: MacroId) -> Cmd {
    let recursion_depth = model.macro_state.player
        .as_ref()
        .map(|p| p.recursion_depth + 1)
        .unwrap_or(0);

    let Some(macro_data) = model.macro_state.registry.get(slot).cloned() else {
        return Cmd::None; // Macro not found
    };

    match MacroPlayer::new(macro_data, recursion_depth) {
        Ok(player) => {
            model.macro_state.player = Some(player);
            // Start execution chain
            Cmd::Batch(vec![
                Cmd::Dispatch(Msg::Macro(MacroMsg::ExecuteNext)),
            ])
        }
        Err(MacroPlayError::RecursionLimit) => {
            // Show error in status bar
            Cmd::Redraw
        }
        Err(_) => Cmd::None,
    }
}

fn execute_next_action(model: &mut AppModel) -> Cmd {
    let Some(player) = &mut model.macro_state.player else {
        return Cmd::None;
    };

    let Some(action) = player.next_action().cloned() else {
        // Playback complete
        if let Some(mut player) = model.macro_state.player.take() {
            // Update play count
            if let Some(macro_data) = model.macro_state.registry.get_mut(player.macro_ref.id) {
                macro_data.play_count += 1;
            }
        }
        return Cmd::Redraw;
    };

    // Convert action to commands
    let cmd = match action {
        MacroAction::Command(command) => {
            // Convert command to messages
            let msgs = command.to_msgs();
            Cmd::Batch(msgs.into_iter().map(Cmd::Dispatch).collect())
        }

        MacroAction::InsertText(text) => {
            // Insert each character
            let msgs: Vec<Cmd> = text
                .chars()
                .map(|c| Cmd::Dispatch(Msg::Document(DocumentMsg::InsertChar(c))))
                .collect();
            Cmd::Batch(msgs)
        }

        MacroAction::Delay(duration) => {
            // Future: implement delay for visualization
            Cmd::None
        }
    };

    // Chain to next action
    Cmd::Batch(vec![
        cmd,
        Cmd::Dispatch(Msg::Macro(MacroMsg::ExecuteNext)),
    ])
}

fn play_macro_repeat(model: &mut AppModel, slot: MacroId, count: u32) -> Cmd {
    // Queue N playbacks
    let msgs: Vec<Cmd> = (0..count)
        .map(|_| Cmd::Dispatch(Msg::Macro(MacroMsg::Play(slot))))
        .collect();
    Cmd::Batch(msgs)
}
```

---

## Keybindings

### Default Bindings

| Action | Mac | Windows/Linux | Context |
|--------|-----|---------------|---------|
| Toggle record | `Cmd+Shift+R` | `Ctrl+Shift+R` | always |
| Play last macro | `Cmd+Shift+E` | `Ctrl+Shift+E` | always |
| Play macro N times | `Cmd+Shift+E, N` | `Ctrl+Shift+E, N` | always |
| Play from slot 1-9 | `Cmd+Ctrl+1-9` | `Ctrl+Alt+1-9` | always |
| Save to slot 1-9 | `Cmd+Shift+1-9` | `Ctrl+Shift+1-9` | always |
| Cancel playback | `Escape` | `Escape` | macro_playing |

### Keymap Configuration

```yaml
# keymap.yaml additions

- key: "cmd+shift+r"
  command: RecordMacro
  when: ["in_editor"]

- key: "cmd+shift+e"
  command: PlayMacro
  when: ["in_editor"]

- key: "cmd+ctrl+1"
  command: PlayMacroFromSlot
  args: { slot: 1 }
  when: ["in_editor"]

# ... slots 2-9 ...

- key: "cmd+shift+1"
  command: SaveMacroToSlot
  args: { slot: 1 }
  when: ["in_editor"]

# ... slots 2-9 ...

- key: "escape"
  command: CancelMacro
  when: ["macro_playing"]
```

### Status Bar Integration

```rust
// Status bar shows recording state

fn macro_status_segment(macro_state: &MacroState) -> Option<StatusSegment> {
    if let Some(recorder) = &macro_state.recorder {
        Some(StatusSegment::new(
            format!("● REC ({})", recorder.action_count),
            StatusStyle::Error, // Red for recording
        ))
    } else if let Some(player) = &macro_state.player {
        Some(StatusSegment::new(
            format!("▶ MACRO {:.0}%", player.progress()),
            StatusStyle::Info,
        ))
    } else {
        None
    }
}
```

---

## Implementation Plan

### Phase 1: Core Types & Recording

**Effort:** M (3-5 days)

- [ ] Create `src/macros/` module structure
- [ ] Implement `MacroAction`, `Macro`, `MacroId` types
- [ ] Implement `MacroRecorder` with command filtering
- [ ] Add `MacroState` to `AppModel`
- [ ] Add `MacroMsg` to `messages.rs`
- [ ] Implement start/stop recording in `update/macros.rs`
- [ ] Wire up `Cmd+Shift+R` keybinding
- [ ] Add "● REC" indicator to status bar

**Test:** Record typing + cursor movements, verify actions captured.

### Phase 2: Playback Engine

**Effort:** M (3-5 days)

- [ ] Implement `MacroPlayer` with action iteration
- [ ] Implement `execute_next_action()` with command dispatch
- [ ] Add recursion depth tracking
- [ ] Handle `InsertText` batched playback
- [ ] Wire up `Cmd+Shift+E` for playback
- [ ] Add escape cancellation during playback
- [ ] Add playback progress to status bar

**Test:** Record 5 actions, play back, verify document matches manual execution.

### Phase 3: Registry & Persistence

**Effort:** S (1-2 days)

- [ ] Implement `MacroRegistry` with HashMap storage
- [ ] Add YAML serialization for macros
- [ ] Load/save from `~/.config/token-editor/macros.yaml`
- [ ] Load registry on startup
- [ ] Save on recording stop

**Test:** Record macro, quit editor, relaunch, play macro successfully.

### Phase 4: Numbered Slots

**Effort:** S (1-2 days)

- [ ] Implement `SaveMacroToSlot` command
- [ ] Implement `PlayMacroFromSlot` command
- [ ] Add keybindings for slots 1-9
- [ ] Command palette entries for macro operations

**Test:** Save macro to slot 3, clear last macro, play from slot 3.

### Phase 5: Multi-Cursor & Edge Cases

**Effort:** M (3-5 days)

- [ ] Test recording with multiple cursors
- [ ] Test playback with multiple cursors
- [ ] Handle edge cases (empty document, end of file)
- [ ] Handle errors gracefully (show in status bar)
- [ ] Prevent recording during playback
- [ ] Add repeat count support (play N times)

**Test:** Record multi-cursor edit, play back, verify all cursor positions edited.

### Phase 6: Polish

**Effort:** S (1-2 days)

- [ ] Visual feedback improvements (color, animation?)
- [ ] Performance optimization for long macros
- [ ] Documentation
- [ ] Integration tests

**Test:** Record 100+ action macro, play without lag.

---

## Testing Strategy

### Unit Tests

```rust
// tests/macros.rs

#[test]
fn test_recorder_filters_macro_commands() {
    let mut recorder = MacroRecorder::new(MacroId::Last);

    // Regular commands are recorded
    assert!(recorder.record(&Command::MoveCursorUp));
    assert!(recorder.record(&Command::InsertChar('a')));

    // Macro commands are filtered
    assert!(!recorder.record(&Command::RecordMacro));
    assert!(!recorder.record(&Command::PlayMacro));

    assert_eq!(recorder.action_count, 2);
}

#[test]
fn test_recorder_batches_text() {
    let mut recorder = MacroRecorder::new(MacroId::Last);

    recorder.record(&Command::InsertChar('h'));
    recorder.record(&Command::InsertChar('e'));
    recorder.record(&Command::InsertChar('l'));
    recorder.record(&Command::InsertChar('l'));
    recorder.record(&Command::InsertChar('o'));

    let macro_data = recorder.finalize();

    // Should batch into single InsertText
    assert_eq!(macro_data.actions.len(), 1);
    assert!(matches!(
        &macro_data.actions[0],
        MacroAction::InsertText(s) if s == "hello"
    ));
}

#[test]
fn test_recorder_flushes_on_non_text() {
    let mut recorder = MacroRecorder::new(MacroId::Last);

    recorder.record(&Command::InsertChar('a'));
    recorder.record(&Command::InsertChar('b'));
    recorder.record(&Command::MoveCursorDown);
    recorder.record(&Command::InsertChar('c'));

    let macro_data = recorder.finalize();

    assert_eq!(macro_data.actions.len(), 3);
    // InsertText("ab"), MoveCursorDown, InsertText("c")
}

#[test]
fn test_player_respects_recursion_limit() {
    let macro_data = Macro::new(MacroId::Last, vec![
        MacroAction::Command(Command::MoveCursorDown),
    ]);

    // Should fail at recursion limit
    let result = MacroPlayer::new(macro_data.clone(), MAX_RECURSION_DEPTH);
    assert!(matches!(result, Err(MacroPlayError::RecursionLimit)));

    // Should succeed below limit
    let result = MacroPlayer::new(macro_data, MAX_RECURSION_DEPTH - 1);
    assert!(result.is_ok());
}

#[test]
fn test_registry_persistence() {
    let mut registry = MacroRegistry::new();

    let macro_data = Macro::new(MacroId::Slot(1), vec![
        MacroAction::Command(Command::MoveCursorUp),
    ]);
    registry.store(macro_data);

    // Serialize and deserialize
    let yaml = serde_yaml::to_string(&registry).unwrap();
    let restored: MacroRegistry = serde_yaml::from_str(&yaml).unwrap();

    assert!(restored.has(MacroId::Slot(1)));
    assert_eq!(restored.get(MacroId::Slot(1)).unwrap().actions.len(), 1);
}

#[test]
fn test_empty_macro_rejected() {
    let macro_data = Macro::new(MacroId::Last, vec![]);

    let result = MacroPlayer::new(macro_data, 0);
    assert!(matches!(result, Err(MacroPlayError::EmptyMacro)));
}
```

### Integration Tests

```rust
// tests/macro_integration.rs

#[test]
fn test_record_and_playback() {
    let mut model = test_model_with_content("hello");
    position_cursor(&mut model, 0, 5); // End of "hello"

    // Start recording
    update(&mut model, Msg::Macro(MacroMsg::ToggleRecord));
    assert!(model.macro_state.is_recording());

    // Type " world"
    for c in " world".chars() {
        update(&mut model, Msg::Document(DocumentMsg::InsertChar(c)));
    }

    // Stop recording
    update(&mut model, Msg::Macro(MacroMsg::ToggleRecord));
    assert!(!model.macro_state.is_recording());

    // Clear and reset
    set_content(&mut model, "foo");
    position_cursor(&mut model, 0, 3);

    // Play macro
    run_until_complete(&mut model, Msg::Macro(MacroMsg::PlayLast));

    assert_eq!(get_content(&model), "foo world");
}

#[test]
fn test_macro_with_cursor_movement() {
    let mut model = test_model_with_content("line1\nline2\nline3");
    position_cursor(&mut model, 0, 0);

    // Record: go down, insert ">> "
    update(&mut model, Msg::Macro(MacroMsg::ToggleRecord));
    update(&mut model, Msg::Editor(EditorMsg::MoveCursor(Direction::Down)));
    for c in ">> ".chars() {
        update(&mut model, Msg::Document(DocumentMsg::InsertChar(c)));
    }
    update(&mut model, Msg::Macro(MacroMsg::ToggleRecord));

    // Reset cursor
    position_cursor(&mut model, 0, 0);

    // Play twice
    run_until_complete(&mut model, Msg::Macro(MacroMsg::PlayRepeat {
        slot: MacroId::Last,
        count: 2,
    }));

    let content = get_content(&model);
    assert!(content.contains(">> line2"));
    assert!(content.contains(">> >> line3")); // Second play adds another prefix
}

#[test]
fn test_escape_cancels_playback() {
    let mut model = test_model_with_content("test");

    // Create a long macro
    let actions: Vec<MacroAction> = (0..100)
        .map(|_| MacroAction::Command(Command::MoveCursorRight))
        .collect();
    model.macro_state.registry.store(Macro::new(MacroId::Last, actions));

    // Start playing
    update(&mut model, Msg::Macro(MacroMsg::PlayLast));
    assert!(model.macro_state.is_playing());

    // Execute a few steps
    for _ in 0..5 {
        update(&mut model, Msg::Macro(MacroMsg::ExecuteNext));
    }

    // Cancel
    update(&mut model, Msg::Macro(MacroMsg::CancelPlayback));
    assert!(!model.macro_state.is_playing());
}

#[test]
fn test_multi_cursor_macro() {
    let mut model = test_model_with_content("aaa\nbbb\nccc");

    // Add cursors on each line
    position_cursor(&mut model, 0, 0);
    update(&mut model, Msg::Editor(EditorMsg::AddCursorBelow));
    update(&mut model, Msg::Editor(EditorMsg::AddCursorBelow));

    // Record: move to end, insert "!"
    update(&mut model, Msg::Macro(MacroMsg::ToggleRecord));
    update(&mut model, Msg::Editor(EditorMsg::MoveToLineEnd));
    update(&mut model, Msg::Document(DocumentMsg::InsertChar('!')));
    update(&mut model, Msg::Macro(MacroMsg::ToggleRecord));

    // Reset
    set_content(&mut model, "xxx\nyyy\nzzz");
    position_cursor(&mut model, 0, 0);
    update(&mut model, Msg::Editor(EditorMsg::AddCursorBelow));
    update(&mut model, Msg::Editor(EditorMsg::AddCursorBelow));

    // Play
    run_until_complete(&mut model, Msg::Macro(MacroMsg::PlayLast));

    assert_eq!(get_content(&model), "xxx!\nyyy!\nzzz!");
}
```

### Manual Testing Checklist

- [ ] Record simple typing macro
- [ ] Record cursor movement + editing macro
- [ ] Play back immediately after recording
- [ ] Play back after editor restart
- [ ] Save to numbered slot
- [ ] Play from numbered slot
- [ ] Record with multi-cursor active
- [ ] Play with multi-cursor active
- [ ] Escape cancels recording
- [ ] Escape cancels playback
- [ ] Status bar shows recording indicator
- [ ] Status bar shows playback progress
- [ ] Empty macro not saved
- [ ] Rapid key repeat during playback

---

## References

### Internal Docs

- [Feature: Snippets](./snippets.md) - Similar expansion pattern
- [Keymapping System](../archived/KEYMAPPING_IMPLEMENTATION_PLAN.md) - Command enum integration
- [Message Architecture](../dev/contracts-update.md) - Msg/Cmd pattern

### External Resources

- [Vim Macros](https://vim.fandom.com/wiki/Macros) - Classic macro UX
- [Emacs Keyboard Macros](https://www.gnu.org/software/emacs/manual/html_node/emacs/Keyboard-Macros.html)
- [VS Code Macro Extension](https://marketplace.visualstudio.com/items?itemName=geddski.macros)

---

## Open Questions

1. **Should macros record at the command level or keystroke level?**
   - Command level is cleaner but loses some fidelity
   - Decision: Command level (matches existing architecture)

2. **How to handle commands that fail during playback?**
   - Option A: Stop playback on first error
   - Option B: Continue, log errors
   - Recommendation: Option A for safety

3. **Should there be a "step through" debug mode for macros?**
   - Useful for debugging but adds complexity
   - Decision: Defer to future phase

---

## Appendix

### Design Decisions

| Decision | Options Considered | Chosen | Rationale |
|----------|-------------------|--------|-----------|
| Storage format | Binary, JSON, YAML | YAML | Human-readable, consistent with other config |
| Recording layer | Keystroke, Command, Message | Command | Clean abstraction, works with keymap system |
| Slot system | Named only, Numbered only, Both | Numbered + last | Simple UX, vim-like familiarity |
| Execution model | Immediate all, Async chunked | Async chunked | Allows cancellation, visual feedback |

### Changelog

| Date | Change |
|------|--------|
| 2025-12-20 | Initial draft |
