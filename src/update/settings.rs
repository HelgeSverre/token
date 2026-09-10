//! Settings keymap interaction; persistence remains behind commands.
use crate::commands::Cmd;
use crate::keymap::preferences::{BaseKeymap, KeymapChange, KeymapSave, MAX_CAPTURE_STROKES};
use crate::keymap::{KeyCode, Keymap, Modifiers};
use crate::messages::SettingsMsg;
use crate::model::{AppModel, ModalState};
use crate::settings::{
    keymap::{Capture, SettingsTab},
    RowKind, SettingsState,
};
use std::sync::Arc;

fn state_mut(model: &mut AppModel) -> Option<&mut SettingsState> {
    match &mut model.ui.active_modal {
        Some(ModalState::Settings(state)) => Some(state),
        _ => None,
    }
}

pub(super) fn open_server(
    model: &mut AppModel,
    def: &'static crate::lsp::LspServerDef,
) -> Option<Cmd> {
    let form = crate::settings::forms::SettingsForm::language_server(def, &model.config);
    let cmd = Cmd::InspectSettingsExecutable {
        session: Arc::clone(&form.session),
        command: form.fields[0].input.text(),
    };
    let state = state_mut(model)?;
    state.form = Some(form);
    state.selected_index = 1;
    state.scroll_offset_px = 0;
    state.refresh_entries();
    super::ui::reveal_settings_selection(model);
    Some(Cmd::Batch(vec![cmd, Cmd::Redraw]))
}

pub(super) fn cancel_form(model: &mut AppModel) -> Option<Cmd> {
    let state = state_mut(model)?;
    if state.form.as_ref().is_some_and(|form| form.saving) {
        return Some(Cmd::Redraw);
    }
    state.form = None;
    state.refresh_entries();
    Some(Cmd::Redraw)
}

pub(super) fn form_choice(
    model: &mut AppModel,
    choice: Option<usize>,
    delta: isize,
) -> Option<Cmd> {
    let state = state_mut(model)?;
    let kind = state
        .entries
        .get(*state.rows.get(state.selected_index)?)?
        .kind;
    let form = state.form.as_mut()?;
    if form.saving {
        return Some(Cmd::Redraw);
    }
    match kind {
        RowKind::FormEnabled => {
            if choice.is_some_and(|choice| choice > 1) {
                return None;
            }
            form.enabled = choice.map_or(!form.enabled, |choice| choice == 1);
            form.focused = None;
            form.changed();
        }
        RowKind::FormField(index) => {
            let field = form.fields.get(index)?;
            if choice.is_some() && (choice != Some(0) || !field.browse) {
                return None;
            }
            if choice == Some(0) && field.browse {
                return Some(Cmd::ChooseSettingsFile {
                    session: Arc::clone(&form.session),
                    field: index,
                    current: field.input.text(),
                });
            }
            form.focused = Some(index);
        }
        RowKind::FormActions => {
            form.focused = None;
            match choice.unwrap_or(if delta < 0 { 1 } else { 0 }) {
                0 => match form.change() {
                    Ok(change) => {
                        form.saving = true;
                        form.status = "Saving configuration…".into();
                        return Some(Cmd::Batch(vec![
                            Cmd::ApplySettingsForm {
                                session: Arc::clone(&form.session),
                                change: Box::new(change),
                            },
                            Cmd::Redraw,
                        ]));
                    }
                    Err(error) => form.status = error.to_string(),
                },
                1 => return cancel_form(model),
                2 => {
                    if let Some(ModalState::Settings(mut state)) = model.ui.active_modal.take() {
                        if let Some(form) = &mut state.form {
                            form.dragging = false;
                            form.status =
                                "Draft retained while viewing the log · Apply or Cancel when ready"
                                    .into();
                        }
                        model.ui.suspended_settings = Some(state);
                    }
                    model.ui.close_modal();
                    return super::layout::open_config_resource(
                        model,
                        crate::commands::ConfigResource::Log,
                    );
                }
                _ => return None,
            }
        }
        _ => return None,
    }
    Some(Cmd::Redraw)
}

pub(super) fn adjust_form(model: &mut AppModel) -> Option<Cmd> {
    let state = state_mut(model)?;
    if matches!(
        state
            .entries
            .get(*state.rows.get(state.selected_index)?)?
            .kind,
        RowKind::FormEnabled
    ) {
        form_choice(model, None, 1)
    } else {
        Some(Cmd::Redraw)
    }
}

