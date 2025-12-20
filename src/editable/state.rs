//! EditableState - the main abstraction for editable text with cursors, selections, and history.

use crate::util::{char_type, CharType};

use super::buffer::{TextBuffer, TextBufferMut};
use super::constraints::EditConstraints;
use super::cursor::{Cursor, Position};
use super::history::{EditHistory, EditOperation};
use super::selection::Selection;

/// Main abstraction for editable text with cursors, selections, and history.
///
/// Generic over the buffer type B (StringBuffer for single-line, RopeBuffer for multi-line).
#[derive(Debug, Clone)]
pub struct EditableState<B: TextBuffer> {
    /// The text buffer
    pub buffer: B,
    /// Cursor positions (always at least one)
    pub cursors: Vec<Cursor>,
    /// Selections (parallel array with cursors)
    pub selections: Vec<Selection>,
    /// Index of the active/primary cursor
    pub active_cursor: usize,
    /// Constraints for this editing context
    pub constraints: EditConstraints,
    /// Edit history for undo/redo
    history: EditHistory,
}

impl<B: TextBuffer> EditableState<B> {
    /// Create a new EditableState with the given buffer and constraints
    pub fn new(buffer: B, constraints: EditConstraints) -> Self {
        let cursor = Cursor::new(0, 0);
        let selection = Selection::collapsed(Position::zero());
        Self {
            buffer,
            cursors: vec![cursor],
            selections: vec![selection],
            active_cursor: 0,
            constraints,
            history: EditHistory::new(),
        }
    }

    /// Get the primary cursor
    pub fn cursor(&self) -> &Cursor {
        &self.cursors[self.active_cursor]
    }

    /// Get the primary cursor mutably
    pub fn cursor_mut(&mut self) -> &mut Cursor {
        &mut self.cursors[self.active_cursor]
    }

    /// Get the primary selection
    pub fn selection(&self) -> &Selection {
        &self.selections[self.active_cursor]
    }

    /// Get the primary selection mutably
    pub fn selection_mut(&mut self) -> &mut Selection {
        &mut self.selections[self.active_cursor]
    }

    /// Get the text content as a String
    pub fn text(&self) -> String {
        self.buffer.content()
    }

    /// Get the selected text (empty string if no selection)
    pub fn selected_text(&self) -> String {
        let sel = self.selection();
        if sel.is_empty() {
            return String::new();
        }
        let start = self
            .buffer
            .position_to_offset(sel.start().line, sel.start().column);
        let end = self
            .buffer
            .position_to_offset(sel.end().line, sel.end().column);
        self.buffer.slice(start..end)
    }

    /// Check if there is a non-empty selection
    pub fn has_selection(&self) -> bool {
        !self.selection().is_empty()
    }

    /// Check if undo is available
    pub fn can_undo(&self) -> bool {
        self.constraints.enable_undo && self.history.can_undo()
    }

    /// Check if redo is available
    pub fn can_redo(&self) -> bool {
        self.constraints.enable_undo && self.history.can_redo()
    }

    /// Collapse to a single cursor (for contexts that don't support multi-cursor)
    pub fn collapse_cursors(&mut self) {
        if self.cursors.len() > 1 {
            let cursor = self.cursors[self.active_cursor];
            let selection = self.selections[self.active_cursor];
            self.cursors = vec![cursor];
            self.selections = vec![selection];
            self.active_cursor = 0;
        }
    }

    /// Sync selection head with cursor position
    fn sync_selection_head(&mut self) {
        let idx = self.active_cursor;
        let pos = self.cursors[idx].to_position();
        self.selections[idx].head = pos;
    }

    /// Collapse selection to cursor position
    pub fn collapse_selection(&mut self) {
        let idx = self.active_cursor;
        let pos = self.cursors[idx].to_position();
        self.selections[idx] = Selection::collapsed(pos);
    }

    /// Collapse all selections to their cursors
    pub fn collapse_all_selections(&mut self) {
        for i in 0..self.cursors.len() {
            let pos = self.cursors[i].to_position();
            self.selections[i] = Selection::collapsed(pos);
        }
    }
}

// =============================================================================
// Movement Operations
// =============================================================================

