//! Keyboard input handling
//!
//! This module handles keyboard input that requires special imperative logic
//! beyond what the declarative keymap system can express.
//!
//! Most keybindings are handled by the keymap system in `src/keymap/`.
//! This file handles:
//! - Modal input routing (when a modal dialog is active)
//! - CSV cell editing input routing
//! - Option double-tap for multi-cursor creation
//! - Navigation with selection collapse (moving clears selection first)
//! - Character input (regular typing)

use std::time::{Duration, Instant};

use winit::keyboard::{Key, NamedKey};

use token::commands::Cmd;
use token::messages::{
    CsvMsg, Direction, DocumentMsg, EditorMsg, LayoutMsg, ModalMsg, Msg, OutlineMsg, TerminalMsg,
    UiMsg, WorkspaceMsg,
};
use token::model::AppModel;
use token::panel::{DockPosition, PanelId};
use token::terminal::{translate_key, TerminalKeyModifiers};
use token::update::update;

/// Bundles the four keyboard modifier flags (Ctrl, Shift, Alt, Logo/Cmd) that
/// are threaded through nearly every keyboard-handling function in this module.
///
/// Replaces the previous pattern of passing `ctrl: bool, shift: bool, alt: bool,
/// logo: bool` as four separate parameters everywhere.
#[derive(Debug, Clone, Copy, Default)]
pub struct KeyModifiers {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub logo: bool,
}

/// Detects Option key double-tap gesture for multi-cursor mode.
///
/// When the Option key is pressed twice within 300ms, `double_tapped` is set to true.
/// It resets when the key is released.
#[derive(Default)]
pub struct OptionKeyGesture {
    last_press: Option<Instant>,
    pub double_tapped: bool,
}

impl OptionKeyGesture {
    /// Call when the Option key is pressed (non-repeat).
    pub fn on_press(&mut self) {
        let now = Instant::now();
        if let Some(last) = self.last_press {
            if now.duration_since(last) < Duration::from_millis(300) {
                self.double_tapped = true;
            }
        }
        self.last_press = Some(now);
    }

    /// Call when the Option key is released.
    pub fn on_release(&mut self) {
        self.double_tapped = false;
    }
}

