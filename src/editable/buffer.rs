//! Text buffer traits and implementations for the unified text editing system.
//!
//! Provides `TextBuffer` (read-only) and `TextBufferMut` (read-write) traits
//! for small editable text fields. Document editing uses its own rope directly.

use std::borrow::Cow;
use std::ops::Range;

/// Read-only view into a text buffer for cursor navigation and rendering.
/// Abstracts over Rope (large files) and String (small inputs).
pub trait TextBuffer {
    /// Number of lines (always >= 1)
    fn line_count(&self) -> usize;

    /// Length of a specific line in characters (excluding newline)
    fn line_length(&self, line: usize) -> usize;

    /// Total length in characters
    fn len_chars(&self) -> usize;

    /// Total length in bytes
    fn len_bytes(&self) -> usize;

    /// Check if buffer is empty
    fn is_empty(&self) -> bool {
        self.len_chars() == 0
    }

    /// Get character at position, None if out of bounds
    fn char_at(&self, line: usize, column: usize) -> Option<char>;

    /// Get line content (without trailing newline)
    fn line(&self, line: usize) -> Option<Cow<'_, str>>;

    /// Convert (line, column) to character offset
    fn position_to_offset(&self, line: usize, column: usize) -> usize;

    /// Convert character offset to (line, column)
    fn offset_to_position(&self, offset: usize) -> (usize, usize);

    /// Get slice of text as String (by character indices)
    fn slice(&self, range: Range<usize>) -> String;

    /// Get full content as String (may be expensive for large buffers)
    fn content(&self) -> String;

    /// Column of first non-whitespace character on line (for smart Home)
    fn first_non_whitespace_column(&self, line: usize) -> usize;

    /// Column after last non-whitespace character on line
    fn last_non_whitespace_column(&self, line: usize) -> usize;
}

/// Mutable buffer operations. Extends TextBuffer.
pub trait TextBufferMut: TextBuffer {
    /// Insert text at character offset
    fn insert(&mut self, offset: usize, text: &str);

    /// Insert single character at character offset
    fn insert_char(&mut self, offset: usize, ch: char);

    /// Remove text in character range
    fn remove(&mut self, range: Range<usize>);

    /// Replace text in range with new text (atomic operation)
    fn replace(&mut self, range: Range<usize>, text: &str) {
        self.remove(range.clone());
        self.insert(range.start, text);
    }

    /// Clear all content
    fn clear(&mut self) {
        let len = self.len_chars();
        if len > 0 {
            self.remove(0..len);
        }
    }

    /// Set content, replacing everything
    fn set_content(&mut self, text: &str) {
        self.clear();
        self.insert(0, text);
    }
}

// =============================================================================
// StringBuffer - for small text fields (modals, CSV cells, settings)
// =============================================================================

/// A small string-backed field. Single-line mode retains literal line breaks
/// (for example in CSV cells); multiline mode exposes them to cursor navigation.
#[derive(Debug, Clone, Default)]
pub struct StringBuffer {
    text: String,
    multiline: bool,
}

impl StringBuffer {
    pub fn new() -> Self {
        Self::default()
    }

    pub(crate) fn multiline() -> Self {
        Self {
            multiline: true,
            ..Self::default()
        }
    }

    /// Create a StringBuffer from a string slice
    pub fn from_text(s: &str) -> Self {
        Self {
            text: s.to_string(),
            multiline: false,
        }
    }

    /// Access the underlying string
    pub fn as_str(&self) -> &str {
        &self.text
    }

    /// Convert char offset to byte offset
    fn char_to_byte(&self, char_offset: usize) -> usize {
        self.text
            .char_indices()
            .nth(char_offset)
            .map(|(i, _)| i)
            .unwrap_or(self.text.len())
    }
}

impl TextBuffer for StringBuffer {
    fn line_count(&self) -> usize {
        if self.multiline {
            self.text.bytes().filter(|&ch| ch == b'\n').count() + 1
        } else {
            1
        }
    }

    fn line_length(&self, line: usize) -> usize {
        self.line(line).map_or(0, |text| text.chars().count())
    }

    fn len_chars(&self) -> usize {
        self.text.chars().count()
    }

    fn len_bytes(&self) -> usize {
        self.text.len()
    }

    fn char_at(&self, line: usize, column: usize) -> Option<char> {
        self.line(line)?.chars().nth(column)
    }