impl<B: TextBuffer> EditableState<B> {
    /// Move cursor left by one character
    pub fn move_left(&mut self, extend_selection: bool) {
        let idx = self.active_cursor;
        let allow_multiline = self.constraints.allow_multiline;

        // Handle selection collapse if not extending
        if !extend_selection && !self.selections[idx].is_empty() {
            let start = self.selections[idx].start();
            self.cursors[idx].line = start.line;
            self.cursors[idx].column = start.column;
            self.cursors[idx].clear_desired_column();
            self.collapse_selection();
            return;
        }

        if self.cursors[idx].column > 0 {
            self.cursors[idx].column -= 1;
        } else if self.cursors[idx].line > 0 && allow_multiline {
            self.cursors[idx].line -= 1;
            self.cursors[idx].column = self.buffer.line_length(self.cursors[idx].line);
        }
        self.cursors[idx].clear_desired_column();

        if extend_selection {
            self.sync_selection_head();
        } else {
            self.collapse_selection();
        }
    }

    /// Move cursor right by one character
    pub fn move_right(&mut self, extend_selection: bool) {
        let idx = self.active_cursor;
        let allow_multiline = self.constraints.allow_multiline;
        let line_len = self.buffer.line_length(self.cursors[idx].line);
        let line_count = self.buffer.line_count();

        // Handle selection collapse if not extending
        if !extend_selection && !self.selections[idx].is_empty() {
            let end = self.selections[idx].end();
            self.cursors[idx].line = end.line;
            self.cursors[idx].column = end.column;
            self.cursors[idx].clear_desired_column();
            self.collapse_selection();
            return;
        }

        if self.cursors[idx].column < line_len {
            self.cursors[idx].column += 1;
        } else if self.cursors[idx].line + 1 < line_count && allow_multiline {
            self.cursors[idx].line += 1;
            self.cursors[idx].column = 0;
        }
        self.cursors[idx].clear_desired_column();

        if extend_selection {
            self.sync_selection_head();
        } else {
            self.collapse_selection();
        }
    }

    /// Move cursor up by one line
    pub fn move_up(&mut self, extend_selection: bool) {
        if !self.constraints.allow_multiline {
            return;
        }

        let idx = self.active_cursor;
        if self.cursors[idx].line == 0 {
            return;
        }

        self.cursors[idx].set_desired_column();
        self.cursors[idx].line -= 1;
        let line_len = self.buffer.line_length(self.cursors[idx].line);
        self.cursors[idx].column = self.cursors[idx].effective_column().min(line_len);

        if extend_selection {
            self.sync_selection_head();
        } else {
            self.collapse_selection();
        }
    }

    /// Move cursor down by one line
    pub fn move_down(&mut self, extend_selection: bool) {
        if !self.constraints.allow_multiline {
            return;
        }

        let idx = self.active_cursor;
        let line_count = self.buffer.line_count();
        if self.cursors[idx].line + 1 >= line_count {
            return;
        }

        self.cursors[idx].set_desired_column();
        self.cursors[idx].line += 1;
        let line_len = self.buffer.line_length(self.cursors[idx].line);
        self.cursors[idx].column = self.cursors[idx].effective_column().min(line_len);

        if extend_selection {
            self.sync_selection_head();
        } else {
            self.collapse_selection();
        }
    }

    /// Move cursor to start of line
    pub fn move_line_start(&mut self, extend_selection: bool) {
        let idx = self.active_cursor;
        self.cursors[idx].column = 0;
        self.cursors[idx].clear_desired_column();

        if extend_selection {
            self.sync_selection_head();
        } else {
            self.collapse_selection();
        }
    }

    /// Move cursor to end of line
    pub fn move_line_end(&mut self, extend_selection: bool) {
        let idx = self.active_cursor;
        let line_len = self.buffer.line_length(self.cursors[idx].line);
        self.cursors[idx].column = line_len;
        self.cursors[idx].clear_desired_column();

        if extend_selection {
            self.sync_selection_head();
        } else {
            self.collapse_selection();
        }
    }

    /// Smart line start: toggle between first non-whitespace and column 0
    pub fn move_line_start_smart(&mut self, extend_selection: bool) {
        let idx = self.active_cursor;
        let first_non_ws = self
            .buffer
            .first_non_whitespace_column(self.cursors[idx].line);
        let current_col = self.cursors[idx].column;

        if current_col == first_non_ws || current_col == 0 {
            self.cursors[idx].column = if current_col == 0 { first_non_ws } else { 0 };
        } else {
            self.cursors[idx].column = first_non_ws;
        }
        self.cursors[idx].clear_desired_column();

        if extend_selection {
            self.sync_selection_head();
        } else {
            self.collapse_selection();
        }
    }

