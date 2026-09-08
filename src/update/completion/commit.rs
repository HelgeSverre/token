//! Commit-character lifecycle. The normal resolve worker and edit transaction
//! remain authoritative; no buffered input queue or separate range mapper.

use super::*;
use crate::completion::menu::PendingCommit;
use crate::messages::DocumentMsg;
use crate::model::{EditOperation, FocusTarget};

fn eligible(model: &AppModel) -> bool {
    model.editor_area.focused_document().is_some()
        && model.editor_area.focused_editor().is_some()
        && completion_enabled(model)
        && lsp_capable(model)
        && model.ui.focus == FocusTarget::Editor
        && !model.ui.has_modal()
        && model.ui.context_menu.is_none()
        && model.editor().is_plain_text_mode()
        && !model.editor().rectangle_selection.active
        && !model.editor().cursors.is_empty()
        && model.editor().selections.len() == model.editor().cursors.len()
        && model
            .editor()
            .selections
            .iter()
            .all(crate::model::Selection::is_empty)
}

pub(super) fn pending_is_valid(model: &AppModel) -> bool {
    let Some(pending) = &model.ui.completion_commit else {
        return false;
    };
    let Some(menu) = &model.ui.completion_menu else {
        return false;
    };
    let Some(overlay) = model.ui.cursor_overlay else {
        return false;
    };
    eligible(model)
        && model.document().id == Some(pending.document_id)
        && model.editor_area.focused_editor_id() == Some(pending.editor_id)
        && model.document().file_path.as_ref() == Some(&pending.file_path)
        && model.document().language == pending.language
        && model.document().revision == pending.revision
        && model.document().undo_stack.len() == pending.undo_len
        && model.editor().cursors == pending.cursors
        && model.editor().active_cursor_index == pending.active_cursor_index
        && menu.document_id == pending.document_id
        && menu.revision.wrapping_add(1) == pending.revision
        && menu.pending_resolve == Some(pending.selected)
        && overlay.kind == CursorOverlayKind::Completion
        && overlay.selected == pending.selected
}

pub(in crate::update) fn cancel_pending_commit(model: &mut AppModel) -> Option<Cmd> {
    model.ui.completion_commit.as_ref()?;
    dismiss_with_cleanup(model)
}

pub(in crate::update) fn reconcile_pending_commit(model: &mut AppModel) -> Option<Cmd> {
    if model.ui.completion_commit.is_some() && !pending_is_valid(model) {
        dismiss_with_cleanup(model)
    } else {
        None
    }
}

fn after_commit(model: &mut AppModel, character: char, edits: Option<Cmd>) -> Option<Cmd> {
    let inline = super::super::inline::after_document_edit(model, Some(character), false);
    let completion = sync_after_document_edit(
        model,
        false,
        char_type(character) == CharType::WordChar,
        Some(character),
    );
    super::super::merge_cmds(edits, super::super::merge_cmds(inline, completion))
}

