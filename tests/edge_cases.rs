//! Edge case and regression tests

mod common;

use common::{test_model, buffer_to_string};
use token::messages::{Direction, DocumentMsg, EditorMsg, Msg};
use token::update::update;

// ========================================================================
// set_cursor_from_position tests
// ========================================================================

#[test]
fn test_set_cursor_from_position_first_line() {
    let mut model = test_model("hello\nworld", 0, 0);
    model.set_cursor_from_position(3);

    assert_eq!(model.editor.cursor().line, 0);
    assert_eq!(model.editor.cursor().column, 3);
}

#[test]
fn test_set_cursor_from_position_second_line() {
    let mut model = test_model("hello\nworld", 0, 0);
    model.set_cursor_from_position(8); // "hello\nwo|rld"

    assert_eq!(model.editor.cursor().line, 1);
    assert_eq!(model.editor.cursor().column, 2);
}

#[test]
fn test_set_cursor_from_position_at_newline() {
    let mut model = test_model("hello\nworld", 0, 0);
    model.set_cursor_from_position(5); // "hello|" just before newline

    assert_eq!(model.editor.cursor().line, 0);
    assert_eq!(model.editor.cursor().column, 5);
}

#[test]
fn test_set_cursor_from_position_past_end() {
    let mut model = test_model("hello\nworld", 0, 0);
    model.set_cursor_from_position(100);

    // Should clamp to end of buffer
    assert_eq!(model.editor.cursor().line, 1);
    assert_eq!(model.editor.cursor().column, 5); // End of "world"
}

// ========================================================================
// Edge case / regression tests
// ========================================================================

#[test]
fn test_insert_preserves_cursor_buffer_position_consistency() {
    let mut model = test_model("hello world", 0, 6); // "hello |world"

    // After each insert, cursor position should match buffer position
    for ch in "foo".chars() {
        let before_pos = model
            .document
            .cursor_to_offset(model.editor.cursor().line, model.editor.cursor().column);
        update(&mut model, Msg::Document(DocumentMsg::InsertChar(ch)));
        let after_pos = model
            .document
            .cursor_to_offset(model.editor.cursor().line, model.editor.cursor().column);

        // Buffer position should advance by 1
        assert_eq!(after_pos, before_pos + 1);

        // Cursor column should match
        assert_eq!(model.editor.cursor().column, after_pos - 0); // On line 0
    }

    assert_eq!(buffer_to_string(&model), "hello fooworld");
}

#[test]
fn test_multiple_inserts_middle_of_line_no_drift() {
    // This specifically tests the "playing catchup" bug
    let mut model = test_model("the quick brown fox", 0, 10); // "the quick |brown fox"

    let initial_pos = model
        .document
        .cursor_to_offset(model.editor.cursor().line, model.editor.cursor().column);
    assert_eq!(initial_pos, 10);

    // Insert multiple characters and verify no drift
    let insertions = "very ";
    for (i, ch) in insertions.chars().enumerate() {
        update(&mut model, Msg::Document(DocumentMsg::InsertChar(ch)));

        let expected_pos = initial_pos + i + 1;
        let actual_pos = model
            .document
            .cursor_to_offset(model.editor.cursor().line, model.editor.cursor().column);

        assert_eq!(
            actual_pos, expected_pos,
            "After inserting '{}', expected pos {} but got {}",
            ch, expected_pos, actual_pos
        );
    }

    assert_eq!(buffer_to_string(&model), "the quick very brown fox");
}

#[test]
fn test_cursor_column_never_exceeds_line_length_after_operations() {
    let mut model = test_model("hello\nworld", 0, 3);

    // Various operations
    update(&mut model, Msg::Document(DocumentMsg::InsertChar('X')));
    assert!(
        model.editor.cursor().column <= model.document.line_length(model.editor.cursor().line)
    );

    update(&mut model, Msg::Document(DocumentMsg::DeleteBackward));
    assert!(
        model.editor.cursor().column <= model.document.line_length(model.editor.cursor().line)
    );

    update(
        &mut model,
        Msg::Editor(EditorMsg::MoveCursor(Direction::Right)),
    );
    assert!(
        model.editor.cursor().column <= model.document.line_length(model.editor.cursor().line)
    );

    update(
        &mut model,
        Msg::Editor(EditorMsg::MoveCursor(Direction::Down)),
    );
    assert!(
        model.editor.cursor().column <= model.document.line_length(model.editor.cursor().line)
    );
}

#[test]
fn test_empty_buffer() {
    let mut model = test_model("", 0, 0);

    assert_eq!(
        model
            .document
            .cursor_to_offset(model.editor.cursor().line, model.editor.cursor().column),
        0
    );
    assert_eq!(model.document.line_length(model.editor.cursor().line), 0);

    update(&mut model, Msg::Document(DocumentMsg::InsertChar('a')));
    assert_eq!(buffer_to_string(&model), "a");
    assert_eq!(model.editor.cursor().column, 1);
}

#[test]
fn test_single_newline_buffer() {
    let mut model = test_model("\n", 0, 0);

    assert_eq!(model.document.line_length(model.editor.cursor().line), 0);

    update(&mut model, Msg::Document(DocumentMsg::InsertChar('a')));
    assert_eq!(buffer_to_string(&model), "a\n");
}
