//! Background document features use the same sync and server identity as navigation.

use super::*;
use token::lsp::document_features::{Feature, MAX_DOCUMENT_BYTES};
use token::model::DocumentId;

pub(super) struct PendingDocumentFeature {
    document_id: DocumentId,
    revision: u64,
    language: token::syntax::LanguageId,
    feature: Feature,
    generation: u64,
    deadline: Instant,
}

impl App {
    pub(super) fn request_document_features(&mut self, document_id: DocumentId) {
        self.cancel_document_features(document_id);
        let Some(document) = self.model.editor_area.documents.get(&document_id) else {
            return;
        };
        if document.buffer.len_bytes() > MAX_DOCUMENT_BYTES.as_usize() {
            return;
        }
        let revision = document.revision;
        let language = document.language;
        // These integrations are enabled for Sema first. Other servers retain
        // their existing behavior until their annotation rendering is tested.
        if language != token::syntax::LanguageId::Sema {
            return;
        }
        let end_line = document.line_count().saturating_sub(1);
        let end = lsp::position_to_lsp(
            document,
            token::model::Position::new(end_line, document.line_length(end_line)),
        );
        let Some(open) = self.lsp.open_documents.get(&document_id) else {
            return;
        };
        let Some(handle) = self
            .lsp
            .servers
            .get(&(open.server_id.clone(), open.root.clone()))
        else {
            return;
        };
        let Some(caps) = handle.capabilities_snapshot() else {
            return;
        };
        let generation = handle.generation;
        for feature in Feature::ALL {
            if !feature.supports(&caps) {
                continue;
            }
            let params = (feature == Feature::InlayHints)
                .then(|| serde_json::json!({"range":{"start":{"line":0,"character":0},"end":end}}));
            if let Ok(key) =
                self.send_lsp_feature_request(document_id, feature.method(), None, |_| true, params)
            {
                self.lsp.document_features.insert(
                    key,
                    PendingDocumentFeature {
                        document_id,
                        revision,
                        language,
                        feature,
                        generation,
                        deadline: Instant::now() + Duration::from_secs(10),
                    },
                );
            }
        }
    }

    pub(super) fn cancel_document_features(&mut self, document_id: DocumentId) {
        let keys: Vec<_> = self
            .lsp
            .document_features
            .iter()
            .filter(|(_, pending)| pending.document_id == document_id)
            .map(|(key, _)| key.clone())
            .collect();
        for key in keys {
            self.retire_document_feature(&key);
        }
    }

    fn retire_document_feature(&mut self, key: &RequestKey) {
        let Some(pending) = self.lsp.document_features.remove(key) else {
            return;
        };
        // A replacement process can reuse the same request ID.
        if self
            .lsp
            .servers
            .get(&(key.0.clone(), key.1.clone()))
            .is_some_and(|handle| handle.generation == pending.generation)
        {
            self.cancel_lsp_request(key);
        }
    }

    pub(super) fn document_features_deadline(&self) -> Option<Instant> {
        self.lsp
            .document_features
            .values()
            .map(|pending| pending.deadline)
            .min()
    }

    pub(super) fn sweep_document_features(&mut self) {
        let now = Instant::now();
        let expired: Vec<_> = self
            .lsp
            .document_features
            .iter()
            .filter(|((id, root, _), pending)| {
                pending.deadline <= now
                    || self
                        .lsp
                        .servers
                        .get(&(id.clone(), root.clone()))
                        .is_none_or(|handle| handle.generation != pending.generation)
            })
            .map(|(key, _)| key.clone())
            .collect();
        for key in expired {
            self.retire_document_feature(&key);
        }
    }

