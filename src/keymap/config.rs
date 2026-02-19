//! YAML configuration parsing for keymaps
//!
//! Parses keymap.yaml files into Keybinding structs.

use std::path::Path;
use std::str::FromStr;

use serde::Deserialize;

use super::binding::Keybinding;
use super::command::Command;
use super::context::Condition;
use super::types::{KeyCode, Keystroke, Modifiers};

/// Root structure of a keymap YAML file
#[derive(Debug, Deserialize)]
pub struct KeymapConfig {
    pub bindings: Vec<BindingConfig>,
}

/// A single binding entry from YAML
#[derive(Debug, Deserialize)]
pub struct BindingConfig {
    pub key: String,
    pub command: String,
    #[serde(default)]
    pub platform: Option<String>,
    #[serde(default)]
    pub when: Option<Vec<String>>,
}

/// Load keybindings from a YAML file
pub fn load_keymap_file(path: &Path) -> Result<Vec<Keybinding>, KeymapError> {
    let content = std::fs::read_to_string(path).map_err(|e| KeymapError::IoError(e.to_string()))?;

    parse_keymap_yaml(&content)
}

/// Parse keybindings from YAML string
pub fn parse_keymap_yaml(yaml: &str) -> Result<Vec<Keybinding>, KeymapError> {
    let config: KeymapConfig =
        serde_yaml::from_str(yaml).map_err(|e| KeymapError::ParseError(e.to_string()))?;

    let current_platform = get_current_platform();
    let mut bindings = Vec::new();

    for entry in config.bindings {
        // Skip if platform-specific and doesn't match current platform
        if let Some(ref platform) = entry.platform {
            if platform != current_platform {
                continue;
            }
        }

        let keystroke = parse_key_string(&entry.key)?;
        let command = parse_command(&entry.command)?;
        let conditions = parse_conditions(&entry.when)?;

        let mut binding = Keybinding::new(keystroke, command);
        if let Some(conds) = conditions {
            binding = binding.when(conds);
        }
        bindings.push(binding);
    }

    Ok(bindings)
}

/// Parse a key string like "cmd+shift+s" into a Keystroke
pub fn parse_key_string(key_str: &str) -> Result<Keystroke, KeymapError> {
    let parts: Vec<&str> = key_str.split('+').collect();

    if parts.is_empty() {
        return Err(KeymapError::InvalidKey(key_str.to_string()));
    }

    let mut mods = Modifiers::NONE;
    let mut key_part = None;

    for part in parts {
        let part_lower = part.to_lowercase();
        match part_lower.as_str() {
            "cmd" => {
                // Platform command key
                mods = mods | Modifiers::cmd();
            }
            "ctrl" | "control" => {
                mods = mods | Modifiers::CTRL;
            }
            "shift" => {
                mods = mods | Modifiers::SHIFT;
            }
            "alt" | "option" | "opt" => {
                mods = mods | Modifiers::ALT;
            }
            "meta" | "super" | "win" => {
                mods = mods | Modifiers::META;
            }
            _ => {
                // This should be the key itself
                if key_part.is_some() {
                    return Err(KeymapError::InvalidKey(format!(
                        "Multiple keys in binding: {}",
                        key_str
                    )));
                }
                key_part = Some(parse_key_code(&part_lower)?);
            }
        }
    }

    let key = key_part
        .ok_or_else(|| KeymapError::InvalidKey(format!("No key found in binding: {}", key_str)))?;

    Ok(Keystroke::new(key, mods))
}

