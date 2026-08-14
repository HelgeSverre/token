//! `LspMsg` handlers (lsp-integration.md Phases 1-3).
//!
//! Phase 1 lifecycle traffic (model mirror, restart) and Phase 2
//! diagnostics projection live here; Phase 3 adds go-to-definition's
//! user-intent capture and revision-guarded response handling. Hover and
//! completion land with their phases.

use crate::commands::Cmd;
use crate::lsp::ServerState;
use crate::messages::{DefinitionOutcome, HoverOutcome, LspMsg, ReferencesOutcome};
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
            // authoritative store lives there too. This mirrors the
            // publish into `model.lsp.diagnostics` (feeds the Problems
            // panel + status counts) BEFORE the open-document projection
            // below, which early-returns for unopened files — the mirror
            // must still update for those.
            if let Some(path) = crate::lsp::uri_to_path(&uri) {
                if diagnostics.is_empty() {
                    model.lsp.diagnostics.remove(&path);
                } else {
                    model.lsp.diagnostics.insert(path, diagnostics.clone());
                }
            }
            // A shrinking publish (fewer/no diagnostics for this file) can
            // leave a stale selected_index/scroll_offset in the Problems
            // panel — clamp unconditionally, same as any other mutation of
            // the mirror.
            crate::update::problems::clamp_problems_selection(model);
            // The rest is purely the model projection onto whatever
            // document (if any) has `uri` open — a publish for an
            // unopened file is a no-op past this point (still retained in
            // the runtime's store and now in the mirror above). But if the
            // Problems panel is open, that mirror update still needs a
            // repaint request — this is exactly the workspace-wide,
            // other-files case the mirror was hoisted above the early
            // return for (mirrors `clear_diagnostics_for_roots`'s
            // `problems_panel_open` handling on the clearing side).
            let document_id = match find_document_by_uri(model, &uri) {
                Some(id) => id,
                None => {
                    let problems_panel_open = model
                        .dock_layout
                        .active_panel_position(crate::panel::PanelId::PROBLEMS)
                        .is_some();
                    return problems_panel_open.then_some(Cmd::Redraw);
                }
            };
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
                    if locations.is_empty() {
                        model.ui.set_status("No definition found");
                        return Some(Cmd::redraw_status_bar());
                    }
                    // Outside every root (stdlib, `~/.cargo/registry`,
                    // ...): route the imminent `didOpen` to the server
                    // that resolved this location rather than letting the
                    // generic open path derive (and possibly spawn a
                    // server for) its own root — design doc's "never
                    // spawn a new server rooted in a toolchain directory".
                    let outside_every_root = |path: &std::path::Path| {
                        model
                            .workspace
                            .as_ref()
                            .is_none_or(|ws| !path.starts_with(&ws.root))
                    };

                    if let [location] = locations.as_slice() {
                        let Some(path) = crate::lsp::uri_to_path(&location.uri) else {
                            model.ui.set_status("No definition found");
                            return Some(Cmd::redraw_status_bar());
                        };
                        if outside_every_root(&path) {
                            model.lsp.route_hint =
                                Some((path.clone(), resolving_server, resolving_root));
                        }
                        return navigation::jump_to_location(
                            model,
                            Some(origin),
                            &path,
                            location.range.start,
                        );
                    }

                    // More than one location: upgrade to the same
                    // cursor-anchored popup Show Usages uses (reusing
                    // `LocationItem`/`reference_list`) instead of the old
                    // first-location-only behavior. Preview is best-effort
                    // (open documents only) — `update()` must not do I/O.
                    let items: Vec<navigation::LocationItem> = locations
                        .iter()
                        .filter_map(|location| {
                            let path = crate::lsp::uri_to_path(&location.uri)?;
                            let preview = model
                                .editor_area
                                .find_open_file(&path)
                                .and_then(|(doc_id, _, _)| model.editor_area.documents.get(&doc_id))
                                .and_then(|doc| {
                                    doc.get_line_cow(location.range.start.line as usize)
                                })
                                .map(|line| line.trim().to_owned())
                                .unwrap_or_default();
                            let route_hint = outside_every_root(&path)
                                .then(|| (resolving_server.clone(), resolving_root.clone()));
                            Some(navigation::LocationItem {
                                path,
                                position: location.range.start,
                                preview,
                                route_hint,
                            })
                        })
                        .collect();
                    open_location_list_popup(model, items)
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
            // Not a mouse-dwell request — the `HoverResolved` guard below
            // must compare against the live caret instead.
            model.ui.mouse_hover_target = None;
            Some(Cmd::LspRequestHover {
                document_id,
                position,
                cursor,
                revision,
            })
        }

        LspMsg::ShowHoverAt { line, col } => {
            let doc = model.try_document()?;
            let document_id = doc.id?;
            let revision = doc.revision;
            doc.file_path.as_ref()?;
            let target = crate::model::editor::Position::new(line, col);
            let position = crate::lsp::position_to_lsp(doc, target);
            // Captured so `HoverResolved` can tell a still-wanted reply
            // from one whose dwell was abandoned before it arrived (the
            // mouse equivalent of comparing against the live caret).
            model.ui.mouse_hover_target = Some(target);
            Some(Cmd::LspRequestHover {
                document_id,
                position,
                cursor: target,
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
            // A mouse-dwell reply (`ShowHoverAt`) is guarded against the
            // captured dwell target instead of the caret — the runtime
            // clears `mouse_hover_target` the moment the pointer moves away
            // from it, so a match here means the dwell is still live. A
            // caret-triggered reply falls back to the original caret check.
            let is_dwell_match = model.ui.mouse_hover_target == Some(cursor);
            if !is_dwell_match && model.editor().active_cursor().to_position() != cursor {
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
                    let anchor = is_dwell_match.then_some((cursor.line, cursor.column));
                    model.ui.hover_card = Some(HoverCardState { content, anchor });
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

        LspMsg::FindReferences => {
            let doc = model.try_document()?;
            let document_id = doc.id?;
            let revision = doc.revision;
            // Untitled documents are never LSP-synced — nothing to
            // request against (same rule as `ShowHover`/`GotoDefinition`).
            doc.file_path.as_ref()?;
            let cursor = model.editor().active_cursor().to_position();
            let position = crate::lsp::position_to_lsp(doc, cursor);
            Some(Cmd::LspRequestReferences {
                document_id,
                position,
                cursor,
                revision,
            })
        }

        LspMsg::ReferencesResolved {
            document_id,
            revision,
            cursor,
            items,
            outcome,
        } => {
            // Revision + focused-document + cursor guards, verbatim from
            // `HoverResolved` (minus the mouse-dwell branch — references
            // has no mouse trigger).
            let doc = model.editor_area.documents.get(&document_id)?;
            if doc.revision != revision {
                return None;
            }
            if model.try_document().and_then(|d| d.id) != Some(document_id) {
                return None;
            }
            if model.editor().active_cursor().to_position() != cursor {
                return None;
            }
            match outcome {
                ReferencesOutcome::StillIndexing => {
                    model.ui.set_status("Language server still indexing…");
                    Some(Cmd::redraw_status_bar())
                }
                ReferencesOutcome::NotSupported => {
                    model
                        .ui
                        .set_status("Find usages not supported by this server");
                    Some(Cmd::redraw_status_bar())
                }
                ReferencesOutcome::NoResult => {
                    model.ui.set_status("No usages found");
                    Some(Cmd::redraw_status_bar())
                }
                ReferencesOutcome::Found => open_location_list_popup(model, items),
            }
        }

        // Consumed by `process_async_messages`'s interception pass before
        // reaching here — mirrors `HoverResponseFromServer`.
        LspMsg::ReferencesResponseFromServer { .. } => None,

        LspMsg::ActivateReference { index } => {
            let item = model
                .ui
                .reference_list
                .as_ref()
                .and_then(|items| items.get(index))
                .cloned();
            model.ui.cursor_overlay = None;
            model.ui.reference_list = None;
            let Some(item) = item else {
                return Some(Cmd::Redraw);
            };
            apply_route_hint(model, &item);
            navigation::jump_to_location(model, None, &item.path, item.position)
        }
    }
}

/// A stored `LocationItem` resolves its own open path when activated — set
/// `model.lsp.route_hint` from the item's own resolving server/root
/// (`None` inside the workspace) before jumping, so an out-of-workspace
/// target still reuses the server that resolved it instead of letting
/// `open_lsp_document`'s generic path derive (and possibly spawn) its own
/// root (lsp-integration.md "never spawn a new server rooted in a
/// toolchain directory").
fn apply_route_hint(model: &mut AppModel, item: &navigation::LocationItem) {
    if let Some((server_id, root)) = &item.route_hint {
        model.lsp.route_hint = Some((item.path.clone(), server_id.clone(), root.clone()));
    }
}

/// Shared activation for a resolved `LocationItem` list with more than one
/// entry — the Show Usages popup, and the multi-def upgrade to
/// `DefinitionResolved`. Exactly one entry jumps directly (`jump_to_location`,
/// origin captured now — callers with an async-captured origin handle that
/// themselves); zero or one is never passed here by either caller.
fn open_location_list_popup(
    model: &mut AppModel,
    mut items: Vec<navigation::LocationItem>,
) -> Option<Cmd> {
    if items.is_empty() {
        model.ui.set_status("No usages found");
        return Some(Cmd::redraw_status_bar());
    }
    // Ordering authority: sorted once here, at construction — the view's
    // spec builder and Enter/click activation both index this same stored
    // `Vec`, never re-deriving it.
    items.sort_by(|a, b| (&a.path, a.position.line).cmp(&(&b.path, b.position.line)));
    if let [only] = items.as_slice() {
        apply_route_hint(model, only);
        let (path, position) = (only.path.clone(), only.position);
        return navigation::jump_to_location(model, None, &path, position);
    }
    model.ui.reference_list = Some(items);
    model.ui.cursor_overlay = Some(CursorOverlayState::new(CursorOverlayKind::References));
    Some(Cmd::Redraw)
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
    use std::path::{Path, PathBuf};

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

    fn diagnostic_at(line: u32) -> lsp_types::Diagnostic {
        lsp_types::Diagnostic {
            range: lsp_types::Range {
                start: lsp_types::Position { line, character: 0 },
                end: lsp_types::Position { line, character: 3 },
            },
            severity: Some(lsp_types::DiagnosticSeverity::ERROR),
            message: "boom".into(),
            ..Default::default()
        }
    }

    #[test]
    fn diagnostics_published_mirrors_into_the_model_even_for_an_unopened_file() {
        let mut model = model();
        let uri = crate::lsp::path_to_uri(&PathBuf::from("/tmp/problems-mirror/unopened.rs"));
        let path = crate::lsp::uri_to_path(&uri).unwrap();

        update_lsp(
            &mut model,
            LspMsg::DiagnosticsPublished {
                uri: uri.clone(),
                version: None,
                diagnostics: vec![diagnostic_at(1)],
            },
        );

        assert_eq!(model.lsp.diagnostics.get(&path).map(Vec::len), Some(1));
    }

    #[test]
    fn diagnostics_published_with_empty_list_removes_the_mirror_entry() {
        let mut model = model();
        let uri = crate::lsp::path_to_uri(&PathBuf::from("/tmp/problems-mirror/cleared.rs"));
        let path = crate::lsp::uri_to_path(&uri).unwrap();
        model
            .lsp
            .diagnostics
            .insert(path.clone(), vec![diagnostic_at(0)]);

        update_lsp(
            &mut model,
            LspMsg::DiagnosticsPublished {
                uri,
                version: None,
                diagnostics: vec![],
            },
        );

        assert!(!model.lsp.diagnostics.contains_key(&path));
    }

    /// The whole reason the mirror update was hoisted above the
    /// unopened-document early return: an open Problems panel must repaint
    /// on a publish for a file that isn't open, or its rows/header counts
    /// go stale until an unrelated event happens to repaint.
    #[test]
    fn diagnostics_published_for_an_unopened_file_redraws_when_the_problems_panel_is_open() {
        let mut model = model();
        model
            .dock_layout
            .bottom
            .activate(crate::panel::PanelId::PROBLEMS);
        let uri = crate::lsp::path_to_uri(&PathBuf::from("/tmp/problems-mirror/other.rs"));

        let cmd = update_lsp(
            &mut model,
            LspMsg::DiagnosticsPublished {
                uri,
                version: None,
                diagnostics: vec![diagnostic_at(1)],
            },
        );

        assert!(matches!(cmd, Some(Cmd::Redraw)));
    }

    #[test]
    fn diagnostics_redraw_follows_problems_to_the_right_dock() {
        let mut model = model();
        model
            .dock_layout
            .bottom
            .panel_ids
            .retain(|&panel| panel != crate::panel::PanelId::PROBLEMS);
        model.dock_layout.bottom.active_index = Some(0);
        model
            .dock_layout
            .right
            .register_panel(crate::panel::PanelId::PROBLEMS);
        model
            .dock_layout
            .right
            .activate(crate::panel::PanelId::PROBLEMS);
        let uri = crate::lsp::path_to_uri(&PathBuf::from("/tmp/problems-mirror/moved.rs"));

        let cmd = update_lsp(
            &mut model,
            LspMsg::DiagnosticsPublished {
                uri,
                version: None,
                diagnostics: vec![diagnostic_at(1)],
            },
        );

        assert!(matches!(cmd, Some(Cmd::Redraw)));
    }

    #[test]
    fn diagnostics_published_for_an_unopened_file_is_a_noop_when_the_problems_panel_is_closed() {
        let mut model = model();
        let uri = crate::lsp::path_to_uri(&PathBuf::from("/tmp/problems-mirror/other.rs"));

        let cmd = update_lsp(
            &mut model,
            LspMsg::DiagnosticsPublished {
                uri,
                version: None,
                diagnostics: vec![diagnostic_at(1)],
            },
        );

        assert!(cmd.is_none());
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

    /// A model with a real (file-backed) document — `ShowHoverAt`, like
    /// `ShowHover`, refuses untitled documents (never LSP-synced), so a
    /// plain `model()` (no path) can't exercise it.
    fn model_with_file() -> (tempfile::TempDir, AppModel) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("main.rs");
        std::fs::write(&path, "fn main() {}\n").unwrap();
        let model = AppModel::new(800, 600, 1.0, vec![path]);
        (dir, model)
    }

    #[test]
    fn show_hover_at_captures_the_given_position_not_the_caret() {
        let (_dir, mut model) = model_with_file();
        // Caret sits at (0, 0); the mouse dwell targets a different cell.
        model.editor_mut().cursors[0] = crate::model::Cursor::at(0, 3);

        let cmd = update_lsp(&mut model, LspMsg::ShowHoverAt { line: 0, col: 8 });

        let Some(Cmd::LspRequestHover { cursor, .. }) = cmd else {
            panic!("expected Cmd::LspRequestHover, got {cmd:?}");
        };
        assert_eq!(cursor, crate::model::editor::Position::new(0, 8));
        assert_eq!(
            model.ui.mouse_hover_target,
            Some(crate::model::editor::Position::new(0, 8)),
            "the dwell target must be captured for HoverResolved's guard"
        );
    }

    /// The runtime clears `mouse_hover_target` the moment the pointer moves
    /// away from a still-pending dwell request (see `App::update_hover_dwell`)
    /// — a reply that arrives after that must never open the card, even
    /// though the caret never moved (it's mouse-driven, not caret-driven).
    #[test]
    fn hover_resolved_for_an_abandoned_dwell_target_is_dropped() {
        let (_dir, mut model) = model_with_file();
        let doc_id = model.document().id.unwrap();
        let revision = model.document().revision;
        let target = crate::model::editor::Position::new(0, 8);

        update_lsp(&mut model, LspMsg::ShowHoverAt { line: 0, col: 8 });
        assert_eq!(model.ui.mouse_hover_target, Some(target));
        // Simulate the runtime's dwell reset (pointer moved on before the
        // reply landed).
        model.ui.mouse_hover_target = None;

        update_lsp(
            &mut model,
            LspMsg::HoverResolved {
                document_id: doc_id,
                revision,
                cursor: target,
                outcome: HoverOutcome::Content(Some("stale dwell hover".to_owned())),
            },
        );

        assert!(
            model.ui.hover_card.is_none(),
            "a reply for an abandoned dwell target must not open the card"
        );
    }

    /// A reply for a dwell target the pointer is still sitting on opens the
    /// card anchored at that text cell, not the (unrelated) caret —
    /// `HoverCardState::anchor` is what `view::modal` uses to place it.
    #[test]
    fn hover_resolved_for_a_live_dwell_target_opens_anchored_at_it() {
        let (_dir, mut model) = model_with_file();
        let doc_id = model.document().id.unwrap();
        let revision = model.document().revision;
        let target = crate::model::editor::Position::new(0, 8);

        update_lsp(&mut model, LspMsg::ShowHoverAt { line: 0, col: 8 });

        update_lsp(
            &mut model,
            LspMsg::HoverResolved {
                document_id: doc_id,
                revision,
                cursor: target,
                outcome: HoverOutcome::Content(Some("fn main()".to_owned())),
            },
        );

        assert_eq!(
            model.ui.hover_card.as_ref().and_then(|c| c.anchor),
            Some((0, 8))
        );
    }

    /// A keyboard-invoked `ShowHover` reply must keep anchoring to the
    /// caret rect (view fallback) rather than a stale mouse target — even
    /// when a dwell happened to target the very same cell earlier.
    #[test]
    fn hover_resolved_for_the_caret_leaves_anchor_unset() {
        let (_dir, mut model) = model_with_file();
        let doc_id = model.document().id.unwrap();
        let revision = model.document().revision;

        let cmd = update_lsp(&mut model, LspMsg::ShowHover);
        let Some(Cmd::LspRequestHover { cursor, .. }) = cmd else {
            panic!("expected Cmd::LspRequestHover, got {cmd:?}");
        };
        assert!(
            model.ui.mouse_hover_target.is_none(),
            "a caret-triggered request must not look like a live dwell target"
        );

        update_lsp(
            &mut model,
            LspMsg::HoverResolved {
                document_id: doc_id,
                revision,
                cursor,
                outcome: HoverOutcome::Content(Some("fn main()".to_owned())),
            },
        );

        assert_eq!(model.ui.hover_card.as_ref().and_then(|c| c.anchor), None);
    }

    fn loc(path: &Path, line: u32, col: u32, preview: &str) -> navigation::LocationItem {
        navigation::LocationItem {
            path: path.to_path_buf(),
            position: lsp_types::Position {
                line,
                character: col,
            },
            preview: preview.to_owned(),
            route_hint: None,
        }
    }

    #[test]
    fn references_resolved_for_a_stale_revision_is_dropped() {
        let (_dir, mut model) = model_with_file();
        let doc_id = model.document().id.unwrap();
        let cursor = model.editor().active_cursor().to_position();
        let path = model.document().file_path.clone().unwrap();
        // One past the document's actual revision — a stale reply.
        let stale_revision = model.document().revision + 1;

        let cmd = update_lsp(
            &mut model,
            LspMsg::ReferencesResolved {
                document_id: doc_id,
                revision: stale_revision,
                cursor,
                items: vec![loc(&path, 0, 0, "fn main() {}")],
                outcome: ReferencesOutcome::Found,
            },
        );

        assert!(cmd.is_none());
        assert!(
            model.ui.cursor_overlay.is_none(),
            "a stale-revision reply must never open the popup"
        );
    }

    #[test]
    fn references_resolved_with_one_item_jumps_without_opening_the_popup() {
        let (_dir, mut model) = model_with_file();
        let doc_id = model.document().id.unwrap();
        let revision = model.document().revision;
        let cursor = model.editor().active_cursor().to_position();
        let path = model.document().file_path.clone().unwrap();

        update_lsp(
            &mut model,
            LspMsg::ReferencesResolved {
                document_id: doc_id,
                revision,
                cursor,
                items: vec![loc(&path, 0, 3, "fn main() {}")],
                outcome: ReferencesOutcome::Found,
            },
        );

        assert!(
            model.ui.cursor_overlay.is_none(),
            "exactly one usage jumps directly, no popup"
        );
        assert_eq!(model.editor().cursors[0].line, 0);
        assert_eq!(model.editor().cursors[0].column, 3);
    }

    #[test]
    fn references_resolved_with_multiple_items_opens_the_popup_sorted() {
        let (_dir, mut model) = model_with_file();
        let doc_id = model.document().id.unwrap();
        let revision = model.document().revision;
        let cursor = model.editor().active_cursor().to_position();
        let path = model.document().file_path.clone().unwrap();

        update_lsp(
            &mut model,
            LspMsg::ReferencesResolved {
                document_id: doc_id,
                revision,
                cursor,
                // Deliberately out of (path, line) order, to assert the
                // popup sorts rather than trusting server reply order.
                items: vec![loc(&path, 5, 0, "second"), loc(&path, 0, 0, "first")],
                outcome: ReferencesOutcome::Found,
            },
        );

        assert_eq!(
            model.ui.cursor_overlay.map(|o| o.kind),
            Some(CursorOverlayKind::References)
        );
        let items = model.ui.reference_list.as_ref().expect("popup rows stored");
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].position.line, 0, "sorted by (path, line)");
        assert_eq!(items[1].position.line, 5);
    }

    #[test]
    fn activate_reference_jumps_to_reference_list_at_the_given_index_ordering_authority() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("main.rs");
        std::fs::write(&path, "one\ntwo\nthree\nfour\nfive\nsix\n").unwrap();
        let mut model = AppModel::new(800, 600, 1.0, vec![path.clone()]);
        model.ui.reference_list = Some(vec![loc(&path, 0, 0, "a"), loc(&path, 5, 0, "b")]);
        model.ui.cursor_overlay = Some(CursorOverlayState::new(CursorOverlayKind::References));

        // Enter on row 1 must jump to `reference_list[1]`, not row 0 — the
        // same stored `Vec` the view rendered from (view order == confirm
        // order).
        let cmd = update_lsp(&mut model, LspMsg::ActivateReference { index: 1 });

        assert!(cmd.is_some());
        assert!(
            model.ui.cursor_overlay.is_none(),
            "popup dismisses on activate"
        );
        assert!(model.ui.reference_list.is_none());
        assert_eq!(model.editor().cursors[0].line, 5);
    }

    #[test]
    fn definition_resolved_with_multiple_locations_opens_the_same_popup() {
        let dir = tempfile::tempdir().unwrap();
        let origin_path = dir.path().join("origin.rs");
        std::fs::write(&origin_path, "one\ntwo\n").unwrap();
        let mut model = AppModel::new(800, 600, 1.0, vec![origin_path.clone()]);
        let origin = navigation::current_jump_entry(&model).unwrap();
        let doc_id = model.document().id.unwrap();
        let revision = model.document().revision;

        let a_uri = crate::lsp::path_to_uri(&dir.path().join("a.rs"));
        let b_uri = crate::lsp::path_to_uri(&dir.path().join("b.rs"));
        let location = |uri: lsp_types::Uri, line: u32| lsp_types::Location {
            uri,
            range: lsp_types::Range {
                start: lsp_types::Position { line, character: 0 },
                end: lsp_types::Position { line, character: 0 },
            },
        };

        let cmd = update_lsp(
            &mut model,
            LspMsg::DefinitionResolved {
                document_id: doc_id,
                revision,
                origin,
                outcome: DefinitionOutcome::Locations {
                    locations: vec![location(a_uri, 0), location(b_uri, 1)],
                    resolving_server: LspServerId::from("rust-analyzer"),
                    resolving_root: PathBuf::from("/tmp"),
                },
            },
        );

        assert!(cmd.is_some());
        assert_eq!(
            model.ui.cursor_overlay.map(|o| o.kind),
            Some(CursorOverlayKind::References),
            "a multi-location definition reply upgrades to the same popup Show Usages uses"
        );
        assert_eq!(model.ui.reference_list.as_ref().map(Vec::len), Some(2));
        // No jump happened — the origin document must still be focused.
        assert_eq!(
            model.document().file_path.as_deref(),
            Some(origin_path.as_path())
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
