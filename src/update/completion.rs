//! Menu completion update logic (autocomplete.md Phase 1;
//! lsp-integration.md Phase 5 adds the LSP source).
//!
//! Entry points:
//! - [`update_completion`] handles `Msg::Completion` (explicit trigger,
//!   navigation, accept, dismiss).
//! - [`sync_after_document_edit`] is called after every text-mutating
//!   `DocumentMsg` to open/refresh/dismiss the menu as the user types.
//! - [`merge_lsp_completion`] folds an async `textDocument/completion`
//!   response into the open menu (items already converted runtime-side).
//! - [`finish_deferred_accept`] applies an accept that was blocked on its
//!   `completionItem/resolve` round trip.
//!
//! Words/snippets stay synchronous sub-millisecond rope scans collected
//! inline (autocomplete.md: "no worker and no debounce for v1 menu
//! sources"). LSP items are asynchronous: the request is debounced
//! runtime-side (`COMPLETION_DEBOUNCE`), responses are revision-guarded,
//! and between round trips the previous LSP items ride along — refiltered
//! locally on every keystroke, replaced wholesale when a fresh response
//! lands.

use crate::commands::Cmd;
use crate::completion::menu::{
    filter_and_sort, CompletionMenuState, LspInsert, MenuInsert, MenuSourceId,
};
use crate::completion::sources::{collect_snippets, collect_words};
use crate::config::WordsMode;
use crate::lsp::lsp_server_def;
use crate::messages::CompletionMsg;
use crate::model::{
    AppModel, Cursor, CursorOverlayKind, CursorOverlayState, EditOperation, Selection,
};
use crate::util::text::{char_type, CharType};
use crate::view::overlay_surface::{SelectableListViewport, MAX_VISIBLE_COMPLETION};

use super::document::word_start_before;
use super::editor::cursors_in_reverse_order;
use super::lsp::schedule_lsp_did_change;
use super::schedule_syntax_parse;

/// Word chars a user must have typed before the menu auto-opens. A single
/// char opening the popup read as noise (every prose word flashed it);
/// Ctrl+Space is unaffected and still works on an empty query.
const MIN_AUTO_TRIGGER_PREFIX: usize = 2;

pub fn update_completion(model: &mut AppModel, msg: CompletionMsg) -> Option<Cmd> {
    match msg {
        CompletionMsg::TriggerMenu => trigger_explicit(model),
        CompletionMsg::MenuNext => move_selection(model, 1),
        CompletionMsg::MenuPrev => move_selection(model, -1),
        CompletionMsg::MenuPageUp => move_selection(model, -(MAX_VISIBLE_COMPLETION as i32)),
        CompletionMsg::MenuPageDown => move_selection(model, MAX_VISIBLE_COMPLETION as i32),
        CompletionMsg::AcceptMenuItem => accept_selected(model),
        CompletionMsg::Dismiss => dismiss_with_cleanup(model),
    }
}

/// Close the popup and drop its state, if open. A no-op if it's already
/// closed (every call site can call this unconditionally). Returns whether
/// anything was actually open, so callers that don't already redraw for
/// other reasons can decide whether a redraw is needed.
pub(crate) fn dismiss(model: &mut AppModel) -> bool {
    let mut was_open = false;
    if model.ui.completion_menu.is_some() {
        model.ui.completion_menu = None;
        was_open = true;
    }
    model.ui.completion_hover_row = None;
    if matches!(
        model.ui.cursor_overlay,
        Some(CursorOverlayState {
            kind: CursorOverlayKind::Completion,
            ..
        })
    ) {
        model.ui.cursor_overlay = None;
        was_open = true;
    }
    was_open
}

/// The `LspCancelCompletion` for a menu about to close — captured BEFORE
/// `dismiss` clears the state. `None` when the document could never have
/// armed runtime completion work (unsynced buffer, LSP disabled).
fn pending_cancel_cmd(model: &AppModel) -> Option<Cmd> {
    let document_id = model.ui.completion_menu.as_ref()?.document_id;
    lsp_capable(model).then_some(Cmd::LspCancelCompletion { document_id })
}

/// `dismiss` plus cancellation of any runtime completion work scheduled
/// for the menu's document (pending debounce / in-flight request). The
/// revision guards make a late response harmless anyway; this just avoids
/// firing a request whose answer would be dropped.
fn dismiss_with_cleanup(model: &mut AppModel) -> Option<Cmd> {
    let cancel = pending_cancel_cmd(model);
    if !dismiss(model) {
        return None;
    }
    Some(match cancel {
        Some(cancel) => Cmd::Batch(vec![Cmd::Redraw, cancel]),
        None => Cmd::Redraw,
    })
}

fn batch_redraw(mut cmds: Vec<Cmd>) -> Cmd {
    if cmds.len() == 1 {
        cmds.swap_remove(0)
    } else {
        Cmd::Batch(cmds)
    }
}

/// The focused document's server completion trigger characters, from the
/// model mirror (`LspMsg::ServerCompletionTriggers`). Empty when the
/// language has no registered server or the server advertised none.
fn trigger_characters_for(model: &AppModel) -> Vec<String> {
    let language = model.document().language;
    lsp_server_def(language)
        .map(|def| def.id)
        .and_then(|id| {
            model
                .lsp
                .completion_trigger_characters
                .get(&crate::lsp::LspServerId::from(id))
        })
        .cloned()
        .unwrap_or_default()
}

/// Whether the focused document could ever carry LSP completion traffic —
/// gates emitting schedule/cancel commands so unsynced buffers never arm
/// runtime bookkeeping. The runtime re-gates everything (open_documents,
/// capability snapshots) when the command lands; this is just the cheap
/// pre-filter that keeps the hot typing path allocation-free for files
/// with no server.
fn lsp_capable(model: &AppModel) -> bool {
    let doc = model.document();
    if doc.file_path.is_none() || !model.config.lsp.enabled {
        return false;
    }
    match lsp_server_def(doc.language) {
        Some(def) => !model
            .config
            .lsp
            .servers
            .get(def.id)
            .is_some_and(|o| o.enabled == Some(false)),
        None => false,
    }
}

/// The word-prefix query at `cursor`: `(query_start_offset, cursor_offset)`,
/// or `None` when the char immediately before `cursor` isn't a word char
/// (nothing to complete against).
fn word_query_offsets(model: &AppModel, cursor: Cursor) -> Option<(usize, usize)> {
    let doc = model.document();
    let offset = doc.cursor_to_offset(cursor.line, cursor.column);
    if offset == 0 || char_type(doc.buffer.char(offset - 1)) != CharType::WordChar {
        return None;
    }
    Some((word_start_before(&doc.buffer, offset), offset))
}

fn menu_enabled(model: &AppModel) -> bool {
    model.config.completion.enabled
}

/// `CommandId::ToggleAutocomplete`: flips `completion.enabled`, persists
/// it, and closes any open menu.
pub fn toggle_enabled(model: &mut AppModel) -> Option<Cmd> {
    let enabled = !model.config.completion.enabled;
    model.config.completion.enabled = enabled;
    if let Err(e) = model.config.save() {
        tracing::warn!("Failed to save autocomplete toggle: {}", e);
    }
    model.ui.set_status(if enabled {
        "Autocomplete enabled"
    } else {
        "Autocomplete disabled"
    });
    let mut cmds = vec![Cmd::redraw_status_bar()];
    if let Some(cancel) = dismiss_with_cleanup(model) {
        cmds.push(cancel);
    }
    Some(Cmd::Batch(cmds))
}

/// Ctrl+Space (or any other explicit-trigger binding): open with whatever
/// query is at the cursor, including an empty one (word chars aren't
/// required — autocomplete.md: "Ctrl+Space always works").
fn trigger_explicit(model: &mut AppModel) -> Option<Cmd> {
    if !menu_enabled(model) {
        model.ui.set_status("Autocomplete disabled");
        return Some(Cmd::redraw_status_bar());
    }
    if !model.editor().is_plain_text_mode() {
        // Previously a silent no-op — an explicit keypress that visibly
        // did nothing read as a broken binding.
        model
            .ui
            .set_status("Completion unavailable for this file type");
        return Some(Cmd::redraw_status_bar());
    }
    let cursor = *model.editor().active_cursor();
    let doc = model.document();
    let offset = doc.cursor_to_offset(cursor.line, cursor.column);
    let query_start_offset = word_query_offsets(model, cursor)
        .map(|(start, _)| start)
        .unwrap_or(offset);
    let mut cmds = vec![Cmd::Redraw];
    if let Some(schedule) = open_or_refresh(model, cursor, query_start_offset, offset, None) {
        cmds.push(schedule);
    }
    Some(batch_redraw(cmds))
}

