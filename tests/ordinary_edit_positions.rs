//! Ordinary edits must map every peer caret and both selection endpoints.
mod common;

use token::messages::{DocumentMsg, LayoutMsg, Msg};
use token::model::{Cursor, Position, Selection, SplitDirection};
use token::update::update;

#[test]
fn ordinary_delete_same_line_siblings_follow_every_removal() {
    for (message, columns) in [
        (DocumentMsg::DeleteBackward, [2, 5]),
        (DocumentMsg::DeleteForward, [1, 4]),
    ] {
        let mut model =
            common::test_model_multi_cursor("abcdef", &[(0, columns[0]), (0, columns[1])]);
        update(&mut model, Msg::Document(message));
        assert_eq!(model.document().buffer.to_string(), "acdf");
        assert_eq!(
            model.editor().cursors,
            vec![Cursor::at(0, 1), Cursor::at(0, 3)]
        );
        assert_eq!(model.document().undo_stack.len(), 1);
        update(&mut model, Msg::Document(DocumentMsg::Undo));
        assert_eq!(model.document().buffer.to_string(), "abcdef");
        assert_eq!(
            model.editor().cursors,
            vec![Cursor::at(0, columns[0]), Cursor::at(0, columns[1])]
        );
    }
}

#[test]
fn ordinary_delete_overlapping_words_are_removed_once() {
    for (message, expected, column) in [
        (DocumentMsg::DeleteWordBackward, "ef gh", 0),
        (DocumentMsg::DeleteWordForward, "ab gh", 2),
    ] {
        let mut model = common::test_model_multi_cursor("abcdef gh", &[(0, 2), (0, 4)]);
        model.editor_mut().active_cursor_index = 1;
        update(&mut model, Msg::Document(message));
        assert_eq!(model.document().buffer.to_string(), expected);
        assert_eq!(model.editor().cursors, vec![Cursor::at(0, column)]);
        assert_eq!(model.editor().active_cursor_index, 0);
        update(&mut model, Msg::Document(DocumentMsg::Undo));
        assert_eq!(model.document().buffer.to_string(), "abcdef gh");
        assert_eq!(model.editor().cursors.len(), 2);
        update(&mut model, Msg::Document(DocumentMsg::Redo));
        assert_eq!(model.document().buffer.to_string(), expected);
    }
}

fn copied_text(cmd: &token::Cmd) -> Option<&str> {
    match cmd {
        token::Cmd::CopyToClipboard(text) => Some(text),
        token::Cmd::Batch(commands) => commands.iter().find_map(copied_text),
        _ => None,
    }
}

#[test]
fn ordinary_delete_overlapping_selections_and_cut_use_pristine_ranges() {
    for message in [
        DocumentMsg::DeleteBackward,
        DocumentMsg::DeleteForward,
        DocumentMsg::Cut,
    ] {
        let cut = matches!(message, DocumentMsg::Cut);
        let mut model = common::test_model_multi_cursor("abcdef", &[(0, 1), (0, 5)]);
        model.editor_mut().selections = vec![
            Selection::from_anchor_head(Position::new(0, 4), Position::new(0, 1)),
            Selection::from_anchor_head(Position::new(0, 3), Position::new(0, 5)),
        ];
        let cmd = update(&mut model, Msg::Document(message));
        assert_eq!(model.document().buffer.to_string(), "af");
        assert_eq!(model.editor().cursors, vec![Cursor::at(0, 1)]);
        if cut {
            // Copy and cut retain per-selection payload/order; only physical
            // deletion coalesces overlap so shared text is removed once.
            assert_eq!(cmd.as_ref().and_then(copied_text), Some("bcd\nde"));
        }
        update(&mut model, Msg::Document(DocumentMsg::Undo));
        assert_eq!(model.document().buffer.to_string(), "abcdef");
    }
}

#[test]
fn ordinary_delete_newline_joins_map_all_carets() {
    let mut model = common::test_model_multi_cursor("ab\ncd\nef", &[(0, 2), (1, 2)]);
    update(&mut model, Msg::Document(DocumentMsg::DeleteForward));
    assert_eq!(model.document().buffer.to_string(), "abcdef");
    assert_eq!(
        model.editor().cursors,
        vec![Cursor::at(0, 2), Cursor::at(0, 4)]
    );
}

