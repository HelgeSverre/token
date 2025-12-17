//! CSV data model types
//!
//! Memory-efficient storage using delimited strings instead of Vec<Vec<String>>.

use super::viewport::CsvViewport;

/// Internal delimiter for cell storage (0xFA - rarely used in real data)
pub const CELL_DELIMITER: char = '\u{00FA}';

/// Supported CSV delimiters
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Delimiter {
    #[default]
    Comma,
    Tab,
    Pipe,
    Semicolon,
}

impl Delimiter {
    /// Get the character for this delimiter
    pub fn char(self) -> char {
        match self {
            Delimiter::Comma => ',',
            Delimiter::Tab => '\t',
            Delimiter::Pipe => '|',
            Delimiter::Semicolon => ';',
        }
    }

    /// Detect delimiter from file extension
    pub fn from_extension(ext: &str) -> Self {
        match ext.to_lowercase().as_str() {
            "tsv" => Delimiter::Tab,
            "psv" => Delimiter::Pipe,
            _ => Delimiter::Comma,
        }
    }
}

/// Position of a cell in the grid
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CellPosition {
    pub row: usize,
    pub col: usize,
}

impl CellPosition {
    pub fn new(row: usize, col: usize) -> Self {
        Self { row, col }
    }
}

/// State for editing a single cell
#[derive(Debug, Clone)]
pub struct CellEditState {
    /// Position of the cell being edited
    pub position: CellPosition,
    /// Current edit buffer content
    pub buffer: String,
    /// Cursor position within buffer (byte offset)
    pub cursor: usize,
    /// Original value before editing (for cancel/undo)
    pub original: String,
}

impl CellEditState {
    /// Create new edit state for a cell
    pub fn new(position: CellPosition, value: String) -> Self {
        let cursor = value.len();
        Self {
            position,
            buffer: value.clone(),
            cursor,
            original: value,
        }
    }

    /// Create new edit state starting with a character (replaces content)
    pub fn with_char(position: CellPosition, original: String, ch: char) -> Self {
        Self {
            position,
            buffer: ch.to_string(),
            cursor: ch.len_utf8(),
            original,
        }
    }

    /// Insert character at cursor position
    pub fn insert_char(&mut self, ch: char) {
        self.buffer.insert(self.cursor, ch);
        self.cursor += ch.len_utf8();
    }

    /// Delete character before cursor (backspace)
    pub fn delete_backward(&mut self) {
        if self.cursor > 0 {
            let prev_boundary = self.buffer[..self.cursor]
                .char_indices()
                .last()
                .map(|(i, _)| i)
                .unwrap_or(0);
            self.buffer.remove(prev_boundary);
            self.cursor = prev_boundary;
        }
    }

    /// Delete character at cursor (delete)
    pub fn delete_forward(&mut self) {
        if self.cursor < self.buffer.len() {
            self.buffer.remove(self.cursor);
        }
    }

    /// Move cursor left
    pub fn cursor_left(&mut self) {
        if self.cursor > 0 {
            self.cursor = self.buffer[..self.cursor]
                .char_indices()
                .last()
                .map(|(i, _)| i)
                .unwrap_or(0);
        }
    }

    /// Move cursor right
    pub fn cursor_right(&mut self) {
        if self.cursor < self.buffer.len() {
            if let Some((_, ch)) = self.buffer[self.cursor..].char_indices().next() {
                self.cursor += ch.len_utf8();
            }
        }
    }

    /// Move cursor to start
    pub fn cursor_home(&mut self) {
        self.cursor = 0;
    }

    /// Move cursor to end
    pub fn cursor_end(&mut self) {
        self.cursor = self.buffer.len();
    }

    /// Check if content changed from original
    pub fn is_modified(&self) -> bool {
        self.buffer != self.original
    }

    /// Get cursor position in characters (for rendering)
    pub fn cursor_char_position(&self) -> usize {
        self.buffer[..self.cursor].chars().count()
    }
}

/// Represents a completed cell edit for sync/undo
#[derive(Debug, Clone)]
pub struct CellEdit {
    pub position: CellPosition,
    pub old_value: String,
    pub new_value: String,
}

