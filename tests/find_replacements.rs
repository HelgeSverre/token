//! Find mutations must use the same history and position policy as ordinary edits.
mod common;

use token::messages::{DocumentMsg, LayoutMsg, ModalMsg, Msg, UiMsg};
use token::model::{AppModel, Cursor, FindReplaceState, ModalState, Position, Selection};
use token::update::update;

fn open_find(model: &mut AppModel, query: &str, replacement: &str, scoped: bool) {
    let mut state = FindReplaceState::default();
    state.set_query(query);
    state.set_replacement(replacement);
    state.case_sensitive = true;
    state.set_selection_only(scoped, model.document(), &model.editor().selections[0]);
    model.ui.open_modal(ModalState::FindReplace(state));
}

fn act(model: &mut AppModel, message: ModalMsg) {
    update(model, Msg::Ui(UiMsg::Modal(message)));
}

fn scope(model: &AppModel) -> Option<(usize, usize)> {
    match model.ui.active_modal.as_ref().unwrap() {
        ModalState::FindReplace(state) => state.scope,
        _ => panic!("expected Find modal"),
    }
}

#[test]
fn find_replace_all_maps_unicode_multiline_carets_and_undo() {
    let text = "header\n  foo!\nfoo tail";
    let mut model = common::test_model(text, 2, 8);
    let peer_id = model.editor_area.focused_editor_id().unwrap();
    model.editor_mut().selections[0] =
        Selection::from_anchor_head(Position::new(2, 8), Position::new(2, 4));
    model.editor_mut().cursors[0] = Cursor::at(2, 4);
    update(
        &mut model,
        Msg::Layout(LayoutMsg::SplitFocused(
            token::model::editor_area::SplitDirection::Vertical,
        )),
    );
    open_find(&mut model, "foo", "🙂\nx", false);
    act(&mut model, ModalMsg::ReplaceAll);
    assert_eq!(
        model.document().buffer.to_string(),
        "header\n  🙂\nx!\n🙂\nx tail"
    );
    assert_eq!(model.editor().cursors[0], Cursor::at(2, 1));
    assert_eq!(model.document().undo_stack.len(), 1);
    let peer = &model.editor_area.editors[&peer_id];
    assert_eq!(peer.cursors[0], Cursor::at(4, 2));
    assert_eq!(
        peer.selections[0],
        Selection::from_anchor_head(Position::new(4, 6), Position::new(4, 2),)
    );
    update(&mut model, Msg::Document(DocumentMsg::Undo));
    assert_eq!(model.document().buffer.to_string(), text);
    assert_eq!(
        model.editor_area.editors[&peer_id].cursors[0],
        Cursor::at(2, 4)
    );
    update(&mut model, Msg::Document(DocumentMsg::Redo));
    assert_eq!(
        model.document().buffer.to_string(),
        "header\n  🙂\nx!\n🙂\nx tail"
    );
    assert_eq!(model.editor().cursors[0], Cursor::at(2, 1));
}

#[test]
fn find_replace_next_includes_adjacent_match_and_is_undoable() {
    let mut model = common::test_model_with_selection("foofoofoo", 0, 0, 0, 3);
    open_find(&mut model, "foo", "x", false);
    act(&mut model, ModalMsg::ReplaceAndFindNext);
    assert_eq!(model.document().buffer.to_string(), "xfoofoo");
    assert_eq!(
        model.editor().selections[0],
        Selection::from_anchor_head(Position::new(0, 1), Position::new(0, 4),)
    );
    assert_eq!(model.document().undo_stack.len(), 1);
    update(&mut model, Msg::Document(DocumentMsg::Undo));
    assert_eq!(model.document().buffer.to_string(), "foofoofoo");
    update(&mut model, Msg::Document(DocumentMsg::Redo));
    assert_eq!(model.document().buffer.to_string(), "xfoofoo");
}

#[test]
fn find_replace_scope_tracks_shortening_and_history_without_leaking() {
    let mut model = common::test_model_with_selection("foo foo foo", 0, 0, 0, 7);
    open_find(&mut model, "foo", "x", true);
    act(&mut model, ModalMsg::FindNext);
    act(&mut model, ModalMsg::ReplaceAndFindNext);
    assert_eq!(model.document().buffer.to_string(), "x foo foo");
    assert_eq!(scope(&model), Some((0, 5)));
    act(&mut model, ModalMsg::ReplaceAndFindNext);
    assert_eq!(model.document().buffer.to_string(), "x x foo");
    assert_eq!(scope(&model), Some((0, 3)));
    act(&mut model, ModalMsg::ReplaceAndFindNext);
    assert_eq!(model.document().buffer.to_string(), "x x foo");
    assert_eq!(model.document().undo_stack.len(), 2);
    update(&mut model, Msg::Document(DocumentMsg::Undo));
    assert_eq!(scope(&model), Some((0, 5)));
    update(&mut model, Msg::Document(DocumentMsg::Redo));
    assert_eq!(scope(&model), Some((0, 3)));
}

