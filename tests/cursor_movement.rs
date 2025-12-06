//! Cursor movement tests - basic movement, smart home/end, word navigation

mod common;

use common::test_model;
use token::messages::{Direction, EditorMsg, Msg};
use token::update::update;

// ========================================================================
// cursor_buffer_position() tests
// ========================================================================

#[test]
fn test_cursor_buffer_position_start_of_file() {
    let model = test_model("hello\nworld\n", 0, 0);
    assert_eq!(
        model
            .document()
            .cursor_to_offset(model.editor().cursor().line, model.editor().cursor().column),
        0
    );
}

#[test]
fn test_cursor_buffer_position_middle_of_first_line() {
    let model = test_model("hello\nworld\n", 0, 3);
    assert_eq!(
        model
            .document()
            .cursor_to_offset(model.editor().cursor().line, model.editor().cursor().column),
        3
    ); // "hel|lo"
}

#[test]
fn test_cursor_buffer_position_end_of_first_line() {
    let model = test_model("hello\nworld\n", 0, 5);
    assert_eq!(
        model
            .document()
            .cursor_to_offset(model.editor().cursor().line, model.editor().cursor().column),
        5
    ); // "hello|"
}

#[test]
fn test_cursor_buffer_position_start_of_second_line() {
    let model = test_model("hello\nworld\n", 1, 0);
    // "hello\n" = 6 chars, so position 6 is start of "world"
    assert_eq!(
        model
            .document()
            .cursor_to_offset(model.editor().cursor().line, model.editor().cursor().column),
        6
    );
}

#[test]
fn test_cursor_buffer_position_middle_of_second_line() {
    let model = test_model("hello\nworld\n", 1, 3);
    // "hello\n" = 6 chars, + 3 = 9
    assert_eq!(
        model
            .document()
            .cursor_to_offset(model.editor().cursor().line, model.editor().cursor().column),
        9
    ); // "wor|ld"
}

#[test]
fn test_cursor_buffer_position_empty_line() {
    let model = test_model("hello\n\nworld\n", 1, 0);
    // "hello\n" = 6 chars, empty line at position 6
    assert_eq!(
        model
            .document()
            .cursor_to_offset(model.editor().cursor().line, model.editor().cursor().column),
        6
    );
}

#[test]
fn test_cursor_buffer_position_after_empty_line() {
    let model = test_model("hello\n\nworld\n", 2, 0);
    // "hello\n" = 6, "\n" = 1, so "world" starts at 7
    assert_eq!(
        model
            .document()
            .cursor_to_offset(model.editor().cursor().line, model.editor().cursor().column),
        7
    );
}

#[test]
fn test_cursor_buffer_position_clamped_column() {
    // Column exceeds line length - should be clamped
    let model = test_model("hi\nworld\n", 0, 10);
    // Line "hi" has length 2, so column should clamp to 2
    assert_eq!(
        model
            .document()
            .cursor_to_offset(model.editor().cursor().line, model.editor().cursor().column),
        2
    );
}

// ========================================================================
// current_line_length() tests
// ========================================================================

#[test]
fn test_current_line_length_with_newline() {
    let model = test_model("hello\nworld\n", 0, 0);
    // "hello\n" has 6 chars, but length should be 5 (excluding newline)
    assert_eq!(
        model.document().line_length(model.editor().cursor().line),
        5
    );
}

#[test]
fn test_current_line_length_without_newline() {
    let model = test_model("hello", 0, 0);
    // "hello" has no newline, length is 5
    assert_eq!(
        model.document().line_length(model.editor().cursor().line),
        5
    );
}

#[test]
fn test_current_line_length_empty_line() {
    let model = test_model("hello\n\nworld\n", 1, 0);
    // Empty line has length 0
    assert_eq!(
        model.document().line_length(model.editor().cursor().line),
        0
    );
}

#[test]
fn test_current_line_length_last_line_with_newline() {
    let model = test_model("hello\nworld\n", 1, 0);
    // "world\n" has 6 chars, length should be 5
    assert_eq!(
        model.document().line_length(model.editor().cursor().line),
        5
    );
}