/// Handle keyboard input for special cases not covered by keymap
///
/// Called as a fallback when:
/// - A modal is active (all input routes to modal)
/// - CSV cell editing is active (all input routes to cell editor)
/// - Option double-tap multi-cursor gesture is in progress
/// - Keymap returns NoMatch or a non-simple command
pub fn handle_key(
    model: &mut AppModel,
    key: Key,
    _physical_key: winit::keyboard::PhysicalKey,
    modifiers: KeyModifiers,
    option_double_tapped: bool,
) -> Option<Cmd> {
    let KeyModifiers {
        ctrl,
        shift,
        alt,
        logo,
    } = modifiers;

    // Cancel splitter drag with Escape (highest priority)
    if model.ui.splitter_drag.is_some() {
        if let Key::Named(NamedKey::Escape) = key {
            return update(model, Msg::Layout(LayoutMsg::CancelSplitterDrag));
        }
    }

    // Focus capture: route keys to modal when active
    if model.ui.has_modal() {
        return handle_modal_key(model, key, modifiers);
    }

    // Focus capture: route keys to CSV cell editor when editing
    if model.is_csv_editing() {
        return handle_csv_edit_key(model, key, modifiers);
    }

    // Focus capture: route keys exclusively to sidebar when it has focus
    // Keys that sidebar doesn't handle are consumed (not passed to editor)
    if is_sidebar_focused(model) {
        return handle_sidebar_key(model, &key, ctrl).or(Some(Cmd::Redraw));
    }

    // Focus capture: route keys to outline panel when right dock outline has focus
    if is_outline_dock_focused(model) {
        return handle_outline_dock_key(model, &key).or(Some(Cmd::Redraw));
    }

    // Focus capture: route keys to terminal panel when bottom dock terminal has focus
    if is_terminal_dock_focused(model) {
        return handle_terminal_dock_key(model, &key, modifiers).or(Some(Cmd::Redraw));
    }

    // Binary placeholder: Enter opens file with default app
    if let Key::Named(NamedKey::Enter) = key {
        if let Some(path) = get_binary_placeholder_path(model) {
            return update(model, Msg::Layout(LayoutMsg::OpenWithDefaultApp(path)));
        }
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

/// A text-editing action expressed independently of which target (modal input
/// vs. CSV cell editor) it will be dispatched to.
///
/// `handle_modal_key` and `handle_csv_edit_key` handle a near-identical set of
/// keystrokes for cursor movement, selection, clipboard, and character entry;
/// they differ only in which `Msg` variant each action maps to. This enum
/// captures "what the keystroke means" once, so that meaning isn't duplicated
/// across both match blocks.
#[derive(Debug, Clone, PartialEq)]
enum TextEditingKeyAction {
    MoveLeft { extend: bool },
    MoveRight { extend: bool },
    MoveWordLeft { extend: bool },
    MoveWordRight { extend: bool },
    MoveHome { extend: bool },
    MoveEnd { extend: bool },
    SelectAll,
    Copy,
    Cut,
    Paste,
    DeleteWordBackward,
    DeleteBackward,
    DeleteForward,
    InsertText(String),
}

/// Classify a keystroke into a `TextEditingKeyAction`, if it maps to one of
/// the movement/selection/clipboard/insertion behaviors shared by the modal
/// input and CSV cell editor.
///
/// Keys with target-specific behavior (Escape, Enter, Tab, Up/Down, Cmd+Z,
/// alt+Delete word-forward) are intentionally NOT handled here - those are
/// matched directly in `handle_modal_key` / `handle_csv_edit_key` before
/// falling back to this classifier.
fn classify_text_editing_key(key: &Key, modifiers: KeyModifiers) -> Option<TextEditingKeyAction> {
    use TextEditingKeyAction::*;

    let KeyModifiers {
        ctrl,
        shift,
        alt,
        logo,
    } = modifiers;

    match key {
        // Word navigation with selection (Shift+Option+Arrow)
        Key::Named(NamedKey::ArrowLeft) if shift && alt => Some(MoveWordLeft { extend: true }),
        Key::Named(NamedKey::ArrowRight) if shift && alt => Some(MoveWordRight { extend: true }),

        // Word navigation (Option/Alt + Arrow)
        Key::Named(NamedKey::ArrowLeft) if alt => Some(MoveWordLeft { extend: false }),
        Key::Named(NamedKey::ArrowRight) if alt => Some(MoveWordRight { extend: false }),

        // Cursor left/right with selection (Shift+Arrow)
        Key::Named(NamedKey::ArrowLeft) if shift => Some(MoveLeft { extend: true }),
        Key::Named(NamedKey::ArrowRight) if shift => Some(MoveRight { extend: true }),

        // Cursor left/right
        Key::Named(NamedKey::ArrowLeft) => Some(MoveLeft { extend: false }),
        Key::Named(NamedKey::ArrowRight) => Some(MoveRight { extend: false }),

        // Home/End with selection (Shift+Home/End)
        Key::Named(NamedKey::Home) if shift => Some(MoveHome { extend: true }),
        Key::Named(NamedKey::End) if shift => Some(MoveEnd { extend: true }),

        // Home/End (also Cmd+Left/Right on Mac)
        Key::Named(NamedKey::Home) => Some(MoveHome { extend: false }),
        Key::Named(NamedKey::End) => Some(MoveEnd { extend: false }),

        // Select all (Cmd+A)
        Key::Character(s) if logo && s.eq_ignore_ascii_case("a") => Some(SelectAll),

        // Copy (Cmd+C)
        Key::Character(s) if logo && s.eq_ignore_ascii_case("c") => Some(Copy),

        // Cut (Cmd+X)
        Key::Character(s) if logo && s.eq_ignore_ascii_case("x") => Some(Cut),

        // Paste (Cmd+V)
        Key::Character(s) if logo && s.eq_ignore_ascii_case("v") => Some(Paste),

        // Word deletion (Option/Alt + Backspace)
        Key::Named(NamedKey::Backspace) if alt => Some(DeleteWordBackward),

        // Backspace: delete character
        Key::Named(NamedKey::Backspace) => Some(DeleteBackward),

        // Delete key: delete forward
        // (alt+Delete word-forward-deletion, where supported, is special-cased
        // by the caller before this classifier runs)
        Key::Named(NamedKey::Delete) => Some(DeleteForward),

        // Character input (only when no Ctrl/Cmd modifiers)
        Key::Character(s) if !(ctrl || logo) => Some(InsertText(s.to_string())),

        // Space (without modifiers)
        Key::Named(NamedKey::Space) if !(ctrl || logo) => Some(InsertText(" ".to_string())),

        _ => None,
    }
}

/// Dispatch a `TextEditingKeyAction` to the modal input's `ModalMsg` variants.
fn dispatch_modal_text_edit(model: &mut AppModel, action: TextEditingKeyAction) -> Option<Cmd> {
    use TextEditingKeyAction::*;

    match action {
        MoveLeft { extend: false } => {
            update(model, Msg::Ui(UiMsg::Modal(ModalMsg::MoveCursorLeft)))
        }
        MoveLeft { extend: true } => update(
            model,
            Msg::Ui(UiMsg::Modal(ModalMsg::MoveCursorLeftWithSelection)),
        ),
        MoveRight { extend: false } => {
            update(model, Msg::Ui(UiMsg::Modal(ModalMsg::MoveCursorRight)))
        }
        MoveRight { extend: true } => update(
            model,
            Msg::Ui(UiMsg::Modal(ModalMsg::MoveCursorRightWithSelection)),
        ),
        MoveWordLeft { extend: false } => {
            update(model, Msg::Ui(UiMsg::Modal(ModalMsg::MoveCursorWordLeft)))
        }
        MoveWordLeft { extend: true } => update(
            model,
            Msg::Ui(UiMsg::Modal(ModalMsg::MoveCursorWordLeftWithSelection)),
        ),
        MoveWordRight { extend: false } => {
            update(model, Msg::Ui(UiMsg::Modal(ModalMsg::MoveCursorWordRight)))
        }
        MoveWordRight { extend: true } => update(
            model,
            Msg::Ui(UiMsg::Modal(ModalMsg::MoveCursorWordRightWithSelection)),
        ),
        MoveHome { extend: false } => {
            update(model, Msg::Ui(UiMsg::Modal(ModalMsg::MoveCursorHome)))
        }
        MoveHome { extend: true } => update(
            model,
            Msg::Ui(UiMsg::Modal(ModalMsg::MoveCursorHomeWithSelection)),
        ),
        MoveEnd { extend: false } => update(model, Msg::Ui(UiMsg::Modal(ModalMsg::MoveCursorEnd))),
        MoveEnd { extend: true } => update(
            model,
            Msg::Ui(UiMsg::Modal(ModalMsg::MoveCursorEndWithSelection)),
        ),
        SelectAll => update(model, Msg::Ui(UiMsg::Modal(ModalMsg::SelectAll))),
        Copy => update(model, Msg::Ui(UiMsg::Modal(ModalMsg::Copy))),
        Cut => update(model, Msg::Ui(UiMsg::Modal(ModalMsg::Cut))),
        Paste => update(model, Msg::Ui(UiMsg::Modal(ModalMsg::Paste))),
        DeleteWordBackward => update(model, Msg::Ui(UiMsg::Modal(ModalMsg::DeleteWordBackward))),
        DeleteBackward => update(model, Msg::Ui(UiMsg::Modal(ModalMsg::DeleteBackward))),
        DeleteForward => update(model, Msg::Ui(UiMsg::Modal(ModalMsg::DeleteForward))),
        InsertText(s) => {
            let mut cmd = None;
            for ch in s.chars() {
                cmd = update(model, Msg::Ui(UiMsg::Modal(ModalMsg::InsertChar(ch)))).or(cmd);
            }
            cmd
        }
    }
}

/// Dispatch a `TextEditingKeyAction` to the CSV cell editor's `CsvMsg` variants.
fn dispatch_csv_text_edit(model: &mut AppModel, action: TextEditingKeyAction) -> Option<Cmd> {
    use TextEditingKeyAction::*;

    match action {
        MoveLeft { extend: false } => update(model, Msg::Csv(CsvMsg::EditCursorLeft)),
        MoveLeft { extend: true } => update(model, Msg::Csv(CsvMsg::EditCursorLeftWithSelection)),
        MoveRight { extend: false } => update(model, Msg::Csv(CsvMsg::EditCursorRight)),
        MoveRight { extend: true } => update(model, Msg::Csv(CsvMsg::EditCursorRightWithSelection)),
        MoveWordLeft { extend: false } => update(model, Msg::Csv(CsvMsg::EditCursorWordLeft)),
        MoveWordLeft { extend: true } => {
            update(model, Msg::Csv(CsvMsg::EditCursorWordLeftWithSelection))
        }
        MoveWordRight { extend: false } => update(model, Msg::Csv(CsvMsg::EditCursorWordRight)),
        MoveWordRight { extend: true } => {
            update(model, Msg::Csv(CsvMsg::EditCursorWordRightWithSelection))
        }
        MoveHome { extend: false } => update(model, Msg::Csv(CsvMsg::EditCursorHome)),
        MoveHome { extend: true } => update(model, Msg::Csv(CsvMsg::EditCursorHomeWithSelection)),
        MoveEnd { extend: false } => update(model, Msg::Csv(CsvMsg::EditCursorEnd)),
        MoveEnd { extend: true } => update(model, Msg::Csv(CsvMsg::EditCursorEndWithSelection)),
        SelectAll => update(model, Msg::Csv(CsvMsg::EditSelectAll)),
        Copy => update(model, Msg::Csv(CsvMsg::EditCopy)),
        Cut => update(model, Msg::Csv(CsvMsg::EditCut)),
        Paste => update(model, Msg::Csv(CsvMsg::EditPaste)),
        DeleteWordBackward => update(model, Msg::Csv(CsvMsg::EditDeleteWordBackward)),
        DeleteBackward => update(model, Msg::Csv(CsvMsg::EditDeleteBackward)),
        DeleteForward => update(model, Msg::Csv(CsvMsg::EditDeleteForward)),
        InsertText(s) => {
            let mut cmd = None;
            for ch in s.chars() {
                cmd = update(model, Msg::Csv(CsvMsg::EditInsertChar(ch))).or(cmd);
            }
            cmd
        }
    }
}

/// Handle keyboard input when a modal is active.
///
/// This captures focus and routes keys to the modal instead of the editor.
fn handle_modal_key(model: &mut AppModel, key: Key, modifiers: KeyModifiers) -> Option<Cmd> {
    let KeyModifiers { shift, alt, .. } = modifiers;

    match key {
        // Escape: close modal
        Key::Named(NamedKey::Escape) => update(model, Msg::Ui(UiMsg::Modal(ModalMsg::Close))),

        // Enter: confirm modal action
        Key::Named(NamedKey::Enter) => update(model, Msg::Ui(UiMsg::Modal(ModalMsg::Confirm))),

        // Arrow Up/Down for navigation in modal lists (only without modifiers)
        Key::Named(NamedKey::ArrowUp) if !shift && !alt => {
            update(model, Msg::Ui(UiMsg::Modal(ModalMsg::SelectPrevious)))
        }
        Key::Named(NamedKey::ArrowDown) if !shift && !alt => {
            update(model, Msg::Ui(UiMsg::Modal(ModalMsg::SelectNext)))
        }

        // Movement / selection / clipboard / insertion shared with CSV cell editing
        _ => match classify_text_editing_key(&key, modifiers) {
            Some(action) => dispatch_modal_text_edit(model, action),
            // Block all other keys when modal is active (consume but don't act)
            None => Some(Cmd::Redraw),
        },
    }
}

/// Handle keyboard input when editing a CSV cell
///
/// This captures focus and routes keys to the cell editor instead of the normal editor.
fn handle_csv_edit_key(model: &mut AppModel, key: Key, modifiers: KeyModifiers) -> Option<Cmd> {
    let KeyModifiers {
        ctrl,
        shift,
        alt,
        logo,
    } = modifiers;

    match key {
        // Escape: cancel edit
        Key::Named(NamedKey::Escape) => update(model, Msg::Csv(CsvMsg::CancelEdit)),

        // Enter: confirm edit and move down
        Key::Named(NamedKey::Enter) => {
            let cmd = update(model, Msg::Csv(CsvMsg::ConfirmEdit));
            update(model, Msg::Csv(CsvMsg::MoveDown));
            cmd
        }

        // Tab: confirm edit and move to next cell
        Key::Named(NamedKey::Tab) => {
            update(model, Msg::Csv(CsvMsg::ConfirmEdit));
            update(model, Msg::Csv(CsvMsg::NextCell))
        }

        // === Undo/Redo (Cmd+Z / Cmd+Shift+Z) ===
        Key::Character(ref s) if logo && s.eq_ignore_ascii_case("z") => {
            if shift {
                update(model, Msg::Csv(CsvMsg::EditRedo))
            } else {
                update(model, Msg::Csv(CsvMsg::EditUndo))
            }
        }

        // Arrow Up/Down: confirm edit and navigate (CSV-specific: no equivalent
        // in modal input, which uses SelectPrevious/SelectNext instead)
        Key::Named(NamedKey::ArrowUp) => {
            update(model, Msg::Csv(CsvMsg::ConfirmEdit));
            update(model, Msg::Csv(CsvMsg::MoveUp))
        }
        Key::Named(NamedKey::ArrowDown) => {
            update(model, Msg::Csv(CsvMsg::ConfirmEdit));
            update(model, Msg::Csv(CsvMsg::MoveDown))
        }

        // Word deletion forward (Option+Delete) - CSV-specific, modal input has
        // no DeleteWordForward equivalent
        Key::Named(NamedKey::Delete) if alt => {
            update(model, Msg::Csv(CsvMsg::EditDeleteWordForward))
        }

        // CSV-specific: unlike modal input, Ctrl/Cmd+Left/Right is not bound to
        // any cell-editing movement action here, so it falls through to the
        // catch-all instead of reaching the shared movement classification
        // (which does not itself check ctrl/logo for arrow-key movement).
        Key::Named(NamedKey::ArrowLeft) | Key::Named(NamedKey::ArrowRight) if ctrl || logo => {
            Some(Cmd::Redraw)
        }

        // Movement / selection / clipboard / insertion shared with modal editing
        _ => match classify_text_editing_key(&key, modifiers) {
            Some(action) => dispatch_csv_text_edit(model, action),
            // Block all other keys when editing (consume but don't act)
            None => Some(Cmd::Redraw),
        },
    }
}

// =============================================================================
// Sidebar Focus Handling
// =============================================================================

/// Check if the sidebar file tree has keyboard focus
fn is_sidebar_focused(model: &AppModel) -> bool {
    use token::model::FocusTarget;
    matches!(model.ui.focus, FocusTarget::Sidebar)
}

/// Handle keyboard input when sidebar file tree is focused
fn handle_sidebar_key(model: &mut AppModel, key: &Key, ctrl: bool) -> Option<Cmd> {
    match key {
        // Arrow Up/Down: navigate file tree
        Key::Named(NamedKey::ArrowUp) => {
            update(model, Msg::Workspace(WorkspaceMsg::SelectPrevious))
        }
        Key::Named(NamedKey::ArrowDown) => update(model, Msg::Workspace(WorkspaceMsg::SelectNext)),

        // Arrow Right: expand folder or move into children
        Key::Named(NamedKey::ArrowRight) => {
            if let Some(workspace) = &model.workspace {
                if let Some(path) = &workspace.selected_item {
                    if path.is_dir() && !workspace.is_expanded(path) {
                        // Expand the folder
                        let path_clone = path.clone();
                        return update(
                            model,
                            Msg::Workspace(WorkspaceMsg::ExpandFolder(path_clone)),
                        );
                    }
                }
            }
            // If already expanded or is a file, move to next item
            update(model, Msg::Workspace(WorkspaceMsg::SelectNext))
        }

        // Arrow Left: collapse folder or jump to parent
        // Standard file tree behavior:
        // - On expanded folder: collapse it
        // - On collapsed folder or file: jump to parent folder
        Key::Named(NamedKey::ArrowLeft) => {
            if let Some(workspace) = &model.workspace {
                if let Some(path) = &workspace.selected_item {
                    if path.is_dir() && workspace.is_expanded(path) {
                        // Collapse the folder
                        let path_clone = path.clone();
                        return update(
                            model,
                            Msg::Workspace(WorkspaceMsg::CollapseFolder(path_clone)),
                        );
                    }
                }
            }
            // If already collapsed or is a file, jump to parent folder
            update(model, Msg::Workspace(WorkspaceMsg::SelectParent))
        }

        // Enter: open file or toggle folder
        Key::Named(NamedKey::Enter) => update(model, Msg::Workspace(WorkspaceMsg::OpenOrToggle)),

        // Space: toggle folder expansion (files do nothing)
        Key::Named(NamedKey::Space) => {
            if let Some(workspace) = &model.workspace {
                if let Some(path) = workspace.selected_item.clone() {
                    if path.is_dir() {
                        return update(model, Msg::Workspace(WorkspaceMsg::ToggleFolder(path)));
                    }
                }
            }
            Some(Cmd::Redraw)
        }

        // Escape: return focus to editor
        Key::Named(NamedKey::Escape) => {
            model.ui.focus_editor();
            Some(Cmd::Redraw)
        }

        // Cmd+R / Ctrl+R: refresh file tree
        Key::Character(ref s) if (ctrl || cfg!(target_os = "macos")) && s == "r" => {
            update(model, Msg::Workspace(WorkspaceMsg::Refresh))
        }

        // Don't consume other keys - let them fall through to normal handling
        _ => None,
    }
}

/// Check if the outline panel (right dock) has keyboard focus
fn is_outline_dock_focused(model: &AppModel) -> bool {
    if model.ui.focused_dock() != Some(DockPosition::Right) {
        return false;
    }

    let right_dock = model.dock_layout.dock(DockPosition::Right);
    right_dock.is_open && right_dock.active_panel() == Some(PanelId::Outline)
}

/// Handle keyboard input when outline panel is focused
fn handle_outline_dock_key(model: &mut AppModel, key: &Key) -> Option<Cmd> {
    match key {
        Key::Named(NamedKey::ArrowUp) => update(model, Msg::Outline(OutlineMsg::SelectPrevious)),
        Key::Named(NamedKey::ArrowDown) => update(model, Msg::Outline(OutlineMsg::SelectNext)),
        Key::Named(NamedKey::ArrowRight) => update(model, Msg::Outline(OutlineMsg::ExpandSelected)),
        Key::Named(NamedKey::ArrowLeft) => {
            update(model, Msg::Outline(OutlineMsg::CollapseSelected))
        }
        Key::Named(NamedKey::Enter) => update(model, Msg::Outline(OutlineMsg::OpenSelected)),
        Key::Named(NamedKey::Escape) => {
            model.ui.focus_editor();
            Some(Cmd::Redraw)
        }
        _ => None,
    }
}

/// Check if the terminal panel (bottom dock) has keyboard focus.
fn is_terminal_dock_focused(model: &AppModel) -> bool {
    if model.ui.focused_dock() != Some(DockPosition::Bottom) {
        return false;
    }

    let bottom_dock = model.dock_layout.dock(DockPosition::Bottom);
    bottom_dock.is_open && bottom_dock.active_panel() == Some(PanelId::TERMINAL)
}

/// Handle keyboard input when the terminal panel is focused.
fn handle_terminal_dock_key(
    model: &mut AppModel,
    key: &Key,
    modifiers: KeyModifiers,
) -> Option<Cmd> {
    if matches!(key, Key::Named(NamedKey::Escape)) {
        model.ui.focus_editor();
        return Some(Cmd::Redraw);
    }

    if modifiers.logo {
        if let Key::Character(s) = key {
            if s.eq_ignore_ascii_case("v") {
                return Some(Cmd::RequestClipboardPaste);
            }
        }
    }

    if modifiers.shift {
        match key {
            Key::Named(NamedKey::PageUp) => {
                let lines = model
                    .terminal
                    .active_session()
                    .map(|session| session.size.0.saturating_sub(1).max(1))?;
                return update(model, Msg::Terminal(TerminalMsg::ScrollUp(lines)));
            }
            Key::Named(NamedKey::PageDown) => {
                let lines = model
                    .terminal
                    .active_session()
                    .map(|session| session.size.0.saturating_sub(1).max(1))?;
                return update(model, Msg::Terminal(TerminalMsg::ScrollDown(lines)));
            }
            _ => {}
        }
    }

    let session_id = model.terminal.active_session().map(|session| session.id)?;
    let terminal_modifiers = TerminalKeyModifiers {
        ctrl: modifiers.ctrl,
        shift: modifiers.shift,
        alt: modifiers.alt,
        logo: modifiers.logo,
    };
    let bytes = translate_key(key, terminal_modifiers)?;

    update(
        model,
        Msg::Terminal(TerminalMsg::WriteToPty { session_id, bytes }),
    )
}

/// If the focused editor is a binary placeholder tab, return its path
fn get_binary_placeholder_path(model: &AppModel) -> Option<std::path::PathBuf> {
    let editor = model.editor_area.focused_editor()?;
    if let token::model::TabContent::BinaryPlaceholder(ref state) = editor.tab_content {
        Some(state.path.clone())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;

    use super::*;
    use token::model::FocusTarget;
    use token::terminal::{PtyHandle, TerminalSession};
    use winit::keyboard::{KeyCode, PhysicalKey};

    fn focused_terminal_model() -> (AppModel, mpsc::Receiver<Vec<u8>>) {
        let mut model = AppModel::new(800, 600, 1.0, vec![]);
        model.dock_layout.bottom.activate(PanelId::TERMINAL);
        model.ui.focus_dock(DockPosition::Bottom);

        let (pty, pty_rx) = PtyHandle::new_for_test();
        let (msg_tx, _msg_rx) = mpsc::channel();
        model
            .terminal
            .sessions
            .push(TerminalSession::new(7, 24, 80, pty, msg_tx));

        (model, pty_rx)
    }

    #[test]
    fn terminal_character_key_writes_utf8_to_active_session() {
        let (mut model, pty_rx) = focused_terminal_model();

        let cmd = handle_key(
            &mut model,
            Key::Character("å".into()),
            PhysicalKey::Code(KeyCode::KeyA),
            KeyModifiers::default(),
            false,
        );

        assert!(matches!(cmd, Some(Cmd::Redraw)));
        assert_eq!(pty_rx.try_recv().unwrap(), "å".as_bytes());
    }

    #[test]
    fn terminal_ctrl_c_writes_control_byte_to_active_session() {
        let (mut model, pty_rx) = focused_terminal_model();

        let cmd = handle_key(
            &mut model,
            Key::Character("c".into()),
            PhysicalKey::Code(KeyCode::KeyC),
            KeyModifiers {
                ctrl: true,
                ..KeyModifiers::default()
            },
            false,
        );

        assert!(matches!(cmd, Some(Cmd::Redraw)));
        assert_eq!(pty_rx.try_recv().unwrap(), vec![0x03]);
    }

    #[test]
    fn terminal_cmd_v_requests_clipboard_paste() {
        let (mut model, pty_rx) = focused_terminal_model();

        let cmd = handle_key(
            &mut model,
            Key::Character("v".into()),
            PhysicalKey::Code(KeyCode::KeyV),
            KeyModifiers {
                logo: true,
                ..KeyModifiers::default()
            },
            false,
        );

        assert!(matches!(cmd, Some(Cmd::RequestClipboardPaste)));
        assert!(pty_rx.try_recv().is_err());
    }

    #[test]
    fn terminal_escape_returns_focus_to_editor() {
        let (mut model, pty_rx) = focused_terminal_model();

        let cmd = handle_key(
            &mut model,
            Key::Named(NamedKey::Escape),
            PhysicalKey::Code(KeyCode::Escape),
            KeyModifiers::default(),
            false,
        );

        assert!(matches!(cmd, Some(Cmd::Redraw)));
        assert!(matches!(model.ui.focus, FocusTarget::Editor));
        assert!(pty_rx.try_recv().is_err());
    }

    #[test]
    fn terminal_shift_page_up_scrolls_scrollback_without_writing_to_pty() {
        let (mut model, pty_rx) = focused_terminal_model();
        let output: Vec<u8> = (0..60)
            .flat_map(|line| format!("line {line}\r\n").into_bytes())
            .collect();
        model
            .terminal
            .active_session_mut()
            .unwrap()
            .apply_bytes(&output);

        let cmd = handle_key(
            &mut model,
            Key::Named(NamedKey::PageUp),
            PhysicalKey::Code(KeyCode::PageUp),
            KeyModifiers {
                shift: true,
                ..KeyModifiers::default()
            },
            false,
        );

        assert!(cmd.as_ref().is_some_and(Cmd::needs_redraw));
        assert_eq!(model.terminal.active_session().unwrap().scroll_offset, 23);
        assert!(pty_rx.try_recv().is_err());
    }

    #[test]
    fn terminal_shift_page_down_scrolls_toward_bottom_without_writing_to_pty() {
        let (mut model, pty_rx) = focused_terminal_model();
        model.terminal.active_session_mut().unwrap().scroll_offset = 10;

        let cmd = handle_key(
            &mut model,
            Key::Named(NamedKey::PageDown),
            PhysicalKey::Code(KeyCode::PageDown),
            KeyModifiers {
                shift: true,
                ..KeyModifiers::default()
            },
            false,
        );

        assert!(cmd.as_ref().is_some_and(Cmd::needs_redraw));
        assert_eq!(model.terminal.active_session().unwrap().scroll_offset, 0);
        assert!(pty_rx.try_recv().is_err());
    }
}