/// Memory-efficient CSV data storage
///
/// Instead of storing `Vec<Vec<String>>` which has significant overhead,
/// each row is stored as a single string with cells delimited by CELL_DELIMITER (0xFA).
/// This reduces memory allocations while still allowing O(1) row access.
#[derive(Debug, Clone, Default)]
pub struct CsvData {
    /// Each row stored as delimiter-separated string
    rows: Vec<String>,
    /// Number of columns (max across all rows)
    column_count: usize,
}

impl CsvData {
    /// Create empty CSV data
    pub fn new() -> Self {
        Self::default()
    }

    /// Create CSV data from parsed rows
    pub fn from_rows(parsed_rows: Vec<Vec<String>>) -> Self {
        let column_count = parsed_rows.iter().map(|r| r.len()).max().unwrap_or(0);

        let rows = parsed_rows
            .into_iter()
            .map(|row| row.join(&CELL_DELIMITER.to_string()))
            .collect();

        Self { rows, column_count }
    }

    /// Get number of rows
    pub fn row_count(&self) -> usize {
        self.rows.len()
    }

    /// Get number of columns
    pub fn column_count(&self) -> usize {
        self.column_count
    }

    /// Get cell value at position
    pub fn get(&self, row: usize, col: usize) -> &str {
        if row >= self.rows.len() {
            return "";
        }

        let row_str = &self.rows[row];
        let mut col_idx = 0;
        let mut start = 0;

        for (i, c) in row_str.char_indices() {
            if c == CELL_DELIMITER {
                if col_idx == col {
                    return &row_str[start..i];
                }
                col_idx += 1;
                start = i + c.len_utf8();
            }
        }

        if col_idx == col {
            return &row_str[start..];
        }

        ""
    }

    /// Get entire row as iterator over cells
    pub fn row_cells(&self, row: usize) -> impl Iterator<Item = &str> {
        self.rows
            .get(row)
            .map(|s| s.as_str())
            .unwrap_or("")
            .split(CELL_DELIMITER)
    }

    /// Set cell value at position
    pub fn set(&mut self, row: usize, col: usize, value: &str) {
        if row >= self.rows.len() {
            return;
        }

        let cells: Vec<&str> = self.rows[row].split(CELL_DELIMITER).collect();
        let mut new_cells: Vec<String> = cells.iter().map(|s| s.to_string()).collect();

        while new_cells.len() <= col {
            new_cells.push(String::new());
        }

        new_cells[col] = value.to_string();
        self.rows[row] = new_cells.join(&CELL_DELIMITER.to_string());

        if col >= self.column_count {
            self.column_count = col + 1;
        }
    }

    /// Check if data is empty
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
}

/// State for CSV view mode
#[derive(Debug, Clone)]
pub struct CsvState {
    /// Parsed CSV data
    pub data: CsvData,
    /// Currently selected cell
    pub selected_cell: CellPosition,
    /// Viewport for visible region
    pub viewport: CsvViewport,
    /// Original delimiter used in file
    pub delimiter: Delimiter,
    /// Whether first row is a header
    pub has_header_row: bool,
    /// Calculated column widths (in characters)
    pub column_widths: Vec<usize>,
    /// Cell editing state (Some when editing a cell)
    pub editing: Option<CellEditState>,
}

impl CsvState {
    /// Create new CSV state from parsed data
    pub fn new(data: CsvData, delimiter: Delimiter) -> Self {
        let column_widths = Self::calculate_column_widths(&data);

        Self {
            data,
            selected_cell: CellPosition::default(),
            viewport: CsvViewport::default(),
            delimiter,
            has_header_row: true,
            column_widths,
            editing: None,
        }
    }

    /// Calculate optimal column widths based on content
    fn calculate_column_widths(data: &CsvData) -> Vec<usize> {
        const MIN_WIDTH: usize = 4;
        const MAX_WIDTH: usize = 40;

        let mut widths = vec![MIN_WIDTH; data.column_count()];

        for row in 0..data.row_count().min(100) {
            for (col, cell) in data.row_cells(row).enumerate() {
                if col < widths.len() {
                    let cell_width = cell.chars().count();
                    widths[col] = widths[col].max(cell_width).min(MAX_WIDTH);
                }
            }
        }

        widths
    }

