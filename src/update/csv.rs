//! CSV mode update functions
//!
//! Handles CsvMsg messages for CSV view mode operations.

use crate::commands::Cmd;
use crate::csv::{detect_delimiter, escape_csv_value, parse_csv, CellEdit, CsvState, Delimiter};
use crate::messages::CsvMsg;
use crate::model::{AppModel, ViewMode};

/// Handle CSV mode messages
pub fn update_csv(model: &mut AppModel, msg: CsvMsg) -> Option<Cmd> {
    match msg {
        CsvMsg::Toggle => toggle_csv_mode(model),
        CsvMsg::Exit => exit_or_cancel_edit(model),
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

        // Cell editing messages
        CsvMsg::StartEditing => start_editing(model),
        CsvMsg::StartEditingWithChar(ch) => start_editing_with_char(model, ch),
        CsvMsg::ConfirmEdit => confirm_edit(model, 1), // Move down
        CsvMsg::ConfirmEditUp => confirm_edit(model, -1), // Move up
        CsvMsg::CancelEdit => cancel_edit(model),
        CsvMsg::EditInsertChar(ch) => edit_insert_char(model, ch),
        CsvMsg::EditDeleteBackward => edit_delete_backward(model),
        CsvMsg::EditDeleteForward => edit_delete_forward(model),
        CsvMsg::EditCursorLeft => edit_cursor_left(model),
        CsvMsg::EditCursorRight => edit_cursor_right(model),
        CsvMsg::EditCursorHome => edit_cursor_home(model),
        CsvMsg::EditCursorEnd => edit_cursor_end(model),

        // Enhanced editing (via unified editable system)
        CsvMsg::EditCursorWordLeft => edit_cursor_word_left(model),
        CsvMsg::EditCursorWordRight => edit_cursor_word_right(model),
        CsvMsg::EditDeleteWordBackward => edit_delete_word_backward(model),
        CsvMsg::EditDeleteWordForward => edit_delete_word_forward(model),
        CsvMsg::EditSelectAll => edit_select_all(model),
        CsvMsg::EditUndo => edit_undo(model),
        CsvMsg::EditRedo => edit_redo(model),

        // Selection movement
        CsvMsg::EditCursorLeftWithSelection => edit_cursor_left_with_selection(model),
        CsvMsg::EditCursorRightWithSelection => edit_cursor_right_with_selection(model),
        CsvMsg::EditCursorHomeWithSelection => edit_cursor_home_with_selection(model),
        CsvMsg::EditCursorEndWithSelection => edit_cursor_end_with_selection(model),
        CsvMsg::EditCursorWordLeftWithSelection => edit_cursor_word_left_with_selection(model),
        CsvMsg::EditCursorWordRightWithSelection => edit_cursor_word_right_with_selection(model),

        // Clipboard
        CsvMsg::EditCopy => edit_copy(model),
        CsvMsg::EditCut => edit_cut(model),
        CsvMsg::EditPaste => edit_paste(model),
    }
}

