//! Utility functions for text editing

/// Check if a character is a non-whitespace symbol that separates words.
fn is_word_boundary_symbol(ch: char) -> bool {
    matches!(
        ch,
        '/' | ':'
            | ','
            | '.'
            | '-'
            | '('
            | ')'
            | '{'
            | '}'
            | '['
            | ']'
            | ';'
            | '"'
            | '\''
            | '<'
            | '>'
            | '='
            | '+'
            | '*'
            | '&'
            | '|'
            | '!'
            | '@'
            | '#'
            | '$'
            | '%'
            | '^'
            | '~'
            | '`'
            | '\\'
            | '?'
    )
}

/// Character type for word navigation (IntelliJ-style)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CharType {
    /// Whitespace characters
    Whitespace,
    /// Alphanumeric characters (word characters)
    WordChar,
    /// A non-whitespace symbol that separates words
    BoundarySymbol,
}

/// Get the character type for word navigation
pub fn char_type(ch: char) -> CharType {
    if ch.is_whitespace() {
        CharType::Whitespace
    } else if is_word_boundary_symbol(ch) {
        CharType::BoundarySymbol
    } else {
        CharType::WordChar
    }
}

/// Tab width for visual column calculations
pub const TABULATOR_WIDTH: usize = 4;

/// Convert a visual column (screen position) to character column.
/// Accounts for tab expansion when converting screen position to character index.
pub fn visual_col_to_char_col(text: &str, visual_col: usize) -> usize {
    visual_col_to_char_col_from(text, 0, usize::MAX, visual_col)
}

/// Convert a character column to visual column (screen position).
/// Accounts for tab expansion when converting character index to screen position.
pub fn char_col_to_visual_col(text: &str, char_col: usize) -> usize {
    char_col_to_visual_col_from(text, 0, char_col)
}

/// Convert an absolute character column to a visual column relative to a
/// segment that begins at `start_col`. Tab expansion restarts at the segment,
/// matching how wrapped segments are painted.
pub fn char_col_to_visual_col_from(text: &str, start_col: usize, char_col: usize) -> usize {
    visual_width(
        text.chars()
            .skip(start_col)
            .take(char_col.saturating_sub(start_col)),
    )
}

/// Width of a character stream using the editor's tab stops.
pub fn visual_width(chars: impl IntoIterator<Item = char>) -> usize {
    let mut visual_col = 0;
    for ch in chars {
        if ch == '\t' {
            visual_col += TABULATOR_WIDTH - (visual_col % TABULATOR_WIDTH);
        } else {
            visual_col += 1;
        }
    }
    visual_col
}

/// Convert a segment-local visual column back to an absolute character column.
pub fn visual_col_to_char_col_from(
    text: &str,
    start_col: usize,
    end_col: usize,
    visual_col: usize,
) -> usize {
    let mut current_visual = 0;
    let mut char_col = start_col;

    for ch in text
        .chars()
        .skip(start_col)
        .take(end_col.saturating_sub(start_col))
    {
        if current_visual >= visual_col {
            break;
        }
        if ch == '\t' {
            current_visual += TABULATOR_WIDTH - (current_visual % TABULATOR_WIDTH);
        } else {
            current_visual += 1;
        }
        char_col += 1;
    }
    char_col
}
