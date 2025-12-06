//! Editor state - cursor, viewport, selections, and view-specific state

use super::document::Document;

/// Strategy for revealing the cursor when it's outside the viewport
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ScrollRevealMode {
    /// Minimal scroll: move viewport just enough to bring cursor into safe zone
    #[default]
    Minimal,
    /// Top-aligned: place cursor at the top of the safe zone (respecting top margin)
    TopAligned,
    /// Bottom-aligned: place cursor at the bottom of the safe zone (respecting bottom margin)
    BottomAligned,
    /// Centered: place cursor in the middle of the viewport
    Centered,
}

/// A position in the document (line and column)
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Position {
    /// Line number (0-indexed)
    pub line: usize,
    /// Column number (0-indexed)
    pub column: usize,
}

impl Position {
    /// Create a new position
    pub const fn new(line: usize, column: usize) -> Self {
        Self { line, column }
    }
}

/// A text selection with anchor (start) and head (cursor end)
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Selection {
    /// Where the selection started (fixed point)
    pub anchor: Position,
    /// Where the cursor is (moving point)
    pub head: Position,
}

impl Selection {
    /// Create a new empty selection at a position
    pub fn new(pos: Position) -> Self {
        Self {
            anchor: pos,
            head: pos,
        }
    }

    /// Create a selection from anchor to head
    pub fn from_anchor_head(anchor: Position, head: Position) -> Self {
        Self { anchor, head }
    }

    /// Check if selection is empty (cursor without selection)
    pub fn is_empty(&self) -> bool {
        self.anchor == self.head
    }

    /// Get the start of the selection (smaller position)
    pub fn start(&self) -> Position {
        if self.anchor <= self.head {
            self.anchor
        } else {
            self.head
        }
    }

    /// Get the end of the selection (larger position)
    pub fn end(&self) -> Position {
        if self.anchor <= self.head {
            self.head
        } else {
            self.anchor
        }
    }

    /// Check if the selection is reversed (head before anchor)
    pub fn is_reversed(&self) -> bool {
        self.head < self.anchor
    }

    /// Extend selection to new head position
    pub fn extend_to(&mut self, pos: Position) {
        self.head = pos;
    }

    /// Collapse selection to its start (both anchor and head at start)
    pub fn collapse_to_start(&mut self) {
        let s = self.start();
        self.anchor = s;
        self.head = s;
    }

    /// Collapse selection to its end (both anchor and head at end)
    pub fn collapse_to_end(&mut self) {
        let e = self.end();
        self.anchor = e;
        self.head = e;
    }

    /// Check if a position is contained within the selection
    pub fn contains(&self, pos: Position) -> bool {
        let start = self.start();
        let end = self.end();
        pos >= start && pos < end
    }
}

/// Cursor position in the document
#[derive(Debug, Clone, Copy, Default)]
pub struct Cursor {
    /// Line number (0-indexed)
    pub line: usize,
    /// Column number (0-indexed)
    pub column: usize,
    /// Desired column for vertical movement (preserves position when moving through short lines)
    pub desired_column: Option<usize>,
}

impl Cursor {
    /// Create a new cursor at position (0, 0)
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a cursor at a specific position
    pub fn at(line: usize, column: usize) -> Self {
        Self {
            line,
            column,
            desired_column: None,
        }
    }

    /// Convert to Position (without desired_column)
    pub fn to_position(&self) -> Position {
        Position::new(self.line, self.column)
    }

    /// Create from Position
    pub fn from_position(pos: Position) -> Self {
        Self::at(pos.line, pos.column)
    }

    /// Reset the desired column (called after horizontal movement)
    pub fn clear_desired_column(&mut self) {
        self.desired_column = None;
    }

    /// Set the desired column (called before vertical movement if not set)
    pub fn remember_column(&mut self) {
        if self.desired_column.is_none() {
            self.desired_column = Some(self.column);
        }
    }
}

/// Viewport state - what portion of the document is visible
#[derive(Debug, Clone)]
pub struct Viewport {
    /// First visible line (0-indexed)
    pub top_line: usize,
    /// First visible column (for horizontal scrolling)
    pub left_column: usize,
    /// Number of lines that fit in the viewport
    pub visible_lines: usize,
    /// Number of columns that fit in the viewport
    pub visible_columns: usize,
}

