//! Pure keymap preference merge, conflict and serialization regressions.
use token::keymap::preferences::{
    config_stroke, conflicts, parse_sequence, platform, BaseKeymap, KeymapSnapshot,
    MAX_KEYMAP_BYTES,
};
use token::keymap::{Command, Condition, KeyCode, Keybinding, Keystroke, Modifiers};

fn binding(key: &str, command: Command) -> Keybinding {
    Keybinding::chord(parse_sequence(key).unwrap(), command)
}

#[test]
fn settings_keymap_command_names_share_the_bindable_registry() {
    for &command in Command::all() {
        assert_eq!(command.name().parse::<Command>().unwrap(), command);
    }
}

#[test]
fn settings_keymap_presets_preserve_user_precedence_and_metadata() {
    let source = "custom: {owner: test}\nbindings:\n  - {key: cmd+p, command: SaveFile}\n";
    let snapshot = KeymapSnapshot::parse(Some(source.into())).unwrap();
    let encoded = snapshot.with_base(BaseKeymap::Conventional).unwrap();
    let value: serde_yaml::Value = serde_yaml::from_str(&encoded).unwrap();
    assert_eq!(value["custom"]["owner"].as_str(), Some("test"));
    assert_eq!(value["bindings"].as_sequence().unwrap().len(), 1);
    let reloaded = KeymapSnapshot::parse(Some(encoded)).unwrap();
    assert_eq!(reloaded.base, BaseKeymap::Conventional);
    assert!(reloaded
        .bindings
        .contains(&binding("cmd+p", Command::SaveFile)));
    assert!(!reloaded
        .bindings
        .contains(&binding("cmd+p", Command::FuzzyFileFinder)));
    assert!(reloaded
        .bindings
        .contains(&binding("cmd+shift+p", Command::ToggleCommandPalette)));
}

#[test]
fn settings_keymap_rebind_preserves_context_siblings_and_foreign_entries() {
    let foreign = if platform() == "macos" {
        "linux"
    } else {
        "macos"
    };
    let source = format!("custom: retained\nbindings:\n  - {{key: ctrl+f24, command: SaveFile, when: [has_selection]}}\n  - {{key: ctrl+f24, command: OpenFile, when: [no_selection]}}\n  - {{key: ctrl+f24, command: Quit, platform: {foreign}, custom: retained}}\n");
    let snapshot = KeymapSnapshot::parse(Some(source)).unwrap();
    let original = binding("ctrl+f24", Command::SaveFile).when(vec![Condition::HasSelection]);
    let encoded = snapshot
        .rebind(
            Some(&original),
            Command::SaveFile,
            &parse_sequence("alt+k alt+s").unwrap(),
        )
        .unwrap();
    let value: serde_yaml::Value = serde_yaml::from_str(&encoded).unwrap();
    assert_eq!(value["custom"].as_str(), Some("retained"));
    assert!(value["bindings"]
        .as_sequence()
        .unwrap()
        .iter()
        .any(|entry| entry["platform"].as_str() == Some(foreign)
            && entry["custom"].as_str() == Some("retained")));
    let reloaded = KeymapSnapshot::parse(Some(encoded)).unwrap();
    assert!(!reloaded.bindings.contains(&original));
    assert!(reloaded
        .bindings
        .contains(&binding("ctrl+f24", Command::OpenFile).when(vec![Condition::NoSelection])));
    assert!(reloaded
        .bindings
        .contains(&binding("alt+k alt+s", Command::SaveFile).when(vec![Condition::HasSelection])));
    assert!(!reloaded
        .bindings
        .contains(&binding("ctrl+f24", Command::Quit)));
    // Repeated rebinds retire prior local overrides without accumulating duplicates.
    let original = binding("alt+k alt+s", Command::SaveFile).when(vec![Condition::HasSelection]);
    let encoded = reloaded
        .rebind(
            Some(&original),
            Command::SaveFile,
            &parse_sequence("ctrl+f24").unwrap(),
        )
        .unwrap();
    let final_map = KeymapSnapshot::parse(Some(encoded)).unwrap();
    assert_eq!(
        final_map
            .bindings
            .iter()
            .filter(|entry| entry.keystrokes == parse_sequence("ctrl+f24").unwrap())
            .count(),
        2
    );
}

