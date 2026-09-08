//! Shared document edit transactions for ordinary editing, completion, Find,
//! and LSP `TextEdit` / `WorkspaceEdit` application.
//!
//! Edits are planned against the pristine buffer (absolute LSP ranges ->
//! char offsets, descending start order) and applied as one undo batch
//! per document, with every editor showing that document keeping its
//! cursors honest.

use crate::commands::Cmd;
use crate::model::document::EditorEditState;
use crate::model::editor_area::{DocumentId, EditorId};
use crate::model::{AppModel, Cursor, Document, EditOperation, Position, Selection};

use super::lsp::{find_document_by_uri, schedule_lsp_did_change};
use super::schedule_syntax_parse;

/// Common post-edit effects. Mutation planners own undo/cursor policy; every
/// accepted edit uses this boundary for highlight invalidation, syntax and LSP.
pub(crate) fn edit_effects(
    model: &mut AppModel,
    document_id: Option<DocumentId>,
    line_change: Option<(usize, usize, usize)>,
) -> Cmd {
    let mut cmds = vec![Cmd::redraw_editor()];
    if let Some(document_id) = document_id {
        if let Some((line, old_count, new_count)) = line_change {
            if let Some(highlights) = model
                .editor_area
                .documents
                .get_mut(&document_id)
                .and_then(|doc| doc.syntax_highlights.as_mut())
            {
                highlights.shift_for_edit(line, old_count, new_count);
            }
        }
        cmds.extend(schedule_syntax_parse(model, document_id));
        cmds.extend(schedule_lsp_did_change(model, document_id));
    }
    if cmds.len() == 1 {
        cmds.remove(0)
    } else {
        Cmd::Batch(cmds)
    }
}

/// One buffer mutation planned against the pristine buffer. Applied in
/// descending `start` order so earlier offsets stay valid throughout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PlannedEdit {
    pub start: usize,
    pub deleted: String,
    pub inserted: String,
}

impl PlannedEdit {
    pub fn end(&self) -> usize {
        self.start + self.deleted.chars().count()
    }
}

/// Plans `edits` against `doc`: descending start order (stable — equal
/// starts keep the spec's "array order is text order"), vanished ranges
/// dropped, and any edit overlapping an already-planned one dropped with
/// a warning (the spec forbids overlaps; a violation degrades to "that
/// edit lost", never corrupted text).
pub(crate) fn plan_text_edits(
    doc: &Document,
    edits: &[(lsp_types::Range, String)],
) -> Vec<PlannedEdit> {
    let mut spans: Vec<(usize, usize, usize, &str)> = edits
        .iter()
        .enumerate()
        .filter_map(|(idx, (range, new_text))| {
            if crate::lsp::position::range_vanished(doc, *range) {
                return None;
            }
            let start_pos = crate::lsp::lsp_to_position(doc, range.start);
            let end_pos = crate::lsp::lsp_to_position(doc, range.end);
            let start = doc.cursor_to_offset(start_pos.line, start_pos.column);
            let end = doc
                .cursor_to_offset(end_pos.line, end_pos.column)
                .max(start);
            Some((start, end, idx, new_text.as_str()))
        })
        .collect();
    // Descending start, then descending end (a replace at `p` applies
    // before an insert at `p`, so the insert lands ahead of it), then
    // descending array index (later inserts at the same point apply
    // first, so earlier ones end up ahead) — all spec text order.
    spans.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)).then(b.2.cmp(&a.2)));

    let mut planned = Vec::with_capacity(spans.len());
    let mut min_start_kept = usize::MAX;
    for (start, end, idx, inserted) in spans {
        if end > min_start_kept {
            tracing::warn!(
                "dropping overlapping text edit #{idx} ({start}..{end}) — server sent overlapping edits"
            );
            continue;
        }
        min_start_kept = start;
        planned.push(PlannedEdit {
            start,
            deleted: doc.buffer.slice(start..end).chars().collect(),
            inserted: inserted.to_owned(),
        });
    }
    planned
}

/// Map a character offset through one replacement. Inserts have right affinity.
/// A replacement's start stays at its start; interior positions retain their
/// relative offset, clamped to the inserted length. Its end follows the new end.
/// This also handles deletion without leaving positions beyond the new buffer.
fn offset_after_edit(offset: usize, start: usize, removed: usize, inserted: usize) -> usize {
    if offset < start {
        offset
    } else if offset >= start + removed {
        offset - removed + inserted
    } else {
        start + (offset - start).min(inserted)
    }
}

#[derive(Debug)]
struct OffsetEdit {
    start: usize,
    removed: usize,
    inserted: usize,
    final_start: usize,
}

/// Indexed mapping for non-overlapping edits in descending pristine order.
/// Build once for a set of offsets, then map each in O(log edits). Equal-point
/// inserts keep application order; a replacement precedes inserts at its start.
#[derive(Debug)]
pub(crate) struct EditOffsetMap {
    // Ascending starts, reversing application order even at equal positions.
    edits: Vec<OffsetEdit>,
}

