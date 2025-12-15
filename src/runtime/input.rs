//! Keyboard input handling
//!
//! This module handles keyboard input that requires special imperative logic
//! beyond what the declarative keymap system can express.
//!
//! Most keybindings are handled by the keymap system in `src/keymap/`.
//! This file handles:
//! - Modal input routing (when a modal dialog is active)
//! - Option double-tap for multi-cursor creation
//! - Navigation with selection collapse (moving clears selection first)
//! - Character input (regular typing)

use winit::keyboard::{Key, NamedKey};

use token::commands::Cmd;
use token::messages::{Direction, DocumentMsg, EditorMsg, ModalMsg, Msg, UiMsg};
use token::model::AppModel;
use token::update::update;

/// Handle keyboard input for special cases not covered by keymap
///
/// Called as a fallback when:
/// - A modal is active (all input routes to modal)
/// - Option double-tap multi-cursor gesture is in progress
/// - Keymap returns NoMatch or a non-simple command
#[allow(clippy::too_many_arguments)]
pub fn handle_key(
    model: &mut AppModel,
    key: Key,
    _physical_key: winit::keyboard::PhysicalKey,
    ctrl: bool,
    shift: bool,
    alt: bool,
    logo: bool,
    option_double_tapped: bool,
) -> Option<Cmd> {
    // Focus capture: route keys to modal when active
    if model.ui.has_modal() {
        return handle_modal_key(model, key, ctrl, shift, alt, logo);
    }

    match key {
        // =====================================================================
        // Multi-cursor: Option double-tap + Arrow
        // This is a temporal gesture (300ms window) that can't be expressed
        // in the keymap, so it's handled here.
        // =====================================================================
        Key::Named(NamedKey::ArrowUp) if alt && option_double_tapped => {
            update(model, Msg::Editor(EditorMsg::AddCursorAbove))
        }
        Key::Named(NamedKey::ArrowDown) if alt && option_double_tapped => {
            update(model, Msg::Editor(EditorMsg::AddCursorBelow))
        }

        // =====================================================================
        // Navigation with selection collapse
        //
        // These navigation commands clear the selection before moving.
        // The keymap handles the movement itself, but can't clear selection first.
        // TODO: Move this logic into the editor's movement handlers.
        // =====================================================================

        // Document navigation (Ctrl+Home/End) - clears selection
        Key::Named(NamedKey::Home) if ctrl && !shift => {
            model.editor_mut().clear_selection();
            update(model, Msg::Editor(EditorMsg::MoveCursorDocumentStart))
        }
        Key::Named(NamedKey::End) if ctrl && !shift => {
            model.editor_mut().clear_selection();
            update(model, Msg::Editor(EditorMsg::MoveCursorDocumentEnd))
        }

        // Line navigation (Cmd+Arrow on macOS) - clears selection
        Key::Named(NamedKey::ArrowLeft) if logo && !shift => {
            model.editor_mut().clear_selection();
            update(model, Msg::Editor(EditorMsg::MoveCursorLineStart))
        }
        Key::Named(NamedKey::ArrowRight) if logo && !shift => {
            model.editor_mut().clear_selection();
            update(model, Msg::Editor(EditorMsg::MoveCursorLineEnd))
        }

        // Line navigation (Home/End keys) - clears selection
        Key::Named(NamedKey::Home) if !shift && !ctrl => {
            model.editor_mut().clear_selection();
            update(model, Msg::Editor(EditorMsg::MoveCursorLineStart))
        }
        Key::Named(NamedKey::End) if !shift && !ctrl => {
            model.editor_mut().clear_selection();
            update(model, Msg::Editor(EditorMsg::MoveCursorLineEnd))
        }

        // Word navigation (Alt+Arrow) - clears selection
        // Note: option_double_tapped case is handled above
        Key::Named(NamedKey::ArrowLeft) if alt && !shift && !option_double_tapped => {
            model.editor_mut().clear_selection();
            update(
                model,
                Msg::Editor(EditorMsg::MoveCursorWord(Direction::Left)),
            )
        }
        Key::Named(NamedKey::ArrowRight) if alt && !shift && !option_double_tapped => {
            model.editor_mut().clear_selection();
            update(
                model,
                Msg::Editor(EditorMsg::MoveCursorWord(Direction::Right)),
            )
        }

        // PageUp/Down with selection - jump to selection edge first
        Key::Named(NamedKey::PageUp) if !shift => {
            if !model.editor().active_selection().is_empty() {
                let start = model.editor().active_selection().start();
                model.editor_mut().active_cursor_mut().line = start.line;
                model.editor_mut().active_cursor_mut().column = start.column;
                model.editor_mut().clear_selection();
            }
            update(model, Msg::Editor(EditorMsg::PageUp))
        }
        Key::Named(NamedKey::PageDown) if !shift => {
            if !model.editor().active_selection().is_empty() {
                let end = model.editor().active_selection().end();
                model.editor_mut().active_cursor_mut().line = end.line;
                model.editor_mut().active_cursor_mut().column = end.column;
                model.editor_mut().clear_selection();
            }
            update(model, Msg::Editor(EditorMsg::PageDown))
        }

        // Arrow Up/Down with selection - jump to selection edge, then move
        Key::Named(NamedKey::ArrowUp) if !shift && !alt && !ctrl && !logo => {
            if !model.editor().active_selection().is_empty() {
                let start = model.editor().active_selection().start();
                model.editor_mut().active_cursor_mut().line = start.line;
                model.editor_mut().active_cursor_mut().column = start.column;
                model.editor_mut().clear_selection();
                update(model, Msg::Editor(EditorMsg::MoveCursor(Direction::Up)))
            } else {
                // No selection - let keymap handle it
                None
            }
        }
        Key::Named(NamedKey::ArrowDown) if !shift && !alt && !ctrl && !logo => {
            if !model.editor().active_selection().is_empty() {
                let end = model.editor().active_selection().end();
                model.editor_mut().active_cursor_mut().line = end.line;
                model.editor_mut().active_cursor_mut().column = end.column;
                model.editor_mut().clear_selection();
                update(model, Msg::Editor(EditorMsg::MoveCursor(Direction::Down)))
            } else {
                // No selection - let keymap handle it
                None
            }
        }

        // =====================================================================
        // Character input
        // Regular typing flows through here, not the keymap.
        // =====================================================================
        Key::Named(NamedKey::Space) if !(ctrl || logo) => {
            update(model, Msg::Document(DocumentMsg::InsertChar(' ')))
        }

        Key::Character(ref s) if !(ctrl || logo) => {
            let mut cmd = None;
            for ch in s.chars() {
                cmd = update(model, Msg::Document(DocumentMsg::InsertChar(ch))).or(cmd);
            }
            cmd
        }

        _ => None,
    }
}

