//! Centralized geometry helpers for rendering and hit-testing
//!
//! This module provides a single source of truth for layout calculations,
//! coordinate transformations, and hit-testing that is shared between
//! the view (rendering) and runtime (input handling) layers.
//!
//! All functions here are pure (no I/O, no side effects) and can be
//! tested independently of the rendering infrastructure.

use crate::model::editor_area::{EditorGroup, Rect, TabId};
use crate::model::{AppModel, Document, EditorState, TextViewportMap};

// ============================================================================
// Layout Constants
// ============================================================================

// Re-export TABULATOR_WIDTH from util::text for single source of truth
pub use crate::util::text::TABULATOR_WIDTH;

// ============================================================================
// Viewport Sizing Helpers
// ============================================================================

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
pub fn is_in_status_bar(y: f64, window_height: u32, status_bar_height: usize) -> bool {
    let status_bar_top = window_height as f64 - status_bar_height as f64;
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
    /// Compute each tab's `(start, end)` x-extent in unscrolled tab-bar
    /// coordinates (relative to the group's left edge).
    ///
    /// Single source of truth for tab positions, shared by `new` (render +
    /// hit-test) and the tab-scroll logic in the update layer.
    pub fn tab_spans(
        group: &EditorGroup,
        model: &AppModel,
        char_width: f32,
    ) -> Vec<(usize, usize)> {
        let metrics = &model.metrics;
        let mut spans = Vec::with_capacity(group.tabs.len());
        let mut x = metrics.padding_medium;

        for tab in &group.tabs {
            let title_chars = get_tab_display_name(model, tab).chars().count();
            let ideal_width =
                (title_chars as f32 * char_width).round() as usize + metrics.padding_large * 2;
            spans.push((x, x + ideal_width));
            x += ideal_width + metrics.padding_small;
        }

        spans
    }

    /// Total width of all tabs including trailing padding, in pixels.
    pub fn total_tabs_width(group: &EditorGroup, model: &AppModel, char_width: f32) -> usize {
        Self::tab_spans(group, model, char_width)
            .last()
            .map(|&(_, end)| end + model.metrics.padding_medium)
            .unwrap_or(0)
    }

    pub fn new(group: &EditorGroup, model: &AppModel, char_width: f32) -> Self {
        let metrics = &model.metrics;
        let rect_x = group.rect.x.round() as usize;
        let rect_y = group.rect.y.round() as usize;
        let rect_w = group.rect.width.round() as usize;
        let rect_h = metrics.tab_bar_height;
        let border_y = (rect_y + rect_h).saturating_sub(1);
        let tab_y = rect_y + metrics.padding_small;
        let tab_height = rect_h.saturating_sub(metrics.padding_medium);
        let right_edge = (rect_x + rect_w) as isize;
        let scroll = group.tab_scroll as isize;

        let spans = Self::tab_spans(group, model, char_width);
        let mut tabs = Vec::with_capacity(group.tabs.len());

        for (index, (tab, &(start, end))) in group.tabs.iter().zip(&spans).enumerate() {
            let tab_x = rect_x as isize + start as isize - scroll;
            let tab_right = rect_x as isize + end as isize - scroll;

            if tab_right <= rect_x as isize {
                // Scrolled fully off the left edge
                continue;
            }
            if tab_x >= right_edge {
                break;
            }

            // Clip to the group's tab bar on both edges; partially visible
            // tabs keep their logical text_x so glyphs stay put and the
            // renderer's per-tab clip trims the overhang.
            let visible_x = tab_x.max(rect_x as isize) as usize;
            let visible_right = tab_right.min(right_edge) as usize;

            tabs.push(TabBarTab {
                index,
                tab_id: tab.id,
                title: get_tab_display_name(model, tab),
                x: visible_x,
                y: tab_y,
                width: visible_right.saturating_sub(visible_x),
                height: tab_height,
                text_x: (tab_x + metrics.padding_large as isize).max(0) as usize,
                text_y: tab_y + metrics.padding_medium,
                is_active: index == group.active_tab_index,
            });
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
    let viewport = TextViewportMap::new(&editor.viewport, document.line_count());
    let local_x = x - group_rect.x as f64;
    let local_y = y - group_rect.y as f64;

    let text_x = crate::model::text_start_x_scaled(
        char_width,
        &model.metrics,
        document.line_count(),
        !document.diagnostics.is_empty(),
    )
    .round() as f64;

    let text_start_y = model.metrics.tab_bar_height as f64;
    let adjusted_y = (local_y - text_start_y).max(0.0);
    let line = viewport.doc_line_for_pixel_y(adjusted_y, line_height);

    let x_offset = local_x - text_x;
    let visual_column = viewport.visual_column_for_x_offset(x_offset, char_width);

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
    let viewport = TextViewportMap::new(&editor.viewport, document.line_count());
    let local_x = x - group_rect.x as f64;
    let local_y = y - group_rect.y as f64;

    let text_x = crate::model::text_start_x_scaled(
        char_width,
        &model.metrics,
        document.line_count(),
        !document.diagnostics.is_empty(),
    )
    .round() as f64;
    let text_start_y = model.metrics.tab_bar_height as f64;
    let adjusted_y = (local_y - text_start_y).max(0.0);
    let line = viewport.doc_line_for_pixel_y(adjusted_y, line_height);

    let x_offset = local_x - text_x;
    let visual_column = viewport.visual_column_for_x_offset(x_offset, char_width);

    let line_text = document.get_line(line).unwrap_or_default();
    let line_text_trimmed = super::helpers::trim_line_ending(&line_text);
    let column = visual_col_to_char_col(line_text_trimmed, visual_column);

    let line_len = document.line_length(line);
    let column = column.min(line_len);

    (line, column)
}

// ============================================================================
// GutterLayout - Gutter lane widths
// ============================================================================

/// Gutter lane widths, in physical pixels.
///
/// `Copy` and allocation-free: it's rebuilt as part of `GroupLayout` on the
/// per-mouse-move hit-test path, so it must stay a plain widths struct with
/// no `Vec`. The line-numbers lane (`numbers_w`, which includes the trailing
/// gutter padding up to the border) is always active; the marks lane
/// activates per-document once it has diagnostics (LSP Phase 2, the first
/// consumer — see editor-decorations.md); fold/diff lanes stay 0 until their
/// consumers ship.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct GutterLayout {
    pub marks_w: u16,
    pub numbers_w: u16,
    pub fold_w: u16,
    pub diff_w: u16,
}

impl GutterLayout {
    /// Build the gutter layout for a document with `line_count` lines.
    /// `has_marks` activates the marks lane (~1 char wide, left of line
    /// numbers per editor-decorations.md's lane order).
    pub fn new(
        char_width: f32,
        metrics: &crate::model::ScaledMetrics,
        line_count: usize,
        has_marks: bool,
    ) -> Self {
        // Round the marks lane and the combined gutter-border width from
        // the *same* single sum `gutter_border_x_scaled` uses, then
        // derive `numbers_w` as the remainder — rounding `marks_w` and
        // `numbers_w` independently could disagree by a pixel with
        // `text_start_x_scaled`'s one combined round (fractional
        // `char_width`), drifting the gutter border and text start apart.
        let marks_w = if has_marks {
            char_width.round() as u16
        } else {
            0
        };
        let border_w = crate::model::gutter_border_x_scaled(
            char_width, metrics, line_count, has_marks,
        )
        .round() as u16;
        let numbers_w = border_w.saturating_sub(marks_w);
        Self {
            marks_w,
            numbers_w,
            fold_w: 0,
            diff_w: 0,
        }
    }

    /// Total gutter width in pixels, from the group's left edge to the border.
    pub fn total_width(&self) -> usize {
        self.marks_w as usize
            + self.numbers_w as usize
            + self.fold_w as usize
            + self.diff_w as usize
    }

    /// Which lane, if any, contains `x_in_gutter` (pixels from the group's
    /// left edge). Lanes left to right: marks, line numbers, fold, diff —
    /// matching the visual order in editor-decorations.md. A zero-width
    /// lane (no active consumer) is skipped, never matched.
    pub fn lane_at(&self, x_in_gutter: usize) -> Option<LaneId> {
        let mut cursor = 0usize;
        for (lane, width) in [
            (LaneId::Marks, self.marks_w as usize),
            (LaneId::LineNumbers, self.numbers_w as usize),
            (LaneId::Fold, self.fold_w as usize),
            (LaneId::Diff, self.diff_w as usize),
        ] {
            if width == 0 {
                continue;
            }
            if x_in_gutter < cursor + width {
                return Some(lane);
            }
            cursor += width;
        }
        None
    }
}

/// One of the gutter's vertical lanes (see `GutterLayout`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaneId {
    Marks,
    LineNumbers,
    Fold,
    Diff,
}

