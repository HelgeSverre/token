//! Selection and multi-cursor tests

mod common;

use common::test_model;
use token::messages::{Direction, EditorMsg, Msg};
use token::model::{Cursor, Position, Selection};
use token::update::update;

// ========================================================================
// Selection Helper Method Tests
// ========================================================================

#[test]
fn test_selection_extend_to() {
    let mut sel = Selection::new(Position::new(0, 5));
    sel.extend_to(Position::new(0, 10));

    assert_eq!(sel.anchor, Position::new(0, 5));
    assert_eq!(sel.head, Position::new(0, 10));
    assert!(!sel.is_empty());
}

#[test]
fn test_selection_collapse_to_start() {
    let mut sel = Selection::from_anchor_head(Position::new(0, 5), Position::new(0, 10));
    sel.collapse_to_start();

    assert_eq!(sel.anchor, Position::new(0, 5));
    assert_eq!(sel.head, Position::new(0, 5));
    assert!(sel.is_empty());
}

#[test]
fn test_selection_collapse_to_end() {
    let mut sel = Selection::from_anchor_head(Position::new(0, 5), Position::new(0, 10));
    sel.collapse_to_end();

    assert_eq!(sel.anchor, Position::new(0, 10));
    assert_eq!(sel.head, Position::new(0, 10));
    assert!(sel.is_empty());
}

#[test]
fn test_selection_contains() {
    let sel = Selection::from_anchor_head(Position::new(0, 5), Position::new(0, 10));

    // Position inside selection
    assert!(sel.contains(Position::new(0, 7)));

    // Position at start (inclusive)
    assert!(sel.contains(Position::new(0, 5)));

    // Position at end (exclusive)
    assert!(!sel.contains(Position::new(0, 10)));

    // Position before
    assert!(!sel.contains(Position::new(0, 3)));

    // Position after
    assert!(!sel.contains(Position::new(0, 12)));
}

// ========================================================================
// Rectangle Selection Tests
// ========================================================================

#[test]
fn test_rectangle_selection_right_to_left_cursor_placement() {
    // Test that when dragging right-to-left, cursors are placed at the
    // dragged-to position (left edge), not the start position (right edge)
    let mut model = test_model("hello world\nfoo bar baz\ntest line 3\n", 0, 0);

    // Simulate dragging from column 10 to column 3 (right-to-left) on lines 0-2
    // Start position: (line 0, col 10)
    // End position: (line 2, col 3)
    model.editor_mut().rectangle_selection.active = true;
    model.editor_mut().rectangle_selection.start = Position::new(0, 10);
    model.editor_mut().rectangle_selection.current = Position::new(2, 3);

    // Finish the rectangle selection
    update(&mut model, Msg::Editor(EditorMsg::FinishRectangleSelection));

    // Should have 3 cursors (one per line)
    assert_eq!(model.editor().cursors.len(), 3);

    // Each cursor should be at column 3 (where we dragged TO), not column 10
    for cursor in &model.editor().cursors {
        assert_eq!(
            cursor.column, 3,
            "Cursor on line {} should be at column 3 (dragged-to position), but was at column {}",
            cursor.line, cursor.column
        );
    }
}

#[test]
fn test_rectangle_selection_left_to_right_cursor_placement() {
    // Test that when dragging left-to-right, cursors are placed at the
    // dragged-to position (right edge)
    let mut model = test_model("hello world\nfoo bar baz\ntest line 3\n", 0, 0);

    // Simulate dragging from column 3 to column 10 (left-to-right) on lines 0-2
    model.editor_mut().rectangle_selection.active = true;
    model.editor_mut().rectangle_selection.start = Position::new(0, 3);
    model.editor_mut().rectangle_selection.current = Position::new(2, 10);

    update(&mut model, Msg::Editor(EditorMsg::FinishRectangleSelection));

    assert_eq!(model.editor().cursors.len(), 3);

    // Each cursor should be at column 10 (where we dragged TO)
    for cursor in &model.editor().cursors {
        assert_eq!(
            cursor.column, 10,
            "Cursor on line {} should be at column 10 (dragged-to position), but was at column {}",
            cursor.line, cursor.column
        );
    }
}

