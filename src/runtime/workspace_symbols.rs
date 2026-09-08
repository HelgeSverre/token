//! One debounced workspace query fans out to existing capable server instances.
//! Unlike document FeatureSlot, this has one owner and multiple pending replies.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use token::lsp::workspace_symbols::{
    finish_results, SymbolProvider, SymbolResults, SymbolSearchRequest,
};
use token::messages::{LspMsg, Msg};

use super::{App, RequestKey};

const DEBOUNCE: Duration = Duration::from_millis(150);
const TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Default)]
pub(super) struct SymbolSearch {
    request: Option<SymbolSearchRequest>,
    debounce: Option<Instant>,
    pending: HashMap<RequestKey, (u64, Instant)>,
    results: SymbolResults,
}

impl SymbolSearch {
    pub(super) fn deadline(&self) -> Option<Instant> {
        self.debounce
            .into_iter()
            .chain(self.pending.values().map(|(_, deadline)| *deadline))
            .min()
    }

    fn finish(&mut self) -> Option<Msg> {
        if self.debounce.is_some() || !self.pending.is_empty() {
            return None;
        }
        let request = self.request.take()?;
        finish_results(&request.query, &mut self.results);
        Some(Msg::Lsp(LspMsg::WorkspaceSymbolsReady {
            request,
            results: std::mem::take(&mut self.results),
        }))
    }
}

impl App {
    pub(super) fn set_workspace_symbol_query(&mut self, request: Option<SymbolSearchRequest>) {
        let previous = std::mem::take(&mut self.lsp.symbols);
        for (key, (generation, _)) in &previous.pending {
            self.cancel_workspace_symbol_request(key, *generation);
        }
        self.lsp.symbols = SymbolSearch {
            debounce: request.as_ref().map(|_| Instant::now() + DEBOUNCE),
            request,
            ..Default::default()
        };
    }

    fn cancel_workspace_symbol_request(&self, key: &RequestKey, generation: u64) {
        // Request IDs can be reused after a restart. Never cancel a request
        // belonging to the replacement process while retiring an old query.
        if self
            .lsp
            .servers
            .get(&(key.0.clone(), key.1.clone()))
            .is_some_and(|handle| handle.generation == generation)
        {
            self.cancel_lsp_request(key);
        }
    }

    pub(super) fn refresh_workspace_symbol_providers(&mut self) {
        let mut providers: Vec<_> = self
            .lsp
            .servers
            .iter()
            .filter_map(|((server_id, root), handle)| {
                let caps = handle.capabilities.lock().ok()?;
                token::lsp::client::supports_workspace_symbols(caps.as_ref()?).then(|| {
                    SymbolProvider {
                        server_id: server_id.clone(),
                        root: root.clone(),
                        generation: handle.generation,
                    }
                })
            })
            .collect();
        providers.sort_by(|a, b| {
            a.server_id
                .0
                .cmp(&b.server_id.0)
                .then_with(|| a.root.cmp(&b.root))
        });
        if self.model.lsp.workspace_symbol_providers != providers {
            self.emit_lsp_msg(Msg::Lsp(LspMsg::WorkspaceSymbolProviders(providers)));
        }
    }

    pub(super) fn check_workspace_symbols(&mut self) {
        let now = Instant::now();
        let mut search = std::mem::take(&mut self.lsp.symbols);
        if search.debounce.is_some_and(|deadline| now >= deadline) {
            search.debounce = None;
            if let Some(request) = &search.request {
                // Flush edited open buffers before querying their workspace.
                let documents: Vec<_> = self
                    .lsp
                    .open_documents
                    .iter()
                    .filter(|(_, open)| {
                        request.providers.iter().any(|provider| {
                            provider.server_id == open.server_id && provider.root == open.root
                        })
                    })
                    .map(|(id, _)| *id)
                    .collect();
                for document in documents {
                    self.flush_lsp_did_change(document);
                }
                for provider in &request.providers {
                    let key = (provider.server_id.clone(), provider.root.clone());
                    let Some(handle) = self
                        .lsp
                        .servers
                        .get(&key)
                        .filter(|handle| handle.generation == provider.generation)
                    else {
                        search.results.failures += 1;
                        continue;
                    };
                    let supported = handle.capabilities.lock().ok().is_some_and(|caps| {
                        caps.as_ref()
                            .is_some_and(token::lsp::client::supports_workspace_symbols)
                    });
                    if !supported {
                        search.results.failures += 1;
                        continue;
                    }
                    let id = handle.begin_request(
                        "workspace/symbol",
                        serde_json::json!({"query": request.query}),
                    );
                    search
                        .pending
                        .insert((key.0, key.1, id), (provider.generation, now + TIMEOUT));
                }
            }
        }
        let expired: Vec<_> = search
            .pending
            .iter()
            .filter(|(_, (_, deadline))| now >= *deadline)
            .map(|(key, _)| key.clone())
            .collect();
        for key in expired {
            if let Some((generation, _)) = search.pending.remove(&key) {
                search.results.failures += 1;
                self.cancel_workspace_symbol_request(&key, generation);
            }
        }
        let reply = search.finish();
        self.lsp.symbols = search;
        if let Some(reply) = reply {
            self.emit_lsp_msg(reply);
        }
    }

