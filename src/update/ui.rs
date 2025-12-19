//! UI message handlers (status bar, cursor blink, transient messages, modals)

use std::time::Duration;

use crate::commands::{filter_commands, Cmd};
use crate::messages::{ModalMsg, UiMsg};
use crate::model::{
    AppModel, FindReplaceState, GotoLineState, ModalId, ModalState, SegmentContent, SegmentId,
    ThemePickerState, TransientMessage,
};
use crate::theme::load_theme;

use super::app::execute_command;

/// Handle UI messages (status bar, cursor blink, modals)
pub fn update_ui(model: &mut AppModel, msg: UiMsg) -> Option<Cmd> {
    match msg {
        UiMsg::SetStatus(message) => {
            // Legacy: also update the StatusMessage segment
            model.ui.status_bar.update_segment(
                SegmentId::StatusMessage,
                SegmentContent::Text(message.clone()),
            );
            model.ui.set_status(message);
            Some(Cmd::Redraw)
        }

        UiMsg::BlinkCursor => {
            if model.ui.update_cursor_blink(Duration::from_millis(500)) {
                Some(Cmd::Redraw)
            } else {
                None
            }
        }

        UiMsg::UpdateSegment { id, content } => {
            model.ui.status_bar.update_segment(id, content);
            Some(Cmd::Redraw)
        }

        UiMsg::SetTransientMessage { text, duration_ms } => {
            let transient = TransientMessage::new(text.clone(), Duration::from_millis(duration_ms));
            model.ui.transient_message = Some(transient);
            // Also update the StatusMessage segment
            model
                .ui
                .status_bar
                .update_segment(SegmentId::StatusMessage, SegmentContent::Text(text));
            Some(Cmd::Redraw)
        }

        UiMsg::ClearTransientMessage => {
            model.ui.transient_message = None;
            model
                .ui
                .status_bar
                .update_segment(SegmentId::StatusMessage, SegmentContent::Empty);
            Some(Cmd::Redraw)
        }

        UiMsg::Modal(modal_msg) => update_modal(model, modal_msg),

        UiMsg::ToggleModal(modal_id) => {
            if let Some(ref active) = model.ui.active_modal {
                if active.id() == modal_id {
                    // Close if same modal
                    model.ui.close_modal();
                    return Some(Cmd::Redraw);
                }
            }
            // Open the requested modal
            let state = match modal_id {
                ModalId::CommandPalette => {
                    let state = model.ui.last_command_palette.clone().unwrap_or_default();
                    ModalState::CommandPalette(state)
                }
                ModalId::GotoLine => ModalState::GotoLine(GotoLineState::default()),
                ModalId::FindReplace => ModalState::FindReplace(FindReplaceState::default()),
                ModalId::ThemePicker => ModalState::ThemePicker(ThemePickerState::default()),
            };
            model.ui.open_modal(state);
            Some(Cmd::Redraw)
        }

        // === File Drag-and-Drop ===
        UiMsg::FileHovered(path) => {
            model.ui.drop_state.start_hover(path);
            Some(Cmd::Redraw)
        }

        UiMsg::FileHoverCancelled => {
            model.ui.drop_state.cancel_hover();
            Some(Cmd::Redraw)
        }
    }
}

