//! Persistent command palette usage tracking: recency, frequency, and pins.
//!
//! Shapes adopted verbatim from
//! [docs/future/command-palette-enhancements.md](../docs/future/command-palette-enhancements.md)
//! (overlay-surface.md Phase 4), cloned off the `recent_files.rs`
//! load/save/`config_paths` template.

use crate::commands::CommandId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::SystemTime;

/// Tracks usage statistics for a single command.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CommandUsage {
    /// Number of times this command has been executed.
    #[serde(default)]
    pub execution_count: u32,
    /// Timestamp of last execution (Unix epoch seconds).
    #[serde(default)]
    pub last_used: u64,
    /// Whether this command is pinned to the top of the Commands tab.
    #[serde(default)]
    pub is_pinned: bool,
}

/// Persistent command history for recency-boosted ranking and pins.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CommandHistory {
    /// Usage data keyed by `CommandId`'s `Debug` name (stable across
    /// re-orderings of the `CommandId` enum, unlike a discriminant).
    pub commands: HashMap<String, CommandUsage>,
    /// Schema version for forward compatibility.
    #[serde(default)]
    pub version: u32,
}

/// Serialization key for a `CommandId` — its `Debug` name (e.g.
/// `"GotoLine"`). Stable across variant re-ordering; only breaks if a
/// variant is renamed, same tradeoff `recent_files.rs` accepts for paths.
fn key(id: CommandId) -> String {
    format!("{id:?}")
}

impl CommandHistory {
    pub const CURRENT_VERSION: u32 = 1;

    /// Load history from disk, returning empty history on any error
    /// (missing file, unreadable, corrupt JSON).
    pub fn load() -> Self {
        let Some(path) = crate::config_paths::command_history_path() else {
            return Self::default();
        };
        match std::fs::read_to_string(&path) {
            Ok(contents) => serde_json::from_str(&contents).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    /// Save history to disk.
    pub fn save(&self) -> std::io::Result<()> {
        let Some(path) = crate::config_paths::command_history_path() else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "No config directory available",
            ));
        };
        crate::config_paths::ensure_all_config_dirs();
        let contents = serde_json::to_string_pretty(self)?;
        std::fs::write(path, contents)
    }

    /// Record that a command was executed (bumps count + recency).
    pub fn record_execution(&mut self, id: CommandId) {
        let entry = self.commands.entry(key(id)).or_default();
        entry.execution_count += 1;
        entry.last_used = now_epoch_secs();
    }

    /// Toggle pin status for a command.
    pub fn toggle_pin(&mut self, id: CommandId) {
        let entry = self.commands.entry(key(id)).or_default();
        entry.is_pinned = !entry.is_pinned;
    }

    /// Check if a command is pinned.
    pub fn is_pinned(&self, id: CommandId) -> bool {
        self.commands.get(&key(id)).is_some_and(|u| u.is_pinned)
    }

    /// Recency+frequency score for ranking (higher = more recently/frequently
    /// used); `last_used` dominates, `execution_count` breaks ties.
    pub fn recency_score(&self, id: CommandId) -> u64 {
        self.commands
            .get(&key(id))
            .map(|u| u.last_used * 1000 + u.execution_count as u64)
            .unwrap_or(0)
    }

    /// Up to `n` command ids, most-recently-used first, excluding never-used
    /// commands — the Commands/All-tab "Recently used" section source.
    pub fn recent_commands(&self, all: &[CommandId], n: usize) -> Vec<CommandId> {
        let mut used: Vec<(CommandId, u64)> = all
            .iter()
            .filter_map(|&id| {
                let usage = self.commands.get(&key(id))?;
                (usage.last_used > 0).then_some((id, usage.last_used))
            })
            .collect();
        used.sort_by_key(|&(_, last_used)| std::cmp::Reverse(last_used));
        used.into_iter().take(n).map(|(id, _)| id).collect()
    }
}

