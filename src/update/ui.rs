//! UI message handlers (status bar, cursor blink, transient messages, modals)

use std::time::Duration;

use crate::commands::Cmd;
use crate::editable::{EditableState, StringBuffer};
use crate::messages::LayoutMsg;
use crate::messages::{ModalMsg, UiMsg};
use crate::model::ui::FindReplaceState;
use crate::model::{
    AppModel, CommandPaletteState, FileFinderState, GotoLineState, LanguagePickerState,
    LspServersState, ModalId, ModalState, RecentFilesState, SearchTab, SegmentContent, SegmentId,
    ThemePickerState, TransientMessage, COMMAND_PALETTE_MAX_VISIBLE,
};
use crate::syntax::LanguageId;
use crate::update::layout::update_layout;
use crate::update::lsp::toggle_lsp_server_enabled;
use crate::update::navigation::push_history;
use crate::update::syntax::update_syntax;
use crate::update::text_edits::{apply_planned_edits, EditCarets, PlannedEdit};
use crate::view::modal::{recent_files_groups, theme_picker_groups};
use crate::view::overlay_surface::{resolve_scroll_for_selection, SectionShape};

use super::app::execute_command;

/// Handle UI messages (status bar, cursor blink, modals)
pub(super) fn update_ui(model: &mut AppModel, msg: UiMsg) -> Option<Cmd> {
    match msg {
        UiMsg::PageDocumentation { forward } => model
            .ui
            .has_documentation()
            .then_some(Cmd::PageDocumentation { forward }),
        UiMsg::DocumentationScrolled(scroll) => {
            if !model.ui.has_documentation() {
                return None;
            }
            let documentation = &mut model.ui.cursor_overlay.as_mut()?.documentation;
            if documentation.scroll == scroll {
                return None;
            }
            documentation.scroll = scroll;
            Some(Cmd::Redraw)
        }
        UiMsg::ToggleDocumentation => {
            if !model.ui.has_documentation() {
                return None;
            }
            // Expansion changes track geometry; an old thumb capture is no longer valid.
            model.ui.scrollbar_drag = None;
            let documentation = &mut model.ui.cursor_overlay.as_mut()?.documentation;
            documentation.expanded = !documentation.expanded;
            Some(Cmd::Redraw)
        }
        UiMsg::OpenFind { replace } => open_find(model, replace),
        UiMsg::CloseFind => {
            if let Some(state) = model.ui.find_bar.take() {
                model.ui.last_find_replace = Some(state);
            }
            model.ui.find_selection_drag = None;
            if model.ui.focus == crate::model::FocusTarget::FindBar {
                model.ui.focus = crate::model::FocusTarget::Editor;
            }
            model.resync_viewports();
            Some(Cmd::Redraw)
        }
        UiMsg::ToggleFindReplaceMode => {
            model.ui.find_selection_drag = None;
            let state = model.ui.find_bar.as_mut()?;
            state.replace_mode = !state.replace_mode;
            state.focused_field = crate::model::FindReplaceField::Query;
            model.resync_viewports();
            Some(Cmd::Redraw)
        }
        UiMsg::FocusFindField(field) => {
            let state = model.ui.find_bar.as_mut()?;
            if field == crate::model::FindReplaceField::Replace && !state.replace_mode {
                return None;
            }
            state.focused_field = field;
            model.ui.focus = crate::model::FocusTarget::FindBar;
            model.ui.reset_cursor_blink();
            Some(Cmd::Redraw)
        }
        UiMsg::FindFieldPointer {
            field,
            column,
            extend,
            clicks,
        } => {
            if model.ui.has_modal() || model.find_bar_inset().is_none() {
                return None;
            }
            let state = model.ui.find_bar.as_mut()?;
            if field == crate::model::FindReplaceField::Replace && !state.replace_mode {
                return None;
            }
            state.focused_field = field;
            let input = state.focused_editable_mut();
            input.set_cursor_column(column, extend);
            match clicks {
                2 => input.select_word(),
                3.. => input.select_all(),
                _ => {}
            }
            model.ui.find_selection_drag = Some(field);
            model.ui.focus = crate::model::FocusTarget::FindBar;
            model.ui.reset_cursor_blink();
            Some(Cmd::Redraw)
        }
        UiMsg::EndFindSelection => {
            model.ui.find_selection_drag = None;
            None
        }
        UiMsg::FindSearchCompleted { request, result } => {
            let document_id = model.editor_area.focused_document_id()?;
            let document = model.editor_area.documents.get(&document_id)?;
            let Some(state) = &mut model.ui.find_bar else {
                return None;
            };
            state
                .finish_search(document, request, result)
                .then_some(Cmd::Redraw)
        }
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
            if id == SegmentId::StatusMessage {
                model.ui.status_message_is_diagnostic = false;
            }
            model.ui.status_bar.update_segment(id, content);
            Some(Cmd::redraw_status_bar())
        }

        UiMsg::SetTransientMessage { text, duration_ms } => {
            model
                .ui
                .set_status_for(text, Duration::from_millis(duration_ms));
            Some(Cmd::redraw_status_bar())
        }

        UiMsg::ClearTransientMessage => {
            model.ui.transient_message = None;
            model.ui.status_message_is_diagnostic = false;
            model
                .ui
                .status_bar
                .update_segment(SegmentId::StatusMessage, SegmentContent::Empty);
            Some(Cmd::redraw_status_bar())
        }

        UiMsg::Modal(modal_msg) => update_modal(model, modal_msg),
        UiMsg::Settings(msg) => super::settings::update_settings(model, msg),

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
                ModalId::UnsavedChanges => return None, // Opened only for a concrete close intention.
                ModalId::FileConflict => return super::file_change::show_focused(model, true),
                ModalId::Settings => {
                    let mut state = model
                        .ui
                        .suspended_settings
                        .take()
                        .unwrap_or_else(|| crate::settings::SettingsState::new(&model.config));
                    state.refresh_entries(&model.config);
                    ModalState::Settings(state)
                }
                ModalId::CommandPalette => {
                    // Cmd+Shift+A: Search Everywhere, pre-focused on All
                    // (overlay-surface.md Phase 4 "Bindings").
                    let mut state = model.ui.last_command_palette.clone().unwrap_or_default();
                    state.files_available = model.workspace.is_some();
                    resolve_palette_rows(&mut state, &model.command_history);
                    state.active_tab = SearchTab::All;
                    // Rebuild (not just lazily fill) on every open — the
                    // workspace/file tree may have changed since the index
                    // was last cached in `last_command_palette`.
                    state.files = build_file_finder_state(model, &state.input());
                    ModalState::CommandPalette(state)
                }
                ModalId::GotoLine => ModalState::GotoLine(GotoLineState::default()),
                // Needs the caret context captured at request time.
                ModalId::RenameSymbol => {
                    return super::lsp::update_lsp(model, crate::messages::LspMsg::RenameSymbol)
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
                ModalId::LspServers => ModalState::LspServers(LspServersState::default()),
                ModalId::LanguagePicker => {
                    let current = model
                        .editor_area
                        .focused_document()
                        .map_or(LanguageId::PlainText, |doc| doc.language);
                    ModalState::LanguagePicker(LanguagePickerState::new(current))
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
            // Rebuild on every open, see `ToggleModal`'s CommandPalette arm.
            state.files = build_file_finder_state(model, &state.input());
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
        UiMsg::ScrollbarTrackClicked {
            target,
            axis,
            new_position,
        } => scroll_target(model, target, axis, new_position),
        UiMsg::ScrollbarThumbPressed(drag) => {
            model.cancel_scroll_animations();
            model.ui.scrollbar_drag = Some(drag);
            Some(Cmd::Redraw)
        }
        UiMsg::ScrollbarDragUpdate { mouse_coord } => {
            let drag = model.ui.scrollbar_drag.as_ref()?;
            scroll_target(
                model,
                drag.target,
                drag.axis,
                drag.position_from_mouse(mouse_coord),
            )
        }
        UiMsg::ScrollbarDragEnd => model.ui.scrollbar_drag.take().map(|_| Cmd::Redraw),
    }
}

fn scroll_target(
    model: &mut AppModel,
    target: crate::model::ui::ScrollbarTarget,
    axis: crate::model::ui::ScrollbarDragAxis,
    position: usize,
) -> Option<Cmd> {
    use crate::model::ui::{ScrollbarDragAxis, ScrollbarTarget};
    match (target, axis) {
        (ScrollbarTarget::Editor(editor_id), axis) => {
            let editor = model.editor_area.editors.get(&editor_id)?;
            let (x, y) = editor.pixel_scroll_position();
            let (dx, dy) = match axis {
                ScrollbarDragAxis::Vertical => (0.0, position as f64 - y),
                ScrollbarDragAxis::Horizontal => (position as f64 - x, 0.0),
            };
            model
                .scroll_editor_pixels_by(editor_id, dx, dy, false)
                .then_some(Cmd::redraw_editor())
        }
        (ScrollbarTarget::Modal(id), ScrollbarDragAxis::Vertical)
            if model
                .ui
                .active_modal
                .as_ref()
                .is_some_and(|modal| modal.id() == id) =>
        {
            modal_scroll_to(model, Some(position), 0)
        }
        (ScrollbarTarget::Modal(_), _) => {
            model.ui.scrollbar_drag = None;
            None
        }
        (ScrollbarTarget::Documentation { kind, selected }, ScrollbarDragAxis::Vertical)
            if model.ui.has_documentation()
                && model.ui.cursor_overlay.is_some_and(|overlay| {
                    overlay.kind == kind && overlay.selected == selected
                }) =>
        {
            update_ui(model, UiMsg::DocumentationScrolled(position))
        }
        (ScrollbarTarget::Documentation { .. }, _) => {
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
        ModalState::Settings(state) => state.focused_input_mut(),
        ModalState::CommandPalette(state) => Some(&mut state.editable),
        ModalState::GotoLine(state) => Some(&mut state.editable),
        ModalState::RenameSymbol(state) => Some(&mut state.editable),
        ModalState::ThemePicker(_) => None,
        ModalState::FileFinder(state) => Some(&mut state.editable),
        ModalState::RecentFiles(state) => Some(&mut state.editable),
        ModalState::LspServers(_)
        | ModalState::LanguagePicker(_)
        | ModalState::FileConflict(_)
        | ModalState::UnsavedChanges(_) => None,
    }
}

/// Run the modal-specific side effect that should happen whenever a modal's
/// text input changes (insert/delete/cut/paste). `CommandPalette` and
/// `RecentFiles` reset their selected index back to the top of the list;
/// `FileFinder` refreshes its fuzzy-matched results. Other modal types have
/// no such side effect.
fn on_modal_input_changed(modal: &mut ModalState, history: &CommandHistory) {
    match modal {
        ModalState::Settings(state) => {
            if let Some(form) = &mut state.form {
                form.changed();
            } else {
                state.resolve_rows();
            }
        }
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
        ModalState::GotoLine(_)
        | ModalState::RenameSymbol(_)
        | ModalState::ThemePicker(_)
        | ModalState::LspServers(_)
        | ModalState::LanguagePicker(_)
        | ModalState::FileConflict(_)
        | ModalState::UnsavedChanges(_) => {}
    }
}

/// Shared editing path for modal inputs and the non-modal find fields.
fn edit_ui_input(model: &mut AppModel, msg: ModalMsg) -> Option<Cmd> {
    if let Some(ModalState::CommandPalette(state)) = &mut model.ui.active_modal {
        if state.input().is_empty() {
            match &msg {
                ModalMsg::InsertChar(ch) => {
                    if let Some(tab) =
                        search_tab_for_prefix(*ch).filter(|tab| state.tab_available(*tab))
                    {
                        state.active_tab = tab;
                        return Some(Cmd::Redraw);
                    }
                }
                ModalMsg::DeleteBackward => state.active_tab = SearchTab::All,
                _ => {}
            }
        }
    }
    let editing = matches!(
        msg,
        ModalMsg::SetInput(_)
            | ModalMsg::InsertChar(_)
            | ModalMsg::DeleteBackward
            | ModalMsg::DeleteForward
            | ModalMsg::DeleteWordBackward
            | ModalMsg::Cut
            | ModalMsg::PasteText(_)
    );
    let goto = matches!(model.ui.active_modal, Some(ModalState::GotoLine(_)));
    let input = if let Some(modal) = model.ui.active_modal.as_mut() {
        modal_editable_mut(modal)
    } else if model.ui.focus == crate::model::FocusTarget::FindBar {
        model
            .ui
            .find_bar
            .as_mut()
            .map(FindReplaceState::focused_editable_mut)
    } else {
        None
    }?;
    let mut effect = None;
    match msg {
        ModalMsg::SetInput(text) => input.set_content(&text),
        ModalMsg::InsertChar(ch) => {
            input.insert_char(ch);
        }
        ModalMsg::DeleteBackward => {
            input.delete_backward();
        }
        ModalMsg::DeleteForward => {
            input.delete_forward();
        }
        ModalMsg::DeleteWordBackward => {
            input.delete_word_backward();
        }
        ModalMsg::MoveCursorLeft => input.move_left(false),
        ModalMsg::MoveCursorRight => input.move_right(false),
        ModalMsg::MoveCursorHome => input.move_line_start(false),
        ModalMsg::MoveCursorEnd => input.move_line_end(false),
        ModalMsg::MoveCursorWordLeft => input.move_word_left(false),
        ModalMsg::MoveCursorWordRight => input.move_word_right(false),
        ModalMsg::MoveCursorLeftWithSelection => input.move_left(true),
        ModalMsg::MoveCursorRightWithSelection => input.move_right(true),
        ModalMsg::MoveCursorHomeWithSelection => input.move_line_start(true),
        ModalMsg::MoveCursorEndWithSelection => input.move_line_end(true),
        ModalMsg::MoveCursorWordLeftWithSelection => input.move_word_left(true),
        ModalMsg::MoveCursorWordRightWithSelection => input.move_word_right(true),
        ModalMsg::SelectAll => input.select_all(),
        ModalMsg::Copy | ModalMsg::Cut => {
            let text = input.selected_text();
            if !text.is_empty() {
                if editing {
                    input.delete_backward();
                }
                effect = Some(Cmd::CopyToClipboard(text));
            }
        }
        ModalMsg::Paste => effect = Some(Cmd::RequestClipboardPaste),
        ModalMsg::PasteText(text) => {
            let text = if input.constraints.allow_multiline {
                text.replace("\r\n", "\n").replace('\r', "\n")
            } else {
                text
            };
            let filtered: String = text
                .chars()
                .filter(|c| {
                    (input.constraints.allow_multiline || (*c != '\n' && *c != '\r'))
                        && (!goto || c.is_ascii_digit())
                })
                .collect();
            input.insert_text(&filtered);
        }
        _ => return None,
    }
    if editing {
        if let Some(modal) = &mut model.ui.active_modal {
            on_modal_input_changed(modal, &model.command_history);
        }
    }
    model.ui.reset_cursor_blink();
    super::merge_cmds(Some(Cmd::Redraw), effect)
}

/// Handle dialog and docked-find input messages.
fn update_modal(model: &mut AppModel, msg: ModalMsg) -> Option<Cmd> {
    // Changing the modal's query/category/selection invalidates captured geometry.
    model.ui.scrollbar_drag = None;
    if super::settings::capturing(model) {
        return match msg {
            ModalMsg::Close => super::settings::capture_action(model, 1),
            ModalMsg::Confirm => super::settings::capture_action(model, 0),
            ModalMsg::ChooseSetting { row: 0, choice } => {
                super::settings::capture_action(model, choice)
            }
            _ => Some(Cmd::Redraw),
        };
    }
    match msg {
        ModalMsg::OpenCommandPalette => {
            let mut state = model.ui.last_command_palette.clone().unwrap_or_default();
            state.files_available = model.workspace.is_some();
            resolve_palette_rows(&mut state, &model.command_history);
            state.active_tab = SearchTab::All;
            // Rebuild on every open, see `UiMsg::ToggleModal`'s CommandPalette arm.
            state.files = build_file_finder_state(model, &state.input());
            model.ui.open_modal(ModalState::CommandPalette(state));
            Some(Cmd::Redraw)
        }

        ModalMsg::OpenGotoLine => {
            model
                .ui
                .open_modal(ModalState::GotoLine(GotoLineState::default()));
            Some(Cmd::Redraw)
        }

        ModalMsg::OpenFindReplace => open_find(model, true),

        ModalMsg::Close => {
            if matches!(&model.ui.active_modal, Some(ModalState::Settings(state)) if state.form.is_some())
            {
                return super::settings::cancel_form(model);
            }
            if model.ui.focus == crate::model::FocusTarget::FindBar {
                return update_ui(model, UiMsg::CloseFind);
            }
            // Restore original theme if closing theme picker without confirming
            if let Some(ModalState::ThemePicker(state)) = &model.ui.active_modal {
                let id = state.original_theme_id.clone();
                model.ui.close_modal();
                return Some(Cmd::Batch(vec![
                    Cmd::LoadTheme { id, persist: false },
                    Cmd::Redraw,
                ]));
            }
            model.ui.close_modal();
            Some(Cmd::Redraw)
        }

        ModalMsg::MoveCursorLeft if matches!(&model.ui.active_modal, Some(ModalState::Settings(state)) if !state.editing_field()) =>
        {
            if matches!(&model.ui.active_modal, Some(ModalState::Settings(state)) if state.form.is_some())
            {
                return super::settings::adjust_form(model);
            }
            change_setting(model, None, -1)
        }
        ModalMsg::MoveCursorRight if matches!(&model.ui.active_modal, Some(ModalState::Settings(state)) if !state.editing_field()) =>
        {
            if matches!(&model.ui.active_modal, Some(ModalState::Settings(state)) if state.form.is_some())
            {
                return super::settings::adjust_form(model);
            }
            change_setting(model, None, 1)
        }
        input @ (ModalMsg::SetInput(_)
        | ModalMsg::InsertChar(_)
        | ModalMsg::DeleteBackward
        | ModalMsg::DeleteForward
        | ModalMsg::DeleteWordBackward
        | ModalMsg::MoveCursorLeft
        | ModalMsg::MoveCursorRight
        | ModalMsg::MoveCursorHome
        | ModalMsg::MoveCursorEnd
        | ModalMsg::MoveCursorWordLeft
        | ModalMsg::MoveCursorWordRight
        | ModalMsg::MoveCursorLeftWithSelection
        | ModalMsg::MoveCursorRightWithSelection
        | ModalMsg::MoveCursorHomeWithSelection
        | ModalMsg::MoveCursorEndWithSelection
        | ModalMsg::MoveCursorWordLeftWithSelection
        | ModalMsg::MoveCursorWordRightWithSelection
        | ModalMsg::SelectAll
        | ModalMsg::Copy
        | ModalMsg::Cut
        | ModalMsg::Paste
        | ModalMsg::PasteText(_)) => edit_ui_input(model, input),

        ModalMsg::SelectPrevious if matches!(&model.ui.active_modal, Some(ModalState::Settings(state)) if state.editing_field()) => {
            super::settings::update_settings(
                model,
                crate::messages::SettingsMsg::MoveFieldCursor {
                    down: false,
                    extend: false,
                },
            )
        }
        ModalMsg::SelectNext if matches!(&model.ui.active_modal, Some(ModalState::Settings(state)) if state.editing_field()) => {
            super::settings::update_settings(
                model,
                crate::messages::SettingsMsg::MoveFieldCursor {
                    down: true,
                    extend: false,
                },
            )
        }
        ModalMsg::SelectPrevious => modal_select(model, -1),

        ModalMsg::SelectNext => modal_select(model, 1),

        ModalMsg::PageUp => modal_page(model, false),

        ModalMsg::PageDown => modal_page(model, true),

        ModalMsg::Scroll(delta) => modal_scroll(model, delta),

        ModalMsg::ChooseSetting { row, choice } => {
            if let Some(ModalState::Settings(state)) = &mut model.ui.active_modal {
                if row >= state.rows.len() {
                    return None;
                }
                state.selected_index = row;
                return change_setting(model, Some(choice), 1);
            }
            None
        }
        ModalMsg::ActivateRow(row) => {
            if let Some(ref mut modal) = model.ui.active_modal {
                set_modal_selected_index(modal, row, &model.config.lsp);
                if matches!(modal, ModalState::Settings(_)) {
                    return super::settings::activate_row(model);
                }
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

        ModalMsg::NextTab if matches!(model.ui.active_modal, Some(ModalState::Settings(_))) => {
            if matches!(&model.ui.active_modal, Some(ModalState::Settings(state)) if state.form.is_some())
            {
                return super::settings::form_focus(model, true);
            }
            super::settings::switch_tab(model, None)
        }
        ModalMsg::NextTab => cycle_search_tab(model, true),

        ModalMsg::PrevTab if matches!(model.ui.active_modal, Some(ModalState::Settings(_))) => {
            if matches!(&model.ui.active_modal, Some(ModalState::Settings(state)) if state.form.is_some())
            {
                return super::settings::form_focus(model, false);
            }
            let Some(ModalState::Settings(state)) = &model.ui.active_modal else {
                return None;
            };
            let count = crate::settings::categories().len();
            let previous = (state.category + count - 1) % count;
            super::settings::switch_tab(model, Some(previous))
        }
        ModalMsg::PrevTab => cycle_search_tab(model, false),

        ModalMsg::ActivateTab(index)
            if matches!(model.ui.active_modal, Some(ModalState::Settings(_))) =>
        {
            super::settings::switch_tab(model, Some(index))
        }
        ModalMsg::ActivateTab(index) => activate_search_tab(model, index),

        ModalMsg::Confirm if model.ui.focus == crate::model::FocusTarget::FindBar => {
            update_modal(model, ModalMsg::FindNext)
        }
        ModalMsg::Confirm if matches!(&model.ui.active_modal, Some(ModalState::Settings(state)) if state.editing_field()) =>
        {
            let multiline = match &mut model.ui.active_modal {
                Some(ModalState::Settings(state)) => state
                    .focused_input_mut()
                    .is_some_and(|input| input.constraints.allow_multiline),
                _ => false,
            };
            if multiline {
                edit_ui_input(model, ModalMsg::InsertChar('\n'))
            } else {
                super::settings::form_focus(model, true)
            }
        }
        ModalMsg::Confirm => confirm_active_modal(model),

        ModalMsg::ToggleFindReplaceField => {
            if let Some(ref mut state) = model.ui.find_bar {
                if !state.replace_mode {
                    return None;
                }
                state.toggle_field();
                model.ui.reset_cursor_blink();
                Some(Cmd::Redraw)
            } else {
                None
            }
        }

        ModalMsg::ToggleFindReplaceCaseSensitive => {
            if let Some(ref mut state) = model.ui.find_bar {
                state.case_sensitive = !state.case_sensitive;
                Some(Cmd::Redraw)
            } else {
                None
            }
        }

        ModalMsg::ToggleFindReplaceWholeWord => {
            if let Some(ref mut state) = model.ui.find_bar {
                state.whole_word = !state.whole_word;
                Some(Cmd::Redraw)
            } else {
                None
            }
        }

        ModalMsg::ToggleFindReplaceRegex => {
            if let Some(ref mut state) = model.ui.find_bar {
                state.use_regex = !state.use_regex;
                Some(Cmd::Redraw)
            } else {
                None
            }
        }

        ModalMsg::ToggleFindReplaceSelectionOnly => {
            let selection = model.editor().selections[0];
            let mut state = model.ui.find_bar.take()?;
            let on = !state.selection_only;
            state.set_selection_only(on, model.document(), &selection);
            model.ui.find_bar = Some(state);
            Some(Cmd::Redraw)
        }

        ModalMsg::FindNext => {
            if let Some(ref state) = model.ui.find_bar {
                if !state.query().is_empty() {
                    let state = state.clone();
                    model.ui.last_find_replace = Some(state.clone());
                    return find_next_in_document(model, &state);
                }
            }
            Some(Cmd::Redraw)
        }

        ModalMsg::FindPrevious => {
            if let Some(ref state) = model.ui.find_bar {
                if !state.query().is_empty() {
                    let state = state.clone();
                    model.ui.last_find_replace = Some(state.clone());
                    return find_prev_in_document(model, &state);
                }
            }
            Some(Cmd::Redraw)
        }

        ModalMsg::ReplaceAndFindNext => {
            if let Some(ref state) = model.ui.find_bar {
                if !state.query().is_empty() {
                    let state = state.clone();
                    let replacement = state.replacement();
                    model.ui.last_find_replace = Some(state.clone());
                    return replace_and_find_next(model, &state, &replacement);
                }
            }
            Some(Cmd::Redraw)
        }

        ModalMsg::ReplaceAll => {
            if let Some(ref state) = model.ui.find_bar {
                if !state.query().is_empty() {
                    let state = state.clone();
                    let replacement = state.replacement();
                    model.ui.last_find_replace = Some(state.clone());
                    return replace_all(model, &state, &replacement);
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
/// skipping tabs without their required workspace/provider context.
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
                if matches!(candidate, SearchTab::Files | SearchTab::All) && state.files.is_none() {
                    if let Some((all_files, root)) = workspace_files {
                        state.files =
                            Some(seeded_file_finder_state(all_files, root, &state.input()));
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
                state.files = Some(seeded_file_finder_state(all_files, root, &state.input()));
            }
        }
        return Some(Cmd::Redraw);
    }
    None
}

/// Set the `FlatIndex`-space selected row for whichever list-body modal is
/// active — used by `ModalMsg::ActivateRow` (row click) ahead of confirming.
/// A no-op for `Fields`/no-list contexts.
fn set_modal_selected_index(
    modal: &mut ModalState,
    row: usize,
    lsp_config: &crate::config::LspConfig,
) {
    match modal {
        ModalState::UnsavedChanges(state) => {
            state.selected_index = row.min(state.actions().len() - 1)
        }
        ModalState::FileConflict(state) => {
            state.selected_index = row.min(state.actions().len() - 1)
        }
        ModalState::Settings(state) => state.selected_index = row.min(state.rows.len()),
        ModalState::CommandPalette(state) => match state.active_tab {
            SearchTab::Commands => state.selected_index = row.min(state.matches.len()),
            SearchTab::Files => {
                if let Some(files) = state.files.as_mut() {
                    files.selected_index = row.min(files.results.len());
                }
            }
            SearchTab::All => state.all_selected = row.min(all_tab_total(state)),
            SearchTab::Symbols => {
                state.symbols.selected_index = row.min(state.symbols.results.items.len())
            }
        },
        ModalState::ThemePicker(state) => state.selected_index = row.min(state.themes.len()),
        ModalState::FileFinder(state) => state.selected_index = row.min(state.results.len()),
        ModalState::RecentFiles(state) => state.selected_index = row.min(state.filtered_rows.len()),
        ModalState::LspServers(state) => {
            state.selected_index =
                row.min(crate::lsp::server_ids(lsp_config).len().saturating_sub(1))
        }
        ModalState::LanguagePicker(state) => {
            state.selected_index = row.min(LanguageId::all().count())
        }
        ModalState::GotoLine(_) | ModalState::RenameSymbol(_) => {}
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
            ModalState::UnsavedChanges(state) => super::closing::confirm(model, state),
            ModalState::FileConflict(state) => super::file_change::resolve(model, state),
            ModalState::CommandPalette(state) => confirm_search_everywhere(model, state),
            ModalState::Settings(state)
                if state.tab == crate::settings::keymap::SettingsTab::Keymap =>
            {
                super::settings::activate_row(model)
            }
            ModalState::Settings(_) => change_setting(model, None, 1),
            ModalState::RenameSymbol(state) => {
                model.ui.close_modal();
                let new_name = state.input();
                if new_name.is_empty() || new_name == state.placeholder {
                    return Some(Cmd::Redraw);
                }
                let doc = model.editor_area.documents.get(&state.document_id)?;
                Some(Cmd::Batch(vec![
                    Cmd::Redraw,
                    Cmd::LspRequestRename {
                        document_id: state.document_id,
                        position: crate::lsp::position_to_lsp(doc, state.position),
                        revision: state.revision,
                        new_name,
                    },
                ]))
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
                push_history(model);
                let editor = model.editor_mut();
                editor.cursors[0].line = clamped_line;
                editor.cursors[0].column = clamped_col;
                editor.clear_selection();
                model.ui.close_modal();
                model.ensure_cursor_visible();
                Some(Cmd::Redraw)
            }
            ModalState::ThemePicker(state) => {
                // Apply selected theme and save config
                if let Some(theme_info) = state.themes.get(state.selected_index) {
                    let id = theme_info.id.clone();
                    model.ui.close_modal();
                    return Some(Cmd::Batch(vec![
                        Cmd::LoadTheme { id, persist: true },
                        Cmd::Redraw,
                    ]));
                }
                model.ui.close_modal();
                Some(Cmd::Redraw)
            }
            ModalState::FileFinder(state) => {
                // Open selected file
                if let Some(file_match) = state.results.get(state.selected_index) {
                    let path = file_match.path.clone();
                    push_history(model);
                    model.ui.close_modal();
                    return update_layout(model, LayoutMsg::OpenFileInNewTab(path));
                }
                model.ui.close_modal();
                Some(Cmd::Redraw)
            }
            ModalState::RecentFiles(state) => {
                if let Some(entry) = state.selected_entry() {
                    let path = entry.path.clone();
                    push_history(model);
                    model.ui.close_modal();
                    return update_layout(model, LayoutMsg::OpenFileInNewTab(path));
                }
                model.ui.close_modal();
                Some(Cmd::Redraw)
            }
            // A management surface, not a picker: toggling a server's
            // enabled override neither selects nor closes anything, so
            // unlike every other Confirm arm above, the modal stays open
            // (Escape is the only way out).
            ModalState::LspServers(state) => {
                let server_id = crate::lsp::server_ids(&model.config.lsp)
                    .get(state.selected_index)
                    .map(|id| (*id).to_owned());
                match server_id.and_then(|id| toggle_lsp_server_enabled(model, &id)) {
                    Some(cmd) => Some(cmd),
                    None => Some(Cmd::Redraw),
                }
            }
            // Pick-apply-close like the Theme Picker; picking the current
            // language is a plain close (no pin, no reparse).
            ModalState::LanguagePicker(state) => {
                model.ui.close_modal();
                let picked = LanguageId::all().nth(state.selected_index);
                let doc = model.editor_area.focused_document_mut();
                let (Some(language), Some(doc)) = (picked, doc) else {
                    return Some(Cmd::Redraw);
                };
                if language == doc.language {
                    return Some(Cmd::Redraw);
                }
                doc.language_pinned = true;
                let document_id = doc.id?;
                let mut cmds = vec![Cmd::Redraw];
                cmds.extend(update_syntax(
                    model,
                    crate::messages::SyntaxMsg::LanguageChanged {
                        document_id,
                        language,
                    },
                ));
                Some(Cmd::Batch(cmds))
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
            let symbols_cap = state.symbols.results.items.len().min(ALL_TAB_GROUP_CAP);
            if symbols_cap > 0 {
                sections.push((Some("Symbols"), symbols_cap));
            }
            sections
        }
        SearchTab::Symbols => {
            let count = state.symbols.results.items.len();
            if count == 0 {
                Vec::new()
            } else {
                vec![(None, count)]
            }
        }
    }
}

/// Total selectable rows on the All tab: capped Commands, Files and Symbols.
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
                Some(path) => {
                    push_history(model);
                    update_layout(model, LayoutMsg::OpenFileInNewTab(path))
                }
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
            let files_shown = state
                .files
                .as_ref()
                .map_or(0, |files| files.results.len().min(ALL_TAB_GROUP_CAP));
            if state.all_selected >= commands_shown + files_shown {
                let symbol = state
                    .symbols
                    .results
                    .items
                    .get(state.all_selected - commands_shown - files_shown)
                    .cloned();
                return match symbol {
                    Some(symbol) => super::workspace_symbols::open(model, symbol),
                    None => Some(Cmd::Redraw),
                };
            }
            let path = state
                .files
                .as_ref()
                .and_then(|f| f.results.get(state.all_selected - commands_shown))
                .map(|m| m.path.clone());
            model.ui.close_modal();
            match path {
                Some(path) => {
                    push_history(model);
                    update_layout(model, LayoutMsg::OpenFileInNewTab(path))
                }
                None => Some(Cmd::Redraw),
            }
        }
        SearchTab::Symbols => {
            let symbol = state
                .symbols
                .results
                .items
                .get(state.symbols.selected_index)
                .cloned();
            match symbol {
                Some(symbol) => super::workspace_symbols::open(model, symbol),
                None => Some(Cmd::Redraw),
            }
        }
    }
}

/// Apply a preset from the settings list's authoritative filtered order.
fn change_setting(model: &mut AppModel, explicit: Option<usize>, delta: isize) -> Option<Cmd> {
    let Some(ModalState::Settings(state)) = &model.ui.active_modal else {
        return None;
    };
    let row = &state.entries[*state.rows.get(state.selected_index)?];
    if state.form.is_some() {
        return super::settings::form_choice(model, explicit, delta);
    }
    if state.tab == crate::settings::keymap::SettingsTab::Keymap {
        return super::settings::choose_base(model, explicit, delta);
    }
    let choices = row.choices();
    if choices.is_empty() || explicit.is_some_and(|choice| choice >= choices.len()) {
        return None;
    }
    let choice = explicit.unwrap_or_else(|| {
        row.active(&model.config)
            .map_or(if delta < 0 { choices.len() - 1 } else { 0 }, |active| {
                (active as isize + delta).rem_euclid(choices.len() as isize) as usize
            })
    });
    if row.active(&model.config) == Some(choice) {
        return Some(Cmd::Redraw);
    }
    let descriptor = match row.kind.clone() {
        crate::settings::RowKind::AddServer => return super::settings::open_server(model, None),
        crate::settings::RowKind::AddProvider => {
            return super::settings::open_provider(model, None)
        }
        crate::settings::RowKind::Provider(id) => {
            return super::settings::open_provider(model, Some(&id))
        }
        crate::settings::RowKind::LspMaster => {
            let effect = super::lsp::toggle_lsp_enabled(model);
            return super::merge_cmds(effect, Some(Cmd::Redraw));
        }
        crate::settings::RowKind::ServerEnabled(id) => {
            return super::lsp::toggle_lsp_server_enabled(model, &id)
        }
        crate::settings::RowKind::Preset(index) => &crate::settings::DESCRIPTORS[index],
        crate::settings::RowKind::ServerCommand(id) => {
            return super::settings::open_server(model, Some(&id))
        }
        crate::settings::RowKind::FormField(_)
        | crate::settings::RowKind::FormChoice(_)
        | crate::settings::RowKind::FormEnabled
        | crate::settings::RowKind::FormActions
        | crate::settings::RowKind::FormInfo => return None,
        crate::settings::RowKind::ServerStatus(_)
        | crate::settings::RowKind::KeymapBase
        | crate::settings::RowKind::KeymapBinding(..)
        | crate::settings::RowKind::CaptureActions => return None,
    };
    if descriptor.setting == crate::settings::Setting::Theme {
        return update_ui(model, UiMsg::ToggleModal(ModalId::ThemePicker));
    }
    if !descriptor.apply(&mut model.config, choice) {
        return Some(Cmd::Redraw);
    }
    model.ui.cursor_visible = true;
    let mut commands = vec![
        Cmd::SaveConfiguration {
            config: Box::new(model.config.clone()),
        },
        Cmd::Redraw,
    ];
    if descriptor.setting == crate::settings::Setting::StatusFont {
        commands.push(Cmd::SyncFontMetrics);
    }
    Some(Cmd::Batch(commands))
}

fn settings_capacity(model: &AppModel) -> usize {
    crate::view::overlay_surface::settings_visible_count(
        model.window_size.0 as usize,
        model.window_size.1 as usize,
        model.metrics.scale_factor,
    )
}

pub(super) fn reveal_settings_selection(model: &mut AppModel) {
    let Some((viewport, positions)) = settings_scroll_geometry(model) else {
        return;
    };
    if let Some(ModalState::Settings(state)) = &mut model.ui.active_modal {
        state.scroll_offset_px = viewport.scroll_to_reveal_range_pixels(
            positions.get(state.selected_index).cloned().unwrap_or(0..1),
        );
    }
}

fn settings_scroll_geometry(
    model: &AppModel,
) -> Option<(crate::layout::RowListView, Vec<std::ops::Range<usize>>)> {
    let ModalState::Settings(_) = model.ui.active_modal.as_ref()? else {
        return None;
    };
    crate::view::modal::with_modal_overlay_layout(
        model,
        model.window_size.0 as usize,
        model.window_size.1 as usize,
        model.metrics.scale_factor,
        |_, layout| {
            layout
                .settings_viewport
                .map(|viewport| (viewport, layout.settings_positions.clone()))
        },
    )
    .flatten()
}

/// Section shapes for an untitled, single-section list body.
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

/// Section shapes for the Language Servers picker: a single, untitled
/// section (like the Command Palette/File Finder) sized from the same
/// config-aware registry as painting and activation.
fn lsp_servers_shapes(config: &crate::config::LspConfig) -> [SectionShape; 1] {
    flat_shapes(crate::lsp::server_ids(config).len())
}

/// Section shapes for the "Set Language..." picker: one flat section over
/// the static language registry.
fn language_picker_shapes() -> [SectionShape; 1] {
    flat_shapes(LanguageId::all().count())
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
    *selected = offset_selection(*selected, total, delta);
    *scroll = resolve_scroll_for_selection(shapes, *selected, COMMAND_PALETTE_MAX_VISIBLE, *scroll);
}

fn offset_selection(selected: usize, total: usize, delta: isize) -> usize {
    if total == 0 {
        return 0;
    }
    (selected as isize + delta).rem_euclid(total as isize) as usize
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
        SearchTab::Symbols => {
            let shapes = flat_shapes(state.symbols.results.items.len());
            move_list_selection(
                &mut state.symbols.selected_index,
                &mut state.symbols.scroll_offset,
                &shapes,
                delta,
            );
        }
    }
}

/// `ModalMsg::SelectPrevious`/`SelectNext`: move selection by `delta`
/// (-1/+1) in whichever list-body modal is active. Theme Picker previews
/// the newly-selected theme live.
fn modal_select(model: &mut AppModel, delta: isize) -> Option<Cmd> {
    let settings_geometry = settings_scroll_geometry(model);
    let modal = model.ui.active_modal.as_mut()?;
    let preview_theme_id = match modal {
        ModalState::Settings(state) => {
            state.selected_index = offset_selection(state.selected_index, state.rows.len(), delta);
            if let Some((viewport, positions)) = settings_geometry {
                state.scroll_offset_px = viewport.scroll_to_reveal_range_pixels(
                    positions.get(state.selected_index).cloned().unwrap_or(0..1),
                );
            }
            None
        }
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
        ModalState::LspServers(state) => {
            let shapes = lsp_servers_shapes(&model.config.lsp);
            move_list_selection(
                &mut state.selected_index,
                &mut state.scroll_offset,
                &shapes,
                delta,
            );
            None
        }
        ModalState::LanguagePicker(state) => {
            let shapes = language_picker_shapes();
            move_list_selection(
                &mut state.selected_index,
                &mut state.scroll_offset,
                &shapes,
                delta,
            );
            None
        }
        ModalState::UnsavedChanges(state) => {
            state.selected_index =
                offset_selection(state.selected_index, state.actions().len(), delta);
            None
        }
        ModalState::FileConflict(state) => {
            state.selected_index =
                offset_selection(state.selected_index, state.actions().len(), delta);
            None
        }
        ModalState::GotoLine(_) | ModalState::RenameSymbol(_) => None,
    };
    if let Some(theme_id) = preview_theme_id {
        return Some(Cmd::Batch(vec![
            Cmd::LoadTheme {
                id: theme_id,
                persist: false,
            },
            Cmd::Redraw,
        ]));
    }
    Some(Cmd::Redraw)
}

/// `ModalMsg::PageUp`/`PageDown`: page selection by a full visible page in
/// whichever list-body modal is active.
fn modal_page(model: &mut AppModel, forward: bool) -> Option<Cmd> {
    if matches!(&model.ui.active_modal, Some(ModalState::Settings(state)) if state.editing_field())
    {
        return super::settings::page_field(model, forward);
    }
    let capacity = settings_capacity(model);
    let settings_geometry = settings_scroll_geometry(model);
    let modal = model.ui.active_modal.as_mut()?;
    match modal {
        ModalState::Settings(state) => {
            state.selected_index = if forward {
                state
                    .selected_index
                    .saturating_add(capacity)
                    .min(state.rows.len().saturating_sub(1))
            } else {
                state.selected_index.saturating_sub(capacity)
            };
            if let Some((viewport, positions)) = settings_geometry {
                state.scroll_offset_px = viewport.scroll_to_reveal_range_pixels(
                    positions.get(state.selected_index).cloned().unwrap_or(0..1),
                );
            }
        }
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
            // All is non-scrolling, but paging still jumps the selection
            // (it's the default landing tab — dead PageUp/Down reads as
            // broken keys). Clamped, not wrapped, like the other tabs.
            SearchTab::All => {
                let total = all_tab_total(state);
                if total > 0 {
                    state.all_selected = if forward {
                        (state.all_selected + COMMAND_PALETTE_MAX_VISIBLE).min(total - 1)
                    } else {
                        state
                            .all_selected
                            .saturating_sub(COMMAND_PALETTE_MAX_VISIBLE)
                    };
                }
            }
            SearchTab::Symbols => {
                let shapes = flat_shapes(state.symbols.results.items.len());
                page_list_selection(
                    &mut state.symbols.selected_index,
                    &mut state.symbols.scroll_offset,
                    &shapes,
                    forward,
                );
            }
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
        ModalState::LspServers(state) => {
            let shapes = lsp_servers_shapes(&model.config.lsp);
            page_list_selection(
                &mut state.selected_index,
                &mut state.scroll_offset,
                &shapes,
                forward,
            );
        }
        ModalState::LanguagePicker(state) => {
            let shapes = language_picker_shapes();
            page_list_selection(
                &mut state.selected_index,
                &mut state.scroll_offset,
                &shapes,
                forward,
            );
        }
        ModalState::UnsavedChanges(state) => {
            state.selected_index = if forward {
                state.actions().len() - 1
            } else {
                0
            };
        }
        ModalState::FileConflict(state) => {
            state.selected_index = if forward {
                state.actions().len() - 1
            } else {
                0
            };
        }
        ModalState::GotoLine(_) | ModalState::RenameSymbol(_) => {}
    }
    Some(Cmd::Redraw)
}

/// `ModalMsg::Scroll`: move the visible window by `delta` rows without
/// moving selection (mouse wheel over a list-body modal).
fn modal_scroll(model: &mut AppModel, delta: isize) -> Option<Cmd> {
    modal_scroll_to(model, None, delta)
}

fn modal_scroll_to(model: &mut AppModel, position: Option<usize>, delta: isize) -> Option<Cmd> {
    if let Some((viewport, _)) = settings_scroll_geometry(model) {
        let ModalState::Settings(state) = model.ui.active_modal.as_mut()? else {
            return None;
        };
        let offset = position
            .unwrap_or_else(|| viewport.scroll_offset_pixels().saturating_add_signed(delta))
            .min(viewport.max_scroll_pixels());
        if offset == state.scroll_offset_px {
            return None;
        }
        state.scroll_offset_px = offset;
        return Some(Cmd::Redraw);
    }
    let capacity = COMMAND_PALETTE_MAX_VISIBLE;
    let modal = model.ui.active_modal.as_mut()?;
    let (scroll, shapes): (&mut usize, Vec<SectionShape>) = match modal {
        ModalState::Settings(_) | ModalState::FileConflict(_) | ModalState::UnsavedChanges(_) => {
            return None
        }
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
            SearchTab::All => return None,
            SearchTab::Symbols => (
                &mut state.symbols.scroll_offset,
                flat_shapes(state.symbols.results.items.len()).to_vec(),
            ),
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
        ModalState::LspServers(state) => (
            &mut state.scroll_offset,
            lsp_servers_shapes(&model.config.lsp).to_vec(),
        ),
        ModalState::LanguagePicker(state) => {
            (&mut state.scroll_offset, language_picker_shapes().to_vec())
        }
        ModalState::GotoLine(_) | ModalState::RenameSymbol(_) => return None,
    };
    let total: usize = shapes.iter().map(|s| s.len).sum();
    if total == 0 {
        return None;
    }
    let new_scroll = if let Some(position) = position {
        crate::view::overlay_surface::resolve_scroll_for_display(&shapes, position, capacity)
    } else {
        let max_scroll = resolve_scroll_for_selection(&shapes, total - 1, capacity, 0);
        scroll.saturating_add_signed(delta).min(max_scroll)
    };
    if new_scroll == *scroll {
        return None;
    }
    *scroll = new_scroll;
    Some(Cmd::Redraw)
}

/// The remembered find state, with its selection scope re-captured from
/// the live selection: a scope from a previous session would point at
/// stale offsets, and an empty selection cannot scope anything.
fn reopened_find_replace(model: &AppModel) -> FindReplaceState {
    let mut state = model.ui.last_find_replace.clone().unwrap_or_default();
    state.reset_search_session();
    state.document_id = model.editor_area.focused_document_id();
    if state.selection_only {
        let selection = model.editor().selections[0];
        state.set_selection_only(true, model.document(), &selection);
    }
    state
}

fn open_find(model: &mut AppModel, replace: bool) -> Option<Cmd> {
    if !model.editor_area.focused_editor()?.is_plain_text_mode() {
        return None;
    }
    let selection = model.editor().selections[0];
    let mut state = model
        .ui
        .find_bar
        .take()
        .unwrap_or_else(|| reopened_find_replace(model));
    bind_find_document(&mut state, model.editor_area.focused_document_id());
    state.replace_mode = replace;
    state.focused_field = crate::model::FindReplaceField::Query;
    if !selection.is_empty() && !state.selection_only {
        let start = selection.start();
        let end = selection.end();
        if start.line == end.line {
            let doc = model.document();
            let text = doc
                .buffer
                .slice(
                    doc.cursor_to_offset(start.line, start.column)
                        ..doc.cursor_to_offset(end.line, end.column),
                )
                .to_string();
            state.set_query(&text);
        }
    }
    state.query_editable.select_all();
    model.ui.open_find(state);
    model.cancel_scroll_animations();
    model.resync_viewports();
    Some(Cmd::Redraw)
}

fn bind_find_document(state: &mut FindReplaceState, document_id: Option<crate::model::DocumentId>) {
    if state.document_id.is_some() && state.document_id != document_id {
        state.scope = None;
        state.selection_only = false;
        state.reset_search_session();
    }
    state.document_id = document_id;
}

/// Schedule once after any update, including edits and focus/tab changes. Display
/// readers never schedule effects or fall back to a large synchronous scan.
pub(super) fn schedule_find_search(model: &mut AppModel) -> Option<Cmd> {
    let editor = model.editor_area.focused_editor()?;
    if !editor.is_plain_text_mode() {
        return None;
    }
    let document = model.editor_area.documents.get(&editor.document_id?)?;
    let Some(state) = &mut model.ui.find_bar else {
        return None;
    };
    bind_find_document(state, document.id);
    state.prepare_search(document).map(Cmd::RunFindSearch)
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
fn find_next_in_document(model: &mut AppModel, state: &FindReplaceState) -> Option<Cmd> {
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

    find_next_from(model, state, start_offset, false)
}

/// Replacements include an adjacent match starting exactly at the new caret;
/// explicit Find Next retains its existing strictly-after navigation policy.
fn find_next_from(
    model: &mut AppModel,
    state: &FindReplaceState,
    start_offset: usize,
    inclusive: bool,
) -> Option<Cmd> {
    let doc = model.document();
    let matches = state.matches(doc);
    let found = matches
        .iter()
        .find(|m| m.start > start_offset || (inclusive && m.start == start_offset))
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
        report_no_matches(model, &state.build_query());
        Some(Cmd::redraw_editor())
    }
}

/// Find previous occurrence in the document and select it
fn find_prev_in_document(model: &mut AppModel, state: &FindReplaceState) -> Option<Cmd> {
    let query = state.build_query();
    let query = &query;
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

    let matches = state.matches(doc);
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
    state: &FindReplaceState,
    replacement: &str,
) -> Option<Cmd> {
    if !model.editor().is_plain_text_mode() {
        return None;
    }
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

            let is_match = state
                .matches(doc)
                .iter()
                .any(|m| m.start == start_offset && m.end == end_offset);

            is_match.then_some((start_offset, end_offset))
        }
    };

    let Some((start_offset, end_offset)) = should_replace else {
        return find_next_in_document(model, state);
    };
    let edit = PlannedEdit {
        start: start_offset,
        deleted: model
            .document()
            .buffer
            .slice(start_offset..end_offset)
            .to_string(),
        inserted: replacement.to_owned(),
    };
    let new_offset = start_offset + replacement.chars().count();
    let effects = apply_find_edits(model, vec![edit], new_offset);

    // The shared mapper has updated the active scope. Do not search the stale
    // clone captured before a length-changing replacement.
    let mut next_state = state.clone();
    if let Some(active) = &model.ui.find_bar {
        next_state.scope = active.scope;
    }
    let next_cmd = find_next_from(model, &next_state, new_offset, true);
    super::merge_cmds(effects, next_cmd)
}

/// Replace all occurrences
fn replace_all(model: &mut AppModel, state: &FindReplaceState, replacement: &str) -> Option<Cmd> {
    if !model.editor().is_plain_text_mode() {
        return None;
    }
    let doc = model.document();
    let occurrences = state.matches(doc);

    if occurrences.is_empty() {
        report_no_matches(model, &state.build_query());
        return Some(Cmd::Redraw);
    }

    let count = occurrences.len();

    let planned = occurrences
        .iter()
        .rev()
        .map(|m| PlannedEdit {
            start: m.start,
            deleted: doc.buffer.slice(m.start..m.end).to_string(),
            inserted: replacement.to_owned(),
        })
        .collect();
    // Earlier matches do not exist, so later replacements cannot shift this
    // offset. Derive its actual line/column through the shared placement policy.
    let first_end = occurrences[0].start + replacement.chars().count();
    let effects = apply_find_edits(model, planned, first_end);

    model.ui.transient_message = Some(TransientMessage::new(
        format!("Replaced {} occurrences", count),
        Duration::from_secs(2),
    ));

    effects
}

/// Find owns only the primary caret's final placement. Other carets, selections,
/// the live search scope, history and effects use the shared transaction.
fn apply_find_edits(
    model: &mut AppModel,
    mut planned: Vec<PlannedEdit>,
    caret: usize,
) -> Option<Cmd> {
    planned.retain(|edit| edit.deleted != edit.inserted);
    if planned.is_empty() {
        return Some(Cmd::redraw_editor());
    }
    let document_id = model.editor_area.focused_document_id()?;
    let editor_id = model.editor_area.focused_editor_id()?;
    model.reset_cursor_blink();
    apply_planned_edits(
        model,
        document_id,
        &planned,
        EditCarets::Place {
            editor_id,
            offsets: &[caret],
            before: None,
        },
    )
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
    let all: Vec<&'static CommandDef> = crate::commands::all_commands().collect();

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
fn seeded_file_finder_state(
    all_files: Vec<PathBuf>,
    root: PathBuf,
    query: &str,
) -> FileFinderState {
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
        get_current_cursor_lines, replace_all, replace_and_find_next, resolve_palette_rows,
        search_everywhere_sections, update_ui,
    };
    use crate::command_history::CommandHistory;
    use crate::commands::{Cmd, CommandId, DamageArea};
    use crate::image::ImageState;
    use crate::messages::{ModalMsg, UiMsg};
    use crate::model::ui::FindReplaceState;
    use crate::model::{AppModel, CommandPaletteState, ModalId, ModalState, SearchTab, ViewMode};

    #[test]
    fn current_cursor_lines_are_reported_for_plain_text_editors() {
        let mut model = AppModel::new(80, 60, 1.0);
        model.editor_mut().cursors[0].line = 7;

        assert_eq!(get_current_cursor_lines(&model), vec![7]);
    }

    #[test]
    fn current_cursor_lines_are_ignored_for_image_editors() {
        let mut model = AppModel::new(80, 60, 1.0);
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
        let mut model = AppModel::new(80, 60, 1.0);
        // Force update_cursor_blink to report a state change on the next call.
        // Zero now means Off, so use an elapsed positive interval.
        model.config.cursor_blink_ms = 1;
        model.ui.last_cursor_blink = std::time::Instant::now() - std::time::Duration::from_secs(1);

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
        let mut model = AppModel::new(80, 60, 1.0);
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
            crate::commands::all_commands().count(),
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
                .map(|d| d.id)
                .collect::<Vec<_>>()
        );
    }

    // ========================================================================
    // Search Everywhere: prefix routing, tab cycling, All tab (Phase 4)
    // ========================================================================

    #[test]
    fn insert_char_gt_on_empty_query_pins_commands_tab_and_is_consumed() {
        let mut model = AppModel::new(80, 60, 1.0);
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
        // With no workspace-symbols provider, Symbols is `Unavailable`.
        // `@` must not park the user on a dead tab; it falls
        // through to a literal char insert instead, same as any other
        // prefix routed to an `Unavailable` tab.
        let mut model = AppModel::new(80, 60, 1.0);
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
        let mut model = AppModel::new(80, 60, 1.0);
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
        let mut model = AppModel::new(80, 60, 1.0);
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
        let mut model = AppModel::new(80, 60, 1.0);
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

    fn palette_state(model: &AppModel) -> &CommandPaletteState {
        match &model.ui.active_modal {
            Some(ModalState::CommandPalette(state)) => state,
            other => panic!("expected command palette modal, got {other:?}"),
        }
    }

    #[test]
    fn page_down_moves_selection_on_the_all_tab() {
        // Regression: PageUp/Down were a no-op on the All tab — the default
        // landing tab — which read as broken keys.
        let mut model = AppModel::new(80, 60, 1.0);
        update_ui(&mut model, UiMsg::ToggleModal(ModalId::CommandPalette));
        assert_tab(&model, SearchTab::All);

        let before = palette_state(&model).all_selected;
        update_ui(&mut model, UiMsg::Modal(ModalMsg::PageDown));
        let after = palette_state(&model).all_selected;
        assert!(after > before, "PageDown must move the All-tab selection");

        update_ui(&mut model, UiMsg::Modal(ModalMsg::PageUp));
        assert_eq!(palette_state(&model).all_selected, before);
    }

    #[test]
    fn all_tab_confirm_executes_the_selected_command_and_records_history() {
        let mut model = AppModel::new(80, 60, 1.0);
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
        let mut model = AppModel::new(80, 60, 1.0);
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
        let mut model = AppModel::new(80, 60, 1.0);
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

        let mut model = AppModel::new(80, 60, 1.0);
        model.open_workspace(dir.path().to_path_buf());
        update_ui(&mut model, UiMsg::ToggleModal(ModalId::CommandPalette));
        update_ui(
            &mut model,
            UiMsg::Modal(ModalMsg::SetInput(query.to_owned())),
        );
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
                let names: Vec<&str> = files.results.iter().map(|m| m.filename.as_str()).collect();
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
    fn reopening_the_palette_picks_up_files_created_since_last_open() {
        // The Files/All-tab index used to populate once (`files.is_none()`
        // gate) and stick around in `last_command_palette` forever after —
        // creating a file and reopening the palette must see it, not the
        // stale cached list.
        let (mut model, dir) = workspace_model_with_query("delta");
        assert!(
            model
                .ui
                .active_modal
                .as_ref()
                .is_some_and(|m| matches!(m, ModalState::CommandPalette(s) if s.files.is_some())),
            "files tab should already be populated (All tab renders a Files group)"
        );
        // Cache the (now stale) state the way a real close does, and close.
        model.ui.last_command_palette = match &model.ui.active_modal {
            Some(ModalState::CommandPalette(state)) => Some(state.clone()),
            other => panic!("expected command palette modal, got {other:?}"),
        };
        model.ui.close_modal();

        std::fs::write(dir.path().join("delta.rs"), "").unwrap();
        // Mirrors what the fs-watcher-driven sidebar refresh does: rebuild
        // the workspace's file tree in place.
        model.open_workspace(dir.path().to_path_buf());

        update_ui(&mut model, UiMsg::ToggleModal(ModalId::CommandPalette));
        update_ui(
            &mut model,
            UiMsg::Modal(ModalMsg::SetInput("delta".to_owned())),
        );

        match &model.ui.active_modal {
            Some(ModalState::CommandPalette(state)) => {
                let files = state.files.as_ref().expect("files tab populated");
                let names: Vec<&str> = files.results.iter().map(|m| m.filename.as_str()).collect();
                assert_eq!(
                    names,
                    vec!["delta.rs"],
                    "reopening the palette must re-scan the workspace, not reuse a stale index"
                );
            }
            other => panic!("expected command palette modal, got {other:?}"),
        }
    }

    #[test]
    fn reopening_the_palette_after_closing_the_workspace_drops_stale_files() {
        let (mut model, _dir) = workspace_model_with_query("alpha");
        model.ui.last_command_palette = match &model.ui.active_modal {
            Some(ModalState::CommandPalette(state)) => Some(state.clone()),
            other => panic!("expected command palette modal, got {other:?}"),
        };
        model.ui.close_modal();
        model.close_workspace();

        update_ui(&mut model, UiMsg::ToggleModal(ModalId::CommandPalette));

        match &model.ui.active_modal {
            Some(ModalState::CommandPalette(state)) => {
                assert!(!state.files_available, "Files tab must be unavailable");
                assert!(
                    state.files.is_none(),
                    "no workspace means no stale file index to fall back on"
                );
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
                let files = state
                    .files
                    .as_ref()
                    .expect("All tab should eagerly load files");
                assert_eq!(files.results.len(), 1);
                let sections = search_everywhere_sections(state);
                assert!(
                    sections
                        .iter()
                        .any(|&(title, len)| title == Some("Files") && len == 1),
                    "expected a Files group with 1 row, got {sections:?}"
                );
            }
            other => panic!("expected command palette modal, got {other:?}"),
        }
    }

    /// Replace All mutates `doc.buffer`/`doc.revision` directly (bypassing
    /// the shared planned-edit transaction), so it must schedule its own
    /// syntax-parse and LSP didChange like every other mutation site — see
    /// docs/feature/lsp-integration.md's flush-before-request invariant.
    #[test]
    fn replace_all_schedules_syntax_parse_and_lsp_did_change() {
        let mut model = AppModel::new(80, 60, 1.0);
        model.document_mut().buffer = ropey::Rope::from_str("foo foo foo");
        let before_revision = model.document().revision;

        let mut state = FindReplaceState::default();
        state.set_query("foo");
        state.case_sensitive = true;
        let cmd = replace_all(&mut model, &state, "bar");

        assert_eq!(model.document().buffer.to_string(), "bar bar bar");
        assert!(model.document().revision > before_revision);

        assert_eq!(lsp_change_count(&cmd.expect("replacement effects")), 1);
    }

    fn lsp_change_count(cmd: &Cmd) -> usize {
        match cmd {
            Cmd::LspScheduleDidChange { .. } => 1,
            Cmd::Batch(cmds) => cmds.iter().map(lsp_change_count).sum(),
            _ => 0,
        }
    }

    #[test]
    fn replace_and_find_next_schedules_lsp_did_change_when_it_replaces() {
        let mut model = AppModel::new(80, 60, 1.0);
        model.document_mut().buffer = ropey::Rope::from_str("foo bar");
        model.editor_mut().selections[0].anchor.column = 0;
        model.editor_mut().selections[0].head.column = 3;
        let before_revision = model.document().revision;

        let mut state = FindReplaceState::default();
        state.set_query("foo");
        state.case_sensitive = true;
        let cmd = replace_and_find_next(&mut model, &state, "baz");

        assert_eq!(model.document().buffer.to_string(), "baz bar");
        assert!(model.document().revision > before_revision);

        assert_eq!(lsp_change_count(&cmd.expect("replacement effects")), 1);
    }

    #[test]
    fn escape_closes_the_lsp_servers_modal() {
        use crate::model::LspServersState;

        let mut model = AppModel::new(80, 60, 1.0);
        model
            .ui
            .open_modal(ModalState::LspServers(LspServersState::default()));

        update_ui(&mut model, UiMsg::Modal(ModalMsg::Close));

        assert!(model.ui.active_modal.is_none());
    }

    /// `EditorConfig::save()` has no explicit-path test seam (unlike
    /// `CommandHistory::save_to`), so it always resolves the real
    /// `~/.config/token-editor/config.yaml` — exercising `ModalMsg::Confirm`
    /// for real here would overwrite the developer's actual config file.
    /// Redirect `XDG_CONFIG_HOME` to a scratch dir for the duration of this
    /// one test instead, restoring it on drop.
    ///
    /// ponytail: process-global env mutation, not race-proof against other
    /// tests' concurrent runtime startup/`EditorConfig::load()` calls (a
    /// transient "no config file found" read falls back to defaults, which
    /// none of them assert against, so this is a correctness no-op for
    /// them) — upgrade to an injectable config path if this ever causes
    /// real flakiness.
    struct ScratchConfigHome {
        previous: Option<std::ffi::OsString>,
        _dir: tempfile::TempDir,
    }

    impl ScratchConfigHome {
        fn new() -> Self {
            let dir = tempfile::tempdir().unwrap();
            let previous = std::env::var_os("XDG_CONFIG_HOME");
            // SAFETY: test-only; restored by `Drop` before this guard's
            // scope ends.
            unsafe { std::env::set_var("XDG_CONFIG_HOME", dir.path()) };
            Self {
                previous,
                _dir: dir,
            }
        }
    }

    impl Drop for ScratchConfigHome {
        fn drop(&mut self) {
            // SAFETY: see `new()`.
            unsafe {
                match &self.previous {
                    Some(v) => std::env::set_var("XDG_CONFIG_HOME", v),
                    None => std::env::remove_var("XDG_CONFIG_HOME"),
                }
            }
        }
    }

    #[test]
    fn confirm_toggles_the_selected_server_persists_and_keeps_the_modal_open() {
        use crate::model::LspServersState;

        let _scratch = ScratchConfigHome::new();
        let mut model = AppModel::new(80, 60, 1.0);
        model
            .ui
            .open_modal(ModalState::LspServers(LspServersState::default()));

        let cmd = update_ui(&mut model, UiMsg::Modal(ModalMsg::Confirm));

        let first_id = crate::lsp::all_server_defs()[0].id;
        assert_eq!(
            model
                .config
                .lsp
                .servers
                .get(first_id)
                .and_then(|o| o.enabled),
            Some(false),
            "a default-enabled server toggles to disabled"
        );
        assert!(
            matches!(cmd, Some(Cmd::Batch(_))),
            "expected a batch of the runtime teardown cmd + redraw, got {cmd:?}"
        );
        assert!(
            matches!(model.ui.active_modal, Some(ModalState::LspServers(_))),
            "the servers picker is a management surface: it stays open after a toggle"
        );

        // Update is I/O-free. Execute only its save effect explicitly against
        // the scratch directory, as the runtime would.
        let saved = crate::config_paths::config_file().unwrap();
        assert!(!saved.exists());
        let Some(Cmd::Batch(cmds)) = cmd else {
            unreachable!()
        };
        let config = cmds
            .iter()
            .find_map(|cmd| match cmd {
                Cmd::SaveConfiguration { config } => Some(config),
                _ => None,
            })
            .expect("toggle emits a save effect");
        config.save().unwrap();
        let content = std::fs::read_to_string(saved).unwrap();
        let reloaded: crate::config::EditorConfig = serde_yaml::from_str(&content).unwrap();
        assert_eq!(reloaded.lsp.servers[first_id].enabled, Some(false));
    }

    #[test]
    fn opening_the_language_picker_preselects_the_current_language() {
        let mut model = AppModel::new(80, 60, 1.0);
        model.document_mut().language = crate::syntax::LanguageId::Rust;

        update_ui(&mut model, UiMsg::ToggleModal(ModalId::LanguagePicker));

        let Some(ModalState::LanguagePicker(state)) = model.ui.active_modal else {
            panic!("expected the language picker to open");
        };
        assert_eq!(
            crate::syntax::LanguageId::all().nth(state.selected_index),
            Some(crate::syntax::LanguageId::Rust)
        );
    }

    #[test]
    fn confirming_a_different_language_pins_and_switches_the_document() {
        use crate::syntax::LanguageId;

        let mut model = AppModel::new(80, 60, 1.0);
        model.document_mut().outline = Some(crate::outline::OutlineData::empty(0));
        let rust_row = LanguageId::all()
            .position(|l| l == LanguageId::Rust)
            .unwrap();
        model.ui.open_modal(ModalState::LanguagePicker(
            crate::model::LanguagePickerState {
                selected_index: rust_row,
                scroll_offset: 0,
            },
        ));

        let cmd = update_ui(&mut model, UiMsg::Modal(ModalMsg::Confirm));

        assert!(model.ui.active_modal.is_none());
        let doc = model.document();
        assert_eq!(doc.language, LanguageId::Rust);
        assert!(doc.language_pinned);
        assert!(doc.syntax_highlights.is_none());
        assert!(doc.outline.is_none(), "a stale outline must not survive");
        let Some(Cmd::Batch(cmds)) = cmd else {
            panic!("expected a Batch, got {cmd:?}");
        };
        assert!(cmds.iter().any(|c| match c {
            Cmd::Batch(inner) => inner
                .iter()
                .any(|c| matches!(c, Cmd::DebouncedSyntaxParse { delay_ms: 0, .. })),
            _ => false,
        }));
    }

    #[test]
    fn confirming_the_current_language_just_closes_the_picker() {
        let mut model = AppModel::new(80, 60, 1.0);
        model
            .ui
            .open_modal(ModalState::LanguagePicker(Default::default()));

        let cmd = update_ui(&mut model, UiMsg::Modal(ModalMsg::Confirm));

        assert!(model.ui.active_modal.is_none());
        assert!(!model.document().language_pinned);
        assert!(matches!(cmd, Some(Cmd::Redraw)), "got {cmd:?}");
    }
}
