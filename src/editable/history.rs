//! Edit history (undo/redo) for the unified text editing system.

use super::cursor::Cursor;

/// A single edit operation that can be undone/redone.
#[derive(Debug, Clone)]
pub struct EditOperation {
    /// Byte offset where the edit occurred
    pub offset: usize,
    /// Text that was deleted (empty for pure inserts)
    pub deleted_text: String,
    /// Text that was inserted (empty for pure deletes)
    pub inserted_text: String,
    /// Cursor positions before the edit
    pub cursors_before: Vec<Cursor>,
    /// Cursor positions after the edit
    pub cursors_after: Vec<Cursor>,
}

impl EditOperation {
    /// Create an insert operation
    pub fn insert(
        offset: usize,
        text: String,
        cursor_before: Cursor,
        cursor_after: Cursor,
    ) -> Self {
        Self {
            offset,
            deleted_text: String::new(),
            inserted_text: text,
            cursors_before: vec![cursor_before],
            cursors_after: vec![cursor_after],
        }
    }

    /// Create a delete operation
    pub fn delete(
        offset: usize,
        text: String,
        cursor_before: Cursor,
        cursor_after: Cursor,
    ) -> Self {
        Self {
            offset,
            deleted_text: text,
            inserted_text: String::new(),
            cursors_before: vec![cursor_before],
            cursors_after: vec![cursor_after],
        }
    }

    /// Create a replace operation
    pub fn replace(
        offset: usize,
        deleted_text: String,
        inserted_text: String,
        cursor_before: Cursor,
        cursor_after: Cursor,
    ) -> Self {
        Self {
            offset,
            deleted_text,
            inserted_text,
            cursors_before: vec![cursor_before],
            cursors_after: vec![cursor_after],
        }
    }

    /// Create an operation with multiple cursors
    pub fn with_multi_cursor(
        offset: usize,
        deleted_text: String,
        inserted_text: String,
        cursors_before: Vec<Cursor>,
        cursors_after: Vec<Cursor>,
    ) -> Self {
        Self {
            offset,
            deleted_text,
            inserted_text,
            cursors_before,
            cursors_after,
        }
    }

    /// Get the inverse operation for undo
    pub fn inverse(&self) -> Self {
        Self {
            offset: self.offset,
            deleted_text: self.inserted_text.clone(),
            inserted_text: self.deleted_text.clone(),
            cursors_before: self.cursors_after.clone(),
            cursors_after: self.cursors_before.clone(),
        }
    }
}

/// Edit history with undo/redo stacks.
#[derive(Debug, Clone, Default)]
pub struct EditHistory {
    undo_stack: Vec<EditOperation>,
    redo_stack: Vec<EditOperation>,
    max_size: usize,
}

impl EditHistory {
    /// Create a new edit history with default max size
    pub fn new() -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            max_size: 1000,
        }
    }

    /// Create a new edit history with specified max size
    pub fn with_max_size(max_size: usize) -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            max_size,
        }
    }

    /// Push an operation onto the undo stack (clears redo stack)
    pub fn push(&mut self, op: EditOperation) {
        self.redo_stack.clear();
        self.undo_stack.push(op);

        // Trim if exceeded max size
        while self.undo_stack.len() > self.max_size {
            self.undo_stack.remove(0);
        }
    }

    /// Pop an operation from the undo stack (moves to redo stack)
    pub fn pop_undo(&mut self) -> Option<EditOperation> {
        let op = self.undo_stack.pop()?;
        self.redo_stack.push(op.inverse());
        Some(op)
    }

    /// Pop an operation from the redo stack (moves to undo stack)
    pub fn pop_redo(&mut self) -> Option<EditOperation> {
        let op = self.redo_stack.pop()?;
        self.undo_stack.push(op.inverse());
        Some(op)
    }

    /// Check if undo is available
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    /// Check if redo is available
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    /// Clear all history
    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
    }

    /// Get the number of operations in the undo stack
    pub fn undo_count(&self) -> usize {
        self.undo_stack.len()
    }

    /// Get the number of operations in the redo stack
    pub fn redo_count(&self) -> usize {
        self.redo_stack.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cursor_at(line: usize, col: usize) -> Cursor {
        Cursor::new(line, col)
    }

    #[test]
    fn test_edit_operation_insert() {
        let op = EditOperation::insert(5, "hello".to_string(), cursor_at(0, 5), cursor_at(0, 10));
        assert_eq!(op.offset, 5);
        assert!(op.deleted_text.is_empty());
        assert_eq!(op.inserted_text, "hello");
    }

    #[test]
    fn test_edit_operation_inverse() {
        let op = EditOperation::replace(
            5,
            "old".to_string(),
            "new".to_string(),
            cursor_at(0, 5),
            cursor_at(0, 8),
        );
        let inv = op.inverse();
        assert_eq!(inv.deleted_text, "new");
        assert_eq!(inv.inserted_text, "old");
        assert_eq!(inv.cursors_before, op.cursors_after);
        assert_eq!(inv.cursors_after, op.cursors_before);
    }

    #[test]
    fn test_history_undo_redo() {
        let mut history = EditHistory::new();

        // Push two operations
        history.push(EditOperation::insert(
            0,
            "a".to_string(),
            cursor_at(0, 0),
            cursor_at(0, 1),
        ));
        history.push(EditOperation::insert(
            1,
            "b".to_string(),
            cursor_at(0, 1),
            cursor_at(0, 2),
        ));

        assert_eq!(history.undo_count(), 2);
        assert!(!history.can_redo());

        // Undo once - returns the original operation
        let op1 = history.pop_undo().unwrap();
        assert_eq!(op1.inserted_text, "b");
        assert!(history.can_redo());

        // Redo - returns the inverse of the inverse (back to original insert)
        // The redo stack contains the inverse (a delete of "b"),
        // and pop_redo returns that inverse and pushes its inverse back to undo
        let op2 = history.pop_redo().unwrap();
        // op2 is the inverse: it has deleted_text="b", inserted_text=""
        assert_eq!(op2.deleted_text, "b");
        assert!(!history.can_redo());
    }

    #[test]
    fn test_history_push_clears_redo() {
        let mut history = EditHistory::new();

        history.push(EditOperation::insert(
            0,
            "a".to_string(),
            cursor_at(0, 0),
            cursor_at(0, 1),
        ));
        history.pop_undo();
        assert!(history.can_redo());

        // New edit clears redo
        history.push(EditOperation::insert(
            0,
            "b".to_string(),
            cursor_at(0, 0),
            cursor_at(0, 1),
        ));
        assert!(!history.can_redo());
    }

    #[test]
    fn test_history_max_size() {
        let mut history = EditHistory::with_max_size(3);

        for i in 0..5 {
            history.push(EditOperation::insert(
                i,
                format!("{}", i),
                cursor_at(0, i),
                cursor_at(0, i + 1),
            ));
        }

        assert_eq!(history.undo_count(), 3);
    }
}