impl LaneId {
    /// Interactive lanes consume gutter press/drag instead of falling
    /// through to the default focus/drag-select behavior (see
    /// editor-decorations.md's Hit-Testing & Interaction section).
    pub fn is_interactive(&self) -> bool {
        matches!(self, LaneId::Fold | LaneId::Marks)
    }
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
    /// Gutter lane widths for this group's document
    pub gutter: GutterLayout,
    /// Gutter border X position (absolute window coordinate)
    pub gutter_right_x: usize,
    /// X coordinate where text content starts (absolute window coordinate)
    pub text_start_x: usize,
}

impl GroupLayout {
    /// Create a new GroupLayout from an editor group.
    ///
    /// All positioning values are computed using scaled metrics from the model,
    /// ensuring DPI-correct rendering on all displays. Gutter width is derived
    /// from the group's active document line count.
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

        let document = model.editor_area.document_for_group(group);
        let line_count = document.map(|doc| doc.line_count()).unwrap_or(1);
        let has_marks = document.is_some_and(|doc| !doc.diagnostics.is_empty());

        let rect_x = group_rect.x.round() as usize;
        let gutter = GutterLayout::new(char_width, metrics, line_count, has_marks);
        let gutter_right_x = rect_x + gutter.total_width();
        let text_start_x = rect_x
            + crate::model::text_start_x_scaled(char_width, metrics, line_count, has_marks).round()
                as usize;