impl Viewport {
    /// Create a new viewport with the given dimensions
    pub fn new(visible_lines: usize, visible_columns: usize) -> Self {
        Self {
            top_line: 0,
            left_column: 0,
            visible_lines,
            visible_columns,
        }
    }
}

impl Default for Viewport {
    fn default() -> Self {
        Self::new(25, 80)
    }
}

/// State for an in-progress rectangle selection (middle mouse drag)
#[derive(Debug, Clone, Default)]
pub struct RectangleSelectionState {
    /// Whether a rectangle selection is currently active
    pub active: bool,
    /// Starting position (where mouse was pressed)
    pub start: Position,
    /// Current position (where mouse is now)
    pub current: Position,
    /// Preview cursor positions (computed during drag, shown before commit)
    pub preview_cursors: Vec<Position>,
}

impl RectangleSelectionState {
    /// Get the top-left corner of the rectangle
    pub fn top_left(&self) -> Position {
        Position::new(
            self.start.line.min(self.current.line),
            self.start.column.min(self.current.column),
        )
    }

    /// Get the bottom-right corner of the rectangle
    pub fn bottom_right(&self) -> Position {
        Position::new(
            self.start.line.max(self.current.line),
            self.start.column.max(self.current.column),
        )
    }
}

/// Editor state - view-specific state for editing a document
///
/// Supports multiple cursors and selections for future multi-cursor editing.
/// Currently, most operations work on the primary cursor (index 0).
#[derive(Debug, Clone)]
pub struct EditorState {
    /// All cursors (primary cursor is at index 0)
    pub cursors: Vec<Cursor>,
    /// Selections corresponding to each cursor (parallel to cursors)
    pub selections: Vec<Selection>,
    /// Viewport showing which portion of the document is visible
    pub viewport: Viewport,
    /// Number of lines of padding to maintain above/below cursor when scrolling
    pub scroll_padding: usize,
    /// Rectangle selection state (for middle mouse drag)
    pub rectangle_selection: RectangleSelectionState,
}

impl EditorState {
    /// Create a new editor state with default settings
    pub fn new() -> Self {
        let cursor = Cursor::new();
        let selection = Selection::new(cursor.to_position());
        Self {
            cursors: vec![cursor],
            selections: vec![selection],
            viewport: Viewport::default(),
            scroll_padding: 1, // JetBrains-style default
            rectangle_selection: RectangleSelectionState::default(),
        }
    }

    /// Create an editor state with specific viewport dimensions
    pub fn with_viewport(visible_lines: usize, visible_columns: usize) -> Self {
        let cursor = Cursor::new();
        let selection = Selection::new(cursor.to_position());
        Self {
            cursors: vec![cursor],
            selections: vec![selection],
            viewport: Viewport::new(visible_lines, visible_columns),
            scroll_padding: 1,
            rectangle_selection: RectangleSelectionState::default(),
        }
    }

    /// Get the primary cursor (read-only)
    #[inline]
    pub fn cursor(&self) -> &Cursor {
        &self.cursors[0]
    }

    /// Get the primary cursor (mutable)
    #[inline]
    pub fn cursor_mut(&mut self) -> &mut Cursor {
        &mut self.cursors[0]
    }

    /// Get the primary selection (read-only)
    #[inline]
    pub fn selection(&self) -> &Selection {
        &self.selections[0]
    }

    /// Get the primary selection (mutable)
    #[inline]
    pub fn selection_mut(&mut self) -> &mut Selection {
        &mut self.selections[0]
    }

    /// Check if there are multiple cursors
    pub fn has_multiple_cursors(&self) -> bool {
        self.cursors.len() > 1
    }

    /// Get the number of cursors
    pub fn cursor_count(&self) -> usize {
        self.cursors.len()
    }

    /// Collapse all cursors to just the primary cursor
    pub fn collapse_to_primary(&mut self) {
        self.cursors.truncate(1);
        self.selections.truncate(1);
    }

    /// Update the primary selection to match the primary cursor (for non-selection moves)
    pub fn clear_selection(&mut self) {
        let pos = self.cursors[0].to_position();
        self.selections[0] = Selection::new(pos);
    }

