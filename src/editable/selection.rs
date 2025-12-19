//! Selection types for the unified text editing system.

use super::cursor::Position;

/// A text selection with anchor (start point) and head (cursor position).
/// The anchor stays fixed while the head moves during selection extension.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Selection {
    /// Where the selection started (fixed point)
    pub anchor: Position,
    /// Where the cursor is (moving point)
    pub head: Position,
}

impl Selection {
    pub fn new(anchor: Position, head: Position) -> Self {
        Self { anchor, head }
    }

    /// Create a collapsed selection (cursor with no selection)
    pub fn collapsed(pos: Position) -> Self {
        Self {
            anchor: pos,
            head: pos,
        }
    }

    /// Check if selection is empty (anchor == head)
    pub fn is_empty(&self) -> bool {
        self.anchor == self.head
    }

    /// Get the start position (minimum of anchor and head)
    pub fn start(&self) -> Position {
        if self.anchor <= self.head {
            self.anchor
        } else {
            self.head
        }
    }

    /// Get the end position (maximum of anchor and head)
    pub fn end(&self) -> Position {
        if self.anchor >= self.head {
            self.anchor
        } else {
            self.head
        }
    }

    /// Check if selection is reversed (head before anchor)
    pub fn is_reversed(&self) -> bool {
        self.head < self.anchor
    }

    /// Extend selection to new head position
    pub fn extend_to(&mut self, pos: Position) {
        self.head = pos;
    }

    /// Collapse selection to head position
    pub fn collapse(&mut self) {
        self.anchor = self.head;
    }

    /// Collapse selection to start position
    pub fn collapse_to_start(&mut self) {
        let start = self.start();
        self.anchor = start;
        self.head = start;
    }

    /// Collapse selection to end position
    pub fn collapse_to_end(&mut self) {
        let end = self.end();
        self.anchor = end;
        self.head = end;
    }

    /// Check if a position is within this selection
    pub fn contains(&self, pos: Position) -> bool {
        pos >= self.start() && pos < self.end()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_selection_collapsed() {
        let sel = Selection::collapsed(Position::new(1, 5));
        assert!(sel.is_empty());
        assert_eq!(sel.anchor, sel.head);
    }

    #[test]
    fn test_selection_start_end() {
        // Forward selection
        let forward = Selection::new(Position::new(0, 0), Position::new(0, 5));
        assert_eq!(forward.start(), Position::new(0, 0));
        assert_eq!(forward.end(), Position::new(0, 5));
        assert!(!forward.is_reversed());

        // Backward selection
        let backward = Selection::new(Position::new(0, 5), Position::new(0, 0));
        assert_eq!(backward.start(), Position::new(0, 0));
        assert_eq!(backward.end(), Position::new(0, 5));
        assert!(backward.is_reversed());
    }

    #[test]
    fn test_selection_extend() {
        let mut sel = Selection::collapsed(Position::new(0, 0));
        sel.extend_to(Position::new(0, 10));
        assert_eq!(sel.anchor, Position::new(0, 0));
        assert_eq!(sel.head, Position::new(0, 10));
    }

    #[test]
    fn test_selection_collapse() {
        let mut sel = Selection::new(Position::new(0, 0), Position::new(0, 10));
        sel.collapse_to_end();
        assert!(sel.is_empty());
        assert_eq!(sel.head, Position::new(0, 10));

        let mut sel2 = Selection::new(Position::new(0, 0), Position::new(0, 10));
        sel2.collapse_to_start();
        assert!(sel2.is_empty());
        assert_eq!(sel2.head, Position::new(0, 0));
    }

    #[test]
    fn test_selection_contains() {
        let sel = Selection::new(Position::new(0, 2), Position::new(0, 8));
        assert!(!sel.contains(Position::new(0, 1)));
        assert!(sel.contains(Position::new(0, 2)));
        assert!(sel.contains(Position::new(0, 5)));
        assert!(sel.contains(Position::new(0, 7)));
        assert!(!sel.contains(Position::new(0, 8))); // End is exclusive
    }
}