        Self {
            group_rect,
            content_rect,
            tab_bar_height,
            gutter,
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

    // =========================================================================
    // Content-level accessors (below tab bar)
    // =========================================================================

    /// Get absolute Y position for content area (below tab bar)
    #[inline]
    pub fn content_y(&self) -> usize {
        self.content_rect.y.round() as usize
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
            // Leave corner for vertical scrollbar; panes narrower than the
            // scrollbar itself must not produce a negative-width track.
            (cr.width - sw).max(0.0),
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

    /// Content area rect after the header, before applying insets.
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

/// Shared geometry contract for preview panes.
///
/// Preview panes have two distinct consumers:
/// - the renderer, which paints the preview chrome and native fallback content
/// - hosted preview content, which needs the exact content rect below the header
///
/// This keeps header/content split and hit-testing aligned across render,
/// hit-test, and hosted preview layout.
#[derive(Debug, Clone, Copy)]
pub struct PreviewPaneLayout {
    pub pane: Pane,
}

impl PreviewPaneLayout {
    pub fn new(rect: Rect, metrics: &crate::model::ScaledMetrics) -> Self {
        Self {
            pane: Pane::with_header(rect, metrics),
        }
    }

    #[inline]
    pub fn hosted_content_rect(&self) -> Rect {
        self.pane.content_rect()
    }

    #[inline]
    pub fn is_in_header(&self, x: f64, y: f64) -> bool {
        self.pane.is_in_header(x, y)
    }

    #[inline]
    pub fn is_in_content(&self, x: f64, y: f64) -> bool {
        self.hosted_content_rect().contains(x as f32, y as f32)
    }
}

// ============================================================================
// Modal Geometry
// ============================================================================

/// Standard padding/spacing constants for modal dialogs, in logical px.
///
/// These used to be bare `usize` constants consumed directly by layout code
/// — never multiplied by `scale_factor`, so modals were effectively
/// half-size on a 2x display. They're logical-px bases now; call the scaled
/// accessor for the value actually used in physical-pixel layout math.
pub struct ModalSpacing;

impl ModalSpacing {
    /// Outer padding inside the modal border
    const BASE_PAD: f64 = 12.0;
    /// Small gap (e.g., title to input, label to input)
    const BASE_GAP_SM: f64 = 4.0;
    /// Medium gap (e.g., between sections)
    const BASE_GAP_MD: f64 = 8.0;
    /// Input field internal vertical padding (total top+bottom)
    const BASE_INPUT_PAD_Y: f64 = 8.0;
    /// Input field internal horizontal padding (each side)
    const BASE_INPUT_PAD_X: f64 = 8.0;

    #[inline]
    fn scaled(base: f64, scale_factor: f64) -> usize {
        (base * scale_factor).round() as usize
    }

    pub fn pad(scale_factor: f64) -> usize {
        Self::scaled(Self::BASE_PAD, scale_factor)
    }

    pub fn gap_sm(scale_factor: f64) -> usize {
        Self::scaled(Self::BASE_GAP_SM, scale_factor)
    }

    pub fn gap_md(scale_factor: f64) -> usize {
        Self::scaled(Self::BASE_GAP_MD, scale_factor)
    }

    pub fn input_pad_y(scale_factor: f64) -> usize {
        Self::scaled(Self::BASE_INPUT_PAD_Y, scale_factor)
    }

    pub fn input_pad_x(scale_factor: f64) -> usize {
        Self::scaled(Self::BASE_INPUT_PAD_X, scale_factor)
    }

    /// Cap on `y = window_height / 4` for small/centered modals — logical
    /// 100px, scaled like every other modal constant.
    pub fn top_offset_cap(scale_factor: f64) -> usize {
        Self::scaled(100.0, scale_factor)
    }
}

/// A positioned widget within a modal layout
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
        scale_factor: f64,
    ) -> Self {
        let pad = ModalSpacing::pad(scale_factor);
        let content_height = vstack.height();
        let modal_height = content_height + pad * 2;
        let modal_x = (window_width.saturating_sub(modal_width)) / 2;
        let modal_y = (window_height / 4).min(ModalSpacing::top_offset_cap(scale_factor));
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
    pub fn input_height(line_height: usize, scale_factor: f64) -> usize {
        line_height + ModalSpacing::input_pad_y(scale_factor)
    }
}

// ============================================================================
// Dock Geometry
// ============================================================================

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
    pub fn compute(model: &AppModel) -> Self {
        let window_width = model.window_size.0 as f32;
        let window_height = model.window_size.1 as f32;
        let status_bar_h = model.status_bar_height as f32;
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
        // Window 600px tall, status bar height 20px -> status bar at y >= 580
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
            gutter: GutterLayout::default(),
            gutter_right_x: 48,
            text_start_x: 60,
        };

        assert_eq!(layout.visible_columns(10.0), 14);
    }