    /// Move cursor to start of document
    pub fn move_document_start(&mut self, extend_selection: bool) {
        let idx = self.active_cursor;
        self.cursors[idx].line = 0;
        self.cursors[idx].column = 0;
        self.cursors[idx].clear_desired_column();

        if extend_selection {
            self.sync_selection_head();
        } else {
            self.collapse_selection();
        }
    }

    /// Move cursor to end of document
    pub fn move_document_end(&mut self, extend_selection: bool) {
        let idx = self.active_cursor;
        let last_line = self.buffer.line_count().saturating_sub(1);
        let last_col = self.buffer.line_length(last_line);
        self.cursors[idx].line = last_line;
        self.cursors[idx].column = last_col;
        self.cursors[idx].clear_desired_column();

        if extend_selection {
            self.sync_selection_head();
        } else {
            self.collapse_selection();
        }
    }

    /// Move cursor by one word to the left
    pub fn move_word_left(&mut self, extend_selection: bool) {
        let idx = self.active_cursor;
        let allow_multiline = self.constraints.allow_multiline;

        // Handle selection collapse if not extending
        if !extend_selection && !self.selections[idx].is_empty() {
            let start = self.selections[idx].start();
            self.cursors[idx].line = start.line;
            self.cursors[idx].column = start.column;
            self.cursors[idx].clear_desired_column();
            self.collapse_selection();
            return;
        }

        // At start of line? Move to end of previous line
        if self.cursors[idx].column == 0 {
            if self.cursors[idx].line > 0 && allow_multiline {
                self.cursors[idx].line -= 1;
                self.cursors[idx].column = self.buffer.line_length(self.cursors[idx].line);
            }
            self.cursors[idx].clear_desired_column();
            if extend_selection {
                self.sync_selection_head();
            } else {
                self.collapse_selection();
            }
            return;
        }

        // Get line length and use char_at for direct character access (avoids Vec<char> allocation)
        let line_len = self.buffer.line_length(self.cursors[idx].line);
        let line = self.cursors[idx].line;

        // Start from position before cursor
        let mut pos = self.cursors[idx].column.min(line_len);

        // Skip any whitespace/punctuation first (moving backwards)
        while pos > 0 {
            if let Some(ch) = self.buffer.char_at(line, pos - 1) {
                if char_type(ch) == CharType::WordChar {
                    break;
                }
            }
            pos -= 1;
        }

        // Then skip word characters
        while pos > 0 {
            if let Some(ch) = self.buffer.char_at(line, pos - 1) {
                if char_type(ch) != CharType::WordChar {
                    break;
                }
            }
            pos -= 1;
        }

        self.cursors[idx].column = pos;
        self.cursors[idx].clear_desired_column();

        if extend_selection {
            self.sync_selection_head();
        } else {
            self.collapse_selection();
        }
    }

    /// Move cursor by one word to the right
    pub fn move_word_right(&mut self, extend_selection: bool) {
        let idx = self.active_cursor;
        let allow_multiline = self.constraints.allow_multiline;

        // Handle selection collapse if not extending
        if !extend_selection && !self.selections[idx].is_empty() {
            let end = self.selections[idx].end();
            self.cursors[idx].line = end.line;
            self.cursors[idx].column = end.column;
            self.cursors[idx].clear_desired_column();
            self.collapse_selection();
            return;
        }

        let line_len = self.buffer.line_length(self.cursors[idx].line);

        // At end of line? Move to start of next line
        if self.cursors[idx].column >= line_len {
            if self.cursors[idx].line + 1 < self.buffer.line_count() && allow_multiline {
                self.cursors[idx].line += 1;
                self.cursors[idx].column = 0;
            }
            self.cursors[idx].clear_desired_column();
            if extend_selection {
                self.sync_selection_head();
            } else {
                self.collapse_selection();
            }
            return;
        }

        // Use char_at for direct character access (avoids Vec<char> allocation)
        let line = self.cursors[idx].line;

        let mut pos = self.cursors[idx].column;

        // Skip current word type
        if let Some(first_ch) = self.buffer.char_at(line, pos) {
            let start_type = char_type(first_ch);
            while pos < line_len {
                if let Some(ch) = self.buffer.char_at(line, pos) {
                    if char_type(ch) != start_type {
                        break;
                    }
                } else {
                    break;
                }
                pos += 1;
            }
        }

        // Skip any following whitespace
        while pos < line_len {
            if let Some(ch) = self.buffer.char_at(line, pos) {
                if char_type(ch) != CharType::Whitespace {
                    break;
                }
            } else {
                break;
            }
            pos += 1;
        }

        self.cursors[idx].column = pos;
        self.cursors[idx].clear_desired_column();

        if extend_selection {
            self.sync_selection_head();
        } else {
            self.collapse_selection();
        }
    }

