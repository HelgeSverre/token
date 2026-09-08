//! History owns lossless per-pane selections, not only mapped caret offsets.
mod common;

use token::messages::{DocumentMsg, LayoutMsg, Msg};
use token::model::{Cursor, EditorState, Position, Selection, SplitDirection};
use token::update::update;

fn state(editor: &EditorState) -> (Vec<Cursor>, Vec<Selection>, usize) {
    (
        editor.cursors.clone(),
        editor.selections.clone(),
        editor.active_cursor_index,
    )
}

#[test]
fn undo_pane_state_restores_merged_cursors_reversed_selections_and_active_index() {
    let mut model = common::test_model_multi_cursor("a🙂bcédef", &[(0, 1), (0, 6)]);
    model.editor_mut().selections = vec![
        Selection::from_anchor_head(Position::new(0, 5), Position::new(0, 1)),
        Selection::from_anchor_head(Position::new(0, 3), Position::new(0, 6)),
    ];
    model.editor_mut().active_cursor_index = 1;
    model.editor_mut().cursors[1].desired_column = Some(20);
    let before = state(model.editor());
    update(&mut model, Msg::Document(DocumentMsg::DeleteBackward));
    assert_eq!(model.document().buffer.to_string(), "aef");
    assert_eq!(model.editor().cursors.len(), 1);
    let after = state(model.editor());
    for _ in 0..3 {
        update(&mut model, Msg::Document(DocumentMsg::Undo));
        assert_eq!(model.document().buffer.to_string(), "a🙂bcédef");
        assert_eq!(state(model.editor()), before);
        update(&mut model, Msg::Document(DocumentMsg::Redo));
        assert_eq!(model.document().buffer.to_string(), "aef");
        assert_eq!(state(model.editor()), after);
    }
}

#[test]
fn undo_pane_state_recovers_clipped_peer_positions_even_when_focus_changes() {
    let mut model = common::test_model("ab🙂cdéfg", 0, 4);
    let peer = model.editor().id.unwrap();
    let peer_group = model.editor_area.focused_group_id;
    model.editor_mut().selections[0] =
        Selection::from_anchor_head(Position::new(0, 5), Position::new(0, 4));
    model.editor_mut().cursors[0].desired_column = Some(12);
    let peer_before = state(model.editor());
    update(
        &mut model,
        Msg::Layout(LayoutMsg::SplitFocused(SplitDirection::Vertical)),
    );
    let author = model.editor().id.unwrap();
    model.editor_mut().cursors = vec![Cursor::at(0, 1)];
    model.editor_mut().selections = vec![Selection::from_anchor_head(
        Position::new(0, 7),
        Position::new(0, 1),
    )];
    let author_before = state(model.editor());
    update(&mut model, Msg::Document(DocumentMsg::DeleteBackward));
    assert_eq!(model.document().buffer.to_string(), "ag");
    let peer_after = state(&model.editor_area.editors[&peer]);
    let author_after = state(&model.editor_area.editors[&author]);
    assert_eq!(peer_after.0[0].column, 1);
    model.editor_area.focused_group_id = peer_group;
    update(&mut model, Msg::Document(DocumentMsg::Undo));
    assert_eq!(state(&model.editor_area.editors[&peer]), peer_before);
    assert_eq!(state(&model.editor_area.editors[&author]), author_before);
    assert_eq!(model.editor_area.focused_group_id, peer_group);
    update(&mut model, Msg::Document(DocumentMsg::Redo));
    assert_eq!(state(&model.editor_area.editors[&peer]), peer_after);
    assert_eq!(state(&model.editor_area.editors[&author]), author_after);
}

#[test]
fn undo_pane_state_redo_restores_nonempty_selections_and_new_branch_discards_redo() {
    let mut model = common::test_model("    a\r\n    bé", 1, 2);
    model.editor_mut().selections[0] =
        Selection::from_anchor_head(Position::new(0, 2), Position::new(1, 2));
    let before = state(model.editor());
    update(&mut model, Msg::Document(DocumentMsg::UnindentLines));
    let after = state(model.editor());
    assert!(!after.1[0].is_empty());
    update(&mut model, Msg::Document(DocumentMsg::Undo));
    assert_eq!(state(model.editor()), before);
    update(&mut model, Msg::Document(DocumentMsg::Redo));
    assert_eq!(state(model.editor()), after);
    update(&mut model, Msg::Document(DocumentMsg::Undo));
    update(&mut model, Msg::Document(DocumentMsg::InsertChar('X')));
    let branch = model.document().buffer.to_string();
    assert!(model.document().redo_stack.is_empty());
    update(&mut model, Msg::Document(DocumentMsg::Redo));
    assert_eq!(model.document().buffer.to_string(), branch);
    update(&mut model, Msg::Document(DocumentMsg::Undo));
    assert_eq!(state(model.editor()), before);
}

#[test]
fn undo_pane_state_typing_captures_before_selection_normalization() {
    let mut model = common::test_model_multi_cursor("abcdefgh", &[(0, 1), (0, 6)]);
    model.editor_mut().selections = vec![
        Selection::from_anchor_head(Position::new(0, 5), Position::new(0, 1)),
        Selection::from_anchor_head(Position::new(0, 3), Position::new(0, 6)),
    ];
    model.editor_mut().active_cursor_index = 1;
    let before = state(model.editor());
    update(&mut model, Msg::Document(DocumentMsg::InsertChar('🙂')));
    assert_eq!(model.document().buffer.to_string(), "a🙂gh");
    let after = state(model.editor());
    assert_eq!(after.0.len(), 1);
    update(&mut model, Msg::Document(DocumentMsg::Undo));
    assert_eq!(state(model.editor()), before);
    update(&mut model, Msg::Document(DocumentMsg::Redo));
    assert_eq!(state(model.editor()), after);
}

#[test]
fn undo_pane_state_new_pane_maps_live_positions_and_closed_panes_stay_closed() {
    let mut model = common::test_model("abcdef", 0, 1);
    let original = model.editor().id.unwrap();
    let original_group = model.editor_area.focused_group_id;
    update(&mut model, Msg::Document(DocumentMsg::InsertChar('X')));
    update(
        &mut model,
        Msg::Layout(LayoutMsg::SplitFocused(SplitDirection::Vertical)),
    );
    let new_pane = model.editor().id.unwrap();
    model.editor_mut().cursors = vec![Cursor::at(0, 5)];
    model.editor_mut().selections = vec![Selection::from_anchor_head(
        Position::new(0, 6),
        Position::new(0, 5),
    )];
    update(
        &mut model,
        Msg::Layout(LayoutMsg::CloseGroup(original_group)),
    );
    assert!(!model.editor_area.editors.contains_key(&original));
    update(&mut model, Msg::Document(DocumentMsg::Undo));
    assert_eq!(model.document().buffer.to_string(), "abcdef");
    assert_eq!(model.editor().id, Some(new_pane));
    assert_eq!(model.editor().cursors[0], Cursor::at(0, 4));
    assert_eq!(
        model.editor().selections[0],
        Selection::from_anchor_head(Position::new(0, 5), Position::new(0, 4),)
    );
    update(&mut model, Msg::Document(DocumentMsg::Redo));
    assert_eq!(model.document().buffer.to_string(), "aXbcdef");
    assert_eq!(model.editor().cursors[0], Cursor::at(0, 5));
    assert!(!model.editor_area.editors.contains_key(&original));
}
