//! CSV mode update functions
//!
//! Handles CsvMsg messages for CSV view mode operations.

use crate::commands::Cmd;
use crate::csv::render::column_width_px;
use crate::csv::{
    detect_delimiter, escape_csv_value, parse_csv, CellEdit, CellEditState, CellPosition, CsvState,
    Delimiter,
};
use crate::editable::MoveTarget;
use crate::messages::CsvMsg;
use crate::model::{AppModel, ViewMode};
use crate::update::lsp::schedule_lsp_did_change;
use crate::update::syntax::schedule_syntax_parse;

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
        CsvMsg::ClickCell {
            row,
            col,
            x_in_cell,
            click_count,
            extend_selection,
        } => click_cell(model, row, col, x_in_cell, click_count, extend_selection),
        CsvMsg::ScrollVertical(delta) => scroll_vertical(model, delta),
        CsvMsg::ScrollHorizontal(delta) => scroll_horizontal(model, delta),

        // Cell editing messages
        CsvMsg::StartEditing => start_editing(model),
        CsvMsg::StartEditingWithChar(ch) => start_editing_with_char(model, ch),
        CsvMsg::ConfirmEdit => confirm_edit(model, 1), // Move down
        CsvMsg::ConfirmEditUp => confirm_edit(model, -1), // Move up
        CsvMsg::CancelEdit => cancel_edit(model),
        // In-cell editing: every arm is one `edit` op; the shared skeleton
        // re-fits the column width and caret scroll afterwards.
        CsvMsg::EditInsertChar(ch) => edit(model, |e| e.insert_char(ch)),
        CsvMsg::EditDeleteBackward => edit(model, |e| e.delete_backward()),
        CsvMsg::EditDeleteForward => edit(model, |e| e.delete_forward()),
        CsvMsg::EditDeleteWordBackward => edit(model, |e| e.delete_word_backward()),
        CsvMsg::EditDeleteWordForward => edit(model, |e| e.delete_word_forward()),
        CsvMsg::EditCursorLeft => edit(model, |e| e.move_cursor(MoveTarget::Left, false)),
        CsvMsg::EditCursorRight => edit(model, |e| e.move_cursor(MoveTarget::Right, false)),
        CsvMsg::EditCursorHome => edit(model, |e| e.move_cursor(MoveTarget::LineStart, false)),
        CsvMsg::EditCursorEnd => edit(model, |e| e.move_cursor(MoveTarget::LineEnd, false)),
        CsvMsg::EditCursorWordLeft => edit(model, |e| e.move_cursor(MoveTarget::WordLeft, false)),
        CsvMsg::EditCursorWordRight => edit(model, |e| e.move_cursor(MoveTarget::WordRight, false)),
        CsvMsg::EditCursorLeftWithSelection => {
            edit(model, |e| e.move_cursor(MoveTarget::Left, true))
        }
        CsvMsg::EditCursorRightWithSelection => {
            edit(model, |e| e.move_cursor(MoveTarget::Right, true))
        }
        CsvMsg::EditCursorHomeWithSelection => {
            edit(model, |e| e.move_cursor(MoveTarget::LineStart, true))
        }
        CsvMsg::EditCursorEndWithSelection => {
            edit(model, |e| e.move_cursor(MoveTarget::LineEnd, true))
        }
        CsvMsg::EditCursorWordLeftWithSelection => {
            edit(model, |e| e.move_cursor(MoveTarget::WordLeft, true))
        }
        CsvMsg::EditCursorWordRightWithSelection => {
            edit(model, |e| e.move_cursor(MoveTarget::WordRight, true))
        }
        CsvMsg::EditSelectAll => edit(model, |e| e.select_all()),
        CsvMsg::EditUndo => edit(model, |e| {
            e.undo();
        }),
        CsvMsg::EditRedo => edit(model, |e| {
            e.redo();
        }),
        CsvMsg::EditCopy => edit_with_cmd(model, |e| {
            let text = e.selected_text();
            (!text.is_empty()).then_some(Cmd::CopyToClipboard(text))
        }),
        CsvMsg::EditCut => edit_with_cmd(model, |e| {
            let text = e.selected_text();
            if text.is_empty() {
                return None;
            }
            e.delete_backward();
            Some(Cmd::CopyToClipboard(text))
        }),
        CsvMsg::EditPaste => Some(Cmd::RequestClipboardPaste),
        CsvMsg::EditPasteText(text) => edit_paste_text(model, text),
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
            // Rows are sized per group by `sync_all_viewports` below;
            // columns stay an approximation refined during render.
            csv_state.set_viewport_size(1, 10);

            // Need to get mutable reference again after the doc borrow is done
            if let Some(editor) = model.editor_area.editors.get_mut(&editor_id) {
                editor.view_mode = ViewMode::Csv(Box::new(csv_state));
            }
            model.resync_viewports();
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

