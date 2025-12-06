# Feature: Configurable Keyboard Mapping System

> **Status:** Design  
> **Priority:** High  
> **Complexity:** Medium-Large (estimated 8-15 hours)

## Overview

Implement a user-configurable keyboard mapping system that allows users to customize keybindings via TOML configuration files. The system integrates cleanly with the existing Elm architecture (Message → Update → Command → Render) and supports multi-key sequences (chords), context-aware bindings, and platform-agnostic configuration.

## Goals

1. **User Customization:** Users can override default bindings in `~/.config/token-editor/keymap.toml`
2. **Platform Agnostic:** `mod+s` means Cmd+S on macOS, Ctrl+S on Windows/Linux
3. **Context-Aware:** Bindings can be conditional (e.g., only when editor has focus, only with selection)
4. **Chord Support:** Multi-key sequences like `Ctrl+K Ctrl+C` for comment line
5. **Clean Integration:** Maps to existing `Msg` enum for Elm-style dispatch

## Non-Goals (Phase 1)

- Full modal/vim-style editing modes (future consideration)
- Complex expression parser for `when` clauses (keep simple for now)
- Per-language keybindings
- Dynamic keymap reloading (requires restart)

---

## Architecture

### Module Structure

```
src/
├── keymap/
│   ├── mod.rs              # Public API, Keymap struct
│   ├── types.rs            # Keystroke, Modifiers, KeyCode
│   ├── binding.rs          # Keybinding struct
│   ├── command.rs          # Command enum + to_msgs()
│   ├── context.rs          # KeyContext, Condition, evaluation
│   ├── config.rs           # TOML parsing, BindingConfig
│   ├── winit_adapter.rs    # Convert winit events → Keystroke
│   └── defaults.rs         # Default keybindings (embedded TOML)
```

### Data Flow

```
┌─────────────────────────────────────────────────────────────────────┐
│                         Event Loop (main.rs)                        │
└─────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────┐
│  winit::KeyEvent                                                    │
│  ┌─────────────────────────────────────────────────────────────┐    │
│  │ logical_key: Key::Character("s")                            │    │
│  │ modifiers: ModifiersState { ctrl: true, ... }               │    │
│  └─────────────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼ winit_adapter::keystroke_from_winit()
┌─────────────────────────────────────────────────────────────────────┐
│  Keystroke { key: Char('s'), mods: { ctrl: true, ... } }           │
└─────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼ keymap.handle_keystroke()
┌─────────────────────────────────────────────────────────────────────┐
│  KeyAction::Execute(Command::SaveFile)                             │
│  KeyAction::AwaitMore       ← chord in progress                    │
│  KeyAction::NoMatch         ← pass to text input                   │
└─────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼ Command::to_msgs()
┌─────────────────────────────────────────────────────────────────────┐
│  Vec<Msg> → feed through update() → execute                        │
└─────────────────────────────────────────────────────────────────────┘
```

---

## Core Types

### Keystroke & Modifiers

```rust
// keymap/types.rs

use bitflags::bitflags;

bitflags! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct Modifiers: u8 {
        const CTRL  = 0b0001;
        const SHIFT = 0b0010;
        const ALT   = 0b0100;
        const META  = 0b1000;  // Cmd on macOS, Win on Windows
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum KeyCode {
    Char(char),          // Normalized to lowercase
    Enter,
    Escape,
    Tab,
    Backspace,
    Delete,
    Space,
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    PageUp,
    PageDown,
    Insert,
    F(u8),               // F1-F24
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Keystroke {
    pub key: KeyCode,
    pub mods: Modifiers,
}

impl Keystroke {
    pub fn new(key: KeyCode, mods: Modifiers) -> Self {
        Self { key, mods }
    }
}
```

### Command Enum

