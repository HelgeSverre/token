//! Default keybindings for the editor
//!
//! These are the standard keybindings that ship with the editor.
//! Can be loaded from keymap.yaml at project root, or falls back to hardcoded defaults.

use std::path::Path;

use super::binding::Keybinding;
use super::command::Command;
use super::config::load_keymap_file;
use super::types::{KeyCode, Keystroke, Modifiers};

/// Default keymap YAML embedded at compile time
const DEFAULT_KEYMAP_YAML: &str = include_str!("../../keymap.yaml");

/// Load the default keymap, trying YAML file first, then embedded defaults
///
/// Attempts to load from:
/// 1. keymap.yaml in current directory
/// 2. Embedded default keymap (compiled into binary)
/// 3. Hardcoded fallback defaults
pub fn load_default_keymap() -> Vec<Keybinding> {
    // Try loading from keymap.yaml in current directory
    if let Ok(bindings) = load_keymap_file(Path::new("keymap.yaml")) {
        tracing::info!("Loaded keymap from keymap.yaml");
        return bindings;
    }
    
    // Try parsing embedded YAML
    match super::config::parse_keymap_yaml(DEFAULT_KEYMAP_YAML) {
        Ok(bindings) => {
            tracing::info!("Using embedded default keymap");
            bindings
        }
        Err(e) => {
            tracing::warn!("Failed to parse embedded keymap: {}, using hardcoded defaults", e);
            default_bindings()
        }
    }
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
        
        assert!(has_up && has_down && has_left && has_right, "Should have all arrow key bindings");
    }

    #[test]
    fn test_platform_cmd_key() {
        let cmd = Modifiers::cmd();
        
        #[cfg(target_os = "macos")]
        assert!(cmd.meta(), "cmd() should be META on macOS");
        
        #[cfg(not(target_os = "macos"))]
        assert!(cmd.ctrl(), "cmd() should be CTRL on non-macOS");
    }
}