// ========================================================================
// Cursor movement tests
// ========================================================================

#[test]
fn test_move_cursor_left() {
    let mut model = test_model("hello", 0, 3);
    update(
        &mut model,
        Msg::Editor(EditorMsg::MoveCursor(Direction::Left)),
    );

    assert_eq!(model.editor().cursor().column, 2);
}

#[test]
fn test_move_cursor_left_at_start_of_line() {
    let mut model = test_model("hello\nworld", 1, 0);
    update(
        &mut model,
        Msg::Editor(EditorMsg::MoveCursor(Direction::Left)),
    );

    // Should move to end of previous line
    assert_eq!(model.editor().cursor().line, 0);
    assert_eq!(model.editor().cursor().column, 5);
}

#[test]
fn test_move_cursor_right() {
    let mut model = test_model("hello", 0, 2);
    update(
        &mut model,
        Msg::Editor(EditorMsg::MoveCursor(Direction::Right)),
    );

    assert_eq!(model.editor().cursor().column, 3);
}

#[test]
fn test_move_cursor_right_at_end_of_line() {
    let mut model = test_model("hello\nworld", 0, 5);
    update(
        &mut model,
        Msg::Editor(EditorMsg::MoveCursor(Direction::Right)),
    );

    // Should move to start of next line
    assert_eq!(model.editor().cursor().line, 1);
    assert_eq!(model.editor().cursor().column, 0);
}

#[test]
fn test_move_cursor_up() {
    let mut model = test_model("hello\nworld", 1, 3);
    update(
        &mut model,
        Msg::Editor(EditorMsg::MoveCursor(Direction::Up)),
    );

    assert_eq!(model.editor().cursor().line, 0);
    assert_eq!(model.editor().cursor().column, 3);
}

#[test]
fn test_move_cursor_up_preserves_desired_column() {
    let mut model = test_model("hello\nhi\nworld", 0, 4);

    // Move down to short line "hi"
    update(
        &mut model,
        Msg::Editor(EditorMsg::MoveCursor(Direction::Down)),
    );
    assert_eq!(model.editor().cursor().line, 1);
    assert_eq!(model.editor().cursor().column, 2); // Clamped to "hi" length

    // Move down to "world"
    update(
        &mut model,
        Msg::Editor(EditorMsg::MoveCursor(Direction::Down)),
    );
    assert_eq!(model.editor().cursor().line, 2);
    assert_eq!(model.editor().cursor().column, 4); // Restored to desired column
}

#[test]
fn test_move_cursor_down() {
    let mut model = test_model("hello\nworld", 0, 3);
    update(
        &mut model,
        Msg::Editor(EditorMsg::MoveCursor(Direction::Down)),
    );

    assert_eq!(model.editor().cursor().line, 1);
    assert_eq!(model.editor().cursor().column, 3);
}

// ========================================================================
// Smart Home/End tests (toggle between line edge and non-whitespace)
// ========================================================================

#[test]
fn test_smart_home_from_middle() {
    // From middle of line → first non-whitespace
    let mut model = test_model("    hello", 0, 6);
    update(&mut model, Msg::Editor(EditorMsg::MoveCursorLineStart));
    assert_eq!(model.editor().cursor().column, 4); // First non-ws is at column 4
}

#[test]
fn test_smart_home_from_column_zero() {
    // From column 0 → first non-whitespace
    let mut model = test_model("    hello", 0, 0);
    update(&mut model, Msg::Editor(EditorMsg::MoveCursorLineStart));
    assert_eq!(model.editor().cursor().column, 4); // First non-ws is at column 4
}

#[test]
fn test_smart_home_toggle() {
    // From first non-ws → back to column 0
    let mut model = test_model("    hello", 0, 4);
    update(&mut model, Msg::Editor(EditorMsg::MoveCursorLineStart));
    assert_eq!(model.editor().cursor().column, 0);
}

