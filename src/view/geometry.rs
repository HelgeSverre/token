//! Centralized geometry helpers for rendering and hit-testing
//!
//! This module provides a single source of truth for layout calculations,
//! coordinate transformations, and hit-testing that is shared between
//! the view (rendering) and runtime (input handling) layers.
//!
//! All functions here are pure (no I/O, no side effects) and can be
//! tested independently of the rendering infrastructure.

use token::model::editor_area::{EditorGroup, Rect};
use token::model::{AppModel, Document, EditorState};

// ============================================================================
// Layout Constants
// ============================================================================

// Re-export TABULATOR_WIDTH from util::text for single source of truth
pub use token::util::text::TABULATOR_WIDTH;

// ============================================================================
// Viewport Sizing Helpers
// ============================================================================

/// Calculate the height of the status bar in pixels
#[inline]
pub fn status_bar_height(line_height: usize) -> usize {
    line_height
}

// ============================================================================
// Tab Expansion Helpers
// ============================================================================

use std::borrow::Cow;

/// Expand tab characters to spaces for display.
///
/// Converts each tab character to the appropriate number of spaces based on
/// the current visual column and `TABULATOR_WIDTH`. This is used for rendering
/// text where tabs need to be visually aligned.
///
/// Returns `Cow::Borrowed` if no tabs are present (zero allocation),
/// or `Cow::Owned` with expanded tabs otherwise.
///
/// # Example
/// ```ignore
/// let text = "a\tb";  // Tab at column 1
/// let expanded = expand_tabs_for_display(text);
/// assert_eq!(&*expanded, "a   b");  // Tab becomes 3 spaces (to reach column 4)
/// ```
pub fn expand_tabs_for_display(text: &str) -> Cow<'_, str> {
    // Fast path: if no tabs, return borrowed reference (no allocation)
    if !text.contains('\t') {
        return Cow::Borrowed(text);
    }

    // Slow path: expand tabs
    let mut result = String::with_capacity(text.len() * 2);
    let mut visual_col = 0;

    for ch in text.chars() {
        if ch == '\t' {
            let spaces = TABULATOR_WIDTH - (visual_col % TABULATOR_WIDTH);
            for _ in 0..spaces {
                result.push(' ');
            }
            visual_col += spaces;
        } else {
            result.push(ch);
            visual_col += 1;
        }
    }

    Cow::Owned(result)
}

/// Convert a character column index to a visual (screen) column position.
///
/// Accounts for tab expansion when calculating the screen position.
/// A character column is an index into the string's characters, while
/// a visual column is the screen position accounting for variable-width tabs.
///
/// # Arguments
/// * `text` - The line of text containing possible tab characters
/// * `char_col` - The character index to convert
///
/// # Returns
/// The visual column (screen position) for the given character index.
pub fn char_col_to_visual_col(text: &str, char_col: usize) -> usize {
    let mut visual_col = 0;
    for (i, ch) in text.chars().enumerate() {
        if i >= char_col {
            break;
        }
        if ch == '\t' {
            visual_col += TABULATOR_WIDTH - (visual_col % TABULATOR_WIDTH);
        } else {
            visual_col += 1;
        }
    }
    visual_col
}

/// Convert a visual (screen) column position to a character column index.
///
/// This is the inverse of `char_col_to_visual_col`. Given a screen position,
/// it returns the character index that would be at that position, accounting
/// for tab expansion.
///
/// # Arguments
/// * `text` - The line of text containing possible tab characters
/// * `visual_col` - The screen column position to convert
///
/// # Returns
/// The character index corresponding to the given visual column.
/// If the visual column is past the end of the line, returns the line length.
pub fn visual_col_to_char_col(text: &str, visual_col: usize) -> usize {
    let mut current_visual = 0;
    let mut char_col = 0;

    for ch in text.chars() {
        if current_visual >= visual_col {
            return char_col;
        }

        if ch == '\t' {
            let tab_width = TABULATOR_WIDTH - (current_visual % TABULATOR_WIDTH);
            current_visual += tab_width;
        } else {
            current_visual += 1;
        }
        char_col += 1;
    }

    char_col
}

// ============================================================================
// Hit-Testing Helpers
// ============================================================================

