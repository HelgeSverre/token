//! Tests for Document cursor/offset conversion functions
//!
//! These functions are critical for performance - they should be O(log n) not O(n).

mod common;

use token::model::Document;

// ============================================================================
// cursor_to_offset tests
// ============================================================================

#[test]
fn cursor_to_offset_first_line_first_char() {
    let doc = Document::with_text("hello\nworld\n");
    assert_eq!(doc.cursor_to_offset(0, 0), 0);
}

#[test]
fn cursor_to_offset_first_line_middle() {
    let doc = Document::with_text("hello\nworld\n");
    assert_eq!(doc.cursor_to_offset(0, 3), 3);
}

#[test]
fn cursor_to_offset_first_line_end() {
    let doc = Document::with_text("hello\nworld\n");
    // "hello\n" is 6 chars, so end of content is column 5
    assert_eq!(doc.cursor_to_offset(0, 5), 5);
}

#[test]
fn cursor_to_offset_second_line_start() {
    let doc = Document::with_text("hello\nworld\n");
    // "hello\n" is 6 chars, second line starts at offset 6
    assert_eq!(doc.cursor_to_offset(1, 0), 6);
}

#[test]
fn cursor_to_offset_second_line_middle() {
    let doc = Document::with_text("hello\nworld\n");
    // offset 6 + column 3 = 9
    assert_eq!(doc.cursor_to_offset(1, 3), 9);
}

#[test]
fn cursor_to_offset_last_line() {
    let doc = Document::with_text("line1\nline2\nline3");
    // "line1\n" = 6, "line2\n" = 6, third line starts at 12
    assert_eq!(doc.cursor_to_offset(2, 0), 12);
    assert_eq!(doc.cursor_to_offset(2, 5), 17);
}

#[test]
fn cursor_to_offset_clamps_column_beyond_line_length() {
    let doc = Document::with_text("hi\nworld\n");
    // Line 0 is "hi" (2 chars), column 100 should clamp to 2
    assert_eq!(doc.cursor_to_offset(0, 100), 2);
}

#[test]
fn cursor_to_offset_empty_document() {
    let doc = Document::with_text("");
    assert_eq!(doc.cursor_to_offset(0, 0), 0);
}

#[test]
fn cursor_to_offset_single_newline() {
    let doc = Document::with_text("\n");
    assert_eq!(doc.cursor_to_offset(0, 0), 0);
    assert_eq!(doc.cursor_to_offset(1, 0), 1);
}

// ============================================================================
// offset_to_cursor tests
// ============================================================================

#[test]
fn offset_to_cursor_zero() {
    let doc = Document::with_text("hello\nworld\n");
    assert_eq!(doc.offset_to_cursor(0), (0, 0));
}

#[test]
fn offset_to_cursor_middle_first_line() {
    let doc = Document::with_text("hello\nworld\n");
    assert_eq!(doc.offset_to_cursor(3), (0, 3));
}

#[test]
fn offset_to_cursor_end_first_line() {
    let doc = Document::with_text("hello\nworld\n");
    // offset 5 = last char of "hello" before newline
    assert_eq!(doc.offset_to_cursor(5), (0, 5));
}

#[test]
fn offset_to_cursor_start_second_line() {
    let doc = Document::with_text("hello\nworld\n");
    // offset 6 = first char of "world"
    assert_eq!(doc.offset_to_cursor(6), (1, 0));
}

#[test]
fn offset_to_cursor_middle_second_line() {
    let doc = Document::with_text("hello\nworld\n");
    assert_eq!(doc.offset_to_cursor(9), (1, 3));
}

#[test]
fn offset_to_cursor_third_line() {
    let doc = Document::with_text("line1\nline2\nline3");
    // "line1\n" = 6, "line2\n" = 6, offset 12 = start of line3
    assert_eq!(doc.offset_to_cursor(12), (2, 0));
    assert_eq!(doc.offset_to_cursor(15), (2, 3));
}

#[test]
fn offset_to_cursor_past_end_returns_document_end() {
    let doc = Document::with_text("hello\nworld");
    // Total chars: 11. Offset 100 should return end of document
    let (line, col) = doc.offset_to_cursor(100);
    assert_eq!(line, 1);
    assert_eq!(col, 5); // "world" has 5 chars
}

#[test]
fn offset_to_cursor_empty_document() {
    let doc = Document::with_text("");
    assert_eq!(doc.offset_to_cursor(0), (0, 0));
}

#[test]
fn offset_to_cursor_single_newline() {
    let doc = Document::with_text("\n");
    assert_eq!(doc.offset_to_cursor(0), (0, 0));
    assert_eq!(doc.offset_to_cursor(1), (1, 0));
}

// ============================================================================
// Round-trip tests (cursor -> offset -> cursor)
// ============================================================================

#[test]
fn roundtrip_cursor_to_offset_to_cursor() {
    let doc = Document::with_text("first line\nsecond line\nthird line\n");

    let positions = [(0, 0), (0, 5), (1, 0), (1, 6), (2, 3), (2, 10)];

    for (line, col) in positions {
        let offset = doc.cursor_to_offset(line, col);
        let (result_line, result_col) = doc.offset_to_cursor(offset);
        assert_eq!(
            (result_line, result_col),
            (line, col),
            "Roundtrip failed for ({}, {}): offset={}, got ({}, {})",
            line,
            col,
            offset,
            result_line,
            result_col
        );
    }
}

#[test]
fn roundtrip_offset_to_cursor_to_offset() {
    let doc = Document::with_text("hello\nworld\ntest\n");

    for offset in 0..doc.buffer.len_chars() {
        let (line, col) = doc.offset_to_cursor(offset);
        let result_offset = doc.cursor_to_offset(line, col);
        assert_eq!(
            result_offset, offset,
            "Roundtrip failed for offset {}: got ({}, {}) -> {}",
            offset, line, col, result_offset
        );
    }
}

// ============================================================================
// Large document tests (these would be slow with O(n) implementation)
// ============================================================================

#[test]
fn cursor_to_offset_large_document_last_line() {
    let lines = 10_000;
    let content = "test line\n".repeat(lines);
    let doc = Document::with_text(&content);

    // Access last line - should be fast with O(log n)
    let offset = doc.cursor_to_offset(lines - 1, 0);
    assert_eq!(offset, (lines - 1) * 10); // Each "test line\n" is 10 chars
}

#[test]
fn offset_to_cursor_large_document_end() {
    let lines = 10_000;
    let content = "test line\n".repeat(lines);
    let doc = Document::with_text(&content);

    // Access near end - should be fast with O(log n)
    let offset = (lines - 1) * 10 + 5;
    let (line, col) = doc.offset_to_cursor(offset);
    assert_eq!(line, lines - 1);
    assert_eq!(col, 5);
}

#[test]
fn roundtrip_large_document_various_positions() {
    let lines = 10_000;
    let content = "test line\n".repeat(lines);
    let doc = Document::with_text(&content);

    // Test various positions throughout the document
    let test_lines = [0, 100, 1000, 5000, 9999];
    for &line in &test_lines {
        for col in [0, 5, 9] {
            let offset = doc.cursor_to_offset(line, col);
            let (result_line, result_col) = doc.offset_to_cursor(offset);
            assert_eq!(
                (result_line, result_col),
                (line, col),
                "Failed at line {}, col {}",
                line,
                col
            );
        }
    }
}
