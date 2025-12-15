//! Configuration system tests
//!
//! Tests for config paths, editor config, and keymap loading/merging.

use token::config::EditorConfig;
use token::config_paths;
use token::keymap::{
    default_bindings, merge_bindings, Command, Condition, Keybinding, KeyCode, Keystroke, Modifiers,
};

// ========================================================================
// Config Paths Tests
// ========================================================================

#[test]
fn test_config_dir_returns_some() {
    assert!(config_paths::config_dir().is_some());
}

#[test]
fn test_config_dir_contains_token_editor() {
    let dir = config_paths::config_dir().unwrap();
    assert!(dir.to_string_lossy().contains("token-editor"));
}

#[test]
fn test_config_dir_uses_dot_config_on_unix() {
    #[cfg(not(target_os = "windows"))]
    {
        let dir = config_paths::config_dir().unwrap();
        assert!(
            dir.to_string_lossy().contains(".config"),
            "Expected .config in path, got: {}",
            dir.display()
        );
    }
}

#[test]
fn test_keymap_file_ends_with_yaml() {
    let path = config_paths::keymap_file().unwrap();
    assert!(path.to_string_lossy().ends_with("keymap.yaml"));
}

#[test]
fn test_config_file_ends_with_yaml() {
    let path = config_paths::config_file().unwrap();
    assert!(path.to_string_lossy().ends_with("config.yaml"));
}

#[test]
fn test_themes_dir_is_subdir_of_config() {
    let config = config_paths::config_dir().unwrap();
    let themes = config_paths::themes_dir().unwrap();
    assert!(themes.starts_with(&config));
}

// ========================================================================
// Editor Config Tests
// ========================================================================

#[test]
fn test_default_config() {
    let config = EditorConfig::default();
    assert_eq!(config.theme, "default-dark");
}

#[test]
fn test_config_path_returns_some() {
    let path = config_paths::config_file();
    if let Some(p) = path {
        let path_str = p.to_string_lossy();
        assert!(path_str.contains("token-editor"));
        assert!(path_str.contains("config.yaml"));
    }
}

#[test]
fn test_config_serialize_deserialize() {
    let config = EditorConfig {
        theme: "fleet-dark".to_string(),
    };
    let yaml = serde_yaml::to_string(&config).unwrap();
    let parsed: EditorConfig = serde_yaml::from_str(&yaml).unwrap();
    assert_eq!(parsed.theme, "fleet-dark");
}

// ========================================================================
// Keymap Helpers
// ========================================================================

fn ctrl_s() -> Keystroke {
    Keystroke::new(KeyCode::Char('s'), Modifiers::CTRL)
}

fn ctrl_z() -> Keystroke {
    Keystroke::new(KeyCode::Char('z'), Modifiers::CTRL)
}

fn ctrl_x() -> Keystroke {
    Keystroke::new(KeyCode::Char('x'), Modifiers::CTRL)
}

// ========================================================================
// Keymap Default Bindings Tests
// ========================================================================

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
    let has_down = bindings
        .iter()
        .any(|b| b.command == Command::MoveCursorDown);
    let has_left = bindings
        .iter()
        .any(|b| b.command == Command::MoveCursorLeft);
    let has_right = bindings
        .iter()
        .any(|b| b.command == Command::MoveCursorRight);

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
// Keymap Merge Tests
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
    let user = vec![Keybinding::new(ctrl_s(), Command::Unbound)];

    let merged = merge_bindings(base, user);
    assert_eq!(merged.len(), 1);
    assert_eq!(merged[0].command, Command::Undo);
}

#[test]
fn test_merge_unbound_on_nonexistent_is_noop() {
    let base = vec![Keybinding::new(ctrl_s(), Command::SaveFile)];
    let user = vec![Keybinding::new(ctrl_x(), Command::Unbound)];

    let merged = merge_bindings(base, user);
    assert_eq!(merged.len(), 1);
    assert_eq!(merged[0].command, Command::SaveFile);
}

#[test]
fn test_merge_conditional_binding_adds_not_overrides() {
    let base = vec![Keybinding::new(
        Keystroke::new(KeyCode::Tab, Modifiers::NONE),
        Command::InsertTab,
    )];
    let user = vec![Keybinding::new(
        Keystroke::new(KeyCode::Tab, Modifiers::NONE),
        Command::IndentLines,
    )
    .when(vec![Condition::HasSelection])];

    let merged = merge_bindings(base, user);
    assert_eq!(merged.len(), 2);
    let has_insert = merged.iter().any(|b| b.command == Command::InsertTab);
    let has_indent = merged.iter().any(|b| b.command == Command::IndentLines);
    assert!(has_insert && has_indent);
}

#[test]
fn test_merge_conditional_override_same_conditions() {
    let tab = Keystroke::new(KeyCode::Tab, Modifiers::NONE);

    let base = vec![Keybinding::new(tab, Command::InsertTab).when(vec![Condition::NoSelection])];
    let user = vec![Keybinding::new(tab, Command::Undo).when(vec![Condition::NoSelection])];

    let merged = merge_bindings(base, user);
    assert_eq!(merged.len(), 1);
    assert_eq!(merged[0].command, Command::Undo);
}

#[test]
fn test_keymap_file_path_structure() {
    let path = config_paths::keymap_file();
    if let Some(p) = path {
        let path_str = p.to_string_lossy();
        assert!(path_str.contains("token-editor"));
        assert!(path_str.contains("keymap.yaml"));
    }
}
