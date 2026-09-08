//! Update functions for the Elm-style architecture
//!
//! All state transformations flow through these functions.
//!
//! Message handlers are internal: use `update(model, Msg)` so special-tab
//! routing and shared lifecycle/status synchronization run.
//! Runtime effect helpers and read-only view projections remain available.
//!
//! ```compile_fail
//! use token::update::update_document;
//! ```
//!
//! ```compile_fail
//! use token::update::layout::update_layout;
//! ```

mod app;
mod completion;
pub mod context_menu;
mod csv;
mod dock;
mod document;
mod editor;
mod file_change;
mod image;
pub mod inline;
mod layout;
mod lsp;
pub mod navigation;
pub(crate) mod outline;
mod preview;
pub mod problems;
mod settings;
mod syntax;
mod terminal;
pub(crate) mod text_edits;
mod ui;
pub mod usages;
mod workspace;
mod workspace_symbols;

use crate::commands::Cmd;
use crate::messages::{CsvMsg, Direction, DocumentMsg, EditorMsg, Msg};
use crate::model::sync_status_bar;
use crate::model::AppModel;
use crate::util::text::{char_type, CharType};

#[cfg(debug_assertions)]
use crate::tracing::CursorSnapshot;
#[cfg(debug_assertions)]
use tracing::{debug, span, Level};

pub use app::execute_command;
use lsp::{close_lsp_document, open_lsp_document, schedule_lsp_did_change};
use syntax::schedule_syntax_parse;
pub use ui::{resolve_palette_rows, search_everywhere_sections, ALL_TAB_GROUP_CAP};

/// Drive text-file effects explicitly in unit fixtures. Production updates never
/// call this; real validation, aliases, image/binary loading and worker ordering
/// are exercised by runtime tests. Preserve generated effects for assertions.
#[cfg(test)]
pub(crate) fn finish_test_file_opens(model: &mut AppModel, cmd: Option<Cmd>) -> Option<Cmd> {
    match cmd? {
        Cmd::PrepareFileOpen(request) => {
            let path = request
                .source
                .path()
                .expect("text fixture requires a resolved path")
                .to_path_buf();
            let result = match crate::model::Document::from_file(path.clone()) {
                Ok(document) => Ok(document),
                Err(error)
                    if error.kind() == std::io::ErrorKind::NotFound
                        && request.policy == crate::model::FileOpenPolicy::CreateOrOpen =>
                {
                    Ok(crate::model::Document::new_with_path(path.clone()))
                }
                Err(error) => Err(error.to_string()),
            }
            .map(|document| {
                Box::new(crate::model::PreparedFile::Loaded {
                    document: Box::new(document),
                    view_mode: crate::model::ViewMode::Text,
                    tab_content: crate::model::TabContent::Text,
                })
            });
            update(
                model,
                Msg::Layout(crate::messages::LayoutMsg::FilePrepared { request, result }),
            )
        }
        Cmd::Batch(commands) => {
            let mut effects = Vec::new();
            for command in commands {
                match finish_test_file_opens(model, Some(command)) {
                    Some(Cmd::Batch(commands)) => effects.extend(commands),
                    Some(command) => effects.push(command),
                    None => {}
                }
            }
            Some(Cmd::Batch(effects))
        }
        command => Some(command),
    }
}