pub(super) fn form_focus(model: &mut AppModel, forward: bool) -> Option<Cmd> {
    let state = state_mut(model)?;
    let form = state.form.as_mut()?;
    if form.saving {
        return Some(Cmd::Redraw);
    }
    // Include the enable switch and action row in keyboard traversal.
    for _ in 0..state.rows.len() {
        state.selected_index = (state.selected_index as isize + if forward { 1 } else { -1 })
            .rem_euclid(state.rows.len() as isize) as usize;
        if matches!(
            state.entries[state.rows[state.selected_index]].kind,
            RowKind::FormField(_) | RowKind::FormEnabled | RowKind::FormActions
        ) {
            break;
        }
    }
    form.focused = match state.entries[state.rows[state.selected_index]].kind {
        RowKind::FormField(index) => Some(index),
        _ => None,
    };
    super::ui::reveal_settings_selection(model);
    Some(Cmd::Redraw)
}

pub(super) fn page_field(model: &mut AppModel, forward: bool) -> Option<Cmd> {
    let row = state_mut(model)?.selected_index;
    let count = crate::view::modal::with_modal_overlay_layout(
        model,
        model.window_size.0 as usize,
        model.window_size.1 as usize,
        model.metrics.scale_factor,
        |spec, layout| {
            crate::view::overlay_surface::settings_field_options(spec, layout, row)
                .map(|opts| opts.rows.max(1))
        },
    )
    .flatten()
    .unwrap_or(1);
    let input = state_mut(model)?.focused_input_mut()?;
    for _ in 0..count {
        if forward {
            input.move_down(false);
        } else {
            input.move_up(false);
        }
    }
    model.ui.reset_cursor_blink();
    Some(Cmd::Redraw)
}

pub(super) fn capturing(model: &AppModel) -> bool {
    matches!(&model.ui.active_modal, Some(ModalState::Settings(state)) if state.keymap.capture.is_some())
}

pub(super) fn switch_tab(model: &mut AppModel, index: Option<usize>) -> Option<Cmd> {
    let state = state_mut(model)?;
    if let Some(form) = &mut state.form {
        form.status = "Apply or Cancel this draft before changing categories".into();
        return Some(Cmd::Redraw);
    }
    let categories = crate::settings::categories();
    if state.keymap.capture.is_some() || index.is_some_and(|i| i >= categories.len()) {
        return Some(Cmd::Redraw);
    }
    state.category = index.unwrap_or((state.category + 1) % categories.len());
    state.tab = if categories[state.category] == Some("Keymap") {
        SettingsTab::Keymap
    } else {
        SettingsTab::General
    };
    state.editable.set_content("");
    state.refresh_entries();
    if state.tab == SettingsTab::Keymap && state.keymap.snapshot.is_none() && !state.keymap.loading
    {
        state.keymap.loading = true;
        state.keymap.status = "Loading keymap…".into();
        return Some(Cmd::Batch(vec![
            Cmd::Redraw,
            Cmd::PrepareKeymap {
                session: Arc::clone(&state.keymap.session),
                save: None,
            },
        ]));
    }
    Some(Cmd::Redraw)
}

pub(super) fn activate_row(model: &mut AppModel) -> Option<Cmd> {
    let state = state_mut(model)?;
    if state.form.is_some() {
        let kind = state
            .entries
            .get(*state.rows.get(state.selected_index)?)?
            .kind;
        if let Some(form) = &mut state.form {
            form.focused = match kind {
                RowKind::FormField(index) => Some(index),
                _ => None,
            };
        }
        return Some(Cmd::Redraw);
    }
    if state.tab != SettingsTab::Keymap || state.keymap.loading || state.keymap.saving {
        return Some(Cmd::Redraw);
    }
    let row = state.entries.get(*state.rows.get(state.selected_index)?)?;
    let RowKind::KeymapBinding(index, command) = row.kind else {
        return Some(Cmd::Redraw);
    };
    let snapshot = state.keymap.snapshot.as_ref()?;
    let original = index
        .and_then(|index| snapshot.bindings.get(index))
        .cloned();
    state.keymap.capture = Some(Capture {
        original,
        command,
        strokes: Vec::new(),
        literal_next: false,
    });
    state.keymap.update_capture_status();
    state.refresh_entries();
    Some(Cmd::Redraw)
}

