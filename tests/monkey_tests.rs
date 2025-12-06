//! Monkey tests - edge cases, fuzzing, and stress testing
//!
//! These tests intentionally push the editor to its limits with
//! weird inputs, extreme values, and unusual sequences of operations.

mod common;

use common::{buffer_to_string, test_model, test_model_with_selection};
use token::messages::{AppMsg, DocumentMsg, EditorMsg, Msg};
use token::update::update;

// ========================================================================
// Window Resize Edge Cases
// ========================================================================

#[test]
fn test_resize_to_zero_width_does_not_crash() {
    let mut model = test_model("hello world\n", 0, 0);

    // Resize to zero width - should not panic
    update(&mut model, Msg::App(AppMsg::Resize(0, 600)));

    // Model should still be usable
    assert_eq!(buffer_to_string(&model), "hello world\n");
}

#[test]
fn test_resize_to_zero_height_does_not_crash() {
    let mut model = test_model("hello world\n", 0, 0);

    // Resize to zero height - should not panic
    update(&mut model, Msg::App(AppMsg::Resize(800, 0)));

    // Model should still be usable
    assert_eq!(buffer_to_string(&model), "hello world\n");
}

#[test]
fn test_resize_to_zero_both_does_not_crash() {
    let mut model = test_model("hello world\n", 0, 0);

    // Resize to zero both - should not panic
    update(&mut model, Msg::App(AppMsg::Resize(0, 0)));

    // Model should still be usable
    assert_eq!(buffer_to_string(&model), "hello world\n");
}

#[test]
fn test_resize_to_tiny_dimensions() {
    let mut model = test_model("hello world\n", 0, 0);

    // Resize to 1x1 - should not panic
    update(&mut model, Msg::App(AppMsg::Resize(1, 1)));

    // Model should still be usable
    assert_eq!(buffer_to_string(&model), "hello world\n");
}

#[test]
fn test_rapid_resize_sequence() {
    let mut model = test_model("hello world\n", 0, 0);

    // Rapid resize sequence
    for i in 0..100 {
        let w = (i * 13) % 2000;
        let h = (i * 17) % 1500;
        update(&mut model, Msg::App(AppMsg::Resize(w, h)));
    }

    // Model should still be usable
    assert_eq!(buffer_to_string(&model), "hello world\n");
}

#[test]
fn test_resize_to_maximum_u32() {
    let mut model = test_model("hello world\n", 0, 0);

    // Resize to maximum u32 values - should not overflow
    update(&mut model, Msg::App(AppMsg::Resize(u32::MAX, u32::MAX)));

    // Model should still be usable
    assert_eq!(buffer_to_string(&model), "hello world\n");
}

#[test]
fn test_resize_very_wide_narrow() {
    let mut model = test_model("hello world\n", 0, 0);

    // Very wide but very narrow
    update(&mut model, Msg::App(AppMsg::Resize(10000, 1)));

    // Model should still be usable
    assert_eq!(buffer_to_string(&model), "hello world\n");
}

#[test]
fn test_resize_very_tall_narrow() {
    let mut model = test_model("hello world\n", 0, 0);

    // Very tall but very narrow
    update(&mut model, Msg::App(AppMsg::Resize(1, 10000)));

    // Model should still be usable
    assert_eq!(buffer_to_string(&model), "hello world\n");
}

#[test]
fn test_resize_then_cursor_movement() {
    let mut model = test_model("hello\nworld\nfoo\nbar\n", 2, 2);

    // Resize to tiny then try cursor operations
    update(&mut model, Msg::App(AppMsg::Resize(10, 10)));
    update(
        &mut model,
        Msg::Editor(EditorMsg::MoveCursor(token::messages::Direction::Down)),
    );
    update(
        &mut model,
        Msg::Editor(EditorMsg::MoveCursor(token::messages::Direction::Up)),
    );
    update(&mut model, Msg::Editor(EditorMsg::PageUp));
    update(&mut model, Msg::Editor(EditorMsg::PageDown));

    // Should not crash
    assert_eq!(buffer_to_string(&model), "hello\nworld\nfoo\nbar\n");
}

#[test]
fn test_resize_then_scrolling() {
    let text: String = (0..100).map(|i| format!("line {}\n", i)).collect();
    let mut model = test_model(&text, 50, 0);

    // Resize to various sizes and try scrolling
    for size in [0, 1, 10, 100, 500] {
        update(&mut model, Msg::App(AppMsg::Resize(size, size)));
        update(&mut model, Msg::Editor(EditorMsg::Scroll(10)));
        update(&mut model, Msg::Editor(EditorMsg::Scroll(-10)));
        update(&mut model, Msg::Editor(EditorMsg::ScrollHorizontal(5)));
        update(&mut model, Msg::Editor(EditorMsg::ScrollHorizontal(-5)));
    }

    // Should not crash
}

