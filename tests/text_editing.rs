//! Text editing tests - insert, delete, undo/redo

mod common;

use common::{buffer_to_string, test_model, test_model_with_selection};
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
    assert_eq!(model.editor().cursor().column, 1);
    assert_eq!(model.editor().cursor().line, 0);
}

#[test]
fn test_insert_char_at_middle() {
    let mut model = test_model("hello", 0, 2);
    update(&mut model, Msg::Document(DocumentMsg::InsertChar('X')));

    assert_eq!(buffer_to_string(&model), "heXllo");
    assert_eq!(model.editor().cursor().column, 3);
}

#[test]
fn test_insert_char_at_end() {
    let mut model = test_model("hello", 0, 5);
    update(&mut model, Msg::Document(DocumentMsg::InsertChar('X')));

    assert_eq!(buffer_to_string(&model), "helloX");
    assert_eq!(model.editor().cursor().column, 6);
}

#[test]
fn test_insert_space_at_middle() {
    let mut model = test_model("helloworld", 0, 5);
    update(&mut model, Msg::Document(DocumentMsg::InsertChar(' ')));

    assert_eq!(buffer_to_string(&model), "hello world");
    assert_eq!(model.editor().cursor().column, 6);
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
    assert_eq!(model.editor().cursor().column, 11);
}

#[test]
fn test_insert_char_on_second_line() {
    let mut model = test_model("hello\nworld", 1, 2);
    update(&mut model, Msg::Document(DocumentMsg::InsertChar('X')));

    assert_eq!(buffer_to_string(&model), "hello\nwoXrld");
    assert_eq!(model.editor().cursor().line, 1);
    assert_eq!(model.editor().cursor().column, 3);
}

#[test]
fn test_insert_multiple_spaces_middle_of_line() {
    let mut model = test_model("helloworld", 0, 5);

    // Insert 3 spaces consecutively - this tests the "playing catchup" bug
    update(&mut model, Msg::Document(DocumentMsg::InsertChar(' ')));
    assert_eq!(buffer_to_string(&model), "hello world");
    assert_eq!(model.editor().cursor().column, 6);

    update(&mut model, Msg::Document(DocumentMsg::InsertChar(' ')));
    assert_eq!(buffer_to_string(&model), "hello  world");
    assert_eq!(model.editor().cursor().column, 7);

    update(&mut model, Msg::Document(DocumentMsg::InsertChar(' ')));
    assert_eq!(buffer_to_string(&model), "hello   world");
    assert_eq!(model.editor().cursor().column, 8);
}