```rust
// keymap/command.rs

use crate::messages::{Msg, EditorMsg, DocumentMsg, AppMsg, Direction};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Command {
    // Cursor Movement
    MoveCursor(Direction),
    MoveCursorLineStart,
    MoveCursorLineEnd,
    MoveCursorDocumentStart,
    MoveCursorDocumentEnd,
    MoveCursorWord(Direction),
    PageUp,
    PageDown,

    // Selection
    MoveCursorWithSelection(Direction),
    MoveCursorLineStartWithSelection,
    MoveCursorLineEndWithSelection,
    MoveCursorWordWithSelection(Direction),
    SelectAll,
    SelectLine,

    // Editing
    InsertNewline,
    DeleteBackward,
    DeleteForward,
    DeleteWord(Direction),
    DeleteLine,
    DuplicateLine,

    // Clipboard
    Copy,
    Cut,
    Paste,

    // Undo/Redo
    Undo,
    Redo,

    // File Operations
    SaveFile,
    SaveFileAs,
    NewFile,
    OpenFile,
    CloseFile,

    // Application
    Quit,

    // Special
    Unbound,  // Explicitly unbind a key
}

impl Command {
    /// Convert command to one or more messages for the Elm pipeline
    pub fn to_msgs(self) -> Vec<Msg> {
        use Command::*;
        match self {
            MoveCursor(d) => vec![Msg::Editor(EditorMsg::MoveCursor(d))],
            MoveCursorLineStart => vec![Msg::Editor(EditorMsg::MoveCursorLineStart)],
            MoveCursorLineEnd => vec![Msg::Editor(EditorMsg::MoveCursorLineEnd)],
            MoveCursorDocumentStart => vec![Msg::Editor(EditorMsg::MoveCursorDocumentStart)],
            MoveCursorDocumentEnd => vec![Msg::Editor(EditorMsg::MoveCursorDocumentEnd)],
            MoveCursorWord(d) => vec![Msg::Editor(EditorMsg::MoveCursorWord(d))],
            PageUp => vec![Msg::Editor(EditorMsg::PageUp)],
            PageDown => vec![Msg::Editor(EditorMsg::PageDown)],

            MoveCursorWithSelection(d) => vec![Msg::Editor(EditorMsg::MoveCursorWithSelection(d))],
            MoveCursorLineStartWithSelection => vec![Msg::Editor(EditorMsg::MoveCursorLineStartWithSelection)],
            MoveCursorLineEndWithSelection => vec![Msg::Editor(EditorMsg::MoveCursorLineEndWithSelection)],
            MoveCursorWordWithSelection(d) => vec![Msg::Editor(EditorMsg::MoveCursorWordWithSelection(d))],
            SelectAll => vec![Msg::Editor(EditorMsg::SelectAll)],
            SelectLine => vec![Msg::Editor(EditorMsg::SelectLine)],

            InsertNewline => vec![Msg::Document(DocumentMsg::InsertNewline)],
            DeleteBackward => vec![Msg::Document(DocumentMsg::DeleteBackward)],
            DeleteForward => vec![Msg::Document(DocumentMsg::DeleteForward)],
            DeleteWord(d) => vec![Msg::Document(DocumentMsg::DeleteWord(d))],
            DeleteLine => vec![Msg::Document(DocumentMsg::DeleteLine)],
            DuplicateLine => vec![Msg::Document(DocumentMsg::DuplicateLine)],

            Copy => vec![Msg::Document(DocumentMsg::Copy)],
            Cut => vec![Msg::Document(DocumentMsg::Cut)],
            Paste => vec![Msg::Document(DocumentMsg::Paste)],

            Undo => vec![Msg::Document(DocumentMsg::Undo)],
            Redo => vec![Msg::Document(DocumentMsg::Redo)],

            SaveFile => vec![Msg::App(AppMsg::SaveFile)],
            SaveFileAs => vec![Msg::App(AppMsg::SaveFileAs)],
            NewFile => vec![Msg::App(AppMsg::NewFile)],
            OpenFile => vec![Msg::App(AppMsg::OpenFile)],
            CloseFile => vec![Msg::App(AppMsg::CloseFile)],

            Quit => vec![Msg::App(AppMsg::Quit)],

            Unbound => vec![], // No action
        }
    }
}
```