/// Called after every text-mutating `DocumentMsg`. Opens the menu once
/// [`MIN_AUTO_TRIGGER_PREFIX`] word chars are typed, refreshes it while
/// already open, keeps it open across a server trigger character (`.`),
/// and dismisses it once the cursor is no longer preceded by a word char.
///
/// Takes the facts about the originating message the sync needs, rather
/// than the message itself — the caller reads them before the message is
/// moved into `update_document`, avoiding a clone of the whole
/// `DocumentMsg` (which, for `InsertText`, would copy a full paste/IME
/// payload) just to inspect its shape here.
pub(crate) fn sync_after_document_edit(
    model: &mut AppModel,
    is_copy: bool,
    opens_on_word_char: bool,
    typed_char: Option<char>,
) -> Option<Cmd> {
    if is_copy {
        return None;
    }
    if !menu_enabled(model) {
        return dismiss_with_cleanup(model);
    }
    if !model.editor().is_plain_text_mode() {
        dismiss(model);
        return None;
    }

    let cursor = *model.editor().active_cursor();
    let trigger_characters = trigger_characters_for(model);
    let on_trigger_char =
        typed_char.is_some_and(|ch| trigger_characters.iter().any(|t| t == &ch.to_string()));

    let Some((query_start_offset, cursor_offset)) = word_query_offsets(model, cursor) else {
        if on_trigger_char {
            // A server trigger character keeps/reopens the menu: the query
            // restarts empty at the cursor and the re-request goes out
            // tagged with the character (lsp-integration.md Phase 5:
            // "on server trigger characters while typing").
            let offset = model
                .document()
                .cursor_to_offset(cursor.line, cursor.column);
            let mut cmds = vec![Cmd::Redraw];
            if let Some(schedule) = open_or_refresh(model, cursor, offset, offset, typed_char) {
                cmds.push(schedule);
            }
            return Some(batch_redraw(cmds));
        }
        return dismiss_with_cleanup(model);
    };

    let menu_open = model.ui.completion_menu.is_some();
    let query_len = cursor_offset - query_start_offset;
    // Refreshing an open menu runs at any query length; only *opening*
    // requires the minimum prefix.
    let should_open = menu_open || (opens_on_word_char && query_len >= MIN_AUTO_TRIGGER_PREFIX);
    if !should_open {
        return None;
    }
    let mut cmds = vec![Cmd::Redraw];
    if let Some(schedule) = open_or_refresh(model, cursor, query_start_offset, cursor_offset, None)
    {
        cmds.push(schedule);
    }
    Some(batch_redraw(cmds))
}

/// Collect words + snippets (+ carried LSP items), filter/sort against the
/// query, and either set (or replace) `completion_menu`/`cursor_overlay`,
/// or dismiss if nothing matched. Returns the debounced LSP request to
/// schedule, when the document could have completion traffic.
fn open_or_refresh(
    model: &mut AppModel,
    cursor: Cursor,
    query_start_offset: usize,
    cursor_offset: usize,
    trigger_character: Option<char>,
) -> Option<Cmd> {
    let doc = model.document();
    let query: String = doc
        .buffer
        .slice(query_start_offset..cursor_offset)
        .chars()
        .collect();
    let document_id = doc.id;
    let position = crate::lsp::position_to_lsp(doc, cursor.to_position());

    // Carry the previous menu's LSP items while the query grows or shrinks
    // along the same word: they're refiltered against the new query below,
    // giving instant local feedback between debounced server round trips.
    // A divergent edit (typed mid-word) invalidates them — the server's
    // list was computed for a different prefix. A trigger character
    // restarts the query empty at a new start, and every string starts
    // with "": without this gate `st.` would list the `st*` items as
    // members until the server replied.
    // The previous menu is replaced wholesale below, so its LSP items are
    // moved out rather than cloned — the carry is allocation-free.
    let (carried_items, carried_incomplete) = model
        .ui
        .completion_menu
        .take_if(|prev| {
            Some(prev.document_id) == document_id
                && trigger_character.is_none()
                && (query.starts_with(prev.query.as_str()) || prev.query.starts_with(&query))
        })
        .map(|prev| {
            let mut items = prev.items;
            items.retain(|item| item.source == MenuSourceId::Lsp);
            (items, prev.is_incomplete)
        })
        .unwrap_or((Vec::new(), false));

    let doc = model.document();
    let mut items = match model.config.completion.words {
        WordsMode::Enabled => collect_words(doc, cursor, &query),
        WordsMode::Fallback if carried_items.is_empty() => collect_words(doc, cursor, &query),
        WordsMode::Fallback | WordsMode::Disabled => Vec::new(),
    };
    items.extend(collect_snippets(doc.language));
    items.extend(carried_items);
    let filtered = filter_and_sort(&items, &query);

    if filtered.is_empty() {
        dismiss(model);
        return None;
    }

    // A document without an id (e.g. an unregistered scratch buffer) can
    // never be re-matched by `accept_selected`'s id/revision guard, so
    // there's nothing safe to open the menu against — no-op rather than
    // panic on what would otherwise be the hot typing path.
    let document_id = document_id?;
    let (query_line, query_col) = doc.offset_to_cursor(query_start_offset);
    let revision = doc.revision;

    model.ui.completion_menu = Some(CompletionMenuState {
        document_id,
        revision,
        query_start: Cursor::at(query_line, query_col),
        query,
        items,
        filtered,
        is_incomplete: carried_incomplete,
        pending_resolve: None,
    });
    model.ui.cursor_overlay = Some(CursorOverlayState::new(CursorOverlayKind::Completion));

    if !lsp_capable(model) {
        return None;
    }
    Some(Cmd::LspScheduleCompletion {
        document_id,
        position,
        revision,
        trigger_character: trigger_character.map(String::from),
    })
}

/// Runtime -> update: a `textDocument/completion` response survived the
/// supersession guards and was converted to menu items. Folds it into the
/// open menu iff the menu is still open against the same document AND the
/// same revision (a keystroke between request and response bumps the
/// revision — a newer request is already in flight, so this reply is
/// dropped rather than merged).
pub(crate) fn merge_lsp_completion(
    model: &mut AppModel,
    document_id: crate::model::editor_area::DocumentId,
    revision: u64,
    items: Vec<crate::completion::menu::MenuItem>,
    is_incomplete: bool,
) -> Option<Cmd> {
    let selected_label = {
        let state = model.ui.completion_menu.as_ref()?;
        if state.document_id != document_id || state.revision != revision {
            return None;
        }
        // An accept blocked on its resolve round trip outranks a refresh:
        // the menu closes the moment the resolution lands, and replacing
        // the list underneath it would silently swallow the user's Enter.
        if state.pending_resolve.is_some() {
            return None;
        }
        model
            .ui
            .cursor_overlay
            .as_ref()
            .and_then(|overlay| state.selected_item(overlay.selected))
            .map(|item| item.label.clone())
    };

    let state = model.ui.completion_menu.as_mut()?;
    let drop_words = model.config.completion.words == WordsMode::Fallback && !items.is_empty();
    state.items.retain(|item| {
        item.source != MenuSourceId::Lsp && !(drop_words && item.source == MenuSourceId::Words)
    });
    state.items.extend(items);
    state.is_incomplete = is_incomplete;
    state.filtered = filter_and_sort(&state.items, &state.query);
    if state.filtered.is_empty() {
        dismiss(model);
        return Some(Cmd::Redraw);
    }

    // Preserve the selection by label when the selected item survived the
    // refilter; otherwise clamp to the new range. The overlay is set and
    // cleared together with `completion_menu` everywhere, so it's present
    // here — handled without `?` regardless, because returning early after
    // the state mutation above would skip the redraw.
    let total = state.filtered.len();
    if let Some(overlay) = model.ui.cursor_overlay.as_mut() {
        if let Some(label) = selected_label {
            if let Some(pos) = state
                .filtered
                .iter()
                .position(|(_, idx, _)| state.items[*idx].label == label)
            {
                overlay.selected = pos;
            }
        }
        overlay.selected = overlay.selected.min(total - 1);
        overlay.scroll = overlay.scroll.min(overlay.selected);
    }
    Some(Cmd::Redraw)
}

fn move_selection(model: &mut AppModel, delta: i32) -> Option<Cmd> {
    let total = model
        .ui
        .completion_menu
        .as_ref()
        .map(|s| s.filtered.len())
        .unwrap_or(0);
    let state = model.ui.cursor_overlay.as_mut()?;
    if total == 0 {
        return Some(Cmd::Redraw);
    }
    // Step reduced modulo `total` so a page-sized jump on a short list
    // can't underflow (`selected + total - step` with step > total).
    let step = (delta.unsigned_abs() as usize) % total;
    state.selected = if delta > 0 {
        (state.selected + step) % total
    } else {
        (state.selected + total - step) % total
    };
    // The popup sizes itself to `min(total, MAX_VISIBLE_COMPLETION)` rows,
    // so the overlay's own minimal-reveal rule is the authority here.
    state.scroll = SelectableListViewport::compute_from(
        total,
        state.selected,
        MAX_VISIBLE_COMPLETION,
        state.scroll,
    )
    .scroll_offset;
    Some(Cmd::Redraw)
}