    /// Ensure selected cell is within valid bounds
    pub fn clamp_selection(&mut self) {
        let max_row = self.data.row_count().saturating_sub(1);
        let max_col = self.data.column_count().saturating_sub(1);

        self.selected_cell.row = self.selected_cell.row.min(max_row);
        self.selected_cell.col = self.selected_cell.col.min(max_col);
    }

    /// Select a specific cell and ensure it's visible
    pub fn select_cell(&mut self, row: usize, col: usize) {
        self.selected_cell = CellPosition::new(row, col);
        self.clamp_selection();
        self.viewport.ensure_visible(
            self.selected_cell.row,
            self.selected_cell.col,
            self.data.row_count(),
            self.data.column_count(),
        );
    }

    /// Check if currently editing a cell
    pub fn is_editing(&self) -> bool {
        self.editing.is_some()
    }

    /// Start editing the selected cell
    pub fn start_editing(&mut self) {
        let value = self
            .data
            .get(self.selected_cell.row, self.selected_cell.col);
        self.editing = Some(CellEditState::new(self.selected_cell, value.to_string()));
    }

    /// Start editing with initial character (replaces cell content)
    pub fn start_editing_with_char(&mut self, ch: char) {
        let original = self
            .data
            .get(self.selected_cell.row, self.selected_cell.col)
            .to_string();
        self.editing = Some(CellEditState::with_char(self.selected_cell, original, ch));
    }

    /// Confirm edit and update data, returning the edit operation if changed
    pub fn confirm_edit(&mut self) -> Option<CellEdit> {
        let edit_state = self.editing.take()?;

        if !edit_state.is_modified() {
            return None;
        }

        let edit = CellEdit {
            position: edit_state.position,
            old_value: edit_state.original,
            new_value: edit_state.buffer.clone(),
        };

        self.data
            .set(edit.position.row, edit.position.col, &edit.new_value);

        Some(edit)
    }

    /// Cancel edit and discard changes
    pub fn cancel_edit(&mut self) {
        self.editing = None;
    }

    /// Insert character into current edit
    pub fn edit_insert_char(&mut self, ch: char) {
        if let Some(edit) = &mut self.editing {
            edit.insert_char(ch);
        }
    }

    /// Delete backward in current edit
    pub fn edit_delete_backward(&mut self) {
        if let Some(edit) = &mut self.editing {
            edit.delete_backward();
        }
    }

    /// Delete forward in current edit
    pub fn edit_delete_forward(&mut self) {
        if let Some(edit) = &mut self.editing {
            edit.delete_forward();
        }
    }

    /// Move cursor left in current edit
    pub fn edit_cursor_left(&mut self) {
        if let Some(edit) = &mut self.editing {
            edit.cursor_left();
        }
    }

    /// Move cursor right in current edit
    pub fn edit_cursor_right(&mut self) {
        if let Some(edit) = &mut self.editing {
            edit.cursor_right();
        }
    }

    /// Move cursor to start in current edit
    pub fn edit_cursor_home(&mut self) {
        if let Some(edit) = &mut self.editing {
            edit.cursor_home();
        }
    }

