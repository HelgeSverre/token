//! Menu completion update logic (autocomplete.md Phase 1).
//!
//! Two entry points:
//! - [`update_completion`] handles `Msg::Completion` (explicit trigger,
//!   navigation, accept, dismiss).
//! - [`sync_after_document_edit`] is called after every text-mutating
//!   `DocumentMsg` to open/refresh/dismiss the menu as the user types —
//!   the "typed char -> should_trigger?" flow in the doc's data-flow
//!   diagram.
//!
//! No worker, no debounce: both sources are sub-millisecond rope scans, so
//! every collection happens synchronously inline (autocomplete.md: "No
//! worker and no debounce for v1 menu sources").

use crate::commands::Cmd;
use crate::completion::menu::{filter_and_sort, CompletionMenuState};
use crate::completion::sources::{collect_snippets, collect_words};
use crate::messages::CompletionMsg;
use crate::model::{
    AppModel, Cursor, CursorOverlayKind, CursorOverlayState, EditOperation, Selection,
};
use crate::util::text::{char_type, CharType};
use crate::view::overlay_surface::MAX_VISIBLE_COMPLETION;

use super::document::word_start_before;
use super::editor::cursors_in_reverse_order;
use super::schedule_syntax_parse;

pub fn update_completion(model: &mut AppModel, msg: CompletionMsg) -> Option<Cmd> {
    match msg {
        CompletionMsg::TriggerMenu => trigger_explicit(model),
        CompletionMsg::MenuNext => move_selection(model, 1),
        CompletionMsg::MenuPrev => move_selection(model, -1),
        CompletionMsg::AcceptMenuItem => accept_selected(model),
        CompletionMsg::Dismiss => {
            dismiss(model);
            Some(Cmd::Redraw)
        }
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

/// Ctrl+Space (or any other explicit-trigger binding): open with whatever
/// query is at the cursor, including an empty one (word chars aren't
/// required — autocomplete.md: "Ctrl+Space always works").
fn trigger_explicit(model: &mut AppModel) -> Option<Cmd> {
    if !model.editor().is_plain_text_mode() {
        return Some(Cmd::Redraw);
    }
    let cursor = *model.editor().active_cursor();
    let doc = model.document();
    let offset = doc.cursor_to_offset(cursor.line, cursor.column);
    let query_start_offset = word_query_offsets(model, cursor)
        .map(|(start, _)| start)
        .unwrap_or(offset);
    open_or_refresh(model, cursor, query_start_offset, offset);
    Some(Cmd::Redraw)
}

/// Called after every text-mutating `DocumentMsg`. Opens the menu on a
/// word-char `InsertChar`, refreshes it while already open, and dismisses it
/// once the cursor is no longer preceded by a word char.
///
/// Takes the two facts about the originating message the sync needs,
/// rather than the message itself — the caller reads them before the
/// message is moved into `update_document`, avoiding a clone of the whole
/// `DocumentMsg` (which, for `InsertText`, would copy a full paste/IME
/// payload) just to inspect its shape here.
pub(crate) fn sync_after_document_edit(
    model: &mut AppModel,
    is_copy: bool,
    opens_on_word_char: bool,
) -> Option<Cmd> {
    if is_copy {
        return None;
    }
    if !model.editor().is_plain_text_mode() {
        dismiss(model);
        return None;
    }

    let cursor = *model.editor().active_cursor();
    let Some((query_start_offset, cursor_offset)) = word_query_offsets(model, cursor) else {
        let was_open = model.ui.completion_menu.is_some();
        dismiss(model);
        return was_open.then_some(Cmd::Redraw);
    };

    let menu_open = model.ui.completion_menu.is_some();
    let should_open = menu_open || opens_on_word_char;
    if !should_open {
        return None;
    }
    open_or_refresh(model, cursor, query_start_offset, cursor_offset);
    Some(Cmd::Redraw)
}

/// Collect words + snippets, filter/sort against the query, and either set
/// (or replace) `completion_menu`/`cursor_overlay`, or dismiss if nothing
/// matched.
///
/// ponytail: always recollects rather than caching items across keystrokes
/// and refiltering-only when the word boundary hasn't changed (the
/// optimization autocomplete.md sketches for the sync case). Both sources
/// are bounded rope scans the doc itself measures at sub-millisecond; add
/// the cache if profiling ever shows otherwise.
fn open_or_refresh(
    model: &mut AppModel,
    cursor: Cursor,
    query_start_offset: usize,
    cursor_offset: usize,
) {
    let doc = model.document();
    let query: String = doc
        .buffer
        .slice(query_start_offset..cursor_offset)
        .chars()
        .collect();

    let mut items = collect_words(doc, cursor, &query);
    items.extend(collect_snippets(doc.language));
    let filtered = filter_and_sort(&items, &query);

    if filtered.is_empty() {
        dismiss(model);
        return;
    }

    // A document without an id (e.g. an unregistered scratch buffer) can
    // never be re-matched by `accept_selected`'s id/revision guard, so
    // there's nothing safe to open the menu against — no-op rather than
    // panic on what would otherwise be the hot typing path.
    let Some(document_id) = doc.id else {
        return;
    };
    let (query_line, query_col) = doc.offset_to_cursor(query_start_offset);
    let revision = doc.revision;

    model.ui.completion_menu = Some(CompletionMenuState {
        document_id,
        revision,
        query_start: Cursor::at(query_line, query_col),
        items,
        filtered,
    });
    model.ui.cursor_overlay = Some(CursorOverlayState::new(CursorOverlayKind::Completion));
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
    state.selected = if delta > 0 {
        (state.selected + 1) % total
    } else {
        (state.selected + total - 1) % total
    };
    if state.selected < state.scroll {
        state.scroll = state.selected;
    } else if state.selected >= state.scroll + MAX_VISIBLE_COMPLETION {
        state.scroll = state.selected + 1 - MAX_VISIBLE_COMPLETION;
    }
    Some(Cmd::Redraw)
}

/// Accept the selected item at every cursor: each cursor's own
/// `[word_start_before(cursor)..cursor)` range is replaced with the same
/// `insert_text`, as one `EditOperation::Batch` (one undo step,
/// multi-cursor-correct). Guarded by the revision snapshot the menu was
/// built against — a stale menu (document changed underneath it without
/// going through `sync_after_document_edit`, e.g. `Undo`/`Redo` racing an
/// explicit trigger) is dropped instead of misapplied.
fn accept_selected(model: &mut AppModel) -> Option<Cmd> {
    let state = model.ui.completion_menu.as_ref()?;
    let selected = model.ui.cursor_overlay?.selected;
    let doc_id_matches = model.document().id == Some(state.document_id);
    let revision_matches = model.document().revision == state.revision;
    if !doc_id_matches || !revision_matches {
        dismiss(model);
        return Some(Cmd::Redraw);
    }
    let Some(item) = state.selected_item(selected) else {
        dismiss(model);
        return Some(Cmd::Redraw);
    };
    let insert_text = item.insert_text.clone();

    let cursors_before: Vec<Cursor> = model.editor().cursors.clone();
    let indices = cursors_in_reverse_order(model);
    let mut operations = Vec::new();

    for idx in indices {
        let cursor = model.editor().cursors[idx];
        let Some((start_offset, cursor_offset)) = word_query_offsets(model, cursor) else {
            continue;
        };
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
            .insert(start_offset, &insert_text);

        let new_offset = start_offset + insert_text.chars().count();
        let (new_line, new_col) = model.document().offset_to_cursor(new_offset);
        operations.push(EditOperation::Replace {
            position: start_offset,
            deleted_text,
            inserted_text: insert_text.clone(),
            cursor_before: cursor,
            cursor_after: Cursor::at(new_line, new_col),
        });

        model.editor_mut().cursors[idx] = Cursor::at(new_line, new_col);
        model.editor_mut().cursors[idx].desired_column = None;
        let new_pos = model.editor().cursors[idx].to_position();
        model.editor_mut().selections[idx] = Selection::new(new_pos);
    }

    dismiss(model);

    if operations.is_empty() {
        return Some(Cmd::Redraw);
    }

    let cursors_after: Vec<Cursor> = model.editor().cursors.clone();
    model.document_mut().push_edit(EditOperation::Batch {
        operations,
        cursors_before,
        cursors_after,
    });
    model.document_mut().is_modified = true;
    model.ensure_cursor_visible();

    if let Some(doc_id) = model.document().id {
        if let Some(parse_cmd) = schedule_syntax_parse(model, doc_id) {
            return Some(Cmd::Batch(vec![Cmd::redraw_editor(), parse_cmd]));
        }
    }
    Some(Cmd::redraw_editor())
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
}