#[test]
fn find_replace_scope_tracks_growth_and_zero_width_boundary_insertions() {
    let mut model = common::test_model_with_selection("foo foo foo", 0, 0, 0, 7);
    open_find(&mut model, "foo", "longer", true);
    act(&mut model, ModalMsg::FindNext);
    act(&mut model, ModalMsg::ReplaceAndFindNext);
    assert_eq!(scope(&model), Some((0, 10)));
    act(&mut model, ModalMsg::ReplaceAndFindNext);
    assert_eq!(model.document().buffer.to_string(), "longer longer foo");
    assert_eq!(scope(&model), Some((0, 13)));

    let mut model = common::test_model_with_selection("foo\noutside", 0, 0, 0, 3);
    open_find(&mut model, "\\b", "_", true);
    if let Some(ModalState::FindReplace(state)) = &mut model.ui.active_modal {
        state.use_regex = true;
    }
    act(&mut model, ModalMsg::ReplaceAll);
    assert_eq!(model.document().buffer.to_string(), "_foo_\noutside");
    assert_eq!(scope(&model), Some((0, 5)));
    update(&mut model, Msg::Document(DocumentMsg::Undo));
    assert_eq!(model.document().buffer.to_string(), "foo\noutside");
    assert_eq!(scope(&model), Some((0, 3)));
}

#[test]
fn find_replace_noop_keeps_dirty_revision_and_redo_history() {
    let mut model = common::test_model("foo", 0, 3);
    update(&mut model, Msg::Document(DocumentMsg::InsertChar('!')));
    update(&mut model, Msg::Document(DocumentMsg::Undo));
    let revision = model.document().revision;
    let modified = model.document().is_modified;
    for query in ["foo", "absent"] {
        open_find(&mut model, query, "foo", false);
        act(&mut model, ModalMsg::ReplaceAll);
        assert_eq!(model.document().revision, revision);
        assert_eq!(model.document().is_modified, modified);
        assert!(model.document().undo_stack.is_empty());
        assert_eq!(model.document().redo_stack.len(), 1);
    }
    model.editor_mut().selections[0] =
        Selection::from_anchor_head(Position::new(0, 0), Position::new(0, 3));
    open_find(&mut model, "foo", "foo", false);
    act(&mut model, ModalMsg::ReplaceAndFindNext);
    assert_eq!(model.document().revision, revision);
    assert_eq!(model.document().redo_stack.len(), 1);
}

#[test]
fn find_replace_maps_secondary_carets_without_collapsing_their_selections() {
    let mut model = common::test_model_multi_cursor("header\nfoo end", &[(0, 0), (1, 7)]);
    model.editor_mut().active_cursor_index = 1;
    model.editor_mut().selections[1] =
        Selection::from_anchor_head(Position::new(1, 4), Position::new(1, 7));
    open_find(&mut model, "foo", "x", false);
    act(&mut model, ModalMsg::ReplaceAll);
    assert_eq!(
        model.editor().cursors,
        vec![Cursor::at(1, 1), Cursor::at(1, 5)]
    );
    assert_eq!(model.editor().active_cursor_index, 1);
    assert_eq!(
        model.editor().selections[1],
        Selection::from_anchor_head(Position::new(1, 2), Position::new(1, 5),)
    );
}

#[test]
fn find_replace_empty_scope_survives_delete_and_undo() {
    let mut model = common::test_model_with_selection("foo outside", 0, 0, 0, 3);
    open_find(&mut model, "foo", "", true);
    act(&mut model, ModalMsg::ReplaceAll);
    assert_eq!(model.document().buffer.to_string(), " outside");
    assert_eq!(scope(&model), Some((0, 0)));
    update(&mut model, Msg::Document(DocumentMsg::Undo));
    assert_eq!(scope(&model), Some((0, 3)));
    update(&mut model, Msg::Document(DocumentMsg::Redo));
    assert_eq!(scope(&model), Some((0, 0)));

    // The same scope policy applies to ordinary commands, not only Find's planner.
    update(&mut model, Msg::Document(DocumentMsg::Undo));
    model.editor_mut().cursors[0] = Cursor::at(0, 3);
    model.editor_mut().selections[0] =
        Selection::from_anchor_head(Position::new(0, 0), Position::new(0, 3));
    update(&mut model, Msg::Document(DocumentMsg::DeleteBackward));
    assert_eq!(scope(&model), Some((0, 0)));
    update(&mut model, Msg::Document(DocumentMsg::Undo));
    assert_eq!(scope(&model), Some((0, 3)));
}

#[test]
fn find_replace_does_not_mutate_nontext_tabs() {
    let mut model = common::test_model_with_selection("foo", 0, 0, 0, 3);
    model.editor_mut().tab_content =
        token::model::TabContent::BinaryPlaceholder(token::model::BinaryPlaceholderState {
            path: "fixture.bin".into(),
            size_bytes: token::util::ByteSize::bytes(3).as_u64(),
        });
    open_find(&mut model, "foo", "bar", false);
    for message in [ModalMsg::ReplaceAll, ModalMsg::ReplaceAndFindNext] {
        act(&mut model, message);
        assert_eq!(model.document().buffer.to_string(), "foo");
        assert!(model.document().undo_stack.is_empty());
    }
}