    /// Move cursor to end in current edit
    pub fn edit_cursor_end(&mut self) {
        if let Some(edit) = &mut self.editing {
            edit.cursor_end();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_csv_data_from_rows() {
        let rows = vec![
            vec!["a".to_string(), "b".to_string(), "c".to_string()],
            vec!["1".to_string(), "2".to_string()],
        ];
        let data = CsvData::from_rows(rows);

        assert_eq!(data.row_count(), 2);
        assert_eq!(data.column_count(), 3);
    }

    #[test]
    fn test_csv_data_get() {
        let rows = vec![
            vec!["name".to_string(), "age".to_string()],
            vec!["Alice".to_string(), "30".to_string()],
        ];
        let data = CsvData::from_rows(rows);

        assert_eq!(data.get(0, 0), "name");
        assert_eq!(data.get(0, 1), "age");
        assert_eq!(data.get(1, 0), "Alice");
        assert_eq!(data.get(1, 1), "30");
        assert_eq!(data.get(1, 2), "");
        assert_eq!(data.get(5, 0), "");
    }

    #[test]
    fn test_csv_data_set() {
        let rows = vec![vec!["a".to_string(), "b".to_string()]];
        let mut data = CsvData::from_rows(rows);

        data.set(0, 0, "updated");
        assert_eq!(data.get(0, 0), "updated");

        data.set(0, 1, "also updated");
        assert_eq!(data.get(0, 1), "also updated");
    }

    #[test]
    fn test_delimiter_from_extension() {
        assert_eq!(Delimiter::from_extension("csv"), Delimiter::Comma);
        assert_eq!(Delimiter::from_extension("CSV"), Delimiter::Comma);
        assert_eq!(Delimiter::from_extension("tsv"), Delimiter::Tab);
        assert_eq!(Delimiter::from_extension("psv"), Delimiter::Pipe);
    }

    #[test]
    fn test_row_cells_iterator() {
        let rows = vec![vec!["a".to_string(), "b".to_string(), "c".to_string()]];
        let data = CsvData::from_rows(rows);

        let cells: Vec<&str> = data.row_cells(0).collect();
        assert_eq!(cells, vec!["a", "b", "c"]);
    }

    #[test]
    fn test_cell_edit_state_new() {
        let pos = CellPosition::new(1, 2);
        let edit = CellEditState::new(pos, "hello".to_string());

        assert_eq!(edit.position, pos);
        assert_eq!(edit.buffer, "hello");
        assert_eq!(edit.cursor, 5);
        assert_eq!(edit.original, "hello");
        assert!(!edit.is_modified());
    }

    #[test]
    fn test_cell_edit_state_with_char() {
        let pos = CellPosition::new(0, 0);
        let edit = CellEditState::with_char(pos, "old".to_string(), 'x');

        assert_eq!(edit.buffer, "x");
        assert_eq!(edit.cursor, 1);
        assert_eq!(edit.original, "old");
        assert!(edit.is_modified());
    }

    #[test]
    fn test_cell_edit_state_insert_char() {
        let pos = CellPosition::new(0, 0);
        let mut edit = CellEditState::new(pos, "ab".to_string());
        edit.cursor = 1;
        edit.insert_char('X');

        assert_eq!(edit.buffer, "aXb");
        assert_eq!(edit.cursor, 2);
    }

    #[test]
    fn test_cell_edit_state_delete_backward() {
        let pos = CellPosition::new(0, 0);
        let mut edit = CellEditState::new(pos, "abc".to_string());
        edit.cursor = 2;
        edit.delete_backward();

        assert_eq!(edit.buffer, "ac");
        assert_eq!(edit.cursor, 1);
    }

    #[test]
    fn test_cell_edit_state_cursor_movement() {
        let pos = CellPosition::new(0, 0);
        let mut edit = CellEditState::new(pos, "hello".to_string());

        edit.cursor_home();
        assert_eq!(edit.cursor, 0);

        edit.cursor_right();
        assert_eq!(edit.cursor, 1);

        edit.cursor_end();
        assert_eq!(edit.cursor, 5);

        edit.cursor_left();
        assert_eq!(edit.cursor, 4);
    }

    #[test]
    fn test_csv_state_editing_lifecycle() {
        use super::super::parse_csv;

        let content = "a,b,c\n1,2,3\n";
        let data = parse_csv(content, Delimiter::Comma).unwrap();
        let mut state = CsvState::new(data, Delimiter::Comma);

        assert!(!state.is_editing());

        state.start_editing();
        assert!(state.is_editing());
        assert_eq!(state.editing.as_ref().unwrap().buffer, "a");

        state.edit_insert_char('X');
        assert_eq!(state.editing.as_ref().unwrap().buffer, "aX");

        let edit = state.confirm_edit();
        assert!(edit.is_some());
        assert!(!state.is_editing());
        assert_eq!(state.data.get(0, 0), "aX");
    }

    #[test]
    fn test_csv_state_cancel_edit() {
        use super::super::parse_csv;

        let content = "a,b,c\n1,2,3\n";
        let data = parse_csv(content, Delimiter::Comma).unwrap();
        let mut state = CsvState::new(data, Delimiter::Comma);

        state.start_editing();
        state.edit_insert_char('X');
        state.cancel_edit();

        assert!(!state.is_editing());
        assert_eq!(state.data.get(0, 0), "a");
    }
}
