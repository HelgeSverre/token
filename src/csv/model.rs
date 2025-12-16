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
}