impl EditOffsetMap {
    pub(crate) fn new(planned: &[PlannedEdit]) -> Self {
        let mut removed = 0;
        let mut inserted = 0;
        let mut previous_end = 0;
        let edits = planned
            .iter()
            .rev()
            .map(|edit| {
                debug_assert!(previous_end <= edit.start, "planned edits must not overlap");
                let entry = OffsetEdit {
                    start: edit.start,
                    removed: edit.deleted.chars().count(),
                    inserted: edit.inserted.chars().count(),
                    final_start: edit.start - removed + inserted,
                };
                previous_end = entry.start + entry.removed;
                removed += entry.removed;
                inserted += entry.inserted;
                entry
            })
            .collect();
        Self { edits }
    }

    pub(crate) fn map(&self, offset: usize) -> usize {
        let index = self.edits.partition_point(|edit| edit.start <= offset);
        let Some(edit) = index.checked_sub(1).map(|index| &self.edits[index]) else {
            return offset;
        };
        let relative = offset - edit.start;
        edit.final_start
            + if relative < edit.removed {
                relative.min(edit.inserted)
            } else {
                relative - edit.removed + edit.inserted
            }
    }

    /// Final offset inside one edit's inserted text, identified by application
    /// index. Later equal-point insertions move this copy, not its ownership.
    pub(crate) fn inserted_offset(&self, edit_index: usize, relative: usize) -> usize {
        let edit = &self.edits[self.edits.len() - 1 - edit_index];
        debug_assert!(relative <= edit.inserted);
        edit.final_start + relative
    }
}

#[derive(Debug, PartialEq, Eq)]
struct CursorOffsets {
    cursor: usize,
    anchor: usize,
    head: usize,
}

/// Live positions in every pane showing a document, independent of buffer
/// mutation. Reused for forward edits and the actual undo/redo operation order.
#[derive(Debug)]
pub(crate) struct EditPositions {
    editors: Vec<(EditorId, Vec<CursorOffsets>)>,
    find_scope: Option<(usize, usize)>,
    changed: bool,
}

impl EditPositions {
    /// Capture only panes whose live positions need mapping. Feature-owned final
    /// carets and history snapshots do not need offsets that will be overwritten.
    /// The Find scope is independent and is always captured when applicable.
    pub(crate) fn capture(
        model: &AppModel,
        document_id: DocumentId,
        include: impl Fn(EditorId) -> bool,
    ) -> Self {
        let doc = &model.editor_area.documents[&document_id];
        Self {
            editors: model
                .editor_area
                .editors
                .iter()
                .filter(|(id, editor)| editor.document_id == Some(document_id) && include(**id))
                .map(|(&id, editor)| {
                    let offsets = editor
                        .cursors
                        .iter()
                        .zip(&editor.selections)
                        .map(|(cursor, selection)| {
                            let position = cursor.to_position();
                            let offset = doc.cursor_to_offset(position.line, position.column);
                            // Collapsed selections share all three positions; even
                            // nonempty selections usually share the caret and head.
                            let endpoint = |pos: Position| {
                                if pos == position {
                                    offset
                                } else {
                                    doc.cursor_to_offset(pos.line, pos.column)
                                }
                            };
                            CursorOffsets {
                                cursor: offset,
                                anchor: endpoint(selection.anchor),
                                head: endpoint(selection.head),
                            }
                        })
                        .collect();
                    (id, offsets)
                })
                .collect(),
            find_scope: if model.editor_area.focused_document_id() == Some(document_id) {
                match &model.ui.active_modal {
                    Some(crate::model::ModalState::FindReplace(state)) if state.selection_only => {
                        state.scope
                    }
                    _ => None,
                }
            } else {
                None
            },
            changed: false,
        }
    }

    pub(crate) fn transform(&mut self, start: usize, removed: usize, inserted: usize) {
        self.transform_scope(start, removed, inserted);
        self.map_cursors(|offset| offset_after_edit(offset, start, removed, inserted));
    }

    fn transform_planned(&mut self, planned: &[PlannedEdit]) {
        if self.editors.is_empty() {
            // Explicit final carets often leave no live pane positions to map.
            // The two Find endpoints still need their sequential affinities.
            if self.find_scope.is_some() {
                for edit in planned {
                    self.transform(
                        edit.start,
                        edit.deleted.chars().count(),
                        edit.inserted.chars().count(),
                    );
                }
            }
            return;
        }
        let map = EditOffsetMap::new(planned);
        self.map_cursors(|offset| map.map(offset));
        for edit in map.edits.iter().rev() {
            self.transform_scope(edit.start, edit.removed, edit.inserted);
        }
    }

    fn transform_scope(&mut self, start: usize, removed: usize, inserted: usize) {
        self.changed |= removed != 0 || inserted != 0;
        if let Some((scope_start, scope_end)) = &mut self.find_scope {
            // Scope includes insertions at either boundary. Unlike a caret, its
            // start has left affinity, including undo of an entirely deleted scope.
            if removed != 0 || *scope_start != start {
                *scope_start = offset_after_edit(*scope_start, start, removed, inserted);
            }
            *scope_end = offset_after_edit(*scope_end, start, removed, inserted);
        }
    }

