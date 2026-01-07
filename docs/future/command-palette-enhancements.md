# Command Palette Enhancements

MRU ordering, favorites/pinned commands, and improved filtering for the command palette.

> **Status:** Planned
> **Priority:** P2
> **Effort:** M
> **Created:** 2025-12-19
> **Milestone:** 1 - Navigation

---

## Table of Contents

1. [Overview](#overview)
2. [Architecture](#architecture)
3. [Data Structures](#data-structures)
4. [Keybindings](#keybindings)
5. [Implementation Plan](#implementation-plan)
6. [Testing Strategy](#testing-strategy)
7. [References](#references)

---

## Overview

### Current State

The command palette exists in `src/model/ui.rs` as `CommandPaletteState`:

```rust
pub struct CommandPaletteState {
    pub editable: EditableState<StringBuffer>,
    pub selected_index: usize,
}
```

Commands are defined in `src/keymap/command.rs` and can be executed via the palette. However:
- Commands appear in a fixed order (definition order)
- No memory of recently used commands
- No way to pin frequently used commands
- Filtering is basic substring matching

### Goals

1. **MRU (Most Recently Used) ordering** - Recently executed commands appear first
2. **Favorites/Pins** - User can pin commands to always appear at the top
3. **Improved fuzzy filtering** - Better matching algorithm (substring + initials)
4. **Usage statistics persistence** - Command usage saved to `~/.config/token-editor/command-history.json`

### Non-Goals

- Custom command aliases (future feature)
- User-defined commands or macros
- Command chaining / sequences

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────┐
│                         Command Palette Flow                             │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│  ┌──────────────────┐    ┌──────────────────┐    ┌──────────────────┐  │
│  │  Open Palette    │───▶│  Load History    │───▶│  Show Commands   │  │
│  │  (Shift+Cmd+A)   │    │  from Disk       │    │  Sorted by MRU   │  │
│  └──────────────────┘    └──────────────────┘    └──────────────────┘  │
│                                                          │               │
│                                                          ▼               │
│  ┌──────────────────┐    ┌──────────────────┐    ┌──────────────────┐  │
│  │  Type to Filter  │───▶│  Fuzzy Match +   │───▶│  Display Order:  │  │
│  │                  │    │  Score Commands  │    │  1. Pinned       │  │
│  └──────────────────┘    └──────────────────┘    │  2. MRU matches  │  │
│                                                  │  3. Other matches│  │
│                                                  └──────────────────┘  │
│                                                          │               │
│                                                          ▼               │
│  ┌──────────────────┐    ┌──────────────────┐    ┌──────────────────┐  │
│  │  Execute Command │◀───│  Press Enter     │◀───│  Navigate List   │  │
│  │  Update History  │    │                  │    │  (Up/Down)       │  │
│  └──────────────────┘    └──────────────────┘    └──────────────────┘  │
│          │                                                              │
│          ▼                                                              │
│  ┌──────────────────┐                                                   │
│  │  Persist History │                                                   │
│  │  to Disk         │                                                   │
│  └──────────────────┘                                                   │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

### Module Structure

```
src/
├── model/
│   └── ui.rs                    # Enhanced CommandPaletteState
├── command_history.rs           # NEW: Command usage tracking
└── config_paths.rs              # Path helpers (already exists)
```

### Message Flow

```
User Action              Msg                           Effect
───────────────────────────────────────────────────────────────────────
Open palette         → ModalMsg::OpenCommandPalette → Load history, show
Type filter          → ModalMsg::SetInput(text)     → Filter + sort
Navigate             → ModalMsg::SelectNext/Prev    → Update selection
Toggle pin (Cmd+P)   → ModalMsg::TogglePinCommand   → Toggle pin status
Execute              → ModalMsg::Confirm             → Run command, save
```

---

## Data Structures

### Command History Entry

```rust
// src/command_history.rs

use crate::keymap::Command;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::SystemTime;

/// Tracks usage statistics for a single command
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandUsage {
    /// Number of times this command has been executed
    pub execution_count: u32,
    /// Timestamp of last execution (Unix epoch seconds)
    pub last_used: u64,
    /// Whether this command is pinned to top
    pub is_pinned: bool,
}

impl Default for CommandUsage {
    fn default() -> Self {
        Self {
            execution_count: 0,
            last_used: 0,
            is_pinned: false,
        }
    }
}

/// Persistent command history for MRU and favorites
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CommandHistory {
    /// Usage data keyed by command name (string for serialization)
    pub commands: HashMap<String, CommandUsage>,
    /// Schema version for forward compatibility
    #[serde(default)]
    pub version: u32,
}

impl CommandHistory {
    pub const CURRENT_VERSION: u32 = 1;

    /// Load history from disk, returning empty history on error
    pub fn load() -> Self {
        let path = crate::config_paths::command_history_path();
        match std::fs::read_to_string(&path) {
            Ok(contents) => {
                serde_json::from_str(&contents).unwrap_or_default()
            }
            Err(_) => Self::default(),
        }
    }

    /// Save history to disk
    pub fn save(&self) -> std::io::Result<()> {
        let path = crate::config_paths::command_history_path();
        let contents = serde_json::to_string_pretty(self)?;
        std::fs::write(path, contents)
    }

    /// Record that a command was executed
    pub fn record_execution(&mut self, command: Command) {
        let name = command.name();
        let entry = self.commands.entry(name.to_string()).or_default();
        entry.execution_count += 1;
        entry.last_used = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
    }

    /// Toggle pin status for a command
    pub fn toggle_pin(&mut self, command: Command) {
        let name = command.name();
        let entry = self.commands.entry(name.to_string()).or_default();
        entry.is_pinned = !entry.is_pinned;
    }

    /// Check if a command is pinned
    pub fn is_pinned(&self, command: Command) -> bool {
        self.commands
            .get(command.name())
            .map(|u| u.is_pinned)
            .unwrap_or(false)
    }

    /// Get MRU score for sorting (higher = more recently/frequently used)
    pub fn mru_score(&self, command: Command) -> u64 {
        self.commands
            .get(command.name())
            .map(|u| {
                // Combine recency and frequency
                // last_used dominates, but execution_count breaks ties
                u.last_used * 1000 + u.execution_count as u64
            })
            .unwrap_or(0)
    }
}
```

### Enhanced Command Palette State

```rust
// Updated in src/model/ui.rs

use crate::command_history::CommandHistory;
use crate::keymap::Command;

/// Filtered and scored command for display
#[derive(Debug, Clone)]
pub struct ScoredCommand {
    /// The command
    pub command: Command,
    /// Display name
    pub name: String,
    /// Fuzzy match score (0 = no match, higher = better)
    pub match_score: u32,
    /// Whether this command is pinned
    pub is_pinned: bool,
    /// MRU score for ordering
    pub mru_score: u64,
}

impl ScoredCommand {
    /// Compute sort key: pinned first, then MRU, then match quality
    pub fn sort_key(&self) -> (bool, u64, u32) {
        // Tuple sorts: pinned (descending), mru (descending), match (descending)
        (!self.is_pinned, u64::MAX - self.mru_score, u32::MAX - self.match_score)
    }
}

/// State for the command palette modal
#[derive(Debug, Clone)]
pub struct CommandPaletteState {
    /// Editable state for the input field
    pub editable: EditableState<StringBuffer>,
    /// Index of selected command in filtered list
    pub selected_index: usize,
    /// Cached filtered and sorted commands
    pub filtered_commands: Vec<ScoredCommand>,
    /// Command history (loaded on open, saved on execute)
    pub history: CommandHistory,
}

impl CommandPaletteState {
    /// Create new state, loading history from disk
    pub fn new() -> Self {
        let history = CommandHistory::load();
        let mut state = Self {
            editable: EditableState::new(StringBuffer::new(), EditConstraints::single_line()),
            selected_index: 0,
            filtered_commands: Vec::new(),
            history,
        };
        state.update_filtered_commands("");
        state
    }

    /// Update filtered commands based on query
    pub fn update_filtered_commands(&mut self, query: &str) {
        let all_commands = Command::all();

        self.filtered_commands = all_commands
            .into_iter()
            .filter_map(|cmd| {
                let name = cmd.display_name();
                let match_score = fuzzy_match(query, &name);

                // Include if query is empty or there's a match
                if query.is_empty() || match_score > 0 {
                    Some(ScoredCommand {
                        command: cmd,
                        name,
                        match_score,
                        is_pinned: self.history.is_pinned(cmd),
                        mru_score: self.history.mru_score(cmd),
                    })
                } else {
                    None
                }
            })
            .collect();

        // Sort by: pinned first, then MRU score, then match score
        self.filtered_commands.sort_by_key(|sc| sc.sort_key());

        // Reset selection to top
        self.selected_index = 0;
    }

    /// Get currently selected command
    pub fn selected_command(&self) -> Option<Command> {
        self.filtered_commands
            .get(self.selected_index)
            .map(|sc| sc.command)
    }

    /// Record execution and save history
    pub fn record_execution(&mut self, command: Command) {
        self.history.record_execution(command);
        if let Err(e) = self.history.save() {
            tracing::warn!("Failed to save command history: {}", e);
        }
    }

    /// Toggle pin status for selected command
    pub fn toggle_selected_pin(&mut self) {
        if let Some(cmd) = self.selected_command() {
            self.history.toggle_pin(cmd);
            let query = self.editable.text();
            self.update_filtered_commands(&query);
            // Save immediately
            if let Err(e) = self.history.save() {
                tracing::warn!("Failed to save command history: {}", e);
            }
        }
    }
}
```

### Fuzzy Matching Algorithm

```rust
// src/command_history.rs

/// Simple fuzzy matching: substring match + consecutive bonus
///
/// Returns score (0 = no match, higher = better match)
pub fn fuzzy_match(query: &str, target: &str) -> u32 {
    if query.is_empty() {
        return 1; // Empty query matches everything with base score
    }

    let query_lower = query.to_lowercase();
    let target_lower = target.to_lowercase();

    // Exact match = highest score
    if target_lower == query_lower {
        return 10000;
    }

    // Prefix match = very high score
    if target_lower.starts_with(&query_lower) {
        return 5000 + (1000 - target.len() as u32).max(0);
    }

    // Word boundary match (initials)
    // e.g., "gln" matches "Go to Line Number"
    let words: Vec<&str> = target.split_whitespace().collect();
    let initials: String = words.iter().filter_map(|w| w.chars().next()).collect();
    if initials.to_lowercase().starts_with(&query_lower) {
        return 3000 + (100 - query.len() as u32).max(0);
    }

    // Substring match
    if let Some(pos) = target_lower.find(&query_lower) {
        // Earlier position = better score
        return 1000 + (500 - pos as u32).max(0);
    }

    // Character-by-character fuzzy match
    let mut query_chars = query_lower.chars().peekable();
    let mut consecutive_bonus = 0u32;
    let mut last_match_pos = 0usize;
    let mut total_score = 0u32;

    for (i, c) in target_lower.chars().enumerate() {
        if query_chars.peek() == Some(&c) {
            query_chars.next();

            // Bonus for consecutive matches
            if i == last_match_pos + 1 {
                consecutive_bonus += 10;
            } else {
                consecutive_bonus = 0;
            }

            total_score += 10 + consecutive_bonus;
            last_match_pos = i;
        }
    }

    // All query characters must be found
    if query_chars.peek().is_none() {
        total_score
    } else {
        0
    }
}
```

### Config Paths Extension

```rust
// Add to src/config_paths.rs

/// Path to command history file
pub fn command_history_path() -> PathBuf {
    config_dir().join("command-history.json")
}
```

### Command Name Helper

```rust
// Add to src/keymap/command.rs

impl Command {
    /// Get the kebab-case name of this command (for serialization)
    pub fn name(&self) -> &'static str {
        match self {
            Command::MoveCursorUp => "move-cursor-up",
            Command::MoveCursorDown => "move-cursor-down",
            // ... all variants
            Command::ToggleCommandPalette => "toggle-command-palette",
            Command::GotoLine => "goto-line",
            // etc.
        }
    }

    /// Get human-readable display name
    pub fn display_name(&self) -> String {
        match self {
            Command::MoveCursorUp => "Move Cursor Up".to_string(),
            Command::MoveCursorDown => "Move Cursor Down".to_string(),
            Command::GotoLine => "Go to Line".to_string(),
            Command::ToggleCommandPalette => "Command Palette".to_string(),
            Command::SaveFile => "Save File".to_string(),
            Command::OpenFile => "Open File".to_string(),
            // ... all variants with human-readable names
            _ => {
                // Fallback: convert enum name to title case
                let name = format!("{:?}", self);
                // Insert spaces before capitals
                let mut result = String::new();
                for (i, c) in name.chars().enumerate() {
                    if i > 0 && c.is_uppercase() {
                        result.push(' ');
                    }
                    result.push(c);
                }
                result
            }
        }
    }

    /// Get all available commands
    pub fn all() -> Vec<Command> {
        vec![
            Command::MoveCursorUp,
            Command::MoveCursorDown,
            Command::MoveCursorLeft,
            Command::MoveCursorRight,
            // ... exhaustive list of all commands
        ]
    }
}
```

---

## Keybindings

| Action | Mac | Windows/Linux | Notes |
|--------|-----|---------------|-------|
| Open command palette | Shift+Cmd+A | Ctrl+Shift+A | Already exists |
| Navigate up | Up Arrow | Up Arrow | Already exists |
| Navigate down | Down Arrow | Down Arrow | Already exists |
| Execute command | Enter | Enter | Already exists |
| Toggle pin | Cmd+P | Ctrl+P | NEW: Pin/unpin selected |
| Close palette | Escape | Escape | Already exists |

---

## Implementation Plan

### Phase 1: Core Data Structures

**Files:** `src/command_history.rs`, `src/config_paths.rs`

- [ ] Create `CommandHistory` struct with serialization
- [ ] Add `CommandUsage` with execution count, timestamp, pin status
- [ ] Implement `load()` and `save()` methods
- [ ] Add `command_history_path()` to config paths
- [ ] Add `Command::name()` and `Command::display_name()` methods
- [ ] Add `Command::all()` to enumerate all commands

**Test:** `CommandHistory::load()` returns empty history for missing file.

### Phase 2: Fuzzy Matching

**Files:** `src/command_history.rs`

- [ ] Implement `fuzzy_match()` function
- [ ] Support exact match, prefix match, initials match, substring match
- [ ] Add consecutive character bonus
- [ ] Add comprehensive unit tests for matching edge cases

**Test:** `fuzzy_match("gln", "Go to Line Number")` returns high score.

### Phase 3: Enhanced Palette State

**Files:** `src/model/ui.rs`

- [ ] Add `ScoredCommand` struct
- [ ] Extend `CommandPaletteState` with `filtered_commands` and `history`
- [ ] Implement `update_filtered_commands()` with scoring and sorting
- [ ] Add `record_execution()` and `toggle_selected_pin()`
- [ ] Update `Default` impl to load history

**Test:** Pinned commands appear first in filtered list.

### Phase 4: Message Handling

**Files:** `src/messages.rs`, `src/update/ui.rs`

- [ ] Add `ModalMsg::TogglePinCommand` message
- [ ] Update `ModalMsg::SetInput` handler to call `update_filtered_commands()`
- [ ] Update `ModalMsg::Confirm` to call `record_execution()`
- [ ] Handle `TogglePinCommand` in modal update

**Test:** Executing a command updates its MRU score.

### Phase 5: Rendering Updates

**Files:** `src/view/modal.rs`

- [ ] Show pin indicator (e.g., star icon) for pinned commands
- [ ] Highlight matched characters in command names
- [ ] Show keyboard shortcut hint next to command (if bound)

**Test:** Pinned commands show visual indicator.

### Phase 6: Polish

- [ ] Add history file migration for version bumps
- [ ] Limit history size (e.g., prune commands not used in 90 days)
- [ ] Handle concurrent access gracefully (file locking or ignore errors)
- [ ] Add transient message on pin/unpin ("Command pinned" / "Command unpinned")

---

## Testing Strategy

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fuzzy_match_exact() {
        assert_eq!(fuzzy_match("save file", "Save File"), 10000);
    }

    #[test]
    fn test_fuzzy_match_prefix() {
        let score = fuzzy_match("save", "Save File");
        assert!(score >= 5000);
    }

    #[test]
    fn test_fuzzy_match_initials() {
        let score = fuzzy_match("sf", "Save File");
        assert!(score >= 3000);
    }

    #[test]
    fn test_fuzzy_match_substring() {
        let score = fuzzy_match("file", "Save File");
        assert!(score >= 1000);
    }

    #[test]
    fn test_fuzzy_match_no_match() {
        assert_eq!(fuzzy_match("xyz", "Save File"), 0);
    }

    #[test]
    fn test_mru_ordering() {
        let mut history = CommandHistory::default();
        history.record_execution(Command::SaveFile);
        history.record_execution(Command::OpenFile);

        assert!(history.mru_score(Command::OpenFile) > history.mru_score(Command::SaveFile));
    }

    #[test]
    fn test_pinned_first() {
        let mut state = CommandPaletteState::new();
        state.history.toggle_pin(Command::GotoLine);
        state.update_filtered_commands("");

        assert_eq!(state.filtered_commands[0].command, Command::GotoLine);
    }
}
```

### Integration Tests

```rust
// tests/command_palette_tests.rs

#[test]
fn test_palette_mru_persistence() {
    // Create temp config directory
    // Execute commands via palette
    // Close and reopen palette
    // Verify MRU order persisted
}

#[test]
fn test_palette_filter_updates() {
    // Open palette
    // Type partial command name
    // Verify filtered list updates
    // Verify selection resets to 0
}
```

---

## References

- **Existing code:** `src/model/ui.rs` - `CommandPaletteState`
- **Commands:** `src/keymap/command.rs` - `Command` enum
- **Config paths:** `src/config_paths.rs` - Configuration file locations
- **Similar feature:** VS Code command palette with MRU
- **Fuzzy matching:** Inspired by fzf and sublime text algorithms
