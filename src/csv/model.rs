//! CSV data model types
//!
//! Memory-efficient storage using packed UTF-8 rows instead of Vec<Vec<String>>.

use crate::editable::{EditConstraints, EditableState, MoveTarget, StringBuffer};

use super::viewport::CsvViewport;

/// Invalid in UTF-8, so no valid cell text can collide with this separator.
const CELL_DELIMITER: u8 = 0xFF;

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
    /// Editable state for the cell content
    pub editable: EditableState<StringBuffer>,
    /// Original value before editing (for cancel/undo)
    pub original: String,
    /// Horizontal scroll offset for wide content
    pub scroll_x: usize,
    /// Original column width before editing (for cancel/restore)
    pub original_column_width: usize,
}

impl CellEditState {
    /// Create new edit state for a cell
    pub fn new(position: CellPosition, value: String, column_width: usize) -> Self {
        let mut editable =
            EditableState::new(StringBuffer::from_text(&value), EditConstraints::csv_cell());
        // Move cursor to end
        editable.move_line_end(false);
        Self {
            position,
            editable,
            original: value,
            scroll_x: 0,
            original_column_width: column_width,
        }
    }

    /// Create new edit state starting with a character (replaces content)
    pub fn with_char(
        position: CellPosition,
        original: String,
        ch: char,
        column_width: usize,
    ) -> Self {
        let mut editable = EditableState::new(
            StringBuffer::from_text(&ch.to_string()),
            EditConstraints::csv_cell(),
        );
        // Move cursor to end (after the character)
        editable.move_line_end(false);
        Self {
            position,
            editable,
            original,
            scroll_x: 0,
            original_column_width: column_width,
        }
    }

    /// Get the current buffer content
    pub fn buffer(&self) -> String {
        self.editable.text()
    }

    /// Insert character at cursor position
    pub fn insert_char(&mut self, ch: char) {
        self.editable.insert_char(ch);
    }

    /// Delete character before cursor (backspace)
    pub fn delete_backward(&mut self) {
        self.editable.delete_backward();
    }

    /// Delete character at cursor (delete)
    pub fn delete_forward(&mut self) {
        self.editable.delete_forward();
    }

    /// Shared dispatch for all cell cursor movement variants.
    ///
    /// Collapses the previously independent `cursor_*` / `cursor_*_with_selection`
    /// one-line wrappers into a single implementation.
    pub fn move_cursor(&mut self, target: MoveTarget, extend: bool) {
        match target {
            MoveTarget::Left => self.editable.move_left(extend),
            MoveTarget::Right => self.editable.move_right(extend),
            MoveTarget::LineStart => self.editable.move_line_start(extend),
            MoveTarget::LineEnd => self.editable.move_line_end(extend),
            MoveTarget::WordLeft => self.editable.move_word_left(extend),
            MoveTarget::WordRight => self.editable.move_word_right(extend),
            other => {
                debug_assert!(
                    false,
                    "move_cursor: unsupported MoveTarget for CSV cell: {other:?}"
                );
            }
        }
    }

    /// Move cursor left
    pub fn cursor_left(&mut self) {
        self.move_cursor(MoveTarget::Left, false);
    }

    /// Move cursor right
    pub fn cursor_right(&mut self) {
        self.move_cursor(MoveTarget::Right, false);
    }

    /// Move cursor to start
    pub fn cursor_home(&mut self) {
        self.move_cursor(MoveTarget::LineStart, false);
    }

    /// Move cursor to end
    pub fn cursor_end(&mut self) {
        self.move_cursor(MoveTarget::LineEnd, false);
    }

    /// Place the cursor at `column` (clamped), optionally extending the
    /// selection — mouse click inside the cell editor.
    pub fn set_cursor_column(&mut self, column: usize, extend_selection: bool) {
        self.editable.set_cursor_column(column, extend_selection);
    }

    /// Select the word around the cursor — double-click inside the editor.
    pub fn select_word(&mut self) {
        self.editable.select_word();
    }

    /// Check if content changed from original
    pub fn is_modified(&self) -> bool {
        self.editable.text() != self.original
    }