    /// Select all text
    pub fn select_all(&mut self) {
        if !self.constraints.allow_selection {
            return;
        }

        let idx = self.active_cursor;
        let last_line = self.buffer.line_count().saturating_sub(1);
        let last_col = self.buffer.line_length(last_line);

        // Set anchor at start, head at end
        self.selections[idx] = Selection::new(Position::zero(), Position::new(last_line, last_col));
        self.cursors[idx].line = last_line;
        self.cursors[idx].column = last_col;
        self.cursors[idx].clear_desired_column();
    }

    /// Select the word at the cursor position
    pub fn select_word(&mut self) {
        if !self.constraints.allow_selection {
            return;
        }

        let idx = self.active_cursor;
        let line = self.cursors[idx].line;
        let col = self.cursors[idx].column;

        let line_content = self.buffer.line(line).unwrap_or_default();
        let chars: Vec<char> = line_content.chars().collect();

        if chars.is_empty() {
            return;
        }

        // Clamp column to valid range
        let col = col.min(chars.len().saturating_sub(1));

        // Check character type at cursor
        let ch = chars[col];
        let target_type = char_type(ch);

        // Find word boundaries
        let mut start = col;
        let mut end = col;

        // Expand left to find start of word
        while start > 0 && char_type(chars[start - 1]) == target_type {
            start -= 1;
        }

        // Expand right to find end of word
        while end < chars.len() && char_type(chars[end]) == target_type {
            end += 1;
        }

        // Set selection
        self.selections[idx] = Selection::new(Position::new(line, start), Position::new(line, end));
        self.cursors[idx].column = end;
        self.cursors[idx].clear_desired_column();
    }
}

// =============================================================================
// Editing Operations (require TextBufferMut)
// =============================================================================

impl<B: TextBuffer + TextBufferMut> EditableState<B> {
    /// Insert a character at the cursor position
    /// Returns true if the character was inserted, false if rejected by constraints
    pub fn insert_char(&mut self, ch: char) -> bool {
        // Check character filter
        if !self.constraints.is_char_allowed(ch) {
            return false;
        }

        // Check newline in single-line mode
        if ch == '\n' && !self.constraints.allow_multiline {
            return false;
        }

        let idx = self.active_cursor;

        // Check max length
        let current_len = self.buffer.len_chars();
        let selection_len = if !self.selections[idx].is_empty() {
            let sel = &self.selections[idx];
            let start = self
                .buffer
                .position_to_offset(sel.start().line, sel.start().column);
            let end = self
                .buffer
                .position_to_offset(sel.end().line, sel.end().column);
            end - start
        } else {
            0
        };
        if self
            .constraints
            .would_exceed_max_length(current_len - selection_len, 1)
        {
            return false;
        }

        let cursor_before = self.cursors[idx];

        // Delete selection first if any
        let (offset, deleted_text) = if !self.selections[idx].is_empty() {
            let sel = self.selections[idx];
            let start_pos = sel.start();
            let start = self
                .buffer
                .position_to_offset(start_pos.line, start_pos.column);
            let end = self
                .buffer
                .position_to_offset(sel.end().line, sel.end().column);
            let deleted = self.buffer.slice(start..end);
            self.buffer.remove(start..end);

            // Move cursor to selection start
            self.cursors[idx].line = start_pos.line;
            self.cursors[idx].column = start_pos.column;
            self.collapse_selection();

            (start, deleted)
        } else {
            let offset = self
                .buffer
                .position_to_offset(self.cursors[idx].line, self.cursors[idx].column);
            (offset, String::new())
        };

        // Insert character
        self.buffer.insert_char(offset, ch);

        // Update cursor position
        if ch == '\n' {
            self.cursors[idx].line += 1;
            self.cursors[idx].column = 0;
        } else {
            self.cursors[idx].column += 1;
        }
        self.cursors[idx].clear_desired_column();
        self.collapse_selection();

        // Record in history
        if self.constraints.enable_undo {
            let cursor_after = self.cursors[idx];
            let op = if deleted_text.is_empty() {
                EditOperation::insert(offset, ch.to_string(), cursor_before, cursor_after)
            } else {
                EditOperation::replace(
                    offset,
                    deleted_text,
                    ch.to_string(),
                    cursor_before,
                    cursor_after,
                )
            };
            self.history.push(op);
        }

        true
    }