#[test]
fn ordinary_delete_crlf_join_is_one_edit() {
    for (message, line, column) in [
        (DocumentMsg::DeleteBackward, 1, 0),
        (DocumentMsg::DeleteForward, 0, 2),
    ] {
        let mut model = common::test_model("ab\r\ncd", line, column);
        update(&mut model, Msg::Document(message));
        assert_eq!(model.document().buffer.to_string(), "abcd");
        assert_eq!(model.editor().cursors[0], Cursor::at(0, 2));
        assert_eq!(model.document().undo_stack.len(), 1);
        update(&mut model, Msg::Document(DocumentMsg::Undo));
        assert_eq!(model.document().buffer.to_string(), "ab\r\ncd");
    }
}

#[test]
fn ordinary_delete_empty_batch_does_not_dirty_or_add_undo() {
    let mut model = common::test_model_multi_cursor("abc", &[(0, 0), (0, 0)]);
    update(&mut model, Msg::Document(DocumentMsg::DeleteBackward));
    assert_eq!(model.document().buffer.to_string(), "abc");
    assert!(!model.document().is_modified);
    assert!(model.document().undo_stack.is_empty());
}

#[test]
fn ordinary_duplicate_selection_maps_same_line_siblings() {
    let mut model = common::test_model_multi_cursor("abcdef", &[(0, 2), (0, 5)]);
    model.editor_mut().selections = vec![
        Selection::from_anchor_head(Position::new(0, 1), Position::new(0, 2)),
        Selection::from_anchor_head(Position::new(0, 4), Position::new(0, 5)),
    ];
    update(&mut model, Msg::Document(DocumentMsg::Duplicate));
    assert_eq!(model.document().buffer.to_string(), "abbcdeef");
    assert_eq!(
        model.editor().cursors,
        vec![Cursor::at(0, 3), Cursor::at(0, 7)]
    );
    update(&mut model, Msg::Document(DocumentMsg::Undo));
    assert_eq!(model.document().buffer.to_string(), "abcdef");
}

#[test]
fn ordinary_duplicate_sources_are_captured_before_any_mutation() {
    let mut model = common::test_model_multi_cursor("abcdef", &[(0, 1), (0, 5)]);
    model.editor_mut().selections[1] =
        Selection::from_anchor_head(Position::new(0, 4), Position::new(0, 5));
    update(&mut model, Msg::Document(DocumentMsg::Duplicate));
    assert_eq!(model.document().buffer.to_string(), "abcdeef\nabcdef");
    assert_eq!(
        model.editor().cursors,
        vec![Cursor::at(1, 1), Cursor::at(0, 6)]
    );
    assert_eq!(model.document().undo_stack.len(), 1);
    update(&mut model, Msg::Document(DocumentMsg::Undo));
    assert_eq!(model.document().buffer.to_string(), "abcdef");
}

#[test]
fn ordinary_line_delete_trailing_run_preserves_no_final_newline() {
    for ending in ["\n", "\r\n"] {
        let text = format!("a{ending}bb{ending}ccc");
        let mut model = common::test_model_multi_cursor(&text, &[(1, 1), (2, 2)]);
        update(&mut model, Msg::Document(DocumentMsg::DeleteLine));
        assert_eq!(model.document().buffer.to_string(), "a");
        assert_eq!(model.editor().cursors, vec![Cursor::at(0, 1)]);
        update(&mut model, Msg::Document(DocumentMsg::Undo));
        assert_eq!(model.document().buffer.to_string(), text);
        assert_eq!(
            model.editor().cursors,
            vec![Cursor::at(1, 1), Cursor::at(2, 2)]
        );
    }
}

