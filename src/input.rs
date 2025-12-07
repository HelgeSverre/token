use winit::keyboard::{Key, KeyCode, NamedKey, PhysicalKey};

use token::commands::Cmd;
use token::messages::{AppMsg, Direction, DocumentMsg, EditorMsg, LayoutMsg, Msg};
use token::model::editor_area::SplitDirection;
use token::model::AppModel;
use token::update::update;

pub fn handle_key(
    model: &mut AppModel,
    key: Key,
    physical_key: PhysicalKey,
    ctrl: bool,
    shift: bool,
    alt: bool,
    logo: bool,
    option_double_tapped: bool,
) -> Option<Cmd> {
    // === Numpad Shortcuts (no modifiers needed) ===
    match physical_key {
        PhysicalKey::Code(KeyCode::Numpad1) => {
            return update(model, Msg::Layout(LayoutMsg::FocusGroupByIndex(1)));
        }
        PhysicalKey::Code(KeyCode::Numpad2) => {
            return update(model, Msg::Layout(LayoutMsg::FocusGroupByIndex(2)));
        }
        PhysicalKey::Code(KeyCode::Numpad3) => {
            return update(model, Msg::Layout(LayoutMsg::FocusGroupByIndex(3)));
        }
        PhysicalKey::Code(KeyCode::Numpad4) => {
            return update(model, Msg::Layout(LayoutMsg::FocusGroupByIndex(4)));
        }
        PhysicalKey::Code(KeyCode::NumpadSubtract) => {
            return update(
                model,
                Msg::Layout(LayoutMsg::SplitFocused(SplitDirection::Horizontal)),
            );
        }
        PhysicalKey::Code(KeyCode::NumpadAdd) => {
            return update(
                model,
                Msg::Layout(LayoutMsg::SplitFocused(SplitDirection::Vertical)),
            );
        }
        _ => {}
    }

    // DEBUG: Print when arrow keys are pressed with alt
    #[cfg(debug_assertions)]
    if alt
        && matches!(
            key,
            Key::Named(NamedKey::ArrowUp) | Key::Named(NamedKey::ArrowDown)
        )
    {
        eprintln!(
            "[DEBUG] Arrow key with alt: key={:?}, option_double_tapped={}",
            key, option_double_tapped
        );
    }

    match key {
        // Double-tap Option + Arrow for multi-cursor (must be before other alt combinations)
        Key::Named(NamedKey::ArrowUp) if alt && option_double_tapped => {
            #[cfg(debug_assertions)]
            eprintln!("[DEBUG] AddCursorAbove triggered");
            update(model, Msg::Editor(EditorMsg::AddCursorAbove))
        }
        Key::Named(NamedKey::ArrowDown) if alt && option_double_tapped => {
            #[cfg(debug_assertions)]
            eprintln!("[DEBUG] AddCursorBelow triggered");
            update(model, Msg::Editor(EditorMsg::AddCursorBelow))
        }

        // Expand/Shrink Selection (Option+Up/Down without double-tap)
        Key::Named(NamedKey::ArrowUp) if alt && !shift => {
            update(model, Msg::Editor(EditorMsg::ExpandSelection))
        }
        Key::Named(NamedKey::ArrowDown) if alt && !shift => {
            update(model, Msg::Editor(EditorMsg::ShrinkSelection))
        }
        // Undo/Redo (Ctrl/Cmd+Z, Ctrl/Cmd+Shift+Z, Ctrl/Cmd+Y)
        Key::Character(ref s) if (ctrl || logo) && s.eq_ignore_ascii_case("z") => {
            if shift {
                update(model, Msg::Document(DocumentMsg::Redo))
            } else {
                update(model, Msg::Document(DocumentMsg::Undo))
            }
        }
        Key::Character(ref s) if (ctrl || logo) && s.eq_ignore_ascii_case("y") => {
            update(model, Msg::Document(DocumentMsg::Redo))
        }

        // Save file (Ctrl+S on Windows/Linux, Cmd+S on macOS)
        Key::Character(ref s) if s.eq_ignore_ascii_case("s") && (ctrl || logo) => {
            update(model, Msg::App(AppMsg::SaveFile))
        }

        // Select All (Cmd+A on macOS, Ctrl+A elsewhere)
        Key::Character(ref s) if s.eq_ignore_ascii_case("a") && (ctrl || logo) => {
            update(model, Msg::Editor(EditorMsg::SelectAll))
        }

        // Copy (Cmd+C on macOS, Ctrl+C elsewhere)
        Key::Character(ref s) if s.eq_ignore_ascii_case("c") && (ctrl || logo) => {
            update(model, Msg::Document(DocumentMsg::Copy))
        }

        // Cut (Cmd+X on macOS, Ctrl+X elsewhere)
        Key::Character(ref s) if s.eq_ignore_ascii_case("x") && (ctrl || logo) => {
            update(model, Msg::Document(DocumentMsg::Cut))
        }

        // Paste (Cmd+V on macOS, Ctrl+V elsewhere)
        Key::Character(ref s) if s.eq_ignore_ascii_case("v") && (ctrl || logo) => {
            update(model, Msg::Document(DocumentMsg::Paste))
        }

        // Duplicate line/selection (Cmd+D on macOS, Ctrl+D elsewhere)
        Key::Character(ref s) if s.eq_ignore_ascii_case("d") && (ctrl || logo) => {
            update(model, Msg::Document(DocumentMsg::Duplicate))
        }

        // Select next occurrence (Cmd+J on macOS, Ctrl+J elsewhere)
        Key::Character(ref s) if s.eq_ignore_ascii_case("j") && (ctrl || logo) && !shift => {
            update(model, Msg::Editor(EditorMsg::SelectNextOccurrence))
        }

        // Unselect last occurrence (Shift+Cmd+J on macOS, Shift+Ctrl+J elsewhere)
        Key::Character(ref s) if s.eq_ignore_ascii_case("j") && (ctrl || logo) && shift => {
            update(model, Msg::Editor(EditorMsg::UnselectOccurrence))
        }

        // === Split View Shortcuts ===

        // Split horizontal (Shift+Option+Cmd+H)
        Key::Character(ref s) if s.eq_ignore_ascii_case("h") && logo && shift && alt => update(
            model,
            Msg::Layout(LayoutMsg::SplitFocused(SplitDirection::Horizontal)),
        ),

        // Split vertical (Shift+Option+Cmd+V)
        Key::Character(ref s) if s.eq_ignore_ascii_case("v") && logo && shift && alt => update(
            model,
            Msg::Layout(LayoutMsg::SplitFocused(SplitDirection::Vertical)),
        ),

        // Close tab (Cmd+W)
        Key::Character(ref s) if s.eq_ignore_ascii_case("w") && logo && !shift && !alt => {
            update(model, Msg::Layout(LayoutMsg::CloseFocusedTab))
        }

        // Next tab (Option+Cmd+Right)
        Key::Named(NamedKey::ArrowRight) if logo && alt && !shift => {
            update(model, Msg::Layout(LayoutMsg::NextTab))
        }

        // Previous tab (Option+Cmd+Left)
        Key::Named(NamedKey::ArrowLeft) if logo && alt && !shift => {
            update(model, Msg::Layout(LayoutMsg::PrevTab))
        }

        // Focus group by index (Shift+Cmd+1/2/3/4)
        Key::Character(ref s) if s == "1" && logo && shift && !alt => {
            update(model, Msg::Layout(LayoutMsg::FocusGroupByIndex(1)))
        }
        Key::Character(ref s) if s == "2" && logo && shift && !alt => {
            update(model, Msg::Layout(LayoutMsg::FocusGroupByIndex(2)))
        }
        Key::Character(ref s) if s == "3" && logo && shift && !alt => {
            update(model, Msg::Layout(LayoutMsg::FocusGroupByIndex(3)))
        }
        Key::Character(ref s) if s == "4" && logo && shift && !alt => {
            update(model, Msg::Layout(LayoutMsg::FocusGroupByIndex(4)))
        }

        // Focus next/previous group (Ctrl+Tab / Ctrl+Shift+Tab)
        Key::Named(NamedKey::Tab) if ctrl && !shift => {
            update(model, Msg::Layout(LayoutMsg::FocusNextGroup))
        }
        Key::Named(NamedKey::Tab) if ctrl && shift => {
            update(model, Msg::Layout(LayoutMsg::FocusPrevGroup))
        }

        // Indent/Unindent (Tab / Shift+Tab)
        Key::Named(NamedKey::Tab) if shift && !(ctrl || logo) => {
            update(model, Msg::Document(DocumentMsg::UnindentLines))
        }
        Key::Named(NamedKey::Tab) if !(ctrl || logo) => {
            if model.editor().active_selection().is_empty() {
                update(model, Msg::Document(DocumentMsg::InsertChar('\t')))
            } else {
                update(model, Msg::Document(DocumentMsg::IndentLines))
            }
        }

        // Escape: clear selection or collapse to single cursor
        Key::Named(NamedKey::Escape) => {
            if model.editor().has_multiple_cursors() {
                update(model, Msg::Editor(EditorMsg::CollapseToSingleCursor))
            } else if !model.editor().active_selection().is_empty() {
                update(model, Msg::Editor(EditorMsg::ClearSelection))
            } else {
                None
            }
        }

        // Document navigation with selection (Shift+Ctrl+Home/End)
        Key::Named(NamedKey::Home) if ctrl && shift => update(
            model,
            Msg::Editor(EditorMsg::MoveCursorDocumentStartWithSelection),
        ),
        Key::Named(NamedKey::End) if ctrl && shift => update(
            model,
            Msg::Editor(EditorMsg::MoveCursorDocumentEndWithSelection),
        ),

        // Document navigation (Ctrl+Home/End)
        Key::Named(NamedKey::Home) if ctrl => {
            model.editor_mut().clear_selection();
            update(model, Msg::Editor(EditorMsg::MoveCursorDocumentStart))
        }
        Key::Named(NamedKey::End) if ctrl => {
            model.editor_mut().clear_selection();
            update(model, Msg::Editor(EditorMsg::MoveCursorDocumentEnd))
        }

        // Line navigation with selection (Shift+Cmd+Arrow on macOS)
        Key::Named(NamedKey::ArrowLeft) if logo && shift => update(
            model,
            Msg::Editor(EditorMsg::MoveCursorLineStartWithSelection),
        ),
        Key::Named(NamedKey::ArrowRight) if logo && shift => update(
            model,
            Msg::Editor(EditorMsg::MoveCursorLineEndWithSelection),
        ),

        // Line navigation (Cmd+Arrow on macOS)
        Key::Named(NamedKey::ArrowLeft) if logo => {
            model.editor_mut().clear_selection();
            update(model, Msg::Editor(EditorMsg::MoveCursorLineStart))
        }
        Key::Named(NamedKey::ArrowRight) if logo => {
            model.editor_mut().clear_selection();
            update(model, Msg::Editor(EditorMsg::MoveCursorLineEnd))
        }

        // Line navigation with selection (Shift+Home/End)
        Key::Named(NamedKey::Home) if shift => update(
            model,
            Msg::Editor(EditorMsg::MoveCursorLineStartWithSelection),
        ),
        Key::Named(NamedKey::End) if shift => update(
            model,
            Msg::Editor(EditorMsg::MoveCursorLineEndWithSelection),
        ),

        // Line navigation (Home/End keys)
        Key::Named(NamedKey::Home) => {
            model.editor_mut().clear_selection();
            update(model, Msg::Editor(EditorMsg::MoveCursorLineStart))
        }
        Key::Named(NamedKey::End) => {
            model.editor_mut().clear_selection();
            update(model, Msg::Editor(EditorMsg::MoveCursorLineEnd))
        }

        // Page navigation with selection (Shift+PageUp/Down)
        Key::Named(NamedKey::PageUp) if shift => {
            update(model, Msg::Editor(EditorMsg::PageUpWithSelection))
        }
        Key::Named(NamedKey::PageDown) if shift => {
            update(model, Msg::Editor(EditorMsg::PageDownWithSelection))
        }

        // Page navigation
        Key::Named(NamedKey::PageUp) => {
            if !model.editor().active_selection().is_empty() {
                // Jump to selection START, then page up
                let start = model.editor().active_selection().start();
                model.editor_mut().active_cursor_mut().line = start.line;
                model.editor_mut().active_cursor_mut().column = start.column;
                model.editor_mut().clear_selection();
            }
            update(model, Msg::Editor(EditorMsg::PageUp))
        }
        Key::Named(NamedKey::PageDown) => {
            if !model.editor().active_selection().is_empty() {
                // Jump to selection END, then page down
                let end = model.editor().active_selection().end();
                model.editor_mut().active_cursor_mut().line = end.line;
                model.editor_mut().active_cursor_mut().column = end.column;
                model.editor_mut().clear_selection();
            }
            update(model, Msg::Editor(EditorMsg::PageDown))
        }

        // Word navigation with selection (Shift+Option/Alt + Arrow)
        Key::Named(NamedKey::ArrowLeft) if alt && shift => update(
            model,
            Msg::Editor(EditorMsg::MoveCursorWordWithSelection(Direction::Left)),
        ),
        Key::Named(NamedKey::ArrowRight) if alt && shift => update(
            model,
            Msg::Editor(EditorMsg::MoveCursorWordWithSelection(Direction::Right)),
        ),

        // Word navigation (Option/Alt + Arrow)
        Key::Named(NamedKey::ArrowLeft) if alt => {
            model.editor_mut().clear_selection();
            update(
                model,
                Msg::Editor(EditorMsg::MoveCursorWord(Direction::Left)),
            )
        }
        Key::Named(NamedKey::ArrowRight) if alt => {
            model.editor_mut().clear_selection();
            update(
                model,
                Msg::Editor(EditorMsg::MoveCursorWord(Direction::Right)),
            )
        }

        // Arrow keys with selection (Shift+Arrow)
        Key::Named(NamedKey::ArrowUp) if shift => update(
            model,
            Msg::Editor(EditorMsg::MoveCursorWithSelection(Direction::Up)),
        ),
        Key::Named(NamedKey::ArrowDown) if shift => update(
            model,
            Msg::Editor(EditorMsg::MoveCursorWithSelection(Direction::Down)),
        ),
        Key::Named(NamedKey::ArrowLeft) if shift => update(
            model,
            Msg::Editor(EditorMsg::MoveCursorWithSelection(Direction::Left)),
        ),
        Key::Named(NamedKey::ArrowRight) if shift => update(
            model,
            Msg::Editor(EditorMsg::MoveCursorWithSelection(Direction::Right)),
        ),

        // Arrow keys (with selection: jump to start/end, then optionally move)
        Key::Named(NamedKey::ArrowUp) => {
            if !model.editor().active_selection().is_empty() {
                // Jump to selection START, then move up
                let start = model.editor().active_selection().start();
                model.editor_mut().active_cursor_mut().line = start.line;
                model.editor_mut().active_cursor_mut().column = start.column;
                model.editor_mut().clear_selection();
                update(model, Msg::Editor(EditorMsg::MoveCursor(Direction::Up)))
            } else {
                update(model, Msg::Editor(EditorMsg::MoveCursor(Direction::Up)))
            }
        }
        Key::Named(NamedKey::ArrowDown) => {
            if !model.editor().active_selection().is_empty() {
                // Jump to selection END, then move down
                let end = model.editor().active_selection().end();
                model.editor_mut().active_cursor_mut().line = end.line;
                model.editor_mut().active_cursor_mut().column = end.column;
                model.editor_mut().clear_selection();
                update(model, Msg::Editor(EditorMsg::MoveCursor(Direction::Down)))
            } else {
                update(model, Msg::Editor(EditorMsg::MoveCursor(Direction::Down)))
            }
        }
        Key::Named(NamedKey::ArrowLeft) => {
            if !model.editor().active_selection().is_empty() {
                // Jump to selection START (no additional move)
                let start = model.editor().active_selection().start();
                model.editor_mut().active_cursor_mut().line = start.line;
                model.editor_mut().active_cursor_mut().column = start.column;
                model.editor_mut().clear_selection();
                model.ensure_cursor_visible();
                model.reset_cursor_blink();
                Some(Cmd::Redraw)
            } else {
                update(model, Msg::Editor(EditorMsg::MoveCursor(Direction::Left)))
            }
        }
        Key::Named(NamedKey::ArrowRight) => {
            if !model.editor().active_selection().is_empty() {
                // Jump to selection END (no additional move)
                let end = model.editor().active_selection().end();
                model.editor_mut().active_cursor_mut().line = end.line;
                model.editor_mut().active_cursor_mut().column = end.column;
                model.editor_mut().clear_selection();
                model.ensure_cursor_visible();
                model.reset_cursor_blink();
                Some(Cmd::Redraw)
            } else {
                update(model, Msg::Editor(EditorMsg::MoveCursor(Direction::Right)))
            }
        }

        // Editing
        Key::Named(NamedKey::Enter) => update(model, Msg::Document(DocumentMsg::InsertNewline)),
        Key::Named(NamedKey::Backspace) if ctrl || logo => {
            update(model, Msg::Document(DocumentMsg::DeleteLine))
        }
        Key::Named(NamedKey::Backspace) => {
            update(model, Msg::Document(DocumentMsg::DeleteBackward))
        }
        Key::Named(NamedKey::Delete) => update(model, Msg::Document(DocumentMsg::DeleteForward)),
        Key::Named(NamedKey::Space) if !(ctrl || logo) => {
            update(model, Msg::Document(DocumentMsg::InsertChar(' ')))
        }

        // Character input (only when no Ctrl/Cmd)
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