/// Parse a key code from string
fn parse_key_code(key: &str) -> Result<KeyCode, KeymapError> {
    // Single character
    if key.len() == 1 {
        let c = key.chars().next().unwrap();
        return Ok(KeyCode::Char(c.to_ascii_lowercase()));
    }

    // Named keys
    match key {
        "enter" | "return" => Ok(KeyCode::Enter),
        "escape" | "esc" => Ok(KeyCode::Escape),
        "tab" => Ok(KeyCode::Tab),
        "backspace" | "back" => Ok(KeyCode::Backspace),
        "delete" | "del" => Ok(KeyCode::Delete),
        "space" => Ok(KeyCode::Space),

        "up" | "arrowup" => Ok(KeyCode::Up),
        "down" | "arrowdown" => Ok(KeyCode::Down),
        "left" | "arrowleft" => Ok(KeyCode::Left),
        "right" | "arrowright" => Ok(KeyCode::Right),

        "home" => Ok(KeyCode::Home),
        "end" => Ok(KeyCode::End),
        "pageup" | "pgup" => Ok(KeyCode::PageUp),
        "pagedown" | "pgdown" | "pgdn" => Ok(KeyCode::PageDown),
        "insert" | "ins" => Ok(KeyCode::Insert),

        // Function keys
        "f1" => Ok(KeyCode::F(1)),
        "f2" => Ok(KeyCode::F(2)),
        "f3" => Ok(KeyCode::F(3)),
        "f4" => Ok(KeyCode::F(4)),
        "f5" => Ok(KeyCode::F(5)),
        "f6" => Ok(KeyCode::F(6)),
        "f7" => Ok(KeyCode::F(7)),
        "f8" => Ok(KeyCode::F(8)),
        "f9" => Ok(KeyCode::F(9)),
        "f10" => Ok(KeyCode::F(10)),
        "f11" => Ok(KeyCode::F(11)),
        "f12" => Ok(KeyCode::F(12)),

        // Numpad
        "numpad0" | "num0" => Ok(KeyCode::Numpad0),
        "numpad1" | "num1" => Ok(KeyCode::Numpad1),
        "numpad2" | "num2" => Ok(KeyCode::Numpad2),
        "numpad3" | "num3" => Ok(KeyCode::Numpad3),
        "numpad4" | "num4" => Ok(KeyCode::Numpad4),
        "numpad5" | "num5" => Ok(KeyCode::Numpad5),
        "numpad6" | "num6" => Ok(KeyCode::Numpad6),
        "numpad7" | "num7" => Ok(KeyCode::Numpad7),
        "numpad8" | "num8" => Ok(KeyCode::Numpad8),
        "numpad9" | "num9" => Ok(KeyCode::Numpad9),
        "numpad_add" | "numadd" | "numplus" => Ok(KeyCode::NumpadAdd),
        "numpad_subtract" | "numsub" | "numminus" => Ok(KeyCode::NumpadSubtract),
        "numpad_multiply" | "nummul" => Ok(KeyCode::NumpadMultiply),
        "numpad_divide" | "numdiv" => Ok(KeyCode::NumpadDivide),
        "numpad_enter" | "numenter" => Ok(KeyCode::NumpadEnter),
        "numpad_decimal" | "numdot" => Ok(KeyCode::NumpadDecimal),

        _ => Err(KeymapError::InvalidKey(format!("Unknown key: {}", key))),
    }
}

/// Parse a command name string into a Command enum
fn parse_command(cmd: &str) -> Result<Command, KeymapError> {
    Command::from_str(cmd).map_err(|_| KeymapError::InvalidCommand(cmd.to_string()))
}

/// Parse condition strings into Condition enums
fn parse_conditions(when: &Option<Vec<String>>) -> Result<Option<Vec<Condition>>, KeymapError> {
    let Some(conditions) = when else {
        return Ok(None);
    };

    let mut result = Vec::with_capacity(conditions.len());
    for cond_str in conditions {
        let condition = parse_condition(cond_str)?;
        result.push(condition);
    }
    Ok(Some(result))
}

/// Parse a single condition string
fn parse_condition(cond: &str) -> Result<Condition, KeymapError> {
    match cond.to_lowercase().as_str() {
        "has_selection" | "hasselection" | "selection" => Ok(Condition::HasSelection),
        "no_selection" | "noselection" => Ok(Condition::NoSelection),
        "has_multiple_cursors" | "hasmultiplecursors" | "multi_cursor" | "multicursor" => {
            Ok(Condition::HasMultipleCursors)
        }
        "single_cursor" | "singlecursor" => Ok(Condition::SingleCursor),
        "modal_active" | "modalactive" | "modal" => Ok(Condition::ModalActive),
        "modal_inactive" | "modalinactive" | "no_modal" | "nomodal" => Ok(Condition::ModalInactive),
        "editor_focused" | "editorfocused" | "editor" => Ok(Condition::EditorFocused),
        _ => Err(KeymapError::InvalidCondition(cond.to_string())),
    }
}

/// Get the current platform identifier
fn get_current_platform() -> &'static str {
    if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else {
        "linux"
    }
}

/// Errors that can occur when parsing keymaps
#[derive(Debug, Clone)]
pub enum KeymapError {
    IoError(String),
    ParseError(String),
    InvalidKey(String),
    InvalidCommand(String),
    InvalidCondition(String),
}

impl std::fmt::Display for KeymapError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            KeymapError::IoError(e) => write!(f, "IO error: {}", e),
            KeymapError::ParseError(e) => write!(f, "Parse error: {}", e),
            KeymapError::InvalidKey(k) => write!(f, "Invalid key: {}", k),
            KeymapError::InvalidCommand(c) => write!(f, "Invalid command: {}", c),
            KeymapError::InvalidCondition(c) => write!(f, "Invalid condition: {}", c),
        }
    }
}

impl std::error::Error for KeymapError {}

