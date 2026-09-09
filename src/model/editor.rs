//! Editor state - cursor, viewport, selections, and view-specific state

use super::document::Document;
use super::editor_area::{DocumentId, EditorId};
use crate::csv::CsvState;
use crate::util::text::{char_type, CharType};
use crate::wrap::{WrapCache, WrapSegment};

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

pub use crate::editable::{Cursor, Position, Selection};

impl Selection {
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
}

/// Complete editor selection state saved by expand/shrink selection.
#[derive(Debug, Clone)]
pub struct SelectionSnapshot {
    pub cursors: Vec<Cursor>,
    pub selections: Vec<Selection>,
    pub active_cursor_index: usize,
}

/// Viewport state - what portion of the document is visible
#[derive(Debug, Clone)]
pub struct Viewport {
    pub animation: Option<super::scroll::ScrollAnimation>,
    /// Measured pixel extent and displacement within the first visible cell.
    pub pixels: super::scroll::PixelViewport,
    /// First visible visual row (logical line when wrapping is disabled)
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
            animation: None,
            pixels: super::scroll::PixelViewport::new(visible_columns, visible_lines),
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

/// Shared mapping between visual viewport rows and logical document positions.
/// Scroll offsets and reveal targets are visual rows; document-facing queries
/// explicitly convert through the pane's wrap cache.
#[derive(Debug, Clone, Copy)]
pub struct TextViewportMap<'a> {
    pixels: super::scroll::PixelViewport,
    top_line: usize,
    left_column: usize,
    visible_lines: usize,
    line_count: usize,
    wrap_cache: Option<&'a WrapCache>,
    folds: Option<&'a crate::folding::FoldProjection>,
    ghost: Option<&'a super::GhostProjection>,
}

impl<'a> TextViewportMap<'a> {
    pub fn new(viewport: &Viewport, line_count: usize) -> Self {
        Self {
            pixels: viewport.pixels,
            top_line: viewport.top_line,
            left_column: viewport.left_column,
            visible_lines: viewport.visible_lines,
            line_count,
            wrap_cache: None,
            folds: None,
            ghost: None,
        }
    }

    pub fn wrapped(viewport: &Viewport, line_count: usize, wrap_cache: &'a WrapCache) -> Self {
        Self {
            pixels: super::scroll::PixelViewport {
                x: super::scroll::PixelAxis {
                    offset: 0.0,
                    ..viewport.pixels.x
                },
                ..viewport.pixels
            },
            top_line: viewport.top_line,
            left_column: 0,
            visible_lines: viewport.visible_lines,
            line_count,
            wrap_cache: Some(wrap_cache),
            folds: None,
            ghost: None,
        }
    }

    #[inline]
    pub fn top_line(&self) -> usize {
        self.top_line
    }

    #[inline]
    pub fn left_column(&self) -> usize {
        self.left_column
    }

    #[inline]
    pub fn visible_lines(&self) -> usize {
        self.visible_lines
    }

    /// Visual rows intersecting the viewport, including partially clipped edges.
    pub fn drawn_rows(&self) -> usize {
        let mut axis = self.pixels.y;
        axis.set_visible_cells(self.visible_lines);
        axis.drawn_count()
    }

    pub fn row_pixel_offset(&self, visible_row: usize, line_height: f64) -> f64 {
        super::scroll::PixelAxis {
            unit: line_height,
            ..self.pixels.y
        }
        .cell_origin(visible_row)
    }

    pub fn column_pixel_offset(&self, column: usize, char_width: f32) -> f64 {
        let cells = if column >= self.left_column {
            (column - self.left_column) as f64
        } else {
            -((self.left_column - column) as f64)
        };
        cells * char_width as f64 - self.pixels.x.offset.round()
    }

    pub fn visible_row_at_pixel(&self, y: f64, line_height: f64) -> usize {
        if line_height <= 0.0 {
            return 0;
        }
        super::scroll::PixelAxis {
            unit: line_height,
            ..self.pixels.y
        }
        .cell_at_pixel(y)
    }

    #[inline]
    pub fn last_line(&self) -> usize {
        self.row_count().saturating_sub(1)
    }

    #[inline]
    pub fn row_count(&self) -> usize {
        let base = self
            .wrap_cache
            .map_or(self.line_count, WrapCache::total_visual_lines);
        let base = self.folds.map_or(base, |folds| folds.row_count(base));
        self.ghost.map_or(base, |ghost| {
            base - self.base_line_rows(ghost.anchor.line) + ghost.rows.len()
        })
    }

    fn base_line_row(&self, line: usize) -> usize {
        let row = self
            .wrap_cache
            .map_or(line, |cache| cache.logical_line_to_visual(line));
        self.folds.map_or(row, |folds| folds.project(row))
    }

    fn base_line_rows(&self, line: usize) -> usize {
        self.wrap_cache
            .map_or(1, |cache| cache.visual_line_count(line))
    }

    /// Remove the insertion's row displacement to address the ordinary cache.
    fn base_row(&self, row: usize) -> usize {
        let row = self.ghost.map_or(row, |ghost| {
            let start = self.base_line_row(ghost.anchor.line);
            if row < start {
                row
            } else if row < start + ghost.rows.len() {
                start
            } else {
                row - ghost.rows.len() + self.base_line_rows(ghost.anchor.line)
            }
        });
        self.folds.map_or(row, |folds| folds.unproject(row))
    }