    pub(super) fn intercept_document_features(&mut self, messages: Vec<Msg>) -> Vec<Msg> {
        messages
            .into_iter()
            .filter_map(|message| {
                if let Msg::Lsp(LspMsg::SemaEvalResponse {
                    server_id,
                    root,
                    generation,
                    output,
                }) = message
                {
                    let handle = self.lsp.servers.get(&(server_id.clone(), root.clone()))?;
                    if handle.generation != generation {
                        return None;
                    }
                    let document_id = self
                        .lsp
                        .open_documents
                        .iter()
                        .find(|(_, open)| {
                            open.server_id == server_id
                                && open.root == root
                                && open.uri == output.uri
                        })
                        .map(|(&id, _)| id)?;
                    let &(revision, requested_generation) =
                        self.lsp.sema_evals.get(&document_id)?;
                    if requested_generation != generation {
                        return None;
                    }
                    self.lsp.sema_evals.remove(&document_id);
                    return Some(Msg::Lsp(LspMsg::SemaEvalResolved {
                        document_id,
                        revision,
                        output,
                    }));
                }
                let Msg::Lsp(LspMsg::DocumentFeatureResponse {
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
                let key = (server_id.clone(), root.clone(), request_id);
                // A late response from an old process must not remove a new process's request.
                let pending = self.lsp.document_features.get(&key)?;
                if pending.generation != generation {
                    return None;
                }
                let pending = self.lsp.document_features.remove(&key)?;
                let handle = self.lsp.servers.get(&(server_id.clone(), root.clone()))?;
                if abandoned
                    || handle.generation != generation
                    || pending.deadline <= Instant::now()
                {
                    return None;
                }
                let open = self.lsp.open_documents.get(&pending.document_id)?;
                if open.server_id != server_id || open.root != root {
                    return None;
                }
                Some(Msg::Lsp(LspMsg::DocumentFeatureResolved {
                    document_id: pending.document_id,
                    revision: pending.revision,
                    language: pending.language,
                    feature: pending.feature,
                    result,
                    capabilities: Box::new(handle.capabilities_snapshot()?),
                }))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    fn pending(document_id: DocumentId, generation: u64) -> PendingDocumentFeature {
        PendingDocumentFeature {
            document_id,
            revision: 0,
            language: LanguageId::Sema,
            feature: Feature::InlayHints,
            generation,
            deadline: Instant::now() + Duration::from_secs(10),
        }
    }

    #[test]
    fn sema_old_generation_reply_cannot_consume_new_request() {
        let mut app = app();
        let document_id = app.model.document().id.unwrap();
        let server_id = LspServerId("sema".into());
        let root = PathBuf::from("/fixture");
        let key = (server_id.clone(), root.clone(), 7);
        app.lsp
            .document_features
            .insert(key.clone(), pending(document_id, 2));
        let messages =
            app.intercept_document_features(vec![Msg::Lsp(LspMsg::DocumentFeatureResponse {
                server_id,
                root,
                generation: 1,
                request_id: 7,
                result: serde_json::Value::Null,
                abandoned: false,
            })]);
        assert!(messages.is_empty());
        assert!(app.lsp.document_features.contains_key(&key));
        app.cancel_document_features(document_id);
        assert!(app.lsp.document_features.is_empty());
    }

    #[test]
    fn sema_feature_requests_expire_when_server_disappears() {
        let mut app = app();
        let document_id = app.model.document().id.unwrap();
        app.lsp.document_features.insert(
            (LspServerId("sema".into()), PathBuf::from("/fixture"), 1),
            pending(document_id, 1),
        );
        assert!(app.document_features_deadline().is_some());
        app.sweep_document_features();
        assert!(app.document_features_deadline().is_none());
    }

    #[test]
    fn sema_cancel_old_annotations_does_not_abandon_replacement_requests() {
        for same_generation in [false, true] {
            let mut app = app();
            let document_id = app.model.document().id.unwrap();
            let (tx, _rx) = std::sync::mpsc::channel();
            let handle = lsp::client::spawn_server(
                "sh",
                &["-c".into(), "exec cat >/dev/null".into()],
                Path::new("/tmp"),
                "sema".into(),
                tx,
                None,
                serde_json::Value::Null,
                serde_json::Value::Null,
            )
            .unwrap();
            let generation = handle.generation;
            let request_id = handle.begin_request("textDocument/hover", serde_json::json!({}));
            let key = (
                LspServerId("sema".into()),
                PathBuf::from("/fixture"),
                request_id,
            );
            app.lsp
                .servers
                .insert((key.0.clone(), key.1.clone()), handle);
            app.lsp.document_features.insert(
                key.clone(),
                pending(
                    document_id,
                    if same_generation {
                        generation
                    } else {
                        generation.wrapping_sub(1)
                    },
                ),
            );
            app.cancel_document_features(document_id);
            let mut handle = app.lsp.servers.remove(&(key.0, key.1)).unwrap();
            let entry = handle.pending.lock().unwrap().resolve(request_id).unwrap();
            handle.kill();
            assert_eq!(entry.abandoned, same_generation);
        }
    }

    fn pump_until(app: &mut App, cond: impl Fn(&App) -> bool) -> bool {
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            app.process_async_messages();
            app.check_lsp_completion_debounces();
            if cond(app) {
                return true;
            }
            if Instant::now() >= deadline {
                return false;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// Exercise the real provider without requiring an interactive window.
    #[test]
    #[ignore = "requires gopls in GOPLS_SMOKE_BINARY and a Go toolchain"]
    fn gopls_live_hover_and_completion() {
        let binary = std::env::var("GOPLS_SMOKE_BINARY").expect("set GOPLS_SMOKE_BINARY");
        let dir = tempfile::tempdir_in("target").unwrap();
        std::fs::write(
            dir.path().join("go.mod"),
            "module example.com/smoke\n\ngo 1.23\n",
        )
        .unwrap();
        let path = dir.path().canonicalize().unwrap().join("smoke.go");
        std::fs::write(
            &path,
            "package smoke\n\nfunc Add(a, b int) int { return a+b }\n\nvar Answer = Add(20, 22)\n",
        )
        .unwrap();
        let mut app = app();
        app.model.config = token::config::EditorConfig::default();
        app.model.config.lsp.servers.insert(
            "gopls".into(),
            token::config::LspServerOverride {
                command: Some(binary),
                ..Default::default()
            },
        );
        let document_id = app.model.document().id.unwrap();
        let mut document = token::model::Document::from_file(path.clone()).unwrap();
        document.id = Some(document_id);
        app.model
            .editor_area
            .documents
            .insert(document_id, document);
        app.process_cmd(Cmd::LspEnsureServer {
            language: LanguageId::Go,
            file_path: path.clone(),
        });
        app.process_cmd(Cmd::LspDidOpen {
            document_id,
            file_path: path,
            language: LanguageId::Go,
        });
        assert!(pump_until(&mut app, |app| app
            .model
            .lsp
            .servers
            .values()
            .any(|state| *state == lsp::ServerState::Ready)));
        app.model.editor_mut().cursors[0] = token::model::Cursor::at(4, 14);
        app.model.editor_mut().clear_selection();
        app.process_automation_msg(Msg::Lsp(LspMsg::ShowHover));
        let hover = pump_until(&mut app, |app| app.model.ui.hover_card.is_some());
        app.process_automation_msg(Msg::Completion(token::messages::CompletionMsg::TriggerMenu));
        let completion = pump_until(&mut app, |app| {
            app.model.ui.completion_menu.as_ref().is_some_and(|menu| {
                menu.items.iter().any(|item| {
                    item.source == token::completion::menu::MenuSourceId::Lsp
                        && item.label.starts_with("Add")
                })
            })
        });
        app.teardown_all_lsp_servers();
        assert!(hover, "gopls did not return hover documentation for Add");
        assert!(completion, "gopls did not suggest Add for its A prefix");
    }

    /// Opt-in integration test: runs only this fixture, never a user's program.
    /// SEMA_SMOKE_BINARY=/path/to/sema cargo test --bin token -- --ignored sema_live
    #[test]
    #[ignore = "requires a current Sema executable in SEMA_SMOKE_BINARY"]
    fn sema_live_async_document_features_and_run() {
        let binary = std::env::var("SEMA_SMOKE_BINARY").expect("set SEMA_SMOKE_BINARY");
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("smoke.sema");
        std::fs::write(&file_path, "(defn add (left right) (+ left right))\n(async/await (async/spawn (lambda () (async/sleep 1) (add 20 22))))\n").unwrap();
        let mut app = app();
        app.model.config.lsp.servers.insert(
            "sema".into(),
            token::config::LspServerOverride {
                command: Some(binary),
                args: Some(vec!["lsp".into()]),
                ..Default::default()
            },
        );
        let document_id = app.model.document().id.unwrap();
        let mut document = token::model::Document::from_file(file_path.clone()).unwrap();
        document.id = Some(document_id);
        app.model
            .editor_area
            .documents
            .insert(document_id, document);
        app.process_cmd(Cmd::LspEnsureServer {
            language: LanguageId::Sema,
            file_path: file_path.clone(),
        });
        app.process_cmd(Cmd::LspDidOpen {
            document_id,
            file_path,
            language: LanguageId::Sema,
        });
        let received = pump_until(&mut app, |app| {
            let features = &app.model.document().lsp_features;
            features.semantic.is_some() && !features.hints.is_empty() && features.lenses.len() >= 2
        });
        if !received {
            app.teardown_all_lsp_servers();
            panic!("Sema did not deliver semantic tokens, hints and lenses");
        }
        let handle = app.lsp.servers.values().next().unwrap();
        let caps = handle.capabilities_snapshot().unwrap();
        assert!(caps
            .completion_provider
            .unwrap()
            .trigger_characters
            .unwrap()
            .contains(&"/".into()));
        assert!(caps
            .execute_command_provider
            .unwrap()
            .commands
            .contains(&"sema.cancelTopLevel".into()));
        assert!(!app
            .model
            .document()
            .lsp_features
            .semantic
            .as_ref()
            .unwrap()
            .lines
            .is_empty());
        let command = app
            .model
            .document()
            .lsp_features
            .lenses
            .last()
            .unwrap()
            .command
            .clone()
            .unwrap();
        app.execute_lsp_command(document_id, command.command, command.arguments);
        assert!(
            pump_until(&mut app, |app| !app
                .model
                .document()
                .lsp_features
                .eval_output
                .is_empty()),
            "no evaluation result"
        );
        assert!(
            app.model
                .document()
                .lsp_features
                .eval_output
                .values()
                .any(|text| text == "Sema: 42"),
            "{:?}",
            app.model.document().lsp_features.eval_output
        );
        app.model.document_mut().revision += 1;
        app.process_cmd(Cmd::LspScheduleDidChange {
            document_id,
            revision: 1,
        });
        app.flush_lsp_did_change(document_id);
        assert!(pump_until(&mut app, |app| app
            .model
            .document()
            .lsp_features
            .revision
            == 1));
        assert!(app.model.document().lsp_features.eval_output.is_empty());
        app.lsp_close_document(document_id);
        assert!(app.model.document().lsp_features.lenses.is_empty());
        assert!(app.lsp.document_features.is_empty());
        app.teardown_all_lsp_servers();
    }
}
