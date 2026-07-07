//! Translate focused terminal keyboard input into PTY byte sequences.

use winit::keyboard::{Key, NamedKey};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TerminalKeyModifiers {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub logo: bool,
}

pub fn translate_key(key: &Key, modifiers: TerminalKeyModifiers) -> Option<Vec<u8>> {
    if modifiers.logo {
        return None;
    }

    if modifiers.ctrl {
        return translate_control_key(key);
    }

    match key {
        Key::Character(text) => Some(text.as_bytes().to_vec()),
        Key::Named(named) => translate_named_key(*named).map(bytes),
        _ => None,
    }
}

fn translate_control_key(key: &Key) -> Option<Vec<u8>> {
    let Key::Character(text) = key else {
        return None;
    };

    let mut chars = text.chars();
    let c = chars.next()?;
    if chars.next().is_some() {
        // Multi-character strings (e.g. composed glyphs) have no single
        // control-byte representation.
        return None;
    }

    if c.is_ascii_alphabetic() {
        // Ctrl+A -> 0x01, Ctrl+B -> 0x02, ..., Ctrl+Z -> 0x1A.
        let byte = c.to_ascii_uppercase() as u8;
        return Some(vec![byte - b'@']);
    }

    match c {
        ' ' => Some(vec![0x00]), // Ctrl+Space == NUL
        '[' => Some(vec![0x1B]), // Ctrl+[ == ESC
        '\\' => Some(vec![0x1C]),
        ']' => Some(vec![0x1D]),
        '^' => Some(vec![0x1E]),
        '_' => Some(vec![0x1F]),
        '?' => Some(vec![0x7F]), // Ctrl+? == DEL
        _ => None,
    }
}

fn translate_named_key(key: NamedKey) -> Option<&'static [u8]> {
    match key {
        NamedKey::Enter => Some(b"\r"),
        NamedKey::Backspace => Some(b"\x7f"),
        NamedKey::Tab => Some(b"\t"),
        NamedKey::Space => Some(b" "),
        NamedKey::Delete => Some(b"\x1b[3~"),
        NamedKey::ArrowUp => Some(b"\x1b[A"),
        NamedKey::ArrowDown => Some(b"\x1b[B"),
        NamedKey::ArrowRight => Some(b"\x1b[C"),
        NamedKey::ArrowLeft => Some(b"\x1b[D"),
        NamedKey::Home => Some(b"\x1b[H"),
        NamedKey::End => Some(b"\x1b[F"),
        NamedKey::PageUp => Some(b"\x1b[5~"),
        NamedKey::PageDown => Some(b"\x1b[6~"),
        NamedKey::F1 => Some(b"\x1bOP"),
        NamedKey::F2 => Some(b"\x1bOQ"),
        NamedKey::F3 => Some(b"\x1bOR"),
        NamedKey::F4 => Some(b"\x1bOS"),
        NamedKey::F5 => Some(b"\x1b[15~"),
        NamedKey::F6 => Some(b"\x1b[17~"),
        NamedKey::F7 => Some(b"\x1b[18~"),
        NamedKey::F8 => Some(b"\x1b[19~"),
        NamedKey::F9 => Some(b"\x1b[20~"),
        NamedKey::F10 => Some(b"\x1b[21~"),
        NamedKey::F11 => Some(b"\x1b[23~"),
        NamedKey::F12 => Some(b"\x1b[24~"),
        _ => None,
    }
}