    /// Insert text at the cursor position
    /// Returns true if the text was inserted
    pub fn insert_text(&mut self, text: &str) -> bool {
        if text.is_empty() {
            return true;
        }

        // Check constraints
        for ch in text.chars() {
            if !self.constraints.is_char_allowed(ch) {
                return false;
            }
            if ch == '\n' && !self.constraints.allow_multiline {
                return false;
            }
        }

        let idx = self.active_cursor;
        let cursor_before = self.cursors[idx];

        // Delete selection first if any
        let (offset, deleted_text) = if !self.selections[idx].is_empty() {
            let sel = self.selections[idx];
            let start_pos = sel.start();
            let start = self
                .buffer
                .position_to_offset(start_pos.line, start_pos.column);
            let end = self
                .buffer
                .position_to_offset(sel.end().line, sel.end().column);
            let deleted = self.buffer.slice(start..end);
            self.buffer.remove(start..end);

            self.cursors[idx].line = start_pos.line;
            self.cursors[idx].column = start_pos.column;
            self.collapse_selection();

            (start, deleted)
        } else {
            let offset = self
                .buffer
                .position_to_offset(self.cursors[idx].line, self.cursors[idx].column);
            (offset, String::new())
        };

        // Insert text
        self.buffer.insert(offset, text);

        // Update cursor position
        let lines_added = text.chars().filter(|c| *c == '\n').count();
        if lines_added > 0 {
            self.cursors[idx].line += lines_added;
            // Find column after last newline
            let last_newline = text.rfind('\n').unwrap();
            self.cursors[idx].column = text[last_newline + 1..].chars().count();
        } else {
            self.cursors[idx].column += text.chars().count();
        }
        self.cursors[idx].clear_desired_column();
        self.collapse_selection();

        // Record in history
        if self.constraints.enable_undo {
            let cursor_after = self.cursors[idx];
            let op = if deleted_text.is_empty() {
                EditOperation::insert(offset, text.to_string(), cursor_before, cursor_after)
            } else {
                EditOperation::replace(
                    offset,
                    deleted_text,
                    text.to_string(),
                    cursor_before,
                    cursor_after,
                )
            };
            self.history.push(op);
        }

        true
    }

    /// Delete character before cursor (Backspace)
    pub fn delete_backward(&mut self) -> bool {
        let idx = self.active_cursor;
        let cursor_before = self.cursors[idx];

        // If there's a selection, delete it
        if !self.selections[idx].is_empty() {
            return self.delete_selection();
        }

        if self.cursors[idx].column == 0 && self.cursors[idx].line == 0 {
            return false;
        }

        let offset = self
            .buffer
            .position_to_offset(self.cursors[idx].line, self.cursors[idx].column);
        if offset == 0 {
            return false;
        }

        // Get character to delete
        let deleted_char = self.buffer.slice(offset - 1..offset);
        self.buffer.remove(offset - 1..offset);

        // Update cursor
        if self.cursors[idx].column > 0 {
            self.cursors[idx].column -= 1;
        } else if self.cursors[idx].line > 0 {
            self.cursors[idx].line -= 1;
            self.cursors[idx].column = self.buffer.line_length(self.cursors[idx].line);
        }
        self.cursors[idx].clear_desired_column();
        self.collapse_selection();

        // Record in history
        if self.constraints.enable_undo {
            let cursor_after = self.cursors[idx];
            self.history.push(EditOperation::delete(
                offset - 1,
                deleted_char,
                cursor_before,
                cursor_after,
            ));
        }

        true
    }

