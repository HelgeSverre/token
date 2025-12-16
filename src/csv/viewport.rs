//! CSV viewport calculations
//!
//! Tracks which portion of the CSV grid is visible.

/// Viewport state for CSV grid
#[derive(Debug, Clone, Default)]
pub struct CsvViewport {
    /// First visible row (0-indexed)
    pub top_row: usize,
    /// First visible column (0-indexed)
    pub left_col: usize,
    /// Number of rows that fit in the viewport
    pub visible_rows: usize,
    /// Approximate number of columns visible (depends on column widths)
    pub visible_cols: usize,
}

impl CsvViewport {
    /// Create a new viewport with given dimensions
    pub fn new(visible_rows: usize, visible_cols: usize) -> Self {
        Self {
            top_row: 0,
            left_col: 0,
            visible_rows,
            visible_cols,
        }
    }

    /// Ensure a cell is visible, scrolling if necessary
    pub fn ensure_visible(&mut self, row: usize, col: usize, total_rows: usize, total_cols: usize) {
        // Vertical scrolling
        if row < self.top_row {
            self.top_row = row;
        } else if row >= self.top_row + self.visible_rows && self.visible_rows > 0 {
            self.top_row = row.saturating_sub(self.visible_rows - 1);
        }

        // Horizontal scrolling
        if col < self.left_col {
            self.left_col = col;
        } else if col >= self.left_col + self.visible_cols && self.visible_cols > 0 {
            self.left_col = col.saturating_sub(self.visible_cols - 1);
        }

        // Clamp to valid range
        let max_top = total_rows.saturating_sub(self.visible_rows);
        let max_left = total_cols.saturating_sub(self.visible_cols);
        self.top_row = self.top_row.min(max_top);
        self.left_col = self.left_col.min(max_left);
    }

    /// Check if a row is visible
    pub fn is_row_visible(&self, row: usize) -> bool {
        row >= self.top_row && row < self.top_row + self.visible_rows
    }

    /// Check if a column is visible
    pub fn is_col_visible(&self, col: usize) -> bool {
        col >= self.left_col && col < self.left_col + self.visible_cols
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_viewport_ensure_visible_scroll_down() {
        let mut vp = CsvViewport::new(10, 5);
        vp.ensure_visible(15, 0, 100, 20);
        assert_eq!(vp.top_row, 6); // 15 - 9 = 6
    }

    #[test]
    fn test_viewport_ensure_visible_scroll_up() {
        let mut vp = CsvViewport::new(10, 5);
        vp.top_row = 20;
        vp.ensure_visible(5, 0, 100, 20);
        assert_eq!(vp.top_row, 5);
    }

    #[test]
    fn test_viewport_ensure_visible_scroll_right() {
        let mut vp = CsvViewport::new(10, 5);
        vp.ensure_visible(0, 8, 100, 20);
        assert_eq!(vp.left_col, 4); // 8 - 4 = 4
    }

    #[test]
    fn test_viewport_is_visible() {
        let vp = CsvViewport {
            top_row: 10,
            left_col: 5,
            visible_rows: 20,
            visible_cols: 8,
        };

        assert!(vp.is_row_visible(10));
        assert!(vp.is_row_visible(29));
        assert!(!vp.is_row_visible(9));
        assert!(!vp.is_row_visible(30));

        assert!(vp.is_col_visible(5));
        assert!(vp.is_col_visible(12));
        assert!(!vp.is_col_visible(4));
        assert!(!vp.is_col_visible(13));
    }
}