    /// Get cursor position in characters (for rendering)
    pub fn cursor_char_position(&self) -> usize {
        self.editable.cursor().column
    }

    // === New methods available through EditableState ===

    /// Move cursor left by one word
    pub fn cursor_word_left(&mut self) {
        self.move_cursor(MoveTarget::WordLeft, false);
    }

    /// Move cursor right by one word
    pub fn cursor_word_right(&mut self) {
        self.move_cursor(MoveTarget::WordRight, false);
    }

    /// Delete word before cursor
    pub fn delete_word_backward(&mut self) {
        self.editable.delete_word_backward();
    }

    /// Delete word after cursor
    pub fn delete_word_forward(&mut self) {
        self.editable.delete_word_forward();
    }

    /// Select all text
    pub fn select_all(&mut self) {
        self.editable.select_all();
    }

    /// Check if there's a selection
    pub fn has_selection(&self) -> bool {
        self.editable.has_selection()
    }

    /// Get the selected text
    pub fn selected_text(&self) -> String {
        self.editable.selected_text()
    }

    /// Undo last operation
    pub fn undo(&mut self) -> bool {
        self.editable.undo()
    }

    /// Redo last undone operation
    pub fn redo(&mut self) -> bool {
        self.editable.redo()
    }

    // === Selection Movement ===

    /// Move cursor left with selection
    pub fn cursor_left_with_selection(&mut self) {
        self.move_cursor(MoveTarget::Left, true);
    }

    /// Move cursor right with selection
    pub fn cursor_right_with_selection(&mut self) {
        self.move_cursor(MoveTarget::Right, true);
    }

    /// Move cursor to start with selection
    pub fn cursor_home_with_selection(&mut self) {
        self.move_cursor(MoveTarget::LineStart, true);
    }

    /// Move cursor to end with selection
    pub fn cursor_end_with_selection(&mut self) {
        self.move_cursor(MoveTarget::LineEnd, true);
    }

    /// Move cursor word left with selection
    pub fn cursor_word_left_with_selection(&mut self) {
        self.move_cursor(MoveTarget::WordLeft, true);
    }

    /// Move cursor word right with selection
    pub fn cursor_word_right_with_selection(&mut self) {
        self.move_cursor(MoveTarget::WordRight, true);
    }

