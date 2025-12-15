//! Integration tests for the keymap system

use super::*;

/// Embedded default keymap YAML for testing
const DEFAULT_KEYMAP_YAML: &str = include_str!("../../keymap.yaml");

#[test]
fn test_embedded_yaml_parses() {
    let bindings = parse_keymap_yaml(DEFAULT_KEYMAP_YAML)
        .expect("Embedded keymap.yaml should parse successfully");

    assert!(!bindings.is_empty(), "Should have bindings");

    // Should have essential bindings
    let has_save = bindings.iter().any(|b| b.command == Command::SaveFile);
    let has_undo = bindings.iter().any(|b| b.command == Command::Undo);
    let has_copy = bindings.iter().any(|b| b.command == Command::Copy);

    assert!(has_save, "Should have SaveFile binding");
    assert!(has_undo, "Should have Undo binding");
    assert!(has_copy, "Should have Copy binding");
}

#[test]
fn test_load_default_keymap() {
    let bindings = load_default_keymap();
    assert!(!bindings.is_empty(), "Default keymap should not be empty");
}

#[test]
fn test_keymap_with_defaults() {
    let keymap = Keymap::with_bindings(default_bindings());

    // Should have bindings
    assert!(!keymap.bindings().is_empty());

    // Should find save command
    let save_binding = keymap.binding_for(Command::SaveFile);
    assert!(save_binding.is_some());
}

#[test]
fn test_keymap_lookup_save() {
    let keymap = Keymap::with_bindings(default_bindings());
    let cmd = Modifiers::cmd();

    let stroke = Keystroke::new(KeyCode::Char('s'), cmd);
    let command = keymap.lookup(&stroke);

    assert_eq!(command, Some(Command::SaveFile));
}

#[test]
fn test_keymap_lookup_undo() {
    let keymap = Keymap::with_bindings(default_bindings());
    let cmd = Modifiers::cmd();

    let stroke = Keystroke::new(KeyCode::Char('z'), cmd);
    let command = keymap.lookup(&stroke);

    assert_eq!(command, Some(Command::Undo));
}

#[test]
fn test_keymap_lookup_arrow_keys() {
    let keymap = Keymap::with_bindings(default_bindings());

    // Plain arrow keys
    assert_eq!(
        keymap.lookup(&Keystroke::key(KeyCode::Up)),
        Some(Command::MoveCursorUp)
    );
    assert_eq!(
        keymap.lookup(&Keystroke::key(KeyCode::Down)),
        Some(Command::MoveCursorDown)
    );
    assert_eq!(
        keymap.lookup(&Keystroke::key(KeyCode::Left)),
        Some(Command::MoveCursorLeft)
    );
    assert_eq!(
        keymap.lookup(&Keystroke::key(KeyCode::Right)),
        Some(Command::MoveCursorRight)
    );
}

#[test]
fn test_keymap_lookup_shift_arrows() {
    let keymap = Keymap::with_bindings(default_bindings());
    let shift = Modifiers::SHIFT;

    assert_eq!(
        keymap.lookup(&Keystroke::new(KeyCode::Up, shift)),
        Some(Command::MoveCursorUpWithSelection)
    );
    assert_eq!(
        keymap.lookup(&Keystroke::new(KeyCode::Down, shift)),
        Some(Command::MoveCursorDownWithSelection)
    );
}

#[test]
fn test_keymap_lookup_word_navigation() {
    let keymap = Keymap::with_bindings(default_bindings());
    let alt = Modifiers::ALT;

    assert_eq!(
        keymap.lookup(&Keystroke::new(KeyCode::Left, alt)),
        Some(Command::MoveCursorWordLeft)
    );
    assert_eq!(
        keymap.lookup(&Keystroke::new(KeyCode::Right, alt)),
        Some(Command::MoveCursorWordRight)
    );
}

#[test]
fn test_keymap_display_for_save() {
    let keymap = Keymap::with_bindings(default_bindings());

    let display = keymap.display_for(Command::SaveFile);
    assert!(display.is_some());

    let s = display.unwrap();
    // Should contain 'S' in some form
    assert!(s.to_uppercase().contains('S'));
}

#[test]
fn test_command_to_msgs() {
    use crate::messages::{DocumentMsg, Msg};

    let msgs = Command::Undo.to_msgs();
    assert_eq!(msgs.len(), 1);
    assert!(matches!(msgs[0], Msg::Document(DocumentMsg::Undo)));
}