/// Mouse press on a data cell. Mirrors the text editor's click-count
/// handling (`runtime/mouse.rs::handle_editor_content_click`) and
/// spreadsheet conventions:
///
/// - inside the cell being edited: place the caret at the pressed column
///   (Shift extends the selection); double-click selects the word, triple
///   selects all;
/// - on another cell while editing: commit the edit in place (clicking away
///   never discards), then treat the press as a fresh click on that cell;
/// - not editing: select; a double-click opens the editor with the caret at
///   the pressed column instead of the end.
fn click_cell(
    model: &mut AppModel,
    row: usize,
    col: usize,
    x_in_cell: f64,
    click_count: u8,
    extend_selection: bool,
) -> Option<Cmd> {
    use crate::csv::render::column_at_cell_x;

    let char_width = model.char_width;
    let clicked = CellPosition::new(row, col);

    let editing_clicked_cell = model
        .editor_area
        .focused_editor()?
        .view_mode
        .as_csv()?
        .editing
        .as_ref()
        .is_some_and(|edit| edit.position == clicked);
    if editing_clicked_cell {
        let csv = model
            .editor_area
            .focused_editor_mut()?
            .view_mode
            .as_csv_mut()?;
        let edit = csv.editing.as_mut()?;
        let column = column_at_cell_x(x_in_cell, edit.scroll_x, char_width)
            .min(edit.buffer().chars().count());
        match click_count {
            2 => {
                edit.set_cursor_column(column, false);
                edit.select_word();
            }
            3 => edit.select_all(),
            _ => edit.set_cursor_column(column, extend_selection),
        }
        update_edit_scroll(char_width, csv);
        return Some(Cmd::redraw_editor());
    }

    // Editing a different cell: commit it first (row_delta 0 = stay put),
    // keeping its document-sync commands.
    let mut cmds = Vec::new();
    let is_editing = model
        .editor_area
        .focused_editor()?
        .view_mode
        .as_csv()?
        .is_editing();
    if is_editing {
        match confirm_edit(model, 0) {
            Some(Cmd::Batch(sync)) => cmds.extend(sync),
            Some(cmd) => cmds.push(cmd),
            None => {}
        }
    }

    let csv = model
        .editor_area
        .focused_editor_mut()?
        .view_mode
        .as_csv_mut()?;
    csv.select_cell(row, col);
    if click_count >= 2 {
        let column =
            column_at_cell_x(x_in_cell, 0, char_width).min(csv.data.get(row, col).chars().count());
        csv.start_editing_at(column);
        update_edit_scroll(char_width, csv);
    }
    cmds.push(Cmd::redraw_editor());
    Some(Cmd::Batch(cmds))
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
    let csv = focused_csv(model)?;
    if csv.is_editing() {
        return edit(model, |e| e.insert_char(ch));
    }
    csv.start_editing_with_char(ch);
    Some(Cmd::redraw_editor())
}