/// Toggle CSV view mode
fn toggle_csv_mode(model: &mut AppModel) -> Option<Cmd> {
    let editor_id = model.editor_area.focused_group()?.active_editor_id()?;
    let editor = model.editor_area.editors.get_mut(&editor_id)?;

    if editor.view_mode.is_csv() {
        // Exit CSV mode - just discard the state
        editor.view_mode = ViewMode::Text;
        return Some(Cmd::redraw_editor());
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
                return Some(Cmd::redraw_editor());
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

    Some(Cmd::redraw_editor())
}

/// Exit CSV mode or cancel edit if editing
fn exit_or_cancel_edit(model: &mut AppModel) -> Option<Cmd> {
    let editor = model.editor_area.focused_editor_mut()?;
    if let Some(csv) = editor.view_mode.as_csv_mut() {
        if csv.is_editing() {
            csv.cancel_edit();
            return Some(Cmd::redraw_editor());
        }
        editor.view_mode = ViewMode::Text;
        Some(Cmd::redraw_editor())
    } else {
        None
    }
}

/// Move selection by delta
fn move_selection(model: &mut AppModel, delta_row: i32, delta_col: i32) -> Option<Cmd> {
    let editor = model.editor_area.focused_editor_mut()?;
    if let Some(csv) = editor.view_mode.as_csv_mut() {
        csv.move_selection(delta_row, delta_col);
        Some(Cmd::redraw_editor())
    } else {
        None
    }
}

/// Move to next cell
fn next_cell(model: &mut AppModel) -> Option<Cmd> {
    let editor = model.editor_area.focused_editor_mut()?;
    if let Some(csv) = editor.view_mode.as_csv_mut() {
        csv.move_to_next_cell();
        Some(Cmd::redraw_editor())
    } else {
        None
    }
}

/// Move to previous cell
fn prev_cell(model: &mut AppModel) -> Option<Cmd> {
    let editor = model.editor_area.focused_editor_mut()?;
    if let Some(csv) = editor.view_mode.as_csv_mut() {
        csv.move_to_prev_cell();
        Some(Cmd::redraw_editor())
    } else {
        None
    }
}

/// Move to first cell
fn first_cell(model: &mut AppModel) -> Option<Cmd> {
    let editor = model.editor_area.focused_editor_mut()?;
    if let Some(csv) = editor.view_mode.as_csv_mut() {
        csv.move_to_first_cell();
        Some(Cmd::redraw_editor())
    } else {
        None
    }
}

/// Move to last cell
fn last_cell(model: &mut AppModel) -> Option<Cmd> {
    let editor = model.editor_area.focused_editor_mut()?;
    if let Some(csv) = editor.view_mode.as_csv_mut() {
        csv.move_to_last_cell();
        Some(Cmd::redraw_editor())
    } else {
        None
    }
}

/// Move to row start
fn row_start(model: &mut AppModel) -> Option<Cmd> {
    let editor = model.editor_area.focused_editor_mut()?;
    if let Some(csv) = editor.view_mode.as_csv_mut() {
        csv.move_to_row_start();
        Some(Cmd::redraw_editor())
    } else {
        None
    }
}

/// Move to row end
fn row_end(model: &mut AppModel) -> Option<Cmd> {
    let editor = model.editor_area.focused_editor_mut()?;
    if let Some(csv) = editor.view_mode.as_csv_mut() {
        csv.move_to_row_end();
        Some(Cmd::redraw_editor())
    } else {
        None
    }
}

/// Page up
fn page_up(model: &mut AppModel) -> Option<Cmd> {
    let editor = model.editor_area.focused_editor_mut()?;
    if let Some(csv) = editor.view_mode.as_csv_mut() {
        csv.page_up();
        Some(Cmd::redraw_editor())
    } else {
        None
    }
}

/// Page down
fn page_down(model: &mut AppModel) -> Option<Cmd> {
    let editor = model.editor_area.focused_editor_mut()?;
    if let Some(csv) = editor.view_mode.as_csv_mut() {
        csv.page_down();
        Some(Cmd::redraw_editor())
    } else {
        None
    }
}

/// Select a specific cell (from mouse click)
fn select_cell(model: &mut AppModel, row: usize, col: usize) -> Option<Cmd> {
    let editor = model.editor_area.focused_editor_mut()?;
    if let Some(csv) = editor.view_mode.as_csv_mut() {
        csv.select_cell(row, col);
        Some(Cmd::redraw_editor())
    } else {
        None
    }
}

/// Scroll viewport vertically (from mouse wheel)
fn scroll_vertical(model: &mut AppModel, delta: i32) -> Option<Cmd> {
    let editor = model.editor_area.focused_editor_mut()?;
    if let Some(csv) = editor.view_mode.as_csv_mut() {
        csv.scroll_vertical(delta);
        Some(Cmd::redraw_editor())
    } else {
        None
    }
}

/// Scroll viewport horizontally (from mouse wheel)
fn scroll_horizontal(model: &mut AppModel, delta: i32) -> Option<Cmd> {
    let editor = model.editor_area.focused_editor_mut()?;
    if let Some(csv) = editor.view_mode.as_csv_mut() {
        csv.scroll_horizontal(delta);
        Some(Cmd::redraw_editor())
    } else {
        None
    }
}

// === Cell Editing Functions ===

/// Start editing the selected cell
fn start_editing(model: &mut AppModel) -> Option<Cmd> {
    let editor = model.editor_area.focused_editor_mut()?;
    if let Some(csv) = editor.view_mode.as_csv_mut() {
        if csv.is_editing() {
            return None;
        }
        csv.start_editing();
        Some(Cmd::redraw_editor())
    } else {
        None
    }
}

/// Start editing with initial character (replaces cell content)
fn start_editing_with_char(model: &mut AppModel, ch: char) -> Option<Cmd> {
    let editor = model.editor_area.focused_editor_mut()?;
    if let Some(csv) = editor.view_mode.as_csv_mut() {
        if csv.is_editing() {
            csv.edit_insert_char(ch);
        } else {
            csv.start_editing_with_char(ch);
        }
        Some(Cmd::redraw_editor())
    } else {
        None
    }
}

/// Confirm edit and sync to document, then move in specified direction
fn confirm_edit(model: &mut AppModel, row_delta: i32) -> Option<Cmd> {
    let editor_id = model.editor_area.focused_group()?.active_editor_id()?;
    let editor = model.editor_area.editors.get_mut(&editor_id)?;

    let (edit, delimiter) = if let Some(csv) = editor.view_mode.as_csv_mut() {
        let delimiter = csv.delimiter;
        (csv.confirm_edit(), delimiter)
    } else {
        return None;
    };

    if let Some(cell_edit) = edit {
        let doc_id = editor.document_id?;
        if let Some(doc) = model.editor_area.documents.get_mut(&doc_id) {
            sync_cell_edit_to_document(doc, &cell_edit, delimiter);
        }
    }

    // Move in specified direction after confirming edit
    if let Some(editor) = model.editor_area.editors.get_mut(&editor_id) {
        if let Some(csv) = editor.view_mode.as_csv_mut() {
            csv.move_selection(row_delta, 0);
        }
    }

    Some(Cmd::redraw_editor())
}

/// Cancel edit and discard changes
fn cancel_edit(model: &mut AppModel) -> Option<Cmd> {
    let editor = model.editor_area.focused_editor_mut()?;
    if let Some(csv) = editor.view_mode.as_csv_mut() {
        csv.cancel_edit();
        Some(Cmd::redraw_editor())
    } else {
        None
    }
}

/// Insert character into edit buffer
fn edit_insert_char(model: &mut AppModel, ch: char) -> Option<Cmd> {
    let editor = model.editor_area.focused_editor_mut()?;
    if let Some(csv) = editor.view_mode.as_csv_mut() {
        csv.edit_insert_char(ch);
        Some(Cmd::redraw_editor())
    } else {
        None
    }
}

/// Delete backward in edit buffer
fn edit_delete_backward(model: &mut AppModel) -> Option<Cmd> {
    let editor = model.editor_area.focused_editor_mut()?;
    if let Some(csv) = editor.view_mode.as_csv_mut() {
        csv.edit_delete_backward();
        Some(Cmd::redraw_editor())
    } else {
        None
    }
}

/// Delete forward in edit buffer
fn edit_delete_forward(model: &mut AppModel) -> Option<Cmd> {
    let editor = model.editor_area.focused_editor_mut()?;
    if let Some(csv) = editor.view_mode.as_csv_mut() {
        csv.edit_delete_forward();
        Some(Cmd::redraw_editor())
    } else {
        None
    }
}

/// Move cursor left in edit buffer
fn edit_cursor_left(model: &mut AppModel) -> Option<Cmd> {
    let editor = model.editor_area.focused_editor_mut()?;
    if let Some(csv) = editor.view_mode.as_csv_mut() {
        csv.edit_cursor_left();
        Some(Cmd::redraw_editor())
    } else {
        None
    }
}

/// Move cursor right in edit buffer
fn edit_cursor_right(model: &mut AppModel) -> Option<Cmd> {
    let editor = model.editor_area.focused_editor_mut()?;
    if let Some(csv) = editor.view_mode.as_csv_mut() {
        csv.edit_cursor_right();
        Some(Cmd::redraw_editor())
    } else {
        None
    }
}

/// Move cursor to start in edit buffer
fn edit_cursor_home(model: &mut AppModel) -> Option<Cmd> {
    let editor = model.editor_area.focused_editor_mut()?;
    if let Some(csv) = editor.view_mode.as_csv_mut() {
        csv.edit_cursor_home();
        Some(Cmd::redraw_editor())
    } else {
        None
    }
}

/// Move cursor to end in edit buffer
fn edit_cursor_end(model: &mut AppModel) -> Option<Cmd> {
    let editor = model.editor_area.focused_editor_mut()?;
    if let Some(csv) = editor.view_mode.as_csv_mut() {
        csv.edit_cursor_end();
        Some(Cmd::redraw_editor())
    } else {
        None
    }
}

/// Move cursor left by word in edit buffer
fn edit_cursor_word_left(model: &mut AppModel) -> Option<Cmd> {
    let editor = model.editor_area.focused_editor_mut()?;
    if let Some(csv) = editor.view_mode.as_csv_mut() {
        if let Some(edit) = &mut csv.editing {
            edit.cursor_word_left();
        }
        Some(Cmd::redraw_editor())
    } else {
        None
    }
}

/// Move cursor right by word in edit buffer
fn edit_cursor_word_right(model: &mut AppModel) -> Option<Cmd> {
    let editor = model.editor_area.focused_editor_mut()?;
    if let Some(csv) = editor.view_mode.as_csv_mut() {
        if let Some(edit) = &mut csv.editing {
            edit.cursor_word_right();
        }
        Some(Cmd::redraw_editor())
    } else {
        None
    }
}

/// Delete word backward in edit buffer
fn edit_delete_word_backward(model: &mut AppModel) -> Option<Cmd> {
    let editor = model.editor_area.focused_editor_mut()?;
    if let Some(csv) = editor.view_mode.as_csv_mut() {
        if let Some(edit) = &mut csv.editing {
            edit.delete_word_backward();
        }
        Some(Cmd::redraw_editor())
    } else {
        None
    }
}

/// Delete word forward in edit buffer
fn edit_delete_word_forward(model: &mut AppModel) -> Option<Cmd> {
    let editor = model.editor_area.focused_editor_mut()?;
    if let Some(csv) = editor.view_mode.as_csv_mut() {
        if let Some(edit) = &mut csv.editing {
            edit.delete_word_forward();
        }
        Some(Cmd::redraw_editor())
    } else {
        None
    }
}

/// Select all text in edit buffer
fn edit_select_all(model: &mut AppModel) -> Option<Cmd> {
    let editor = model.editor_area.focused_editor_mut()?;
    if let Some(csv) = editor.view_mode.as_csv_mut() {
        if let Some(edit) = &mut csv.editing {
            edit.select_all();
        }
        Some(Cmd::redraw_editor())
    } else {
        None
    }
}

/// Undo last edit operation
fn edit_undo(model: &mut AppModel) -> Option<Cmd> {
    let editor = model.editor_area.focused_editor_mut()?;
    if let Some(csv) = editor.view_mode.as_csv_mut() {
        if let Some(edit) = &mut csv.editing {
            edit.undo();
        }
        Some(Cmd::redraw_editor())
    } else {
        None
    }
}

/// Redo last undone operation
fn edit_redo(model: &mut AppModel) -> Option<Cmd> {
    let editor = model.editor_area.focused_editor_mut()?;
    if let Some(csv) = editor.view_mode.as_csv_mut() {
        if let Some(edit) = &mut csv.editing {
            edit.redo();
        }
        Some(Cmd::redraw_editor())
    } else {
        None
    }
}

// === Selection Movement ===

/// Move cursor left with selection
fn edit_cursor_left_with_selection(model: &mut AppModel) -> Option<Cmd> {
    let editor = model.editor_area.focused_editor_mut()?;
    if let Some(csv) = editor.view_mode.as_csv_mut() {
        if let Some(edit) = &mut csv.editing {
            edit.cursor_left_with_selection();
        }
        Some(Cmd::redraw_editor())
    } else {
        None
    }
}

/// Move cursor right with selection
fn edit_cursor_right_with_selection(model: &mut AppModel) -> Option<Cmd> {
    let editor = model.editor_area.focused_editor_mut()?;
    if let Some(csv) = editor.view_mode.as_csv_mut() {
        if let Some(edit) = &mut csv.editing {
            edit.cursor_right_with_selection();
        }
        Some(Cmd::redraw_editor())
    } else {
        None
    }
}

/// Move cursor to start with selection
fn edit_cursor_home_with_selection(model: &mut AppModel) -> Option<Cmd> {
    let editor = model.editor_area.focused_editor_mut()?;
    if let Some(csv) = editor.view_mode.as_csv_mut() {
        if let Some(edit) = &mut csv.editing {
            edit.cursor_home_with_selection();
        }
        Some(Cmd::redraw_editor())
    } else {
        None
    }
}

/// Move cursor to end with selection
fn edit_cursor_end_with_selection(model: &mut AppModel) -> Option<Cmd> {
    let editor = model.editor_area.focused_editor_mut()?;
    if let Some(csv) = editor.view_mode.as_csv_mut() {
        if let Some(edit) = &mut csv.editing {
            edit.cursor_end_with_selection();
        }
        Some(Cmd::redraw_editor())
    } else {
        None
    }
}

/// Move cursor word left with selection
fn edit_cursor_word_left_with_selection(model: &mut AppModel) -> Option<Cmd> {
    let editor = model.editor_area.focused_editor_mut()?;
    if let Some(csv) = editor.view_mode.as_csv_mut() {
        if let Some(edit) = &mut csv.editing {
            edit.cursor_word_left_with_selection();
        }
        Some(Cmd::redraw_editor())
    } else {
        None
    }
}

/// Move cursor word right with selection
fn edit_cursor_word_right_with_selection(model: &mut AppModel) -> Option<Cmd> {
    let editor = model.editor_area.focused_editor_mut()?;
    if let Some(csv) = editor.view_mode.as_csv_mut() {
        if let Some(edit) = &mut csv.editing {
            edit.cursor_word_right_with_selection();
        }
        Some(Cmd::redraw_editor())
    } else {
        None
    }
}

// === Clipboard ===

/// Copy selection to clipboard
fn edit_copy(model: &mut AppModel) -> Option<Cmd> {
    let editor = model.editor_area.focused_editor_mut()?;
    if let Some(csv) = editor.view_mode.as_csv_mut() {
        if let Some(edit) = &mut csv.editing {
            let text = edit.selected_text();
            if !text.is_empty() {
                if let Ok(mut clipboard) = arboard::Clipboard::new() {
                    let _ = clipboard.set_text(&text);
                }
            }
        }
        Some(Cmd::redraw_editor())
    } else {
        None
    }
}

/// Cut selection to clipboard
fn edit_cut(model: &mut AppModel) -> Option<Cmd> {
    let editor = model.editor_area.focused_editor_mut()?;
    if let Some(csv) = editor.view_mode.as_csv_mut() {
        if let Some(edit) = &mut csv.editing {
            let text = edit.selected_text();
            if !text.is_empty() {
                if let Ok(mut clipboard) = arboard::Clipboard::new() {
                    let _ = clipboard.set_text(&text);
                }
                edit.delete_backward();
            }
        }
        Some(Cmd::redraw_editor())
    } else {
        None
    }
}

/// Paste from clipboard
fn edit_paste(model: &mut AppModel) -> Option<Cmd> {
    let clipboard_text = if let Ok(mut clipboard) = arboard::Clipboard::new() {
        clipboard.get_text().ok()
    } else {
        None
    };

    if let Some(text) = clipboard_text {
        let editor = model.editor_area.focused_editor_mut()?;
        if let Some(csv) = editor.view_mode.as_csv_mut() {
            if let Some(edit) = &mut csv.editing {
                // Filter out newlines for single-line cell editing
                let filtered: String = text.chars().filter(|c| *c != '\n' && *c != '\r').collect();
                edit.insert_text(&filtered);
            }
            return Some(Cmd::redraw_editor());
        }
    }
    None
}

// === Document Sync ===

use crate::model::Document;

/// Sync a cell edit back to the document text buffer
fn sync_cell_edit_to_document(doc: &mut Document, edit: &CellEdit, delimiter: Delimiter) {
    let content = doc.buffer.to_string();

    let row_range = match find_row_byte_range(&content, edit.position.row) {
        Some(r) => r,
        None => {
            tracing::warn!("Could not find row {} in document", edit.position.row);
            return;
        }
    };

    let row_content = &content[row_range.clone()];

    let cell_range = match find_field_byte_range(row_content, edit.position.col, delimiter) {
        Some(r) => r,
        None => {
            tracing::warn!(
                "Could not find field {} in row {}",
                edit.position.col,
                edit.position.row
            );
            return;
        }
    };

    let abs_start = row_range.start + cell_range.start;
    let abs_end = row_range.start + cell_range.end;

    let escaped = escape_csv_value(&edit.new_value, delimiter);

    doc.buffer.remove(abs_start..abs_end);
    doc.buffer.insert(abs_start, &escaped);

    doc.is_modified = true;
    doc.revision = doc.revision.wrapping_add(1);
}

/// Find byte range of a row in the document (excluding newline)
fn find_row_byte_range(content: &str, row_idx: usize) -> Option<std::ops::Range<usize>> {
    let mut current_row = 0;
    let mut row_start = 0;

    for (i, ch) in content.char_indices() {
        if ch == '\n' {
            if current_row == row_idx {
                return Some(row_start..i);
            }
            current_row += 1;
            row_start = i + 1;
        }
    }

    if current_row == row_idx {
        return Some(row_start..content.len());
    }

    None
}

/// Find byte range of a field within a CSV row (handles quoted fields)
fn find_field_byte_range(
    row: &str,
    field_idx: usize,
    delimiter: Delimiter,
) -> Option<std::ops::Range<usize>> {
    let delim = delimiter.char();
    let mut field_start = 0;
    let mut current_field = 0;
    let mut in_quotes = false;

    for (i, ch) in row.char_indices() {
        if ch == '"' {
            in_quotes = !in_quotes;
        } else if ch == delim && !in_quotes {
            if current_field == field_idx {
                return Some(field_start..i);
            }
            current_field += 1;
            field_start = i + ch.len_utf8();
        }
    }

    if current_field == field_idx {
        return Some(field_start..row.len());
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_row_byte_range() {
        let content = "a,b,c\n1,2,3\nx,y,z";

        assert_eq!(find_row_byte_range(content, 0), Some(0..5));
        assert_eq!(find_row_byte_range(content, 1), Some(6..11));
        assert_eq!(find_row_byte_range(content, 2), Some(12..17));
        assert_eq!(find_row_byte_range(content, 3), None);
    }

    #[test]
    fn test_find_field_byte_range() {
        let row = "alice,30,engineer";

        assert_eq!(find_field_byte_range(row, 0, Delimiter::Comma), Some(0..5));
        assert_eq!(find_field_byte_range(row, 1, Delimiter::Comma), Some(6..8));
        assert_eq!(find_field_byte_range(row, 2, Delimiter::Comma), Some(9..17));
        assert_eq!(find_field_byte_range(row, 3, Delimiter::Comma), None);
    }

    #[test]
    fn test_find_field_byte_range_quoted() {
        let row = "\"hello, world\",test,123";
        // Field 0: "hello, world" (positions 0..14, the comma inside is at index 6)
        // Delimiter at position 14
        // Field 1: test (positions 15..19)
        // Delimiter at position 19
        // Field 2: 123 (positions 20..23)

        assert_eq!(find_field_byte_range(row, 0, Delimiter::Comma), Some(0..14));
        assert_eq!(
            find_field_byte_range(row, 1, Delimiter::Comma),
            Some(15..19)
        );
        assert_eq!(
            find_field_byte_range(row, 2, Delimiter::Comma),
            Some(20..23)
        );
    }

    #[test]
    fn test_find_field_byte_range_tab_delimiter() {
        let row = "a\tb\tc";

        assert_eq!(find_field_byte_range(row, 0, Delimiter::Tab), Some(0..1));
        assert_eq!(find_field_byte_range(row, 1, Delimiter::Tab), Some(2..3));
        assert_eq!(find_field_byte_range(row, 2, Delimiter::Tab), Some(4..5));
    }
}
