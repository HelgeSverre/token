//! Text editing tests - insert, delete, undo/redo

mod common;

use common::{test_model, buffer_to_string};
use token::messages::{DocumentMsg, Msg};
use token::update::update;

// ========================================================================
// InsertChar tests
// ========================================================================

#[test]
fn test_insert_char_at_start() {
    let mut model = test_model("hello", 0, 0);
    update(&mut model, Msg::Document(DocumentMsg::InsertChar('X')));

    assert_eq!(buffer_to_string(&model), "Xhello");
    assert_eq!(model.editor.cursor().column, 1);
    assert_eq!(model.editor.cursor().line, 0);
}

#[test]
fn test_insert_char_at_middle() {
    let mut model = test_model("hello", 0, 2);
    update(&mut model, Msg::Document(DocumentMsg::InsertChar('X')));

    assert_eq!(buffer_to_string(&model), "heXllo");
    assert_eq!(model.editor.cursor().column, 3);
}

#[test]
fn test_insert_char_at_end() {
    let mut model = test_model("hello", 0, 5);
    update(&mut model, Msg::Document(DocumentMsg::InsertChar('X')));

    assert_eq!(buffer_to_string(&model), "helloX");
    assert_eq!(model.editor.cursor().column, 6);
}

#[test]
fn test_insert_space_at_middle() {
    let mut model = test_model("helloworld", 0, 5);
    update(&mut model, Msg::Document(DocumentMsg::InsertChar(' ')));

    assert_eq!(buffer_to_string(&model), "hello world");
    assert_eq!(model.editor.cursor().column, 6);
}

#[test]
fn test_insert_multiple_chars_consecutively() {
    let mut model = test_model("hello", 0, 5);
    update(&mut model, Msg::Document(DocumentMsg::InsertChar(' ')));
    update(&mut model, Msg::Document(DocumentMsg::InsertChar('w')));
    update(&mut model, Msg::Document(DocumentMsg::InsertChar('o')));
    update(&mut model, Msg::Document(DocumentMsg::InsertChar('r')));
    update(&mut model, Msg::Document(DocumentMsg::InsertChar('l')));
    update(&mut model, Msg::Document(DocumentMsg::InsertChar('d')));

    assert_eq!(buffer_to_string(&model), "hello world");
    assert_eq!(model.editor.cursor().column, 11);
}

#[test]
fn test_insert_char_on_second_line() {
    let mut model = test_model("hello\nworld", 1, 2);
    update(&mut model, Msg::Document(DocumentMsg::InsertChar('X')));

    assert_eq!(buffer_to_string(&model), "hello\nwoXrld");
    assert_eq!(model.editor.cursor().line, 1);
    assert_eq!(model.editor.cursor().column, 3);
}

#[test]
fn test_insert_multiple_spaces_middle_of_line() {
    let mut model = test_model("helloworld", 0, 5);

    // Insert 3 spaces consecutively - this tests the "playing catchup" bug
    update(&mut model, Msg::Document(DocumentMsg::InsertChar(' ')));
    assert_eq!(buffer_to_string(&model), "hello world");
    assert_eq!(model.editor.cursor().column, 6);

    update(&mut model, Msg::Document(DocumentMsg::InsertChar(' ')));
    assert_eq!(buffer_to_string(&model), "hello  world");
    assert_eq!(model.editor.cursor().column, 7);

    update(&mut model, Msg::Document(DocumentMsg::InsertChar(' ')));
    assert_eq!(buffer_to_string(&model), "hello   world");
    assert_eq!(model.editor.cursor().column, 8);
}

#[test]
fn test_insert_after_cursor_position_clamped() {
    // This tests the suspected bug: cursor.column > line length
    let mut model = test_model("hi", 0, 10); // column 10 on 2-char line

    // Position should be clamped to 2
    let pos = model
        .document
        .cursor_to_offset(model.editor.cursor().line, model.editor.cursor().column);
    assert_eq!(pos, 2);

    // Insert should happen at clamped position
    update(&mut model, Msg::Document(DocumentMsg::InsertChar('X')));
    assert_eq!(buffer_to_string(&model), "hiX");

    // After insert, cursor.column should be valid
    assert!(
        model.editor.cursor().column <= model.document.line_length(model.editor.cursor().line)
    );
}

// ========================================================================
// InsertNewline tests
// ========================================================================

#[test]
fn test_insert_newline_at_end() {
    let mut model = test_model("hello", 0, 5);
    update(&mut model, Msg::Document(DocumentMsg::InsertNewline));

    assert_eq!(buffer_to_string(&model), "hello\n");
    assert_eq!(model.editor.cursor().line, 1);
    assert_eq!(model.editor.cursor().column, 0);
}

