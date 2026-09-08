//! Contracts for shared derived state and deterministic configuration effects.
mod common;

use std::sync::Arc;
use token::commands::Cmd;
use token::messages::{AppMsg, Msg};
use token::model::{Document, FindReplaceState};
use token::update::update;

fn completion_model(text: &str, cursors: Vec<token::model::Cursor>) -> token::model::AppModel {
    let mut model = token::model::AppModel::new(800, 600, 1.0);
    model.document_mut().buffer = text.into();
    model.editor_mut().selections = cursors
        .iter()
        .map(|cursor| token::model::Selection::new(cursor.to_position()))
        .collect();
    model.editor_mut().cursors = cursors;
    model
}

#[test]
fn movement_target_matrix_preserves_positions_and_selection_policy() {
    use token::messages::{Direction, EditorMsg};
    use token::model::Position;
    let cases = [
        (
            EditorMsg::MoveCursor(Direction::Left),
            EditorMsg::MoveCursorWithSelection(Direction::Left),
            (2, 6),
        ),
        (
            EditorMsg::MoveCursor(Direction::Right),
            EditorMsg::MoveCursorWithSelection(Direction::Right),
            (2, 8),
        ),
        (
            EditorMsg::MoveCursor(Direction::Up),
            EditorMsg::MoveCursorWithSelection(Direction::Up),
            (1, 2),
        ),
        (
            EditorMsg::MoveCursor(Direction::Down),
            EditorMsg::MoveCursorWithSelection(Direction::Down),
            (3, 4),
        ),
        (
            EditorMsg::MoveCursorLineStart,
            EditorMsg::MoveCursorLineStartWithSelection,
            (2, 2),
        ),
        (
            EditorMsg::MoveCursorLineEnd,
            EditorMsg::MoveCursorLineEndWithSelection,
            (2, 13),
        ),
        (
            EditorMsg::MoveCursorDocumentStart,
            EditorMsg::MoveCursorDocumentStartWithSelection,
            (0, 0),
        ),
        (
            EditorMsg::MoveCursorDocumentEnd,
            EditorMsg::MoveCursorDocumentEndWithSelection,
            (3, 4),
        ),
        (
            EditorMsg::MoveCursorWord(Direction::Left),
            EditorMsg::MoveCursorWordWithSelection(Direction::Left),
            (2, 2),
        ),
        (
            EditorMsg::MoveCursorWord(Direction::Right),
            EditorMsg::MoveCursorWordWithSelection(Direction::Right),
            (2, 8),
        ),
        (EditorMsg::PageUp, EditorMsg::PageUpWithSelection, (0, 7)),
        (
            EditorMsg::PageDown,
            EditorMsg::PageDownWithSelection,
            (3, 4),
        ),
    ];
    let text = "  alpha beta\nxy\n  gamma delta\nlast";
    for (movement, extending, expected) in cases {
        for (message, extend) in [(movement, false), (extending, true)] {
            let mut model = common::test_model(text, 2, 7);
            model.ui.cursor_visible = false;
            let name = format!("{message:?}");
            assert!(update(&mut model, Msg::Editor(message)).is_some(), "{name}");
            let position = Position::new(expected.0, expected.1);
            assert_eq!(
                model.editor().active_cursor().to_position(),
                position,
                "{name}"
            );
            let selection = model.editor().active_selection();
            assert_eq!(selection.head, position, "{name}");
            assert_eq!(
                selection.anchor,
                if extend {
                    Position::new(2, 7)
                } else {
                    position
                },
                "{name}"
            );
            assert!(model.ui.cursor_visible, "{name}");
            assert_eq!(model.document().buffer.to_string(), text);
        }
    }
}

#[test]
fn movement_horizontal_collapse_and_noop_word_direction_keep_legacy_policy() {
    use token::messages::{Direction, EditorMsg};
    use token::model::{Cursor, Position, Selection};
    for (direction, column) in [(Direction::Left, 2), (Direction::Right, 7)] {
        for reversed in [false, true] {
            let mut model = common::test_model("alpha beta", 0, if reversed { 2 } else { 7 });
            let (anchor, head) = if reversed { (7, 2) } else { (2, 7) };
            model.editor_mut().selections[0] =
                Selection::from_positions(Position::new(0, anchor), Position::new(0, head));
            update(&mut model, Msg::Editor(EditorMsg::MoveCursor(direction)));
            assert_eq!(model.editor().cursors[0], Cursor::at(0, column));
            assert!(model.editor().selections[0].is_empty());
        }
    }
    let mut model = common::test_model("alpha beta", 0, 7);
    let selected = Selection::from_positions(Position::new(0, 2), Position::new(0, 7));
    model.editor_mut().selections[0] = selected;
    update(
        &mut model,
        Msg::Editor(EditorMsg::MoveCursorWordWithSelection(Direction::Up)),
    );
    assert_eq!(model.editor().selections[0], selected);
    update(
        &mut model,
        Msg::Editor(EditorMsg::MoveCursorWord(Direction::Down)),
    );
    assert_eq!(model.editor().cursors[0], Cursor::at(0, 7));
    assert!(model.editor().selections[0].is_empty());
}