/// Handle keyboard input when a modal is active.
///
/// This captures focus and routes keys to the modal instead of the editor.
#[allow(clippy::too_many_arguments)]
fn handle_modal_key(
    model: &mut AppModel,
    key: Key,
    ctrl: bool,
    _shift: bool,
    alt: bool,
    logo: bool,
) -> Option<Cmd> {
    match key {
        // Escape: close modal
        Key::Named(NamedKey::Escape) => update(model, Msg::Ui(UiMsg::Modal(ModalMsg::Close))),

        // Enter: confirm modal action
        Key::Named(NamedKey::Enter) => update(model, Msg::Ui(UiMsg::Modal(ModalMsg::Confirm))),

        // Arrow keys for navigation in modal lists
        Key::Named(NamedKey::ArrowUp) => {
            update(model, Msg::Ui(UiMsg::Modal(ModalMsg::SelectPrevious)))
        }
        Key::Named(NamedKey::ArrowDown) => {
            update(model, Msg::Ui(UiMsg::Modal(ModalMsg::SelectNext)))
        }

        // Word navigation (Option/Alt + Arrow)
        Key::Named(NamedKey::ArrowLeft) if alt => {
            update(model, Msg::Ui(UiMsg::Modal(ModalMsg::MoveCursorWordLeft)))
        }
        Key::Named(NamedKey::ArrowRight) if alt => {
            update(model, Msg::Ui(UiMsg::Modal(ModalMsg::MoveCursorWordRight)))
        }

        // Word deletion (Option/Alt + Backspace)
        Key::Named(NamedKey::Backspace) if alt => {
            update(model, Msg::Ui(UiMsg::Modal(ModalMsg::DeleteWordBackward)))
        }

        // Backspace: delete character
        Key::Named(NamedKey::Backspace) => {
            update(model, Msg::Ui(UiMsg::Modal(ModalMsg::DeleteBackward)))
        }

        // Character input (only when no Ctrl/Cmd modifiers)
        Key::Character(ref s) if !(ctrl || logo) => {
            let mut cmd = None;
            for ch in s.chars() {
                cmd = update(model, Msg::Ui(UiMsg::Modal(ModalMsg::InsertChar(ch)))).or(cmd);
            }
            cmd
        }

        // Space (without modifiers)
        Key::Named(NamedKey::Space) if !(ctrl || logo) => {
            update(model, Msg::Ui(UiMsg::Modal(ModalMsg::InsertChar(' '))))
        }

        // Block all other keys when modal is active (consume but don't act)
        _ => Some(Cmd::Redraw),
    }
}
