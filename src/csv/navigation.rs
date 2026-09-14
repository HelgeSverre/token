//! Cell navigation logic for CSV mode
//!
//! Handles arrow key movement, Tab/Shift+Tab, Enter, and viewport scrolling.

use super::model::{CellPosition, CsvState};

impl CsvState {
    /// Move selection by delta (handles bounds)
    pub fn move_selection(&mut self, delta_row: i32, delta_col: i32) {
        let new_row = (self.selected_cell.row as i32 + delta_row)
            .max(0)
            .min(self.data.row_count().saturating_sub(1) as i32) as usize;

        let new_col = (self.selected_cell.col as i32 + delta_col)
            .max(0)
            .min(self.data.column_count().saturating_sub(1) as i32) as usize;

        self.selected_cell = CellPosition::new(new_row, new_col);
        self.ensure_selection_visible();
    }

    /// Move to next cell (Tab behavior)
    pub fn move_to_next_cell(&mut self) {
        let col_count = self.data.column_count();
        let row_count = self.data.row_count();

        if col_count == 0 || row_count == 0 {
            return;
        }

        let mut new_col = self.selected_cell.col + 1;
        let mut new_row = self.selected_cell.row;

        if new_col >= col_count {
            new_col = 0;
            new_row += 1;
            if new_row >= row_count {
                new_row = row_count - 1;
                new_col = col_count - 1;
            }
        }

        self.selected_cell = CellPosition::new(new_row, new_col);
        self.ensure_selection_visible();
    }

    /// Move to previous cell (Shift+Tab behavior)
    pub fn move_to_prev_cell(&mut self) {
        let col_count = self.data.column_count();
        let row_count = self.data.row_count();

        if col_count == 0 || row_count == 0 {
            return;
        }

        if self.selected_cell.col > 0 {
            self.selected_cell.col -= 1;
        } else if self.selected_cell.row > 0 {
            self.selected_cell.row -= 1;
            self.selected_cell.col = col_count - 1;
        }

        self.ensure_selection_visible();
    }

    /// Move to first cell (Cmd+Home)
    pub fn move_to_first_cell(&mut self) {
        self.selected_cell = CellPosition::new(0, 0);
        self.ensure_selection_visible();
    }

    /// Move to last cell (Cmd+End)
    pub fn move_to_last_cell(&mut self) {
        let row = self.data.row_count().saturating_sub(1);
        let col = self.data.column_count().saturating_sub(1);
        self.selected_cell = CellPosition::new(row, col);
        self.ensure_selection_visible();
    }

    /// Move to first column in current row (Home)
    pub fn move_to_row_start(&mut self) {
        self.selected_cell.col = 0;
        self.ensure_selection_visible();
    }

    /// Move to last column in current row (End)
    pub fn move_to_row_end(&mut self) {
        self.selected_cell.col = self.data.column_count().saturating_sub(1);
        self.ensure_selection_visible();
    }

    /// Page up navigation
    pub fn page_up(&mut self) {
        let page_size = self.viewport.visible_rows.max(1);
        let new_row = self.selected_cell.row.saturating_sub(page_size);
        self.selected_cell.row = new_row;
        self.ensure_selection_visible();
    }

    /// Page down navigation
    pub fn page_down(&mut self) {
        let page_size = self.viewport.visible_rows.max(1);
        let new_row =
            (self.selected_cell.row + page_size).min(self.data.row_count().saturating_sub(1));
        self.selected_cell.row = new_row;
        self.ensure_selection_visible();
    }

    /// Ensure the selected cell is visible, scrolling viewport if necessary
    pub fn ensure_selection_visible(&mut self) {
        self.viewport.ensure_visible(
            self.selected_cell.row,
            self.selected_cell.col,
            self.data.row_count(),
            self.data.column_count(),
        );
    }

    /// Synchronize viewport dimensions, revealing the selection only on resize.
    pub fn set_viewport_size(&mut self, rows: usize, cols: usize) {
        // Rendering synchronizes dimensions every frame. Leave manual scrolling
        // intact when the viewport size has not changed.
        if self.viewport.visible_rows == rows && self.viewport.visible_cols == cols {
            return;
        }
        self.viewport.visible_rows = rows;
        self.viewport.visible_cols = cols;
        self.ensure_selection_visible();
    }

    /// Scroll viewport vertically (from mouse wheel)
    pub fn scroll_vertical(&mut self, delta: i32) {
        let max_top = self
            .data
            .row_count()
            .saturating_sub(self.viewport.visible_rows.max(1));
        let new_top = (self.viewport.top_row as i32 + delta)
            .max(0)
            .min(max_top as i32) as usize;
        self.viewport.top_row = new_top;
    }

