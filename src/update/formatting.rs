//! Provider-independent formatting routing and edit application.
use super::text_edits::{apply_planned_edits, plan_text_edits};
use crate::commands::Cmd;
use crate::messages::FormattingMsg;
use crate::model::{AppModel, DocumentId};

/// Request formatting for the focused selection or document. Saving has its own
/// document-targeted continuation and does not depend on interactive focus.
pub(crate) fn request_formatting(model: &mut AppModel, selection_only: bool) -> Option<Cmd> {
    // Interactive formatting replaces the runtime formatting slot. Settle any
    // earlier save first so its continuation cannot be orphaned by supersession.
    let prior = model.try_document()?.pending_save.clone();
    let settled = prior.and_then(|intent| {
        let revision = intent.revision;
        super::app::finish_save_formatting(model, intent, revision, None)
    });
    let doc = model.try_document()?;
    let document_id = doc.id?;
    if !model.editor().is_plain_text_mode() {
        return settled;
    }
    let range = if selection_only {
        let sel = *model.editor().active_selection();
        if sel.is_empty() {
            model.ui.set_status("No selection to format");
            return super::merge_cmds(settled, Some(Cmd::redraw_status_bar()));
        }
        Some(lsp_types::Range::new(
            crate::lsp::position_to_lsp(doc, sel.start()),
            crate::lsp::position_to_lsp(doc, sel.end()),
        ))
    } else {
        None
    };
    super::merge_cmds(settled, route(model, document_id, range, None))
}

pub(super) fn formatting_options(
    settings: crate::model::DocumentTextSettings,
) -> lsp_types::FormattingOptions {
    lsp_types::FormattingOptions {
        tab_size: settings.indent_size as u32,
        insert_spaces: settings.indent_style == crate::model::IndentStyle::Space,
        trim_trailing_whitespace: settings.trim_trailing_whitespace,
        insert_final_newline: settings.insert_final_newline,
        ..Default::default()
    }
}

pub(super) fn update_formatting(model: &mut AppModel, msg: FormattingMsg) -> Option<Cmd> {
    match msg {
        FormattingMsg::FormatDocument { selection_only } => {
            request_formatting(model, selection_only)
        }
        FormattingMsg::FormattingResolved {
            document_id,
            revision,
            edits,
            save,
        } => {
            if let Some(intent) = save {
                if intent.document_id != document_id {
                    return None;
                }
                return super::app::finish_save_formatting(model, intent, revision, edits);
            }
            if model
                .editor_area
                .documents
                .get(&document_id)
                .is_none_or(|doc| doc.revision != revision)
            {
                return None;
            }
            let mut cmd = None;
            match edits.as_deref() {
                Some([]) => model.ui.set_status("Already formatted"),
                Some(edits) => {
                    let planned =
                        plan_text_edits(model.editor_area.documents.get(&document_id)?, edits);
                    cmd = apply_planned_edits(
                        model,
                        document_id,
                        &planned,
                        super::text_edits::EditCarets::Preserve,
                    );
                }
                None => model
                    .ui
                    .set_status("Formatting not supported by this server"),
            }
            super::merge_cmds(cmd, Some(Cmd::redraw_status_bar()))
        }
        FormattingMsg::ExternalResolved {
            document_id,
            language,
            revision,
            result,
            save,
            ..
        } => {
            let doc = model.editor_area.documents.get(&document_id)?;
            if let Some(intent) = &save {
                if !intent.is_current(doc)
                    || !doc.pending_save.as_ref().is_some_and(|pending| {
                        pending.resolution_generation == intent.resolution_generation
                    })
                {
                    return None;
                }
            }
            if save.is_none() && (doc.revision != revision || doc.language != language) {
                return None;
            }
            let (edits, error) = match result {
                Ok(text) => {
                    let edits = replacement_edit(doc, &text).into_iter().collect();
                    (Some(edits), None)
                }
                Err(error) => (None, Some(error)),
            };
            let saving = save.is_some();
            let cmd = update_formatting(
                model,
                FormattingMsg::FormattingResolved {
                    document_id,
                    revision,
                    edits,
                    save,
                },
            );
            if let Some(error) = error.filter(|_| !saving || cmd.is_some()) {
                model.ui.set_status(if saving {
                    format!("Formatting failed, saved unformatted: {error}")
                } else {
                    format!("Formatting failed: {error}")
                });
            }
            super::merge_cmds(cmd, Some(Cmd::redraw_status_bar()))
        }
    }
}

/// Select commands before LSP; selection formatting always remains an LSP request.
pub(super) fn route(
    model: &AppModel,
    document_id: DocumentId,
    range: Option<lsp_types::Range>,
    save: Option<crate::model::SaveIntent>,
) -> Option<Cmd> {
    let doc = model.editor_area.documents.get(&document_id)?;
    if range.is_none() {
        if let Some(formatter) = model
            .config
            .formatters
            .get(&doc.language)
            .filter(|value| value.enabled)
        {
            return Some(Cmd::RunFormatter {
                document_id,
                revision: doc.revision,
                formatter: formatter.clone(),
                text: doc.buffer.to_string(),
                file: save
                    .as_ref()
                    .map(|intent| intent.path.clone())
                    .or_else(|| doc.file_path.clone()),
                language: doc.language,
                workspace: model.workspace_root().cloned(),
                save,
            });
        }
    }
    Some(Cmd::LspRequestFormatting {
        document_id,
        revision: doc.revision,
        range,
        options: formatting_options(
            save.as_ref()
                .map_or(doc.text_settings, |intent| intent.settings),
        ),
        save,
    })
}

/// Trim unchanged edges so existing offsets outside the changed span track edits.
fn replacement_edit(
    doc: &crate::model::Document,
    text: &str,
) -> Option<(lsp_types::Range, String)> {
    let mut prefix = doc
        .buffer
        .chars()
        .zip(text.chars())
        .take_while(|(a, b)| a == b)
        .count();
    let old_len = doc.buffer.len_chars();
    let new_len = text.chars().count();
    if prefix == old_len && prefix == new_len {
        return None;
    }
    let mut suffix = doc
        .buffer
        .chars_at(old_len)
        .reversed()
        .zip(text.chars().rev())
        .take((old_len - prefix).min(new_len - prefix))
        .take_while(|(a, b)| a == b)
        .count();
    // LSP positions cannot address the middle of CRLF (or other line endings).
    // Expand a trimmed boundary to include the complete ending before conversion.
    let (line, column) = doc.offset_to_cursor(prefix);
    prefix -= column.saturating_sub(doc.line_length(line));
    let (line, column) = doc.offset_to_cursor(old_len - suffix);
    if column > doc.line_length(line) {
        suffix = old_len
            - doc
                .buffer
                .line_to_char((line + 1).min(doc.buffer.len_lines()));
    }
    let (start_line, start_column) = doc.offset_to_cursor(prefix);
    let (end_line, end_column) = doc.offset_to_cursor(old_len - suffix);
    Some((
        lsp_types::Range::new(
            crate::lsp::position_to_lsp(doc, crate::model::Position::new(start_line, start_column)),
            crate::lsp::position_to_lsp(doc, crate::model::Position::new(end_line, end_column)),
        ),
        text.chars()
            .skip(prefix)
            .take(new_len - prefix - suffix)
            .collect(),
    ))
}
