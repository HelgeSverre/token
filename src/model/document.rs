//! Document model - represents the text buffer and file state

use ropey::Rope;
use std::path::PathBuf;

use super::editor::Cursor;
use super::editor_area::DocumentId;
use crate::syntax::{LanguageId, SyntaxHighlights};

/// Represents an edit operation for undo/redo functionality
#[derive(Debug, Clone)]
pub enum EditOperation {
    Insert {
        position: usize,
        text: String,
        cursor_before: Cursor,
        cursor_after: Cursor,
    },
    Delete {
        position: usize,
        text: String,
        cursor_before: Cursor,
        cursor_after: Cursor,
    },
    /// Replace operation - used when typing over a selection to make undo atomic
    Replace {
        position: usize,
        deleted_text: String,
        inserted_text: String,
        cursor_before: Cursor,
        cursor_after: Cursor,
    },
    /// Batch operation - groups multiple edits for atomic multi-cursor undo/redo
    Batch {
        /// Individual operations (applied in order for redo, reverse order for undo)
        operations: Vec<EditOperation>,
        /// All cursor positions before the batch
        cursors_before: Vec<Cursor>,
        /// All cursor positions after the batch
        cursors_after: Vec<Cursor>,
    },
}

/// Document state - the text buffer and associated file metadata
#[derive(Debug, Clone)]
pub struct Document {
    /// Unique identifier (set when added to EditorArea)
    pub id: Option<DocumentId>,

    /// The text buffer
    pub buffer: Rope,
    /// Path to the file on disk (None for new/unsaved files)
    pub file_path: Option<PathBuf>,
    /// Display name for untitled documents (e.g., "Untitled", "Untitled-2")
    pub untitled_name: Option<String>,
    /// Whether the buffer has unsaved changes
    pub is_modified: bool,
    /// Undo stack
    pub undo_stack: Vec<EditOperation>,
    /// Redo stack
    pub redo_stack: Vec<EditOperation>,

    // === Syntax Highlighting ===
    /// Detected language for syntax highlighting
    pub language: LanguageId,
    /// Current syntax highlights (updated asynchronously)
    pub syntax_highlights: Option<SyntaxHighlights>,
    /// Document revision counter (incremented on each edit)
    /// Used for staleness checking in async parsing
    pub revision: u64,
}

impl Document {
    /// Create a new empty document
    pub fn new() -> Self {
        Self {
            id: None,
            buffer: Rope::from(""),
            file_path: None,
            untitled_name: None,
            is_modified: false,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            language: LanguageId::PlainText,
            syntax_highlights: None,
            revision: 0,
        }
    }

    /// Create a document with initial text
    pub fn with_text(text: &str) -> Self {
        Self {
            id: None,
            buffer: Rope::from(text),
            file_path: None,
            untitled_name: None,
            is_modified: false,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            language: LanguageId::PlainText,
            syntax_highlights: None,
            revision: 0,
        }
    }

    /// Load a document from a file path
    pub fn from_file(path: PathBuf) -> Result<Self, std::io::Error> {
        let content = std::fs::read_to_string(&path)?;
        let language = LanguageId::from_path(&path);
        Ok(Self {
            id: None,
            buffer: Rope::from(content),
            file_path: Some(path),
            untitled_name: None,
            is_modified: false,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            language,
            syntax_highlights: None,
            revision: 0,
        })
    }

    /// Create a new empty document with a target file path
    ///
    /// Used when the user specifies a non-existent file path on the command line.
    /// The file will be created when the user saves.
    pub fn new_with_path(path: PathBuf) -> Self {
        let language = LanguageId::from_path(&path);
        Self {
            id: None,
            buffer: Rope::from(""),
            file_path: Some(path),
            untitled_name: None,
            is_modified: true, // Mark as modified since file doesn't exist yet
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            language,
            syntax_highlights: None,
            revision: 0,
        }
    }

    /// Get the display name for this document.
    /// Returns the filename if saved, the untitled name if set, or "Untitled" as fallback.
    pub fn display_name(&self) -> String {
        if let Some(path) = &self.file_path {
            if let Some(name) = path.file_name() {
                return name.to_string_lossy().to_string();
            }
        }
        if let Some(name) = &self.untitled_name {
            return name.clone();
        }
        "Untitled".to_string()
    }