fn bytes(slice: &'static [u8]) -> Vec<u8> {
    slice.to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;
    use winit::keyboard::NamedKey;

    fn key_bytes(key: Key) -> Option<Vec<u8>> {
        translate_key(&key, TerminalKeyModifiers::default())
    }

    #[test]
    fn regular_characters_translate_to_utf8_bytes() {
        assert_eq!(key_bytes(Key::Character("a".into())), Some(b"a".to_vec()));
        assert_eq!(
            key_bytes(Key::Character("å".into())),
            Some("å".as_bytes().to_vec())
        );
        assert_eq!(
            key_bytes(Key::Character("λ".into())),
            Some("λ".as_bytes().to_vec())
        );
    }

    #[test]
    fn basic_named_keys_translate_to_terminal_bytes() {
        assert_eq!(key_bytes(Key::Named(NamedKey::Enter)), Some(b"\r".to_vec()));
        assert_eq!(
            key_bytes(Key::Named(NamedKey::Backspace)),
            Some(b"\x7f".to_vec())
        );
        assert_eq!(key_bytes(Key::Named(NamedKey::Tab)), Some(b"\t".to_vec()));
        assert_eq!(key_bytes(Key::Named(NamedKey::Space)), Some(b" ".to_vec()));
        assert_eq!(
            key_bytes(Key::Named(NamedKey::Delete)),
            Some(b"\x1b[3~".to_vec())
        );
    }

    #[test]
    fn arrow_and_navigation_keys_translate_to_escape_sequences() {
        assert_eq!(
            key_bytes(Key::Named(NamedKey::ArrowUp)),
            Some(b"\x1b[A".to_vec())
        );
        assert_eq!(
            key_bytes(Key::Named(NamedKey::ArrowDown)),
            Some(b"\x1b[B".to_vec())
        );
        assert_eq!(
            key_bytes(Key::Named(NamedKey::ArrowRight)),
            Some(b"\x1b[C".to_vec())
        );
        assert_eq!(
            key_bytes(Key::Named(NamedKey::ArrowLeft)),
            Some(b"\x1b[D".to_vec())
        );
        assert_eq!(
            key_bytes(Key::Named(NamedKey::Home)),
            Some(b"\x1b[H".to_vec())
        );
        assert_eq!(
            key_bytes(Key::Named(NamedKey::End)),
            Some(b"\x1b[F".to_vec())
        );
        assert_eq!(
            key_bytes(Key::Named(NamedKey::PageUp)),
            Some(b"\x1b[5~".to_vec())
        );
        assert_eq!(
            key_bytes(Key::Named(NamedKey::PageDown)),
            Some(b"\x1b[6~".to_vec())
        );
    }

    #[test]
    fn control_shortcuts_translate_to_control_bytes() {
        assert_eq!(
            translate_key(
                &Key::Character("c".into()),
                TerminalKeyModifiers {
                    ctrl: true,
                    ..TerminalKeyModifiers::default()
                },
            ),
            Some(vec![0x03])
        );
        assert_eq!(
            translate_key(
                &Key::Character("D".into()),
                TerminalKeyModifiers {
                    ctrl: true,
                    shift: true,
                    ..TerminalKeyModifiers::default()
                },
            ),
            Some(vec![0x04])
        );
        assert_eq!(
            translate_key(
                &Key::Character("z".into()),
                TerminalKeyModifiers {
                    ctrl: true,
                    ..TerminalKeyModifiers::default()
                },
            ),
            Some(vec![0x1A])
        );
        assert_eq!(
            translate_key(
                &Key::Character("l".into()),
                TerminalKeyModifiers {
                    ctrl: true,
                    ..TerminalKeyModifiers::default()
                },
            ),
            Some(vec![0x0C])
        );
    }

    #[test]
    fn all_ascii_letters_translate_to_control_bytes() {
        for (letter, expected) in [
            ('a', 0x01),
            ('b', 0x02),
            ('c', 0x03),
            ('d', 0x04),
            ('e', 0x05),
            ('f', 0x06),
            ('g', 0x07),
            ('h', 0x08),
            ('i', 0x09),
            ('j', 0x0A),
            ('k', 0x0B),
            ('l', 0x0C),
            ('m', 0x0D),
            ('n', 0x0E),
            ('o', 0x0F),
            ('p', 0x10),
            ('q', 0x11),
            ('r', 0x12),
            ('s', 0x13),
            ('t', 0x14),
            ('u', 0x15),
            ('v', 0x16),
            ('w', 0x17),
            ('x', 0x18),
            ('y', 0x19),
            ('z', 0x1A),
        ] {
            assert_eq!(
                translate_key(
                    &Key::Character(letter.to_string().into()),
                    TerminalKeyModifiers {
                        ctrl: true,
                        ..TerminalKeyModifiers::default()
                    },
                ),
                Some(vec![expected]),
                "Ctrl+{letter} should map to 0x{expected:02X}"
            );
            assert_eq!(
                translate_key(
                    &Key::Character(letter.to_ascii_uppercase().to_string().into()),
                    TerminalKeyModifiers {
                        ctrl: true,
                        ..TerminalKeyModifiers::default()
                    },
                ),
                Some(vec![expected]),
                "Ctrl+{} should map to 0x{expected:02X}",
                letter.to_ascii_uppercase()
            );
        }
    }

    #[test]
    fn common_non_letter_control_keys_translate_to_control_bytes() {
        assert_eq!(
            translate_key(
                &Key::Character(" ".into()),
                TerminalKeyModifiers {
                    ctrl: true,
                    ..TerminalKeyModifiers::default()
                },
            ),
            Some(vec![0x00])
        );
        assert_eq!(
            translate_key(
                &Key::Character("[".into()),
                TerminalKeyModifiers {
                    ctrl: true,
                    ..TerminalKeyModifiers::default()
                },
            ),
            Some(vec![0x1B])
        );
        assert_eq!(
            translate_key(
                &Key::Character("?".into()),
                TerminalKeyModifiers {
                    ctrl: true,
                    ..TerminalKeyModifiers::default()
                },
            ),
            Some(vec![0x7F])
        );
    }

    #[test]
    fn control_modified_non_ascii_characters_are_ignored() {
        assert_eq!(
            translate_key(
                &Key::Character("å".into()),
                TerminalKeyModifiers {
                    ctrl: true,
                    ..TerminalKeyModifiers::default()
                },
            ),
            None
        );
    }

    #[test]
    fn function_keys_translate_to_common_xterm_sequences() {
        assert_eq!(
            key_bytes(Key::Named(NamedKey::F1)),
            Some(b"\x1bOP".to_vec())
        );
        assert_eq!(
            key_bytes(Key::Named(NamedKey::F2)),
            Some(b"\x1bOQ".to_vec())
        );
        assert_eq!(
            key_bytes(Key::Named(NamedKey::F3)),
            Some(b"\x1bOR".to_vec())
        );
        assert_eq!(
            key_bytes(Key::Named(NamedKey::F4)),
            Some(b"\x1bOS".to_vec())
        );
        assert_eq!(
            key_bytes(Key::Named(NamedKey::F5)),
            Some(b"\x1b[15~".to_vec())
        );
        assert_eq!(
            key_bytes(Key::Named(NamedKey::F12)),
            Some(b"\x1b[24~".to_vec())
        );
    }

    #[test]
    fn command_modified_keys_are_left_for_app_shortcuts() {
        assert_eq!(
            translate_key(
                &Key::Character("v".into()),
                TerminalKeyModifiers {
                    logo: true,
                    ..TerminalKeyModifiers::default()
                },
            ),
            None
        );
    }
}