    fn map_cursors(&mut self, map: impl Fn(usize) -> usize) {
        for (_, offsets) in &mut self.editors {
            for positions in offsets {
                for offset in [
                    &mut positions.cursor,
                    &mut positions.anchor,
                    &mut positions.head,
                ] {
                    *offset = map(*offset);
                }
            }
        }
    }

    pub(crate) fn restore(self, model: &mut AppModel, document_id: DocumentId) {
        // Empty batches and other no-op commands must not reset peer navigation
        // state (desired columns, occurrence state or selection history).
        if !self.changed {
            return;
        }
        if let Some(scope) = self.find_scope {
            if let Some(crate::model::ModalState::FindReplace(state)) = &mut model.ui.active_modal {
                state.scope = Some(scope);
            }
        }
        let doc = &model.editor_area.documents[&document_id];
        let position = |offset| {
            let (line, column) = doc.offset_to_cursor(offset);
            Position::new(line, column)
        };
        for (id, offsets) in self.editors {
            let Some(editor) = model.editor_area.editors.get_mut(&id) else {
                continue;
            };
            for ((cursor, selection), offsets) in editor
                .cursors
                .iter_mut()
                .zip(&mut editor.selections)
                .zip(offsets)
            {
                let pos = position(offsets.cursor);
                let endpoint = |offset| {
                    if offset == offsets.cursor {
                        pos
                    } else {
                        position(offset)
                    }
                };
                *cursor = Cursor::at(pos.line, pos.column);
                *selection =
                    Selection::from_anchor_head(endpoint(offsets.anchor), endpoint(offsets.head));
            }
            editor.occurrence_state = None;
            editor.clear_selection_history();
        }
    }
}

/// Most edits retain mapped selections. Completion may instead place the
/// accepting pane's carets at feature-owned final offsets (e.g. a snippet stop).
pub(crate) enum EditCarets<'a> {
    Preserve,
    Place {
        editor_id: EditorId,
        offsets: &'a [usize],
        /// Original selection state if the planner normalized overlapping ranges.
        before: Option<EditorEditState>,
    },
}

/// Applies `planned` (descending order, from [`plan_text_edits`]) to
/// `document_id` — not necessarily the focused document — as ONE undo
/// batch, mapping every cursor and selection endpoint in every pane. Returns
/// the resync commands (redraw,
/// syntax parse, debounced `didChange`); `None` when nothing to apply.
pub(crate) fn apply_planned_edits(
    model: &mut AppModel,
    document_id: DocumentId,
    planned: &[PlannedEdit],
    carets: EditCarets<'_>,
) -> Option<Cmd> {
    if planned.is_empty() {
        return None;
    }
    let document = model.editor_area.documents.get(&document_id)?;
    let old_line_count = document.line_count();
    let edit_line = planned
        .iter()
        .map(|edit| document.offset_to_cursor(edit.start).0)
        .min()?;

    let editor_ids = model.editor_area.editors_for_document(document_id);
    let mut editors_before: Vec<_> = editor_ids
        .iter()
        .map(|id| EditorEditState::capture(*id, &model.editor_area.editors[id]))
        .collect();
    // When every caret has an explicit final position, mapping that pane's old
    // cursor/selection offsets only to overwrite them below is redundant. Keep
    // the general mapping for partial placement and all other panes.
    let mut positions = match &carets {
        EditCarets::Place {
            editor_id, offsets, ..
        } if model
            .editor_area
            .editors
            .get(editor_id)
            .is_some_and(|editor| {
                offsets.len() == editor.cursors.len()
                    && editor.selections.len() == editor.cursors.len()
            }) =>
        {
            EditPositions::capture(model, document_id, |id| id != *editor_id)
        }
        _ => EditPositions::capture(model, document_id, |_| true),
    };
    let anchor = model
        .editor_area
        .focused_editor()
        .filter(|editor| editor.document_id == Some(document_id))
        .map(|editor| *editor.active_cursor())
        .unwrap_or_default();

    positions.transform_planned(planned);
    let doc = model.editor_area.documents.get_mut(&document_id)?;
    let operations = planned
        .iter()
        .map(|edit| {
            doc.buffer.remove(edit.start..edit.end());
            doc.buffer.insert(edit.start, &edit.inserted);
            EditOperation::Replace {
                position: edit.start,
                deleted_text: edit.deleted.clone(),
                inserted_text: edit.inserted.clone(),
                cursor_before: anchor,
                cursor_after: anchor,
            }
        })
        .collect();

    positions.restore(model, document_id);
    let doc = &model.editor_area.documents[&document_id];
    if let EditCarets::Place {
        editor_id,
        offsets,
        before,
    } = carets
    {
        if let Some(before) = before {
            if let Some(state) = editors_before
                .iter_mut()
                .find(|state| state.editor_id == before.editor_id)
            {
                *state = before;
            }
        }
        if let Some(editor) = model.editor_area.editors.get_mut(&editor_id) {
            for ((cursor, selection), &offset) in editor
                .cursors
                .iter_mut()
                .zip(&mut editor.selections)
                .zip(offsets)
            {
                let (line, column) = doc.offset_to_cursor(offset);
                *cursor = Cursor::at(line, column);
                *selection = Selection::new(cursor.to_position());
            }
            editor.deduplicate_cursors();
            editor.occurrence_state = None;
            editor.clear_selection_history();
        }
    }
    let editors_after = editor_ids
        .iter()
        .map(|id| EditorEditState::capture(*id, &model.editor_area.editors[id]))
        .collect();
    model
        .editor_area
        .documents
        .get_mut(&document_id)?
        .push_edit(EditOperation::Batch {
            operations,
            editors_before,
            editors_after,
        });

    if model.editor_area.focused_document_id() == Some(document_id) {
        model.ensure_cursor_visible();
    }
    let new_line_count = model.editor_area.documents[&document_id].line_count();
    Some(edit_effects(
        model,
        Some(document_id),
        Some((edit_line, old_line_count, new_line_count)),
    ))
}