    /// Get the number of lines in the document
    pub fn line_count(&self) -> usize {
        self.buffer.len_lines()
    }

    /// Get a line by index (allocates a String)
    ///
    /// For rendering hot paths, prefer `get_line_slice()` which can avoid allocation
    /// by iterating over the rope slice directly.
    pub fn get_line(&self, line_idx: usize) -> Option<String> {
        if line_idx < self.buffer.len_lines() {
            let line = self.buffer.line(line_idx);
            Some(line.to_string())
        } else {
            None
        }
    }

    /// Get a line as a RopeSlice for zero-allocation iteration
    ///
    /// Use this in rendering hot paths to avoid String allocation.
    /// The returned slice can be iterated with `.chars()` or converted
    /// to a contiguous slice with `Cow<str>` when needed.
    #[inline]
    pub fn get_line_slice(&self, line_idx: usize) -> Option<ropey::RopeSlice<'_>> {
        if line_idx < self.buffer.len_lines() {
            Some(self.buffer.line(line_idx))
        } else {
            None
        }
    }

    /// Get line content as Cow<str>, avoiding allocation when possible
    ///
    /// Returns Cow::Borrowed if the line is stored contiguously in a single chunk,
    /// otherwise returns Cow::Owned with the line as a String.
    /// Also trims the trailing newline for display purposes.
    #[inline]
    pub fn get_line_cow(&self, line_idx: usize) -> Option<std::borrow::Cow<'_, str>> {
        use std::borrow::Cow;

        if line_idx >= self.buffer.len_lines() {
            return None;
        }

        let line = self.buffer.line(line_idx);
        let len = line.len_chars();

        // Calculate trim length (remove trailing newline)
        let trim_len = if len > 0 && line.char(len - 1) == '\n' {
            if len > 1 && line.char(len - 2) == '\r' {
                2 // CRLF
            } else {
                1 // LF
            }
        } else {
            0
        };

        let trimmed = line.slice(..len - trim_len);

        // Try to get as a contiguous slice (zero allocation)
        if let Some(s) = trimmed.as_str() {
            Some(Cow::Borrowed(s))
        } else {
            // Falls back to allocation only when line spans multiple chunks
            Some(Cow::Owned(trimmed.to_string()))
        }
    }

    /// Get the length of a line (excluding newline character)
    pub fn line_length(&self, line_idx: usize) -> usize {
        if line_idx < self.buffer.len_lines() {
            let line = self.buffer.line(line_idx);
            line.len_chars().saturating_sub(
                if line.len_chars() > 0 && line.chars().last() == Some('\n') {
                    1
                } else {
                    0
                },
            )
        } else {
            0
        }
    }

    /// Convert a (line, column) position to a buffer offset
    /// Uses ropey's O(log n) line_to_char method instead of O(n) iteration
    pub fn cursor_to_offset(&self, line: usize, column: usize) -> usize {
        if line >= self.buffer.len_lines() {
            return self.buffer.len_chars();
        }
        let line_start = self.buffer.line_to_char(line);
        line_start + column.min(self.line_length(line))
    }

    /// Convert a buffer offset to (line, column) position
    /// Uses ropey's O(log n) char_to_line method instead of O(n) iteration
    pub fn offset_to_cursor(&self, offset: usize) -> (usize, usize) {
        let clamped = offset.min(self.buffer.len_chars());
        let line = self.buffer.char_to_line(clamped);
        let line_start = self.buffer.line_to_char(line);
        (line, clamped - line_start)
    }

    /// Get the column of the first non-whitespace character on a line
    pub fn first_non_whitespace_column(&self, line_idx: usize) -> usize {
        if line_idx >= self.buffer.len_lines() {
            return 0;
        }
        let line = self.buffer.line(line_idx);
        line.chars()
            .take_while(|c| c.is_whitespace() && *c != '\n')
            .count()
    }

    /// Get the column after the last non-whitespace character on a line
    pub fn last_non_whitespace_column(&self, line_idx: usize) -> usize {
        if line_idx >= self.buffer.len_lines() {
            return 0;
        }
        let line = self.buffer.line(line_idx);
        let line_str: String = line.chars().collect();
        let trimmed = line_str.trim_end_matches(|c: char| c.is_whitespace());
        trimmed.len()
    }

    /// Push an edit operation onto the undo stack and clear redo stack
    pub fn push_edit(&mut self, op: EditOperation) {
        self.undo_stack.push(op);
        self.redo_stack.clear();
        self.is_modified = true;
        self.revision = self.revision.wrapping_add(1);
        // Keep existing syntax highlights until new ones arrive.
        // This prevents "flash of unstyled text" during the debounce window.
        // The revision check in ParseCompleted ensures only matching highlights are applied.
    }

    /// Get highlight tokens for a specific line
    pub fn get_line_highlights(&self, line: usize) -> &[crate::syntax::HighlightToken] {
        self.syntax_highlights
            .as_ref()
            .and_then(|h| h.get_line(line))
            .map(|lh| lh.tokens.as_slice())
            .unwrap_or(&[])
    }

    /// Find all occurrences of text in the document
    /// Returns Vec of (start_char_offset, end_char_offset) in character indices
    pub fn find_all_occurrences(&self, needle: &str) -> Vec<(usize, usize)> {
        self.find_all_occurrences_with_options(needle, true)
    }

    /// Find all occurrences with case sensitivity option
    /// Returns Vec of (start_char_offset, end_char_offset) in character indices
    pub fn find_all_occurrences_with_options(
        &self,
        needle: &str,
        case_sensitive: bool,
    ) -> Vec<(usize, usize)> {
        if needle.is_empty() {
            return Vec::new();
        }

        let haystack = self.buffer.to_string();
        let needle_char_len = needle.chars().count();

        // For case-insensitive search, convert both to lowercase
        let (search_haystack, search_needle) = if case_sensitive {
            (haystack.clone(), needle.to_string())
        } else {
            (haystack.to_lowercase(), needle.to_lowercase())
        };

        let needle_byte_len = search_needle.len();

        // Build byte offset → char index mapping (only for char boundaries)
        // Use original haystack for char mapping since byte positions correspond
        let byte_to_char: std::collections::HashMap<usize, usize> = haystack
            .char_indices()
            .enumerate()
            .map(|(char_idx, (byte_idx, _))| (byte_idx, char_idx))
            .collect();

        let mut results = Vec::new();
        let mut start_byte = 0;

        while start_byte <= search_haystack.len().saturating_sub(needle_byte_len) {
            if let Some(rel_byte) = search_haystack[start_byte..].find(&search_needle) {
                let match_start_byte = start_byte + rel_byte;

                // Convert byte offset to char offset
                if let Some(&start_char) = byte_to_char.get(&match_start_byte) {
                    let end_char = start_char + needle_char_len;
                    results.push((start_char, end_char));
                }

                // Advance by the byte length of the first character of the match
                // to allow overlapping matches while staying on char boundaries
                let first_char_byte_len = search_haystack[match_start_byte..]
                    .chars()
                    .next()
                    .map(|c| c.len_utf8())
                    .unwrap_or(1);
                start_byte = match_start_byte + first_char_byte_len;
            } else {
                break;
            }
        }
        results
    }

    /// Find next occurrence after given offset (wraps back to start)
    pub fn find_next_occurrence(
        &self,
        needle: &str,
        after_offset: usize,
    ) -> Option<(usize, usize)> {
        self.find_next_occurrence_with_options(needle, after_offset, true)
    }

    /// Find next occurrence with case sensitivity option (wraps back to start)
    pub fn find_next_occurrence_with_options(
        &self,
        needle: &str,
        after_offset: usize,
        case_sensitive: bool,
    ) -> Option<(usize, usize)> {
        if needle.is_empty() {
            return None;
        }

        let occurrences = self.find_all_occurrences_with_options(needle, case_sensitive);

        // Find first occurrence after current position
        if let Some(&occ) = occurrences.iter().find(|(start, _)| *start > after_offset) {
            return Some(occ);
        }

        // Wrap around to first occurrence
        occurrences.first().copied()
    }

    /// Find previous occurrence before given offset (wraps to end)
    pub fn find_prev_occurrence_with_options(
        &self,
        needle: &str,
        before_offset: usize,
        case_sensitive: bool,
    ) -> Option<(usize, usize)> {
        if needle.is_empty() {
            return None;
        }

        let occurrences = self.find_all_occurrences_with_options(needle, case_sensitive);

        // Find last occurrence before current position
        if let Some(&occ) = occurrences
            .iter()
            .rev()
            .find(|(start, _)| *start < before_offset)
        {
            return Some(occ);
        }

        // Wrap around to last occurrence
        occurrences.last().copied()
    }
}

