//! UI message handlers (status bar, cursor blink, transient messages, modals)

use std::time::Duration;

use crate::commands::{filter_commands, Cmd};
use crate::messages::LayoutMsg;
use crate::messages::{ModalMsg, UiMsg};
use crate::model::{
    AppModel, FileFinderState, GotoLineState, ModalId, ModalState, SegmentContent, SegmentId,
    ThemePickerState, TransientMessage,
};
use crate::theme::load_theme;
use crate::update::layout::update_layout;

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
            Some(Cmd::redraw_status_bar())
        }

        UiMsg::BlinkCursor => {
            if model.ui.update_cursor_blink(Duration::from_millis(600)) {
                // Compute dirty lines for cursor blink optimization
                let current_cursor_lines = get_current_cursor_lines(model);
                let previous_cursor_lines = &model.ui.previous_cursor_lines;

                // Dirty lines = union of previous and current cursor lines
                let mut dirty_lines: Vec<usize> = current_cursor_lines.clone();
                for &line in previous_cursor_lines {
                    if !dirty_lines.contains(&line) {
                        dirty_lines.push(line);
                    }
                }

                // Update previous cursor lines for next blink
                model.ui.previous_cursor_lines = current_cursor_lines;

                // Return cursor-lines-only damage (or Full if no focused editor)
                if dirty_lines.is_empty() {
                    Some(Cmd::Redraw)
                } else {
                    Some(Cmd::redraw_cursor_lines(dirty_lines))
                }
            } else {
                None
            }
        }

        UiMsg::UpdateSegment { id, content } => {
            model.ui.status_bar.update_segment(id, content);
            Some(Cmd::redraw_status_bar())
        }

        UiMsg::SetTransientMessage { text, duration_ms } => {
            let transient = TransientMessage::new(text.clone(), Duration::from_millis(duration_ms));
            model.ui.transient_message = Some(transient);
            // Also update the StatusMessage segment
            model
                .ui
                .status_bar
                .update_segment(SegmentId::StatusMessage, SegmentContent::Text(text));
            Some(Cmd::redraw_status_bar())
        }

        UiMsg::ClearTransientMessage => {
            model.ui.transient_message = None;
            model
                .ui
                .status_bar
                .update_segment(SegmentId::StatusMessage, SegmentContent::Empty);
            Some(Cmd::redraw_status_bar())
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
                ModalId::FindReplace => {
                    let state = model.ui.last_find_replace.clone().unwrap_or_default();
                    ModalState::FindReplace(state)
                }
                ModalId::ThemePicker => ModalState::ThemePicker(ThemePickerState::default()),
                ModalId::FileFinder => {
                    // Get files from workspace (if open)
                    if let Some(ref workspace) = model.workspace {
                        let all_files = workspace.file_tree.get_all_file_paths();
                        let workspace_root = workspace.root.clone();
                        let mut state = FileFinderState::new(all_files, workspace_root);
                        // Initialize results with all files (empty query shows all)
                        update_file_finder_results(&mut state);
                        ModalState::FileFinder(state)
                    } else {
                        model.ui.set_status("No workspace open");
                        return Some(Cmd::Redraw);
                    }
                }
            };
            model.ui.open_modal(state);
            Some(Cmd::Redraw)
        }

        UiMsg::OpenFuzzyFileFinder => {
            // Check if workspace is open
            if model.workspace.is_none() {
                model
                    .ui
                    .set_status("No workspace open - use Cmd+O to open a file");
                return Some(Cmd::Redraw);
            }

            // Get files from workspace
            let (all_files, workspace_root) = if let Some(ref workspace) = model.workspace {
                (
                    workspace.file_tree.get_all_file_paths(),
                    workspace.root.clone(),
                )
            } else {
                return Some(Cmd::Redraw);
            };

            let mut state = FileFinderState::new(all_files, workspace_root);
            // Initialize results with all files (empty query shows all)
            update_file_finder_results(&mut state);
            model.ui.open_modal(ModalState::FileFinder(state));
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
            let state = model.ui.last_find_replace.clone().unwrap_or_default();
            model.ui.open_modal(ModalState::FindReplace(state));
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
                    ModalState::FileFinder(state) => {
                        state.set_input(&text);
                        update_file_finder_results(state);
                    }
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
                    ModalState::FileFinder(state) => {
                        state.editable.insert_char(ch);
                        update_file_finder_results(state);
                    }
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
                    ModalState::FileFinder(state) => {
                        state.editable.delete_backward();
                        update_file_finder_results(state);
                    }
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
                    ModalState::FileFinder(state) => {
                        state.editable.delete_word_backward();
                        update_file_finder_results(state);
                    }
                }
                Some(Cmd::Redraw)
            } else {
                None
            }
        }

        ModalMsg::MoveCursorWordLeft => {
            if let Some(ref mut modal) = model.ui.active_modal {
                match modal {
                    ModalState::CommandPalette(state) => state.editable.move_word_left(false),
                    ModalState::GotoLine(state) => state.editable.move_word_left(false),
                    ModalState::FindReplace(state) => {
                        state.focused_editable_mut().move_word_left(false)
                    }
                    ModalState::FileFinder(state) => state.editable.move_word_left(false),
                    ModalState::ThemePicker(_) => {}
                }
            }
            Some(Cmd::Redraw)
        }

        ModalMsg::MoveCursorWordRight => {
            if let Some(ref mut modal) = model.ui.active_modal {
                match modal {
                    ModalState::CommandPalette(state) => state.editable.move_word_right(false),
                    ModalState::GotoLine(state) => state.editable.move_word_right(false),
                    ModalState::FindReplace(state) => {
                        state.focused_editable_mut().move_word_right(false)
                    }
                    ModalState::FileFinder(state) => state.editable.move_word_right(false),
                    ModalState::ThemePicker(_) => {}
                }
            }
            Some(Cmd::Redraw)
        }

        ModalMsg::MoveCursorLeft => {
            if let Some(ref mut modal) = model.ui.active_modal {
                match modal {
                    ModalState::CommandPalette(state) => state.editable.move_left(false),
                    ModalState::GotoLine(state) => state.editable.move_left(false),
                    ModalState::FindReplace(state) => state.focused_editable_mut().move_left(false),
                    ModalState::FileFinder(state) => state.editable.move_left(false),
                    ModalState::ThemePicker(_) => {}
                }
            }
            Some(Cmd::Redraw)
        }

        ModalMsg::MoveCursorRight => {
            if let Some(ref mut modal) = model.ui.active_modal {
                match modal {
                    ModalState::CommandPalette(state) => state.editable.move_right(false),
                    ModalState::GotoLine(state) => state.editable.move_right(false),
                    ModalState::FindReplace(state) => {
                        state.focused_editable_mut().move_right(false)
                    }
                    ModalState::FileFinder(state) => state.editable.move_right(false),
                    ModalState::ThemePicker(_) => {}
                }
            }
            Some(Cmd::Redraw)
        }

        ModalMsg::MoveCursorHome => {
            if let Some(ref mut modal) = model.ui.active_modal {
                match modal {
                    ModalState::CommandPalette(state) => state.editable.move_line_start(false),
                    ModalState::GotoLine(state) => state.editable.move_line_start(false),
                    ModalState::FindReplace(state) => {
                        state.focused_editable_mut().move_line_start(false)
                    }
                    ModalState::FileFinder(state) => state.editable.move_line_start(false),
                    ModalState::ThemePicker(_) => {}
                }
            }
            Some(Cmd::Redraw)
        }

        ModalMsg::MoveCursorEnd => {
            if let Some(ref mut modal) = model.ui.active_modal {
                match modal {
                    ModalState::CommandPalette(state) => state.editable.move_line_end(false),
                    ModalState::GotoLine(state) => state.editable.move_line_end(false),
                    ModalState::FindReplace(state) => {
                        state.focused_editable_mut().move_line_end(false)
                    }
                    ModalState::FileFinder(state) => state.editable.move_line_end(false),
                    ModalState::ThemePicker(_) => {}
                }
            }
            Some(Cmd::Redraw)
        }

        ModalMsg::MoveCursorLeftWithSelection => {
            if let Some(ref mut modal) = model.ui.active_modal {
                match modal {
                    ModalState::CommandPalette(state) => state.editable.move_left(true),
                    ModalState::GotoLine(state) => state.editable.move_left(true),
                    ModalState::FindReplace(state) => state.focused_editable_mut().move_left(true),
                    ModalState::FileFinder(state) => state.editable.move_left(true),
                    ModalState::ThemePicker(_) => {}
                }
            }
            Some(Cmd::Redraw)
        }

        ModalMsg::MoveCursorRightWithSelection => {
            if let Some(ref mut modal) = model.ui.active_modal {
                match modal {
                    ModalState::CommandPalette(state) => state.editable.move_right(true),
                    ModalState::GotoLine(state) => state.editable.move_right(true),
                    ModalState::FindReplace(state) => state.focused_editable_mut().move_right(true),
                    ModalState::FileFinder(state) => state.editable.move_right(true),
                    ModalState::ThemePicker(_) => {}
                }
            }
            Some(Cmd::Redraw)
        }

        ModalMsg::MoveCursorHomeWithSelection => {
            if let Some(ref mut modal) = model.ui.active_modal {
                match modal {
                    ModalState::CommandPalette(state) => state.editable.move_line_start(true),
                    ModalState::GotoLine(state) => state.editable.move_line_start(true),
                    ModalState::FindReplace(state) => {
                        state.focused_editable_mut().move_line_start(true)
                    }
                    ModalState::FileFinder(state) => state.editable.move_line_start(true),
                    ModalState::ThemePicker(_) => {}
                }
            }
            Some(Cmd::Redraw)
        }

        ModalMsg::MoveCursorEndWithSelection => {
            if let Some(ref mut modal) = model.ui.active_modal {
                match modal {
                    ModalState::CommandPalette(state) => state.editable.move_line_end(true),
                    ModalState::GotoLine(state) => state.editable.move_line_end(true),
                    ModalState::FindReplace(state) => {
                        state.focused_editable_mut().move_line_end(true)
                    }
                    ModalState::FileFinder(state) => state.editable.move_line_end(true),
                    ModalState::ThemePicker(_) => {}
                }
            }
            Some(Cmd::Redraw)
        }

        ModalMsg::MoveCursorWordLeftWithSelection => {
            if let Some(ref mut modal) = model.ui.active_modal {
                match modal {
                    ModalState::CommandPalette(state) => state.editable.move_word_left(true),
                    ModalState::GotoLine(state) => state.editable.move_word_left(true),
                    ModalState::FindReplace(state) => {
                        state.focused_editable_mut().move_word_left(true)
                    }
                    ModalState::FileFinder(state) => state.editable.move_word_left(true),
                    ModalState::ThemePicker(_) => {}
                }
            }
            Some(Cmd::Redraw)
        }

        ModalMsg::MoveCursorWordRightWithSelection => {
            if let Some(ref mut modal) = model.ui.active_modal {
                match modal {
                    ModalState::CommandPalette(state) => state.editable.move_word_right(true),
                    ModalState::GotoLine(state) => state.editable.move_word_right(true),
                    ModalState::FindReplace(state) => {
                        state.focused_editable_mut().move_word_right(true)
                    }
                    ModalState::FileFinder(state) => state.editable.move_word_right(true),
                    ModalState::ThemePicker(_) => {}
                }
            }
            Some(Cmd::Redraw)
        }

        ModalMsg::SelectAll => {
            if let Some(ref mut modal) = model.ui.active_modal {
                match modal {
                    ModalState::CommandPalette(state) => state.editable.select_all(),
                    ModalState::GotoLine(state) => state.editable.select_all(),
                    ModalState::FindReplace(state) => state.focused_editable_mut().select_all(),
                    ModalState::FileFinder(state) => state.editable.select_all(),
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
                    ModalState::FileFinder(state) => state.editable.selected_text(),
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
                    ModalState::FileFinder(state) => {
                        let t = state.editable.selected_text();
                        if !t.is_empty() {
                            state.editable.delete_backward();
                            update_file_finder_results(state);
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
                            ModalState::FileFinder(state) => {
                                state.editable.insert_text(&filtered);
                                update_file_finder_results(state);
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
                    ModalState::FileFinder(state) => {
                        state.editable.delete_forward();
                        update_file_finder_results(state);
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
                    ModalState::FileFinder(state) => {
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
                    ModalState::FileFinder(state) => {
                        let max_index = state.results.len().saturating_sub(1);
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
                    ModalState::FindReplace(state) => {
                        // For Confirm, treat it as FindNext
                        let query = state.query();
                        if !query.is_empty() {
                            let case_sensitive = state.case_sensitive;
                            model.ui.last_find_replace = Some(state);
                            return find_next_in_document(model, &query, case_sensitive);
                        }
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
                    ModalState::FileFinder(state) => {
                        // Open selected file
                        if let Some(file_match) = state.results.get(state.selected_index) {
                            let path = file_match.path.clone();
                            model.ui.close_modal();
                            return update_layout(model, LayoutMsg::OpenFileInNewTab(path));
                        }
                        model.ui.close_modal();
                        Some(Cmd::Redraw)
                    }
                }
            } else {
                None
            }
        }

        ModalMsg::ToggleFindReplaceField => {
            if let Some(ModalState::FindReplace(ref mut state)) = model.ui.active_modal {
                state.toggle_field();
                Some(Cmd::Redraw)
            } else {
                None
            }
        }

        ModalMsg::ToggleFindReplaceCaseSensitive => {
            if let Some(ModalState::FindReplace(ref mut state)) = model.ui.active_modal {
                state.case_sensitive = !state.case_sensitive;
                Some(Cmd::Redraw)
            } else {
                None
            }
        }

        ModalMsg::FindNext => {
            if let Some(ModalState::FindReplace(ref state)) = model.ui.active_modal {
                let query = state.query();
                let case_sensitive = state.case_sensitive;
                if !query.is_empty() {
                    model.ui.last_find_replace = model.ui.active_modal.clone().and_then(|m| {
                        if let ModalState::FindReplace(s) = m {
                            Some(s)
                        } else {
                            None
                        }
                    });
                    return find_next_in_document(model, &query, case_sensitive);
                }
            }
            Some(Cmd::Redraw)
        }

        ModalMsg::FindPrevious => {
            if let Some(ModalState::FindReplace(ref state)) = model.ui.active_modal {
                let query = state.query();
                let case_sensitive = state.case_sensitive;
                if !query.is_empty() {
                    model.ui.last_find_replace = model.ui.active_modal.clone().and_then(|m| {
                        if let ModalState::FindReplace(s) = m {
                            Some(s)
                        } else {
                            None
                        }
                    });
                    return find_prev_in_document(model, &query, case_sensitive);
                }
            }
            Some(Cmd::Redraw)
        }

        ModalMsg::ReplaceAndFindNext => {
            if let Some(ModalState::FindReplace(ref state)) = model.ui.active_modal {
                let query = state.query();
                let replacement = state.replacement();
                let case_sensitive = state.case_sensitive;
                if !query.is_empty() {
                    model.ui.last_find_replace = model.ui.active_modal.clone().and_then(|m| {
                        if let ModalState::FindReplace(s) = m {
                            Some(s)
                        } else {
                            None
                        }
                    });
                    return replace_and_find_next(model, &query, &replacement, case_sensitive);
                }
            }
            Some(Cmd::Redraw)
        }

        ModalMsg::ReplaceAll => {
            if let Some(ModalState::FindReplace(ref state)) = model.ui.active_modal {
                let query = state.query();
                let replacement = state.replacement();
                let case_sensitive = state.case_sensitive;
                if !query.is_empty() {
                    model.ui.last_find_replace = model.ui.active_modal.clone().and_then(|m| {
                        if let ModalState::FindReplace(s) = m {
                            Some(s)
                        } else {
                            None
                        }
                    });
                    return replace_all(model, &query, &replacement, case_sensitive);
                }
            }
            Some(Cmd::Redraw)
        }
    }
}

/// Find next occurrence in the document and select it
fn find_next_in_document(model: &mut AppModel, query: &str, case_sensitive: bool) -> Option<Cmd> {
    let editor = model.editor();
    let doc = model.document();

    // Get current cursor position as the search start point
    let start_offset = if !editor.selections[0].is_empty() {
        // If there's a selection, search from after the selection end
        let sel_end = editor.selections[0].end();
        doc.cursor_to_offset(sel_end.line, sel_end.column)
    } else {
        doc.cursor_to_offset(editor.cursors[0].line, editor.cursors[0].column)
    };

    if let Some((start, end)) =
        doc.find_next_occurrence_with_options(query, start_offset, case_sensitive)
    {
        let (start_line, start_col) = doc.offset_to_cursor(start);
        let (end_line, end_col) = doc.offset_to_cursor(end);

        let editor = model.editor_mut();
        // Set cursor to end of match
        editor.cursors[0].line = end_line;
        editor.cursors[0].column = end_col;
        editor.cursors[0].desired_column = None;

        // Set selection to cover the match
        editor.selections[0] = crate::model::Selection::from_anchor_head(
            crate::model::Position::new(start_line, start_col),
            crate::model::Position::new(end_line, end_col),
        );

        model.ensure_cursor_visible();
        Some(Cmd::Redraw)
    } else {
        // No match found - show transient message
        model.ui.transient_message = Some(TransientMessage::new(
            "No matches found".to_string(),
            Duration::from_secs(2),
        ));
        Some(Cmd::Redraw)
    }
}

/// Find previous occurrence in the document and select it
fn find_prev_in_document(model: &mut AppModel, query: &str, case_sensitive: bool) -> Option<Cmd> {
    let editor = model.editor();
    let doc = model.document();

    // Get current cursor position as the search start point
    let start_offset = if !editor.selections[0].is_empty() {
        // If there's a selection, search from before the selection start
        let sel_start = editor.selections[0].start();
        doc.cursor_to_offset(sel_start.line, sel_start.column)
    } else {
        doc.cursor_to_offset(editor.cursors[0].line, editor.cursors[0].column)
    };

    if let Some((start, end)) =
        doc.find_prev_occurrence_with_options(query, start_offset, case_sensitive)
    {
        let (start_line, start_col) = doc.offset_to_cursor(start);
        let (end_line, end_col) = doc.offset_to_cursor(end);

        let editor = model.editor_mut();
        // Set cursor to start of match (for prev, cursor goes to start)
        editor.cursors[0].line = start_line;
        editor.cursors[0].column = start_col;
        editor.cursors[0].desired_column = None;

        // Set selection to cover the match
        editor.selections[0] = crate::model::Selection::from_anchor_head(
            crate::model::Position::new(start_line, start_col),
            crate::model::Position::new(end_line, end_col),
        );

        model.ensure_cursor_visible();
        Some(Cmd::Redraw)
    } else {
        model.ui.transient_message = Some(TransientMessage::new(
            "No matches found".to_string(),
            Duration::from_secs(2),
        ));
        Some(Cmd::Redraw)
    }
}

/// Replace current selection if it matches, then find next
fn replace_and_find_next(
    model: &mut AppModel,
    query: &str,
    replacement: &str,
    case_sensitive: bool,
) -> Option<Cmd> {
    // First, gather all the info we need without holding borrows
    let should_replace = {
        let editor = model.editor();
        let doc = model.document();

        if editor.selections[0].is_empty() {
            None
        } else {
            let sel = &editor.selections[0];
            let start = sel.start();
            let end = sel.end();
            let start_offset = doc.cursor_to_offset(start.line, start.column);
            let end_offset = doc.cursor_to_offset(end.line, end.column);

            let selected_text = doc.buffer.slice(start_offset..end_offset).to_string();
            let matches = if case_sensitive {
                selected_text == query
            } else {
                selected_text.to_lowercase() == query.to_lowercase()
            };

            if matches {
                Some((start_offset, end_offset))
            } else {
                None
            }
        }
    };

    // Now do the replacement if needed
    if let Some((start_offset, end_offset)) = should_replace {
        let doc = model.document_mut();
        doc.buffer.remove(start_offset..end_offset);
        doc.buffer.insert(start_offset, replacement);
        doc.is_modified = true;
        doc.revision += 1;

        // Update cursor position
        let new_offset = start_offset + replacement.chars().count();
        let (new_line, new_col) = doc.offset_to_cursor(new_offset);

        let editor = model.editor_mut();
        editor.cursors[0].line = new_line;
        editor.cursors[0].column = new_col;
        editor.clear_selection();
    }

    // Now find next
    find_next_in_document(model, query, case_sensitive)
}

/// Replace all occurrences
fn replace_all(
    model: &mut AppModel,
    query: &str,
    replacement: &str,
    case_sensitive: bool,
) -> Option<Cmd> {
    let doc = model.document();
    let occurrences = doc.find_all_occurrences_with_options(query, case_sensitive);

    if occurrences.is_empty() {
        model.ui.transient_message = Some(TransientMessage::new(
            "No matches found".to_string(),
            Duration::from_secs(2),
        ));
        return Some(Cmd::Redraw);
    }

    let count = occurrences.len();

    // Replace from end to start to preserve offsets
    let doc = model.document_mut();
    let replacement_char_len = replacement.chars().count();
    for (start, end) in occurrences.into_iter().rev() {
        doc.buffer.remove(start..end);
        doc.buffer.insert(start, replacement);
    }
    doc.is_modified = true;
    doc.revision += 1;

    // Position cursor at end of last replacement (which is now first in document)
    let editor = model.editor_mut();
    editor.cursors[0].line = 0;
    editor.cursors[0].column = replacement_char_len;
    editor.clear_selection();

    model.ui.transient_message = Some(TransientMessage::new(
        format!("Replaced {} occurrences", count),
        Duration::from_secs(2),
    ));
    Some(Cmd::Redraw)
}

/// Get the line numbers of all cursors in the focused editor
/// Returns empty vec if no focused editor exists
fn get_current_cursor_lines(model: &AppModel) -> Vec<usize> {
    // Get the focused editor's cursors
    if let Some(editor) = model.focused_editor() {
        editor.cursors.iter().map(|c| c.line).collect()
    } else {
        Vec::new()
    }
}

// ============================================================================
// Fuzzy File Finder
// ============================================================================

use crate::model::FileMatch;
use nucleo_matcher::{Config, Matcher, Utf32Str};
use std::path::Path;

/// Update file finder results based on current query
pub fn update_file_finder_results(state: &mut FileFinderState) {
    let query = state.input();
    state.results = fuzzy_match_files(&state.all_files, &query, &state.workspace_root);
    // Reset selection to first item
    state.selected_index = 0;
}

/// Perform fuzzy matching on file paths
fn fuzzy_match_files(
    files: &[std::path::PathBuf],
    query: &str,
    workspace_root: &Path,
) -> Vec<FileMatch> {
    if query.is_empty() {
        // Show all files sorted alphabetically when no query (limit to first 100)
        return files
            .iter()
            .take(100)
            .map(|p| FileMatch::from_path(p, workspace_root, 0, vec![]))
            .collect();
    }

    let mut matcher = Matcher::new(Config::DEFAULT);
    let mut query_buf = Vec::new();
    let needle = Utf32Str::new(query, &mut query_buf);

    let mut results: Vec<FileMatch> = files
        .iter()
        .filter_map(|path| {
            let filename = path.file_name()?.to_str()?;
            let mut filename_buf = Vec::new();
            let haystack = Utf32Str::new(filename, &mut filename_buf);

            // Get fuzzy match score
            let score = matcher.fuzzy_match(haystack, needle)?;

            // Get match indices for highlighting
            let mut indices = vec![];
            matcher.fuzzy_indices(haystack, needle, &mut indices);
            let indices = indices.to_vec();

            Some(FileMatch::from_path(
                path,
                workspace_root,
                score as u32,
                indices,
            ))
        })
        .collect();

    // Sort by score descending
    results.sort_by(|a, b| b.score.cmp(&a.score));

    // Limit results
    results.truncate(50);
    results
}