    /// Delete character after cursor (Delete key)
    pub fn delete_forward(&mut self) -> bool {
        let idx = self.active_cursor;
        let cursor_before = self.cursors[idx];

        // If there's a selection, delete it
        if !self.selections[idx].is_empty() {
            return self.delete_selection();
        }

        let offset = self
            .buffer
            .position_to_offset(self.cursors[idx].line, self.cursors[idx].column);
        if offset >= self.buffer.len_chars() {
            return false;
        }

        let deleted_char = self.buffer.slice(offset..offset + 1);
        self.buffer.remove(offset..offset + 1);

        // Cursor position doesn't change
        self.cursors[idx].clear_desired_column();

        // Record in history
        if self.constraints.enable_undo {
            let cursor_after = self.cursors[idx];
            self.history.push(EditOperation::delete(
                offset,
                deleted_char,
                cursor_before,
                cursor_after,
            ));
        }

        true
    }

    /// Delete selection if any
    fn delete_selection(&mut self) -> bool {
        let idx = self.active_cursor;
        if self.selections[idx].is_empty() {
            return false;
        }

        let cursor_before = self.cursors[idx];
        let sel = self.selections[idx];
        let start_pos = sel.start();
        let start = self
            .buffer
            .position_to_offset(start_pos.line, start_pos.column);
        let end = self
            .buffer
            .position_to_offset(sel.end().line, sel.end().column);

        let deleted_text = self.buffer.slice(start..end);
        self.buffer.remove(start..end);

        // Move cursor to selection start
        self.cursors[idx].line = start_pos.line;
        self.cursors[idx].column = start_pos.column;
        self.cursors[idx].clear_desired_column();
        self.collapse_selection();

        // Record in history
        if self.constraints.enable_undo {
            let cursor_after = self.cursors[idx];
            self.history.push(EditOperation::delete(
                start,
                deleted_text,
                cursor_before,
                cursor_after,
            ));
        }

        true
    }

    /// Delete word before cursor
    pub fn delete_word_backward(&mut self) -> bool {
        let idx = self.active_cursor;

        if !self.selections[idx].is_empty() {
            return self.delete_selection();
        }

        let cursor_before = self.cursors[idx];
        let start_offset = self
            .buffer
            .position_to_offset(self.cursors[idx].line, self.cursors[idx].column);

        if start_offset == 0 {
            return false;
        }

        // Save current position
        let original_line = self.cursors[idx].line;
        let original_col = self.cursors[idx].column;

        // Move word left to find the start
        self.move_word_left(false);
        let end_offset = self
            .buffer
            .position_to_offset(self.cursors[idx].line, self.cursors[idx].column);

        if end_offset >= start_offset {
            // No movement happened, restore cursor
            self.cursors[idx].line = original_line;
            self.cursors[idx].column = original_col;
            return false;
        }

        let deleted_text = self.buffer.slice(end_offset..start_offset);
        self.buffer.remove(end_offset..start_offset);

        // Record in history
        if self.constraints.enable_undo {
            let cursor_after = self.cursors[idx];
            self.history.push(EditOperation::delete(
                end_offset,
                deleted_text,
                cursor_before,
                cursor_after,
            ));
        }

        true
    }

    /// Delete word after cursor
    pub fn delete_word_forward(&mut self) -> bool {
        let idx = self.active_cursor;

        if !self.selections[idx].is_empty() {
            return self.delete_selection();
        }

        let cursor_before = self.cursors[idx];
        let start_offset = self
            .buffer
            .position_to_offset(self.cursors[idx].line, self.cursors[idx].column);

        if start_offset >= self.buffer.len_chars() {
            return false;
        }

        // Save current position
        let original_line = self.cursors[idx].line;
        let original_col = self.cursors[idx].column;

        // Move word right to find the end
        self.move_word_right(false);
        let end_offset = self
            .buffer
            .position_to_offset(self.cursors[idx].line, self.cursors[idx].column);

        if end_offset <= start_offset {
            // No movement happened, restore cursor
            self.cursors[idx].line = original_line;
            self.cursors[idx].column = original_col;
            return false;
        }

        // Move cursor back to start
        self.cursors[idx].line = original_line;
        self.cursors[idx].column = original_col;

        let deleted_text = self.buffer.slice(start_offset..end_offset);
        self.buffer.remove(start_offset..end_offset);

        // Cursor stays at start position
        self.cursors[idx].clear_desired_column();
        self.collapse_selection();

        // Record in history
        if self.constraints.enable_undo {
            let cursor_after = self.cursors[idx];
            self.history.push(EditOperation::delete(
                start_offset,
                deleted_text,
                cursor_before,
                cursor_after,
            ));
        }

        true
    }