    /// Scroll viewport horizontally (from mouse wheel)
    pub fn scroll_horizontal(&mut self, delta: i32) {
        let max_left = self
            .data
            .column_count()
            .saturating_sub(self.viewport.visible_cols.max(1));
        let new_left = (self.viewport.left_col as i32 + delta)
            .max(0)
            .min(max_left as i32) as usize;
        self.viewport.left_col = new_left;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::csv::{parse_csv, Delimiter};

    fn make_csv_state(rows: usize, cols: usize) -> CsvState {
        let mut content = String::new();
        for r in 0..rows {
            for c in 0..cols {
                if c > 0 {
                    content.push(',');
                }
                content.push_str(&format!("r{}c{}", r, c));
            }
            content.push('\n');
        }
        let data = parse_csv(&content, Delimiter::Comma).unwrap();
        CsvState::new(data, Delimiter::Comma)
    }

    #[test]
    fn test_move_selection_down() {
        let mut state = make_csv_state(5, 3);
        state.move_selection(1, 0);
        assert_eq!(state.selected_cell.row, 1);
        assert_eq!(state.selected_cell.col, 0);
    }

    #[test]
    fn test_move_selection_right() {
        let mut state = make_csv_state(5, 3);
        state.move_selection(0, 1);
        assert_eq!(state.selected_cell.row, 0);
        assert_eq!(state.selected_cell.col, 1);
    }

    #[test]
    fn test_move_selection_clamped() {
        let mut state = make_csv_state(5, 3);
        state.move_selection(-10, 0);
        assert_eq!(state.selected_cell.row, 0);

        state.move_selection(100, 0);
        assert_eq!(state.selected_cell.row, 4);
    }

    #[test]
    fn test_move_to_next_cell_wrap() {
        let mut state = make_csv_state(3, 3);
        state.selected_cell = CellPosition::new(0, 2);
        state.move_to_next_cell();
        assert_eq!(state.selected_cell.row, 1);
        assert_eq!(state.selected_cell.col, 0);
    }

    #[test]
    fn test_move_to_prev_cell_wrap() {
        let mut state = make_csv_state(3, 3);
        state.selected_cell = CellPosition::new(1, 0);
        state.move_to_prev_cell();
        assert_eq!(state.selected_cell.row, 0);
        assert_eq!(state.selected_cell.col, 2);
    }

    #[test]
    fn test_move_to_first_last_cell() {
        let mut state = make_csv_state(5, 4);
        state.selected_cell = CellPosition::new(2, 2);

        state.move_to_first_cell();
        assert_eq!(state.selected_cell.row, 0);
        assert_eq!(state.selected_cell.col, 0);

        state.move_to_last_cell();
        assert_eq!(state.selected_cell.row, 4);
        assert_eq!(state.selected_cell.col, 3);
    }

    #[test]
    fn test_page_navigation() {
        let mut state = make_csv_state(100, 5);
        state.viewport.visible_rows = 10;

        state.page_down();
        assert_eq!(state.selected_cell.row, 10);

        state.page_up();
        assert_eq!(state.selected_cell.row, 0);
    }

    #[test]
    fn test_viewport_sync_preserves_manual_scrolling() {
        let mut state = make_csv_state(100, 20);
        state.set_viewport_size(10, 5);

        state.scroll_vertical(20);
        state.scroll_horizontal(10);
        state.set_viewport_size(10, 5);

        assert_eq!(state.viewport.top_row, 20);
        assert_eq!(state.viewport.left_col, 10);
        assert_eq!(state.selected_cell, CellPosition::new(0, 0));

        state.move_to_last_cell();
        state.scroll_vertical(-20);
        state.scroll_horizontal(-10);
        let top_row = state.viewport.top_row;
        let left_col = state.viewport.left_col;
        state.set_viewport_size(10, 5);

        assert_eq!(state.viewport.top_row, top_row);
        assert_eq!(state.viewport.left_col, left_col);
        assert_eq!(state.selected_cell, CellPosition::new(99, 19));
    }

    #[test]
    fn test_navigation_reveals_selection_after_manual_scrolling() {
        let mut state = make_csv_state(100, 20);
        state.set_viewport_size(10, 5);
        state.scroll_vertical(20);
        state.scroll_horizontal(10);

        state.move_selection(1, 1);

        assert_eq!(state.selected_cell, CellPosition::new(1, 1));
        assert_eq!(state.viewport.top_row, 1);
        assert_eq!(state.viewport.left_col, 1);
    }

    #[test]
    fn test_viewport_resize_reveals_selection() {
        let mut state = make_csv_state(100, 20);
        state.set_viewport_size(10, 5);
        state.scroll_vertical(20);
        state.set_viewport_size(11, 5);
        assert_eq!(state.viewport.top_row, 0);

        state.scroll_horizontal(10);
        state.set_viewport_size(11, 6);
        assert_eq!(state.viewport.left_col, 0);
    }
}