#[test]
fn settings_keymap_conflicts_use_prefixes_and_feasible_contexts() {
    let plain = binding("ctrl+k", Command::SaveFile);
    let chord = binding("ctrl+k ctrl+c", Command::OpenFile);
    assert_eq!(
        conflicts(std::slice::from_ref(&plain), &chord, None),
        vec![0]
    );
    assert_eq!(conflicts(&[chord], &plain, None), vec![0]);
    assert!(conflicts(std::slice::from_ref(&plain), &plain, Some(0)).is_empty());
    for (left, right) in [
        (Condition::HasSelection, Condition::NoSelection),
        (Condition::HasMultipleCursors, Condition::SingleCursor),
        (Condition::ModalActive, Condition::ModalInactive),
        (Condition::EditorFocused, Condition::SidebarFocused),
        (Condition::ModalActive, Condition::EditorFocused),
    ] {
        assert!(conflicts(
            &[plain.clone().when(vec![left])],
            &plain.clone().when(vec![right]),
            None
        )
        .is_empty());
    }
    assert_eq!(
        conflicts(
            &[plain.clone().when(vec![Condition::EditorFocused])],
            &plain.clone().when(vec![Condition::HasSelection]),
            None
        ),
        vec![0]
    );
    assert!(conflicts(&[plain], &Keybinding::chord(vec![], Command::Quit), None).is_empty());
}

#[test]
fn settings_keymap_capture_keys_round_trip_without_display_string_parsing() {
    for key in [
        KeyCode::Char('+'),
        KeyCode::Char(' '),
        KeyCode::Char('Æ'),
        KeyCode::Char('中'),
        KeyCode::F(24),
        KeyCode::Space,
        KeyCode::PageDown,
        KeyCode::Numpad0,
        KeyCode::NumpadAdd,
        KeyCode::NumpadDecimal,
    ] {
        let stroke = Keystroke::new(
            key,
            Modifiers::CTRL | Modifiers::ALT | Modifiers::SHIFT | Modifiers::META,
        );
        assert_eq!(
            parse_sequence(&config_stroke(&stroke)).unwrap(),
            vec![stroke]
        );
        let snapshot = KeymapSnapshot::parse(None).unwrap();
        let encoded = snapshot.rebind(None, Command::SaveFile, &[stroke]).unwrap();
        assert!(KeymapSnapshot::parse(Some(encoded))
            .unwrap()
            .bindings
            .contains(&Keybinding::new(stroke, Command::SaveFile)));
    }
}

#[test]
fn settings_keymap_rejects_invalid_documents_and_capture_without_mutation() {
    for source in [
        "",
        "[]",
        "bindings: [",
        "base: unknown\nbindings: []",
        "bindings: [{key: ctrl+k, command: Unknown}]",
        "bindings: [{key: '', command: SaveFile}]",
    ] {
        assert!(
            KeymapSnapshot::parse(Some(source.into())).is_err(),
            "{source}"
        );
    }
    assert!(
        KeymapSnapshot::parse(Some(" ".repeat(MAX_KEYMAP_BYTES.as_u64() as usize + 1))).is_err()
    );
    let snapshot = KeymapSnapshot::parse(None).unwrap();
    assert!(snapshot.rebind(None, Command::SaveFile, &[]).is_err());
    assert!(snapshot
        .rebind(
            None,
            Command::SaveFile,
            &[Keystroke::char_with_mods('a', Modifiers::CTRL); 5]
        )
        .is_err());
    assert!(snapshot
        .rebind(
            Some(&binding("ctrl+f24", Command::SaveFile)),
            Command::SaveFile,
            &parse_sequence("ctrl+k").unwrap()
        )
        .is_err());
    assert!(snapshot.source.is_none());
}