/// Get the focused group, editor, and document from the model.
///
/// This helper centralizes the lookup of the currently focused editor context,
/// which is needed for hit-testing functions that need to convert global window
/// coordinates to local group coordinates.
fn focused_group_editor_document(
    model: &AppModel,
) -> Option<(&EditorGroup, &EditorState, &Document)> {
    let editor_area = &model.editor_area;

    let group = editor_area.focused_group()?;
    let editor_id = group.active_editor_id()?;
    let editor = editor_area.editors.get(&editor_id)?;
    let doc_id = editor.document_id?;
    let document = editor_area.documents.get(&doc_id)?;

    Some((group, editor, document))
}

/// Check if a y-coordinate is within the status bar region
#[inline]
pub fn is_in_status_bar(y: f64, window_height: u32, line_height: usize) -> bool {
    let status_bar_top = window_height as f64 - line_height as f64;
    y >= status_bar_top
}

/// Check if a point is within a group's tab bar region
#[inline]
pub fn is_in_group_tab_bar(y: f64, group_rect: &Rect, tab_bar_height: usize) -> bool {
    let local_y = y - group_rect.y as f64;
    local_y >= 0.0 && local_y < tab_bar_height as f64
}

use super::helpers::get_tab_display_name;

/// Find which tab index is at the given x position within a group's tab bar.
/// Returns None if the click is not on any tab.
pub fn tab_at_position(
    x: f64,
    char_width: f32,
    model: &AppModel,
    group: &EditorGroup,
) -> Option<usize> {
    let mut tab_x = 4.0; // Initial padding

    for (idx, tab) in group.tabs.iter().enumerate() {
        let title = get_tab_display_name(model, tab);
        let tab_width = (title.len() as f32 * char_width).round() as f64 + 16.0;

        if x >= tab_x && x < tab_x + tab_width {
            return Some(idx);
        }

        tab_x += tab_width + 2.0; // tab width + gap
    }

    None
}

/// Convert pixel coordinates to document line and column for the focused editor.
///
/// Takes into account the group's position (including sidebar offset), tab bar,
/// gutter, scroll offset, and horizontal scrolling.
///
/// This function delegates to `pixel_to_cursor_in_group` using the focused group's
/// rect, which includes the sidebar offset and any split view positioning.
pub fn pixel_to_cursor(
    x: f64,
    y: f64,
    char_width: f32,
    line_height: f64,
    model: &AppModel,
) -> (usize, usize) {
    if let Some((group, editor, document)) = focused_group_editor_document(model) {
        pixel_to_cursor_in_group(
            x,
            y,
            char_width,
            line_height,
            &group.rect,
            model,
            editor,
            document,
        )
    } else {
        // No focused group/editor/document - safe fallback
        (0, 0)
    }
}

/// Convert pixel coordinates to line and VISUAL column (screen position).
/// Used for rectangle selection where the raw visual column is needed,
/// independent of any specific line's text content.
/// Returns (line, visual_column) where visual_column is the screen column.
///
/// This function delegates to `pixel_to_line_and_visual_column_in_group` using
/// the focused group's rect, which includes the sidebar offset and split positioning.
pub fn pixel_to_line_and_visual_column(
    x: f64,
    y: f64,
    char_width: f32,
    line_height: f64,
    model: &AppModel,
) -> (usize, usize) {
    if let Some((group, editor, document)) = focused_group_editor_document(model) {
        pixel_to_line_and_visual_column_in_group(
            x,
            y,
            char_width,
            line_height,
            &group.rect,
            model,
            editor,
            document,
        )
    } else {
        // No focused group/editor/document - safe fallback
        (0, 0)
    }
}

/// Convert pixel coordinates to line and VISUAL column for a specific group.
///
/// Accounts for the group's rect position within the window.
/// Used for rectangle selection where the raw visual column is needed.
#[allow(clippy::too_many_arguments)]
pub fn pixel_to_line_and_visual_column_in_group(
    x: f64,
    y: f64,
    char_width: f32,
    line_height: f64,
    group_rect: &Rect,
    model: &AppModel,
    editor: &EditorState,
    document: &Document,
) -> (usize, usize) {
    let local_x = x - group_rect.x as f64;
    let local_y = y - group_rect.y as f64;

    let text_x = token::model::text_start_x_scaled(char_width, &model.metrics).round() as f64;

    let text_start_y = model.metrics.tab_bar_height as f64;
    let adjusted_y = (local_y - text_start_y).max(0.0);
    let visual_line = (adjusted_y / line_height).floor() as usize;
    let line = editor.viewport.top_line + visual_line;
    let line = line.min(document.buffer.len_lines().saturating_sub(1));

    let x_offset = local_x - text_x;
    let visual_column = if x_offset > 0.0 {
        editor.viewport.left_column + (x_offset / char_width as f64).round() as usize
    } else {
        editor.viewport.left_column
    };

    (line, visual_column)
}

