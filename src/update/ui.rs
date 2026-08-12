//! UI message handlers (status bar, cursor blink, transient messages, modals)

use std::time::Duration;

use crate::commands::Cmd;
use crate::editable::{EditableState, StringBuffer};
use crate::messages::LayoutMsg;
use crate::messages::{ModalMsg, UiMsg};
use crate::model::{
    AppModel, CommandPaletteState, FileFinderState, GotoLineState, ModalId, ModalState,
    RecentFilesState, SearchTab, SegmentContent, SegmentId, ThemePickerState, TransientMessage,
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
                    // Cmd+Shift+A: Search Everywhere, pre-focused on All
                    // (overlay-surface.md Phase 4 "Bindings").
                    let mut state = model.ui.last_command_palette.clone().unwrap_or_default();
                    state.files_available = model.workspace.is_some();
                    resolve_palette_rows(&mut state, &model.command_history);
                    state.active_tab = SearchTab::All;
                    if state.files.is_none() {
                        state.files = build_file_finder_state(model, &state.input());
                    }
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
            // Cmd+Shift+O retargets to Search Everywhere pre-focused on the
            // Files tab (overlay-surface.md Phase 4: the standalone File
            // Finder modal is retired). With no workspace, Files is
            // `Unavailable` rather than refusing to open.
            let mut state = model.ui.last_command_palette.clone().unwrap_or_default();
            state.files_available = model.workspace.is_some();
            resolve_palette_rows(&mut state, &model.command_history);
            state.active_tab = SearchTab::Files;
            if state.files.is_none() {
                state.files = build_file_finder_state(model, &state.input());
            }
            model.ui.open_modal(ModalState::CommandPalette(state));
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
fn on_modal_input_changed(modal: &mut ModalState, history: &CommandHistory) {
    match modal {
        ModalState::CommandPalette(state) => {
            resolve_palette_rows(state, history);
            // Query is shared across tabs — keep the (lazily-populated)
            // Files tab's own results in sync (overlay-surface.md Phase 4:
            // "query persists across tabs").
            let query = state.input();
            if let Some(files) = state.files.as_mut() {
                files.set_input(&query);
                update_file_finder_results(files);
            }
        }
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
            state.files_available = model.workspace.is_some();
            resolve_palette_rows(&mut state, &model.command_history);
            state.active_tab = SearchTab::All;
            if state.files.is_none() {
                state.files = build_file_finder_state(model, &state.input());
            }
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
                on_modal_input_changed(modal, &model.command_history);
                Some(Cmd::Redraw)
            } else {
                None
            }
        }

        ModalMsg::InsertChar(ch) => {
            if let Some(ref mut modal) = model.ui.active_modal {
                // Prefix routing (overlay-surface.md Phase 4): `>`/`@` as
                // char 0 of a *previously empty* query pins the Commands/
                // Symbols tab and is consumed, not inserted.
                if let ModalState::CommandPalette(state) = modal {
                    if state.input().is_empty() {
                        if let Some(tab) = search_tab_for_prefix(ch) {
                            // Mirror `activate_search_tab`/`cycle_search_tab`:
                            // never park on an `Unavailable` tab (Symbols is
                            // always `Unavailable` today) — fall through to
                            // inserting the char instead.
                            if state.tab_available(tab) {
                                state.active_tab = tab;
                                return Some(Cmd::Redraw);
                            }
                        }
                    }
                }
                if let Some(editable) = modal_editable_mut(modal) {
                    editable.insert_char(ch);
                }
                on_modal_input_changed(modal, &model.command_history);
                Some(Cmd::Redraw)
            } else {
                None
            }
        }

        ModalMsg::DeleteBackward => {
            if let Some(ref mut modal) = model.ui.active_modal {
                // Backspace on an already-empty query returns to the All
                // tab — the mirror of prefix routing above.
                if let ModalState::CommandPalette(state) = modal {
                    if state.input().is_empty() {
                        state.active_tab = SearchTab::All;
                    }
                }
                if let Some(editable) = modal_editable_mut(modal) {
                    editable.delete_backward();
                }
                on_modal_input_changed(modal, &model.command_history);
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
                on_modal_input_changed(modal, &model.command_history);
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
                    on_modal_input_changed(modal, &model.command_history);
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
                        on_modal_input_changed(modal, &model.command_history);
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
                on_modal_input_changed(modal, &model.command_history);
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
            // Commands tab: pin/unpin the selected command
            // (overlay-surface.md Phase 4 Behaviour: "Pinning: commands via
            // CommandUsage.is_pinned (⌘. toggle in the Commands tab)").
            if let Some(ModalState::CommandPalette(ref mut state)) = model.ui.active_modal {
                if state.active_tab == SearchTab::Commands {
                    if let Some(cmd_id) = state.matches.get(state.selected_index).map(|m| m.def.id)
                    {
                        model.command_history.toggle_pin(cmd_id);
                        if let Some(ModalState::CommandPalette(ref mut state)) =
                            model.ui.active_modal
                        {
                            resolve_palette_rows(state, &model.command_history);
                            if let Some(idx) = state.matches.iter().position(|m| m.def.id == cmd_id)
                            {
                                state.selected_index = idx;
                                let shapes = flat_shapes(state.matches.len());
                                state.scroll_offset = resolve_scroll_for_selection(
                                    &shapes,
                                    idx,
                                    COMMAND_PALETTE_MAX_VISIBLE,
                                    state.scroll_offset,
                                );
                            }
                        }
                        let history = model.command_history.clone();
                        return Some(Cmd::Batch(vec![
                            Cmd::Redraw,
                            Cmd::SaveCommandHistory { history },
                        ]));
                    }
                }
            }
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

        ModalMsg::NextTab => cycle_search_tab(model, true),

        ModalMsg::PrevTab => cycle_search_tab(model, false),

        ModalMsg::ActivateTab(index) => activate_search_tab(model, index),

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

        ModalMsg::ToggleFindReplaceWholeWord => {
            if let Some(ModalState::FindReplace(ref mut state)) = model.ui.active_modal {
                state.whole_word = !state.whole_word;
                Some(Cmd::Redraw)
            } else {
                None
            }
        }

        ModalMsg::ToggleFindReplaceRegex => {
            if let Some(ModalState::FindReplace(ref mut state)) = model.ui.active_modal {
                state.use_regex = !state.use_regex;
                Some(Cmd::Redraw)
            } else {
                None
            }
        }

        ModalMsg::FindNext => {
            if let Some(ModalState::FindReplace(ref state)) = model.ui.active_modal {
                let query = state.build_query();
                if !state.query().is_empty() {
                    model.ui.last_find_replace = model.ui.active_modal.clone().and_then(|m| {
                        if let ModalState::FindReplace(s) = m {
                            Some(s)
                        } else {
                            None
                        }
                    });
                    return find_next_in_document(model, &query);
                }
            }
            Some(Cmd::Redraw)
        }

        ModalMsg::FindPrevious => {
            if let Some(ModalState::FindReplace(ref state)) = model.ui.active_modal {
                let query = state.build_query();
                if !state.query().is_empty() {
                    model.ui.last_find_replace = model.ui.active_modal.clone().and_then(|m| {
                        if let ModalState::FindReplace(s) = m {
                            Some(s)
                        } else {
                            None
                        }
                    });
                    return find_prev_in_document(model, &query);
                }
            }
            Some(Cmd::Redraw)
        }

        ModalMsg::ReplaceAndFindNext => {
            if let Some(ModalState::FindReplace(ref state)) = model.ui.active_modal {
                let query = state.build_query();
                let replacement = state.replacement();
                if !state.query().is_empty() {
                    model.ui.last_find_replace = model.ui.active_modal.clone().and_then(|m| {
                        if let ModalState::FindReplace(s) = m {
                            Some(s)
                        } else {
                            None
                        }
                    });
                    return replace_and_find_next(model, &query, &replacement);
                }
            }
            Some(Cmd::Redraw)
        }

        ModalMsg::ReplaceAll => {
            if let Some(ModalState::FindReplace(ref state)) = model.ui.active_modal {
                let query = state.build_query();
                let replacement = state.replacement();
                if !state.query().is_empty() {
                    model.ui.last_find_replace = model.ui.active_modal.clone().and_then(|m| {
                        if let ModalState::FindReplace(s) = m {
                            Some(s)
                        } else {
                            None
                        }
                    });
                    return replace_all(model, &query, &replacement);
                }
            }
            Some(Cmd::Redraw)
        }
    }
}

/// Prefix routing (overlay-surface.md Phase 4 Key routing): `>` pins the
/// Commands tab, `@` pins Symbols. No other prefix is recognized (`:`
/// goto-line was explicitly dropped — Cmd+L already exists).
fn search_tab_for_prefix(ch: char) -> Option<SearchTab> {
    match ch {
        '>' => Some(SearchTab::Commands),
        '@' => Some(SearchTab::Symbols),
        _ => None,
    }
}

/// `ModalMsg::NextTab`/`PrevTab` (⇥/⇧⇥): cycle Search Everywhere's tabs,
/// skipping `Unavailable` ones (Symbols always; Files with no workspace).
/// A no-op for every other modal.
fn cycle_search_tab(model: &mut AppModel, forward: bool) -> Option<Cmd> {
    // Computed up front (owned data, not borrowed from `model`) so it can
    // still be used after `state` takes a mutable borrow of
    // `model.ui.active_modal` below.
    let workspace_files = model
        .workspace
        .as_ref()
        .map(|w| (w.file_tree.get_all_file_paths(), w.root.clone()));

    if let Some(ModalState::CommandPalette(ref mut state)) = model.ui.active_modal {
        let order = SearchTab::ORDER;
        let current = order.iter().position(|&t| t == state.active_tab)?;
        let n = order.len();
        for step in 1..=n {
            let idx = if forward {
                (current + step) % n
            } else {
                (current + n - step) % n
            };
            let candidate = order[idx];
            if state.tab_available(candidate) {
                state.active_tab = candidate;
                // The All tab also renders a Files group, so it needs the
                // index loaded just as much as the Files tab itself.
                if matches!(candidate, SearchTab::Files | SearchTab::All) && state.files.is_none()
                {
                    if let Some((all_files, root)) = workspace_files {
                        state.files = Some(seeded_file_finder_state(
                            all_files,
                            root,
                            &state.input(),
                        ));
                    }
                }
                break;
            }
        }
        return Some(Cmd::Redraw);
    }
    None
}

/// `ModalMsg::ActivateTab` (tab click): switch to `SearchTab::ORDER[index]`,
/// a no-op if out of range or `Unavailable`.
fn activate_search_tab(model: &mut AppModel, index: usize) -> Option<Cmd> {
    let workspace_files = model
        .workspace
        .as_ref()
        .map(|w| (w.file_tree.get_all_file_paths(), w.root.clone()));

    if let Some(ModalState::CommandPalette(ref mut state)) = model.ui.active_modal {
        let candidate = *SearchTab::ORDER.get(index)?;
        if !state.tab_available(candidate) {
            return Some(Cmd::Redraw);
        }
        state.active_tab = candidate;
        if matches!(candidate, SearchTab::Files | SearchTab::All) && state.files.is_none() {
            if let Some((all_files, root)) = workspace_files {
                state.files = Some(seeded_file_finder_state(
                    all_files,
                    root,
                    &state.input(),
                ));
            }
        }
        return Some(Cmd::Redraw);
    }
    None
}

/// Set the `FlatIndex`-space selected row for whichever list-body modal is
/// active — used by `ModalMsg::ActivateRow` (row click) ahead of confirming.
/// A no-op for `Fields`/no-list contexts.
fn set_modal_selected_index(modal: &mut ModalState, row: usize) {
    match modal {
        ModalState::CommandPalette(state) => match state.active_tab {
            SearchTab::Commands => state.selected_index = row.min(state.matches.len()),
            SearchTab::Files => {
                if let Some(files) = state.files.as_mut() {
                    files.selected_index = row.min(files.results.len());
                }
            }
            SearchTab::All => state.all_selected = row.min(all_tab_total(state)),
            SearchTab::Symbols => {}
        },
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
            ModalState::CommandPalette(state) => confirm_search_everywhere(model, state),
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
                if !state.query().is_empty() {
                    let query = state.build_query();
                    model.ui.last_find_replace = Some(state);
                    return find_next_in_document(model, &query);
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

/// Per-group cap on the All tab's merged, non-scrolling summary
/// (overlay-surface.md Phase 4: "per-group cap 4–5 rows").
pub const ALL_TAB_GROUP_CAP: usize = 5;

/// `(title, row_count)` for the sections of whichever Search Everywhere tab
/// is active — real, selectable rows only (empty-state messages are drawn
/// as decoration outside `Body::List`, so they don't occupy `FlatIndex`
/// space). The single source of truth both the view's content spec and its
/// shape-only twin (hit-testing/caret placement) slice against, and that
/// [`commands_tab_shapes`]/[`all_tab_total`] below derive their counts
/// from — one function, so render, hit-test, and Confirm/SelectNext can't
/// drift out of step (overlay-surface.md "Hit-testing": one layout, two
/// consumers).
pub fn search_everywhere_sections(
    state: &CommandPaletteState,
) -> Vec<(Option<&'static str>, usize)> {
    match state.active_tab {
        SearchTab::Commands => {
            if state.recent_count > 0 {
                vec![
                    (Some("Recently Used"), state.recent_count),
                    (None, state.matches.len() - state.recent_count),
                ]
            } else {
                vec![(None, state.matches.len())]
            }
        }
        SearchTab::Files => {
            let count = state.files.as_ref().map(|f| f.results.len()).unwrap_or(0);
            if count > 0 {
                vec![(None, count)]
            } else {
                Vec::new()
            }
        }
        SearchTab::All => {
            let commands_cap = state.matches.len().min(ALL_TAB_GROUP_CAP);
            let file_count = state.files.as_ref().map(|f| f.results.len()).unwrap_or(0);
            let files_cap = file_count.min(ALL_TAB_GROUP_CAP);
            let mut sections = Vec::new();
            if commands_cap > 0 {
                sections.push((Some("Commands"), commands_cap));
            }
            if file_count > 0 {
                sections.push((Some("Files"), files_cap));
            }
            sections
        }
        SearchTab::Symbols => Vec::new(),
    }
}

/// Total selectable rows on the All tab: capped Commands + capped Files
/// (Symbols never contributes — disabled-state only).
fn all_tab_total(state: &CommandPaletteState) -> usize {
    search_everywhere_sections(state)
        .iter()
        .map(|(_, len)| len)
        .sum()
}

/// Section shapes for the Commands tab: an optional "Recently used" header
/// (only when `recent_count > 0` — the query is empty) plus the full list.
fn commands_tab_shapes(state: &CommandPaletteState) -> Vec<SectionShape> {
    search_everywhere_sections(state)
        .into_iter()
        .map(|(title, len)| SectionShape {
            has_title: title.is_some(),
            len,
        })
        .collect()
}

/// `ModalMsg::Confirm` for the Search Everywhere modal (overlay-surface.md
/// Phase 4): reads whichever tab's ordering-authority cache is active.
fn confirm_search_everywhere(model: &mut AppModel, state: CommandPaletteState) -> Option<Cmd> {
    match state.active_tab {
        SearchTab::Commands => {
            let idx = state.selected_index.min(state.matches.len());
            if let Some(cmd_match) = state.matches.get(idx) {
                let cmd_id = cmd_match.def.id;
                model.command_history.record_execution(cmd_id);
                let history = model.command_history.clone();
                model.ui.last_command_palette = Some(state);
                model.ui.close_modal();
                let exec = execute_command(model, cmd_id).unwrap_or(Cmd::Redraw);
                return Some(Cmd::Batch(vec![exec, Cmd::SaveCommandHistory { history }]));
            }
            model.ui.close_modal();
            Some(Cmd::Redraw)
        }
        SearchTab::Files => {
            let path = state
                .files
                .as_ref()
                .and_then(|f| f.results.get(f.selected_index))
                .map(|m| m.path.clone());
            model.ui.close_modal();
            match path {
                Some(path) => update_layout(model, LayoutMsg::OpenFileInNewTab(path)),
                None => Some(Cmd::Redraw),
            }
        }
        SearchTab::All => {
            let commands_shown = state.matches.len().min(ALL_TAB_GROUP_CAP);
            if state.all_selected < commands_shown {
                if let Some(cmd_match) = state.matches.get(state.all_selected) {
                    let cmd_id = cmd_match.def.id;
                    model.command_history.record_execution(cmd_id);
                    let history = model.command_history.clone();
                    model.ui.last_command_palette = Some(state);
                    model.ui.close_modal();
                    let exec = execute_command(model, cmd_id).unwrap_or(Cmd::Redraw);
                    return Some(Cmd::Batch(vec![exec, Cmd::SaveCommandHistory { history }]));
                }
                model.ui.close_modal();
                return Some(Cmd::Redraw);
            }
            let path = state
                .files
                .as_ref()
                .and_then(|f| f.results.get(state.all_selected - commands_shown))
                .map(|m| m.path.clone());
            model.ui.close_modal();
            match path {
                Some(path) => update_layout(model, LayoutMsg::OpenFileInNewTab(path)),
                None => Some(Cmd::Redraw),
            }
        }
        SearchTab::Symbols => {
            model.ui.close_modal();
            Some(Cmd::Redraw)
        }
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

/// Move the active tab's selection by `delta` (-1/+1), wrapping — the
/// Search Everywhere equivalent of `move_list_selection`, dispatched by
/// `state.active_tab` (overlay-surface.md Phase 4: per-tab selection).
fn move_search_everywhere_selection(state: &mut CommandPaletteState, delta: isize) {
    match state.active_tab {
        SearchTab::Commands => {
            let shapes = commands_tab_shapes(state);
            move_list_selection(
                &mut state.selected_index,
                &mut state.scroll_offset,
                &shapes,
                delta,
            );
        }
        SearchTab::Files => {
            if let Some(files) = state.files.as_mut() {
                let shapes = flat_shapes(files.results.len());
                move_list_selection(
                    &mut files.selected_index,
                    &mut files.scroll_offset,
                    &shapes,
                    delta,
                );
            }
        }
        SearchTab::All => {
            let total = all_tab_total(state);
            if total > 0 {
                state.all_selected =
                    (state.all_selected as isize + delta).rem_euclid(total as isize) as usize;
            }
        }
        SearchTab::Symbols => {}
    }
}

/// `ModalMsg::SelectPrevious`/`SelectNext`: move selection by `delta`
/// (-1/+1) in whichever list-body modal is active. Theme Picker previews
/// the newly-selected theme live.
fn modal_select(model: &mut AppModel, delta: isize) -> Option<Cmd> {
    let modal = model.ui.active_modal.as_mut()?;
    let preview_theme_id = match modal {
        ModalState::CommandPalette(state) => {
            move_search_everywhere_selection(state, delta);
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
        ModalState::CommandPalette(state) => match state.active_tab {
            SearchTab::Commands => {
                let shapes = commands_tab_shapes(state);
                page_list_selection(
                    &mut state.selected_index,
                    &mut state.scroll_offset,
                    &shapes,
                    forward,
                );
            }
            SearchTab::Files => {
                if let Some(files) = state.files.as_mut() {
                    let shapes = flat_shapes(files.results.len());
                    page_list_selection(
                        &mut files.selected_index,
                        &mut files.scroll_offset,
                        &shapes,
                        forward,
                    );
                }
            }
            // All is a non-scrolling summary — no paging.
            SearchTab::All | SearchTab::Symbols => {}
        },
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
        ModalState::CommandPalette(state) => match state.active_tab {
            SearchTab::Commands => {
                let shapes = commands_tab_shapes(state);
                (&mut state.scroll_offset, shapes)
            }
            SearchTab::Files => {
                let files = state.files.as_mut()?;
                let shapes = flat_shapes(files.results.len()).to_vec();
                (&mut files.scroll_offset, shapes)
            }
            // All is a non-scrolling summary.
            SearchTab::All | SearchTab::Symbols => return None,
        },
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

/// Show "No matches found", or the regex error if the query failed to
/// compile — shared by find-next/find-previous/replace-all.
fn report_no_matches(model: &mut AppModel, query: &crate::search::SearchQuery) {
    let text = match &query.error {
        Some(err) => format!("Invalid regex: {}", err),
        None => "No matches found".to_string(),
    };
    model.ui.transient_message = Some(TransientMessage::new(text, Duration::from_secs(2)));
}

/// Find next occurrence in the document and select it
fn find_next_in_document(model: &mut AppModel, query: &crate::search::SearchQuery) -> Option<Cmd> {
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

    let matches = doc.search_matches(query);
    let found = matches
        .iter()
        .find(|m| m.start > start_offset)
        .or_else(|| matches.first())
        .copied();

    if let Some(m) = found {
        let (start_line, start_col) = doc.offset_to_cursor(m.start);
        let (end_line, end_col) = doc.offset_to_cursor(m.end);

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
        report_no_matches(model, query);
        Some(Cmd::redraw_editor())
    }
}

/// Find previous occurrence in the document and select it
fn find_prev_in_document(model: &mut AppModel, query: &crate::search::SearchQuery) -> Option<Cmd> {
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

    let matches = doc.search_matches(query);
    let found = matches
        .iter()
        .rev()
        .find(|m| m.start < start_offset)
        .or_else(|| matches.last())
        .copied();

    if let Some(m) = found {
        let (start_line, start_col) = doc.offset_to_cursor(m.start);
        let (end_line, end_col) = doc.offset_to_cursor(m.end);

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
        report_no_matches(model, query);
        Some(Cmd::redraw_editor())
    }
}

/// Replace current selection if it matches, then find next
fn replace_and_find_next(
    model: &mut AppModel,
    query: &crate::search::SearchQuery,
    replacement: &str,
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

            let is_match = doc
                .search_matches(query)
                .iter()
                .any(|m| m.start == start_offset && m.end == end_offset);

            is_match.then_some((start_offset, end_offset))
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
    find_next_in_document(model, query)
}

/// Replace all occurrences
fn replace_all(
    model: &mut AppModel,
    query: &crate::search::SearchQuery,
    replacement: &str,
) -> Option<Cmd> {
    let doc = model.document();
    let occurrences = doc.search_matches(query);

    if occurrences.is_empty() {
        report_no_matches(model, query);
        return Some(Cmd::Redraw);
    }

    let count = occurrences.len();

    // Replace from end to start to preserve offsets
    let doc = model.document_mut();
    let replacement_char_len = replacement.chars().count();
    for m in occurrences.into_iter().rev() {
        doc.buffer.remove(m.start..m.end);
        doc.buffer.insert(m.start, replacement);
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

use crate::command_history::CommandHistory;
use crate::commands::CommandDef;
use crate::model::CommandMatch;

/// The ordering authority for the command palette (overlay-surface.md
/// "Ordering authority"): the *only* place that filters/ranks commands.
/// Both the palette's spec builder and `ModalMsg::Confirm`/`SelectNext`
/// consume this cache instead of re-deriving the list, so Enter always
/// activates the row the user actually sees selected.
///
/// On an empty query the leading `state.recent_count` entries of
/// `state.matches` are duplicated from the "Recently used" (top 3 by
/// recency) set — overlay-surface.md Phase 4: "on an empty query, the
/// Commands tab (and All) show a 'Recently used' section ... above
/// unfiltered commands. On the first typed char the section disappears".
pub fn resolve_palette_rows(state: &mut CommandPaletteState, history: &CommandHistory) {
    let query = state.input();
    let mut matches = fuzzy_match_commands(&query, history);

    state.recent_count = if query.is_empty() {
        let all_ids: Vec<crate::commands::CommandId> = matches.iter().map(|m| m.def.id).collect();
        let recent_ids = history.recent_commands(&all_ids, 3);
        let recent: Vec<CommandMatch> = recent_ids
            .iter()
            .filter_map(|id| matches.iter().find(|m| m.def.id == *id).cloned())
            .collect();
        let n = recent.len();
        // The "Recently used" section sits *above* unfiltered commands
        // (overlay-surface.md Phase 4) — drop the promoted entries from the
        // list below it so they don't also show up as their own (recency-
        // sorted) row immediately after the section.
        let rest = matches
            .into_iter()
            .filter(|m| !recent_ids.contains(&m.def.id));
        matches = recent.into_iter().chain(rest).collect();
        n
    } else {
        0
    };

    state.matches = matches;
    state.selected_index = 0;
    state.scroll_offset = 0;
    state.all_selected = 0;
}

/// A used-at-all command's fuzzy score is nudged up by this much before
/// ranking — small enough that a strictly better match still wins, but
/// enough to break ties/near-ties in a recently-used command's favor.
/// ponytail: flat boost rather than a normalized recency curve; revisit
/// with a decaying bonus (e.g. score + k / (1 + hours_since_use)) if a
/// heavily-used command's staleness starts to matter.
const RECENCY_BOOST: u32 = 3;

/// Fuzzy-match commands against `query` using nucleo, the same pattern the
/// file finder uses below (`fuzzy_match_files`) — replaces the old bespoke
/// `fuzzy_match_score`. An empty query returns every command in registry
/// order. Ranking (overlay-surface.md Phase 4 Behaviour): pinned first,
/// then recency-*boosted* fuzzy score (a used command's score gets
/// `RECENCY_BOOST` added, not an outright recency-major sort — a strictly
/// better match still outranks a single stale execution) — ties (e.g. an
/// empty query with no usage history) keep registry order via the stable
/// sort.
fn fuzzy_match_commands(query: &str, history: &CommandHistory) -> Vec<CommandMatch> {
    let all: Vec<&'static CommandDef> = crate::commands::all_commands();

    let scored: Vec<(CommandMatch, u32)> = if query.is_empty() {
        all.into_iter()
            .map(|def| {
                (
                    CommandMatch {
                        def,
                        indices: Vec::new(),
                    },
                    0u32,
                )
            })
            .collect()
    } else {
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

        all.into_iter()
            .filter_map(|def| {
                let label_lower = def.label.to_lowercase();
                let mut label_buf = Vec::new();
                let haystack = Utf32Str::new(&label_lower, &mut label_buf);
                let score = matcher.fuzzy_match(haystack, needle)?;

                let mut indices = vec![];
                matcher.fuzzy_indices(haystack, needle, &mut indices);

                Some((CommandMatch { def, indices }, score as u32))
            })
            .collect()
    };

    let mut results = scored;
    results.sort_by_key(|(m, score)| {
        let pinned = history.is_pinned(m.def.id);
        let boosted = if history.recency_score(m.def.id) > 0 {
            score.saturating_add(RECENCY_BOOST)
        } else {
            *score
        };
        (std::cmp::Reverse(pinned), std::cmp::Reverse(boosted))
    });
    results.into_iter().map(|(m, _)| m).collect()
}

// ============================================================================
// Fuzzy File Finder
// ============================================================================

use crate::model::FileMatch;
use nucleo_matcher::{Config, Matcher, Utf32Str};
use std::path::{Path, PathBuf};

/// Build the Files tab's backing state from `all_files`/`root`, seeded with
/// `query` — the palette's shared query, so the newly-populated Files tab's
/// own results match what the header already shows instead of starting
/// unfiltered (overlay-surface.md Phase 4: "query persists across tabs").
fn seeded_file_finder_state(all_files: Vec<PathBuf>, root: PathBuf, query: &str) -> FileFinderState {
    let mut state = FileFinderState::new(all_files, root);
    state.set_input(query);
    update_file_finder_results(&mut state);
    state
}

/// Build the Files tab's backing state from the open workspace, matched
/// against `query` — `None` when no workspace is open (the tab is
/// `Unavailable` in that case, per overlay-surface.md Phase 4 State merge).
/// Lazy: only called on first activation of the Files tab or the All tab
/// (which also renders a Files group), not unconditionally at Search
/// Everywhere open time.
fn build_file_finder_state(model: &AppModel, query: &str) -> Option<FileFinderState> {
    let workspace = model.workspace.as_ref()?;
    let all_files = workspace.file_tree.get_all_file_paths();
    Some(seeded_file_finder_state(
        all_files,
        workspace.root.clone(),
        query,
    ))
}

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
    use super::{
        get_current_cursor_lines, resolve_palette_rows, search_everywhere_sections, update_ui,
    };
    use crate::command_history::CommandHistory;
    use crate::commands::{Cmd, CommandId, DamageArea};
    use crate::image::ImageState;
    use crate::messages::{ModalMsg, UiMsg};
    use crate::model::{AppModel, CommandPaletteState, ModalId, ModalState, SearchTab, ViewMode};

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
        // Cmd+Shift+A opens on the All tab (overlay-surface.md Phase 4).
        update_ui(&mut model, UiMsg::ToggleModal(ModalId::CommandPalette));

        // Empty query: `matches` is every command in registry order —
        // deterministic, so index 1 is known ahead of time (`OpenFile`).
        update_ui(&mut model, UiMsg::Modal(ModalMsg::SelectNext));

        let expected_id = match &model.ui.active_modal {
            Some(ModalState::CommandPalette(state)) => state.matches[state.all_selected].def.id,
            other => panic!("expected command palette modal, got {other:?}"),
        };
        assert_eq!(expected_id, CommandId::OpenFile);

        let cmd = update_ui(&mut model, UiMsg::Modal(ModalMsg::Confirm));

        assert!(model.ui.active_modal.is_none(), "palette closes on confirm");
        // Batched with `Cmd::SaveCommandHistory` (overlay-surface.md Phase 4
        // usage tracking) — dig out the `ShowOpenFileDialog` among the batch.
        let opened_file_dialog = match &cmd {
            Some(Cmd::Batch(cmds)) => cmds
                .iter()
                .any(|c| matches!(c, Cmd::ShowOpenFileDialog { .. })),
            Some(Cmd::ShowOpenFileDialog { .. }) => true,
            _ => false,
        };
        assert!(
            opened_file_dialog,
            "Confirm should have executed OpenFile (the cached row selected), got {cmd:?}"
        );
    }

    #[test]
    fn resolve_palette_rows_empty_query_does_not_duplicate_recents_below_the_section() {
        let mut history = CommandHistory::default();
        history.record_execution(CommandId::SaveFile);
        history.record_execution(CommandId::GotoLine);

        let mut state = CommandPaletteState::default();
        resolve_palette_rows(&mut state, &history);

        assert_eq!(state.recent_count, 2);
        let ids: Vec<CommandId> = state.matches.iter().map(|m| m.def.id).collect();
        // Both commands were recorded within the same wall-clock second in
        // this test, so their exact relative order (a `last_used` tie) is
        // not under test here — only that both lead and neither repeats.
        let mut head = ids[..2].to_vec();
        head.sort_by_key(|id| format!("{id:?}"));
        assert_eq!(head, [CommandId::GotoLine, CommandId::SaveFile]);
        assert!(
            !ids[2..].contains(&CommandId::GotoLine) && !ids[2..].contains(&CommandId::SaveFile),
            "recent commands should not also appear in the unfiltered list below the \
             Recently Used section, got {ids:?}"
        );
        assert_eq!(
            ids.len(),
            crate::commands::all_commands().len(),
            "no commands should be dropped, only reordered"
        );
    }

    #[test]
    fn resolve_palette_rows_ranks_fuzzy_matches_and_resets_selection() {
        let mut state = CommandPaletteState {
            selected_index: 5,
            ..Default::default()
        };
        state.set_input("gtln"); // fuzzy subsequence of "Go to Line..."
        resolve_palette_rows(&mut state, &CommandHistory::default());

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
    fn recency_boosts_but_does_not_dominate_a_strictly_better_match() {
        // "gl" fuzzy-matches "Go to Line..." (score 55) well above "Go to
        // File..." (score 43) — a single stale execution of the weaker
        // match must not permanently outrank it; recency is a tie-breaking
        // boost, not a primary sort key.
        let mut history = CommandHistory::default();
        history.record_execution(CommandId::FuzzyFileFinder);

        let mut state = CommandPaletteState::default();
        state.set_input("gl");
        resolve_palette_rows(&mut state, &history);

        assert_eq!(
            state.matches[0].def.id,
            CommandId::GotoLine,
            "expected the stronger fuzzy match to win despite the weaker match's recency"
        );
    }

    #[test]
    fn resolve_palette_rows_empty_query_returns_all_commands_in_registry_order() {
        let mut state = CommandPaletteState::default();
        resolve_palette_rows(&mut state, &CommandHistory::default());
        assert_eq!(
            state.matches.iter().map(|m| m.def.id).collect::<Vec<_>>(),
            crate::commands::all_commands()
                .iter()
                .map(|d| d.id)
                .collect::<Vec<_>>()
        );
    }

    // ========================================================================
    // Search Everywhere: prefix routing, tab cycling, All tab (Phase 4)
    // ========================================================================

    #[test]
    fn insert_char_gt_on_empty_query_pins_commands_tab_and_is_consumed() {
        let mut model = AppModel::new(80, 60, 1.0, vec![]);
        update_ui(&mut model, UiMsg::ToggleModal(ModalId::CommandPalette));
        assert!(matches!(
            &model.ui.active_modal,
            Some(ModalState::CommandPalette(s)) if s.active_tab == SearchTab::All
        ));

        update_ui(&mut model, UiMsg::Modal(ModalMsg::InsertChar('>')));

        match &model.ui.active_modal {
            Some(ModalState::CommandPalette(state)) => {
                assert_eq!(state.active_tab, SearchTab::Commands);
                assert_eq!(state.input(), "", "prefix char is consumed, not inserted");
            }
            other => panic!("expected command palette modal, got {other:?}"),
        }
    }

    #[test]
    fn insert_char_at_on_empty_query_does_not_pin_unavailable_symbols_tab() {
        // Symbols is always `Unavailable` (no workspace-symbols provider
        // exists yet) — `@` must not park the user on a dead tab; it falls
        // through to a literal char insert instead, same as any other
        // prefix routed to an `Unavailable` tab.
        let mut model = AppModel::new(80, 60, 1.0, vec![]);
        update_ui(&mut model, UiMsg::ToggleModal(ModalId::CommandPalette));
        update_ui(&mut model, UiMsg::Modal(ModalMsg::InsertChar('@')));
        match &model.ui.active_modal {
            Some(ModalState::CommandPalette(state)) => {
                assert_eq!(state.active_tab, SearchTab::All);
                assert_eq!(state.input(), "@");
            }
            other => panic!("expected command palette modal, got {other:?}"),
        }
    }

    #[test]
    fn prefix_char_only_recognized_on_previously_empty_query() {
        let mut model = AppModel::new(80, 60, 1.0, vec![]);
        update_ui(&mut model, UiMsg::ToggleModal(ModalId::CommandPalette));
        update_ui(&mut model, UiMsg::Modal(ModalMsg::InsertChar('g')));
        update_ui(&mut model, UiMsg::Modal(ModalMsg::InsertChar('>')));
        match &model.ui.active_modal {
            Some(ModalState::CommandPalette(state)) => {
                // Not a prefix mid-query — inserted literally, tab unchanged.
                assert_eq!(state.input(), "g>");
                assert_eq!(state.active_tab, SearchTab::All);
            }
            other => panic!("expected command palette modal, got {other:?}"),
        }
    }

    #[test]
    fn backspace_on_empty_query_returns_to_all_tab() {
        let mut model = AppModel::new(80, 60, 1.0, vec![]);
        update_ui(&mut model, UiMsg::ToggleModal(ModalId::CommandPalette));
        update_ui(&mut model, UiMsg::Modal(ModalMsg::InsertChar('>')));
        // Consumed the prefix; query is empty, tab is Commands.
        update_ui(&mut model, UiMsg::Modal(ModalMsg::DeleteBackward));
        match &model.ui.active_modal {
            Some(ModalState::CommandPalette(state)) => {
                assert_eq!(state.active_tab, SearchTab::All);
            }
            other => panic!("expected command palette modal, got {other:?}"),
        }
    }

    #[test]
    fn next_tab_skips_unavailable_files_and_symbols_with_no_workspace() {
        let mut model = AppModel::new(80, 60, 1.0, vec![]);
        assert!(model.workspace.is_none());
        update_ui(&mut model, UiMsg::ToggleModal(ModalId::CommandPalette));
        // All -> Commands -> (Files unavailable, Symbols unavailable) -> All
        update_ui(&mut model, UiMsg::Modal(ModalMsg::NextTab));
        assert_tab(&model, SearchTab::Commands);
        update_ui(&mut model, UiMsg::Modal(ModalMsg::NextTab));
        assert_tab(&model, SearchTab::All);
        update_ui(&mut model, UiMsg::Modal(ModalMsg::PrevTab));
        assert_tab(&model, SearchTab::Commands);
    }

    fn assert_tab(model: &AppModel, expected: SearchTab) {
        match &model.ui.active_modal {
            Some(ModalState::CommandPalette(state)) => assert_eq!(state.active_tab, expected),
            other => panic!("expected command palette modal, got {other:?}"),
        }
    }

    #[test]
    fn all_tab_confirm_executes_the_selected_command_and_records_history() {
        let mut model = AppModel::new(80, 60, 1.0, vec![]);
        update_ui(&mut model, UiMsg::ToggleModal(ModalId::CommandPalette));
        assert_tab(&model, SearchTab::All);

        let cmd = update_ui(&mut model, UiMsg::Modal(ModalMsg::Confirm));
        assert!(model.ui.active_modal.is_none());
        let batched_save = matches!(
            &cmd,
            Some(Cmd::Batch(cmds)) if cmds.iter().any(|c| matches!(c, Cmd::SaveCommandHistory { .. }))
        );
        assert!(
            batched_save,
            "confirming a command should batch Cmd::SaveCommandHistory, got {cmd:?}"
        );
        // NewFile (registry index 0) was executed — history now remembers it.
        assert!(model.command_history.recency_score(CommandId::NewFile) > 0);
    }

    /// Regression for the test-coverage gap overlay-surface.md Phase 4
    /// flags: the pre-existing confirm-order test never ran with a
    /// non-empty `CommandHistory` (a "Recently Used" section) or a Files
    /// group, so neither the recents offset nor the All tab's per-group
    /// cap offset into the second group was covered by a
    /// view-order == confirm-order assertion.
    #[test]
    fn all_tab_confirm_order_matches_view_order_with_recents_and_a_files_group() {
        let mut model = AppModel::new(80, 60, 1.0, vec![]);
        model.command_history.record_execution(CommandId::SaveFile);
        model.command_history.record_execution(CommandId::GotoLine);

        update_ui(&mut model, UiMsg::ToggleModal(ModalId::CommandPalette));
        assert_tab(&model, SearchTab::All);

        // Inject a Files group directly (bypassing real workspace file
        // indexing, which isn't under test here) so the All tab's merged
        // view has both a "Recently Used" offset *and* a Files group at the
        // Commands-cap boundary.
        let expected_path = std::path::PathBuf::from("/test/beta.rs");
        match &mut model.ui.active_modal {
            Some(ModalState::CommandPalette(state)) => {
                state.files_available = true;
                let mut files =
                    crate::model::FileFinderState::new(vec![], std::path::PathBuf::from("/test"));
                files.results = vec![
                    crate::model::FileMatch {
                        path: std::path::PathBuf::from("/test/alpha.rs"),
                        filename: "alpha.rs".to_string(),
                        relative_path: "alpha.rs".to_string(),
                        score: 0,
                        indices: Vec::new(),
                    },
                    crate::model::FileMatch {
                        path: expected_path.clone(),
                        filename: "beta.rs".to_string(),
                        relative_path: "beta.rs".to_string(),
                        score: 0,
                        indices: Vec::new(),
                    },
                ];
                state.files = Some(files);
                // Flat index `ALL_TAB_GROUP_CAP + 1`: past the (recents +
                // commands) group, at the second file in the Files group —
                // exactly the offset-into-second-group case the old test
                // never reached.
                state.all_selected = super::ALL_TAB_GROUP_CAP + 1;
            }
            other => panic!("expected command palette modal, got {other:?}"),
        }

        // The view slices `state.matches`/Files results using the same
        // `search_everywhere_sections` boundaries Confirm indexes into —
        // assert the row at `all_selected` really is `beta.rs` before
        // confirming, so this test fails loudly if the two ever drift.
        {
            let state = match &model.ui.active_modal {
                Some(ModalState::CommandPalette(state)) => state,
                other => panic!("expected command palette modal, got {other:?}"),
            };
            let sections = search_everywhere_sections(state);
            let commands_cap: usize = sections
                .iter()
                .filter(|(title, _)| *title != Some("Files"))
                .map(|(_, len)| len)
                .sum();
            assert_eq!(commands_cap, super::ALL_TAB_GROUP_CAP);
            let files = state.files.as_ref().unwrap();
            assert_eq!(
                files.results[state.all_selected - commands_cap].path,
                expected_path
            );
        }

        update_ui(&mut model, UiMsg::Modal(ModalMsg::Confirm));
        assert!(model.ui.active_modal.is_none(), "palette closes on confirm");

        let status = match model
            .ui
            .status_bar
            .get_segment(crate::model::SegmentId::StatusMessage)
        {
            Some(crate::model::StatusSegment {
                content: crate::model::SegmentContent::Text(text),
                ..
            }) => text.clone(),
            other => panic!("expected a status message, got {other:?}"),
        };
        assert!(
            status.contains("beta.rs"),
            "Confirm should have opened the row selected in the cached view order \
             (beta.rs, index ALL_TAB_GROUP_CAP+1), got status {status:?}"
        );
    }

    #[test]
    fn toggle_pin_on_commands_tab_pins_selected_command() {
        let mut model = AppModel::new(80, 60, 1.0, vec![]);
        let state = CommandPaletteState {
            active_tab: SearchTab::Commands,
            ..Default::default()
        };
        model.ui.open_modal(ModalState::CommandPalette(state));

        update_ui(&mut model, UiMsg::Modal(ModalMsg::TogglePin));

        let first_id = match &model.ui.active_modal {
            Some(ModalState::CommandPalette(state)) => state.matches[0].def.id,
            other => panic!("expected command palette modal, got {other:?}"),
        };
        assert!(model.command_history.is_pinned(first_id));
    }

    /// Build a real tempdir workspace with a couple of files matching one
    /// query and one that doesn't, for the Files-tab/All-tab regression
    /// tests below.
    fn workspace_model_with_query(query: &str) -> (AppModel, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("alpha.rs"), "").unwrap();
        std::fs::write(dir.path().join("beta.rs"), "").unwrap();
        std::fs::write(dir.path().join("gamma.txt"), "").unwrap();

        let mut model = AppModel::new(80, 60, 1.0, vec![]);
        model.open_workspace(dir.path().to_path_buf());
        update_ui(&mut model, UiMsg::ToggleModal(ModalId::CommandPalette));
        update_ui(&mut model, UiMsg::Modal(ModalMsg::SetInput(query.to_owned())));
        (model, dir)
    }

    #[test]
    fn switching_to_files_tab_seeds_it_with_the_shared_query() {
        let (mut model, _dir) = workspace_model_with_query("alpha");
        update_ui(&mut model, UiMsg::Modal(ModalMsg::NextTab)); // All -> Commands
        update_ui(&mut model, UiMsg::Modal(ModalMsg::NextTab)); // Commands -> Files
        assert_tab(&model, SearchTab::Files);

        match &model.ui.active_modal {
            Some(ModalState::CommandPalette(state)) => {
                let files = state.files.as_ref().expect("files tab populated");
                assert_eq!(files.input(), "alpha");
                let names: Vec<&str> =
                    files.results.iter().map(|m| m.filename.as_str()).collect();
                assert_eq!(names, vec!["alpha.rs"]);
            }
            other => panic!("expected command palette modal, got {other:?}"),
        }
    }

    #[test]
    fn open_fuzzy_file_finder_seeds_files_tab_with_restored_query() {
        // `last_command_palette` is how a query round-trips across a close
        // + reopen (see the Commands/Files confirm paths that populate it);
        // simulate a restored session with a query already typed.
        let (model, _dir) = workspace_model_with_query("alpha");
        let mut model = model;
        let mut restored = match &model.ui.active_modal {
            Some(ModalState::CommandPalette(state)) => state.clone(),
            other => panic!("expected command palette modal, got {other:?}"),
        };
        // Force the lazy-population path in `OpenFuzzyFileFinder` itself,
        // rather than trivially passing off the All tab's already-loaded
        // `files` from above.
        restored.files = None;
        model.ui.last_command_palette = Some(restored);
        model.ui.close_modal();

        update_ui(&mut model, UiMsg::OpenFuzzyFileFinder);
        assert_tab(&model, SearchTab::Files);
        match &model.ui.active_modal {
            Some(ModalState::CommandPalette(state)) => {
                let files = state.files.as_ref().expect("files tab populated");
                assert_eq!(files.input(), "alpha");
                assert_eq!(files.results.len(), 1);
            }
            other => panic!("expected command palette modal, got {other:?}"),
        }
    }

    #[test]
    fn all_tab_sections_are_empty_when_nothing_matches() {
        // No bare "Commands" header with zero rows underneath it — the
        // empty-state message in `view::modal` relies on `sections` being
        // genuinely empty to know when to render "No matches".
        let state = CommandPaletteState {
            active_tab: SearchTab::All,
            matches: Vec::new(),
            ..Default::default()
        };
        assert!(search_everywhere_sections(&state).is_empty());
    }

    #[test]
    fn all_tab_includes_matching_files_without_visiting_the_files_tab() {
        let (model, _dir) = workspace_model_with_query("alpha");
        assert_tab(&model, SearchTab::All);

        match &model.ui.active_modal {
            Some(ModalState::CommandPalette(state)) => {
                let files = state.files.as_ref().expect("All tab should eagerly load files");
                assert_eq!(files.results.len(), 1);
                let sections = search_everywhere_sections(state);
                assert!(
                    sections.iter().any(|&(title, len)| title == Some("Files") && len == 1),
                    "expected a Files group with 1 row, got {sections:?}"
                );
            }
            other => panic!("expected command palette modal, got {other:?}"),
        }
    }
}