impl Default for Document {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // Document creation tests
    // ========================================================================

    #[test]
    fn test_new_document_has_no_path() {
        let doc = Document::new();
        assert!(doc.file_path.is_none());
        assert!(!doc.is_modified);
    }

    #[test]
    fn test_new_document_empty_stacks() {
        let doc = Document::new();
        assert!(doc.undo_stack.is_empty());
        assert!(doc.redo_stack.is_empty());
    }

    #[test]
    fn test_new_document_default_language() {
        let doc = Document::new();
        assert_eq!(doc.language, LanguageId::PlainText);
    }

    #[test]
    fn test_with_text_creates_buffer() {
        let doc = Document::with_text("hello\nworld");
        assert_eq!(doc.buffer.to_string(), "hello\nworld");
        assert_eq!(doc.line_count(), 2);
    }

    // ========================================================================
    // Document::new_with_path tests
    // ========================================================================

    #[test]
    fn test_new_with_path_preserves_path() {
        let path = PathBuf::from("/tmp/newfile.rs");
        let doc = Document::new_with_path(path.clone());

        assert_eq!(doc.file_path, Some(path));
        assert!(doc.is_modified); // Should be marked modified since file doesn't exist
        assert_eq!(doc.buffer.to_string(), ""); // Empty content
    }