/// What [`apply_workspace_edit`] did: files touched, edits applied, and
/// human-readable reasons for anything it could not apply.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct ApplyReport {
    pub files: usize,
    pub edits: usize,
    pub skipped: Vec<String>,
}

/// Prepare every unopened target before applying any edit or reporting success.
/// The continuation owns its action rather than rereading a mutable popup row.
pub(crate) fn start_workspace_edit(
    model: &mut AppModel,
    edit: lsp_types::WorkspaceEdit,
    action: crate::model::WorkspaceEditAction,
) -> Option<Cmd> {
    use crate::model::PendingWorkspaceEdit;
    use lsp_types::{DocumentChangeOperation, DocumentChanges};
    let mut uris: Vec<_> = edit
        .changes
        .iter()
        .flat_map(|changes| changes.keys())
        .collect();
    match &edit.document_changes {
        Some(DocumentChanges::Edits(edits)) => {
            uris.extend(edits.iter().map(|edit| &edit.text_document.uri))
        }
        Some(DocumentChanges::Operations(operations)) => {
            uris.extend(operations.iter().filter_map(|operation| {
                if let DocumentChangeOperation::Edit(edit) = operation {
                    Some(&edit.text_document.uri)
                } else {
                    None
                }
            }))
        }
        None => {}
    }
    let mut waiting = std::collections::HashSet::new();
    let mut expected = Vec::new();
    let mut paths = std::collections::HashSet::new();
    let mut commands = Vec::new();
    for uri in uris {
        if let Some(id) = find_document_by_uri(model, uri) {
            let doc = &model.editor_area.documents[&id];
            if !expected.iter().any(|(found, _, _)| *found == id) {
                expected.push((id, doc.revision, doc.file_path.clone()));
            }
        } else if let Some(path) = crate::lsp::uri_to_path(uri) {
            if paths.insert(path.clone()) {
                if let Some(cmd) = super::layout::open_file_for_edit(model, path) {
                    if let Cmd::PrepareFileOpen(request) = &cmd {
                        waiting.insert(request.id());
                    }
                    commands.push(cmd);
                }
            }
        }
    }
    if waiting.is_empty() {
        return complete_workspace_edit(model, edit, action, false);
    }
    model
        .editor_area
        .file_opens
        .workspace_edits
        .push(PendingWorkspaceEdit {
            waiting,
            expected,
            failed: false,
            edit,
            action,
        });
    model.ui.set_status("Preparing workspace edit…");
    commands.push(Cmd::Redraw);
    Some(Cmd::Batch(commands))
}

pub(super) fn resume_workspace_opens(
    model: &mut AppModel,
    sequence: u64,
    document_id: Option<DocumentId>,
) -> Option<Cmd> {
    let pending = std::mem::take(&mut model.editor_area.file_opens.workspace_edits);
    let mut commands = Vec::new();
    for mut operation in pending {
        if operation.waiting.remove(&sequence) {
            if let Some(doc) = document_id.and_then(|id| model.editor_area.documents.get(&id)) {
                let id = document_id.expect("document lookup succeeded");
                if !operation.expected.iter().any(|(found, _, _)| *found == id) {
                    operation
                        .expected
                        .push((id, doc.revision, doc.file_path.clone()));
                }
            } else {
                operation.failed = true;
            }
        }
        if operation.waiting.is_empty() {
            let changed = operation.expected.iter().any(|(id, revision, path)| {
                model
                    .editor_area
                    .documents
                    .get(id)
                    .is_none_or(|doc| doc.revision != *revision || doc.file_path != *path)
            });
            commands.extend(complete_workspace_edit(
                model,
                operation.edit,
                operation.action,
                operation.failed || changed,
            ));
        } else {
            model.editor_area.file_opens.workspace_edits.push(operation);
        }
    }
    (!commands.is_empty()).then_some(Cmd::Batch(commands))
}