    /// Undo the last operation
    pub fn undo(&mut self) -> bool {
        if !self.constraints.enable_undo {
            return false;
        }

        let op = match self.history.pop_undo() {
            Some(op) => op,
            None => return false,
        };

        // Apply the inverse operation
        if !op.inserted_text.is_empty() {
            // Original was an insert, so we delete
            let end = op.offset + op.inserted_text.chars().count();
            self.buffer.remove(op.offset..end);
        }
        if !op.deleted_text.is_empty() {
            // Original was a delete, so we insert
            self.buffer.insert(op.offset, &op.deleted_text);
        }

        // Restore cursors
        if !op.cursors_before.is_empty() {
            self.cursors = op.cursors_before.clone();
            self.selections = op
                .cursors_before
                .iter()
                .map(|c| Selection::collapsed(c.to_position()))
                .collect();
            self.active_cursor = 0.min(self.cursors.len() - 1);
        }

        true
    }

    /// Redo the last undone operation
    pub fn redo(&mut self) -> bool {
        if !self.constraints.enable_undo {
            return false;
        }

        let op = match self.history.pop_redo() {
            Some(op) => op,
            None => return false,
        };

        // Apply the inverse operation (which restores the original)
        if !op.inserted_text.is_empty() {
            let end = op.offset + op.inserted_text.chars().count();
            self.buffer.remove(op.offset..end);
        }
        if !op.deleted_text.is_empty() {
            self.buffer.insert(op.offset, &op.deleted_text);
        }

        // Restore cursors
        if !op.cursors_before.is_empty() {
            self.cursors = op.cursors_before.clone();
            self.selections = op
                .cursors_before
                .iter()
                .map(|c| Selection::collapsed(c.to_position()))
                .collect();
            self.active_cursor = 0.min(self.cursors.len() - 1);
        }

        true
    }

    /// Clear the buffer and reset cursor
    pub fn clear(&mut self) {
        let idx = self.active_cursor;
        let cursor_before = self.cursors[idx];
        let content = self.buffer.content();

        self.buffer.clear();
        self.cursors = vec![Cursor::new(0, 0)];
        self.selections = vec![Selection::collapsed(Position::zero())];
        self.active_cursor = 0;

        if self.constraints.enable_undo && !content.is_empty() {
            let cursor_after = self.cursors[0];
            self.history.push(EditOperation::delete(
                0,
                content,
                cursor_before,
                cursor_after,
            ));
        }
    }