fn save(state: &mut SettingsState, change: KeymapChange) -> Option<Cmd> {
    if state.keymap.loading || state.keymap.saving {
        return Some(Cmd::Redraw);
    }
    let request = KeymapSave {
        expected: state.keymap.snapshot.as_ref()?.source.clone(),
        change,
    };
    state.keymap.saving = true;
    state.keymap.status = "Saving keymap…".into();
    Some(Cmd::Batch(vec![
        Cmd::Redraw,
        Cmd::PrepareKeymap {
            session: Arc::clone(&state.keymap.session),
            save: Some(Box::new(request)),
        },
    ]))
}

pub(super) fn choose_base(
    model: &mut AppModel,
    choice: Option<usize>,
    delta: isize,
) -> Option<Cmd> {
    let state = state_mut(model)?;
    if !matches!(
        state
            .entries
            .get(*state.rows.get(state.selected_index)?)
            .map(|r| r.kind),
        Some(RowKind::KeymapBase)
    ) {
        return Some(Cmd::Redraw);
    }
    let current = state.keymap.base_index()?;
    let choice = choice.unwrap_or_else(|| (current as isize + delta).rem_euclid(2) as usize);
    if choice > 1 || choice == current {
        return Some(Cmd::Redraw);
    }
    save(
        state,
        KeymapChange::Base(if choice == 0 {
            BaseKeymap::Token
        } else {
            BaseKeymap::Conventional
        }),
    )
}

pub(super) fn capture_action(model: &mut AppModel, action: usize) -> Option<Cmd> {
    let state = state_mut(model)?;
    if state.keymap.saving {
        return Some(Cmd::Redraw);
    }
    let capture = state.keymap.capture.as_mut()?;
    match action {
        0 if !capture.strokes.is_empty() => {
            let change = KeymapChange::Rebind {
                original: capture.original.clone(),
                command: capture.command,
                strokes: capture.strokes.clone(),
            };
            return save(state, change);
        }
        0 => state.keymap.status = "Record at least one key before saving".into(),
        1 => {
            state.keymap.capture = None;
            state.keymap.status = "Capture cancelled; keymap unchanged".into();
        }
        2 => {
            capture.literal_next = true;
            state.keymap.update_capture_status();
        }
        _ => return None,
    }
    state.refresh_entries();
    Some(Cmd::Redraw)
}