### Keybinding

```rust
// keymap/binding.rs

use super::{Keystroke, Command};
use super::context::Condition;
use smallvec::SmallVec;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum BindingSource {
    Default = 0,
    User = 1,
}

#[derive(Debug, Clone)]
pub struct Keybinding {
    /// Keystroke sequence (usually 1-2 keys)
    pub sequence: SmallVec<[Keystroke; 2]>,
    /// Command to execute
    pub command: Command,
    /// Optional context condition
    pub when: Option<Condition>,
    /// Source for precedence ordering
    pub source: BindingSource,
}

impl Keybinding {
    pub fn new(sequence: impl Into<SmallVec<[Keystroke; 2]>>, command: Command) -> Self {
        Self {
            sequence: sequence.into(),
            command,
            when: None,
            source: BindingSource::Default,
        }
    }

    pub fn with_when(mut self, condition: Condition) -> Self {
        self.when = Some(condition);
        self
    }

    pub fn with_source(mut self, source: BindingSource) -> Self {
        self.source = source;
        self
    }
}
```

### Context System

```rust
// keymap/context.rs

use crate::model::AppModel;

/// Runtime context for evaluating binding conditions
#[derive(Clone, Debug, Default)]
pub struct KeyContext {
    pub editor_focus: bool,
    pub has_selection: bool,
    pub has_multiple_cursors: bool,
    pub is_readonly: bool,
}

impl KeyContext {
    pub fn from_model(model: &AppModel) -> Self {
        Self {
            editor_focus: true, // TODO: track actual focus when we have multiple panels
            has_selection: model.editor_state.has_selection(),
            has_multiple_cursors: model.editor_state.cursors.len() > 1,
            is_readonly: false, // TODO: implement read-only mode
        }
    }
}

/// Named context keys for condition expressions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ContextKey {
    EditorFocus,
    HasSelection,
    HasMultipleCursors,
    IsReadonly,
}

/// Condition expression for when clauses
#[derive(Debug, Clone)]
pub enum Condition {
    True,
    Key(ContextKey),
    Not(Box<Condition>),
    And(Vec<Condition>),
}

impl Condition {
    pub fn eval(&self, ctx: &KeyContext) -> bool {
        match self {
            Condition::True => true,
            Condition::Key(k) => match k {
                ContextKey::EditorFocus => ctx.editor_focus,
                ContextKey::HasSelection => ctx.has_selection,
                ContextKey::HasMultipleCursors => ctx.has_multiple_cursors,
                ContextKey::IsReadonly => ctx.is_readonly,
            },
            Condition::Not(inner) => !inner.eval(ctx),
            Condition::And(conditions) => conditions.iter().all(|c| c.eval(ctx)),
        }
    }
}

/// Parse simple condition strings: "editor_focus && !has_selection"
pub fn parse_condition(expr: &str) -> Option<Condition> {
    let expr = expr.trim();
    if expr.is_empty() {
        return Some(Condition::True);
    }

    let mut conditions = Vec::new();
    for token in expr.split("&&") {
        let token = token.trim();
        let (negated, name) = if let Some(rest) = token.strip_prefix('!') {
            (true, rest.trim())
        } else {
            (false, token)
        };

        let key = match name {
            "editor_focus" => ContextKey::EditorFocus,
            "has_selection" => ContextKey::HasSelection,
            "has_multiple_cursors" => ContextKey::HasMultipleCursors,
            "is_readonly" => ContextKey::IsReadonly,
            _ => return None,
        };

        let condition = Condition::Key(key);
        conditions.push(if negated {
            Condition::Not(Box::new(condition))
        } else {
            condition
        });
    }

    match conditions.len() {
        0 => Some(Condition::True),
        1 => Some(conditions.remove(0)),
        _ => Some(Condition::And(conditions)),
    }
}
```