/// Convert pixel coordinates to document line and column for a specific group.
///
/// Accounts for the group's rect position within the window.
/// This is the core hit-testing function that handles coordinate conversion
/// from absolute window coordinates to local group coordinates.
#[allow(clippy::too_many_arguments)]
pub fn pixel_to_cursor_in_group(
    x: f64,
    y: f64,
    char_width: f32,
    line_height: f64,
    group_rect: &Rect,
    model: &AppModel,
    editor: &EditorState,
    document: &Document,
) -> (usize, usize) {
    let local_x = x - group_rect.x as f64;
    let local_y = y - group_rect.y as f64;

    let text_x = token::model::text_start_x_scaled(char_width, &model.metrics).round() as f64;
    let text_start_y = model.metrics.tab_bar_height as f64;
    let adjusted_y = (local_y - text_start_y).max(0.0);
    let visual_line = (adjusted_y / line_height).floor() as usize;
    let line = editor.viewport.top_line + visual_line;
    let line = line.min(document.buffer.len_lines().saturating_sub(1));

    let x_offset = local_x - text_x;
    let visual_column = if x_offset > 0.0 {
        editor.viewport.left_column + (x_offset / char_width as f64).round() as usize
    } else {
        editor.viewport.left_column
    };

    let line_text = document.get_line(line).unwrap_or_default();
    let line_text_trimmed = super::helpers::trim_line_ending(&line_text);
    let column = visual_col_to_char_col(line_text_trimmed, visual_column);

    let line_len = document.line_length(line);
    let column = column.min(line_len);

    (line, column)
}

// ============================================================================
// GroupLayout - Unified Layout Computation
// ============================================================================

/// Pre-computed layout for an editor group, with all positions in window coordinates.
///
/// This struct provides a single source of truth for all positioning calculations
/// within an editor group. It uses scaled metrics (DPI-aware) and ensures consistent
/// positioning across all rendering functions.
///
/// # Usage
/// ```ignore
/// let layout = GroupLayout::new(group, model, char_width);
/// // Use layout.content_y(), layout.gutter_right_x, etc.
/// ```
#[derive(Debug, Clone, Copy)]
pub struct GroupLayout {
    /// The group's rect in window coordinates (from compute_layout_scaled)
    pub group_rect: Rect,
    /// Content area (excludes tab bar), in window coordinates
    pub content_rect: Rect,
    /// Tab bar height (scaled for DPI)
    pub tab_bar_height: usize,
    /// Gutter border X position (absolute window coordinate)
    pub gutter_right_x: usize,
    /// X coordinate where text content starts (absolute window coordinate)
    pub text_start_x: usize,
}

impl GroupLayout {
    /// Create a new GroupLayout from an editor group.
    ///
    /// All positioning values are computed using scaled metrics from the model,
    /// ensuring DPI-correct rendering on all displays.
    pub fn new(group: &EditorGroup, model: &AppModel, char_width: f32) -> Self {
        let group_rect = group.rect;
        let metrics = &model.metrics;

        let tab_bar_height = metrics.tab_bar_height;
        let content_rect = Rect::new(
            group_rect.x,
            group_rect.y + tab_bar_height as f32,
            group_rect.width,
            (group_rect.height - tab_bar_height as f32).max(0.0),
        );

        let rect_x = group_rect.x.round() as usize;
        let gutter_right_x =
            rect_x + token::model::gutter_border_x_scaled(char_width, metrics).round() as usize;
        let text_start_x =
            rect_x + token::model::text_start_x_scaled(char_width, metrics).round() as usize;

        Self {
            group_rect,
            content_rect,
            tab_bar_height,
            gutter_right_x,
            text_start_x,
        }
    }

    // =========================================================================
    // Group-level accessors (tab bar area)
    // =========================================================================

    /// Get absolute X position of the group
    #[inline]
    pub fn rect_x(&self) -> usize {
        self.group_rect.x.round() as usize
    }