/// Accept the selected item at every cursor. Guarded by the revision
/// snapshot the menu was built against — a stale menu (document changed
/// underneath it without going through `sync_after_document_edit`, e.g.
/// `Undo`/`Redo` racing an explicit trigger) is dropped instead of
/// misapplied.
///
/// LSP items whose server advertises `resolveProvider` defer instead:
/// accepting one issues `completionItem/resolve` and blocks further
/// accepts until [`finish_deferred_accept`] applies it (ts-ls returns
/// minimal items whose auto-import `additionalTextEdits` only exist after
/// resolve — skipping resolve silently drops imports).
fn accept_selected(model: &mut AppModel) -> Option<Cmd> {
    let state = model.ui.completion_menu.as_ref()?;
    let selected = model.ui.cursor_overlay?.selected;
    let doc_id_matches = model.document().id == Some(state.document_id);
    let revision_matches = model.document().revision == state.revision;
    if !doc_id_matches || !revision_matches {
        return Some(dismiss_with_cleanup(model).unwrap_or(Cmd::Redraw));
    }
    let Some(item) = state.selected_item(selected) else {
        return Some(dismiss_with_cleanup(model).unwrap_or(Cmd::Redraw));
    };
    let insert = item.insert.clone();

    if let MenuInsert::Lsp(data) = &insert {
        if data.can_resolve && !data.resolved {
            let resolving = model
                .ui
                .completion_menu
                .as_ref()
                .and_then(|s| s.pending_resolve)
                .is_some();
            if resolving {
                // One resolve round trip at a time; Enter during it is
                // absorbed (the deferred accept applies on resolution).
                return Some(Cmd::Redraw);
            }
            let (server_id, root, raw_item) = (
                data.server_id.clone(),
                data.root.clone(),
                (*data.raw).clone(),
            );
            let (document_id, revision) = {
                let state = model.ui.completion_menu.as_ref()?;
                (state.document_id, state.revision)
            };
            model.ui.completion_menu.as_mut()?.pending_resolve = Some(selected);
            return Some(Cmd::Batch(vec![
                Cmd::LspResolveCompletionItem {
                    document_id,
                    revision,
                    server_id,
                    root,
                    raw_item,
                    selected,
                },
                Cmd::Redraw,
            ]));
        }
    }

    match &insert {
        MenuInsert::Text(text) => apply_text_accept(model, text),
        MenuInsert::Lsp(data) => apply_lsp_accept(model, data),
    }
}

/// Plain-text accept (words, snippets, LSP items without protocol edits):
/// each cursor's own `[word_start_before(cursor)..cursor)` range is
/// replaced with the same text, as one `EditOperation::Batch` (one undo
/// step, multi-cursor-correct).
fn apply_text_accept(model: &mut AppModel, insert_text: &str) -> Option<Cmd> {
    let cursors_before: Vec<Cursor> = model.editor().cursors.clone();
    let indices = cursors_in_reverse_order(model);
    let mut operations = Vec::new();

    for idx in indices {
        let cursor = model.editor().cursors[idx];
        let doc = model.document();
        let cursor_offset = doc.cursor_to_offset(cursor.line, cursor.column);
        // No word prefix at this cursor (e.g. an explicit trigger on an
        // empty line) -> insert at the cursor rather than skipping it, so
        // Ctrl+Space's "always works" promise holds through accept too.
        let start_offset =
            word_query_offsets(model, cursor).map_or(cursor_offset, |(start, _)| start);
        let deleted_text: String = model
            .document()
            .buffer
            .slice(start_offset..cursor_offset)
            .chars()
            .collect();

        model
            .document_mut()
            .buffer
            .remove(start_offset..cursor_offset);
        model
            .document_mut()
            .buffer
            .insert(start_offset, insert_text);

        let new_offset = start_offset + insert_text.chars().count();
        let (new_line, new_col) = model.document().offset_to_cursor(new_offset);
        operations.push(EditOperation::Replace {
            position: start_offset,
            deleted_text,
            inserted_text: insert_text.to_owned(),
            cursor_before: cursor,
            cursor_after: Cursor::at(new_line, new_col),
        });

        model.editor_mut().cursors[idx] = Cursor::at(new_line, new_col);
        model.editor_mut().cursors[idx].desired_column = None;
        let new_pos = model.editor().cursors[idx].to_position();
        model.editor_mut().selections[idx] = Selection::new(new_pos);
    }

    finish_accept(model, operations, cursors_before)
}

/// Shared postlude for both accept paths: edit-invalidation, menu close
/// (plus LSP-work cancellation), the single undo step, and the syntax/LSP
/// resync commands every buffer mutation schedules.
fn finish_accept(
    model: &mut AppModel,
    operations: Vec<EditOperation>,
    cursors_before: Vec<Cursor>,
) -> Option<Cmd> {
    // The same edit-invalidation every DocumentMsg runs: an accept is a
    // buffer mutation, so occurrence tracking and the expand-selection
    // history are stale (they could repaint ranges over the inserted text).
    {
        let editor = model.editor_mut();
        editor.occurrence_state = None;
        editor.clear_selection_history();
    }
    model.reset_cursor_blink();

    let cancel = pending_cancel_cmd(model);
    dismiss(model);

    if operations.is_empty() {
        return Some(match cancel {
            Some(cancel) => Cmd::Batch(vec![Cmd::Redraw, cancel]),
            None => Cmd::Redraw,
        });
    }

    let cursors_after: Vec<Cursor> = model.editor().cursors.clone();
    model.document_mut().push_edit(EditOperation::Batch {
        operations,
        cursors_before,
        cursors_after,
    });
    model.document_mut().is_modified = true;
    model.ensure_cursor_visible();

    let mut cmds = vec![Cmd::redraw_editor()];
    if let Some(doc_id) = model.document().id {
        if let Some(parse_cmd) = schedule_syntax_parse(model, doc_id) {
            cmds.push(parse_cmd);
        }
        if let Some(lsp_cmd) = schedule_lsp_did_change(model, doc_id) {
            cmds.push(lsp_cmd);
        }
    }
    if let Some(cancel) = cancel {
        cmds.push(cancel);
    }
    Some(Cmd::Batch(cmds))
}

/// One buffer mutation planned against the pristine buffer. Applied in
/// descending `start` order so earlier offsets stay valid throughout.
struct PlannedEdit {
    start: usize,
    deleted: String,
    inserted: String,
}