#[test]
fn test_keystroke_equality() {
    let s1 = Keystroke::char_with_mods('s', Modifiers::CTRL);
    let s2 = Keystroke::char_with_mods('S', Modifiers::CTRL);

    // Should be equal (normalized to lowercase)
    assert_eq!(s1, s2);
}

#[test]
fn test_modifiers_platform_cmd() {
    let cmd = Modifiers::cmd();

    // Should work on any platform
    assert!(!cmd.is_empty());
}

#[test]
fn test_numpad_bindings() {
    let keymap = Keymap::with_bindings(default_bindings());

    // Numpad 1-4 should focus groups
    assert_eq!(
        keymap.lookup(&Keystroke::key(KeyCode::Numpad1)),
        Some(Command::FocusGroup1)
    );
    assert_eq!(
        keymap.lookup(&Keystroke::key(KeyCode::Numpad2)),
        Some(Command::FocusGroup2)
    );
}

#[test]
fn test_context_aware_tab_with_selection() {
    let keymap = Keymap::with_bindings(load_default_keymap());
    let tab = Keystroke::key(KeyCode::Tab);

    // With selection → IndentLines
    let ctx_selection = KeyContext {
        has_selection: true,
        has_multiple_cursors: false,
        modal_active: false,
        editor_focused: true,
    };

    let result = keymap.lookup_with_context(&tab, Some(&ctx_selection));
    assert_eq!(result, Some(Command::IndentLines));
}

#[test]
fn test_context_aware_tab_without_selection() {
    let keymap = Keymap::with_bindings(load_default_keymap());
    let tab = Keystroke::key(KeyCode::Tab);

    // Without selection → InsertTab
    let ctx_no_selection = KeyContext {
        has_selection: false,
        has_multiple_cursors: false,
        modal_active: false,
        editor_focused: true,
    };

    let result = keymap.lookup_with_context(&tab, Some(&ctx_no_selection));
    assert_eq!(result, Some(Command::InsertTab));
}

#[test]
fn test_context_aware_escape_multi_cursor() {
    let keymap = Keymap::with_bindings(load_default_keymap());
    let escape = Keystroke::key(KeyCode::Escape);

    // With multiple cursors → CollapseToSingleCursor
    let ctx = KeyContext {
        has_selection: true, // Even if there's selection, multi-cursor takes priority
        has_multiple_cursors: true,
        modal_active: false,
        editor_focused: true,
    };

    let result = keymap.lookup_with_context(&escape, Some(&ctx));
    assert_eq!(result, Some(Command::CollapseToSingleCursor));
}

#[test]
fn test_context_aware_escape_selection() {
    let keymap = Keymap::with_bindings(load_default_keymap());
    let escape = Keystroke::key(KeyCode::Escape);

    // Single cursor with selection → ClearSelection
    let ctx = KeyContext {
        has_selection: true,
        has_multiple_cursors: false,
        modal_active: false,
        editor_focused: true,
    };

    let result = keymap.lookup_with_context(&escape, Some(&ctx));
    assert_eq!(result, Some(Command::ClearSelection));
}

#[test]
fn test_context_aware_escape_fallback() {
    let keymap = Keymap::with_bindings(load_default_keymap());
    let escape = Keystroke::key(KeyCode::Escape);

    // No selection, single cursor → EscapeSmartClear (fallback)
    let ctx = KeyContext {
        has_selection: false,
        has_multiple_cursors: false,
        modal_active: false,
        editor_focused: true,
    };

    let result = keymap.lookup_with_context(&escape, Some(&ctx));
    assert_eq!(result, Some(Command::EscapeSmartClear));
}

#[test]
fn test_conditional_binding_yaml_parsing() {
    let yaml = r#"
bindings:
  - key: "tab"
    command: IndentLines
    when: ["has_selection"]
  - key: "tab"
    command: InsertTab
    when: ["no_selection"]
"#;

    let bindings = parse_keymap_yaml(yaml).expect("Should parse");
    assert_eq!(bindings.len(), 2);

    // First binding has condition
    assert!(bindings[0].when.is_some());
    assert_eq!(bindings[0].when.as_ref().unwrap().len(), 1);

    // Second binding has different condition
    assert!(bindings[1].when.is_some());
}
