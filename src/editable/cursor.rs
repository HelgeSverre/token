//! Cursor and position types for the unified text editing system.

/// A position in the text buffer (line and column, both 0-indexed).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Position {
    pub line: usize,
    pub column: usize,
}

impl Position {
    pub const fn new(line: usize, column: usize) -> Self {
        Self { line, column }
    }

    pub const fn zero() -> Self {
        Self { line: 0, column: 0 }
    }
}

/// A cursor in the text buffer with optional desired column for vertical movement.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Cursor {
    pub line: usize,
    pub column: usize,
    /// Desired column for vertical movement.
    /// When moving up/down through lines of varying length, this preserves
    /// the "intended" column position even when a shorter line is traversed.
    pub desired_column: Option<usize>,
}

impl Cursor {
    pub const fn new(line: usize, column: usize) -> Self {
        Self {
            line,
            column,
            desired_column: None,
        }
    }

    pub fn at_position(pos: Position) -> Self {
        Self::new(pos.line, pos.column)
    }

    pub const fn to_position(&self) -> Position {
        Position::new(self.line, self.column)
    }

    /// Clear desired column (call after horizontal movement)
    pub fn clear_desired_column(&mut self) {
        self.desired_column = None;
    }

    /// Set desired column to current column (call before vertical movement)
    pub fn set_desired_column(&mut self) {
        if self.desired_column.is_none() {
            self.desired_column = Some(self.column);
        }
    }

    /// Get the effective column for positioning (uses desired_column if set)
    pub fn effective_column(&self) -> usize {
        self.desired_column.unwrap_or(self.column)
    }
}

impl From<Position> for Cursor {
    fn from(pos: Position) -> Self {
        Self::at_position(pos)
    }
}

impl From<Cursor> for Position {
    fn from(cursor: Cursor) -> Self {
        cursor.to_position()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_position_ordering() {
        let a = Position::new(0, 5);
        let b = Position::new(1, 0);
        let c = Position::new(1, 3);

        assert!(a < b);
        assert!(b < c);
        assert!(a < c);
    }

    #[test]
    fn test_cursor_desired_column() {
        let mut cursor = Cursor::new(5, 10);
        assert_eq!(cursor.effective_column(), 10);

        cursor.set_desired_column();
        assert_eq!(cursor.desired_column, Some(10));

        cursor.column = 5; // Moved to shorter line
        assert_eq!(cursor.effective_column(), 10); // Still wants column 10

        cursor.clear_desired_column();
        assert_eq!(cursor.effective_column(), 5);
    }

    #[test]
    fn test_cursor_position_conversions() {
        let pos = Position::new(3, 7);
        let cursor = Cursor::from(pos);
        assert_eq!(cursor.line, 3);
        assert_eq!(cursor.column, 7);

        let pos2: Position = cursor.into();
        assert_eq!(pos2, pos);
    }
}