/// Main update function - dispatches to sub-handlers
///
/// In debug builds, this wraps with tracing instrumentation.
/// In release builds, it's a direct dispatch with zero overhead.
#[inline]
pub fn update(model: &mut AppModel, msg: Msg) -> Option<Cmd> {
    let was_loading = model.ui.is_loading;
    let had_path_completion = model.ui.completion_path.is_some();
    #[cfg(debug_assertions)]
    let result = update_traced(model, msg);
    #[cfg(not(debug_assertions))]
    let result = update_inner(model, msg);
    let result = merge_cmds(result, usages::reconcile(model));
    let result = merge_cmds(result, file_change::reconcile(model));
    // This wrapper also covers early returns in special-tab dispatch.
    let completion_cleanup = completion::reconcile_pending_commit(model);
    let path_cleanup = completion::reconcile_paths(model);
    let inline_cleanup = inline::reconcile(model);
    let projection_change = inline::sync_projection(model);
    // Closed groups cannot retain obsolete activation choices.
    let area = &mut model.editor_area;
    // Retain closed-group tokens until the worker replies, so runtime waiters
    // receive an explicit rejected completion instead of waiting forever.
    area.file_opens
        .latest
        .retain(|group, _| area.groups.contains_key(group));
    model.ui.is_loading = !area.file_opens.pending.is_empty()
        || area
            .documents
            .values()
            .any(|doc| doc.file_io.pending(crate::model::FileRequestKind::Read));
    if inline_cleanup.is_some() {
        sync_status_bar(model);
    }
    let result = merge_cmds(result, completion_cleanup);
    let result = merge_cmds(result, path_cleanup);
    let result = if had_path_completion && model.ui.completion_path.is_none() {
        merge_cmds(result, Some(Cmd::CancelPathCompletion))
    } else {
        result
    };
    let result = merge_cmds(merge_cmds(result, inline_cleanup), projection_change);
    let find_search = ui::schedule_find_search(model);
    let result = merge_cmds(result, find_search);
    let result = merge_cmds(result, workspace_symbols::reconcile(model));
    if was_loading != model.ui.is_loading && result.as_ref().is_none_or(|cmd| !cmd.needs_redraw()) {
        merge_cmds(result, Some(Cmd::redraw_status_bar()))
    } else {
        result
    }
}