    /// Get absolute Y position of the group
    #[inline]
    pub fn rect_y(&self) -> usize {
        self.group_rect.y.round() as usize
    }

    /// Get group width in pixels
    #[inline]
    pub fn rect_w(&self) -> usize {
        self.group_rect.width.round() as usize
    }

    /// Get group height in pixels
    #[inline]
    #[allow(dead_code)]
    pub fn rect_h(&self) -> usize {
        self.group_rect.height.round() as usize
    }

    // =========================================================================
    // Content-level accessors (below tab bar)
    // =========================================================================

    /// Get absolute X position for content area
    #[inline]
    #[allow(dead_code)]
    pub fn content_x(&self) -> usize {
        self.content_rect.x.round() as usize
    }

    /// Get absolute Y position for content area (below tab bar)
    #[inline]
    pub fn content_y(&self) -> usize {
        self.content_rect.y.round() as usize
    }

    /// Get content width in pixels
    #[inline]
    #[allow(dead_code)]
    pub fn content_w(&self) -> usize {
        self.content_rect.width.round() as usize
    }

    /// Get content height in pixels
    #[inline]
    pub fn content_h(&self) -> usize {
        self.content_rect.height.round() as usize
    }

    // =========================================================================
    // Gutter accessors
    // =========================================================================

    /// Get gutter width in pixels (from rect_x to gutter_right_x)
    #[inline]
    pub fn gutter_width(&self) -> usize {
        self.gutter_right_x - self.rect_x()
    }

    // =========================================================================
    // Line positioning helpers
    // =========================================================================

    /// Convert a document line number to screen Y coordinate.
    ///
    /// Returns `Some(y)` if the line is visible in the viewport,
    /// or `None` if the line is outside the visible area.
    #[inline]
    pub fn line_to_screen_y(
        &self,
        doc_line: usize,
        viewport_top: usize,
        line_height: usize,
    ) -> Option<usize> {
        if doc_line < viewport_top {
            return None;
        }
        let screen_line = doc_line - viewport_top;
        let y = self.content_y() + screen_line * line_height;

        // Check if line is within visible content area
        if y + line_height <= self.content_y() + self.content_h() {
            Some(y)
        } else {
            None
        }
    }

    /// Calculate visible line count for this group
    #[inline]
    pub fn visible_lines(&self, line_height: usize) -> usize {
        self.content_h() / line_height
    }
}

// ============================================================================
// Pane Layout System
// ============================================================================

/// Border configuration for a pane.
#[derive(Debug, Clone, Copy, Default)]
pub struct PaneBorders {
    /// Show border on top edge
    pub top: bool,
    /// Show border on bottom edge
    pub bottom: bool,
    /// Show border on left edge
    pub left: bool,
    /// Show border on right edge
    pub right: bool,
}

impl PaneBorders {
    /// No borders
    pub const NONE: Self = Self {
        top: false,
        bottom: false,
        left: false,
        right: false,
    };

    /// All borders
    #[allow(dead_code)]
    pub const ALL: Self = Self {
        top: true,
        bottom: true,
        left: true,
        right: true,
    };

    /// Bottom border only (common for headers)
    #[allow(dead_code)]
    pub const BOTTOM: Self = Self {
        top: false,
        bottom: true,
        left: false,
        right: false,
    };
}

/// Insets (padding) configuration for a pane.
#[derive(Debug, Clone, Copy)]
pub struct PaneInsets {
    pub top: usize,
    pub bottom: usize,
    pub left: usize,
    pub right: usize,
}

impl PaneInsets {
    /// Create uniform insets
    pub fn all(size: usize) -> Self {
        Self {
            top: size,
            bottom: size,
            left: size,
            right: size,
        }
    }

    /// Create horizontal/vertical insets
    #[allow(dead_code)]
    pub fn symmetric(horizontal: usize, vertical: usize) -> Self {
        Self {
            top: vertical,
            bottom: vertical,
            left: horizontal,
            right: horizontal,
        }
    }

    /// No insets
    pub const NONE: Self = Self {
        top: 0,
        bottom: 0,
        left: 0,
        right: 0,
    };
}

impl Default for PaneInsets {
    fn default() -> Self {
        Self::NONE
    }
}