### Keymap & Dispatch

```rust
// keymap/mod.rs

use crate::model::AppModel;
use smallvec::SmallVec;

mod types;
mod binding;
mod command;
mod context;
mod config;
mod winit_adapter;
mod defaults;

pub use types::{Keystroke, KeyCode, Modifiers};
pub use binding::{Keybinding, BindingSource};
pub use command::Command;
pub use context::{KeyContext, Condition, ContextKey};
pub use winit_adapter::keystroke_from_winit;

/// Result of keystroke handling
#[derive(Debug, Clone)]
pub enum KeyAction {
    /// Execute the matched command
    Execute(Command),
    /// Partial match - wait for more keys
    AwaitMore,
    /// No binding matched
    NoMatch,
}

/// The keymap dispatcher
pub struct Keymap {
    bindings: Vec<Keybinding>,
    pending: SmallVec<[Keystroke; 2]>,
}

impl Keymap {
    /// Create keymap from bindings, sorted by precedence
    pub fn new(mut bindings: Vec<Keybinding>) -> Self {
        // Sort: User bindings first, then by sequence length (longer first)
        bindings.sort_by(|a, b| {
            b.source.cmp(&a.source)
                .then_with(|| b.sequence.len().cmp(&a.sequence.len()))
        });

        Self {
            bindings,
            pending: SmallVec::new(),
        }
    }

    /// Load default + user keymap
    pub fn load() -> Self {
        let mut bindings = defaults::default_bindings();

        if let Some(user_bindings) = config::load_user_keymap() {
            bindings.extend(user_bindings);
        }

        Self::new(bindings)
    }

    /// Reset pending sequence (e.g., on timeout or Escape)
    pub fn reset(&mut self) {
        self.pending.clear();
    }

    /// Handle a keystroke, returning the action to take
    pub fn handle_keystroke(&mut self, stroke: Keystroke, model: &AppModel) -> KeyAction {
        self.pending.push(stroke);
        let ctx = KeyContext::from_model(model);

        let mut exact_match: Option<&Keybinding> = None;
        let mut has_prefix = false;

        for binding in &self.bindings {
            // Check context condition
            if let Some(ref condition) = binding.when {
                if !condition.eval(&ctx) {
                    continue;
                }
            }

            // Check if binding sequence starts with pending keystrokes
            if !self.sequence_matches(&binding.sequence) {
                continue;
            }

            if binding.sequence.len() == self.pending.len() {
                exact_match = Some(binding);
                break; // First match wins (sorted by precedence)
            } else if binding.sequence.len() > self.pending.len() {
                has_prefix = true;
            }
        }

        if let Some(binding) = exact_match {
            self.pending.clear();
            return KeyAction::Execute(binding.command);
        }

        if has_prefix {
            return KeyAction::AwaitMore;
        }

        self.pending.clear();
        KeyAction::NoMatch
    }

    fn sequence_matches(&self, binding_seq: &[Keystroke]) -> bool {
        if binding_seq.len() < self.pending.len() {
            return false;
        }
        self.pending.iter()
            .zip(binding_seq.iter())
            .all(|(a, b)| a == b)
    }
}
```

---

## Configuration Format

### User Keymap (`~/.config/token-editor/keymap.toml`)

```toml
# Override default bindings
[[binding]]
key = "mod+s"
command = "file.save"

[[binding]]
key = "mod+shift+s"
command = "file.save_as"

# Multi-key sequences (chords)
[[binding]]
key = "ctrl+k ctrl+c"
command = "editor.comment_line"
when = "has_selection"

# Unbind a default
[[binding]]
key = "ctrl+w"
command = "unbound"

# Platform-specific (rare)
[[binding]]
key = "cmd+option+left"
command = "cursor.word_left"
```

### Config Parsing