/// Inner update logic (no tracing)
fn update_inner(model: &mut AppModel, msg: Msg) -> Option<Cmd> {
    let inline_progress_before = (model.ui.inline_in_flight, model.ui.cursor_visible);
    model.editor_area.refresh_wrap_caches();
    // Signature help's dismissal anchor: it survives edits and moves
    // along its line, but not the caret leaving that line, a tab switch,
    // or focus leaving the editor — compared once at the bottom for every
    // message instead of chasing each cursor/tab path.
    let signature_anchor = model
        .ui
        .signature_help
        .is_some()
        .then(|| signature_help_anchor(model));
    let result = match msg {
        Msg::Editor(m) => {
            // Any cursor/selection movement invalidates the completion
            // query (autocomplete.md dismiss rule: "cursor line change").
            // Up/Down/Enter/Tab/Escape never reach here while the menu is
            // open — they're claimed by the pre-keymap cursor-overlay
            // branch in runtime/input.rs before an `EditorMsg` is ever
            // built, so this only fires for keys that should dismiss. Most
            // editor messages already redraw, but some (e.g. a no-op
            // cursor move) return `None`, so the dismiss needs its own
            // redraw merged in or the now-stale popup stays on screen.
            let was_dismissed = completion::dismiss(model);
            let redraw_for_dismiss = was_dismissed.then(Cmd::redraw_editor);

            // Block editor messages in image mode and binary placeholder mode
            let is_non_text = model.editor_area.focused_editor().is_some_and(|e| {
                e.view_mode.is_image()
                    || matches!(
                        e.tab_content,
                        crate::model::editor::TabContent::BinaryPlaceholder(_)
                    )
            });
            if is_non_text {
                return redraw_for_dismiss;
            }

            // When in CSV mode, intercept navigation messages and route to CSV
            let csv_info = model
                .editor_area
                .focused_editor()
                .and_then(|e| e.view_mode.as_csv().map(|csv| (true, csv.is_editing())));

            if let Some((true, is_editing)) = csv_info {
                if let Some(csv_msg) = map_editor_to_csv(&m, is_editing) {
                    return merge_cmds(csv::update_csv(model, csv_msg), redraw_for_dismiss);
                }
                return redraw_for_dismiss;
            }
            merge_cmds(editor::update_editor(model, m), redraw_for_dismiss)
        }
        Msg::Document(m) => {
            // Block document messages in image mode and binary placeholder mode
            let is_non_text = model.editor_area.focused_editor().is_some_and(|e| {
                e.view_mode.is_image()
                    || matches!(
                        e.tab_content,
                        crate::model::editor::TabContent::BinaryPlaceholder(_)
                    )
            });
            if is_non_text {
                return None;
            }

            // When in CSV mode, intercept document messages for cell editing
            let csv_info = model
                .editor_area
                .focused_editor()
                .and_then(|e| e.view_mode.as_csv().map(|csv| (true, csv.is_editing())));

            if let Some((true, is_editing)) = csv_info {
                if let Some(csv_msg) = map_document_to_csv(&m, is_editing) {
                    return csv::update_csv(model, csv_msg);
                }
                return None;
            }
            // Only these facts of `m` matter to the completion sync
            // below; read them before `m` moves into `update_document`
            // rather than cloning the whole message (cheap for most
            // variants, but `InsertText(String)` carries a full paste/IME
            // payload that a clone would copy for nothing).
            let is_copy = matches!(m, DocumentMsg::Copy);
            // Only physical character messages commit dropdown items. Paste and
            // IME/automation text insertion must not accept a highlighted row.
            let pending_cancel = if is_copy {
                None
            } else {
                completion::cancel_pending_commit(model)
            };
            let committed = match &m {
                DocumentMsg::InsertChar(character) => {
                    completion::try_commit_character(model, *character)
                }
                _ => None,
            };
            if let Some(committed) = committed {
                // Stay in the shared update finalization: status, wrap caches
                // and signature dismissal must see the accepted document too.
                merge_cmds(pending_cancel, Some(committed))
            } else {
                let opens_on_word_char = matches!(&m, DocumentMsg::InsertChar(ch) if char_type(*ch) == CharType::WordChar);
                // A one-char `InsertText` (IME commit, automation `text`) is
                // typing too, for the trigger-character and ghost-text paths.
                let typed_char = match &m {
                    DocumentMsg::InsertChar(ch) => Some(*ch),
                    DocumentMsg::InsertText(text) if text.chars().count() == 1 => {
                        text.chars().next()
                    }
                    _ => None,
                };
                let backspaced = matches!(m, DocumentMsg::DeleteBackward);
                let result = document::update_document(model, m);
                // Reconcile typed-through ghost text before auto-opening a menu.
                // In the middle of an identifier, fallback words can otherwise hide
                // the compatible remainder before it consumes the typed character.
                let inline_cmd = if is_copy {
                    None
                } else {
                    inline::after_document_edit(model, typed_char, backspaced)
                };
                let completion_cmd = completion::sync_after_document_edit(
                    model,
                    is_copy,
                    opens_on_word_char,
                    typed_char,
                );
                merge_cmds(
                    pending_cancel,
                    merge_cmds(merge_cmds(result, completion_cmd), inline_cmd),
                )
            }
        }
        Msg::Ui(m) => ui::update_ui(model, m),
        Msg::Layout(m) => layout::update_layout(model, m),
        Msg::App(m) => app::update_app(model, m),
        Msg::Syntax(m) => syntax::update_syntax(model, m),
        Msg::Csv(m) => csv::update_csv(model, m),
        Msg::Image(m) => image::update_image(model, m),
        Msg::Preview(m) => preview::update_preview(model, m),
        Msg::Workspace(m) => workspace::update_workspace(model, m),
        Msg::Dock(m) => dock::update_dock(model, m),
        Msg::Outline(m) => outline::update_outline(model, m),
        Msg::Problems(m) => problems::update_problems(model, m),
        Msg::Usages(m) => usages::update_usages(model, m),
        Msg::Terminal(m) => terminal::update_terminal(model, m),
        Msg::Completion(m) => completion::update_completion(model, m),
        Msg::Lsp(m) => lsp::update_lsp(model, m),
        Msg::ContextMenu(m) => context_menu::update_context_menu(model, m),
    };

    // Any focus change away from the editor (modal open, dock/sidebar
    // focus, ...) must dismiss the completion popup: `cursor_overlay` is
    // claimed pre-keymap purely on `.is_some()` regardless of focus
    // (runtime/app.rs), so a still-open menu would hijack Up/Down/Enter/Tab
    // meant for whatever now actually has focus. This single check covers
    // every message that can move focus (`Msg::Ui` modals, `Msg::Dock`,
    // `Msg::App`, ...) instead of chasing each one individually.
    let result =
        if model.ui.focus != crate::model::FocusTarget::Editor && completion::dismiss(model) {
            merge_cmds(result, Some(Cmd::redraw_editor()))
        } else {
            result
        };
    let result = match signature_anchor {
        Some(before)
            if model.ui.signature_help.is_some() && signature_help_anchor(model) != before =>
        {
            model.ui.signature_help = None;
            merge_cmds(result, Some(Cmd::redraw_editor()))
        }
        _ => result,
    };

    model.editor_area.refresh_wrap_caches();
    sync_status_bar(model);
    if inline_progress_before.0 != model.ui.inline_in_flight
        || (model.ui.inline_in_flight && inline_progress_before.1 != model.ui.cursor_visible)
    {
        merge_cmds(result, Some(Cmd::redraw_status_bar()))
    } else {
        result
    }
}

