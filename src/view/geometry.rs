//! Centralized geometry helpers for rendering and hit-testing
//!
//! This module provides a single source of truth for layout calculations,
//! coordinate transformations, and hit-testing that is shared between
//! the view (rendering) and runtime (input handling) layers.
//!
//! All functions here are pure (no I/O, no side effects) and can be
//! tested independently of the rendering infrastructure.

use token::model::editor_area::{EditorGroup, Rect};
use token::model::AppModel;

// ============================================================================
// Layout Constants
// ============================================================================

/// Height of the tab bar in pixels (base value at scale factor 1.0)
/// For actual rendering, use `model.metrics.tab_bar_height`
pub const TAB_BAR_HEIGHT: usize = 28;

/// Get tab bar height from model's scaled metrics
#[inline]
#[allow(dead_code)]
pub fn tab_bar_height(model: &AppModel) -> usize {
    model.metrics.tab_bar_height
}

// Re-export TABULATOR_WIDTH from util::text for single source of truth
pub use token::util::text::TABULATOR_WIDTH;

// Re-export model geometry helpers for unified access
pub use token::model::{gutter_border_x, text_start_x};

// ============================================================================
// Viewport Sizing Helpers
// ============================================================================

/// Calculate the height of the status bar in pixels
#[inline]
pub fn status_bar_height(line_height: usize) -> usize {
    line_height
}

/// Compute number of visible text lines given window height
///
/// Delegates to `ViewportGeometry::compute_visible_lines()` for the canonical calculation.
/// Kept for API compatibility - prefer using `ViewportGeometry` directly.
#[inline]
#[allow(dead_code)]
pub fn compute_visible_lines(window_height: u32, line_height: usize, status_bar_h: usize) -> usize {
    token::model::ViewportGeometry::compute_visible_lines(window_height, line_height, status_bar_h)
}

/// Compute number of visible columns given window width
///
/// Delegates to `ViewportGeometry::compute_visible_columns()` for the canonical calculation.
/// Kept for API compatibility - prefer using `ViewportGeometry` directly.
#[inline]
#[allow(dead_code)]
pub fn compute_visible_columns(window_width: u32, char_width: f32) -> usize {
    token::model::ViewportGeometry::compute_visible_columns(window_width, char_width)
}

// ============================================================================
// Tab Expansion Helpers
// ============================================================================