// Implement FromStr for Command to parse from YAML
impl FromStr for Command {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            // Cursor movement
            "MoveCursorUp" => Ok(Command::MoveCursorUp),
            "MoveCursorDown" => Ok(Command::MoveCursorDown),
            "MoveCursorLeft" => Ok(Command::MoveCursorLeft),
            "MoveCursorRight" => Ok(Command::MoveCursorRight),
            "MoveCursorLineStart" => Ok(Command::MoveCursorLineStart),
            "MoveCursorLineEnd" => Ok(Command::MoveCursorLineEnd),
            "MoveCursorDocumentStart" => Ok(Command::MoveCursorDocumentStart),
            "MoveCursorDocumentEnd" => Ok(Command::MoveCursorDocumentEnd),
            "MoveCursorWordLeft" => Ok(Command::MoveCursorWordLeft),
            "MoveCursorWordRight" => Ok(Command::MoveCursorWordRight),
            "PageUp" => Ok(Command::PageUp),
            "PageDown" => Ok(Command::PageDown),

            // Selection movement
            "MoveCursorUpWithSelection" => Ok(Command::MoveCursorUpWithSelection),
            "MoveCursorDownWithSelection" => Ok(Command::MoveCursorDownWithSelection),
            "MoveCursorLeftWithSelection" => Ok(Command::MoveCursorLeftWithSelection),
            "MoveCursorRightWithSelection" => Ok(Command::MoveCursorRightWithSelection),
            "MoveCursorLineStartWithSelection" => Ok(Command::MoveCursorLineStartWithSelection),
            "MoveCursorLineEndWithSelection" => Ok(Command::MoveCursorLineEndWithSelection),
            "MoveCursorDocumentStartWithSelection" => {
                Ok(Command::MoveCursorDocumentStartWithSelection)
            }
            "MoveCursorDocumentEndWithSelection" => Ok(Command::MoveCursorDocumentEndWithSelection),
            "MoveCursorWordLeftWithSelection" => Ok(Command::MoveCursorWordLeftWithSelection),
            "MoveCursorWordRightWithSelection" => Ok(Command::MoveCursorWordRightWithSelection),
            "PageUpWithSelection" => Ok(Command::PageUpWithSelection),
            "PageDownWithSelection" => Ok(Command::PageDownWithSelection),

            // Selection commands
            "SelectAll" => Ok(Command::SelectAll),
            "SelectWord" => Ok(Command::SelectWord),
            "SelectLine" => Ok(Command::SelectLine),
            "ClearSelection" => Ok(Command::ClearSelection),
            "ExpandSelection" => Ok(Command::ExpandSelection),
            "ShrinkSelection" => Ok(Command::ShrinkSelection),

            // Multi-cursor
            "AddCursorAbove" => Ok(Command::AddCursorAbove),
            "AddCursorBelow" => Ok(Command::AddCursorBelow),
            "CollapseToSingleCursor" => Ok(Command::CollapseToSingleCursor),
            "SelectNextOccurrence" => Ok(Command::SelectNextOccurrence),
            "UnselectOccurrence" => Ok(Command::UnselectOccurrence),

            // Text editing
            "InsertNewline" => Ok(Command::InsertNewline),
            "DeleteBackward" => Ok(Command::DeleteBackward),
            "DeleteForward" => Ok(Command::DeleteForward),
            "DeleteWordBackward" => Ok(Command::DeleteWordBackward),
            "DeleteWordForward" => Ok(Command::DeleteWordForward),
            "DeleteLine" => Ok(Command::DeleteLine),
            "Duplicate" => Ok(Command::Duplicate),
            "IndentLines" => Ok(Command::IndentLines),
            "UnindentLines" => Ok(Command::UnindentLines),
            "InsertTab" => Ok(Command::InsertTab),

            // Clipboard
            "Copy" => Ok(Command::Copy),
            "Cut" => Ok(Command::Cut),
            "Paste" => Ok(Command::Paste),

            // Undo/Redo
            "Undo" => Ok(Command::Undo),
            "Redo" => Ok(Command::Redo),

            // File operations
            "SaveFile" => Ok(Command::SaveFile),
            "SaveFileAs" => Ok(Command::SaveFileAs),
            "OpenFile" => Ok(Command::OpenFile),
            "FuzzyFileFinder" => Ok(Command::FuzzyFileFinder),
            "NewFile" => Ok(Command::NewFile),
            "Quit" => Ok(Command::Quit),

            // Modals
            "ToggleCommandPalette" => Ok(Command::ToggleCommandPalette),
            "ToggleGotoLine" => Ok(Command::ToggleGotoLine),
            "ToggleFindReplace" => Ok(Command::ToggleFindReplace),
            "OpenRecentFiles" => Ok(Command::OpenRecentFiles),