```rust
// keymap/config.rs

use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
pub struct BindingConfig {
    pub key: String,
    pub command: String,
    #[serde(default)]
    pub when: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct KeymapConfig {
    #[serde(default)]
    pub binding: Vec<BindingConfig>,
}

pub fn user_keymap_path() -> Option<PathBuf> {
    dirs::config_dir().map(|p| p.join("token-editor").join("keymap.toml"))
}

pub fn load_user_keymap() -> Option<Vec<Keybinding>> {
    let path = user_keymap_path()?;
    let content = std::fs::read_to_string(&path).ok()?;
    let config: KeymapConfig = toml::from_str(&content).ok()?;

    let is_macos = cfg!(target_os = "macos");

    Some(config.binding.iter()
        .filter_map(|bc| parse_binding_config(bc, is_macos))
        .map(|mut b| { b.source = BindingSource::User; b })
        .collect())
}

fn parse_binding_config(config: &BindingConfig, is_macos: bool) -> Option<Keybinding> {
    let sequence = parse_key_sequence(&config.key, is_macos)?;
    let command = command_from_str(&config.command)?;
    let when = config.when.as_ref()
        .and_then(|w| context::parse_condition(w));

    Some(Keybinding {
        sequence,
        command,
        when,
        source: BindingSource::Default, // Caller overrides to User
    })
}
```

---

## Default Bindings

```rust
// keymap/defaults.rs

use super::*;

/// Generate default bindings (embedded, not from file)
pub fn default_bindings() -> Vec<Keybinding> {
    let is_macos = cfg!(target_os = "macos");
    let mod_key = if is_macos { Modifiers::META } else { Modifiers::CTRL };

    vec![
        // File operations
        binding(mod_key, KeyCode::Char('s'), Command::SaveFile),
        binding(mod_key | Modifiers::SHIFT, KeyCode::Char('s'), Command::SaveFileAs),
        binding(mod_key, KeyCode::Char('n'), Command::NewFile),
        binding(mod_key, KeyCode::Char('o'), Command::OpenFile),
        binding(mod_key, KeyCode::Char('w'), Command::CloseFile),
        binding(mod_key, KeyCode::Char('q'), Command::Quit),

        // Undo/Redo
        binding(mod_key, KeyCode::Char('z'), Command::Undo),
        binding(mod_key | Modifiers::SHIFT, KeyCode::Char('z'), Command::Redo),

        // Clipboard
        binding(mod_key, KeyCode::Char('c'), Command::Copy),
        binding(mod_key, KeyCode::Char('x'), Command::Cut),
        binding(mod_key, KeyCode::Char('v'), Command::Paste),

        // Selection
        binding(mod_key, KeyCode::Char('a'), Command::SelectAll),

        // Navigation
        binding(Modifiers::empty(), KeyCode::Up, Command::MoveCursor(Direction::Up)),
        binding(Modifiers::empty(), KeyCode::Down, Command::MoveCursor(Direction::Down)),
        binding(Modifiers::empty(), KeyCode::Left, Command::MoveCursor(Direction::Left)),
        binding(Modifiers::empty(), KeyCode::Right, Command::MoveCursor(Direction::Right)),
        binding(Modifiers::empty(), KeyCode::Home, Command::MoveCursorLineStart),
        binding(Modifiers::empty(), KeyCode::End, Command::MoveCursorLineEnd),
        binding(mod_key, KeyCode::Home, Command::MoveCursorDocumentStart),
        binding(mod_key, KeyCode::End, Command::MoveCursorDocumentEnd),
        binding(Modifiers::empty(), KeyCode::PageUp, Command::PageUp),
        binding(Modifiers::empty(), KeyCode::PageDown, Command::PageDown),

        // Word navigation (Alt on macOS, Ctrl elsewhere for word movement)
        binding(Modifiers::ALT, KeyCode::Left, Command::MoveCursorWord(Direction::Left)),
        binding(Modifiers::ALT, KeyCode::Right, Command::MoveCursorWord(Direction::Right)),

        // Selection with Shift
        binding(Modifiers::SHIFT, KeyCode::Up, Command::MoveCursorWithSelection(Direction::Up)),
        binding(Modifiers::SHIFT, KeyCode::Down, Command::MoveCursorWithSelection(Direction::Down)),
        binding(Modifiers::SHIFT, KeyCode::Left, Command::MoveCursorWithSelection(Direction::Left)),
        binding(Modifiers::SHIFT, KeyCode::Right, Command::MoveCursorWithSelection(Direction::Right)),
        binding(Modifiers::SHIFT, KeyCode::Home, Command::MoveCursorLineStartWithSelection),
        binding(Modifiers::SHIFT, KeyCode::End, Command::MoveCursorLineEndWithSelection),

        // Editing
        binding(Modifiers::empty(), KeyCode::Enter, Command::InsertNewline),
        binding(Modifiers::empty(), KeyCode::Backspace, Command::DeleteBackward),
        binding(Modifiers::empty(), KeyCode::Delete, Command::DeleteForward),
        binding(mod_key | Modifiers::SHIFT, KeyCode::Char('k'), Command::DeleteLine),
        binding(mod_key | Modifiers::SHIFT, KeyCode::Char('d'), Command::DuplicateLine),
    ]
}

fn binding(mods: Modifiers, key: KeyCode, command: Command) -> Keybinding {
    Keybinding::new(
        smallvec::smallvec![Keystroke::new(key, mods)],
        command,
    )
}
```