#[test]
fn movement_dispatch_preserves_wrapped_desired_column_through_short_rows() {
    use token::messages::{Direction, EditorMsg};
    use token::model::Position;
    let mut model = common::test_model("abcdefghijklmnopqrst\nx\nabcdefghijklmnopqrst", 0, 7);
    model.editor_mut().viewport.visible_columns = 4;
    model.editor_mut().soft_wrap = true;
    for expected in [(0, 11), (0, 15), (0, 19), (1, 1), (2, 3)] {
        update(
            &mut model,
            Msg::Editor(EditorMsg::MoveCursorWithSelection(Direction::Down)),
        );
        assert_eq!(
            model.editor().active_cursor().to_position(),
            Position::new(expected.0, expected.1)
        );
        assert_eq!(
            model.editor().active_selection().anchor,
            Position::new(0, 7)
        );
    }
    update(
        &mut model,
        Msg::Editor(EditorMsg::MoveCursor(Direction::Up)),
    );
    assert_eq!(
        model.editor().active_cursor().to_position(),
        Position::new(1, 1)
    );
    update(
        &mut model,
        Msg::Editor(EditorMsg::MoveCursor(Direction::Up)),
    );
    assert_eq!(
        model.editor().active_cursor().to_position(),
        Position::new(0, 19)
    );
    assert!(model.editor().active_selection().is_empty());
}

fn accept_word(model: &mut token::model::AppModel) {
    use token::messages::CompletionMsg;
    update(model, Msg::Completion(CompletionMsg::TriggerMenu));
    assert!(model.ui.completion_menu.is_some());
    update(model, Msg::Completion(CompletionMsg::AcceptMenuItem));
}

#[test]
fn completion_reconciles_same_line_cursors_and_undo_redo() {
    use token::messages::DocumentMsg;
    use token::model::Cursor;
    let original = "valueA\nval val\n";
    let before = vec![Cursor::at(1, 3), Cursor::at(1, 7)];
    let after = vec![Cursor::at(1, 6), Cursor::at(1, 13)];
    let mut model = completion_model(original, before.clone());
    model.editor_mut().active_cursor_index = 1;
    accept_word(&mut model);
    assert_eq!(
        model.document().buffer.to_string(),
        "valueA\nvalueA valueA\n"
    );
    assert_eq!(model.editor().cursors, after);
    assert_eq!(model.editor().active_cursor_index, 1);
    assert_eq!(model.document().undo_stack.len(), 1);
    update(&mut model, Msg::Document(DocumentMsg::Undo));
    assert_eq!(model.document().buffer.to_string(), original);
    assert_eq!(model.editor().cursors, before);
    update(&mut model, Msg::Document(DocumentMsg::Redo));
    assert_eq!(model.editor().cursors, after);
}

#[test]
fn completion_reconciles_peer_cursor_and_reversed_selection_through_history() {
    use token::messages::{DocumentMsg, LayoutMsg};
    use token::model::{Cursor, Position, Selection, SplitDirection};
    let mut model = completion_model("val\nvalueA\n", vec![Cursor::at(0, 3)]);
    let peer = model.editor().id.unwrap();
    let before = Selection::from_anchor_head(Position::new(1, 6), Position::new(0, 3));
    model.editor_mut().selections[0] = before;
    update(
        &mut model,
        Msg::Layout(LayoutMsg::SplitFocused(SplitDirection::Horizontal)),
    );
    model.editor_mut().cursors = vec![Cursor::at(0, 3)];
    model.editor_mut().selections = vec![Selection::new(Position::new(0, 3))];
    accept_word(&mut model);
    let after = Selection::from_anchor_head(Position::new(1, 6), Position::new(0, 6));
    assert_eq!(
        model.editor_area.editors[&peer].cursors[0],
        Cursor::at(0, 6)
    );
    assert_eq!(model.editor_area.editors[&peer].selections[0], after);
    update(&mut model, Msg::Document(DocumentMsg::Undo));
    assert_eq!(
        model.editor_area.editors[&peer].cursors[0],
        Cursor::at(0, 3)
    );
    assert_eq!(model.editor_area.editors[&peer].selections[0], before);
    update(&mut model, Msg::Document(DocumentMsg::Redo));
    assert_eq!(
        model.editor_area.editors[&peer].cursors[0],
        Cursor::at(0, 6)
    );
    assert_eq!(model.editor_area.editors[&peer].selections[0], after);
}

#[test]
fn completion_reconciles_unicode_prefixes_in_unsorted_cursor_order() {
    use token::model::Cursor;
    let mut model = completion_model(
        "välueA\n🙂 väl väl\n",
        vec![Cursor::at(1, 9), Cursor::at(1, 5)],
    );
    accept_word(&mut model);
    assert_eq!(
        model.document().buffer.to_string(),
        "välueA\n🙂 välueA välueA\n"
    );
    assert_eq!(
        model.editor().cursors,
        vec![Cursor::at(1, 15), Cursor::at(1, 8)]
    );
}

