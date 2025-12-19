//! Text buffer traits and implementations for the unified text editing system.
//!
//! Provides `TextBuffer` (read-only) and `TextBufferMut` (read-write) traits
//! that abstract over different buffer backends (String for small inputs, Rope for large files).

use ropey::Rope;
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

    /// Convert (line, column) to byte offset
    fn position_to_offset(&self, line: usize, column: usize) -> usize;

    /// Convert byte offset to (line, column)
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
// StringBuffer - for single-line inputs (modals, CSV cells)
// =============================================================================

/// TextBuffer implementation wrapping String. Used for single-line inputs.
#[derive(Debug, Clone, Default)]
pub struct StringBuffer {
    text: String,
}

impl StringBuffer {
    pub fn new() -> Self {
        Self {
            text: String::new(),
        }
    }

    /// Create a StringBuffer from a string slice
    pub fn from_text(s: &str) -> Self {
        Self {
            text: s.to_string(),
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
        // Single-line buffer always has exactly 1 line
        1
    }

    fn line_length(&self, line: usize) -> usize {
        if line == 0 {
            self.text.chars().count()
        } else {
            0
        }
    }

    fn len_chars(&self) -> usize {
        self.text.chars().count()
    }

    fn len_bytes(&self) -> usize {
        self.text.len()
    }

    fn char_at(&self, line: usize, column: usize) -> Option<char> {
        if line != 0 {
            return None;
        }
        self.text.chars().nth(column)
    }

    fn line(&self, line: usize) -> Option<Cow<'_, str>> {
        if line == 0 {
            Some(Cow::Borrowed(&self.text))
        } else {
            None
        }
    }

    fn position_to_offset(&self, line: usize, column: usize) -> usize {
        if line != 0 {
            return self.len_chars();
        }
        column.min(self.len_chars())
    }

    fn offset_to_position(&self, offset: usize) -> (usize, usize) {
        (0, offset.min(self.len_chars()))
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
        if line != 0 {
            return 0;
        }
        self.text.chars().take_while(|c| c.is_whitespace()).count()
    }

    fn last_non_whitespace_column(&self, line: usize) -> usize {
        if line != 0 {
            return 0;
        }
        let trimmed = self.text.trim_end();
        trimmed.chars().count()
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

// =============================================================================
// RopeBuffer - for multi-line document editing
// =============================================================================

/// TextBuffer implementation wrapping ropey::Rope.
/// Used for multi-line document editing with efficient operations on large files.
#[derive(Debug, Clone)]
pub struct RopeBuffer {
    rope: Rope,
}

impl RopeBuffer {
    pub fn new() -> Self {
        Self { rope: Rope::new() }
    }

    /// Create a RopeBuffer from a string slice
    pub fn from_text(s: &str) -> Self {
        Self {
            rope: Rope::from_str(s),
        }
    }

    /// Access the underlying Rope for rope-specific operations
    pub fn rope(&self) -> &Rope {
        &self.rope
    }

    pub fn rope_mut(&mut self) -> &mut Rope {
        &mut self.rope
    }
}

impl Default for RopeBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl TextBuffer for RopeBuffer {
    fn line_count(&self) -> usize {
        self.rope.len_lines().max(1)
    }

    fn line_length(&self, line: usize) -> usize {
        if line >= self.rope.len_lines() {
            return 0;
        }
        let line_slice = self.rope.line(line);
        let len = line_slice.len_chars();
        // Exclude trailing newline if present
        if len > 0 && line_slice.char(len - 1) == '\n' {
            len - 1
        } else {
            len
        }
    }

    fn len_chars(&self) -> usize {
        self.rope.len_chars()
    }

    fn len_bytes(&self) -> usize {
        self.rope.len_bytes()
    }

    fn char_at(&self, line: usize, column: usize) -> Option<char> {
        if line >= self.rope.len_lines() {
            return None;
        }
        let line_start = self.rope.line_to_char(line);
        let line_len = self.line_length(line);
        if column >= line_len {
            return None;
        }
        Some(self.rope.char(line_start + column))
    }

    fn line(&self, line: usize) -> Option<Cow<'_, str>> {
        if line >= self.rope.len_lines() {
            return None;
        }
        let line_slice = self.rope.line(line);
        let s = line_slice.to_string();
        // Strip trailing newline
        let trimmed = s.trim_end_matches(&['\n', '\r'][..]).to_string();
        Some(Cow::Owned(trimmed))
    }

    fn position_to_offset(&self, line: usize, column: usize) -> usize {
        if line >= self.rope.len_lines() {
            return self.rope.len_chars();
        }
        let line_start = self.rope.line_to_char(line);
        line_start + column.min(self.line_length(line))
    }

    fn offset_to_position(&self, offset: usize) -> (usize, usize) {
        let clamped = offset.min(self.rope.len_chars());
        let line = self.rope.char_to_line(clamped);
        let line_start = self.rope.line_to_char(line);
        (line, clamped - line_start)
    }

    fn slice(&self, range: Range<usize>) -> String {
        let start = range.start.min(self.len_chars());
        let end = range.end.min(self.len_chars());
        if start >= end {
            return String::new();
        }
        self.rope.slice(start..end).to_string()
    }

    fn content(&self) -> String {
        self.rope.to_string()
    }

    fn first_non_whitespace_column(&self, line: usize) -> usize {
        if line >= self.rope.len_lines() {
            return 0;
        }
        let line_slice = self.rope.line(line);
        line_slice
            .chars()
            .take_while(|c| c.is_whitespace() && *c != '\n')
            .count()
    }

    fn last_non_whitespace_column(&self, line: usize) -> usize {
        if line >= self.rope.len_lines() {
            return 0;
        }
        let line_slice = self.rope.line(line);
        let s: String = line_slice.chars().collect();
        let trimmed = s.trim_end_matches(|c: char| c.is_whitespace());
        trimmed.chars().count()
    }
}

impl TextBufferMut for RopeBuffer {
    fn insert(&mut self, offset: usize, text: &str) {
        let clamped = offset.min(self.len_chars());
        self.rope.insert(clamped, text);
    }