// ========================================================================
// AddCursorAbove/Below Tests
// ========================================================================

#[test]
fn test_add_cursor_above() {
    // Start with cursor on line 2
    let mut model = test_model("line 0\nline 1\nline 2\nline 3\n", 2, 3);

    // Add cursor above
    update(&mut model, Msg::Editor(EditorMsg::AddCursorAbove));

    // Should now have 2 cursors: one on line 1, one on line 2
    assert_eq!(model.editor().cursor_count(), 2, "Should have 2 cursors");
    // Cursors should be sorted by position, so line 1 first
    assert_eq!(
        model.editor().cursors[0].line,
        1,
        "First cursor should be on line 1"
    );
    assert_eq!(
        model.editor().cursors[1].line,
        2,
        "Second cursor should be on line 2"
    );
}

#[test]
fn test_add_cursor_below() {
    // Start with cursor on line 1
    let mut model = test_model("line 0\nline 1\nline 2\nline 3\n", 1, 3);

    // Add cursor below
    update(&mut model, Msg::Editor(EditorMsg::AddCursorBelow));

    // Should now have 2 cursors: one on line 1, one on line 2
    assert_eq!(model.editor().cursor_count(), 2, "Should have 2 cursors");
    assert_eq!(
        model.editor().cursors[0].line,
        1,
        "First cursor should be on line 1"
    );
    assert_eq!(
        model.editor().cursors[1].line,
        2,
        "Second cursor should be on line 2"
    );
}

#[test]
fn test_add_cursor_above_at_top() {
    // Start with cursor on line 0 (top of document)
    let mut model = test_model("line 0\nline 1\nline 2\n", 0, 3);

    // Add cursor above - should do nothing (already at top)
    update(&mut model, Msg::Editor(EditorMsg::AddCursorAbove));

    // Should still have just 1 cursor
    assert_eq!(
        model.editor().cursor_count(),
        1,
        "Should still have 1 cursor at top"
    );
}

#[test]
fn test_add_cursor_below_at_bottom() {
    // Start with cursor on last line (line 2, no trailing newline)
    let mut model = test_model("line 0\nline 1\nline 2", 2, 3);

    // Add cursor below - should do nothing (already at bottom)
    update(&mut model, Msg::Editor(EditorMsg::AddCursorBelow));

    // Should still have just 1 cursor
    assert_eq!(
        model.editor().cursor_count(),
        1,
        "Should still have 1 cursor at bottom"
    );
}

#[test]
fn test_deduplicate_cursors() {
    // Create a model and manually add duplicate cursors
    let mut model = test_model("line 0\nline 1\nline 2\n", 1, 3);

    // Add another cursor at the same position
    model.editor_mut().cursors.push(Cursor::at(1, 3));
    model
        .editor_mut()
        .selections
        .push(Selection::new(Position::new(1, 3)));

    // Now we have 2 cursors at the same position
    assert_eq!(model.editor().cursor_count(), 2);

    // Deduplicate
    model.editor_mut().deduplicate_cursors();

    // Should now have just 1 cursor
    assert_eq!(
        model.editor().cursor_count(),
        1,
        "Duplicates should be removed"
    );
}

// Note: Tests for arrow keys with selection (that require clearing selection before movement)
// are in src/main.rs since they need access to the handle_key() function which is
// in the binary, not the library.

// ========================================================================
// Word Selection Tests (Shift+Option+Arrow)
// ========================================================================

#[test]
fn test_word_selection_right_from_start() {
    // "hello world" - cursor at start, select word right
    let mut model = test_model("hello world\n", 0, 0);

    update(
        &mut model,
        Msg::Editor(EditorMsg::MoveCursorWordWithSelection(Direction::Right)),
    );

    // Should select "hello" (cursor moves to end of word)
    let selection = model.editor().selection();
    assert!(!selection.is_empty(), "Selection should not be empty");
    assert_eq!(selection.anchor, Position::new(0, 0), "Anchor at start");
    assert_eq!(
        selection.head,
        Position::new(0, 5),
        "Head at end of 'hello'"
    );
}

