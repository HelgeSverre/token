//! One close gate; the existing file/save pipeline remains the authority on success.

use crate::commands::Cmd;
use crate::model::closing::{CloseSaves, CloseTarget, UnsavedDocument};
use crate::model::{AppModel, EditorId, ModalState, SaveReason, UnsavedChangesState};

fn closing_editors(model: &AppModel, target: &CloseTarget) -> Vec<EditorId> {
    let tabs: Vec<_> = model
        .editor_area
        .layout
        .group_ids()
        .into_iter()
        .filter_map(|id| model.editor_area.groups.get(&id))
        .flat_map(|group| &group.tabs)
        .collect();
    if let CloseTarget::Tabs(ids) = target {
        return super::layout::closable_tabs(model, ids)
            .into_iter()
            .filter_map(|id| tabs.iter().find(|tab| tab.id == id))
            .map(|tab| tab.editor_id)
            .collect();
    }
    tabs.into_iter().map(|tab| tab.editor_id).collect()
}

fn unsaved_documents(model: &AppModel, target: &CloseTarget) -> Vec<UnsavedDocument> {
    let editors = closing_editors(model, target);
    let mut documents = Vec::new();
    for &editor_id in &editors {
        let Some(id) = model
            .editor_area
            .editors
            .get(&editor_id)
            .and_then(|e| e.document_id)
        else {
            continue;
        };
        if documents.iter().any(|doc: &UnsavedDocument| doc.id == id) {
            continue;
        }
        let Some(doc) = model.editor_area.documents.get(&id) else {
            continue;
        };
        let cells: Vec<_> = editors
            .iter()
            .filter_map(|&editor_id| {
                let editor = model.editor_area.editors.get(&editor_id)?;
                if editor.document_id != Some(id) {
                    return None;
                }
                let edit = editor.view_mode.as_csv()?.editing.as_ref()?;
                let text = edit.buffer();
                (text != edit.original).then_some((editor_id, edit.position, text))
            })
            .collect();
        let last_view = model.editor_area.editors.iter().all(|(editor_id, editor)| {
            editor.document_id != Some(id) || editors.contains(editor_id)
        });
        if (doc.is_modified && last_view) || !cells.is_empty() {
            documents.push(UnsavedDocument {
                id,
                revision: doc.revision,
                path: doc.file_path.clone(),
                cells,
            });
        }
    }
    documents
}

pub(super) fn request(model: &mut AppModel, target: CloseTarget) -> Option<Cmd> {
    // Repeated OS close events cannot reset an in-progress save/confirmation.
    if matches!(model.ui.active_modal, Some(ModalState::UnsavedChanges(_))) {
        return Some(Cmd::Redraw);
    }
    let documents = unsaved_documents(model, &target);
    if documents.is_empty() {
        return finish(model, target);
    }
    model
        .ui
        .open_modal(ModalState::UnsavedChanges(UnsavedChangesState {
            target,
            documents,
            saves: None,
            selected_index: 0,
        }));
    Some(Cmd::Redraw)
}

fn finish(model: &mut AppModel, target: CloseTarget) -> Option<Cmd> {
    match target {
        CloseTarget::Quit => Some(Cmd::Quit),
        CloseTarget::Tabs(tabs) => Some(super::layout::close_tabs(model, &tabs)),
    }
}

pub(super) fn confirm(model: &mut AppModel, mut state: UnsavedChangesState) -> Option<Cmd> {
    if state.selected_index == 0 || state.saves.is_some() {
        model.ui.close_modal();
        return Some(Cmd::Redraw);
    }
    // Neither Save nor Discard may approve changes made after the prompt appeared.
    let current = unsaved_documents(model, &state.target);
    if current != state.documents {
        model.ui.close_modal();
        model
            .ui
            .set_status("Documents changed while the dialog was open; review them before closing");
        return request(model, state.target);
    }
    match state.selected_index {
        1 => {
            state.saves = Some(CloseSaves {
                remaining: state.documents.iter().map(|doc| doc.id).collect(),
                waiting: None,
            });
            state.selected_index = 0;
            model.ui.active_modal = Some(ModalState::UnsavedChanges(state));
            super::merge_cmds(Some(Cmd::Redraw), reconcile(model))
        }
        2 => {
            model.ui.close_modal();
            finish(model, state.target)
        }
        _ => None,
    }
}