fn complete_workspace_edit(
    model: &mut AppModel,
    edit: lsp_types::WorkspaceEdit,
    action: crate::model::WorkspaceEditAction,
    preparation_failed: bool,
) -> Option<Cmd> {
    use crate::model::WorkspaceEditAction;
    let (cmd, report) = if preparation_failed {
        (
            None,
            ApplyReport {
                skipped: vec!["file preparation failed or a target changed while loading".into()],
                ..Default::default()
            },
        )
    } else {
        apply_workspace_edit(model, edit)
    };
    let mut commands: Vec<_> = cmd.into_iter().collect();
    let mut status = match action {
        WorkspaceEditAction::Rename if report.files == 0 && report.skipped.is_empty() => {
            "Nothing to rename".to_owned()
        }
        WorkspaceEditAction::Rename => format!(
            "Renamed in {} file(s), {} edit(s)",
            report.files, report.edits
        ),
        WorkspaceEditAction::CodeAction {
            title,
            command,
            document_id,
        } => {
            if report.skipped.is_empty() {
                if let (Some(command), Some(document_id)) = (command, document_id) {
                    commands.push(Cmd::LspExecuteCommand {
                        document_id,
                        command: command.command,
                        arguments: command.arguments,
                    });
                }
            }
            format!("Applied: {title}")
        }
        WorkspaceEditAction::Server {
            server_id,
            root,
            request_id,
            label,
        } => {
            let result = if report.skipped.is_empty() {
                serde_json::json!({ "applied": true })
            } else {
                serde_json::json!({ "applied": false, "failureReason": report.skipped.join("; ") })
            };
            commands.push(Cmd::LspRespondToServer {
                server_id,
                root,
                request_id,
                result,
            });
            label.unwrap_or_else(|| {
                format!("Applied {} edits in {} files", report.edits, report.files)
            })
        }
    };
    if !report.skipped.is_empty() {
        status.push_str(&format!(" (skipped: {})", report.skipped.join("; ")));
    }
    model.ui.set_status(status);
    commands.push(Cmd::Redraw);
    Some(Cmd::Batch(commands))
}

