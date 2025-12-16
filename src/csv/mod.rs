//! CSV Viewer/Editor Mode
//!
//! Provides a spreadsheet-like view for CSV/TSV/PSV files with:
//! - Grid rendering with row/column headers
//! - Cell navigation (arrow keys, Tab, Enter)
//! - Cell editing with sync back to document buffer (Phase 2)
//!
//! # Architecture
//!
//! CSV mode is an alternate view of an existing `EditorState`. The same
//! `Document` is shared between text mode and CSV mode.
//!
//! ```text
//! EditorState
//! └── ViewMode
//!     ├── Text (default)
//!     └── Csv(CsvState)
//!             ├── CsvData (parsed grid)
//!             ├── CsvViewport (visible region)
//!             └── CellEditState (when editing)
//! ```

mod model;
mod navigation;
mod parser;
pub mod render;
mod viewport;

pub use model::{CellPosition, CsvData, CsvState, Delimiter};
pub use parser::{detect_delimiter, parse_csv, ParseError};
pub use viewport::CsvViewport;