#[test]
fn test_smart_home_no_leading_whitespace() {
    // Line with no leading whitespace: stays at 0
    let mut model = test_model("hello", 0, 0);
    update(&mut model, Msg::Editor(EditorMsg::MoveCursorLineStart));
    assert_eq!(model.editor().cursor().column, 0); // first_non_ws is 0, so stays at 0
}

#[test]
fn test_smart_home_empty_line() {
    // Empty line: stays at 0
    let mut model = test_model("", 0, 0);
    update(&mut model, Msg::Editor(EditorMsg::MoveCursorLineStart));
    assert_eq!(model.editor().cursor().column, 0);
}

#[test]
fn test_smart_home_whitespace_only_line() {
    // Whitespace-only line: 0 → end of whitespace
    let mut model = test_model("    ", 0, 0);
    update(&mut model, Msg::Editor(EditorMsg::MoveCursorLineStart));
    assert_eq!(model.editor().cursor().column, 4); // All whitespace, so first_non_ws is line length
}

#[test]
fn test_smart_end_from_middle() {
    // From middle of line → last non-whitespace
    let mut model = test_model("hello    ", 0, 3);
    update(&mut model, Msg::Editor(EditorMsg::MoveCursorLineEnd));
    assert_eq!(model.editor().cursor().column, 5); // After 'o' in "hello"
}

#[test]
fn test_smart_end_from_line_end() {
    // From end of line → last non-whitespace
    let mut model = test_model("hello    ", 0, 9);
    update(&mut model, Msg::Editor(EditorMsg::MoveCursorLineEnd));
    assert_eq!(model.editor().cursor().column, 5); // After 'o' in "hello"
}

#[test]
fn test_smart_end_toggle() {
    // From last non-ws → back to end
    let mut model = test_model("hello    ", 0, 5);
    update(&mut model, Msg::Editor(EditorMsg::MoveCursorLineEnd));
    assert_eq!(model.editor().cursor().column, 9);
}

#[test]
fn test_smart_end_no_trailing_whitespace() {
    // Line with no trailing whitespace: stays at end
    let mut model = test_model("hello", 0, 5);
    update(&mut model, Msg::Editor(EditorMsg::MoveCursorLineEnd));
    assert_eq!(model.editor().cursor().column, 5); // last_non_ws = line_end, so stays
}

#[test]
fn test_smart_end_empty_line() {
    // Empty line: stays at 0
    let mut model = test_model("", 0, 0);
    update(&mut model, Msg::Editor(EditorMsg::MoveCursorLineEnd));
    assert_eq!(model.editor().cursor().column, 0);
}

#[test]
fn test_smart_end_whitespace_only_line() {
    // Whitespace-only line: end → 0 (last_non_ws is 0)
    let mut model = test_model("    ", 0, 4);
    update(&mut model, Msg::Editor(EditorMsg::MoveCursorLineEnd));
    assert_eq!(model.editor().cursor().column, 0); // No non-whitespace chars
}

// ========================================================================
// Word navigation tests (IntelliJ-style: whitespace is a navigable unit)
// ========================================================================

#[test]
fn test_word_left_from_end() {
    let mut model = test_model("hello world", 0, 11);
    update(
        &mut model,
        Msg::Editor(EditorMsg::MoveCursorWord(Direction::Left)),
    );

    // Should move to start of "world"
    assert_eq!(model.editor().cursor().column, 6);
}

#[test]
fn test_word_left_stops_at_whitespace_start() {
    // IntelliJ-style: whitespace is its own navigable unit
    // From middle of whitespace, go to start of whitespace (end of "hello")
    let mut model = test_model("hello   world", 0, 8);
    update(
        &mut model,
        Msg::Editor(EditorMsg::MoveCursorWord(Direction::Left)),
    );

    // Should stop at start of whitespace (end of "hello")
    assert_eq!(model.editor().cursor().column, 5);
}

#[test]
fn test_word_right_stops_at_word_end() {
    // IntelliJ-style: from start of word, go to END of current word
    let mut model = test_model("hello world", 0, 0);
    update(
        &mut model,
        Msg::Editor(EditorMsg::MoveCursorWord(Direction::Right)),
    );

    // Should move to end of "hello", not past the space
    assert_eq!(model.editor().cursor().column, 5);
}