/// A reusable pane layout with optional header, borders, and content insets.
///
/// Panes are the building blocks for UI panels, preview panes, dialogs, etc.
/// They provide consistent sizing and positioning across the application.
///
/// # Layout Structure
/// ```text
/// ┌─────────────────────────────────┐ ← outer_rect.y
/// │ Header (optional)               │
/// │─────────────────────────────────│ ← header border
/// │ ┌─────────────────────────────┐ │ ← content_rect.y (with insets)
/// │ │                             │ │
/// │ │     Content Area            │ │
/// │ │                             │ │
/// │ └─────────────────────────────┘ │
/// └─────────────────────────────────┘
/// ```
#[derive(Debug, Clone, Copy)]
pub struct Pane {
    /// Full outer rect of the pane
    pub outer_rect: Rect,
    /// Header height (0 if no header)
    pub header_height: usize,
    /// Whether to show header border
    pub header_border: bool,
    /// Content area insets
    pub insets: PaneInsets,
    /// Border configuration
    pub borders: PaneBorders,
    /// Border width in pixels
    pub border_width: usize,
}

impl Pane {
    /// Create a pane with a header (uses tab_bar_height).
    pub fn with_header(rect: Rect, metrics: &token::model::ScaledMetrics) -> Self {
        Self {
            outer_rect: rect,
            header_height: metrics.tab_bar_height,
            header_border: true,
            insets: PaneInsets::all(metrics.padding_large + metrics.padding_medium),
            borders: PaneBorders::NONE,
            border_width: metrics.border_width,
        }
    }

    /// Create a pane without a header.
    #[allow(dead_code)]
    pub fn without_header(rect: Rect, metrics: &token::model::ScaledMetrics) -> Self {
        Self {
            outer_rect: rect,
            header_height: 0,
            header_border: false,
            insets: PaneInsets::all(metrics.padding_large),
            borders: PaneBorders::NONE,
            border_width: metrics.border_width,
        }
    }

    /// Set content insets
    #[allow(dead_code)]
    pub fn with_insets(mut self, insets: PaneInsets) -> Self {
        self.insets = insets;
        self
    }

    /// Set border configuration
    #[allow(dead_code)]
    pub fn with_borders(mut self, borders: PaneBorders) -> Self {
        self.borders = borders;
        self
    }

    // =========================================================================
    // Outer rect accessors
    // =========================================================================

    /// Outer rect X position
    #[inline]
    pub fn x(&self) -> usize {
        self.outer_rect.x.round() as usize
    }

    /// Outer rect Y position
    #[inline]
    pub fn y(&self) -> usize {
        self.outer_rect.y.round() as usize
    }

    /// Outer rect width
    #[inline]
    pub fn width(&self) -> usize {
        self.outer_rect.width.round() as usize
    }

    /// Outer rect height
    #[inline]
    pub fn height(&self) -> usize {
        self.outer_rect.height.round() as usize
    }

    // =========================================================================
    // Header accessors
    // =========================================================================

    /// Whether this pane has a header
    #[inline]
    pub fn has_header(&self) -> bool {
        self.header_height > 0
    }

    /// Header rect (returns zero-height rect if no header)
    #[inline]
    #[allow(dead_code)]
    pub fn header_rect(&self) -> Rect {
        Rect::new(
            self.outer_rect.x,
            self.outer_rect.y,
            self.outer_rect.width,
            self.header_height as f32,
        )
    }

    /// X position for header title text
    #[inline]
    pub fn header_title_x(&self) -> usize {
        self.x() + self.insets.left
    }

    /// Y position for header title text (vertically centered)
    #[inline]
    pub fn header_title_y(&self, metrics: &token::model::ScaledMetrics) -> usize {
        self.y() + metrics.padding_medium
    }

    /// Y position of header border line
    #[inline]
    pub fn header_border_y(&self) -> usize {
        self.y() + self.header_height.saturating_sub(self.border_width)
    }

    // =========================================================================
    // Content rect accessors
    // =========================================================================

    /// Content area rect (after header and insets)
    #[allow(dead_code)]
    pub fn content_rect(&self) -> Rect {
        let y = self.outer_rect.y + self.header_height as f32;
        let height = (self.outer_rect.height - self.header_height as f32).max(0.0);
        Rect::new(self.outer_rect.x, y, self.outer_rect.width, height)
    }

    /// Content area X position (with left inset)
    #[inline]
    pub fn content_x(&self) -> usize {
        self.x() + self.insets.left
    }

