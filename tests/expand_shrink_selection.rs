//! Tests for expand/shrink selection feature

mod common;

use common::test_model;
use token::messages::{DocumentMsg, EditorMsg, Msg};
use token::syntax::{LanguageId, ParserState, SyntaxTreeSnapshot};
use token::update::update;

fn attach_tree(model: &mut token::model::AppModel, language: LanguageId) {
    let document_id = model
        .document()
        .id
        .expect("test document should have an id");
    let source = model.document().buffer.to_string();
    let revision = 1;
    model.document_mut().language = language;
    model.document_mut().revision = revision;

    let mut parser = ParserState::new();
    parser.parse_and_highlight(&source, language, document_id, revision);
    let tree = parser
        .get_cached_tree(document_id)
        .expect("test language should produce a parse tree")
        .0
        .clone();
    model.document_mut().syntax_tree = Some(SyntaxTreeSnapshot::new(revision, language, tree));
}

fn attach_rust_tree(model: &mut token::model::AppModel) {
    attach_tree(model, LanguageId::Rust);
}

// ============================================================================
// Expand Selection Tests
// ============================================================================

#[test]
fn test_expand_from_cursor_selects_word() {
    // Cursor in middle of "hello"
    let mut model = test_model("hello world\n", 0, 2);

    update(&mut model, Msg::Editor(EditorMsg::ExpandSelection));

    // Should select "hello" (columns 0-5)
    let sel = model.editor().primary_selection();
    assert_eq!(sel.start().line, 0);
    assert_eq!(sel.start().column, 0);
    assert_eq!(sel.end().line, 0);
    assert_eq!(sel.end().column, 5);
}

#[test]
fn test_expand_uses_rust_syntax_ancestors() {
    let source = "fn main() { let value = foo.bar(); }\n";
    let bar_column = source.find("bar").expect("bar should exist") + 1;
    let mut model = test_model(source, 0, bar_column);
    attach_rust_tree(&mut model);

    update(&mut model, Msg::Editor(EditorMsg::ExpandSelection));
    assert_eq!(
        model
            .editor()
            .primary_selection()
            .get_text(model.document()),
        "bar"
    );

    update(&mut model, Msg::Editor(EditorMsg::ExpandSelection));
    assert_eq!(
        model
            .editor()
            .primary_selection()
            .get_text(model.document()),
        "foo.bar"
    );

    update(&mut model, Msg::Editor(EditorMsg::ExpandSelection));
    assert_eq!(
        model
            .editor()
            .primary_selection()
            .get_text(model.document()),
        "foo.bar()"
    );
}

#[test]
fn test_expand_at_markup_eol_selects_completed_element() {
    let source = "<book>\n  <price>5.95</price>   \n  <title>Next</title>\n</book>\n";
    let column = source
        .lines()
        .nth(1)
        .expect("fixture line should exist")
        .len();
    let mut model = test_model(source, 1, column);
    attach_tree(&mut model, LanguageId::Xml);

    update(&mut model, Msg::Editor(EditorMsg::ExpandSelection));

    assert_eq!(
        model
            .editor()
            .primary_selection()
            .get_text(model.document()),
        "<price>5.95</price>"
    );
}

#[test]
fn test_expand_at_code_eol_selects_completed_statement() {
    let source = "fn main() {\n    let value = foo();\n    next();\n}\n";
    let line = source.lines().nth(1).expect("fixture line should exist");
    let mut model = test_model(source, 1, line.chars().count());
    attach_tree(&mut model, LanguageId::Rust);

    update(&mut model, Msg::Editor(EditorMsg::ExpandSelection));

    assert_eq!(
        model
            .editor()
            .primary_selection()
            .get_text(model.document()),
        "let value = foo();"
    );
}

#[test]
fn test_expand_at_ambiguous_separator_eol_falls_back_to_line() {
    let source = "{\n  \"first\": 1,\n  \"second\": 2\n}\n";
    let line = source.lines().nth(1).expect("fixture line should exist");
    let mut model = test_model(source, 1, line.chars().count());
    attach_tree(&mut model, LanguageId::Json);

    update(&mut model, Msg::Editor(EditorMsg::ExpandSelection));

    assert_eq!(
        model
            .editor()
            .primary_selection()
            .get_text(model.document()),
        "  \"first\": 1,"
    );
}