---

## Integration with Event Loop

```rust
// main.rs (integration points)

use keymap::{Keymap, KeyAction, keystroke_from_winit};

struct App {
    model: AppModel,
    keymap: Keymap,
    // ...
}

impl App {
    fn new() -> Self {
        Self {
            model: AppModel::default(),
            keymap: Keymap::load(),
            // ...
        }
    }

    fn handle_key_event(&mut self, event: &winit::event::KeyEvent) {
        use winit::event::ElementState;

        if event.state != ElementState::Pressed {
            return;
        }

        // Convert winit event to our Keystroke
        let Some(stroke) = keystroke_from_winit(&event.logical_key, event.modifiers) else {
            return;
        };

        // Escape cancels pending sequences
        if stroke.key == KeyCode::Escape {
            self.keymap.reset();
            // Also handle Escape as a command if needed
            return;
        }

        match self.keymap.handle_keystroke(stroke, &self.model) {
            KeyAction::Execute(command) => {
                for msg in command.to_msgs() {
                    if let Some(cmd) = update(&mut self.model, msg) {
                        run_cmd(cmd, &mut self.model);
                    }
                }
            }
            KeyAction::AwaitMore => {
                // Optionally show "Ctrl+K-" in status bar
                // Start timeout to reset after 2 seconds
            }
            KeyAction::NoMatch => {
                // Not a shortcut - let ReceivedCharacter handle text input
            }
        }
    }

    fn handle_char_event(&mut self, ch: char) {
        // Ignore control characters
        if ch.is_control() {
            return;
        }

        let msg = Msg::Document(DocumentMsg::InsertChar(ch));
        if let Some(cmd) = update(&mut self.model, msg) {
            run_cmd(cmd, &mut self.model);
        }
    }
}
```

---

## Implementation Plan

### Phase 1: Core Types (2h)

- [ ] Create `src/keymap/` module structure
- [ ] Implement `Keystroke`, `Modifiers`, `KeyCode` types
- [ ] Implement winit adapter

### Phase 2: Command Mapping (2h)

- [ ] Define `Command` enum with all editor actions
- [ ] Implement `Command::to_msgs()` mapping
- [ ] Ensure all existing keybinding behaviors are covered

### Phase 3: Keymap Dispatch (3h)