    /// Content area Y position (below header, with top inset)
    #[inline]
    pub fn content_y(&self) -> usize {
        self.y() + self.header_height + self.insets.top
    }

    /// Content area width (with horizontal insets)
    #[inline]
    pub fn content_width(&self) -> usize {
        self.width()
            .saturating_sub(self.insets.left + self.insets.right)
    }

    /// Content area height (with vertical insets, after header)
    #[inline]
    pub fn content_height(&self) -> usize {
        self.height()
            .saturating_sub(self.header_height + self.insets.top + self.insets.bottom)
    }

    /// Inner content rect (with all insets applied)
    #[allow(dead_code)]
    pub fn inner_content_rect(&self) -> Rect {
        Rect::new(
            self.content_x() as f32,
            self.content_y() as f32,
            self.content_width() as f32,
            self.content_height() as f32,
        )
    }

    // =========================================================================
    // Utility helpers
    // =========================================================================

    /// Calculate visible lines given line height
    #[inline]
    pub fn visible_lines(&self, line_height: usize) -> usize {
        if line_height == 0 {
            return 0;
        }
        self.content_height() / line_height
    }

    /// Calculate max text width (content width)
    #[inline]
    pub fn max_text_width(&self) -> usize {
        self.content_width()
    }

    /// Check if a point is within the pane header area
    #[inline]
    pub fn is_in_header(&self, x: f64, y: f64) -> bool {
        if !self.has_header() {
            return false;
        }
        let px = x as f32;
        let py = y as f32;
        px >= self.outer_rect.x
            && px < self.outer_rect.x + self.outer_rect.width
            && py >= self.outer_rect.y
            && py < self.outer_rect.y + self.header_height as f32
    }
}

// ============================================================================
// Modal Geometry
// ============================================================================

/// Calculate the modal bounds for hit-testing.
/// Returns (x, y, width, height) of the modal dialog.
pub fn modal_bounds(
    window_width: usize,
    window_height: usize,
    line_height: usize,
    has_list: bool,
    list_items: usize,
) -> (usize, usize, usize, usize) {
    let max_visible_items = 8;
    let visible_items = list_items.min(max_visible_items);

    let modal_width = (window_width as f32 * 0.5).clamp(300.0, 500.0) as usize;
    let base_height = line_height * 3 + 20;
    let list_height = if has_list {
        visible_items * line_height + 8
    } else {
        0
    };
    let modal_height = base_height + list_height;
    let modal_x = (window_width - modal_width) / 2;
    let modal_y = (window_height / 4).min(100);

    (modal_x, modal_y, modal_width, modal_height)
}

// ============================================================================
// Dock Geometry
// ============================================================================

use token::panel::{DockLayout, DockPosition};

/// Computed rectangles for all dock areas
///
/// Used by both rendering and hit-testing to ensure consistent layout.
/// Currently unused but will be integrated when dock rendering is implemented.
#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
pub struct DockRects {
    /// Left dock area (if visible)
    pub left: Option<Rect>,
    /// Right dock area (if visible)
    pub right: Option<Rect>,
    /// Bottom dock area (if visible)
    pub bottom: Option<Rect>,
    /// Remaining editor area after docks are subtracted
    pub editor_area: Rect,
}

#[allow(dead_code)]
impl DockRects {
    /// Compute dock rectangles from current layout state
    ///
    /// The layout follows VS Code/IntelliJ style:
    /// 1. Left dock spans full content height (sidebar to status bar)
    /// 2. Right dock spans full content height (from top to above bottom dock)
    /// 3. Bottom dock spans from left dock to window right edge (under right dock)
    /// 4. Editor area is what remains (between left/right docks, above bottom dock)
    pub fn compute(
        dock_layout: &DockLayout,
        window_width: u32,
        window_height: u32,
        status_bar_height: usize,
        scale_factor: f64,
    ) -> Self {
        let w = window_width as f32;
        let content_height = (window_height as usize).saturating_sub(status_bar_height) as f32;

        let left_width = dock_layout.left.size(scale_factor);
        let right_width = dock_layout.right.size(scale_factor);
        let bottom_height = dock_layout.bottom.size(scale_factor);

        // Side docks span content height minus bottom dock
        let side_dock_height = content_height - bottom_height;

        // Left dock: full height on left side
        let left = if left_width > 0.0 {
            Some(Rect::new(0.0, 0.0, left_width, side_dock_height))
        } else {
            None
        };

        // Right dock: full height on right side (above bottom dock)
        let right = if right_width > 0.0 {
            Some(Rect::new(w - right_width, 0.0, right_width, side_dock_height))
        } else {
            None
        };

        // Bottom dock: spans from left dock edge to window right edge (under right dock)
        let bottom = if bottom_height > 0.0 {
            Some(Rect::new(
                left_width,
                side_dock_height,
                w - left_width, // spans under right dock to window edge
                bottom_height,
            ))
        } else {
            None
        };

        // Editor area: between left and right docks, above bottom dock
        let editor_area = Rect::new(
            left_width,
            0.0,
            w - left_width - right_width,
            side_dock_height,
        );

        Self {
            left,
            right,
            bottom,
            editor_area,
        }
    }

