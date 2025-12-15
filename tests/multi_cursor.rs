//! Multi-cursor behavior tests
//!
//! Tests for multi-cursor operations including:
//! - Adding/removing cursors
//! - Active cursor tracking
//! - Collapsing to single cursor
//! - Cursor deduplication

mod common;

use common::test_model;
use token::messages::{EditorMsg, Msg};
use token::model::{Cursor, Position, Selection};
use token::update::update;

// ========================================================================
// AddCursorAbove/Below Tests
// ========================================================================

#[test]
fn test_add_cursor_above() {
    let mut model = test_model("line 0\nline 1\nline 2\nline 3\n", 2, 3);

    update(&mut model, Msg::Editor(EditorMsg::AddCursorAbove));

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
fn test_add_cursor_below() {
    let mut model = test_model("line 0\nline 1\nline 2\nline 3\n", 1, 3);

    update(&mut model, Msg::Editor(EditorMsg::AddCursorBelow));

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
    let mut model = test_model("line 0\nline 1\nline 2\n", 0, 3);

    update(&mut model, Msg::Editor(EditorMsg::AddCursorAbove));

    assert_eq!(
        model.editor().cursor_count(),
        1,
        "Should still have 1 cursor at top"
    );
}

#[test]
fn test_add_cursor_below_at_bottom() {
    let mut model = test_model("line 0\nline 1\nline 2", 2, 3);

    update(&mut model, Msg::Editor(EditorMsg::AddCursorBelow));

    assert_eq!(
        model.editor().cursor_count(),
        1,
        "Should still have 1 cursor at bottom"
    );
}

// ========================================================================
// Cursor Deduplication Tests
// ========================================================================

#[test]
fn test_deduplicate_cursors() {
    let mut model = test_model("line 0\nline 1\nline 2\n", 1, 3);

    model.editor_mut().cursors.push(Cursor::at(1, 3));
    model
        .editor_mut()
        .selections
        .push(Selection::new(Position::new(1, 3)));

    assert_eq!(model.editor().cursor_count(), 2);

    model.editor_mut().deduplicate_cursors();

    assert_eq!(
        model.editor().cursor_count(),
        1,
        "Duplicates should be removed"
    );
}

// ========================================================================
// Active Cursor Tracking Tests
// ========================================================================

#[test]
fn test_add_cursor_below_sets_active() {
    let mut model = test_model("line 0\nline 1\nline 2\nline 3\n", 1, 0);

    update(&mut model, Msg::Editor(EditorMsg::AddCursorBelow));

    assert_eq!(model.editor().cursor_count(), 2, "Should have 2 cursors");

    let active = model.editor().active_cursor();
    assert_eq!(
        active.line, 2,
        "Active cursor should be on the new line (line 2)"
    );
    assert_eq!(model.editor().active_cursor_index, 1);
}

#[test]
fn test_add_cursor_above_sets_active() {
    let mut model = test_model("line 0\nline 1\nline 2\nline 3\n", 2, 0);

    update(&mut model, Msg::Editor(EditorMsg::AddCursorAbove));

    assert_eq!(model.editor().cursor_count(), 2, "Should have 2 cursors");

    let active = model.editor().active_cursor();
    assert_eq!(
        active.line, 1,
        "Active cursor should be on the new line (line 1)"
    );
    assert_eq!(model.editor().active_cursor_index, 0);
}

#[test]
fn test_active_cursor_survives_sort() {
    let mut model = test_model("line 0\nline 1\nline 2\nline 3\n", 2, 0);

    model.editor_mut().toggle_cursor_at(0, 0);
    assert_eq!(
        model.editor().active_cursor().line,
        0,
        "New cursor at line 0 should be active"
    );

    model.editor_mut().toggle_cursor_at(3, 0);
    assert_eq!(
        model.editor().active_cursor().line,
        3,
        "New cursor at line 3 should be active"
    );

    assert_eq!(model.editor().cursor_count(), 3);
    assert_eq!(model.editor().cursors[0].line, 0);
    assert_eq!(model.editor().cursors[1].line, 2);
    assert_eq!(model.editor().cursors[2].line, 3);
    assert_eq!(model.editor().active_cursor().line, 3);
}

#[test]
fn test_active_cursor_handles_dedup() {
    let mut model = test_model("line 0\nline 1\n", 0, 0);

    let cursor_clone = model.editor().cursors[0];
    let selection_clone = model.editor().selections[0];
    {
        let editor = model.editor_mut();
        editor.cursors.push(cursor_clone);
        editor.selections.push(selection_clone);
        editor.active_cursor_index = 1;
    }

    assert_eq!(model.editor().cursor_count(), 2);

    model.editor_mut().deduplicate_cursors();

    assert_eq!(model.editor().cursor_count(), 1);
    assert_eq!(model.editor().active_cursor_index, 0);
    assert_eq!(model.editor().active_cursor().line, 0);
}

// ========================================================================
// Collapse to Single Cursor Tests
// ========================================================================

#[test]
fn test_collapse_to_single_cursor_resets_active_index() {
    let mut model = test_model("line 0\nline 1\nline 2\nline 3\n", 0, 0);

    model.editor_mut().add_cursor_at(1, 0);
    model.editor_mut().add_cursor_at(2, 0);

    assert_eq!(model.editor().cursor_count(), 3, "Should have 3 cursors");
    assert!(
        model.editor().active_cursor_index > 0,
        "Active cursor should not be primary after adding cursors"
    );

    update(&mut model, Msg::Editor(EditorMsg::CollapseToSingleCursor));

    assert_eq!(
        model.editor().cursor_count(),
        1,
        "Should have 1 cursor after collapse"
    );
    assert_eq!(
        model.editor().active_cursor_index,
        0,
        "Active cursor index must be reset to 0 after collapse"
    );

    let _ = model.editor().active_cursor();
    let _ = model.editor().active_selection();
}

#[test]
fn test_collapse_to_single_cursor_no_panic_on_active_cursor_access() {
    let mut model = test_model("line 0\nline 1\nline 2\nline 3\n", 0, 0);

    model.editor_mut().add_cursor_at(1, 0);
    model.editor_mut().add_cursor_at(2, 0);
    model.editor_mut().add_cursor_at(3, 0);

    assert_eq!(model.editor().cursor_count(), 4, "Should have 4 cursors");
    assert_eq!(
        model.editor().active_cursor_index,
        3,
        "Active should be last added"
    );

    update(&mut model, Msg::Editor(EditorMsg::CollapseToSingleCursor));

    assert_eq!(model.editor().cursor_count(), 1);
    assert_eq!(
        model.editor().active_cursor_index,
        0,
        "Active index must be 0"
    );
    assert_eq!(model.editor().active_cursor().line, 0);
    let _ = model.editor().active_selection();
}

// ========================================================================
// View / Highlighting Tests
// ========================================================================

#[test]
fn test_all_cursor_lines_accessible() {
    let mut model = test_model("line 0\nline 1\nline 2\nline 3\nline 4\nline 5\n", 1, 0);

    model.editor_mut().toggle_cursor_at(3, 0);
    model.editor_mut().toggle_cursor_at(5, 0);

    assert_eq!(model.editor().cursor_count(), 3);

    let cursor_lines: Vec<usize> = model.editor().cursors.iter().map(|c| c.line).collect();
    assert!(cursor_lines.contains(&1), "Should have cursor at line 1");
    assert!(cursor_lines.contains(&3), "Should have cursor at line 3");
    assert!(cursor_lines.contains(&5), "Should have cursor at line 5");
}

// ========================================================================
// Edge Cursor Expansion Tests
// ========================================================================

#[test]
fn test_add_cursor_below_expands_from_bottom_edge() {
    let mut model = test_model("0\n1\n2\n3\n4\n", 1, 0);

    update(&mut model, Msg::Editor(EditorMsg::AddCursorBelow));
    update(&mut model, Msg::Editor(EditorMsg::AddCursorBelow));
    update(&mut model, Msg::Editor(EditorMsg::AddCursorBelow));

    let lines: Vec<usize> = model.editor().cursors.iter().map(|c| c.line).collect();
    assert_eq!(
        lines,
        vec![1, 2, 3, 4],
        "Cursors should expand downward from bottom edge"
    );
}

#[test]
fn test_add_cursor_above_expands_from_top_edge() {
    let mut model = test_model("0\n1\n2\n3\n4\n", 3, 0);

    update(&mut model, Msg::Editor(EditorMsg::AddCursorAbove));
    update(&mut model, Msg::Editor(EditorMsg::AddCursorAbove));
    update(&mut model, Msg::Editor(EditorMsg::AddCursorAbove));

    let lines: Vec<usize> = model.editor().cursors.iter().map(|c| c.line).collect();
    assert_eq!(
        lines,
        vec![0, 1, 2, 3],
        "Cursors should expand upward from top edge"
    );
}

#[test]
fn test_edge_cursor_helpers() {
    let mut model = test_model("0\n1\n2\n3\n4\n", 2, 0);

    model.editor_mut().add_cursor_at(0, 0);
    model.editor_mut().add_cursor_at(4, 0);

    assert_eq!(model.editor().top_cursor().line, 0);
    assert_eq!(model.editor().bottom_cursor().line, 4);
    assert_eq!(model.editor().edge_cursor_vertical(true).line, 0);
    assert_eq!(model.editor().edge_cursor_vertical(false).line, 4);
}

// ========================================================================
// Multi-Cursor DeleteLine Tests
// ========================================================================

#[test]
fn test_delete_line_multi_cursor_deletes_all_lines() {
    use token::messages::{DocumentMsg, Msg};

    let mut model = test_model("line 0\nline 1\nline 2\nline 3\nline 4\n", 1, 0);

    model.editor_mut().add_cursor_at(2, 0);
    model.editor_mut().add_cursor_at(3, 0);

    assert_eq!(model.editor().cursor_count(), 3);

    update(&mut model, Msg::Document(DocumentMsg::DeleteLine));

    let content: String = model.document().buffer.chars().collect();
    assert_eq!(
        content, "line 0\nline 4\n",
        "Lines 1, 2, 3 should be deleted"
    );
    assert_eq!(
        model.editor().cursor_count(),
        1,
        "Should collapse to single cursor"
    );
}

#[test]
fn test_delete_line_multi_cursor_non_adjacent() {
    use token::messages::{DocumentMsg, Msg};

    let mut model = test_model("line 0\nline 1\nline 2\nline 3\nline 4\n", 1, 0);

    model.editor_mut().add_cursor_at(3, 0);

    assert_eq!(model.editor().cursor_count(), 2);

    update(&mut model, Msg::Document(DocumentMsg::DeleteLine));

    let content: String = model.document().buffer.chars().collect();
    assert_eq!(
        content, "line 0\nline 2\nline 4\n",
        "Lines 1 and 3 should be deleted"
    );
}

#[test]
fn test_delete_line_multi_cursor_all_lines() {
    use token::messages::{DocumentMsg, Msg};

    let mut model = test_model("line 0\nline 1\nline 2\n", 0, 0);

    model.editor_mut().add_cursor_at(1, 0);
    model.editor_mut().add_cursor_at(2, 0);

    assert_eq!(model.editor().cursor_count(), 3);

    update(&mut model, Msg::Document(DocumentMsg::DeleteLine));

    let content: String = model.document().buffer.chars().collect();
    assert!(
        content.is_empty() || content == "\n" || content.trim().is_empty(),
        "All lines should be deleted, got: {:?}",
        content
    );
}

// ========================================================================
// Placeholder Tests for Unimplemented Multi-Cursor Features
// ========================================================================

#[test]
fn test_multi_cursor_undo_redo_preserves_all_cursors() {
    use token::messages::{DocumentMsg, Msg};

    let mut model = test_model("aaa\nbbb\nccc\n", 0, 0);

    model.editor_mut().add_cursor_at(1, 0);
    model.editor_mut().add_cursor_at(2, 0);
    assert_eq!(model.editor().cursor_count(), 3);

    let cursors_before: Vec<_> = model.editor().cursors.clone();

    update(&mut model, Msg::Document(DocumentMsg::InsertChar('X')));

    let content: String = model.document().buffer.chars().collect();
    assert_eq!(content, "Xaaa\nXbbb\nXccc\n");

    assert_eq!(model.editor().cursor_count(), 3);
    for cursor in &model.editor().cursors {
        assert_eq!(cursor.column, 1, "All cursors should be at column 1 after insert");
    }
    let cursors_after: Vec<_> = model.editor().cursors.clone();

    update(&mut model, Msg::Document(DocumentMsg::Undo));

    let content: String = model.document().buffer.chars().collect();
    assert_eq!(content, "aaa\nbbb\nccc\n", "Undo should restore original text");

    assert_eq!(
        model.editor().cursor_count(),
        3,
        "Undo should preserve all 3 cursors"
    );
    for (i, cursor) in model.editor().cursors.iter().enumerate() {
        assert_eq!(
            cursor.line, cursors_before[i].line,
            "Cursor {} line should be restored",
            i
        );
        assert_eq!(
            cursor.column, cursors_before[i].column,
            "Cursor {} column should be restored",
            i
        );
    }

    update(&mut model, Msg::Document(DocumentMsg::Redo));

    let content: String = model.document().buffer.chars().collect();
    assert_eq!(content, "Xaaa\nXbbb\nXccc\n", "Redo should re-apply inserts");

    assert_eq!(
        model.editor().cursor_count(),
        3,
        "Redo should preserve all 3 cursors"
    );
    for (i, cursor) in model.editor().cursors.iter().enumerate() {
        assert_eq!(
            cursor.line, cursors_after[i].line,
            "Cursor {} line should match after redo",
            i
        );
        assert_eq!(
            cursor.column, cursors_after[i].column,
            "Cursor {} column should match after redo",
            i
        );
    }
}

// ========================================================================
// Multi-Cursor Indent/Unindent Tests
// ========================================================================

#[test]
fn test_indent_multi_cursor_indents_all_lines() {
    use token::messages::{DocumentMsg, Msg};

    let mut model = test_model("line 0\nline 1\nline 2\nline 3\nline 4\n", 1, 0);

    // Add cursors on lines 1, 2, 3
    model.editor_mut().add_cursor_at(2, 0);
    model.editor_mut().add_cursor_at(3, 0);

    assert_eq!(model.editor().cursor_count(), 3);

    update(&mut model, Msg::Document(DocumentMsg::IndentLines));

    let content: String = model.document().buffer.chars().collect();
    assert_eq!(
        content, "line 0\n\tline 1\n\tline 2\n\tline 3\nline 4\n",
        "Lines 1, 2, 3 should be indented"
    );

    // All cursors should have column incremented
    for cursor in &model.editor().cursors {
        assert_eq!(cursor.column, 1, "Cursor column should be 1 after indent");
    }
}

#[test]
fn test_indent_multi_cursor_non_adjacent() {
    use token::messages::{DocumentMsg, Msg};

    let mut model = test_model("line 0\nline 1\nline 2\nline 3\nline 4\n", 1, 0);

    // Add cursor on line 3 (non-adjacent)
    model.editor_mut().add_cursor_at(3, 0);

    assert_eq!(model.editor().cursor_count(), 2);

    update(&mut model, Msg::Document(DocumentMsg::IndentLines));

    let content: String = model.document().buffer.chars().collect();
    assert_eq!(
        content, "line 0\n\tline 1\nline 2\n\tline 3\nline 4\n",
        "Lines 1 and 3 should be indented"
    );
}

#[test]
fn test_unindent_multi_cursor_unindents_all_lines() {
    use token::messages::{DocumentMsg, Msg};

    let mut model = test_model("line 0\n\tline 1\n\tline 2\n\tline 3\nline 4\n", 1, 1);

    // Add cursors on lines 2, 3 (at column 1 after the tab)
    model.editor_mut().add_cursor_at(2, 1);
    model.editor_mut().add_cursor_at(3, 1);

    assert_eq!(model.editor().cursor_count(), 3);

    update(&mut model, Msg::Document(DocumentMsg::UnindentLines));

    let content: String = model.document().buffer.chars().collect();
    assert_eq!(
        content, "line 0\nline 1\nline 2\nline 3\nline 4\n",
        "Lines 1, 2, 3 should be unindented"
    );

    // All cursors should have column decremented to 0
    for cursor in &model.editor().cursors {
        assert_eq!(cursor.column, 0, "Cursor column should be 0 after unindent");
    }
}

#[test]
fn test_unindent_multi_cursor_non_adjacent() {
    use token::messages::{DocumentMsg, Msg};

    let mut model = test_model("line 0\n\tline 1\nline 2\n\tline 3\nline 4\n", 1, 1);

    // Add cursor on line 3 (non-adjacent)
    model.editor_mut().add_cursor_at(3, 1);

    assert_eq!(model.editor().cursor_count(), 2);

    update(&mut model, Msg::Document(DocumentMsg::UnindentLines));

    let content: String = model.document().buffer.chars().collect();
    assert_eq!(
        content, "line 0\nline 1\nline 2\nline 3\nline 4\n",
        "Lines 1 and 3 should be unindented"
    );
}

#[test]
fn test_unindent_spaces_multi_cursor() {
    use token::messages::{DocumentMsg, Msg};

    let mut model = test_model("line 0\n    line 1\n    line 2\nline 3\n", 1, 4);

    model.editor_mut().add_cursor_at(2, 4);

    assert_eq!(model.editor().cursor_count(), 2);

    update(&mut model, Msg::Document(DocumentMsg::UnindentLines));

    let content: String = model.document().buffer.chars().collect();
    assert_eq!(
        content, "line 0\nline 1\nline 2\nline 3\n",
        "4-space indent should be removed from lines 1 and 2"
    );
}

// ========================================================================
// Multi-Cursor Duplicate Tests
// ========================================================================

#[test]
fn test_duplicate_line_multi_cursor() {
    use token::messages::{DocumentMsg, Msg};

    let mut model = test_model("line 0\nline 1\nline 2\nline 3\n", 1, 0);

    // Add cursor on line 2
    model.editor_mut().add_cursor_at(2, 0);

    assert_eq!(model.editor().cursor_count(), 2);

    update(&mut model, Msg::Document(DocumentMsg::Duplicate));

    let content: String = model.document().buffer.chars().collect();
    assert_eq!(
        content, "line 0\nline 1\nline 1\nline 2\nline 2\nline 3\n",
        "Lines 1 and 2 should be duplicated"
    );

    // Cursors should move to duplicated lines
    assert_eq!(model.editor().cursor_count(), 2);
}

#[test]
fn test_duplicate_line_multi_cursor_non_adjacent() {
    use token::messages::{DocumentMsg, Msg};

    let mut model = test_model("line 0\nline 1\nline 2\nline 3\nline 4\n", 1, 0);

    // Add cursor on line 3 (non-adjacent)
    model.editor_mut().add_cursor_at(3, 0);

    assert_eq!(model.editor().cursor_count(), 2);

    update(&mut model, Msg::Document(DocumentMsg::Duplicate));

    let content: String = model.document().buffer.chars().collect();
    assert_eq!(
        content, "line 0\nline 1\nline 1\nline 2\nline 3\nline 3\nline 4\n",
        "Lines 1 and 3 should be duplicated"
    );
}

#[test]
fn test_duplicate_with_selection_multi_cursor() {
    use token::messages::{DocumentMsg, Msg};

    let mut model = test_model("hello world\nfoo bar\n", 0, 0);

    // Select "hello" on line 0
    model.editor_mut().selections[0].anchor = Position::new(0, 0);
    model.editor_mut().selections[0].head = Position::new(0, 5);
    model.editor_mut().cursors[0].column = 5;

    // Add cursor with selection "foo" on line 1
    model.editor_mut().add_cursor_at(1, 3);
    model.editor_mut().selections[1].anchor = Position::new(1, 0);
    model.editor_mut().selections[1].head = Position::new(1, 3);

    assert_eq!(model.editor().cursor_count(), 2);

    update(&mut model, Msg::Document(DocumentMsg::Duplicate));

    let content: String = model.document().buffer.chars().collect();
    assert_eq!(
        content, "hellohello world\nfoofoo bar\n",
        "Selections should be duplicated in place"
    );
}

// ========================================================================
// Bug #26: Arrow key with selection should collapse to selection boundary
// ========================================================================

#[test]
fn test_left_arrow_collapses_selection_to_start_multi_cursor() {
    use token::messages::{Direction, EditorMsg, Msg};

    // Setup: "// One\n// Two\n// Three\n"
    // Three cursors with selections covering "// " on each line
    let mut model = test_model("// One\n// Two\n// Three\n", 0, 3);

    // First cursor selects "// " on line 0 (columns 0-3)
    model.editor_mut().selections[0].anchor = Position::new(0, 0);
    model.editor_mut().selections[0].head = Position::new(0, 3);
    model.editor_mut().cursors[0].column = 3;

    // Add second cursor on line 1 with selection "// " (columns 0-3)
    model.editor_mut().add_cursor_at(1, 3);
    model.editor_mut().selections[1].anchor = Position::new(1, 0);
    model.editor_mut().selections[1].head = Position::new(1, 3);

    // Add third cursor on line 2 with selection "// " (columns 0-3)
    model.editor_mut().add_cursor_at(2, 3);
    model.editor_mut().selections[2].anchor = Position::new(2, 0);
    model.editor_mut().selections[2].head = Position::new(2, 3);

    assert_eq!(model.editor().cursor_count(), 3);

    // Press Left arrow - all cursors should move to selection START (column 0)
    update(
        &mut model,
        Msg::Editor(EditorMsg::MoveCursor(Direction::Left)),
    );

    // All cursors should be at column 0 (start of their selections)
    assert_eq!(model.editor().cursors[0].line, 0);
    assert_eq!(
        model.editor().cursors[0].column,
        0,
        "Cursor 0 should be at selection start (col 0)"
    );

    assert_eq!(model.editor().cursors[1].line, 1);
    assert_eq!(
        model.editor().cursors[1].column,
        0,
        "Cursor 1 should be at selection start (col 0)"
    );

    assert_eq!(model.editor().cursors[2].line, 2);
    assert_eq!(
        model.editor().cursors[2].column,
        0,
        "Cursor 2 should be at selection start (col 0)"
    );

    // All selections should be cleared (empty)
    for (idx, selection) in model.editor().selections.iter().enumerate() {
        assert!(
            selection.is_empty(),
            "Selection {} should be empty after left arrow",
            idx
        );
    }
}

#[test]
fn test_right_arrow_collapses_selection_to_end_multi_cursor() {
    use token::messages::{Direction, EditorMsg, Msg};

    let mut model = test_model("// One\n// Two\n// Three\n", 0, 0);

    // First cursor at column 0, selects "// " (0-3)
    model.editor_mut().selections[0].anchor = Position::new(0, 0);
    model.editor_mut().selections[0].head = Position::new(0, 3);
    model.editor_mut().cursors[0].column = 3;

    // Add second cursor on line 1
    model.editor_mut().add_cursor_at(1, 3);
    model.editor_mut().selections[1].anchor = Position::new(1, 0);
    model.editor_mut().selections[1].head = Position::new(1, 3);

    // Add third cursor on line 2
    model.editor_mut().add_cursor_at(2, 3);
    model.editor_mut().selections[2].anchor = Position::new(2, 0);
    model.editor_mut().selections[2].head = Position::new(2, 3);

    assert_eq!(model.editor().cursor_count(), 3);

    // Press Right arrow - all cursors should stay at selection END (column 3)
    update(
        &mut model,
        Msg::Editor(EditorMsg::MoveCursor(Direction::Right)),
    );

    // All cursors should be at column 3 (end of their selections)
    assert_eq!(
        model.editor().cursors[0].column,
        3,
        "Cursor 0 should be at selection end (col 3)"
    );
    assert_eq!(
        model.editor().cursors[1].column,
        3,
        "Cursor 1 should be at selection end (col 3)"
    );
    assert_eq!(
        model.editor().cursors[2].column,
        3,
        "Cursor 2 should be at selection end (col 3)"
    );

    // All selections should be cleared
    for (idx, selection) in model.editor().selections.iter().enumerate() {
        assert!(
            selection.is_empty(),
            "Selection {} should be empty after right arrow",
            idx
        );
    }
}

#[test]
fn test_left_arrow_with_reversed_selection_multi_cursor() {
    use token::messages::{Direction, EditorMsg, Msg};

    // Reversed selection: anchor is AFTER head (user selected backwards)
    let mut model = test_model("hello world\nfoo bar\n", 0, 0);

    // Cursor 0: reversed selection from col 5 to col 0 (head before anchor)
    model.editor_mut().selections[0].anchor = Position::new(0, 5);
    model.editor_mut().selections[0].head = Position::new(0, 0);
    model.editor_mut().cursors[0].column = 0;

    // Cursor 1: reversed selection from col 3 to col 0
    model.editor_mut().add_cursor_at(1, 0);
    model.editor_mut().selections[1].anchor = Position::new(1, 3);
    model.editor_mut().selections[1].head = Position::new(1, 0);

    assert_eq!(model.editor().cursor_count(), 2);

    // Press Left - should go to start of selection (col 0)
    update(
        &mut model,
        Msg::Editor(EditorMsg::MoveCursor(Direction::Left)),
    );

    assert_eq!(
        model.editor().cursors[0].column,
        0,
        "Cursor 0 should be at selection start"
    );
    assert_eq!(
        model.editor().cursors[1].column,
        0,
        "Cursor 1 should be at selection start"
    );

    for selection in model.editor().selections.iter() {
        assert!(selection.is_empty(), "Selection should be cleared");
    }
}

#[test]
fn test_left_arrow_no_selection_moves_by_one_char() {
    use token::messages::{Direction, EditorMsg, Msg};

    let mut model = test_model("hello\nworld\n", 0, 3);
    model.editor_mut().add_cursor_at(1, 3);

    assert_eq!(model.editor().cursor_count(), 2);
    // No selections (selections are empty/collapsed)

    update(
        &mut model,
        Msg::Editor(EditorMsg::MoveCursor(Direction::Left)),
    );

    // Should move left by 1 char
    assert_eq!(
        model.editor().cursors[0].column,
        2,
        "Cursor 0 should move left by 1"
    );
    assert_eq!(
        model.editor().cursors[1].column,
        2,
        "Cursor 1 should move left by 1"
    );
}

// ========================================================================
// Bug #27: Multi-cursor duplicate line should adjust cursor positions
// ========================================================================

#[test]
fn test_duplicate_line_multi_cursor_cursors_stay_visually_correct() {
    use token::messages::{DocumentMsg, Msg};

    // Setup: three lines, cursors on lines 0, 1, 2
    let mut model = test_model("// One\n// Two\n// Three\n", 0, 0);

    model.editor_mut().add_cursor_at(1, 0);
    model.editor_mut().add_cursor_at(2, 0);

    assert_eq!(model.editor().cursor_count(), 3);

    // Duplicate line (no selections)
    update(&mut model, Msg::Document(DocumentMsg::Duplicate));

    let content: String = model.document().buffer.chars().collect();
    // Each line should be duplicated
    assert_eq!(
        content, "// One\n// One\n// Two\n// Two\n// Three\n// Three\n",
        "All three lines should be duplicated"
    );

    // Cursors should be on the duplicated lines (lines 1, 3, 5)
    // so they visually stay on the "same" text they were on
    assert_eq!(model.editor().cursor_count(), 3);
    assert_eq!(
        model.editor().cursors[0].line,
        1,
        "Cursor 0 should be on line 1 (duplicated // One)"
    );
    assert_eq!(
        model.editor().cursors[1].line,
        3,
        "Cursor 1 should be on line 3 (duplicated // Two)"
    );
    assert_eq!(
        model.editor().cursors[2].line,
        5,
        "Cursor 2 should be on line 5 (duplicated // Three)"
    );
}

#[test]
fn test_duplicate_selection_multi_cursor_inserts_after_each_selection() {
    use token::messages::{DocumentMsg, Msg};

    // Setup: "// One\n// Two\n// Three\n"
    // Select "// " on each line, duplicate should insert "// " after each selection
    let mut model = test_model("// One\n// Two\n// Three\n", 0, 3);

    // First cursor selects "// " on line 0 (columns 0-3)
    model.editor_mut().selections[0].anchor = Position::new(0, 0);
    model.editor_mut().selections[0].head = Position::new(0, 3);
    model.editor_mut().cursors[0].column = 3;

    // Add second cursor on line 1 with selection "// " (columns 0-3)
    model.editor_mut().add_cursor_at(1, 3);
    model.editor_mut().selections[1].anchor = Position::new(1, 0);
    model.editor_mut().selections[1].head = Position::new(1, 3);

    // Add third cursor on line 2 with selection "// " (columns 0-3)
    model.editor_mut().add_cursor_at(2, 3);
    model.editor_mut().selections[2].anchor = Position::new(2, 0);
    model.editor_mut().selections[2].head = Position::new(2, 3);

    assert_eq!(model.editor().cursor_count(), 3);

    update(&mut model, Msg::Document(DocumentMsg::Duplicate));

    let content: String = model.document().buffer.chars().collect();
    // "// " should be duplicated after each selection point
    assert_eq!(
        content, "// // One\n// // Two\n// // Three\n",
        "Each '// ' selection should be duplicated in place"
    );

    // Cursors should be at the end of each duplicated text (column 6)
    assert_eq!(
        model.editor().cursors[0].column,
        6,
        "Cursor 0 should be at col 6"
    );
    assert_eq!(
        model.editor().cursors[1].column,
        6,
        "Cursor 1 should be at col 6"
    );
    assert_eq!(
        model.editor().cursors[2].column,
        6,
        "Cursor 2 should be at col 6"
    );
}

// ========================================================================
// Bug #28: DeleteWordBackward (Option+Backspace)
// ========================================================================

#[test]
fn test_delete_word_backward_single_cursor() {
    use token::messages::{DocumentMsg, Msg};

    let mut model = test_model("hello world", 0, 11); // cursor at end

    update(&mut model, Msg::Document(DocumentMsg::DeleteWordBackward));

    let content: String = model.document().buffer.chars().collect();
    assert_eq!(content, "hello ", "Should delete 'world'");
    assert_eq!(model.editor().cursors[0].column, 6);
}

#[test]
fn test_delete_word_backward_multi_cursor() {
    use token::messages::{DocumentMsg, Msg};

    // Three lines, each ending with a word we want to delete
    // "/// This is anotherword" = 23 chars (0-22), cursor at 23
    // "/// This is notreallyaword" = 26 chars (0-25), cursor at 26
    // "/// This is actuallyword" = 24 chars (0-23), cursor at 24
    let mut model = test_model(
        "/// This is anotherword\n/// This is notreallyaword\n/// This is actuallyword\n",
        0,
        23,
    );

    // Position cursors at the end of each line
    model.editor_mut().cursors[0].column = 23; // end of line 0
    model.editor_mut().add_cursor_at(1, 26); // end of line 1
    model.editor_mut().add_cursor_at(2, 24); // end of line 2

    assert_eq!(model.editor().cursor_count(), 3);

    update(&mut model, Msg::Document(DocumentMsg::DeleteWordBackward));

    let content: String = model.document().buffer.chars().collect();
    assert_eq!(
        content, "/// This is \n/// This is \n/// This is \n",
        "Each word should be deleted"
    );
}

#[test]
fn test_delete_word_backward_at_beginning() {
    use token::messages::{DocumentMsg, Msg};

    let mut model = test_model("hello", 0, 0); // cursor at beginning

    update(&mut model, Msg::Document(DocumentMsg::DeleteWordBackward));

    let content: String = model.document().buffer.chars().collect();
    assert_eq!(content, "hello", "Nothing should be deleted at beginning");
}

// ========================================================================
// Bug #29: DeleteLine should preserve cursors for non-contiguous lines
// ========================================================================

#[test]
fn test_delete_line_non_contiguous_preserves_cursor_count() {
    use token::messages::{DocumentMsg, Msg};

    // Lines with empty lines between them
    // Line 0: "line 0"
    // Line 1: ""
    // Line 2: "line 2"
    // Line 3: ""
    // Line 4: "line 4"
    let mut model = test_model("line 0\n\nline 2\n\nline 4\n", 0, 0);

    // Add cursors on lines 0, 2, 4 (non-contiguous)
    model.editor_mut().add_cursor_at(2, 0);
    model.editor_mut().add_cursor_at(4, 0);

    assert_eq!(model.editor().cursor_count(), 3);

    update(&mut model, Msg::Document(DocumentMsg::DeleteLine));

    let content: String = model.document().buffer.chars().collect();
    // Lines 0, 2, 4 deleted, only empty lines remain
    assert_eq!(content, "\n\n", "Only the empty lines should remain");

    // Should still have cursors (may be deduplicated if they land on same position)
    assert!(
        model.editor().cursor_count() >= 1,
        "Should preserve cursors"
    );
}

#[test]
fn test_delete_line_contiguous_collapses_to_single_cursor() {
    use token::messages::{DocumentMsg, Msg};

    let mut model = test_model("line 0\nline 1\nline 2\nline 3\n", 0, 0);

    // Add cursors on lines 0, 1, 2 (contiguous)
    model.editor_mut().add_cursor_at(1, 0);
    model.editor_mut().add_cursor_at(2, 0);

    assert_eq!(model.editor().cursor_count(), 3);

    update(&mut model, Msg::Document(DocumentMsg::DeleteLine));

    let content: String = model.document().buffer.chars().collect();
    assert_eq!(content, "line 3\n", "Lines 0, 1, 2 should be deleted");

    // Contiguous deletion should collapse to single cursor
    assert_eq!(
        model.editor().cursor_count(),
        1,
        "Should collapse to single cursor for contiguous lines"
    );
}

// ========================================================================
// Bug #30: InsertNewline should adjust other cursors after each insertion
// ========================================================================

#[test]
fn test_insert_newline_multi_cursor_adjusts_positions() {
    use token::messages::{DocumentMsg, Msg};

    // Two lines, cursors at end of each line
    let mut model = test_model("hello\nworld\n", 0, 5);
    model.editor_mut().add_cursor_at(1, 5);

    assert_eq!(model.editor().cursor_count(), 2);
    assert_eq!(model.editor().cursors[0].line, 0);
    assert_eq!(model.editor().cursors[1].line, 1);

    // Insert newline at each cursor position
    update(&mut model, Msg::Document(DocumentMsg::InsertNewline));

    let content: String = model.document().buffer.chars().collect();
    // "hello\n" + newline + "world\n" + newline = "hello\n\nworld\n\n"
    assert_eq!(
        content, "hello\n\nworld\n\n",
        "Two newlines should be inserted"
    );

    // Cursor 0 was at (0, 5), after newline should be at (1, 0)
    // Cursor 1 was at (1, 5), after its newline should be at (3, 0)
    // (Because cursor 0's newline shifted line 1 to line 2, then cursor 1's newline at old line 2 makes it line 3)
    assert_eq!(
        model.editor().cursors[0].line,
        1,
        "Cursor 0 should be on line 1"
    );
    assert_eq!(model.editor().cursors[0].column, 0);
    assert_eq!(
        model.editor().cursors[1].line,
        3,
        "Cursor 1 should be on line 3"
    );
    assert_eq!(model.editor().cursors[1].column, 0);
}

// ========================================================================
// Bug #31: DeleteBackward should adjust other cursors after deleting newline
// ========================================================================

#[test]
fn test_delete_backward_newline_multi_cursor_adjusts_positions() {
    use token::messages::{DocumentMsg, Msg};

    // Setup: 4 lines with cursors at the start of each (column 0)
    // "Text1\nText2\nText3\nText4\n"
    // Cursors at lines 1, 2, 3, 4 (start of each line after Text1)
    let mut model = test_model("Text1\nText2\nText3\nText4\n", 1, 0);
    model.editor_mut().add_cursor_at(2, 0);
    model.editor_mut().add_cursor_at(3, 0);
    model.editor_mut().add_cursor_at(4, 0);

    assert_eq!(model.editor().cursor_count(), 4);
    assert_eq!(model.editor().cursors[0].line, 1);
    assert_eq!(model.editor().cursors[1].line, 2);
    assert_eq!(model.editor().cursors[2].line, 3);
    assert_eq!(model.editor().cursors[3].line, 4);

    // Delete backward at each cursor (should delete the newline before each cursor)
    update(&mut model, Msg::Document(DocumentMsg::DeleteBackward));

    let content: String = model.document().buffer.chars().collect();
    // Each backspace deletes the newline before it, joining all lines
    assert_eq!(
        content, "Text1Text2Text3Text4",
        "All newlines should be deleted"
    );

    // All cursors should now be on line 0 at appropriate columns
    // After deleting newlines in reverse order (bottom to top):
    // - Cursor 3 (was line 4, col 0) deletes \n after Text4, becomes (line 3, col 5)
    // - Cursor 2 (was line 3, col 0) deletes \n after Text3, becomes (line 2, col 5)
    //   But cursor 3 is adjusted down to line 2
    // - And so on...
    // Final result: all on line 0
    assert_eq!(model.editor().cursor_count(), 4);
    for (i, cursor) in model.editor().cursors.iter().enumerate() {
        assert_eq!(cursor.line, 0, "Cursor {} should be on line 0", i);
    }
}

#[test]
fn test_delete_backward_newline_multi_cursor_column_positions() {
    use token::messages::{DocumentMsg, Msg};

    // Simpler test: 2 cursors at start of lines 1 and 2
    let mut model = test_model("AAA\nBBB\nCCC\n", 1, 0);
    model.editor_mut().add_cursor_at(2, 0);

    assert_eq!(model.editor().cursor_count(), 2);

    update(&mut model, Msg::Document(DocumentMsg::DeleteBackward));

    let content: String = model.document().buffer.chars().collect();
    assert_eq!(content, "AAABBBCCC\n", "Both newlines should be deleted");

    // Cursor 0: was at (1, 0), deleted \n before it, now at (0, 3) - after "AAA"
    // Cursor 1: was at (2, 0), deleted \n before it, now at (0, 6) - after "AAABBB"
    assert_eq!(model.editor().cursors[0].line, 0);
    assert_eq!(
        model.editor().cursors[0].column,
        3,
        "Cursor 0 should be at column 3"
    );
    assert_eq!(model.editor().cursors[1].line, 0);
    assert_eq!(
        model.editor().cursors[1].column,
        6,
        "Cursor 1 should be at column 6"
    );
}
