//! `LspMsg` handlers (lsp-integration.md Phases 1-3).
//!
//! Phase 1 lifecycle traffic (model mirror, restart) and Phase 2
//! diagnostics projection live here; Phase 3 adds go-to-definition's
//! user-intent capture and revision-guarded response handling. Hover and
//! completion land with their phases.

use crate::commands::Cmd;
use crate::lsp::ServerState;
use crate::messages::{DefinitionOutcome, LspMsg, ReferencesOutcome};
use crate::model::editor::Position;
use crate::model::editor_area::DocumentId;
use crate::model::{AppModel, CursorOverlayKind, CursorOverlayState};
use crate::update::navigation;
use crate::update::text_edits::{apply_planned_edits, plan_text_edits};

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
    model.ui.set_status(if enabled {
        "LSP enabled"
    } else {
        "LSP disabled"
    });
    Some(Cmd::Batch(vec![
        Cmd::SaveConfiguration {
            config: Box::new(model.config.clone()),
        },
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
    Some(Cmd::Batch(vec![
        Cmd::SaveConfiguration {
            config: Box::new(model.config.clone()),
        },
        Cmd::LspSetServerEnabled {
            server_id: crate::lsp::LspServerId::from(server_id),
            enabled,
        },
        Cmd::Redraw,
    ]))
}

/// The shared staleness gate every async LSP feature reply passes before
/// it may act: the requesting document must still exist and be unedited
/// (revision match), and must still be the *focused* editor's document.
/// Returns `true` when the reply must be dropped.
///
/// The focus check compares against the document the request was issued
/// for — if focus has since moved to a different tab/split, comparing
/// against whatever editor is now focused would compare unrelated
/// positions (both commonly at 0,0) and could open a popup over the
/// wrong document.
///
/// Cursor guards are intentionally NOT part of this helper: hover's
/// mouse-dwell replies stay live while the pointer rests on the captured
/// target even after the caret moved, and references has no dwell case —
/// each site expresses its own rule in one line below.
fn stale_feature_response(
    model: &AppModel,
    document_id: crate::model::editor_area::DocumentId,
    revision: u64,
) -> bool {
    let Some(doc) = model.editor_area.documents.get(&document_id) else {
        return true;
    };
    if doc.revision != revision {
        return true;
    }
    model.try_document().and_then(|d| d.id) != Some(document_id)
}

/// `LspMsg::JumpDiagnostic`: caret to the next/previous diagnostic start
/// (sorted by position, wrapping), skipping ranges the buffer has since
/// outgrown. Flashes the diagnostic's first line in the status bar.
fn jump_diagnostic(model: &mut AppModel, forward: bool) -> Option<Cmd> {
    let doc = model.document();
    let cursor = model.editor().active_cursor();
    let caret = (cursor.line, cursor.column);
    let mut targets: Vec<_> = doc
        .diagnostics
        .iter()
        .filter(|d| !crate::lsp::position::range_vanished(doc, d.range))
        .map(|d| {
            let p = crate::lsp::lsp_to_position(doc, d.range.start);
            ((p.line, p.column), d.range.start, d.message.clone())
        })
        .collect();
    targets.sort_by_key(|t| t.0);
    let pick = if forward {
        targets.iter().find(|t| t.0 > caret).or(targets.first())
    } else {
        targets
            .iter()
            .rev()
            .find(|t| t.0 < caret)
            .or(targets.last())
    };
    let Some((_, start, message)) = pick.cloned() else {
        model.ui.set_status("No diagnostics in this file");
        return Some(Cmd::redraw_status_bar());
    };
    let path = doc.file_path.clone()?;
    let cmd = navigation::jump_to_location(model, None, &path, start);
    model
        .ui
        .set_status(crate::model::status_bar::truncate_status_message(
            message.lines().next().unwrap_or_default(),
        ));
    super::merge_cmds(cmd, Some(Cmd::redraw_status_bar()))
}

/// `Cmd::LspRequestSignatureHelp` at the caret of the focused, file-backed
/// document — shared by `ShowSignatureHelp` (explicit invoke) and the
/// typing-driven path in `update/completion.rs`.
pub(crate) fn request_signature_help(
    model: &AppModel,
    trigger: Option<String>,
    is_retrigger: bool,
) -> Option<Cmd> {
    let doc = model.try_document()?;
    let document_id = doc.id?;
    doc.file_path.as_ref()?;
    let cursor = model.editor().active_cursor().to_position();
    Some(Cmd::LspRequestSignatureHelp {
        document_id,
        position: crate::lsp::position_to_lsp(doc, cursor),
        cursor,
        revision: doc.revision,
        trigger,
        is_retrigger,
    })
}

/// The identifier the caret touches (either side), as
/// `(start_offset, end_offset)`; empty when the caret is not on a word.
fn word_at_caret(doc: &crate::model::Document, cursor: Position) -> (usize, usize) {
    use crate::update::document::{word_end_after, word_start_before};
    use crate::util::text::{char_type, CharType};
    let buffer = &doc.buffer;
    let offset = doc.cursor_to_offset(cursor.line, cursor.column);
    let is_word =
        |i: usize| i < buffer.len_chars() && char_type(buffer.char(i)) == CharType::WordChar;
    let start = if offset > 0 && is_word(offset - 1) {
        word_start_before(buffer, offset)
    } else {
        offset
    };
    let end = if is_word(offset) {
        word_end_after(buffer, offset)
    } else {
        offset
    };
    (start, end)
}

/// Opens the Rename Symbol prompt prefilled with `placeholder`, or flashes
/// "Cannot rename here" when there is nothing to rename.
fn open_rename_prompt(
    model: &mut AppModel,
    document_id: DocumentId,
    revision: u64,
    position: Position,
    placeholder: String,
) -> Option<Cmd> {
    if placeholder.is_empty() {
        model.ui.set_status("Cannot rename here");
        return Some(Cmd::redraw_status_bar());
    }
    model.ui.open_modal(crate::model::ModalState::RenameSymbol(
        crate::model::RenameSymbolState::new(placeholder, document_id, revision, position),
    ));
    Some(Cmd::Redraw)
}

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
    doc.file_path.as_ref()?;
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
    super::merge_cmds(
        settled,
        Some(Cmd::LspRequestFormatting {
            document_id,
            revision: doc.revision,
            range,
            options: formatting_options(doc.text_settings),
            save: None,
        }),
    )
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

pub(super) fn update_lsp(model: &mut AppModel, msg: LspMsg) -> Option<Cmd> {
    match msg {
        LspMsg::WorkspaceSymbolProviders(providers) => {
            model.lsp.workspace_symbol_providers = providers;
            Some(Cmd::Redraw)
        }
        LspMsg::WorkspaceSymbolsReady { request, results } => {
            if model.ui.workspace_symbol_request.as_ref() != Some(&request) {
                return None;
            }
            let Some(crate::model::ModalState::CommandPalette(state)) = &mut model.ui.active_modal
            else {
                return None;
            };
            if state.input() != request.query {
                return None;
            }
            state.symbols.results = results;
            state.symbols.searching = false;
            state.symbols.selected_index = 0;
            state.symbols.scroll_offset = 0;
            Some(Cmd::Redraw)
        }
        LspMsg::WorkspaceSymbolsResponseFromServer { .. } => None,
        LspMsg::FormatDocument { selection_only } => request_formatting(model, selection_only),
        LspMsg::FormattingResolved {
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
            if stale_feature_response(model, document_id, revision) {
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
        LspMsg::FormattingResponseFromServer { .. } => None,
        LspMsg::ShowSignatureHelp => request_signature_help(model, None, false),
        LspMsg::RenameSymbol => {
            let doc = model.try_document()?;
            let document_id = doc.id?;
            doc.file_path.as_ref()?;
            let cursor = model.editor().active_cursor().to_position();
            let (start, end) = word_at_caret(doc, cursor);
            // The runtime decides between `prepareRename` and prompting
            // straight away with `fallback` (server capabilities live
            // there) — see `App::request_lsp_prepare_rename`.
            Some(Cmd::LspRequestPrepareRename {
                document_id,
                position: crate::lsp::position_to_lsp(doc, cursor),
                cursor,
                revision: doc.revision,
                fallback: doc.buffer.slice(start..end).to_string(),
            })
        }
        LspMsg::PrepareRenameResolved {
            document_id,
            revision,
            cursor,
            placeholder,
        } => {
            if stale_feature_response(model, document_id, revision) {
                return None;
            }
            open_rename_prompt(
                model,
                document_id,
                revision,
                cursor,
                placeholder.unwrap_or_default(),
            )
        }
        LspMsg::PrepareRenameResponseFromServer { .. } => None,
        LspMsg::RenameResolved {
            document_id,
            revision,
            edit,
        } => {
            if stale_feature_response(model, document_id, revision) {
                return None;
            }
            let Some(edit) = edit else {
                model.ui.set_status("Nothing to rename");
                return Some(Cmd::redraw_status_bar());
            };
            super::text_edits::start_workspace_edit(
                model,
                *edit,
                crate::model::WorkspaceEditAction::Rename,
            )
        }
        LspMsg::RenameResponseFromServer { .. } => None,
        LspMsg::ServerSignatureTriggers {
            server_id,
            trigger,
            retrigger,
        } => {
            if trigger.is_empty() && retrigger.is_empty() {
                model.lsp.signature_trigger_characters.remove(&server_id);
            } else {
                model
                    .lsp
                    .signature_trigger_characters
                    .insert(server_id, (trigger, retrigger));
            }
            None
        }
        LspMsg::SignatureHelpResolved {
            document_id,
            revision,
            cursor,
            help,
        } => {
            // Revision + focus guards, then a same-line caret guard: the
            // float follows the caret along the line (typing arguments),
            // but a reply for another line must never open it there.
            if stale_feature_response(model, document_id, revision) {
                return None;
            }
            if model.editor().active_cursor().line != cursor.line {
                return None;
            }
            model.ui.signature_help = help.filter(|h| !h.signatures.is_empty());
            Some(Cmd::Redraw)
        }
        LspMsg::SignatureHelpResponseFromServer { .. } => None,
        LspMsg::ServerStateChanged {
            server_id, state, ..
        } => {
            if let Some(message) = status_transient_for(&server_id, state) {
                model.ui.set_status(message);
            }
            model.lsp.servers.insert(server_id, state);
            Some(
                if matches!(
                    model.ui.active_modal,
                    Some(
                        crate::model::ModalState::Settings(_)
                            | crate::model::ModalState::LspServers(_)
                    )
                ) {
                    Cmd::Redraw
                } else {
                    Cmd::redraw_status_bar()
                },
            )
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

        LspMsg::JumpDiagnostic { forward } => jump_diagnostic(model, forward),
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
            if stale_feature_response(model, document_id, revision) {
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
        // reaching here — mirrors `DefinitionResponseFromServer`.
        LspMsg::DefinitionResponseFromServer { .. } => None,

        // Consumed by `process_async_messages`'s interception passes —
        // mirrors the other raw worker replies.
        LspMsg::CompletionResponseFromServer { .. } | LspMsg::ResolveResponseFromServer { .. } => {
            None
        }

        LspMsg::ShowHover => {
            let cursor = model.editor().active_cursor().to_position();
            super::hover::show(model, cursor, crate::model::hover::HoverOrigin::Keyboard)
        }
        LspMsg::DismissHover => super::hover::dismiss(model),
        LspMsg::ShowHoverAt { line, col } => super::hover::show(
            model,
            Position::new(line, col),
            crate::model::hover::HoverOrigin::Mouse,
        ),

        LspMsg::HoverResolved {
            document_id,
            revision,
            cursor,
            outcome,
        } => super::hover::resolved(model, document_id, revision, cursor, outcome),

        // Consumed by `process_async_messages`'s interception pass before
        // reaching here — mirrors `DefinitionResponseFromServer`.
        LspMsg::HoverResponseFromServer { .. } => None,

        LspMsg::ShowCodeActions => {
            let doc = model.try_document()?;
            let document_id = doc.id?;
            doc.file_path.as_ref()?;
            let editor = model.editor();
            let cursor = editor.active_cursor().to_position();
            let selection = editor.active_selection();
            let (start, end) = if selection.is_empty() {
                (cursor, cursor)
            } else {
                (selection.start(), selection.end())
            };
            let range = lsp_types::Range {
                start: crate::lsp::position_to_lsp(doc, start),
                end: crate::lsp::position_to_lsp(doc, end),
            };
            let diagnostics = doc
                .diagnostics
                .iter()
                .filter(|d| d.range.start <= range.end && range.start <= d.range.end)
                .cloned()
                .collect();
            Some(Cmd::LspRequestCodeActions {
                document_id,
                position: range.start,
                range,
                cursor,
                revision: doc.revision,
                diagnostics,
            })
        }

        LspMsg::CodeActionsResolved {
            document_id,
            revision,
            cursor,
            mut actions,
            outcome,
        } => {
            if stale_feature_response(model, document_id, revision) {
                return None;
            }
            if model.editor().active_cursor().to_position() != cursor {
                return None;
            }
            let status = match outcome {
                ReferencesOutcome::StillIndexing => "Language server still indexing…",
                ReferencesOutcome::NotSupported => "Code actions not supported by this server",
                ReferencesOutcome::NoResult | ReferencesOutcome::TimedOut => {
                    "Code actions: server did not answer"
                }
                ReferencesOutcome::Found if actions.is_empty() => "No code actions",
                ReferencesOutcome::Found => {
                    // Stable sort: preferred first, server order within.
                    actions.sort_by_key(|a| !a.is_preferred);
                    model.ui.code_action_list = Some(actions);
                    model.ui.cursor_overlay =
                        Some(CursorOverlayState::new(CursorOverlayKind::CodeActions));
                    return Some(Cmd::Redraw);
                }
            };
            model.ui.set_status(status);
            Some(Cmd::redraw_status_bar())
        }

        LspMsg::CodeActionsResponseFromServer { .. } => None,

        LspMsg::ActivateCodeAction { index } => {
            let item = model
                .ui
                .code_action_list
                .as_ref()
                .and_then(|items| items.get(index))
                .cloned();
            model.ui.cursor_overlay = None;
            model.ui.code_action_list = None;
            let Some(item) = item else {
                return Some(Cmd::Redraw);
            };
            let document_id = model.try_document().and_then(|d| d.id);
            super::text_edits::start_workspace_edit(
                model,
                item.edit.map(|edit| *edit).unwrap_or_default(),
                crate::model::WorkspaceEditAction::CodeAction {
                    title: item.title,
                    command: item.command,
                    document_id,
                },
            )
        }

        LspMsg::FindReferences => super::usages::request(model, false),
        LspMsg::FindUsagesInPanel => super::usages::request(model, true),

        LspMsg::ReferencesResolved {
            target,
            document_id,
            revision,
            cursor,
            items,
            outcome,
        } => {
            if let crate::model::usages::ReferencesTarget::Panel(token) = target {
                return super::usages::resolve(model, token, document_id, revision, items, outcome);
            }
            // Revision + focus guards (see `stale_feature_response`),
            // plus the caret guard verbatim from `HoverResolved` (minus
            // the mouse-dwell branch — references has no mouse trigger).
            if stale_feature_response(model, document_id, revision) {
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
                ReferencesOutcome::TimedOut => {
                    model.ui.set_status("Usages request timed out; try again");
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
            navigation::activate_location(model, &item)
        }

        // ==== Completion (lsp-integration.md Phase 5) ====
        LspMsg::ServerCompletionTriggers {
            server_id,
            characters,
        } => {
            if characters.is_empty() {
                model.lsp.completion_trigger_characters.remove(&server_id);
            } else {
                model
                    .lsp
                    .completion_trigger_characters
                    .insert(server_id, characters);
            }
            None
        }
        LspMsg::CompletionResolved {
            document_id,
            revision,
            items,
            is_incomplete,
        } => super::completion::merge_lsp_completion(
            model,
            document_id,
            revision,
            items,
            is_incomplete,
        ),
        LspMsg::CompletionItemResolved {
            document_id,
            revision,
            selected,
            detail,
            documentation,
            additional_text_edits,
        } => super::completion::finish_deferred_accept(
            model,
            document_id,
            revision,
            selected,
            detail,
            documentation,
            additional_text_edits,
        ),
        LspMsg::ApplyEditRequested {
            server_id,
            root,
            request_id,
            edit,
            label,
        } => super::text_edits::start_workspace_edit(
            model,
            *edit,
            crate::model::WorkspaceEditAction::Server {
                server_id,
                root,
                request_id,
                label,
            },
        ),
    }
}

/// A stored `LocationItem` resolves its own open path when activated — set
/// `model.lsp.route_hint` from the item's own resolving server/root
/// (`None` inside the workspace) before jumping, so an out-of-workspace
/// target still reuses the server that resolved it instead of letting
/// `open_lsp_document`'s generic path derive (and possibly spawn) its own
/// root (lsp-integration.md "never spawn a new server rooted in a
/// toolchain directory").
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
        return navigation::activate_location(model, only);
    }
    model.ui.reference_list = Some(items);
    model.ui.cursor_overlay = Some(CursorOverlayState::new(CursorOverlayKind::References));
    Some(Cmd::Redraw)
}

/// Diagnostics and workspace edits share the document's boundary-resolved identity.
pub(crate) fn find_document_by_uri(model: &AppModel, uri: &lsp_types::Uri) -> Option<DocumentId> {
    model
        .editor_area
        .find_document_by_path(&crate::lsp::uri_to_path(uri)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lsp::{LspServerId, ServerState};
    use crate::messages::HoverOutcome;
    use std::path::{Path, PathBuf};

    fn model() -> AppModel {
        AppModel::new(800, 600, 1.0)
    }

    #[test]
    fn file_identity_diagnostics_and_problems_use_the_loaded_snapshot() {
        let mut model = model();
        let document_id = model.document().id;
        let identity = crate::util::FileIdentity::from_resolved(
            "/fixture/different-name.rs".into(),
            Path::new("/fixture/real.rs"),
        );
        *model.document_mut() =
            crate::model::Document::from_loaded_text("fn café() {}", identity.clone());
        model.document_mut().id = document_id;
        let diagnostic = lsp_types::Diagnostic::new_simple(Default::default(), "snapshot".into());
        update_lsp(
            &mut model,
            LspMsg::DiagnosticsPublished {
                uri: identity.uri().clone(),
                version: None,
                diagnostics: vec![diagnostic.clone()],
            },
        );
        assert_eq!(model.document().diagnostics, vec![diagnostic]);
        assert_eq!(crate::update::problems::problems_row_count(&model), 2);
        model.document_mut().file_path = Some("/fixture/other.rs".into());
        assert!(find_document_by_uri(&model, identity.uri()).is_none());
        assert_eq!(crate::update::problems::problems_row_count(&model), 0);
    }

    #[test]
    fn jump_diagnostic_walks_sorted_starts_wraps_and_skips_vanished_ranges() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.rs");
        std::fs::write(&path, "x\n".repeat(12)).unwrap();
        let mut model = AppModel::with_document(
            800,
            600,
            1.0,
            crate::model::Document::from_file(path).unwrap(),
        );
        let diag = |line: u32, message: &str| lsp_types::Diagnostic {
            range: lsp_types::Range::new(
                lsp_types::Position::new(line, 0),
                lsp_types::Position::new(line, 1),
            ),
            message: message.to_owned(),
            ..Default::default()
        };
        // Deliberately unsorted, with one range past the end of the buffer.
        model.document_mut().diagnostics = vec![
            diag(9, "nine"),
            diag(50, "vanished"),
            diag(1, "one\nsecond line"),
            diag(5, "five"),
        ];
        model.editor_mut().cursors[0].line = 5;

        update_lsp(&mut model, LspMsg::JumpDiagnostic { forward: true });
        assert_eq!(model.editor().cursors[0].line, 9);
        update_lsp(&mut model, LspMsg::JumpDiagnostic { forward: true });
        assert_eq!(
            model.editor().cursors[0].line,
            1,
            "wraps past the vanished range"
        );
        assert_eq!(model.ui.transient_message.as_ref().unwrap().text, "one");
        update_lsp(&mut model, LspMsg::JumpDiagnostic { forward: false });
        assert_eq!(model.editor().cursors[0].line, 9);

        model.document_mut().diagnostics.clear();
        update_lsp(&mut model, LspMsg::JumpDiagnostic { forward: true });
        assert_eq!(
            model.ui.transient_message.as_ref().unwrap().text,
            "No diagnostics in this file"
        );
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
        let (_dir, mut model) = model_with_file();
        update_lsp(&mut model, LspMsg::ShowHover);
        assert!(model.ui.hover_request.is_some());
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
                outcome: HoverOutcome::Content(Some("stale hover".into())),
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

    /// A failed deferred open must never apply destination coordinates to the
    /// origin document ("no stale result ever moves a cursor").
    #[test]
    fn definition_resolved_to_an_unopenable_target_does_not_move_the_origin_cursor() {
        let dir = tempfile::tempdir().unwrap();
        let origin_path = dir.path().join("origin.rs");
        std::fs::write(&origin_path, "one\ntwo\nthree\n").unwrap();
        let mut model = AppModel::with_document(
            800,
            600,
            1.0,
            crate::model::Document::from_file(origin_path.clone()).unwrap(),
        );
        model.editor_mut().cursors[0].line = 0;
        model.editor_mut().cursors[0].column = 0;

        let origin = navigation::current_jump_entry(&model).unwrap();
        let doc_id = model.document().id.unwrap();
        let revision = model.document().revision;

        // A directory "location" (a server bug, but not one the client may
        // trust) makes `validate_file_for_opening` reject it as
        // `IsDirectory` — rejection leaves the origin view unchanged.
        let target_path = dir.path().join("target_dir");
        std::fs::create_dir(&target_path).unwrap();
        let target_uri = crate::lsp::path_to_uri(&target_path);

        let cmd = update_lsp(
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
        crate::update::finish_test_file_opens(&mut model, cmd);

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
        let mut model = AppModel::with_document(
            800,
            600,
            1.0,
            crate::model::Document::from_file(ws_dir.path().join("main.rs")).unwrap(),
        );
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
        let cmd = crate::update::finish_test_file_opens(&mut model, cmd);
        assert!(
            cmd.is_some_and(|c| contains_did_open_on_server(&c, &server_id, &resolving_root)),
            "opening the out-of-workspace target must route didOpen to the resolving server, \
             not the generic ensure-server/resolve-root path"
        );

        // The hint was captured by the open intent and consumed after the
        // destination loaded, never left in global state across the async gap.
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
    fn apply_edit_requested_applies_and_replies_applied_true() {
        let (dir, mut model) = model_with_file();
        let path = dir.path().join("main.rs");
        #[allow(clippy::mutable_key_type)]
        let changes = std::collections::HashMap::from([(
            crate::lsp::path_to_uri(&path),
            vec![lsp_types::TextEdit::new(
                lsp_types::Range::new(
                    lsp_types::Position::new(0, 3),
                    lsp_types::Position::new(0, 7),
                ),
                "start".to_owned(),
            )],
        )]);
        let cmd = update_lsp(
            &mut model,
            LspMsg::ApplyEditRequested {
                server_id: LspServerId::from("fake"),
                root: dir.path().to_path_buf(),
                request_id: serde_json::json!(7),
                edit: Box::new(lsp_types::WorkspaceEdit::new(changes)),
                label: None,
            },
        )
        .expect("cmd");
        assert_eq!(model.document().buffer.to_string(), "fn start() {}\n");
        fn find_reply(cmd: &Cmd) -> Option<(&serde_json::Value, &serde_json::Value)> {
            match cmd {
                Cmd::Batch(cmds) => cmds.iter().find_map(find_reply),
                Cmd::LspRespondToServer {
                    request_id, result, ..
                } => Some((request_id, result)),
                _ => None,
            }
        }
        let (id, result) = find_reply(&cmd).expect("LspRespondToServer");
        assert_eq!(id, &serde_json::json!(7));
        assert_eq!(result, &serde_json::json!({ "applied": true }));
        assert!(model
            .ui
            .transient_message
            .as_ref()
            .is_some_and(|t| t.text == "Applied 1 edits in 1 files"));
    }

    #[test]
    #[cfg(unix)]
    fn find_document_by_uri_matches_through_a_renaming_symlink() {
        use std::os::unix::fs::symlink;

        let dir = tempfile::tempdir().unwrap();
        let real_path = dir.path().join("real.rs");
        std::fs::write(&real_path, "fn main() {}\n").unwrap();
        let link_path = dir.path().join("link.rs");
        symlink(&real_path, &link_path).unwrap();

        let model = AppModel::with_document(
            800,
            600,
            1.0,
            crate::model::Document::from_file(link_path.clone()).unwrap(),
        );
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
        let model = AppModel::with_document(
            800,
            600,
            1.0,
            crate::model::Document::from_file(path).unwrap(),
        );
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
            model.ui.hover_request.map(|request| request.position),
            Some(crate::model::editor::Position::new(0, 8)),
            "the dwell target must be captured for HoverResolved's guard"
        );
    }

    /// The runtime clears the hover intent the moment the pointer moves
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
        assert_eq!(
            model.ui.hover_request.map(|request| request.position),
            Some(target)
        );
        // Simulate the runtime's dwell reset (pointer moved on before the
        // reply landed).
        update_lsp(&mut model, LspMsg::DismissHover);

        update_lsp(
            &mut model,
            LspMsg::HoverResolved {
                document_id: doc_id,
                revision,
                cursor: target,
                outcome: HoverOutcome::Content(Some("stale dwell hover".into())),
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
                outcome: HoverOutcome::Content(Some("fn main()".into())),
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
            model.ui.hover_request.is_some_and(|request| request.origin == crate::model::hover::HoverOrigin::Keyboard),
            "a caret-triggered request must not look like a live dwell target"
        );

        update_lsp(
            &mut model,
            LspMsg::HoverResolved {
                document_id: doc_id,
                revision,
                cursor,
                outcome: HoverOutcome::Content(Some("fn main()".into())),
            },
        );

        assert_eq!(model.ui.hover_card.as_ref().and_then(|c| c.anchor), None);
    }

    #[test]
    fn hover_drops_replies_after_editing_or_moving_the_caret() {
        for edit in [false, true] {
            let (_dir, mut model) = model_with_file();
            update_lsp(&mut model, LspMsg::ShowHoverAt { line: 0, col: 8 });
            let request = model.ui.hover_request.unwrap();
            let change = if edit {
                crate::messages::Msg::Document(crate::messages::DocumentMsg::InsertChar('x'))
            } else {
                crate::messages::Msg::Editor(crate::messages::EditorMsg::MoveCursor(
                    crate::messages::Direction::Right,
                ))
            };
            crate::update::update(&mut model, change);
            assert!(model.ui.hover_request.is_none());
            update_lsp(
                &mut model,
                LspMsg::HoverResolved {
                    document_id: request.anchor.document_id,
                    revision: request.anchor.revision,
                    cursor: request.position,
                    outcome: HoverOutcome::Content(Some("stale".into())),
                },
            );
            assert!(model.ui.hover_card.is_none());
        }
    }

    #[test]
    fn automatic_hover_is_silent_and_yields_to_other_popups() {
        let (_dir, mut model) = model_with_file();
        for outcome in [
            HoverOutcome::Content(None),
            HoverOutcome::StillIndexing,
            HoverOutcome::NotSupported,
        ] {
            update_lsp(&mut model, LspMsg::ShowHoverAt { line: 0, col: 8 });
            let request = model.ui.hover_request.unwrap();
            let status = model
                .ui
                .transient_message
                .as_ref()
                .map(|message| message.text.clone());
            assert!(update_lsp(
                &mut model,
                LspMsg::HoverResolved {
                    document_id: request.anchor.document_id,
                    revision: request.anchor.revision,
                    cursor: request.position,
                    outcome,
                }
            )
            .is_none());
            assert_eq!(
                model
                    .ui
                    .transient_message
                    .as_ref()
                    .map(|message| message.text.clone()),
                status
            );
            assert!(model.ui.hover_card.is_none());
            update_lsp(&mut model, LspMsg::DismissHover);
        }
        model.ui.cursor_overlay = Some(CursorOverlayState::new(CursorOverlayKind::Completion));
        assert!(update_lsp(&mut model, LspMsg::ShowHoverAt { line: 0, col: 8 }).is_none());
        assert!(model.ui.hover_request.is_none());
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
                target: crate::model::usages::ReferencesTarget::Popup,
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
                target: crate::model::usages::ReferencesTarget::Popup,
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
                target: crate::model::usages::ReferencesTarget::Popup,
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
        let mut model = AppModel::with_document(
            800,
            600,
            1.0,
            crate::model::Document::from_file(path.clone()).unwrap(),
        );
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
        let mut model = AppModel::with_document(
            800,
            600,
            1.0,
            crate::model::Document::from_file(origin_path.clone()).unwrap(),
        );
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

    // ---- formatting ----

    fn range(line: u32, start: u32, end: u32) -> lsp_types::Range {
        lsp_types::Range::new(
            lsp_types::Position::new(line, start),
            lsp_types::Position::new(line, end),
        )
    }

    fn resolve_formatting(
        model: &mut AppModel,
        revision: u64,
        edits: Option<Vec<(lsp_types::Range, String)>>,
        then_save: bool,
    ) -> Option<Cmd> {
        let document_id = model.document().id.unwrap();
        let save = then_save.then(|| {
            model.config.format_on_save = true;
            let path = model.document().file_path.clone().unwrap();
            let cmd = super::super::app::request_save(
                model,
                document_id,
                path,
                crate::model::SaveReason::Manual,
            );
            let Some(Cmd::LspRequestFormatting {
                save: Some(save), ..
            }) = cmd
            else {
                panic!("expected save formatting request");
            };
            save
        });
        update_lsp(
            model,
            LspMsg::FormattingResolved {
                document_id,
                revision,
                edits,
                save,
            },
        )
    }

    fn has_cmd(cmd: &Option<Cmd>, pred: &dyn Fn(&Cmd) -> bool) -> bool {
        fn walk(cmd: &Cmd, pred: &dyn Fn(&Cmd) -> bool) -> bool {
            match cmd {
                Cmd::Batch(cmds) => cmds.iter().any(|c| walk(c, pred)),
                other => pred(other),
            }
        }
        cmd.as_ref().is_some_and(|c| walk(c, pred))
    }

    #[test]
    fn format_document_emits_the_request_with_options() {
        let (_dir, mut model) = model_with_file();
        let cmd = update_lsp(
            &mut model,
            LspMsg::FormatDocument {
                selection_only: false,
            },
        );
        let Some(Cmd::LspRequestFormatting {
            range,
            options,
            save,
            ..
        }) = cmd
        else {
            panic!("expected Cmd::LspRequestFormatting, got {cmd:?}");
        };
        assert!(range.is_none());
        assert!(save.is_none());
        assert_eq!(options.tab_size, 4);
        assert!(!options.insert_spaces);
    }

    #[test]
    fn format_selection_without_a_selection_sets_the_status() {
        let (_dir, mut model) = model_with_file();
        let cmd = update_lsp(
            &mut model,
            LspMsg::FormatDocument {
                selection_only: true,
            },
        );
        assert!(!matches!(cmd, Some(Cmd::LspRequestFormatting { .. })));
        assert!(model
            .ui
            .transient_message
            .as_ref()
            .is_some_and(|t| t.text == "No selection to format"));
    }

    #[test]
    fn format_selection_sends_the_selected_range() {
        let (_dir, mut model) = model_with_file();
        *model.editor_mut().active_selection_mut() =
            crate::model::editor::Selection::from_anchor_head(
                crate::model::editor::Position::new(0, 3),
                crate::model::editor::Position::new(0, 7),
            );
        let cmd = update_lsp(
            &mut model,
            LspMsg::FormatDocument {
                selection_only: true,
            },
        );
        let Some(Cmd::LspRequestFormatting { range: sent, .. }) = cmd else {
            panic!("expected Cmd::LspRequestFormatting, got {cmd:?}");
        };
        assert_eq!(sent, Some(range(0, 3, 7)));
    }

    #[test]
    fn formatting_resolved_applies_edits_as_one_undo_step() {
        let (_dir, mut model) = model_with_file(); // "fn main() {}\n"
        model.editor_mut().cursors[0] = crate::model::Cursor::at(0, 12);
        let revision = model.document().revision;
        let edits = vec![
            (range(0, 2, 3), "  ".to_owned()),
            (range(0, 10, 12), "{\n}".to_owned()),
        ];
        resolve_formatting(&mut model, revision, Some(edits), false);
        assert_eq!(model.document().buffer.to_string(), "fn  main() {\n}\n");
        assert_eq!(model.document().undo_stack.len(), 1);
        let cursor = model.editor().cursors[0];
        assert_eq!((cursor.line, cursor.column), (1, 1));
    }

    #[test]
    fn formatting_resolved_for_a_stale_revision_is_dropped() {
        let (_dir, mut model) = model_with_file();
        let revision = model.document().revision;
        resolve_formatting(
            &mut model,
            revision + 1,
            Some(vec![(range(0, 0, 2), "XX".to_owned())]),
            false,
        );
        assert_eq!(model.document().buffer.to_string(), "fn main() {}\n");
        assert!(model.document().undo_stack.is_empty());
    }

    #[test]
    fn formatting_resolved_with_no_edits_reports_already_formatted() {
        let (_dir, mut model) = model_with_file();
        let revision = model.document().revision;
        resolve_formatting(&mut model, revision, Some(vec![]), false);
        assert!(model
            .ui
            .transient_message
            .as_ref()
            .is_some_and(|t| t.text == "Already formatted"));
    }

    #[test]
    fn formatting_resolved_with_then_save_applies_then_saves() {
        let (_dir, mut model) = model_with_file();
        let revision = model.document().revision;
        let cmd = resolve_formatting(
            &mut model,
            revision,
            Some(vec![(range(0, 0, 2), "FN".to_owned())]),
            true,
        );
        assert!(has_cmd(&cmd, &|c| matches!(
            c,
            Cmd::SaveFile { content, .. } if content.chars().eq("FN main() {}\n".chars())
        )));
        assert!(model.ui.is_saving);
    }

    #[test]
    fn formatting_unavailable_with_then_save_still_saves() {
        let (_dir, mut model) = model_with_file();
        let revision = model.document().revision;
        let cmd = resolve_formatting(&mut model, revision, None, true);
        assert!(has_cmd(&cmd, &|c| matches!(c, Cmd::SaveFile { .. })));
        assert!(model
            .ui
            .transient_message
            .as_ref()
            .is_some_and(|t| t.text.contains("saved unformatted")));
    }

    // ---- signature help ----

    fn one_signature() -> crate::model::SignatureHelpState {
        crate::model::SignatureHelpState {
            signatures: vec![crate::model::SignatureView {
                label: "fn f(a: i32)".to_owned(),
                active_parameter_range: Some((5, 11)),
                doc: None,
                parameter_doc: None,
            }],
            active: 0,
        }
    }

    fn resolve_signature(
        model: &mut AppModel,
        revision: u64,
        help: Option<crate::model::SignatureHelpState>,
    ) {
        let document_id = model.document().id.unwrap();
        let cursor = model.editor().active_cursor().to_position();
        update_lsp(
            model,
            LspMsg::SignatureHelpResolved {
                document_id,
                revision,
                cursor,
                help,
            },
        );
    }

    #[test]
    fn signature_help_resolved_sets_the_state() {
        let (_dir, mut model) = model_with_file();
        let revision = model.document().revision;
        resolve_signature(&mut model, revision, Some(one_signature()));
        assert_eq!(model.ui.signature_help, Some(one_signature()));
    }

    #[test]
    fn signature_help_resolved_for_a_stale_revision_is_dropped() {
        let (_dir, mut model) = model_with_file();
        let revision = model.document().revision;
        resolve_signature(&mut model, revision + 1, Some(one_signature()));
        assert!(model.ui.signature_help.is_none());
    }

    #[test]
    fn signature_help_resolved_with_no_help_clears_the_state() {
        let (_dir, mut model) = model_with_file();
        let revision = model.document().revision;
        model.ui.signature_help = Some(one_signature());
        resolve_signature(&mut model, revision, None);
        assert!(model.ui.signature_help.is_none());
    }

    #[test]
    fn moving_the_caret_to_another_line_dismisses_signature_help() {
        use crate::messages::{Direction, Msg};
        let (_dir, mut model) = model_with_file();
        model.ui.signature_help = Some(one_signature());

        crate::update::update(&mut model, Msg::move_cursor(Direction::Right));
        assert!(
            model.ui.signature_help.is_some(),
            "moving along the line keeps it"
        );
        crate::update::update(&mut model, Msg::move_cursor(Direction::Down));
        assert!(model.ui.signature_help.is_none());
    }

    // ---- rename symbol ----

    /// A file whose caret sits inside `main` (line 0, col 5) and that names
    /// `main` twice, so a rename edit has two locations in one document.
    fn model_for_rename() -> (tempfile::TempDir, AppModel) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("main.rs");
        std::fs::write(&path, "fn main() {}\nfn other() { main() }\n").unwrap();
        let mut model = AppModel::with_document(
            800,
            600,
            1.0,
            crate::model::Document::from_file(path).unwrap(),
        );
        model.editor_mut().cursors[0] = crate::model::Cursor::at(0, 5);
        model.editor_mut().collapse_selections_to_cursors();
        (dir, model)
    }

    fn open_rename_modal(model: &mut AppModel, placeholder: &str) {
        let document_id = model.document().id.unwrap();
        let revision = model.document().revision;
        update_lsp(
            model,
            LspMsg::PrepareRenameResolved {
                document_id,
                revision,
                cursor: crate::model::editor::Position::new(0, 5),
                placeholder: Some(placeholder.to_owned()),
            },
        );
    }

    #[test]
    fn rename_symbol_requests_prepare_rename_with_the_caret_word_as_fallback() {
        let (_dir, mut model) = model_for_rename();
        let cmd = update_lsp(&mut model, LspMsg::RenameSymbol);
        let Some(Cmd::LspRequestPrepareRename {
            fallback, cursor, ..
        }) = cmd
        else {
            panic!("expected LspRequestPrepareRename, got {cmd:?}");
        };
        assert_eq!(fallback, "main");
        assert_eq!(cursor, crate::model::editor::Position::new(0, 5));
    }

    #[test]
    fn prepare_rename_resolved_opens_the_prompt_prefilled_and_selected() {
        let (_dir, mut model) = model_for_rename();
        open_rename_modal(&mut model, "main");
        let Some(crate::model::ModalState::RenameSymbol(state)) = &model.ui.active_modal else {
            panic!("expected the rename modal, got {:?}", model.ui.active_modal);
        };
        assert_eq!(state.input(), "main");
        assert!(state.editable.has_selection(), "placeholder is select-all");
    }

    #[test]
    fn prepare_rename_resolved_without_a_placeholder_flashes_cannot_rename() {
        let (_dir, mut model) = model_for_rename();
        let document_id = model.document().id.unwrap();
        let revision = model.document().revision;
        update_lsp(
            &mut model,
            LspMsg::PrepareRenameResolved {
                document_id,
                revision,
                cursor: crate::model::editor::Position::new(0, 5),
                placeholder: None,
            },
        );
        assert!(model.ui.active_modal.is_none());
        assert_eq!(
            model.ui.transient_message.as_ref().unwrap().text,
            "Cannot rename here"
        );
    }

    #[test]
    fn confirming_the_prompt_with_a_new_name_requests_the_rename() {
        use crate::messages::{ModalMsg, Msg, UiMsg};
        let (_dir, mut model) = model_for_rename();
        let revision = model.document().revision;
        open_rename_modal(&mut model, "main");
        crate::update::update(
            &mut model,
            Msg::Ui(UiMsg::Modal(ModalMsg::SetInput("start".to_owned()))),
        );

        let cmd = crate::update::update(&mut model, Msg::Ui(UiMsg::Modal(ModalMsg::Confirm)));

        assert!(model.ui.active_modal.is_none());
        let Some(Cmd::Batch(cmds)) = cmd else {
            panic!("expected a batch, got {cmd:?}");
        };
        assert!(cmds.iter().any(|c| matches!(
            c,
            Cmd::LspRequestRename { new_name, revision: r, position, .. }
                if new_name == "start" && *r == revision && position.character == 5
        )));
    }

    #[test]
    fn confirming_the_prompt_with_the_placeholder_just_closes() {
        use crate::messages::{ModalMsg, Msg, UiMsg};
        let (_dir, mut model) = model_for_rename();
        open_rename_modal(&mut model, "main");
        let cmd = crate::update::update(&mut model, Msg::Ui(UiMsg::Modal(ModalMsg::Confirm)));
        assert!(model.ui.active_modal.is_none());
        assert!(!matches!(cmd, Some(Cmd::Batch(_))));
    }

    fn two_location_edit(model: &AppModel) -> lsp_types::WorkspaceEdit {
        let uri = crate::lsp::path_to_uri(model.document().file_path.as_ref().unwrap());
        let edit = |line, start, end| lsp_types::TextEdit {
            range: lsp_types::Range::new(
                lsp_types::Position::new(line, start),
                lsp_types::Position::new(line, end),
            ),
            new_text: "start".to_owned(),
        };
        lsp_types::WorkspaceEdit::new(std::collections::HashMap::from([(
            uri,
            vec![edit(0, 3, 7), edit(1, 13, 17)],
        )]))
    }

    #[test]
    fn rename_resolved_applies_the_edit_in_one_undo_step_and_reports() {
        use crate::messages::{DocumentMsg, Msg};
        let (_dir, mut model) = model_for_rename();
        let document_id = model.document().id.unwrap();
        let revision = model.document().revision;
        let edit = two_location_edit(&model);

        let cmd = update_lsp(
            &mut model,
            LspMsg::RenameResolved {
                document_id,
                revision,
                edit: Some(Box::new(edit)),
            },
        );

        assert!(cmd.is_some());
        assert_eq!(
            model.document().buffer.to_string(),
            "fn start() {}\nfn other() { start() }\n"
        );
        assert_eq!(
            model.ui.transient_message.as_ref().unwrap().text,
            "Renamed in 1 file(s), 2 edit(s)"
        );
        assert_eq!(model.document().undo_stack.len(), 1);
        crate::update::update(&mut model, Msg::Document(DocumentMsg::Undo));
        assert_eq!(
            model.document().buffer.to_string(),
            "fn main() {}\nfn other() { main() }\n"
        );
    }

    #[test]
    fn rename_resolved_with_no_edit_flashes_nothing_to_rename() {
        let (_dir, mut model) = model_for_rename();
        let document_id = model.document().id.unwrap();
        let revision = model.document().revision;
        update_lsp(
            &mut model,
            LspMsg::RenameResolved {
                document_id,
                revision,
                edit: None,
            },
        );
        assert_eq!(
            model.ui.transient_message.as_ref().unwrap().text,
            "Nothing to rename"
        );
    }

    #[test]
    fn rename_resolved_for_a_stale_revision_is_dropped() {
        let (_dir, mut model) = model_for_rename();
        let document_id = model.document().id.unwrap();
        let revision = model.document().revision;
        let edit = two_location_edit(&model);
        let cmd = update_lsp(
            &mut model,
            LspMsg::RenameResolved {
                document_id,
                revision: revision + 1,
                edit: Some(Box::new(edit)),
            },
        );
        assert!(cmd.is_none());
        assert_eq!(
            model.document().buffer.to_string(),
            "fn main() {}\nfn other() { main() }\n"
        );
    }

    // ---- code actions ----

    fn diagnostic_on_line(line: u32) -> lsp_types::Diagnostic {
        lsp_types::Diagnostic {
            range: lsp_types::Range {
                start: lsp_types::Position { line, character: 0 },
                end: lsp_types::Position { line, character: 2 },
            },
            message: format!("line {line}"),
            ..Default::default()
        }
    }

    fn action(title: &str, is_preferred: bool) -> crate::model::CodeActionItem {
        crate::model::CodeActionItem {
            title: title.to_owned(),
            kind: Some("quickfix".to_owned()),
            is_preferred,
            edit: None,
            command: None,
        }
    }

    fn resolve_code_actions(
        model: &mut AppModel,
        revision: u64,
        actions: Vec<crate::model::CodeActionItem>,
    ) -> Option<Cmd> {
        let document_id = model.document().id.unwrap();
        let cursor = model.editor().active_cursor().to_position();
        update_lsp(
            model,
            LspMsg::CodeActionsResolved {
                document_id,
                revision,
                cursor,
                actions,
                outcome: ReferencesOutcome::Found,
            },
        )
    }

    #[test]
    fn show_code_actions_sends_only_the_diagnostics_overlapping_the_caret() {
        let (_dir, mut model) = model_with_file();
        model.document_mut().diagnostics = vec![diagnostic_on_line(0), diagnostic_on_line(1)];

        let cmd = update_lsp(&mut model, LspMsg::ShowCodeActions);

        let Some(Cmd::LspRequestCodeActions {
            range, diagnostics, ..
        }) = cmd
        else {
            panic!("expected LspRequestCodeActions");
        };
        assert_eq!(range.start, range.end, "caret only: an empty range");
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].message, "line 0");
    }

    #[test]
    fn code_actions_resolved_opens_the_popup_with_preferred_actions_first() {
        let (_dir, mut model) = model_with_file();
        let revision = model.document().revision;

        let cmd = resolve_code_actions(
            &mut model,
            revision,
            vec![action("Extract", false), action("Fix it", true)],
        );

        assert!(cmd.is_some());
        assert_eq!(
            model.ui.cursor_overlay.map(|o| o.kind),
            Some(CursorOverlayKind::CodeActions)
        );
        let titles: Vec<&str> = model
            .ui
            .code_action_list
            .as_ref()
            .unwrap()
            .iter()
            .map(|a| a.title.as_str())
            .collect();
        assert_eq!(titles, ["Fix it", "Extract"]);
    }

    #[test]
    fn code_actions_resolved_with_no_actions_sets_the_status() {
        let (_dir, mut model) = model_with_file();
        let revision = model.document().revision;
        resolve_code_actions(&mut model, revision, vec![]);
        assert!(model.ui.cursor_overlay.is_none());
        assert_eq!(
            model.ui.transient_message.as_ref().map(|t| t.text.as_str()),
            Some("No code actions")
        );
    }

    #[test]
    fn code_actions_resolved_for_a_stale_revision_is_dropped() {
        let (_dir, mut model) = model_with_file();
        let revision = model.document().revision;
        let cmd = resolve_code_actions(&mut model, revision + 1, vec![action("Fix", true)]);
        assert!(cmd.is_none());
        assert!(model.ui.cursor_overlay.is_none());
    }

    #[test]
    fn activating_a_code_action_with_an_edit_applies_it_and_closes_the_popup() {
        let (_dir, mut model) = model_with_file();
        let uri = crate::lsp::path_to_uri(model.document().file_path.as_deref().unwrap());
        let edit = lsp_types::WorkspaceEdit {
            changes: Some(std::collections::HashMap::from([(
                uri,
                vec![lsp_types::TextEdit {
                    range: lsp_types::Range {
                        start: lsp_types::Position {
                            line: 0,
                            character: 0,
                        },
                        end: lsp_types::Position {
                            line: 0,
                            character: 2,
                        },
                    },
                    new_text: "FN".to_owned(),
                }],
            )])),
            document_changes: None,
            change_annotations: None,
        };
        let mut item = action("Shout", true);
        item.edit = Some(Box::new(edit));
        model.ui.code_action_list = Some(vec![item]);
        model.ui.cursor_overlay = Some(CursorOverlayState::new(CursorOverlayKind::CodeActions));

        update_lsp(&mut model, LspMsg::ActivateCodeAction { index: 0 });

        assert_eq!(model.document().buffer.to_string(), "FN main() {}\n");
        assert!(model.ui.cursor_overlay.is_none());
        assert!(model.ui.code_action_list.is_none());
        assert_eq!(
            model.ui.transient_message.as_ref().map(|t| t.text.as_str()),
            Some("Applied: Shout")
        );
    }

    #[test]
    fn activating_a_code_action_with_a_command_emits_execute_command() {
        let (_dir, mut model) = model_with_file();
        let document_id = model.document().id.unwrap();
        let mut item = action("Run", false);
        item.command = Some(lsp_types::Command::new(
            "Run".to_owned(),
            "server.doIt".to_owned(),
            Some(vec![serde_json::json!(1)]),
        ));
        model.ui.code_action_list = Some(vec![item]);
        model.ui.cursor_overlay = Some(CursorOverlayState::new(CursorOverlayKind::CodeActions));

        let cmd = update_lsp(&mut model, LspMsg::ActivateCodeAction { index: 0 });

        let Some(Cmd::Batch(cmds)) = cmd else {
            panic!("expected a batch");
        };
        assert!(cmds.iter().any(|c| matches!(
            c,
            Cmd::LspExecuteCommand { document_id: id, command, arguments }
                if *id == document_id
                    && command == "server.doIt"
                    && *arguments == Some(vec![serde_json::json!(1)])
        )));
        assert!(model.ui.cursor_overlay.is_none());
    }
}