#[test]
fn test_word_right_through_whitespace() {
    // From end of "hello" (start of whitespace), go through whitespace to start of "world"
    let mut model = test_model("hello   world", 0, 5);
    update(
        &mut model,
        Msg::Editor(EditorMsg::MoveCursorWord(Direction::Right)),
    );

    // Should stop at end of whitespace (start of "world")
    assert_eq!(model.editor().cursor().column, 8);
}

#[test]
fn test_word_left_through_word() {
    // From start of "world", go to start of whitespace (end of "hello")
    let mut model = test_model("hello   world", 0, 8);
    update(
        &mut model,
        Msg::Editor(EditorMsg::MoveCursorWord(Direction::Left)),
    );

    // Should stop at start of whitespace
    assert_eq!(model.editor().cursor().column, 5);
}

#[test]
fn test_word_navigation_full_sequence() {
    // Test full navigation through: "hello     world"
    // Positions: h=0, e=1, l=2, l=3, o=4, ' '=5,6,7,8,9, w=10, o=11, r=12, l=13, d=14
    let mut model = test_model("hello     world", 0, 0);

    // From 0, word right should go to 5 (end of "hello")
    update(
        &mut model,
        Msg::Editor(EditorMsg::MoveCursorWord(Direction::Right)),
    );
    assert_eq!(model.editor().cursor().column, 5);

    // From 5, word right should go to 10 (end of whitespace = start of "world")
    update(
        &mut model,
        Msg::Editor(EditorMsg::MoveCursorWord(Direction::Right)),
    );
    assert_eq!(model.editor().cursor().column, 10);

    // From 10, word right should go to 15 (end of "world")
    update(
        &mut model,
        Msg::Editor(EditorMsg::MoveCursorWord(Direction::Right)),
    );
    assert_eq!(model.editor().cursor().column, 15);

    // From 15, word left should go to 10 (start of "world")
    update(
        &mut model,
        Msg::Editor(EditorMsg::MoveCursorWord(Direction::Left)),
    );
    assert_eq!(model.editor().cursor().column, 10);

    // From 10, word left should go to 5 (start of whitespace = end of "hello")
    update(
        &mut model,
        Msg::Editor(EditorMsg::MoveCursorWord(Direction::Left)),
    );
    assert_eq!(model.editor().cursor().column, 5);

    // From 5, word left should go to 0 (start of "hello")
    update(
        &mut model,
        Msg::Editor(EditorMsg::MoveCursorWord(Direction::Left)),
    );
    assert_eq!(model.editor().cursor().column, 0);
}

#[test]
fn test_word_navigation_with_punctuation() {
    // Test: "hello, world"
    // Positions: h=0, e=1, l=2, l=3, o=4, ,=5, ' '=6, w=7, o=8, r=9, l=10, d=11
    let mut model = test_model("hello, world", 0, 0);

    // From 0, word right should go to 5 (end of "hello")
    update(
        &mut model,
        Msg::Editor(EditorMsg::MoveCursorWord(Direction::Right)),
    );
    assert_eq!(model.editor().cursor().column, 5);

    // From 5, word right should go to 6 (end of punctuation ",")
    update(
        &mut model,
        Msg::Editor(EditorMsg::MoveCursorWord(Direction::Right)),
    );
    assert_eq!(model.editor().cursor().column, 6);

    // From 6, word right should go to 7 (end of space)
    update(
        &mut model,
        Msg::Editor(EditorMsg::MoveCursorWord(Direction::Right)),
    );
    assert_eq!(model.editor().cursor().column, 7);

    // From 7, word right should go to 12 (end of "world")
    update(
        &mut model,
        Msg::Editor(EditorMsg::MoveCursorWord(Direction::Right)),
    );
    assert_eq!(model.editor().cursor().column, 12);
}

// ========================================================================
// Multi-cursor movement tests
// ========================================================================

use token::model::Cursor;

