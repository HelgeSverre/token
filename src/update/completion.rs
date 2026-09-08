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
//! - [`finish_deferred_accept`] merges a `completionItem/resolve` reply into
//!   its item (documentation for the docs card, auto-import edits) and
//!   applies the accept that was blocked on it, if any.
//!
//! Words/snippets stay synchronous sub-millisecond rope scans collected
//! inline (autocomplete.md: "no worker and no debounce for v1 menu
//! sources"). LSP items are asynchronous: the request is debounced
//! runtime-side (`COMPLETION_DEBOUNCE`), responses are revision-guarded,
//! and between round trips the previous LSP items ride along — refiltered
//! locally on every keystroke, replaced wholesale when a fresh response
//! lands.

mod commit;
mod paths;

pub(super) use commit::{cancel_pending_commit, reconcile_pending_commit, try_commit_character};
pub(super) use paths::reconcile as reconcile_paths;

use crate::commands::{Cmd, ResolvePurpose};
use crate::completion::context::CompletionContext;
use crate::completion::menu::{
    filter_and_sort, CompletionMenuState, LspInsert, MenuInsert, MenuSourceId,
};
use crate::completion::sources::{collect_snippets, collect_words};
use crate::config::WordsMode;
use crate::lsp::lsp_server_def;
use crate::messages::CompletionMsg;
#[cfg(test)]
use crate::model::Selection;
use crate::model::{AppModel, Cursor, CursorOverlayKind, CursorOverlayState};
use crate::util::text::{char_type, CharType};
use crate::view::overlay_surface::{SelectableListViewport, MAX_VISIBLE_COMPLETION};

use super::document::word_start_before;
use super::text_edits::{
    apply_planned_edits, plan_text_edits, EditCarets, EditOffsetMap, PlannedEdit,
};

/// Word chars a user must have typed before the menu auto-opens. A single
/// char opening the popup read as noise (every prose word flashed it);
/// Ctrl+Space is unaffected and still works on an empty query.
const MIN_AUTO_TRIGGER_PREFIX: usize = 2;

pub(super) fn update_completion(model: &mut AppModel, msg: CompletionMsg) -> Option<Cmd> {
    match msg {
        CompletionMsg::PageDocumentation { forward } => {
            has_documentation(model).then_some(Cmd::PageCompletionDocumentation { forward })
        }
        CompletionMsg::DocumentationScrolled(scroll) => {
            if !has_documentation(model) {
                return None;
            }
            let overlay = model.ui.cursor_overlay.as_mut()?;
            if overlay.docs_scroll == scroll {
                return None;
            }
            overlay.docs_scroll = scroll;
            Some(Cmd::Redraw)
        }
        CompletionMsg::ToggleDocumentation => {
            if !has_documentation(model) {
                return None;
            }
            let overlay = model.ui.cursor_overlay.as_mut()?;
            overlay.docs_expanded = !overlay.docs_expanded;
            Some(Cmd::Redraw)
        }
        CompletionMsg::PathsReady { request, result } => paths::ready(model, request, result),
        CompletionMsg::InlineStatisticsSaved(result) => {
            let failed = result.is_err();
            let notify = failed && !model.ui.inline_statistics_failed;
            model.ui.inline_statistics_failed = failed;
            if notify {
                model.ui.transient_message = Some(crate::model::TransientMessage::new(
                    "Inline statistics could not be saved; check permissions or file format",
                    std::time::Duration::from_secs(5),
                ));
                Some(Cmd::redraw_status_bar())
            } else {
                None
            }
        }
        CompletionMsg::TriggerMenu => {
            let cancel = cancel_pending_commit(model);
            super::merge_cmds(cancel, trigger_explicit(model))
        }
        CompletionMsg::MenuNext => move_selection(model, 1),
        CompletionMsg::MenuPrev => move_selection(model, -1),
        CompletionMsg::MenuPageUp => move_selection(model, -(MAX_VISIBLE_COMPLETION as i32)),
        CompletionMsg::MenuPageDown => move_selection(model, MAX_VISIBLE_COMPLETION as i32),
        CompletionMsg::AcceptMenuItem => accept_selected(model),
        CompletionMsg::Dismiss => dismiss_with_cleanup(model),
        CompletionMsg::TriggerInline { explicit } => super::inline::trigger(model, explicit),
        CompletionMsg::AcceptInline(granularity) => super::inline::accept(model, granularity),
        CompletionMsg::CycleInline { forward } => super::inline::cycle(model, forward),
        CompletionMsg::DismissInline => super::inline::dismiss(model),
        CompletionMsg::InlineDeadlineFired { snapshot, explicit } => {
            super::inline::deadline_fired(model, snapshot, explicit)
        }
        CompletionMsg::InlineContextReady { job, root } => {
            super::inline::context_ready(model, job, root)
        }
        CompletionMsg::InlineReady { snapshot, texts } => {
            super::inline::ready(model, snapshot, texts)
        }
        CompletionMsg::InlineFailed { snapshot, error } => {
            super::inline::failed(model, snapshot, error)
        }
    }
}

fn has_documentation(model: &AppModel) -> bool {
    !model.ui.has_modal()
        && model.ui.focus == crate::model::FocusTarget::Editor
        && model.ui.has_visible_completion()
        && model.ui.cursor_overlay.is_some_and(|overlay| {
            model
                .ui
                .completion_menu
                .as_ref()
                .is_some_and(|menu| menu.selected_documentation(overlay.selected).is_some())
        })
}

/// Close the popup and drop its state, if open. A no-op if it's already
/// closed (every call site can call this unconditionally). Returns whether
/// anything was actually open, so callers that don't already redraw for
/// other reasons can decide whether a redraw is needed.
pub(crate) fn dismiss(model: &mut AppModel) -> bool {
    let mut was_open = model.ui.completion_commit.take().is_some();
    was_open |= model.ui.completion_path.take().is_some();
    if model.ui.completion_menu.is_some() {
        model.ui.completion_menu = None;
        was_open = true;
    }
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
    focused_server_id(model)
        .and_then(|id| model.lsp.completion_trigger_characters.get(&id))
        .cloned()
        .unwrap_or_default()
}

/// The focused document's server `(trigger, retrigger)` signature-help
/// characters (`LspMsg::ServerSignatureTriggers` mirror).
fn signature_triggers_for(model: &AppModel) -> (Vec<String>, Vec<String>) {
    focused_server_id(model)
        .and_then(|id| model.lsp.signature_trigger_characters.get(&id))
        .cloned()
        .unwrap_or_default()
}