/// Confirm edit and sync to document, then move in specified direction
fn confirm_edit(model: &mut AppModel, row_delta: i32) -> Option<Cmd> {
    let editor_id = model.editor_area.focused_group()?.active_editor_id()?;
    let editor = model.editor_area.editors.get_mut(&editor_id)?;

    let (edit, delimiter) = {
        let csv = editor.view_mode.as_csv_mut()?;
        let delimiter = csv.delimiter;
        (csv.confirm_edit(), delimiter)
    };

    let mut sync_cmds = Vec::new();
    if let Some(cell_edit) = edit {
        let doc_id = editor.document_id?;
        if let Some(doc) = model.editor_area.documents.get_mut(&doc_id) {
            sync_cell_edit_to_document(doc, &cell_edit, delimiter);
        }

        sync_cmds.extend(schedule_syntax_parse(model, doc_id));
        sync_cmds.extend(schedule_lsp_did_change(model, doc_id));

        // Keep current column width - already correctly sized from grow-only updates during editing
    }

    // Move in specified direction after confirming edit
    if let Some(editor) = model.editor_area.editors.get_mut(&editor_id) {
        if let Some(csv) = editor.view_mode.as_csv_mut() {
            csv.move_selection(row_delta, 0);
        }
    }

    sync_cmds.push(Cmd::redraw_editor());
    Some(Cmd::Batch(sync_cmds))
}

/// Cancel edit and discard changes
fn cancel_edit(model: &mut AppModel) -> Option<Cmd> {
    let editor = model.editor_area.focused_editor_mut()?;
    if let Some(csv) = editor.view_mode.as_csv_mut() {
        // Get original column width before canceling
        let (col, original_width) = if let Some(edit) = &csv.editing {
            (edit.position.col, edit.original_column_width)
        } else {
            return None;
        };

        csv.cancel_edit();

        // Restore original column width
        if let Some(width) = csv.column_widths.get_mut(col) {
            *width = original_width;
        }

        Some(Cmd::redraw_editor())
    } else {
        None
    }
}

// ===== Cell Editing Helpers =====

/// Update column width to fit current edit content (up to EDIT_MAX_WIDTH)
/// Uses grow-only logic: column can grow but never shrinks below existing width
fn update_column_width_for_edit(csv: &mut CsvState, content_len: usize) {
    const MIN_WIDTH: usize = 4;
    const EDIT_MAX_WIDTH: usize = 32;

    if let Some(edit) = &csv.editing {
        let col = edit.position.col;
        if let Some(width) = csv.column_widths.get_mut(col) {
            // Grow-only: max(new_content, current_width)
            let new_width = content_len.clamp(MIN_WIDTH, EDIT_MAX_WIDTH);
            *width = (*width).max(new_width);
        }
    }
}

/// Update horizontal scroll to keep cursor visible in cell editor
fn update_edit_scroll(char_width: f32, csv: &mut CsvState) {
    const EDIT_MAX_WIDTH: usize = 32;

    if let Some(edit) = &mut csv.editing {
        let cursor_col = edit.cursor_char_position();
        let col_width = csv
            .column_widths
            .get(edit.position.col)
            .copied()
            .unwrap_or(4);

        // If column is at max width, use scrolling
        if col_width >= EDIT_MAX_WIDTH {
            // Calculate visible characters in the cell
            let col_width_px = column_width_px(col_width, char_width);
            let padding = crate::csv::render::CELL_TEXT_PAD_X * 2;
            let visible_chars =
                ((col_width_px.saturating_sub(padding)) as f32 / char_width) as usize;

            // Calculate scroll offset to keep cursor visible (with 2-char margin)
            let margin = 2;
            edit.scroll_x = if cursor_col < edit.scroll_x + margin {
                // Cursor is too far left, scroll left
                cursor_col.saturating_sub(margin)
            } else if cursor_col >= edit.scroll_x + visible_chars.saturating_sub(margin) {
                // Cursor is too far right, scroll right
                cursor_col.saturating_sub(visible_chars.saturating_sub(margin + 1))
            } else {
                // Cursor is visible, keep current scroll
                edit.scroll_x
            };
        } else {
            // Column not at max, no scrolling needed
            edit.scroll_x = 0;
        }
    }
}