    fn projected_row(&self, base: usize) -> usize {
        let base = self.folds.map_or(base, |folds| folds.project(base));
        let Some(ghost) = self.ghost else {
            return base;
        };
        let end = self.base_line_row(ghost.anchor.line) + self.base_line_rows(ghost.anchor.line);
        if base < end {
            base
        } else {
            base - self.base_line_rows(ghost.anchor.line) + ghost.rows.len()
        }
    }

    pub(crate) fn ghost_row_for_visible_row(
        &self,
        visible_row: usize,
    ) -> Option<&'a super::GhostRow> {
        let ghost = self.ghost?;
        let index = self
            .top_line
            .saturating_add(visible_row)
            .checked_sub(self.base_line_row(ghost.anchor.line))?;
        ghost.rows.get(index)
    }

    #[inline]
    pub fn end_line(&self) -> usize {
        self.top_line
            .saturating_add(self.drawn_rows())
            .min(self.row_count())
    }

    #[inline]
    pub fn bottom_line(&self) -> usize {
        self.top_line
            .saturating_add(self.visible_lines.saturating_sub(1))
            .min(self.last_line())
    }

    #[inline]
    pub fn doc_line_for_visible_row(&self, visible_row: usize) -> Option<usize> {
        let visual_line = self.top_line.saturating_add(visible_row);
        if visual_line >= self.row_count() {
            return None;
        }
        let base = self.base_row(visual_line);
        Some(
            self.wrap_cache
                .map_or(base, |cache| cache.visual_line_to_logical(base)),
        )
    }

    #[inline]
    pub fn visible_row_for_doc_line(&self, doc_line: usize) -> Option<usize> {
        if doc_line >= self.line_count || self.hidden_header(doc_line).is_some() {
            return None;
        }
        let visual_line = self.visual_line_for_position(doc_line, 0);
        let rows = self
            .ghost
            .filter(|g| g.anchor.line == doc_line)
            .map_or_else(|| self.base_line_rows(doc_line), |g| g.rows.len());
        let last_visual_line = visual_line + rows.saturating_sub(1);
        if last_visual_line < self.top_line {
            return None;
        }
        let visible_row = visual_line.saturating_sub(self.top_line);
        (visible_row < self.drawn_rows()).then_some(visible_row)
    }

    /// Logical lines intersecting the visible rows, including partial lines.
    pub fn visible_doc_lines(&self) -> std::ops::Range<usize> {
        let Some(first) = self.doc_line_for_visible_row(0) else {
            return self.line_count..self.line_count;
        };
        if self.drawn_rows() == 0 {
            return first..first;
        }
        let last = self
            .doc_line_for_visible_row(self.drawn_rows() - 1)
            .unwrap_or(self.line_count.saturating_sub(1));
        first..last.saturating_add(1)
    }

    /// Contiguous visible document ranges, excluding hidden bodies. Decoration
    /// producers can query their indexes without visiting matches inside folds.
    pub fn visible_doc_ranges(&self) -> Vec<std::ops::Range<usize>> {
        if self
            .folds
            .is_none_or(crate::folding::FoldProjection::is_empty)
        {
            return vec![self.visible_doc_lines()];
        }
        let mut ranges: Vec<std::ops::Range<usize>> = Vec::new();
        for row in 0..self.drawn_rows() {
            let Some(line) = self.doc_line_for_visible_row(row) else {
                break;
            };
            if let Some(last) = ranges.last_mut() {
                if line < last.end {
                    continue;
                }
                if line == last.end {
                    last.end += 1;
                    continue;
                }
            }
            ranges.push(line..line + 1);
        }
        ranges
    }

    /// Row and tab-expanded column for a logical position.
    pub fn display_position(
        &self,
        document: &Document,
        line: usize,
        column: usize,
    ) -> (usize, usize) {
        if let Some(header) = self.hidden_header(line) {
            return self.display_position(document, header, document.line_length(header));
        }
        if let Some(ghost) = self.ghost.filter(|g| g.anchor.line == line) {
            let (row, column) = ghost.display_position(column);
            return (self.base_line_row(line) + row, column);
        }
        let row = self.visual_line_for_position(line, column);
        let start = self
            .wrap_cache
            .and_then(|cache| cache.segment_for_visual_line(self.base_row(row)))
            .map_or(0, |segment| segment.start_col);
        let text = document.get_line_cow(line).unwrap_or_default();
        (
            row,
            document
                .text_settings
                .tabs
                .char_col_to_visual_col_from(&text, start, column),
        )
    }

    /// Logical position on a global visual row at the supplied display column.
    /// Internal wrap boundaries belong to the following row, so clicks and
    /// vertical movement on a row's right margin stop before that boundary.
    pub fn position_at_display_column(
        &self,
        document: &Document,
        row: usize,
        column: usize,
    ) -> Position {
        let row = row.min(self.last_line());
        if let Some(ghost) = self.ghost {
            let start = self.base_line_row(ghost.anchor.line);
            if (start..start + ghost.rows.len()).contains(&row) {
                return Position::new(ghost.anchor.line, ghost.source_column(row - start, column));
            }
        }
        let row = self.base_row(row);
        let (line, start, end) = if let Some(cache) = self.wrap_cache {
            let line = cache.visual_line_to_logical(row);
            let segment = cache.segment_for_visual_line(row);
            let start = segment.map_or(0, |s| s.start_col);
            let mut end = segment.map_or(0, |s| s.end_col());
            if end < document.line_length(line) {
                end = end.saturating_sub(1).max(start);
            }
            (line, start, end)
        } else {
            (row, 0, document.line_length(row))
        };
        let text = document.get_line_cow(line).unwrap_or_default();
        Position::new(
            line,
            document
                .text_settings
                .tabs
                .visual_col_to_char_col_from(&text, start, end, column),
        )
    }

    pub fn position_for_pixel(
        &self,
        document: &Document,
        x_offset: f64,
        y_offset: f64,
        char_width: f32,
        line_height: f64,
    ) -> Position {
        let visible_row = self.visible_row_at_pixel(y_offset, line_height);
        self.position_at_display_column(
            document,
            self.top_line.saturating_add(visible_row),
            self.visual_column_for_x_offset(x_offset, char_width),
        )
    }

    pub fn visible_row_for_position(&self, line: usize, column: usize) -> Option<usize> {
        if line >= self.line_count || self.hidden_header(line).is_some() {
            return None;
        }
        let visual_line = self.visual_line_for_position(line, column);
        let visible_row = visual_line.checked_sub(self.top_line)?;
        (visible_row < self.drawn_rows()).then_some(visible_row)
    }

    #[inline]
    pub fn visual_line_for_position(&self, line: usize, column: usize) -> usize {
        if let Some(ghost) = self.ghost.filter(|g| g.anchor.line == line) {
            return self.base_line_row(line) + ghost.display_position(column).0;
        }
        self.projected_row(
            self.wrap_cache
                .map_or(line, |cache| cache.logical_to_visual(line, column).0),
        )
    }

    /// Distinguish an aggregated hidden location from a real visible caret row.
    pub fn hidden_header(&self, line: usize) -> Option<usize> {
        self.folds.and_then(|folds| folds.hidden_header(line))
    }

    pub fn segment_for_visible_row(
        &self,
        document: &Document,
        visible_row: usize,
    ) -> Option<WrapSegment> {
        let visual_line = self.top_line.saturating_add(visible_row);
        if visual_line >= self.row_count() {
            return None;
        }
        if let Some(row) = self.ghost_row_for_visible_row(visible_row) {
            let anchor = self.ghost?.anchor;
            let start = row
                .sources
                .iter()
                .flatten()
                .next()
                .map_or(anchor.column, |s| s.columns.start);
            let end = row
                .sources
                .iter()
                .flatten()
                .last()
                .map_or(start, |s| s.columns.end);
            return Some(WrapSegment {
                start_col: start,
                len: end - start,
                visual_line,
                is_continuation: visual_line != self.base_line_row(anchor.line),
            });
        }
        let base = self.base_row(visual_line);
        self.wrap_cache
            .and_then(|cache| cache.segment_for_visual_line(base).copied())
            .map(|segment| WrapSegment {
                visual_line,
                ..segment
            })
            .or_else(|| {
                Some(WrapSegment {
                    start_col: 0,
                    len: document.line_length(base),
                    visual_line,
                    is_continuation: false,
                })
            })
    }

    #[inline]
    pub fn contains_doc_line(&self, doc_line: usize) -> bool {
        self.visible_row_for_doc_line(doc_line).is_some()
    }

    #[inline]
    pub fn doc_line_for_pixel_y(&self, adjusted_y: f64, line_height: f64) -> usize {
        if line_height <= 0.0 {
            return self
                .doc_line_for_visible_row(0)
                .unwrap_or_else(|| self.line_count.saturating_sub(1));
        }

        let visible_row = self.visible_row_at_pixel(adjusted_y, line_height);
        self.doc_line_for_visible_row(visible_row)
            .unwrap_or_else(|| self.line_count.saturating_sub(1))
    }

    #[inline]
    pub fn visual_column_for_x_offset(&self, x_offset: f64, char_width: f32) -> usize {
        let x_offset = x_offset + self.pixels.x.offset.round();
        if x_offset > 0.0 && char_width > 0.0 {
            self.left_column
                .saturating_add((x_offset / char_width as f64).round() as usize)
        } else {
            self.left_column
        }
    }
}