#[test]
fn test_stale_tree_uses_plaintext_fallback() {
    let source = "fn main() { let value = foo.bar(); }\n";
    let bar_column = source.find("bar").expect("bar should exist") + 1;
    let mut model = test_model(source, 0, bar_column);
    attach_rust_tree(&mut model);
    model.document_mut().revision += 1;

    update(&mut model, Msg::Editor(EditorMsg::ExpandSelection));
    update(&mut model, Msg::Editor(EditorMsg::ExpandSelection));

    assert_eq!(model.editor().primary_selection().start().column, 0);
    assert_eq!(
        model.editor().primary_selection().end().column,
        model.document().line_length(0)
    );
}

#[test]
fn test_expand_from_word_selects_line() {
    let mut model = test_model("hello world\n", 0, 2);

    // First expand: cursor → word
    update(&mut model, Msg::Editor(EditorMsg::ExpandSelection));

    // Second expand: word → line
    update(&mut model, Msg::Editor(EditorMsg::ExpandSelection));

    // Should select entire line content (NOT including newline)
    let sel = model.editor().primary_selection();
    assert_eq!(sel.start().line, 0);
    assert_eq!(sel.start().column, 0);
    assert_eq!(sel.end().line, 0);
    assert_eq!(sel.end().column, 11); // "hello world" = 11 chars
}

#[test]
fn test_expand_from_line_selects_all() {
    let mut model = test_model("hello\nworld\n", 0, 2);

    // Expand three times: cursor → word → line → all
    update(&mut model, Msg::Editor(EditorMsg::ExpandSelection));
    update(&mut model, Msg::Editor(EditorMsg::ExpandSelection));
    update(&mut model, Msg::Editor(EditorMsg::ExpandSelection));

    // Should select entire document
    // Document: "hello\nworld\n" = 2 lines + trailing newline creates empty line 2
    // Last line is line 2 (empty), so end should be at (2, 0) or the actual last content
    let sel = model.editor().primary_selection();
    assert_eq!(sel.start().line, 0);
    assert_eq!(sel.start().column, 0);
    // The document has "hello\nworld\n" which is 12 chars
    // Line count is 2, last line is index 1, last column is 6 (world\n = 6 chars)
    // But line_length returns chars excluding newline, so it's 5
    // Actually: the selection should go to the very end
    let total_lines = model.document().line_count();
    let last_line = total_lines.saturating_sub(1);
    let last_col = model.document().line_length(last_line);
    assert_eq!(sel.end().line, last_line);
    assert_eq!(sel.end().column, last_col);
}

#[test]
fn test_expand_already_all_does_nothing() {
    let mut model = test_model("hello\nworld\n", 0, 0);

    // Expand to all
    update(&mut model, Msg::Editor(EditorMsg::ExpandSelection));
    update(&mut model, Msg::Editor(EditorMsg::ExpandSelection));
    update(&mut model, Msg::Editor(EditorMsg::ExpandSelection));

    let sel_before = *model.editor().primary_selection();

    // Expand again - should not change
    update(&mut model, Msg::Editor(EditorMsg::ExpandSelection));

    let sel_after = model.editor().primary_selection();
    assert_eq!(sel_before.start(), sel_after.start());
    assert_eq!(sel_before.end(), sel_after.end());
}

#[test]
fn test_expand_on_whitespace() {
    // Cursor on space between words
    let mut model = test_model("hello world\n", 0, 5);

    update(&mut model, Msg::Editor(EditorMsg::ExpandSelection));

    // Cursor is at end of "hello", so word_under_cursor may not find a word
    // Should expand to line instead
    let sel = model.editor().primary_selection();
    assert_eq!(sel.start().line, 0);
    assert_eq!(sel.start().column, 0);
}

#[test]
fn test_expand_on_empty_line() {
    let mut model = test_model("hello\n\nworld\n", 1, 0);

    update(&mut model, Msg::Editor(EditorMsg::ExpandSelection));

    // The zero-sized line range is not a strict expansion, so skip to all.
    let sel = model.editor().primary_selection();
    assert_eq!(sel.start().line, 0);
    assert_eq!(sel.start().column, 0);
    let last_line = model.document().line_count().saturating_sub(1);
    assert_eq!(sel.end().line, last_line);
    assert_eq!(sel.end().column, model.document().line_length(last_line));
}