#[test]
fn test_word_selection_right_multiple() {
    // "hello world" - cursor at start, select through to second word
    // Word navigation: hello(0-5) -> space(5-6) -> world(6-11)
    let mut model = test_model("hello world\n", 0, 0);

    // Move 1: Select "hello" (0->5)
    update(
        &mut model,
        Msg::Editor(EditorMsg::MoveCursorWordWithSelection(Direction::Right)),
    );
    assert_eq!(model.editor().selection().head.column, 5, "After 'hello'");

    // Move 2: Skip space (5->6)
    update(
        &mut model,
        Msg::Editor(EditorMsg::MoveCursorWordWithSelection(Direction::Right)),
    );
    assert_eq!(model.editor().selection().head.column, 6, "After space");

    // Move 3: Select "world" (6->11)
    update(
        &mut model,
        Msg::Editor(EditorMsg::MoveCursorWordWithSelection(Direction::Right)),
    );

    let selection = model.editor().selection();
    assert_eq!(
        selection.anchor,
        Position::new(0, 0),
        "Anchor stays at start"
    );
    assert_eq!(selection.head.column, 11, "Head at end of 'world'");
}

#[test]
fn test_word_selection_left_from_end() {
    // "hello world" - cursor at end, select word left
    let mut model = test_model("hello world\n", 0, 11);

    update(
        &mut model,
        Msg::Editor(EditorMsg::MoveCursorWordWithSelection(Direction::Left)),
    );

    let selection = model.editor().selection();
    assert!(!selection.is_empty(), "Selection should not be empty");
    assert_eq!(selection.anchor, Position::new(0, 11), "Anchor at end");
    assert_eq!(
        selection.head,
        Position::new(0, 6),
        "Head at start of 'world'"
    );
}

#[test]
fn test_word_selection_left_multiple() {
    // "hello world" - cursor at end, select backwards
    // Word navigation: world(11->6) -> space(6->5) -> hello(5->0)
    let mut model = test_model("hello world\n", 0, 11);

    // Move 1: Select "world" backwards (11->6)
    update(
        &mut model,
        Msg::Editor(EditorMsg::MoveCursorWordWithSelection(Direction::Left)),
    );
    assert_eq!(
        model.editor().selection().head.column,
        6,
        "At start of 'world'"
    );

    // Move 2: Skip space backwards (6->5)
    update(
        &mut model,
        Msg::Editor(EditorMsg::MoveCursorWordWithSelection(Direction::Left)),
    );
    assert_eq!(model.editor().selection().head.column, 5, "After space");

    // Move 3: Select "hello" backwards (5->0)
    update(
        &mut model,
        Msg::Editor(EditorMsg::MoveCursorWordWithSelection(Direction::Left)),
    );

    let selection = model.editor().selection();
    assert_eq!(
        selection.anchor,
        Position::new(0, 11),
        "Anchor stays at end"
    );
    assert_eq!(selection.head, Position::new(0, 0), "Head at start of line");
}

#[test]
fn test_word_selection_extends_existing_selection() {
    // Start with an existing selection, then extend by word
    // "hello world test" - word boundaries: hello(0-5), space(5-6), world(6-11), space(11-12), test(12-16)
    let mut model = test_model("hello world test\n", 0, 0);

    // First select "hello" by word (0->5)
    update(
        &mut model,
        Msg::Editor(EditorMsg::MoveCursorWordWithSelection(Direction::Right)),
    );
    assert_eq!(model.editor().selection().head.column, 5);

    // Skip space (5->6)
    update(
        &mut model,
        Msg::Editor(EditorMsg::MoveCursorWordWithSelection(Direction::Right)),
    );
    assert_eq!(model.editor().selection().head.column, 6);

    // Extend to end of "world" (6->11)
    update(
        &mut model,
        Msg::Editor(EditorMsg::MoveCursorWordWithSelection(Direction::Right)),
    );

    let selection = model.editor().selection();
    assert_eq!(selection.anchor, Position::new(0, 0), "Anchor unchanged");
    assert_eq!(selection.head.column, 11, "Head extended to end of 'world'");
}
