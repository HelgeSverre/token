//! Open-file observations and conflict policy. Filesystem work stays on the
//! ordered runtime worker; this layer only compares immutable snapshots.

use std::path::PathBuf;

use crate::commands::Cmd;
use crate::model::{
    AppModel, DiskContent, DocumentId, ExternalFileChange, FileRequest, FileRequestKind,
    ObservedFile,
};

fn supported(model: &AppModel, id: DocumentId) -> bool {
    !model.editor_area.editors.values().any(|editor| {
        editor.document_id == Some(id)
            && (editor.view_mode.is_image()
                || matches!(
                    editor.tab_content,
                    crate::model::TabContent::BinaryPlaceholder(_)
                ))
    })
}

pub(super) fn changed(model: &mut AppModel, paths: &[PathBuf]) -> Option<Cmd> {
    for doc in model.editor_area.documents.values_mut() {
        if doc.file_path.as_ref().is_some_and(|source| {
            paths.is_empty()
                || paths.iter().any(|path| {
                    source.starts_with(path)
                        || doc
                            .file_identity()
                            .is_some_and(|id| id.path().starts_with(path))
                })
        }) {
            doc.file_io.check_again = true;
        }
    }
    None // The shared update reconciliation schedules each eligible observation.
}

pub(super) fn reconcile(model: &mut AppModel) -> Option<Cmd> {
    if matches!(&model.ui.active_modal, Some(crate::model::ModalState::FileConflict(state))
        if !model.editor_area.documents.get(&state.document_id).is_some_and(|doc|
            doc.file_path.as_ref() == Some(&state.path) && doc.external_change.is_some()))
    {
        model.ui.close_modal();
    }
    let ids: Vec<_> = model
        .editor_area
        .documents
        .iter()
        .filter(|(id, doc)| doc.file_io.check_again && supported(model, **id))
        .map(|(&id, _)| id)
        .collect();
    let mut commands = Vec::new();
    for id in ids {
        let Some(doc) = model.editor_area.documents.get_mut(&id) else {
            continue;
        };
        if doc.file_path.is_none() {
            doc.file_io.check_again = false;
            continue;
        }
        if [
            FileRequestKind::Read,
            FileRequestKind::Write,
            FileRequestKind::Observe,
            FileRequestKind::SaveDialog,
        ]
        .into_iter()
        .any(|kind| doc.file_io.pending(kind))
        {
            continue;
        }
        doc.file_io.check_again = false;
        if let Some(target) = doc.begin_file_request(FileRequestKind::Observe) {
            commands.push(Cmd::ObserveFile(target));
        }
    }
    commands.extend(show_focused(model, false));
    (!commands.is_empty()).then_some(Cmd::Batch(commands))
}

pub(super) fn observed(
    model: &mut AppModel,
    target: FileRequest,
    observed: ObservedFile,
) -> Option<Cmd> {
    let id = target.document_id;
    let doc = model.editor_area.documents.get_mut(&id)?;
    if !doc.file_io.finish(&target, FileRequestKind::Observe) || doc.file_path != target.source_path
    {
        return None;
    }
    // Retargeting a symlink can leave the bytes unchanged while moving the
    // actual file to another directory. Refresh subscriptions and LSP routing
    // even when the buffer itself needs no reload.
    let identity_changed = observed
        .identity
        .as_ref()
        .is_some_and(|identity| doc.file_identity() != Some(identity));
    let mut commands = Vec::new();
    if identity_changed {
        doc.set_file_identity(observed.identity.clone());
        doc.diagnostics.clear();
        model.resync_viewports();
        commands.push(super::close_lsp_document(id));
        commands.extend(super::open_lsp_document(model, id));
        commands.push(Cmd::Redraw);
    }
    commands.extend(apply_observation(model, &target, observed));
    (!commands.is_empty()).then_some(Cmd::Batch(commands))
}