    #[test]
    fn test_new_with_path_detects_language() {
        let rs_doc = Document::new_with_path(PathBuf::from("test.rs"));
        assert_eq!(rs_doc.language, LanguageId::Rust);

        let py_doc = Document::new_with_path(PathBuf::from("script.py"));
        assert_eq!(py_doc.language, LanguageId::Python);

        let txt_doc = Document::new_with_path(PathBuf::from("readme.txt"));
        assert_eq!(txt_doc.language, LanguageId::PlainText);
    }

    #[test]
    fn test_new_with_path_detects_all_common_languages() {
        let test_cases = [
            ("file.js", LanguageId::JavaScript),
            ("file.ts", LanguageId::TypeScript),
            ("file.tsx", LanguageId::Tsx), // TSX is a separate language
            ("file.json", LanguageId::Json),
            ("file.yaml", LanguageId::Yaml),
            ("file.yml", LanguageId::Yaml),
            ("file.toml", LanguageId::Toml),
            ("file.md", LanguageId::Markdown),
            ("file.html", LanguageId::Html),
            ("file.css", LanguageId::Css),
            ("file.go", LanguageId::Go),
            ("file.c", LanguageId::C),
            ("file.cpp", LanguageId::Cpp),
            ("file.java", LanguageId::Java),
            ("file.sh", LanguageId::Bash),
            ("file.php", LanguageId::Php),
        ];

        for (filename, expected_lang) in test_cases {
            let doc = Document::new_with_path(PathBuf::from(filename));
            assert_eq!(
                doc.language, expected_lang,
                "Language detection failed for {}",
                filename
            );
        }
    }

    #[test]
    fn test_new_with_path_empty_stacks() {
        let doc = Document::new_with_path(PathBuf::from("/path/to/new.rs"));
        assert!(doc.undo_stack.is_empty());
        assert!(doc.redo_stack.is_empty());
    }

    #[test]
    fn test_new_with_path_no_syntax_highlights() {
        let doc = Document::new_with_path(PathBuf::from("/path/to/new.rs"));
        assert!(doc.syntax_highlights.is_none());
    }

    #[test]
    fn test_new_with_path_zero_revision() {
        let doc = Document::new_with_path(PathBuf::from("/path/to/new.rs"));
        assert_eq!(doc.revision, 0);
    }

    #[test]
    fn test_new_with_path_no_id() {
        let doc = Document::new_with_path(PathBuf::from("/path/to/new.rs"));
        assert!(doc.id.is_none());
    }

    #[test]
    fn test_new_with_path_absolute_path() {
        let path = PathBuf::from("/home/user/projects/myapp/src/main.rs");
        let doc = Document::new_with_path(path.clone());
        assert_eq!(doc.file_path, Some(path));
    }