/// The focused editor's CSV state, if it is in CSV mode.
fn focused_csv(model: &mut AppModel) -> Option<&mut CsvState> {
    model
        .editor_area
        .focused_editor_mut()?
        .view_mode
        .as_csv_mut()
}

/// The shared skeleton of every in-cell edit message: apply `op` to the open
/// cell editor (a no-op when none is open), then re-fit the column width to
/// the content (grow-only) and the horizontal scroll to the caret. `op` may
/// return an extra command (clipboard) batched after the redraw.
fn edit_with_cmd(
    model: &mut AppModel,
    op: impl FnOnce(&mut CellEditState) -> Option<Cmd>,
) -> Option<Cmd> {
    let char_width = model.char_width;
    let csv = focused_csv(model)?;
    let extra = csv.editing.as_mut().and_then(op);
    if let Some(content_len) = csv.editing.as_ref().map(|e| e.buffer().chars().count()) {
        update_column_width_for_edit(csv, content_len);
    }
    update_edit_scroll(char_width, csv);
    Some(match extra {
        Some(cmd) => Cmd::Batch(vec![Cmd::redraw_editor(), cmd]),
        None => Cmd::redraw_editor(),
    })
}

/// [`edit_with_cmd`] for ops that only mutate the editor.
fn edit(model: &mut AppModel, op: impl FnOnce(&mut CellEditState)) -> Option<Cmd> {
    edit_with_cmd(model, |e| {
        op(e);
        None
    })
}