/// State for an in-progress rectangle selection (middle mouse drag)
/// Uses VISUAL columns (screen position) rather than character columns
/// so rectangle selection works consistently across lines of different lengths.
#[derive(Debug, Clone, Default)]
pub struct RectangleSelectionState {
    /// Whether a rectangle selection is currently active
    pub active: bool,
    /// Starting visual row (logical line when wrapping is disabled)
    pub start_line: usize,
    /// Starting visual column (screen position)
    pub start_visual_col: usize,
    /// Current visual row (where mouse is now)
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

/// Document movement targets; modal text fields retain their own editing engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CursorMovement {
    Left,
    Right,
    Up,
    Down,
    LineStart,
    LineEnd,
    DocumentStart,
    DocumentEnd,
    WordLeft,
    WordRight,
    PageUp(usize),
    PageDown(usize),
    /// Unsupported vertical word movement is a legacy no-op.
    Stay,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MovementSelection {
    Move,
    Extend,
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
    pub selection_history: Vec<SelectionSnapshot>,
    /// Current view mode (Text or CSV)
    pub view_mode: ViewMode,
    /// What kind of content this tab displays (text, image, binary placeholder)
    pub tab_content: TabContent,
    /// Matching bracket pair positions (if cursor is adjacent to a bracket)
    pub matched_brackets: Option<(Position, Position)>,
    /// Whether long logical lines wrap to the pane's visible width.
    pub soft_wrap: bool,
    /// Per-pane logical/visual row mapping, rebuilt by revision and width.
    pub wrap_cache: WrapCache,
    pub folds: crate::folding::FoldState,
    /// Derived inline insertion geometry for this pane only.
    pub ghost_text: super::GhostText,
    /// Shared Find/diagnostic scrollbar projection, memoized per pane.
    pub overview_cache: super::OverviewCache,
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
            soft_wrap: false,
            wrap_cache: WrapCache::new(),
            folds: Default::default(),
            ghost_text: super::GhostText::default(),
            overview_cache: super::OverviewCache::default(),
        }
    }

    /// Create an editor state with specific viewport dimensions
    pub fn with_viewport(visible_lines: usize, visible_columns: usize) -> Self {
        Self {
            viewport: Viewport::new(visible_lines, visible_columns),
            ..Self::new()
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

    /// Whether this editor is in the normal plain-text rendering path.
    ///
    /// Image, CSV, and binary-placeholder tabs all use specialized renderers
    /// and must not opt into text-specific fast paths such as cursor-line-only
    /// redraws.
    pub fn is_plain_text_mode(&self) -> bool {
        matches!(self.tab_content, TabContent::Text) && matches!(self.view_mode, ViewMode::Text)
    }

    /// Apply a pane-local fold action without changing the buffer or its history.
    pub fn fold(
        &mut self,
        document: &Document,
        action: crate::folding::FoldAction,
        header: Option<usize>,
    ) -> bool {
        use crate::folding::FoldAction;
        if !self.is_plain_text_mode() {
            return false;
        }
        self.ensure_wrap_cache(document);
        let mut collapsed = self.folds.collapsed().to_vec();
        let mut collapsed_headers: std::collections::HashSet<_> =
            collapsed.iter().map(|region| region.header).collect();
        if action == FoldAction::ExpandAll {
            collapsed.clear();
        } else {
            let Some(candidates) = &document.folds else {
                return false;
            };
            let line = header.unwrap_or(self.active_cursor().line);
            let selected = candidates
                .regions
                .iter()
                .filter(|r| {
                    if header.is_some() {
                        r.header == line
                    } else {
                        r.contains(line)
                    }
                })
                .min_by_key(|r| r.end - r.header);
            let regions: Vec<_> = if action == FoldAction::CollapseAll {
                candidates.regions.iter().collect()
            } else {
                selected.into_iter().collect()
            };
            for region in regions {
                let already = collapsed_headers.contains(&region.header);
                let expand =
                    action == FoldAction::Expand || (action == FoldAction::Toggle && already);
                if expand {
                    collapsed.retain(|r| r.header != region.header);
                    continue;
                }
                // A selection retains its exact contents, even across a hidden body.
                if already
                    || self.selections.iter().any(|selection| {
                        !selection.is_empty()
                            && selection.start().line < region.end
                            && (selection.end().line > region.header + 1
                                || selection.end().line == region.header + 1
                                    && selection.end().column > 0)
                    })
                {
                    continue;
                }
                for (cursor, selection) in self.cursors.iter_mut().zip(&mut self.selections) {
                    if selection.is_empty() && region.hides(cursor.line) {
                        *cursor = Cursor::at(region.header, document.line_length(region.header));
                        *selection = Selection::new(cursor.to_position());
                    }
                }
                collapsed_headers.insert(region.header);
                collapsed.push(region.clone());
            }
        }
        collapsed.sort_by_key(|r| r.header);
        if !self.folds.replace(collapsed) {
            return false;
        }
        self.deduplicate_cursors();
        self.clear_selection_history();
        self.ensure_wrap_cache(document);
        if self.ghost_text.0.as_ref().is_some_and(|ghost| {
            self.folds
                .projection
                .hidden_header(ghost.anchor.line)
                .is_some()
        }) {
            self.set_ghost_text(document, None);
        }
        true
    }

    /// Explicit navigation and selection endpoints take precedence over folding.
    pub(crate) fn reveal_folded_carets(&mut self, document: &Document) -> bool {
        if self.folds.collapsed().is_empty() {
            return false;
        }
        let mut collapsed = self.folds.collapsed().to_vec();
        collapsed.retain(|region| {
            self.is_plain_text_mode()
                && region.end <= document.line_count()
                && !self.cursors.iter().any(|cursor| region.hides(cursor.line))
                && !self.selections.iter().any(|selection| {
                    region.hides(selection.anchor.line) || region.hides(selection.head.line)
                })
        });
        self.folds.replace(collapsed)
    }

    /// Toggle wrapping without changing the logical cursor position.
    pub fn toggle_soft_wrap(&mut self, document: &Document) {
        if !self.is_plain_text_mode() {
            return;
        }
        if self.ghost_text.0.is_some() {
            self.set_ghost_text(document, None);
        }
        let top = self.viewport_map(document).position_at_display_column(
            document,
            self.viewport.top_line,
            0,
        );
        self.soft_wrap = !self.soft_wrap;
        if !self.soft_wrap {
            self.wrap_cache.invalidate();
        }
        self.ensure_wrap_cache(document);
        self.viewport.top_line = self
            .viewport_map(document)
            .visual_line_for_position(top.line, top.column);
        self.viewport.left_column = 0;
        self.viewport.pixels.x.offset = 0.0;
        self.viewport.animation = None;
        for cursor in &mut self.cursors {
            cursor.clear_desired_column();
        }
        self.ensure_cursor_visible(document);
    }

    /// Rebuild the per-pane wrap cache after content or width changes.
    pub fn ensure_wrap_cache(&mut self, document: &Document) {
        let top = self.viewport_map(document).position_at_display_column(
            document,
            self.viewport.top_line,
            0,
        );
        let mut geometry_changed = false;
        // The integral viewport dimensions remain the grid/navigation contract.
        // Retain the measured trailing partial cell when a caller resizes that grid.
        let pixels = &mut self.viewport.pixels;
        pixels.y.set_visible_cells(self.viewport.visible_lines);
        if !self.soft_wrap {
            pixels.x.set_visible_cells(self.viewport.visible_columns);
        }
        let reflow = if self.ghost_text.0.as_ref().is_some_and(|g| {
            !self.is_plain_text_mode()
                || !g.source_is_current(
                    document,
                    self.soft_wrap.then_some(self.viewport.visible_columns),
                )
        }) {
            let previous = self.ghost_text.0.clone();
            self.set_ghost_text(document, None);
            previous
        } else {
            None
        };
        if self.soft_wrap
            && self.is_plain_text_mode()
            && self
                .wrap_cache
                .needs_refresh(document, self.viewport.visible_columns)
        {
            self.wrap_cache
                .refresh(document, self.viewport.visible_columns);
            geometry_changed = true;
            self.viewport.left_column = 0;
            self.viewport.pixels.x.offset = 0.0;
            self.viewport.animation = None;
        }
        geometry_changed |= self.folds.refresh(
            (self.soft_wrap && self.is_plain_text_mode() && self.wrap_cache.is_valid())
                .then_some(&self.wrap_cache),
            document.line_count(),
        );
        if geometry_changed && self.is_plain_text_mode() {
            self.viewport.top_line = self.viewport_map(document).visual_line_for_position(
                top.line.min(document.line_count().saturating_sub(1)),
                top.column,
            );
            self.viewport.animation = None;
        }
        // Runtime metric/gutter changes also refresh viewports directly, outside
        // update(). Retain a current suggestion across those width changes.
        if self.is_plain_text_mode() {
            if let Some(projection) = reflow.and_then(|g| {
                g.reflow(
                    document,
                    self.soft_wrap.then_some(self.viewport.visible_columns),
                )
            }) {
                self.set_ghost_text(document, Some(std::sync::Arc::new(projection)));
            }
            let rows = self.viewport_map(document).row_count();
            let y = self.viewport.pixels.y.position(self.viewport.top_line);
            self.viewport
                .pixels
                .y
                .set_position(&mut self.viewport.top_line, y, rows);
        }
    }

    /// Global visual row containing the active cursor.
    pub fn cursor_visual_line(&self, document: &Document) -> usize {
        let cursor = self.active_cursor();
        self.viewport_map(document)
            .visual_line_for_position(cursor.line, cursor.column)
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
        self.viewport.pixels.x.extent = visible_columns as f64 * self.viewport.pixels.x.unit;
        self.viewport.pixels.y.extent = visible_lines as f64 * self.viewport.pixels.y.unit;
    }

    /// Set a continuous physical-pixel position on both axes. Row/column indices
    /// remain normalized anchors for wrapping and rope traversal.
    pub fn set_pixel_scroll(&mut self, document: &Document, x: f64, y: f64) -> bool {
        self.viewport.animation = None;
        if !self.is_plain_text_mode() {
            return false;
        }
        self.ensure_wrap_cache(document);
        let rows = self.viewport_map(document).row_count();
        let vertical = self
            .viewport
            .pixels
            .y
            .set_position(&mut self.viewport.top_line, y, rows);
        let columns = self.scrollable_columns(document);
        let horizontal =
            self.viewport
                .pixels
                .x
                .set_position(&mut self.viewport.left_column, x, columns);
        vertical || horizontal
    }

    pub fn pixel_scroll_position(&self) -> (f64, f64) {
        (
            self.viewport.pixels.x.position(self.viewport.left_column),
            self.viewport.pixels.y.position(self.viewport.top_line),
        )
    }

    pub fn scroll_pixels(&mut self, document: &Document, dx: f64, dy: f64, animated: bool) -> bool {
        use super::scroll::ScrollAnimation;
        if !self.is_plain_text_mode() || !dx.is_finite() || !dy.is_finite() {
            return false;
        }
        self.ensure_wrap_cache(document);
        let current = self.pixel_scroll_position();
        if !animated {
            return self.set_pixel_scroll(document, current.0 + dx, current.1 + dy);
        }
        let previous = self
            .viewport
            .animation
            .as_ref()
            .map_or(current, |a| a.target);
        let target = (
            ScrollAnimation::retarget_axis(current.0, previous.0, dx).clamp(
                0.0,
                self.viewport
                    .pixels
                    .x
                    .max_position(self.scrollable_columns(document)),
            ),
            ScrollAnimation::retarget_axis(current.1, previous.1, dy).clamp(
                0.0,
                self.viewport
                    .pixels
                    .y
                    .max_position(self.viewport_map(document).row_count()),
            ),
        );
        if target == current {
            self.viewport.animation = None;
            return false;
        }
        let cursor = self.active_cursor();
        self.viewport.animation = Some(ScrollAnimation::new(
            current,
            target,
            document.revision,
            (cursor.line, cursor.column),
        ));
        true
    }

    /// Build the current text viewport map for this editor/document pair.
    pub fn viewport_map<'a>(&'a self, document: &Document) -> TextViewportMap<'a> {
        let mut map = if self.soft_wrap && self.is_plain_text_mode() && self.wrap_cache.is_valid() {
            TextViewportMap::wrapped(&self.viewport, document.line_count(), &self.wrap_cache)
        } else {
            TextViewportMap::new(&self.viewport, document.line_count())
        };
        if self.is_plain_text_mode() {
            map.folds = Some(&self.folds.projection);
            map.ghost = self
                .ghost_text
                .0
                .as_deref()
                .filter(|ghost| map.hidden_header(ghost.anchor.line).is_none());
        }
        map
    }

    pub(crate) fn set_ghost_text(
        &mut self,
        document: &Document,
        ghost: Option<std::sync::Arc<super::GhostProjection>>,
    ) {
        if !self.is_plain_text_mode() {
            self.ghost_text.0 = None;
            self.overview_cache = super::OverviewCache::default();
            return;
        }
        let top = self.viewport_map(document).position_at_display_column(
            document,
            self.viewport.top_line,
            0,
        );
        self.ghost_text.0 = ghost;
        let map = self.viewport_map(document);
        let row = map.visual_line_for_position(top.line, top.column);
        let rows = map.row_count();
        let y = self.viewport.pixels.y.position(row);
        self.viewport
            .pixels
            .y
            .set_position(&mut self.viewport.top_line, y, rows);
        // A wide suggestion can be the only horizontally scrollable content.
        // Removing it must not leave the source document off the left edge.
        let x = self.viewport.pixels.x.position(self.viewport.left_column);
        let columns = self.scrollable_columns(document);
        self.viewport
            .pixels
            .x
            .set_position(&mut self.viewport.left_column, x, columns);
        self.viewport.animation = None;
        self.overview_cache = super::OverviewCache::default();
    }

    /// Clamp the viewport's top line against the current document.
    pub fn set_top_line_clamped(&mut self, document: &Document, top_line: usize) -> bool {
        self.ensure_wrap_cache(document);
        let (x, _) = self.pixel_scroll_position();
        self.set_pixel_scroll(document, x, top_line as f64 * self.viewport.pixels.y.unit)
    }

    /// Scroll the viewport vertically while respecting the current document bounds.
    pub fn scroll_vertical_by(&mut self, document: &Document, delta: isize) -> bool {
        if delta == 0 {
            return false;
        }
        self.scroll_pixels(
            document,
            0.0,
            delta as f64 * self.viewport.pixels.y.unit,
            false,
        )
    }

    /// Return the longest logical line visible in the current viewport window.
    pub fn max_visible_line_length(&self, document: &Document) -> usize {
        if self.soft_wrap {
            return self.viewport.visible_columns;
        }
        let viewport = self.viewport_map(document);
        (0..viewport.end_line().saturating_sub(viewport.top_line()))
            .filter_map(|row| {
                if let Some(ghost) = viewport.ghost_row_for_visible_row(row) {
                    Some(document.text_settings.tabs.visual_width(ghost.text.chars()))
                } else {
                    viewport.doc_line_for_visible_row(row).map(|line| {
                        let length = document.line_length(line);
                        let text = document.buffer.line(line);
                        if text.chunks().any(|chunk| chunk.contains('\t')) {
                            document
                                .text_settings
                                .tabs
                                .visual_width(text.chars().take(length))
                        } else {
                            length
                        }
                    })
                }
            })
            .max()
            .unwrap_or(0)
    }

    /// Return the maximum horizontal scroll position for the visible viewport window.
    pub fn max_left_column_for_visible_window(&self, document: &Document) -> usize {
        if self.soft_wrap {
            return 0;
        }
        self.scrollable_columns(document)
            .saturating_sub(self.viewport.visible_columns)
    }

    /// Include the insertion caret and the four-column cursor-reveal margin;
    /// otherwise the end-of-line caret clips behind the viewport's right edge.
    pub fn scrollable_columns(&self, document: &Document) -> usize {
        if self.soft_wrap {
            return 0;
        }
        match self.max_visible_line_length(document) {
            0 => 0,
            length => length.saturating_add(5),
        }
    }

    /// Clamp the viewport's left column against the visible viewport window.
    pub fn set_left_column_clamped(&mut self, document: &Document, left_column: usize) -> bool {
        let (_, y) = self.pixel_scroll_position();
        self.set_pixel_scroll(
            document,
            left_column as f64 * self.viewport.pixels.x.unit,
            y,
        )
    }

    /// Scroll horizontally within the currently visible viewport window.
    pub fn scroll_horizontal_visible_window_by(
        &mut self,
        document: &Document,
        delta: isize,
    ) -> bool {
        if delta == 0 {
            return false;
        }
        self.scroll_pixels(
            document,
            delta as f64 * self.viewport.pixels.x.unit,
            0.0,
            false,
        )
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
        self.reveal_folded_carets(document);
        self.ensure_wrap_cache(document);
        let cursor = self.cursors[self.active_cursor_index];
        let viewport = self.viewport_map(document);
        let (row, column) = viewport.display_position(document, cursor.line, cursor.column);
        let (x, y) = self.pixel_scroll_position();
        let pixels = self.viewport.pixels;
        // Clicking a partially visible edge cell must not move it under the pointer.
        let reveal = |axis: super::scroll::PixelAxis, first, cell, current| {
            let start = cell as f64 * axis.unit;
            if start + axis.unit > current && start < current + axis.extent {
                current
            } else {
                axis.reveal(first, cell, 0, ScrollRevealMode::Minimal)
            }
        };
        let x = reveal(pixels.x, self.viewport.left_column, column, x);
        let y = reveal(pixels.y, self.viewport.top_line, row, y);
        self.set_pixel_scroll(document, x, y);
    }

    /// Ensure the active cursor is visible using the specified reveal strategy
    ///
    /// - `Minimal`: scroll just enough to bring cursor into safe zone
    /// - `TopAligned`: place cursor at top of safe zone (good for upward movement)
    /// - `BottomAligned`: place cursor at bottom of safe zone (good for downward movement)
    /// - `Centered`: place cursor in center of viewport (good for jumps/search)
    pub fn ensure_cursor_visible_with_mode(&mut self, document: &Document, mode: ScrollRevealMode) {
        self.reveal_folded_carets(document);
        self.ensure_wrap_cache(document);
        let cursor = self.cursors[self.active_cursor_index];
        let padding = self.scroll_padding;
        let viewport = self.viewport_map(document);
        let (row, column) = viewport.display_position(document, cursor.line, cursor.column);
        let pixels = self.viewport.pixels;
        let y = pixels.y.reveal(self.viewport.top_line, row, padding, mode);
        let x = pixels.x.reveal(
            self.viewport.left_column,
            column,
            4,
            ScrollRevealMode::Minimal,
        );
        self.set_pixel_scroll(document, x, y);
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
    // Per-cursor movement primitives
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
        if self.is_plain_text_mode() && (self.soft_wrap || !self.folds.collapsed().is_empty()) {
            self.move_cursor_visual_by(doc, idx, -1);
            return;
        }
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
        if self.is_plain_text_mode() && (self.soft_wrap || !self.folds.collapsed().is_empty()) {
            self.move_cursor_visual_by(doc, idx, 1);
            return;
        }
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
        if self.is_plain_text_mode() && (self.soft_wrap || !self.folds.collapsed().is_empty()) {
            self.move_cursor_visual_by(doc, idx, -(jump.min(isize::MAX as usize) as isize));
            return;
        }
        let cursor = &mut self.cursors[idx];
        cursor.line = cursor.line.saturating_sub(jump);
        let desired = cursor.desired_column.unwrap_or(cursor.column);
        let line_len = doc.line_length(cursor.line);
        cursor.column = desired.min(line_len);
        cursor.desired_column = Some(desired);
    }

    /// Move a single cursor down by `jump` lines (for page down)
    pub fn page_down_at(&mut self, doc: &Document, jump: usize, idx: usize) {
        if self.is_plain_text_mode() && (self.soft_wrap || !self.folds.collapsed().is_empty()) {
            self.move_cursor_visual_by(doc, idx, jump.min(isize::MAX as usize) as isize);
            return;
        }
        let cursor = &mut self.cursors[idx];
        let max_line = doc.line_count().saturating_sub(1);
        cursor.line = (cursor.line + jump).min(max_line);
        let desired = cursor.desired_column.unwrap_or(cursor.column);
        let line_len = doc.line_length(cursor.line);
        cursor.column = desired.min(line_len);
        cursor.desired_column = Some(desired);
    }

    fn move_cursor_visual_by(&mut self, document: &Document, idx: usize, delta: isize) {
        self.ensure_wrap_cache(document);
        let cursor = self.cursors[idx];
        let map = self.viewport_map(document);
        let (row, column) = map.display_position(document, cursor.line, cursor.column);
        let desired = cursor.desired_column.unwrap_or(column);
        let target = row.saturating_add_signed(delta).min(map.last_line());
        if target == row {
            return;
        }
        let position = map.position_at_display_column(document, target, desired);
        let cursor = &mut self.cursors[idx];
        cursor.line = position.line;
        cursor.column = position.column;
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

    /// Move every cursor through one target/selection contract, then reconcile
    /// duplicate cursors once. Horizontal arrows collapse existing selections.
    pub(crate) fn move_cursors(
        &mut self,
        doc: &Document,
        movement: CursorMovement,
        selection: MovementSelection,
    ) {
        if movement == CursorMovement::Stay {
            if selection == MovementSelection::Move {
                self.collapse_selections_to_cursors();
            }
            return;
        }
        for i in 0..self.cursors.len() {
            if selection == MovementSelection::Move && !self.selections[i].is_empty() {
                let collapse = match movement {
                    CursorMovement::Left => Some(self.selections[i].start()),
                    CursorMovement::Right => Some(self.selections[i].end()),
                    _ => None,
                };
                if let Some(position) = collapse {
                    self.cursors[i] = Cursor::at(position.line, position.column);
                    self.selections[i] = Selection::new(position);
                    continue;
                }
            }
            match movement {
                CursorMovement::Left => self.move_cursor_left_at(doc, i),
                CursorMovement::Right => self.move_cursor_right_at(doc, i),
                CursorMovement::Up => self.move_cursor_up_at(doc, i),
                CursorMovement::Down => self.move_cursor_down_at(doc, i),
                CursorMovement::LineStart => self.move_cursor_line_start_at(doc, i),
                CursorMovement::LineEnd => self.move_cursor_line_end_at(doc, i),
                CursorMovement::DocumentStart => self.move_cursor_document_start_at(i),
                CursorMovement::DocumentEnd => self.move_cursor_document_end_at(doc, i),
                CursorMovement::WordLeft => self.move_cursor_word_left_at(doc, i),
                CursorMovement::WordRight => self.move_cursor_word_right_at(doc, i),
                CursorMovement::PageUp(jump) => self.page_up_at(doc, jump, i),
                CursorMovement::PageDown(jump) => self.page_down_at(doc, jump, i),
                CursorMovement::Stay => {}
            }
            if selection == MovementSelection::Extend {
                self.selections[i].head = self.cursors[i].to_position();
            }
        }
        self.deduplicate_cursors();
        if selection == MovementSelection::Move {
            self.collapse_selections_to_cursors();
        }
    }
}