#[test]
fn test_resize_oscillating_zero_nonzero() {
    let mut model = test_model("hello world\n", 0, 0);

    // Oscillate between zero and normal sizes
    for _ in 0..50 {
        update(&mut model, Msg::App(AppMsg::Resize(0, 0)));
        update(&mut model, Msg::App(AppMsg::Resize(800, 600)));
        update(&mut model, Msg::App(AppMsg::Resize(0, 600)));
        update(&mut model, Msg::App(AppMsg::Resize(800, 0)));
    }

    // Model should still be usable
    assert_eq!(buffer_to_string(&model), "hello world\n");
}

#[test]
fn test_resize_with_selection_active() {
    let mut model = test_model_with_selection("hello world\nline two\n", 0, 2, 1, 5);

    // Resize with active selection
    update(&mut model, Msg::App(AppMsg::Resize(0, 0)));
    update(&mut model, Msg::App(AppMsg::Resize(1, 1)));
    update(&mut model, Msg::App(AppMsg::Resize(10000, 10000)));

    // Selection should still be valid
    assert!(!model.editor.selection().is_empty());
}

#[test]
fn test_resize_with_cursor_beyond_viewport() {
    let text: String = (0..1000).map(|i| format!("line {}\n", i)).collect();
    let mut model = test_model(&text, 500, 0);

    // Resize to tiny viewport while cursor is way below
    update(&mut model, Msg::App(AppMsg::Resize(100, 20)));

    // Cursor should still be valid
    assert_eq!(model.editor.cursor().line, 500);
}

#[test]
fn test_resize_powers_of_two() {
    let mut model = test_model("hello world\n", 0, 0);

    // Test various powers of two (edge cases for bit operations)
    for exp in 0..16 {
        let size = 1u32 << exp;
        update(&mut model, Msg::App(AppMsg::Resize(size, size)));
    }

    // Model should still be usable
    assert_eq!(buffer_to_string(&model), "hello world\n");
}

#[test]
fn test_resize_with_text_operations() {
    let mut model = test_model("hello", 0, 5);

    // Interleave resize with text operations
    for i in 0..20 {
        update(
            &mut model,
            Msg::App(AppMsg::Resize((i * 100) % 1000, (i * 50) % 500)),
        );
        update(&mut model, Msg::Document(DocumentMsg::InsertChar('x')));
        update(
            &mut model,
            Msg::App(AppMsg::Resize((i * 77) % 800, (i * 33) % 400)),
        );
        update(&mut model, Msg::Document(DocumentMsg::DeleteBackward));
    }

    // Buffer should be unchanged (insert then delete)
    assert_eq!(buffer_to_string(&model), "hello");
}

#[test]
fn test_resize_status_bar_edge() {
    let mut model = test_model("hello world\n", 0, 0);

    // Resize to exactly line_height (status bar takes all space)
    let line_height = model.line_height as u32;
    update(&mut model, Msg::App(AppMsg::Resize(800, line_height)));

    // Resize to less than line_height
    update(
        &mut model,
        Msg::App(AppMsg::Resize(800, line_height.saturating_sub(1))),
    );

    // Should not crash
    assert_eq!(buffer_to_string(&model), "hello world\n");
}

// ========================================================================
// Empty Document Edge Cases
// ========================================================================

#[test]
fn test_operations_on_empty_document() {
    let mut model = test_model("", 0, 0);

    // Various operations that might fail on empty doc
    update(
        &mut model,
        Msg::Editor(EditorMsg::MoveCursor(token::messages::Direction::Up)),
    );
    update(
        &mut model,
        Msg::Editor(EditorMsg::MoveCursor(token::messages::Direction::Down)),
    );
    update(
        &mut model,
        Msg::Editor(EditorMsg::MoveCursor(token::messages::Direction::Left)),
    );
    update(
        &mut model,
        Msg::Editor(EditorMsg::MoveCursor(token::messages::Direction::Right)),
    );
    update(&mut model, Msg::Editor(EditorMsg::MoveCursorLineStart));
    update(&mut model, Msg::Editor(EditorMsg::MoveCursorLineEnd));
    update(&mut model, Msg::Editor(EditorMsg::MoveCursorDocumentStart));
    update(&mut model, Msg::Editor(EditorMsg::MoveCursorDocumentEnd));
    update(&mut model, Msg::Editor(EditorMsg::PageUp));
    update(&mut model, Msg::Editor(EditorMsg::PageDown));
    update(&mut model, Msg::Editor(EditorMsg::SelectAll));
    update(&mut model, Msg::Editor(EditorMsg::SelectWord));
    update(&mut model, Msg::Editor(EditorMsg::SelectLine));
    update(&mut model, Msg::Document(DocumentMsg::DeleteBackward));
    update(&mut model, Msg::Document(DocumentMsg::DeleteForward));
    update(&mut model, Msg::Document(DocumentMsg::Undo));
    update(&mut model, Msg::Document(DocumentMsg::Redo));
    update(&mut model, Msg::Document(DocumentMsg::Duplicate));

    // Note: Duplicate on empty line adds a newline (duplicating the empty line)
    // This is expected behavior - we just verify it doesn't crash
}

