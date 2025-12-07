//! Selection tests
//!
//! Multi-cursor tests are in multi_cursor.rs

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
    let selection = model.editor().primary_selection();
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
    assert_eq!(
        model.editor().primary_selection().head.column,
        5,
        "After 'hello'"
    );

    // Move 2: Skip space (5->6)
    update(
        &mut model,
        Msg::Editor(EditorMsg::MoveCursorWordWithSelection(Direction::Right)),
    );
    assert_eq!(
        model.editor().primary_selection().head.column,
        6,
        "After space"
    );

    // Move 3: Select "world" (6->11)
    update(
        &mut model,
        Msg::Editor(EditorMsg::MoveCursorWordWithSelection(Direction::Right)),
    );

    let selection = model.editor().primary_selection();
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

    let selection = model.editor().primary_selection();
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
        model.editor().primary_selection().head.column,
        6,
        "At start of 'world'"
    );

    // Move 2: Skip space backwards (6->5)
    update(
        &mut model,
        Msg::Editor(EditorMsg::MoveCursorWordWithSelection(Direction::Left)),
    );
    assert_eq!(
        model.editor().primary_selection().head.column,
        5,
        "After space"
    );

    // Move 3: Select "hello" backwards (5->0)
    update(
        &mut model,
        Msg::Editor(EditorMsg::MoveCursorWordWithSelection(Direction::Left)),
    );

    let selection = model.editor().primary_selection();
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
    assert_eq!(model.editor().primary_selection().head.column, 5);

    // Skip space (5->6)
    update(
        &mut model,
        Msg::Editor(EditorMsg::MoveCursorWordWithSelection(Direction::Right)),
    );
    assert_eq!(model.editor().primary_selection().head.column, 6);

    // Extend to end of "world" (6->11)
    update(
        &mut model,
        Msg::Editor(EditorMsg::MoveCursorWordWithSelection(Direction::Right)),
    );

    let selection = model.editor().primary_selection();
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
    let model = test_model("café latte", 0, 2);
    let result = model.editor().word_under_cursor(model.document());
    assert_eq!(
        result,
        Some(("café".to_string(), Position::new(0, 0), Position::new(0, 4)))
    );
}

