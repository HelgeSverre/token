//! Document model - represents the text buffer and file state

use ropey::Rope;
use std::path::PathBuf;

use super::editor::Cursor;

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
}

/// Document state - the text buffer and associated file metadata
#[derive(Debug, Clone)]
pub struct Document {
    /// The text buffer
    pub buffer: Rope,
    /// Path to the file on disk (None for new/unsaved files)
    pub file_path: Option<PathBuf>,
    /// Whether the buffer has unsaved changes
    pub is_modified: bool,
    /// Undo stack
    pub undo_stack: Vec<EditOperation>,
    /// Redo stack
    pub redo_stack: Vec<EditOperation>,
}

impl Document {
    /// Create a new empty document
    pub fn new() -> Self {
        Self {
            buffer: Rope::from(""),
            file_path: None,
            is_modified: false,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    /// Create a document with initial text
    pub fn with_text(text: &str) -> Self {
        Self {
            buffer: Rope::from(text),
            file_path: None,
            is_modified: false,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    /// Load a document from a file path
    pub fn from_file(path: PathBuf) -> Result<Self, std::io::Error> {
        let content = std::fs::read_to_string(&path)?;
        Ok(Self {
            buffer: Rope::from(content),
            file_path: Some(path),
            is_modified: false,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        })
    }

    /// Get the number of lines in the document
    pub fn line_count(&self) -> usize {
        self.buffer.len_lines()
    }

    /// Get a line by index
    pub fn get_line(&self, line_idx: usize) -> Option<String> {
        if line_idx < self.buffer.len_lines() {
            let line = self.buffer.line(line_idx);
            Some(line.to_string())
        } else {
            None
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
    pub fn cursor_to_offset(&self, line: usize, column: usize) -> usize {
        let mut pos = 0;
        for i in 0..line {
            if i < self.buffer.len_lines() {
                pos += self.buffer.line(i).len_chars();
            }
        }
        pos + column.min(self.line_length(line))
    }

    /// Convert a buffer offset to (line, column) position
    pub fn offset_to_cursor(&self, offset: usize) -> (usize, usize) {
        let mut remaining = offset;
        for line_idx in 0..self.buffer.len_lines() {
            let line = self.buffer.line(line_idx);
            let line_len = line.len_chars();
            if remaining < line_len {
                return (line_idx, remaining);
            }
            remaining -= line_len;
        }
        // Past end - return end of document
        let last_line = self.buffer.len_lines().saturating_sub(1);
        (last_line, self.line_length(last_line))
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
    }
}

impl Default for Document {
    fn default() -> Self {
        Self::new()
    }
}
