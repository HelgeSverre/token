//! `LspMsg` handlers (lsp-integration.md Phases 1-3).
//!
//! Phase 1 lifecycle traffic (model mirror, restart) and Phase 2
//! diagnostics projection live here; Phase 3 adds go-to-definition's
//! user-intent capture and revision-guarded response handling. Hover and
//! completion land with their phases.

use crate::commands::Cmd;
use crate::lsp::ServerState;
use crate::messages::{DefinitionOutcome, HoverOutcome, LspMsg};
use crate::model::editor_area::DocumentId;
use crate::model::{AppModel, CursorOverlayKind, CursorOverlayState, HoverCardState};
use crate::update::navigation;

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
pub fn open_lsp_document(model: &mut AppModel, document_id: DocumentId) -> Option<Cmd> {
    let doc = model.editor_area.documents.get(&document_id)?;
    let file_path = doc.file_path.clone()?;
    let language = doc.language;

    // One-shot routing hint from a cross-file definition jump into a
    // location outside every root (see `LspMsg::DefinitionResolved`):
    // skip the generic ensure-server/resolve-root path entirely and
    // `didOpen` directly against the server that resolved it.
    if let Some((hint_path, server_id, root)) = model.lsp.route_hint.take() {
        if hint_path == file_path {
            return Some(Cmd::LspDidOpenOnServer {
                document_id,
                file_path,
                server_id,
                root,
            });
        }
    }

    Some(Cmd::Batch(vec![
        Cmd::LspEnsureServer {
            language,
            file_path: file_path.clone(),
        },
        Cmd::LspDidOpen {
            document_id,
            file_path,
            language,
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

/// Flips `config.lsp.enabled` in place and returns the new value —
/// factored out of `toggle_lsp_enabled` so the state transition is
/// testable without going through `EditorConfig::save()`'s real disk path
/// (mirrors `EditorConfig::save_to`'s own explicit-path test seam).
fn apply_lsp_master_toggle(config: &mut crate::config::EditorConfig) -> bool {
    let enabled = !config.lsp.enabled;
    config.lsp.enabled = enabled;
    enabled
}

/// Flips `lsp.servers.<id>.enabled` (absent means enabled) in place and
/// returns the new value — the `toggle_lsp_server_enabled` counterpart to
/// `apply_lsp_master_toggle`.
fn apply_lsp_server_toggle(lsp: &mut crate::config::LspConfig, server_id: &str) -> bool {
    let currently_enabled = lsp
        .servers
        .get(server_id)
        .and_then(|o| o.enabled)
        .unwrap_or(true);
    let enabled = !currently_enabled;
    lsp.servers.entry(server_id.to_owned()).or_default().enabled = Some(enabled);
    enabled
}

/// `CommandId::ToggleLsp`: flips the master `lsp.enabled` switch, persists
/// it, and applies it live. Disabling tears down every running server (a
/// non-quit variant of `Cmd::Quit`'s graceful teardown, bounded by the same
/// grace budget) and clears their diagnostics; enabling just clears the
/// missing-server memo — a fresh spawn is attempted lazily on the next
/// matching open/edit, matching the design doc's Process Model.
pub fn toggle_lsp_enabled(model: &mut AppModel) -> Option<Cmd> {
    let enabled = apply_lsp_master_toggle(&mut model.config);
    if let Err(e) = model.config.save() {
        tracing::warn!("Failed to save LSP toggle: {}", e);
    }
    model.ui.set_status(if enabled {
        "LSP enabled"
    } else {
        "LSP disabled"
    });
    Some(Cmd::Batch(vec![
        Cmd::LspSetEnabled { enabled },
        Cmd::redraw_status_bar(),
    ]))
}

/// Toggles `lsp.servers.<id>.enabled` (absent means enabled) for one
/// server — the Language Servers picker's row action. Same persist/
/// apply-live shape as `toggle_lsp_enabled`, scoped to a single server;
/// `None` only if the config can't be reached (never for a valid `id`).
pub fn toggle_lsp_server_enabled(model: &mut AppModel, server_id: &str) -> Option<Cmd> {
    let enabled = apply_lsp_server_toggle(&mut model.config.lsp, server_id);
    if let Err(e) = model.config.save() {
        tracing::warn!("Failed to save LSP server toggle: {}", e);
    }
    Some(Cmd::Batch(vec![
        Cmd::LspSetServerEnabled {
            server_id: crate::lsp::LspServerId::from(server_id),
            enabled,
        },
        Cmd::Redraw,
    ]))
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
            let had_marks = !doc.diagnostics.is_empty();
            let has_marks = !diagnostics.is_empty();
            doc.diagnostics = diagnostics;
            // Marks-lane activation changes gutter width, which
            // `visible_columns` must track (see `resync_viewports`'s doc
            // comment) — only worth the pass when it actually flipped.
            if had_marks != has_marks {
                model.resync_viewports();
            }
            Some(Cmd::redraw_editor())
        }

        LspMsg::GotoDefinition => {
            let doc = model.try_document()?;
            let document_id = doc.id?;
            let revision = doc.revision;
            // Untitled documents are never LSP-synced (design doc's
            // Document Synchronization section) — nothing to request
            // against.
            doc.file_path.as_ref()?;
            // The user's most recently active cursor (multi-cursor
            // editing) — same source `ShowHover` and jump history's
            // `current_jump_entry` use, so a go-to-definition request and
            // the back-stack entry it pushes always agree on "where the
            // user was".
            let position =
                crate::lsp::position_to_lsp(doc, model.editor().active_cursor().to_position());
            let origin = navigation::current_jump_entry(model)?;
            Some(Cmd::LspRequestDefinition {
                document_id,
                position,
                revision,
                origin,
            })
        }

        LspMsg::NavigateBack => navigation::navigate_back(model),
        LspMsg::NavigateForward => navigation::navigate_forward(model),

        LspMsg::DefinitionResolved {
            document_id,
            revision,
            origin,
            outcome,
        } => {
            // Revision guard: a response for text that has since changed
            // is discarded outright, never moving the cursor (design
            // doc's "no stale result ever moves a cursor").
            let doc = model.editor_area.documents.get(&document_id)?;
            if doc.revision != revision {
                return None;
            }
            match outcome {
                DefinitionOutcome::Locations {
                    locations,
                    resolving_server,
                    resolving_root,
                } => {
                    let Some(location) = locations.into_iter().next() else {
                        model.ui.set_status("No definition found");
                        return Some(Cmd::redraw_status_bar());
                    };
                    let Some(path) = crate::lsp::uri_to_path(&location.uri) else {
                        model.ui.set_status("No definition found");
                        return Some(Cmd::redraw_status_bar());
                    };
                    // Outside every root (stdlib, `~/.cargo/registry`, ...):
                    // route the imminent `didOpen` to the server that
                    // resolved this location rather than letting the
                    // generic open path derive (and possibly spawn a
                    // server for) its own root — design doc's "never
                    // spawn a new server rooted in a toolchain directory".
                    let outside_every_root = model
                        .workspace
                        .as_ref()
                        .is_none_or(|ws| !path.starts_with(&ws.root));
                    if outside_every_root {
                        model.lsp.route_hint =
                            Some((path.clone(), resolving_server, resolving_root));
                    }
                    navigation::push_history_entry(model, origin);
                    let open_cmd = navigation::open_or_focus(model, path.clone());
                    // `open_or_focus` can fall through without changing
                    // focus (target failed to open, or was reused into a
                    // different split) — `focused_tab_shows` catches that
                    // so a bad open never moves the *origin* document's
                    // cursor (design doc: "no stale result ever moves a
                    // cursor").
                    let cursor_cmd = if navigation::focused_tab_is_text(model)
                        && navigation::focused_tab_shows(model, &path)
                    {
                        let target_doc = model.document();
                        let position =
                            crate::lsp::lsp_to_position(target_doc, location.range.start);
                        navigation::place_cursor_char(model, position.line, position.column)
                    } else {
                        None
                    };
                    Some(navigation::combine(open_cmd, cursor_cmd).unwrap_or(Cmd::Redraw))
                }
                DefinitionOutcome::StillIndexing => {
                    model.ui.set_status("Language server still indexing…");
                    Some(Cmd::redraw_status_bar())
                }
                DefinitionOutcome::NotSupported => {
                    model
                        .ui
                        .set_status("Go to definition not supported by this server");
                    Some(Cmd::redraw_status_bar())
                }
                DefinitionOutcome::NoResult => {
                    model.ui.set_status("No definition found");
                    Some(Cmd::redraw_status_bar())
                }
            }
        }

        // Consumed by `process_async_messages`'s interception pass before
        // reaching here — see the message's doc comment.
        LspMsg::DefinitionResponseFromServer { .. } => None,

        LspMsg::ShowHover => {
            let doc = model.try_document()?;
            let document_id = doc.id?;
            let revision = doc.revision;
            // Untitled documents are never LSP-synced — nothing to
            // request against (same rule as `GotoDefinition`).
            doc.file_path.as_ref()?;
            let cursor = model.editor().active_cursor().to_position();
            let position = crate::lsp::position_to_lsp(doc, cursor);
            Some(Cmd::LspRequestHover {
                document_id,
                position,
                cursor,
                revision,
            })
        }

        LspMsg::HoverResolved {
            document_id,
            revision,
            cursor,
            outcome,
        } => {
            // Revision guard, same as `DefinitionResolved`, plus a cursor
            // guard: a document edit doesn't necessarily bump the cursor,
            // but a bare cursor move (no edit) doesn't bump the revision
            // either — a stale reply for a position the user has since
            // moved away from must never open, per the design doc's "any
            // ... cursor move" dismissal rule extended to in-flight
            // requests.
            let doc = model.editor_area.documents.get(&document_id)?;
            if doc.revision != revision {
                return None;
            }
            // The cursor guard only makes sense against the document the
            // request was issued for — if the focus has since moved to a
            // different tab/split, comparing against the *focused* editor's
            // cursor would compare unrelated positions (both commonly at
            // 0,0) and could open the card over the wrong document.
            if model.try_document().and_then(|d| d.id) != Some(document_id) {
                return None;
            }
            if model.editor().active_cursor().to_position() != cursor {
                return None;
            }
            match outcome {
                HoverOutcome::Content(content) => {
                    let has_diagnostics =
                        !crate::model::decorations::diagnostics_at_position(doc, cursor).is_empty();
                    if content.is_none() && !has_diagnostics {
                        model.ui.set_status("No hover information");
                        return Some(Cmd::redraw_status_bar());
                    }
                    model.ui.hover_card = Some(HoverCardState { content });
                    model.ui.cursor_overlay =
                        Some(CursorOverlayState::new(CursorOverlayKind::Hover));
                    Some(Cmd::Redraw)
                }
                HoverOutcome::StillIndexing => {
                    model.ui.set_status("Language server still indexing…");
                    Some(Cmd::redraw_status_bar())
                }
                HoverOutcome::NotSupported => {
                    model.ui.set_status("Hover not supported by this server");
                    Some(Cmd::redraw_status_bar())
                }
            }
        }

        // Consumed by `process_async_messages`'s interception pass before
        // reaching here — mirrors `DefinitionResponseFromServer`.
        LspMsg::HoverResponseFromServer { .. } => None,
    }
}

/// Finds the open document whose file path canonicalizes to `uri`, per
/// `lsp::path_to_uri` (the "raw `PathBuf`s are never compared" rule from
/// the design doc's URIs and Paths section).
///
/// `path_to_uri` canonicalizes (an `fs::canonicalize` syscall), so calling
/// it for every open document on every publish is wasteful — a
/// workspace-wide publish burst over many distinct URIs costs O(open
/// docs) syscalls per publish. `uri_to_path` is a pure string decode (no
/// syscall); filtering on file name first shrinks the canonicalize calls
/// to the (normally 0-or-1) documents that could plausibly match.
fn find_document_by_uri(model: &AppModel, uri: &lsp_types::Uri) -> Option<DocumentId> {
    let target_name = crate::lsp::uri_to_path(uri)?.file_name()?.to_owned();
    let fast_path = model
        .editor_area
        .documents
        .iter()
        .filter(|(_, doc)| {
            doc.file_path
                .as_deref()
                .and_then(std::path::Path::file_name)
                .is_some_and(|name| name == target_name)
        })
        .find(|(_, doc)| {
            doc.file_path
                .as_deref()
                .is_some_and(|path| &crate::lsp::path_to_uri(path) == uri)
        })
        .map(|(id, _)| *id);
    // The file-name prefilter above is a fast-path optimization only — it
    // assumes the open document's raw basename matches the canonical
    // publish URI's, which a symlink to a differently-named target
    // (bazel/node_modules/dotfile layouts) breaks. Fall back to a full
    // canonicalizing scan rather than silently dropping the publish; still
    // O(open docs), same as every other document already pays here.
    fast_path.or_else(|| {
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
    })
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
    fn apply_lsp_master_toggle_flips_and_returns_the_new_value() {
        let mut config = crate::config::EditorConfig::default();
        assert!(config.lsp.enabled);

        assert!(!apply_lsp_master_toggle(&mut config));
        assert!(!config.lsp.enabled);

        assert!(apply_lsp_master_toggle(&mut config));
        assert!(config.lsp.enabled);
    }

    #[test]
    fn apply_lsp_server_toggle_defaults_to_enabled_and_flips_in_place() {
        let mut lsp = crate::config::LspConfig::default();
        assert!(
            !lsp.servers.contains_key("rust-analyzer"),
            "test setup: no override yet"
        );

        // Absent override reads as enabled, so the first toggle disables.
        assert!(!apply_lsp_server_toggle(&mut lsp, "rust-analyzer"));
        assert_eq!(lsp.servers["rust-analyzer"].enabled, Some(false));

        assert!(apply_lsp_server_toggle(&mut lsp, "rust-analyzer"));
        assert_eq!(lsp.servers["rust-analyzer"].enabled, Some(true));
    }

    /// A `HoverResolved` reply for a document the user has since switched
    /// away from must never open the card, even when the now-focused
    /// editor's cursor happens to coincide with the request's captured
    /// position (both at 0,0 is the common case for a freshly opened
    /// file) — the cursor guard must compare against the *requested*
    /// document, not merely whatever editor is currently focused.
    #[test]
    fn hover_resolved_for_a_document_no_longer_focused_is_dropped() {
        let mut model = model();
        let requested_doc = model.document().id.unwrap();

        // Simulate the focused editor switching to a different document
        // (tab/split change) between request and reply — the new
        // document's cursor starts at (0, 0), matching the stale
        // request's captured cursor below.
        let other_id = DocumentId(requested_doc.0 + 1);
        let mut other_doc = crate::model::Document::with_text("second file\n");
        other_doc.id = Some(other_id);
        model.editor_area.documents.insert(other_id, other_doc);
        model.editor_area.focused_editor_mut().unwrap().document_id = Some(other_id);
        assert_eq!(model.try_document().and_then(|d| d.id), Some(other_id));

        let revision = model.editor_area.documents[&requested_doc].revision;
        update_lsp(
            &mut model,
            LspMsg::HoverResolved {
                document_id: requested_doc,
                revision,
                cursor: crate::model::editor::Position::new(0, 0),
                outcome: HoverOutcome::Content(Some("stale hover".to_owned())),
            },
        );

        assert!(
            model.ui.hover_card.is_none(),
            "hover reply must not open the card once focus has left the requested document"
        );
        assert!(model.ui.cursor_overlay.is_none());
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

    /// A definition target that fails to open (permission denied, gone,
    /// binary the loader rejects) must never move the cursor in the
    /// *origin* document — `open_or_focus` falls through leaving the
    /// origin focused, and `focused_tab_shows` must catch that mismatch
    /// (design doc: "no stale result ever moves a cursor").
    #[test]
    fn definition_resolved_to_an_unopenable_target_does_not_move_the_origin_cursor() {
        let dir = tempfile::tempdir().unwrap();
        let origin_path = dir.path().join("origin.rs");
        std::fs::write(&origin_path, "one\ntwo\nthree\n").unwrap();
        let mut model = AppModel::new(800, 600, 1.0, vec![origin_path.clone()]);
        model.editor_mut().cursors[0].line = 0;
        model.editor_mut().cursors[0].column = 0;

        let origin = navigation::current_jump_entry(&model).unwrap();
        let doc_id = model.document().id.unwrap();
        let revision = model.document().revision;

        // A directory "location" (a server bug, but not one the client may
        // trust) makes `validate_file_for_opening` reject it as
        // `IsDirectory` — `open_file_in_new_tab` returns without touching
        // focus, exactly the real failure mode this guard exists for.
        let target_path = dir.path().join("target_dir");
        std::fs::create_dir(&target_path).unwrap();
        let target_uri = crate::lsp::path_to_uri(&target_path);

        update_lsp(
            &mut model,
            LspMsg::DefinitionResolved {
                document_id: doc_id,
                revision,
                origin,
                outcome: DefinitionOutcome::Locations {
                    locations: vec![lsp_types::Location {
                        uri: target_uri,
                        range: lsp_types::Range {
                            start: lsp_types::Position {
                                line: 2,
                                character: 0,
                            },
                            end: lsp_types::Position {
                                line: 2,
                                character: 0,
                            },
                        },
                    }],
                    resolving_server: LspServerId::from("rust-analyzer"),
                    resolving_root: PathBuf::from("/tmp"),
                },
            },
        );

        assert_eq!(
            model.document().file_path.as_deref(),
            Some(origin_path.as_path()),
            "a failed open must leave the origin document focused"
        );
        assert_eq!(
            (
                model.editor().cursors[0].line,
                model.editor().cursors[0].column
            ),
            (0, 0),
            "the origin document's cursor must not move for a target that never opened"
        );
    }

    #[test]
    fn definition_outside_the_workspace_sets_a_route_hint_the_open_path_consumes() {
        let ws_dir = tempfile::tempdir().unwrap();
        std::fs::write(ws_dir.path().join("main.rs"), "fn main() {}\n").unwrap();
        let mut model = AppModel::new(800, 600, 1.0, vec![ws_dir.path().join("main.rs")]);
        model.workspace =
            crate::model::workspace::Workspace::new(ws_dir.path().to_path_buf(), &model.metrics)
                .ok();

        let outside_dir = tempfile::tempdir().unwrap();
        let target_path = outside_dir.path().join("target.rs");
        std::fs::write(&target_path, "one\ntwo\n").unwrap();
        let target_uri = crate::lsp::path_to_uri(&target_path);
        let origin = navigation::current_jump_entry(&model).unwrap();
        let server_id = LspServerId::from("rust-analyzer");
        let resolving_root = PathBuf::from("/tmp/token-editor-workspace-root");

        let doc_id = model.document().id.unwrap();
        let revision = model.document().revision;
        let cmd = update_lsp(
            &mut model,
            LspMsg::DefinitionResolved {
                document_id: doc_id,
                revision,
                origin,
                outcome: DefinitionOutcome::Locations {
                    locations: vec![lsp_types::Location {
                        uri: target_uri,
                        range: lsp_types::Range::default(),
                    }],
                    resolving_server: server_id.clone(),
                    resolving_root: resolving_root.clone(),
                },
            },
        );

        fn contains_did_open_on_server(cmd: &Cmd, server_id: &LspServerId, root: &PathBuf) -> bool {
            match cmd {
                Cmd::LspDidOpenOnServer {
                    server_id: s,
                    root: r,
                    ..
                } => s == server_id && r == root,
                Cmd::Batch(cmds) => cmds
                    .iter()
                    .any(|c| contains_did_open_on_server(c, server_id, root)),
                _ => false,
            }
        }
        assert!(
            cmd.is_some_and(|c| contains_did_open_on_server(&c, &server_id, &resolving_root)),
            "opening the out-of-workspace target must route didOpen to the resolving server, \
             not the generic ensure-server/resolve-root path"
        );

        // The hint is consumed synchronously by `open_lsp_document` inside
        // the same `update_lsp` call (`navigation::open_or_focus` ->
        // `LayoutMsg::OpenFileInNewTab`) — nothing should be left pending.
        assert!(
            model.lsp.route_hint.is_none(),
            "the one-shot hint must be consumed by the same update"
        );
        // `path_to_uri` canonicalizes (macOS's `/tmp` -> `/private/tmp`
        // symlink), so compare against the canonical form.
        assert_eq!(
            model.document().file_path.as_deref(),
            Some(target_path.canonicalize().unwrap().as_path()),
            "the new tab must focus the out-of-workspace target"
        );
    }

    /// A publish for a document opened through a symlink to a
    /// differently-named target (bazel/node_modules/dotfile layouts) must
    /// still match — the fast filename prefilter must not silently drop
    /// every diagnostic for it.
    #[test]
    #[cfg(unix)]
    fn find_document_by_uri_matches_through_a_renaming_symlink() {
        use std::os::unix::fs::symlink;

        let dir = tempfile::tempdir().unwrap();
        let real_path = dir.path().join("real.rs");
        std::fs::write(&real_path, "fn main() {}\n").unwrap();
        let link_path = dir.path().join("link.rs");
        symlink(&real_path, &link_path).unwrap();

        let model = AppModel::new(800, 600, 1.0, vec![link_path.clone()]);
        let doc_id = model.document().id.unwrap();
        assert_eq!(
            model.document().file_path.as_deref(),
            Some(link_path.as_path())
        );

        // The server publishes under the canonical URI it resolved the
        // symlink to, not the one it was opened with.
        let canonical_uri = crate::lsp::path_to_uri(&real_path);
        let found = find_document_by_uri(&model, &canonical_uri);
        assert_eq!(found, Some(doc_id));
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