/// Applies a `WorkspaceEdit` to prepared, open text documents. The entry point
/// is `start_workspace_edit`, which loads closed targets first without moving
/// focus. `CreateFile`/`RenameFile`/`DeleteFile`
/// operations are reported in `skipped`, not applied.
pub(crate) fn apply_workspace_edit(
    model: &mut AppModel,
    edit: lsp_types::WorkspaceEdit,
) -> (Option<Cmd>, ApplyReport) {
    use lsp_types::{DocumentChangeOperation, DocumentChanges, OneOf, ResourceOp};

    let mut report = ApplyReport::default();
    let mut per_uri: Vec<(lsp_types::Uri, Vec<(lsp_types::Range, String)>)> = Vec::new();
    let mut push = |uri: lsp_types::Uri, edits: Vec<(lsp_types::Range, String)>| match per_uri
        .iter_mut()
        .find(|(u, _)| *u == uri)
    {
        Some((_, existing)) => existing.extend(edits),
        None => per_uri.push((uri, edits)),
    };
    let text_document_edit = |doc_edit: lsp_types::TextDocumentEdit| {
        let edits = doc_edit
            .edits
            .into_iter()
            .map(|e| match e {
                OneOf::Left(edit) => (edit.range, edit.new_text),
                OneOf::Right(annotated) => {
                    (annotated.text_edit.range, annotated.text_edit.new_text)
                }
            })
            .collect();
        (doc_edit.text_document.uri, edits)
    };

    if let Some(changes) = edit.changes {
        for (uri, edits) in changes {
            push(
                uri,
                edits.into_iter().map(|e| (e.range, e.new_text)).collect(),
            );
        }
    }
    match edit.document_changes {
        Some(DocumentChanges::Edits(docs)) => {
            for doc_edit in docs {
                let (uri, edits) = text_document_edit(doc_edit);
                push(uri, edits);
            }
        }
        Some(DocumentChanges::Operations(ops)) => {
            for op in ops {
                match op {
                    DocumentChangeOperation::Edit(doc_edit) => {
                        let (uri, edits) = text_document_edit(doc_edit);
                        push(uri, edits);
                    }
                    DocumentChangeOperation::Op(op) => {
                        let (kind, uri) = match &op {
                            ResourceOp::Create(c) => ("create", c.uri.as_str()),
                            ResourceOp::Rename(r) => ("rename", r.old_uri.as_str()),
                            ResourceOp::Delete(d) => ("delete", d.uri.as_str()),
                        };
                        report
                            .skipped
                            .push(format!("{kind} {uri}: file operations unsupported"));
                    }
                }
            }
        }
        None => {}
    }

    let mut cmds = Vec::new();
    for (uri, edits) in per_uri {
        let Some(document_id) = find_document_by_uri(model, &uri) else {
            report
                .skipped
                .push(format!("{}: could not open", uri.as_str()));
            continue;
        };
        let Some(doc) = model.editor_area.documents.get(&document_id) else {
            continue;
        };
        if model.editor_area.editors.values().any(|editor| {
            editor.document_id == Some(document_id)
                && (!matches!(editor.tab_content, crate::model::TabContent::Text)
                    || editor.view_mode.is_image())
        }) {
            report
                .skipped
                .push(format!("{}: not a text document", uri.as_str()));
            continue;
        }
        let planned = plan_text_edits(doc, &edits);
        report.files += 1;
        report.edits += planned.len();
        cmds.extend(apply_planned_edits(
            model,
            document_id,
            &planned,
            EditCarets::Preserve,
        ));
    }
    let cmd = match cmds.len() {
        0 => None,
        _ => Some(Cmd::Batch(cmds)),
    };
    (cmd, report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::messages::{DocumentMsg, Msg};
    use crate::update::update;
    use lsp_types::{Position, Range, TextEdit, WorkspaceEdit};
    use std::path::Path;

    fn range(line: u32, start: u32, end: u32) -> Range {
        Range::new(Position::new(line, start), Position::new(line, end))
    }

    fn model_with_text(text: &str) -> AppModel {
        let mut model = AppModel::new(800, 600, 1.0);
        model.document_mut().buffer = ropey::Rope::from_str(text);
        model
    }

    #[test]
    fn position_mapping_defines_insertion_replacement_and_deletion_boundaries() {
        let mapped = |removed, inserted| {
            (0..=7)
                .map(|offset| offset_after_edit(offset, 2, removed, inserted))
                .collect::<Vec<_>>()
        };
        assert_eq!(mapped(0, 3), [0, 1, 5, 6, 7, 8, 9, 10]);
        assert_eq!(mapped(3, 1), [0, 1, 2, 3, 3, 3, 4, 5]);
        assert_eq!(mapped(3, 5), [0, 1, 2, 3, 4, 7, 8, 9]);
        assert_eq!(mapped(3, 0), [0, 1, 2, 2, 2, 2, 3, 4]);
    }

    fn numeric_edit(start: usize, removed: usize, inserted: usize) -> PlannedEdit {
        PlannedEdit {
            start,
            deleted: "🙂".repeat(removed),
            inserted: "é".repeat(inserted),
        }
    }

    fn sequential_offset(planned: &[PlannedEdit], offset: usize) -> usize {
        planned.iter().fold(offset, |offset, edit| {
            offset_after_edit(
                offset,
                edit.start,
                edit.deleted.chars().count(),
                edit.inserted.chars().count(),
            )
        })
    }

    #[test]
    fn batch_offset_map_matches_sequential_mapping_exhaustively() {
        fn check(planned: &[PlannedEdit]) {
            let map = EditOffsetMap::new(planned);
            for offset in 0..=9 {
                assert_eq!(
                    map.map(offset),
                    sequential_offset(planned, offset),
                    "offset {offset}, plan {planned:?}"
                );
            }
            for (index, edit) in planned.iter().enumerate() {
                for relative in 0..=edit.inserted.chars().count() {
                    assert_eq!(
                        map.inserted_offset(index, relative),
                        sequential_offset(&planned[index + 1..], edit.start + relative),
                        "inserted offset {relative} in edit {index}, plan {planned:?}",
                    );
                }
            }
        }
        fn enumerate(planned: &mut Vec<PlannedEdit>, checked: &mut usize) {
            check(planned);
            *checked += 1;
            if planned.len() == 3 {
                return;
            }
            for start in 0..=5 {
                for removed in 0..=2 {
                    if planned
                        .last()
                        .is_some_and(|edit| start + removed > edit.start)
                    {
                        continue;
                    }
                    for inserted in 0..=3 {
                        planned.push(numeric_edit(start, removed, inserted));
                        enumerate(planned, checked);
                        planned.pop();
                    }
                }
            }
        }
        let mut checked = 0;
        enumerate(&mut Vec::new(), &mut checked);
        assert_eq!(checked, 42_601);
    }

    #[test]
    fn batch_position_mapping_preserves_sequential_find_scope_affinities() {
        let plans = [
            vec![],
            vec![numeric_edit(2, 0, 0)],
            vec![
                numeric_edit(5, 2, 1),
                numeric_edit(5, 0, 2),
                numeric_edit(2, 3, 0),
                numeric_edit(2, 0, 2),
                numeric_edit(0, 1, 3),
            ],
            vec![numeric_edit(2, 0, 1), numeric_edit(2, 0, 2)],
        ];
        for planned in plans {
            for pane_count in 0..=2 {
                for start in 0..=9 {
                    for end in start..=9 {
                        let positions = || EditPositions {
                            editors: (0..pane_count)
                                .map(|id| {
                                    (
                                        EditorId(id),
                                        (0..=9)
                                            .map(|offset| CursorOffsets {
                                                cursor: offset,
                                                anchor: 9 - offset,
                                                head: offset,
                                            })
                                            .collect(),
                                    )
                                })
                                .collect(),
                            find_scope: Some((start, end)),
                            changed: false,
                        };
                        let mut actual = positions();
                        actual.transform_planned(&planned);
                        let mut expected = positions();
                        for edit in &planned {
                            expected.transform(
                                edit.start,
                                edit.deleted.chars().count(),
                                edit.inserted.chars().count(),
                            );
                        }
                        assert_eq!(
                            actual.find_scope, expected.find_scope,
                            "scope {start}..{end}, plan {planned:?}"
                        );
                        assert_eq!(actual.editors, expected.editors);
                        assert_eq!(actual.changed, expected.changed);
                    }
                }
            }
        }
    }

    #[test]
    fn filtered_position_capture_keeps_find_scope_when_all_panes_are_excluded() {
        let mut model = model_with_text("abc tail");
        let document_id = model.document().id.unwrap();
        let mut find = crate::model::FindReplaceState::default();
        find.set_selection_only(
            true,
            model.document(),
            &Selection::from_anchor_head(
                crate::model::Position::new(0, 0),
                crate::model::Position::new(0, 3),
            ),
        );
        model
            .ui
            .open_modal(crate::model::ModalState::FindReplace(find));
        assert_eq!(
            EditPositions::capture(&model, document_id, |_| true)
                .editors
                .len(),
            1
        );
        let mut positions = EditPositions::capture(&model, document_id, |_| false);
        assert!(positions.editors.is_empty());
        assert_eq!(positions.find_scope, Some((0, 3)));
        positions.transform(0, 3, 0);
        model.document_mut().buffer.remove(0..3);
        positions.restore(&mut model, document_id);
        let Some(crate::model::ModalState::FindReplace(find)) = &model.ui.active_modal else {
            panic!("Find scope must survive history mapping");
        };
        assert_eq!(find.scope, Some((0, 0)));
    }

    #[test]
    fn planned_edits_preserve_reversed_selection_and_clamp_deleted_positions() {
        let mut model = model_with_text("α🙂bc\ntail\n");
        model.editor_mut().cursors[0] = Cursor::at(0, 3);
        model.editor_mut().selections[0] = Selection::from_anchor_head(
            crate::model::Position::new(1, 4),
            crate::model::Position::new(0, 3),
        );
        let id = model.document().id.unwrap();
        apply_planned_edits(
            &mut model,
            id,
            &[PlannedEdit {
                start: 1,
                deleted: "🙂bc\n".into(),
                inserted: "é".into(),
            }],
            EditCarets::Preserve,
        );
        assert_eq!(model.document().buffer.to_string(), "αétail\n");
        assert_eq!(model.editor().cursors[0], Cursor::at(0, 2));
        assert_eq!(
            model.editor().selections[0],
            Selection::from_anchor_head(
                crate::model::Position::new(0, 6),
                crate::model::Position::new(0, 2)
            )
        );
    }

    fn find_cmd(cmd: &Cmd, pred: &dyn Fn(&Cmd) -> bool) -> bool {
        match cmd {
            Cmd::Batch(cmds) => cmds.iter().any(|c| find_cmd(c, pred)),
            other => pred(other),
        }
    }

    #[test]
    fn two_edits_on_one_line_apply_in_descending_order() {
        let mut model = model_with_text("hello world\n");
        model.editor_mut().cursors[0] = Cursor::at(0, 11);
        model.editor_mut().clear_selection();
        let doc_id = model.document().id.unwrap();
        let edits = vec![
            (range(0, 0, 5), "greetings".to_owned()),
            (range(0, 6, 11), "you".to_owned()),
        ];
        let planned = plan_text_edits(model.document(), &edits);
        assert_eq!(planned.iter().map(|e| e.start).collect::<Vec<_>>(), [6, 0]);

        let cmd = apply_planned_edits(&mut model, doc_id, &planned, EditCarets::Preserve)
            .expect("applied");
        assert_eq!(model.document().buffer.to_string(), "greetings you\n");
        assert_eq!(model.document().undo_stack.len(), 1);
        assert!(model.document().is_modified);
        let cursor = model.editor().cursors[0];
        assert_eq!((cursor.line, cursor.column), (0, 13));
        assert!(find_cmd(&cmd, &|c| matches!(
            c,
            Cmd::LspScheduleDidChange { .. }
        )));

        update(&mut model, Msg::Document(DocumentMsg::Undo));
        assert_eq!(model.document().buffer.to_string(), "hello world\n");
    }

    #[test]
    fn an_overlapping_second_edit_is_dropped() {
        let model = model_with_text("hello world\n");
        let edits = vec![
            (range(0, 0, 5), "X".to_owned()),
            (range(0, 3, 8), "Y".to_owned()),
        ];
        let planned = plan_text_edits(model.document(), &edits);
        assert_eq!(planned.len(), 1);
        assert_eq!(planned[0].start, 3);
        assert_eq!(planned[0].inserted, "Y");
    }

    fn workspace(dir: &Path, names: &[&str]) -> AppModel {
        for name in names {
            std::fs::write(dir.join(name), "fn main() {}\n").unwrap();
        }
        let mut model = AppModel::with_document(
            800,
            600,
            1.0,
            crate::model::Document::from_file(dir.join(names[0])).unwrap(),
        );
        for name in &names[1..] {
            let cmd = super::super::layout::update_layout(
                &mut model,
                crate::messages::LayoutMsg::OpenFileInNewTab(dir.join(name)),
            );
            crate::update::finish_test_file_opens(&mut model, cmd);
        }
        model
    }

    fn rename_main(dir: &Path, name: &str) -> (lsp_types::Uri, Vec<TextEdit>) {
        (
            crate::lsp::path_to_uri(&dir.join(name)),
            vec![TextEdit::new(range(0, 3, 7), "start".to_owned())],
        )
    }

    fn focus_file(model: &mut AppModel, path: &Path) {
        let (_, group_id, tab_idx) = model.editor_area.find_open_file(path).unwrap();
        model.editor_area.focused_group_id = group_id;
        model
            .editor_area
            .groups
            .get_mut(&group_id)
            .unwrap()
            .active_tab_index = tab_idx;
    }

    #[test]
    fn a_two_file_workspace_edit_yields_one_undo_batch_per_document() {
        let dir = tempfile::tempdir().unwrap();
        let mut model = workspace(dir.path(), &["a.rs", "b.rs"]);
        #[allow(clippy::mutable_key_type)]
        let changes = std::collections::HashMap::from([
            rename_main(dir.path(), "a.rs"),
            rename_main(dir.path(), "b.rs"),
        ]);
        let edit = WorkspaceEdit::new(changes);

        let (cmd, report) = apply_workspace_edit(&mut model, edit);
        assert!(cmd.is_some());
        assert_eq!(report.files, 2);
        assert_eq!(report.edits, 2);
        assert!(report.skipped.is_empty());

        for name in ["a.rs", "b.rs"] {
            focus_file(&mut model, &dir.path().join(name));
            assert_eq!(model.document().buffer.to_string(), "fn start() {}\n");
            assert_eq!(model.document().undo_stack.len(), 1);
            update(&mut model, Msg::Document(DocumentMsg::Undo));
            assert_eq!(model.document().buffer.to_string(), "fn main() {}\n");
        }
    }

    #[test]
    fn an_edit_for_a_closed_file_opens_it_without_moving_focus() {
        let dir = tempfile::tempdir().unwrap();
        let mut model = workspace(dir.path(), &["a.rs"]);
        std::fs::write(dir.path().join("c.rs"), "fn main() {}\n").unwrap();
        let c_path = dir.path().join("c.rs");
        let edit = WorkspaceEdit::new(std::collections::HashMap::from([rename_main(
            dir.path(),
            "c.rs",
        )]));

        let cmd = start_workspace_edit(&mut model, edit, crate::model::WorkspaceEditAction::Rename);
        assert!(model.ui.is_loading);
        let cmd = crate::update::finish_test_file_opens(&mut model, cmd);
        assert!(!model.ui.is_loading);
        assert!(model
            .ui
            .transient_message
            .as_ref()
            .unwrap()
            .text
            .contains("Renamed in 1 file(s), 1 edit(s)"));
        assert!(find_cmd(cmd.as_ref().unwrap(), &|c| matches!(
            c,
            Cmd::LspDidOpen { .. }
        )));
        // Focus stayed on a.rs; c.rs is open and edited.
        assert_eq!(
            model.document().file_path.as_deref(),
            Some(dir.path().join("a.rs").as_path())
        );
        // The LSP edit opened its canonical URI spelling; resolve the fixture
        // query explicitly instead of relying on filesystem I/O in UI lookup.
        let (c_id, _, _) = model
            .editor_area
            .find_open_file(&c_path.canonicalize().unwrap())
            .unwrap();
        let c_doc = &model.editor_area.documents[&c_id];
        assert_eq!(c_doc.buffer.to_string(), "fn start() {}\n");
        assert!(c_doc.is_modified);
    }

    #[test]
    fn a_create_file_operation_is_reported_skipped() {
        let mut model = model_with_text("");
        let edit = WorkspaceEdit {
            changes: None,
            document_changes: Some(lsp_types::DocumentChanges::Operations(vec![
                lsp_types::DocumentChangeOperation::Op(lsp_types::ResourceOp::Create(
                    lsp_types::CreateFile {
                        uri: "file:///tmp/new.rs".parse().unwrap(),
                        options: None,
                        annotation_id: None,
                    },
                )),
            ])),
            change_annotations: None,
        };
        let (cmd, report) = apply_workspace_edit(&mut model, edit);
        assert!(cmd.is_none());
        assert_eq!(report.files, 0);
        assert_eq!(report.skipped.len(), 1);
        assert!(report.skipped[0].contains("file operations unsupported"));
    }
}