/// Only a live saving confirmation owns the close intention. Dismissing or
/// replacing it cancels closing, even if an already-dispatched save later succeeds.
pub(super) fn reconcile(model: &mut AppModel) -> Option<Cmd> {
    if !matches!(&model.ui.active_modal, Some(ModalState::UnsavedChanges(state)) if state.saves.is_some())
    {
        return None;
    }
    let Some(ModalState::UnsavedChanges(mut state)) = model.ui.active_modal.take() else {
        return None;
    };
    let mut commands = Vec::new();
    loop {
        let progress = state.saves.as_mut()?;
        if let Some(id) = progress.waiting {
            if let Some(doc) = model.editor_area.documents.get(&id) {
                if doc.save_busy() {
                    model.ui.active_modal = Some(ModalState::UnsavedChanges(state));
                    return (!commands.is_empty()).then_some(Cmd::Batch(commands));
                }
                if doc.is_modified || doc.save_error.is_some() {
                    let reason = doc
                        .save_error
                        .as_ref()
                        .map_or("changes were not saved".to_owned(), |(_, error)| {
                            error.clone()
                        });
                    model.ui.close_modal();
                    model.ui.set_status(format!(
                        "Closing cancelled: {reason}. Your tabs remain open."
                    ));
                    commands.push(Cmd::Redraw);
                    return Some(Cmd::Batch(commands));
                }
            }
            progress.waiting = None;
        }
        let Some(id) = progress.remaining.pop_front() else {
            model.ui.close_modal();
            // Also catches new edits/cell buffers or tabs opened during an async save.
            commands.extend(request(model, state.target));
            return Some(Cmd::Batch(commands));
        };
        progress.waiting = Some(id);
        if model
            .editor_area
            .documents
            .get(&id)
            .is_some_and(|doc| doc.save_busy())
        {
            continue;
        }
        if unsaved_documents(model, &state.target).iter().any(|doc| {
            doc.id == id
                && doc
                    .cells
                    .iter()
                    .enumerate()
                    .any(|(index, (_, position, text))| {
                        doc.cells[..index]
                            .iter()
                            .any(|(_, other_position, other_text)| {
                                position == other_position && text != other_text
                            })
                    })
        }) {
            model.ui.close_modal();
            model.ui.set_status("Closing cancelled: two views have different edits to the same CSV cell; finish one edit first");
            commands.push(Cmd::Redraw);
            return Some(Cmd::Batch(commands));
        }
        for editor_id in closing_editors(model, &state.target) {
            if model
                .editor_area
                .editors
                .get(&editor_id)
                .is_some_and(|editor| editor.document_id == Some(id))
            {
                commands.extend(super::csv::commit_edit(model, editor_id));
                if model
                    .editor_area
                    .editors
                    .get(&editor_id)
                    .and_then(|editor| editor.view_mode.as_csv())
                    .is_some_and(|csv| csv.editing.is_some())
                {
                    model.ui.close_modal();
                    model.ui.set_status("Closing cancelled: the CSV cell edit could not be applied and has been retained");
                    commands.push(Cmd::Redraw);
                    return Some(Cmd::Batch(commands));
                }
            }
        }
        let Some(doc) = model.editor_area.documents.get(&id) else {
            continue;
        };
        if doc.external_change.is_some() {
            model.ui.close_modal();
            model.ui.set_status(
                "Closing cancelled: resolve the external file change, then close again",
            );
            commands.extend(super::file_change::show_document(model, id, true));
            commands.push(Cmd::Redraw);
            return Some(Cmd::Batch(commands));
        }
        if !doc.is_modified {
            continue;
        }
        commands.extend(match doc.file_path.clone() {
            Some(path) => super::app::request_save(model, id, path, SaveReason::Manual),
            None => super::app::request_save_as(model, id),
        });
    }
}