    #[test]
    fn test_new_with_path_relative_path() {
        let path = PathBuf::from("./src/lib.rs");
        let doc = Document::new_with_path(path.clone());
        assert_eq!(doc.file_path, Some(path));
    }

    #[test]
    fn test_new_with_path_windows_style_path() {
        let path = PathBuf::from("C:\\Users\\dev\\project\\main.rs");
        let doc = Document::new_with_path(path.clone());
        assert_eq!(doc.file_path, Some(path));
        assert_eq!(doc.language, LanguageId::Rust);
    }

    #[test]
    fn test_new_with_path_no_extension() {
        // Files without extensions default to PlainText (unless they're special)
        let doc = Document::new_with_path(PathBuf::from("README"));
        assert_eq!(doc.language, LanguageId::PlainText);
    }

    #[test]
    fn test_new_with_path_makefile() {
        // Makefile is detected as Bash (shell syntax)
        let doc = Document::new_with_path(PathBuf::from("Makefile"));
        assert_eq!(doc.language, LanguageId::Bash);
    }

    #[test]
    fn test_new_with_path_hidden_file() {
        let doc = Document::new_with_path(PathBuf::from(".gitignore"));
        assert!(doc.file_path.is_some());
        assert!(doc.is_modified);
    }

    #[test]
    fn test_new_with_path_deeply_nested() {
        let path = PathBuf::from("/a/b/c/d/e/f/g/h/file.rs");
        let doc = Document::new_with_path(path.clone());
        assert_eq!(doc.file_path, Some(path));
    }

    // ========================================================================
    // Display name tests
    // ========================================================================

    #[test]
    fn test_display_name_with_path() {
        let doc = Document::new_with_path(PathBuf::from("/path/to/myfile.rs"));
        assert_eq!(doc.display_name(), "myfile.rs");
    }

    #[test]
    fn test_display_name_with_untitled() {
        let mut doc = Document::new();
        doc.untitled_name = Some("Untitled-3".to_string());
        assert_eq!(doc.display_name(), "Untitled-3");
    }

    #[test]
    fn test_display_name_fallback() {
        let doc = Document::new();
        assert_eq!(doc.display_name(), "Untitled");
    }

    // ========================================================================
    // Line operations tests
    // ========================================================================

    #[test]
    fn test_line_count_empty() {
        let doc = Document::new();
        assert_eq!(doc.line_count(), 1); // Empty rope has 1 line
    }

    #[test]
    fn test_line_count_single_line() {
        let doc = Document::with_text("hello");
        assert_eq!(doc.line_count(), 1);
    }

    #[test]
    fn test_line_count_multiple_lines() {
        let doc = Document::with_text("line1\nline2\nline3");
        assert_eq!(doc.line_count(), 3);
    }

    #[test]
    fn test_line_length_excludes_newline() {
        let doc = Document::with_text("hello\nworld\n");
        assert_eq!(doc.line_length(0), 5); // "hello" not "hello\n"
        assert_eq!(doc.line_length(1), 5); // "world"
    }

    #[test]
    fn test_line_length_empty_line() {
        let doc = Document::with_text("hello\n\nworld");
        assert_eq!(doc.line_length(1), 0); // Empty line
    }

    #[test]
    fn test_get_line_valid() {
        let doc = Document::with_text("first\nsecond\nthird");
        assert_eq!(doc.get_line(0), Some("first\n".to_string()));
        assert_eq!(doc.get_line(1), Some("second\n".to_string()));
        assert_eq!(doc.get_line(2), Some("third".to_string()));
    }

    #[test]
    fn test_get_line_out_of_bounds() {
        let doc = Document::with_text("single line");
        assert!(doc.get_line(99).is_none());
    }

    // ========================================================================
    // Cursor/offset conversion tests
    // ========================================================================

    #[test]
    fn test_cursor_to_offset_start() {
        let doc = Document::with_text("hello\nworld");
        assert_eq!(doc.cursor_to_offset(0, 0), 0);
    }

    #[test]
    fn test_cursor_to_offset_second_line() {
        let doc = Document::with_text("hello\nworld");
        assert_eq!(doc.cursor_to_offset(1, 0), 6);
    }

    #[test]
    fn test_offset_to_cursor_roundtrip() {
        let doc = Document::with_text("first\nsecond\nthird");
        for offset in 0..doc.buffer.len_chars() {
            let (line, col) = doc.offset_to_cursor(offset);
            let result = doc.cursor_to_offset(line, col);
            assert_eq!(result, offset);
        }
    }