/// Paste given text into the cell (newlines stripped: single-line editor).
pub(crate) fn edit_paste_text(model: &mut AppModel, text: String) -> Option<Cmd> {
    let filtered: String = text.chars().filter(|c| *c != '\n' && *c != '\r').collect();
    edit(model, |e| e.insert_text(&filtered))
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

    let abs_start_byte = row_range.start + cell_range.start;
    let abs_end_byte = row_range.start + cell_range.end;

    // `find_row_byte_range`/`find_field_byte_range` operate on `str::char_indices`,
    // which yields byte offsets. `Rope::remove`/`Rope::insert` expect char offsets,
    // so convert here before touching the rope. Without this, any multi-byte UTF-8
    // content (accents, CJK, emoji) at or before the edited cell causes the wrong
    // range to be mutated, or a panic when the byte offset exceeds `len_chars()`.
    let abs_start = doc.buffer.byte_to_char(abs_start_byte);
    let abs_end = doc.buffer.byte_to_char(abs_end_byte);

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
    use crate::model::AppModel;

    #[test]
    fn csv_visible_rows_follow_the_groups_own_content_height() {
        use crate::messages::{LayoutMsg, Msg};
        use crate::model::SplitDirection;

        // Group rects are solved by the frame's layout pass; the test runs
        // that pass by hand after every structural change.
        fn relayout(model: &mut AppModel) {
            let rect = crate::layout::chrome::shell(model)
                .rect(crate::layout::UiKey::EditorArea)
                .unwrap();
            model
                .editor_area
                .compute_layout_scaled(rect, model.metrics.splitter_width);
            model.resync_viewports();
        }

        let mut model = AppModel::new(800, 600, 1.0, vec![]);
        model.document_mut().buffer = ropey::Rope::from("a,b\n1,2\n3,4\n");
        relayout(&mut model);
        toggle_csv_mode(&mut model);
        let full_height_rows = model
            .editor()
            .view_mode
            .as_csv()
            .expect("csv mode")
            .viewport
            .visible_rows;

        // A top/bottom split halves the group's content height; the CSV
        // viewport must follow the group, not the whole editor area.
        crate::update::update(
            &mut model,
            Msg::Layout(LayoutMsg::SplitFocused(SplitDirection::Vertical)),
        );
        relayout(&mut model);
        let csv_editor = model
            .editor_area
            .editors
            .values()
            .find_map(|e| e.view_mode.as_csv())
            .expect("csv editor survives the split");
        let group = model
            .editor_area
            .groups
            .values()
            .find(|g| {
                g.tabs.iter().any(|t| {
                    model
                        .editor_area
                        .editors
                        .get(&t.editor_id)
                        .is_some_and(|e| e.view_mode.is_csv())
                })
            })
            .expect("group hosting the csv editor");
        let content_height =
            (group.rect.height as usize).saturating_sub(model.metrics.tab_bar_height);
        let expected = crate::csv::rows_for_content_height(content_height, model.line_height);
        assert_eq!(csv_editor.viewport.visible_rows, expected);
        assert!(csv_editor.viewport.visible_rows < full_height_rows);
    }

    /// `confirm_edit` mutates `doc.buffer`/`doc.revision` directly (like
    /// Replace All), so it must schedule syntax-parse and LSP didChange
    /// itself rather than relying on the generic editor-mutation path.
    #[test]
    fn confirm_edit_schedules_syntax_parse_and_lsp_did_change() {
        let mut model = AppModel::new(80, 60, 1.0, vec![]);
        model.document_mut().buffer = ropey::Rope::from_str("a,b\n1,2\n");

        update_csv(&mut model, CsvMsg::Toggle).expect("csv toggle should produce a redraw cmd");
        assert!(model.editor().view_mode.is_csv(), "expected CSV mode");

        update_csv(&mut model, CsvMsg::StartEditing);
        let before_revision = model.document().revision;
        update_csv(&mut model, CsvMsg::EditInsertChar('x'));
        let cmd = update_csv(&mut model, CsvMsg::ConfirmEdit);

        assert!(model.document().revision > before_revision);

        let cmds = match cmd {
            Some(Cmd::Batch(cmds)) => cmds,
            other => panic!("expected a batch of sync commands, got {other:?}"),
        };
        assert!(
            cmds.iter()
                .any(|c| matches!(c, Cmd::LspScheduleDidChange { .. })),
            "expected LspScheduleDidChange in {cmds:?}"
        );
    }

    fn csv_model() -> AppModel {
        let mut model = AppModel::new(800, 600, 1.0, vec![]);
        model.char_width = 8.0;
        model.document_mut().buffer = ropey::Rope::from_str("hello world,b\nfoo,bar\n");
        update_csv(&mut model, CsvMsg::Toggle).expect("csv toggle");
        model
    }

    fn csv(model: &AppModel) -> &CsvState {
        model.editor().view_mode.as_csv().expect("csv mode")
    }

    /// Press `column` chars into cell (row, col) at scroll 0.
    fn click(model: &mut AppModel, row: usize, col: usize, column: usize, count: u8, shift: bool) {
        use crate::csv::render::CELL_TEXT_PAD_X;
        let x_in_cell = CELL_TEXT_PAD_X as f64 + column as f64 * model.char_width as f64;
        update_csv(
            model,
            CsvMsg::ClickCell {
                row,
                col,
                x_in_cell,
                click_count: count,
                extend_selection: shift,
            },
        );
    }

    #[test]
    fn single_click_places_the_caret_in_the_edited_cell_without_committing() {
        let mut model = csv_model();
        update_csv(&mut model, CsvMsg::StartEditing); // cell (0,0) "hello world", caret at end
        update_csv(&mut model, CsvMsg::EditInsertChar('!'));

        click(&mut model, 0, 0, 3, 1, false);

        let edit = csv(&model).editing.as_ref().expect("still editing");
        assert_eq!(edit.cursor_char_position(), 3);
        assert_eq!(edit.buffer(), "hello world!", "no commit, no reset");
        assert!(!edit.editable.has_selection());
    }

    #[test]
    fn shift_click_extends_the_selection_from_the_caret() {
        let mut model = csv_model();
        update_csv(&mut model, CsvMsg::StartEditing);
        click(&mut model, 0, 0, 2, 1, false);
        click(&mut model, 0, 0, 7, 1, true);

        let edit = csv(&model).editing.as_ref().unwrap();
        assert_eq!(edit.editable.selected_text(), "llo w");
    }

    #[test]
    fn clicking_another_cell_while_editing_commits_and_selects_it() {
        let mut model = csv_model();
        update_csv(&mut model, CsvMsg::StartEditing);
        update_csv(&mut model, CsvMsg::EditInsertChar('!'));
        let before_revision = model.document().revision;

        click(&mut model, 1, 1, 0, 1, false);

        let state = csv(&model);
        assert!(state.editing.is_none(), "click-away commits");
        assert_eq!(state.data.get(0, 0), "hello world!");
        assert_eq!(state.selected_cell, CellPosition::new(1, 1));
        assert!(
            model.document().revision > before_revision,
            "document synced"
        );
    }

    #[test]
    fn double_click_starts_editing_at_the_pressed_column() {
        let mut model = csv_model();
        click(&mut model, 0, 0, 1, 1, false);
        assert!(csv(&model).editing.is_none());

        click(&mut model, 0, 0, 4, 2, false);

        let edit = csv(&model).editing.as_ref().expect("double-click edits");
        assert_eq!(edit.position, CellPosition::new(0, 0));
        assert_eq!(edit.cursor_char_position(), 4);
        assert_eq!(edit.buffer(), "hello world");
    }

    #[test]
    fn double_click_while_editing_selects_the_word_and_triple_selects_all() {
        let mut model = csv_model();
        update_csv(&mut model, CsvMsg::StartEditing);

        click(&mut model, 0, 0, 7, 2, false);
        assert_eq!(
            csv(&model)
                .editing
                .as_ref()
                .unwrap()
                .editable
                .selected_text(),
            "world"
        );

        click(&mut model, 0, 0, 7, 3, false);
        assert_eq!(
            csv(&model)
                .editing
                .as_ref()
                .unwrap()
                .editable
                .selected_text(),
            "hello world"
        );
    }

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

    /// Regression test: `find_row_byte_range`/`find_field_byte_range` return BYTE
    /// offsets (from `str::char_indices`), but `Rope::remove`/`Rope::insert` expect
    /// CHAR offsets. When a row before the edited cell contains multi-byte UTF-8
    /// (like "café", where 'é' is 2 bytes but 1 char), byte offsets and char offsets
    /// diverge, and passing raw byte offsets into the rope mutates the wrong range.
    ///
    /// Before the fix, editing the "age" field of the "bob" row below would corrupt
    /// the buffer (e.g. deleting a wrong char and swallowing the following newline)
    /// instead of producing "name,age\ncafé,30\nbob,26".
    #[test]
    fn test_sync_cell_edit_to_document_multibyte_row_before_edit() {
        let content = "name,age\ncafé,30\nbob,25";
        let mut doc = Document::with_text(content);

        let edit = CellEdit {
            position: CellPosition::new(2, 1),
            old_value: "25".to_string(),
            new_value: "26".to_string(),
        };

        sync_cell_edit_to_document(&mut doc, &edit, Delimiter::Comma);

        assert_eq!(doc.buffer.to_string(), "name,age\ncafé,30\nbob,26");
        assert!(doc.is_modified);
    }

    /// Same hazard as above, but the edit targets a cell in the very row that
    /// contains the multi-byte character, exercising the field-range conversion
    /// as well as the row-range conversion.
    #[test]
    fn test_sync_cell_edit_to_document_multibyte_in_edited_row() {
        let content = "name,age\ncafé,30\nbob,25";
        let mut doc = Document::with_text(content);

        let edit = CellEdit {
            position: CellPosition::new(1, 1),
            old_value: "30".to_string(),
            new_value: "31".to_string(),
        };

        sync_cell_edit_to_document(&mut doc, &edit, Delimiter::Comma);

        assert_eq!(doc.buffer.to_string(), "name,age\ncafé,31\nbob,25");
        assert!(doc.is_modified);
    }
}