#[test]
fn test_expand_to_line_cursor_at_end_of_line_content() {
    // When expanding to line, cursor and selection should be at end of line content
    // (before newline), not at the start of the next line
    let mut model = test_model("hello world\nline two\n", 0, 2);

    // Expand: cursor → word
    update(&mut model, Msg::Editor(EditorMsg::ExpandSelection));

    // Expand: word → line
    update(&mut model, Msg::Editor(EditorMsg::ExpandSelection));

    // Selection should NOT include the newline - ends at column 11 (end of "hello world")
    let sel = model.editor().primary_selection();
    assert_eq!(sel.start().line, 0);
    assert_eq!(sel.start().column, 0);
    assert_eq!(sel.end().line, 0);
    assert_eq!(sel.end().column, 11); // "hello world" = 11 chars

    // Cursor should be at end of line 0's content
    let cursor = model.editor().primary_cursor();
    assert_eq!(cursor.line, 0, "cursor should stay on the original line");
    assert_eq!(cursor.column, 11, "cursor should be at end of line content");
}

#[test]
fn test_expand_to_line_on_last_line_cursor_at_end() {
    // On a line with multiple words, expanding to line should keep cursor at end of line
    let mut model = test_model("hello\nworld test", 1, 2);

    // Expand: cursor → word ("world")
    update(&mut model, Msg::Editor(EditorMsg::ExpandSelection));

    // Expand: word → line
    update(&mut model, Msg::Editor(EditorMsg::ExpandSelection));

    // Selection covers line 1 (end of "world test" = 10 chars)
    let sel = model.editor().primary_selection();
    assert_eq!(sel.start().line, 1);
    assert_eq!(sel.start().column, 0);
    assert_eq!(sel.end().line, 1);
    assert_eq!(sel.end().column, 10); // "world test" = 10 chars

    // Cursor should be at end of line content
    let cursor = model.editor().primary_cursor();
    assert_eq!(cursor.line, 1);
    assert_eq!(cursor.column, 10);
}

// ============================================================================
// Shrink Selection Tests
// ============================================================================

#[test]
fn test_shrink_restores_previous_selection() {
    let mut model = test_model("hello world\n", 0, 2);

    // Expand: cursor → word
    update(&mut model, Msg::Editor(EditorMsg::ExpandSelection));

    // Shrink: word → cursor
    update(&mut model, Msg::Editor(EditorMsg::ShrinkSelection));

    // Should be back to empty selection at original position
    let sel = model.editor().primary_selection();
    assert!(sel.is_empty());
    assert_eq!(model.editor().primary_cursor().line, 0);
    assert_eq!(model.editor().primary_cursor().column, 2);
}

#[test]
fn test_shrink_from_word_to_cursor() {
    let mut model = test_model("hello world\n", 0, 2);

    // Expand to word
    update(&mut model, Msg::Editor(EditorMsg::ExpandSelection));

    // Shrink to cursor
    update(&mut model, Msg::Editor(EditorMsg::ShrinkSelection));

    let sel = model.editor().primary_selection();
    assert!(sel.is_empty());
}

#[test]
fn test_shrink_with_empty_history_clears_selection() {
    let mut model = test_model("hello world\n", 0, 0);

    // Manually set a selection (simulating user made selection some other way)
    model.editor_mut().primary_selection_mut().anchor = token::model::Position::new(0, 0);
    model.editor_mut().primary_selection_mut().head = token::model::Position::new(0, 5);
    model.editor_mut().primary_cursor_mut().column = 5;

    // History is empty, so shrink should collapse to cursor
    update(&mut model, Msg::Editor(EditorMsg::ShrinkSelection));

    let sel = model.editor().primary_selection();
    assert!(sel.is_empty());
}

#[test]
fn test_expand_then_shrink_round_trip() {
    let mut model = test_model("hello world\nline two\n", 0, 2);

    // Expand three times: cursor → word → line → all
    update(&mut model, Msg::Editor(EditorMsg::ExpandSelection));
    update(&mut model, Msg::Editor(EditorMsg::ExpandSelection));
    update(&mut model, Msg::Editor(EditorMsg::ExpandSelection));

    // Shrink three times: all → line → word → cursor
    update(&mut model, Msg::Editor(EditorMsg::ShrinkSelection));
    update(&mut model, Msg::Editor(EditorMsg::ShrinkSelection));
    update(&mut model, Msg::Editor(EditorMsg::ShrinkSelection));

    // Should be back to original cursor position with no selection
    let sel = model.editor().primary_selection();
    assert!(sel.is_empty());
    assert_eq!(model.editor().primary_cursor().line, 0);
    assert_eq!(model.editor().primary_cursor().column, 2);
}

// ============================================================================
// History Management Tests
// ============================================================================