    fn insert_char(&mut self, offset: usize, ch: char) {
        let clamped = offset.min(self.len_chars());
        self.rope.insert_char(clamped, ch);
    }

    fn remove(&mut self, range: Range<usize>) {
        let start = range.start.min(self.len_chars());
        let end = range.end.min(self.len_chars());
        if start < end {
            self.rope.remove(start..end);
        }
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
    }

    // RopeBuffer tests
    #[test]
    fn test_rope_buffer_multiline() {
        let buf = RopeBuffer::from_text("line1\nline2\nline3");
        assert_eq!(buf.line_count(), 3);
        assert_eq!(buf.line(0).unwrap().as_ref(), "line1");
        assert_eq!(buf.line(1).unwrap().as_ref(), "line2");
        assert_eq!(buf.line(2).unwrap().as_ref(), "line3");
    }

    #[test]
    fn test_rope_buffer_line_length() {
        let buf = RopeBuffer::from_text("hello\nworld");
        assert_eq!(buf.line_length(0), 5); // "hello" without newline
        assert_eq!(buf.line_length(1), 5); // "world"
    }

    #[test]
    fn test_rope_buffer_position_conversion() {
        let buf = RopeBuffer::from_text("hello\nworld");
        assert_eq!(buf.offset_to_position(0), (0, 0));
        assert_eq!(buf.offset_to_position(5), (0, 5));
        assert_eq!(buf.offset_to_position(6), (1, 0));
        assert_eq!(buf.offset_to_position(11), (1, 5));

        assert_eq!(buf.position_to_offset(0, 0), 0);
        assert_eq!(buf.position_to_offset(1, 0), 6);
        assert_eq!(buf.position_to_offset(1, 5), 11);
    }

    #[test]
    fn test_rope_buffer_insert() {
        let mut buf = RopeBuffer::from_text("hello\nworld");
        buf.insert(6, "beautiful ");
        assert_eq!(buf.content(), "hello\nbeautiful world");
    }

    #[test]
    fn test_rope_buffer_remove() {
        let mut buf = RopeBuffer::from_text("hello\nworld");
        buf.remove(5..6); // Remove newline
        assert_eq!(buf.content(), "helloworld");
    }

    #[test]
    fn test_rope_buffer_first_non_whitespace() {
        let buf = RopeBuffer::from_text("    hello\n  world");
        assert_eq!(buf.first_non_whitespace_column(0), 4);
        assert_eq!(buf.first_non_whitespace_column(1), 2);
    }

    #[test]
    fn test_buffer_clear() {
        let mut buf = StringBuffer::from_text("hello");
        buf.clear();
        assert!(buf.is_empty());
        assert_eq!(buf.content(), "");
    }
}
