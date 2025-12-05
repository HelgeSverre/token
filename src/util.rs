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
            | '_'
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
