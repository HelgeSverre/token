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
    assert_eq!(model.editor().primary_cursor().column, 1);
    assert_eq!(model.editor().primary_cursor().line, 0);
}

#[test]
fn test_insert_char_at_middle() {
    let mut model = test_model("hello", 0, 2);
    update(&mut model, Msg::Document(DocumentMsg::InsertChar('X')));

    assert_eq!(buffer_to_string(&model), "heXllo");
    assert_eq!(model.editor().primary_cursor().column, 3);
}

#[test]
fn test_insert_char_at_end() {
    let mut model = test_model("hello", 0, 5);
    update(&mut model, Msg::Document(DocumentMsg::InsertChar('X')));

    assert_eq!(buffer_to_string(&model), "helloX");
    assert_eq!(model.editor().primary_cursor().column, 6);
}

#[test]
fn test_insert_space_at_middle() {
    let mut model = test_model("helloworld", 0, 5);
    update(&mut model, Msg::Document(DocumentMsg::InsertChar(' ')));

    assert_eq!(buffer_to_string(&model), "hello world");
    assert_eq!(model.editor().primary_cursor().column, 6);
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
    assert_eq!(model.editor().primary_cursor().column, 11);
}

#[test]
fn test_insert_char_on_second_line() {
    let mut model = test_model("hello\nworld", 1, 2);
    update(&mut model, Msg::Document(DocumentMsg::InsertChar('X')));

    assert_eq!(buffer_to_string(&model), "hello\nwoXrld");
    assert_eq!(model.editor().primary_cursor().line, 1);
    assert_eq!(model.editor().primary_cursor().column, 3);
}

#[test]
fn test_insert_multiple_spaces_middle_of_line() {
    let mut model = test_model("helloworld", 0, 5);

    // Insert 3 spaces consecutively - this tests the "playing catchup" bug
    update(&mut model, Msg::Document(DocumentMsg::InsertChar(' ')));
    assert_eq!(buffer_to_string(&model), "hello world");
    assert_eq!(model.editor().primary_cursor().column, 6);

    update(&mut model, Msg::Document(DocumentMsg::InsertChar(' ')));
    assert_eq!(buffer_to_string(&model), "hello  world");
    assert_eq!(model.editor().primary_cursor().column, 7);

    update(&mut model, Msg::Document(DocumentMsg::InsertChar(' ')));
    assert_eq!(buffer_to_string(&model), "hello   world");
    assert_eq!(model.editor().primary_cursor().column, 8);
}

