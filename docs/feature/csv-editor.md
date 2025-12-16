# CSV Viewer/Editor Mode

A spreadsheet-like view for tabular data files (CSV, TSV, PSV) with cell editing, horizontal scrolling, and synchronization with the underlying text buffer.

> **Status:** Phase 1 Complete ✅ (Read-only viewer with full navigation)
> **Inspired by:** [Tablecruncher](https://github.com/Tablecruncher/tablecruncher), TablePlus
> **Target:** ~~Phase 1 (Read-only)~~ → Phase 2 (Cell Editing) → Phase 3 (Advanced Features)

---

## Phase 1 Implementation Summary

Phase 1 delivers a fully functional read-only CSV viewer with:

- **Grid rendering** with row numbers (1, 2, 3...) and column headers (A, B, C...)
- **Cell selection** via mouse click or keyboard navigation
- **Keyboard navigation**: Arrow keys, Tab/Shift+Tab, Home/End, Page Up/Down, Cmd+Home/End
- **Mouse wheel scrolling** for both vertical and horizontal navigation
- **Delimiter detection** from file extension (.csv, .tsv, .psv) or content sniffing
- **Column width auto-calculation** based on content (sampled from first 100 rows)
- **Theme integration** with configurable colors for headers, grid lines, selection
- **Command palette integration** via "Toggle CSV View" command
- **Large file support** tested with 10,000+ row files

**Test command:** `make csv` opens a 10,001-line sample CSV file for testing.

---

## Design Decisions

These decisions were resolved during planning:

| Decision | Resolution |
|----------|------------|
| **State preservation** | Discard state on toggle. Re-parse on each toggle. Simple, stateless approach. |
| **Auto-enable** | No auto-enable. Manual toggle via command palette ("Toggle CSV View"). Users can bind their own shortcut. |
| **Size limits** | No hard limits. Implementation should handle large files efficiently. Defer warnings if needed later. |
| **Internal delimiter (0xFA)** | Accepted risk. Re-assess if collisions become an issue in practice. |

---

## Table of Contents

1. [Overview](#1-overview)
2. [User Experience](#2-user-experience)
3. [Architecture](#3-architecture)
4. [Data Model](#4-data-model)
5. [Parsing Strategy](#5-parsing-strategy)
6. [Rendering](#6-rendering)
7. [Cell Navigation & Selection](#7-cell-navigation--selection)
8. [Cell Editing (Phase 2)](#8-cell-editing-phase-2)
9. [Synchronization](#9-synchronization)
10. [Error Handling](#10-error-handling)
11. [Performance & Benchmarks](#11-performance--benchmarks)
12. [Testing Strategy](#12-testing-strategy)
13. [Implementation Phases](#13-implementation-phases)
14. [Message Types](#14-message-types)
15. [Commands](#15-commands)
16. [Configuration](#16-configuration)

---

## 1. Overview

### 1.1 Goals

- Provide a spreadsheet-like view for CSV/TSV files with rows and columns
- Allow horizontal scrolling when columns exceed viewport width
- Support cell-level editing that syncs back to the underlying text buffer
- Handle large files efficiently (1K, 5K, 50K, 500K rows)
- Graceful error handling for malformed/non-CSV files

### 1.2 Non-Goals (Initial Release)

- Formula evaluation or calculated cells
- Data validation rules
- Import/export to other formats (Excel, JSON)
- Multiple sheets within a single file
- Filtering, sorting, or aggregation
- Column resize with mouse drag (use fixed/auto-width initially)

### 1.3 Supported File Types

| Extension | Delimiter | Detected By |
|-----------|-----------|-------------|
| `.csv`    | `,`       | Extension or content sniffing |
| `.tsv`    | `\t`      | Extension |
| `.psv`    | `\|`      | Extension (pipe-separated) |

---

## 2. User Experience

### 2.1 Toggling CSV Mode

**Entry Point:**
- **Command palette:** "Toggle CSV View" command (no default keybinding; users can bind their own)

**Exit Points:**
1. Same command toggles off
2. Press Escape while in CSV mode (returns to text view)

> **Note:** No auto-enable for `.csv` files. This keeps the behavior explicit and predictable.

### 2.2 Visual Layout

```
┌─────────────────────────────────────────────────────────────────────────┐
│ [file.csv] [×]                                              Tab Bar    │
├─────────────────────────────────────────────────────────────────────────┤
│ ┌─────┬────────────┬────────────┬────────────┬────────────┬──────────┐ │
│ │  #  │     A      │     B      │     C      │     D      │    E ... │ │ ◄─ Header Row
│ ├─────┼────────────┼────────────┼────────────┼────────────┼──────────┤ │
│ │  1  │ Alice      │ 30         │ Engineer   │ NYC        │ ...      │ │
│ │  2  │ Bob        │ 25         │ Designer   │ LA         │ ...      │ │
│ │  3  │ Carol      │ 35         │ Manager    │ Chicago    │ ...      │ │
│ │  4  │ ▌█████████ │ (editing)  │            │            │ ...      │ │ ◄─ Active Cell
│ │  5  │ Eve        │ 28         │ DevOps     │ Seattle    │ ...      │ │
│ │ ... │            │            │            │            │          │ │
│ └─────┴────────────┴────────────┴────────────┴────────────┴──────────┘ │
│ ◄────────────────── horizontal scroll ──────────────────────────────► │
├─────────────────────────────────────────────────────────────────────────┤
│ file.csv │ CSV │ Row 4, Col 2 │ 1,234 rows × 12 cols │ (editing)      │ Status Bar
└─────────────────────────────────────────────────────────────────────────┘
```

### 2.3 Navigation

| Key | Action |
|-----|--------|
| Arrow keys | Move cell selection |
| Tab | Move to next cell (right, then wrap to next row) |
| Shift+Tab | Move to previous cell |
| Enter | Edit current cell / Confirm edit and move down |
| Escape | Cancel edit / Exit CSV mode |
| Cmd+Home | Go to cell A1 |
| Cmd+End | Go to last cell |
| Page Up/Down | Scroll by viewport height |
| Home/End | Go to first/last column in current row |

### 2.4 Status Bar Segments

When CSV mode is active, the status bar shows:

| Segment | Example | Position |
|---------|---------|----------|
| File name | `data.csv` | Left |
| Mode indicator | `CSV` | Left (new segment) |
| Cell position | `Row 42, Col 3` | Right |
| Dimensions | `1,234 rows × 12 cols` | Right |
| Edit state | `(editing)` | Right (when editing) |

---

## 3. Architecture

### 3.1 Integration with Existing Codebase

The CSV viewer integrates as an **alternate view mode** for an existing `EditorState`. The same `Document` is shared between text mode and CSV mode.

```
┌─────────────────────────────────────────────────────────────────┐
│                        EditorArea                               │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │                     EditorState                           │  │
│  │  ┌─────────────────────────────────────────────────────┐  │  │
│  │  │                   ViewMode                          │  │  │
│  │  │  ┌───────────────┐     ┌───────────────────────┐   │  │  │
│  │  │  │   TextMode    │ OR  │      CsvMode          │   │  │  │
│  │  │  │ (default)     │     │ CsvState, CsvViewport │   │  │  │
│  │  │  └───────────────┘     └───────────────────────┘   │  │  │
│  │  └─────────────────────────────────────────────────────┘  │  │
│  │                          │                                 │  │
│  │                          ▼                                 │  │
│  │               ┌─────────────────────┐                     │  │
│  │               │     Document        │                     │  │
│  │               │  (Rope text buffer) │                     │  │
│  │               └─────────────────────┘                     │  │
│  └───────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
```

### 3.2 Module Structure

```
src/
├── csv/
│   ├── mod.rs           # Module exports
│   ├── parser.rs        # CSV parsing (using `csv` crate)
│   ├── model.rs         # CsvState, CsvData, Cell types
│   ├── viewport.rs      # CsvViewport calculations
│   ├── navigation.rs    # Cell navigation logic
│   ├── editing.rs       # Cell editing and sync (Phase 2)
│   └── render.rs        # CSV-specific rendering
├── model/
│   └── editor.rs        # Add ViewMode enum
├── view/
│   └── mod.rs           # Add CSV rendering dispatch
└── messages.rs          # Add CsvMsg variants
```

### 3.3 Crate Dependencies

```toml
[dependencies]
csv = "1.3"  # RFC 4180 compliant parser with streaming support
```

### 3.4 Keymap Integration

CSV mode requires integration with the existing keymap system in `src/keymap/`:

**Key routing flow:**
1. Key events captured via `src/runtime/input.rs`
2. Keymap lookup in `src/keymap/keymap.rs` checks context
3. CSV mode active → `KeyContext::csv_mode = true`
4. CSV-specific bindings take precedence when in CSV mode

**Required additions to `src/keymap/command.rs`:**
```rust
pub enum KeymapCommand {
    // ... existing commands ...
    
    // CSV mode commands
    CsvToggle,
    CsvMoveUp,
    CsvMoveDown,
    CsvMoveLeft,
    CsvMoveRight,
    CsvNextCell,      // Tab
    CsvPrevCell,      // Shift+Tab
    CsvStartEdit,     // Enter
    CsvConfirmEdit,   // Enter (when editing)
    CsvCancelEdit,    // Escape
}
```

**Example keymap.yaml additions:**
```yaml
- key: shift+cmd+t
  command: CsvToggle
  
- key: tab
  command: CsvNextCell
  when: [csv_mode]
  
- key: shift+tab
  command: CsvPrevCell
  when: [csv_mode]
  
- key: enter
  command: CsvStartEdit
  when: [csv_mode, not_editing]
  
- key: escape
  command: CsvCancelEdit
  when: [csv_mode, editing]
```

**Context conditions to add:**
- `csv_mode` - True when editor is in CSV view mode
- `csv_editing` - True when actively editing a cell

---

## 4. Data Model

### 4.1 Core Types

```rust
/// View mode for an editor - either text or CSV
/// 
/// **State preservation:** State is discarded on toggle (stateless approach).
/// 
/// - **Text → CSV:** CsvState freshly initialized, cell (0,0) selected, document re-parsed.
/// - **CSV → Text:** CsvState discarded, text cursor placed at document start.
/// - **Re-enabling CSV:** Full re-parse (no caching).
/// 
/// This keeps the implementation simple. Re-parsing is fast (< 100ms for 50K rows).
#[derive(Debug, Clone, Default)]
pub enum ViewMode {
    #[default]
    Text,
    Csv(CsvState),
}

/// Complete CSV view state
#[derive(Debug, Clone)]
pub struct CsvState {
    /// Parsed CSV data (cached)
    pub data: CsvData,
    
    /// Current cell selection (row, column)
    pub selected_cell: CellPosition,
    
    /// Multi-cell selection (optional, for Phase 3)
    pub selection_range: Option<CellRange>,
    
    /// Viewport state for scrolling
    pub viewport: CsvViewport,
    
    /// Whether we're actively editing a cell
    pub editing: Option<CellEditState>,
    
    /// Column widths (computed or configured)
    pub column_widths: Vec<usize>,
    
    /// Whether first row is header
    pub has_header_row: bool,
    
    /// Detected delimiter
    pub delimiter: u8,
    
    /// Parse error (if any)
    pub parse_error: Option<String>,
}

/// Parsed CSV data
#[derive(Debug, Clone)]
pub struct CsvData {
    /// All rows as vectors of strings
    /// Using delimited-string storage (Tablecruncher approach) for memory efficiency
    rows: Vec<String>,
    
    /// Field delimiter byte (0xFA for internal storage)
    field_delimiter: u8,
    
    /// Number of columns (max across all rows)
    column_count: usize,
    
    /// Document revision when this was parsed
    parsed_revision: u64,
}

/// Position in the CSV grid
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CellPosition {
    /// Row index (0-indexed, excluding header if present)
    pub row: usize,
    /// Column index (0-indexed)
    pub col: usize,
}

/// Range of cells (for future multi-select)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CellRange {
    pub start: CellPosition,
    pub end: CellPosition,
}

/// State for an in-progress cell edit
#[derive(Debug, Clone)]
pub struct CellEditState {
    /// Position of cell being edited
    pub position: CellPosition,
    /// Current edit buffer
    pub buffer: String,
    /// Cursor position within the buffer
    pub cursor: usize,
    /// Original value (for cancel/undo)
    pub original: String,
}

/// Viewport for CSV mode (similar to text Viewport but 2D)
#[derive(Debug, Clone)]
pub struct CsvViewport {
    /// First visible row (0-indexed)
    pub top_row: usize,
    /// First visible column (0-indexed)
    pub left_col: usize,
    /// Number of visible rows
    pub visible_rows: usize,
    /// Number of visible columns (depends on column widths)
    pub visible_cols: usize,
}
```

### 4.2 Memory-Efficient Storage (Tablecruncher Pattern)

Instead of `Vec<Vec<String>>` (expensive), store each row as a single delimited string:

```rust
impl CsvData {
    /// Internal delimiter - uses byte 0xFA
    /// 
    /// **Accepted risk:** 0xFA can appear in multi-byte UTF-8 sequences, but this is
    /// rare in typical CSV content. If collision occurs, cell data will be incorrectly
    /// split. Re-assess if this becomes an issue in practice.
    const FIELD_DELIMITER: u8 = 0xFA;
    
    /// Get cell value at (row, col)
    pub fn get(&self, row: usize, col: usize) -> &str {
        if row >= self.rows.len() {
            return "";
        }
        self.get_field(&self.rows[row], col)
    }
    
    /// Set cell value at (row, col)
    pub fn set(&mut self, row: usize, col: usize, value: &str) {
        if row >= self.rows.len() {
            return;
        }
        self.rows[row] = self.set_field(&self.rows[row], col, value);
    }
    
    /// Extract field from delimited row string
    fn get_field<'a>(&self, row: &'a str, col: usize) -> &'a str {
        let delim = char::from(Self::FIELD_DELIMITER);
        row.split(delim)
            .nth(col)
            .unwrap_or("")
    }
    
    /// Replace field in delimited row string
    fn set_field(&self, row: &str, col: usize, value: &str) -> String {
        let delim = char::from(Self::FIELD_DELIMITER);
        let mut fields: Vec<&str> = row.split(delim).collect();
        
        // Extend if needed
        while fields.len() <= col {
            fields.push("");
        }
        
        // Replace
        fields[col] = value;
        
        fields.join(&delim.to_string())
    }
    
    /// Row count
    pub fn row_count(&self) -> usize {
        self.rows.len()
    }
    
    /// Column count
    pub fn column_count(&self) -> usize {
        self.column_count
    }
}
```

### 4.3 Column Width Calculation

```rust
impl CsvState {
    /// Calculate column widths based on content
    pub fn compute_column_widths(&mut self, max_sample_rows: usize) {
        let sample_count = self.data.row_count().min(max_sample_rows);
        let col_count = self.data.column_count();
        
        // Initialize with header widths or column letters (A, B, C...)
        let mut widths: Vec<usize> = (0..col_count)
            .map(|i| column_letter(i).len().max(3))
            .collect();
        
        // Sample rows to find max width per column
        for row_idx in 0..sample_count {
            for col_idx in 0..col_count {
                let cell = self.data.get(row_idx, col_idx);
                let cell_width = cell.chars().count();
                widths[col_idx] = widths[col_idx].max(cell_width);
            }
        }
        
        // Apply min/max constraints
        const MIN_COL_WIDTH: usize = 4;
        const MAX_COL_WIDTH: usize = 50;
        
        self.column_widths = widths
            .into_iter()
            .map(|w| w.clamp(MIN_COL_WIDTH, MAX_COL_WIDTH))
            .collect();
    }
}

/// Convert column index to letter (0 = A, 1 = B, ..., 25 = Z, 26 = AA, ...)
/// 
/// **Boundary verification required:** The `-1` adjustment at `n / 26 - 1` handles
/// the base-26 bijective numeration correctly. Test cases to verify:
/// - `column_letter(0)` → "A"
/// - `column_letter(25)` → "Z"  
/// - `column_letter(26)` → "AA" (first two-letter)
/// - `column_letter(27)` → "AB"
/// - `column_letter(701)` → "ZZ" (last two-letter)
/// - `column_letter(702)` → "AAA" (first three-letter)
pub fn column_letter(index: usize) -> String {
    let mut result = String::new();
    let mut n = index;
    loop {
        result.insert(0, char::from(b'A' + (n % 26) as u8));
        if n < 26 {
            break;
        }
        n = n / 26 - 1;
    }
    result
}
```

---

## 5. Parsing Strategy

### 5.1 Initial Parse (Streaming)

```rust
use csv::{ReaderBuilder, Terminator};

/// Parse result with potential warnings
pub struct ParseResult {
    pub data: CsvData,
    pub warnings: Vec<ParseWarning>,
}

pub enum ParseWarning {
    /// Rows have different column counts
    InconsistentColumns { expected: usize, found: usize, row: usize },
    /// Truncated due to size limit
    Truncated { max_rows: usize },
    /// Invalid UTF-8 replaced
    InvalidUtf8 { row: usize },
}

/// Parse CSV from document buffer
pub fn parse_csv(
    source: &str,
    delimiter: u8,
    max_rows: Option<usize>,
) -> Result<ParseResult, CsvParseError> {
    let mut reader = ReaderBuilder::new()
        .delimiter(delimiter)
        .has_headers(false)
        .flexible(true) // Allow varying column counts
        .from_reader(source.as_bytes());
    
    let mut rows: Vec<String> = Vec::new();
    let mut max_cols = 0;
    let mut warnings = Vec::new();
    
    for (row_idx, result) in reader.records().enumerate() {
        // Check row limit
        if let Some(max) = max_rows {
            if row_idx >= max {
                warnings.push(ParseWarning::Truncated { max_rows: max });
                break;
            }
        }
        
        match result {
            Ok(record) => {
                let col_count = record.len();
                
                // Track column count variance
                if row_idx > 0 && col_count != max_cols {
                    warnings.push(ParseWarning::InconsistentColumns {
                        expected: max_cols,
                        found: col_count,
                        row: row_idx,
                    });
                }
                max_cols = max_cols.max(col_count);
                
                // Convert to delimited string
                let row_str = record
                    .iter()
                    .collect::<Vec<_>>()
                    .join(&char::from(CsvData::FIELD_DELIMITER).to_string());
                
                rows.push(row_str);
            }
            Err(e) => {
                // Log but continue (flexible parsing)
                tracing::warn!("CSV parse error at row {}: {}", row_idx, e);
                rows.push(String::new());
            }
        }
    }
    
    Ok(ParseResult {
        data: CsvData {
            rows,
            field_delimiter: CsvData::FIELD_DELIMITER,
            column_count: max_cols,
            parsed_revision: 0,
        },
        warnings,
    })
}
```

### 5.2 Delimiter Detection

```rust
/// Detect delimiter by sampling first few lines
pub fn detect_delimiter(source: &str) -> u8 {
    const CANDIDATES: &[u8] = &[b',', b'\t', b';', b'|'];
    const SAMPLE_LINES: usize = 5;
    
    let lines: Vec<&str> = source.lines().take(SAMPLE_LINES).collect();
    if lines.is_empty() {
        return b','; // Default
    }
    
    let mut scores: Vec<(u8, f64)> = CANDIDATES
        .iter()
        .map(|&delim| {
            let counts: Vec<usize> = lines
                .iter()
                .map(|line| line.bytes().filter(|&b| b == delim).count())
                .collect();
            
            // Score: consistency (low variance) + presence (count > 0)
            let mean = counts.iter().sum::<usize>() as f64 / counts.len() as f64;
            if mean == 0.0 {
                return (delim, 0.0);
            }
            
            let variance = counts.iter()
                .map(|&c| (c as f64 - mean).powi(2))
                .sum::<f64>() / counts.len() as f64;
            
            // Higher score = better (high count, low variance)
            let consistency = if variance == 0.0 { 1.0 } else { 1.0 / (1.0 + variance) };
            (delim, mean * consistency)
        })
        .collect();
    
    scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    
    scores.first().map(|(d, _)| *d).unwrap_or(b',')
}
```

### 5.3 Incremental Re-parse (Phase 2+)

For editing, we need to detect which rows changed:

```rust
/// Track what changed between parses
pub struct CsvDiff {
    pub modified_rows: Vec<usize>,
    pub inserted_rows: Vec<usize>,
    pub deleted_rows: Vec<usize>,
}

/// Re-parse only if document revision changed
impl CsvState {
    pub fn maybe_reparse(&mut self, document: &Document) -> Option<CsvDiff> {
        if self.data.parsed_revision == document.revision {
            return None; // Already up to date
        }
        
        // Full reparse for now (incremental is complex)
        let source = document.buffer.to_string();
        match parse_csv(&source, self.delimiter, None) {
            Ok(result) => {
                let old_rows = self.data.row_count();
                self.data = result.data;
                self.data.parsed_revision = document.revision;
                self.compute_column_widths(1000);
                
                // Simplified diff: assume all rows changed
                Some(CsvDiff {
                    modified_rows: (0..self.data.row_count()).collect(),
                    inserted_rows: vec![],
                    deleted_rows: vec![],
                })
            }
            Err(e) => {
                self.parse_error = Some(e.to_string());
                None
            }
        }
    }
}
```

---

## 6. Rendering

### 6.1 CSV Render Pipeline

The CSV renderer replaces the text renderer when `ViewMode::Csv` is active:

```rust
impl Renderer {
    fn render_editor_group(/* ... */) {
        // ... existing code ...
        
        match &editor.view_mode {
            ViewMode::Text => {
                // Existing text rendering
                Self::render_text_area(/* ... */);
            }
            ViewMode::Csv(csv_state) => {
                // CSV grid rendering
                Self::render_csv_grid(
                    frame,
                    painter,
                    model,
                    editor,
                    document,
                    csv_state,
                    rect_x,
                    rect_w,
                    content_y,
                    content_h,
                    line_height,
                    char_width,
                    is_focused,
                );
            }
        }
    }
}
```

### 6.2 Grid Rendering

```rust
impl Renderer {
    #[allow(clippy::too_many_arguments)]
    fn render_csv_grid(
        frame: &mut Frame,
        painter: &mut TextPainter,
        model: &AppModel,
        editor: &EditorState,
        document: &Document,
        csv: &CsvState,
        rect_x: usize,
        rect_w: usize,
        content_y: usize,
        content_h: usize,
        line_height: usize,
        char_width: f32,
        is_focused: bool,
    ) {
        let theme = &model.theme;
        
        // Calculate layout
        let row_header_width = Self::row_header_width(csv.data.row_count(), char_width);
        let grid_x = rect_x + row_header_width;
        let grid_w = rect_w.saturating_sub(row_header_width);
        
        // Determine visible rows
        let header_rows = if csv.has_header_row { 1 } else { 0 };
        let data_start_y = content_y + header_rows * line_height;
        let visible_data_rows = (content_h - header_rows * line_height) / line_height;
        let end_row = (csv.viewport.top_row + visible_data_rows)
            .min(csv.data.row_count());
        
        // Determine visible columns
        let (visible_cols, col_x_offsets) = Self::compute_visible_columns(
            csv, grid_w, char_width,
        );
        
        // 1. Render header row (if present)
        if csv.has_header_row && csv.data.row_count() > 0 {
            Self::render_csv_header_row(
                frame, painter, csv, theme,
                grid_x, content_y, line_height, char_width,
                &visible_cols, &col_x_offsets,
            );
        }
        
        // 2. Render column headers (A, B, C, ...)
        Self::render_column_headers(
            frame, painter, csv, theme,
            grid_x, content_y, line_height, char_width,
            &visible_cols, &col_x_offsets,
        );
        
        // 3. Render row headers (1, 2, 3, ...)
        Self::render_row_headers(
            frame, painter, csv, theme,
            rect_x, data_start_y, row_header_width, line_height,
            csv.viewport.top_row, end_row,
        );
        
        // 4. Render grid lines
        Self::render_grid_lines(
            frame, csv, theme,
            grid_x, data_start_y, grid_w, content_h,
            line_height, &col_x_offsets, csv.viewport.top_row, end_row,
        );
        
        // 5. Render cells
        Self::render_cells(
            frame, painter, csv, theme,
            grid_x, data_start_y, line_height, char_width,
            csv.viewport.top_row, end_row,
            &visible_cols, &col_x_offsets,
        );
        
        // 6. Render selection highlight
        Self::render_cell_selection(
            frame, csv, theme,
            grid_x, data_start_y, row_header_width, line_height,
            csv.viewport.top_row, &visible_cols, &col_x_offsets,
            is_focused,
        );
        
        // 7. Render edit cursor (if editing)
        if let Some(edit_state) = &csv.editing {
            Self::render_cell_editor(
                frame, painter, csv, theme, model,
                grid_x, data_start_y, line_height, char_width,
                csv.viewport.top_row, &col_x_offsets,
                edit_state,
            );
        }
    }
    
    /// Compute visible columns and their X offsets
    fn compute_visible_columns(
        csv: &CsvState,
        grid_w: usize,
        char_width: f32,
    ) -> (Vec<usize>, Vec<usize>) {
        let mut visible = Vec::new();
        let mut offsets = Vec::new();
        let mut x = 0;
        
        for col in csv.viewport.left_col..csv.column_widths.len() {
            let col_width = (csv.column_widths[col] as f32 * char_width).ceil() as usize + 8; // padding
            
            if x + col_width > grid_w && !visible.is_empty() {
                break; // Stop when we overflow (but include at least one column)
            }
            
            offsets.push(x);
            visible.push(col);
            x += col_width;
        }
        
        (visible, offsets)
    }
    
    /// Width needed for row numbers
    fn row_header_width(row_count: usize, char_width: f32) -> usize {
        let digits = ((row_count.max(1) as f64).log10().floor() as usize) + 1;
        let min_digits = 3; // At least "999"
        ((digits.max(min_digits) as f32 * char_width) + 12.0) as usize // padding
    }
}
```

### 6.3 Cell Rendering with Visual Cues

```rust
impl Renderer {
    fn render_cells(
        frame: &mut Frame,
        painter: &mut TextPainter,
        csv: &CsvState,
        theme: &Theme,
        grid_x: usize,
        data_start_y: usize,
        line_height: usize,
        char_width: f32,
        start_row: usize,
        end_row: usize,
        visible_cols: &[usize],
        col_x_offsets: &[usize],
    ) {
        let fg_color = theme.editor.foreground.to_argb_u32();
        let number_color = theme.csv.number_foreground.to_argb_u32(); // New theme field
        
        for (screen_row, data_row) in (start_row..end_row).enumerate() {
            let y = data_start_y + screen_row * line_height;
            
            for (screen_col, &data_col) in visible_cols.iter().enumerate() {
                let x = grid_x + col_x_offsets[screen_col] + 4; // 4px padding
                let col_width = csv.column_widths.get(data_col).copied().unwrap_or(10);
                
                let cell_value = csv.data.get(data_row, data_col);
                
                // Determine text alignment and color based on content type
                let (display_text, color, align_right) = if is_number(cell_value) {
                    (cell_value.to_string(), number_color, true)
                } else {
                    // Truncate if too long
                    let truncated = truncate_text(cell_value, col_width);
                    (truncated, fg_color, false)
                };
                
                // Calculate X position for alignment
                let text_x = if align_right {
                    let text_width = (display_text.chars().count() as f32 * char_width).ceil() as usize;
                    let max_x = col_x_offsets.get(screen_col + 1)
                        .map(|&next| grid_x + next - 8)
                        .unwrap_or(x + (col_width as f32 * char_width) as usize);
                    max_x.saturating_sub(text_width)
                } else {
                    x
                };
                
                painter.draw(frame, text_x, y, &display_text, color);
                
                // Draw overflow indicator if truncated
                if cell_value.chars().count() > col_width {
                    // Small dots in bottom-right corner
                    let indicator_x = grid_x + col_x_offsets.get(screen_col + 1)
                        .copied()
                        .unwrap_or(col_x_offsets[screen_col] + (col_width as f32 * char_width) as usize) - 6;
                    let indicator_y = y + line_height - 4;
                    frame.fill_rect_px(indicator_x, indicator_y, 2, 2, fg_color);
                }
            }
        }
    }
}

/// Check if a string looks like a number
fn is_number(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    s.parse::<f64>().is_ok()
}

/// Truncate text with ellipsis if too long
fn truncate_text(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else if max_chars <= 3 {
        s.chars().take(max_chars).collect()
    } else {
        let mut result: String = s.chars().take(max_chars - 1).collect();
        result.push('…');
        result
    }
}
```

---

## 7. Cell Navigation & Selection

### 7.1 Navigation Functions

```rust
impl CsvState {
    /// Move selection by delta (handles bounds)
    pub fn move_selection(&mut self, delta_row: i32, delta_col: i32) {
        let new_row = (self.selected_cell.row as i32 + delta_row)
            .max(0)
            .min(self.data.row_count().saturating_sub(1) as i32) as usize;
        
        let new_col = (self.selected_cell.col as i32 + delta_col)
            .max(0)
            .min(self.data.column_count().saturating_sub(1) as i32) as usize;
        
        self.selected_cell = CellPosition { row: new_row, col: new_col };
        self.ensure_selection_visible();
    }
    
    /// Move to next cell (Tab behavior)
    pub fn move_to_next_cell(&mut self) {
        let col_count = self.data.column_count();
        let row_count = self.data.row_count();
        
        if self.selected_cell.col + 1 < col_count {
            self.selected_cell.col += 1;
        } else if self.selected_cell.row + 1 < row_count {
            self.selected_cell.row += 1;
            self.selected_cell.col = 0;
        }
        // Else: stay at last cell
        
        self.ensure_selection_visible();
    }
    
    /// Move to previous cell (Shift+Tab behavior)
    pub fn move_to_prev_cell(&mut self) {
        if self.selected_cell.col > 0 {
            self.selected_cell.col -= 1;
        } else if self.selected_cell.row > 0 {
            self.selected_cell.row -= 1;
            self.selected_cell.col = self.data.column_count().saturating_sub(1);
        }
        // Else: stay at first cell
        
        self.ensure_selection_visible();
    }
    
    /// Ensure selected cell is visible in viewport
    pub fn ensure_selection_visible(&mut self) {
        // Vertical scroll
        if self.selected_cell.row < self.viewport.top_row {
            self.viewport.top_row = self.selected_cell.row;
        } else if self.selected_cell.row >= self.viewport.top_row + self.viewport.visible_rows {
            self.viewport.top_row = self.selected_cell.row
                .saturating_sub(self.viewport.visible_rows - 1);
        }
        
        // Horizontal scroll
        if self.selected_cell.col < self.viewport.left_col {
            self.viewport.left_col = self.selected_cell.col;
        } else if self.selected_cell.col >= self.viewport.left_col + self.viewport.visible_cols {
            self.viewport.left_col = self.selected_cell.col
                .saturating_sub(self.viewport.visible_cols - 1);
        }
    }
    
    /// Go to specific cell
    pub fn go_to_cell(&mut self, row: usize, col: usize) {
        self.selected_cell = CellPosition {
            row: row.min(self.data.row_count().saturating_sub(1)),
            col: col.min(self.data.column_count().saturating_sub(1)),
        };
        self.ensure_selection_visible();
    }
    
    /// Go to first cell (Cmd+Home)
    pub fn go_to_first_cell(&mut self) {
        self.go_to_cell(0, 0);
    }
    
    /// Go to last cell (Cmd+End)
    pub fn go_to_last_cell(&mut self) {
        self.go_to_cell(
            self.data.row_count().saturating_sub(1),
            self.data.column_count().saturating_sub(1),
        );
    }
}
```

### 7.2 Mouse Hit Testing

```rust
impl CsvState {
    /// Convert viewport pixel position to cell position
    pub fn pixel_to_cell(
        &self,
        x: f64,
        y: f64,
        grid_x: usize,
        data_start_y: usize,
        row_header_width: usize,
        line_height: usize,
        char_width: f32,
    ) -> Option<CellPosition> {
        // Check if in grid area (not headers)
        if x < grid_x as f64 || y < data_start_y as f64 {
            return None;
        }
        
        // Row from Y
        let relative_y = y as usize - data_start_y;
        let row = self.viewport.top_row + relative_y / line_height;
        if row >= self.data.row_count() {
            return None;
        }
        
        // Column from X
        let relative_x = x as usize - grid_x;
        let mut col = self.viewport.left_col;
        let mut col_x = 0;
        
        for c in self.viewport.left_col..self.data.column_count() {
            let col_width = (self.column_widths[c] as f32 * char_width).ceil() as usize + 8;
            if relative_x < col_x + col_width {
                col = c;
                break;
            }
            col_x += col_width;
            col = c + 1;
        }
        
        if col >= self.data.column_count() {
            col = self.data.column_count().saturating_sub(1);
        }
        
        Some(CellPosition { row, col })
    }
}
```

---

## 8. Cell Editing (Phase 2)

### 8.1 Edit Lifecycle

```rust
impl CsvState {
    /// Start editing the selected cell
    pub fn start_editing(&mut self) {
        let value = self.data.get(self.selected_cell.row, self.selected_cell.col);
        self.editing = Some(CellEditState {
            position: self.selected_cell,
            buffer: value.to_string(),
            cursor: value.len(),
            original: value.to_string(),
        });
    }
    
    /// Start editing with initial character (from typing)
    pub fn start_editing_with_char(&mut self, ch: char) {
        self.editing = Some(CellEditState {
            position: self.selected_cell,
            buffer: ch.to_string(),
            cursor: 1,
            original: self.data.get(self.selected_cell.row, self.selected_cell.col).to_string(),
        });
    }
    
    /// Confirm edit and update data
    pub fn confirm_edit(&mut self) -> Option<CellEdit> {
        let edit_state = self.editing.take()?;
        
        if edit_state.buffer == edit_state.original {
            return None; // No change
        }
        
        let edit = CellEdit {
            position: edit_state.position,
            old_value: edit_state.original,
            new_value: edit_state.buffer.clone(),
        };
        
        self.data.set(edit.position.row, edit.position.col, &edit.new_value);
        
        Some(edit)
    }
    
    /// Cancel edit and discard changes
    pub fn cancel_edit(&mut self) {
        self.editing = None;
    }
    
    /// Insert character at cursor
    pub fn edit_insert_char(&mut self, ch: char) {
        if let Some(edit) = &mut self.editing {
            edit.buffer.insert(edit.cursor, ch);
            edit.cursor += ch.len_utf8();
        }
    }
    
    /// Delete character before cursor (Backspace)
    pub fn edit_delete_backward(&mut self) {
        if let Some(edit) = &mut self.editing {
            if edit.cursor > 0 {
                let prev_char_boundary = edit.buffer[..edit.cursor]
                    .char_indices()
                    .last()
                    .map(|(i, _)| i)
                    .unwrap_or(0);
                edit.buffer.remove(prev_char_boundary);
                edit.cursor = prev_char_boundary;
            }
        }
    }
}

/// Represents a cell edit operation for sync/undo
#[derive(Debug, Clone)]
pub struct CellEdit {
    pub position: CellPosition,
    pub old_value: String,
    pub new_value: String,
}
```

### 8.2 Edit Rendering

```rust
impl Renderer {
    fn render_cell_editor(
        frame: &mut Frame,
        painter: &mut TextPainter,
        csv: &CsvState,
        theme: &Theme,
        model: &AppModel,
        grid_x: usize,
        data_start_y: usize,
        line_height: usize,
        char_width: f32,
        top_row: usize,
        col_x_offsets: &[usize],
        edit_state: &CellEditState,
    ) {
        let pos = &edit_state.position;
        
        // Check if cell is visible
        if pos.row < top_row || pos.row >= top_row + csv.viewport.visible_rows {
            return;
        }
        let screen_col_idx = match csv.viewport.left_col..csv.data.column_count() {
            range if range.contains(&pos.col) => pos.col - csv.viewport.left_col,
            _ => return,
        };
        if screen_col_idx >= col_x_offsets.len() {
            return;
        }
        
        // Calculate cell bounds
        let screen_row = pos.row - top_row;
        let cell_y = data_start_y + screen_row * line_height;
        let cell_x = grid_x + col_x_offsets[screen_col_idx];
        let cell_width = col_x_offsets.get(screen_col_idx + 1)
            .map(|&next| next - col_x_offsets[screen_col_idx])
            .unwrap_or((csv.column_widths[pos.col] as f32 * char_width) as usize + 8);
        
        // Draw edit background
        let edit_bg = theme.overlay.input_background.to_argb_u32();
        frame.fill_rect_px(cell_x, cell_y, cell_width, line_height, edit_bg);
        
        // Draw text
        let fg = theme.overlay.foreground.to_argb_u32();
        painter.draw(frame, cell_x + 4, cell_y, &edit_state.buffer, fg);
        
        // Draw cursor
        if model.ui.cursor_visible {
            let cursor_x = cell_x + 4 + (edit_state.cursor as f32 * char_width).round() as usize;
            let cursor_color = theme.overlay.highlight.to_argb_u32();
            frame.fill_rect_px(cursor_x, cell_y, 2, line_height, cursor_color);
        }
    }
}
```

---

## 9. Synchronization

### 9.1 CSV → Text Buffer Sync

When a cell is edited in CSV mode, we must update the underlying `Document` buffer:

```rust
/// Sync a cell edit back to the document text buffer
pub fn sync_cell_to_document(
    document: &mut Document,
    csv: &CsvState,
    edit: &CellEdit,
) -> Result<(), SyncError> {
    // 1. Find the byte range for this cell in the document
    let cell_range = find_cell_range(document, csv, edit.position)?;
    
    // 2. Replace the range with the new value (properly escaped)
    let escaped_value = escape_csv_value(&edit.new_value, csv.delimiter);
    
    // 3. Apply edit to document
    let start_offset = cell_range.start;
    let end_offset = cell_range.end;
    
    // Delete old content
    document.buffer.remove(start_offset..end_offset);
    
    // Insert new content
    document.buffer.insert(start_offset, &escaped_value);
    
    // Mark as modified
    document.is_modified = true;
    document.revision = document.revision.wrapping_add(1);
    
    Ok(())
}

/// Find byte range of a cell in the document
/// 
/// **IMPORTANT:** This naive implementation counts raw newlines to find rows.
/// This breaks for CSV files with quoted multi-line fields:
/// ```csv
/// Name,Description
/// Alice,"Line 1
/// Line 2"
/// Bob,Simple
/// ```
/// Row 2 ("Bob") would be miscounted because the quoted field contains a newline.
/// 
/// **TODO:** Use CSV-aware row finding that respects quoted fields. Consider using
/// the `csv` crate's `Reader` for row boundary detection, or implement a proper
/// CSV-aware position mapping that tracks logical vs physical line numbers.
fn find_cell_range(
    document: &Document,
    csv: &CsvState,
    pos: CellPosition,
) -> Result<std::ops::Range<usize>, SyncError> {
    let content = document.buffer.to_string();
    let delimiter = csv.delimiter as char;
    
    // Find row start (FIXME: doesn't handle quoted multi-line fields)
    let mut row_start = 0;
    for (i, _) in content.match_indices('\n').take(pos.row) {
        row_start = i + 1;
    }
    if pos.row > 0 && row_start == 0 {
        return Err(SyncError::RowNotFound(pos.row));
    }
    
    // Find row end
    let row_end = content[row_start..]
        .find('\n')
        .map(|i| row_start + i)
        .unwrap_or(content.len());
    
    let row_content = &content[row_start..row_end];
    
    // Find cell within row (accounting for quoted fields)
    let (cell_start_in_row, cell_end_in_row) = find_csv_field_range(
        row_content,
        pos.col,
        delimiter,
    )?;
    
    Ok((row_start + cell_start_in_row)..(row_start + cell_end_in_row))
}

/// Find byte range of a field in a CSV row (handles quotes)
fn find_csv_field_range(
    row: &str,
    field_idx: usize,
    delimiter: char,
) -> Result<(usize, usize), SyncError> {
    let mut field_start = 0;
    let mut current_field = 0;
    let mut in_quotes = false;
    
    for (i, ch) in row.char_indices() {
        if ch == '"' {
            in_quotes = !in_quotes;
        } else if ch == delimiter && !in_quotes {
            if current_field == field_idx {
                return Ok((field_start, i));
            }
            current_field += 1;
            field_start = i + 1;
        }
    }
    
    // Last field
    if current_field == field_idx {
        return Ok((field_start, row.len()));
    }
    
    Err(SyncError::FieldNotFound(field_idx))
}

/// Escape a value for CSV output
fn escape_csv_value(value: &str, delimiter: u8) -> String {
    let delim = delimiter as char;
    let needs_quotes = value.contains(delim) 
        || value.contains('"') 
        || value.contains('\n')
        || value.contains('\r');
    
    if needs_quotes {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

#[derive(Debug)]
pub enum SyncError {
    RowNotFound(usize),
    FieldNotFound(usize),
}
```

### 9.2 Text Buffer → CSV Sync

When the document changes externally (undo, external edit), we need to re-parse:

```rust
impl CsvState {
    /// Check if document has changed and needs re-parse
    pub fn needs_sync(&self, document: &Document) -> bool {
        self.data.parsed_revision != document.revision
    }
    
    /// Sync from document changes
    pub fn sync_from_document(&mut self, document: &Document) {
        if !self.needs_sync(document) {
            return;
        }
        
        // Re-parse
        let source = document.buffer.to_string();
        match parse_csv(&source, self.delimiter, None) {
            Ok(result) => {
                self.data = result.data;
                self.data.parsed_revision = document.revision;
                self.compute_column_widths(1000);
                self.parse_error = None;
                
                // Clamp selection to new bounds
                self.selected_cell.row = self.selected_cell.row
                    .min(self.data.row_count().saturating_sub(1));
                self.selected_cell.col = self.selected_cell.col
                    .min(self.data.column_count().saturating_sub(1));
            }
            Err(e) => {
                self.parse_error = Some(e.to_string());
            }
        }
    }
}
```

---

## 10. Error Handling

### 10.1 Parse Errors

```rust
#[derive(Debug, Clone)]
pub enum CsvParseError {
    /// File is not valid CSV
    InvalidFormat(String),
    /// File appears to be binary
    BinaryFile,
    /// Encoding issues
    InvalidUtf8(usize), // line number
    /// Too large to parse
    TooLarge { rows: usize, limit: usize },
}

impl std::fmt::Display for CsvParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidFormat(msg) => write!(f, "Invalid CSV: {}", msg),
            Self::BinaryFile => write!(f, "Cannot parse binary file as CSV"),
            Self::InvalidUtf8(line) => write!(f, "Invalid UTF-8 at line {}", line),
            Self::TooLarge { rows, limit } => {
                write!(f, "CSV too large ({} rows, limit {})", rows, limit)
            }
        }
    }
}
```

### 10.2 Error Display in Status Bar

```rust
// In status bar sync
fn sync_csv_status(model: &mut AppModel) {
    if let ViewMode::Csv(csv) = &model.editor().view_mode {
        // Mode indicator
        model.ui.status_bar.update_segment(
            SegmentId::StatusMessage,
            SegmentContent::Text("CSV".to_string()),
        );
        
        // Error or position
        if let Some(error) = &csv.parse_error {
            model.ui.status_bar.update_segment(
                SegmentId::CursorPosition,
                SegmentContent::Text(format!("⚠ {}", error)),
            );
        } else {
            model.ui.status_bar.update_segment(
                SegmentId::CursorPosition,
                SegmentContent::Text(format!(
                    "Row {}, Col {}",
                    csv.selected_cell.row + 1,
                    column_letter(csv.selected_cell.col)
                )),
            );
        }
        
        // Dimensions
        model.ui.status_bar.update_segment(
            SegmentId::LineCount,
            SegmentContent::Text(format!(
                "{} × {}",
                csv.data.row_count(),
                csv.data.column_count()
            )),
        );
    }
}
```

### 10.3 Graceful Degradation

```rust
impl CsvState {
    /// Attempt to enable CSV mode for a document
    pub fn try_enable(document: &Document, delimiter: Option<u8>) -> Result<Self, CsvParseError> {
        let source = document.buffer.to_string();
        
        // Check for binary content
        if source.bytes().any(|b| b == 0) {
            return Err(CsvParseError::BinaryFile);
        }
        
        // Detect or use provided delimiter
        let delim = delimiter.unwrap_or_else(|| detect_delimiter(&source));
        
        // Parse
        let result = parse_csv(&source, delim, Some(1_000_000))?;
        
        // Sanity check: must have at least 1 column
        if result.data.column_count() == 0 {
            return Err(CsvParseError::InvalidFormat(
                "No columns detected".to_string()
            ));
        }
        
        let mut state = CsvState {
            data: result.data,
            selected_cell: CellPosition::default(),
            selection_range: None,
            viewport: CsvViewport {
                top_row: 0,
                left_col: 0,
                visible_rows: 25, // Will be updated by renderer
                visible_cols: 10,
            },
            editing: None,
            column_widths: vec![],
            has_header_row: true, // Default assumption
            delimiter: delim,
            parse_error: None,
        };
        
        state.compute_column_widths(1000);
        state.data.parsed_revision = document.revision;
        
        Ok(state)
    }
}
```

---

## 11. Performance & Benchmarks

### 11.1 Benchmarking Requirements

Create benchmarks for:
- Parse time: 1K, 5K, 50K, 500K rows
- Render time: Full viewport redraw
- Cell lookup time: Random access
- Scroll performance: Continuous scroll

### 11.2 Benchmark Implementation

```rust
// benches/csv.rs (matches existing convention: rope_operations.rs, rendering.rs, etc.)
use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use token::csv::{parse_csv, CsvData, CsvState};

/// Generate test CSV data
fn generate_csv(rows: usize, cols: usize) -> String {
    let mut result = String::new();
    
    // Header
    for c in 0..cols {
        if c > 0 { result.push(','); }
        result.push_str(&format!("Column{}", c));
    }
    result.push('\n');
    
    // Data rows
    for r in 0..rows {
        for c in 0..cols {
            if c > 0 { result.push(','); }
            result.push_str(&format!("Row{}Col{}", r, c));
        }
        result.push('\n');
    }
    
    result
}

fn bench_parse(c: &mut Criterion) {
    let mut group = c.benchmark_group("csv_parse");
    
    for &row_count in &[1_000, 5_000, 50_000, 500_000] {
        let csv_data = generate_csv(row_count, 10);
        
        group.bench_with_input(
            BenchmarkId::new("rows", row_count),
            &csv_data,
            |b, data| {
                b.iter(|| {
                    parse_csv(black_box(data), b',', None).unwrap()
                });
            },
        );
    }
    
    group.finish();
}

fn bench_cell_access(c: &mut Criterion) {
    let csv_data = generate_csv(50_000, 20);
    let result = parse_csv(&csv_data, b',', None).unwrap();
    let data = result.data;
    
    c.bench_function("cell_access_random", |b| {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        
        b.iter(|| {
            let row = rng.gen_range(0..data.row_count());
            let col = rng.gen_range(0..data.column_count());
            black_box(data.get(row, col));
        });
    });
}

fn bench_column_width_calculation(c: &mut Criterion) {
    let csv_data = generate_csv(50_000, 50);
    let result = parse_csv(&csv_data, b',', None).unwrap();
    
    c.bench_function("column_width_calc", |b| {
        b.iter(|| {
            let mut state = CsvState::default();
            state.data = result.data.clone();
            state.compute_column_widths(black_box(10_000));
        });
    });
}

criterion_group!(benches, bench_parse, bench_cell_access, bench_column_width_calculation);
criterion_main!(benches);
```

### 11.3 Performance Targets

| Operation | Target | Notes |
|-----------|--------|-------|
| Parse 1K rows | < 10ms | Cold start |
| Parse 50K rows | < 100ms | Interactive feel |
| Parse 500K rows | < 1s | Show progress |
| Cell access | < 1µs | O(1) with field delimiter |
| Full viewport render | < 16ms | 60 FPS |
| Scroll 1 page | < 10ms | Smooth scroll |

---

## 12. Testing Strategy

### 12.1 Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_parse_simple_csv() {
        let csv = "a,b,c\n1,2,3\n4,5,6";
        let result = parse_csv(csv, b',', None).unwrap();
        
        assert_eq!(result.data.row_count(), 3);
        assert_eq!(result.data.column_count(), 3);
        assert_eq!(result.data.get(0, 0), "a");
        assert_eq!(result.data.get(2, 2), "6");
    }
    
    #[test]
    fn test_parse_quoted_fields() {
        let csv = r#""Hello, World",123,"Line1
Line2""#;
        let result = parse_csv(csv, b',', None).unwrap();
        
        assert_eq!(result.data.get(0, 0), "Hello, World");
        assert_eq!(result.data.get(0, 2), "Line1\nLine2");
    }
    
    #[test]
    fn test_parse_inconsistent_columns() {
        let csv = "a,b,c\n1,2\n3,4,5,6";
        let result = parse_csv(csv, b',', None).unwrap();
        
        assert_eq!(result.data.column_count(), 4); // Max columns
        assert_eq!(result.warnings.len(), 2); // Two inconsistent rows
    }
    
    #[test]
    fn test_delimiter_detection() {
        assert_eq!(detect_delimiter("a,b,c\n1,2,3"), b',');
        assert_eq!(detect_delimiter("a\tb\tc\n1\t2\t3"), b'\t');
        assert_eq!(detect_delimiter("a;b;c\n1;2;3"), b';');
        assert_eq!(detect_delimiter("a|b|c\n1|2|3"), b'|');
    }
    
    #[test]
    fn test_cell_navigation() {
        let mut state = CsvState::default();
        state.data = create_test_data(10, 5);
        state.selected_cell = CellPosition { row: 0, col: 0 };
        
        state.move_selection(1, 0); // Down
        assert_eq!(state.selected_cell.row, 1);
        
        state.move_selection(0, 1); // Right
        assert_eq!(state.selected_cell.col, 1);
        
        state.move_selection(-10, 0); // Up past top
        assert_eq!(state.selected_cell.row, 0); // Clamped
    }
    
    #[test]
    fn test_cell_edit_and_sync() {
        let mut doc = Document::with_text("a,b,c\n1,2,3");
        let mut state = CsvState::try_enable(&doc, Some(b',')).unwrap();
        
        // Edit cell (1, 1) from "2" to "updated"
        state.selected_cell = CellPosition { row: 1, col: 1 };
        state.start_editing();
        state.editing.as_mut().unwrap().buffer = "updated".to_string();
        
        let edit = state.confirm_edit().unwrap();
        sync_cell_to_document(&mut doc, &state, &edit).unwrap();
        
        assert!(doc.buffer.to_string().contains("updated"));
    }
    
    #[test]
    fn test_escape_csv_value() {
        assert_eq!(escape_csv_value("simple", b','), "simple");
        assert_eq!(escape_csv_value("has,comma", b','), "\"has,comma\"");
        assert_eq!(escape_csv_value("has\"quote", b','), "\"has\"\"quote\"");
        assert_eq!(escape_csv_value("has\nnewline", b','), "\"has\nnewline\"");
    }
    
    #[test]
    fn test_column_letter() {
        assert_eq!(column_letter(0), "A");
        assert_eq!(column_letter(25), "Z");
        assert_eq!(column_letter(26), "AA");
        assert_eq!(column_letter(27), "AB");
        assert_eq!(column_letter(701), "ZZ");
        assert_eq!(column_letter(702), "AAA");
    }
    
    // --- Additional test cases from gap analysis ---
    
    #[test]
    fn test_field_delimiter_in_data() {
        // Verify behavior when source contains 0xFA byte
        let csv = "a,b\nval\xFAwith_delimiter,c";
        let result = parse_csv(csv, b',', None);
        // Document expected behavior: corruption or graceful handling
    }
    
    #[test]
    fn test_multiline_quoted_fields() {
        let csv = r#"Name,Description
Alice,"Line 1
Line 2"
Bob,Simple"#;
        let result = parse_csv(csv, b',', None).unwrap();
        assert_eq!(result.data.row_count(), 3);
        assert_eq!(result.data.get(1, 1), "Line 1\nLine 2");
        assert_eq!(result.data.get(2, 0), "Bob");
    }
    
    #[test]
    fn test_parse_with_bom() {
        // UTF-8 BOM: EF BB BF
        let csv = "\u{FEFF}a,b,c\n1,2,3";
        let result = parse_csv(csv, b',', None).unwrap();
        assert_eq!(result.data.get(0, 0), "a"); // BOM should be stripped
    }
    
    #[test]
    fn test_empty_file() {
        let csv = "";
        let result = parse_csv(csv, b',', None).unwrap();
        assert_eq!(result.data.row_count(), 0);
        assert_eq!(result.data.column_count(), 0);
    }
    
    #[test]
    fn test_single_column_csv() {
        let csv = "a\nb\nc";
        let result = parse_csv(csv, b',', None).unwrap();
        assert_eq!(result.data.column_count(), 1);
        assert_eq!(result.data.row_count(), 3);
    }
    
    #[test]
    fn test_trailing_delimiter() {
        let csv = "a,b,c,\n1,2,3,";
        let result = parse_csv(csv, b',', None).unwrap();
        assert_eq!(result.data.column_count(), 4); // Empty trailing field
    }
    
    #[test]
    fn test_unicode_content() {
        let csv = "名前,説明\nアリス,こんにちは\n🎉,emoji";
        let result = parse_csv(csv, b',', None).unwrap();
        assert_eq!(result.data.get(0, 0), "名前");
        assert_eq!(result.data.get(2, 0), "🎉");
    }
}
```

### 12.2 Integration Tests

```rust
// tests/csv_integration.rs

#[test]
fn test_toggle_csv_mode() {
    let mut model = create_test_model_with_file("test.csv", "a,b,c\n1,2,3");
    
    // Toggle on
    model.handle_message(Msg::Csv(CsvMsg::ToggleCsvMode));
    assert!(matches!(model.editor().view_mode, ViewMode::Csv(_)));
    
    // Toggle off
    model.handle_message(Msg::Csv(CsvMsg::ToggleCsvMode));
    assert!(matches!(model.editor().view_mode, ViewMode::Text));
}

#[test]
fn test_csv_mode_undo_sync() {
    let mut model = create_test_model_with_file("test.csv", "a,b,c\n1,2,3");
    model.handle_message(Msg::Csv(CsvMsg::ToggleCsvMode));
    
    // Edit cell
    model.handle_message(Msg::Csv(CsvMsg::StartEditing));
    model.handle_message(Msg::Csv(CsvMsg::EditInsertChar('X')));
    model.handle_message(Msg::Csv(CsvMsg::ConfirmEdit));
    
    // Undo
    model.handle_message(Msg::Document(DocumentMsg::Undo));
    
    // CSV should be re-synced
    if let ViewMode::Csv(csv) = &model.editor().view_mode {
        assert_eq!(csv.data.get(0, 0), "a"); // Original value restored
    }
}

// --- Additional integration tests from gap analysis ---

#[test]
fn test_toggle_preserves_no_text_state() {
    // Verify cursor/selection NOT preserved on round-trip (per design)
    let mut model = create_test_model_with_file("test.csv", "a,b,c\n1,2,3\n4,5,6");
    
    // Set text cursor to specific position
    model.editor_mut().cursor.position = 10;
    
    // Toggle to CSV
    model.handle_message(Msg::Csv(CsvMsg::ToggleCsvMode));
    
    // Toggle back to text
    model.handle_message(Msg::Csv(CsvMsg::ToggleCsvMode));
    
    // Cursor should be reset (not preserved)
    assert_eq!(model.editor().cursor.position, 0);
}

#[test]
fn test_theme_without_csv_section() {
    // Verify graceful fallback to defaults when theme lacks csv section
    let theme = load_theme("default-dark"); // Has no csv section
    let csv_theme = theme.csv_theme(); // Should return defaults
    
    // Should not panic, should return usable colors
    assert!(csv_theme.header_background.to_argb_u32() != 0);
}

#[test]
fn test_keyboard_shortcuts_csv_mode() {
    let mut model = create_test_model_with_file("test.csv", "a,b,c\n1,2,3");
    model.handle_message(Msg::Csv(CsvMsg::ToggleCsvMode));
    
    // Arrow keys should navigate cells
    model.handle_message(Msg::Csv(CsvMsg::MoveSelection { delta_row: 1, delta_col: 0 }));
    if let ViewMode::Csv(csv) = &model.editor().view_mode {
        assert_eq!(csv.selected_cell.row, 1);
    }
}

#[test]
fn test_status_bar_csv_segments() {
    let mut model = create_test_model_with_file("test.csv", "a,b,c\n1,2,3");
    model.handle_message(Msg::Csv(CsvMsg::ToggleCsvMode));
    
    // Status bar should show CSV-specific info
    let status = model.status_bar();
    assert!(status.contains_segment("CSV"));
    assert!(status.contains_segment("Row 1, Col 1")); // or similar
}
```

### 12.3 Stress Tests

```rust
// tests/csv_stress.rs

#[test]
fn test_large_csv_1k() {
    let csv = generate_csv(1_000, 20);
    let start = std::time::Instant::now();
    let result = parse_csv(&csv, b',', None).unwrap();
    let duration = start.elapsed();
    
    assert_eq!(result.data.row_count(), 1001); // +1 header
    assert!(duration.as_millis() < 100, "Took too long: {:?}", duration);
}

#[test]
fn test_large_csv_50k() {
    let csv = generate_csv(50_000, 20);
    let start = std::time::Instant::now();
    let result = parse_csv(&csv, b',', None).unwrap();
    let duration = start.elapsed();
    
    assert_eq!(result.data.row_count(), 50001);
    assert!(duration.as_millis() < 500, "Took too long: {:?}", duration);
}

#[test]
fn test_large_csv_500k() {
    let csv = generate_csv(500_000, 10);
    let start = std::time::Instant::now();
    let result = parse_csv(&csv, b',', None).unwrap();
    let duration = start.elapsed();
    
    assert_eq!(result.data.row_count(), 500001);
    assert!(duration.as_secs() < 3, "Took too long: {:?}", duration);
}

// --- Additional stress tests from gap analysis ---

#[test]
fn test_wide_columns() {
    // Test cells with 1000+ character values
    let long_value = "x".repeat(1500);
    let csv = format!("header\n{}", long_value);
    let result = parse_csv(&csv, b',', None).unwrap();
    assert_eq!(result.data.get(1, 0).len(), 1500);
}

#[test]
fn test_many_columns() {
    // Test 500+ columns
    let header: String = (0..500).map(|i| format!("col{}", i)).collect::<Vec<_>>().join(",");
    let row: String = (0..500).map(|i| i.to_string()).collect::<Vec<_>>().join(",");
    let csv = format!("{}\n{}", header, row);
    let result = parse_csv(&csv, b',', None).unwrap();
    assert_eq!(result.data.column_count(), 500);
}

#[test]
fn test_rapid_toggle() {
    // Toggle mode 100x quickly without issues
    let mut model = create_test_model_with_file("test.csv", "a,b,c\n1,2,3");
    for _ in 0..100 {
        model.handle_message(Msg::Csv(CsvMsg::ToggleCsvMode));
    }
    // Should not panic or corrupt state
}
```

### 12.4 Render Tests

```rust
// tests/csv_render.rs

#[test]
fn test_csv_grid_render_empty() {
    let mut model = create_test_model_with_file("test.csv", "");
    model.handle_message(Msg::Csv(CsvMsg::ToggleCsvMode));
    // Should render without crash, showing empty grid or message
}

#[test]
fn test_csv_viewport_clipping() {
    // Cells outside viewport should not be rendered
    let csv = generate_csv(1000, 50);
    let mut model = create_test_model_with_file("test.csv", &csv);
    model.handle_message(Msg::Csv(CsvMsg::ToggleCsvMode));
    
    // Render should complete in reasonable time (viewport clipping works)
    let start = std::time::Instant::now();
    model.render(); // or equivalent
    assert!(start.elapsed().as_millis() < 50);
}

#[test]
fn test_csv_selection_highlight() {
    let mut model = create_test_model_with_file("test.csv", "a,b,c\n1,2,3");
    model.handle_message(Msg::Csv(CsvMsg::ToggleCsvMode));
    
    // Active cell should have selection border
    // (This would require render inspection or snapshot testing)
}
```

---

## 13. Implementation Phases

### Phase 1: Read-Only Viewer (MVP)

**Goals:** Display CSV as grid, navigate cells, horizontal scrolling

**Deliverables:**
- [ ] `CsvState` and `CsvData` types
- [ ] CSV parsing with `csv` crate
- [ ] Delimiter detection
- [ ] Grid rendering (cells, headers, row numbers)
- [ ] Cell navigation (arrow keys, Tab)
- [ ] Horizontal scroll for many columns
- [ ] Toggle command (`Cmd+Shift+T`)
- [ ] Status bar integration
- [ ] Basic error handling (show in status bar)
- [ ] Unit tests for parsing

**Estimated effort:** 3-4 days

### Phase 2: Cell Editing

**Goals:** Edit cells in place, sync to document buffer

**Deliverables:**
- [ ] `CellEditState` and edit lifecycle
- [ ] Cell editor rendering (cursor, input)
- [ ] Keyboard input while editing
- [ ] Confirm (Enter) / Cancel (Escape)
- [ ] Sync cell edit → document buffer
- [ ] Sync document changes → CSV (re-parse)
- [ ] Undo/redo integration
- [ ] Integration tests for edit sync

**Estimated effort:** 2-3 days

### Phase 3: Polish & Performance

**Goals:** Large file support, UX improvements

**Deliverables:**
- [ ] Performance benchmarks
- [ ] Virtual scrolling optimization
- [ ] Progress indicator for large files
- [ ] Column auto-width improvements
- [ ] Mouse click to select cell
- [ ] Copy cell value (Cmd+C)
- [ ] Stress tests (500K rows)

**Estimated effort:** 2-3 days

### Phase 4: Advanced Features (Future)

**Potential features:**
- [ ] Multi-cell selection (Shift+Arrow, Shift+Click)
- [ ] Copy/paste range
- [ ] Column resize (mouse drag)
- [ ] Sort by column (click header)
- [ ] Filter rows
- [ ] Find in CSV
- [ ] Import/export formats

---

## 14. Message Types

```rust
/// CSV-specific messages
#[derive(Debug, Clone)]
pub enum CsvMsg {
    /// Toggle CSV mode on/off
    ToggleCsvMode,
    
    /// Force enable CSV mode (even if detection fails)
    EnableCsvMode { delimiter: Option<u8> },
    
    /// Disable CSV mode
    DisableCsvMode,
    
    // === Navigation ===
    /// Move cell selection
    MoveSelection { delta_row: i32, delta_col: i32 },
    /// Tab to next cell
    NextCell,
    /// Shift+Tab to previous cell
    PrevCell,
    /// Go to specific cell
    GoToCell { row: usize, col: usize },
    /// Go to first cell (Cmd+Home)
    GoToFirstCell,
    /// Go to last cell (Cmd+End)
    GoToLastCell,
    
    // === Scrolling ===
    /// Scroll horizontally by columns
    ScrollHorizontal(i32),
    /// Page up
    PageUp,
    /// Page down
    PageDown,
    
    // === Editing (Phase 2) ===
    /// Start editing selected cell
    StartEditing,
    /// Start editing with initial character
    StartEditingWithChar(char),
    /// Confirm edit and update data
    ConfirmEdit,
    /// Cancel edit
    CancelEdit,
    /// Insert character while editing
    EditInsertChar(char),
    /// Delete backward while editing
    EditDeleteBackward,
    /// Move cursor left in edit buffer
    EditCursorLeft,
    /// Move cursor right in edit buffer
    EditCursorRight,
    
    // === Mouse ===
    /// Click on cell
    ClickCell { row: usize, col: usize },
}
```

---

## 15. Commands

Add to `src/commands.rs`:

```rust
pub enum CommandId {
    // ... existing commands ...
    
    /// Toggle CSV viewer mode
    ToggleCsvView,
}

pub static COMMANDS: &[CommandDef] = &[
    // ... existing commands ...
    
    CommandDef {
        id: CommandId::ToggleCsvView,
        label: "Toggle CSV View",
        keybinding: Some("⇧⌘T"),
    },
];
```

---

## 16. Configuration

### 16.1 Theme Extensions

Add to `theme.rs`:

```rust
pub struct CsvTheme {
    /// Header row background
    pub header_background: Color,
    /// Header row foreground
    pub header_foreground: Color,
    /// Row number (gutter) background
    pub row_number_background: Color,
    /// Row number foreground
    pub row_number_foreground: Color,
    /// Grid line color
    pub grid_line: Color,
    /// Selected cell border color
    pub selection_border: Color,
    /// Number cell foreground (right-aligned)
    pub number_foreground: Color,
    /// Alternating row background (optional)
    pub alternate_row_background: Option<Color>,
}

/// Default fallbacks when theme has no `csv` section
/// (Current themes: dark, fleet-dark, github-dark, github-light lack csv sections)
impl Default for CsvTheme {
    fn default() -> Self {
        // Derive from existing theme fields
        Self {
            header_background: theme.gutter.background,
            header_foreground: theme.gutter.foreground_active,
            row_number_background: theme.gutter.background,
            row_number_foreground: theme.gutter.foreground,
            grid_line: theme.gutter.border_color,
            selection_border: theme.editor.cursor_color,
            number_foreground: theme.syntax.number,
            alternate_row_background: None,
        }
    }
}
```

### 16.2 Editor Config Extensions

Add to `config.rs`:

```rust
pub struct CsvConfig {
    /// Treat first row as header
    pub default_has_header: bool,
    /// Maximum rows to sample for column width calculation
    pub width_sample_rows: usize,
    /// Minimum column width (characters)
    pub min_column_width: usize,
    /// Maximum column width (characters)
    pub max_column_width: usize,
}

impl Default for CsvConfig {
    fn default() -> Self {
        Self {
            default_has_header: true,
            width_sample_rows: 1000,
            min_column_width: 4,
            max_column_width: 50,
        }
    }
}
```

> **Note:** No `auto_enable` setting—CSV mode is always manual via command palette.
> No size limits are enforced; large files should work with the virtual scrolling implementation.

---

## Appendix A: File Type Detection

```rust
impl LanguageId {
    /// Check if this language ID represents a CSV-like format
    pub fn is_csv_like(&self) -> bool {
        matches!(self, 
            LanguageId::Csv | 
            LanguageId::Tsv
        )
    }
}

// In Document::from_file
pub fn from_file(path: PathBuf) -> Result<Self, std::io::Error> {
    let content = std::fs::read_to_string(&path)?;
    let language = LanguageId::from_path(&path);
    
    // Could add CSV auto-detection here if language.is_csv_like()
    
    Ok(Self { /* ... */ })
}
```

---

## Appendix B: References

- [RFC 4180 - Common Format and MIME Type for CSV Files](https://tools.ietf.org/html/rfc4180)
- [Tablecruncher Source](https://github.com/Tablecruncher/tablecruncher)
- [Rust `csv` crate](https://docs.rs/csv/latest/csv/)
- [EDITOR_UI_REFERENCE.md](../EDITOR_UI_REFERENCE.md) - Viewport and coordinate systems

---

*Last updated: 2025-12-16*