#[test]
fn test_multi_cursor_arrow_left_moves_all() {
    let mut model = test_model("hello world", 0, 5);
    // Add a second cursor at column 10
    model.editor_mut().cursors.push(Cursor::at(0, 10));
    model
        .editor_mut()
        .selections
        .push(token::model::Selection::new(token::model::Position::new(
            0, 10,
        )));

    update(
        &mut model,
        Msg::Editor(EditorMsg::MoveCursor(Direction::Left)),
    );

    assert_eq!(model.editor().cursors.len(), 2);
    assert_eq!(model.editor().cursors[0].column, 4);
    assert_eq!(model.editor().cursors[1].column, 9);
}

#[test]
fn test_multi_cursor_arrow_right_moves_all() {
    let mut model = test_model("hello world", 0, 0);
    model.editor_mut().cursors.push(Cursor::at(0, 5));
    model
        .editor_mut()
        .selections
        .push(token::model::Selection::new(token::model::Position::new(
            0, 5,
        )));

    update(
        &mut model,
        Msg::Editor(EditorMsg::MoveCursor(Direction::Right)),
    );

    assert_eq!(model.editor().cursors.len(), 2);
    assert_eq!(model.editor().cursors[0].column, 1);
    assert_eq!(model.editor().cursors[1].column, 6);
}

#[test]
fn test_multi_cursor_arrow_up_moves_all() {
    let mut model = test_model("line one\nline two\nline three", 2, 5);
    model.editor_mut().cursors.push(Cursor::at(1, 3));
    model
        .editor_mut()
        .selections
        .push(token::model::Selection::new(token::model::Position::new(
            1, 3,
        )));

    update(
        &mut model,
        Msg::Editor(EditorMsg::MoveCursor(Direction::Up)),
    );

    assert_eq!(model.editor().cursors.len(), 2);
    assert_eq!(model.editor().cursors[0].line, 1);
    assert_eq!(model.editor().cursors[1].line, 0);
}

#[test]
fn test_multi_cursor_arrow_down_moves_all() {
    let mut model = test_model("line one\nline two\nline three", 0, 3);
    model.editor_mut().cursors.push(Cursor::at(1, 5));
    model
        .editor_mut()
        .selections
        .push(token::model::Selection::new(token::model::Position::new(
            1, 5,
        )));

    update(
        &mut model,
        Msg::Editor(EditorMsg::MoveCursor(Direction::Down)),
    );

    assert_eq!(model.editor().cursors.len(), 2);
    assert_eq!(model.editor().cursors[0].line, 1);
    assert_eq!(model.editor().cursors[1].line, 2);
}

#[test]
fn test_multi_cursor_movement_deduplicates_on_collision() {
    let mut model = test_model("abc", 0, 1);
    model.editor_mut().cursors.push(Cursor::at(0, 2));
    model
        .editor_mut()
        .selections
        .push(token::model::Selection::new(token::model::Position::new(
            0, 2,
        )));

    // Move left - both cursors move, cursor at 2 becomes 1, cursor at 1 becomes 0
    update(
        &mut model,
        Msg::Editor(EditorMsg::MoveCursor(Direction::Left)),
    );

    // Should still have 2 cursors (at 0 and 1)
    assert_eq!(model.editor().cursors.len(), 2);

    // Move left again - cursor at 1 becomes 0, cursor at 0 stays at 0 → collision
    update(
        &mut model,
        Msg::Editor(EditorMsg::MoveCursor(Direction::Left)),
    );

    // Should deduplicate to 1 cursor
    assert_eq!(model.editor().cursors.len(), 1);
    assert_eq!(model.editor().cursors[0].column, 0);
}

#[test]
fn test_multi_cursor_home_moves_all() {
    let mut model = test_model("  hello\n  world", 0, 5);
    model.editor_mut().cursors.push(Cursor::at(1, 5));
    model
        .editor_mut()
        .selections
        .push(token::model::Selection::new(token::model::Position::new(
            1, 5,
        )));

    update(&mut model, Msg::Editor(EditorMsg::MoveCursorLineStart));

    assert_eq!(model.editor().cursors.len(), 2);
    // Both should move to first non-whitespace (column 2)
    assert_eq!(model.editor().cursors[0].column, 2);
    assert_eq!(model.editor().cursors[1].column, 2);
}

