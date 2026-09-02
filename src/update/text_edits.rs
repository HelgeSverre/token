//! Shared LSP `TextEdit` / `WorkspaceEdit` application — the groundwork
//! rename, code actions, formatting and server-initiated
//! `workspace/applyEdit` all build on.
//!
//! Edits are planned against the pristine buffer (absolute LSP ranges ->
//! char offsets, descending start order) and applied as one undo batch
//! per document, with every editor showing that document keeping its
//! cursors honest.

use crate::commands::Cmd;
use crate::model::editor_area::DocumentId;
use crate::model::{AppModel, Cursor, Document, EditOperation, Selection};

use super::lsp::{find_document_by_uri, schedule_lsp_did_change};
use super::schedule_syntax_parse;

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

/// Net size change a pristine `offset` experiences from every planned
/// edit entirely before it (an insert exactly at `offset` counts).
pub(crate) fn shift_at(planned: &[PlannedEdit], offset: usize) -> i64 {
    planned
        .iter()
        .filter(|edit| edit.end() <= offset)
        .map(|edit| edit.inserted.chars().count() as i64 - edit.deleted.chars().count() as i64)
        .sum()
}

fn shifted(planned: &[PlannedEdit], offset: usize) -> usize {
    (offset as i64 + shift_at(planned, offset)).max(0) as usize
}

/// Applies `planned` (descending order, from [`plan_text_edits`]) to
/// `document_id` — not necessarily the focused document — as ONE undo
/// batch, shifting every cursor of every editor showing it by the net
/// delta of the edits before it. Returns the resync commands (redraw,
/// syntax parse, debounced `didChange`); `None` when nothing to apply.
pub(crate) fn apply_planned_edits(
    model: &mut AppModel,
    document_id: DocumentId,
    planned: &[PlannedEdit],
) -> Option<Cmd> {
    if planned.is_empty() {
        return None;
    }
    let doc = model.editor_area.documents.get(&document_id)?;

    // Editors showing this document, focused one first — the batch's
    // cursor snapshots belong to whoever undoes it, which is the focused
    // editor in practice.
    let mut editor_ids = model.editor_area.editors_for_document(document_id);
    if let Some(focused) = model.editor_area.focused_editor_id() {
        if let Some(pos) = editor_ids.iter().position(|id| *id == focused) {
            editor_ids.swap(0, pos);
        }
    }
    let pristine: Vec<Vec<usize>> = editor_ids
        .iter()
        .filter_map(|id| model.editor_area.editors.get(id))
        .map(|editor| {
            editor
                .cursors
                .iter()
                .map(|c| doc.cursor_to_offset(c.line, c.column))
                .collect()
        })
        .collect();
    let cursors_before: Vec<Cursor> = editor_ids
        .first()
        .and_then(|id| model.editor_area.editors.get(id))
        .map(|e| e.cursors.clone())
        .unwrap_or_else(|| vec![Cursor::at(0, 0)]);
    let anchor = cursors_before[0];

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

    for (editor_id, offsets) in editor_ids.iter().zip(&pristine) {
        let Some(editor) = model.editor_area.editors.get_mut(editor_id) else {
            continue;
        };
        for (idx, offset) in offsets.iter().enumerate() {
            let (line, col) = doc.offset_to_cursor(shifted(planned, *offset));
            editor.cursors[idx] = Cursor::at(line, col);
            editor.selections[idx] = Selection::new(editor.cursors[idx].to_position());
        }
    }
    let cursors_after = editor_ids
        .first()
        .and_then(|id| model.editor_area.editors.get(id))
        .map(|e| e.cursors.clone())
        .unwrap_or_else(|| cursors_before.clone());
    doc.push_edit(EditOperation::Batch {
        operations,
        cursors_before,
        cursors_after,
    });

    if model.editor_area.focused_document_id() == Some(document_id) {
        model.ensure_cursor_visible();
    }
    let mut cmds = vec![Cmd::redraw_editor()];
    cmds.extend(schedule_syntax_parse(model, document_id));
    cmds.extend(schedule_lsp_did_change(model, document_id));
    Some(Cmd::Batch(cmds))
}

/// What [`apply_workspace_edit`] did: files touched, edits applied, and
/// human-readable reasons for anything it could not apply.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct ApplyReport {
    pub files: usize,
    pub edits: usize,
    pub skipped: Vec<String>,
}

/// Applies a `WorkspaceEdit` across open documents, opening closed files
/// through the regular open path (so they get a real undo step and LSP
/// sync) without moving focus. `CreateFile`/`RenameFile`/`DeleteFile`
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
        let Some(document_id) = ensure_document_open(model, &uri, &mut cmds) else {
            report
                .skipped
                .push(format!("{}: could not open", uri.as_str()));
            continue;
        };
        let Some(doc) = model.editor_area.documents.get(&document_id) else {
            continue;
        };
        let planned = plan_text_edits(doc, &edits);
        report.files += 1;
        report.edits += planned.len();
        cmds.extend(apply_planned_edits(model, document_id, &planned));
    }
    let cmd = match cmds.len() {
        0 => None,
        _ => Some(Cmd::Batch(cmds)),
    };
    (cmd, report)
}

/// The open document for `uri`, opening the file in the focused group
/// (without changing the active tab) when it isn't. The open path's own
/// commands (didOpen, syntax parse) are appended to `cmds`.
fn ensure_document_open(
    model: &mut AppModel,
    uri: &lsp_types::Uri,
    cmds: &mut Vec<Cmd>,
) -> Option<DocumentId> {
    if let Some(id) = find_document_by_uri(model, uri) {
        return Some(id);
    }
    let path = crate::lsp::uri_to_path(uri)?;
    if !path.is_file() {
        return None;
    }
    let group_id = model.editor_area.focused_group_id;
    let active_tab = model
        .editor_area
        .groups
        .get(&group_id)
        .map(|g| g.active_tab_index);
    cmds.extend(super::navigation::open_or_focus(model, path));
    if let (Some(group), Some(index)) = (model.editor_area.groups.get_mut(&group_id), active_tab) {
        group.active_tab_index = index;
    }
    find_document_by_uri(model, uri)
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
        let mut model = AppModel::new(800, 600, 1.0, vec![]);
        model.document_mut().buffer = ropey::Rope::from_str(text);
        model
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

        let cmd = apply_planned_edits(&mut model, doc_id, &planned).expect("applied");
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
        let mut model = AppModel::new(800, 600, 1.0, vec![dir.join(names[0])]);
        for name in &names[1..] {
            super::super::navigation::open_or_focus(&mut model, dir.join(name));
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

        let (cmd, report) = apply_workspace_edit(&mut model, edit);
        assert_eq!((report.files, report.edits), (1, 1));
        assert!(report.skipped.is_empty());
        assert!(find_cmd(cmd.as_ref().unwrap(), &|c| matches!(
            c,
            Cmd::LspDidOpen { .. }
        )));
        // Focus stayed on a.rs; c.rs is open and edited.
        assert_eq!(
            model.document().file_path.as_deref(),
            Some(dir.path().join("a.rs").as_path())
        );
        let (c_id, _, _) = model.editor_area.find_open_file(&c_path).unwrap();
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
