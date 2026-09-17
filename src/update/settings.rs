//! Settings interaction and preference effects; persistence remains behind commands.
use crate::commands::Cmd;
use crate::keymap::preferences::{BaseKeymap, KeymapChange, KeymapSave, MAX_CAPTURE_STROKES};
use crate::keymap::{KeyCode, Keymap, Modifiers};
use crate::messages::SettingsMsg;
use crate::model::{AppModel, ModalState};
use crate::settings::forms::{CollectionKind, FormKind, SettingsChange, SettingsForm};
use crate::settings::{
    keymap::{Capture, SettingsTab},
    RowKind, SettingsState,
};
use std::sync::Arc;

/// Shared mutation path for ordinary preferences, including controls on record pages.
pub(super) fn apply_setting(
    model: &mut AppModel,
    setting: crate::settings::Setting,
    choice: usize,
) -> Option<Cmd> {
    use crate::settings::catalog::{Control, SettingAction, SettingEffect};
    let descriptor = setting.descriptor();
    if choice >= descriptor.labels().len() {
        return None;
    }
    if let Control::Picker { action, .. } = descriptor.control {
        return match action {
            SettingAction::ChooseTheme => super::ui::update_ui(
                model,
                crate::messages::UiMsg::ToggleModal(crate::model::ModalId::ThemePicker),
            ),
        };
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
    match descriptor.effect {
        SettingEffect::Redraw => {}
        SettingEffect::FontMetrics => commands.push(Cmd::SyncFontMetrics),
    }
    Some(Cmd::Batch(commands))
}

fn state_mut(ui: &mut crate::model::UiState) -> Option<&mut SettingsState> {
    match &mut ui.active_modal {
        Some(ModalState::Settings(state)) => Some(state),
        _ => None,
    }
}

pub(super) fn open_formatter(model: &mut AppModel, id: Option<&str>) -> Option<Cmd> {
    open_form(model, SettingsForm::formatter(id, &model.config))
}

pub(super) fn open_server(model: &mut AppModel, id: Option<&str>) -> Option<Cmd> {
    open_form(model, SettingsForm::language_server(id, &model.config))
}

pub(super) fn open_provider(model: &mut AppModel, id: Option<&str>) -> Option<Cmd> {
    open_form(model, SettingsForm::inline_provider(id, &model.config))
}

fn open_form(model: &mut AppModel, form: SettingsForm) -> Option<Cmd> {
    model.ui.scrollbar_drag = None;
    model.ui.settings_hover_action = None;
    model.ui.modal_hover_choice = None;
    let mut commands = vec![Cmd::Redraw];
    if let Some(command) = form
        .executable_field()
        .map(|index| form.fields[index].input.text())
        .filter(|command| !command.is_empty())
    {
        commands.push(Cmd::InspectSettingsExecutable {
            session: Arc::clone(&form.session),
            command,
        });
    }
    let state = state_mut(&mut model.ui)?;
    let category = match form.kind {
        FormKind::Formatter(_) => crate::settings::CategoryId::Formatting,
        FormKind::LanguageServer(_) => crate::settings::CategoryId::LanguageServers,
        FormKind::InlineProvider(_) => crate::settings::CategoryId::AiProviders,
    };
    state.category = category;
    let focused = form.focused;
    state.form = Some(form);
    state.scroll_offset_px = 0;
    state.refresh_entries(&model.config);
    state.selected_index = state
        .entries
        .iter()
        .position(|row| matches!(row.kind, RowKind::FormField(index) if Some(index) == focused))
        .unwrap_or(0);
    super::ui::reveal_settings_selection(model);
    Some(Cmd::Batch(commands))
}

pub(super) fn cancel_form(model: &mut AppModel) -> Option<Cmd> {
    let state = state_mut(&mut model.ui)?;
    if state.form.as_ref().is_some_and(|form| form.saving) {
        return Some(Cmd::Redraw);
    }
    let kind = state.form.as_ref()?.kind.clone();
    let command = match kind {
        FormKind::Formatter(id) => open_formatter(model, id.as_deref()),
        FormKind::LanguageServer(id) => open_server(model, id.as_deref()),
        FormKind::InlineProvider(id) => open_provider(model, id.as_deref()),
    };
    if let Some(form) = state_mut(&mut model.ui).and_then(|state| state.form.as_mut()) {
        form.focused = None;
    }
    command
}

pub(super) fn form_choice(
    model: &mut AppModel,
    choice: Option<usize>,
    delta: isize,
) -> Option<Cmd> {
    let state = state_mut(&mut model.ui)?;
    let kind = state
        .entries
        .get(*state.rows.get(state.selected_index)?)?
        .kind
        .clone();
    let form = state.form.as_mut()?;
    if form.saving {
        return Some(Cmd::Redraw);
    }
    form.open_select = None;
    match kind {
        RowKind::FormPreset => {
            let selected = choice.unwrap_or_else(|| {
                (form.preset.map_or(0, |index| index + 1) as isize + delta)
                    .rem_euclid((form.presets().len() + 1) as isize) as usize
            });
            let draft = form.draft_from_preset(selected, &model.config)?;
            return open_form(model, draft);
        }
        RowKind::FormAdvanced => {
            form.advanced = !form.advanced;
            form.focused = None;
            state.refresh_entries(&model.config);
        }
        RowKind::FormChoice(index) => {
            let value = form.choices.get_mut(index)?;
            let choice = choice.unwrap_or_else(|| {
                (value.active as isize + delta).rem_euclid(value.labels.len() as isize) as usize
            });
            if choice >= value.labels.len() {
                return None;
            }
            value.active = choice;
            form.focused = None;
            form.changed();
            state.refresh_entries(&model.config);
            state.selected_index = state
                .entries
                .iter()
                .position(|row| matches!(row.kind, RowKind::FormChoice(i) if i == index))
                .unwrap_or(0);
        }
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
        RowKind::FormInstallCommand(index, step) => {
            let option = form
                .source_tool()?
                .installation_options(crate::tooling::Platform::current())
                .nth(index)?;
            return Some(Cmd::CopyToClipboard(
                option.steps.get(step)?.command?.into(),
            ));
        }
        RowKind::FormInstallGuide(index) => {
            let option = form
                .source_tool()?
                .installation_options(crate::tooling::Platform::current())
                .nth(index)?;
            return Some(Cmd::OpenWebUrl(option.source_url.into()));
        }
        RowKind::FormRecheck => {
            return Some(Cmd::InspectSettingsExecutable {
                session: Arc::clone(&form.session),
                command: form.fields[form.executable_field()?].input.text(),
            });
        }
        RowKind::FormActions => {
            form.focused = None;
            let action = choice.unwrap_or(if delta < 0 { 1 } else { 0 });
            if form.remove_pending && action == 0 {
                form.remove_pending = false;
                form.status = "Removal cancelled · draft retained".into();
                return Some(Cmd::Redraw);
            }
            if form.remove_pending && action == 2 {
                return Some(Cmd::Redraw);
            }
            match action {
                0 | 2 if action == 0 || matches!(form.kind, FormKind::InlineProvider(_)) => {
                    match form.change(&model.config) {
                        Ok(mut change) => {
                            if let SettingsChange::InlineProvider { select, .. } = &mut change {
                                *select = action == 2;
                            }
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
                    }
                }
                1 => return cancel_form(model),
                2 if matches!(form.kind, FormKind::LanguageServer(_)) => {
                    if let Some(ModalState::Settings(mut state)) = model.ui.active_modal.take() {
                        if let Some(form) = &mut state.form {
                            form.dragging = false;
                            form.status =
                                "Draft retained while viewing the log · Save or Cancel when ready"
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
                3 => {
                    let change = form.removal()?;
                    if !form.remove_pending {
                        form.remove_pending = true;
                        form.status = "Remove this entry and discard its draft? Confirm remove to save this change.".into();
                    } else {
                        form.saving = true;
                        form.status = "Removing configuration…".into();
                        return Some(Cmd::Batch(vec![
                            Cmd::ApplySettingsForm {
                                session: Arc::clone(&form.session),
                                change: Box::new(change),
                            },
                            Cmd::Redraw,
                        ]));
                    }
                }
                _ => return None,
            }
        }
        _ => return None,
    }
    Some(Cmd::Redraw)
}

pub(super) fn select_key(model: &mut AppModel, delta: isize, confirm: bool) -> Option<Cmd> {
    let state = state_mut(&mut model.ui)?;
    let form = state.form.as_mut()?;
    let row = form.open_select?;
    let count = match state.entries.get(*state.rows.get(row)?)?.kind {
        RowKind::FormChoice(index) => form.choices.get(index)?.labels.len(),
        RowKind::FormPreset => form.presets().len() + 1,
        _ => return None,
    };
    if confirm {
        let choice = form.select_cursor;
        state.selected_index = row;
        form_choice(model, Some(choice), 0)
    } else {
        form.select_cursor =
            (form.select_cursor as isize + delta).rem_euclid(count as isize) as usize;
        Some(Cmd::Redraw)
    }
}

pub(super) fn adjust_form(model: &mut AppModel) -> Option<Cmd> {
    let state = state_mut(&mut model.ui)?;
    if matches!(
        state
            .entries
            .get(*state.rows.get(state.selected_index)?)?
            .kind,
        RowKind::FormEnabled | RowKind::FormChoice(_) | RowKind::FormPreset | RowKind::FormAdvanced
    ) {
        form_choice(model, None, 1)
    } else {
        Some(Cmd::Redraw)
    }
}

pub(super) fn form_focus(model: &mut AppModel, forward: bool) -> Option<Cmd> {
    let state = state_mut(&mut model.ui)?;
    let form = state.form.as_mut()?;
    if form.saving {
        return Some(Cmd::Redraw);
    }
    form.open_select = None;
    // Include the enable switch and action row in keyboard traversal.
    for _ in 0..state.rows.len() {
        state.selected_index = (state.selected_index as isize + if forward { 1 } else { -1 })
            .rem_euclid(state.rows.len() as isize) as usize;
        if matches!(
            state.entries[state.rows[state.selected_index]].kind,
            RowKind::FormField(_)
                | RowKind::FormEnabled
                | RowKind::FormAdvanced
                | RowKind::FormPreset
                | RowKind::FormChoice(_)
                | RowKind::FormActions
                | RowKind::FormInstallCommand(..)
                | RowKind::FormInstallGuide(_)
                | RowKind::FormRecheck
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
    let row = state_mut(&mut model.ui)?.selected_index;
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
    let input = state_mut(&mut model.ui)?.focused_input_mut()?;
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
    let state = state_mut(&mut model.ui)?;
    let categories = crate::settings::categories();
    if state.keymap.capture.is_some() || index.is_some_and(|i| i >= categories.len()) {
        return Some(Cmd::Redraw);
    }
    if let Some(form) = &mut state.form {
        if form.dirty || form.saving {
            form.status = "Save or Cancel this draft before changing categories".into();
            return Some(Cmd::Redraw);
        }
    }
    state.form = None;
    state.category = categories[index.unwrap_or((state.category.index() + 1) % categories.len())];
    state.tab = if state.category.page().kind == crate::settings::pages::PageKind::Keymap {
        SettingsTab::Keymap
    } else {
        SettingsTab::General
    };
    state.editable.set_content("");
    state.refresh_entries(&model.config);
    match state.category.page().kind {
        crate::settings::pages::PageKind::Formatters => {
            let id = crate::settings::forms::formatter_ids(&model.config)
                .first()
                .copied();
            return open_formatter(model, id);
        }
        crate::settings::pages::PageKind::LanguageServers => {
            let id = crate::lsp::server_ids(&model.config.lsp)
                .first()
                .map(|id| (*id).to_owned());
            return open_server(model, id.as_deref());
        }
        crate::settings::pages::PageKind::AiProviders => {
            let id = model.config.completion.providers.keys().min().cloned();
            return open_provider(model, id.as_deref());
        }
        crate::settings::pages::PageKind::Preferences
        | crate::settings::pages::PageKind::Keymap => {}
    }
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
    let state = state_mut(&mut model.ui)?;
    if state.form.is_some() {
        let kind = state
            .entries
            .get(*state.rows.get(state.selected_index)?)?
            .kind
            .clone();
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
    state.refresh_entries(&model.config);
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
    let state = state_mut(&mut model.ui)?;
    if !matches!(
        state
            .entries
            .get(*state.rows.get(state.selected_index)?)
            .map(|r| r.kind.clone()),
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
    let state = state_mut(&mut model.ui)?;
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
    state.refresh_entries(&model.config);
    Some(Cmd::Redraw)
}

pub(super) fn update_settings(model: &mut AppModel, msg: SettingsMsg) -> Option<Cmd> {
    match msg {
        SettingsMsg::ScrollRecords { delta, max } => {
            let form = state_mut(&mut model.ui)?.form.as_mut()?;
            form.records_scroll = form.records_scroll.saturating_add_signed(delta).min(max);
            Some(Cmd::Redraw)
        }
        SettingsMsg::CollectionAction(action) => {
            use crate::messages::SettingsCollectionAction;
            let form = state_mut(&mut model.ui)?.form.as_mut()?;
            if form.saving {
                return Some(Cmd::Redraw);
            }
            match action {
                SettingsCollectionAction::ToggleSelect(row) => {
                    let state = state_mut(&mut model.ui)?;
                    let kind = &state.entries.get(*state.rows.get(row)?)?.kind;
                    let form = state.form.as_mut()?;
                    form.select_cursor = match kind {
                        RowKind::FormChoice(index) => form.choices.get(*index)?.active,
                        RowKind::FormPreset => form.preset.map_or(0, |index| index + 1),
                        _ => return None,
                    };
                    form.open_select = (form.open_select != Some(row)).then_some(row);
                    form.focused = None;
                    state_mut(&mut model.ui)?.selected_index = row;
                    return Some(Cmd::Redraw);
                }
                SettingsCollectionAction::CloseSelect => {
                    form.open_select = None;
                    return Some(Cmd::Redraw);
                }
                SettingsCollectionAction::ToggleMaster => {
                    return match form.kind {
                        FormKind::Formatter(_) => None,
                        FormKind::LanguageServer(_) => Some(Cmd::Batch(vec![
                            super::lsp::toggle_lsp_enabled(model)?,
                            Cmd::Redraw,
                        ])),
                        FormKind::InlineProvider(_) => {
                            let choice = usize::from(!model.config.completion.inline.enabled);
                            apply_setting(model, crate::settings::Setting::InlineEnabled, choice)
                        }
                    };
                }
                _ => {}
            }
            if form.dirty {
                form.status = "Save or Cancel this draft before selecting another entry".into();
                return Some(Cmd::Redraw);
            }
            let kind = form.kind.clone();
            let ids = match kind {
                FormKind::Formatter(_) => crate::settings::forms::formatter_ids(&model.config)
                    .into_iter()
                    .map(str::to_owned)
                    .collect(),
                FormKind::LanguageServer(_) => crate::lsp::server_ids(&model.config.lsp)
                    .into_iter()
                    .map(str::to_owned)
                    .collect::<Vec<_>>(),
                FormKind::InlineProvider(_) => {
                    let mut ids = model
                        .config
                        .completion
                        .providers
                        .keys()
                        .cloned()
                        .collect::<Vec<_>>();
                    ids.sort();
                    ids
                }
            };
            let id = match action {
                SettingsCollectionAction::Select(index) => Some(ids.get(index)?.as_str()),
                SettingsCollectionAction::Add => None,
                SettingsCollectionAction::ToggleMaster
                | SettingsCollectionAction::ToggleSelect(_)
                | SettingsCollectionAction::CloseSelect => return None,
            };
            match kind {
                FormKind::Formatter(_) => open_formatter(model, id),
                FormKind::LanguageServer(_) => open_server(model, id),
                FormKind::InlineProvider(_) => open_provider(model, id),
            }
        }
        SettingsMsg::EndFieldSelection => {
            state_mut(&mut model.ui)?.form.as_mut()?.dragging = false;
            None
        }
        SettingsMsg::FieldPointer {
            row,
            position,
            extend,
        } => {
            let state = state_mut(&mut model.ui)?;
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
            let input = state_mut(&mut model.ui)?.focused_input_mut()?;
            if down {
                input.move_down(extend);
            } else {
                input.move_up(extend);
            }
            model.ui.reset_cursor_blink();
            Some(Cmd::Redraw)
        }
        SettingsMsg::UndoField { redo } => {
            let input = state_mut(&mut model.ui)?.focused_input_mut()?;
            if redo {
                input.redo();
            } else {
                input.undo();
            }
            if let Some(form) = state_mut(&mut model.ui)?.form.as_mut() {
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
            let state = state_mut(&mut model.ui)?;
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
                let mut commands = vec![Cmd::Redraw];
                if form.executable_field() == Some(field) {
                    commands.push(Cmd::InspectSettingsExecutable { session, command });
                }
                return Some(Cmd::Batch(commands));
            }
            Some(Cmd::Redraw)
        }
        SettingsMsg::ExecutableChecked {
            session,
            command,
            status,
        } => {
            let form = state_mut(&mut model.ui)?.form.as_mut()?;
            if !Arc::ptr_eq(&form.session, &session)
                || form
                    .executable_field()
                    .is_none_or(|index| form.fields[index].input.text() != command)
            {
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
            if let Some(form) = state_mut(&mut model.ui)
                .and_then(|state| state.form.as_mut())
                .filter(|form| Arc::ptr_eq(&form.session, &session))
            {
                form.saving = false;
                if success {
                    form.applied(&change);
                }
                form.status = match result {
                    Ok(()) => "Saved · the configuration is applied".into(),
                    Err(error) => format!("Not applied: {error}"),
                };
            }
            if success {
                let removed_current = state_mut(&mut model.ui).is_some_and(|state| {
                    matches!(*change, SettingsChange::Remove { .. })
                        && state
                            .form
                            .as_ref()
                            .is_some_and(|form| Arc::ptr_eq(&form.session, &session))
                });
                if let Some(state) = state_mut(&mut model.ui) {
                    if matches!(*change, SettingsChange::Remove { .. })
                        && state
                            .form
                            .as_ref()
                            .is_some_and(|form| Arc::ptr_eq(&form.session, &session))
                    {
                        state.form = None;
                    }
                    state.refresh_entries(&model.config);
                }
                let mut commands = vec![Cmd::Redraw];
                if removed_current {
                    let category = state_mut(&mut model.ui).map(|state| state.category.index());
                    if let Some(command) = switch_tab(model, category) {
                        commands.push(command);
                    }
                }
                match *change {
                    SettingsChange::Formatter { .. }
                    | SettingsChange::Remove {
                        collection: CollectionKind::Formatters,
                        ..
                    } => {
                        if let Some(form) =
                            state_mut(&mut model.ui).and_then(|state| state.form.as_ref())
                        {
                            commands.push(Cmd::InspectSettingsExecutable {
                                session: Arc::clone(&form.session),
                                command: form.fields[0].input.text(),
                            });
                        }
                    }
                    SettingsChange::LanguageServer {
                        id, previous_id, ..
                    } => {
                        commands.push(Cmd::LspApplyConfiguration {
                            server_id: id.into(),
                            previous_id: previous_id.map(Into::into),
                        });
                        if let Some(form) =
                            state_mut(&mut model.ui).and_then(|state| state.form.as_ref())
                        {
                            if let Some(index) = form.executable_field() {
                                commands.push(Cmd::InspectSettingsExecutable {
                                    session: Arc::clone(&form.session),
                                    command: form.fields[index].input.text(),
                                });
                            }
                        }
                    }
                    SettingsChange::InlineProvider { .. } => {
                        commands.extend(super::inline::dismiss(model))
                    }
                    SettingsChange::Remove {
                        collection: CollectionKind::LanguageServers,
                        id,
                    } => commands.push(Cmd::LspApplyConfiguration {
                        server_id: id.into(),
                        previous_id: None,
                    }),
                    SettingsChange::Remove {
                        collection: CollectionKind::InlineProviders,
                        ..
                    } => commands.extend(super::inline::dismiss(model)),
                }
                Some(Cmd::Batch(commands))
            } else {
                Some(Cmd::Redraw)
            }
        }
        SettingsMsg::CaptureRejected(reason) => {
            let state = state_mut(&mut model.ui)?;
            if state.keymap.capture.is_some() && !state.keymap.saving {
                state.keymap.status = reason;
                state.refresh_entries(&model.config);
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
            let state = state_mut(&mut model.ui)?;
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
                state.refresh_entries(&model.config);
            }
            Some(Cmd::Redraw)
        }
        SettingsMsg::CaptureKey(stroke) => {
            let state = state_mut(&mut model.ui)?;
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
                    state.refresh_entries(&model.config);
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
            state.refresh_entries(&model.config);
            Some(Cmd::Redraw)
        }
    }
}

#[cfg(test)]
mod tooling_tests {
    use super::*;
    use crate::model::ModalId;

    #[test]
    fn guidance_actions_only_copy_open_or_probe_without_applying_the_draft() {
        let mut model = AppModel::new(1200, 900, 1.0);
        super::super::update(
            &mut model,
            crate::messages::Msg::Ui(crate::messages::UiMsg::ToggleModal(ModalId::Settings)),
        );
        open_formatter(&mut model, Some("Python"));
        let original = model.config.clone();
        let state = state_mut(&mut model.ui).unwrap();
        let form = state.form.as_mut().unwrap();
        form.fields[0].input.set_content("/my/custom-ruff");
        form.changed();
        let session = Arc::clone(&form.session);
        let pick = |model: &mut AppModel, predicate: fn(&RowKind) -> bool| {
            let state = state_mut(&mut model.ui).unwrap();
            state.selected_index = state
                .rows
                .iter()
                .position(|&index| predicate(&state.entries[index].kind))
                .unwrap();
            form_choice(model, Some(0), 0).unwrap()
        };
        assert!(
            matches!(pick(&mut model, |kind| matches!(kind, RowKind::FormInstallCommand(..))), Cmd::CopyToClipboard(command) if command == "uv tool install ruff@latest")
        );
        assert!(
            matches!(pick(&mut model, |kind| matches!(kind, RowKind::FormInstallGuide(_))), Cmd::OpenWebUrl(url) if url == "https://docs.astral.sh/ruff/installation/")
        );
        assert!(
            matches!(pick(&mut model, |kind| matches!(kind, RowKind::FormRecheck)), Cmd::InspectSettingsExecutable { session: actual, command } if Arc::ptr_eq(&session, &actual) && command == "/my/custom-ruff")
        );
        assert_eq!(model.config.formatters, original.formatters);
        assert_eq!(model.config.lsp, original.lsp);
        let form = state_mut(&mut model.ui).unwrap().form.as_ref().unwrap();
        assert!(form.dirty);
        assert!(!form.saving);
        assert_eq!(form.fields[0].input.text(), "/my/custom-ruff");
    }

    #[test]
    fn removing_the_selected_record_cannot_leave_that_record_selected() {
        let mut model = AppModel::new(1200, 900, 1.0);
        super::super::update(
            &mut model,
            crate::messages::Msg::Ui(crate::messages::UiMsg::ToggleModal(ModalId::Settings)),
        );
        open_server(&mut model, Some("rust-analyzer"));
        let session = Arc::clone(
            &state_mut(&mut model.ui)
                .and_then(|state| state.form.as_ref())
                .expect("server form is open")
                .session,
        );

        update_settings(
            &mut model,
            SettingsMsg::FormApplied {
                session,
                change: Box::new(SettingsChange::Remove {
                    collection: CollectionKind::LanguageServers,
                    id: "rust-analyzer".into(),
                }),
                result: Ok(()),
            },
        );

        assert!(!model.config.lsp.servers.contains_key("rust-analyzer"));
        let state = state_mut(&mut model.ui).expect("Settings remains open");
        assert_eq!(state.category, crate::settings::CategoryId::LanguageServers);
        assert!(
            matches!(
                state.form.as_ref().map(|form| &form.kind),
                Some(FormKind::LanguageServer(Some(id))) if id != "rust-analyzer"
            ),
            "the collection may select a remaining record, but not the removed one"
        );
    }
}