/// Handle modal-specific messages
fn update_modal(model: &mut AppModel, msg: ModalMsg) -> Option<Cmd> {
    match msg {
        ModalMsg::OpenCommandPalette => {
            let state = model.ui.last_command_palette.clone().unwrap_or_default();
            model.ui.open_modal(ModalState::CommandPalette(state));
            Some(Cmd::Redraw)
        }

        ModalMsg::OpenGotoLine => {
            model
                .ui
                .open_modal(ModalState::GotoLine(GotoLineState::default()));
            Some(Cmd::Redraw)
        }

        ModalMsg::OpenFindReplace => {
            model
                .ui
                .open_modal(ModalState::FindReplace(FindReplaceState::default()));
            Some(Cmd::Redraw)
        }

        ModalMsg::Close => {
            model.ui.close_modal();
            Some(Cmd::Redraw)
        }

        ModalMsg::SetInput(text) => {
            if let Some(ref mut modal) = model.ui.active_modal {
                match modal {
                    ModalState::CommandPalette(state) => state.set_input(&text),
                    ModalState::GotoLine(state) => state.set_input(&text),
                    ModalState::FindReplace(state) => state.set_query(&text),
                    ModalState::ThemePicker(_) => {} // No text input for theme picker
                }
                Some(Cmd::Redraw)
            } else {
                None
            }
        }

        ModalMsg::InsertChar(ch) => {
            if let Some(ref mut modal) = model.ui.active_modal {
                match modal {
                    ModalState::CommandPalette(state) => {
                        state.editable.insert_char(ch);
                        state.selected_index = 0; // Reset selection when input changes
                    }
                    ModalState::GotoLine(state) => {
                        // EditableState handles the char filter constraint
                        state.editable.insert_char(ch);
                    }
                    ModalState::FindReplace(state) => {
                        state.focused_editable_mut().insert_char(ch);
                    }
                    ModalState::ThemePicker(_) => {} // No text input for theme picker
                }
                Some(Cmd::Redraw)
            } else {
                None
            }
        }

        ModalMsg::DeleteBackward => {
            if let Some(ref mut modal) = model.ui.active_modal {
                match modal {
                    ModalState::CommandPalette(state) => {
                        state.editable.delete_backward();
                        state.selected_index = 0; // Reset selection when input changes
                    }
                    ModalState::GotoLine(state) => {
                        state.editable.delete_backward();
                    }
                    ModalState::FindReplace(state) => {
                        state.focused_editable_mut().delete_backward();
                    }
                    ModalState::ThemePicker(_) => {} // No text input for theme picker
                }
                Some(Cmd::Redraw)
            } else {
                None
            }
        }

        ModalMsg::DeleteWordBackward => {
            if let Some(ref mut modal) = model.ui.active_modal {
                match modal {
                    ModalState::CommandPalette(state) => {
                        state.editable.delete_word_backward();
                        state.selected_index = 0;
                    }
                    ModalState::GotoLine(state) => {
                        state.editable.delete_word_backward();
                    }
                    ModalState::FindReplace(state) => {
                        state.focused_editable_mut().delete_word_backward();
                    }
                    ModalState::ThemePicker(_) => {} // No text input for theme picker
                }
                Some(Cmd::Redraw)
            } else {
                None
            }
        }

        ModalMsg::MoveCursorWordLeft => {
            if let Some(ref mut modal) = model.ui.active_modal {
                match modal {
                    ModalState::CommandPalette(state) => {
                        state.editable.move_word_left(false);
                    }
                    ModalState::GotoLine(state) => {
                        state.editable.move_word_left(false);
                    }
                    ModalState::FindReplace(state) => {
                        state.focused_editable_mut().move_word_left(false);
                    }
                    ModalState::ThemePicker(_) => {}
                }
            }
            Some(Cmd::Redraw)
        }

        ModalMsg::MoveCursorWordRight => {
            if let Some(ref mut modal) = model.ui.active_modal {
                match modal {
                    ModalState::CommandPalette(state) => {
                        state.editable.move_word_right(false);
                    }
                    ModalState::GotoLine(state) => {
                        state.editable.move_word_right(false);
                    }
                    ModalState::FindReplace(state) => {
                        state.focused_editable_mut().move_word_right(false);
                    }
                    ModalState::ThemePicker(_) => {}
                }
            }
            Some(Cmd::Redraw)
        }

        ModalMsg::MoveCursorLeft => {
            if let Some(ref mut modal) = model.ui.active_modal {
                match modal {
                    ModalState::CommandPalette(state) => {
                        state.editable.move_left(false);
                    }
                    ModalState::GotoLine(state) => {
                        state.editable.move_left(false);
                    }
                    ModalState::FindReplace(state) => {
                        state.focused_editable_mut().move_left(false);
                    }
                    ModalState::ThemePicker(_) => {}
                }
            }
            Some(Cmd::Redraw)
        }

        ModalMsg::MoveCursorRight => {
            if let Some(ref mut modal) = model.ui.active_modal {
                match modal {
                    ModalState::CommandPalette(state) => {
                        state.editable.move_right(false);
                    }
                    ModalState::GotoLine(state) => {
                        state.editable.move_right(false);
                    }
                    ModalState::FindReplace(state) => {
                        state.focused_editable_mut().move_right(false);
                    }
                    ModalState::ThemePicker(_) => {}
                }
            }
            Some(Cmd::Redraw)
        }

        ModalMsg::MoveCursorHome => {
            if let Some(ref mut modal) = model.ui.active_modal {
                match modal {
                    ModalState::CommandPalette(state) => {
                        state.editable.move_line_start(false);
                    }
                    ModalState::GotoLine(state) => {
                        state.editable.move_line_start(false);
                    }
                    ModalState::FindReplace(state) => {
                        state.focused_editable_mut().move_line_start(false);
                    }
                    ModalState::ThemePicker(_) => {}
                }
            }
            Some(Cmd::Redraw)
        }

        ModalMsg::MoveCursorEnd => {
            if let Some(ref mut modal) = model.ui.active_modal {
                match modal {
                    ModalState::CommandPalette(state) => {
                        state.editable.move_line_end(false);
                    }
                    ModalState::GotoLine(state) => {
                        state.editable.move_line_end(false);
                    }
                    ModalState::FindReplace(state) => {
                        state.focused_editable_mut().move_line_end(false);
                    }
                    ModalState::ThemePicker(_) => {}
                }
            }
            Some(Cmd::Redraw)
        }

        ModalMsg::MoveCursorLeftWithSelection => {
            if let Some(ref mut modal) = model.ui.active_modal {
                match modal {
                    ModalState::CommandPalette(state) => {
                        state.editable.move_left(true);
                    }
                    ModalState::GotoLine(state) => {
                        state.editable.move_left(true);
                    }
                    ModalState::FindReplace(state) => {
                        state.focused_editable_mut().move_left(true);
                    }
                    ModalState::ThemePicker(_) => {}
                }
            }
            Some(Cmd::Redraw)
        }

        ModalMsg::MoveCursorRightWithSelection => {
            if let Some(ref mut modal) = model.ui.active_modal {
                match modal {
                    ModalState::CommandPalette(state) => {
                        state.editable.move_right(true);
                    }
                    ModalState::GotoLine(state) => {
                        state.editable.move_right(true);
                    }
                    ModalState::FindReplace(state) => {
                        state.focused_editable_mut().move_right(true);
                    }
                    ModalState::ThemePicker(_) => {}
                }
            }
            Some(Cmd::Redraw)
        }

        ModalMsg::MoveCursorHomeWithSelection => {
            if let Some(ref mut modal) = model.ui.active_modal {
                match modal {
                    ModalState::CommandPalette(state) => {
                        state.editable.move_line_start(true);
                    }
                    ModalState::GotoLine(state) => {
                        state.editable.move_line_start(true);
                    }
                    ModalState::FindReplace(state) => {
                        state.focused_editable_mut().move_line_start(true);
                    }
                    ModalState::ThemePicker(_) => {}
                }
            }
            Some(Cmd::Redraw)
        }

        ModalMsg::MoveCursorEndWithSelection => {
            if let Some(ref mut modal) = model.ui.active_modal {
                match modal {
                    ModalState::CommandPalette(state) => {
                        state.editable.move_line_end(true);
                    }
                    ModalState::GotoLine(state) => {
                        state.editable.move_line_end(true);
                    }
                    ModalState::FindReplace(state) => {
                        state.focused_editable_mut().move_line_end(true);
                    }
                    ModalState::ThemePicker(_) => {}
                }
            }
            Some(Cmd::Redraw)
        }

        ModalMsg::MoveCursorWordLeftWithSelection => {
            if let Some(ref mut modal) = model.ui.active_modal {
                match modal {
                    ModalState::CommandPalette(state) => {
                        state.editable.move_word_left(true);
                    }
                    ModalState::GotoLine(state) => {
                        state.editable.move_word_left(true);
                    }
                    ModalState::FindReplace(state) => {
                        state.focused_editable_mut().move_word_left(true);
                    }
                    ModalState::ThemePicker(_) => {}
                }
            }
            Some(Cmd::Redraw)
        }

        ModalMsg::MoveCursorWordRightWithSelection => {
            if let Some(ref mut modal) = model.ui.active_modal {
                match modal {
                    ModalState::CommandPalette(state) => {
                        state.editable.move_word_right(true);
                    }
                    ModalState::GotoLine(state) => {
                        state.editable.move_word_right(true);
                    }
                    ModalState::FindReplace(state) => {
                        state.focused_editable_mut().move_word_right(true);
                    }
                    ModalState::ThemePicker(_) => {}
                }
            }
            Some(Cmd::Redraw)
        }

        ModalMsg::SelectAll => {
            if let Some(ref mut modal) = model.ui.active_modal {
                match modal {
                    ModalState::CommandPalette(state) => {
                        state.editable.select_all();
                    }
                    ModalState::GotoLine(state) => {
                        state.editable.select_all();
                    }
                    ModalState::FindReplace(state) => {
                        state.focused_editable_mut().select_all();
                    }
                    ModalState::ThemePicker(_) => {}
                }
            }
            Some(Cmd::Redraw)
        }

        ModalMsg::Copy => {
            if let Some(ref mut modal) = model.ui.active_modal {
                let text = match modal {
                    ModalState::CommandPalette(state) => state.editable.selected_text(),
                    ModalState::GotoLine(state) => state.editable.selected_text(),
                    ModalState::FindReplace(state) => state.focused_editable_mut().selected_text(),
                    ModalState::ThemePicker(_) => String::new(),
                };
                if !text.is_empty() {
                    if let Ok(mut clipboard) = arboard::Clipboard::new() {
                        let _ = clipboard.set_text(&text);
                    }
                }
            }
            Some(Cmd::Redraw)
        }

        ModalMsg::Cut => {
            if let Some(ref mut modal) = model.ui.active_modal {
                let text = match modal {
                    ModalState::CommandPalette(state) => {
                        let t = state.editable.selected_text();
                        if !t.is_empty() {
                            state.editable.delete_backward();
                            state.selected_index = 0;
                        }
                        t
                    }
                    ModalState::GotoLine(state) => {
                        let t = state.editable.selected_text();
                        if !t.is_empty() {
                            state.editable.delete_backward();
                        }
                        t
                    }
                    ModalState::FindReplace(state) => {
                        let editable = state.focused_editable_mut();
                        let t = editable.selected_text();
                        if !t.is_empty() {
                            editable.delete_backward();
                        }
                        t
                    }
                    ModalState::ThemePicker(_) => String::new(),
                };
                if !text.is_empty() {
                    if let Ok(mut clipboard) = arboard::Clipboard::new() {
                        let _ = clipboard.set_text(&text);
                    }
                }
            }
            Some(Cmd::Redraw)
        }

        ModalMsg::Paste => {
            let clipboard_text = if let Ok(mut clipboard) = arboard::Clipboard::new() {
                clipboard.get_text().ok()
            } else {
                None
            };

            if let Some(text) = clipboard_text {
                // Filter out newlines for single-line modal inputs
                let filtered: String = text.chars().filter(|c| *c != '\n' && *c != '\r').collect();
                if !filtered.is_empty() {
                    if let Some(ref mut modal) = model.ui.active_modal {
                        match modal {
                            ModalState::CommandPalette(state) => {
                                state.editable.insert_text(&filtered);
                                state.selected_index = 0;
                            }
                            ModalState::GotoLine(state) => {
                                // Filter to only digits for goto line
                                let digits: String =
                                    filtered.chars().filter(|c| c.is_ascii_digit()).collect();
                                state.editable.insert_text(&digits);
                            }
                            ModalState::FindReplace(state) => {
                                state.focused_editable_mut().insert_text(&filtered);
                            }
                            ModalState::ThemePicker(_) => {}
                        }
                    }
                }
            }
            Some(Cmd::Redraw)
        }

        ModalMsg::DeleteForward => {
            if let Some(ref mut modal) = model.ui.active_modal {
                match modal {
                    ModalState::CommandPalette(state) => {
                        state.editable.delete_forward();
                        state.selected_index = 0;
                    }
                    ModalState::GotoLine(state) => {
                        state.editable.delete_forward();
                    }
                    ModalState::FindReplace(state) => {
                        state.focused_editable_mut().delete_forward();
                    }
                    ModalState::ThemePicker(_) => {}
                }
                Some(Cmd::Redraw)
            } else {
                None
            }
        }

        ModalMsg::SelectPrevious => {
            if let Some(ref mut modal) = model.ui.active_modal {
                match modal {
                    ModalState::CommandPalette(state) => {
                        state.selected_index = state.selected_index.saturating_sub(1);
                    }
                    ModalState::ThemePicker(state) => {
                        state.selected_index = state.selected_index.saturating_sub(1);
                    }
                    _ => {}
                }
                Some(Cmd::Redraw)
            } else {
                None
            }
        }

        ModalMsg::SelectNext => {
            if let Some(ref mut modal) = model.ui.active_modal {
                match modal {
                    ModalState::CommandPalette(state) => {
                        let input_text = state.input();
                        let filtered = filter_commands(&input_text);
                        let max_index = filtered.len().saturating_sub(1);
                        state.selected_index =
                            state.selected_index.saturating_add(1).min(max_index);
                    }
                    ModalState::ThemePicker(state) => {
                        let max_index = state.themes.len().saturating_sub(1);
                        state.selected_index =
                            state.selected_index.saturating_add(1).min(max_index);
                    }
                    _ => {}
                }
                Some(Cmd::Redraw)
            } else {
                None
            }
        }

        ModalMsg::Confirm => {
            // Handle confirmation based on modal type
            // Clone the modal state to avoid borrow issues
            let modal = model.ui.active_modal.clone();
            if let Some(modal) = modal {
                match modal {
                    ModalState::CommandPalette(state) => {
                        // Get the selected command
                        let input_text = state.input();
                        let filtered = filter_commands(&input_text);
                        let selected_index =
                            state.selected_index.min(filtered.len().saturating_sub(1));

                        if let Some(cmd_def) = filtered.get(selected_index) {
                            let cmd_id = cmd_def.id;
                            // Save state for next time (only on successful execution)
                            model.ui.last_command_palette = Some(state);
                            model.ui.close_modal();
                            return execute_command(model, cmd_id);
                        }
                        model.ui.close_modal();
                        Some(Cmd::Redraw)
                    }
                    ModalState::GotoLine(state) => {
                        // Parse line:col or just line format
                        let input_text = state.input();
                        let (target_line, target_col) =
                            if let Some((line_str, col_str)) = input_text.split_once(':') {
                                let line = line_str.parse::<usize>().unwrap_or(1);
                                let col = col_str.parse::<usize>().unwrap_or(1);
                                (line, col)
                            } else {
                                let line = input_text.parse::<usize>().unwrap_or(1);
                                (line, 1)
                            };

                        // Convert to 0-indexed
                        let target_line = target_line.saturating_sub(1);
                        let target_col = target_col.saturating_sub(1);
                        let total_lines = model.document().buffer.len_lines();
                        let clamped_line = target_line.min(total_lines.saturating_sub(1));

                        // Get line length to clamp column
                        let line_len = model
                            .document()
                            .buffer
                            .line(clamped_line)
                            .len_chars()
                            .saturating_sub(1); // exclude newline
                        let clamped_col = target_col.min(line_len);

                        // Move cursor to the line:col
                        let editor = model.editor_mut();
                        editor.cursors[0].line = clamped_line;
                        editor.cursors[0].column = clamped_col;
                        editor.clear_selection();
                        model.ui.close_modal();
                        model.ensure_cursor_visible();
                        Some(Cmd::Redraw)
                    }
                    ModalState::FindReplace(_state) => {
                        // TODO: Execute find/replace (Phase 6)
                        model.ui.close_modal();
                        Some(Cmd::Redraw)
                    }
                    ModalState::ThemePicker(state) => {
                        // Apply selected theme and save config
                        if let Some(theme_info) = state.themes.get(state.selected_index) {
                            let theme_id = theme_info.id.clone();
                            if let Ok(theme) = load_theme(&theme_id) {
                                model.theme = theme;
                                // Save theme preference to config
                                if let Err(e) = model.config.set_theme(&theme_id) {
                                    tracing::warn!("Failed to save theme preference: {}", e);
                                }
                            }
                        }
                        model.ui.close_modal();
                        Some(Cmd::Redraw)
                    }
                }
            } else {
                None
            }
        }
    }
}