/// LSP accept with protocol edits. `textEdit`/`additionalTextEdits` carry
/// absolute document ranges, so unlike [`apply_text_accept`] this plans
/// every mutation against the pristine buffer up front:
///
/// - the primary `textEdit` replaces its own range **re-anchored to the
///   live cursor** (the type-then-Enter race is one character wide but
///   common — chars typed after the response arrived are part of the
///   completed word), falling back to the query range when the item has
///   no `textEdit`;
/// - `additionalTextEdits` (auto-imports) apply at their absolute ranges;
///   vanished lines are skipped ("never a panic"), and edits overlapping
///   the primary range are dropped rather than double-applied;
/// - cursors away from the active one shift by the net size of the
///   additional edits before them, keeping their positions honest.
fn apply_lsp_accept(model: &mut AppModel, data: &LspInsert) -> Option<Cmd> {
    // `textEdit`/`additionalTextEdits` ranges are absolute and only
    // meaningful for the active cursor; a multi-cursor accept takes the
    // plain-text path, which is multi-cursor-correct by construction.
    if model.editor().cursors.len() > 1 {
        return apply_text_accept(model, &data.text);
    }
    let cursors_before: Vec<Cursor> = model.editor().cursors.clone();
    let active_index = model.editor().active_cursor_index;
    let active_cursor = *model.editor().active_cursor();

    // ---- Planning phase (pristine buffer reads only) ----
    let doc = model.document();
    let cursor_offset = doc.cursor_to_offset(active_cursor.line, active_cursor.column);

    let (primary_start, primary_text) = match &data.text_edit {
        Some((range, new_text)) => {
            let start_pos = crate::lsp::lsp_to_position(doc, range.start);
            let start_offset = doc.cursor_to_offset(start_pos.line, start_pos.column);
            // Degenerate range (user deleted back past its start): clamp
            // to the cursor — insert-only rather than corrupting text.
            (start_offset.min(cursor_offset), new_text.clone())
        }
        None => (
            word_query_offsets(model, active_cursor).map_or(cursor_offset, |(start, _)| start),
            data.text.clone(),
        ),
    };
    let primary_deleted: String = doc
        .buffer
        .slice(primary_start..cursor_offset)
        .chars()
        .collect();

    let mut additional: Vec<(usize, usize, String, String)> = data
        .additional_text_edits
        .iter()
        .filter_map(|(range, new_text)| {
            if crate::lsp::position::range_vanished(doc, *range) {
                return None;
            }
            let start_pos = crate::lsp::lsp_to_position(doc, range.start);
            let end_pos = crate::lsp::lsp_to_position(doc, range.end);
            let start = doc.cursor_to_offset(start_pos.line, start_pos.column);
            let end = doc.cursor_to_offset(end_pos.line, end_pos.column);
            // Drop anything touching the primary range, pure-insert points
            // included (`start == end`): the spec forbids servers sending
            // overlaps, but a violation here must degrade to "import lost",
            // not swallowed characters and a misaligned undo. Boundary
            // contact is fine — an insert exactly at either edge shifts
            // with the primary op via `shift_at`.
            if start < cursor_offset && end > primary_start {
                return None;
            }
            let deleted: String = doc.buffer.slice(start..end).chars().collect();
            Some((start, end, deleted, new_text.clone()))
        })
        .collect();
    // Descending application order; stable so equal starts keep spec order.
    additional.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)));

    let planned: Vec<PlannedEdit> = additional
        .into_iter()
        .map(|(start, _end, deleted, inserted)| PlannedEdit {
            start,
            deleted,
            inserted,
        })
        .collect();

    // Net shift each position experiences from additional edits entirely
    // before it — used to move the primary range and every cursor into the
    // post-additional coordinate space.
    let shift_at = |offset: usize| -> i64 {
        planned
            .iter()
            .filter(|edit| edit.start + edit.deleted.chars().count() <= offset)
            .map(|edit| edit.inserted.chars().count() as i64 - edit.deleted.chars().count() as i64)
            .sum()
    };
    let shifted = |offset: usize| -> usize { (offset as i64 + shift_at(offset)).max(0) as usize };

    let pristine_offsets: Vec<usize> = cursors_before
        .iter()
        .map(|cur| doc.cursor_to_offset(cur.line, cur.column))
        .collect();
    // Planning done — `doc` borrow ends here.

    // ---- Application phase (descending positions) ----
    let mut operations = Vec::with_capacity(planned.len() + 1);
    for edit in &planned {
        model
            .document_mut()
            .buffer
            .remove(edit.start..edit.start + edit.deleted.chars().count());
        model
            .document_mut()
            .buffer
            .insert(edit.start, &edit.inserted);
        operations.push(EditOperation::Replace {
            position: edit.start,
            deleted_text: edit.deleted.clone(),
            inserted_text: edit.inserted.clone(),
            cursor_before: active_cursor,
            cursor_after: active_cursor,
        });
    }

    let adj_start = shifted(primary_start);
    let adj_cursor = shifted(cursor_offset);
    model.document_mut().buffer.remove(adj_start..adj_cursor);
    model.document_mut().buffer.insert(adj_start, &primary_text);

    let new_offset = adj_start + primary_text.chars().count();
    let (new_line, new_col) = model.document().offset_to_cursor(new_offset);
    operations.push(EditOperation::Replace {
        position: adj_start,
        deleted_text: primary_deleted,
        inserted_text: primary_text,
        cursor_before: active_cursor,
        cursor_after: Cursor::at(new_line, new_col),
    });

    // Cursors: the active one lands after the inserted text; the others
    // shift by the additional edits before them (an auto-import above
    // moves every later line down).
    for (idx, offset) in pristine_offsets.iter().enumerate() {
        let (line, col) = if idx == active_index {
            (new_line, new_col)
        } else {
            model.document().offset_to_cursor(shifted(*offset))
        };
        model.editor_mut().cursors[idx] = Cursor::at(line, col);
        model.editor_mut().cursors[idx].desired_column = None;
        let pos = model.editor().cursors[idx].to_position();
        model.editor_mut().selections[idx] = Selection::new(pos);
    }

    finish_accept(model, operations, cursors_before)
}