#[test]
fn test_delete_backward_on_empty_repeatedly() {
    let mut model = test_model("", 0, 0);

    // Hammer delete on empty doc
    for _ in 0..100 {
        update(&mut model, Msg::Document(DocumentMsg::DeleteBackward));
    }

    assert_eq!(buffer_to_string(&model), "");
}

#[test]
fn test_undo_redo_on_empty_document() {
    let mut model = test_model("", 0, 0);

    // Spam undo/redo on empty doc
    for _ in 0..50 {
        update(&mut model, Msg::Document(DocumentMsg::Undo));
        update(&mut model, Msg::Document(DocumentMsg::Redo));
    }

    assert_eq!(buffer_to_string(&model), "");
}

// ========================================================================
// Extreme Cursor Positions
// ========================================================================

#[test]
fn test_cursor_at_extreme_positions() {
    let mut model = test_model("short\n", 0, 0);

    // Try to set cursor way beyond line length
    model.editor.cursor_mut().column = 999999;
    update(&mut model, Msg::Document(DocumentMsg::InsertChar('X')));

    // Should handle gracefully (insert at clamped position)
    // The cursor_to_offset clamps the column
    assert!(buffer_to_string(&model).contains('X'));
}

#[test]
fn test_cursor_at_extreme_line() {
    let mut model = test_model("line1\nline2\n", 0, 0);

    // Try to set cursor way beyond document
    model.editor.cursor_mut().line = 999999;
    model.editor.cursor_mut().column = 0;

    // Operations should handle gracefully
    update(
        &mut model,
        Msg::Editor(EditorMsg::MoveCursor(token::messages::Direction::Down)),
    );
    update(
        &mut model,
        Msg::Editor(EditorMsg::MoveCursor(token::messages::Direction::Up)),
    );

    // Should not crash
    assert_eq!(buffer_to_string(&model), "line1\nline2\n");
}

// ========================================================================
// Selection Edge Cases
// ========================================================================

#[test]
fn test_selection_with_inverted_anchor_head() {
    // Anchor after head (backwards selection)
    let mut model = test_model_with_selection("hello world", 0, 8, 0, 2);

    // Operations should work with backwards selection
    update(&mut model, Msg::Document(DocumentMsg::InsertChar('X')));

    // Should replace selection correctly
    assert!(buffer_to_string(&model).len() < 11); // Text was replaced
}

#[test]
fn test_selection_spanning_beyond_document() {
    let mut model = test_model("hi", 0, 0);

    // Manually set selection beyond document bounds
    model.editor.selection_mut().anchor = token::model::Position::new(0, 0);
    model.editor.selection_mut().head = token::model::Position::new(999, 999);

    // Delete selection - should handle gracefully
    update(&mut model, Msg::Document(DocumentMsg::DeleteBackward));

    // Should not crash
}

// ========================================================================
// Rapid Operation Sequences
// ========================================================================

#[test]
fn test_rapid_insert_delete_cycle() {
    let mut model = test_model("", 0, 0);

    for i in 0..100 {
        update(&mut model, Msg::Document(DocumentMsg::InsertChar('a')));
        if i % 3 == 0 {
            update(&mut model, Msg::Document(DocumentMsg::DeleteBackward));
        }
    }

    // Should not crash
    assert!(buffer_to_string(&model).len() > 0);
}

#[test]
fn test_rapid_undo_during_typing() {
    let mut model = test_model("", 0, 0);

    for i in 0..50 {
        update(
            &mut model,
            Msg::Document(DocumentMsg::InsertChar(
                ('a' as u8 + (i % 26) as u8) as char,
            )),
        );
        if i % 2 == 0 {
            update(&mut model, Msg::Document(DocumentMsg::Undo));
        }
    }

    // Should not crash
}

