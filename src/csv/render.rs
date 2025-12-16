//! CSV grid rendering
//!
//! Renders the CSV spreadsheet view with:
//! - Row numbers column
//! - Column headers (A, B, C, ...)
//! - Cell grid with horizontal/vertical scrolling
//! - Selected cell highlight

use super::model::{CellPosition, CsvState};
use crate::model::editor_area::Rect;

/// Convert column index to letter(s): 0->A, 1->B, ..., 25->Z, 26->AA, etc.
pub fn column_to_letters(col: usize) -> String {
    let mut result = String::new();
    let mut n = col;
    loop {
        result.insert(0, (b'A' + (n % 26) as u8) as char);
        if n < 26 {
            break;
        }
        n = n / 26 - 1;
    }
    result
}

/// Check if a string looks like a number (for right-alignment)
pub fn is_number(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    s.parse::<f64>().is_ok()
}

/// Truncate text with ellipsis if too long
pub fn truncate_text(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else if max_chars <= 1 {
        s.chars().take(max_chars).collect()
    } else {
        let mut result: String = s.chars().take(max_chars - 1).collect();
        result.push('…');
        result
    }
}

/// Layout information for rendering
pub struct CsvRenderLayout {
    /// X offset for row header column
    pub row_header_x: usize,
    /// Width of row header column in pixels
    pub row_header_width: usize,
    /// X offset where grid content starts
    pub grid_x: usize,
    /// Y offset for column headers
    pub col_header_y: usize,
    /// Height of column header row
    pub col_header_height: usize,
    /// Y offset where grid data starts
    pub data_y: usize,
    /// Visible column indices and their x positions
    pub visible_columns: Vec<(usize, usize)>, // (col_index, x_offset)
    /// Width of each visible column in pixels
    pub column_widths_px: Vec<usize>,
}

impl CsvRenderLayout {
    /// Calculate layout from CSV state and available space
    pub fn calculate(
        csv: &CsvState,
        rect_x: usize,
        rect_w: usize,
        content_y: usize,
        line_height: usize,
        char_width: f32,
    ) -> Self {
        // Row header width: enough for row numbers + padding
        let row_count = csv.data.row_count();
        let digits = ((row_count.max(1) as f64).log10().floor() as usize) + 1;
        let min_digits = 3;
        let row_header_width = ((digits.max(min_digits) as f32 * char_width) + 16.0) as usize;

        let grid_x = rect_x + row_header_width;
        let grid_w = rect_w.saturating_sub(row_header_width);

        // Column header height
        let col_header_height = line_height;
        let data_y = content_y + col_header_height;

        // Calculate visible columns
        let mut visible_columns = Vec::new();
        let mut column_widths_px = Vec::new();
        let mut x = 0;

        for col in csv.viewport.left_col..csv.column_widths.len() {
            let col_width_chars = csv.column_widths.get(col).copied().unwrap_or(10);
            let col_width_px = ((col_width_chars as f32 * char_width) + 12.0).ceil() as usize; // padding

            if x + col_width_px > grid_w && !visible_columns.is_empty() {
                break; // Stop when overflow
            }

            visible_columns.push((col, x));
            column_widths_px.push(col_width_px);
            x += col_width_px;
        }

        Self {
            row_header_x: rect_x,
            row_header_width,
            grid_x,
            col_header_y: content_y,
            col_header_height,
            data_y,
            visible_columns,
            column_widths_px,
        }
    }
}

/// Hit-test a CSV cell given window coordinates.
///
/// Returns None if the click is outside the data grid (e.g., in headers or padding).
pub fn pixel_to_csv_cell(
    csv: &CsvState,
    group_rect: &Rect,
    x: f64,
    y: f64,
    line_height: usize,
    char_width: f32,
    tab_bar_height: usize,
) -> Option<CellPosition> {
    let local_x = x - group_rect.x as f64;
    let local_y = y - group_rect.y as f64;

    if local_x < 0.0 || local_y < 0.0 {
        return None;
    }

    let content_y = tab_bar_height;
    if local_y < content_y as f64 {
        return None;
    }

    let rect_x = 0usize;
    let rect_w = group_rect.width as usize;
    let layout = CsvRenderLayout::calculate(csv, rect_x, rect_w, content_y, line_height, char_width);

    if local_x < (layout.grid_x as f64) {
        return None;
    }

    if local_y < layout.data_y as f64 {
        return None;
    }

    let row_idx_in_view = ((local_y - layout.data_y as f64) / line_height as f64).floor() as usize;
    let row = csv.viewport.top_row + row_idx_in_view;
    if row >= csv.data.row_count() {
        return None;
    }

    let cell_x_in_grid = local_x - layout.grid_x as f64;

    for (i, (col_index, col_x_offset)) in layout.visible_columns.iter().enumerate() {
        let col_start = *col_x_offset as f64;
        let col_end = col_start + layout.column_widths_px[i] as f64;
        if cell_x_in_grid >= col_start && cell_x_in_grid < col_end {
            return Some(CellPosition::new(row, *col_index));
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_column_to_letters() {
        assert_eq!(column_to_letters(0), "A");
        assert_eq!(column_to_letters(1), "B");
        assert_eq!(column_to_letters(25), "Z");
        assert_eq!(column_to_letters(26), "AA");
        assert_eq!(column_to_letters(27), "AB");
        assert_eq!(column_to_letters(51), "AZ");
        assert_eq!(column_to_letters(52), "BA");
    }

    #[test]
    fn test_is_number() {
        assert!(is_number("123"));
        assert!(is_number("-45.67"));
        assert!(is_number("0"));
        assert!(!is_number(""));
        assert!(!is_number("abc"));
        assert!(!is_number("12abc"));
    }

    #[test]
    fn test_truncate_text() {
        assert_eq!(truncate_text("hello", 10), "hello");
        assert_eq!(truncate_text("hello world", 5), "hell…");
        assert_eq!(truncate_text("ab", 2), "ab");
        assert_eq!(truncate_text("abc", 1), "a");
    }
}
