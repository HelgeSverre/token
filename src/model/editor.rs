//! Editor state - cursor, viewport, selections, and view-specific state

use super::document::Document;
use super::editor_area::{DocumentId, EditorId};
use crate::csv::CsvState;
use crate::util::{char_type, CharType};

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

    /// Extract selected text from document
    pub fn get_text(&self, document: &Document) -> String {
        if self.is_empty() {
            return String::new();
        }
        let start = self.start();
        let end = self.end();
        let start_offset = document.cursor_to_offset(start.line, start.column);
        let end_offset = document.cursor_to_offset(end.line, end.column);
        document.buffer.slice(start_offset..end_offset).to_string()
    }

    /// Create a selection spanning from start to end positions
    pub fn from_positions(start: Position, end: Position) -> Self {
        Self {
            anchor: start,
            head: end,
        }
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
/// Uses VISUAL columns (screen position) rather than character columns
/// so rectangle selection works consistently across lines of different lengths.
#[derive(Debug, Clone, Default)]
pub struct RectangleSelectionState {
    /// Whether a rectangle selection is currently active
    pub active: bool,
    /// Starting line
    pub start_line: usize,
    /// Starting visual column (screen position)
    pub start_visual_col: usize,
    /// Current line (where mouse is now)
    pub current_line: usize,
    /// Current visual column (screen position)
    pub current_visual_col: usize,
    /// Preview cursor positions (computed during drag, shown before commit)
    pub preview_cursors: Vec<Position>,
}

impl RectangleSelectionState {
    /// Get the top line of the rectangle
    pub fn top_line(&self) -> usize {
        self.start_line.min(self.current_line)
    }

    /// Get the bottom line of the rectangle
    pub fn bottom_line(&self) -> usize {
        self.start_line.max(self.current_line)
    }

    /// Get the left visual column of the rectangle
    pub fn left_visual_col(&self) -> usize {
        self.start_visual_col.min(self.current_visual_col)
    }

    /// Get the right visual column of the rectangle
    pub fn right_visual_col(&self) -> usize {
        self.start_visual_col.max(self.current_visual_col)
    }
}

/// Tracks occurrence selection state for Cmd+J (select next occurrence)
#[derive(Debug, Clone, Default)]
pub struct OccurrenceState {
    /// The text being searched for
    pub search_text: String,
    /// Stack of cursor indices added via Cmd+J (for undo with Shift+Cmd+J)
    pub added_cursor_indices: Vec<usize>,
    /// Last search position (byte offset) for finding "next"
    pub last_search_offset: usize,
}

/// What kind of content this tab displays
#[derive(Debug, Clone, Default)]
pub enum TabContent {
    /// Normal text/code editing (uses Document rope + ViewMode)
    #[default]
    Text,
    /// Placeholder for unsupported binary files
    BinaryPlaceholder(BinaryPlaceholderState),
}

/// State for a binary file placeholder tab
#[derive(Debug, Clone)]
pub struct BinaryPlaceholderState {
    /// Path to the binary file
    pub path: std::path::PathBuf,
    /// File size in bytes
    pub size_bytes: u64,
}

/// Allows switching between normal text editing and specialized views
/// like CSV grid mode. The underlying Document is shared.
#[derive(Debug, Clone, Default)]
pub enum ViewMode {
    /// Normal text editing mode (default)
    #[default]
    Text,
    /// CSV spreadsheet view mode
    Csv(Box<CsvState>),
    /// Image viewer mode
    Image(Box<crate::image::ImageState>),
}

impl ViewMode {
    /// Check if in CSV mode
    pub fn is_csv(&self) -> bool {
        matches!(self, ViewMode::Csv(_))
    }

    /// Get CSV state if in CSV mode
    pub fn as_csv(&self) -> Option<&CsvState> {
        match self {
            ViewMode::Csv(state) => Some(state),
            _ => None,
        }
    }

    /// Get mutable CSV state if in CSV mode
    pub fn as_csv_mut(&mut self) -> Option<&mut CsvState> {
        match self {
            ViewMode::Csv(state) => Some(state),
            _ => None,
        }
    }

    /// Check if in image mode
    pub fn is_image(&self) -> bool {
        matches!(self, ViewMode::Image(_))
    }

    /// Get image state if in image mode
    pub fn as_image(&self) -> Option<&crate::image::ImageState> {
        match self {
            ViewMode::Image(state) => Some(state),
            _ => None,
        }
    }

    /// Get mutable image state if in image mode
    pub fn as_image_mut(&mut self) -> Option<&mut crate::image::ImageState> {
        match self {
            ViewMode::Image(state) => Some(state),
            _ => None,
        }
    }
}

/// Editor state - view-specific state for editing a document
///
/// Supports multiple cursors and selections. The "active" cursor is the one
/// the user is currently focused on (for viewport scrolling, line highlighting, etc.).
/// Cursors are stored sorted by document position, but active_cursor_index
/// tracks which one is the user's focus.
#[derive(Debug, Clone)]
pub struct EditorState {
    /// Unique identifier (set when added to EditorArea)
    pub id: Option<EditorId>,
    /// The document this editor is viewing (set when added to EditorArea)
    pub document_id: Option<DocumentId>,
    /// All cursors sorted by document position (line, then column)
    pub cursors: Vec<Cursor>,
    /// Selections corresponding to each cursor (parallel to cursors)
    pub selections: Vec<Selection>,
    /// Index of the "active" cursor - the one user is focused on.
    /// This cursor drives viewport scrolling and gets primary highlighting.
    /// Valid range: 0..cursors.len()
    pub active_cursor_index: usize,
    /// Viewport showing which portion of the document is visible
    pub viewport: Viewport,
    /// Number of lines of padding to maintain above/below cursor when scrolling
    pub scroll_padding: usize,
    /// Rectangle selection state (for middle mouse drag)
    pub rectangle_selection: RectangleSelectionState,
    /// Occurrence selection state (for Cmd+J "select next occurrence")
    pub occurrence_state: Option<OccurrenceState>,
    /// Selection history stack for expand/shrink selection (Option+Up/Down)
    /// Push before expanding, pop when shrinking
    pub selection_history: Vec<Selection>,
    /// Current view mode (Text or CSV)
    pub view_mode: ViewMode,
    /// What kind of content this tab displays (text, image, binary placeholder)
    pub tab_content: TabContent,
    /// Matching bracket pair positions (if cursor is adjacent to a bracket)
    pub matched_brackets: Option<(Position, Position)>,
}

impl EditorState {
    /// Create a new editor state with default settings
    pub fn new() -> Self {
        let cursor = Cursor::new();
        let selection = Selection::new(cursor.to_position());
        Self {
            id: None,
            document_id: None,
            cursors: vec![cursor],
            selections: vec![selection],
            active_cursor_index: 0,
            viewport: Viewport::default(),
            scroll_padding: 1,
            rectangle_selection: RectangleSelectionState::default(),
            occurrence_state: None,
            selection_history: Vec::new(),
            view_mode: ViewMode::default(),
            tab_content: TabContent::default(),
            matched_brackets: None,
        }
    }

    /// Create an editor state with specific viewport dimensions
    pub fn with_viewport(visible_lines: usize, visible_columns: usize) -> Self {
        let cursor = Cursor::new();
        let selection = Selection::new(cursor.to_position());
        Self {
            id: None,
            document_id: None,
            cursors: vec![cursor],
            selections: vec![selection],
            active_cursor_index: 0,
            viewport: Viewport::new(visible_lines, visible_columns),
            scroll_padding: 1,
            rectangle_selection: RectangleSelectionState::default(),
            occurrence_state: None,
            selection_history: Vec::new(),
            view_mode: ViewMode::default(),
            tab_content: TabContent::default(),
            matched_brackets: None,
        }
    }

    /// Clear selection history (called when selection is changed by other means)
    pub fn clear_selection_history(&mut self) {
        self.selection_history.clear();
    }

    /// Get the primary cursor (index 0, top-most in document)
    #[inline]
    pub fn primary_cursor(&self) -> &Cursor {
        &self.cursors[0]
    }

    /// Get the primary cursor (mutable)
    #[inline]
    pub fn primary_cursor_mut(&mut self) -> &mut Cursor {
        &mut self.cursors[0]
    }

    /// Get the primary selection (index 0)
    #[inline]
    pub fn primary_selection(&self) -> &Selection {
        &self.selections[0]
    }

    /// Get the primary selection (mutable)
    #[inline]
    pub fn primary_selection_mut(&mut self) -> &mut Selection {
        &mut self.selections[0]
    }

    /// Get the active cursor (the one user is focused on)
    #[inline]
    pub fn active_cursor(&self) -> &Cursor {
        &self.cursors[self.active_cursor_index]
    }

    /// Get the active cursor (mutable)
    #[inline]
    pub fn active_cursor_mut(&mut self) -> &mut Cursor {
        &mut self.cursors[self.active_cursor_index]
    }

    /// Get the active selection (corresponding to active cursor)
    #[inline]
    pub fn active_selection(&self) -> &Selection {
        &self.selections[self.active_cursor_index]
    }

    /// Get the active selection (mutable)
    #[inline]
    pub fn active_selection_mut(&mut self) -> &mut Selection {
        &mut self.selections[self.active_cursor_index]
    }

    /// Set which cursor is active by index
    /// Panics if index is out of bounds
    pub fn set_active_cursor(&mut self, index: usize) {
        assert!(
            index < self.cursors.len(),
            "active cursor index out of bounds"
        );
        self.active_cursor_index = index;
    }

    /// Check if there are multiple cursors
    pub fn has_multiple_cursors(&self) -> bool {
        self.cursors.len() > 1
    }

    /// Get the number of cursors
    pub fn cursor_count(&self) -> usize {
        self.cursors.len()
    }

    /// Get the top-most cursor (smallest document position, same as primary)
    #[inline]
    pub fn top_cursor(&self) -> &Cursor {
        &self.cursors[0]
    }

    /// Get the bottom-most cursor (largest document position)
    #[inline]
    pub fn bottom_cursor(&self) -> &Cursor {
        self.cursors
            .last()
            .expect("EditorState must always have at least one cursor")
    }

    /// Get the vertical edge cursor in the given direction
    /// - `up = true` → top-most cursor (for AddCursorAbove)
    /// - `up = false` → bottom-most cursor (for AddCursorBelow)
    #[inline]
    pub fn edge_cursor_vertical(&self, up: bool) -> &Cursor {
        if up {
            self.top_cursor()
        } else {
            self.bottom_cursor()
        }
    }

    /// Collapse all cursors to just the primary cursor, clearing any selection
    pub fn collapse_to_primary(&mut self) {
        self.cursors.truncate(1);
        self.selections.truncate(1);
        self.active_cursor_index = 0;
        // Also clear the selection on the remaining cursor
        let pos = self.cursors[0].to_position();
        self.selections[0] = Selection::new(pos);
    }

    /// Update the primary selection to match the primary cursor (for non-selection moves)
    pub fn clear_selection(&mut self) {
        let pos = self.cursors[0].to_position();
        self.selections[0] = Selection::new(pos);
    }

    /// Collapse all selections so that anchor == head == cursor position for each cursor.
    /// This should be called after all non-shift cursor movements to maintain invariants.
    pub fn collapse_selections_to_cursors(&mut self) {
        for (cursor, selection) in self.cursors.iter().zip(self.selections.iter_mut()) {
            let pos = cursor.to_position();
            selection.anchor = pos;
            selection.head = pos;
        }
    }

    /// Toggle a cursor at the given position
    /// If a cursor exists at that position, remove it (unless it's the only one)
    /// If no cursor exists there, add one and make it active
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
                // Update active cursor index if we removed a cursor before it
                if idx < self.active_cursor_index {
                    self.active_cursor_index -= 1;
                } else if idx == self.active_cursor_index {
                    // Removed the active cursor - fall back to 0
                    self.active_cursor_index = 0;
                }
                return false;
            }
            // Can't remove the only cursor
            return false;
        }

        // No cursor at this position - add one and make it active
        let new_cursor = Cursor::at(line, column);
        let new_selection = Selection::new(Position::new(line, column));
        self.cursors.push(new_cursor);
        self.selections.push(new_selection);

        // Set new cursor as active before sorting (sort_cursors will track it)
        self.active_cursor_index = self.cursors.len() - 1;

        // Sort cursors by position (line, then column) to maintain order
        self.sort_cursors();

        true
    }

    /// Add a cursor at the given position (without toggle behavior)
    /// The new cursor becomes the active cursor
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

        // Set new cursor as active before sorting (sort_cursors will track it)
        self.active_cursor_index = self.cursors.len() - 1;

        self.sort_cursors();
    }

    /// Sort cursors by position (line, then column)
    fn sort_cursors(&mut self) {
        // Remember the active cursor's position before sorting
        let active_cursor_pos = self.cursors[self.active_cursor_index].to_position();

        // Create pairs of (cursor, selection, original_index), sort by cursor position
        let mut pairs: Vec<_> = self
            .cursors
            .iter()
            .cloned()
            .zip(self.selections.iter().cloned())
            .enumerate()
            .map(|(i, (c, s))| (c, s, i))
            .collect();

        pairs.sort_by(|(a, _, _), (b, _, _)| {
            a.line.cmp(&b.line).then_with(|| a.column.cmp(&b.column))
        });

        // Find the new index of the previously active cursor
        let new_active_index = pairs
            .iter()
            .position(|(c, _, _)| c.to_position() == active_cursor_pos)
            .unwrap_or(0);

        self.cursors = pairs.iter().map(|(c, _, _)| *c).collect();
        self.selections = pairs.iter().map(|(_, s, _)| *s).collect();
        self.active_cursor_index = new_active_index;
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
            // Find the new active cursor index:
            // If the active cursor is kept, find its new position
            // If the active cursor was removed (duplicate), find the surviving cursor at same position
            let active_pos = self.cursors[self.active_cursor_index].to_position();
            let new_active_index = keep_indices
                .iter()
                .position(|&i| self.cursors[i].to_position() == active_pos)
                .unwrap_or(0);

            self.cursors = keep_indices.iter().map(|&i| self.cursors[i]).collect();
            self.selections = keep_indices.iter().map(|&i| self.selections[i]).collect();
            self.active_cursor_index = new_active_index;
        }
    }

    /// Merge overlapping or touching selections into single selections.
    ///
    /// After operations like SelectWord or SelectLine with multiple cursors,
    /// some selections may overlap. This method merges them and removes
    /// the corresponding duplicate cursors.
    ///
    /// Invariants maintained:
    /// - `cursors.len() == selections.len()`
    /// - `cursors[i].to_position() == selections[i].head`
    /// - All selections are canonical (forward: anchor <= head)
    /// - `active_cursor_index` points to the merged selection containing the original active cursor
    pub fn merge_overlapping_selections(&mut self) {
        if self.selections.len() <= 1 {
            return;
        }

        // 1) Collect (start, end, original_index) for all selections
        let mut indexed: Vec<(Position, Position, usize)> = self
            .selections
            .iter()
            .enumerate()
            .map(|(i, s)| (s.start(), s.end(), i))
            .collect();

        // 2) Sort by start position, then by end position
        indexed.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));

        // 3) Sweep through and merge overlapping/touching selections
        // Track which original indices get merged into each result
        let mut merged: Vec<(Position, Position, Vec<usize>)> = Vec::new();
        for (start, end, orig_idx) in indexed {
            if let Some((_, last_end, orig_indices)) = merged.last_mut() {
                // Overlapping or touching: next.start <= current.end
                if start <= *last_end {
                    // Extend the current merged range if this one goes further
                    if end > *last_end {
                        *last_end = end;
                    }
                    orig_indices.push(orig_idx);
                    continue;
                }
            }
            merged.push((start, end, vec![orig_idx]));
        }

        // 4) Find which merged selection contains the original active cursor
        let new_active_index = merged
            .iter()
            .position(|(_, _, orig_indices)| orig_indices.contains(&self.active_cursor_index))
            .unwrap_or(0);

        // 5) Rebuild cursors and selections from merged ranges
        // Create canonical forward selections with cursor at end
        self.cursors.clear();
        self.selections.clear();

        for (start, end, _) in merged {
            self.cursors.push(Cursor::from_position(end));
            self.selections.push(Selection::from_positions(start, end));
        }

        self.active_cursor_index = new_active_index;
    }

    /// Update viewport dimensions (e.g., on window resize)
    pub fn resize_viewport(&mut self, visible_lines: usize, visible_columns: usize) {
        self.viewport.visible_lines = visible_lines;
        self.viewport.visible_columns = visible_columns;
    }

    /// Ensure the active cursor is visible within the viewport with padding (minimal scroll)
    pub fn ensure_cursor_visible(&mut self, document: &Document) {
        self.ensure_cursor_visible_with_mode(document, ScrollRevealMode::Minimal);
    }

    /// Ensure the active cursor is visible without applying scroll padding.
    ///
    /// Use this for mouse clicks where the target position is already visible on screen.
    /// Only scrolls if the cursor is completely outside the viewport bounds.
    pub fn ensure_cursor_visible_no_padding(&mut self, document: &Document) {
        let cursor = &self.cursors[self.active_cursor_index];
        let total_lines = document.line_count();

        // Vertical scrolling - only if cursor is fully outside viewport
        if total_lines > self.viewport.visible_lines && self.viewport.visible_lines > 0 {
            let max_top = total_lines.saturating_sub(self.viewport.visible_lines);
            let viewport_bottom = self
                .viewport
                .top_line
                .saturating_add(self.viewport.visible_lines)
                .saturating_sub(1);

            if cursor.line < self.viewport.top_line {
                // Cursor above viewport: scroll up to put cursor at top
                self.viewport.top_line = cursor.line.min(max_top);
            } else if cursor.line > viewport_bottom {
                // Cursor below viewport: scroll down to put cursor at bottom
                self.viewport.top_line = (cursor.line + 1)
                    .saturating_sub(self.viewport.visible_lines)
                    .min(max_top);
            }
            // Cursor within viewport: no scroll
        } else {
            self.viewport.top_line = 0;
        }

        // Horizontal scrolling (same as normal - always check)
        const HORIZONTAL_MARGIN: usize = 4;
        let left_safe = self.viewport.left_column.saturating_add(HORIZONTAL_MARGIN);
        let right_safe = self
            .viewport
            .left_column
            .saturating_add(self.viewport.visible_columns)
            .saturating_sub(HORIZONTAL_MARGIN);

        if cursor.column < left_safe {
            self.viewport.left_column = cursor.column.saturating_sub(HORIZONTAL_MARGIN);
        } else if cursor.column >= right_safe {
            self.viewport.left_column = cursor
                .column
                .saturating_add(HORIZONTAL_MARGIN)
                .saturating_add(1)
                .saturating_sub(self.viewport.visible_columns);
        }
    }

    /// Ensure the active cursor is visible using the specified reveal strategy
    ///
    /// - `Minimal`: scroll just enough to bring cursor into safe zone
    /// - `TopAligned`: place cursor at top of safe zone (good for upward movement)
    /// - `BottomAligned`: place cursor at bottom of safe zone (good for downward movement)
    /// - `Centered`: place cursor in center of viewport (good for jumps/search)
    pub fn ensure_cursor_visible_with_mode(&mut self, document: &Document, mode: ScrollRevealMode) {
        let cursor = &self.cursors[self.active_cursor_index];
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

    /// Get word under primary cursor (using char_type for boundaries)
    /// Returns (word, start_position, end_position) or None if cursor not on a word
    pub fn word_under_cursor(&self, document: &Document) -> Option<(String, Position, Position)> {
        self.word_under_cursor_at(document, 0)
    }

    /// Get word under cursor at specified index (using char_type for boundaries)
    /// Returns (word, start_position, end_position) or None if cursor not on a word
    pub fn word_under_cursor_at(
        &self,
        document: &Document,
        idx: usize,
    ) -> Option<(String, Position, Position)> {
        let cursor = &self.cursors[idx];
        let line_content = document.get_line(cursor.line)?;

        if line_content.is_empty() {
            return None;
        }

        // Remove trailing newline for character processing
        let line_content = line_content.trim_end_matches('\n');
        if line_content.is_empty() {
            return None;
        }

        // Convert to chars first, then clamp column to char count (not byte length!)
        let chars: Vec<char> = line_content.chars().collect();
        if chars.is_empty() {
            return None;
        }

        // FIX: clamp to chars.len(), not line_content.len() (which is bytes)
        let col = cursor.column.min(chars.len().saturating_sub(1));

        // Check if cursor is on a word character
        if char_type(chars[col]) != CharType::WordChar {
            return None;
        }

        // Find word boundaries using char_type
        let mut start = col;
        while start > 0 && char_type(chars[start - 1]) == CharType::WordChar {
            start -= 1;
        }

        let mut end = col;
        while end < chars.len() && char_type(chars[end]) == CharType::WordChar {
            end += 1;
        }

        if start == end {
            return None; // Cursor not on a word
        }

        let word: String = chars[start..end].iter().collect();
        Some((
            word,
            Position::new(cursor.line, start),
            Position::new(cursor.line, end),
        ))
    }

    /// Assert cursor/selection invariants (debug builds only)
    #[cfg(debug_assertions)]
    pub fn assert_invariants(&self) {
        self.assert_invariants_with_context("unknown");
    }

    /// Assert invariants with context about what triggered the check
    #[cfg(debug_assertions)]
    pub fn assert_invariants_with_context(&self, context: &str) {
        debug_assert!(
            !self.cursors.is_empty(),
            "[{}] Must have at least one cursor",
            context
        );
        debug_assert_eq!(
            self.cursors.len(),
            self.selections.len(),
            "[{}] Cursor and selection counts must match: {} cursors, {} selections",
            context,
            self.cursors.len(),
            self.selections.len()
        );
        for (i, (cursor, selection)) in self.cursors.iter().zip(&self.selections).enumerate() {
            debug_assert_eq!(
                cursor.to_position(),
                selection.head,
                "[{}] Cursor {} position ({},{}) must match selection head ({},{})",
                context,
                i,
                cursor.line,
                cursor.column,
                selection.head.line,
                selection.head.column
            );
        }
        debug_assert!(
            self.active_cursor_index < self.cursors.len(),
            "[{}] Active cursor index {} out of bounds (have {} cursors)",
            context,
            self.active_cursor_index,
            self.cursors.len()
        );
    }

    /// No-op in release builds
    #[cfg(not(debug_assertions))]
    #[inline]
    pub fn assert_invariants(&self) {}

    /// No-op in release builds
    #[cfg(not(debug_assertions))]
    #[inline]
    pub fn assert_invariants_with_context(&self, _context: &str) {}

    // =========================================================================
    // Per-cursor movement primitives (Phase 0)
    // =========================================================================

    /// Move a single cursor left by one character
    pub fn move_cursor_left_at(&mut self, doc: &Document, idx: usize) {
        let cursor = &mut self.cursors[idx];
        if cursor.column > 0 {
            cursor.column -= 1;
            cursor.desired_column = None;
        } else if cursor.line > 0 {
            cursor.line -= 1;
            cursor.column = doc.line_length(cursor.line);
            cursor.desired_column = None;
        }
    }

    /// Move a single cursor right by one character
    pub fn move_cursor_right_at(&mut self, doc: &Document, idx: usize) {
        let cursor = &mut self.cursors[idx];
        let line_len = doc.line_length(cursor.line);
        if cursor.column < line_len {
            cursor.column += 1;
            cursor.desired_column = None;
        } else if cursor.line < doc.line_count().saturating_sub(1) {
            cursor.line += 1;
            cursor.column = 0;
            cursor.desired_column = None;
        }
    }

    /// Move a single cursor up by one line
    pub fn move_cursor_up_at(&mut self, doc: &Document, idx: usize) {
        let cursor = &mut self.cursors[idx];
        if cursor.line > 0 {
            cursor.line -= 1;
            let desired = cursor.desired_column.unwrap_or(cursor.column);
            let line_len = doc.line_length(cursor.line);
            cursor.column = desired.min(line_len);
            cursor.desired_column = Some(desired);
        }
    }

    /// Move a single cursor down by one line
    pub fn move_cursor_down_at(&mut self, doc: &Document, idx: usize) {
        let cursor = &mut self.cursors[idx];
        if cursor.line < doc.line_count().saturating_sub(1) {
            cursor.line += 1;
            let desired = cursor.desired_column.unwrap_or(cursor.column);
            let line_len = doc.line_length(cursor.line);
            cursor.column = desired.min(line_len);
            cursor.desired_column = Some(desired);
        }
    }

    /// Move a single cursor to line start (smart: first non-ws or column 0)
    pub fn move_cursor_line_start_at(&mut self, doc: &Document, idx: usize) {
        let cursor = &mut self.cursors[idx];
        let first_non_ws = doc.first_non_whitespace_column(cursor.line);
        if cursor.column == first_non_ws {
            cursor.column = 0;
        } else {
            cursor.column = first_non_ws;
        }
        cursor.desired_column = None;
    }

    /// Move a single cursor to line end (smart: last non-ws or line end)
    pub fn move_cursor_line_end_at(&mut self, doc: &Document, idx: usize) {
        let cursor = &mut self.cursors[idx];
        let line_len = doc.line_length(cursor.line);
        let last_non_ws = doc.last_non_whitespace_column(cursor.line);
        if cursor.column == last_non_ws {
            cursor.column = line_len;
        } else {
            cursor.column = last_non_ws;
        }
        cursor.desired_column = None;
    }

    /// Move a single cursor to document start
    pub fn move_cursor_document_start_at(&mut self, idx: usize) {
        let cursor = &mut self.cursors[idx];
        cursor.line = 0;
        cursor.column = 0;
        cursor.desired_column = None;
    }

    /// Move a single cursor to document end
    pub fn move_cursor_document_end_at(&mut self, doc: &Document, idx: usize) {
        let cursor = &mut self.cursors[idx];
        cursor.line = doc.line_count().saturating_sub(1);
        cursor.column = doc.line_length(cursor.line);
        cursor.desired_column = None;
    }

    /// Move a single cursor up by `jump` lines (for page up)
    pub fn page_up_at(&mut self, doc: &Document, jump: usize, idx: usize) {
        let cursor = &mut self.cursors[idx];
        cursor.line = cursor.line.saturating_sub(jump);
        let desired = cursor.desired_column.unwrap_or(cursor.column);
        let line_len = doc.line_length(cursor.line);
        cursor.column = desired.min(line_len);
        cursor.desired_column = Some(desired);
    }

    /// Move a single cursor down by `jump` lines (for page down)
    pub fn page_down_at(&mut self, doc: &Document, jump: usize, idx: usize) {
        let cursor = &mut self.cursors[idx];
        let max_line = doc.line_count().saturating_sub(1);
        cursor.line = (cursor.line + jump).min(max_line);
        let desired = cursor.desired_column.unwrap_or(cursor.column);
        let line_len = doc.line_length(cursor.line);
        cursor.column = desired.min(line_len);
        cursor.desired_column = Some(desired);
    }

    /// Move a single cursor one word left
    pub fn move_cursor_word_left_at(&mut self, doc: &Document, idx: usize) {
        let cursor = &self.cursors[idx];
        let pos = doc.cursor_to_offset(cursor.line, cursor.column);
        if pos == 0 {
            return;
        }

        let text: String = doc.buffer.slice(..pos).chars().collect();
        let chars: Vec<char> = text.chars().collect();
        let mut i = chars.len();

        if i > 0 {
            let current_type = char_type(chars[i - 1]);
            while i > 0 && char_type(chars[i - 1]) == current_type {
                i -= 1;
            }
        }

        let (line, column) = doc.offset_to_cursor(i);
        let cursor = &mut self.cursors[idx];
        cursor.line = line;
        cursor.column = column;
        cursor.desired_column = None;
    }

    /// Move a single cursor one word right
    pub fn move_cursor_word_right_at(&mut self, doc: &Document, idx: usize) {
        let cursor = &self.cursors[idx];
        let pos = doc.cursor_to_offset(cursor.line, cursor.column);
        let total_chars = doc.buffer.len_chars();
        if pos >= total_chars {
            return;
        }

        let text: String = doc.buffer.slice(pos..).chars().collect();
        let chars: Vec<char> = text.chars().collect();
        let mut i = 0;

        if !chars.is_empty() {
            let current_type = char_type(chars[0]);
            while i < chars.len() && char_type(chars[i]) == current_type {
                i += 1;
            }
        }

        let new_pos = pos + i;
        let (line, column) = doc.offset_to_cursor(new_pos);
        let cursor = &mut self.cursors[idx];
        cursor.line = line;
        cursor.column = column;
        cursor.desired_column = None;
    }

    // =========================================================================
    // All-cursors movement wrappers (Phase 1)
    // =========================================================================

    /// Apply a cursor movement operation to all cursors, then deduplicate.
    ///
    /// This is the base helper for multi-cursor operations. The closure receives
    /// the cursor index and should perform the movement for that cursor.
    fn for_each_cursor<F>(&mut self, mut f: F)
    where
        F: FnMut(&mut Self, usize),
    {
        for i in 0..self.cursors.len() {
            f(self, i);
        }
        self.deduplicate_cursors();
    }

    /// Apply a cursor movement operation to all cursors and extend selections.
    ///
    /// After moving each cursor, updates the selection head to the new position.
    fn for_each_cursor_extend_selection<F>(&mut self, mut f: F)
    where
        F: FnMut(&mut Self, usize),
    {
        for i in 0..self.cursors.len() {
            f(self, i);
            let pos = self.cursors[i].to_position();
            self.selections[i].head = pos;
        }
        self.deduplicate_cursors();
    }

    /// Move all cursors left
    /// If a cursor has a selection, collapse to selection start instead of moving by 1 char
    pub fn move_all_cursors_left(&mut self, doc: &Document) {
        let len = self.cursors.len();
        for i in 0..len {
            if self.selections[i].is_empty() {
                self.move_cursor_left_at(doc, i);
            } else {
                let start = self.selections[i].start();
                let cursor = &mut self.cursors[i];
                cursor.line = start.line;
                cursor.column = start.column;
                cursor.desired_column = None;
                self.selections[i].anchor = start;
                self.selections[i].head = start;
            }
        }
        self.deduplicate_cursors();
    }

    /// Move all cursors right
    /// If a cursor has a selection, collapse to selection end instead of moving by 1 char
    pub fn move_all_cursors_right(&mut self, doc: &Document) {
        let len = self.cursors.len();
        for i in 0..len {
            if self.selections[i].is_empty() {
                self.move_cursor_right_at(doc, i);
            } else {
                let end = self.selections[i].end();
                let cursor = &mut self.cursors[i];
                cursor.line = end.line;
                cursor.column = end.column;
                cursor.desired_column = None;
                self.selections[i].anchor = end;
                self.selections[i].head = end;
            }
        }
        self.deduplicate_cursors();
    }

    /// Move all cursors up
    pub fn move_all_cursors_up(&mut self, doc: &Document) {
        self.for_each_cursor(|s, i| s.move_cursor_up_at(doc, i));
    }

    /// Move all cursors down
    pub fn move_all_cursors_down(&mut self, doc: &Document) {
        self.for_each_cursor(|s, i| s.move_cursor_down_at(doc, i));
    }

    /// Move all cursors to line start
    pub fn move_all_cursors_line_start(&mut self, doc: &Document) {
        self.for_each_cursor(|s, i| s.move_cursor_line_start_at(doc, i));
    }

    /// Move all cursors to line end
    pub fn move_all_cursors_line_end(&mut self, doc: &Document) {
        self.for_each_cursor(|s, i| s.move_cursor_line_end_at(doc, i));
    }

    /// Move all cursors to document start
    pub fn move_all_cursors_document_start(&mut self) {
        self.for_each_cursor(|s, i| s.move_cursor_document_start_at(i));
    }

    /// Move all cursors to document end
    pub fn move_all_cursors_document_end(&mut self, doc: &Document) {
        self.for_each_cursor(|s, i| s.move_cursor_document_end_at(doc, i));
    }

    /// Move all cursors word left
    pub fn move_all_cursors_word_left(&mut self, doc: &Document) {
        self.for_each_cursor(|s, i| s.move_cursor_word_left_at(doc, i));
    }

    /// Move all cursors word right
    pub fn move_all_cursors_word_right(&mut self, doc: &Document) {
        self.for_each_cursor(|s, i| s.move_cursor_word_right_at(doc, i));
    }

    /// Page up all cursors
    pub fn page_up_all_cursors(&mut self, doc: &Document, jump: usize) {
        self.for_each_cursor(|s, i| s.page_up_at(doc, jump, i));
    }

    /// Page down all cursors
    pub fn page_down_all_cursors(&mut self, doc: &Document, jump: usize) {
        self.for_each_cursor(|s, i| s.page_down_at(doc, jump, i));
    }

    // =========================================================================
    // Selection movement helpers (Phase 3)
    // =========================================================================

    /// Move all cursors left and extend selections
    pub fn move_all_cursors_left_with_selection(&mut self, doc: &Document) {
        self.for_each_cursor_extend_selection(|s, i| s.move_cursor_left_at(doc, i));
    }

    /// Move all cursors right and extend selections
    pub fn move_all_cursors_right_with_selection(&mut self, doc: &Document) {
        self.for_each_cursor_extend_selection(|s, i| s.move_cursor_right_at(doc, i));
    }

    /// Move all cursors up and extend selections
    pub fn move_all_cursors_up_with_selection(&mut self, doc: &Document) {
        self.for_each_cursor_extend_selection(|s, i| s.move_cursor_up_at(doc, i));
    }

    /// Move all cursors down and extend selections
    pub fn move_all_cursors_down_with_selection(&mut self, doc: &Document) {
        self.for_each_cursor_extend_selection(|s, i| s.move_cursor_down_at(doc, i));
    }

    /// Move all cursors to line start and extend selections
    pub fn move_all_cursors_line_start_with_selection(&mut self, doc: &Document) {
        self.for_each_cursor_extend_selection(|s, i| s.move_cursor_line_start_at(doc, i));
    }

    /// Move all cursors to line end and extend selections
    pub fn move_all_cursors_line_end_with_selection(&mut self, doc: &Document) {
        self.for_each_cursor_extend_selection(|s, i| s.move_cursor_line_end_at(doc, i));
    }

    /// Move all cursors to document start and extend selections
    pub fn move_all_cursors_document_start_with_selection(&mut self) {
        self.for_each_cursor_extend_selection(|s, i| s.move_cursor_document_start_at(i));
    }

    /// Move all cursors to document end and extend selections
    pub fn move_all_cursors_document_end_with_selection(&mut self, doc: &Document) {
        self.for_each_cursor_extend_selection(|s, i| s.move_cursor_document_end_at(doc, i));
    }

    /// Move all cursors word left and extend selections
    pub fn move_all_cursors_word_left_with_selection(&mut self, doc: &Document) {
        self.for_each_cursor_extend_selection(|s, i| s.move_cursor_word_left_at(doc, i));
    }

    /// Move all cursors word right and extend selections
    pub fn move_all_cursors_word_right_with_selection(&mut self, doc: &Document) {
        self.for_each_cursor_extend_selection(|s, i| s.move_cursor_word_right_at(doc, i));
    }

    /// Page up all cursors and extend selections
    pub fn page_up_all_cursors_with_selection(&mut self, doc: &Document, jump: usize) {
        self.for_each_cursor_extend_selection(|s, i| s.page_up_at(doc, jump, i));
    }

    /// Page down all cursors and extend selections
    pub fn page_down_all_cursors_with_selection(&mut self, doc: &Document, jump: usize) {
        self.for_each_cursor_extend_selection(|s, i| s.page_down_at(doc, jump, i));
    }
}

impl Default for EditorState {
    fn default() -> Self {
        Self::new()
    }
}