/// `Some` consumes the character; the caller must not insert it a second time.
pub(in crate::update) fn try_commit_character(
    model: &mut AppModel,
    character: char,
) -> Option<Cmd> {
    let menu = model.ui.completion_menu.as_ref()?;
    let overlay = model.ui.cursor_overlay?;
    if character.is_control() || !eligible(model) {
        return None;
    }
    if overlay.kind != CursorOverlayKind::Completion
        || model.document().id != Some(menu.document_id)
        || model.document().revision != menu.revision
    {
        return None;
    }
    let doc = model.document();
    let cursor = *model.editor().active_cursor();
    let start = doc.cursor_to_offset(menu.query_start.line, menu.query_start.column);
    let end = doc.cursor_to_offset(cursor.line, cursor.column);
    if start > end || doc.buffer.slice(start..end) != menu.query {
        return None;
    }
    let MenuInsert::Lsp(data) = &menu.selected_item(overlay.selected)?.insert else {
        return None;
    };
    if !data.commit_characters.contains(&character) {
        return None;
    }
    if focused_server_id(model).as_ref() != Some(&data.server_id) {
        return None;
    }
    if !data.can_resolve || data.resolved {
        let data = data.clone();
        let edits = apply_lsp_accept(model, &data, Some(character));
        return after_commit(model, character, edits).or(Some(Cmd::Redraw));
    }

    let document_id = menu.document_id;
    let editor_id = model.editor_area.focused_editor_id()?;
    let file_path = model.document().file_path.clone()?;
    let language = model.document().language;
    // Resolve against the original item before staging the literal keystroke.
    // A pending Enter resolve is reused, not superseded by another request.
    let resolve = accept_selected(model);
    let insertion =
        super::super::document::update_document(model, DocumentMsg::InsertChar(character));
    model.ui.completion_commit = Some(PendingCommit {
        character,
        document_id,
        editor_id,
        file_path,
        language,
        revision: model.document().revision,
        cursors: model.editor().cursors.clone(),
        active_cursor_index: model.editor().active_cursor_index,
        undo_len: model.document().undo_stack.len(),
        selected: overlay.selected,
    });
    // Keep the menu's original revision until resolve; ordinary refiltering
    // would discard it at the punctuation boundary. Run the resolve effect LAST:
    // a missing server can synchronously accept, and the literal's older syntax
    // deadline must not overwrite the final accepted revision's deadline.
    // Follow-up signature/completion requests use the final caret after accept.
    super::super::merge_cmds(insertion, resolve).or(Some(Cmd::Redraw))
}

pub(super) fn finish_pending(
    model: &mut AppModel,
    data: &LspInsert,
    pending: PendingCommit,
) -> Option<Cmd> {
    // The caller has validated revision, pane, menu and every caret. Verify the
    // literal characters too before any mutation, then retract them only inside
    // this deterministic update. No intermediate frame or worker command is sent.
    let doc = model.document();
    let offsets: Option<Vec<_>> = pending
        .cursors
        .iter()
        .map(|cursor| {
            doc.cursor_to_offset(cursor.line, cursor.column)
                .checked_sub(1)
        })
        .collect();
    let Some(mut offsets) = offsets else {
        return dismiss_with_cleanup(model);
    };
    offsets.sort_unstable();
    offsets.dedup();
    if offsets
        .iter()
        .any(|&offset| doc.buffer.get_char(offset) != Some(pending.character))
    {
        return dismiss_with_cleanup(model);
    }
    let removals: Vec<_> = offsets
        .into_iter()
        .rev()
        .map(|start| PlannedEdit {
            start,
            deleted: pending.character.to_string(),
            inserted: String::new(),
        })
        .collect();
    // Restore the pristine coordinate basis so all existing UTF-16 conversion,
    // overlap filtering, snippet placement and multi-cursor rules remain shared.
    let _ = apply_planned_edits(model, pending.document_id, &removals, EditCarets::Preserve);
    let edits = apply_lsp_accept(model, data, Some(pending.character));
    join_history(model, pending.undo_len);
    after_commit(model, pending.character, edits)
}

/// Join literal typing, internal retraction and final acceptance. Keep the first
/// pane snapshot and the final one; ordered operations give exact Undo/Redo for
/// panes opened later as well. Never coalesce unrelated earlier user edits.
fn join_history(model: &mut AppModel, target_len: usize) {
    let history = &mut model.document_mut().undo_stack;
    while history.len() > target_len {
        let Some(last) = history.pop() else {
            break;
        };
        match (history.last_mut(), last) {
            (
                Some(EditOperation::Batch {
                    operations,
                    editors_after,
                    ..
                }),
                EditOperation::Batch {
                    operations: later,
                    editors_after: after,
                    ..
                },
            ) => {
                operations.extend(later);
                *editors_after = after;
            }
            (_, last) => {
                history.push(last);
                break;
            }
        }
    }
}
