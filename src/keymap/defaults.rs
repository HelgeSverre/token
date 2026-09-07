//! Default keybindings for the editor
//!
//! These are the standard keybindings that ship with the editor.
//! The embedded keymap YAML is the sole full default registry.

use super::binding::Keybinding;
use super::command::Command;
use super::config::load_keymap_file;
use super::types::{KeyCode, Keystroke, Modifiers};

/// Default keymap YAML embedded at compile time
const DEFAULT_KEYMAP_YAML: &str = include_str!("../../keymap.yaml");

/// Get the default keymap YAML content (for copying to user config)
pub fn get_default_keymap_yaml() -> &'static str {
    DEFAULT_KEYMAP_YAML
}

/// Load and merge keymaps: defaults + user overrides
///
/// Loading order (each layer overrides the previous):
/// 1. Embedded default keymap (compiled into binary)
/// 2. User config at ~/.config/token-editor/keymap.yaml
///
/// User bindings with `command: Unbound` will remove matching default bindings.
pub fn load_default_keymap() -> Vec<Keybinding> {
    let mut bindings = default_bindings();
    // Try loading user config
    if let Some(user_path) = crate::config_paths::keymap_file() {
        if user_path.exists() {
            match load_keymap_file(&user_path) {
                Ok(user_bindings) => {
                    tracing::info!(
                        "Merging user keymap from {} ({} bindings)",
                        user_path.display(),
                        user_bindings.len()
                    );
                    bindings = merge_bindings(bindings, user_bindings);
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to load user keymap from {}: {}",
                        user_path.display(),
                        e
                    );
                }
            }
        }
    }

    bindings
}

/// Merge user bindings into base bindings
///
/// User bindings override or extend base bindings:
/// - If user binding has same keystroke + conditions → replaces base
/// - If user binding command is `Unbound` → removes matching base bindings
/// - Otherwise → user binding is added
pub fn merge_bindings(base: Vec<Keybinding>, user: Vec<Keybinding>) -> Vec<Keybinding> {
    let mut result = base;

    for user_binding in user {
        // Handle Unbound: remove any matching base bindings
        if user_binding.command == Command::Unbound {
            result.retain(|b| {
                // Keep bindings that don't match the user's keystroke
                b.keystrokes != user_binding.keystrokes
            });
            continue;
        }

        // Check if this overrides an existing binding
        let existing_idx = result
            .iter()
            .position(|b| b.keystrokes == user_binding.keystrokes && b.when == user_binding.when);

        if let Some(idx) = existing_idx {
            // Replace existing binding
            result[idx] = user_binding;
        } else {
            // Add new binding
            result.push(user_binding);
        }
    }

    result
}

/// The embedded YAML is the single full default registry. Parsing is pure;
/// filesystem/user overrides belong to `load_default_keymap` in the runtime.
pub fn default_bindings() -> Vec<Keybinding> {
    static DEFAULTS: std::sync::OnceLock<Vec<Keybinding>> = std::sync::OnceLock::new();
    DEFAULTS
        .get_or_init(|| {
            super::config::parse_keymap_yaml(DEFAULT_KEYMAP_YAML).unwrap_or_else(|error| {
                tracing::error!("Invalid embedded keymap: {error}; using emergency bindings");
                emergency_bindings()
            })
        })
        .clone()
}

/// Deliberately minimal recovery controls, not a second default registry.
fn emergency_bindings() -> Vec<Keybinding> {
    [
        ('s', Command::SaveFile),
        ('o', Command::OpenFile),
        ('q', Command::Quit),
    ]
    .into_iter()
    .map(|(key, command)| {
        Keybinding::new(
            Keystroke::new(KeyCode::Char(key), Modifiers::cmd()),
            command,
        )
    })
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_yaml_is_valid_and_is_the_full_default_registry() {
        let parsed = super::super::parse_keymap_yaml(get_default_keymap_yaml()).unwrap();
        assert_eq!(default_bindings(), parsed);
    }

    #[test]
    fn default_registry_returns_independent_snapshots() {
        let expected = default_bindings();
        let mut changed = default_bindings();
        changed.clear();
        assert_eq!(default_bindings(), expected);
        assert!(!expected.is_empty());
    }

    #[test]
    fn default_registry_merges_overrides_conditions_chords_and_unbinding() {
        use super::super::context::Condition;

        let defaults = default_bindings();
        let save = defaults
            .iter()
            .find(|binding| binding.command == Command::SaveFile && binding.when.is_none())
            .unwrap()
            .clone();
        let mut replacement = save.clone();
        replacement.command = Command::Copy;
        let mut conditional = save.clone().when_single(Condition::HasSelection);
        conditional.command = Command::Cut;
        let chord = Keybinding::chord(
            vec![
                Keystroke::new(KeyCode::Char('k'), Modifiers::CTRL),
                Keystroke::new(KeyCode::Char('c'), Modifiers::CTRL),
            ],
            Command::Copy,
        );
        let merged = merge_bindings(
            defaults.clone(),
            vec![replacement.clone(), conditional.clone(), chord.clone()],
        );
        assert!(merged.contains(&replacement));
        assert!(merged.contains(&conditional));
        assert!(merged.contains(&chord));
        assert!(!merged.contains(&save));
        for untouched in defaults.iter().filter(|binding| **binding != save) {
            assert!(merged.contains(untouched));
        }

        let mut updated_chord = chord.clone();
        updated_chord.command = Command::Paste;
        let merged = merge_bindings(merged, vec![updated_chord.clone()]);
        assert!(merged.contains(&updated_chord));
        assert!(!merged.contains(&chord));
        let mut unbound = conditional;
        unbound.command = Command::Unbound;
        let merged = merge_bindings(merged, vec![unbound]);
        // Existing Unbound semantics remove every context for the exact
        // sequence, but leave unrelated sequences (including chords) intact.
        assert!(!merged
            .iter()
            .any(|binding| binding.keystrokes == save.keystrokes));
        assert!(merged.contains(&updated_chord));
        assert_eq!(default_bindings(), defaults);
    }

    #[test]
    fn emergency_keymap_is_deliberately_minimal() {
        let bindings = emergency_bindings();
        assert_eq!(
            bindings
                .iter()
                .map(|binding| binding.command)
                .collect::<Vec<_>>(),
            vec![Command::SaveFile, Command::OpenFile, Command::Quit]
        );
    }
}