    /// Set the content, replacing everything
    pub fn set_content(&mut self, text: &str) {
        self.buffer.set_content(text);
        // Reset cursor to end
        let last_line = self.buffer.line_count().saturating_sub(1);
        let last_col = self.buffer.line_length(last_line);
        self.cursors = vec![Cursor::new(last_line, last_col)];
        self.selections = vec![Selection::collapsed(Position::new(last_line, last_col))];
        self.active_cursor = 0;
        self.history.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::super::buffer::StringBuffer;
    use super::*;

    fn create_test_state(text: &str) -> EditableState<StringBuffer> {
        EditableState::new(
            StringBuffer::from_text(text),
            EditConstraints::single_line(),
        )
    }

    #[test]
    fn test_cursor_movement() {
        let mut state = create_test_state("hello");
        state.cursors[0].column = 2;
        state.collapse_selection();

        state.move_left(false);
        assert_eq!(state.cursor().column, 1);

        state.move_right(false);
        assert_eq!(state.cursor().column, 2);
    }

    #[test]
    fn test_word_movement() {
        let mut state = create_test_state("hello world");
        state.cursors[0].column = 0;
        state.collapse_selection();

        state.move_word_right(false);
        assert_eq!(state.cursor().column, 6); // After "hello " (at 'w')

        state.move_word_left(false);
        assert_eq!(state.cursor().column, 0);
    }

    #[test]
    fn test_selection() {
        let mut state = create_test_state("hello world");
        state.cursors[0].column = 0;
        state.collapse_selection();

        state.move_word_right(true);
        assert_eq!(state.selection().anchor, Position::new(0, 0));
        assert_eq!(state.selection().head, Position::new(0, 6));
        assert_eq!(state.selected_text(), "hello ");
    }

    #[test]
    fn test_insert_char() {
        let mut state = create_test_state("hllo");
        state.cursors[0].column = 1;
        state.collapse_selection();

        assert!(state.insert_char('e'));
        assert_eq!(state.text(), "hello");
        assert_eq!(state.cursor().column, 2);
    }

    #[test]
    fn test_insert_replaces_selection() {
        let mut state = create_test_state("hello world");
        state.selections[0] = Selection::new(Position::new(0, 0), Position::new(0, 5));
        state.cursors[0].column = 5;

        state.insert_char('X');
        assert_eq!(state.text(), "X world");
        assert_eq!(state.cursor().column, 1);
    }

    #[test]
    fn test_delete_backward() {
        let mut state = create_test_state("hello");
        state.cursors[0].column = 5;
        state.collapse_selection();

        assert!(state.delete_backward());
        assert_eq!(state.text(), "hell");
        assert_eq!(state.cursor().column, 4);
    }

    #[test]
    fn test_delete_forward() {
        let mut state = create_test_state("hello");
        state.cursors[0].column = 0;
        state.collapse_selection();

        assert!(state.delete_forward());
        assert_eq!(state.text(), "ello");
        assert_eq!(state.cursor().column, 0);
    }

    #[test]
    fn test_delete_word_backward() {
        let mut state = create_test_state("hello world");
        state.cursors[0].column = 11;
        state.collapse_selection();

        assert!(state.delete_word_backward());
        assert_eq!(state.text(), "hello ");
    }

    #[test]
    fn test_delete_word_forward() {
        let mut state = create_test_state("hello world");
        state.cursors[0].column = 0;
        state.collapse_selection();

        assert!(state.delete_word_forward());
        assert_eq!(state.text(), "world");
    }

    #[test]
    fn test_undo_redo() {
        let mut state = create_test_state("");

        state.insert_char('a');
        state.insert_char('b');
        assert_eq!(state.text(), "ab");

        assert!(state.undo());
        assert_eq!(state.text(), "a");

        assert!(state.undo());
        assert_eq!(state.text(), "");

        assert!(state.redo());
        assert_eq!(state.text(), "a");
    }

    #[test]
    fn test_select_all() {
        let mut state = create_test_state("hello");

        state.select_all();
        assert_eq!(state.selected_text(), "hello");
    }

    #[test]
    fn test_char_filter() {
        let mut state = EditableState::new(StringBuffer::new(), EditConstraints::numeric());

        assert!(state.insert_char('5'));
        assert!(!state.insert_char('a'));
        assert_eq!(state.text(), "5");
    }

    #[test]
    fn test_line_start_end() {
        let mut state = create_test_state("hello");
        state.cursors[0].column = 2;
        state.collapse_selection();

        state.move_line_end(false);
        assert_eq!(state.cursor().column, 5);

        state.move_line_start(false);
        assert_eq!(state.cursor().column, 0);
    }

    #[test]
    fn test_select_word_middle_of_word() {
        let mut state = create_test_state("hello world");
        state.cursors[0].column = 2; // middle of "hello"
        state.collapse_selection();

        state.select_word();
        assert_eq!(state.selected_text(), "hello");
        assert_eq!(state.selection().start().column, 0);
        assert_eq!(state.selection().end().column, 5);
    }

    #[test]
    fn test_select_word_on_space() {
        let mut state = create_test_state("hello world");
        state.cursors[0].column = 5; // on the space
        state.collapse_selection();

        state.select_word();
        // Space is a single-char word
        assert_eq!(state.selected_text(), " ");
    }

    #[test]
    fn test_select_word_at_start() {
        let mut state = create_test_state("hello world");
        state.cursors[0].column = 0;
        state.collapse_selection();

        state.select_word();
        assert_eq!(state.selected_text(), "hello");
    }

    #[test]
    fn test_select_word_at_end() {
        let mut state = create_test_state("hello world");
        state.cursors[0].column = 10; // on 'd' of "world"
        state.collapse_selection();

        state.select_word();
        assert_eq!(state.selected_text(), "world");
    }
}