#[test]
fn ordinary_line_delete_noncontiguous_preserves_active_caret() {
    let mut model = common::test_model_multi_cursor("a0\nb1\nc2\nd3\ne4", &[(0, 1), (2, 1)]);
    model.editor_mut().active_cursor_index = 1;
    update(&mut model, Msg::Document(DocumentMsg::DeleteLine));
    assert_eq!(model.document().buffer.to_string(), "b1\nd3\ne4");
    assert_eq!(
        model.editor().cursors,
        vec![Cursor::at(0, 1), Cursor::at(1, 1)]
    );
    assert_eq!(model.editor().active_cursor_index, 1);
}

#[test]
fn ordinary_duplicate_equal_points_keep_each_copy_owner() {
    let mut model = common::test_model_multi_cursor("ab\n", &[(0, 0), (0, 1)]);
    model.editor_mut().active_cursor_index = 1;
    update(&mut model, Msg::Document(DocumentMsg::Duplicate));
    assert_eq!(model.document().buffer.to_string(), "ab\nab\nab\n");
    assert_eq!(
        model.editor().cursors,
        vec![Cursor::at(1, 0), Cursor::at(2, 1)]
    );
    assert_eq!(model.editor().active_cursor_index, 1);
    update(&mut model, Msg::Document(DocumentMsg::Undo));
    assert_eq!(model.document().buffer.to_string(), "ab\n");
    update(&mut model, Msg::Document(DocumentMsg::Redo));
    assert_eq!(
        model.editor().cursors,
        vec![Cursor::at(1, 0), Cursor::at(2, 1)]
    );
}

#[test]
fn ordinary_clipboard_preserves_selection_order_and_maps_unselected_carets() {
    for message in [DocumentMsg::Copy, DocumentMsg::Cut] {
        let cut = matches!(message, DocumentMsg::Cut);
        let mut model = common::test_model_multi_cursor("🙂 a é z", &[(0, 5), (0, 3), (0, 1)]);
        model.editor_mut().selections[0] =
            Selection::from_anchor_head(Position::new(0, 4), Position::new(0, 5));
        model.editor_mut().selections[2] =
            Selection::from_anchor_head(Position::new(0, 0), Position::new(0, 1));
        model.editor_mut().active_cursor_index = 1;
        let cmd = update(&mut model, Msg::Document(message));
        assert_eq!(cmd.as_ref().and_then(copied_text), Some("é\n🙂"));
        assert_eq!(
            model.ui.transient_message.as_ref().unwrap().text,
            if cut { "Cut 3 chars" } else { "Copied 3 chars" }
        );
        assert_eq!(model.editor().active_cursor_index, 1);
        if cut {
            assert_eq!(model.document().buffer.to_string(), " a  z");
            assert_eq!(
                model.editor().cursors,
                vec![Cursor::at(0, 3), Cursor::at(0, 2), Cursor::at(0, 0)]
            );
            assert_eq!(model.document().undo_stack.len(), 1);
        } else {
            assert_eq!(model.document().buffer.to_string(), "🙂 a é z");
            assert!(model.document().undo_stack.is_empty());
            assert!(!model.editor().selections[0].is_empty());
        }
    }
}

#[test]
fn ordinary_unindent_maps_reversed_selections_and_clips_leading_columns() {
    let text = "\t🙂\n    abc\nz";
    let mut model = common::test_model_multi_cursor(text, &[(0, 1), (1, 2)]);
    model.editor_mut().selections = vec![
        Selection::from_anchor_head(Position::new(0, 2), Position::new(0, 1)),
        Selection::from_anchor_head(Position::new(2, 0), Position::new(1, 2)),
    ];
    model.editor_mut().active_cursor_index = 1;
    update(&mut model, Msg::Document(DocumentMsg::UnindentLines));
    assert_eq!(model.document().buffer.to_string(), "🙂\nabc\nz");
    assert_eq!(
        model.editor().cursors,
        vec![Cursor::at(0, 0), Cursor::at(1, 0)]
    );
    assert_eq!(
        model.editor().selections,
        vec![
            Selection::from_anchor_head(Position::new(0, 1), Position::new(0, 0)),
            Selection::from_anchor_head(Position::new(2, 0), Position::new(1, 0)),
        ]
    );
    assert_eq!(model.editor().active_cursor_index, 1);
    update(&mut model, Msg::Document(DocumentMsg::Undo));
    assert_eq!(model.document().buffer.to_string(), text);
}