fn focused_server_id(model: &AppModel) -> Option<crate::lsp::LspServerId> {
    lsp_server_def(model.document().language).map(|def| crate::lsp::LspServerId::from(def.id))
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

fn completion_enabled(model: &AppModel) -> bool {
    model.config.completion.enabled
}

/// `CommandId::ToggleAutocomplete`: flips `completion.enabled`, persists
/// it, and closes any open menu.
pub fn toggle_enabled(model: &mut AppModel) -> Option<Cmd> {
    let enabled = !model.config.completion.enabled;
    model.config.completion.enabled = enabled;
    model.ui.set_status(if enabled {
        "Autocomplete enabled"
    } else {
        "Autocomplete disabled"
    });
    let mut cmds = vec![
        Cmd::SaveConfiguration {
            config: Box::new(model.config.clone()),
        },
        Cmd::redraw_status_bar(),
    ];
    if let Some(cancel) = dismiss_with_cleanup(model) {
        cmds.push(cancel);
    }
    Some(Cmd::Batch(cmds))
}

/// Ctrl+Space (or any other explicit-trigger binding): open with whatever
/// query is at the cursor, including an empty one (word chars aren't
/// required — autocomplete.md: "Ctrl+Space always works").
fn trigger_explicit(model: &mut AppModel) -> Option<Cmd> {
    if !completion_enabled(model) {
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
    if let Some(command) = paths::open(model, true) {
        return Some(command);
    }
    let cursor = *model.editor().active_cursor();
    let doc = model.document();
    let offset = doc.cursor_to_offset(cursor.line, cursor.column);
    let query_start_offset = word_query_offsets(model, cursor)
        .map(|(start, _)| start)
        .unwrap_or(offset);
    let mut cmds = vec![Cmd::Redraw];
    if let Some(schedule) = open_or_refresh(model, cursor, query_start_offset, offset, None, true) {
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
    if !completion_enabled(model) {
        return dismiss_with_cleanup(model);
    }
    if !model.editor().is_plain_text_mode() {
        dismiss(model);
        return None;
    }
    let signature = signature_help_after_edit(model, typed_char);
    let completion = sync_completion_after_edit(model, opens_on_word_char, typed_char);
    match (completion, signature) {
        (Some(a), Some(b)) => Some(Cmd::Batch(vec![a, b])),
        (a, b) => a.or(b),
    }
}

/// Signature help's typing-driven request: a server trigger character
/// opens it; while open, a retrigger character or any other edit
/// re-requests (`isRetrigger`) so the active parameter tracks the caret.
/// Revision-guarded on reply, so a burst of keystrokes only lands the
/// last one.
fn signature_help_after_edit(model: &AppModel, typed_char: Option<char>) -> Option<Cmd> {
    if !lsp_capable(model) {
        return None;
    }
    let (trigger, retrigger) = signature_triggers_for(model);
    let is_open = model.ui.signature_help.is_some();
    let typed = typed_char.map(|ch| ch.to_string());
    let on_trigger = typed
        .as_ref()
        .is_some_and(|t| trigger.contains(t) || (is_open && retrigger.contains(t)));
    if !on_trigger && !is_open {
        return None;
    }
    super::lsp::request_signature_help(model, on_trigger.then_some(typed).flatten(), is_open)
}

fn sync_completion_after_edit(
    model: &mut AppModel,
    opens_on_word_char: bool,
    typed_char: Option<char>,
) -> Option<Cmd> {
    if super::inline::visible(model).is_some() {
        return dismiss_with_cleanup(model);
    }
    // Once explicitly opened, a session still refines normally while typing.
    // This setting only prevents automatic opening of a new dropdown; the
    // master switch above remains authoritative for all completion requests.
    if !model.config.completion.menu.enabled && model.ui.completion_menu.is_none() {
        return None;
    }
    if opens_on_word_char || typed_char.is_some() || model.ui.completion_path.is_some() {
        let explicit = model
            .ui
            .completion_path
            .as_ref()
            .is_some_and(|request| request.explicit);
        if let Some(command) = paths::open(model, explicit) {
            return Some(command);
        }
        if model.ui.completion_path.is_some() {
            return dismiss_with_cleanup(model);
        }
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
            if let Some(schedule) = open_or_refresh(model, cursor, offset, offset, typed_char, true)
            {
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
    if let Some(schedule) =
        open_or_refresh(model, cursor, query_start_offset, cursor_offset, None, true)
    {
        cmds.push(schedule);
    }
    Some(batch_redraw(cmds))
}

/// Collect context-appropriate sources and retain a request session even when
/// there are no visible rows yet. Only a nonempty list owns the cursor overlay.
fn open_or_refresh(
    model: &mut AppModel,
    cursor: Cursor,
    query_start_offset: usize,
    cursor_offset: usize,
    trigger_character: Option<char>,
    request_lsp: bool,
) -> Option<Cmd> {
    let doc = model.document();
    let document_id = doc.id?;
    let query = doc
        .buffer
        .slice(query_start_offset..cursor_offset)
        .to_string();
    let position = crate::lsp::position_to_lsp(doc, cursor.to_position());
    let (line, column) = doc.offset_to_cursor(query_start_offset);
    let query_start = Cursor::at(line, column);
    let context = CompletionContext::at(doc, query_start);
    let capable = lsp_capable(model);
    let selected = model
        .ui
        .cursor_overlay
        .map_or(0, |overlay| overlay.selected);
    let previous = model.ui.completion_menu.take();
    let (carried_items, carried_incomplete, selected_label) = previous
        .filter(|prev| {
            prev.document_id == document_id
                && prev.query_start == query_start
                && trigger_character.is_none()
                && (query.starts_with(&prev.query) || prev.query.starts_with(&query))
        })
        .map(|prev| {
            let selected_label = (prev.selection_changed && prev.query == query)
                .then(|| prev.selected_item(selected).map(|item| item.label.clone()))
                .flatten();
            let mut items = prev.items;
            items.retain(|item| item.source == MenuSourceId::Lsp);
            (items, prev.is_incomplete, selected_label)
        })
        .unwrap_or_default();
    let doc = model.document();
    let mut items = Vec::new();
    if !query.is_empty() && context.allows_words() {
        let menu = &model.config.completion.menu;
        let collect = match menu.words {
            WordsMode::Enabled => true,
            WordsMode::Fallback => carried_items.is_empty(),
            WordsMode::Disabled => false,
        };
        if collect {
            items.extend(collect_words(doc, cursor, &query, menu.min_word_length));
        }
    }
    if !query.is_empty() && context.allows_snippets() {
        items.extend(collect_snippets(doc.language));
    }
    items.extend(carried_items);
    let filtered = filter_and_sort(&items, &query);
    // Retain an invisible request session. An empty local list must neither
    // prevent the LSP request nor steal Tab/Enter while its answer is pending.
    // Unknown syntax may acquire safe local candidates when parsing completes.
    if filtered.is_empty() && !capable && context != CompletionContext::Unknown {
        dismiss(model);
        return None;
    }
    let revision = doc.revision;
    let mut state = CompletionMenuState {
        document_id,
        revision,
        query_start,
        query,
        items,
        filtered,
        is_incomplete: carried_incomplete,
        pending_resolve: None,
        context,
        selection_changed: selected_label.is_some(),
    };
    let selected = selected_label
        .and_then(|label| {
            state
                .filtered
                .iter()
                .position(|(_, index, _)| state.items[*index].label == label)
        })
        .unwrap_or_else(|| {
            state.selection_changed = false;
            state.preferred_index()
        });
    let visible = !state.filtered.is_empty();
    model.ui.completion_menu = Some(state);
    model.ui.cursor_overlay = visible.then(|| {
        let mut overlay = CursorOverlayState::new(CursorOverlayKind::Completion);
        overlay.selected = selected;
        overlay
    });
    if !capable {
        return Some(Cmd::Redraw);
    }
    let mut cmds = Vec::new();
    if request_lsp {
        cmds.push(Cmd::LspScheduleCompletion {
            document_id,
            position,
            revision,
            trigger_character: trigger_character.map(String::from),
        });
    } else {
        cmds.push(Cmd::Redraw);
    }
    cmds.extend(schedule_docs_resolve(model));
    Some(batch_redraw(cmds))
}

/// A fresh parse can supply safe identifiers for a session that was waiting on
/// syntax. Do not open a dismissed session or disturb a deferred acceptance.
pub(crate) fn refresh_after_syntax(
    model: &mut AppModel,
    document_id: crate::model::DocumentId,
) -> Option<Cmd> {
    if model.ui.completion_path.is_some() {
        return paths::refresh_after_syntax(model, document_id);
    }
    let state = model.ui.completion_menu.as_ref()?;
    if state.document_id != document_id
        || state.context != CompletionContext::Unknown
        || state.pending_resolve.is_some()
        || model.document().id != Some(document_id)
        || state.revision != model.document().revision
    {
        return None;
    }
    let start = model
        .document()
        .cursor_to_offset(state.query_start.line, state.query_start.column);
    let cursor = *model.editor().active_cursor();
    let end = model
        .document()
        .cursor_to_offset(cursor.line, cursor.column);
    if start > end || model.document().buffer.slice(start..end) != state.query {
        return None;
    }
    // The query has not changed. Do not postpone the existing LSP debounce or
    // supersede an in-flight request just because syntax became available.
    open_or_refresh(model, cursor, start, end, None, false)
}

/// `Cmd::LspScheduleResolve` for the selected row when it is an LSP item
/// that still needs its resolve round trip (documentation typically only
/// arrives on resolve). `None` while an accept's resolve is in flight —
/// a docs request must never supersede it.
fn schedule_docs_resolve(model: &AppModel) -> Option<Cmd> {
    let state = model.ui.completion_menu.as_ref()?;
    if state.pending_resolve.is_some() {
        return None;
    }
    let selected = model.ui.cursor_overlay?.selected;
    let MenuInsert::Lsp(data) = &state.selected_item(selected)?.insert else {
        return None;
    };
    if !data.can_resolve || data.resolved {
        return None;
    }
    Some(Cmd::LspScheduleResolve {
        document_id: state.document_id,
        revision: state.revision,
        server_id: data.server_id.clone(),
        root: data.root.clone(),
        raw_item: (*data.raw).clone(),
        selected,
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
    let path_session = if let Some(request) = &model.ui.completion_path {
        if !paths::valid(model, request) {
            return None;
        }
        true
    } else {
        false
    };
    let selected_label = {
        let state = model.ui.completion_menu.as_ref()?;
        if state.document_id != document_id
            || state.revision != revision
            || model.document().id != Some(document_id)
            || model.document().revision != revision
        {
            return None;
        }
        let cursor = *model.editor().active_cursor();
        let start = model
            .document()
            .cursor_to_offset(state.query_start.line, state.query_start.column);
        let end = model
            .document()
            .cursor_to_offset(cursor.line, cursor.column);
        if !path_session
            && (start > end || model.document().buffer.slice(start..end) != state.query)
        {
            return None;
        }
        // An accept blocked on its resolve round trip outranks a refresh:
        // the menu closes the moment the resolution lands, and replacing
        // the list underneath it would silently swallow the user's Enter.
        if state.pending_resolve.is_some() {
            return None;
        }
        state
            .selection_changed
            .then(|| {
                model
                    .ui
                    .cursor_overlay
                    .as_ref()
                    .and_then(|overlay| state.selected_item(overlay.selected))
                    .map(|item| item.label.clone())
            })
            .flatten()
    };

    let state = model.ui.completion_menu.as_mut()?;
    let drop_words = model.config.completion.menu.words == WordsMode::Fallback && !items.is_empty();
    state.items.retain(|item| {
        item.source != MenuSourceId::Lsp && !(drop_words && item.source == MenuSourceId::Words)
    });
    state.items.extend(items);
    state.is_incomplete = is_incomplete;
    state.filtered = filter_and_sort(&state.items, &state.query);
    if state.filtered.is_empty() {
        model.ui.cursor_overlay = None;
        return Some(Cmd::Redraw);
    }

    // Preserve deliberate navigation; otherwise honor the server's preferred
    // item. A pending session can acquire its first visible rows here.
    let total = state.filtered.len();
    let preferred = state.preferred_index();
    let overlay = model
        .ui
        .cursor_overlay
        .get_or_insert_with(|| CursorOverlayState::new(CursorOverlayKind::Completion));
    {
        overlay.reset_documentation();
        overlay.selected = preferred;
        let preserved = selected_label.and_then(|label| {
            state
                .filtered
                .iter()
                .position(|(_, idx, _)| state.items[*idx].label == label)
        });
        state.selection_changed = preserved.is_some();
        if let Some(pos) = preserved {
            overlay.selected = pos;
        }
        overlay.selected = overlay.selected.min(total - 1);
        overlay.scroll = SelectableListViewport::compute_from(
            total,
            overlay.selected,
            MAX_VISIBLE_COMPLETION,
            overlay.scroll,
        )
        .scroll_offset;
    }
    let mut cmds = vec![Cmd::Redraw];
    cmds.extend(schedule_docs_resolve(model));
    Some(batch_redraw(cmds))
}

fn move_selection(model: &mut AppModel, delta: i32) -> Option<Cmd> {
    if model.ui.completion_commit.is_some() {
        return dismiss_with_cleanup(model);
    }
    if let Some(state) = model.ui.completion_menu.as_mut() {
        state.selection_changed = true;
        // Navigation withdraws an earlier Enter accept. Its late resolve may
        // enrich the row, but must not accept the previous selection.
        state.pending_resolve = None;
    }
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
    let previous_selection = state.selected;
    state.selected = if delta > 0 {
        (state.selected + step) % total
    } else {
        (state.selected + total - step) % total
    };
    if state.selected != previous_selection {
        state.reset_documentation();
    }
    // The popup sizes itself to `min(total, MAX_VISIBLE_COMPLETION)` rows,
    // so the overlay's own minimal-reveal rule is the authority here.
    state.scroll = SelectableListViewport::compute_from(
        total,
        state.selected,
        MAX_VISIBLE_COMPLETION,
        state.scroll,
    )
    .scroll_offset;
    let mut cmds = vec![Cmd::Redraw];
    cmds.extend(schedule_docs_resolve(model));
    Some(batch_redraw(cmds))
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
    if model.ui.completion_commit.is_some() {
        return reconcile_pending_commit(model).or(Some(Cmd::Redraw));
    }
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
    let source = item.source;
    let insert = item.insert.clone();
    if source == MenuSourceId::Paths {
        if let MenuInsert::Text(text) = &insert {
            return paths::accept(model, text);
        }
    }

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
                    purpose: ResolvePurpose::Accept,
                },
                Cmd::Redraw,
            ]));
        }
    }

    match &insert {
        MenuInsert::Text(text) => apply_text_accept(model, text),
        MenuInsert::Lsp(data) => apply_lsp_accept(model, data, None),
    }
}

/// Replace each cursor's word prefix, planning against the untouched document.
/// Overlapping prefixes in the same word complete once, with both carets placed
/// at that completion's end. Adjacent, independent words remain separate edits.
fn apply_text_accept(model: &mut AppModel, insert_text: &str) -> Option<Cmd> {
    if model.ui.completion_path.is_some() {
        return paths::apply(model, insert_text);
    }
    let doc = model.document();
    let ranges: Vec<_> = model
        .editor()
        .cursors
        .iter()
        .map(|&cursor| {
            let end = doc.cursor_to_offset(cursor.line, cursor.column);
            (
                word_query_offsets(model, cursor).map_or(end, |(start, _)| start),
                end,
            )
        })
        .collect();
    let mut span_indices = vec![0; ranges.len()];
    let mut ordered: Vec<_> = ranges.into_iter().enumerate().collect();
    ordered.sort_unstable_by_key(|&(_, range)| range);
    let mut spans: Vec<(usize, usize)> = Vec::new();
    for (index, (start, end)) in ordered {
        if let Some(previous) = spans.last_mut() {
            if start < previous.1 || (start, end) == *previous {
                previous.1 = previous.1.max(end);
                span_indices[index] = spans.len() - 1;
                continue;
            }
        }
        span_indices[index] = spans.len();
        spans.push((start, end));
    }
    let planned: Vec<_> = spans
        .iter()
        .rev()
        .map(|&(start, end)| PlannedEdit {
            start,
            deleted: doc.buffer.slice(start..end).to_string(),
            inserted: insert_text.to_owned(),
        })
        .collect();
    // Resolve each merged completion's final end once. Counting every inserted
    // string again for every cursor would make placement quadratic in payload.
    let inserted_len = insert_text.chars().count();
    let mut removed = 0;
    let mut added = 0;
    let ends: Vec<_> = spans
        .iter()
        .map(|&(start, end)| {
            let result = start - removed + added + inserted_len;
            removed += end - start;
            added += inserted_len;
            result
        })
        .collect();
    let offsets: Vec<_> = span_indices.into_iter().map(|span| ends[span]).collect();
    finish_accept(model, &planned, &offsets)
}

/// Shared transaction for word/LSP completion; feature-owned caret placement
/// happens before the undo snapshot, while peer pane selections remain mapped.
fn finish_accept(model: &mut AppModel, planned: &[PlannedEdit], offsets: &[usize]) -> Option<Cmd> {
    let document_id = model.editor_area.focused_document_id()?;
    let editor_id = model.editor_area.focused_editor_id()?;
    let cancel = pending_cancel_cmd(model);
    dismiss(model);
    model.reset_cursor_blink();
    let effects = apply_planned_edits(
        model,
        document_id,
        planned,
        EditCarets::Place {
            editor_id,
            offsets,
            before: None,
        },
    );
    let mut cmds: Vec<_> = effects.into_iter().chain(cancel).collect();
    if cmds.is_empty() {
        Some(Cmd::Redraw)
    } else if cmds.len() == 1 {
        cmds.pop()
    } else {
        Some(Cmd::Batch(cmds))
    }
}

/// LSP ranges and the primary prefix replacement share one pristine-coordinate
/// plan. Snippet caret placement stays feature-owned; all peer positions and
/// undo effects go through the same transaction as ordinary completion.
fn apply_lsp_accept(
    model: &mut AppModel,
    data: &LspInsert,
    character: Option<char>,
) -> Option<Cmd> {
    if model.editor().cursors.len() > 1 {
        if let Some(character) = character {
            let mut text = data.text.clone();
            text.push(character);
            return apply_text_accept(model, &text);
        }
        return apply_text_accept(model, &data.text);
    }
    let active_cursor = *model.editor().active_cursor();
    let doc = model.document();
    let cursor_offset = doc.cursor_to_offset(active_cursor.line, active_cursor.column);
    let path_context = model
        .ui
        .completion_path
        .as_ref()
        .map(|request| &request.context);
    let primary_end = path_context.map_or(cursor_offset, |context| {
        if let Some((range, _)) = &data.text_edit {
            let position = crate::lsp::lsp_to_position(doc, range.end);
            doc.cursor_to_offset(position.line, position.column)
                .max(cursor_offset)
        } else {
            doc.cursor_to_offset(context.end.line, context.end.column)
        }
    });
    let (primary_start, mut primary_text) = match &data.text_edit {
        Some((range, new_text)) => {
            let start_pos = crate::lsp::lsp_to_position(doc, range.start);
            let start = doc.cursor_to_offset(start_pos.line, start_pos.column);
            (start.min(cursor_offset), new_text.clone())
        }
        None => (
            path_context.map_or_else(
                || {
                    word_query_offsets(model, active_cursor)
                        .map_or(cursor_offset, |(start, _)| start)
                },
                |context| doc.cursor_to_offset(context.start.line, context.start.column),
            ),
            data.text.clone(),
        ),
    };
    // Reject overlaps, including an insertion strictly inside the replacement.
    // Boundary inserts remain separate edits and must never be swallowed by it.
    let mut planned: Vec<_> = plan_text_edits(doc, &data.additional_text_edits)
        .into_iter()
        .filter(|edit| !(edit.start < primary_end && edit.end() > primary_start))
        .collect();
    let inserted_len = primary_text.chars().count();
    let mut caret_offset = data
        .caret_offset
        .map_or(inserted_len, |off| off.min(inserted_len));
    if let Some(character) = character {
        let byte = primary_text
            .char_indices()
            .nth(caret_offset)
            .map_or(primary_text.len(), |(byte, _)| byte);
        primary_text.insert(byte, character);
        caret_offset += 1;
    }
    let caret = EditOffsetMap::new(&planned).map(primary_start) + caret_offset;
    // Equal-point additional inserts precede the primary completion in the final
    // text: apply the primary first, then the additions in their existing order.
    planned.insert(
        0,
        PlannedEdit {
            start: primary_start,
            deleted: doc.buffer.slice(primary_start..primary_end).to_string(),
            inserted: primary_text,
        },
    );
    planned.sort_by(|a, b| b.start.cmp(&a.start).then(b.end().cmp(&a.end())));
    finish_accept(model, &planned, &[caret])
}

/// Runtime -> update: a `completionItem/resolve` round trip finished (or,
/// for a deferred accept, timed out / failed — extras then empty). Folds
/// the resolved fields into the item, then applies the accept blocked on
/// it, if any (a docs-purpose resolve just updates the card; the item is
/// marked resolved either way, so a later Enter needs no second trip).
pub(crate) fn finish_deferred_accept(
    model: &mut AppModel,
    document_id: crate::model::editor_area::DocumentId,
    revision: u64,
    selected: usize,
    detail: Option<String>,
    documentation: Option<crate::model::StyledText>,
    additional_text_edits: Vec<(lsp_types::Range, String)>,
) -> Option<Cmd> {
    if let Some(pending) = &model.ui.completion_commit {
        if !commit::pending_is_valid(model) {
            return dismiss_with_cleanup(model);
        }
        if pending.document_id != document_id || pending.selected != selected {
            return None;
        }
    }
    if !merge_resolved_item(
        model,
        document_id,
        revision,
        selected,
        detail,
        documentation,
        additional_text_edits,
    ) {
        return None;
    }
    let insert = {
        let state = model.ui.completion_menu.as_mut()?;
        if state.pending_resolve != Some(selected) {
            return Some(Cmd::Redraw);
        }
        state.pending_resolve = None;
        state.selected_item(selected)?.insert.clone()
    };
    match &insert {
        MenuInsert::Text(text) => apply_text_accept(model, text),
        MenuInsert::Lsp(data) => match model.ui.completion_commit.take() {
            Some(pending) => commit::finish_pending(model, data, pending),
            None => apply_lsp_accept(model, data, None),
        },
    }
}

/// Merges a resolve reply into `items[filtered[selected]]` and marks it
/// resolved. `false` when the menu is gone, on another document/revision,
/// or `selected` no longer indexes a row.
fn merge_resolved_item(
    model: &mut AppModel,
    document_id: crate::model::editor_area::DocumentId,
    revision: u64,
    selected: usize,
    detail: Option<String>,
    documentation: Option<crate::model::StyledText>,
    additional_text_edits: Vec<(lsp_types::Range, String)>,
) -> bool {
    let Some(state) = model.ui.completion_menu.as_mut() else {
        return false;
    };
    if state.document_id != document_id || state.revision != revision {
        return false;
    }
    let Some((_, idx, _)) = state.filtered.get(selected) else {
        return false;
    };
    let item = &mut state.items[*idx];
    if let Some(detail) = detail {
        item.detail = Some(detail);
    }
    if let MenuInsert::Lsp(data) = &mut item.insert {
        data.resolved = true;
        if let Some(documentation) = documentation {
            data.documentation = Some(documentation);
        }
        // The resolved item is the whole item: servers that sent edits
        // up front send them again, so replace rather than append. An
        // empty reply (timeout/failure) keeps what was known.
        if !additional_text_edits.is_empty() {
            data.additional_text_edits = additional_text_edits;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::messages::{DocumentMsg, Msg};
    use crate::model::AppModel;
    use crate::update::update;

    fn model_with_text(text: &str) -> AppModel {
        let mut model = AppModel::new(800, 600, 1.0);
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

        select_item(&mut model, "valueA");
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
    fn explicit_empty_query_does_not_dump_buffer_words() {
        let mut model = model_with_text("value_one\n\n");
        place_cursor(&mut model, 1, 0);
        update(&mut model, Msg::Completion(CompletionMsg::TriggerMenu));
        assert!(model.ui.completion_menu.is_none());
        assert!(model.ui.cursor_overlay.is_none());
    }

    /// Ctrl+Space on an empty-prefix cursor (e.g. an empty line) opens the
    /// menu against an empty query; accepting it must insert at the cursor
    /// instead of silently doing nothing (`word_query_offsets` has no word
    /// range to re-derive there).
    #[test]
    fn accept_after_explicit_trigger_on_empty_query_inserts_at_cursor() {
        let mut model = model_with_text("value_one\n\n");
        model.document_mut().language = crate::syntax::LanguageId::Rust;
        model.document_mut().file_path = Some("/tmp/proj/lib.rs".into());
        place_cursor(&mut model, 1, 0);
        update(&mut model, Msg::Completion(CompletionMsg::TriggerMenu));
        assert!(model.ui.completion_menu.is_some());
        assert!(model.ui.cursor_overlay.is_none());
        let state = model.ui.completion_menu.as_ref().unwrap();
        let (id, revision) = (state.document_id, state.revision);
        merge_lsp_completion(&mut model, id, revision, vec![lsp_item("value_one")], false);

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
                commit_characters: std::sync::Arc::from([]),
                caret_offset: None,
                documentation: None,
            })),
            kind: MenuItemKind::Function,
            source: MenuSourceId::Lsp,
            detail: None,
            sort_text: None,
            preselect: false,
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

    fn commit_fixture(resolve: bool) -> AppModel {
        let mut model = model_with_text("// head\n\n// tail\n");
        model.document_mut().language = crate::syntax::LanguageId::Rust;
        model.document_mut().file_path = Some("/tmp/proj/src/lib.rs".into());
        open_menu_with_lsp_response(&mut model, &["vacuum"]);
        let state = model.ui.completion_menu.as_mut().unwrap();
        let index = state.filtered[0].1;
        let MenuInsert::Lsp(data) = &mut state.items[index].insert else {
            panic!("LSP fixture");
        };
        data.can_resolve = resolve;
        data.commit_characters = std::sync::Arc::from(['(']);
        model
    }

    fn resolve_commit(model: &mut AppModel, id: crate::model::DocumentId, revision: u64) {
        update(
            model,
            Msg::Lsp(crate::messages::LspMsg::CompletionItemResolved {
                document_id: id,
                revision,
                selected: 0,
                detail: None,
                documentation: None,
                additional_text_edits: Vec::new(),
            }),
        );
    }

    #[test]
    fn commit_character_accepts_resolved_item_and_undoes_once() {
        let mut model = commit_fixture(false);
        update(&mut model, Msg::Document(DocumentMsg::InsertChar('(')));
        assert_eq!(
            model.document().buffer.to_string(),
            "// head\nvacuum(\n// tail\n"
        );
        assert_eq!(*model.editor().active_cursor(), Cursor::at(1, 7));
        assert_eq!(
            model
                .ui
                .status_bar
                .get_segment(crate::model::SegmentId::CursorPosition)
                .unwrap()
                .content
                .display_text(),
            "Ln 2, Col 8"
        );
        update(&mut model, Msg::Document(DocumentMsg::Undo));
        assert_eq!(
            model.document().buffer.to_string(),
            "// head\nva\n// tail\n"
        );
        assert_eq!(*model.editor().active_cursor(), Cursor::at(1, 2));
        update(&mut model, Msg::Document(DocumentMsg::Redo));
        assert_eq!(
            model.document().buffer.to_string(),
            "// head\nvacuum(\n// tail\n"
        );
    }

    #[test]
    fn commit_character_is_immediate_while_resolving_and_forms_one_undo() {
        let mut model = commit_fixture(true);
        let state = model.ui.completion_menu.clone().unwrap();
        let history = model.document().undo_stack.len();
        update(&mut model, Msg::Document(DocumentMsg::InsertChar('(')));
        assert_eq!(
            model.document().buffer.to_string(),
            "// head\nva(\n// tail\n"
        );
        assert_eq!(model.document().undo_stack.len(), history + 1);
        assert_eq!(
            model
                .ui
                .status_bar
                .get_segment(crate::model::SegmentId::CursorPosition)
                .unwrap()
                .content
                .display_text(),
            "Ln 2, Col 4"
        );
        resolve_commit(&mut model, state.document_id, state.revision);
        assert_eq!(
            model.document().buffer.to_string(),
            "// head\nvacuum(\n// tail\n"
        );
        assert_eq!(model.document().undo_stack.len(), history + 1);
        update(&mut model, Msg::Document(DocumentMsg::Undo));
        assert_eq!(
            model.document().buffer.to_string(),
            "// head\nva\n// tail\n"
        );
        update(&mut model, Msg::Document(DocumentMsg::Redo));
        assert_eq!(
            model.document().buffer.to_string(),
            "// head\nvacuum(\n// tail\n"
        );
    }

    #[test]
    fn commit_character_late_reply_preserves_subsequent_typing() {
        let mut model = commit_fixture(true);
        let state = model.ui.completion_menu.clone().unwrap();
        type_str(&mut model, "(x");
        resolve_commit(&mut model, state.document_id, state.revision);
        assert_eq!(
            model.document().buffer.to_string(),
            "// head\nva(x\n// tail\n"
        );
    }

    #[test]
    fn commit_character_does_not_accept_pasted_text_or_unlisted_characters() {
        for message in [
            DocumentMsg::InsertText("(".into()),
            DocumentMsg::InsertChar(';'),
        ] {
            let mut model = commit_fixture(false);
            update(&mut model, Msg::Document(message));
            assert!(!model.document().buffer.to_string().contains("vacuum"));
        }
    }

    #[test]
    fn commit_character_preserves_utf16_imports_suffix_edits_and_snippet_caret() {
        use lsp_types::{Position, Range};
        for (resolve, late_edits) in [(false, false), (true, false), (true, true)] {
            let mut model = commit_fixture(resolve);
            let original = "// head\r\n🦀.va\r\n// tail\r\n";
            model.document_mut().buffer = original.into();
            place_cursor(&mut model, 1, 4);
            let extras = vec![
                (
                    Range::new(Position::new(0, 0), Position::new(0, 0)),
                    "// $0 import\r\n".to_owned(),
                ),
                (
                    Range::new(Position::new(2, 0), Position::new(2, 7)),
                    "// done".to_owned(),
                ),
            ];
            let menu = model.ui.completion_menu.as_mut().unwrap();
            menu.query_start = Cursor::at(1, 2);
            let index = menu.filtered[0].1;
            let MenuInsert::Lsp(data) = &mut menu.items[index].insert else {
                panic!("LSP fixture");
            };
            data.text_edit = Some((
                Range::new(Position::new(1, 3), Position::new(1, 5)),
                "vacuum()".into(),
            ));
            data.caret_offset = Some(7);
            data.commit_characters = std::sync::Arc::from(['🦀']);
            if !late_edits {
                data.additional_text_edits = extras.clone();
            }
            let state = menu.clone();
            let history = model.document().undo_stack.len();
            update(&mut model, Msg::Document(DocumentMsg::InsertChar('🦀')));
            if resolve {
                assert_eq!(
                    model.document().buffer.to_string(),
                    "// head\r\n🦀.va🦀\r\n// tail\r\n"
                );
                update(
                    &mut model,
                    Msg::Lsp(crate::messages::LspMsg::CompletionItemResolved {
                        document_id: state.document_id,
                        revision: state.revision,
                        selected: 0,
                        detail: None,
                        documentation: None,
                        additional_text_edits: if late_edits { extras } else { Vec::new() },
                    }),
                );
            }
            let expected = "// $0 import\r\n// head\r\n🦀.vacuum(🦀)\r\n// done\r\n";
            assert_eq!(model.document().buffer.to_string(), expected);
            assert_eq!(*model.editor().active_cursor(), Cursor::at(2, 10));
            assert_eq!(model.document().undo_stack.len(), history + 1);
            update(&mut model, Msg::Document(DocumentMsg::Undo));
            assert_eq!(model.document().buffer.to_string(), original);
            assert_eq!(*model.editor().active_cursor(), Cursor::at(1, 4));
            update(&mut model, Msg::Document(DocumentMsg::Redo));
            assert_eq!(model.document().buffer.to_string(), expected);
        }
    }

    #[test]
    fn commit_character_rejects_late_replies_after_navigation_focus_or_undo() {
        for action in [
            Msg::Editor(crate::messages::EditorMsg::MoveCursor(
                crate::messages::Direction::Left,
            )),
            Msg::Completion(CompletionMsg::MenuNext),
            Msg::Completion(CompletionMsg::Dismiss),
            Msg::Document(DocumentMsg::Undo),
            Msg::Layout(crate::messages::LayoutMsg::SplitFocused(
                crate::model::SplitDirection::Vertical,
            )),
        ] {
            let mut model = commit_fixture(true);
            let state = model.ui.completion_menu.clone().unwrap();
            update(&mut model, Msg::Document(DocumentMsg::InsertChar('(')));
            update(&mut model, action);
            let before_reply = model.document().buffer.to_string();
            resolve_commit(&mut model, state.document_id, state.revision);
            assert_eq!(model.document().buffer.to_string(), before_reply);
            assert!(model.ui.completion_commit.is_none());
        }
        for disable in [false, true] {
            let mut model = commit_fixture(true);
            let state = model.ui.completion_menu.clone().unwrap();
            update(&mut model, Msg::Document(DocumentMsg::InsertChar('(')));
            if disable {
                model.config.completion.enabled = false;
            } else {
                model.ui.focus = crate::model::FocusTarget::Modal;
            }
            resolve_commit(&mut model, state.document_id, state.revision);
            assert_eq!(
                model.document().buffer.to_string(),
                "// head\nva(\n// tail\n"
            );
            assert!(model.ui.completion_commit.is_none());
        }
    }

    #[test]
    fn commit_character_wait_survives_copy_blink_stale_reply_and_repeat_accept() {
        let mut model = commit_fixture(true);
        let state = model.ui.completion_menu.clone().unwrap();
        let cmd = update(&mut model, Msg::Document(DocumentMsg::InsertChar('(')));
        assert!(cmd_contains(&cmd, |cmd| matches!(
            cmd,
            Cmd::LspResolveCompletionItem {
                purpose: ResolvePurpose::Accept,
                ..
            }
        )));
        for action in [
            Msg::Document(DocumentMsg::Copy),
            Msg::Ui(crate::messages::UiMsg::BlinkCursor),
            Msg::Completion(CompletionMsg::AcceptMenuItem),
        ] {
            let cmd = update(&mut model, action);
            assert!(!cmd_contains(&cmd, |cmd| matches!(
                cmd,
                Cmd::LspResolveCompletionItem { .. }
            )));
            assert!(model.ui.completion_commit.is_some());
        }
        assert!(refresh_after_syntax(&mut model, state.document_id).is_none());
        resolve_commit(
            &mut model,
            state.document_id,
            state.revision.wrapping_sub(1),
        );
        assert!(model.ui.completion_commit.is_some());
        resolve_commit(&mut model, state.document_id, state.revision);
        assert_eq!(
            model.document().buffer.to_string(),
            "// head\nvacuum(\n// tail\n"
        );
    }

    #[test]
    fn commit_character_multi_cursor_restores_exact_peer_history() {
        for resolve in [false, true] {
            let mut model = commit_fixture(resolve);
            let menu = model.ui.completion_menu.clone();
            let overlay = model.ui.cursor_overlay;
            update(
                &mut model,
                Msg::Layout(crate::messages::LayoutMsg::SplitFocused(
                    crate::model::SplitDirection::Vertical,
                )),
            );
            model.ui.completion_menu = menu;
            model.ui.cursor_overlay = overlay;
            model.document_mut().buffer = "// head\nva va\n// tail\n".into();
            for editor in model.editor_area.editors.values_mut() {
                editor.cursors = vec![
                    Cursor {
                        desired_column: Some(7),
                        ..Cursor::at(1, 2)
                    },
                    Cursor {
                        desired_column: Some(9),
                        ..Cursor::at(1, 5)
                    },
                ];
                editor.selections = editor
                    .cursors
                    .iter()
                    .map(|cursor| Selection::new(cursor.to_position()))
                    .collect();
                editor.active_cursor_index = 0;
            }
            let before: Vec<_> = model
                .editor_area
                .editors
                .iter()
                .map(|(id, editor)| (*id, editor.cursors.clone(), editor.selections.clone()))
                .collect();
            let state = model.ui.completion_menu.clone().unwrap();
            let history = model.document().undo_stack.len();
            update(&mut model, Msg::Document(DocumentMsg::InsertChar('(')));
            if resolve {
                assert_eq!(
                    model.document().buffer.to_string(),
                    "// head\nva( va(\n// tail\n"
                );
                resolve_commit(&mut model, state.document_id, state.revision);
            }
            let expected = "// head\nvacuum( vacuum(\n// tail\n";
            assert_eq!(model.document().buffer.to_string(), expected);
            assert_eq!(model.document().undo_stack.len(), history + 1);
            assert_eq!(
                model.editor().cursors,
                vec![Cursor::at(1, 7), Cursor::at(1, 15)]
            );
            update(&mut model, Msg::Document(DocumentMsg::Undo));
            assert_eq!(
                model.document().buffer.to_string(),
                "// head\nva va\n// tail\n"
            );
            for (id, cursors, selections) in before {
                assert_eq!(model.editor_area.editors[&id].cursors, cursors);
                assert_eq!(model.editor_area.editors[&id].selections, selections);
            }
            update(&mut model, Msg::Document(DocumentMsg::Redo));
            assert_eq!(model.document().buffer.to_string(), expected);
        }
    }

    #[test]
    fn commit_character_can_reuse_an_existing_enter_resolve() {
        let mut model = commit_fixture(true);
        let state = model.ui.completion_menu.clone().unwrap();
        update(&mut model, Msg::Completion(CompletionMsg::AcceptMenuItem));
        let cmd = update(&mut model, Msg::Document(DocumentMsg::InsertChar('(')));
        assert!(!cmd_contains(&cmd, |cmd| matches!(
            cmd,
            Cmd::LspResolveCompletionItem { .. }
        )));
        resolve_commit(&mut model, state.document_id, state.revision);
        assert_eq!(
            model.document().buffer.to_string(),
            "// head\nvacuum(\n// tail\n"
        );
    }

    #[test]
    fn commit_character_guards_selection_visibility_server_and_file_identity() {
        for mode in 0..5 {
            let mut model = commit_fixture(false);
            match mode {
                0 => {
                    model.editor_mut().selections[0] = Selection::from_anchor_head(
                        crate::model::Position::new(1, 0),
                        crate::model::Position::new(1, 2),
                    )
                }
                1 => model.editor_mut().rectangle_selection.active = true,
                2 => model.ui.cursor_overlay = None,
                3 => model.config.lsp.enabled = false,
                4 => model.config.completion.enabled = false,
                _ => unreachable!(),
            }
            update(&mut model, Msg::Document(DocumentMsg::InsertChar('(')));
            assert!(!model.document().buffer.to_string().contains("vacuum"));
            assert!(model.ui.completion_commit.is_none());
        }
        let mut model = commit_fixture(true);
        let state = model.ui.completion_menu.clone().unwrap();
        update(&mut model, Msg::Document(DocumentMsg::InsertChar('(')));
        model.document_mut().file_path = Some("/tmp/proj/renamed.rs".into());
        resolve_commit(&mut model, state.document_id, state.revision);
        assert_eq!(
            model.document().buffer.to_string(),
            "// head\nva(\n// tail\n"
        );
        assert!(model.ui.completion_commit.is_none());
    }

    #[test]
    fn commit_character_followup_triggers_use_the_final_cursor_and_revision() {
        for resolve in [false, true] {
            let mut model = commit_fixture(resolve);
            let server = crate::lsp::LspServerId::from("rust-analyzer");
            model
                .lsp
                .completion_trigger_characters
                .insert(server.clone(), vec!["(".into()]);
            model
                .lsp
                .signature_trigger_characters
                .insert(server, (vec!["(".into()], vec![]));
            let state = model.ui.completion_menu.clone().unwrap();
            let mut cmd = update(&mut model, Msg::Document(DocumentMsg::InsertChar('(')));
            if resolve {
                assert!(!cmd_contains(&cmd, |cmd| matches!(
                    cmd,
                    Cmd::LspScheduleCompletion { .. }
                )));
                cmd = update(
                    &mut model,
                    Msg::Lsp(crate::messages::LspMsg::CompletionItemResolved {
                        document_id: state.document_id,
                        revision: state.revision,
                        selected: 0,
                        detail: None,
                        documentation: None,
                        additional_text_edits: Vec::new(),
                    }),
                );
            }
            let revision = model.document().revision;
            let position = lsp_types::Position::new(1, 7);
            assert!(cmd_contains(
                &cmd,
                |cmd| matches!(cmd, Cmd::LspScheduleCompletion {
                position: at, revision: rev, trigger_character: Some(trigger), ..
            } if *at == position && *rev == revision && trigger == "(")
            ));
            assert!(cmd_contains(
                &cmd,
                |cmd| matches!(cmd, Cmd::LspRequestSignatureHelp {
                position: at, revision: rev, trigger: Some(trigger), ..
            } if *at == position && *rev == revision && trigger == "(")
            ));
            assert_eq!(
                model.document().buffer.to_string(),
                "// head\nvacuum(\n// tail\n"
            );
        }
    }

    #[test]
    fn commit_character_uses_the_new_selection_after_pending_enter_navigation() {
        let mut model = commit_fixture(true);
        let state = model.ui.completion_menu.as_mut().unwrap();
        let mut second = state.items[state.filtered[0].1].clone();
        second.label = "valid".into();
        second.filter_text = "valid".into();
        let MenuInsert::Lsp(data) = &mut second.insert else {
            panic!("LSP fixture");
        };
        data.text = "valid".into();
        state.items.push(second);
        state.filtered = filter_and_sort(&state.items, "va");
        let state = state.clone();
        update(&mut model, Msg::Completion(CompletionMsg::AcceptMenuItem));
        update(&mut model, Msg::Completion(CompletionMsg::MenuNext));
        assert_eq!(model.ui.cursor_overlay.unwrap().selected, 1);
        let cmd = update(&mut model, Msg::Document(DocumentMsg::InsertChar('(')));
        assert!(cmd_contains(&cmd, |cmd| matches!(
            cmd,
            Cmd::LspResolveCompletionItem { selected: 1, .. }
        )));
        resolve_commit(&mut model, state.document_id, state.revision);
        assert!(model.ui.completion_commit.is_some());
        assert_eq!(
            model.document().buffer.to_string(),
            "// head\nva(\n// tail\n"
        );
        update(
            &mut model,
            Msg::Lsp(crate::messages::LspMsg::CompletionItemResolved {
                document_id: state.document_id,
                revision: state.revision,
                selected: 1,
                detail: None,
                documentation: None,
                additional_text_edits: Vec::new(),
            }),
        );
        assert_eq!(
            model.document().buffer.to_string(),
            "// head\nvalid(\n// tail\n"
        );
    }

    #[test]
    fn merging_lsp_items_replaces_only_the_lsp_block() {
        let mut model = model_with_text("value_vector\n\n");
        model.config.completion.menu.words = WordsMode::Enabled;
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

    // ==== completion.enabled / completion.menu.words ====

    #[test]
    fn completion_menu_config_auto_disabled_still_allows_explicit_refinement() {
        let mut model = model_with_text("value_one\nvalue_two\n\n");
        model.config.lsp.enabled = false;
        model.config.completion.menu.enabled = false;
        place_cursor(&mut model, 2, 0);
        type_str(&mut model, "va");
        assert!(model.ui.completion_menu.is_none());
        update(&mut model, Msg::Completion(CompletionMsg::TriggerMenu));
        assert!(model.ui.has_visible_completion());
        type_str(&mut model, "l");
        assert_eq!(model.ui.completion_menu.as_ref().unwrap().query, "val");
        update(&mut model, Msg::Completion(CompletionMsg::Dismiss));
        type_str(&mut model, "u");
        assert!(model.ui.completion_menu.is_none());
    }

    #[test]
    fn completion_menu_config_candidate_length_is_not_a_prefix_threshold() {
        let mut model = model_with_text("value\nvaluable\n\n");
        model.config.lsp.enabled = false;
        model.config.completion.menu.min_word_length = 6;
        place_cursor(&mut model, 2, 0);
        type_str(&mut model, "va");
        let menu = model.ui.completion_menu.as_ref().unwrap();
        let words: Vec<_> = menu
            .items
            .iter()
            .filter(|item| item.source == MenuSourceId::Words)
            .map(|item| item.label.as_str())
            .collect();
        assert_eq!(words, vec!["valuable"]);
    }

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
        let mut model = model_with_text("value_vector\n\n");
        model.config.completion.menu.words = WordsMode::Fallback;
        open_menu_with_lsp_response(&mut model, &["vacuum"]);
        let state = model.ui.completion_menu.as_ref().expect("still open");
        assert!(state.items.iter().all(|i| i.source != MenuSourceId::Words));
        assert!(state.items.iter().any(|i| i.source == MenuSourceId::Lsp));
    }

    #[test]
    fn enabled_words_mode_keeps_words_after_lsp_answers() {
        let mut model = model_with_text("value_vector\n\n");
        model.config.completion.menu.words = WordsMode::Enabled;
        open_menu_with_lsp_response(&mut model, &["vacuum"]);
        let state = model.ui.completion_menu.as_ref().expect("still open");
        assert!(state.items.iter().any(|i| i.source == MenuSourceId::Words));
    }

    #[test]
    fn disabled_words_mode_never_lists_words() {
        let mut model = model_with_text("value_one\n\n");
        model.config.completion.menu.words = WordsMode::Disabled;
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
        let mut model = model_with_text("value_vector\n\n");
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
        let mut model = model_with_text("value_vector\n\n");
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
        let mut model = model_with_text("value_vector\n\n");
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
        let mut model = model_with_text("value_vector\n\n");
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

    #[test]
    fn converted_upfront_completion_edits_survive_accept_and_empty_resolve() {
        for can_resolve in [false, true] {
            let mut model = model_with_text("value_vector\n\n");
            open_menu_with_lsp_response(&mut model, &["vacuum"]);
            let state = model.ui.completion_menu.as_ref().unwrap();
            let (document_id, revision) = (state.document_id, state.revision);
            let raw: lsp_types::CompletionItem = serde_json::from_value(serde_json::json!({
                "label": "vacuum",
                "additionalTextEdits": [{
                    "range": { "start": { "line": 0, "character": 0 }, "end": { "line": 0, "character": 0 } },
                    "newText": "use vacuum;\n"
                }]
            })).unwrap();
            let options = lsp_types::CompletionOptions {
                resolve_provider: Some(can_resolve),
                ..Default::default()
            };
            let items = crate::completion::lsp::items_to_menu_items(
                vec![raw],
                &crate::lsp::LspServerId::from("fixture"),
                std::path::Path::new("/tmp/proj"),
                Some(&options),
            );
            update(
                &mut model,
                Msg::Lsp(crate::messages::LspMsg::CompletionResolved {
                    document_id,
                    revision,
                    items,
                    is_incomplete: false,
                }),
            );
            select_item(&mut model, "vacuum");
            let before = model.document().buffer.to_string();
            update(&mut model, Msg::Completion(CompletionMsg::AcceptMenuItem));
            if can_resolve {
                assert_eq!(model.document().buffer.to_string(), before);
                update(
                    &mut model,
                    Msg::Lsp(crate::messages::LspMsg::CompletionItemResolved {
                        document_id,
                        revision,
                        selected: 0,
                        detail: None,
                        documentation: None,
                        additional_text_edits: vec![],
                    }),
                );
            }
            assert_eq!(
                model.document().buffer.to_string(),
                "use vacuum;\nvalue_vector\nvacuum\n"
            );
            assert_eq!(*model.editor().active_cursor(), Cursor::at(2, 6));
            update(&mut model, Msg::Document(DocumentMsg::Undo));
            assert_eq!(model.document().buffer.to_string(), before);
            assert_eq!(*model.editor().active_cursor(), Cursor::at(1, 2));
        }
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
        let mut model = model_with_text("value_vector\n\n");
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
            "value_vector\nva\n",
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
                documentation: None,
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
        let mut model = model_with_text("value_vector\n\n");
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
                documentation: None,
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
            context: Default::default(),
            selection_changed: false,
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
                documentation: None,
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
        let mut model = model_with_text("value_vector\n\n\n");
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
            "value_vector\nvacuum\nvacuum\n"
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
        let mut model = model_with_text("value_vector\n\n");
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
                documentation: None,
                additional_text_edits: vec![],
            }),
        );
        assert_eq!(
            model.document().buffer.to_string(),
            "value_vector\nva\n",
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
            context: Default::default(),
            selection_changed: false,
        });
        model.ui.cursor_overlay = Some(CursorOverlayState::new(CursorOverlayKind::Completion));

        update(&mut model, Msg::Completion(CompletionMsg::AcceptMenuItem));

        let line = model.document().get_line_cow(1).unwrap();
        assert_eq!(line.trim_end_matches('\n'), "self.bar");
        assert_eq!(model.editor().cursors[0].column, "self.bar".len());
    }

    #[test]
    fn accepting_a_snippet_item_places_the_caret_at_its_caret_offset() {
        let mut model = model_with_text("value_vector\n\n");
        place_cursor(&mut model, 1, 0);
        type_str(&mut model, "va");
        let state = model.ui.completion_menu.clone().expect("menu open");
        let mut item = lsp_item("vacuum");
        if let MenuInsert::Lsp(data) = &mut item.insert {
            // `println!("$0")` as conversion leaves it.
            data.text = "println!(\"\")".to_owned();
            data.caret_offset = Some(10);
        }
        merge_lsp_completion(
            &mut model,
            state.document_id,
            state.revision,
            vec![item],
            false,
        )
        .expect("merge redraws");
        select_item(&mut model, "vacuum");

        update(&mut model, Msg::Completion(CompletionMsg::AcceptMenuItem));

        let line = model.document().get_line_cow(1).unwrap();
        assert_eq!(line.trim_end_matches('\n'), "println!(\"\")");
        assert_eq!(
            model.editor().cursors[0].column,
            10,
            "caret between the quotes"
        );
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
            context: Default::default(),
            selection_changed: false,
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
    fn lsp_boundary_inserts_preserve_snippet_caret_peer_positions_and_history() {
        use crate::messages::LayoutMsg;
        use crate::model::{Position, SplitDirection};
        let mut model = model_with_text("va tail\n");
        place_cursor(&mut model, 0, 2);
        let peer = model.editor().id.unwrap();
        let before = Selection::from_anchor_head(Position::new(0, 7), Position::new(0, 2));
        model.editor_mut().selections[0] = before;
        update(
            &mut model,
            Msg::Layout(LayoutMsg::SplitFocused(SplitDirection::Horizontal)),
        );
        place_cursor(&mut model, 0, 2);
        let MenuInsert::Lsp(mut data) = lsp_item("value").insert else {
            unreachable!()
        };
        data.text = "value(🙂)".into();
        data.caret_offset = Some(6);
        data.additional_text_edits = [(0, "α\n"), (2, "!")]
            .into_iter()
            .map(|(column, text)| {
                let point = lsp_types::Position::new(0, column);
                (lsp_types::Range::new(point, point), text.to_owned())
            })
            .collect();
        apply_lsp_accept(&mut model, &data, None);
        assert_eq!(model.document().buffer.to_string(), "α\nvalue(🙂)! tail\n");
        assert_eq!(*model.editor().active_cursor(), Cursor::at(1, 6));
        assert_eq!(
            model.editor_area.editors[&peer].cursors[0],
            Cursor::at(1, 9)
        );
        let after = Selection::from_anchor_head(Position::new(1, 14), Position::new(1, 9));
        assert_eq!(model.editor_area.editors[&peer].selections[0], after);
        assert_eq!(model.document().undo_stack.len(), 1);
        update(&mut model, Msg::Document(DocumentMsg::Undo));
        assert_eq!(model.document().buffer.to_string(), "va tail\n");
        assert_eq!(model.editor_area.editors[&peer].selections[0], before);
        update(&mut model, Msg::Document(DocumentMsg::Redo));
        assert_eq!(*model.editor().active_cursor(), Cursor::at(1, 6));
        assert_eq!(model.editor_area.editors[&peer].selections[0], after);
    }

    #[test]
    fn lsp_empty_prefix_and_equal_point_additions_keep_their_text_order() {
        let mut model = model_with_text("tail\n");
        let MenuInsert::Lsp(mut data) = lsp_item("value").insert else {
            unreachable!()
        };
        data.text = "value".into();
        let point = lsp_types::Position::new(0, 0);
        data.additional_text_edits = ["α", "β"]
            .into_iter()
            .map(|text| (lsp_types::Range::new(point, point), text.to_owned()))
            .collect();
        apply_lsp_accept(&mut model, &data, None);
        assert_eq!(model.document().buffer.to_string(), "αβvaluetail\n");
        assert_eq!(*model.editor().active_cursor(), Cursor::at(0, 7));
        update(&mut model, Msg::Document(DocumentMsg::Undo));
        assert_eq!(model.document().buffer.to_string(), "tail\n");
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
            context: Default::default(),
            selection_changed: false,
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
        let mut model = model_with_text("value_vector\n\n");
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
        let mut model = model_with_text("value_vector\n\n");
        model.config.completion.menu.words = WordsMode::Enabled;
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

    // ==== Docs resolve (documentation card) ====

    fn open_menu_with_resolvable_item(model: &mut AppModel, label: &str) {
        open_menu_with_lsp_response(model, &[label]);
        let state = model.ui.completion_menu.as_mut().unwrap();
        for item in &mut state.items {
            if let MenuInsert::Lsp(data) = &mut item.insert {
                data.can_resolve = true;
            }
        }
    }

    fn selected_lsp_data(model: &AppModel) -> LspInsert {
        let state = model.ui.completion_menu.as_ref().unwrap();
        let selected = model.ui.cursor_overlay.unwrap().selected;
        match &state.selected_item(selected).unwrap().insert {
            MenuInsert::Lsp(data) => (**data).clone(),
            MenuInsert::Text(_) => panic!("expected an LSP item"),
        }
    }

    fn cmd_contains(cmd: &Option<Cmd>, pred: impl Fn(&Cmd) -> bool) -> bool {
        fn contains(cmd: &Cmd, pred: &impl Fn(&Cmd) -> bool) -> bool {
            pred(cmd)
                || matches!(cmd, Cmd::Batch(cmds) if cmds.iter().any(|cmd| contains(cmd, pred)))
        }
        cmd.as_ref().is_some_and(|cmd| contains(cmd, &pred))
    }

    #[test]
    fn a_docs_resolution_without_a_pending_accept_merges_docs_and_marks_resolved() {
        let mut model = model_with_text("value_vector\n\n");
        open_menu_with_resolvable_item(&mut model, "valid_fn");
        select_item(&mut model, "valid_fn");
        let state = model.ui.completion_menu.clone().unwrap();
        assert_eq!(state.pending_resolve, None);
        let selected = model.ui.cursor_overlay.unwrap().selected;

        update(
            &mut model,
            Msg::Lsp(crate::messages::LspMsg::CompletionItemResolved {
                document_id: state.document_id,
                revision: state.revision,
                selected,
                detail: Some("fn valid_fn()".to_owned()),
                documentation: Some("Does the valid thing.".into()),
                additional_text_edits: vec![],
            }),
        );

        assert!(model.ui.completion_menu.is_some(), "menu stays open");
        assert_eq!(
            model.document().buffer.to_string(),
            "value_vector\nva\n",
            "a docs resolve must not touch the buffer"
        );
        let data = selected_lsp_data(&model);
        assert!(data.resolved);
        assert_eq!(
            data.documentation.as_ref().map(|t| t.text.as_str()),
            Some("Does the valid thing.")
        );
    }

    #[test]
    fn accept_after_a_docs_resolution_takes_the_fast_path() {
        let mut model = model_with_text("value_vector\n\n");
        open_menu_with_resolvable_item(&mut model, "valid_fn");
        select_item(&mut model, "valid_fn");
        let state = model.ui.completion_menu.clone().unwrap();
        let selected = model.ui.cursor_overlay.unwrap().selected;
        update(
            &mut model,
            Msg::Lsp(crate::messages::LspMsg::CompletionItemResolved {
                document_id: state.document_id,
                revision: state.revision,
                selected,
                detail: None,
                documentation: Some("docs".into()),
                additional_text_edits: vec![],
            }),
        );

        let cmd = update(&mut model, Msg::Completion(CompletionMsg::AcceptMenuItem));
        assert!(
            !cmd_contains(&cmd, |c| matches!(c, Cmd::LspResolveCompletionItem { .. })),
            "an already-resolved item must not resolve again: {cmd:?}"
        );
        assert!(model.ui.completion_menu.is_none(), "accept applied");
        let line = model.document().get_line_cow(1).unwrap();
        assert_eq!(line.trim_end_matches('\n'), "valid_fn");
    }

    #[test]
    fn moving_onto_an_unresolved_resolvable_item_schedules_a_docs_resolve() {
        let mut model = model_with_text("value_vector\n\n");
        open_menu_with_lsp_response(&mut model, &["valid_a", "valid_b"]);
        {
            let state = model.ui.completion_menu.as_mut().unwrap();
            for item in &mut state.items {
                if let MenuInsert::Lsp(data) = &mut item.insert {
                    data.can_resolve = true;
                    // `valid_b` already resolved; `valid_a` not yet.
                    data.resolved = item.label == "valid_b";
                }
            }
        }
        // Land on "valid_b" first so the next step moves onto "valid_a".
        select_item(&mut model, "valid_b");
        let onto_b = update(&mut model, Msg::Completion(CompletionMsg::MenuPrev));
        assert!(
            !selected_lsp_data(&model).resolved,
            "test setup: MenuPrev must land on the unresolved item"
        );
        assert!(
            cmd_contains(&onto_b, |c| matches!(
                c,
                Cmd::LspScheduleResolve { selected, .. }
                    if *selected == model.ui.cursor_overlay.unwrap().selected
            )),
            "unresolved item must schedule a docs resolve: {onto_b:?}"
        );

        let onto_resolved = update(&mut model, Msg::Completion(CompletionMsg::MenuNext));
        assert!(selected_lsp_data(&model).resolved);
        assert!(
            !cmd_contains(&onto_resolved, |c| matches!(
                c,
                Cmd::LspScheduleResolve { .. }
            )),
            "resolved item must not schedule: {onto_resolved:?}"
        );
    }

    // ---- signature help triggering ----

    /// A Rust file with mirrored signature triggers: `(`/`,` trigger,
    /// `)` retrigger.
    fn rust_completion_model(text: &str) -> AppModel {
        let mut model = model_with_text(text);
        model.document_mut().language = crate::syntax::LanguageId::Rust;
        model.document_mut().file_path = Some("/tmp/proj/build.rs".into());
        model
    }

    #[test]
    fn chained_member_completion_requests_lsp_without_local_noise() {
        for words in [WordsMode::Enabled, WordsMode::Fallback, WordsMode::Disabled] {
            let mut model = rust_completion_model(
                "// compile_garbage\ncc::Build::new()\n    .file(scanner)\n    .",
            );
            model.config.completion.menu.words = words;
            place_cursor(&mut model, 3, 5);
            let cmd = trigger_explicit(&mut model);
            assert!(cmd_contains(&cmd, |cmd| matches!(
                cmd,
                Cmd::LspScheduleCompletion { .. }
            )));
            let state = model.ui.completion_menu.as_ref().unwrap();
            assert_eq!(state.context, CompletionContext::Member);
            assert!(state.items.is_empty());
            assert!(model.ui.cursor_overlay.is_none());
            let (id, revision) = (state.document_id, state.revision);
            merge_lsp_completion(
                &mut model,
                id,
                revision,
                vec![lsp_item("compile"), lsp_item("ar_flag")],
                false,
            );
            assert!(model
                .ui
                .completion_menu
                .as_ref()
                .unwrap()
                .items
                .iter()
                .all(|item| item.source == MenuSourceId::Lsp));
            type_str(&mut model, "com");
            let state = model.ui.completion_menu.as_ref().unwrap();
            assert_eq!(state.context, CompletionContext::Member);
            assert_eq!(state.filtered.len(), 1);
            assert_eq!(state.selected_item(0).unwrap().label, "compile");
        }
    }

    #[test]
    fn no_local_match_still_requests_lsp_and_empty_response_stays_hidden() {
        let mut model = rust_completion_model("zz");
        place_cursor(&mut model, 0, 2);
        model.config.completion.menu.words = WordsMode::Disabled;
        let cmd = trigger_explicit(&mut model);
        assert!(cmd_contains(&cmd, |cmd| matches!(
            cmd,
            Cmd::LspScheduleCompletion { .. }
        )));
        let state = model.ui.completion_menu.as_ref().unwrap();
        let (id, revision) = (state.document_id, state.revision);
        merge_lsp_completion(&mut model, id, revision, vec![], false);
        assert!(model.ui.cursor_overlay.is_none());
        assert!(accept_selected(&mut model).is_none());
        dismiss_with_cleanup(&mut model);
        assert!(
            merge_lsp_completion(&mut model, id, revision, vec![lsp_item("zzz")], false).is_none()
        );
        assert!(model.ui.completion_menu.is_none());
    }

    #[test]
    fn fresh_parse_populates_waiting_local_session_without_reviving_dismissed_menu() {
        let mut model = rust_completion_model("fn main() { let value_real = 1; va }");
        place_cursor(&mut model, 0, 34);
        trigger_explicit(&mut model);
        assert!(model.ui.cursor_overlay.is_none());
        let doc = model.document();
        let id = doc.id.unwrap();
        let highlights = crate::syntax::ParserState::new().parse_and_highlight(
            &doc.buffer.to_string(),
            doc.language,
            id,
            doc.revision,
        );
        model.document_mut().syntax_highlights = Some(highlights);
        let cmd = refresh_after_syntax(&mut model, id);
        assert!(
            !cmd_contains(&cmd, |cmd| matches!(cmd, Cmd::LspScheduleCompletion { .. })),
            "a parse must not restart the LSP debounce"
        );
        let state = model.ui.completion_menu.as_ref().unwrap();
        assert_eq!(state.query, "va");
        assert_eq!(state.selected_item(0).unwrap().label, "value_real");
        dismiss(&mut model);
        assert!(refresh_after_syntax(&mut model, id).is_none());
        assert!(model.ui.completion_menu.is_none());
    }

    #[test]
    fn preferred_item_is_selected_until_the_user_navigates() {
        let mut model = rust_completion_model("builder.");
        place_cursor(&mut model, 0, 8);
        trigger_explicit(&mut model);
        let doc = model.document();
        let (id, revision) = (doc.id.unwrap(), doc.revision);
        let items = || {
            let mut preferred = lsp_item("compile");
            preferred.preselect = true;
            vec![lsp_item("ar_flag"), preferred]
        };
        merge_lsp_completion(&mut model, id, revision, items(), false);
        assert_eq!(model.ui.cursor_overlay.unwrap().selected, 1);
        move_selection(&mut model, -1);
        merge_lsp_completion(&mut model, id, revision, items(), false);
        assert_eq!(model.ui.cursor_overlay.unwrap().selected, 0);
        trigger_explicit(&mut model);
        assert_eq!(model.ui.cursor_overlay.unwrap().selected, 0);
    }

    #[test]
    fn late_completion_for_a_moved_caret_is_ignored() {
        let mut model = rust_completion_model("builder.");
        place_cursor(&mut model, 0, 8);
        trigger_explicit(&mut model);
        let doc = model.document();
        let (id, revision) = (doc.id.unwrap(), doc.revision);
        place_cursor(&mut model, 0, 0);
        assert!(
            merge_lsp_completion(&mut model, id, revision, vec![lsp_item("compile")], false)
                .is_none()
        );
        assert!(model.ui.cursor_overlay.is_none());
    }

    fn signature_model() -> AppModel {
        use crate::lsp::LspServerId;
        use crate::syntax::LanguageId;

        let mut model = model_with_text("\n");
        model.document_mut().language = LanguageId::Rust;
        model.document_mut().file_path = Some(std::path::PathBuf::from("/tmp/proj/lib.rs"));
        model.lsp.signature_trigger_characters.insert(
            LspServerId::from("rust-analyzer"),
            (vec!["(".to_owned(), ",".to_owned()], vec![")".to_owned()]),
        );
        place_cursor(&mut model, 0, 0);
        model
    }

    fn find_signature_request(cmd: &Cmd) -> Option<(Option<String>, bool)> {
        match cmd {
            Cmd::LspRequestSignatureHelp {
                trigger,
                is_retrigger,
                ..
            } => Some((trigger.clone(), *is_retrigger)),
            Cmd::Batch(cmds) => cmds.iter().find_map(find_signature_request),
            _ => None,
        }
    }

    fn type_char_signature_request(
        model: &mut AppModel,
        ch: char,
    ) -> Option<(Option<String>, bool)> {
        update(model, Msg::Document(DocumentMsg::InsertChar(ch)))
            .as_ref()
            .and_then(find_signature_request)
    }

    #[test]
    fn typing_a_signature_trigger_character_requests_signature_help() {
        let mut model = signature_model();
        assert_eq!(type_char_signature_request(&mut model, 'f'), None);
        assert_eq!(
            type_char_signature_request(&mut model, '('),
            Some((Some("(".to_owned()), false))
        );
    }

    #[test]
    fn completion_menu_config_auto_disabled_preserves_signature_help_and_manual_members() {
        let mut model = signature_model();
        model.config.completion.menu.enabled = false;
        model.lsp.completion_trigger_characters.insert(
            crate::lsp::LspServerId::from("rust-analyzer"),
            vec![".".into()],
        );
        type_str(&mut model, "builder.");
        assert!(model.ui.completion_menu.is_none());
        update(&mut model, Msg::Completion(CompletionMsg::TriggerMenu));
        assert!(model.ui.completion_menu.is_some());
        assert!(model
            .ui
            .completion_menu
            .as_ref()
            .unwrap()
            .items
            .iter()
            .all(|item| item.source == MenuSourceId::Lsp));
        update(&mut model, Msg::Completion(CompletionMsg::Dismiss));
        assert_eq!(
            type_char_signature_request(&mut model, '('),
            Some((Some("(".to_owned()), false))
        );
        assert!(model.ui.completion_menu.is_none());
    }

    #[test]
    fn typing_while_signature_help_is_open_re_requests_as_a_retrigger() {
        let mut model = signature_model();
        model.ui.signature_help = Some(crate::model::SignatureHelpState {
            signatures: vec![],
            active: 0,
        });
        assert_eq!(
            type_char_signature_request(&mut model, 'x'),
            Some((None, true))
        );
        assert_eq!(
            type_char_signature_request(&mut model, ')'),
            Some((Some(")".to_owned()), true)),
            "a retrigger character is tagged while open"
        );
    }

    #[test]
    fn typing_a_trigger_character_with_no_server_triggers_requests_nothing() {
        let mut model = signature_model();
        model.lsp.signature_trigger_characters.clear();
        assert_eq!(type_char_signature_request(&mut model, '('), None);
    }
}