#[test]
fn test_history_cleared_on_cursor_movement() {
    use token::messages::Direction;

    let mut model = test_model("hello world\n", 0, 2);

    // Expand to word
    update(&mut model, Msg::Editor(EditorMsg::ExpandSelection));

    // Move cursor (clears history)
    update(
        &mut model,
        Msg::Editor(EditorMsg::MoveCursor(Direction::Right)),
    );

    // Shrink should now just clear selection (no history to restore)
    update(&mut model, Msg::Editor(EditorMsg::ShrinkSelection));

    // Selection should be empty, cursor at moved position
    assert!(model.editor().primary_selection().is_empty());
}

#[test]
fn test_history_cleared_on_select_all() {
    let mut model = test_model("hello world\n", 0, 2);

    // Expand to word
    update(&mut model, Msg::Editor(EditorMsg::ExpandSelection));
    assert!(!model.editor().selection_history.is_empty());

    // SelectAll (clears expand/shrink history)
    update(&mut model, Msg::Editor(EditorMsg::SelectAll));

    // History should be cleared
    assert!(model.editor().selection_history.is_empty());
}

#[test]
fn test_history_preserved_during_expand_shrink_sequence() {
    let mut model = test_model("hello world\n", 0, 2);

    // Expand twice
    update(&mut model, Msg::Editor(EditorMsg::ExpandSelection));
    update(&mut model, Msg::Editor(EditorMsg::ExpandSelection));

    // History should have 2 entries (original cursor, word selection)
    assert_eq!(model.editor().selection_history.len(), 2);

    // Shrink once
    update(&mut model, Msg::Editor(EditorMsg::ShrinkSelection));

    // History should have 1 entry
    assert_eq!(model.editor().selection_history.len(), 1);
}

// ============================================================================
// Edge Cases
// ============================================================================

#[test]
fn test_expand_at_word_boundary() {
    // Cursor at exact end of word (before space)
    let mut model = test_model("hello world\n", 0, 5);

    update(&mut model, Msg::Editor(EditorMsg::ExpandSelection));

    // At position 5 (the space), there's no word, so should expand to line
    let sel = model.editor().primary_selection();
    assert!(!sel.is_empty());
}

#[test]
fn test_expand_on_single_char_word() {
    let mut model = test_model("a b c\n", 0, 0);

    update(&mut model, Msg::Editor(EditorMsg::ExpandSelection));

    // Should select just "a"
    let sel = model.editor().primary_selection();
    assert_eq!(sel.start().column, 0);
    assert_eq!(sel.end().column, 1);
}

#[test]
fn test_expand_on_underscore_word() {
    // Underscore should be treated as word char
    let mut model = test_model("hello_world test\n", 0, 3);

    update(&mut model, Msg::Editor(EditorMsg::ExpandSelection));

    // Should select "hello_world" (columns 0-11)
    let sel = model.editor().primary_selection();
    assert_eq!(sel.start().column, 0);
    assert_eq!(sel.end().column, 11);
}

#[test]
fn test_expand_empty_document() {
    let mut model = test_model("", 0, 0);

    update(&mut model, Msg::Editor(EditorMsg::ExpandSelection));

    // Should handle gracefully - selection at (0,0)
    let sel = model.editor().primary_selection();
    assert_eq!(sel.start().line, 0);
    assert_eq!(sel.start().column, 0);
}

#[test]
fn test_shrink_empty_history_no_crash() {
    let mut model = test_model("hello\n", 0, 0);

    // Shrink with no history should not crash
    update(&mut model, Msg::Editor(EditorMsg::ShrinkSelection));
    update(&mut model, Msg::Editor(EditorMsg::ShrinkSelection));
    update(&mut model, Msg::Editor(EditorMsg::ShrinkSelection));

    // Should just have empty selection at cursor
    assert!(model.editor().primary_selection().is_empty());
}

// ============================================================================
// Multi-Cursor Expand Selection Tests
// ============================================================================

#[test]
fn test_expand_multi_cursor_selects_words() {
    // Two cursors on different lines, both inside words
    // Line 0: "THIS IS A LINE OF EXAMPLE TEXT"
    // Line 1: "some arbitrary example text here"
    let mut model = test_model(
        "THIS IS A LINE OF EXAMPLE TEXT\nsome arbitrary example text here\n",
        0,
        20, // cursor in "EXAMPLE"
    );

    // Add second cursor on line 1 in "example"
    model.editor_mut().add_cursor_at(1, 17); // in "example"

    assert_eq!(model.editor().cursor_count(), 2);

    // Expand selection on all cursors
    update(&mut model, Msg::Editor(EditorMsg::ExpandSelection));

    // Both cursors should now have word selections
    assert_eq!(model.editor().cursor_count(), 2);

    // First cursor should have "EXAMPLE" selected (columns 18-25)
    let sel0 = &model.editor().selections[0];
    assert!(!sel0.is_empty(), "First selection should not be empty");
    assert_eq!(sel0.start().line, 0);

    // Second cursor should have "example" selected
    let sel1 = &model.editor().selections[1];
    assert!(!sel1.is_empty(), "Second selection should not be empty");
    assert_eq!(sel1.start().line, 1);
}