impl Default for EditorState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use crate::csv::{CsvData, CsvState, Delimiter};
    use crate::model::Document;

    use super::{BinaryPlaceholderState, EditorState, TabContent, TextViewportMap, ViewMode};

    #[test]
    fn plain_text_mode_requires_text_tab_and_text_view_mode() {
        let editor = EditorState::new();
        assert!(editor.is_plain_text_mode());
    }

    #[test]
    fn plain_text_mode_excludes_image_and_csv_view_modes() {
        let mut editor = EditorState::new();
        editor.view_mode = ViewMode::Image(Box::new(crate::image::ImageState::new(
            vec![255, 255, 255, 255],
            1,
            1,
            0,
            "PNG".into(),
            10,
            10,
        )));
        assert!(!editor.is_plain_text_mode());

        let csv = CsvState::new(
            CsvData::from_rows(vec![vec!["value".into()]]),
            Delimiter::Comma,
        );
        editor.view_mode = ViewMode::Csv(Box::new(csv));
        assert!(!editor.is_plain_text_mode());
    }

    #[test]
    fn plain_text_mode_excludes_binary_placeholder_tabs() {
        let mut editor = EditorState::new();
        editor.tab_content = TabContent::BinaryPlaceholder(BinaryPlaceholderState {
            path: std::path::PathBuf::from("image.bin"),
            size_bytes: 128,
        });
        assert!(!editor.is_plain_text_mode());
    }

    #[test]
    fn text_viewport_map_maps_visible_rows_and_doc_lines() {
        let mut editor = EditorState::with_viewport(3, 80);
        editor.viewport.top_line = 2;
        editor.viewport.left_column = 5;
        let document = Document::with_text("a\nb\nc\nd\ne\n");
        let viewport = TextViewportMap::new(&editor.viewport, document.line_count());

        assert_eq!(viewport.top_line(), 2);
        assert_eq!(viewport.left_column(), 5);
        assert_eq!(viewport.doc_line_for_visible_row(0), Some(2));
        assert_eq!(viewport.doc_line_for_visible_row(2), Some(4));
        assert_eq!(viewport.doc_line_for_visible_row(3), Some(5));
        assert_eq!(viewport.end_line(), 5);
        assert_eq!(viewport.visible_row_for_doc_line(1), None);
        assert_eq!(viewport.visible_row_for_doc_line(4), Some(2));
        assert!(!viewport.contains_doc_line(5));
    }

    #[test]
    fn text_viewport_map_clamps_pixel_and_column_conversion() {
        let mut editor = EditorState::with_viewport(3, 80);
        editor.viewport.top_line = 1;
        editor.viewport.left_column = 4;
        let document = Document::with_text("a\nb\nc\n");
        let viewport = TextViewportMap::new(&editor.viewport, document.line_count());

        assert_eq!(viewport.doc_line_for_pixel_y(0.0, 20.0), 1);
        assert_eq!(viewport.doc_line_for_pixel_y(41.0, 20.0), 3);
        assert_eq!(viewport.doc_line_for_pixel_y(400.0, 20.0), 3);
        assert_eq!(viewport.visual_column_for_x_offset(-3.0, 8.0), 4);
        assert_eq!(viewport.visual_column_for_x_offset(17.0, 8.0), 6);
    }

    #[test]
    fn editor_scroll_methods_clamp_to_document_and_visible_window() {
        let mut editor = EditorState::with_viewport(3, 5);
        editor.viewport.top_line = 1;
        let document = Document::with_text("abc\nabcdefghij\nxy\n1234567\nzz\n");

        assert!(editor.set_top_line_clamped(&document, 99));
        assert_eq!(editor.viewport.top_line, 3);

        assert!(editor.scroll_vertical_by(&document, -1));
        assert_eq!(editor.viewport.top_line, 2);

        assert_eq!(editor.max_visible_line_length(&document), 7);
        assert_eq!(editor.max_left_column_for_visible_window(&document), 7);

        assert!(editor.set_left_column_clamped(&document, 99));
        assert_eq!(editor.viewport.left_column, 7);

        assert!(editor.scroll_horizontal_visible_window_by(&document, -7));
        assert_eq!(editor.viewport.left_column, 0);
    }
}
