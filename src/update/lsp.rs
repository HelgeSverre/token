//! `LspMsg` handlers (lsp-integration.md Phase 1).
//!
//! Phase 1 only has lifecycle traffic — the model mirror update and
//! restart requests. Feature messages (diagnostics, definition, hover,
//! completion) and their revision guards land with their phases.

use crate::commands::Cmd;
use crate::lsp::ServerState;
use crate::messages::LspMsg;
use crate::model::editor_area::DocumentId;
use crate::model::AppModel;

/// A short status-bar transient for a server-state change, or `None` for
/// states not worth flashing (`Indexing` fires often via `$/progress`
/// begin/end and would spam the bar).
fn status_transient_for(server_id: &crate::lsp::LspServerId, state: ServerState) -> Option<String> {
    match state {
        ServerState::Starting => Some(format!("{server_id}: starting…")),
        ServerState::Ready => Some(format!("{server_id}: ready")),
        ServerState::Restarting { attempt } => {
            Some(format!("{server_id}: restarting (attempt {attempt})…"))
        }
        ServerState::Failed => Some(format!("{server_id}: failed to start")),
        ServerState::Missing => Some(format!("{server_id}: not found on PATH")),
        ServerState::Indexing | ServerState::ShuttingDown => None,
    }
}

/// A document just gained a file path + language (opened, or Save As) —
/// the "matching document" from the design doc's Document Synchronization
/// section. Returns the `Cmd`s that spawn a server if needed and send
/// `didOpen`; `None` if the document has no path (untitled docs are
/// unsynced) or doesn't exist.
pub fn open_lsp_document(model: &AppModel, document_id: DocumentId) -> Option<Cmd> {
    let doc = model.editor_area.documents.get(&document_id)?;
    let file_path = doc.file_path.clone()?;
    Some(Cmd::Batch(vec![
        Cmd::LspEnsureServer {
            language: doc.language,
            file_path: file_path.clone(),
        },
        Cmd::LspDidOpen {
            document_id,
            file_path,
            language: doc.language,
        },
    ]))
}

/// `didClose` — call only from `release_document_if_unreferenced`, never
/// on tab close alone (documents are refcounted across splits/groups).
pub fn close_lsp_document(document_id: DocumentId) -> Cmd {
    Cmd::LspDidClose { document_id }
}

/// `didSave` after a successful save.
pub fn save_lsp_document(document_id: DocumentId) -> Cmd {
    Cmd::LspDidSave { document_id }
}

/// Schedules a debounced `didChange` for an edited document — pair with
/// `schedule_syntax_parse` at edit sites.
pub fn schedule_lsp_did_change(model: &AppModel, document_id: DocumentId) -> Option<Cmd> {
    let doc = model.editor_area.documents.get(&document_id)?;
    Some(Cmd::LspScheduleDidChange {
        document_id,
        revision: doc.revision,
    })
}

pub fn update_lsp(model: &mut AppModel, msg: LspMsg) -> Option<Cmd> {
    match msg {
        LspMsg::ServerStateChanged {
            server_id, state, ..
        } => {
            if let Some(message) = status_transient_for(&server_id, state) {
                model.ui.set_status(message);
            }
            model.lsp.servers.insert(server_id, state);
            Some(Cmd::redraw_status_bar())
        }
        // The runtime's `LspManager` owns backoff/restart bookkeeping;
        // the model mirror just reflects whatever state it reports next.
        LspMsg::ServerExited { .. } => None,
        LspMsg::RestartServer { server_id } => Some(Cmd::LspRestartServer { server_id }),
        // Consumed by `ServerHandle::graceful_shutdown`'s own blocking
        // poll of `msg_rx` during quit teardown — never reaches `update()`
        // in practice, but the match must stay exhaustive.
        LspMsg::ShutdownAcked { .. } => None,
        LspMsg::DiagnosticsPublished {
            uri, diagnostics, ..
        } => {
            // Staleness (out-of-order `version`) is already filtered by
            // the runtime before this reaches `update()`; the
            // authoritative store lives there too. This is purely the
            // model projection onto whatever document (if any) has
            // `uri` open — a publish for an unopened file is a silent
            // no-op here (still retained in the runtime's store).
            let document_id = find_document_by_uri(model, &uri)?;
            let doc = model.editor_area.documents.get_mut(&document_id)?;
            doc.diagnostics = diagnostics;
            Some(Cmd::redraw_editor())
        }
    }
}

/// Finds the open document whose file path canonicalizes to `uri`, per
/// `lsp::path_to_uri` (the "raw `PathBuf`s are never compared" rule from
/// the design doc's URIs and Paths section).
fn find_document_by_uri(model: &AppModel, uri: &lsp_types::Uri) -> Option<DocumentId> {
    model
        .editor_area
        .documents
        .iter()
        .find(|(_, doc)| {
            doc.file_path
                .as_deref()
                .is_some_and(|path| &crate::lsp::path_to_uri(path) == uri)
        })
        .map(|(id, _)| *id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lsp::{LspServerId, ServerState};
    use std::path::PathBuf;

    fn model() -> AppModel {
        AppModel::new(800, 600, 1.0, vec![])
    }

    #[test]
    fn server_state_changed_updates_the_mirror() {
        let mut model = model();
        let id = LspServerId::from("rust-analyzer");
        update_lsp(
            &mut model,
            LspMsg::ServerStateChanged {
                server_id: id.clone(),
                root: PathBuf::from("/ws"),
                state: ServerState::Starting,
            },
        );
        assert_eq!(model.lsp.servers.get(&id), Some(&ServerState::Starting));

        update_lsp(
            &mut model,
            LspMsg::ServerStateChanged {
                server_id: id.clone(),
                root: PathBuf::from("/ws"),
                state: ServerState::Ready,
            },
        );
        assert_eq!(model.lsp.servers.get(&id), Some(&ServerState::Ready));
    }

    #[test]
    fn ready_state_flashes_a_status_transient() {
        let mut model = model();
        let id = LspServerId::from("rust-analyzer");
        update_lsp(
            &mut model,
            LspMsg::ServerStateChanged {
                server_id: id,
                root: PathBuf::from("/ws"),
                state: ServerState::Ready,
            },
        );
        assert!(model
            .ui
            .transient_message
            .as_ref()
            .is_some_and(|t| t.text.contains("ready")));
    }

    #[test]
    fn indexing_does_not_flash_a_status_transient() {
        let mut model = model();
        let before = model.ui.transient_message.clone();
        let id = LspServerId::from("rust-analyzer");
        update_lsp(
            &mut model,
            LspMsg::ServerStateChanged {
                server_id: id,
                root: PathBuf::from("/ws"),
                state: ServerState::Indexing,
            },
        );
        assert_eq!(
            model.ui.transient_message.map(|t| t.text),
            before.map(|t| t.text),
            "Indexing must not overwrite whatever status was already showing"
        );
    }

    #[test]
    fn restart_server_produces_the_restart_command() {
        let mut model = model();
        let id = LspServerId::from("pyright");
        let cmd = update_lsp(
            &mut model,
            LspMsg::RestartServer {
                server_id: id.clone(),
            },
        );
        assert!(matches!(cmd, Some(Cmd::LspRestartServer { server_id }) if server_id == id));
    }
}