#[test]
fn test_expand_multi_cursor_word_to_line() {
    let mut model = test_model("hello world\nfoo bar\n", 0, 2);

    // Add second cursor on line 1
    model.editor_mut().add_cursor_at(1, 1);

    assert_eq!(model.editor().cursor_count(), 2);

    // First expand: cursor → word
    update(&mut model, Msg::Editor(EditorMsg::ExpandSelection));

    // Check both have word selections
    assert!(!model.editor().selections[0].is_empty());
    assert!(!model.editor().selections[1].is_empty());

    // Second expand: word → line
    update(&mut model, Msg::Editor(EditorMsg::ExpandSelection));

    // First cursor's selection should cover line 0
    let sel0 = &model.editor().selections[0];
    assert_eq!(
        sel0.start().column,
        0,
        "Line selection should start at column 0"
    );

    // Second cursor's selection should cover line 1
    let sel1 = &model.editor().selections[1];
    assert_eq!(sel1.start().line, 1);
    assert_eq!(
        sel1.start().column,
        0,
        "Line selection should start at column 0"
    );
}

#[test]
fn test_multi_cursor_shrink_restores_complete_snapshot() {
    let mut model = test_model("hello world\nfoo bar\n", 0, 2);
    model.editor_mut().add_cursor_at(1, 1);
    let original_cursors = model.editor().cursors.clone();
    let original_selections = model.editor().selections.clone();
    let original_active = model.editor().active_cursor_index;

    update(&mut model, Msg::Editor(EditorMsg::ExpandSelection));
    update(&mut model, Msg::Editor(EditorMsg::ShrinkSelection));

    assert_eq!(model.editor().cursors.len(), original_cursors.len());
    assert_eq!(model.editor().selections, original_selections);
    assert_eq!(model.editor().active_cursor_index, original_active);
    for (actual, expected) in model.editor().cursors.iter().zip(original_cursors) {
        assert_eq!(
            (actual.line, actual.column),
            (expected.line, expected.column)
        );
    }
}

#[test]
fn test_document_edit_invalidates_selection_history() {
    let mut model = test_model("hello world\n", 0, 2);
    update(&mut model, Msg::Editor(EditorMsg::ExpandSelection));
    assert!(!model.editor().selection_history.is_empty());

    update(&mut model, Msg::Document(DocumentMsg::InsertChar('!')));
    assert!(model.editor().selection_history.is_empty());

    let cursor_after_edit = *model.editor().primary_cursor();
    update(&mut model, Msg::Editor(EditorMsg::ShrinkSelection));
    let cursor_after_shrink = model.editor().primary_cursor();
    assert_eq!(
        (cursor_after_shrink.line, cursor_after_shrink.column),
        (cursor_after_edit.line, cursor_after_edit.column)
    );
}

#[test]
fn test_multicursor_change_invalidates_selection_history() {
    let mut model = test_model("hello\nworld\n", 0, 2);
    update(&mut model, Msg::Editor(EditorMsg::ExpandSelection));
    assert!(!model.editor().selection_history.is_empty());

    update(
        &mut model,
        Msg::Editor(EditorMsg::ToggleCursorAtPosition { line: 1, column: 2 }),
    );
    assert!(model.editor().selection_history.is_empty());
}

#[test]
fn test_expand_multi_cursor_line_collapses_to_select_all() {
    let mut model = test_model("hello\nworld\n", 0, 2);

    // Add second cursor on line 1
    model.editor_mut().add_cursor_at(1, 2);

    assert_eq!(model.editor().cursor_count(), 2);

    // Expand three times: cursor → word → line → all
    update(&mut model, Msg::Editor(EditorMsg::ExpandSelection));
    update(&mut model, Msg::Editor(EditorMsg::ExpandSelection));
    update(&mut model, Msg::Editor(EditorMsg::ExpandSelection));

    // Should collapse to single cursor with full document selection
    assert_eq!(
        model.editor().cursor_count(),
        1,
        "Should collapse to single cursor on select all"
    );

    let sel = model.editor().primary_selection();
    assert_eq!(sel.start().line, 0);
    assert_eq!(sel.start().column, 0);
}