#[test]
fn test_word_under_cursor_unicode_end_of_line() {
    // Cursor at end of line with multi-byte char at end
    let model = test_model("café", 0, 10);
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

    // First call: should ONLY select word under cursor (not find next)
    update(&mut model, Msg::Editor(EditorMsg::SelectNextOccurrence));
    assert!(
        !model.editor().primary_selection().is_empty(),
        "Word should be selected"
    );
    assert_eq!(
        model.editor().cursors.len(),
        1,
        "First call only selects word, no additional cursors"
    );

    // Verify the selection is correct
    let sel = model.editor().primary_selection();
    assert_eq!(sel.anchor, Position::new(0, 0));
    assert_eq!(sel.head, Position::new(0, 3));

    // Second call: should find and add cursor at next occurrence
    update(&mut model, Msg::Editor(EditorMsg::SelectNextOccurrence));
    assert_eq!(
        model.editor().cursors.len(),
        2,
        "Second call adds cursor at next occurrence"
    );

    // Third call: should find the third "abc"
    update(&mut model, Msg::Editor(EditorMsg::SelectNextOccurrence));
    assert_eq!(model.editor().cursors.len(), 3, "Should have 3 cursors now");

    // Fourth call: should wrap around, all already selected
    update(&mut model, Msg::Editor(EditorMsg::SelectNextOccurrence));
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

    // First call: only selects "café" under cursor
    update(&mut model, Msg::Editor(EditorMsg::SelectNextOccurrence));
    assert_eq!(
        model.editor().cursors.len(),
        1,
        "First call only selects word"
    );

    // Second call: adds cursor at next occurrence
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

#[test]
fn test_select_next_occurrence_cursor_mid_word() {
    // Cursor in middle of word: "he|llo hello hello"
    let mut model = test_model("hello hello hello", 0, 2);

    // First call: selects "hello" (the word cursor is on)
    update(&mut model, Msg::Editor(EditorMsg::SelectNextOccurrence));
    assert_eq!(model.editor().cursors.len(), 1);
    let sel = model.editor().primary_selection();
    assert_eq!(sel.anchor, Position::new(0, 0));
    assert_eq!(sel.head, Position::new(0, 5));

    // Second call: adds second "hello"
    update(&mut model, Msg::Editor(EditorMsg::SelectNextOccurrence));
    assert_eq!(model.editor().cursors.len(), 2);

    // Third call: adds third "hello"
    update(&mut model, Msg::Editor(EditorMsg::SelectNextOccurrence));
    assert_eq!(model.editor().cursors.len(), 3);
}

#[test]
fn test_select_next_occurrence_with_existing_selection() {
    // If selection already exists, should immediately find next
    let mut model = test_model("foo bar foo baz foo", 0, 0);

    // Manually create a selection of "foo"
    model.editor_mut().primary_selection_mut().anchor = Position::new(0, 0);
    model.editor_mut().primary_selection_mut().head = Position::new(0, 3);

    // First call with existing selection: should find next occurrence
    update(&mut model, Msg::Editor(EditorMsg::SelectNextOccurrence));
    assert_eq!(
        model.editor().cursors.len(),
        2,
        "Should add cursor at next occurrence"
    );
}

#[test]
fn test_select_next_occurrence_single_occurrence() {
    // Only one occurrence of the word
    let mut model = test_model("unique word here", 0, 0);

    // First call: selects "unique"
    update(&mut model, Msg::Editor(EditorMsg::SelectNextOccurrence));
    assert_eq!(model.editor().cursors.len(), 1);
    assert!(!model.editor().primary_selection().is_empty());

    // Second call: no more occurrences, should stay at 1 cursor
    update(&mut model, Msg::Editor(EditorMsg::SelectNextOccurrence));
    assert_eq!(model.editor().cursors.len(), 1);
}

#[test]
fn test_select_next_occurrence_cursor_at_word_end() {
    // Cursor at last char of word: "hell|o hello" (column 4)
    let mut model = test_model("hello hello", 0, 4);

    // First call: should select "hello" (word cursor is on)
    update(&mut model, Msg::Editor(EditorMsg::SelectNextOccurrence));
    assert_eq!(model.editor().cursors.len(), 1);
    let sel = model.editor().primary_selection();
    assert_eq!(sel.anchor, Position::new(0, 0));
    assert_eq!(sel.head, Position::new(0, 5));

    // Second call: adds next occurrence
    update(&mut model, Msg::Editor(EditorMsg::SelectNextOccurrence));
    assert_eq!(model.editor().cursors.len(), 2);
}

#[test]
fn test_select_next_occurrence_on_whitespace() {
    // Cursor on whitespace between words should do nothing
    let mut model = test_model("foo bar", 0, 3); // cursor on space

    update(&mut model, Msg::Editor(EditorMsg::SelectNextOccurrence));
    // Should still have empty selection (no word under cursor)
    assert!(model.editor().primary_selection().is_empty());
    assert_eq!(model.editor().cursors.len(), 1);
}

// ========================================================================
// Cursor/Selection Invariant Tests
// ========================================================================

#[test]
fn test_move_cursor_clears_selection() {
    // Start with a selection
    let mut model = test_model_with_selection("hello world", 0, 0, 0, 5);
    assert!(
        !model.editor().primary_selection().is_empty(),
        "Should have selection"
    );

    // Move cursor right (non-shift) - should clear selection
    update(
        &mut model,
        Msg::Editor(EditorMsg::MoveCursor(Direction::Right)),
    );

    assert!(
        model.editor().primary_selection().is_empty(),
        "Selection should be cleared after non-shift move"
    );
    // Verify invariant: cursor position == selection head
    assert_eq!(
        model.editor().primary_cursor().to_position(),
        model.editor().primary_selection().head,
        "Cursor position should equal selection head"
    );
}

#[test]
fn test_set_cursor_position_clears_selection() {
    let mut model = test_model_with_selection("hello world", 0, 0, 0, 5);
    assert!(!model.editor().primary_selection().is_empty());

    update(
        &mut model,
        Msg::Editor(EditorMsg::SetCursorPosition { line: 0, column: 8 }),
    );

    assert!(
        model.editor().primary_selection().is_empty(),
        "Selection should be cleared"
    );
    assert_eq!(model.editor().primary_cursor().column, 8);
}

#[test]
fn test_page_down_clears_selection() {
    // Create multi-line doc
    let mut model = test_model_with_selection("line1\nline2\nline3\nline4\nline5", 0, 0, 0, 3);
    assert!(!model.editor().primary_selection().is_empty());

    update(&mut model, Msg::Editor(EditorMsg::PageDown));

    assert!(
        model.editor().primary_selection().is_empty(),
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

// ========================================================================
// merge_overlapping_selections() Tests
// ========================================================================

#[test]
fn test_merge_non_overlapping_unchanged() {
    let mut model = test_model("hello world test", 0, 0);

    // Manually set up two non-overlapping selections: [0,0..0,3] and [0,6..0,9]
    model.editor_mut().cursors.clear();
    model.editor_mut().selections.clear();
    model.editor_mut().cursors.push(Cursor::at(0, 3));
    model.editor_mut().cursors.push(Cursor::at(0, 9));
    model
        .editor_mut()
        .selections
        .push(Selection::from_positions(
            Position::new(0, 0),
            Position::new(0, 3),
        ));
    model
        .editor_mut()
        .selections
        .push(Selection::from_positions(
            Position::new(0, 6),
            Position::new(0, 9),
        ));

    model.editor_mut().merge_overlapping_selections();

    assert_eq!(
        model.editor().cursors.len(),
        2,
        "Should still have 2 cursors"
    );
    assert_eq!(
        model.editor().selections.len(),
        2,
        "Should still have 2 selections"
    );
}

#[test]
fn test_merge_overlapping_same_line() {
    let mut model = test_model("hello world test", 0, 0);

    // Set up two overlapping selections: [0,0..0,5] and [0,3..0,8]
    model.editor_mut().cursors.clear();
    model.editor_mut().selections.clear();
    model.editor_mut().cursors.push(Cursor::at(0, 5));
    model.editor_mut().cursors.push(Cursor::at(0, 8));
    model
        .editor_mut()
        .selections
        .push(Selection::from_positions(
            Position::new(0, 0),
            Position::new(0, 5),
        ));
    model
        .editor_mut()
        .selections
        .push(Selection::from_positions(
            Position::new(0, 3),
            Position::new(0, 8),
        ));

    model.editor_mut().merge_overlapping_selections();

    assert_eq!(model.editor().cursors.len(), 1, "Should merge to 1 cursor");
    assert_eq!(
        model.editor().selections.len(),
        1,
        "Should merge to 1 selection"
    );

    let sel = &model.editor().selections[0];
    assert_eq!(
        sel.start(),
        Position::new(0, 0),
        "Merged start should be 0,0"
    );
    assert_eq!(sel.end(), Position::new(0, 8), "Merged end should be 0,8");
}

#[test]
fn test_merge_touching_adjacent() {
    let mut model = test_model("hello world", 0, 0);

    // Set up two touching (adjacent) selections: [0,0..0,5] and [0,5..0,10]
    model.editor_mut().cursors.clear();
    model.editor_mut().selections.clear();
    model.editor_mut().cursors.push(Cursor::at(0, 5));
    model.editor_mut().cursors.push(Cursor::at(0, 10));
    model
        .editor_mut()
        .selections
        .push(Selection::from_positions(
            Position::new(0, 0),
            Position::new(0, 5),
        ));
    model
        .editor_mut()
        .selections
        .push(Selection::from_positions(
            Position::new(0, 5),
            Position::new(0, 10),
        ));

    model.editor_mut().merge_overlapping_selections();

    assert_eq!(
        model.editor().cursors.len(),
        1,
        "Should merge touching to 1 cursor"
    );

    let sel = &model.editor().selections[0];
    assert_eq!(sel.start(), Position::new(0, 0));
    assert_eq!(sel.end(), Position::new(0, 10));
}

#[test]
fn test_merge_multiline_overlap() {
    let mut model = test_model("line one\nline two\nline three", 0, 0);

    // Set up two multiline overlapping selections:
    // [0,5..1,4] and [0,7..2,3]
    model.editor_mut().cursors.clear();
    model.editor_mut().selections.clear();
    model.editor_mut().cursors.push(Cursor::at(1, 4));
    model.editor_mut().cursors.push(Cursor::at(2, 3));
    model
        .editor_mut()
        .selections
        .push(Selection::from_positions(
            Position::new(0, 5),
            Position::new(1, 4),
        ));
    model
        .editor_mut()
        .selections
        .push(Selection::from_positions(
            Position::new(0, 7),
            Position::new(2, 3),
        ));

    model.editor_mut().merge_overlapping_selections();

    assert_eq!(model.editor().cursors.len(), 1, "Should merge to 1 cursor");

    let sel = &model.editor().selections[0];
    assert_eq!(sel.start(), Position::new(0, 5), "Merged start");
    assert_eq!(sel.end(), Position::new(2, 3), "Merged end");
}

#[test]
fn test_merge_duplicates() {
    let mut model = test_model("hello world", 0, 0);

    // Set up two identical selections
    model.editor_mut().cursors.clear();
    model.editor_mut().selections.clear();
    model.editor_mut().cursors.push(Cursor::at(0, 5));
    model.editor_mut().cursors.push(Cursor::at(0, 5));
    model
        .editor_mut()
        .selections
        .push(Selection::from_positions(
            Position::new(0, 0),
            Position::new(0, 5),
        ));
    model
        .editor_mut()
        .selections
        .push(Selection::from_positions(
            Position::new(0, 0),
            Position::new(0, 5),
        ));

    model.editor_mut().merge_overlapping_selections();

    assert_eq!(
        model.editor().cursors.len(),
        1,
        "Duplicates should merge to 1"
    );
    assert_eq!(model.editor().selections.len(), 1);
}

#[test]
fn test_merge_preserves_invariants() {
    let mut model = test_model("hello world test", 0, 0);

    // Set up overlapping selections
    model.editor_mut().cursors.clear();
    model.editor_mut().selections.clear();
    model.editor_mut().cursors.push(Cursor::at(0, 5));
    model.editor_mut().cursors.push(Cursor::at(0, 8));
    model
        .editor_mut()
        .selections
        .push(Selection::from_positions(
            Position::new(0, 0),
            Position::new(0, 5),
        ));
    model
        .editor_mut()
        .selections
        .push(Selection::from_positions(
            Position::new(0, 3),
            Position::new(0, 8),
        ));

    model.editor_mut().merge_overlapping_selections();

    // Check invariants
    assert_eq!(
        model.editor().cursors.len(),
        model.editor().selections.len(),
        "cursors.len() == selections.len()"
    );

    for (i, (cursor, sel)) in model
        .editor()
        .cursors
        .iter()
        .zip(model.editor().selections.iter())
        .enumerate()
    {
        assert_eq!(
            cursor.to_position(),
            sel.head,
            "Cursor {} position should equal selection head",
            i
        );
    }
}

// ========================================================================
// SelectAll Tests
// ========================================================================

#[test]
fn test_select_all_single_cursor() {
    let mut model = test_model("hello\nworld\ntest", 0, 3);

    update(&mut model, Msg::Editor(EditorMsg::SelectAll));

    assert_eq!(model.editor().cursors.len(), 1, "Should have 1 cursor");
    assert_eq!(
        model.editor().selections.len(),
        1,
        "Should have 1 selection"
    );

    let sel = &model.editor().selections[0];
    assert_eq!(
        sel.start(),
        Position::new(0, 0),
        "Selection should start at 0,0"
    );
    // Last line is "test" (line 2), length 4
    assert_eq!(
        sel.end(),
        Position::new(2, 4),
        "Selection should end at document end"
    );
}

#[test]
fn test_select_all_collapses_multi_cursor() {
    let mut model = test_model("hello\nworld\ntest", 0, 0);

    // Add multiple cursors
    model.editor_mut().cursors.push(Cursor::at(1, 2));
    model.editor_mut().cursors.push(Cursor::at(2, 3));
    model
        .editor_mut()
        .selections
        .push(Selection::new(Position::new(1, 2)));
    model
        .editor_mut()
        .selections
        .push(Selection::new(Position::new(2, 3)));

    assert_eq!(
        model.editor().cursors.len(),
        3,
        "Should have 3 cursors before SelectAll"
    );

    update(&mut model, Msg::Editor(EditorMsg::SelectAll));

    assert_eq!(
        model.editor().cursors.len(),
        1,
        "Should collapse to 1 cursor"
    );
    assert_eq!(
        model.editor().selections.len(),
        1,
        "Should have 1 selection"
    );

    let sel = &model.editor().selections[0];
    assert_eq!(sel.start(), Position::new(0, 0));
    assert_eq!(sel.end(), Position::new(2, 4));
}

// ========================================================================
// SelectWord Tests
// ========================================================================

#[test]
fn test_select_word_single_cursor() {
    let mut model = test_model("hello world", 0, 2);

    update(&mut model, Msg::Editor(EditorMsg::SelectWord));

    let sel = &model.editor().selections[0];
    assert_eq!(
        sel.start(),
        Position::new(0, 0),
        "Should select from start of 'hello'"
    );
    assert_eq!(
        sel.end(),
        Position::new(0, 5),
        "Should select to end of 'hello'"
    );
}

#[test]
fn test_select_word_on_whitespace() {
    let mut model = test_model("hello world", 0, 5);

    // Clear existing selection and set cursor on whitespace
    model.editor_mut().cursors[0].column = 5;
    model.editor_mut().selections[0] = Selection::new(Position::new(0, 5));

    update(&mut model, Msg::Editor(EditorMsg::SelectWord));

    // Cursor on space - no word selection should happen
    // Selection should remain empty or unchanged
    let sel = &model.editor().selections[0];
    // The word_under_cursor_at returns None for whitespace, so selection stays as-is
    assert!(
        sel.is_empty() || sel.start() == sel.end(),
        "Should not select whitespace"
    );
}

#[test]
fn test_select_word_multi_cursor_different_words() {
    let mut model = test_model("hello world test", 0, 2);

    // Add second cursor on "world"
    model.editor_mut().cursors.push(Cursor::at(0, 8));
    model
        .editor_mut()
        .selections
        .push(Selection::new(Position::new(0, 8)));

    update(&mut model, Msg::Editor(EditorMsg::SelectWord));

    assert_eq!(
        model.editor().cursors.len(),
        2,
        "Should still have 2 cursors"
    );
    assert_eq!(
        model.editor().selections.len(),
        2,
        "Should have 2 selections"
    );

    // First selection: "hello" (0-5)
    assert_eq!(model.editor().selections[0].start(), Position::new(0, 0));
    assert_eq!(model.editor().selections[0].end(), Position::new(0, 5));

    // Second selection: "world" (6-11)
    assert_eq!(model.editor().selections[1].start(), Position::new(0, 6));
    assert_eq!(model.editor().selections[1].end(), Position::new(0, 11));
}

#[test]
fn test_select_word_multi_cursor_same_word_merges() {
    let mut model = test_model("hello world", 0, 1);

    // Add second cursor also in "hello"
    model.editor_mut().cursors.push(Cursor::at(0, 3));
    model
        .editor_mut()
        .selections
        .push(Selection::new(Position::new(0, 3)));

    update(&mut model, Msg::Editor(EditorMsg::SelectWord));

    // Both cursors selected "hello", should merge to 1
    assert_eq!(model.editor().cursors.len(), 1, "Should merge to 1 cursor");
    assert_eq!(
        model.editor().selections.len(),
        1,
        "Should have 1 selection"
    );

    let sel = &model.editor().selections[0];
    assert_eq!(sel.start(), Position::new(0, 0));
    assert_eq!(sel.end(), Position::new(0, 5));
}

// ========================================================================
// SelectLine Tests
// ========================================================================

#[test]
fn test_select_line_single_cursor() {
    let mut model = test_model("hello\nworld\ntest", 1, 2);

    update(&mut model, Msg::Editor(EditorMsg::SelectLine));

    let sel = &model.editor().selections[0];
    assert_eq!(
        sel.start(),
        Position::new(1, 0),
        "Should start at line 1, column 0"
    );
    // Line 1 "world" + newline, so selection ends at line 2, column 0
    assert_eq!(
        sel.end(),
        Position::new(2, 0),
        "Should end at start of next line"
    );
}

#[test]
fn test_select_line_last_line() {
    let mut model = test_model("hello\nworld\ntest", 2, 2);

    update(&mut model, Msg::Editor(EditorMsg::SelectLine));

    let sel = &model.editor().selections[0];
    assert_eq!(sel.start(), Position::new(2, 0));
    // Last line has no trailing newline, so end is at end of line
    assert_eq!(sel.end(), Position::new(2, 4), "Should end at line length");
}

#[test]
fn test_select_line_multi_cursor_different_lines() {
    let mut model = test_model("hello\nworld\ntest", 0, 2);

    // Add cursor on line 2
    model.editor_mut().cursors.push(Cursor::at(2, 1));
    model
        .editor_mut()
        .selections
        .push(Selection::new(Position::new(2, 1)));

    update(&mut model, Msg::Editor(EditorMsg::SelectLine));

    assert_eq!(
        model.editor().cursors.len(),
        2,
        "Should still have 2 cursors"
    );

    // First selection: line 0
    assert_eq!(model.editor().selections[0].start(), Position::new(0, 0));
    assert_eq!(model.editor().selections[0].end(), Position::new(1, 0));

    // Second selection: line 2 (last line)
    assert_eq!(model.editor().selections[1].start(), Position::new(2, 0));
    assert_eq!(model.editor().selections[1].end(), Position::new(2, 4));
}

#[test]
fn test_select_line_multi_cursor_same_line_merges() {
    let mut model = test_model("hello\nworld\ntest", 0, 1);

    // Add second cursor also on line 0
    model.editor_mut().cursors.push(Cursor::at(0, 4));
    model
        .editor_mut()
        .selections
        .push(Selection::new(Position::new(0, 4)));

    update(&mut model, Msg::Editor(EditorMsg::SelectLine));

    // Both cursors on same line, should merge to 1
    assert_eq!(model.editor().cursors.len(), 1, "Should merge to 1 cursor");
    assert_eq!(model.editor().selections.len(), 1);

    let sel = &model.editor().selections[0];
    assert_eq!(sel.start(), Position::new(0, 0));
    assert_eq!(sel.end(), Position::new(1, 0));
}

// ========================================================================
// ExtendSelectionToPosition Tests
// ========================================================================

#[test]
fn test_extend_selection_single_cursor() {
    let mut model = test_model("hello world", 0, 0);

    update(
        &mut model,
        Msg::Editor(EditorMsg::ExtendSelectionToPosition { line: 0, column: 5 }),
    );

    let sel = &model.editor().selections[0];
    assert_eq!(
        sel.anchor,
        Position::new(0, 0),
        "Anchor should be at original position"
    );
    assert_eq!(
        sel.head,
        Position::new(0, 5),
        "Head should be at target position"
    );
    assert_eq!(
        model.editor().primary_cursor().column,
        5,
        "Cursor should be at target"
    );
}

#[test]
fn test_extend_selection_collapses_multi_cursor() {
    let mut model = test_model("hello\nworld\ntest", 0, 0);

    // Add multiple cursors
    model.editor_mut().cursors.push(Cursor::at(1, 2));
    model.editor_mut().cursors.push(Cursor::at(2, 3));
    model
        .editor_mut()
        .selections
        .push(Selection::new(Position::new(1, 2)));
    model
        .editor_mut()
        .selections
        .push(Selection::new(Position::new(2, 3)));

    assert_eq!(
        model.editor().cursors.len(),
        3,
        "Should have 3 cursors before"
    );

    update(
        &mut model,
        Msg::Editor(EditorMsg::ExtendSelectionToPosition { line: 1, column: 4 }),
    );

    assert_eq!(
        model.editor().cursors.len(),
        1,
        "Should collapse to 1 cursor"
    );
    assert_eq!(
        model.editor().selections.len(),
        1,
        "Should have 1 selection"
    );

    let sel = &model.editor().selections[0];
    assert_eq!(
        sel.anchor,
        Position::new(0, 0),
        "Anchor from primary cursor"
    );
    assert_eq!(sel.head, Position::new(1, 4), "Head at target position");
}
