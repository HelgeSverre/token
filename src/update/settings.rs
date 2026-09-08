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

pub(super) fn capturing(model: &AppModel) -> bool {
    matches!(&model.ui.active_modal, Some(ModalState::Settings(state)) if state.keymap.capture.is_some())
}

pub(super) fn switch_tab(model: &mut AppModel, index: Option<usize>) -> Option<Cmd> {
    let state = state_mut(model)?;
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
