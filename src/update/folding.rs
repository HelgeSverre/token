//! Pane state, candidate installation and shared edit reconciliation.
use super::text_edits::{EditOffsetMap, PlannedEdit};

pub(super) fn remember_closed(model: &mut AppModel, id: EditorId) {
    let Some(editor) = model.editor_area.editors.get(&id) else {
        return;
    };
    let Some(doc) = editor
        .document_id
        .and_then(|id| model.editor_area.documents.get(&id))
    else {
        return;
    };
    if !editor.is_plain_text_mode() {
        return;
    }
    let Some(path) = doc
        .file_identity()
        .map(|id| id.path())
        .or(doc.file_path.as_deref())
    else {
        return;
    };
    let Some(folds) = editor.folds.saved.clone().or_else(|| {
        editor
            .folds
            .pending
            .as_ref()
            .map(|pending| pending.saved.clone())
    }) else {
        return;
    };
    let record = crate::folding::persistence::RecentFolds {
        path: path.into(),
        folds,
    };
    model
        .editor_area
        .recent_folds
        .retain(|old| old.path != record.path);
    model.editor_area.recent_folds.insert(0, record);
    crate::folding::persistence::prune_recent(&mut model.editor_area.recent_folds);
}

pub(super) fn offer_recent(model: &AppModel, editor: &mut crate::model::EditorState) {
    let Some(doc) = editor
        .document_id
        .and_then(|id| model.editor_area.documents.get(&id))
    else {
        return;
    };
    let Some(path) = doc
        .file_identity()
        .map(|id| id.path())
        .or(doc.file_path.as_deref())
    else {
        return;
    };
    if !editor.is_plain_text_mode() {
        return;
    }
    if let Some(record) = model
        .editor_area
        .recent_folds
        .iter()
        .find(|record| record.path == path)
    {
        editor.folds.pending = Some(crate::folding::persistence::PendingFolds {
            saved: record.folds.clone(),
            top: None,
        });
        editor.restore_folds(doc);
    }
}
use crate::{
    commands::Cmd,
    folding::{FoldAction, FoldRegion},
    model::{AppModel, DocumentId, EditorId},
};

pub(super) fn action(
    model: &mut AppModel,
    editor_id: Option<EditorId>,
    header: Option<usize>,
    action: FoldAction,
) -> Option<Cmd> {
    let id = editor_id.or_else(|| model.editor().id)?;
    let editor = model.editor_area.editors.get_mut(&id)?;
    let doc = model.editor_area.documents.get(&editor.document_id?)?;
    if !editor.fold(doc, action, header) {
        return None;
    }
    super::completion::dismiss(model);
    super::inline::dismiss(model);
    Some(Cmd::redraw_editor())
}

pub(super) fn install(model: &mut AppModel, id: DocumentId) {
    let doc = &model.editor_area.documents[&id];
    let Some(candidates) = &doc.folds else {
        return;
    };
    for editor in model
        .editor_area
        .editors
        .values_mut()
        .filter(|e| e.document_id == Some(id))
    {
        let collapsed = editor
            .folds
            .collapsed()
            .iter()
            .filter_map(|old| {
                candidates
                    .regions
                    .binary_search_by_key(&old.header, |region| region.header)
                    .ok()
                    .map(|index| &candidates.regions[index])
                    .filter(|new| new.end == old.end && new.kind == old.kind)
                    .cloned()
            })
            .collect();
        editor.folds.replace(collapsed);
        editor.restore_folds(doc);
        editor.reveal_folded_carets(doc);
        editor.ensure_wrap_cache(doc);
        editor.refresh_saved_folds(doc);
    }
}

