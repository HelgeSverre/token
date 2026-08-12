//! Editor (line, char-column) <-> LSP (line, UTF-16 column) conversion.
//!
//! Always UTF-16 on the LSP side — the reviewed decision recorded in
//! docs/feature/lsp-integration.md: two of the three target servers
//! don't support the 3.17 UTF-8 `positionEncoding` negotiation, so a
//! second code path would double the test matrix for nothing
//! measurable. Every helper here is document-parameterized rather than
//! reading global editor state, matching the pattern the expand-selection
//! byte<->char helpers use in `syntax/selection.rs`/`syntax/parser.rs`
//! (those are private to that module; this is new code, not an
//! extension of them).

use crate::model::document::Document;
use crate::model::editor::Position;

/// Converts an editor position (0-based line, char column) to an LSP
/// position (0-based line, UTF-16 code-unit column).
///
/// Clamps the column to the line's length and the line to the
/// document's line count, mirroring `Document::cursor_to_offset`'s
/// defensive clamping — callers may hand in a position derived from a
/// stale snapshot.
pub fn position_to_lsp(document: &Document, position: Position) -> lsp_types::Position {
    let line = position.line.min(document.line_count().saturating_sub(1));
    let char_col = position.column.min(document.line_length(line));
    let utf16_col = char_col_to_utf16(document, line, char_col);
    lsp_types::Position {
        line: line as u32,
        character: utf16_col as u32,
    }
}

/// Whether a diagnostic range's start line no longer exists in the
/// document (an edit deleted the line after the server computed the
/// range). `lsp_to_position` clamps such a line onto the last line
/// instead of erroring, which is right for cursor/selection math but
/// wrong for diagnostic rendering — per lsp-integration.md, "a vanished
/// line's diagnostic is skipped, never a panic" and never a false mark
/// on whatever line happens to clamp onto. Callers check this *before*
/// calling `lsp_to_position` and skip the diagnostic outright.
pub fn range_vanished(document: &Document, range: lsp_types::Range) -> bool {
    range.start.line as usize >= document.line_count()
}

/// Converts an LSP position (UTF-16 columns) to an editor position
/// (char columns), clamped into the document.
pub fn lsp_to_position(document: &Document, lsp_position: lsp_types::Position) -> Position {
    let line = (lsp_position.line as usize).min(document.line_count().saturating_sub(1));
    let char_col = utf16_col_to_char(document, line, lsp_position.character as usize);
    Position::new(line, char_col)
}

/// Number of UTF-16 code units the first `char_col` characters of
/// `line` occupy.
fn char_col_to_utf16(document: &Document, line: usize, char_col: usize) -> usize {
    let Some(line_slice) = document.get_line_slice(line) else {
        return 0;
    };
    let clamped = char_col.min(document.line_length(line));
    line_slice.chars().take(clamped).map(char::len_utf16).sum()
}