    #[test]
    fn test_gutter_layout_lane_at_skips_zero_width_lanes() {
        // Only the line-numbers lane is active (today's shipped state) —
        // any x in the gutter resolves to LineNumbers, never Marks/Fold/Diff.
        let gutter = GutterLayout {
            marks_w: 0,
            numbers_w: 40,
            fold_w: 0,
            diff_w: 0,
        };
        assert_eq!(gutter.lane_at(0), Some(LaneId::LineNumbers));
        assert_eq!(gutter.lane_at(39), Some(LaneId::LineNumbers));
        assert_eq!(gutter.lane_at(40), None);
    }

    #[test]
    fn test_gutter_layout_lane_at_orders_active_lanes_left_to_right() {
        let gutter = GutterLayout {
            marks_w: 8,
            numbers_w: 40,
            fold_w: 8,
            diff_w: 4,
        };
        assert_eq!(gutter.lane_at(0), Some(LaneId::Marks));
        assert_eq!(gutter.lane_at(7), Some(LaneId::Marks));
        assert_eq!(gutter.lane_at(8), Some(LaneId::LineNumbers));
        assert_eq!(gutter.lane_at(47), Some(LaneId::LineNumbers));
        assert_eq!(gutter.lane_at(48), Some(LaneId::Fold));
        assert_eq!(gutter.lane_at(56), Some(LaneId::Diff));
        assert_eq!(gutter.lane_at(60), None);
    }

