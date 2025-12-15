//! Core types for the keymap system: Keystroke, Modifiers, KeyCode

use std::fmt;

/// Modifier keys as a bitfield for efficient storage and comparison
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Modifiers(u8);

impl Modifiers {
    pub const NONE: Modifiers = Modifiers(0);
    pub const CTRL: Modifiers = Modifiers(0b0001);
    pub const SHIFT: Modifiers = Modifiers(0b0010);
    pub const ALT: Modifiers = Modifiers(0b0100);
    pub const META: Modifiers = Modifiers(0b1000); // Cmd on macOS, Win on Windows

    /// Create modifiers from individual flags
    pub const fn new(ctrl: bool, shift: bool, alt: bool, meta: bool) -> Self {
        let mut bits = 0u8;
        if ctrl {
            bits |= 0b0001;
        }
        if shift {
            bits |= 0b0010;
        }
        if alt {
            bits |= 0b0100;
        }
        if meta {
            bits |= 0b1000;
        }
        Modifiers(bits)
    }

    /// Check if ctrl is held
    #[inline]
    pub const fn ctrl(self) -> bool {
        self.0 & 0b0001 != 0
    }

    /// Check if shift is held
    #[inline]
    pub const fn shift(self) -> bool {
        self.0 & 0b0010 != 0
    }

    /// Check if alt/option is held
    #[inline]
    pub const fn alt(self) -> bool {
        self.0 & 0b0100 != 0
    }

    /// Check if meta (cmd/win) is held
    #[inline]
    pub const fn meta(self) -> bool {
        self.0 & 0b1000 != 0
    }

    /// Check if no modifiers are held
    #[inline]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Combine two modifier sets
    #[inline]
    pub const fn union(self, other: Modifiers) -> Modifiers {
        Modifiers(self.0 | other.0)
    }

    /// Check if this contains all modifiers in other
    #[inline]
    pub const fn contains(self, other: Modifiers) -> bool {
        (self.0 & other.0) == other.0
    }

    /// Get the platform-specific "command" modifier (Cmd on macOS, Ctrl elsewhere)
    pub fn cmd() -> Modifiers {
        if cfg!(target_os = "macos") {
            Modifiers::META
        } else {
            Modifiers::CTRL
        }
    }

    /// Check if the platform command key is held (Cmd on macOS, Ctrl elsewhere)
    pub fn has_cmd(self) -> bool {
        if cfg!(target_os = "macos") {
            self.meta()
        } else {
            self.ctrl()
        }
    }
}

impl std::ops::BitOr for Modifiers {
    type Output = Modifiers;

    fn bitor(self, rhs: Self) -> Self::Output {
        self.union(rhs)
    }
}

impl fmt::Display for Modifiers {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut parts = Vec::new();
        if self.ctrl() {
            parts.push("Ctrl");
        }
        if self.shift() {
            parts.push("Shift");
        }
        if self.alt() {
            parts.push(if cfg!(target_os = "macos") {
                "Option"
            } else {
                "Alt"
            });
        }
        if self.meta() {
            parts.push(if cfg!(target_os = "macos") {
                "Cmd"
            } else {
                "Win"
            });
        }
        write!(f, "{}", parts.join("+"))
    }
}

/// A key code representing a physical or logical key
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum KeyCode {
    /// A character key (normalized to lowercase)
    Char(char),

    // Named keys
    Enter,
    Escape,
    Tab,
    Backspace,
    Delete,
    Space,

    // Arrow keys
    Up,
    Down,
    Left,
    Right,

    // Navigation
    Home,
    End,
    PageUp,
    PageDown,
    Insert,

    // Function keys
    F(u8), // F1-F24

    // Numpad (physical keys)
    Numpad0,
    Numpad1,
    Numpad2,
    Numpad3,
    Numpad4,
    Numpad5,
    Numpad6,
    Numpad7,
    Numpad8,
    Numpad9,
    NumpadAdd,
    NumpadSubtract,
    NumpadMultiply,
    NumpadDivide,
    NumpadEnter,
    NumpadDecimal,
}

impl fmt::Display for KeyCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KeyCode::Char(c) => write!(f, "{}", c.to_uppercase()),
            KeyCode::Enter => write!(f, "Enter"),
            KeyCode::Escape => write!(f, "Escape"),
            KeyCode::Tab => write!(f, "Tab"),
            KeyCode::Backspace => write!(f, "Backspace"),
            KeyCode::Delete => write!(f, "Delete"),
            KeyCode::Space => write!(f, "Space"),
            KeyCode::Up => write!(f, "↑"),
            KeyCode::Down => write!(f, "↓"),
            KeyCode::Left => write!(f, "←"),
            KeyCode::Right => write!(f, "→"),
            KeyCode::Home => write!(f, "Home"),
            KeyCode::End => write!(f, "End"),
            KeyCode::PageUp => write!(f, "PageUp"),
            KeyCode::PageDown => write!(f, "PageDown"),
            KeyCode::Insert => write!(f, "Insert"),
            KeyCode::F(n) => write!(f, "F{}", n),
            KeyCode::Numpad0 => write!(f, "Num0"),
            KeyCode::Numpad1 => write!(f, "Num1"),
            KeyCode::Numpad2 => write!(f, "Num2"),
            KeyCode::Numpad3 => write!(f, "Num3"),
            KeyCode::Numpad4 => write!(f, "Num4"),
            KeyCode::Numpad5 => write!(f, "Num5"),
            KeyCode::Numpad6 => write!(f, "Num6"),
            KeyCode::Numpad7 => write!(f, "Num7"),
            KeyCode::Numpad8 => write!(f, "Num8"),
            KeyCode::Numpad9 => write!(f, "Num9"),
            KeyCode::NumpadAdd => write!(f, "Num+"),
            KeyCode::NumpadSubtract => write!(f, "Num-"),
            KeyCode::NumpadMultiply => write!(f, "Num*"),
            KeyCode::NumpadDivide => write!(f, "Num/"),
            KeyCode::NumpadEnter => write!(f, "NumEnter"),
            KeyCode::NumpadDecimal => write!(f, "Num."),
        }
    }
}