/// Called before any render, including navigation paths without cursor reveal.
pub(super) fn reconcile(model: &mut AppModel) -> Option<Cmd> {
    let mut changed = false;
    let mut commands = Vec::new();
    for (&document_id, document) in &mut model.editor_area.documents {
        if document.fold_policy_generation != document.text_policy_generation {
            document.fold_policy_generation = document.text_policy_generation;
            commands.push(Cmd::DebouncedSyntaxParse {
                document_id,
                revision: document.revision,
                delay_ms: 0,
            });
        }
    }
    for editor in model.editor_area.editors.values_mut() {
        let Some(doc) = editor
            .document_id
            .and_then(|id| model.editor_area.documents.get(&id))
        else {
            continue;
        };
        if doc.folds.as_ref().is_none_or(|folds| {
            folds.stamp.language != doc.language
                || folds.stamp.policy_generation != doc.text_policy_generation
        }) {
            changed |= editor.folds.replace(Vec::new());
        }
        changed |= editor.reveal_folded_carets(doc);
        changed |= editor.restore_folds(doc);
        editor.ensure_wrap_cache(doc);
        editor.refresh_saved_folds(doc);
    }
    if changed {
        commands.push(Cmd::redraw_editor());
    }
    (!commands.is_empty()).then_some(Cmd::Batch(commands))
}

/// Regions touched by a transaction expand; the rest follow the same offset map
/// as selections. This covers all panes, formatter edits, cleanup and history.
pub(crate) fn before_edits(model: &mut AppModel, id: DocumentId, planned: &[PlannedEdit]) {
    let mapping = EditOffsetMap::new(planned);
    let ascending: Vec<_> = planned.iter().rev().collect();
    let map = |region: &FoldRegion| -> Option<FoldRegion> {
        let end_index = ascending.partition_point(|edit| edit.start < region.offsets.end);
        let start_index = ascending.partition_point(|edit| edit.end() < region.offsets.start);
        let touched = ascending[start_index.min(end_index)..end_index]
            .iter()
            .any(|edit| {
                let insertion_before_header = edit.deleted.is_empty()
                    && edit.start == region.offsets.start
                    && edit.inserted.ends_with(['\n', '\r']);
                !insertion_before_header
                    && edit.start < region.offsets.end
                    && edit.end() >= region.offsets.start
            });
        if touched {
            return None;
        }
        let mut result = region.clone();
        result.offsets = mapping.map(region.offsets.start)..mapping.map_left(region.offsets.end);
        Some(result)
    };
    for editor in model
        .editor_area
        .editors
        .values_mut()
        .filter(|e| e.document_id == Some(id))
    {
        editor
            .folds
            .replace(editor.folds.collapsed().iter().filter_map(&map).collect());
    }
    if let Some(candidates) = model
        .editor_area
        .documents
        .get_mut(&id)
        .and_then(|doc| doc.folds.as_mut())
    {
        let candidates = std::sync::Arc::make_mut(candidates);
        candidates.regions = candidates.regions.iter().filter_map(map).collect();
    }
}

pub(crate) fn after_edits(model: &mut AppModel, id: DocumentId) {
    let Some(doc) = model.editor_area.documents.get_mut(&id) else {
        return;
    };
    let buffer = &doc.buffer;
    let update = |region: &mut FoldRegion| -> bool {
        if region.offsets.end > buffer.len_chars() {
            return false;
        }
        let header = buffer.char_to_line(region.offsets.start);
        let end = if region.offsets.end == buffer.len_chars()
            && buffer.line(buffer.len_lines() - 1).len_chars() > 0
        {
            buffer.len_lines()
        } else {
            buffer.char_to_line(region.offsets.end)
        };
        if buffer.line_to_char(header) != region.offsets.start || header + 1 >= end {
            return false;
        }
        region.header = header;
        region.end = end;
        true
    };
    for editor in model
        .editor_area
        .editors
        .values_mut()
        .filter(|e| e.document_id == Some(id))
    {
        let mut collapsed = editor.folds.collapsed().to_vec();
        collapsed.retain_mut(&update);
        editor.folds.replace(collapsed);
    }
    if let Some(candidates) = &mut doc.folds {
        std::sync::Arc::make_mut(candidates)
            .regions
            .retain_mut(update);
    }
}