fn apply_observation(
    model: &mut AppModel,
    target: &FileRequest,
    observed: ObservedFile,
) -> Option<Cmd> {
    let id = target.document_id;
    let csv_edit = has_pending_cell_edit(model, id);
    let doc = model.editor_area.documents.get_mut(&id)?;
    let unchanged = match &observed.content {
        DiskContent::Text(text) => doc.saved_disk_content() == Some(text),
        DiskContent::Missing => doc.saved_disk_content().is_none(),
        DiskContent::Unavailable(_) => false,
    };
    if unchanged {
        return doc.external_change.take().map(|_| Cmd::Redraw);
    }
    // CSV's in-progress cell text is not yet part of the document Rope.
    // Treat an open cell editor as dirty rather than replacing its backing data.
    if let DiskContent::Text(text) = &observed.content {
        if !csv_edit && doc.buffer == *text {
            doc.record_saved_buffer(text.clone());
            return Some(Cmd::Redraw);
        }
        if !csv_edit
            && !doc.is_modified
            && doc.revision == target.revision
            && model.config.auto_reload
        {
            let mut target = doc.begin_file_request(FileRequestKind::Read)?;
            target.external_reload = true;
            let path = doc.file_path.clone()?;
            return super::app::finish_load(
                model,
                target,
                path,
                observed.identity,
                Ok(text.to_string()),
            );
        }
    }
    if doc
        .external_change
        .as_ref()
        .is_some_and(|change| change.observed == observed)
    {
        return None;
    }
    doc.external_change = Some(ExternalFileChange {
        observed,
        notified: false,
    });
    Some(Cmd::Redraw)
}

pub(super) fn show_focused(model: &mut AppModel, explicit: bool) -> Option<Cmd> {
    if model.ui.active_modal.is_some()
        || (!explicit && model.ui.focus != crate::model::ui::FocusTarget::Editor)
    {
        return None;
    }
    let doc = model.try_document()?;
    let change = doc.external_change.as_ref()?;
    if change.notified && !explicit {
        return None;
    }
    if has_pending_cell_edit(model, doc.id?) {
        if explicit {
            model.ui.set_status(
                "Finish or cancel the CSV cell edit before resolving the external change",
            );
            return Some(Cmd::redraw_status_bar());
        }
        return None;
    }
    let state = crate::model::FileConflictState {
        document_id: doc.id?,
        path: doc.file_path.clone()?,
        revision: doc.revision,
        observed: change.observed.clone(),
        selected_index: 0,
    };
    model.document_mut().external_change.as_mut()?.notified = true;
    model
        .ui
        .open_modal(crate::model::ModalState::FileConflict(state));
    Some(Cmd::Redraw)
}

pub(super) fn resolve(model: &mut AppModel, state: crate::model::FileConflictState) -> Option<Cmd> {
    use crate::model::FileConflictAction;
    if matches!(
        state.actions().get(state.selected_index),
        Some(FileConflictAction::Overwrite)
    ) && super::file_policy::saving_is_blocked(model, state.document_id)
    {
        model
            .ui
            .set_status("Waiting for file settings before overwriting");
        return Some(Cmd::Redraw);
    }
    model.ui.close_modal();
    if has_pending_cell_edit(model, state.document_id) {
        model
            .ui
            .set_status("Finish or cancel the CSV cell edit before resolving the external change");
        return Some(Cmd::Redraw);
    }
    let doc = model.editor_area.documents.get_mut(&state.document_id)?;
    if doc.file_path.as_ref() != Some(&state.path)
        || doc.revision != state.revision
        || !doc
            .external_change
            .as_ref()
            .is_some_and(|change| change.observed == state.observed)
    {
        if let Some(change) = &mut doc.external_change {
            change.notified = false;
        }
        model
            .ui
            .set_status("File changed while the dialog was open; review the current versions");
        return Some(Cmd::Redraw);
    }
    match *state.actions().get(state.selected_index)? {
        FileConflictAction::KeepEditing => Some(Cmd::Redraw),
        FileConflictAction::Reload => {
            let mut target = doc.begin_file_request(FileRequestKind::Read)?;
            target.external_reload = true;
            Some(Cmd::LoadFile {
                target,
                path: state.path,
            })
        }
        FileConflictAction::Overwrite => {
            let saved = match state.observed.content {
                DiskContent::Text(text) => Some(text),
                DiskContent::Missing => None,
                DiskContent::Unavailable(_) => return Some(Cmd::Redraw),
            };
            let settings = doc.text_settings;
            let cleanup = super::save_cleanup::apply(model, state.document_id, settings);
            let mut command = super::app::begin_save(model, state.document_id, state.path)?;
            if let Cmd::SaveFile { target, .. } = &mut command {
                target.write_guard.saved = saved;
                target.write_guard.queued = None;
                target.write_guard.save_as = false;
            }
            super::merge_cmds(cleanup, Some(command))
        }
        FileConflictAction::SaveAs => {
            let target = doc.begin_file_request(FileRequestKind::SaveDialog)?;
            Some(Cmd::ShowSaveFileDialog {
                suggested_path: Some(state.path),
                target,
            })
        }
    }
}

pub(super) fn has_pending_cell_edit(model: &AppModel, id: DocumentId) -> bool {
    model.editor_area.editors.values().any(|editor| {
        editor.document_id == Some(id)
            && editor
                .view_mode
                .as_csv()
                .is_some_and(|csv| csv.is_editing())
    })
}
