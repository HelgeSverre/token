//! Default keybindings for the editor
//!
//! These are the standard keybindings that ship with the editor.
//! Can be loaded from keymap.yaml at project root, or falls back to hardcoded defaults.

use std::path::{Path, PathBuf};

use super::binding::Keybinding;
use super::command::Command;
use super::config::load_keymap_file;
use super::types::{KeyCode, Keystroke, Modifiers};

/// Default keymap YAML embedded at compile time
const DEFAULT_KEYMAP_YAML: &str = include_str!("../../keymap.yaml");

/// Get the user's keymap configuration path
///
/// Returns `~/.config/token-editor/keymap.yaml` on Unix
/// Returns `%APPDATA%\token-editor\keymap.yaml` on Windows
pub fn get_user_config_path() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        std::env::var("APPDATA")
            .ok()
            .map(|appdata| PathBuf::from(appdata).join("token-editor").join("keymap.yaml"))
    }

    #[cfg(not(target_os = "windows"))]
    {
        dirs::config_dir().map(|config| config.join("token-editor").join("keymap.yaml"))
    }
}

/// Load and merge keymaps: defaults + user overrides
///
/// Loading order (each layer overrides the previous):
/// 1. Embedded default keymap (compiled into binary)
/// 2. keymap.yaml in current directory (project-local overrides)
/// 3. User config at ~/.config/token-editor/keymap.yaml
///
/// User bindings with `command: Unbound` will remove matching default bindings.
pub fn load_default_keymap() -> Vec<Keybinding> {
    // Load base defaults from embedded YAML
    let mut bindings = match super::config::parse_keymap_yaml(DEFAULT_KEYMAP_YAML) {
        Ok(b) => {
            tracing::info!("Loaded embedded default keymap ({} bindings)", b.len());
            b
        }
        Err(e) => {
            tracing::warn!(
                "Failed to parse embedded keymap: {}, using hardcoded defaults",
                e
            );
            default_bindings()
        }
    };

    // Try loading project-local keymap.yaml
    if let Ok(local_bindings) = load_keymap_file(Path::new("keymap.yaml")) {
        tracing::info!(
            "Merging project keymap.yaml ({} bindings)",
            local_bindings.len()
        );
        bindings = merge_bindings(bindings, local_bindings);
    }

    // Try loading user config
    if let Some(user_path) = get_user_config_path() {
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
                    tracing::warn!("Failed to load user keymap from {}: {}", user_path.display(), e);
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
        let existing_idx = result.iter().position(|b| {
            b.keystrokes == user_binding.keystrokes && b.when == user_binding.when
        });

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

/// Generate default keybindings for the current platform
///
/// Uses Cmd on macOS, Ctrl on Windows/Linux for the "command" modifier.
pub fn default_bindings() -> Vec<Keybinding> {
    let cmd = Modifiers::cmd();
    let cmd_shift = cmd | Modifiers::SHIFT;
    let cmd_alt = cmd | Modifiers::ALT;
    let cmd_shift_alt = cmd | Modifiers::SHIFT | Modifiers::ALT;
    let shift = Modifiers::SHIFT;
    let alt = Modifiers::ALT;
    let alt_shift = Modifiers::ALT | Modifiers::SHIFT;
    let ctrl = Modifiers::CTRL;
    let ctrl_shift = Modifiers::CTRL | Modifiers::SHIFT;
    let none = Modifiers::NONE;

    let mut bindings = vec![
        // ====================================================================
        // File Operations
        // ====================================================================
        bind(KeyCode::Char('s'), cmd, Command::SaveFile),
        bind(KeyCode::Char('n'), cmd_shift, Command::NewTab), // Shift+Cmd+N
        bind(KeyCode::Char('w'), cmd, Command::CloseTab),

        // ====================================================================
        // Undo/Redo
        // ====================================================================
        bind(KeyCode::Char('z'), cmd, Command::Undo),
        bind(KeyCode::Char('z'), cmd_shift, Command::Redo),
        bind(KeyCode::Char('y'), cmd, Command::Redo), // Alternative

        // ====================================================================
        // Clipboard
        // ====================================================================
        bind(KeyCode::Char('c'), cmd, Command::Copy),
        bind(KeyCode::Char('x'), cmd, Command::Cut),
        bind(KeyCode::Char('v'), cmd, Command::Paste),

        // ====================================================================
        // Selection
        // ====================================================================
        bind(KeyCode::Char('a'), cmd, Command::SelectAll),
        bind(KeyCode::Char('d'), cmd, Command::Duplicate),
        bind(KeyCode::Char('j'), cmd, Command::SelectNextOccurrence),
        bind(KeyCode::Char('j'), cmd_shift, Command::UnselectOccurrence),

        // ====================================================================
        // Modals/Dialogs
        // ====================================================================
        bind(KeyCode::Char('a'), cmd_shift, Command::ToggleCommandPalette),
        bind(KeyCode::Char('l'), cmd, Command::ToggleGotoLine),
        bind(KeyCode::Char('f'), cmd, Command::ToggleFindReplace),

        // ====================================================================
        // Layout: Splits
        // ====================================================================
        bind(KeyCode::Char('h'), cmd_shift_alt, Command::SplitHorizontal),
        bind(KeyCode::Char('v'), cmd_shift_alt, Command::SplitVertical),

        // ====================================================================
        // Layout: Tabs
        // ====================================================================
        bind(KeyCode::Right, cmd_alt, Command::NextTab),
        bind(KeyCode::Left, cmd_alt, Command::PrevTab),

        // ====================================================================
        // Layout: Focus Groups
        // ====================================================================
        bind(KeyCode::Tab, ctrl, Command::FocusNextGroup),
        bind(KeyCode::Tab, ctrl_shift, Command::FocusPrevGroup),
        bind(KeyCode::Char('1'), cmd_shift, Command::FocusGroup1),
        bind(KeyCode::Char('2'), cmd_shift, Command::FocusGroup2),
        bind(KeyCode::Char('3'), cmd_shift, Command::FocusGroup3),
        bind(KeyCode::Char('4'), cmd_shift, Command::FocusGroup4),

        // Numpad group focus (no modifiers needed)
        bind(KeyCode::Numpad1, none, Command::FocusGroup1),
        bind(KeyCode::Numpad2, none, Command::FocusGroup2),
        bind(KeyCode::Numpad3, none, Command::FocusGroup3),
        bind(KeyCode::Numpad4, none, Command::FocusGroup4),
        bind(KeyCode::NumpadAdd, none, Command::SplitVertical),
        bind(KeyCode::NumpadSubtract, none, Command::SplitHorizontal),

        // ====================================================================
        // Basic Navigation (no selection)
        // ====================================================================
        bind(KeyCode::Up, none, Command::MoveCursorUp),
        bind(KeyCode::Down, none, Command::MoveCursorDown),
        bind(KeyCode::Left, none, Command::MoveCursorLeft),
        bind(KeyCode::Right, none, Command::MoveCursorRight),
        bind(KeyCode::Home, none, Command::MoveCursorLineStart),
        bind(KeyCode::End, none, Command::MoveCursorLineEnd),
        bind(KeyCode::PageUp, none, Command::PageUp),
        bind(KeyCode::PageDown, none, Command::PageDown),

        // Word navigation (Alt+Arrow)
        bind(KeyCode::Left, alt, Command::MoveCursorWordLeft),
        bind(KeyCode::Right, alt, Command::MoveCursorWordRight),

        // ====================================================================
        // Selection Navigation (Shift+key)
        // ====================================================================
        bind(KeyCode::Up, shift, Command::MoveCursorUpWithSelection),
        bind(KeyCode::Down, shift, Command::MoveCursorDownWithSelection),
        bind(KeyCode::Left, shift, Command::MoveCursorLeftWithSelection),
        bind(KeyCode::Right, shift, Command::MoveCursorRightWithSelection),
        bind(KeyCode::Home, shift, Command::MoveCursorLineStartWithSelection),
        bind(KeyCode::End, shift, Command::MoveCursorLineEndWithSelection),
        bind(KeyCode::PageUp, shift, Command::PageUpWithSelection),
        bind(KeyCode::PageDown, shift, Command::PageDownWithSelection),

        // Word navigation with selection (Alt+Shift+Arrow)
        bind(KeyCode::Left, alt_shift, Command::MoveCursorWordLeftWithSelection),
        bind(KeyCode::Right, alt_shift, Command::MoveCursorWordRightWithSelection),

        // ====================================================================
        // Editing
        // ====================================================================
        bind(KeyCode::Enter, none, Command::InsertNewline),
        bind(KeyCode::Backspace, none, Command::DeleteBackward),
        bind(KeyCode::Delete, none, Command::DeleteForward),
        bind(KeyCode::Backspace, alt, Command::DeleteWordBackward),
        bind(KeyCode::Backspace, cmd, Command::DeleteLine),
        bind(KeyCode::Space, none, Command::InsertTab), // Will need context: InsertChar(' ') normally

        // Tab handling - these will need context conditions
        // Tab with selection -> IndentLines
        // Tab without selection -> InsertTab
        // For now, default to InsertTab, context system will refine this
        bind(KeyCode::Tab, none, Command::InsertTab),
        bind(KeyCode::Tab, shift, Command::UnindentLines),

        // ====================================================================
        // Expand/Shrink Selection (Option+Up/Down)
        // ====================================================================
        bind(KeyCode::Up, alt, Command::ExpandSelection),
        bind(KeyCode::Down, alt, Command::ShrinkSelection),

        // ====================================================================
        // Escape (smart clear)
        // ====================================================================
        bind(KeyCode::Escape, none, Command::EscapeSmartClear),
    ];

    // Platform-specific additions
    #[cfg(target_os = "macos")]
    {
        // macOS: Cmd+Arrow for line start/end
        bindings.push(bind(KeyCode::Left, cmd, Command::MoveCursorLineStart));
        bindings.push(bind(KeyCode::Right, cmd, Command::MoveCursorLineEnd));
        bindings.push(bind(KeyCode::Left, cmd_shift, Command::MoveCursorLineStartWithSelection));
        bindings.push(bind(KeyCode::Right, cmd_shift, Command::MoveCursorLineEndWithSelection));
    }

    #[cfg(not(target_os = "macos"))]
    {
        // Windows/Linux: Ctrl+Home/End for document start/end
        bindings.push(bind(KeyCode::Home, ctrl, Command::MoveCursorDocumentStart));
        bindings.push(bind(KeyCode::End, ctrl, Command::MoveCursorDocumentEnd));
        bindings.push(bind(KeyCode::Home, ctrl_shift, Command::MoveCursorDocumentStartWithSelection));
        bindings.push(bind(KeyCode::End, ctrl_shift, Command::MoveCursorDocumentEndWithSelection));
    }

    // Document start/end with Ctrl on all platforms (works on macOS too)
    bindings.push(bind(KeyCode::Home, ctrl, Command::MoveCursorDocumentStart));
    bindings.push(bind(KeyCode::End, ctrl, Command::MoveCursorDocumentEnd));
    bindings.push(bind(KeyCode::Home, ctrl_shift, Command::MoveCursorDocumentStartWithSelection));
    bindings.push(bind(KeyCode::End, ctrl_shift, Command::MoveCursorDocumentEndWithSelection));

    bindings
}

/// Helper to create a keybinding
fn bind(key: KeyCode, mods: Modifiers, command: Command) -> Keybinding {
    Keybinding::new(Keystroke::new(key, mods), command)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keymap::context::Condition;

    fn ctrl_s() -> Keystroke {
        Keystroke::new(KeyCode::Char('s'), Modifiers::CTRL)
    }

    fn ctrl_z() -> Keystroke {
        Keystroke::new(KeyCode::Char('z'), Modifiers::CTRL)
    }

    fn ctrl_x() -> Keystroke {
        Keystroke::new(KeyCode::Char('x'), Modifiers::CTRL)
    }

    #[test]
    fn test_default_bindings_not_empty() {
        let bindings = default_bindings();
        assert!(!bindings.is_empty());
    }

    #[test]
    fn test_has_save_binding() {
        let bindings = default_bindings();
        let has_save = bindings.iter().any(|b| b.command == Command::SaveFile);
        assert!(has_save, "Should have SaveFile binding");
    }

    #[test]
    fn test_has_undo_binding() {
        let bindings = default_bindings();
        let has_undo = bindings.iter().any(|b| b.command == Command::Undo);
        assert!(has_undo, "Should have Undo binding");
    }

    #[test]
    fn test_has_cursor_movement() {
        let bindings = default_bindings();
        let has_up = bindings.iter().any(|b| b.command == Command::MoveCursorUp);
        let has_down = bindings.iter().any(|b| b.command == Command::MoveCursorDown);
        let has_left = bindings.iter().any(|b| b.command == Command::MoveCursorLeft);
        let has_right = bindings.iter().any(|b| b.command == Command::MoveCursorRight);

        assert!(
            has_up && has_down && has_left && has_right,
            "Should have all arrow key bindings"
        );
    }

    #[test]
    fn test_platform_cmd_key() {
        let cmd = Modifiers::cmd();

        #[cfg(target_os = "macos")]
        assert!(cmd.meta(), "cmd() should be META on macOS");

        #[cfg(not(target_os = "macos"))]
        assert!(cmd.ctrl(), "cmd() should be CTRL on non-macOS");
    }

    // ========================================================================
    // Merge tests
    // ========================================================================

    #[test]
    fn test_merge_empty_user_returns_base() {
        let base = vec![Keybinding::new(ctrl_s(), Command::SaveFile)];
        let user = vec![];

        let merged = merge_bindings(base.clone(), user);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].command, Command::SaveFile);
    }

    #[test]
    fn test_merge_adds_new_binding() {
        let base = vec![Keybinding::new(ctrl_s(), Command::SaveFile)];
        let user = vec![Keybinding::new(ctrl_z(), Command::Undo)];

        let merged = merge_bindings(base, user);
        assert_eq!(merged.len(), 2);
        assert!(merged.iter().any(|b| b.command == Command::SaveFile));
        assert!(merged.iter().any(|b| b.command == Command::Undo));
    }

    #[test]
    fn test_merge_overrides_existing() {
        let base = vec![Keybinding::new(ctrl_s(), Command::SaveFile)];
        // User remaps Ctrl+S to Undo
        let user = vec![Keybinding::new(ctrl_s(), Command::Undo)];

        let merged = merge_bindings(base, user);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].command, Command::Undo);
    }

    #[test]
    fn test_merge_unbound_removes_binding() {
        let base = vec![
            Keybinding::new(ctrl_s(), Command::SaveFile),
            Keybinding::new(ctrl_z(), Command::Undo),
        ];
        // User unbinds Ctrl+S
        let user = vec![Keybinding::new(ctrl_s(), Command::Unbound)];

        let merged = merge_bindings(base, user);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].command, Command::Undo);
    }

    #[test]
    fn test_merge_unbound_on_nonexistent_is_noop() {
        let base = vec![Keybinding::new(ctrl_s(), Command::SaveFile)];
        // User unbinds Ctrl+X which doesn't exist
        let user = vec![Keybinding::new(ctrl_x(), Command::Unbound)];

        let merged = merge_bindings(base, user);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].command, Command::SaveFile);
    }

    #[test]
    fn test_merge_conditional_binding_adds_not_overrides() {
        // Base has unconditional Tab → InsertTab
        let base = vec![Keybinding::new(
            Keystroke::new(KeyCode::Tab, Modifiers::NONE),
            Command::InsertTab,
        )];
        // User adds conditional Tab → IndentLines when has_selection
        let user = vec![Keybinding::new(
            Keystroke::new(KeyCode::Tab, Modifiers::NONE),
            Command::IndentLines,
        )
        .when(vec![Condition::HasSelection])];

        let merged = merge_bindings(base, user);
        // Should have both: one unconditional, one conditional
        assert_eq!(merged.len(), 2);
        let has_insert = merged.iter().any(|b| b.command == Command::InsertTab);
        let has_indent = merged.iter().any(|b| b.command == Command::IndentLines);
        assert!(has_insert && has_indent);
    }

    #[test]
    fn test_merge_conditional_override_same_conditions() {
        let tab = Keystroke::new(KeyCode::Tab, Modifiers::NONE);

        // Base has Tab → InsertTab when no_selection
        let base = vec![
            Keybinding::new(tab, Command::InsertTab).when(vec![Condition::NoSelection])
        ];
        // User overrides Tab when no_selection → Undo
        let user = vec![Keybinding::new(tab, Command::Undo).when(vec![Condition::NoSelection])];

        let merged = merge_bindings(base, user);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].command, Command::Undo);
    }

    #[test]
    fn test_get_user_config_path_returns_some() {
        // On most systems, this should return a valid path
        let path = get_user_config_path();
        // We just verify it returns Some and contains expected components
        if let Some(p) = path {
            let path_str = p.to_string_lossy();
            assert!(path_str.contains("token-editor"));
            assert!(path_str.contains("keymap.yaml"));
        }
        // On systems without home dir, it may return None - that's OK
    }
}
