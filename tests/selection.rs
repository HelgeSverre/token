//! Selection and multi-cursor tests

mod common;

use common::{test_model, test_model_with_selection};
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

// ========================================================================
// word_under_cursor Unicode tests
// ========================================================================

#[test]
fn test_word_under_cursor_ascii() {
    let model = test_model("hello world", 0, 0);
    let result = model.editor().word_under_cursor(model.document());
    assert_eq!(
        result,
        Some((
            "hello".to_string(),
            Position::new(0, 0),
            Position::new(0, 5)
        ))
    );
}

#[test]
fn test_word_under_cursor_unicode() {
    // "café" has 4 chars but 5 bytes (é is 2 bytes)
    let mut model = test_model("café latte", 0, 2);
    let result = model.editor().word_under_cursor(model.document());
    assert_eq!(
        result,
        Some(("café".to_string(), Position::new(0, 0), Position::new(0, 4)))
    );
}

#[test]
fn test_word_under_cursor_unicode_end_of_line() {
    // Cursor at end of line with multi-byte char at end
    let mut model = test_model("café", 0, 10);
    // Put cursor past end - should clamp to last char
    let result = model.editor().word_under_cursor(model.document());
    // Should still find "café" since cursor clamps to valid position
    assert_eq!(
        result,
        Some(("café".to_string(), Position::new(0, 0), Position::new(0, 4)))
    );
}

#[test]
fn test_word_under_cursor_emoji() {
    // Emoji are single chars but 4 bytes in UTF-8
    // Emoji are treated as word characters (not whitespace, not punctuation)
    // so "hello🎉world" is one word
    let model = test_model("hello🎉world", 0, 6);
    // Cursor on 'w' at position 6 (h=0, e=1, l=2, l=3, o=4, 🎉=5, w=6)
    let result = model.editor().word_under_cursor(model.document());
    // The whole thing is treated as one word since emoji is a word char
    assert_eq!(
        result,
        Some((
            "hello🎉world".to_string(),
            Position::new(0, 0),
            Position::new(0, 11)
        ))
    );
}

#[test]
fn test_word_under_cursor_emoji_separated() {
    // Test with space separation
    let model = test_model("hello 🎉 world", 0, 9);
    // Cursor on 'w' at position 9 (h=0..5, space=5, 🎉=6, space=7, w=8)
    // Wait, let's recalculate: "hello " = 6 chars, "🎉" = 1 char, " " = 1 char, "world" starts at 8
    let result = model.editor().word_under_cursor(model.document());
    assert_eq!(
        result,
        Some((
            "world".to_string(),
            Position::new(0, 8),
            Position::new(0, 13)
        ))
    );
}

#[test]
fn test_word_under_cursor_on_whitespace() {
    let model = test_model("hello world", 0, 5); // On space
    let result = model.editor().word_under_cursor(model.document());
    assert_eq!(result, None);
}

// ========================================================================
// SelectNextOccurrence tests
// ========================================================================

#[test]
fn test_select_next_occurrence_finds_all() {
    // Text with 3 occurrences of "abc"
    let mut model = test_model("abc def abc ghi abc", 0, 0);

    // First call: selects word under cursor AND finds next occurrence
    update(&mut model, Msg::Editor(EditorMsg::SelectNextOccurrence));
    assert!(
        !model.editor().selection().is_empty(),
        "Word should be selected"
    );
    assert_eq!(
        model.editor().cursors.len(),
        2,
        "First call selects word and adds cursor at next occurrence"
    );

    // Second call: should find the third "abc" at position 16
    update(&mut model, Msg::Editor(EditorMsg::SelectNextOccurrence));
    assert_eq!(model.editor().cursors.len(), 3, "Should have 3 cursors now");

    // Third call: should wrap around and find the first one is already selected
    update(&mut model, Msg::Editor(EditorMsg::SelectNextOccurrence));
    // Should still be 3 cursors (no new ones added since all are selected)
    assert_eq!(
        model.editor().cursors.len(),
        3,
        "Should still have 3 cursors"
    );
}

#[test]
fn test_select_next_occurrence_unicode() {
    // Text with 2 occurrences of "café"
    let mut model = test_model("café latte café mocha", 0, 0);

    // First call: selects "café" under cursor AND adds cursor at next occurrence
    update(&mut model, Msg::Editor(EditorMsg::SelectNextOccurrence));
    assert_eq!(model.editor().cursors.len(), 2, "Should find both cafés");

    // Third call: should wrap and see all are selected
    update(&mut model, Msg::Editor(EditorMsg::SelectNextOccurrence));
    assert_eq!(
        model.editor().cursors.len(),
        2,
        "Still 2 cursors - all occurrences selected"
    );
}

// ========================================================================
// Cursor/Selection Invariant Tests
// ========================================================================

#[test]
fn test_move_cursor_clears_selection() {
    // Start with a selection
    let mut model = test_model_with_selection("hello world", 0, 0, 0, 5);
    assert!(
        !model.editor().selection().is_empty(),
        "Should have selection"
    );

    // Move cursor right (non-shift) - should clear selection
    update(
        &mut model,
        Msg::Editor(EditorMsg::MoveCursor(Direction::Right)),
    );

    assert!(
        model.editor().selection().is_empty(),
        "Selection should be cleared after non-shift move"
    );
    // Verify invariant: cursor position == selection head
    assert_eq!(
        model.editor().cursor().to_position(),
        model.editor().selection().head,
        "Cursor position should equal selection head"
    );
}

#[test]
fn test_set_cursor_position_clears_selection() {
    let mut model = test_model_with_selection("hello world", 0, 0, 0, 5);
    assert!(!model.editor().selection().is_empty());

    update(
        &mut model,
        Msg::Editor(EditorMsg::SetCursorPosition { line: 0, column: 8 }),
    );

    assert!(
        model.editor().selection().is_empty(),
        "Selection should be cleared"
    );
    assert_eq!(model.editor().cursor().column, 8);
}

#[test]
fn test_page_down_clears_selection() {
    // Create multi-line doc
    let mut model = test_model_with_selection("line1\nline2\nline3\nline4\nline5", 0, 0, 0, 3);
    assert!(!model.editor().selection().is_empty());

    update(&mut model, Msg::Editor(EditorMsg::PageDown));

    assert!(
        model.editor().selection().is_empty(),
        "Selection should be cleared after PageDown"
    );
}

#[test]
fn test_select_all_occurrences() {
    // Text with 3 occurrences of "abc"
    let mut model = test_model("abc def abc ghi abc", 0, 0);

    // Select all occurrences
    update(&mut model, Msg::Editor(EditorMsg::SelectAllOccurrences));

    // Should have 3 cursors, one for each occurrence
    assert_eq!(
        model.editor().cursors.len(),
        3,
        "Should have 3 cursors for 3 occurrences"
    );

    // All selections should be non-empty and selecting "abc"
    for selection in &model.editor().selections {
        assert!(!selection.is_empty(), "Each selection should be non-empty");
    }
}

#[test]
fn test_select_all_occurrences_unicode() {
    let mut model = test_model("café latte café mocha café", 0, 0);

    update(&mut model, Msg::Editor(EditorMsg::SelectAllOccurrences));

    assert_eq!(
        model.editor().cursors.len(),
        3,
        "Should find all 3 café occurrences"
    );
}