- [ ] Implement `Keybinding` struct
- [ ] Implement `Keymap` with sequence handling
- [ ] Add `KeyAction` result type
- [ ] Add sequence timeout handling

### Phase 4: Default Bindings (1h)

- [ ] Create `defaults.rs` with standard bindings
- [ ] Test platform-specific modifier mapping (mod → Cmd/Ctrl)

### Phase 5: User Configuration (2h)

- [ ] Implement TOML config parsing
- [ ] Implement config file loading from `~/.config/token-editor/`
- [ ] Merge user bindings with defaults

### Phase 6: Context System (2h)

- [ ] Implement `KeyContext` from `AppModel`
- [ ] Implement `Condition` enum and evaluation
- [ ] Parse simple when expressions

### Phase 7: Integration (2h)

- [ ] Integrate `Keymap` into event loop
- [ ] Remove hardcoded key handling from current code
- [ ] Add sequence state display to status bar (optional)

### Phase 8: Testing (1h)

- [ ] Unit tests for keystroke parsing
- [ ] Unit tests for sequence matching
- [ ] Unit tests for condition evaluation

---

## Command Reference

| Command Name            | Description            | Default Binding (macOS) |
| ----------------------- | ---------------------- | ----------------------- |
| `file.save`             | Save current file      | `⌘S`                    |
| `file.save_as`          | Save as new file       | `⌘⇧S`                   |
| `file.new`              | Create new file        | `⌘N`                    |
| `file.open`             | Open file              | `⌘O`                    |
| `file.close`            | Close current file     | `⌘W`                    |
| `app.quit`              | Quit application       | `⌘Q`                    |
| `edit.undo`             | Undo last change       | `⌘Z`                    |
| `edit.redo`             | Redo last change       | `⌘⇧Z`                   |
| `edit.copy`             | Copy selection         | `⌘C`                    |
| `edit.cut`              | Cut selection          | `⌘X`                    |
| `edit.paste`            | Paste from clipboard   | `⌘V`                    |
| `edit.select_all`       | Select all text        | `⌘A`                    |
| `edit.delete_line`      | Delete current line    | `⌘⇧K`                   |
| `edit.duplicate_line`   | Duplicate current line | `⌘⇧D`                   |
| `cursor.up`             | Move cursor up         | `↑`                     |
| `cursor.down`           | Move cursor down       | `↓`                     |
| `cursor.left`           | Move cursor left       | `←`                     |
| `cursor.right`          | Move cursor right      | `→`                     |
| `cursor.line_start`     | Move to line start     | `Home`                  |
| `cursor.line_end`       | Move to line end       | `End`                   |
| `cursor.document_start` | Move to document start | `⌘Home`                 |
| `cursor.document_end`   | Move to document end   | `⌘End`                  |
| `cursor.word_left`      | Move cursor word left  | `⌥←`                    |
| `cursor.word_right`     | Move cursor word right | `⌥→`                    |
| `cursor.page_up`        | Page up                | `PageUp`                |
| `cursor.page_down`      | Page down              | `PageDown`              |

---

## Future Considerations

- **Vim/Modal Mode:** Add `mode` to `KeyContext` and support mode-specific bindings
- **Command Palette:** Commands should be discoverable and executable from palette
- **Dynamic Reload:** Watch keymap file for changes and reload without restart
- **Conflict Detection:** Warn about conflicting bindings in config
- **Keymap Profiles:** Support multiple keymap files (e.g., vim.toml, emacs.toml)
- **Record Macro:** Record keystroke sequences as custom commands

---

## References

- [VS Code Keybindings](https://code.visualstudio.com/docs/getstarted/keybindings)
- [Helix Keymap](https://docs.helix-editor.com/keymap.html)
- [Zed Keybindings](https://zed.dev/docs/key-bindings)
- [EDITOR_UI_REFERENCE.md Chapter 13](/docs/EDITOR_UI_REFERENCE.md#chapter-13-keyboard-mapping-and-command-dispatch)