fn now_epoch_secs() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // `XDG_CONFIG_HOME` is process-wide state; serialize the tests below
    // that point it at a scratch dir so they can't interleave with each
    // other across `cargo test`'s parallel threads.
    static XDG_CONFIG_HOME_LOCK: Mutex<()> = Mutex::new(());

    /// Run `body` with `XDG_CONFIG_HOME` pointed at a fresh scratch dir,
    /// restoring the previous value afterwards.
    fn with_scratch_config_dir(body: impl FnOnce()) {
        let _guard = XDG_CONFIG_HOME_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let dir = tempfile::tempdir().unwrap();
        let previous = std::env::var_os("XDG_CONFIG_HOME");
        std::env::set_var("XDG_CONFIG_HOME", dir.path());

        body();

        match previous {
            Some(v) => std::env::set_var("XDG_CONFIG_HOME", v),
            None => std::env::remove_var("XDG_CONFIG_HOME"),
        }
    }

    #[test]
    fn load_returns_empty_history_for_missing_file() {
        // Point XDG_CONFIG_HOME at a scratch dir with no command-history.json
        // in it, so `load()` genuinely exercises its missing-file branch
        // rather than asserting properties of `Default` in isolation.
        with_scratch_config_dir(|| {
            let history = CommandHistory::load();
            assert!(history.commands.is_empty());
            assert_eq!(history.version, 0);
        });
    }

    #[test]
    fn load_round_trips_a_saved_history() {
        with_scratch_config_dir(|| {
            let mut saved = CommandHistory {
                version: CommandHistory::CURRENT_VERSION,
                ..Default::default()
            };
            saved.record_execution(CommandId::SaveFile);
            saved.save().unwrap();
            let loaded = CommandHistory::load();

            assert_eq!(
                loaded.commands[&key(CommandId::SaveFile)].execution_count,
                1
            );
        });
    }

    #[test]
    fn record_execution_bumps_count_and_recency() {
        let mut history = CommandHistory::default();
        history.record_execution(CommandId::SaveFile);
        history.record_execution(CommandId::SaveFile);
        let usage = &history.commands[&key(CommandId::SaveFile)];
        assert_eq!(usage.execution_count, 2);
        assert!(usage.last_used > 0);
    }

    #[test]
    fn toggle_pin_flips_and_is_idempotent_on_unused_command() {
        let mut history = CommandHistory::default();
        assert!(!history.is_pinned(CommandId::GotoLine));
        history.toggle_pin(CommandId::GotoLine);
        assert!(history.is_pinned(CommandId::GotoLine));
        history.toggle_pin(CommandId::GotoLine);
        assert!(!history.is_pinned(CommandId::GotoLine));
    }

    #[test]
    fn recency_score_prefers_more_recently_used() {
        let mut history = CommandHistory::default();
        history.commands.insert(
            key(CommandId::SaveFile),
            CommandUsage {
                execution_count: 1,
                last_used: 100,
                is_pinned: false,
            },
        );
        history.commands.insert(
            key(CommandId::OpenFile),
            CommandUsage {
                execution_count: 1,
                last_used: 200,
                is_pinned: false,
            },
        );
        assert!(
            history.recency_score(CommandId::OpenFile) > history.recency_score(CommandId::SaveFile)
        );
    }

    #[test]
    fn recent_commands_excludes_never_used_and_caps_at_n() {
        let mut history = CommandHistory::default();
        history.record_execution(CommandId::SaveFile);
        history.record_execution(CommandId::OpenFile);
        history.record_execution(CommandId::GotoLine);
        let all = [
            CommandId::SaveFile,
            CommandId::OpenFile,
            CommandId::GotoLine,
            CommandId::Undo, // never used — excluded
        ];
        let recent = history.recent_commands(&all, 2);
        assert_eq!(recent.len(), 2);
        assert!(!recent.contains(&CommandId::Undo));
    }

    #[test]
    fn serialization_roundtrip() {
        let mut history = CommandHistory {
            version: CommandHistory::CURRENT_VERSION,
            ..Default::default()
        };
        history.record_execution(CommandId::SaveFile);
        history.toggle_pin(CommandId::SaveFile);

        let json = serde_json::to_string(&history).unwrap();
        let loaded: CommandHistory = serde_json::from_str(&json).unwrap();

        assert!(loaded.is_pinned(CommandId::SaveFile));
        assert_eq!(
            loaded.commands[&key(CommandId::SaveFile)].execution_count,
            1
        );
        assert_eq!(loaded.version, 1);
    }
}