#[test]
fn ordinary_insert_maps_same_line_siblings_and_replaces_selections() {
    for (message, expected, columns) in [
        (DocumentMsg::InsertChar('X'), "aXcdXf", vec![2, 5]),
        (DocumentMsg::InsertText("🙂".into()), "a🙂cd🙂f", vec![2, 5]),
        (DocumentMsg::InsertText("XX".into()), "aXXcdXXf", vec![3, 7]),
        (
            DocumentMsg::InsertText("🙂\nz".into()),
            "a🙂cdzf",
            vec![2, 5],
        ),
    ] {
        let mut model = common::test_model_multi_cursor("abcdef", &[(0, 2), (0, 5)]);
        model.editor_mut().selections = vec![
            Selection::from_anchor_head(Position::new(0, 1), Position::new(0, 2)),
            Selection::from_anchor_head(Position::new(0, 4), Position::new(0, 5)),
        ];
        update(&mut model, Msg::Document(message));
        assert_eq!(model.document().buffer.to_string(), expected);
        assert_eq!(
            model
                .editor()
                .cursors
                .iter()
                .map(|cursor| cursor.column)
                .collect::<Vec<_>>(),
            columns
        );
        assert!(model.editor().selections.iter().all(Selection::is_empty));
        assert_eq!(model.document().undo_stack.len(), 1);
        update(&mut model, Msg::Document(DocumentMsg::Undo));
        assert_eq!(model.document().buffer.to_string(), "abcdef");
        update(&mut model, Msg::Document(DocumentMsg::Redo));
        assert_eq!(model.document().buffer.to_string(), expected);
    }
}

#[test]
fn ordinary_insert_newlines_replace_selected_text_and_map_siblings() {
    let mut model = common::test_model_multi_cursor("abcdef", &[(0, 2), (0, 5)]);
    model.editor_mut().selections = vec![
        Selection::from_anchor_head(Position::new(0, 1), Position::new(0, 2)),
        Selection::from_anchor_head(Position::new(0, 4), Position::new(0, 5)),
    ];
    update(&mut model, Msg::Document(DocumentMsg::InsertNewline));
    assert_eq!(model.document().buffer.to_string(), "a\ncd\nf");
    assert_eq!(
        model.editor().cursors,
        vec![Cursor::at(1, 0), Cursor::at(2, 0)]
    );
}

#[test]
fn ordinary_insert_same_line_carets_follow_all_insertions() {
    let mut model = common::test_model_multi_cursor("abcd", &[(0, 3), (0, 1)]);
    model.editor_mut().active_cursor_index = 1;
    update(&mut model, Msg::Document(DocumentMsg::InsertChar('🙂')));
    assert_eq!(model.document().buffer.to_string(), "a🙂bc🙂d");
    assert_eq!(
        model.editor().cursors,
        vec![Cursor::at(0, 5), Cursor::at(0, 2)]
    );
    assert_eq!(model.editor().active_cursor_index, 1);
}

#[test]
fn ordinary_insert_full_multiline_paste_maps_both_selected_ranges() {
    let mut model = common::test_model_multi_cursor("abcdef", &[(0, 1), (0, 5)]);
    model.editor_mut().selections = vec![
        Selection::from_anchor_head(Position::new(0, 2), Position::new(0, 1)),
        Selection::from_anchor_head(Position::new(0, 4), Position::new(0, 5)),
    ];
    update(
        &mut model,
        Msg::Document(DocumentMsg::InsertText("🙂\nz\nq".into())),
    );
    assert_eq!(model.document().buffer.to_string(), "a🙂\nz\nqcd🙂\nz\nqf");
    assert_eq!(
        model.editor().cursors,
        vec![Cursor::at(2, 1), Cursor::at(4, 1)]
    );
    update(&mut model, Msg::Document(DocumentMsg::Undo));
    assert_eq!(model.document().buffer.to_string(), "abcdef");
    assert_eq!(
        model.editor().cursors,
        vec![Cursor::at(0, 1), Cursor::at(0, 5)]
    );
}