#[test]
fn test_multi_cursor_end_moves_all() {
    let mut model = test_model("hello  \nworld  ", 0, 0);
    model.editor_mut().cursors.push(Cursor::at(1, 0));
    model
        .editor_mut()
        .selections
        .push(token::model::Selection::new(token::model::Position::new(
            1, 0,
        )));

    update(&mut model, Msg::Editor(EditorMsg::MoveCursorLineEnd));

    assert_eq!(model.editor().cursors.len(), 2);
    // Both should move to last non-whitespace (column 5 for "hello" and "world")
    assert_eq!(model.editor().cursors[0].column, 5);
    assert_eq!(model.editor().cursors[1].column, 5);
}

#[test]
fn test_multi_cursor_word_right_moves_all() {
    let mut model = test_model("hello world test", 0, 0);
    model.editor_mut().cursors.push(Cursor::at(0, 6));
    model
        .editor_mut()
        .selections
        .push(token::model::Selection::new(token::model::Position::new(
            0, 6,
        )));

    update(
        &mut model,
        Msg::Editor(EditorMsg::MoveCursorWord(Direction::Right)),
    );

    assert_eq!(model.editor().cursors.len(), 2);
    // First cursor: 0 → 5 (end of "hello")
    assert_eq!(model.editor().cursors[0].column, 5);
    // Second cursor: 6 → 11 (end of "world")
    assert_eq!(model.editor().cursors[1].column, 11);
}

#[test]
fn test_multi_cursor_selection_extends_all() {
    let mut model = test_model("hello world", 0, 0);
    model.editor_mut().cursors.push(Cursor::at(0, 6));
    model
        .editor_mut()
        .selections
        .push(token::model::Selection::new(token::model::Position::new(
            0, 6,
        )));

    // Shift+Right to extend selection
    update(
        &mut model,
        Msg::Editor(EditorMsg::MoveCursorWithSelection(Direction::Right)),
    );

    assert_eq!(model.editor().cursors.len(), 2);
    // Both cursors moved
    assert_eq!(model.editor().cursors[0].column, 1);
    assert_eq!(model.editor().cursors[1].column, 7);
    // Both selections extended (anchor at original, head at new)
    assert_eq!(model.editor().selections[0].anchor.column, 0);
    assert_eq!(model.editor().selections[0].head.column, 1);
    assert_eq!(model.editor().selections[1].anchor.column, 6);
    assert_eq!(model.editor().selections[1].head.column, 7);
}

#[test]
fn test_multi_cursor_vertical_preserves_desired_column() {
    // Test that each cursor maintains its own desired_column through ragged lines
    let mut model = test_model("long line here\nshort\nanother long line", 0, 12);
    model.editor_mut().cursors.push(Cursor::at(2, 15));
    model
        .editor_mut()
        .selections
        .push(token::model::Selection::new(token::model::Position::new(
            2, 15,
        )));

    // Move down - first cursor goes from line 0 col 12 to line 1, col should clamp to 5
    // Second cursor at line 2 can't go down (already at last line)
    update(
        &mut model,
        Msg::Editor(EditorMsg::MoveCursor(Direction::Down)),
    );

    // First cursor moved down, clamped to line length
    assert_eq!(model.editor().cursors[0].line, 1);
    assert_eq!(model.editor().cursors[0].column, 5); // "short" has length 5
    assert_eq!(model.editor().cursors[0].desired_column, Some(12));

    // Second cursor stayed at line 2 (can't go past last line)
    assert_eq!(model.editor().cursors[1].line, 2);

    // Move down again - first cursor goes to line 2, should restore to column 12
    update(
        &mut model,
        Msg::Editor(EditorMsg::MoveCursor(Direction::Down)),
    );

    assert_eq!(model.editor().cursors[0].line, 2);
    assert_eq!(model.editor().cursors[0].column, 12); // restored from desired_column
}