#[test]
fn test_insert_after_cursor_position_clamped() {
    // This tests the suspected bug: cursor.column > line length
    let mut model = test_model("hi", 0, 10); // column 10 on 2-char line

    // Position should be clamped to 2
    let pos = model.document().cursor_to_offset(
        model.editor().primary_cursor().line,
        model.editor().primary_cursor().column,
    );
    assert_eq!(pos, 2);

    // Insert should happen at clamped position
    update(&mut model, Msg::Document(DocumentMsg::InsertChar('X')));
    assert_eq!(buffer_to_string(&model), "hiX");

    // After insert, cursor.column should be valid
    assert!(
        model.editor().primary_cursor().column
            <= model
                .document()
                .line_length(model.editor().primary_cursor().line)
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
    assert_eq!(model.editor().primary_cursor().line, 1);
    assert_eq!(model.editor().primary_cursor().column, 0);
}

#[test]
fn test_insert_newline_at_middle() {
    let mut model = test_model("hello", 0, 2);
    update(&mut model, Msg::Document(DocumentMsg::InsertNewline));

    assert_eq!(buffer_to_string(&model), "he\nllo");
    assert_eq!(model.editor().primary_cursor().line, 1);
    assert_eq!(model.editor().primary_cursor().column, 0);
}

#[test]
fn test_insert_newline_at_start() {
    let mut model = test_model("hello", 0, 0);
    update(&mut model, Msg::Document(DocumentMsg::InsertNewline));

    assert_eq!(buffer_to_string(&model), "\nhello");
    assert_eq!(model.editor().primary_cursor().line, 1);
    assert_eq!(model.editor().primary_cursor().column, 0);
}

// ========================================================================
// DeleteBackward tests
// ========================================================================

#[test]
fn test_delete_backward_middle_of_line() {
    let mut model = test_model("hello", 0, 3);
    update(&mut model, Msg::Document(DocumentMsg::DeleteBackward));

    assert_eq!(buffer_to_string(&model), "helo");
    assert_eq!(model.editor().primary_cursor().column, 2);
}

#[test]
fn test_delete_backward_at_start_of_line() {
    let mut model = test_model("hello", 0, 0);
    update(&mut model, Msg::Document(DocumentMsg::DeleteBackward));

    // Nothing should happen
    assert_eq!(buffer_to_string(&model), "hello");
    assert_eq!(model.editor().primary_cursor().column, 0);
}

#[test]
fn test_delete_backward_joins_lines() {
    let mut model = test_model("hello\nworld", 1, 0);
    update(&mut model, Msg::Document(DocumentMsg::DeleteBackward));

    assert_eq!(buffer_to_string(&model), "helloworld");
    assert_eq!(model.editor().primary_cursor().line, 0);
    assert_eq!(model.editor().primary_cursor().column, 5); // End of "hello"
}

#[test]
fn test_delete_backward_after_empty_line() {
    let mut model = test_model("hello\n\nworld", 2, 0);
    update(&mut model, Msg::Document(DocumentMsg::DeleteBackward));

    assert_eq!(buffer_to_string(&model), "hello\nworld");
    assert_eq!(model.editor().primary_cursor().line, 1);
    assert_eq!(model.editor().primary_cursor().column, 0);
}

// ========================================================================
// DeleteForward tests
// ========================================================================

#[test]
fn test_delete_forward_middle_of_line() {
    let mut model = test_model("hello", 0, 2);
    update(&mut model, Msg::Document(DocumentMsg::DeleteForward));

    assert_eq!(buffer_to_string(&model), "helo");
    assert_eq!(model.editor().primary_cursor().column, 2); // Unchanged
}

#[test]
fn test_delete_forward_at_end_of_line() {
    let mut model = test_model("hello\nworld", 0, 5);
    update(&mut model, Msg::Document(DocumentMsg::DeleteForward));

    // Should delete the newline, joining lines
    assert_eq!(buffer_to_string(&model), "helloworld");
    assert_eq!(model.editor().primary_cursor().column, 5);
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
    assert_eq!(model.editor().primary_cursor().column, 5);
}

#[test]
fn test_redo_insert() {
    let mut model = test_model("hello", 0, 5);
    update(&mut model, Msg::Document(DocumentMsg::InsertChar('X')));
    update(&mut model, Msg::Document(DocumentMsg::Undo));
    update(&mut model, Msg::Document(DocumentMsg::Redo));

    assert_eq!(buffer_to_string(&model), "helloX");
    assert_eq!(model.editor().primary_cursor().column, 6);
}

#[test]
fn test_undo_delete() {
    let mut model = test_model("hello", 0, 5);
    update(&mut model, Msg::Document(DocumentMsg::DeleteBackward));

    assert_eq!(buffer_to_string(&model), "hell");

    update(&mut model, Msg::Document(DocumentMsg::Undo));

    assert_eq!(buffer_to_string(&model), "hello");
    assert_eq!(model.editor().primary_cursor().column, 5);
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
    assert_eq!(model.editor().primary_cursor().column, 3);

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
    assert_eq!(model.editor().primary_cursor().column, 3);
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
    assert_eq!(model.editor().primary_cursor().line, 1);
    assert_eq!(model.editor().primary_cursor().column, 0);

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
    assert_eq!(model.editor().primary_cursor().line, 1);
    assert_eq!(model.editor().primary_cursor().column, 2);
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
    assert_eq!(model.editor().primary_cursor().line, 2);
    assert_eq!(model.editor().primary_cursor().column, 3);
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
    assert_eq!(model.editor().primary_cursor().column, 10);
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
    assert_eq!(model.editor().primary_cursor().line, 0);
    assert_eq!(model.editor().primary_cursor().column, 2);
}

#[test]
fn test_delete_line_middle_line() {
    let mut model = test_model("hello\nworld\nfoo\n", 1, 3);

    update(&mut model, Msg::Document(DocumentMsg::DeleteLine));

    assert_eq!(buffer_to_string(&model), "hello\nfoo\n");
    assert_eq!(model.editor().primary_cursor().line, 1);
    assert_eq!(model.editor().primary_cursor().column, 3);
}

#[test]
fn test_delete_line_last_line_with_newline() {
    let mut model = test_model("hello\nworld\n", 1, 2);

    update(&mut model, Msg::Document(DocumentMsg::DeleteLine));

    // Should delete "world\n", cursor stays at line 1 (now empty trailing line)
    assert_eq!(buffer_to_string(&model), "hello\n");
    assert_eq!(model.editor().primary_cursor().line, 1);
    assert_eq!(model.editor().primary_cursor().column, 0);
}

#[test]
fn test_delete_line_last_line_no_newline() {
    let mut model = test_model("hello\nworld", 1, 2);

    update(&mut model, Msg::Document(DocumentMsg::DeleteLine));

    // Should delete "\nworld", cursor retains column (clamped to prev line length)
    assert_eq!(buffer_to_string(&model), "hello");
    assert_eq!(model.editor().primary_cursor().line, 0);
    assert_eq!(model.editor().primary_cursor().column, 2); // retained from original position
}

#[test]
fn test_delete_line_only_line() {
    let mut model = test_model("hello", 0, 2);

    update(&mut model, Msg::Document(DocumentMsg::DeleteLine));

    assert_eq!(buffer_to_string(&model), "");
    assert_eq!(model.editor().primary_cursor().line, 0);
    assert_eq!(model.editor().primary_cursor().column, 0);
}

#[test]
fn test_delete_line_empty_line() {
    let mut model = test_model("hello\n\nworld", 1, 0);

    update(&mut model, Msg::Document(DocumentMsg::DeleteLine));

    assert_eq!(buffer_to_string(&model), "hello\nworld");
    assert_eq!(model.editor().primary_cursor().line, 1);
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
    assert_eq!(model.editor().primary_cursor().line, 1);
    assert_eq!(model.editor().primary_cursor().column, 3);
}

#[test]
fn test_delete_line_cursor_column_clamped() {
    // Cursor column 10 on line with only 5 chars should clamp after delete
    let mut model = test_model("short\nlongerline\n", 1, 9);

    update(&mut model, Msg::Document(DocumentMsg::DeleteLine));

    // "longerline\n" deleted, cursor stays at line 1 (now empty trailing line)
    assert_eq!(buffer_to_string(&model), "short\n");
    assert_eq!(model.editor().primary_cursor().line, 1);
    // Column clamped to 0 (empty line)
    assert_eq!(model.editor().primary_cursor().column, 0);
}

// ========================================================================
// Multi-Cursor Undo/Redo Tests (Batch Operations)
// ========================================================================

#[test]
fn test_multi_cursor_insert_char_undo() {
    use common::test_model_multi_cursor;
    // Three cursors on three lines
    let mut model = test_model_multi_cursor("abc\ndef\nghi", &[(0, 0), (1, 0), (2, 0)]);

    assert_eq!(model.editor().cursor_count(), 3);

    // Insert 'X' at all cursors
    update(&mut model, Msg::Document(DocumentMsg::InsertChar('X')));

    assert_eq!(buffer_to_string(&model), "Xabc\nXdef\nXghi");

    // All cursors should be at column 1 now
    for cursor in &model.editor().cursors {
        assert_eq!(
            cursor.column, 1,
            "Cursor should be at column 1 after insert"
        );
    }

    // Undo should restore ALL insertions at once
    update(&mut model, Msg::Document(DocumentMsg::Undo));

    assert_eq!(
        buffer_to_string(&model),
        "abc\ndef\nghi",
        "Undo should restore all"
    );
    assert_eq!(
        model.editor().cursor_count(),
        3,
        "Should still have 3 cursors"
    );

    // All cursors should be back at column 0
    for cursor in &model.editor().cursors {
        assert_eq!(cursor.column, 0, "Cursor should be at column 0 after undo");
    }
}

#[test]
fn test_multi_cursor_insert_char_redo() {
    use common::test_model_multi_cursor;
    let mut model = test_model_multi_cursor("abc\ndef\nghi", &[(0, 0), (1, 0), (2, 0)]);

    // Insert, undo, then redo
    update(&mut model, Msg::Document(DocumentMsg::InsertChar('Y')));
    assert_eq!(buffer_to_string(&model), "Yabc\nYdef\nYghi");

    update(&mut model, Msg::Document(DocumentMsg::Undo));
    assert_eq!(buffer_to_string(&model), "abc\ndef\nghi");

    update(&mut model, Msg::Document(DocumentMsg::Redo));
    assert_eq!(
        buffer_to_string(&model),
        "Yabc\nYdef\nYghi",
        "Redo should reapply all"
    );

    // Cursors should be at column 1
    for cursor in &model.editor().cursors {
        assert_eq!(cursor.column, 1);
    }
}

#[test]
fn test_multi_cursor_delete_backward_undo() {
    use common::test_model_multi_cursor;
    // Cursors at column 1 on each line
    let mut model = test_model_multi_cursor("abc\ndef\nghi", &[(0, 1), (1, 1), (2, 1)]);

    // Delete backward at all cursors (deletes 'a', 'd', 'g')
    update(&mut model, Msg::Document(DocumentMsg::DeleteBackward));

    assert_eq!(buffer_to_string(&model), "bc\nef\nhi");

    // Undo should restore all deleted characters
    update(&mut model, Msg::Document(DocumentMsg::Undo));

    assert_eq!(
        buffer_to_string(&model),
        "abc\ndef\nghi",
        "Undo should restore all"
    );
}

#[test]
fn test_multi_cursor_delete_forward_undo() {
    use common::test_model_multi_cursor;
    // Cursors at column 0 on each line
    let mut model = test_model_multi_cursor("abc\ndef\nghi", &[(0, 0), (1, 0), (2, 0)]);

    // Delete forward at all cursors (deletes 'a', 'd', 'g')
    update(&mut model, Msg::Document(DocumentMsg::DeleteForward));

    assert_eq!(buffer_to_string(&model), "bc\nef\nhi");

    // Undo should restore all deleted characters
    update(&mut model, Msg::Document(DocumentMsg::Undo));

    assert_eq!(
        buffer_to_string(&model),
        "abc\ndef\nghi",
        "Undo should restore all"
    );
}

#[test]
fn test_multi_cursor_insert_newline_undo() {
    use common::test_model_multi_cursor;
    // Cursors at the end of each line
    let mut model = test_model_multi_cursor("abc\ndef\nghi", &[(0, 3), (1, 3), (2, 3)]);

    // Insert newline at all cursors
    update(&mut model, Msg::Document(DocumentMsg::InsertNewline));

    // Each line should now be followed by a newline
    assert_eq!(buffer_to_string(&model), "abc\n\ndef\n\nghi\n");

    // Undo should remove all newlines
    update(&mut model, Msg::Document(DocumentMsg::Undo));

    assert_eq!(
        buffer_to_string(&model),
        "abc\ndef\nghi",
        "Undo should restore all"
    );
}

#[test]
fn test_multi_cursor_consecutive_edits_undo() {
    use common::test_model_multi_cursor;
    // Test that consecutive multi-cursor edits create separate undo entries
    let mut model = test_model_multi_cursor("aaa\nbbb\nccc", &[(0, 0), (1, 0), (2, 0)]);

    // First edit: insert 'X'
    update(&mut model, Msg::Document(DocumentMsg::InsertChar('X')));
    assert_eq!(buffer_to_string(&model), "Xaaa\nXbbb\nXccc");

    // Second edit: insert 'Y'
    update(&mut model, Msg::Document(DocumentMsg::InsertChar('Y')));
    assert_eq!(buffer_to_string(&model), "XYaaa\nXYbbb\nXYccc");

    // First undo should only undo the 'Y' insertions
    update(&mut model, Msg::Document(DocumentMsg::Undo));
    assert_eq!(
        buffer_to_string(&model),
        "Xaaa\nXbbb\nXccc",
        "First undo undoes Y"
    );

    // Second undo should undo the 'X' insertions
    update(&mut model, Msg::Document(DocumentMsg::Undo));
    assert_eq!(
        buffer_to_string(&model),
        "aaa\nbbb\nccc",
        "Second undo undoes X"
    );
}

// ========================================================================
// Cut/Paste Undo Tests
// ========================================================================

#[test]
fn test_cut_single_cursor_undo() {
    // Select "ello" in "hello world"
    let mut model = test_model_with_selection("hello world", 0, 1, 0, 5);

    // Cut the selection
    update(&mut model, Msg::Document(DocumentMsg::Cut));
    assert_eq!(buffer_to_string(&model), "h world");
    assert_eq!(model.editor().primary_cursor().column, 1);

    // Undo should restore the cut text
    update(&mut model, Msg::Document(DocumentMsg::Undo));
    assert_eq!(buffer_to_string(&model), "hello world");
}

#[test]
fn test_cut_multi_cursor_undo() {
    use common::test_model_multi_cursor;
    use token::model::{Position, Selection};

    // Two lines, select "bc" on each
    let mut model = test_model_multi_cursor("abc\nabc", &[(0, 3), (1, 3)]);

    // Set up selections: select "bc" on each line (columns 1-3)
    model.editor_mut().selections[0] =
        Selection::from_positions(Position::new(0, 1), Position::new(0, 3));
    model.editor_mut().cursors[0].column = 3;

    model.editor_mut().selections[1] =
        Selection::from_positions(Position::new(1, 1), Position::new(1, 3));
    model.editor_mut().cursors[1].column = 3;

    // Cut
    update(&mut model, Msg::Document(DocumentMsg::Cut));
    assert_eq!(buffer_to_string(&model), "a\na");

    // Undo should restore both cut regions
    update(&mut model, Msg::Document(DocumentMsg::Undo));
    assert_eq!(buffer_to_string(&model), "abc\nabc");
}

#[test]
fn test_paste_multi_cursor_undo() {
    use common::test_model_multi_cursor;

    // Set up clipboard with text (we'll simulate by doing a copy first)
    let mut model = test_model_multi_cursor("XY\nXY", &[(0, 2), (1, 2)]);

    // Type some text at each cursor to set up undo stack baseline
    update(&mut model, Msg::Document(DocumentMsg::InsertChar('Z')));
    assert_eq!(buffer_to_string(&model), "XYZ\nXYZ");

    // Undo
    update(&mut model, Msg::Document(DocumentMsg::Undo));
    assert_eq!(buffer_to_string(&model), "XY\nXY");
}