/// Runtime -> update: a deferred accept's `completionItem/resolve` round
/// trip finished (or timed out / failed — extras then empty). Folds the
/// resolved fields into the item and immediately applies the blocked
/// accept. Dropped unless the menu is still open, at the same revision,
/// with the same selection still pending.
pub(crate) fn finish_deferred_accept(
    model: &mut AppModel,
    document_id: crate::model::editor_area::DocumentId,
    revision: u64,
    selected: usize,
    detail: Option<String>,
    additional_text_edits: Vec<(lsp_types::Range, String)>,
) -> Option<Cmd> {
    let insert = {
        let state = model.ui.completion_menu.as_mut()?;
        if state.document_id != document_id
            || state.revision != revision
            || state.pending_resolve != Some(selected)
        {
            return None;
        }
        state.pending_resolve = None;
        let Some((_, idx, _)) = state.filtered.get(selected) else {
            return Some(Cmd::Redraw);
        };
        let item = &mut state.items[*idx];
        if let Some(detail) = detail {
            item.detail = Some(detail);
        }
        if let MenuInsert::Lsp(data) = &mut item.insert {
            data.resolved = true;
            // The resolved item is the whole item: servers that sent edits
            // up front send them again, so replace rather than append. An
            // empty reply (timeout/failure) keeps what was known.
            if !additional_text_edits.is_empty() {
                data.additional_text_edits = additional_text_edits;
            }
        }
        item.insert.clone()
    };

    match &insert {
        MenuInsert::Text(text) => apply_text_accept(model, text),
        MenuInsert::Lsp(data) => apply_lsp_accept(model, data),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::messages::{DocumentMsg, Msg};
    use crate::model::AppModel;
    use crate::update::update;

    fn model_with_text(text: &str) -> AppModel {
        let mut model = AppModel::new(800, 600, 1.0, vec![]);
        model.document_mut().buffer = ropey::Rope::from_str(text);
        model
    }

    fn type_str(model: &mut AppModel, s: &str) {
        for ch in s.chars() {
            update(model, Msg::Document(DocumentMsg::InsertChar(ch)));
        }
    }

    /// Place cursor 0 and keep its selection collapsed there, so
    /// `assert_invariants_with_context`'s cursor/selection consistency
    /// check (debug builds only) doesn't fire on tests that move the
    /// cursor directly instead of via `update()`.
    fn place_cursor(model: &mut AppModel, line: usize, column: usize) {
        model.editor_mut().cursors[0] = Cursor::at(line, column);
        model.editor_mut().clear_selection();
    }

    #[test]
    fn typing_a_word_char_opens_the_menu() {
        let mut model = model_with_text("value_one\nvalue_two\n");
        place_cursor(&mut model, 2, 0);
        type_str(&mut model, "val");
        let state = model.ui.completion_menu.as_ref().expect("menu open");
        assert!(!state.filtered.is_empty());
        assert_eq!(
            model.ui.cursor_overlay.map(|o| o.kind),
            Some(CursorOverlayKind::Completion)
        );
    }

    #[test]
    fn a_single_char_does_not_auto_open_the_menu() {
        // MIN_AUTO_TRIGGER_PREFIX: one typed word char is noise; the menu
        // must stay closed until the second.
        let mut model = model_with_text("value_one\n\n");
        place_cursor(&mut model, 1, 0);
        type_str(&mut model, "v");
        assert!(model.ui.completion_menu.is_none(), "one char must not open");
        type_str(&mut model, "a");
        assert!(model.ui.completion_menu.is_some(), "two chars must open");
    }

    #[test]
    fn non_word_char_dismisses_the_menu() {
        let mut model = model_with_text("value_one\n\n");
        place_cursor(&mut model, 1, 0);
        type_str(&mut model, "val");
        assert!(model.ui.completion_menu.is_some());
        type_str(&mut model, " ");
        assert!(model.ui.completion_menu.is_none());
        assert!(model.ui.cursor_overlay.is_none());
    }

    #[test]
    fn accept_replaces_the_query_with_the_selected_item() {
        let mut model = model_with_text("value_one\n\n");
        place_cursor(&mut model, 1, 0);
        type_str(&mut model, "val");
        assert!(model.ui.completion_menu.is_some());

        update(&mut model, Msg::Completion(CompletionMsg::AcceptMenuItem));

        assert!(model.ui.completion_menu.is_none());
        let line = model.document().get_line_cow(1).unwrap();
        assert_eq!(line.trim_end_matches('\n'), "value_one");
        assert_eq!(model.editor().cursors[0].column, "value_one".len());
    }

    #[test]
    fn accept_is_one_undo_step_across_multiple_cursors() {
        let mut model = model_with_text("valueA\n\nvalueB\n\n");
        model.editor_mut().cursors = vec![Cursor::at(1, 0), Cursor::at(3, 0)];
        model.editor_mut().selections = vec![
            Selection::new(model.editor().cursors[0].to_position()),
            Selection::new(model.editor().cursors[1].to_position()),
        ];
        type_str(&mut model, "val");
        assert!(model.ui.completion_menu.is_some());

        update(&mut model, Msg::Completion(CompletionMsg::AcceptMenuItem));

        assert_eq!(
            model
                .document()
                .get_line_cow(1)
                .unwrap()
                .trim_end_matches('\n'),
            "valueA"
        );
        assert_eq!(
            model
                .document()
                .get_line_cow(3)
                .unwrap()
                .trim_end_matches('\n'),
            "valueA"
        );

        // One undo reverts the whole accept batch at both cursors (not one
        // cursor at a time), back to what typing "val" alone had produced.
        update(&mut model, Msg::Document(DocumentMsg::Undo));
        assert_eq!(
            model
                .document()
                .get_line_cow(1)
                .unwrap()
                .trim_end_matches('\n'),
            "val"
        );
        assert_eq!(
            model
                .document()
                .get_line_cow(3)
                .unwrap()
                .trim_end_matches('\n'),
            "val"
        );
    }

    #[test]
    fn accept_leaves_no_selection_at_the_insertion_site() {
        // Regression: Tab-accepting a completion left a stale selection
        // highlight behind (visible immediately with stale occurrence /
        // expand-selection state, and after undo/redo via
        // restore_batch_cursors keeping old anchors).
        let mut model = model_with_text("value_one\n\n");
        place_cursor(&mut model, 1, 0);
        type_str(&mut model, "val");
        assert!(model.ui.completion_menu.is_some());

        update(&mut model, Msg::Completion(CompletionMsg::AcceptMenuItem));

        assert!(
            model.editor().selections.iter().all(|s| s.is_empty()),
            "accept must not leave a selection"
        );
        assert!(model.editor().occurrence_state.is_none());
        assert!(model.ui.cursor_visible, "accept resets the caret blink");
    }

    #[test]
    fn undo_redo_across_an_accept_keeps_selections_collapsed() {
        let mut model = model_with_text("value_one\n\n");
        place_cursor(&mut model, 1, 0);
        type_str(&mut model, "val");
        update(&mut model, Msg::Completion(CompletionMsg::AcceptMenuItem));

        update(
            &mut model,
            Msg::Document(crate::messages::DocumentMsg::Undo),
        );
        assert!(
            model.editor().selections.iter().all(|s| s.is_empty()),
            "undo across a Batch must rebuild collapsed selections"
        );
        update(
            &mut model,
            Msg::Document(crate::messages::DocumentMsg::Redo),
        );
        assert!(
            model.editor().selections.iter().all(|s| s.is_empty()),
            "redo across a Batch must rebuild collapsed selections"
        );
    }

    #[test]
    fn accept_is_multi_byte_safe_with_an_emoji_elsewhere_on_the_line() {
        // The emoji sits before the word being completed, separated by a
        // space — a boundary char, so it's outside the query. Accept must
        // replace only the query range without corrupting the emoji, and
        // the char-offset math (not byte-offset) must land the cursor
        // correctly afterward.
        let mut model = model_with_text("value_one\n🎉 \n");
        place_cursor(&mut model, 1, 2); // after "🎉 "
        type_str(&mut model, "val");
        assert!(model.ui.completion_menu.is_some());

        update(&mut model, Msg::Completion(CompletionMsg::AcceptMenuItem));

        let line = model.document().get_line_cow(1).unwrap();
        assert_eq!(line.trim_end_matches('\n'), "🎉 value_one");
        assert_eq!(
            model.editor().cursors[0].column,
            2 + "value_one".chars().count()
        );
    }

    #[test]
    fn stale_revision_drops_accept_instead_of_misapplying() {
        let mut model = model_with_text("value_one\n\n");
        place_cursor(&mut model, 1, 0);
        type_str(&mut model, "val");
        let mut state = model.ui.completion_menu.clone().expect("menu open");
        state.revision = state.revision.wrapping_sub(1); // simulate a stale collection
        model.ui.completion_menu = Some(state);

        let before = model.document().buffer.to_string();
        update(&mut model, Msg::Completion(CompletionMsg::AcceptMenuItem));

        assert_eq!(model.document().buffer.to_string(), before);
        assert!(model.ui.completion_menu.is_none());
    }

    #[test]
    fn menu_next_wraps_and_updates_selection() {
        let mut model = model_with_text("value_one\nvalue_two\nvalue_three\n\n");
        place_cursor(&mut model, 3, 0);
        type_str(&mut model, "val");
        let total = model.ui.completion_menu.as_ref().unwrap().filtered.len();
        assert!(total >= 2);

        for _ in 0..total {
            update(&mut model, Msg::Completion(CompletionMsg::MenuNext));
        }
        assert_eq!(model.ui.cursor_overlay.unwrap().selected, 0);
    }

    #[test]
    fn page_down_moves_by_a_page_and_clamps() {
        let mut model = model_with_text("value_one\nvalue_two\nvalue_three\n\n");
        place_cursor(&mut model, 3, 0);
        type_str(&mut model, "val");
        let total = model.ui.completion_menu.as_ref().unwrap().filtered.len();
        assert!(total >= 2);

        update(&mut model, Msg::Completion(CompletionMsg::MenuPageDown));
        let after_page = model.ui.cursor_overlay.unwrap().selected;
        // Step wraps modulo the list length (a page on a short list).
        assert_eq!(after_page, MAX_VISIBLE_COMPLETION % total);

        update(&mut model, Msg::Completion(CompletionMsg::MenuPageUp));
        assert_eq!(model.ui.cursor_overlay.unwrap().selected, 0);
    }

    #[test]
    fn opening_a_modal_dismisses_the_completion_menu() {
        use crate::messages::{ModalMsg, UiMsg};

        let mut model = model_with_text("value_one\n\n");
        place_cursor(&mut model, 1, 0);
        type_str(&mut model, "val");
        assert!(model.ui.completion_menu.is_some());
        assert!(model.ui.cursor_overlay.is_some());

        update(
            &mut model,
            Msg::Ui(UiMsg::Modal(ModalMsg::OpenCommandPalette)),
        );

        assert!(model.ui.has_modal());
        assert!(
            model.ui.completion_menu.is_none(),
            "an open modal must claim keys instead of the stale completion popup"
        );
        assert!(model.ui.cursor_overlay.is_none());
    }

    #[test]
    fn dock_focus_dismisses_the_completion_menu() {
        // Reproduces the hijack a modal-only dismiss check misses: focusing
        // a dock panel (e.g. Cmd+J -> terminal) never routes through
        // `Msg::Ui`/`has_modal()`, but still moves focus off the editor —
        // and `cursor_overlay.is_some()` alone still claims Up/Down/Enter/
        // Tab/Escape pre-keymap (runtime/app.rs) regardless of focus.
        use crate::messages::DockMsg;
        use crate::panel::PanelId;

        let mut model = model_with_text("value_one\n\n");
        place_cursor(&mut model, 1, 0);
        type_str(&mut model, "val");
        assert!(model.ui.completion_menu.is_some());
        assert!(model.ui.cursor_overlay.is_some());

        update(
            &mut model,
            Msg::Dock(DockMsg::FocusOrTogglePanel(PanelId::TERMINAL)),
        );

        assert_ne!(model.ui.focus, crate::model::FocusTarget::Editor);
        assert!(
            model.ui.completion_menu.is_none(),
            "dock focus must claim keys instead of the stale completion popup"
        );
        assert!(model.ui.cursor_overlay.is_none());
    }

    #[test]
    fn explicit_trigger_opens_with_empty_query() {
        let mut model = model_with_text("value_one\n\n");
        place_cursor(&mut model, 1, 0);
        update(&mut model, Msg::Completion(CompletionMsg::TriggerMenu));
        let state = model.ui.completion_menu.as_ref().expect("menu open");
        assert_eq!(state.query_start.line, 1);
        assert_eq!(state.query_start.column, 0);
    }

    /// Ctrl+Space on an empty-prefix cursor (e.g. an empty line) opens the
    /// menu against an empty query; accepting it must insert at the cursor
    /// instead of silently doing nothing (`word_query_offsets` has no word
    /// range to re-derive there).
    #[test]
    fn accept_after_explicit_trigger_on_empty_query_inserts_at_cursor() {
        let mut model = model_with_text("value_one\n\n");
        place_cursor(&mut model, 1, 0);
        update(&mut model, Msg::Completion(CompletionMsg::TriggerMenu));
        assert!(model.ui.completion_menu.is_some());

        update(&mut model, Msg::Completion(CompletionMsg::AcceptMenuItem));

        assert!(model.ui.completion_menu.is_none());
        let line = model.document().get_line_cow(1).unwrap();
        assert_eq!(line.trim_end_matches('\n'), "value_one");
        assert_eq!(model.editor().cursors[0].column, "value_one".len());
    }

    // ==== LSP source (lsp-integration.md Phase 5) ====

    use crate::completion::menu::{LspInsert, MenuInsert, MenuItem, MenuItemKind, MenuSourceId};
    use crate::model::editor_area::DocumentId;

    fn lsp_item(label: &str) -> MenuItem {
        MenuItem {
            label: label.to_owned(),
            filter_text: label.to_owned(),
            insert: MenuInsert::Lsp(Box::new(LspInsert {
                text: label.to_owned(),
                server_id: crate::lsp::LspServerId::from("rust-analyzer"),
                root: std::path::PathBuf::from("/tmp/proj"),
                raw: std::sync::Arc::new(serde_json::json!({ "label": label })),
                can_resolve: false,
                resolved: false,
                text_edit: None,
                additional_text_edits: Vec::new(),
            })),
            kind: MenuItemKind::Function,
            source: MenuSourceId::Lsp,
            detail: None,
            sort_text: None,
        }
    }

    /// Opens the menu by typing, then injects an LSP response at the
    /// menu's own (document_id, revision).
    fn open_menu_with_lsp_response(model: &mut AppModel, labels: &[&str]) {
        place_cursor(model, 1, 0);
        type_str(model, "va");
        let state = model.ui.completion_menu.clone().expect("menu open");
        merge_lsp_completion(
            model,
            state.document_id,
            state.revision,
            labels.iter().map(|l| lsp_item(l)).collect(),
            false,
        )
        .expect("merge redraws");
    }

    #[test]
    fn merging_lsp_items_replaces_only_the_lsp_block() {
        let mut model = model_with_text("vector_value\n\n");
        model.config.completion.words = WordsMode::Enabled;
        // Labels prefix-match the typed "va" so they occupy the
        // word-start tier above the buffer words (which don't).
        open_menu_with_lsp_response(&mut model, &["vacuum", "valid"]);

        let state = model.ui.completion_menu.as_ref().expect("still open");
        let sources: Vec<_> = state.items.iter().map(|i| i.source).collect();
        assert!(sources.contains(&MenuSourceId::Lsp));
        assert!(
            sources.contains(&MenuSourceId::Words),
            "offline items must survive the merge"
        );
        // Prefix-matching LSP items sort above the non-prefix buffer words.
        assert_eq!(state.items[state.filtered[0].1].source, MenuSourceId::Lsp);
    }

    // ==== completion.enabled / completion.words ====

    #[test]
    fn disabled_autocomplete_does_not_open_on_typing() {
        let mut model = model_with_text("value_one\n\n");
        model.config.completion.enabled = false;
        place_cursor(&mut model, 1, 0);
        type_str(&mut model, "va");
        assert!(model.ui.completion_menu.is_none());
    }

    #[test]
    fn disabled_autocomplete_ignores_explicit_trigger_and_sets_status() {
        let mut model = model_with_text("value_one\n\n");
        model.config.completion.enabled = false;
        place_cursor(&mut model, 1, 0);
        update_completion(&mut model, CompletionMsg::TriggerMenu);
        assert!(model.ui.completion_menu.is_none());
        assert_eq!(
            model.ui.transient_message.map(|t| t.text).as_deref(),
            Some("Autocomplete disabled")
        );
    }

    #[test]
    fn fallback_words_mode_drops_words_once_lsp_answers() {
        let mut model = model_with_text("vector_value\n\n");
        model.config.completion.words = WordsMode::Fallback;
        open_menu_with_lsp_response(&mut model, &["vacuum"]);
        let state = model.ui.completion_menu.as_ref().expect("still open");
        assert!(state.items.iter().all(|i| i.source != MenuSourceId::Words));
        assert!(state.items.iter().any(|i| i.source == MenuSourceId::Lsp));
    }

    #[test]
    fn enabled_words_mode_keeps_words_after_lsp_answers() {
        let mut model = model_with_text("vector_value\n\n");
        model.config.completion.words = WordsMode::Enabled;
        open_menu_with_lsp_response(&mut model, &["vacuum"]);
        let state = model.ui.completion_menu.as_ref().expect("still open");
        assert!(state.items.iter().any(|i| i.source == MenuSourceId::Words));
    }

    #[test]
    fn disabled_words_mode_never_lists_words() {
        let mut model = model_with_text("value_one\n\n");
        model.config.completion.words = WordsMode::Disabled;
        place_cursor(&mut model, 1, 0);
        type_str(&mut model, "va");
        assert!(model
            .ui
            .completion_menu
            .as_ref()
            .is_none_or(|s| s.items.iter().all(|i| i.source != MenuSourceId::Words)));
    }

    #[test]
    fn merging_again_replaces_the_previous_lsp_items() {
        let mut model = model_with_text("vector_value\n\n");
        open_menu_with_lsp_response(&mut model, &["vacuum"]);
        let state = model.ui.completion_menu.clone().unwrap();
        merge_lsp_completion(
            &mut model,
            state.document_id,
            state.revision,
            vec![lsp_item("valid")],
            false,
        )
        .unwrap();

        let state = model.ui.completion_menu.as_ref().unwrap();
        let lsp_labels: Vec<&str> = state
            .items
            .iter()
            .filter(|i| i.source == MenuSourceId::Lsp)
            .map(|i| i.label.as_str())
            .collect();
        assert_eq!(
            lsp_labels,
            vec!["valid"],
            "old LSP items must not accumulate"
        );
    }

    #[test]
    fn a_merge_for_a_stale_revision_is_dropped() {
        let mut model = model_with_text("vector_value\n\n");
        open_menu_with_lsp_response(&mut model, &["vacuum"]);
        let state = model.ui.completion_menu.clone().unwrap();
        let before = model.ui.completion_menu.clone().unwrap().filtered.len();

        merge_lsp_completion(
            &mut model,
            state.document_id,
            state.revision + 1,
            vec![lsp_item("late")],
            false,
        );

        let state = model.ui.completion_menu.as_ref().unwrap();
        assert_eq!(
            state.filtered.len(),
            before,
            "stale merge must not touch the menu"
        );
        assert!(!state.items.iter().any(|i| i.label == "late"));
    }

    #[test]
    fn typing_extends_the_query_and_carries_lsp_items_through_a_refilter() {
        let mut model = model_with_text("vector_value\n\n");
        open_menu_with_lsp_response(&mut model, &["vacuum", "valid", "unrelated"]);

        // Type another char: the menu refreshes synchronously (words +
        // snippets + carried LSP items refiltered against "vac").
        type_str(&mut model, "c");
        let state = model.ui.completion_menu.as_ref().expect("still open");
        assert_eq!(state.query, "vac");
        let lsp_labels: Vec<&str> = state
            .items
            .iter()
            .filter(|i| i.source == MenuSourceId::Lsp)
            .map(|i| i.label.as_str())
            .collect();
        assert_eq!(lsp_labels.len(), 3, "carried items survive the keystroke");
        // And the local filter already narrowed them.
        let visible: Vec<&str> = state
            .filtered
            .iter()
            .map(|(_, idx, _)| state.items[*idx].label.as_str())
            .collect();
        assert!(visible.contains(&"vacuum"));
        assert!(!visible.contains(&"unrelated"));
    }
    #[test]
    fn accepting_a_plain_lsp_item_inserts_its_text() {
        let mut model = model_with_text("vector_value\n\n");
        open_menu_with_lsp_response(&mut model, &["vacuum"]);

        // Select the LSP item explicitly.
        let state = model.ui.completion_menu.clone().unwrap();
        let pos = state
            .filtered
            .iter()
            .position(|(_, idx, _)| state.items[*idx].label == "vacuum")
            .unwrap();
        model.ui.cursor_overlay.as_mut().unwrap().selected = pos;

        update(&mut model, Msg::Completion(CompletionMsg::AcceptMenuItem));

        let line = model.document().get_line_cow(1).unwrap();
        assert_eq!(line.trim_end_matches('\n'), "vacuum");
        assert!(model.ui.completion_menu.is_none());
    }

    /// Moves the popup selection onto the item with `label`.
    fn select_item(model: &mut AppModel, label: &str) {
        let state = model.ui.completion_menu.clone().unwrap();
        let pos = state
            .filtered
            .iter()
            .position(|(_, idx, _)| state.items[*idx].label == label)
            .unwrap();
        model.ui.cursor_overlay.as_mut().unwrap().selected = pos;
    }

    #[test]
    fn accepting_an_unresolved_lsp_item_defers_to_resolve_then_applies() {
        let mut model = model_with_text("vector_value\n\n");
        open_menu_with_lsp_response(&mut model, &["valid_fn"]);
        // Mark the item resolve-needing.
        {
            let state = model.ui.completion_menu.as_mut().unwrap();
            for item in &mut state.items {
                if let MenuInsert::Lsp(data) = &mut item.insert {
                    data.can_resolve = true;
                }
            }
        }
        select_item(&mut model, "valid_fn");

        let cmd = update(&mut model, Msg::Completion(CompletionMsg::AcceptMenuItem));
        // The accept deferred: menu still open, buffer untouched, a resolve
        // command went out.
        assert!(
            model.ui.completion_menu.is_some(),
            "menu must stay open while resolving"
        );
        assert_eq!(
            model.document().buffer.to_string(),
            "vector_value\nva\n",
            "deferring must not touch the buffer"
        );
        let cmd = cmd.expect("resolve command");
        let Cmd::Batch(cmds) = &cmd else {
            panic!("expected batch, got {cmd:?}")
        };
        assert!(cmds.iter().any(|c| matches!(c,
            Cmd::LspResolveCompletionItem { raw_item, selected, .. }
                if raw_item["label"] == serde_json::json!("valid_fn") && *selected == 0)));

        // Resolution lands: the deferred accept applies.
        let state = model.ui.completion_menu.clone().unwrap();
        update(
            &mut model,
            Msg::Lsp(crate::messages::LspMsg::CompletionItemResolved {
                document_id: state.document_id,
                revision: state.revision,
                selected: 0,
                detail: Some("fn valid_fn()".to_owned()),
                additional_text_edits: vec![],
            }),
        );

        assert!(
            model.ui.completion_menu.is_none(),
            "resolved accept closes the menu"
        );
        let line = model.document().get_line_cow(1).unwrap();
        assert_eq!(line.trim_end_matches('\n'), "valid_fn");
    }

    #[test]
    fn a_completion_response_landing_mid_resolve_does_not_swallow_the_accept() {
        let mut model = model_with_text("vector_value\n\n");
        open_menu_with_lsp_response(&mut model, &["valid_fn"]);
        {
            let state = model.ui.completion_menu.as_mut().unwrap();
            for item in &mut state.items {
                if let MenuInsert::Lsp(data) = &mut item.insert {
                    data.can_resolve = true;
                }
            }
        }
        select_item(&mut model, "valid_fn");
        update(&mut model, Msg::Completion(CompletionMsg::AcceptMenuItem));
        let state = model.ui.completion_menu.clone().unwrap();
        assert_eq!(state.pending_resolve, Some(0));

        // A fresh list at the same revision arrives while resolving.
        merge_lsp_completion(
            &mut model,
            state.document_id,
            state.revision,
            vec![lsp_item("vanished"), lsp_item("valid_fn")],
            false,
        );
        assert_eq!(
            model.ui.completion_menu.as_ref().unwrap().pending_resolve,
            Some(0),
            "the in-progress accept must survive the refresh"
        );

        update(
            &mut model,
            Msg::Lsp(crate::messages::LspMsg::CompletionItemResolved {
                document_id: state.document_id,
                revision: state.revision,
                selected: 0,
                detail: None,
                additional_text_edits: vec![],
            }),
        );
        let line = model.document().get_line_cow(1).unwrap();
        assert_eq!(line.trim_end_matches('\n'), "valid_fn");
    }

    #[test]
    fn resolve_replaces_upfront_additional_edits_instead_of_duplicating_them() {
        // gopls/clangd send `additionalTextEdits` up front AND advertise
        // resolve; the resolved item repeats them. Appending would insert
        // the import twice.
        let mut model = model_with_text("fn main() {}\n\n    imported_fn()\n");
        place_cursor(&mut model, 2, 15);
        let doc = model.document();
        let document_id = doc.id.unwrap();
        let revision = doc.revision;
        let import = (
            lsp_types::Range::new(
                lsp_types::Position::new(0, 0),
                lsp_types::Position::new(0, 0),
            ),
            "use crate::imported_fn;\n".to_owned(),
        );
        let mut item = lsp_item("imported_fn");
        if let MenuInsert::Lsp(data) = &mut item.insert {
            data.can_resolve = true;
            data.additional_text_edits = vec![import.clone()];
        }
        model.ui.completion_menu = Some(CompletionMenuState {
            document_id,
            revision,
            query_start: Cursor::at(2, 4),
            query: "imported_fn".to_owned(),
            items: vec![item],
            filtered: vec![(0, 0, Vec::new())],
            is_incomplete: false,
            pending_resolve: None,
        });
        model.ui.cursor_overlay = Some(CursorOverlayState::new(CursorOverlayKind::Completion));

        update(&mut model, Msg::Completion(CompletionMsg::AcceptMenuItem));
        update(
            &mut model,
            Msg::Lsp(crate::messages::LspMsg::CompletionItemResolved {
                document_id,
                revision,
                selected: 0,
                detail: None,
                additional_text_edits: vec![import],
            }),
        );
        assert_eq!(
            model.document().buffer.to_string(),
            "use crate::imported_fn;\nfn main() {}\n\n    imported_fn()\n"
        );
    }

    #[test]
    fn a_multi_cursor_lsp_accept_inserts_at_every_cursor() {
        // Protocol edit ranges only mean anything for the active cursor;
        // multi-cursor takes the plain-text path so every cursor gets the
        // completion and none lands at a stale offset.
        let mut model = model_with_text("vector_value\n\n\n");
        place_cursor(&mut model, 1, 0);
        type_str(&mut model, "va");
        model.editor_mut().cursors.push(Cursor::at(2, 0));
        model
            .editor_mut()
            .selections
            .push(Selection::new(Cursor::at(2, 0).to_position()));
        let line2 = model.document().cursor_to_offset(2, 0);
        model.document_mut().buffer.insert(line2, "va");
        model.editor_mut().cursors[1] = Cursor::at(2, 2);
        let state = model.ui.completion_menu.clone().unwrap();
        let mut item = lsp_item("vacuum");
        if let MenuInsert::Lsp(data) = &mut item.insert {
            data.text_edit = Some((
                lsp_types::Range::new(
                    lsp_types::Position::new(1, 0),
                    lsp_types::Position::new(1, 2),
                ),
                "vacuum".to_owned(),
            ));
        }
        model.ui.completion_menu = Some(CompletionMenuState {
            revision: model.document().revision,
            items: vec![item],
            filtered: vec![(0, 0, Vec::new())],
            ..state
        });
        model.ui.cursor_overlay = Some(CursorOverlayState::new(CursorOverlayKind::Completion));

        update(&mut model, Msg::Completion(CompletionMsg::AcceptMenuItem));

        assert_eq!(
            model.document().buffer.to_string(),
            "vector_value\nvacuum\nvacuum\n"
        );
        let at = |i: usize| {
            let c = model.editor().cursors[i];
            (c.line, c.column)
        };
        assert_eq!(at(0), (1, 6));
        assert_eq!(at(1), (2, 6));
    }

    #[test]
    fn a_resolution_for_a_moved_selection_is_dropped() {
        let mut model = model_with_text("vector_value\n\n");
        open_menu_with_lsp_response(&mut model, &["valid_fn"]);
        {
            let state = model.ui.completion_menu.as_mut().unwrap();
            for item in &mut state.items {
                if let MenuInsert::Lsp(data) = &mut item.insert {
                    data.can_resolve = true;
                }
            }
        }
        select_item(&mut model, "valid_fn");
        update(&mut model, Msg::Completion(CompletionMsg::AcceptMenuItem));
        assert!(model.ui.completion_menu.is_some());

        // Wrong selected index: the resolution must not fire the accept.
        let state = model.ui.completion_menu.clone().unwrap();
        update(
            &mut model,
            Msg::Lsp(crate::messages::LspMsg::CompletionItemResolved {
                document_id: state.document_id,
                revision: state.revision,
                selected: 7,
                detail: None,
                additional_text_edits: vec![],
            }),
        );
        assert_eq!(
            model.document().buffer.to_string(),
            "vector_value\nva\n",
            "deferring must not touch the buffer"
        );
    }

    #[test]
    fn accepting_an_lsp_item_with_a_text_edit_applies_the_edit_range() {
        // Server targets a wider range than the typed query: the textEdit
        // range wins over the query range. Here it covers the whole
        // `self.ba` fragment and completes to `self.bar`.
        let mut model = model_with_text("struct S;\nself.ba\n");
        place_cursor(&mut model, 1, 7); // after "self.ba"

        // Build the menu directly (typing "ba" after "self." would have
        // dismissed at the dot; the LSP trigger path is what reopens it).
        let doc = model.document();
        let document_id = doc.id.unwrap();
        let revision = doc.revision;
        let mut item = lsp_item("bar");
        if let MenuInsert::Lsp(data) = &mut item.insert {
            data.text_edit = Some((
                lsp_types::Range::new(
                    lsp_types::Position::new(1, 0), // start of "self.ba"
                    lsp_types::Position::new(1, 7),
                ),
                "self.bar".to_owned(),
            ));
        }
        model.ui.completion_menu = Some(CompletionMenuState {
            document_id,
            revision,
            query_start: Cursor::at(1, 5),
            query: "ba".to_owned(),
            items: vec![item],
            filtered: vec![(0, 0, Vec::new())],
            is_incomplete: false,
            pending_resolve: None,
        });
        model.ui.cursor_overlay = Some(CursorOverlayState::new(CursorOverlayKind::Completion));

        update(&mut model, Msg::Completion(CompletionMsg::AcceptMenuItem));

        let line = model.document().get_line_cow(1).unwrap();
        assert_eq!(line.trim_end_matches('\n'), "self.bar");
        assert_eq!(model.editor().cursors[0].column, "self.bar".len());
    }

    #[test]
    fn additional_text_edits_apply_atomically_with_the_primary_edit() {
        // Auto-import shape: an import line inserted at the top plus the
        // primary completion — one undo step reverts both.
        let mut model = model_with_text("fn main() {}\n\n    imported_fn()\n");
        place_cursor(&mut model, 2, 15); // after "imported_fn", before ")"
        let doc = model.document();
        let document_id = doc.id.unwrap();
        let revision = doc.revision;
        let mut item = lsp_item("imported_fn");
        if let MenuInsert::Lsp(data) = &mut item.insert {
            data.additional_text_edits = vec![(
                lsp_types::Range::new(
                    lsp_types::Position::new(0, 0),
                    lsp_types::Position::new(0, 0),
                ),
                "use crate::imported_fn;\n".to_owned(),
            )];
        }
        model.ui.completion_menu = Some(CompletionMenuState {
            document_id,
            revision,
            query_start: Cursor::at(2, 4),
            query: "imported_fn".to_owned(),
            items: vec![item],
            filtered: vec![(0, 0, Vec::new())],
            is_incomplete: false,
            pending_resolve: None,
        });
        model.ui.cursor_overlay = Some(CursorOverlayState::new(CursorOverlayKind::Completion));

        update(&mut model, Msg::Completion(CompletionMsg::AcceptMenuItem));

        let text = model.document().buffer.to_string();
        assert_eq!(
            text, "use crate::imported_fn;\nfn main() {}\n\n    imported_fn()\n",
            "additional edit and primary edit both apply"
        );

        // One undo reverts BOTH edits.
        update(&mut model, Msg::Document(DocumentMsg::Undo));
        assert_eq!(
            model.document().buffer.to_string(),
            "fn main() {}\n\n    imported_fn()\n"
        );
    }

    #[test]
    fn an_additional_edit_inside_the_primary_range_is_dropped_not_swallowed() {
        // Spec violation (edits must not overlap the main edit), but a
        // misbehaving server must cost an import, not swallowed characters:
        // a pure insert strictly inside [query_start..cursor) is dropped.
        let mut model = model_with_text("fn main() {}\n\n    imported_fn()\n");
        place_cursor(&mut model, 2, 15);

        let doc = model.document();
        let document_id = doc.id.unwrap();
        let revision = doc.revision;
        let mut item = lsp_item("imported_fn");
        if let MenuInsert::Lsp(data) = &mut item.insert {
            data.additional_text_edits = vec![
                // Strictly inside the primary range (cols 4..15 of line 2).
                (
                    lsp_types::Range::new(
                        lsp_types::Position::new(2, 8),
                        lsp_types::Position::new(2, 8),
                    ),
                    "GHOST".to_owned(),
                ),
                // This one is legitimate (top of file) and must survive.
                (
                    lsp_types::Range::new(
                        lsp_types::Position::new(0, 0),
                        lsp_types::Position::new(0, 0),
                    ),
                    "use crate::imported_fn;\n".to_owned(),
                ),
            ];
        }
        model.ui.completion_menu = Some(CompletionMenuState {
            document_id,
            revision,
            query_start: Cursor::at(2, 4),
            query: "imported_fn".to_owned(),
            items: vec![item],
            filtered: vec![(0, 0, Vec::new())],
            is_incomplete: false,
            pending_resolve: None,
        });
        model.ui.cursor_overlay = Some(CursorOverlayState::new(CursorOverlayKind::Completion));

        update(&mut model, Msg::Completion(CompletionMsg::AcceptMenuItem));

        let text = model.document().buffer.to_string();
        assert_eq!(
            text, "use crate::imported_fn;\nfn main() {}\n\n    imported_fn()\n",
            "the inside insert must be dropped, the outside one applied"
        );
    }

    #[test]
    fn incomplete_responses_keep_flagging_re_requests() {
        let mut model = model_with_text("vector_value\n\n");
        open_menu_with_lsp_response(&mut model, &["vax_data"]);
        assert!(!model.ui.completion_menu.as_ref().unwrap().is_incomplete);

        let state = model.ui.completion_menu.clone().unwrap();
        merge_lsp_completion(
            &mut model,
            state.document_id,
            state.revision,
            vec![lsp_item("vax_data")],
            true,
        )
        .unwrap();
        assert!(model.ui.completion_menu.as_ref().unwrap().is_incomplete);

        // Carried through a keystroke refresh.
        type_str(&mut model, "x");
        assert!(model.ui.completion_menu.as_ref().unwrap().is_incomplete);
    }

    #[test]
    fn a_server_trigger_character_keeps_the_menu_open_with_an_empty_query() {
        use crate::lsp::LspServerId;
        use crate::syntax::LanguageId;

        let mut model = model_with_text("struct S;\n\n");
        model.document_mut().language = LanguageId::Rust;
        model.document_mut().file_path = Some(std::path::PathBuf::from("/tmp/proj/lib.rs"));
        model
            .lsp
            .completion_trigger_characters
            .insert(LspServerId::from("rust-analyzer"), vec![".".to_owned()]);
        place_cursor(&mut model, 1, 0);
        type_str(&mut model, "st");
        assert!(model.ui.completion_menu.is_some());

        // Typing `.` (a boundary char that would normally dismiss) keeps
        // the menu open and restarts the query at the cursor.
        type_str(&mut model, ".");
        let state = model.ui.completion_menu.as_ref().expect("menu stays open");
        assert_eq!(state.query, "", "query restarts after the trigger char");
        // The `.` itself is already inserted when the sync runs, so the
        // new query starts right after it.
        assert_eq!(state.query_start.column, 3);
    }

    #[test]
    fn a_trigger_character_does_not_carry_the_previous_words_lsp_items() {
        use crate::lsp::LspServerId;
        use crate::syntax::LanguageId;

        let mut model = model_with_text("struct S;\n\n");
        model.document_mut().language = LanguageId::Rust;
        model.document_mut().file_path = Some(std::path::PathBuf::from("/tmp/proj/lib.rs"));
        model
            .lsp
            .completion_trigger_characters
            .insert(LspServerId::from("rust-analyzer"), vec![".".to_owned()]);
        place_cursor(&mut model, 1, 0);
        type_str(&mut model, "st");
        let state = model.ui.completion_menu.clone().unwrap();
        merge_lsp_completion(
            &mut model,
            state.document_id,
            state.revision,
            vec![lsp_item("std"), lsp_item("str")],
            false,
        );

        // `st.` lists members of `st`, not the `st*` items from before —
        // every string starts with "", so the prefix carry alone would
        // keep them.
        type_str(&mut model, ".");
        let items = &model.ui.completion_menu.as_ref().unwrap().items;
        assert!(
            !items.iter().any(|i| i.source == MenuSourceId::Lsp),
            "previous word's LSP items must not carry across a trigger char"
        );
    }

    #[test]
    fn a_trigger_character_without_a_server_dismisses_normally() {
        // PlainText has no registered server: `.` is just a boundary char.
        let mut model = model_with_text("value_one\n\n");
        place_cursor(&mut model, 1, 0);
        type_str(&mut model, "val");
        assert!(model.ui.completion_menu.is_some());
        type_str(&mut model, ".");
        assert!(model.ui.completion_menu.is_none());
    }

    #[test]
    fn a_document_without_an_id_never_opens_the_menu() {
        let mut model = model_with_text("value_one\n\n");
        model.document_mut().id = None;
        place_cursor(&mut model, 1, 0);
        type_str(&mut model, "val");
        assert!(model.ui.completion_menu.is_none());
    }

    #[test]
    fn page_up_down_are_noops_without_a_menu() {
        let mut model = model_with_text("value_one\n\n");
        place_cursor(&mut model, 1, 0);
        // Must not panic with no menu/overlay.
        update(&mut model, Msg::Completion(CompletionMsg::MenuPageDown));
        update(&mut model, Msg::Completion(CompletionMsg::MenuPageUp));
    }

    #[test]
    fn document_ids_stay_distinct_in_merge_guards() {
        let mut model = model_with_text("vector_value\n\n");
        model.config.completion.words = WordsMode::Enabled;
        open_menu_with_lsp_response(&mut model, &["vec_new"]);
        let state = model.ui.completion_menu.clone().unwrap();
        let wrong_doc = DocumentId(state.document_id.0 + 99);
        merge_lsp_completion(
            &mut model,
            wrong_doc,
            state.revision,
            vec![lsp_item("other_doc")],
            false,
        );
        assert!(!model
            .ui
            .completion_menu
            .as_ref()
            .unwrap()
            .items
            .iter()
            .any(|i| i.label == "other_doc"));
    }
}
