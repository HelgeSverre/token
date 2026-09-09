//! Automatic-save policy; clocks and pending deadlines belong to the runtime.

use crate::{
    commands::Cmd,
    config::AutoSaveMode,
    messages::AutoSaveRequest,
    model::{AppModel, DocumentId, SaveReason, TabContent},
};

/// Whether a dirty document can enter the automatic-save pipeline right now.
pub fn eligible(model: &AppModel, id: DocumentId) -> bool {
    model.editor_area.documents.get(&id).is_some_and(|doc| {
        doc.is_modified
            && doc.file_path.is_some()
            && !doc.save_busy()
            && !super::file_policy::saving_is_blocked(model, id)
            && doc.external_change.is_none()
            && !doc
                .save_error
                .as_ref()
                .is_some_and(|(revision, _)| *revision == doc.revision)
    }) && !model.editor_area.editors.values().any(|editor| {
        editor.document_id == Some(id)
            && (editor.view_mode.is_image()
                || !matches!(editor.tab_content, TabContent::Text)
                || editor
                    .view_mode
                    .as_csv()
                    .is_some_and(|csv| csv.is_editing()))
    })
}

pub(super) fn save(model: &mut AppModel, request: AutoSaveRequest) -> Option<Cmd> {
    if request.policy != model.config.auto_save || !eligible(model, request.document_id) {
        return None;
    }
    let enabled = match request.reason {
        SaveReason::Idle => request.policy.mode.after_delay(),
        SaveReason::FocusLoss => request.policy.mode.on_focus_loss(),
        SaveReason::Manual | SaveReason::SaveAs => false,
    };
    let doc = model.editor_area.documents.get(&request.document_id)?;
    if !enabled || doc.revision != request.revision || doc.file_path.as_ref() != Some(&request.path)
    {
        return None;
    }
    super::app::request_save(model, request.document_id, request.path, request.reason)
}

/// Covers all accepted mutations, including background LSP edits and CSV commits.
/// A revision is notified once, so no-op editing and navigation cannot reset idle.
pub(super) fn reconcile(model: &mut AppModel) -> Option<Cmd> {
    let mut commands = Vec::new();
    for (&id, doc) in &mut model.editor_area.documents {
        if doc.pending_save.as_ref().is_some_and(|intent| {
            intent
                .automatic_policy
                .as_ref()
                .is_some_and(|policy| policy != &model.config.auto_save)
        }) {
            doc.pending_save = None;
        }
        if model.config.auto_save.mode == AutoSaveMode::Off
            || !doc.is_modified
            || doc.file_path.is_none()
        {
            if doc.auto_save_notified_revision.take().is_some() {
                commands.push(Cmd::CancelAutoSave(id));
            }
        } else if doc.auto_save_notified_revision != Some(doc.revision) {
            doc.auto_save_notified_revision = Some(doc.revision);
            commands.push(Cmd::ScheduleAutoSave {
                document_id: id,
                revision: doc.revision,
            });
        }
    }
    (!commands.is_empty()).then_some(Cmd::Batch(commands))
}
