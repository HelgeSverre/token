//! Utility functions for text editing

/// Check if a character is a punctuation/symbol boundary (not whitespace)
pub fn is_punctuation(ch: char) -> bool {
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
    /// Punctuation and symbols
    Punctuation,
}

/// Get the character type for word navigation
pub fn char_type(ch: char) -> CharType {
    if ch.is_whitespace() {
        CharType::Whitespace
    } else if is_punctuation(ch) {
        CharType::Punctuation
    } else {
        CharType::WordChar
    }
}

/// Check if a character is a word boundary (symbol or whitespace)
#[allow(dead_code)]
pub fn is_word_boundary(ch: char) -> bool {
    ch.is_whitespace() || is_punctuation(ch)
}

/// Tab width for visual column calculations
pub const TABULATOR_WIDTH: usize = 4;

/// Convert a visual column (screen position) to character column.
/// Accounts for tab expansion when converting screen position to character index.
pub fn visual_col_to_char_col(text: &str, visual_col: usize) -> usize {
    let mut current_visual = 0;
    let mut char_col = 0;

    for ch in text.chars() {
        if current_visual >= visual_col {
            return char_col;
        }

        if ch == '\t' {
            let tab_width = TABULATOR_WIDTH - (current_visual % TABULATOR_WIDTH);
            current_visual += tab_width;
        } else {
            current_visual += 1;
        }
        char_col += 1;
    }

    char_col
}

/// Convert a character column to visual column (screen position).
/// Accounts for tab expansion when converting character index to screen position.
pub fn char_col_to_visual_col(text: &str, char_col: usize) -> usize {
    let mut visual_col = 0;

    for (i, ch) in text.chars().enumerate() {
        if i >= char_col {
            break;
        }

        if ch == '\t' {
            let tab_width = TABULATOR_WIDTH - (visual_col % TABULATOR_WIDTH);
            visual_col += tab_width;
        } else {
            visual_col += 1;
        }
    }

    visual_col
}