    /// Toggle a cursor at the given position
    /// If a cursor exists at that position, remove it (unless it's the only one)
    /// If no cursor exists there, add one
    /// Returns true if a cursor was added, false if removed
    pub fn toggle_cursor_at(&mut self, line: usize, column: usize) -> bool {
        // Check if there's already a cursor at this position
        let existing_idx = self
            .cursors
            .iter()
            .position(|c| c.line == line && c.column == column);

        if let Some(idx) = existing_idx {
            // Cursor exists - remove it if not the only one
            if self.cursors.len() > 1 {
                self.cursors.remove(idx);
                self.selections.remove(idx);
                return false;
            }
            // Can't remove the only cursor
            return false;
        }

        // No cursor at this position - add one
        let new_cursor = Cursor::at(line, column);
        let new_selection = Selection::new(Position::new(line, column));
        self.cursors.push(new_cursor);
        self.selections.push(new_selection);

        // Sort cursors by position (line, then column) to maintain order
        self.sort_cursors();

        true
    }

    /// Add a cursor at the given position (without toggle behavior)
    pub fn add_cursor_at(&mut self, line: usize, column: usize) {
        // Check if cursor already exists
        let exists = self
            .cursors
            .iter()
            .any(|c| c.line == line && c.column == column);
        if exists {
            return;
        }

        let new_cursor = Cursor::at(line, column);
        let new_selection = Selection::new(Position::new(line, column));
        self.cursors.push(new_cursor);
        self.selections.push(new_selection);

        self.sort_cursors();
    }

    /// Sort cursors by position (line, then column)
    fn sort_cursors(&mut self) {
        // Create pairs of (cursor, selection), sort by cursor position, then unzip
        let mut pairs: Vec<_> = self
            .cursors
            .iter()
            .cloned()
            .zip(self.selections.iter().cloned())
            .collect();

        pairs.sort_by(|(a, _), (b, _)| a.line.cmp(&b.line).then_with(|| a.column.cmp(&b.column)));

        self.cursors = pairs.iter().map(|(c, _)| c.clone()).collect();
        self.selections = pairs.iter().map(|(_, s)| s.clone()).collect();
    }

    /// Remove duplicate cursor positions, keeping the first occurrence
    pub fn deduplicate_cursors(&mut self) {
        use std::collections::HashSet;
        let mut seen = HashSet::new();
        let mut keep_indices = Vec::new();

        for (i, cursor) in self.cursors.iter().enumerate() {
            let key = (cursor.line, cursor.column);
            if seen.insert(key) {
                keep_indices.push(i);
            }
        }

        // Only rebuild if we removed duplicates
        if keep_indices.len() < self.cursors.len() {
            self.cursors = keep_indices
                .iter()
                .map(|&i| self.cursors[i].clone())
                .collect();
            self.selections = keep_indices
                .iter()
                .map(|&i| self.selections[i].clone())
                .collect();
        }
    }

    /// Update viewport dimensions (e.g., on window resize)
    pub fn resize_viewport(&mut self, visible_lines: usize, visible_columns: usize) {
        self.viewport.visible_lines = visible_lines;
        self.viewport.visible_columns = visible_columns;
    }

    /// Ensure the primary cursor is visible within the viewport with padding (minimal scroll)
    pub fn ensure_cursor_visible(&mut self, document: &Document) {
        self.ensure_cursor_visible_with_mode(document, ScrollRevealMode::Minimal);
    }

