//! Utility functions for text editing

/// Borrowed lines retaining LF, CRLF, or CR endings without a trailing empty
/// item. Unlike `str::lines`, this also recognizes CR-only documents.
pub fn lines_with_endings(mut text: &str) -> impl Iterator<Item = &str> {
    std::iter::from_fn(move || {
        if text.is_empty() {
            return None;
        }
        let end = text.find(['\r', '\n']).map_or(text.len(), |index| {
            index
                + if text[index..].starts_with("\r\n") {
                    2
                } else {
                    1
                }
        });
        let (line, rest) = text.split_at(end);
        text = rest;
        Some(line)
    })
}

/// Remove one LF, CRLF, or CR ending. Other Unicode separators are unchanged.
pub fn trim_line_ending(text: &str) -> &str {
    text.strip_suffix("\r\n")
        .or_else(|| text.strip_suffix(['\r', '\n']))
        .unwrap_or(text)
}

pub fn line_ending_chars(line: ropey::RopeSlice<'_>) -> usize {
    let len = line.len_chars();
    match len.checked_sub(1).map(|i| line.char(i)) {
        Some('\n') if len > 1 && line.char(len - 2) == '\r' => 2,
        Some('\r' | '\n') => 1,
        _ => 0,
    }
}

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

/// Validated display tab stops. Character and byte offsets never use this width.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TabStops(usize);

impl Default for TabStops {
    fn default() -> Self {
        Self(TABULATOR_WIDTH)
    }
}

impl TabStops {
    pub fn new(width: usize) -> Self {
        Self(width.clamp(1, 256))
    }

    pub fn width(self) -> usize {
        self.0
    }

    pub fn advance(self, column: usize) -> usize {
        self.0 - column % self.0
    }

    pub fn visual_width(self, chars: impl IntoIterator<Item = char>) -> usize {
        chars.into_iter().fold(0, |column, ch| {
            column + if ch == '\t' { self.advance(column) } else { 1 }
        })
    }

    pub fn char_col_to_visual_col(self, text: &str, column: usize) -> usize {
        self.char_col_to_visual_col_from(text, 0, column)
    }

    pub fn char_col_to_visual_col_from(self, text: &str, start: usize, column: usize) -> usize {
        self.visual_width(text.chars().skip(start).take(column.saturating_sub(start)))
    }

    pub fn visual_col_to_char_col_from(
        self,
        text: &str,
        start: usize,
        end: usize,
        column: usize,
    ) -> usize {
        let mut visual = 0;
        let mut result = start;
        for ch in text.chars().skip(start).take(end.saturating_sub(start)) {
            if visual >= column {
                break;
            }
            visual += if ch == '\t' { self.advance(visual) } else { 1 };
            result += 1;
        }
        result
    }

    /// Expand tabs lazily so rendering can stop at the viewport's right edge.
    pub fn expanded_chars(
        self,
        chars: impl IntoIterator<Item = char>,
    ) -> impl Iterator<Item = char> {
        let mut column = 0;
        chars.into_iter().flat_map(move |ch| {
            let count = if ch == '\t' { self.advance(column) } else { 1 };
            column += count;
            std::iter::repeat_n(if ch == '\t' { ' ' } else { ch }, count)
        })
    }

    pub fn expand(self, text: &str) -> std::borrow::Cow<'_, str> {
        if !text.contains('\t') {
            return text.into();
        }
        self.expanded_chars(text.chars()).collect::<String>().into()
    }
}

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
    TabStops::default().char_col_to_visual_col_from(text, start_col, char_col)
}

/// Width of a character stream using the editor's tab stops.
pub fn visual_width(chars: impl IntoIterator<Item = char>) -> usize {
    TabStops::default().visual_width(chars)
}

/// Convert a segment-local visual column back to an absolute character column.
pub fn visual_col_to_char_col_from(
    text: &str,
    start_col: usize,
    end_col: usize,
    visual_col: usize,
) -> usize {
    TabStops::default().visual_col_to_char_col_from(text, start_col, end_col, visual_col)
}
