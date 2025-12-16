//! CSV mode update functions
//!
//! Handles CsvMsg messages for CSV view mode operations.

use crate::commands::Cmd;
use crate::csv::{detect_delimiter, parse_csv, CsvState, Delimiter};
use crate::messages::CsvMsg;
use crate::model::{AppModel, ViewMode};

/// Handle CSV mode messages
pub fn update_csv(model: &mut AppModel, msg: CsvMsg) -> Option<Cmd> {
    match msg {
        CsvMsg::Toggle => toggle_csv_mode(model),
        CsvMsg::Exit => exit_csv_mode(model),
        CsvMsg::MoveUp => move_selection(model, -1, 0),
        CsvMsg::MoveDown => move_selection(model, 1, 0),
        CsvMsg::MoveLeft => move_selection(model, 0, -1),
        CsvMsg::MoveRight => move_selection(model, 0, 1),
        CsvMsg::NextCell => next_cell(model),
        CsvMsg::PrevCell => prev_cell(model),
        CsvMsg::FirstCell => first_cell(model),
        CsvMsg::LastCell => last_cell(model),
        CsvMsg::RowStart => row_start(model),
        CsvMsg::RowEnd => row_end(model),
        CsvMsg::PageUp => page_up(model),
        CsvMsg::PageDown => page_down(model),
        CsvMsg::SelectCell { row, col } => select_cell(model, row, col),
        CsvMsg::ScrollVertical(delta) => scroll_vertical(model, delta),
        CsvMsg::ScrollHorizontal(delta) => scroll_horizontal(model, delta),
    }
}

/// Toggle CSV view mode
fn toggle_csv_mode(model: &mut AppModel) -> Option<Cmd> {
    let editor_id = model.editor_area.focused_group()?.active_editor_id()?;
    let editor = model.editor_area.editors.get_mut(&editor_id)?;

    if editor.view_mode.is_csv() {
        // Exit CSV mode - just discard the state
        editor.view_mode = ViewMode::Text;
        return Some(Cmd::Redraw);
    }

    // Get document content to parse
    let doc_id = editor.document_id?;
    let doc = model.editor_area.documents.get(&doc_id)?;
    let content = doc.buffer.to_string();

    // Detect delimiter from file extension or content
    let delimiter = doc
        .file_path
        .as_ref()
        .and_then(|p| p.extension())
        .and_then(|e| e.to_str())
        .map(Delimiter::from_extension)
        .unwrap_or_else(|| detect_delimiter(&content));

    match parse_csv(&content, delimiter) {
        Ok(data) => {
            if data.is_empty() || data.column_count() == 0 {
                tracing::warn!("CSV parsing produced empty data");
                return Some(Cmd::Redraw);
            }
            let mut csv_state = CsvState::new(data, delimiter);

            // Calculate visible rows based on window dimensions
            let line_height = model.line_height.max(1);
            let tab_bar_height = model.metrics.tab_bar_height;
            let status_bar_height = line_height;
            let col_header_height = line_height;
            let content_height = (model.window_size.1 as usize)
                .saturating_sub(tab_bar_height)
                .saturating_sub(status_bar_height)
                .saturating_sub(col_header_height);
            let visible_rows = content_height / line_height;
            let visible_cols = 10; // Approximate, will be refined during render
            csv_state.set_viewport_size(visible_rows.max(1), visible_cols);

            // Need to get mutable reference again after the doc borrow is done
            if let Some(editor) = model.editor_area.editors.get_mut(&editor_id) {
                editor.view_mode = ViewMode::Csv(Box::new(csv_state));
            }
        }
        Err(e) => {
            tracing::error!("Failed to parse CSV: {}", e);
        }
    }

    Some(Cmd::Redraw)
}

/// Exit CSV mode
fn exit_csv_mode(model: &mut AppModel) -> Option<Cmd> {
    let editor = model.editor_area.focused_editor_mut()?;
    if editor.view_mode.is_csv() {
        editor.view_mode = ViewMode::Text;
        Some(Cmd::Redraw)
    } else {
        None
    }
}