#[test]
fn ordinary_insert_overlapping_ranges_are_replaced_once() {
    let mut model = common::test_model_multi_cursor("abcdef", &[(0, 4), (0, 5)]);
    model.editor_mut().selections = vec![
        Selection::from_anchor_head(Position::new(0, 1), Position::new(0, 4)),
        Selection::from_anchor_head(Position::new(0, 3), Position::new(0, 5)),
    ];
    model.editor_mut().active_cursor_index = 1;
    update(&mut model, Msg::Document(DocumentMsg::InsertChar('X')));
    assert_eq!(model.document().buffer.to_string(), "aXf");
    assert_eq!(model.editor().cursors, vec![Cursor::at(0, 2)]);
    assert_eq!(model.editor().active_cursor_index, 0);
    update(&mut model, Msg::Document(DocumentMsg::Undo));
    assert_eq!(model.document().buffer.to_string(), "abcdef");
}

#[test]
fn ordinary_insert_mixed_surround_and_plain_caret_respects_config() {
    for (enabled, expected, carets) in [
        (true, "(ab) cd(", vec![Cursor::at(0, 4), Cursor::at(0, 8)]),
        (false, "( cd(", vec![Cursor::at(0, 1), Cursor::at(0, 5)]),
    ] {
        let mut model = common::test_model_multi_cursor("ab cd", &[(0, 2), (0, 5)]);
        model.config.auto_surround = enabled;
        model.editor_mut().selections[0] =
            Selection::from_anchor_head(Position::new(0, 0), Position::new(0, 2));
        update(&mut model, Msg::Document(DocumentMsg::InsertChar('(')));
        assert_eq!(model.document().buffer.to_string(), expected);
        assert_eq!(model.editor().cursors, carets);
    }
}

#[test]
fn ordinary_insert_surround_keeps_unicode_peer_on_surviving_text() {
    let mut model = common::test_model("a🙂b tail", 0, 2);
    let peer_id = model.editor_area.focused_editor_id().unwrap();
    update(
        &mut model,
        Msg::Layout(LayoutMsg::SplitFocused(SplitDirection::Vertical)),
    );
    model.config.auto_surround = true;
    model.editor_mut().cursors[0] = Cursor::at(0, 3);
    model.editor_mut().selections[0] =
        Selection::from_anchor_head(Position::new(0, 0), Position::new(0, 3));
    update(&mut model, Msg::Document(DocumentMsg::InsertChar('(')));
    assert_eq!(model.document().buffer.to_string(), "(a🙂b) tail");
    assert_eq!(model.editor().cursors[0], Cursor::at(0, 5));
    assert_eq!(
        model.editor_area.editors[&peer_id].cursors[0],
        Cursor::at(0, 3)
    );
    update(&mut model, Msg::Document(DocumentMsg::Undo));
    assert_eq!(model.document().buffer.to_string(), "a🙂b tail");
    assert_eq!(
        model.editor_area.editors[&peer_id].cursors[0],
        Cursor::at(0, 2)
    );
}

fn peer_positions(editor: &mut token::model::EditorState, cursor: Cursor, selection: Selection) {
    editor.cursors = vec![
        cursor,
        Cursor::at(selection.head.line, selection.head.column),
    ];
    editor.selections = vec![Selection::new(cursor.to_position()), selection];
}