    /// Ensure the primary cursor is visible using the specified reveal strategy
    ///
    /// - `Minimal`: scroll just enough to bring cursor into safe zone
    /// - `TopAligned`: place cursor at top of safe zone (good for upward movement)
    /// - `BottomAligned`: place cursor at bottom of safe zone (good for downward movement)
    /// - `Centered`: place cursor in center of viewport (good for jumps/search)
    pub fn ensure_cursor_visible_with_mode(&mut self, document: &Document, mode: ScrollRevealMode) {
        let cursor = &self.cursors[0];
        let padding = self.scroll_padding;
        let total_lines = document.line_count();

        // Vertical scrolling
        if total_lines > self.viewport.visible_lines && self.viewport.visible_lines > 0 {
            let max_top = total_lines.saturating_sub(self.viewport.visible_lines);

            // Current safe-zone boundaries (use saturating_sub to avoid overflow)
            let safe_top = self.viewport.top_line + padding;
            let safe_bottom = self.viewport.top_line
                + self
                    .viewport
                    .visible_lines
                    .saturating_sub(padding)
                    .saturating_sub(1);

            let line = cursor.line;
            let off_above = line < safe_top;
            let off_below = line > safe_bottom;

            if off_above || off_below {
                self.viewport.top_line = match mode {
                    ScrollRevealMode::Minimal => {
                        if off_above {
                            // Cursor above safe zone: scroll up to put cursor at top margin
                            line.saturating_sub(padding)
                        } else {
                            // Cursor below safe zone: scroll down to put cursor at bottom margin
                            line + padding + 1 - self.viewport.visible_lines
                        }
                    }
                    ScrollRevealMode::TopAligned => {
                        // Put cursor at top of safe zone (respecting top padding)
                        line.saturating_sub(padding)
                    }
                    ScrollRevealMode::BottomAligned => {
                        // Put cursor at bottom of safe zone (respecting bottom padding)
                        (line + padding + 1).saturating_sub(self.viewport.visible_lines)
                    }
                    ScrollRevealMode::Centered => {
                        // Place cursor in the middle of the viewport
                        line.saturating_sub(self.viewport.visible_lines / 2)
                    }
                }
                .min(max_top);
            }
            // If cursor is already in safe zone, don't scroll (preserves smooth movement)
        } else {
            self.viewport.top_line = 0;
        }

        // Horizontal scrolling (always check, independent of vertical)
        const HORIZONTAL_MARGIN: usize = 4;
        let left_safe = self.viewport.left_column.saturating_add(HORIZONTAL_MARGIN);
        let right_safe = self
            .viewport
            .left_column
            .saturating_add(self.viewport.visible_columns)
            .saturating_sub(HORIZONTAL_MARGIN);

        if cursor.column < left_safe {
            // Scroll left: put cursor exactly at left safe boundary
            self.viewport.left_column = cursor.column.saturating_sub(HORIZONTAL_MARGIN);
        } else if cursor.column >= right_safe {
            // Scroll right: put cursor exactly at right safe boundary
            self.viewport.left_column = cursor
                .column
                .saturating_add(HORIZONTAL_MARGIN)
                .saturating_add(1)
                .saturating_sub(self.viewport.visible_columns);
        }
    }

    /// Set primary cursor position from buffer offset (clears selection)
    pub fn set_cursor_from_offset(&mut self, document: &Document, offset: usize) {
        self.move_cursor_to_offset(document, offset);
        self.clear_selection();
    }

    /// Move primary cursor to buffer offset without clearing selection
    pub fn move_cursor_to_offset(&mut self, document: &Document, offset: usize) {
        let (line, column) = document.offset_to_cursor(offset);
        self.cursors[0].line = line;
        self.cursors[0].column = column;
        self.cursors[0].desired_column = None;
    }

    /// Get buffer offset from primary cursor position
    pub fn cursor_offset(&self, document: &Document) -> usize {
        document.cursor_to_offset(self.cursors[0].line, self.cursors[0].column)
    }

    /// Get the length of the current line (based on primary cursor)
    pub fn current_line_length(&self, document: &Document) -> usize {
        document.line_length(self.cursors[0].line)
    }

    /// Assert cursor/selection invariants (debug builds only)
    #[cfg(debug_assertions)]
    pub fn assert_invariants(&self) {
        debug_assert!(!self.cursors.is_empty(), "Must have at least one cursor");
        debug_assert_eq!(
            self.cursors.len(),
            self.selections.len(),
            "Cursor and selection counts must match"
        );
        for (i, (cursor, selection)) in self.cursors.iter().zip(&self.selections).enumerate() {
            debug_assert_eq!(
                cursor.to_position(),
                selection.head,
                "Cursor {} position must match selection head",
                i
            );
        }
    }

    /// No-op in release builds
    #[cfg(not(debug_assertions))]
    #[inline]
    pub fn assert_invariants(&self) {}
}

impl Default for EditorState {
    fn default() -> Self {
        Self::new()
    }
}