#[test]
fn test_insert_after_cursor_position_clamped() {
    // This tests the suspected bug: cursor.column > line length
    let mut model = test_model("hi", 0, 10); // column 10 on 2-char line

    // Position should be clamped to 2
    let pos = model
        .document()
        .cursor_to_offset(model.editor().cursor().line, model.editor().cursor().column);
    assert_eq!(pos, 2);

    // Insert should happen at clamped position
    update(&mut model, Msg::Document(DocumentMsg::InsertChar('X')));
    assert_eq!(buffer_to_string(&model), "hiX");

    // After insert, cursor.column should be valid
    assert!(
        model.editor().cursor().column
            <= model.document().line_length(model.editor().cursor().line)
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
    assert_eq!(model.editor().cursor().line, 1);
    assert_eq!(model.editor().cursor().column, 0);
}

#[test]
fn test_insert_newline_at_middle() {
    let mut model = test_model("hello", 0, 2);
    update(&mut model, Msg::Document(DocumentMsg::InsertNewline));

    assert_eq!(buffer_to_string(&model), "he\nllo");
    assert_eq!(model.editor().cursor().line, 1);
    assert_eq!(model.editor().cursor().column, 0);
}

#[test]
fn test_insert_newline_at_start() {
    let mut model = test_model("hello", 0, 0);
    update(&mut model, Msg::Document(DocumentMsg::InsertNewline));

    assert_eq!(buffer_to_string(&model), "\nhello");
    assert_eq!(model.editor().cursor().line, 1);
    assert_eq!(model.editor().cursor().column, 0);
}

// ========================================================================
// DeleteBackward tests
// ========================================================================

#[test]
fn test_delete_backward_middle_of_line() {
    let mut model = test_model("hello", 0, 3);
    update(&mut model, Msg::Document(DocumentMsg::DeleteBackward));

    assert_eq!(buffer_to_string(&model), "helo");
    assert_eq!(model.editor().cursor().column, 2);
}

#[test]
fn test_delete_backward_at_start_of_line() {
    let mut model = test_model("hello", 0, 0);
    update(&mut model, Msg::Document(DocumentMsg::DeleteBackward));

    // Nothing should happen
    assert_eq!(buffer_to_string(&model), "hello");
    assert_eq!(model.editor().cursor().column, 0);
}

#[test]
fn test_delete_backward_joins_lines() {
    let mut model = test_model("hello\nworld", 1, 0);
    update(&mut model, Msg::Document(DocumentMsg::DeleteBackward));

    assert_eq!(buffer_to_string(&model), "helloworld");
    assert_eq!(model.editor().cursor().line, 0);
    assert_eq!(model.editor().cursor().column, 5); // End of "hello"
}

#[test]
fn test_delete_backward_after_empty_line() {
    let mut model = test_model("hello\n\nworld", 2, 0);
    update(&mut model, Msg::Document(DocumentMsg::DeleteBackward));

    assert_eq!(buffer_to_string(&model), "hello\nworld");
    assert_eq!(model.editor().cursor().line, 1);
    assert_eq!(model.editor().cursor().column, 0);
}

// ========================================================================
// DeleteForward tests
// ========================================================================

#[test]
fn test_delete_forward_middle_of_line() {
    let mut model = test_model("hello", 0, 2);
    update(&mut model, Msg::Document(DocumentMsg::DeleteForward));

    assert_eq!(buffer_to_string(&model), "helo");
    assert_eq!(model.editor().cursor().column, 2); // Unchanged
}

#[test]
fn test_delete_forward_at_end_of_line() {
    let mut model = test_model("hello\nworld", 0, 5);
    update(&mut model, Msg::Document(DocumentMsg::DeleteForward));

    // Should delete the newline, joining lines
    assert_eq!(buffer_to_string(&model), "helloworld");
    assert_eq!(model.editor().cursor().column, 5);
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
    assert_eq!(model.editor().cursor().column, 5);
}

#[test]
fn test_redo_insert() {
    let mut model = test_model("hello", 0, 5);
    update(&mut model, Msg::Document(DocumentMsg::InsertChar('X')));
    update(&mut model, Msg::Document(DocumentMsg::Undo));
    update(&mut model, Msg::Document(DocumentMsg::Redo));

    assert_eq!(buffer_to_string(&model), "helloX");
    assert_eq!(model.editor().cursor().column, 6);
}

#[test]
fn test_undo_delete() {
    let mut model = test_model("hello", 0, 5);
    update(&mut model, Msg::Document(DocumentMsg::DeleteBackward));

    assert_eq!(buffer_to_string(&model), "hell");

    update(&mut model, Msg::Document(DocumentMsg::Undo));

    assert_eq!(buffer_to_string(&model), "hello");
    assert_eq!(model.editor().cursor().column, 5);
}

// ========================================================================
// Undo/Redo with selection tests
// ========================================================================

#[test]
fn test_undo_insert_char_over_selection_restores_original_text() {
    // Text: "hello world", select "llo w" (cols 2..7)
    let mut model = test_model_with_selection("hello world", 0, 2, 0, 7);
    // Cursor is at head (line 0, col 7) before typing

    // Type 'X' to replace selection
    update(&mut model, Msg::Document(DocumentMsg::InsertChar('X')));

    // Net effect should be "heXorld"
    assert_eq!(buffer_to_string(&model), "heXorld");
    assert_eq!(model.editor().cursor().column, 3);

    // Undo should fully restore original text in ONE undo operation
    update(&mut model, Msg::Document(DocumentMsg::Undo));
    assert_eq!(
        buffer_to_string(&model),
        "hello world",
        "Undo should restore the full original text including deleted selection"
    );
}

#[test]
fn test_redo_insert_char_over_selection_reapplies_replacement() {
    // Text: "hello world", select "llo w" (cols 2..7)
    let mut model = test_model_with_selection("hello world", 0, 2, 0, 7);

    // Type 'X' to replace selection
    update(&mut model, Msg::Document(DocumentMsg::InsertChar('X')));
    assert_eq!(buffer_to_string(&model), "heXorld");

    // Undo
    update(&mut model, Msg::Document(DocumentMsg::Undo));
    assert_eq!(buffer_to_string(&model), "hello world");

    // Redo should re-apply the replacement
    update(&mut model, Msg::Document(DocumentMsg::Redo));
    assert_eq!(
        buffer_to_string(&model),
        "heXorld",
        "Redo should re-apply the replacement"
    );
    assert_eq!(model.editor().cursor().column, 3);
}

#[test]
fn test_undo_delete_backward_with_selection() {
    // Text: "hello world", select "llo w" (cols 2..7)
    let mut model = test_model_with_selection("hello world", 0, 2, 0, 7);

    // DeleteBackward with selection should delete the selection
    update(&mut model, Msg::Document(DocumentMsg::DeleteBackward));
    assert_eq!(buffer_to_string(&model), "heorld");

    // Undo should restore the deleted selection
    update(&mut model, Msg::Document(DocumentMsg::Undo));
    assert_eq!(
        buffer_to_string(&model),
        "hello world",
        "Undo should restore deleted selection"
    );
}

#[test]
fn test_undo_insert_newline_over_selection() {
    // Text: "hello world", select "llo wo" (cols 2..8)
    let mut model = test_model_with_selection("hello world", 0, 2, 0, 8);

    // Insert newline to replace selection
    update(&mut model, Msg::Document(DocumentMsg::InsertNewline));

    // Net effect: "he\nrld"
    assert_eq!(buffer_to_string(&model), "he\nrld");
    assert_eq!(model.editor().cursor().line, 1);
    assert_eq!(model.editor().cursor().column, 0);

    // Undo should fully restore original text in ONE undo operation
    update(&mut model, Msg::Document(DocumentMsg::Undo));
    assert_eq!(
        buffer_to_string(&model),
        "hello world",
        "Undo should restore the full original text"
    );
}

// ========================================================================
// Duplicate Line/Selection tests (Cmd+D)
// ========================================================================

#[test]
fn test_duplicate_line_no_selection() {
    // When no selection, Cmd+D should duplicate the current line
    let mut model = test_model("hello\nworld\n", 0, 2);

    update(&mut model, Msg::Document(DocumentMsg::Duplicate));

    assert_eq!(
        buffer_to_string(&model),
        "hello\nhello\nworld\n",
        "Current line should be duplicated below"
    );
    // Cursor should be on the duplicated line at same column
    assert_eq!(model.editor().cursor().line, 1);
    assert_eq!(model.editor().cursor().column, 2);
}

#[test]
fn test_duplicate_line_last_line_no_newline() {
    // Duplicate a line that doesn't end with newline
    let mut model = test_model("hello\nworld", 1, 3);

    update(&mut model, Msg::Document(DocumentMsg::Duplicate));

    assert_eq!(
        buffer_to_string(&model),
        "hello\nworld\nworld",
        "Last line without newline should be duplicated"
    );
    assert_eq!(model.editor().cursor().line, 2);
    assert_eq!(model.editor().cursor().column, 3);
}

#[test]
fn test_duplicate_selection_single_line() {
    // When there's a selection, duplicate the selected text after it
    let mut model = test_model_with_selection("hello world", 0, 0, 0, 5);

    update(&mut model, Msg::Document(DocumentMsg::Duplicate));

    assert_eq!(
        buffer_to_string(&model),
        "hellohello world",
        "Selection 'hello' should be duplicated after itself"
    );
    // Cursor should be at end of duplicated text
    assert_eq!(model.editor().cursor().column, 10);
}

#[test]
fn test_duplicate_selection_multiline() {
    // Duplicate a multi-line selection
    let mut model = test_model_with_selection("line1\nline2\nline3\n", 0, 0, 1, 5);

    update(&mut model, Msg::Document(DocumentMsg::Duplicate));

    // Selection is "line1\nline2", should be inserted after "line2"
    assert_eq!(
        buffer_to_string(&model),
        "line1\nline2line1\nline2\nline3\n",
        "Multi-line selection should be duplicated"
    );
}

#[test]
fn test_duplicate_can_be_undone() {
    let mut model = test_model("hello\nworld\n", 0, 2);

    update(&mut model, Msg::Document(DocumentMsg::Duplicate));
    assert_eq!(buffer_to_string(&model), "hello\nhello\nworld\n");

    update(&mut model, Msg::Document(DocumentMsg::Undo));
    assert_eq!(
        buffer_to_string(&model),
        "hello\nworld\n",
        "Duplicate should be undoable"
    );
}

// ========================================================================
// Delete Line tests (Cmd+Backspace)
// ========================================================================

#[test]
fn test_delete_line_first_line() {
    let mut model = test_model("hello\nworld\nfoo\n", 0, 2);

    update(&mut model, Msg::Document(DocumentMsg::DeleteLine));

    assert_eq!(buffer_to_string(&model), "world\nfoo\n");
    assert_eq!(model.editor().cursor().line, 0);
    assert_eq!(model.editor().cursor().column, 2);
}

#[test]
fn test_delete_line_middle_line() {
    let mut model = test_model("hello\nworld\nfoo\n", 1, 3);

    update(&mut model, Msg::Document(DocumentMsg::DeleteLine));

    assert_eq!(buffer_to_string(&model), "hello\nfoo\n");
    assert_eq!(model.editor().cursor().line, 1);
    assert_eq!(model.editor().cursor().column, 3);
}

#[test]
fn test_delete_line_last_line_with_newline() {
    let mut model = test_model("hello\nworld\n", 1, 2);

    update(&mut model, Msg::Document(DocumentMsg::DeleteLine));

    // Should delete "world\n" and move cursor to end of previous line
    assert_eq!(buffer_to_string(&model), "hello\n");
    assert_eq!(model.editor().cursor().line, 0);
    assert_eq!(model.editor().cursor().column, 5);
}

#[test]
fn test_delete_line_last_line_no_newline() {
    let mut model = test_model("hello\nworld", 1, 2);

    update(&mut model, Msg::Document(DocumentMsg::DeleteLine));

    // Should delete "\nworld" (the newline before and the content)
    assert_eq!(buffer_to_string(&model), "hello");
    assert_eq!(model.editor().cursor().line, 0);
    assert_eq!(model.editor().cursor().column, 5);
}

#[test]
fn test_delete_line_only_line() {
    let mut model = test_model("hello", 0, 2);

    update(&mut model, Msg::Document(DocumentMsg::DeleteLine));

    assert_eq!(buffer_to_string(&model), "");
    assert_eq!(model.editor().cursor().line, 0);
    assert_eq!(model.editor().cursor().column, 0);
}

#[test]
fn test_delete_line_empty_line() {
    let mut model = test_model("hello\n\nworld", 1, 0);

    update(&mut model, Msg::Document(DocumentMsg::DeleteLine));

    assert_eq!(buffer_to_string(&model), "hello\nworld");
    assert_eq!(model.editor().cursor().line, 1);
}

#[test]
fn test_delete_line_can_be_undone() {
    let mut model = test_model("hello\nworld\nfoo\n", 1, 3);

    update(&mut model, Msg::Document(DocumentMsg::DeleteLine));
    assert_eq!(buffer_to_string(&model), "hello\nfoo\n");

    update(&mut model, Msg::Document(DocumentMsg::Undo));
    assert_eq!(
        buffer_to_string(&model),
        "hello\nworld\nfoo\n",
        "Delete line should be undoable"
    );
    assert_eq!(model.editor().cursor().line, 1);
    assert_eq!(model.editor().cursor().column, 3);
}

#[test]
fn test_delete_line_cursor_column_clamped() {
    // Cursor column 10 on line with only 5 chars should clamp after delete
    let mut model = test_model("short\nlongerline\n", 1, 9);

    update(&mut model, Msg::Document(DocumentMsg::DeleteLine));

    // "longerline\n" deleted, now on "short" line
    assert_eq!(buffer_to_string(&model), "short\n");
    assert_eq!(model.editor().cursor().line, 0);
    // Column should be clamped to line length (5)
    assert_eq!(model.editor().cursor().column, 5);
}