/// A single keystroke: a key with modifiers
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Keystroke {
    pub key: KeyCode,
    pub mods: Modifiers,
}

impl Keystroke {
    /// Create a new keystroke
    pub const fn new(key: KeyCode, mods: Modifiers) -> Self {
        Self { key, mods }
    }

    /// Create a keystroke with no modifiers
    pub const fn key(key: KeyCode) -> Self {
        Self {
            key,
            mods: Modifiers::NONE,
        }
    }

    /// Create a keystroke with a character key
    pub fn char(c: char) -> Self {
        Self {
            key: KeyCode::Char(c.to_ascii_lowercase()),
            mods: Modifiers::NONE,
        }
    }

    /// Create a keystroke with a character and modifiers
    pub fn char_with_mods(c: char, mods: Modifiers) -> Self {
        Self {
            key: KeyCode::Char(c.to_ascii_lowercase()),
            mods,
        }
    }

    /// Display the keystroke using platform-specific symbols
    pub fn display_string(&self) -> String {
        let mut parts = Vec::new();

        if cfg!(target_os = "macos") {
            // macOS uses symbols: ⌃ ⇧ ⌥ ⌘
            if self.mods.ctrl() {
                parts.push("⌃");
            }
            if self.mods.alt() {
                parts.push("⌥");
            }
            if self.mods.shift() {
                parts.push("⇧");
            }
            if self.mods.meta() {
                parts.push("⌘");
            }
        } else {
            // Windows/Linux uses text
            if self.mods.ctrl() {
                parts.push("Ctrl+");
            }
            if self.mods.alt() {
                parts.push("Alt+");
            }
            if self.mods.shift() {
                parts.push("Shift+");
            }
            if self.mods.meta() {
                parts.push("Win+");
            }
        }

        let key_str = match self.key {
            KeyCode::Char(c) => c.to_uppercase().to_string(),
            KeyCode::Up => "↑".to_string(),
            KeyCode::Down => "↓".to_string(),
            KeyCode::Left => "←".to_string(),
            KeyCode::Right => "→".to_string(),
            _ => format!("{}", self.key),
        };

        if cfg!(target_os = "macos") {
            format!("{}{}", parts.join(""), key_str)
        } else {
            format!("{}{}", parts.join(""), key_str)
        }
    }
}

impl fmt::Display for Keystroke {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if !self.mods.is_empty() {
            write!(f, "{}+{}", self.mods, self.key)
        } else {
            write!(f, "{}", self.key)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_modifiers_empty() {
        let mods = Modifiers::NONE;
        assert!(mods.is_empty());
        assert!(!mods.ctrl());
        assert!(!mods.shift());
        assert!(!mods.alt());
        assert!(!mods.meta());
    }

    #[test]
    fn test_modifiers_individual() {
        assert!(Modifiers::CTRL.ctrl());
        assert!(!Modifiers::CTRL.shift());

        assert!(Modifiers::SHIFT.shift());
        assert!(!Modifiers::SHIFT.ctrl());

        assert!(Modifiers::ALT.alt());
        assert!(Modifiers::META.meta());
    }

    #[test]
    fn test_modifiers_combined() {
        let mods = Modifiers::CTRL | Modifiers::SHIFT;
        assert!(mods.ctrl());
        assert!(mods.shift());
        assert!(!mods.alt());
        assert!(!mods.meta());
    }

    #[test]
    fn test_modifiers_new() {
        let mods = Modifiers::new(true, false, true, false);
        assert!(mods.ctrl());
        assert!(!mods.shift());
        assert!(mods.alt());
        assert!(!mods.meta());
    }

    #[test]
    fn test_keystroke_display() {
        let stroke = Keystroke::new(KeyCode::Char('s'), Modifiers::CTRL);
        let display = format!("{}", stroke);
        assert!(display.contains('S') || display.contains('s'));
        assert!(display.contains("Ctrl"));
    }

    #[test]
    fn test_keystroke_char_lowercase() {
        let stroke1 = Keystroke::char('A');
        let stroke2 = Keystroke::char('a');
        assert_eq!(stroke1, stroke2);
    }
}