            // Layout
            "NewTab" => Ok(Command::NewTab),
            "CloseTab" => Ok(Command::CloseTab),
            "NextTab" => Ok(Command::NextTab),
            "PrevTab" => Ok(Command::PrevTab),
            "SplitHorizontal" => Ok(Command::SplitHorizontal),
            "SplitVertical" => Ok(Command::SplitVertical),
            "FocusNextGroup" => Ok(Command::FocusNextGroup),
            "FocusPrevGroup" => Ok(Command::FocusPrevGroup),
            "FocusGroup1" => Ok(Command::FocusGroup1),
            "FocusGroup2" => Ok(Command::FocusGroup2),
            "FocusGroup3" => Ok(Command::FocusGroup3),
            "FocusGroup4" => Ok(Command::FocusGroup4),

            // Workspace
            "ToggleSidebar" => Ok(Command::ToggleSidebar),
            "RevealInSidebar" => Ok(Command::RevealInSidebar),
            "FileTreeSelectPrevious" => Ok(Command::FileTreeSelectPrevious),
            "FileTreeSelectNext" => Ok(Command::FileTreeSelectNext),
            "FileTreeOpenOrToggle" => Ok(Command::FileTreeOpenOrToggle),
            "FileTreeRefresh" => Ok(Command::FileTreeRefresh),

            // Panels/Docks
            "ToggleFileExplorer" => Ok(Command::ToggleFileExplorer),
            "ToggleTerminal" => Ok(Command::ToggleTerminal),
            "ToggleOutline" => Ok(Command::ToggleOutline),
            "CloseFocusedDock" => Ok(Command::CloseFocusedDock),

            // Markdown preview
            "MarkdownTogglePreview" => Ok(Command::MarkdownTogglePreview),
            "MarkdownOpenPreviewToSide" => Ok(Command::MarkdownOpenPreviewToSide),

            // Special
            "EscapeSmartClear" => Ok(Command::EscapeSmartClear),
            "Unbound" => Ok(Command::Unbound),

            _ => Err(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_key() {
        let stroke = parse_key_string("a").unwrap();
        assert_eq!(stroke.key, KeyCode::Char('a'));
        assert!(stroke.mods.is_empty());
    }

    #[test]
    fn test_parse_key_with_modifier() {
        let stroke = parse_key_string("ctrl+s").unwrap();
        assert_eq!(stroke.key, KeyCode::Char('s'));
        assert!(stroke.mods.ctrl());
    }

    #[test]
    fn test_parse_key_with_multiple_modifiers() {
        let stroke = parse_key_string("ctrl+shift+s").unwrap();
        assert_eq!(stroke.key, KeyCode::Char('s'));
        assert!(stroke.mods.ctrl());
        assert!(stroke.mods.shift());
    }

    #[test]
    fn test_parse_cmd_modifier() {
        let stroke = parse_key_string("cmd+s").unwrap();
        assert_eq!(stroke.key, KeyCode::Char('s'));
        // cmd maps to META on macOS, CTRL elsewhere
        assert!(stroke.mods.has_cmd());
    }

    #[test]
    fn test_parse_named_key() {
        let stroke = parse_key_string("enter").unwrap();
        assert_eq!(stroke.key, KeyCode::Enter);

        let stroke = parse_key_string("escape").unwrap();
        assert_eq!(stroke.key, KeyCode::Escape);

        let stroke = parse_key_string("up").unwrap();
        assert_eq!(stroke.key, KeyCode::Up);
    }

    #[test]
    fn test_parse_command() {
        assert_eq!(Command::from_str("SaveFile"), Ok(Command::SaveFile));
        assert_eq!(Command::from_str("Undo"), Ok(Command::Undo));
        assert_eq!(Command::from_str("MoveCursorUp"), Ok(Command::MoveCursorUp));
    }

    #[test]
    fn test_parse_yaml() {
        let yaml = r#"
bindings:
  - key: "cmd+s"
    command: SaveFile
  - key: "cmd+z"
    command: Undo
"#;

        let bindings = parse_keymap_yaml(yaml).unwrap();
        assert_eq!(bindings.len(), 2);
        assert_eq!(bindings[0].command, Command::SaveFile);
        assert_eq!(bindings[1].command, Command::Undo);
    }

    #[test]
    fn test_parse_yaml_with_platform() {
        let yaml = r#"
bindings:
  - key: "cmd+s"
    command: SaveFile
  - key: "meta+left"
    command: MoveCursorLineStart
    platform: macos
"#;

        let bindings = parse_keymap_yaml(yaml).unwrap();

        // On macOS, should have 2 bindings; on other platforms, 1
        #[cfg(target_os = "macos")]
        assert_eq!(bindings.len(), 2);

        #[cfg(not(target_os = "macos"))]
        assert_eq!(bindings.len(), 1);
    }

    #[test]
    fn test_parse_numpad() {
        let stroke = parse_key_string("numpad1").unwrap();
        assert_eq!(stroke.key, KeyCode::Numpad1);

        let stroke = parse_key_string("numpad_add").unwrap();
        assert_eq!(stroke.key, KeyCode::NumpadAdd);
    }
}