#[test]
fn test_insert_newline_at_middle() {
    let mut model = test_model("hello", 0, 2);
    update(&mut model, Msg::Document(DocumentMsg::InsertNewline));

    assert_eq!(buffer_to_string(&model), "he\nllo");
    assert_eq!(model.editor.cursor().line, 1);
    assert_eq!(model.editor.cursor().column, 0);
}

#[test]
fn test_insert_newline_at_start() {
    let mut model = test_model("hello", 0, 0);
    update(&mut model, Msg::Document(DocumentMsg::InsertNewline));

    assert_eq!(buffer_to_string(&model), "\nhello");
    assert_eq!(model.editor.cursor().line, 1);
    assert_eq!(model.editor.cursor().column, 0);
}

// ========================================================================
// DeleteBackward tests
// ========================================================================

#[test]
fn test_delete_backward_middle_of_line() {
    let mut model = test_model("hello", 0, 3);
    update(&mut model, Msg::Document(DocumentMsg::DeleteBackward));

    assert_eq!(buffer_to_string(&model), "helo");
    assert_eq!(model.editor.cursor().column, 2);
}

#[test]
fn test_delete_backward_at_start_of_line() {
    let mut model = test_model("hello", 0, 0);
    update(&mut model, Msg::Document(DocumentMsg::DeleteBackward));

    // Nothing should happen
    assert_eq!(buffer_to_string(&model), "hello");
    assert_eq!(model.editor.cursor().column, 0);
}

#[test]
fn test_delete_backward_joins_lines() {
    let mut model = test_model("hello\nworld", 1, 0);
    update(&mut model, Msg::Document(DocumentMsg::DeleteBackward));

    assert_eq!(buffer_to_string(&model), "helloworld");
    assert_eq!(model.editor.cursor().line, 0);
    assert_eq!(model.editor.cursor().column, 5); // End of "hello"
}

#[test]
fn test_delete_backward_after_empty_line() {
    let mut model = test_model("hello\n\nworld", 2, 0);
    update(&mut model, Msg::Document(DocumentMsg::DeleteBackward));

    assert_eq!(buffer_to_string(&model), "hello\nworld");
    assert_eq!(model.editor.cursor().line, 1);
    assert_eq!(model.editor.cursor().column, 0);
}

// ========================================================================
// DeleteForward tests
// ========================================================================

#[test]
fn test_delete_forward_middle_of_line() {
    let mut model = test_model("hello", 0, 2);
    update(&mut model, Msg::Document(DocumentMsg::DeleteForward));

    assert_eq!(buffer_to_string(&model), "helo");
    assert_eq!(model.editor.cursor().column, 2); // Unchanged
}

#[test]
fn test_delete_forward_at_end_of_line() {
    let mut model = test_model("hello\nworld", 0, 5);
    update(&mut model, Msg::Document(DocumentMsg::DeleteForward));

    // Should delete the newline, joining lines
    assert_eq!(buffer_to_string(&model), "helloworld");
    assert_eq!(model.editor.cursor().column, 5);
}

#[test]
fn test_delete_forward_at_end_of_buffer() {
    let mut model = test_model("hello", 0, 5);
    update(&mut model, Msg::Document(DocumentMsg::DeleteForward));

    // Nothing to delete
    assert_eq!(buffer_to_string(&model), "hello");
}

// ========================================================================
// Undo/Redo tests
// ========================================================================

#[test]
fn test_undo_insert() {
    let mut model = test_model("hello", 0, 5);
    update(&mut model, Msg::Document(DocumentMsg::InsertChar('X')));

    assert_eq!(buffer_to_string(&model), "helloX");

    update(&mut model, Msg::Document(DocumentMsg::Undo));

    assert_eq!(buffer_to_string(&model), "hello");
    assert_eq!(model.editor.cursor().column, 5);
}

#[test]
fn test_redo_insert() {
    let mut model = test_model("hello", 0, 5);
    update(&mut model, Msg::Document(DocumentMsg::InsertChar('X')));
    update(&mut model, Msg::Document(DocumentMsg::Undo));
    update(&mut model, Msg::Document(DocumentMsg::Redo));

    assert_eq!(buffer_to_string(&model), "helloX");
    assert_eq!(model.editor.cursor().column, 6);
}

#[test]
fn test_undo_delete() {
    let mut model = test_model("hello", 0, 5);
    update(&mut model, Msg::Document(DocumentMsg::DeleteBackward));

    assert_eq!(buffer_to_string(&model), "hell");

    update(&mut model, Msg::Document(DocumentMsg::Undo));

    assert_eq!(buffer_to_string(&model), "hello");
    assert_eq!(model.editor.cursor().column, 5);
}