    // ========================================================================
    // Edit operation tests
    // ========================================================================

    #[test]
    fn test_push_edit_increments_revision() {
        let mut doc = Document::with_text("hello");
        let initial_rev = doc.revision;

        doc.push_edit(EditOperation::Insert {
            position: 0,
            text: "X".to_string(),
            cursor_before: Cursor::default(),
            cursor_after: Cursor::default(),
        });

        assert_eq!(doc.revision, initial_rev + 1);
    }

    #[test]
    fn test_push_edit_marks_modified() {
        let mut doc = Document::with_text("hello");
        doc.is_modified = false;

        doc.push_edit(EditOperation::Insert {
            position: 0,
            text: "X".to_string(),
            cursor_before: Cursor::default(),
            cursor_after: Cursor::default(),
        });

        assert!(doc.is_modified);
    }

    #[test]
    fn test_push_edit_clears_redo_stack() {
        let mut doc = Document::with_text("hello");
        doc.redo_stack.push(EditOperation::Insert {
            position: 0,
            text: "old".to_string(),
            cursor_before: Cursor::default(),
            cursor_after: Cursor::default(),
        });

        doc.push_edit(EditOperation::Insert {
            position: 0,
            text: "new".to_string(),
            cursor_before: Cursor::default(),
            cursor_after: Cursor::default(),
        });

        assert!(doc.redo_stack.is_empty());
    }

    // ========================================================================
    // Find tests
    // ========================================================================