#[test]
fn test_alternating_cursor_movements() {
    let mut model = test_model("line1\nline2\nline3\nline4\nline5\n", 2, 2);

    for _ in 0..100 {
        update(
            &mut model,
            Msg::Editor(EditorMsg::MoveCursor(token::messages::Direction::Up)),
        );
        update(
            &mut model,
            Msg::Editor(EditorMsg::MoveCursor(token::messages::Direction::Down)),
        );
        update(
            &mut model,
            Msg::Editor(EditorMsg::MoveCursor(token::messages::Direction::Left)),
        );
        update(
            &mut model,
            Msg::Editor(EditorMsg::MoveCursor(token::messages::Direction::Right)),
        );
    }

    // Should not crash
    assert_eq!(
        buffer_to_string(&model),
        "line1\nline2\nline3\nline4\nline5\n"
    );
}

// ========================================================================
// Large Document Stress Tests
// ========================================================================

#[test]
fn test_operations_on_large_document() {
    // Create a 10,000 line document
    let text: String = (0..10000).map(|i| format!("line {}\n", i)).collect();
    let mut model = test_model(&text, 5000, 3);

    // Various operations
    update(&mut model, Msg::Editor(EditorMsg::PageUp));
    update(&mut model, Msg::Editor(EditorMsg::PageDown));
    update(&mut model, Msg::Editor(EditorMsg::MoveCursorDocumentStart));
    update(&mut model, Msg::Editor(EditorMsg::MoveCursorDocumentEnd));
    update(&mut model, Msg::Editor(EditorMsg::SelectAll));

    // Cursor should be at end after select all
    assert!(model.editor.cursor().line > 9000);
}

#[test]
fn test_duplicate_on_large_selection() {
    // Create a moderately large document
    let text: String = (0..100).map(|i| format!("line {}\n", i)).collect();
    let mut model = test_model(&text, 0, 0);

    // Select all and duplicate
    update(&mut model, Msg::Editor(EditorMsg::SelectAll));
    update(&mut model, Msg::Document(DocumentMsg::Duplicate));

    // Document should be roughly doubled
    assert!(model.document.line_count() > 150);
}

// ========================================================================
// Unicode and Special Characters
// ========================================================================

#[test]
fn test_operations_with_unicode() {
    let mut model = test_model("héllo wörld 🎉\n日本語テスト\n", 0, 0);

    // Move through unicode
    for _ in 0..20 {
        update(
            &mut model,
            Msg::Editor(EditorMsg::MoveCursor(token::messages::Direction::Right)),
        );
    }

    // Delete some unicode
    update(&mut model, Msg::Document(DocumentMsg::DeleteBackward));

    // Should not crash
}

#[test]
fn test_insert_special_characters() {
    let mut model = test_model("", 0, 0);

    // Insert various special chars
    let special_chars = ['\0', '\t', '\r', '\n', '🎉', '日', 'é', '\u{FEFF}'];

    for ch in special_chars {
        update(&mut model, Msg::Document(DocumentMsg::InsertChar(ch)));
    }

    // Should not crash (though output may be weird)
}

// ========================================================================
// Viewport Edge Cases
// ========================================================================

#[test]
fn test_scroll_beyond_document() {
    let mut model = test_model("short doc\n", 0, 0);

    // Try to scroll way beyond
    model.editor.viewport.top_line = 999999;

    // Operations should handle gracefully
    update(
        &mut model,
        Msg::Editor(EditorMsg::MoveCursor(token::messages::Direction::Down)),
    );

    // Should not crash
}

#[test]
fn test_visible_lines_zero() {
    let mut model = test_model("hello\nworld\n", 0, 0);

    model.editor.viewport.visible_lines = 0;

    // Page up/down with zero visible lines
    update(&mut model, Msg::Editor(EditorMsg::PageUp));
    update(&mut model, Msg::Editor(EditorMsg::PageDown));

    // Should not crash
}

// ========================================================================
// Multi-cursor Edge Cases
// ========================================================================

#[test]
fn test_add_many_cursors() {
    let mut model = test_model("line1\nline2\nline3\nline4\nline5\n", 0, 0);

    // Add many cursors
    for _ in 0..100 {
        update(&mut model, Msg::Editor(EditorMsg::AddCursorBelow));
    }

    // Type something
    update(&mut model, Msg::Document(DocumentMsg::InsertChar('X')));

    // Should not crash
}

#[test]
fn test_cursors_at_same_position() {
    let mut model = test_model("hello", 0, 2);

    // Try to add cursor at same position multiple times
    for _ in 0..10 {
        update(
            &mut model,
            Msg::Editor(EditorMsg::ToggleCursorAtPosition { line: 0, column: 2 }),
        );
    }

    // Should deduplicate and not crash
}