    /// Insert text at cursor (for paste)
    pub fn insert_text(&mut self, text: &str) {
        self.editable.insert_text(text);
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
/// each row is stored as a single byte buffer with cells delimited by 0xFF.
/// This reduces memory allocations while still allowing O(1) row access.
#[derive(Debug, Clone, Default)]
pub struct CsvData {
    /// Valid UTF-8 cells separated by the non-UTF-8 byte CELL_DELIMITER.
    rows: Vec<Vec<u8>>,
    /// Number of columns (max across all rows)
    column_count: usize,
    /// Parser record ranges, including terminators; never physical line numbers.
    record_ranges: Vec<std::ops::Range<usize>>,
    /// Fenwick tree of edit deltas: prefix sums shift parser offsets in O(log n).
    record_deltas: Vec<isize>,
    pub(super) source: Option<ropey::Rope>,
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
            .map(|row| {
                row.iter()
                    .map(|cell| cell.as_bytes())
                    .collect::<Vec<_>>()
                    .join(&CELL_DELIMITER)
            })
            .collect();

        Self {
            rows,
            column_count,
            ..Self::default()
        }
    }

    /// Append directly from the parser's reusable record, without per-cell strings.
    pub(super) fn push_record(&mut self, record: &csv::StringRecord, end: usize) {
        let mut row = Vec::with_capacity(record.as_slice().len() + record.len().saturating_sub(1));
        for (index, cell) in record.iter().enumerate() {
            if index > 0 {
                row.push(CELL_DELIMITER);
            }
            row.extend_from_slice(cell.as_bytes());
        }
        self.column_count = self.column_count.max(record.len());
        self.rows.push(row);
        self.record_ranges
            .push(record.position().expect("parser record position").byte() as usize..end);
        self.record_deltas.push(0);
    }

    pub(crate) fn record_range(
        &self,
        row: usize,
        source: &ropey::Rope,
    ) -> Option<std::ops::Range<usize>> {
        self.source.as_ref().filter(|old| old.is_instance(source))?;
        let range = self.record_ranges.get(row)?;
        Some(
            range.start.checked_add_signed(self.record_shift(row))?
                ..range.end.checked_add_signed(self.record_shift(row + 1))?,
        )
    }

    fn record_shift(&self, mut count: usize) -> isize {
        let mut shift = 0;
        while count > 0 {
            shift += self.record_deltas[count - 1];
            count &= count - 1;
        }
        shift
    }

    pub(crate) fn matches_source(&self, source: &ropey::Rope) -> bool {
        self.source
            .as_ref()
            .is_some_and(|old| old.is_instance(source))
    }

    /// Adjust the index in O(log n), regardless of which record changed.
    pub(crate) fn record_edited(
        &mut self,
        row: usize,
        removed: usize,
        inserted: usize,
        source: &ropey::Rope,
    ) {
        let delta = inserted as isize - removed as isize;
        let mut index = row + 1;
        while index <= self.record_deltas.len() {
            self.record_deltas[index - 1] += delta;
            index += index.isolate_lowest_one();
        }
        self.source = Some(source.clone());
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
        self.row_cells(row).nth(col).unwrap_or("")
    }

    /// Get entire row as iterator over cells
    pub fn row_cells(&self, row: usize) -> impl Iterator<Item = &str> {
        self.rows
            .get(row)
            .map(Vec::as_slice)
            .unwrap_or(&[])
            .split(|&byte| byte == CELL_DELIMITER)
            .map(|cell| std::str::from_utf8(cell).expect("packed cells originate from valid UTF-8"))
    }

    /// Set cell value at position
    pub fn set(&mut self, row: usize, col: usize, value: &str) {
        if row >= self.rows.len() {
            return;
        }

        let mut new_cells: Vec<&[u8]> = self.rows[row]
            .split(|&byte| byte == CELL_DELIMITER)
            .collect();

        while new_cells.len() <= col {
            new_cells.push(&[]);
        }

        new_cells[col] = value.as_bytes();
        self.rows[row] = new_cells.join(&CELL_DELIMITER);

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
    /// Available grid rectangle width and font advance, synchronized by layout.
    pub(super) viewport_geometry: Option<(usize, f32)>,
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
            viewport_geometry: None,
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
        let col_width = self
            .column_widths
            .get(self.selected_cell.col)
            .copied()
            .unwrap_or(4);
        self.editing = Some(CellEditState::new(
            self.selected_cell,
            value.to_string(),
            col_width,
        ));
    }

    /// Start editing the selected cell with the cursor at `column` (clamped)
    /// rather than at the end — double-click placement.
    pub fn start_editing_at(&mut self, column: usize) {
        self.start_editing();
        if let Some(edit) = &mut self.editing {
            edit.set_cursor_column(column, false);
        }
    }

    /// Start editing with initial character (replaces cell content)
    pub fn start_editing_with_char(&mut self, ch: char) {
        let original = self
            .data
            .get(self.selected_cell.row, self.selected_cell.col)
            .to_string();
        let col_width = self
            .column_widths
            .get(self.selected_cell.col)
            .copied()
            .unwrap_or(4);
        self.editing = Some(CellEditState::with_char(
            self.selected_cell,
            original,
            ch,
            col_width,
        ));
    }

    /// Confirm edit and update data, returning the edit operation if changed
    pub fn confirm_edit(&mut self) -> Option<CellEdit> {
        let edit_state = self.editing.take()?;

        if !edit_state.is_modified() {
            return None;
        }

        let new_value = edit_state.buffer();
        let edit = CellEdit {
            position: edit_state.position,
            old_value: edit_state.original,
            new_value: new_value.clone(),
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
    fn packed_cells_preserve_all_utf8_including_old_separator_and_nul() {
        let cells = vec![
            "ú".to_owned(),
            "".to_owned(),
            "🙂\0ÿ".to_owned(),
            "".to_owned(),
        ];
        let mut data = CsvData::from_rows(vec![cells.clone()]);
        assert_eq!(data.row_cells(0).collect::<Vec<_>>(), cells);
        data.set(0, 2, "úú");
        assert_eq!(data.row_cells(0).collect::<Vec<_>>(), ["ú", "", "úú", ""]);
        let parsed = crate::csv::parse_csv("ú,,🙂\0ÿ,\n", Delimiter::Comma).unwrap();
        assert_eq!(parsed.row_cells(0).collect::<Vec<_>>(), cells);
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
        let edit = CellEditState::new(pos, "hello".to_string(), 10);

        assert_eq!(edit.position, pos);
        assert_eq!(edit.buffer(), "hello");
        assert_eq!(edit.cursor_char_position(), 5); // cursor at end
        assert_eq!(edit.original, "hello");
        assert_eq!(edit.original_column_width, 10);
        assert!(!edit.is_modified());
    }

    #[test]
    fn test_cell_edit_state_with_char() {
        let pos = CellPosition::new(0, 0);
        let edit = CellEditState::with_char(pos, "old".to_string(), 'x', 10);

        assert_eq!(edit.buffer(), "x");
        assert_eq!(edit.cursor_char_position(), 1); // cursor after 'x'
        assert_eq!(edit.original, "old");
        assert_eq!(edit.original_column_width, 10);
        assert!(edit.is_modified());
    }

    #[test]
    fn test_cell_edit_state_insert_char() {
        let pos = CellPosition::new(0, 0);
        let mut edit = CellEditState::new(pos, "ab".to_string(), 10);
        // Cursor starts at end (2), move to position 1
        edit.cursor_home();
        edit.cursor_right();
        edit.insert_char('X');

        assert_eq!(edit.buffer(), "aXb");
        assert_eq!(edit.cursor_char_position(), 2);
    }

    #[test]
    fn test_cell_edit_state_delete_backward() {
        let pos = CellPosition::new(0, 0);
        let mut edit = CellEditState::new(pos, "abc".to_string(), 10);
        // Cursor at end (3), move to position 2
        edit.cursor_left();
        edit.delete_backward();

        assert_eq!(edit.buffer(), "ac");
        assert_eq!(edit.cursor_char_position(), 1);
    }

    #[test]
    fn test_cell_edit_state_cursor_movement() {
        let pos = CellPosition::new(0, 0);
        let mut edit = CellEditState::new(pos, "hello".to_string(), 10);

        // Cursor starts at end (5)
        edit.cursor_home();
        assert_eq!(edit.cursor_char_position(), 0);

        edit.cursor_right();
        assert_eq!(edit.cursor_char_position(), 1);

        edit.cursor_end();
        assert_eq!(edit.cursor_char_position(), 5);

        edit.cursor_left();
        assert_eq!(edit.cursor_char_position(), 4);
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
        assert_eq!(state.editing.as_ref().unwrap().buffer(), "a");

        state.edit_insert_char('X');
        assert_eq!(state.editing.as_ref().unwrap().buffer(), "aX");

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

    #[test]
    fn test_cell_edit_state_word_movement() {
        let pos = CellPosition::new(0, 0);
        let mut edit = CellEditState::new(pos, "hello world".to_string(), 12);

        edit.cursor_home();
        assert_eq!(edit.cursor_char_position(), 0);

        edit.cursor_word_right();
        // Should be at position 6 (after "hello ")
        assert_eq!(edit.cursor_char_position(), 6);

        edit.cursor_word_left();
        assert_eq!(edit.cursor_char_position(), 0);
    }

    #[test]
    fn test_cell_edit_state_selection() {
        let pos = CellPosition::new(0, 0);
        let mut edit = CellEditState::new(pos, "hello".to_string(), 10);

        assert!(!edit.has_selection());

        edit.select_all();
        assert!(edit.has_selection());
        assert_eq!(edit.selected_text(), "hello");
    }
}