/// Move selection by delta
fn move_selection(model: &mut AppModel, delta_row: i32, delta_col: i32) -> Option<Cmd> {
    let editor = model.editor_area.focused_editor_mut()?;
    if let Some(csv) = editor.view_mode.as_csv_mut() {
        csv.move_selection(delta_row, delta_col);
        Some(Cmd::Redraw)
    } else {
        None
    }
}

/// Move to next cell
fn next_cell(model: &mut AppModel) -> Option<Cmd> {
    let editor = model.editor_area.focused_editor_mut()?;
    if let Some(csv) = editor.view_mode.as_csv_mut() {
        csv.move_to_next_cell();
        Some(Cmd::Redraw)
    } else {
        None
    }
}

/// Move to previous cell
fn prev_cell(model: &mut AppModel) -> Option<Cmd> {
    let editor = model.editor_area.focused_editor_mut()?;
    if let Some(csv) = editor.view_mode.as_csv_mut() {
        csv.move_to_prev_cell();
        Some(Cmd::Redraw)
    } else {
        None
    }
}

/// Move to first cell
fn first_cell(model: &mut AppModel) -> Option<Cmd> {
    let editor = model.editor_area.focused_editor_mut()?;
    if let Some(csv) = editor.view_mode.as_csv_mut() {
        csv.move_to_first_cell();
        Some(Cmd::Redraw)
    } else {
        None
    }
}

/// Move to last cell
fn last_cell(model: &mut AppModel) -> Option<Cmd> {
    let editor = model.editor_area.focused_editor_mut()?;
    if let Some(csv) = editor.view_mode.as_csv_mut() {
        csv.move_to_last_cell();
        Some(Cmd::Redraw)
    } else {
        None
    }
}

/// Move to row start
fn row_start(model: &mut AppModel) -> Option<Cmd> {
    let editor = model.editor_area.focused_editor_mut()?;
    if let Some(csv) = editor.view_mode.as_csv_mut() {
        csv.move_to_row_start();
        Some(Cmd::Redraw)
    } else {
        None
    }
}

/// Move to row end
fn row_end(model: &mut AppModel) -> Option<Cmd> {
    let editor = model.editor_area.focused_editor_mut()?;
    if let Some(csv) = editor.view_mode.as_csv_mut() {
        csv.move_to_row_end();
        Some(Cmd::Redraw)
    } else {
        None
    }
}

/// Page up
fn page_up(model: &mut AppModel) -> Option<Cmd> {
    let editor = model.editor_area.focused_editor_mut()?;
    if let Some(csv) = editor.view_mode.as_csv_mut() {
        csv.page_up();
        Some(Cmd::Redraw)
    } else {
        None
    }
}

/// Page down
fn page_down(model: &mut AppModel) -> Option<Cmd> {
    let editor = model.editor_area.focused_editor_mut()?;
    if let Some(csv) = editor.view_mode.as_csv_mut() {
        csv.page_down();
        Some(Cmd::Redraw)
    } else {
        None
    }
}

/// Select a specific cell (from mouse click)
fn select_cell(model: &mut AppModel, row: usize, col: usize) -> Option<Cmd> {
    let editor = model.editor_area.focused_editor_mut()?;
    if let Some(csv) = editor.view_mode.as_csv_mut() {
        csv.select_cell(row, col);
        Some(Cmd::Redraw)
    } else {
        None
    }
}

/// Scroll viewport vertically (from mouse wheel)
fn scroll_vertical(model: &mut AppModel, delta: i32) -> Option<Cmd> {
    let editor = model.editor_area.focused_editor_mut()?;
    if let Some(csv) = editor.view_mode.as_csv_mut() {
        csv.scroll_vertical(delta);
        Some(Cmd::Redraw)
    } else {
        None
    }
}

/// Scroll viewport horizontally (from mouse wheel)
fn scroll_horizontal(model: &mut AppModel, delta: i32) -> Option<Cmd> {
    let editor = model.editor_area.focused_editor_mut()?;
    if let Some(csv) = editor.view_mode.as_csv_mut() {
        csv.scroll_horizontal(delta);
        Some(Cmd::Redraw)
    } else {
        None
    }
}