    pub(super) fn intercept_workspace_symbols(&mut self, messages: Vec<Msg>) -> Vec<Msg> {
        messages
            .into_iter()
            .filter_map(|message| {
                let Msg::Lsp(LspMsg::WorkspaceSymbolsResponseFromServer {
                    server_id,
                    root,
                    generation,
                    request_id,
                    result,
                    abandoned,
                }) = message
                else {
                    return Some(message);
                };
                let key = (server_id, root, request_id);
                let search = &mut self.lsp.symbols;
                if search
                    .pending
                    .get(&key)
                    .is_none_or(|(expected, _)| *expected != generation)
                {
                    return None;
                }
                search.pending.remove(&key);
                match result {
                    Ok(result) if !abandoned => {
                        search.results.items.extend(result.items);
                        search.results.failures += result.failures;
                        search.results.truncated |= result.truncated;
                        if let Some(request) = &search.request {
                            finish_results(&request.query, &mut search.results);
                        }
                    }
                    _ => search.results.failures += 1,
                }
                search.finish()
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use token::cli::{StartupConfig, StartupMode};

    fn app() -> App {
        App::new(
            800,
            600,
            StartupConfig {
                mode: StartupMode::Empty,
                initial_position: None,
                restore_session: false,
                wait_mode: false,
            },
            None,
            None,
            None,
        )
    }

    fn request() -> SymbolSearchRequest {
        SymbolSearchRequest {
            id: 1,
            query: "symbol".into(),
            workspace: "/workspace".into(),
            providers: ["a", "b"]
                .into_iter()
                .map(|id| SymbolProvider {
                    server_id: id.into(),
                    root: "/workspace".into(),
                    generation: 7,
                })
                .collect(),
        }
    }

    fn pending(app: &mut App) {
        let request = request();
        app.lsp.symbols = SymbolSearch {
            pending: request
                .providers
                .iter()
                .map(|provider| {
                    (
                        (provider.server_id.clone(), provider.root.clone(), 1),
                        (provider.generation, Instant::now() + TIMEOUT),
                    )
                })
                .collect(),
            request: Some(request),
            ..Default::default()
        };
    }

    fn response(id: &str, generation: u64, abandoned: bool) -> Msg {
        Msg::Lsp(LspMsg::WorkspaceSymbolsResponseFromServer {
            server_id: id.into(),
            root: "/workspace".into(),
            generation,
            request_id: 1,
            result: Ok(SymbolResults::default()),
            abandoned,
        })
    }

    #[test]
    fn workspace_symbols_fanout_waits_for_all_providers_and_reports_partial_failure() {
        let mut app = app();
        pending(&mut app);
        assert!(app
            .intercept_workspace_symbols(vec![response("a", 7, false)])
            .is_empty());
        assert_eq!(app.lsp.symbols.pending.len(), 1);
        let messages = app.intercept_workspace_symbols(vec![response("b", 7, true)]);
        let [Msg::Lsp(LspMsg::WorkspaceSymbolsReady { results, .. })] = messages.as_slice() else {
            panic!("one aggregate reply")
        };
        assert_eq!(results.failures, 1);
        assert!(app.lsp.symbols.pending.is_empty());
        assert!(app.lsp.symbols.request.is_none());
        assert!(app.lsp.symbols.deadline().is_none());
    }

    #[test]
    fn workspace_symbols_stale_generation_and_unknown_ids_cannot_consume_pending() {
        let mut app = app();
        pending(&mut app);
        assert!(app
            .intercept_workspace_symbols(vec![
                response("a", 6, false),
                response("unknown", 7, false)
            ])
            .is_empty());
        assert_eq!(app.lsp.symbols.pending.len(), 2);
        assert!(app
            .intercept_workspace_symbols(vec![response("a", 7, false)])
            .is_empty());
        assert_eq!(app.lsp.symbols.pending.len(), 1);
        assert!(app
            .intercept_workspace_symbols(vec![response("a", 7, false)])
            .is_empty());
        assert_eq!(app.lsp.symbols.pending.len(), 1);
    }

    #[test]
    fn workspace_symbols_replacement_cancels_pending_and_debounces_without_starting_servers() {
        let mut app = app();
        pending(&mut app);
        let mut next = request();
        next.id += 1;
        app.set_workspace_symbol_query(Some(next.clone()));
        assert!(app.lsp.symbols.pending.is_empty());
        assert_eq!(app.lsp.symbols.request, Some(next));
        assert!(app.lsp.symbols.deadline().is_some());
        app.check_workspace_symbols();
        assert!(app.lsp.symbols.pending.is_empty());
        assert!(app.lsp.symbols.debounce.is_some());
        app.lsp.symbols.debounce = Some(Instant::now());
        app.check_workspace_symbols();
        assert!(app.lsp.servers.is_empty());
        assert!(app.lsp.symbols.request.is_none());
        app.set_workspace_symbol_query(None);
        assert!(app.lsp.symbols.deadline().is_none());
    }

    #[test]
    fn workspace_symbols_expired_queries_clear_pending_and_ignore_late_replies() {
        let mut app = app();
        pending(&mut app);
        for (_, deadline) in app.lsp.symbols.pending.values_mut() {
            *deadline = Instant::now();
        }
        app.check_workspace_symbols();
        assert!(app.lsp.symbols.pending.is_empty());
        assert!(app.lsp.symbols.request.is_none());
        assert!(app
            .intercept_workspace_symbols(vec![response("a", 7, false)])
            .is_empty());
    }

    fn handle() -> token::lsp::client::ServerHandle {
        let (tx, _rx) = std::sync::mpsc::channel();
        token::lsp::client::spawn_server(
            "sh",
            &["-c".into(), "exec cat >/dev/null".into()],
            std::path::Path::new("/tmp"),
            "a".into(),
            tx,
            None,
            serde_json::Value::Null,
            serde_json::Value::Null,
        )
        .unwrap()
    }

    #[test]
    fn workspace_symbols_cancellation_cannot_abandon_a_replacement_servers_request() {
        let mut app = app();
        let handle = handle();
        let generation = handle.generation;
        let request_id = handle.begin_request("textDocument/hover", serde_json::json!({}));
        let key: RequestKey = ("a".into(), "/workspace".into(), request_id);
        app.lsp
            .servers
            .insert((key.0.clone(), key.1.clone()), handle);
        app.lsp
            .symbols
            .pending
            .insert(key.clone(), (generation.wrapping_sub(1), Instant::now()));
        app.set_workspace_symbol_query(None);
        let mut handle = app.lsp.servers.remove(&(key.0, key.1)).unwrap();
        let entry = handle.pending.lock().unwrap().resolve(request_id);
        handle.kill();
        assert!(
            !entry.unwrap().abandoned,
            "replacement request must survive old-query cancellation"
        );
    }

    #[test]
    fn workspace_symbols_runtime_queries_capable_existing_handles_and_cancels_their_ids() {
        let mut app = app();
        let mut handle = handle();
        let (outbound, received) = std::sync::mpsc::channel();
        let _original_writer = std::mem::replace(&mut handle.outbound_tx, outbound);
        *handle.capabilities.lock().unwrap() = Some(lsp_types::ServerCapabilities {
            workspace_symbol_provider: Some(lsp_types::OneOf::Left(true)),
            ..Default::default()
        });
        let provider = SymbolProvider {
            server_id: "a".into(),
            root: "/workspace".into(),
            generation: handle.generation,
        };
        let key = (provider.server_id.clone(), provider.root.clone());
        app.lsp.servers.insert(key.clone(), handle);
        let document_id = app.model.document().id.unwrap();
        app.model.document_mut().buffer = "unsaved symbol".into();
        app.model.document_mut().revision = 2;
        app.lsp.open_documents.insert(
            document_id,
            super::super::OpenDocState {
                server_id: provider.server_id.clone(),
                root: provider.root.clone(),
                uri: "file:///workspace/source.rs".parse().unwrap(),
                synced_revision: 1,
            },
        );
        app.lsp_change_deadlines
            .record_edit(document_id, 2, Instant::now(), TIMEOUT, TIMEOUT);
        app.refresh_workspace_symbol_providers();
        let mirrored = app.model.lsp.workspace_symbol_providers.clone();
        let request = SymbolSearchRequest {
            providers: vec![provider.clone()],
            ..request()
        };
        app.set_workspace_symbol_query(Some(request));
        app.lsp.symbols.debounce = Some(Instant::now());
        app.check_workspace_symbols();
        let pending: Vec<_> = app.lsp.symbols.pending.keys().cloned().collect();
        app.set_workspace_symbol_query(None);
        let mut handle = app.lsp.servers.remove(&key).unwrap();
        let entries: Vec<_> = pending
            .iter()
            .filter_map(|key| handle.pending.lock().unwrap().resolve(key.2))
            .collect();
        handle.kill();
        let traffic: Vec<_> = received.try_iter().collect();
        assert!(
            matches!(traffic.first(), Some(token::lsp::client::WorkerCmd::Notify {method, params})
            if method == "textDocument/didChange" && params["contentChanges"][0]["text"] == "unsaved symbol")
        );
        assert!(
            matches!(traffic.get(1), Some(token::lsp::client::WorkerCmd::Request {method, ..}) if method == "workspace/symbol")
        );
        assert!(
            matches!(traffic.get(2), Some(token::lsp::client::WorkerCmd::Notify {method, ..}) if method == "$/cancelRequest")
        );
        assert_eq!(app.lsp.open_documents[&document_id].synced_revision, 2);
        assert!(app.lsp_change_deadlines.next_deadline().is_none());
        assert_eq!(mirrored, vec![provider]);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].method, "workspace/symbol");
        assert!(entries[0].abandoned);
        assert!(app.lsp.symbols.pending.is_empty());
    }
}