pub(super) fn update_settings(model: &mut AppModel, msg: SettingsMsg) -> Option<Cmd> {
    match msg {
        SettingsMsg::EndFieldSelection => {
            state_mut(model)?.form.as_mut()?.dragging = false;
            None
        }
        SettingsMsg::FieldPointer {
            row,
            position,
            extend,
        } => {
            let state = state_mut(model)?;
            let RowKind::FormField(index) = state.entries.get(*state.rows.get(row)?)?.kind else {
                return None;
            };
            let form = state.form.as_mut()?;
            if form.saving {
                return None;
            }
            form.focused = Some(index);
            form.dragging = true;
            state.selected_index = row;
            form.fields
                .get_mut(index)?
                .input
                .set_cursor_position(position, extend);
            model.ui.reset_cursor_blink();
            Some(Cmd::Redraw)
        }
        SettingsMsg::MoveFieldCursor { down, extend } => {
            let input = state_mut(model)?.focused_input_mut()?;
            if down {
                input.move_down(extend);
            } else {
                input.move_up(extend);
            }
            model.ui.reset_cursor_blink();
            Some(Cmd::Redraw)
        }
        SettingsMsg::UndoField { redo } => {
            let input = state_mut(model)?.focused_input_mut()?;
            if redo {
                input.redo();
            } else {
                input.undo();
            }
            if let Some(form) = state_mut(model)?.form.as_mut() {
                form.changed();
            }
            model.ui.reset_cursor_blink();
            Some(Cmd::Redraw)
        }
        SettingsMsg::FileChosen {
            session,
            field,
            path,
        } => {
            let state = state_mut(model)?;
            let form = state.form.as_mut()?;
            if !Arc::ptr_eq(&form.session, &session) || form.saving {
                return None;
            }
            if let Some(path) = path {
                let input = &mut form.fields.get_mut(field)?.input;
                input.set_content(&path.to_string_lossy());
                let command = input.text();
                form.focused = Some(field);
                if let Some(row) = state.rows.iter().position(|&id| matches!(state.entries[id].kind, RowKind::FormField(index) if index == field)) {
                    state.selected_index = row;
                }
                form.changed();
                return Some(Cmd::Batch(vec![
                    Cmd::InspectSettingsExecutable { session, command },
                    Cmd::Redraw,
                ]));
            }
            Some(Cmd::Redraw)
        }
        SettingsMsg::ExecutableChecked {
            session,
            command,
            status,
        } => {
            let form = state_mut(model)?.form.as_mut()?;
            if !Arc::ptr_eq(&form.session, &session) || form.fields[0].input.text() != command {
                return None;
            }
            form.executable_status = status;
            Some(Cmd::Redraw)
        }
        SettingsMsg::FormApplied {
            session,
            change,
            result,
        } => {
            let success = result.is_ok();
            if success {
                change.apply(&mut model.config);
            }
            if let Some(form) = state_mut(model)
                .and_then(|state| state.form.as_mut())
                .filter(|form| Arc::ptr_eq(&form.session, &session))
            {
                form.saving = false;
                form.status = match result {
                    Ok(()) => "Saved · the configuration is applied".into(),
                    Err(error) => format!("Not applied: {error}"),
                };
            }
            if success {
                let crate::settings::forms::SettingsChange::LanguageServer { id, .. } = *change;
                let mut commands = vec![
                    Cmd::LspApplyConfiguration {
                        server_id: id.into(),
                    },
                    Cmd::Redraw,
                ];
                if let Some(form) = state_mut(model).and_then(|state| state.form.as_ref()) {
                    commands.push(Cmd::InspectSettingsExecutable {
                        session: Arc::clone(&form.session),
                        command: form.fields[0].input.text(),
                    });
                }
                Some(Cmd::Batch(commands))
            } else {
                Some(Cmd::Redraw)
            }
        }
        SettingsMsg::CaptureRejected(reason) => {
            let state = state_mut(model)?;
            if state.keymap.capture.is_some() && !state.keymap.saving {
                state.keymap.status = reason;
                state.refresh_entries();
            }
            Some(Cmd::Redraw)
        }
        SettingsMsg::KeymapResult {
            session,
            saved,
            result,
        } => {
            let active = matches!(&model.ui.active_modal, Some(ModalState::Settings(state))
                if Arc::ptr_eq(&state.keymap.session, &session));
            // A committed save still applies after Settings closes. The ordered
            // worker delivers saves in the same order they reached the disk.
            if saved || active {
                if let Ok(snapshot) = &result {
                    model.ui.keymap = Keymap::with_bindings(snapshot.bindings.clone());
                }
            }
            if !active {
                return saved.then_some(Cmd::Redraw);
            }
            let state = state_mut(model)?;
            state.keymap.loading = false;
            state.keymap.saving = false;
            match result {
                Ok(snapshot) => {
                    state.keymap.snapshot = Some(*snapshot);
                    state.keymap.capture = None;
                    state.keymap.status = if saved {
                        "Keymap saved and applied"
                    } else {
                        "Click or Enter to rebind · conflicts are shown per row"
                    }
                    .into();
                }
                Err(error) => state.keymap.status = error,
            }
            if state.tab == SettingsTab::Keymap {
                state.refresh_entries();
            }
            Some(Cmd::Redraw)
        }
        SettingsMsg::CaptureKey(stroke) => {
            let state = state_mut(model)?;
            if state.keymap.saving {
                return Some(Cmd::Redraw);
            }
            let capture = state.keymap.capture.as_mut()?;
            if !capture.literal_next {
                if stroke.key == KeyCode::Escape && stroke.mods.is_empty() {
                    return capture_action(model, 1);
                }
                if stroke.key == KeyCode::Enter && stroke.mods == Modifiers::CTRL {
                    return capture_action(model, 0);
                }
                if stroke.key == KeyCode::Backspace && stroke.mods.is_empty() {
                    capture.strokes.pop();
                    state.keymap.update_capture_status();
                    state.refresh_entries();
                    return Some(Cmd::Redraw);
                }
            }
            if capture.strokes.len() == MAX_CAPTURE_STROKES {
                state.keymap.status = "Four-key limit; Backspace removes a stroke".into();
            } else {
                capture.strokes.push(stroke);
                capture.literal_next = false;
                state.keymap.update_capture_status();
            }
            state.refresh_entries();
            Some(Cmd::Redraw)
        }
    }
}
