//! UI message handlers (status bar, cursor blink, transient messages, modals)

use std::time::Duration;

use crate::commands::Cmd;
use crate::editable::{EditableState, StringBuffer};
use crate::messages::LayoutMsg;
use crate::messages::{ModalMsg, UiMsg};
use crate::model::{
    AppModel, CommandPaletteState, FileFinderState, GotoLineState, ModalId, ModalState,
    RecentFilesState, SegmentContent, SegmentId, ThemePickerState, TransientMessage,
    COMMAND_PALETTE_MAX_VISIBLE,
};
use crate::theme::load_theme;
use crate::update::layout::update_layout;
use crate::view::modal::{recent_files_groups, theme_picker_groups};
use crate::view::overlay_surface::{resolve_scroll_for_selection, SectionShape};

use super::app::execute_command;

/// Handle UI messages (status bar, cursor blink, modals)
pub fn update_ui(model: &mut AppModel, msg: UiMsg) -> Option<Cmd> {
    match msg {
        UiMsg::BlinkCursor => {
            if model
                .ui
                .update_cursor_blink(Duration::from_millis(model.config.cursor_blink_ms))
            {
                // Compute dirty lines for cursor blink optimization
                let current_cursor_lines = get_current_cursor_lines(model);
                let previous_cursor_lines = &model.ui.previous_cursor_lines;

                // Dirty lines = union of previous and current cursor lines
                let mut dirty_lines_set: std::collections::HashSet<usize> =
                    current_cursor_lines.iter().copied().collect();
                dirty_lines_set.extend(previous_cursor_lines.iter().copied());
                let dirty_lines: Vec<usize> = dirty_lines_set.into_iter().collect();

                // Update previous cursor lines for next blink
                model.ui.previous_cursor_lines = current_cursor_lines;

                // Return cursor-lines-only damage (or None if no focused editor)
                if dirty_lines.is_empty() {
                    None
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
                    let mut state = model.ui.last_command_palette.clone().unwrap_or_default();
                    resolve_palette_rows(&mut state);
                    ModalState::CommandPalette(state)
                }
                ModalId::GotoLine => ModalState::GotoLine(GotoLineState::default()),
                ModalId::FindReplace => {
                    let state = model.ui.last_find_replace.clone().unwrap_or_default();
                    ModalState::FindReplace(state)
                }
                ModalId::ThemePicker => {
                    ModalState::ThemePicker(ThemePickerState::new(model.config.theme.clone()))
                }
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
                ModalId::RecentFiles => {
                    let current_file = model
                        .editor_area
                        .focused_document()
                        .and_then(|doc| doc.file_path.clone());
                    ModalState::RecentFiles(RecentFilesState::new(
                        &model.recent_files,
                        current_file.as_deref(),
                    ))
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

        // === Scrollbar interaction ===
        UiMsg::ScrollbarTrackClickedVertical {
            editor_id,
            new_position,
        } => model
            .set_editor_vertical_scroll(editor_id, new_position)
            .then_some(Cmd::redraw_editor()),

        UiMsg::ScrollbarTrackClickedHorizontal {
            editor_id,
            new_position,
        } => model
            .set_editor_horizontal_scroll(editor_id, new_position)
            .then_some(Cmd::redraw_editor()),

        UiMsg::ScrollbarThumbPressedVertical {
            editor_id,
            grab_offset,
            track_start,
            track_size,
            thumb_size,
            max_scroll,
        } => {
            model.ui.scrollbar_drag = Some(crate::model::ui::ScrollbarDragState {
                editor_id,
                axis: crate::model::ui::ScrollbarDragAxis::Vertical,
                grab_offset,
                track_start,
                track_size,
                thumb_size,
                max_scroll,
            });
            None
        }

        UiMsg::ScrollbarThumbPressedHorizontal {
            editor_id,
            grab_offset,
            track_start,
            track_size,
            thumb_size,
            max_scroll,
        } => {
            model.ui.scrollbar_drag = Some(crate::model::ui::ScrollbarDragState {
                editor_id,
                axis: crate::model::ui::ScrollbarDragAxis::Horizontal,
                grab_offset,
                track_start,
                track_size,
                thumb_size,
                max_scroll,
            });
            None
        }

        UiMsg::ScrollbarDragUpdate { mouse_coord } => {
            let Some(drag) = &model.ui.scrollbar_drag else {
                return None;
            };
            let new_pos = drag.position_from_mouse(mouse_coord);
            let editor_id = drag.editor_id;
            let axis = drag.axis;
            let changed = match axis {
                crate::model::ui::ScrollbarDragAxis::Vertical => {
                    model.set_editor_vertical_scroll(editor_id, new_pos.min(drag.max_scroll))
                }
                crate::model::ui::ScrollbarDragAxis::Horizontal => {
                    model.set_editor_horizontal_scroll(editor_id, new_pos.min(drag.max_scroll))
                }
            };
            if changed {
                Some(Cmd::redraw_editor())
            } else {
                None
            }
        }

        UiMsg::ScrollbarDragEnd => {
            model.ui.scrollbar_drag = None;
            None
        }
    }
}

/// Get mutable access to the "active" editable text field for a modal state,
/// if it has one. This is the single field that plain text-editing
/// `ModalMsg` variants (insert/delete/move/select/copy/cut) operate on.
///
/// `FindReplace` has two editable fields (query + replacement); it exposes
/// whichever one currently has focus via `focused_editable_mut()`.
/// `ThemePicker` has no text input at all and returns `None`.
fn modal_editable_mut(modal: &mut ModalState) -> Option<&mut EditableState<StringBuffer>> {
    match modal {
        ModalState::CommandPalette(state) => Some(&mut state.editable),
        ModalState::GotoLine(state) => Some(&mut state.editable),
        ModalState::FindReplace(state) => Some(state.focused_editable_mut()),
        ModalState::ThemePicker(_) => None,
        ModalState::FileFinder(state) => Some(&mut state.editable),
        ModalState::RecentFiles(state) => Some(&mut state.editable),
    }
}

/// Run the modal-specific side effect that should happen whenever a modal's
/// text input changes (insert/delete/cut/paste). `CommandPalette` and
/// `RecentFiles` reset their selected index back to the top of the list;
/// `FileFinder` refreshes its fuzzy-matched results. Other modal types have
/// no such side effect.
fn on_modal_input_changed(modal: &mut ModalState) {
    match modal {
        ModalState::CommandPalette(state) => resolve_palette_rows(state),
        ModalState::FileFinder(state) => update_file_finder_results(state),
        ModalState::RecentFiles(state) => resolve_recent_rows(state),
        ModalState::GotoLine(_) | ModalState::FindReplace(_) | ModalState::ThemePicker(_) => {}
    }
}

/// Handle modal-specific messages
fn update_modal(model: &mut AppModel, msg: ModalMsg) -> Option<Cmd> {
    match msg {
        ModalMsg::OpenCommandPalette => {
            let mut state = model.ui.last_command_palette.clone().unwrap_or_default();
            resolve_palette_rows(&mut state);
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
            // Restore original theme if closing theme picker without confirming
            if let Some(ModalState::ThemePicker(state)) = &model.ui.active_modal {
                if let Ok(theme) = load_theme(&state.original_theme_id) {
                    model.theme = theme;
                }
            }
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
                    ModalState::FileFinder(state) => state.set_input(&text),
                    ModalState::RecentFiles(state) => state.editable.set_content(&text),
                }
                on_modal_input_changed(modal);
                Some(Cmd::Redraw)
            } else {
                None
            }
        }

        ModalMsg::InsertChar(ch) => {
            if let Some(ref mut modal) = model.ui.active_modal {
                if let Some(editable) = modal_editable_mut(modal) {
                    editable.insert_char(ch);
                }
                on_modal_input_changed(modal);
                Some(Cmd::Redraw)
            } else {
                None
            }
        }

        ModalMsg::DeleteBackward => {
            if let Some(ref mut modal) = model.ui.active_modal {
                if let Some(editable) = modal_editable_mut(modal) {
                    editable.delete_backward();
                }
                on_modal_input_changed(modal);
                Some(Cmd::Redraw)
            } else {
                None
            }
        }

        ModalMsg::DeleteWordBackward => {
            if let Some(ref mut modal) = model.ui.active_modal {
                if let Some(editable) = modal_editable_mut(modal) {
                    editable.delete_word_backward();
                }
                on_modal_input_changed(modal);
                Some(Cmd::Redraw)
            } else {
                None
            }
        }

        ModalMsg::MoveCursorWordLeft => {
            if let Some(ref mut modal) = model.ui.active_modal {
                if let Some(editable) = modal_editable_mut(modal) {
                    editable.move_word_left(false);
                }
            }
            Some(Cmd::Redraw)
        }

        ModalMsg::MoveCursorWordRight => {
            if let Some(ref mut modal) = model.ui.active_modal {
                if let Some(editable) = modal_editable_mut(modal) {
                    editable.move_word_right(false);
                }
            }
            Some(Cmd::Redraw)
        }

        ModalMsg::MoveCursorLeft => {
            if let Some(ref mut modal) = model.ui.active_modal {
                if let Some(editable) = modal_editable_mut(modal) {
                    editable.move_left(false);
                }
            }
            Some(Cmd::Redraw)
        }

        ModalMsg::MoveCursorRight => {
            if let Some(ref mut modal) = model.ui.active_modal {
                if let Some(editable) = modal_editable_mut(modal) {
                    editable.move_right(false);
                }
            }
            Some(Cmd::Redraw)
        }

        ModalMsg::MoveCursorHome => {
            if let Some(ref mut modal) = model.ui.active_modal {
                if let Some(editable) = modal_editable_mut(modal) {
                    editable.move_line_start(false);
                }
            }
            Some(Cmd::Redraw)
        }

        ModalMsg::MoveCursorEnd => {
            if let Some(ref mut modal) = model.ui.active_modal {
                if let Some(editable) = modal_editable_mut(modal) {
                    editable.move_line_end(false);
                }
            }
            Some(Cmd::Redraw)
        }

        ModalMsg::MoveCursorLeftWithSelection => {
            if let Some(ref mut modal) = model.ui.active_modal {
                if let Some(editable) = modal_editable_mut(modal) {
                    editable.move_left(true);
                }
            }
            Some(Cmd::Redraw)
        }

        ModalMsg::MoveCursorRightWithSelection => {
            if let Some(ref mut modal) = model.ui.active_modal {
                if let Some(editable) = modal_editable_mut(modal) {
                    editable.move_right(true);
                }
            }
            Some(Cmd::Redraw)
        }

        ModalMsg::MoveCursorHomeWithSelection => {
            if let Some(ref mut modal) = model.ui.active_modal {
                if let Some(editable) = modal_editable_mut(modal) {
                    editable.move_line_start(true);
                }
            }
            Some(Cmd::Redraw)
        }

        ModalMsg::MoveCursorEndWithSelection => {
            if let Some(ref mut modal) = model.ui.active_modal {
                if let Some(editable) = modal_editable_mut(modal) {
                    editable.move_line_end(true);
                }
            }
            Some(Cmd::Redraw)
        }

        ModalMsg::MoveCursorWordLeftWithSelection => {
            if let Some(ref mut modal) = model.ui.active_modal {
                if let Some(editable) = modal_editable_mut(modal) {
                    editable.move_word_left(true);
                }
            }
            Some(Cmd::Redraw)
        }

        ModalMsg::MoveCursorWordRightWithSelection => {
            if let Some(ref mut modal) = model.ui.active_modal {
                if let Some(editable) = modal_editable_mut(modal) {
                    editable.move_word_right(true);
                }
            }
            Some(Cmd::Redraw)
        }

        ModalMsg::SelectAll => {
            if let Some(ref mut modal) = model.ui.active_modal {
                if let Some(editable) = modal_editable_mut(modal) {
                    editable.select_all();
                }
            }
            Some(Cmd::Redraw)
        }

        ModalMsg::Copy => {
            let mut cmd = Cmd::Redraw;
            if let Some(ref mut modal) = model.ui.active_modal {
                let text = modal_editable_mut(modal)
                    .map(|editable| editable.selected_text())
                    .unwrap_or_default();
                if !text.is_empty() {
                    cmd = Cmd::Batch(vec![cmd, Cmd::CopyToClipboard(text)]);
                }
            }
            Some(cmd)
        }

        ModalMsg::Cut => {
            let mut cmd = Cmd::Redraw;
            if let Some(ref mut modal) = model.ui.active_modal {
                let text = modal_editable_mut(modal)
                    .map(|editable| editable.selected_text())
                    .unwrap_or_default();
                if !text.is_empty() {
                    if let Some(editable) = modal_editable_mut(modal) {
                        editable.delete_backward();
                    }
                    on_modal_input_changed(modal);
                    cmd = Cmd::Batch(vec![cmd, Cmd::CopyToClipboard(text)]);
                }
            }
            Some(cmd)
        }

        ModalMsg::Paste => Some(Cmd::RequestClipboardPaste),

        ModalMsg::PasteText(text) => {
            // Filter out newlines for single-line modal inputs
            let filtered: String = text.chars().filter(|c| *c != '\n' && *c != '\r').collect();
            if !filtered.is_empty() {
                if let Some(ref mut modal) = model.ui.active_modal {
                    if let ModalState::GotoLine(state) = modal {
                        // Filter to only digits for goto line
                        let digits: String =
                            filtered.chars().filter(|c| c.is_ascii_digit()).collect();
                        state.editable.insert_text(&digits);
                    } else {
                        if let Some(editable) = modal_editable_mut(modal) {
                            editable.insert_text(&filtered);
                        }
                        on_modal_input_changed(modal);
                    }
                }
            }
            Some(Cmd::Redraw)
        }

        ModalMsg::DeleteForward => {
            if let Some(ref mut modal) = model.ui.active_modal {
                if let Some(editable) = modal_editable_mut(modal) {
                    editable.delete_forward();
                }
                on_modal_input_changed(modal);
                Some(Cmd::Redraw)
            } else {
                None
            }
        }

        ModalMsg::SelectPrevious => modal_select(model, -1),

        ModalMsg::SelectNext => modal_select(model, 1),

        ModalMsg::PageUp => modal_page(model, false),

        ModalMsg::PageDown => modal_page(model, true),

        ModalMsg::Scroll(delta) => modal_scroll(model, delta),

        ModalMsg::ActivateRow(row) => {
            if let Some(ref mut modal) = model.ui.active_modal {
                set_modal_selected_index(modal, row);
            }
            confirm_active_modal(model)
        }

        ModalMsg::TogglePin => {
            if let Some(ModalState::RecentFiles(ref mut state)) = model.ui.active_modal {
                if let Some(path) = state.selected_entry().map(|e| e.path.clone()) {
                    model.recent_files.toggle_pin(&path);
                    if let Some(e) = state.entries.iter_mut().find(|e| e.path == path) {
                        e.pinned = !e.pinned;
                    }
                    state.recompute_filtered_rows();
                    // Keep the same entry selected even though the
                    // Pinned/date grouping just reordered it.
                    if let Some(new_idx) = state
                        .filtered_rows
                        .iter()
                        .position(|&i| state.entries[i].path == path)
                    {
                        state.selected_index = new_idx;
                        state.scroll_offset = resolve_scroll_for_selection(
                            &recent_files_shapes(state),
                            new_idx,
                            COMMAND_PALETTE_MAX_VISIBLE,
                            state.scroll_offset,
                        );
                    }
                    let recent = model.recent_files.clone();
                    return Some(Cmd::Batch(vec![
                        Cmd::Redraw,
                        Cmd::SaveRecentFiles { recent },
                    ]));
                }
            }
            Some(Cmd::Redraw)
        }

        ModalMsg::Confirm => confirm_active_modal(model),

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

/// Set the `FlatIndex`-space selected row for whichever list-body modal is
/// active — used by `ModalMsg::ActivateRow` (row click) ahead of confirming.
/// A no-op for `Fields`/no-list contexts.
fn set_modal_selected_index(modal: &mut ModalState, row: usize) {
    match modal {
        ModalState::CommandPalette(state) => state.selected_index = row.min(state.matches.len()),
        ModalState::ThemePicker(state) => state.selected_index = row.min(state.themes.len()),
        ModalState::FileFinder(state) => state.selected_index = row.min(state.results.len()),
        ModalState::RecentFiles(state) => state.selected_index = row.min(state.filtered_rows.len()),
        ModalState::GotoLine(_) | ModalState::FindReplace(_) => {}
    }
}

/// Confirm/execute the modal action (Enter, or a row click via
/// `ActivateRow`). The single place every list-body context reads its
/// selection from the same ordering-authority cache the view rendered —
/// see overlay-surface.md "Ordering authority".
fn confirm_active_modal(model: &mut AppModel) -> Option<Cmd> {
    // Clone the modal state to avoid borrow issues
    let modal = model.ui.active_modal.clone();
    if let Some(modal) = modal {
        match modal {
            ModalState::CommandPalette(state) => {
                // Read the selected command from the same `matches` cache
                // the view rendered — the ordering-authority hazard this
                // closes: Confirm used to re-derive the filtered list
                // independently via `filter_commands`.
                let selected_index = state.selected_index.min(state.matches.len());
                if let Some(cmd_match) = state.matches.get(selected_index) {
                    let cmd_id = cmd_match.def.id;
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
            ModalState::RecentFiles(state) => {
                if let Some(entry) = state.selected_entry() {
                    let path = entry.path.clone();
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

/// Section shapes for a flat (untitled, single-section) list body — the
/// Command Palette and File Finder, which have no headers.
fn flat_shapes(total: usize) -> [SectionShape; 1] {
    [SectionShape {
        has_title: false,
        len: total,
    }]
}

/// Section shapes for the Recent Files modal's Pinned/Today/Yesterday/
/// Earlier grouping — the same boundaries `recent_files_groups` renders,
/// so selection movement and the view agree on where headers fall.
fn recent_files_shapes(state: &RecentFilesState) -> Vec<SectionShape> {
    recent_files_groups(state)
        .iter()
        .map(|(_, indices)| SectionShape {
            has_title: true,
            len: indices.len(),
        })
        .collect()
}

/// Section shapes for the Theme Picker's User/Built-in grouping.
fn theme_picker_shapes(state: &ThemePickerState) -> Vec<SectionShape> {
    theme_picker_groups(&state.themes)
        .iter()
        .map(|(_, range)| SectionShape {
            has_title: true,
            len: range.len(),
        })
        .collect()
}

/// Move `*selected` by `delta`, wrapping at both ends, keeping `*scroll`
/// following it (minimal-reveal scrolling, header-aware) — shared by
/// every list-body modal context (overlay-surface.md Behaviour: "Up/Down
/// skip headers and wrap at the ends").
fn move_list_selection(
    selected: &mut usize,
    scroll: &mut usize,
    shapes: &[SectionShape],
    delta: isize,
) {
    let total: usize = shapes.iter().map(|s| s.len).sum();
    if total == 0 {
        return;
    }
    let current = *selected as isize;
    *selected = (current + delta).rem_euclid(total as isize) as usize;
    *scroll = resolve_scroll_for_selection(shapes, *selected, COMMAND_PALETTE_MAX_VISIBLE, *scroll);
}

/// Page `*selected` by a full visible page, clamping (not wrapping —
/// PageUp/PageDown are jumps, not cyclic navigation).
fn page_list_selection(
    selected: &mut usize,
    scroll: &mut usize,
    shapes: &[SectionShape],
    forward: bool,
) {
    let total: usize = shapes.iter().map(|s| s.len).sum();
    if total == 0 {
        return;
    }
    let max_index = total - 1;
    *selected = if forward {
        (*selected + COMMAND_PALETTE_MAX_VISIBLE).min(max_index)
    } else {
        selected.saturating_sub(COMMAND_PALETTE_MAX_VISIBLE)
    };
    *scroll = resolve_scroll_for_selection(shapes, *selected, COMMAND_PALETTE_MAX_VISIBLE, *scroll);
}

/// `ModalMsg::SelectPrevious`/`SelectNext`: move selection by `delta`
/// (-1/+1) in whichever list-body modal is active. Theme Picker previews
/// the newly-selected theme live.
fn modal_select(model: &mut AppModel, delta: isize) -> Option<Cmd> {
    let modal = model.ui.active_modal.as_mut()?;
    let preview_theme_id = match modal {
        ModalState::CommandPalette(state) => {
            let shapes = flat_shapes(state.matches.len());
            move_list_selection(
                &mut state.selected_index,
                &mut state.scroll_offset,
                &shapes,
                delta,
            );
            None
        }
        ModalState::ThemePicker(state) => {
            let shapes = theme_picker_shapes(state);
            move_list_selection(
                &mut state.selected_index,
                &mut state.scroll_offset,
                &shapes,
                delta,
            );
            state.themes.get(state.selected_index).map(|t| t.id.clone())
        }
        ModalState::FileFinder(state) => {
            let shapes = flat_shapes(state.results.len());
            move_list_selection(
                &mut state.selected_index,
                &mut state.scroll_offset,
                &shapes,
                delta,
            );
            None
        }
        ModalState::RecentFiles(state) => {
            let shapes = recent_files_shapes(state);
            move_list_selection(
                &mut state.selected_index,
                &mut state.scroll_offset,
                &shapes,
                delta,
            );
            None
        }
        ModalState::GotoLine(_) | ModalState::FindReplace(_) => None,
    };
    if let Some(theme_id) = preview_theme_id {
        if let Ok(theme) = load_theme(&theme_id) {
            model.theme = theme;
        }
    }
    Some(Cmd::Redraw)
}

/// `ModalMsg::PageUp`/`PageDown`: page selection by a full visible page in
/// whichever list-body modal is active.
fn modal_page(model: &mut AppModel, forward: bool) -> Option<Cmd> {
    let modal = model.ui.active_modal.as_mut()?;
    match modal {
        ModalState::CommandPalette(state) => {
            let shapes = flat_shapes(state.matches.len());
            page_list_selection(
                &mut state.selected_index,
                &mut state.scroll_offset,
                &shapes,
                forward,
            );
        }
        ModalState::ThemePicker(state) => {
            let shapes = theme_picker_shapes(state);
            page_list_selection(
                &mut state.selected_index,
                &mut state.scroll_offset,
                &shapes,
                forward,
            );
        }
        ModalState::FileFinder(state) => {
            let shapes = flat_shapes(state.results.len());
            page_list_selection(
                &mut state.selected_index,
                &mut state.scroll_offset,
                &shapes,
                forward,
            );
        }
        ModalState::RecentFiles(state) => {
            let shapes = recent_files_shapes(state);
            page_list_selection(
                &mut state.selected_index,
                &mut state.scroll_offset,
                &shapes,
                forward,
            );
        }
        ModalState::GotoLine(_) | ModalState::FindReplace(_) => {}
    }
    Some(Cmd::Redraw)
}

/// `ModalMsg::Scroll`: move the visible window by `delta` rows without
/// moving selection (mouse wheel over a list-body modal).
fn modal_scroll(model: &mut AppModel, delta: isize) -> Option<Cmd> {
    let modal = model.ui.active_modal.as_mut()?;
    let (scroll, shapes): (&mut usize, Vec<SectionShape>) = match modal {
        ModalState::CommandPalette(state) => (
            &mut state.scroll_offset,
            flat_shapes(state.matches.len()).to_vec(),
        ),
        ModalState::ThemePicker(state) => {
            let shapes = theme_picker_shapes(state);
            (&mut state.scroll_offset, shapes)
        }
        ModalState::FileFinder(state) => (
            &mut state.scroll_offset,
            flat_shapes(state.results.len()).to_vec(),
        ),
        ModalState::RecentFiles(state) => {
            let shapes = recent_files_shapes(state);
            (&mut state.scroll_offset, shapes)
        }
        ModalState::GotoLine(_) | ModalState::FindReplace(_) => return None,
    };
    let total: usize = shapes.iter().map(|s| s.len).sum();
    if total == 0 {
        return None;
    }
    let max_scroll =
        resolve_scroll_for_selection(&shapes, total - 1, COMMAND_PALETTE_MAX_VISIBLE, 0) as isize;
    let new_scroll = (*scroll as isize + delta).clamp(0, max_scroll.max(0)) as usize;
    if new_scroll == *scroll {
        return None;
    }
    *scroll = new_scroll;
    Some(Cmd::Redraw)
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
        Some(Cmd::redraw_editor())
    } else {
        // No match found - show transient message
        model.ui.transient_message = Some(TransientMessage::new(
            "No matches found".to_string(),
            Duration::from_secs(2),
        ));
        Some(Cmd::redraw_editor())
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
        Some(Cmd::redraw_editor())
    } else {
        model.ui.transient_message = Some(TransientMessage::new(
            "No matches found".to_string(),
            Duration::from_secs(2),
        ));
        Some(Cmd::redraw_editor())
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
    Some(Cmd::redraw_editor())
}

/// Get the line numbers of all cursors in the focused editor
/// Returns empty vec if no focused editor exists
fn get_current_cursor_lines(model: &AppModel) -> Vec<usize> {
    // Get the focused editor's cursors
    if let Some(editor) = model.focused_editor() {
        if editor.is_plain_text_mode() {
            editor.cursors.iter().map(|c| c.line).collect()
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    }
}

// ============================================================================
// Command Palette Ordering Authority
// ============================================================================

use crate::commands::CommandDef;
use crate::model::CommandMatch;

/// The ordering authority for the command palette (overlay-surface.md
/// "Ordering authority"): the *only* place that filters/ranks commands.
/// Both the palette's spec builder and `ModalMsg::Confirm`/`SelectNext`
/// consume this cache instead of re-deriving the list, so Enter always
/// activates the row the user actually sees selected.
pub fn resolve_palette_rows(state: &mut CommandPaletteState) {
    state.matches = fuzzy_match_commands(&state.input());
    state.selected_index = 0;
    state.scroll_offset = 0;
}

/// Fuzzy-match commands against `query` using nucleo, the same pattern the
/// file finder uses below (`fuzzy_match_files`) — replaces the old bespoke
/// `fuzzy_match_score`. An empty query returns every command, unranked, in
/// registry order.
fn fuzzy_match_commands(query: &str) -> Vec<CommandMatch> {
    let all: Vec<&'static CommandDef> = crate::commands::all_commands();

    if query.is_empty() {
        return all
            .into_iter()
            .map(|def| CommandMatch {
                def,
                indices: Vec::new(),
            })
            .collect();
    }

    let mut matcher = Matcher::new(Config::DEFAULT);
    // Lower-case both sides: nucleo's smart-case path (triggered by an
    // uppercase query char) has a known crash against some target lengths
    // (nucleo-matcher#footgun — "should have been caught by prefilter" in
    // `fuzzy_optimal.rs`); forcing case-insensitive matching here also
    // matches the bespoke matcher's prior behavior and the file finder's
    // typically-lowercase filenames.
    let query_lower = query.to_lowercase();
    let mut query_buf = Vec::new();
    let needle = Utf32Str::new(&query_lower, &mut query_buf);

    let mut results: Vec<(CommandMatch, u16)> = all
        .into_iter()
        .filter_map(|def| {
            let label_lower = def.label.to_lowercase();
            let mut label_buf = Vec::new();
            let haystack = Utf32Str::new(&label_lower, &mut label_buf);
            let score = matcher.fuzzy_match(haystack, needle)?;

            let mut indices = vec![];
            matcher.fuzzy_indices(haystack, needle, &mut indices);

            Some((CommandMatch { def, indices }, score))
        })
        .collect();

    results.sort_by_key(|(_, score)| std::cmp::Reverse(*score));
    results.into_iter().map(|(m, _)| m).collect()
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
    state.scroll_offset = 0;
}

// ============================================================================
// Recent Files Ordering Authority
// ============================================================================

/// The ordering authority for the Recent Files modal (overlay-surface.md
/// "Ordering authority"): recomputes `state.filtered_rows` (filtered +
/// Pinned/date-grouped) and resets selection, mirroring
/// `resolve_palette_rows`. Both the view's spec builder and
/// `ModalMsg::Confirm`/`SelectNext` read `filtered_rows` instead of
/// re-deriving it.
pub fn resolve_recent_rows(state: &mut RecentFilesState) {
    state.recompute_filtered_rows();
    state.selected_index = 0;
    state.scroll_offset = 0;
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
    results.sort_by_key(|a| std::cmp::Reverse(a.score));

    // Limit results
    results.truncate(50);
    results
}

#[cfg(test)]
mod tests {
    use super::{get_current_cursor_lines, resolve_palette_rows, update_ui};
    use crate::commands::{Cmd, CommandId, DamageArea};
    use crate::image::ImageState;
    use crate::messages::{ModalMsg, UiMsg};
    use crate::model::{AppModel, CommandPaletteState, ModalId, ModalState, ViewMode};

    #[test]
    fn current_cursor_lines_are_reported_for_plain_text_editors() {
        let mut model = AppModel::new(80, 60, 1.0, vec![]);
        model.editor_mut().cursors[0].line = 7;

        assert_eq!(get_current_cursor_lines(&model), vec![7]);
    }

    #[test]
    fn current_cursor_lines_are_ignored_for_image_editors() {
        let mut model = AppModel::new(80, 60, 1.0, vec![]);
        model.editor_mut().view_mode = ViewMode::Image(Box::new(ImageState::new(
            vec![255, 255, 255, 255],
            1,
            1,
            0,
            "PNG".into(),
            80,
            60,
        )));
        model.editor_mut().cursors[0].line = 7;

        assert!(get_current_cursor_lines(&model).is_empty());
    }

    #[test]
    fn blink_cursor_dedupes_dirty_lines_from_previous_and_current() {
        let mut model = AppModel::new(80, 60, 1.0, vec![]);
        // Force update_cursor_blink to report a state change on the next call.
        model.config.cursor_blink_ms = 0;

        // Two cursors: one overlaps a previous line, one is new.
        model.editor_mut().cursors[0].line = 3;
        let mut second_cursor = model.editor_mut().cursors[0];
        second_cursor.line = 5;
        model.editor_mut().cursors.push(second_cursor);

        // Previous cursor lines overlap partially (3) and add a line not present now (9).
        model.ui.previous_cursor_lines = vec![3, 9];

        let cmd = update_ui(&mut model, UiMsg::BlinkCursor);

        let areas = match cmd {
            Some(Cmd::RedrawAreas(areas)) => areas,
            other => panic!("expected Cmd::RedrawAreas, got {other:?}"),
        };
        assert_eq!(areas.len(), 1);
        let mut lines = match &areas[0] {
            DamageArea::CursorLines(lines) => lines.clone(),
            other => panic!("expected DamageArea::CursorLines, got {other:?}"),
        };
        lines.sort_unstable();
        assert_eq!(lines, vec![3, 5, 9], "dirty lines should be deduplicated");

        // previous_cursor_lines should now be updated to the current cursor lines.
        let mut updated_previous = model.ui.previous_cursor_lines.clone();
        updated_previous.sort_unstable();
        assert_eq!(updated_previous, vec![3, 5]);
    }

    // ========================================================================
    // Command Palette Ordering Authority
    // ========================================================================

    /// Regression for the pre-existing hazard overlay-surface.md calls out:
    /// `Confirm` used to re-derive the filtered list independently via
    /// `filter_commands`, which only worked because nothing reordered. Now
    /// both the view and `Confirm` read `state.matches` — this proves Enter
    /// activates the exact row the cache (and thus the view) showed as
    /// selected, not an independently re-derived list.
    #[test]
    fn confirm_executes_the_row_selected_in_the_cached_view_order() {
        let mut model = AppModel::new(80, 60, 1.0, vec![]);
        update_ui(&mut model, UiMsg::ToggleModal(ModalId::CommandPalette));

        // Empty query: `matches` is every command in registry order —
        // deterministic, so index 1 is known ahead of time (`OpenFile`).
        update_ui(&mut model, UiMsg::Modal(ModalMsg::SelectNext));

        let expected_id = match &model.ui.active_modal {
            Some(ModalState::CommandPalette(state)) => state.matches[state.selected_index].def.id,
            other => panic!("expected command palette modal, got {other:?}"),
        };
        assert_eq!(expected_id, CommandId::OpenFile);

        let cmd = update_ui(&mut model, UiMsg::Modal(ModalMsg::Confirm));

        assert!(model.ui.active_modal.is_none(), "palette closes on confirm");
        assert!(
            matches!(cmd, Some(Cmd::ShowOpenFileDialog { .. })),
            "Confirm should have executed OpenFile (the cached row selected), got {cmd:?}"
        );
    }

    #[test]
    fn resolve_palette_rows_ranks_fuzzy_matches_and_resets_selection() {
        let mut state = CommandPaletteState {
            selected_index: 5,
            ..Default::default()
        };
        state.set_input("gtln"); // fuzzy subsequence of "Go to Line..."
        resolve_palette_rows(&mut state);

        assert_eq!(state.selected_index, 0);
        assert_eq!(state.scroll_offset, 0);
        assert!(!state.matches.is_empty());
        assert_eq!(state.matches[0].def.id, CommandId::GotoLine);
        assert!(
            !state.matches[0].indices.is_empty(),
            "match indices should be populated for a non-empty query"
        );
    }

    #[test]
    fn resolve_palette_rows_empty_query_returns_all_commands_in_registry_order() {
        let mut state = CommandPaletteState::default();
        resolve_palette_rows(&mut state);
        assert_eq!(
            state.matches.iter().map(|m| m.def.id).collect::<Vec<_>>(),
            crate::commands::all_commands()
                .iter()
                .map(|d| d.id)
                .collect::<Vec<_>>()
        );
    }
}