    #[test]
    fn test_find_all_occurrences_basic() {
        let doc = Document::with_text("abc abc abc");
        let results = doc.find_all_occurrences("abc");
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn test_find_all_occurrences_empty_needle() {
        let doc = Document::with_text("hello");
        let results = doc.find_all_occurrences("");
        assert!(results.is_empty());
    }

    #[test]
    fn test_find_next_occurrence_wraps() {
        let doc = Document::with_text("abc xyz abc");
        // After position 5, next "abc" is at 8
        let result = doc.find_next_occurrence("abc", 5);
        assert_eq!(result, Some((8, 11)));

        // After position 10, wraps to first occurrence at 0
        let result = doc.find_next_occurrence("abc", 10);
        assert_eq!(result, Some((0, 3)));
    }

    #[test]
    fn test_find_next_occurrence_empty_needle() {
        let doc = Document::with_text("hello");
        assert_eq!(doc.find_next_occurrence("", 0), None);
    }

    #[test]
    fn test_find_next_occurrence_not_found() {
        let doc = Document::with_text("abc xyz");
        assert_eq!(doc.find_next_occurrence("zzz", 0), None);
    }

    #[test]
    fn test_find_next_occurrence_from_match_start() {
        let doc = Document::with_text("abc xyz abc");
        // Start at first match start: should find next match (not same one)
        // because we search for occurrences where start > after_offset
        assert_eq!(doc.find_next_occurrence("abc", 0), Some((8, 11)));
    }

    #[test]
    fn test_find_next_occurrence_start_past_end() {
        let doc = Document::with_text("abc xyz");
        let len = doc.buffer.len_chars();
        // Past end should wrap to first
        assert_eq!(doc.find_next_occurrence("abc", len + 10), Some((0, 3)));
    }

    // ========================================================================
    // Case-insensitive find tests
    // ========================================================================

    #[test]
    fn test_find_case_insensitive_basic() {
        let doc = Document::with_text("Hello HELLO hello");
        let results = doc.find_all_occurrences_with_options("hello", false);
        assert_eq!(results.len(), 3);
        assert_eq!(results[0], (0, 5));
        assert_eq!(results[1], (6, 11));
        assert_eq!(results[2], (12, 17));
    }

    #[test]
    fn test_find_case_sensitive_basic() {
        let doc = Document::with_text("Hello HELLO hello");
        let results = doc.find_all_occurrences_with_options("hello", true);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], (12, 17));
    }

    #[test]
    fn test_find_next_case_insensitive() {
        let doc = Document::with_text("Hello HELLO hello");
        let result = doc.find_next_occurrence_with_options("HELLO", 0, false);
        assert_eq!(result, Some((6, 11))); // First after position 0
    }

    // ========================================================================
    // Unicode find tests
    // ========================================================================

    #[test]
    fn test_find_unicode_single_char() {
        let doc = Document::with_text("αβγ αβγ");
        let results = doc.find_all_occurrences("β");
        assert_eq!(results, vec![(1, 2), (5, 6)]);
    }

    #[test]
    fn test_find_unicode_word() {
        let doc = Document::with_text("café café");
        let results = doc.find_all_occurrences("café");
        assert_eq!(results, vec![(0, 4), (5, 9)]);
    }

    #[test]
    fn test_find_unicode_emoji() {
        let doc = Document::with_text("hello 🎉 world 🎉 end");
        let results = doc.find_all_occurrences("🎉");
        assert_eq!(results, vec![(6, 7), (14, 15)]);
    }

    #[test]
    fn test_find_unicode_mixed() {
        let doc = Document::with_text("日本語テスト 日本語");
        let results = doc.find_all_occurrences("日本語");
        assert_eq!(results, vec![(0, 3), (7, 10)]);
    }

    #[test]
    fn test_find_unicode_case_insensitive() {
        let doc = Document::with_text("Ößer ößer ÖSSER");
        // German sharp s case folding
        let results = doc.find_all_occurrences_with_options("ößer", false);
        // Note: simple lowercase may not handle ẞ properly, but ö should work
        assert!(results.len() >= 2);
    }

    // ========================================================================
    // Find previous tests
    // ========================================================================

    #[test]
    fn test_find_prev_occurrence_basic() {
        let doc = Document::with_text("abc xyz abc");
        // Before position 5, prev "abc" is at 0
        let result = doc.find_prev_occurrence_with_options("abc", 5, true);
        assert_eq!(result, Some((0, 3)));

        // Before position 10 (after second "abc" starts at 8), prev is at 8
        let result = doc.find_prev_occurrence_with_options("abc", 10, true);
        assert_eq!(result, Some((8, 11)));
    }

    #[test]
    fn test_find_prev_occurrence_wraps() {
        let doc = Document::with_text("abc xyz abc");
        // Before position 2 (inside first match), should wrap to last
        let result = doc.find_prev_occurrence_with_options("abc", 0, true);
        assert_eq!(result, Some((8, 11)));
    }

    #[test]
    fn test_find_prev_occurrence_empty_needle() {
        let doc = Document::with_text("hello");
        assert_eq!(doc.find_prev_occurrence_with_options("", 5, true), None);
    }

    // ========================================================================
    // Overlapping match tests
    // ========================================================================

    #[test]
    fn test_find_overlapping_matches() {
        let doc = Document::with_text("aaaa");
        let results = doc.find_all_occurrences("aa");
        // Should find overlapping: (0,2), (1,3), (2,4)
        assert_eq!(results, vec![(0, 2), (1, 3), (2, 4)]);
    }

    #[test]
    fn test_find_overlapping_pattern() {
        let doc = Document::with_text("ababa");
        let results = doc.find_all_occurrences("aba");
        // Should find overlapping: (0,3), (2,5)
        assert_eq!(results, vec![(0, 3), (2, 5)]);
    }

    // ========================================================================
    // Edge case tests
    // ========================================================================

    #[test]
    fn test_find_in_empty_document() {
        let doc = Document::with_text("");
        assert!(doc.find_all_occurrences("test").is_empty());
        assert_eq!(doc.find_next_occurrence("test", 0), None);
    }

    #[test]
    fn test_find_needle_longer_than_haystack() {
        let doc = Document::with_text("ab");
        assert!(doc.find_all_occurrences("abcdef").is_empty());
    }

    #[test]
    fn test_find_exact_match() {
        let doc = Document::with_text("hello");
        let results = doc.find_all_occurrences("hello");
        assert_eq!(results, vec![(0, 5)]);
    }

    #[test]
    fn test_find_with_newlines() {
        let doc = Document::with_text("line1\nline2\nline1");
        let results = doc.find_all_occurrences("line1");
        assert_eq!(results, vec![(0, 5), (12, 17)]);
    }

    #[test]
    fn test_find_newline_character() {
        let doc = Document::with_text("a\nb\nc");
        let results = doc.find_all_occurrences("\n");
        assert_eq!(results, vec![(1, 2), (3, 4)]);
    }
}