#[test]
fn ordinary_peer_positions_follow_insert_delete_and_line_edits() {
    for (message, origin, peer, expected, anchor, head) in [
        (
            DocumentMsg::InsertChar('X'),
            (0, 1),
            (0, 2),
            (0, 3),
            (2, 1),
            (0, 2),
        ),
        (
            DocumentMsg::InsertNewline,
            (0, 1),
            (0, 2),
            (1, 1),
            (3, 1),
            (1, 0),
        ),
        (
            DocumentMsg::InsertText("🙂\nz".into()),
            (0, 1),
            (0, 2),
            (1, 2),
            (3, 1),
            (1, 1),
        ),
        (
            DocumentMsg::DeleteBackward,
            (1, 0),
            (1, 1),
            (0, 3),
            (1, 1),
            (0, 1),
        ),
        (
            DocumentMsg::DeleteForward,
            (0, 2),
            (1, 1),
            (0, 3),
            (1, 1),
            (0, 1),
        ),
        (
            DocumentMsg::DeleteWordBackward,
            (1, 2),
            (1, 2),
            (1, 0),
            (2, 1),
            (0, 1),
        ),
        (
            DocumentMsg::DeleteWordForward,
            (1, 0),
            (1, 2),
            (1, 0),
            (2, 1),
            (0, 1),
        ),
        (
            DocumentMsg::DeleteLine,
            (1, 1),
            (1, 2),
            (1, 0),
            (1, 1),
            (0, 1),
        ),
        (
            DocumentMsg::Duplicate,
            (0, 1),
            (1, 1),
            (2, 1),
            (3, 1),
            (0, 1),
        ),
    ] {
        let label = format!("{message:?}");
        let mut model = common::test_model("ab\ncd\nef", origin.0, origin.1);
        let peer_id = model.editor_area.focused_editor_id().unwrap();
        update(
            &mut model,
            Msg::Layout(LayoutMsg::SplitFocused(SplitDirection::Vertical)),
        );
        model.editor_mut().cursors[0] = Cursor::at(origin.0, origin.1);
        model.editor_mut().selections[0] = Selection::new(Position::new(origin.0, origin.1));
        {
            let editor = model.editor_area.editors.get_mut(&peer_id).unwrap();
            peer_positions(
                editor,
                Cursor::at(peer.0, peer.1),
                Selection::from_anchor_head(Position::new(2, 1), Position::new(0, 1)),
            );
        }
        update(&mut model, Msg::Document(message));
        let editor = &model.editor_area.editors[&peer_id];
        assert_eq!(
            editor.cursors[0],
            Cursor::at(expected.0, expected.1),
            "{label}"
        );
        assert_eq!(
            editor.selections[1],
            Selection::from_anchor_head(
                Position::new(anchor.0, anchor.1),
                Position::new(head.0, head.1)
            ),
            "{label}"
        );
    }
}

#[test]
fn ordinary_peer_positions_follow_selection_replacement_and_cut() {
    for (message, column) in [
        (DocumentMsg::InsertText("X".into()), 3),
        (DocumentMsg::Cut, 2),
        (DocumentMsg::DeleteBackward, 2),
    ] {
        let mut model = common::test_model("ab\ncd\nef", 1, 2);
        let peer_id = model.editor_area.focused_editor_id().unwrap();
        peer_positions(
            model.editor_mut(),
            Cursor::at(1, 2),
            Selection::from_anchor_head(Position::new(2, 1), Position::new(0, 1)),
        );
        update(
            &mut model,
            Msg::Layout(LayoutMsg::SplitFocused(SplitDirection::Vertical)),
        );
        model.editor_mut().cursors[0] = Cursor::at(1, 1);
        model.editor_mut().selections[0] =
            Selection::from_anchor_head(Position::new(0, 1), Position::new(1, 1));
        update(&mut model, Msg::Document(message));
        let editor = &model.editor_area.editors[&peer_id];
        assert_eq!(editor.cursors[0], Cursor::at(0, column));
        assert_eq!(
            editor.selections[1],
            Selection::from_anchor_head(Position::new(1, 1), Position::new(0, 1))
        );
        assert_eq!(model.document().undo_stack.len(), 1);
    }
}

