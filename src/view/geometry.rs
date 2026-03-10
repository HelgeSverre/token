//! Centralized geometry helpers for rendering and hit-testing
//!
//! This module provides a single source of truth for layout calculations,
//! coordinate transformations, and hit-testing that is shared between
//! the view (rendering) and runtime (input handling) layers.
//!
//! All functions here are pure (no I/O, no side effects) and can be
//! tested independently of the rendering infrastructure.

use crate::model::editor_area::{EditorGroup, Rect, TabId};
use crate::model::{AppModel, Document, EditorState, ScaledMetrics};

// ============================================================================
// Layout Constants
// ============================================================================

// Re-export TABULATOR_WIDTH from util::text for single source of truth
pub use crate::util::text::TABULATOR_WIDTH;

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

/// Convert a visual column into a viewport-relative pixel x-coordinate.
///
/// The returned x-position is clamped to the left edge of the text area when
/// the target column is scrolled offscreen.
#[inline]
pub fn column_to_pixel_x(
    visual_col: usize,
    viewport_left: usize,
    text_start_x: usize,
    char_width: f32,
) -> usize {
    let visible_col = visual_col.saturating_sub(viewport_left);
    text_start_x + (visible_col as f32 * char_width).round() as usize
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

use super::helpers::get_tab_display_name;

#[derive(Debug, Clone)]
pub struct TabBarTab {
    pub index: usize,
    pub tab_id: TabId,
    pub title: String,
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
    pub text_x: usize,
    pub text_y: usize,
    pub is_active: bool,
}

#[derive(Debug, Clone)]
pub struct TabBarLayout {
    pub rect_x: usize,
    pub rect_y: usize,
    pub rect_w: usize,
    pub rect_h: usize,
    pub border_y: usize,
    pub tabs: Vec<TabBarTab>,
}

impl TabBarLayout {
    pub fn new(group: &EditorGroup, model: &AppModel, char_width: f32) -> Self {
        let metrics = &model.metrics;
        let rect_x = group.rect.x.round() as usize;
        let rect_y = group.rect.y.round() as usize;
        let rect_w = group.rect.width.round() as usize;
        let rect_h = metrics.tab_bar_height;
        let border_y = (rect_y + rect_h).saturating_sub(1);
        let tab_y = rect_y + metrics.padding_small;
        let tab_height = rect_h.saturating_sub(metrics.padding_medium);
        let right_edge = rect_x + rect_w;

        let mut tabs = Vec::with_capacity(group.tabs.len());
        let mut tab_x = rect_x + metrics.padding_medium;

        for (index, tab) in group.tabs.iter().enumerate() {
            if tab_x >= right_edge {
                break;
            }

            let title = get_tab_display_name(model, tab);
            let title_chars = title.chars().count();
            let ideal_width =
                (title_chars as f32 * char_width).round() as usize + metrics.padding_large * 2;
            let width = ideal_width.min(right_edge.saturating_sub(tab_x));

            tabs.push(TabBarTab {
                index,
                tab_id: tab.id,
                title,
                x: tab_x,
                y: tab_y,
                width,
                height: tab_height,
                text_x: tab_x + metrics.padding_large,
                text_y: tab_y + metrics.padding_medium,
                is_active: index == group.active_tab_index,
            });

            tab_x += ideal_width + metrics.padding_small;
        }

        Self {
            rect_x,
            rect_y,
            rect_w,
            rect_h,
            border_y,
            tabs,
        }
    }

    #[inline]
    pub fn contains(&self, x: f64, y: f64) -> bool {
        x >= self.rect_x as f64
            && x < (self.rect_x + self.rect_w) as f64
            && y >= self.rect_y as f64
            && y < (self.rect_y + self.rect_h) as f64
    }

    pub fn tab_at(&self, x: f64, y: f64) -> Option<&TabBarTab> {
        if !self.contains(x, y) {
            return None;
        }

        self.tabs.iter().find(|tab| {
            x >= tab.x as f64
                && x < (tab.x + tab.width) as f64
                && y >= tab.y as f64
                && y < (tab.y + tab.height) as f64
        })
    }
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

    let text_x = crate::model::text_start_x_scaled(char_width, &model.metrics).round() as f64;

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

    let text_x = crate::model::text_start_x_scaled(char_width, &model.metrics).round() as f64;
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
            rect_x + crate::model::gutter_border_x_scaled(char_width, metrics).round() as usize;
        let text_start_x =
            rect_x + crate::model::text_start_x_scaled(char_width, metrics).round() as usize;

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

    /// Calculate visible text columns for this group.
    #[inline]
    pub fn visible_columns(&self, char_width: f32) -> usize {
        if char_width <= 0.0 {
            return 0;
        }

        let text_start_x_offset = self.text_start_x.saturating_sub(self.rect_x());
        ((self.rect_w() as f32 - text_start_x_offset as f32) / char_width)
            .floor()
            .max(0.0) as usize
    }

    // =========================================================================
    // Scrollbar rects (overlay-style: rendered on top of content right/bottom edge)
    // =========================================================================

    /// Get the vertical scrollbar track rect (right edge of content area).
    ///
    /// Returns `None` if scrollbars are disabled (`show_scrollbar` is false).
    /// The scrollbar is rendered as an overlay over the content area's right edge.
    #[inline]
    pub fn v_scrollbar_rect(&self, scrollbar_width: usize) -> Option<Rect> {
        if scrollbar_width == 0 {
            return None;
        }
        let sw = scrollbar_width as f32;
        let cr = self.content_rect;
        Some(Rect::new(cr.x + cr.width - sw, cr.y, sw, cr.height))
    }

    /// Get the horizontal scrollbar track rect (bottom edge of content area).
    ///
    /// Returns `None` if scrollbars are disabled (`show_scrollbar` is false).
    /// Only shown when content is wider than the viewport.
    #[inline]
    pub fn h_scrollbar_rect(&self, scrollbar_width: usize) -> Option<Rect> {
        if scrollbar_width == 0 {
            return None;
        }
        let sw = scrollbar_width as f32;
        let cr = self.content_rect;
        Some(Rect::new(
            cr.x,
            cr.y + cr.height - sw,
            cr.width - sw, // leave corner for vertical scrollbar
            sw,
        ))
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
    pub fn with_header(rect: Rect, metrics: &crate::model::ScaledMetrics) -> Self {
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
    pub fn without_header(rect: Rect, metrics: &crate::model::ScaledMetrics) -> Self {
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
    pub fn header_title_y(&self, metrics: &crate::model::ScaledMetrics) -> usize {
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

/// Standard padding/spacing constants for modal dialogs
pub struct ModalSpacing;

impl ModalSpacing {
    /// Outer padding inside the modal border
    pub const PAD: usize = 12;
    /// Small gap (e.g., title to input, label to input)
    pub const GAP_SM: usize = 4;
    /// Medium gap (e.g., between sections)
    pub const GAP_MD: usize = 8;
    /// Input field internal vertical padding (total top+bottom)
    pub const INPUT_PAD_Y: usize = 8;
    /// Input field internal horizontal padding (each side)
    pub const INPUT_PAD_X: usize = 8;
}

/// A positioned widget within a modal layout
#[derive(Clone, Copy, Debug)]
pub struct WidgetRect {
    pub x: usize,
    pub y: usize,
    pub w: usize,
    pub h: usize,
}

/// Vertical stack layout builder for modal dialogs.
///
/// Tracks a cursor position that advances as widgets are pushed.
/// Height is derived automatically from the content that's actually laid out.
pub struct VStack {
    cursor_y: usize,
    content_width: usize,
    widgets: Vec<WidgetRect>,
}

impl VStack {
    pub fn new(content_width: usize) -> Self {
        Self {
            cursor_y: 0,
            content_width,
            widgets: Vec::new(),
        }
    }

    /// Add vertical spacing
    pub fn gap(&mut self, h: usize) {
        self.cursor_y += h;
    }

    /// Push a widget with the given height, spanning the full content width.
    /// Returns the index into `widgets` for later retrieval.
    pub fn push(&mut self, h: usize) -> usize {
        let idx = self.widgets.len();
        self.widgets.push(WidgetRect {
            x: 0,
            y: self.cursor_y,
            w: self.content_width,
            h,
        });
        self.cursor_y += h;
        idx
    }

    /// Total height consumed by all widgets and gaps
    pub fn height(&self) -> usize {
        self.cursor_y
    }
}

/// Computed layout for a modal dialog.
///
/// Single source of truth for modal positioning — used by both rendering
/// and hit-testing. The outer rect defines the modal border/background,
/// and widgets are positioned absolutely within the window.
#[derive(Clone, Debug)]
pub struct ModalLayout {
    /// Modal outer bounds (background + border)
    pub x: usize,
    pub y: usize,
    pub w: usize,
    pub h: usize,
    /// Absolutely-positioned widget rects (indices from VStack::push)
    pub widgets: Vec<WidgetRect>,
}

impl ModalLayout {
    /// Build a modal layout from a VStack and positioning parameters.
    pub fn build(
        vstack: VStack,
        modal_width: usize,
        window_width: usize,
        window_height: usize,
    ) -> Self {
        let pad = ModalSpacing::PAD;
        let content_height = vstack.height();
        let modal_height = content_height + pad * 2;
        let modal_x = (window_width.saturating_sub(modal_width)) / 2;
        let modal_y = (window_height / 4).min(100);
        let content_x = modal_x + pad;
        let content_y = modal_y + pad;

        // Translate widget rects from local to absolute coordinates
        let widgets = vstack
            .widgets
            .into_iter()
            .map(|w| WidgetRect {
                x: content_x + w.x,
                y: content_y + w.y,
                w: w.w,
                h: w.h,
            })
            .collect();

        Self {
            x: modal_x,
            y: modal_y,
            w: modal_width,
            h: modal_height,
            widgets,
        }
    }

    /// Check if a point is inside the modal bounds
    pub fn contains(&self, px: usize, py: usize) -> bool {
        px >= self.x && px < self.x + self.w && py >= self.y && py < self.y + self.h
    }

    /// Get a widget rect by index
    pub fn widget(&self, idx: usize) -> &WidgetRect {
        &self.widgets[idx]
    }

    /// Height of an input field (line_height + padding)
    pub fn input_height(line_height: usize) -> usize {
        line_height + ModalSpacing::INPUT_PAD_Y
    }
}

// ============================================================================
// Per-Modal Layout Functions
// ============================================================================

/// Layout indices for the Find/Replace modal widgets
pub struct FindReplaceWidgets {
    pub title: usize,
    pub find_label: Option<usize>,
    pub find_input: usize,
    pub replace_label: Option<usize>,
    pub replace_input: Option<usize>,
}

/// Compute layout for the Find/Replace modal.
///
/// Height is derived automatically from the content. In find-only mode,
/// no "Find:" label is shown (the title says "Find"). In replace mode,
/// both "Find:" and "Replace:" labels are shown.
pub fn find_replace_layout(
    window_width: usize,
    window_height: usize,
    line_height: usize,
    replace_mode: bool,
) -> (ModalLayout, FindReplaceWidgets) {
    let modal_width = (window_width as f32 * 0.5).clamp(300.0, 500.0) as usize;
    let pad = ModalSpacing::PAD;
    let content_width = modal_width.saturating_sub(pad * 2);
    let input_height = ModalLayout::input_height(line_height);

    let mut v = VStack::new(content_width);

    let title = v.push(line_height);

    let (find_label, find_input, replace_label, replace_input);

    if replace_mode {
        v.gap(ModalSpacing::GAP_MD);
        find_label = Some(v.push(line_height));
        v.gap(ModalSpacing::GAP_SM);
        find_input = v.push(input_height);
        v.gap(ModalSpacing::GAP_MD);
        replace_label = Some(v.push(line_height));
        v.gap(ModalSpacing::GAP_SM);
        replace_input = Some(v.push(input_height));
    } else {
        v.gap(ModalSpacing::GAP_SM);
        find_label = None;
        find_input = v.push(input_height);
        replace_label = None;
        replace_input = None;
    }

    let layout = ModalLayout::build(v, modal_width, window_width, window_height);
    let widgets = FindReplaceWidgets {
        title,
        find_label,
        find_input,
        replace_label,
        replace_input,
    };

    (layout, widgets)
}

/// Layout indices for GotoLine modal widgets
pub struct GotoLineWidgets {
    pub title: usize,
    pub input: usize,
}

/// Compute layout for the Go to Line modal.
pub fn goto_line_layout(
    window_width: usize,
    window_height: usize,
    line_height: usize,
) -> (ModalLayout, GotoLineWidgets) {
    let modal_width = (window_width as f32 * 0.5).clamp(300.0, 500.0) as usize;
    let pad = ModalSpacing::PAD;
    let content_width = modal_width.saturating_sub(pad * 2);
    let input_height = ModalLayout::input_height(line_height);

    let mut v = VStack::new(content_width);
    let title = v.push(line_height);
    v.gap(ModalSpacing::GAP_SM);
    let input = v.push(input_height);

    let layout = ModalLayout::build(v, modal_width, window_width, window_height);
    (layout, GotoLineWidgets { title, input })
}

/// Layout indices for CommandPalette modal widgets
pub struct CommandPaletteWidgets {
    pub title: usize,
    pub input: usize,
    pub list: Option<usize>,
}

/// Compute layout for the Command Palette modal.
pub fn command_palette_layout(
    window_width: usize,
    window_height: usize,
    line_height: usize,
    list_items: usize,
) -> (ModalLayout, CommandPaletteWidgets) {
    let modal_width = (window_width as f32 * 0.5).clamp(300.0, 500.0) as usize;
    let pad = ModalSpacing::PAD;
    let content_width = modal_width.saturating_sub(pad * 2);
    let input_height = ModalLayout::input_height(line_height);

    let mut v = VStack::new(content_width);
    let title = v.push(line_height);
    v.gap(ModalSpacing::GAP_SM);
    let input = v.push(input_height);

    let max_visible = 8;
    let visible = list_items.min(max_visible);
    let has_overflow = list_items > max_visible;
    let list = if visible > 0 {
        v.gap(ModalSpacing::GAP_MD);
        // Add extra line for "... and X more" overflow indicator
        let list_rows = if has_overflow { visible + 1 } else { visible };
        Some(v.push(list_rows * line_height))
    } else {
        None
    };

    let layout = ModalLayout::build(v, modal_width, window_width, window_height);
    (layout, CommandPaletteWidgets { title, input, list })
}

/// Layout indices for FileFinder modal widgets
pub struct FileFinderWidgets {
    pub title: usize,
    pub input: usize,
    pub list: Option<usize>,
}

/// Compute layout for the File Finder modal.
///
/// `has_query` should be true when the input is non-empty, so we reserve
/// space for the "No files match" message even when `list_items` is 0.
pub fn file_finder_layout(
    window_width: usize,
    window_height: usize,
    line_height: usize,
    list_items: usize,
    has_query: bool,
) -> (ModalLayout, FileFinderWidgets) {
    let modal_width = (window_width as f32 * 0.7).clamp(500.0, 900.0) as usize;
    let pad = ModalSpacing::PAD;
    let content_width = modal_width.saturating_sub(pad * 2);
    let input_height = ModalLayout::input_height(line_height);

    let mut v = VStack::new(content_width);
    let title = v.push(line_height);
    v.gap(ModalSpacing::GAP_MD);
    let input = v.push(input_height);

    let max_visible = 10;
    let visible = list_items.min(max_visible);
    // Always reserve at least 1 row when there's a query (for "No files match" message)
    let list_rows = if visible > 0 {
        visible
    } else if has_query {
        1
    } else {
        0
    };
    let list = if list_rows > 0 {
        v.gap(ModalSpacing::GAP_MD);
        Some(v.push(list_rows * line_height))
    } else {
        None
    };

    let layout = ModalLayout::build(v, modal_width, window_width, window_height);
    (layout, FileFinderWidgets { title, input, list })
}

/// Layout indices for ThemePicker modal widgets
pub struct ThemePickerWidgets {
    pub title: usize,
    pub list: usize,
}

/// Compute layout for the Theme Picker modal.
///
/// `total_rows` should include section headers (User Themes / Built-in Themes).
pub fn theme_picker_layout(
    window_width: usize,
    window_height: usize,
    line_height: usize,
    total_rows: usize,
) -> (ModalLayout, ThemePickerWidgets) {
    let modal_width = 400;
    let pad = ModalSpacing::PAD;
    let content_width = modal_width - pad * 2;

    let mut v = VStack::new(content_width);
    let title = v.push(line_height);
    v.gap(ModalSpacing::GAP_MD);
    let list = v.push(total_rows * line_height);

    // ThemePicker uses window_height/4 without the min(100) cap
    let modal_x = window_width.saturating_sub(modal_width) / 2;
    let modal_y = window_height / 4;
    let content_height = v.height();
    let modal_height = content_height + pad * 2;
    let content_x = modal_x + pad;
    let content_y = modal_y + pad;

    let widgets: Vec<WidgetRect> = v
        .widgets
        .into_iter()
        .map(|w| WidgetRect {
            x: content_x + w.x,
            y: content_y + w.y,
            w: w.w,
            h: w.h,
        })
        .collect();

    let layout = ModalLayout {
        x: modal_x,
        y: modal_y,
        w: modal_width,
        h: modal_height,
        widgets,
    };
    (layout, ThemePickerWidgets { title, list })
}

// ============================================================================
// Dock Geometry
// ============================================================================

use crate::panel::{DockLayout, DockPosition};

/// Shared top-level window layout used by rendering and hit-testing.
#[derive(Debug, Clone, Copy)]
pub struct WindowLayout {
    /// Full content area above the status bar.
    pub content_rect: Rect,
    /// Status bar rectangle.
    pub status_bar_rect: Rect,
    /// Sidebar rectangle (if visible).
    pub sidebar_rect: Option<Rect>,
    /// Remaining editor area after sidebar/right/bottom panels are subtracted.
    pub editor_area_rect: Rect,
    /// Right dock rectangle (if open).
    pub right_dock_rect: Option<Rect>,
    /// Bottom dock rectangle (if open).
    pub bottom_dock_rect: Option<Rect>,
}

impl WindowLayout {
    /// Compute the current top-level window layout from the app model.
    pub fn compute(model: &AppModel, line_height: usize) -> Self {
        let window_width = model.window_size.0 as f32;
        let window_height = model.window_size.1 as f32;
        let status_bar_h = status_bar_height(line_height) as f32;
        let content_height = (window_height - status_bar_h).max(0.0);

        let sidebar_width = model
            .workspace
            .as_ref()
            .filter(|ws| ws.sidebar_visible)
            .map(|ws| ws.sidebar_width(model.metrics.scale_factor))
            .unwrap_or(0.0);
        let right_dock_width = model.dock_layout.right.size(model.metrics.scale_factor);
        let bottom_dock_height = model.dock_layout.bottom.size(model.metrics.scale_factor);
        let side_panel_height = (content_height - bottom_dock_height).max(0.0);

        let content_rect = Rect::new(0.0, 0.0, window_width, content_height);
        let status_bar_rect = Rect::new(0.0, content_height, window_width, status_bar_h);
        let sidebar_rect = if sidebar_width > 0.0 {
            Some(Rect::new(0.0, 0.0, sidebar_width, content_height))
        } else {
            None
        };
        let right_dock_rect = if right_dock_width > 0.0 {
            Some(Rect::new(
                window_width - right_dock_width,
                0.0,
                right_dock_width,
                side_panel_height,
            ))
        } else {
            None
        };
        let bottom_dock_rect = if bottom_dock_height > 0.0 {
            Some(Rect::new(
                sidebar_width,
                side_panel_height,
                (window_width - sidebar_width).max(0.0),
                bottom_dock_height,
            ))
        } else {
            None
        };
        let editor_area_rect = Rect::new(
            sidebar_width,
            0.0,
            (window_width - sidebar_width - right_dock_width).max(0.0),
            side_panel_height,
        );

        Self {
            content_rect,
            status_bar_rect,
            sidebar_rect,
            editor_area_rect,
            right_dock_rect,
            bottom_dock_rect,
        }
    }
}

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
            Some(Rect::new(
                w - right_width,
                0.0,
                right_width,
                side_dock_height,
            ))
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
            if px >= rect.x && px < rect.x + rect.width && py >= rect.y && py < rect.y + rect.height
            {
                return Some(DockPosition::Left);
            }
        }

        if let Some(rect) = &self.right {
            if px >= rect.x && px < rect.x + rect.width && py >= rect.y && py < rect.y + rect.height
            {
                return Some(DockPosition::Right);
            }
        }

        if let Some(rect) = &self.bottom {
            if px >= rect.x && px < rect.x + rect.width && py >= rect.y && py < rect.y + rect.height
            {
                return Some(DockPosition::Bottom);
            }
        }

        None
    }
}

// ============================================================================
// Binary Placeholder Layout
// ============================================================================

/// Button label used for binary placeholder tabs.
pub const BINARY_PLACEHOLDER_BUTTON_LABEL: &str = "Open with Default Application";

/// Layout positions for the binary file placeholder screen.
///
/// Pre-computes all vertical positions and the button rect so rendering
/// and hit-testing use the exact same geometry.
pub struct BinaryPlaceholderLayout {
    /// Horizontal center of the content area
    pub center_x: usize,
    /// Y position for the filename text
    pub name_y: usize,
    /// Y position for the file size text
    pub size_y: usize,
    /// The button's bounding rect
    pub button_rect: Rect,
}

/// Compute binary placeholder layout from the content area dimensions.
///
/// Used by both the renderer and hit-test code to ensure consistent positioning.
pub fn binary_placeholder_layout(
    content_rect: Rect,
    line_height: usize,
    char_width: f32,
    padding_large: usize,
    padding_medium: usize,
    button_label: &str,
) -> BinaryPlaceholderLayout {
    let center_x = content_rect.x as usize + content_rect.width as usize / 2;
    let center_y = content_rect.y as usize + content_rect.height as usize / 2;

    let name_y = center_y.saturating_sub(line_height * 2);
    let size_y = name_y + line_height + line_height / 2;
    let btn_y = size_y + line_height * 3;

    let padding_h = padding_large * 2;
    let padding_v = padding_medium;
    let button_rect = super::button::button_rect(
        center_x,
        btn_y,
        button_label,
        char_width,
        line_height,
        padding_h,
        padding_v,
    );

    BinaryPlaceholderLayout {
        center_x,
        name_y,
        size_y,
        button_rect,
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
    fn test_group_layout_visible_columns_respects_text_start() {
        let layout = GroupLayout {
            group_rect: Rect::new(0.0, 0.0, 200.0, 120.0),
            content_rect: Rect::new(0.0, 24.0, 200.0, 96.0),
            tab_bar_height: 24,
            gutter_right_x: 48,
            text_start_x: 60,
        };

        assert_eq!(layout.visible_columns(10.0), 14);
    }

    #[test]
    fn test_tab_bar_layout_hits_tabs_and_empty_space() {
        let mut model = crate::model::AppModel::new(400, 300, 1.0, vec![]);
        let group_id = model.editor_area.focused_group_id;

        {
            let group = model.editor_area.groups.get_mut(&group_id).unwrap();
            group.rect = Rect::new(10.0, 20.0, 260.0, 120.0);

            let mut second_tab = group.tabs[0].clone();
            second_tab.id = crate::model::editor_area::TabId(999);
            group.tabs.push(second_tab);
        }

        let group = model.editor_area.groups.get(&group_id).unwrap();
        let layout = TabBarLayout::new(group, &model, 8.0);

        assert_eq!(layout.tabs.len(), 2);
        assert!(layout.contains(11.0, 21.0));
        assert!(layout
            .tab_at((layout.rect_x + 1) as f64, (layout.rect_y + 1) as f64)
            .is_none());

        let first = layout
            .tab_at((layout.tabs[0].x + 1) as f64, (layout.tabs[0].y + 1) as f64)
            .unwrap();
        assert_eq!(first.index, 0);
        assert_eq!(first.tab_id, group.tabs[0].id);

        let second = layout
            .tab_at((layout.tabs[1].x + 1) as f64, (layout.tabs[1].y + 1) as f64)
            .unwrap();
        assert_eq!(second.index, 1);
        assert_eq!(second.tab_id, group.tabs[1].id);
    }

    #[test]
    fn test_tab_bar_layout_clips_tabs_at_group_edge() {
        let mut model = crate::model::AppModel::new(400, 300, 1.0, vec![]);
        let group_id = model.editor_area.focused_group_id;

        {
            let group = model.editor_area.groups.get_mut(&group_id).unwrap();
            group.rect = Rect::new(10.0, 20.0, 70.0, 120.0);

            let mut second_tab = group.tabs[0].clone();
            second_tab.id = crate::model::editor_area::TabId(999);
            group.tabs.push(second_tab);
        }

        let group = model.editor_area.groups.get(&group_id).unwrap();
        let layout = TabBarLayout::new(group, &model, 8.0);
        let right_edge = layout.rect_x + layout.rect_w;

        assert_eq!(layout.tabs.len(), 1);
        assert_eq!(layout.tabs[0].x + layout.tabs[0].width, right_edge);
    }

    #[test]
    fn test_window_layout_editor_area_accounts_for_docks() {
        use crate::panel::DockPosition;

        let mut model = crate::model::AppModel::new(1000, 700, 1.0, vec![]);
        model.line_height = 20;
        model.dock_layout.dock_mut(DockPosition::Right).is_open = true;
        model.dock_layout.dock_mut(DockPosition::Right).size_logical = 180.0;
        model.dock_layout.dock_mut(DockPosition::Bottom).is_open = true;
        model
            .dock_layout
            .dock_mut(DockPosition::Bottom)
            .size_logical = 140.0;

        let layout = WindowLayout::compute(&model, model.line_height);

        assert_eq!(layout.content_rect.height, 680.0);
        assert_eq!(layout.status_bar_rect.y, 680.0);
        assert_eq!(layout.right_dock_rect.unwrap().width, 180.0);
        assert_eq!(layout.bottom_dock_rect.unwrap().height, 140.0);
        assert_eq!(layout.editor_area_rect.width, 820.0);
        assert_eq!(layout.editor_area_rect.height, 540.0);
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

    #[test]
    fn test_column_to_pixel_x() {
        assert_eq!(column_to_pixel_x(2, 0, 100, 8.0), 116);
        assert_eq!(column_to_pixel_x(6, 4, 100, 8.0), 116);
        assert_eq!(column_to_pixel_x(2, 4, 100, 8.0), 100);
        assert_eq!(column_to_pixel_x(3, 1, 100, 7.5), 115);
    }

    // ====================================================================
    // VStack / ModalLayout tests
    // ====================================================================

    #[test]
    fn test_vstack_empty() {
        let v = VStack::new(200);
        assert_eq!(v.height(), 0);
    }

    #[test]
    fn test_vstack_push_and_gap() {
        let mut v = VStack::new(200);
        let a = v.push(30);
        v.gap(10);
        let b = v.push(20);

        assert_eq!(a, 0);
        assert_eq!(b, 1);
        assert_eq!(v.widgets[a].y, 0);
        assert_eq!(v.widgets[a].h, 30);
        assert_eq!(v.widgets[b].y, 40); // 30 + 10 gap
        assert_eq!(v.widgets[b].h, 20);
        assert_eq!(v.height(), 60); // 30 + 10 + 20
    }

    #[test]
    fn test_modal_layout_build_translation() {
        let mut v = VStack::new(200);
        v.push(30); // widget 0
        v.gap(8);
        v.push(20); // widget 1

        let layout = ModalLayout::build(v, 224, 1000, 800);
        let pad = ModalSpacing::PAD;

        // Modal is centered: (1000 - 224) / 2 = 388
        assert_eq!(layout.x, 388);
        // Modal y: min(800/4, 100) = 100
        assert_eq!(layout.y, 100);
        // Modal height: content(30+8+20) + 2*pad
        assert_eq!(layout.h, 58 + pad * 2);

        // Widget 0: translated to (388+pad, 100+pad)
        let w0 = layout.widget(0);
        assert_eq!(w0.x, 388 + pad);
        assert_eq!(w0.y, 100 + pad);
        assert_eq!(w0.h, 30);

        // Widget 1: translated, y = 100+pad+30+8 = 100+pad+38
        let w1 = layout.widget(1);
        assert_eq!(w1.y, 100 + pad + 38);
    }

    #[test]
    fn test_modal_layout_contains_boundary() {
        let layout = ModalLayout {
            x: 100,
            y: 50,
            w: 200,
            h: 100,
            widgets: vec![],
        };

        // Corners: inclusive at (x,y), exclusive at (x+w, y+h)
        assert!(layout.contains(100, 50));
        assert!(layout.contains(299, 149));
        assert!(!layout.contains(300, 50));
        assert!(!layout.contains(100, 150));
        assert!(!layout.contains(99, 50));
        assert!(!layout.contains(100, 49));
    }

    #[test]
    fn test_input_height() {
        assert_eq!(
            ModalLayout::input_height(20),
            20 + ModalSpacing::INPUT_PAD_Y
        );
    }

    // ====================================================================
    // Per-modal layout tests
    // ====================================================================

    #[test]
    fn test_goto_line_layout() {
        let lh = 20;
        let (layout, w) = goto_line_layout(1000, 800, lh);

        // Has title + input
        assert_eq!(layout.widgets.len(), 2);
        let title = layout.widget(w.title);
        let input = layout.widget(w.input);
        assert_eq!(title.h, lh);
        assert_eq!(input.h, ModalLayout::input_height(lh));
        // Input starts below title + gap
        assert!(input.y > title.y + title.h);
    }

    #[test]
    fn test_find_replace_layout_find_only() {
        let lh = 20;
        let (layout, w) = find_replace_layout(1000, 800, lh, false);

        // Find-only: title + find_input (no labels)
        assert!(w.find_label.is_none());
        assert!(w.replace_label.is_none());
        assert!(w.replace_input.is_none());
        assert_eq!(layout.widgets.len(), 2);
    }

    #[test]
    fn test_find_replace_layout_replace_mode() {
        let lh = 20;
        let (layout, w) = find_replace_layout(1000, 800, lh, true);

        // Replace mode: title + find_label + find_input + replace_label + replace_input
        assert!(w.find_label.is_some());
        assert!(w.replace_label.is_some());
        assert!(w.replace_input.is_some());
        assert_eq!(layout.widgets.len(), 5);

        // Replace input is below find input
        let find_input = layout.widget(w.find_input);
        let repl_input = layout.widget(w.replace_input.unwrap());
        assert!(repl_input.y > find_input.y + find_input.h);
    }

    #[test]
    fn test_command_palette_layout_empty_list() {
        let lh = 20;
        let (layout, w) = command_palette_layout(1000, 800, lh, 0);

        assert!(w.list.is_none());
        // Only title + input
        assert_eq!(layout.widgets.len(), 2);
    }

    #[test]
    fn test_command_palette_layout_with_overflow() {
        let lh = 20;
        // 15 items > max_visible(8) -> should have overflow row
        let (layout, w) = command_palette_layout(1000, 800, lh, 15);

        assert!(w.list.is_some());
        let list = layout.widget(w.list.unwrap());
        // 8 visible + 1 overflow row = 9 * lh
        assert_eq!(list.h, 9 * lh);
    }

    #[test]
    fn test_command_palette_layout_no_overflow() {
        let lh = 20;
        // 5 items <= max_visible(8) -> no overflow row
        let (_, w) = command_palette_layout(1000, 800, lh, 5);

        let list = &w.list;
        assert!(list.is_some());
    }

    #[test]
    fn test_file_finder_layout_empty_no_query() {
        let lh = 20;
        let (_, w) = file_finder_layout(1000, 800, lh, 0, false);

        // No query, no results -> no list area
        assert!(w.list.is_none());
    }

    #[test]
    fn test_file_finder_layout_empty_with_query() {
        let lh = 20;
        let (layout, w) = file_finder_layout(1000, 800, lh, 0, true);

        // Has query but no results -> 1 row for "No files match" message
        assert!(w.list.is_some());
        let list = layout.widget(w.list.unwrap());
        assert_eq!(list.h, lh);
    }

    #[test]
    fn test_theme_picker_layout() {
        let lh = 20;
        let (layout, w) = theme_picker_layout(1000, 800, lh, 10);

        let title = layout.widget(w.title);
        let list = layout.widget(w.list);
        assert_eq!(title.h, lh);
        assert_eq!(list.h, 10 * lh);
        // Modal width is always 400
        assert_eq!(layout.w, 400);
    }

    #[test]
    fn test_tree_list_layout_positions() {
        use crate::model::ScaledMetrics;
        let metrics = ScaledMetrics::new(1.0);
        let tl = TreeListLayout::from_metrics(&metrics);

        // Depth 0: just left_padding
        let pos = tl.node_position(0, 100);
        assert_eq!(pos.icon_x, tl.left_padding);
        assert_eq!(pos.text_x, tl.left_padding + tl.indicator_width);
        assert_eq!(pos.text_y, 100 + tl.text_top_padding);

        // Depth 1: left_padding + indent
        let pos1 = tl.node_position(1, 100);
        assert!(pos1.icon_x > pos.icon_x);
    }

    #[test]
    fn test_outline_panel_layout_content_geometry() {
        let metrics = ScaledMetrics::new(1.0);
        let layout = OutlinePanelLayout::new(Rect::new(700.0, 0.0, 300.0, 540.0), &metrics);

        assert_eq!(layout.title_x, 708);
        assert_eq!(layout.title_y, 4);
        assert_eq!(
            layout.content_rect.y,
            metrics.file_tree_row_height as f32 + metrics.padding_medium as f32
        );
        assert_eq!(
            layout.content_rect.height,
            540.0 - metrics.file_tree_row_height as f32 - metrics.padding_medium as f32
        );
        assert_eq!(
            layout.visible_capacity(),
            (layout.content_rect.height / metrics.file_tree_row_height as f32) as usize
        );
    }

    #[test]
    fn test_outline_panel_layout_row_and_chevron_hit_testing() {
        let metrics = ScaledMetrics::new(1.0);
        let layout = OutlinePanelLayout::new(Rect::new(700.0, 0.0, 300.0, 540.0), &metrics);
        let row_start = layout.content_rect.y;
        let next_row = row_start + metrics.file_tree_row_height as f32;

        assert_eq!(layout.row_index_at_y(row_start - 0.1, 3), None);
        assert_eq!(layout.row_index_at_y(row_start, 3), Some(3));
        assert_eq!(layout.row_index_at_y(next_row - 0.1, 3), Some(3));
        assert_eq!(layout.row_index_at_y(next_row, 3), Some(4));

        assert!(layout.is_on_chevron(0, 708.0));
        assert!(!layout.is_on_chevron(0, 725.0));
    }
}

// ============================================================================
// Tree List Layout
// ============================================================================

/// Reusable layout parameters for scrollable tree-list widgets (sidebar, outline).
///
/// Encapsulates the padding, indent, and spacing calculations that are shared
/// between the sidebar file tree and the outline panel, providing a single
/// source of truth for tree-node positioning.
#[derive(Debug, Clone, Copy)]
pub struct TreeListLayout {
    /// Left padding from container edge to first-level icons
    pub left_padding: usize,
    /// Width reserved for the expand/collapse indicator
    pub indicator_width: usize,
    /// Vertical padding from row top to text baseline
    pub text_top_padding: usize,
    /// Horizontal indent per nesting level
    pub indent: f32,
}

/// Shared layout for the outline dock panel.
///
/// Centralizes the title/content split and row hit-test geometry so render,
/// scroll logic, and mouse handling all use the same measurements.
#[derive(Debug, Clone, Copy)]
pub struct OutlinePanelLayout {
    /// Full dock panel rectangle.
    pub rect: Rect,
    /// Precomputed title x position.
    pub title_x: usize,
    /// Precomputed title y position.
    pub title_y: usize,
    /// Scrollable content area below the title bar.
    pub content_rect: Rect,
    /// Tree row height in pixels.
    pub row_height: usize,
    /// Tree indentation/padding rules for the outline panel.
    pub tree: TreeListLayout,
}

impl OutlinePanelLayout {
    /// Build outline panel geometry from the dock rectangle and scaled metrics.
    pub fn new(rect: Rect, metrics: &ScaledMetrics) -> Self {
        let row_height = metrics.file_tree_row_height;
        let title_x = (rect.x + metrics.padding_large as f32) as usize;
        let title_y = (rect.y + metrics.padding_medium as f32) as usize;
        let content_y = rect.y + row_height as f32 + metrics.padding_medium as f32;
        let content_height =
            (rect.height - row_height as f32 - metrics.padding_medium as f32).max(0.0);

        Self {
            rect,
            title_x,
            title_y,
            content_rect: Rect::new(rect.x, content_y, rect.width, content_height),
            row_height,
            tree: TreeListLayout::outline_from_metrics(metrics),
        }
    }

    /// Number of whole outline rows that fit in the content area.
    #[inline]
    pub fn visible_capacity(&self) -> usize {
        if self.row_height == 0 {
            0
        } else {
            (self.content_rect.height / self.row_height as f32).max(0.0) as usize
        }
    }

    /// Resolve a mouse y-coordinate to a flattened visible row index.
    #[inline]
    pub fn row_index_at_y(&self, y: f32, scroll_offset: usize) -> Option<usize> {
        if self.row_height == 0
            || y < self.content_rect.y
            || y >= self.content_rect.y + self.content_rect.height
        {
            return None;
        }

        let visual_row = ((y - self.content_rect.y) / self.row_height as f32) as usize;
        Some(scroll_offset.saturating_add(visual_row))
    }

    /// Whether the x-coordinate lands on the collapse/expand indicator for a row depth.
    #[inline]
    pub fn is_on_chevron(&self, depth: usize, x: f32) -> bool {
        let start = self.rect.x + self.tree.x_offset(depth) as f32;
        let end = start + self.tree.indicator_width as f32;
        x >= start && x < end
    }
}

/// Computed positions for a single tree node at a given depth and y.
#[derive(Debug, Clone, Copy)]
pub struct TreeNodePosition {
    /// X coordinate for the expand/collapse icon
    pub icon_x: usize,
    /// X coordinate for the text label
    pub text_x: usize,
    /// Y coordinate for the text (row y + top padding)
    pub text_y: usize,
}

impl TreeListLayout {
    /// Create a tree list layout from scaled metrics.
    pub fn from_metrics(metrics: &crate::model::ScaledMetrics) -> Self {
        Self {
            left_padding: metrics.padding_large,
            indicator_width: metrics.padding_large + metrics.padding_large / 2,
            text_top_padding: metrics.padding_small,
            indent: metrics.file_tree_indent,
        }
    }

    /// Create a tree list layout for the outline panel (slightly smaller indicator).
    pub fn outline_from_metrics(metrics: &crate::model::ScaledMetrics) -> Self {
        Self {
            left_padding: metrics.padding_large,
            indicator_width: metrics.padding_large + metrics.padding_medium,
            text_top_padding: metrics.padding_small,
            indent: metrics.file_tree_indent,
        }
    }

    /// Compute the x-offset for a node at the given depth.
    #[inline]
    pub fn x_offset(&self, depth: usize) -> usize {
        (depth as f32 * self.indent) as usize + self.left_padding
    }

    /// Compute icon_x, text_x, and text_y for a node at the given depth and row y.
    #[inline]
    pub fn node_position(&self, depth: usize, row_y: usize) -> TreeNodePosition {
        let x_offset = self.x_offset(depth);
        TreeNodePosition {
            icon_x: x_offset,
            text_x: x_offset + self.indicator_width,
            text_y: row_y + self.text_top_padding,
        }
    }

    /// Compute the available width for text given container width and text_x.
    #[inline]
    pub fn available_text_width(&self, container_width: usize, text_x: usize) -> usize {
        container_width.saturating_sub(text_x + self.left_padding)
    }
}