/// Expand tab characters to spaces for display.
///
/// Converts each tab character to the appropriate number of spaces based on
/// the current visual column and `TABULATOR_WIDTH`. This is used for rendering
/// text where tabs need to be visually aligned.
///
/// # Example
/// ```ignore
/// let text = "a\tb";  // Tab at column 1
/// let expanded = expand_tabs_for_display(text);
/// assert_eq!(expanded, "a   b");  // Tab becomes 3 spaces (to reach column 4)
/// ```
pub fn expand_tabs_for_display(text: &str) -> String {
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

    result
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

/// Check if a y-coordinate is within the status bar region
#[inline]
pub fn is_in_status_bar(y: f64, window_height: u32, line_height: usize) -> bool {
    let status_bar_top = window_height as f64 - line_height as f64;
    y >= status_bar_top
}

/// Check if a y-coordinate is within the global tab bar region (top of window)
/// Note: For split views, use `is_in_group_tab_bar` instead
/// Uses the base TAB_BAR_HEIGHT constant (not scaled).
#[inline]
#[allow(dead_code)]
pub fn is_in_tab_bar(y: f64) -> bool {
    y < TAB_BAR_HEIGHT as f64
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
/// Takes into account the tab bar, gutter, scroll offset, and horizontal scrolling.
pub fn pixel_to_cursor(
    x: f64,
    y: f64,
    char_width: f32,
    line_height: f64,
    model: &AppModel,
) -> (usize, usize) {
    let text_x = token::model::text_start_x_scaled(char_width, &model.metrics).round() as f64;

    let text_start_y = model.metrics.tab_bar_height as f64;
    let adjusted_y = (y - text_start_y).max(0.0);
    let visual_line = (adjusted_y / line_height).floor() as usize;
    let line = model.editor().viewport.top_line + visual_line;
    let line = line.min(model.document().buffer.len_lines().saturating_sub(1));

    let x_offset = x - text_x;
    let visual_column = if x_offset > 0.0 {
        model.editor().viewport.left_column + (x_offset / char_width as f64).round() as usize
    } else {
        model.editor().viewport.left_column
    };

    let line_text = model.document().get_line(line).unwrap_or_default();
    let line_text_trimmed = super::helpers::trim_line_ending(&line_text);
    let column = visual_col_to_char_col(line_text_trimmed, visual_column);

    let line_len = model.document().line_length(line);
    let column = column.min(line_len);

    (line, column)
}

/// Convert pixel coordinates to line and VISUAL column (screen position).
/// Used for rectangle selection where the raw visual column is needed,
/// independent of any specific line's text content.
/// Returns (line, visual_column) where visual_column is the screen column.
pub fn pixel_to_line_and_visual_column(
    x: f64,
    y: f64,
    char_width: f32,
    line_height: f64,
    model: &AppModel,
) -> (usize, usize) {
    let text_x = token::model::text_start_x_scaled(char_width, &model.metrics).round() as f64;

    let text_start_y = model.metrics.tab_bar_height as f64;
    let adjusted_y = (y - text_start_y).max(0.0);
    let visual_line = (adjusted_y / line_height).floor() as usize;
    let line = model.editor().viewport.top_line + visual_line;
    let line = line.min(model.document().buffer.len_lines().saturating_sub(1));

    let x_offset = x - text_x;
    let visual_column = if x_offset > 0.0 {
        model.editor().viewport.left_column + (x_offset / char_width as f64).round() as usize
    } else {
        model.editor().viewport.left_column
    };

    (line, visual_column)
}

/// Convert pixel coordinates to document line and column for a specific group.
///
/// Accounts for the group's rect position within the window.
#[allow(clippy::too_many_arguments)]
#[allow(dead_code)]
pub fn pixel_to_cursor_in_group(
    x: f64,
    y: f64,
    char_width: f32,
    line_height: f64,
    group_rect: &Rect,
    model: &AppModel,
    editor: &token::model::EditorState,
    document: &token::model::Document,
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
// Layout Rect Helpers
// ============================================================================

/// Compute the content area rect for an editor group (excluding tab bar)
/// This version uses the model's scaled metrics.
#[inline]
#[allow(dead_code)]
pub fn group_content_rect_scaled(group_rect: &Rect, model: &AppModel) -> Rect {
    let tbh = model.metrics.tab_bar_height as f32;
    Rect::new(
        group_rect.x,
        group_rect.y + tbh,
        group_rect.width,
        (group_rect.height - tbh).max(0.0),
    )
}

/// Compute the content area rect for an editor group (excluding tab bar)
/// Legacy version using base TAB_BAR_HEIGHT constant.
#[inline]
pub fn group_content_rect(group_rect: &Rect) -> Rect {
    Rect::new(
        group_rect.x,
        group_rect.y + TAB_BAR_HEIGHT as f32,
        group_rect.width,
        (group_rect.height - TAB_BAR_HEIGHT as f32).max(0.0),
    )
}

/// Compute the gutter rect for an editor group
#[inline]
#[allow(dead_code)]
pub fn group_gutter_rect(group_rect: &Rect, char_width: f32) -> Rect {
    let content = group_content_rect(group_rect);
    let gutter_width = gutter_border_x(char_width);
    Rect::new(content.x, content.y, gutter_width, content.height)
}

/// Compute the text area rect for an editor group
#[inline]
#[allow(dead_code)]
pub fn group_text_area_rect(group_rect: &Rect, char_width: f32) -> Rect {
    let content = group_content_rect(group_rect);
    let text_x = text_start_x(char_width);
    Rect::new(
        content.x + text_x,
        content.y,
        (content.width - text_x).max(0.0),
        content.height,
    )
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

/// Check if a point is inside the modal dialog
pub fn is_in_modal(
    x: f64,
    y: f64,
    window_width: usize,
    window_height: usize,
    line_height: usize,
    has_list: bool,
    list_items: usize,
) -> bool {
    let (mx, my, mw, mh) = modal_bounds(
        window_width,
        window_height,
        line_height,
        has_list,
        list_items,
    );
    let px = x as usize;
    let py = y as usize;
    px >= mx && px < mx + mw && py >= my && py < my + mh
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
    fn test_is_in_tab_bar() {
        assert!(is_in_tab_bar(0.0));
        assert!(is_in_tab_bar(27.0));
        assert!(!is_in_tab_bar(28.0));
        assert!(!is_in_tab_bar(100.0));
    }

    #[test]
    fn test_compute_visible_lines() {
        // 600px height, 20px line height, 20px status bar = 580 / 20 = 29 lines
        assert_eq!(compute_visible_lines(600, 20, 20), 29);
        // Edge case: zero line height
        assert_eq!(compute_visible_lines(600, 0, 20), 25); // fallback
    }

    #[test]
    fn test_compute_visible_columns() {
        // Assume text_start_x returns ~60px for char_width=10
        // So (800 - 60) / 10 = 74 columns
        let cols = compute_visible_columns(800, 10.0);
        assert!(cols > 0);
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

    #[test]
    fn test_group_content_rect() {
        let group_rect = Rect::new(100.0, 50.0, 400.0, 300.0);
        let content = group_content_rect(&group_rect);
        assert_eq!(content.x, 100.0);
        assert_eq!(content.y, 50.0 + TAB_BAR_HEIGHT as f32);
        assert_eq!(content.width, 400.0);
        assert_eq!(content.height, 300.0 - TAB_BAR_HEIGHT as f32);
    }
}
