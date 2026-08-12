//! `LspMsg` handlers (lsp-integration.md Phase 1).
//!
//! Phase 1 only has lifecycle traffic — the model mirror update and
//! restart requests. Feature messages (diagnostics, definition, hover,
//! completion) and their revision guards land with their phases.

use crate::commands::Cmd;
use crate::messages::LspMsg;
use crate::model::AppModel;

pub fn update_lsp(model: &mut AppModel, msg: LspMsg) -> Option<Cmd> {
    match msg {
        LspMsg::ServerStateChanged {
            server_id, state, ..
        } => {
            model.lsp.servers.insert(server_id, state);
            Some(Cmd::redraw_status_bar())
        }
        // The runtime's `LspManager` owns backoff/restart bookkeeping;
        // the model mirror just reflects whatever state it reports next.
        LspMsg::ServerExited { .. } => None,
        LspMsg::RestartServer { server_id } => Some(Cmd::LspRestartServer { server_id }),
    }
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