#[test]
fn ordinary_peer_batch_mapping_and_history_apply_once() {
    let mut model = common::test_model("ab\ncd\nef", 2, 2);
    let peer_id = model.editor_area.focused_editor_id().unwrap();
    peer_positions(
        model.editor_mut(),
        Cursor::at(2, 2),
        Selection::from_anchor_head(Position::new(2, 1), Position::new(0, 2)),
    );
    update(
        &mut model,
        Msg::Layout(LayoutMsg::SplitFocused(SplitDirection::Vertical)),
    );
    model.editor_mut().cursors = vec![Cursor::at(0, 1), Cursor::at(1, 1)];
    model.editor_mut().selections = vec![
        Selection::new(Position::new(0, 1)),
        Selection::new(Position::new(1, 1)),
    ];
    update(&mut model, Msg::Document(DocumentMsg::InsertNewline));
    assert_eq!(model.document().buffer.to_string(), "a\nb\nc\nd\nef");
    assert_eq!(model.document().undo_stack.len(), 1);
    let check = |model: &token::model::AppModel, line| {
        let peer = &model.editor_area.editors[&peer_id];
        assert_eq!(peer.cursors[0], Cursor::at(line, 2));
        assert_eq!(peer.selections[1].anchor, Position::new(line, 1));
    };
    check(&model, 4);
    assert_eq!(
        model.editor_area.editors[&peer_id].selections[1].head,
        Position::new(1, 1)
    );
    update(&mut model, Msg::Document(DocumentMsg::Undo));
    check(&model, 2);
    assert_eq!(
        model.editor_area.editors[&peer_id].selections[1].head,
        Position::new(0, 2)
    );
    update(&mut model, Msg::Document(DocumentMsg::Redo));
    check(&model, 4);
    assert_eq!(
        model.editor_area.editors[&peer_id].selections[1].head,
        Position::new(1, 1)
    );
}

#[test]
fn ordinary_peer_indent_unindent_maps_each_line() {
    let mut model = common::test_model("ab\ncd\nef", 1, 1);
    let peer_id = model.editor_area.focused_editor_id().unwrap();
    peer_positions(
        model.editor_mut(),
        Cursor::at(1, 1),
        Selection::from_anchor_head(Position::new(2, 1), Position::new(0, 0)),
    );
    update(
        &mut model,
        Msg::Layout(LayoutMsg::SplitFocused(SplitDirection::Vertical)),
    );
    model.editor_mut().cursors[0] = Cursor::at(1, 2);
    model.editor_mut().selections[0] =
        Selection::from_anchor_head(Position::new(0, 0), Position::new(1, 2));
    update(&mut model, Msg::Document(DocumentMsg::IndentLines));
    let peer = &model.editor_area.editors[&peer_id];
    assert_eq!(peer.cursors[0], Cursor::at(1, 2));
    assert_eq!(
        peer.selections[1],
        Selection::from_anchor_head(Position::new(2, 1), Position::new(0, 1))
    );
    update(&mut model, Msg::Document(DocumentMsg::UnindentLines));
    let peer = &model.editor_area.editors[&peer_id];
    assert_eq!(peer.cursors[0], Cursor::at(1, 1));
    assert_eq!(
        peer.selections[1],
        Selection::from_anchor_head(Position::new(2, 1), Position::new(0, 0))
    );
}

#[test]
fn ordinary_peer_noop_preserves_navigation_state() {
    for message in [
        DocumentMsg::InsertText(String::new()),
        DocumentMsg::DeleteBackward,
        DocumentMsg::UnindentLines,
        DocumentMsg::Copy,
        DocumentMsg::Paste,
    ] {
        let mut model = common::test_model("ab", 0, 0);
        let peer_id = model.editor_area.focused_editor_id().unwrap();
        model.editor_mut().cursors[0].desired_column = Some(12);
        let snapshot = token::model::editor::SelectionSnapshot {
            cursors: model.editor().cursors.clone(),
            selections: model.editor().selections.clone(),
            active_cursor_index: 0,
        };
        model.editor_mut().selection_history.push(snapshot);
        update(
            &mut model,
            Msg::Layout(LayoutMsg::SplitFocused(SplitDirection::Vertical)),
        );
        // Also cover an empty multi-cursor deletion batch at the buffer start.
        if matches!(message, DocumentMsg::DeleteBackward) {
            model.editor_mut().cursors = vec![Cursor::at(0, 0); 2];
            model.editor_mut().selections = vec![Selection::new(Position::new(0, 0)); 2];
        }
        update(&mut model, Msg::Document(message));
        let peer = &model.editor_area.editors[&peer_id];
        assert_eq!(peer.cursors[0].desired_column, Some(12));
        assert_eq!(peer.selection_history.len(), 1);
        assert_eq!(model.document().buffer.to_string(), "ab");
    }
}