    #[test]
    fn test_lane_id_interactivity() {
        assert!(LaneId::Marks.is_interactive());
        assert!(LaneId::Fold.is_interactive());
        assert!(!LaneId::LineNumbers.is_interactive());
        assert!(!LaneId::Diff.is_interactive());
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
        model.status_bar_height = 20;
        model.dock_layout.dock_mut(DockPosition::Right).is_open = true;
        model.dock_layout.dock_mut(DockPosition::Right).size_logical = 180.0;
        model.dock_layout.dock_mut(DockPosition::Bottom).is_open = true;
        model
            .dock_layout
            .dock_mut(DockPosition::Bottom)
            .size_logical = 140.0;

        let layout = WindowLayout::compute(&model);

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
    fn test_crlf_cursor_column_math_matches_rendered_text() {
        // Regression test for a CRLF cursor-placement bug: `get_line_cow`
        // (used for rendering) strips a trailing `\r\n`, but before this fix
        // `trim_line_ending` (used for click->column math) and `line_length`
        // (used for column clamping) only stripped the `\n`, leaving the `\r`
        // counted. That mismatch let a click past the end of "hello" land on
        // column 6 (as if the line were "hello\r") instead of clamping to 5.
        let document = crate::model::Document::with_text("hello\r\nworld\r\n");

        let rendered = document.get_line_cow(0).unwrap();
        assert_eq!(&*rendered, "hello");

        let line_text = document.get_line(0).unwrap();
        let trimmed = crate::view::helpers::trim_line_ending(&line_text);
        assert_eq!(trimmed, rendered.as_ref());

        // Simulate clicking far past the end of the line (visual column 100).
        let column = visual_col_to_char_col(trimmed, 100);
        let line_len = document.line_length(0);
        let clamped_column = column.min(line_len);

        assert_eq!(line_len, 5);
        assert_eq!(
            clamped_column, 5,
            "cursor must clamp to 'hello', not include the \\r"
        );
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

        let layout = ModalLayout::build(v, 224, 1000, 800, 1.0);
        let pad = ModalSpacing::pad(1.0);

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
            ModalLayout::input_height(20, 1.0),
            20 + ModalSpacing::input_pad_y(1.0)
        );
    }

    // ====================================================================
    // Per-modal layout tests
    // ====================================================================

    #[test]
    fn test_command_palette_layout_y_position_scales_with_scale_factor() {
        let lh = 40;
        let mut v = VStack::new(400);
        v.push(lh);
        let layout = ModalLayout::build(v, 800, 2000, 4000, 2.0);
        assert_eq!(layout.y, 200);
    }

    #[test]
    fn test_tree_list_layout_positions() {
        let metrics = crate::model::ScaledMetrics::new(1.0);
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
    fn test_preview_pane_layout_splits_header_and_hosted_content() {
        let metrics = crate::model::ScaledMetrics::new(1.0);
        let layout = PreviewPaneLayout::new(Rect::new(100.0, 40.0, 320.0, 240.0), &metrics);
        let hosted = layout.hosted_content_rect();

        assert_eq!(hosted.x, 100.0);
        assert_eq!(hosted.y, 40.0 + metrics.tab_bar_height as f32);
        assert_eq!(hosted.width, 320.0);
        assert_eq!(hosted.height, 240.0 - metrics.tab_bar_height as f32);
    }

    #[test]
    fn test_preview_pane_layout_hit_testing() {
        let metrics = crate::model::ScaledMetrics::new(1.0);
        let layout = PreviewPaneLayout::new(Rect::new(100.0, 40.0, 320.0, 240.0), &metrics);
        let header_y = 40.0 + (metrics.tab_bar_height as f64 / 2.0);
        let content_y = 40.0 + metrics.tab_bar_height as f64 + 10.0;

        assert!(layout.is_in_header(120.0, header_y));
        assert!(!layout.is_in_content(120.0, header_y));
        assert!(!layout.is_in_header(120.0, content_y));
        assert!(layout.is_in_content(120.0, content_y));
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

    /// Whether an x-coordinate lands on the collapse/expand indicator for a
    /// row at `depth`, in a container whose content starts at `base_x`.
    #[inline]
    pub fn is_on_chevron(&self, base_x: f32, depth: usize, x: f32) -> bool {
        let start = base_x + self.x_offset(depth) as f32;
        let end = start + self.indicator_width as f32;
        x >= start && x < end
    }
}
