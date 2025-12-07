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

    let cursor_clone = model.editor().cursors[0].clone();
    let selection_clone = model.editor().selections[0].clone();
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
#[ignore = "Requires undo/redo to be updated for multi-cursor support"]
fn test_multi_cursor_undo_redo_preserves_all_cursors() {
    // Undo/redo should preserve all cursor positions, not just primary
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