    /// Check if a point is in any dock resize handle
    ///
    /// Returns the dock position if the point is within the resize zone.
    /// Resize zones are on the inner edge of each dock:
    /// - Left dock: right edge
    /// - Right dock: left edge
    /// - Bottom dock: top edge
    pub fn hit_test_resize(&self, x: f64, y: f64, resize_zone: f32) -> Option<DockPosition> {
        let px = x as f32;
        let py = y as f32;

        // Left dock resize handle (right edge)
        if let Some(rect) = &self.left {
            let handle_x = rect.x + rect.width - resize_zone;
            if px >= handle_x
                && px < rect.x + rect.width + resize_zone
                && py >= rect.y
                && py < rect.y + rect.height
            {
                return Some(DockPosition::Left);
            }
        }

        // Right dock resize handle (left edge)
        if let Some(rect) = &self.right {
            let handle_x = rect.x - resize_zone;
            if px >= handle_x
                && px < rect.x + resize_zone
                && py >= rect.y
                && py < rect.y + rect.height
            {
                return Some(DockPosition::Right);
            }
        }

        // Bottom dock resize handle (top edge)
        if let Some(rect) = &self.bottom {
            let handle_y = rect.y - resize_zone;
            if py >= handle_y
                && py < rect.y + resize_zone
                && px >= rect.x
                && px < rect.x + rect.width
            {
                return Some(DockPosition::Bottom);
            }
        }

        None
    }

    /// Check if a point is in a dock's content area (not resize handle)
    pub fn hit_test_content(&self, x: f64, y: f64) -> Option<DockPosition> {
        let px = x as f32;
        let py = y as f32;

        if let Some(rect) = &self.left {
            if px >= rect.x
                && px < rect.x + rect.width
                && py >= rect.y
                && py < rect.y + rect.height
            {
                return Some(DockPosition::Left);
            }
        }

        if let Some(rect) = &self.right {
            if px >= rect.x
                && px < rect.x + rect.width
                && py >= rect.y
                && py < rect.y + rect.height
            {
                return Some(DockPosition::Right);
            }
        }

        if let Some(rect) = &self.bottom {
            if px >= rect.x
                && px < rect.x + rect.width
                && py >= rect.y
                && py < rect.y + rect.height
            {
                return Some(DockPosition::Bottom);
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_in_status_bar() {
        // Window 600px tall, line height 20px -> status bar at y >= 580
        assert!(!is_in_status_bar(579.0, 600, 20));
        assert!(is_in_status_bar(580.0, 600, 20));
        assert!(is_in_status_bar(590.0, 600, 20));
    }

    #[test]
    fn test_expand_tabs() {
        assert_eq!(expand_tabs_for_display("a\tb"), "a   b"); // tab at col 1 -> 3 spaces
        assert_eq!(expand_tabs_for_display("\t"), "    "); // tab at col 0 -> 4 spaces
    }

    #[test]
    fn test_char_col_to_visual_col() {
        assert_eq!(char_col_to_visual_col("abc", 2), 2);
        // "a\tb": 'a' at char 0 (visual 0), '\t' at char 1 (visual 1-3), 'b' at char 2 (visual 4)
        assert_eq!(char_col_to_visual_col("a\tb", 2), 4);
    }

    #[test]
    fn test_visual_col_to_char_col() {
        assert_eq!(visual_col_to_char_col("abc", 2), 2);
        assert_eq!(visual_col_to_char_col("a\tb", 4), 2); // visual 4 is 'b' which is char 2
    }
}