/// `(focus, focused document, caret line)` — signature help closes when
/// any of these change.
fn signature_help_anchor(
    model: &AppModel,
) -> (
    crate::model::FocusTarget,
    Option<crate::model::editor_area::DocumentId>,
    Option<usize>,
) {
    (
        model.ui.focus,
        model.try_document().and_then(|d| d.id),
        model
            .editor_area
            .focused_editor()
            .map(|e| e.active_cursor().line),
    )
}

/// Combine two independently-produced `Cmd`s into one, batching when both
/// are present. Used where a message can both perform its own effect (e.g.
/// document edit + redraw/syntax-parse) and trigger a side effect elsewhere
/// (e.g. the completion menu opening/closing) that also wants to redraw.
fn merge_cmds(a: Option<Cmd>, b: Option<Cmd>) -> Option<Cmd> {
    match (a, b) {
        (Some(a), Some(b)) => Some(Cmd::Batch(vec![a, b])),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
}

/// Map text editor movement messages to CSV navigation messages
///
/// When not editing: arrows move cell selection
/// When editing: left/right move cursor in cell, up/down confirm and navigate
fn map_editor_to_csv(editor_msg: &EditorMsg, is_editing: bool) -> Option<CsvMsg> {
    match (editor_msg, is_editing) {
        // When editing, left/right move cursor within cell
        (EditorMsg::MoveCursor(Direction::Left), true) => Some(CsvMsg::EditCursorLeft),
        (EditorMsg::MoveCursor(Direction::Right), true) => Some(CsvMsg::EditCursorRight),
        // When editing, up/down confirm edit and navigate
        (EditorMsg::MoveCursor(Direction::Up), true) => Some(CsvMsg::ConfirmEditUp),
        (EditorMsg::MoveCursor(Direction::Down), true) => Some(CsvMsg::ConfirmEdit),
        // When editing, Home/End move cursor within cell
        (EditorMsg::MoveCursorLineStart, true) => Some(CsvMsg::EditCursorHome),
        (EditorMsg::MoveCursorLineEnd, true) => Some(CsvMsg::EditCursorEnd),

        // When not editing, standard cell navigation
        (EditorMsg::MoveCursor(Direction::Up), false) => Some(CsvMsg::MoveUp),
        (EditorMsg::MoveCursor(Direction::Down), false) => Some(CsvMsg::MoveDown),
        (EditorMsg::MoveCursor(Direction::Left), false) => Some(CsvMsg::MoveLeft),
        (EditorMsg::MoveCursor(Direction::Right), false) => Some(CsvMsg::MoveRight),
        (EditorMsg::MoveCursorLineStart, false) => Some(CsvMsg::RowStart),
        (EditorMsg::MoveCursorLineEnd, false) => Some(CsvMsg::RowEnd),
        (EditorMsg::MoveCursorDocumentStart, _) => Some(CsvMsg::FirstCell),
        (EditorMsg::MoveCursorDocumentEnd, _) => Some(CsvMsg::LastCell),
        (EditorMsg::PageUp, _) => Some(CsvMsg::PageUp),
        (EditorMsg::PageDown, _) => Some(CsvMsg::PageDown),
        _ => None,
    }
}

/// Map document messages to CSV cell editing messages
///
/// When not editing: InsertNewline starts editing, InsertChar starts with that char
/// When editing: InsertNewline confirms edit, InsertChar inserts into buffer
fn map_document_to_csv(doc_msg: &DocumentMsg, is_editing: bool) -> Option<CsvMsg> {
    match (doc_msg, is_editing) {
        (DocumentMsg::InsertNewline, false) => Some(CsvMsg::StartEditing),
        (DocumentMsg::InsertNewline, true) => Some(CsvMsg::ConfirmEdit),
        (DocumentMsg::InsertChar(ch), false) => Some(CsvMsg::StartEditingWithChar(*ch)),
        (DocumentMsg::InsertChar(ch), true) => Some(CsvMsg::EditInsertChar(*ch)),
        (DocumentMsg::DeleteBackward, true) => Some(CsvMsg::EditDeleteBackward),
        (DocumentMsg::DeleteForward, true) => Some(CsvMsg::EditDeleteForward),
        _ => None,
    }
}

/// Traced update wrapper (debug builds only)
///
/// Captures before/after cursor state and logs diffs for debugging.
/// Filters out noisy messages like BlinkCursor from logging.
#[cfg(debug_assertions)]
fn update_traced(model: &mut AppModel, msg: Msg) -> Option<Cmd> {
    use crate::messages::UiMsg;

    // Skip logging for noisy periodic messages
    let is_noisy = matches!(&msg, Msg::Ui(UiMsg::BlinkCursor));

    let msg_name = msg_type_name(&msg);
    let _span = if is_noisy {
        None
    } else {
        Some(span!(Level::DEBUG, "update", msg = %msg_name).entered())
    };

    let before = model.focused_editor().map(CursorSnapshot::from_editor);

    if !is_noisy {
        debug!(target: "message", msg = %msg_name, "processing");
    }

    let result = update_inner(model, msg);

    let diff = if let (Some(ref before), Some(editor)) = (&before, model.focused_editor()) {
        let after = CursorSnapshot::from_editor(editor);
        let d = before.diff(&after);
        if let Some(ref diff) = d {
            debug!(target: "cursor", %diff, "state changed");
        }
        d
    } else {
        None
    };

    if let Some(editor) = model.focused_editor() {
        editor.assert_invariants_with_context(&msg_name);
    }

    if !is_noisy {
        if let Some(ref mut overlay) = model.debug_overlay {
            overlay.record_message(msg_name.clone(), diff);
        }
    }

    result
}

/// Get a display name for a message type
///
/// Uses Debug formatting to include variant names and arguments.
/// Example outputs:
/// - `Editor::MoveCursor(Up)`
/// - `Document::InsertChar('x')`
/// - `App::Resize(1920, 1080)`
#[cfg(debug_assertions)]
fn msg_type_name(msg: &Msg) -> String {
    match msg {
        Msg::Editor(m) => format!("Editor::{:?}", m),
        Msg::Document(m) => format!("Document::{:?}", m),
        Msg::Ui(m) => format!("Ui::{:?}", m),
        Msg::Layout(m) => format!("Layout::{:?}", m),
        Msg::App(crate::messages::AppMsg::SaveCompleted { target, result, .. }) => format!(
            "App::SaveCompleted(document={:?}, revision={}, success={})",
            target.document_id,
            target.revision,
            result.is_ok()
        ),
        Msg::App(crate::messages::AppMsg::FileLoaded { target, result, .. }) => format!(
            "App::FileLoaded(document={:?}, revision={}, success={})",
            target.document_id,
            target.revision,
            result.is_ok()
        ),
        Msg::App(m) => format!("App::{:?}", m),
        Msg::Syntax(m) => format!("Syntax::{:?}", m),
        Msg::Csv(m) => format!("Csv::{:?}", m),
        Msg::Image(m) => format!("Image::{:?}", m),
        Msg::Preview(m) => format!("Preview::{:?}", m),
        Msg::Workspace(m) => format!("Workspace::{:?}", m),
        Msg::Dock(m) => format!("Dock::{:?}", m),
        Msg::Outline(m) => format!("Outline::{:?}", m),
        Msg::Problems(m) => format!("Problems::{:?}", m),
        Msg::Usages(m) => format!("Usages::{:?}", m),
        Msg::Terminal(m) => format!("Terminal::{:?}", m),
        Msg::Completion(crate::messages::CompletionMsg::InlineContextReady { job, .. }) => {
            format!(
                "Completion::InlineContextReady(request={})",
                job.request.snapshot.request_id
            )
        }
        Msg::Completion(m) => format!("Completion::{:?}", m),
        Msg::Lsp(crate::messages::LspMsg::CompletionResolved {
            document_id, revision, items, is_incomplete,
        }) => format!("Lsp::CompletionResolved(document={document_id:?}, revision={revision}, items={}, incomplete={is_incomplete})", items.len()),
        Msg::Lsp(m) => format!("Lsp::{:?}", m),
        Msg::ContextMenu(m) => format!("ContextMenu::{:?}", m),
    }
}

#[cfg(all(test, debug_assertions))]
#[test]
fn async_reply_trace_names_exclude_source_payloads() {
    use crate::messages::AppMsg;
    use crate::model::{Document, DocumentId, FileRequestKind};
    let mut doc = Document::with_text("do-not-log-file-contents");
    doc.id = Some(DocumentId(1));
    let target = doc.begin_file_request(FileRequestKind::Write).unwrap();
    let save = Msg::App(AppMsg::SaveCompleted {
        identity: None,
        target,
        path: "/fixture/a.txt".into(),
        content: doc.buffer.clone(),
        result: Ok(()),
    });
    let target = doc.begin_file_request(FileRequestKind::Read).unwrap();
    let load = Msg::App(AppMsg::FileLoaded {
        identity: None,
        target,
        path: "/fixture/a.txt".into(),
        result: Ok(doc.buffer.to_string()),
    });
    for message in [save, load] {
        let name = msg_type_name(&message);
        assert!(name.contains("success=true"));
        assert!(!name.contains("do-not-log-file-contents"));
        assert!(name.chars().count() < 128);
    }
    let items = crate::completion::lsp::items_to_menu_items(
        vec![lsp_types::CompletionItem {
            label: "do-not-log-completion-contents".into(),
            ..Default::default()
        }],
        &crate::lsp::LspServerId::from("fixture"),
        std::path::Path::new("/fixture"),
        None,
    );
    let name = msg_type_name(&Msg::Lsp(crate::messages::LspMsg::CompletionResolved {
        document_id: DocumentId(1),
        revision: 0,
        items,
        is_incomplete: false,
    }));
    assert!(name.contains("items=1"));
    assert!(!name.contains("do-not-log-completion-contents"));
    assert!(name.chars().count() < 128);
}