/// The char index whose UTF-16 offset within `line` is `utf16_col`.
///
/// A `utf16_col` that lands inside a surrogate pair (a position the
/// spec says clients shouldn't send, but servers are not required to
/// validate) snaps forward to the character boundary after it, rather
/// than panicking or truncating a multi-unit character.
fn utf16_col_to_char(document: &Document, line: usize, utf16_col: usize) -> usize {
    let Some(line_slice) = document.get_line_slice(line) else {
        return 0;
    };
    let line_len = document.line_length(line);
    let mut utf16_count = 0;
    for (char_idx, ch) in line_slice.chars().take(line_len).enumerate() {
        if utf16_count >= utf16_col {
            return char_idx;
        }
        utf16_count += ch.len_utf16();
    }
    line_len
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(text: &str) -> Document {
        Document::with_text(text)
    }

    fn lsp(line: u32, character: u32) -> lsp_types::Position {
        lsp_types::Position { line, character }
    }

    #[test]
    fn ascii_round_trips() {
        let document = doc("hello world\nsecond line\n");
        let pos = Position::new(1, 6);
        let converted = position_to_lsp(&document, pos);
        assert_eq!(converted, lsp(1, 6));
        assert_eq!(lsp_to_position(&document, converted), pos);
    }

    #[test]
    fn bmp_multibyte_characters_count_as_one_utf16_unit() {
        // "héllo": é is 2 UTF-8 bytes but 1 char and 1 UTF-16 unit.
        let document = doc("héllo\n");
        // Column 3 = char index 3 = "l" right after "hél".
        let converted = position_to_lsp(&document, Position::new(0, 3));
        assert_eq!(converted, lsp(0, 3));
        assert_eq!(lsp_to_position(&document, converted), Position::new(0, 3));
    }

    #[test]
    fn astral_emoji_counts_as_two_utf16_units() {
        // "a🦀b": 'a' (1 char), crab (1 char, 2 UTF-16 units), 'b' (1 char).
        let document = doc("a🦀b\n");

        // Char column 2 is right after the crab -> UTF-16 column 3 (1 + 2).
        let converted = position_to_lsp(&document, Position::new(0, 2));
        assert_eq!(converted, lsp(0, 3));
        assert_eq!(lsp_to_position(&document, converted), Position::new(0, 2));

        // Char column 1 (between 'a' and the crab) -> UTF-16 column 1.
        assert_eq!(position_to_lsp(&document, Position::new(0, 1)), lsp(0, 1));

        // A UTF-16 column that lands inside the surrogate pair (2) snaps
        // forward to the next char boundary (after the crab, char col 2).
        assert_eq!(lsp_to_position(&document, lsp(0, 2)), Position::new(0, 2));
    }

    #[test]
    fn line_end_position_is_the_line_length_not_the_line_ending() {
        let document = doc("abc\ndef\n");
        // "abc" has char length 3; the position at the end of the line
        // (before the \n) must round-trip, not overshoot into the newline.
        let end = position_to_lsp(&document, Position::new(0, 3));
        assert_eq!(end, lsp(0, 3));
        assert_eq!(lsp_to_position(&document, end), Position::new(0, 3));
    }

    #[test]
    fn crlf_line_length_excludes_the_carriage_return() {
        let document = doc("abc\r\ndef\r\n");
        // Column 3 is the end of "abc", same as the LF case above — \r\n
        // must not leak into either the char or UTF-16 column space.
        let end = position_to_lsp(&document, Position::new(0, 3));
        assert_eq!(end, lsp(0, 3));
        assert_eq!(lsp_to_position(&document, end), Position::new(0, 3));
    }

    #[test]
    fn crlf_with_multibyte_content_converts_correctly() {
        let document = doc("héllo\r\nworld\r\n");
        let end_of_first_line = position_to_lsp(&document, Position::new(0, 5));
        assert_eq!(end_of_first_line, lsp(0, 5));
        assert_eq!(
            lsp_to_position(&document, end_of_first_line),
            Position::new(0, 5)
        );
    }

    #[test]
    fn out_of_range_column_clamps_to_line_length() {
        let document = doc("abc\n");
        let converted = position_to_lsp(&document, Position::new(0, 999));
        assert_eq!(converted, lsp(0, 3));
    }

    #[test]
    fn out_of_range_line_clamps_to_last_line() {
        let document = doc("abc\ndef\n");
        let converted = position_to_lsp(&document, Position::new(50, 0));
        // A trailing "\n" leaves ropey with a final empty line, so the
        // last valid line index is 2 ("", after "abc" and "def").
        assert_eq!(converted.line, 2);
    }

    #[test]
    fn lsp_utf16_column_beyond_line_end_clamps_to_line_length() {
        let document = doc("abc\n");
        let position = lsp_to_position(&document, lsp(0, 999));
        assert_eq!(position, Position::new(0, 3));
    }

    #[test]
    fn range_vanished_detects_a_start_line_past_the_document() {
        let document = doc("a\nb\n");
        let range = lsp_types::Range::new(lsp(9, 0), lsp(9, 3));
        assert!(range_vanished(&document, range));
    }

    #[test]
    fn range_vanished_is_false_for_an_in_bounds_line() {
        let document = doc("a\nb\n");
        let range = lsp_types::Range::new(lsp(1, 0), lsp(1, 1));
        assert!(!range_vanished(&document, range));
    }
}