    fn line(&self, line: usize) -> Option<Cow<'_, str>> {
        if self.multiline {
            self.text.split('\n').nth(line).map(Cow::Borrowed)
        } else {
            (line == 0).then_some(Cow::Borrowed(self.text.as_str()))
        }
    }

    fn position_to_offset(&self, line: usize, column: usize) -> usize {
        if !self.multiline {
            return if line == 0 {
                column.min(self.len_chars())
            } else {
                self.len_chars()
            };
        }
        let mut offset = 0;
        for (index, text) in self.text.split('\n').enumerate() {
            let length = text.chars().count();
            if index == line {
                return offset + column.min(length);
            }
            offset += length + 1;
        }
        self.len_chars()
    }

    fn offset_to_position(&self, offset: usize) -> (usize, usize) {
        if !self.multiline {
            return (0, offset.min(self.len_chars()));
        }
        self.text
            .chars()
            .take(offset)
            .fold((0, 0), |(line, column), ch| {
                if ch == '\n' {
                    (line + 1, 0)
                } else {
                    (line, column + 1)
                }
            })
    }

    fn slice(&self, range: Range<usize>) -> String {
        let start = range.start.min(self.len_chars());
        let end = range.end.min(self.len_chars());
        self.text.chars().skip(start).take(end - start).collect()
    }

    fn content(&self) -> String {
        self.text.clone()
    }

    fn first_non_whitespace_column(&self, line: usize) -> usize {
        self.line(line).map_or(0, |text| {
            text.chars().take_while(|c| c.is_whitespace()).count()
        })
    }

    fn last_non_whitespace_column(&self, line: usize) -> usize {
        self.line(line)
            .map_or(0, |text| text.trim_end().chars().count())
    }
}

impl TextBufferMut for StringBuffer {
    fn insert(&mut self, offset: usize, text: &str) {
        let byte_offset = self.char_to_byte(offset);
        self.text.insert_str(byte_offset, text);
    }

    fn insert_char(&mut self, offset: usize, ch: char) {
        let byte_offset = self.char_to_byte(offset);
        self.text.insert(byte_offset, ch);
    }

    fn remove(&mut self, range: Range<usize>) {
        let start_byte = self.char_to_byte(range.start);
        let end_byte = self.char_to_byte(range.end);
        self.text.replace_range(start_byte..end_byte, "");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // StringBuffer tests
    #[test]
    fn test_string_buffer_basic() {
        let buf = StringBuffer::from_text("hello");
        assert_eq!(buf.len_chars(), 5);
        assert_eq!(buf.len_bytes(), 5);
        assert_eq!(buf.line_count(), 1);
        assert_eq!(buf.line_length(0), 5);
    }

    #[test]
    fn test_string_buffer_utf8() {
        let buf = StringBuffer::from_text("héllo");
        assert_eq!(buf.len_chars(), 5);
        assert_eq!(buf.len_bytes(), 6); // é is 2 bytes
        assert_eq!(buf.char_at(0, 1), Some('é'));
    }

    #[test]
    fn test_string_buffer_insert() {
        let mut buf = StringBuffer::from_text("hello");
        buf.insert(5, " world");
        assert_eq!(buf.content(), "hello world");
    }

    #[test]
    fn test_string_buffer_insert_utf8() {
        let mut buf = StringBuffer::from_text("héllo");
        buf.insert(2, "X"); // After é
        assert_eq!(buf.content(), "héXllo");
    }

    #[test]
    fn test_string_buffer_remove() {
        let mut buf = StringBuffer::from_text("hello world");
        buf.remove(5..11);
        assert_eq!(buf.content(), "hello");
    }

    #[test]
    fn test_string_buffer_slice() {
        let buf = StringBuffer::from_text("hello world");
        assert_eq!(buf.slice(0..5), "hello");
        assert_eq!(buf.slice(6..11), "world");
    }

    #[test]
    fn test_string_buffer_position_conversion() {
        let buf = StringBuffer::from_text("hello");
        assert_eq!(buf.offset_to_position(3), (0, 3));
        assert_eq!(buf.position_to_offset(0, 3), 3);
        let mut multiline = StringBuffer::multiline();
        multiline.set_content("é\n xy\n");
        assert_eq!(multiline.line_count(), 3);
        assert_eq!(multiline.line_length(1), 3);
        assert_eq!(multiline.line(2).as_deref(), Some(""));
        assert_eq!(multiline.char_at(1, 1), Some('x'));
        assert_eq!(multiline.position_to_offset(1, 99), 5);
        assert_eq!(multiline.offset_to_position(99), (2, 0));
        for offset in 0..=multiline.len_chars() {
            let (line, column) = multiline.offset_to_position(offset);
            assert_eq!(multiline.position_to_offset(line, column), offset);
        }
        let literal = StringBuffer::from_text(multiline.as_str());
        assert_eq!(
            literal.line_count(),
            1,
            "CSV fields keep literal separators"
        );
        assert_eq!(literal.line_length(0), multiline.len_chars());
    }

    #[test]
    fn test_buffer_clear() {
        let mut buf = StringBuffer::from_text("hello");
        buf.clear();
        assert!(buf.is_empty());
        assert_eq!(buf.content(), "");
    }
}