#[test]
fn completion_reconciles_overlapping_prefixes_once_and_deduplicates_carets() {
    use token::messages::DocumentMsg;
    use token::model::Cursor;
    let original = "valueA\nval\n";
    let mut model = completion_model(original, vec![Cursor::at(1, 2), Cursor::at(1, 3)]);
    model.editor_mut().active_cursor_index = 1;
    accept_word(&mut model);
    assert_eq!(model.document().buffer.to_string(), "valueA\nvalueA\n");
    assert_eq!(model.editor().cursors, vec![Cursor::at(1, 6)]);
    assert_eq!(model.editor().active_cursor_index, 0);
    update(&mut model, Msg::Document(DocumentMsg::Undo));
    assert_eq!(model.document().buffer.to_string(), original);
    assert_eq!(
        model.editor().cursors,
        vec![Cursor::at(1, 2), Cursor::at(1, 3)]
    );
    update(&mut model, Msg::Document(DocumentMsg::Redo));
    update(&mut model, Msg::Document(DocumentMsg::InsertChar('!')));
    assert_eq!(model.document().buffer.to_string(), "valueA\nvalueA!\n");
}

#[test]
fn completion_reconciles_an_empty_prefix_at_an_adjacent_replacement_start() {
    use token::messages::DocumentMsg;
    use token::model::Cursor;
    let mut model = completion_model("valueA\nval\n", vec![Cursor::at(1, 0), Cursor::at(1, 3)]);
    model.editor_mut().active_cursor_index = 1;
    accept_word(&mut model);
    // An explicit empty-prefix caret inserts independently at the boundary;
    // it is not an overlapping replacement and must retain its own endpoint.
    assert_eq!(
        model.document().buffer.to_string(),
        "valueA\nvalueAvalueA\n"
    );
    assert_eq!(
        model.editor().cursors,
        vec![Cursor::at(1, 6), Cursor::at(1, 12)]
    );
    update(&mut model, Msg::Document(DocumentMsg::Undo));
    assert_eq!(model.document().buffer.to_string(), "valueA\nval\n");
}

#[test]
fn find_cache_reuses_results_and_invalidates_all_search_inputs() {
    let mut doc = Document::with_text("foo Foo foobar\nfoo");
    let mut state = FindReplaceState::default();
    state.set_query("foo");
    let first = state.matches(&doc);
    assert_eq!(first.len(), 4);
    assert!(Arc::ptr_eq(&first, &state.matches(&doc)));
    state.case_sensitive = true;
    assert_eq!(state.matches(&doc).len(), 3);
    state.whole_word = true;
    assert_eq!(state.matches(&doc).len(), 2);
    state.selection_only = true;
    state.scope = Some((0, 3));
    assert_eq!(state.matches(&doc).len(), 1);
    state.scope = Some((0, 0));
    assert!(state.matches(&doc).is_empty());
    state.selection_only = false;
    state.set_query("f.o");
    assert!(state.matches(&doc).is_empty());
    state.use_regex = true;
    assert_eq!(state.matches(&doc).len(), 2);
    // Reload/direct mutation may leave the revision unchanged.
    doc.buffer = ropey::Rope::from_str("féo");
    let reloaded = state.matches(&doc);
    assert_eq!(reloaded[0], token::search::Match { start: 0, end: 3 });
    doc.revision += 1;
    assert!(!Arc::ptr_eq(&reloaded, &state.matches(&doc)));
    doc.buffer.remove(0..3);
    assert!(state.matches(&doc).is_empty());
}

#[test]
fn find_cache_retains_regex_errors_and_status_reuses_results() {
    let doc = Document::with_text("foo foo");
    let mut state = FindReplaceState::default();
    state.use_regex = true;
    state.set_query("[");
    let matches = state.matches(&doc);
    assert!(state.status(&doc, &Default::default()).unwrap().is_error());
    assert!(Arc::ptr_eq(&matches, &state.matches(&doc)));
    state.set_query("foo");
    let matches = state.matches(&doc);
    assert!(!state.status(&doc, &Default::default()).unwrap().is_error());
    assert!(Arc::ptr_eq(&matches, &state.matches(&doc)));
}

#[test]
fn configuration_effects_apply_only_runtime_results_and_report_save_errors() {
    let mut model = common::test_model("hello", 0, 0);
    let original = model.config.theme.clone();
    assert!(matches!(
        update(&mut model, Msg::App(AppMsg::ReloadConfiguration)),
        Some(Cmd::ReloadConfiguration)
    ));
    assert_eq!(model.config.theme, original);
    let command = update(
        &mut model,
        Msg::App(AppMsg::ConfigurationSaved(Err("read-only".into()))),
    );
    assert!(command.is_some());
    assert!(update(&mut model, Msg::App(AppMsg::ConfigurationSaved(Ok(())))).is_none());
}
