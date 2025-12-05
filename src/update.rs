//! Update functions for the Elm-style architecture
//!
//! All state transformations flow through these functions.

use std::time::Duration;

use crate::commands::Cmd;
use crate::messages::{AppMsg, Direction, DocumentMsg, EditorMsg, Msg, UiMsg};
use crate::model::{AppModel, Cursor, EditOperation, Position, Selection};
use crate::util::char_type;

use crate::model::sync_status_bar;

/// Main update function - dispatches to sub-handlers
pub fn update(model: &mut AppModel, msg: Msg) -> Option<Cmd> {
    let result = match msg {
        Msg::Editor(m) => update_editor(model, m),
        Msg::Document(m) => update_document(model, m),
        Msg::Ui(m) => update_ui(model, m),
        Msg::App(m) => update_app(model, m),
    };

    // Sync status bar segments after state changes
    sync_status_bar(model);

    result
}

/// Handle editor messages (cursor movement, viewport scrolling)
pub fn update_editor(model: &mut AppModel, msg: EditorMsg) -> Option<Cmd> {
    match msg {
        EditorMsg::MoveCursor(direction) => {
            match direction {
                Direction::Up => move_cursor_up(model),
                Direction::Down => move_cursor_down(model),
                Direction::Left => move_cursor_left(model),
                Direction::Right => move_cursor_right(model),
            }
            // Use direction-aware reveal: up reveals at top, down at bottom
            let vertical_hint = match direction {
                Direction::Up => Some(true),
                Direction::Down => Some(false),
                _ => None,
            };
            model.ensure_cursor_visible_directional(vertical_hint);
            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        EditorMsg::MoveCursorLineStart => {
            let first_non_ws = model.first_non_whitespace_column();
            if model.editor.cursor_mut().column == first_non_ws {
                model.editor.cursor_mut().column = 0;
            } else {
                model.editor.cursor_mut().column = first_non_ws;
            }
            model.editor.cursor_mut().desired_column = None;
            model.ensure_cursor_visible();
            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        EditorMsg::MoveCursorLineEnd => {
            let line_end = model.current_line_length();
            let last_non_ws = model.last_non_whitespace_column();
            if model.editor.cursor_mut().column == last_non_ws {
                model.editor.cursor_mut().column = line_end;
            } else {
                model.editor.cursor_mut().column = last_non_ws;
            }
            model.editor.cursor_mut().desired_column = None;
            model.ensure_cursor_visible();
            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        EditorMsg::MoveCursorDocumentStart => {
            model.editor.cursor_mut().line = 0;
            model.editor.cursor_mut().column = 0;
            model.editor.cursor_mut().desired_column = None;
            model.editor.viewport.top_line = 0;
            model.ensure_cursor_visible();
            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        EditorMsg::MoveCursorDocumentEnd => {
            model.editor.cursor_mut().line = model.document.line_count().saturating_sub(1);
            model.editor.cursor_mut().column = model.current_line_length();
            model.editor.cursor_mut().desired_column = None;
            if model.editor.cursor_mut().line >= model.editor.viewport.visible_lines {
                model.editor.viewport.top_line =
                    model.editor.cursor_mut().line - model.editor.viewport.visible_lines + 1;
            }
            model.ensure_cursor_visible();
            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        EditorMsg::MoveCursorWord(direction) => {
            match direction {
                Direction::Left => move_cursor_word_left(model),
                Direction::Right => move_cursor_word_right(model),
                _ => {} // Up/Down not used for word movement
            }
            model.ensure_cursor_visible();
            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        EditorMsg::PageUp => {
            let jump = model.editor.viewport.visible_lines.saturating_sub(2);
            model.editor.cursor_mut().line = model.editor.cursor_mut().line.saturating_sub(jump);

            let desired = model
                .editor
                .cursor()
                .desired_column
                .unwrap_or(model.editor.cursor_mut().column);
            let line_len = model.current_line_length();
            model.editor.cursor_mut().column = desired.min(line_len);
            model.editor.cursor_mut().desired_column = Some(desired);

            model.editor.viewport.top_line = model.editor.viewport.top_line.saturating_sub(jump);
            // Use top-aligned reveal for upward page movement
            model.ensure_cursor_visible_directional(Some(true));
            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        EditorMsg::PageDown => {
            let jump = model.editor.viewport.visible_lines.saturating_sub(2);
            let max_line = model.document.line_count().saturating_sub(1);
            model.editor.cursor_mut().line = (model.editor.cursor_mut().line + jump).min(max_line);

            let desired = model
                .editor
                .cursor()
                .desired_column
                .unwrap_or(model.editor.cursor_mut().column);
            let line_len = model.current_line_length();
            model.editor.cursor_mut().column = desired.min(line_len);
            model.editor.cursor_mut().desired_column = Some(desired);

            if model.editor.cursor_mut().line
                >= model.editor.viewport.top_line + model.editor.viewport.visible_lines
            {
                model.editor.viewport.top_line = model
                    .editor
                    .cursor()
                    .line
                    .saturating_sub(model.editor.viewport.visible_lines - 1);
            }
            // Use bottom-aligned reveal for downward page movement
            model.ensure_cursor_visible_directional(Some(false));
            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        EditorMsg::SetCursorPosition { line, column } => {
            model.editor.cursor_mut().line = line;
            model.editor.cursor_mut().column = column;
            model.editor.cursor_mut().desired_column = None;
            model.ensure_cursor_visible();
            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        EditorMsg::Scroll(delta) => {
            let total_lines = model.document.line_count();
            if total_lines <= model.editor.viewport.visible_lines {
                return None;
            }

            let max_top = total_lines.saturating_sub(model.editor.viewport.visible_lines);

            if delta > 0 {
                model.editor.viewport.top_line =
                    (model.editor.viewport.top_line + delta as usize).min(max_top);
            } else if delta < 0 {
                model.editor.viewport.top_line = model
                    .editor
                    .viewport
                    .top_line
                    .saturating_sub(delta.unsigned_abs() as usize);
            }

            Some(Cmd::Redraw)
        }

        EditorMsg::ScrollHorizontal(delta) => {
            let max_line_len = (model.editor.viewport.top_line
                ..model.editor.viewport.top_line + model.editor.viewport.visible_lines)
                .filter_map(|i| {
                    if i < model.document.line_count() {
                        Some(model.document.line_length(i))
                    } else {
                        None
                    }
                })
                .max()
                .unwrap_or(0);

            if max_line_len <= model.editor.viewport.visible_columns {
                model.editor.viewport.left_column = 0;
                return None;
            }

            let max_left = max_line_len.saturating_sub(model.editor.viewport.visible_columns);

            if delta > 0 {
                model.editor.viewport.left_column =
                    (model.editor.viewport.left_column + delta as usize).min(max_left);
            } else if delta < 0 {
                model.editor.viewport.left_column = model
                    .editor
                    .viewport
                    .left_column
                    .saturating_sub(delta.unsigned_abs() as usize);
            }

            Some(Cmd::Redraw)
        }

        // === Selection Movement (Shift+key) ===
        EditorMsg::MoveCursorWithSelection(direction) => {
            // If selection is empty, anchor starts at current cursor
            if model.editor.selection().is_empty() {
                let pos = model.editor.cursor().to_position();
                model.editor.selection_mut().anchor = pos;
            }

            // Move the cursor
            match direction {
                Direction::Up => move_cursor_up(model),
                Direction::Down => move_cursor_down(model),
                Direction::Left => move_cursor_left(model),
                Direction::Right => move_cursor_right(model),
            }

            // Update head to new cursor position
            model.editor.selection_mut().head = model.editor.cursor().to_position();
            // Use direction-aware reveal for consistency with regular movement
            let vertical_hint = match direction {
                Direction::Up => Some(true),
                Direction::Down => Some(false),
                _ => None,
            };
            model.ensure_cursor_visible_directional(vertical_hint);
            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        EditorMsg::MoveCursorLineStartWithSelection => {
            if model.editor.selection().is_empty() {
                let pos = model.editor.cursor().to_position();
                model.editor.selection_mut().anchor = pos;
            }

            let first_non_ws = model.first_non_whitespace_column();
            if model.editor.cursor().column == first_non_ws {
                model.editor.cursor_mut().column = 0;
            } else {
                model.editor.cursor_mut().column = first_non_ws;
            }
            model.editor.cursor_mut().desired_column = None;
            model.ensure_cursor_visible();

            model.editor.selection_mut().head = model.editor.cursor().to_position();
            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        EditorMsg::MoveCursorLineEndWithSelection => {
            if model.editor.selection().is_empty() {
                let pos = model.editor.cursor().to_position();
                model.editor.selection_mut().anchor = pos;
            }

            let line_end = model.current_line_length();
            let last_non_ws = model.last_non_whitespace_column();
            if model.editor.cursor().column == last_non_ws {
                model.editor.cursor_mut().column = line_end;
            } else {
                model.editor.cursor_mut().column = last_non_ws;
            }
            model.editor.cursor_mut().desired_column = None;
            model.ensure_cursor_visible();

            model.editor.selection_mut().head = model.editor.cursor().to_position();
            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        EditorMsg::MoveCursorDocumentStartWithSelection => {
            if model.editor.selection().is_empty() {
                let pos = model.editor.cursor().to_position();
                model.editor.selection_mut().anchor = pos;
            }

            model.editor.cursor_mut().line = 0;
            model.editor.cursor_mut().column = 0;
            model.editor.cursor_mut().desired_column = None;
            model.editor.viewport.top_line = 0;
            model.ensure_cursor_visible();

            model.editor.selection_mut().head = model.editor.cursor().to_position();
            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        EditorMsg::MoveCursorDocumentEndWithSelection => {
            if model.editor.selection().is_empty() {
                let pos = model.editor.cursor().to_position();
                model.editor.selection_mut().anchor = pos;
            }

            model.editor.cursor_mut().line = model.document.line_count().saturating_sub(1);
            model.editor.cursor_mut().column = model.current_line_length();
            model.editor.cursor_mut().desired_column = None;
            if model.editor.cursor().line >= model.editor.viewport.visible_lines {
                model.editor.viewport.top_line =
                    model.editor.cursor().line - model.editor.viewport.visible_lines + 1;
            }
            model.ensure_cursor_visible();

            model.editor.selection_mut().head = model.editor.cursor().to_position();
            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        EditorMsg::MoveCursorWordWithSelection(direction) => {
            if model.editor.selection().is_empty() {
                let pos = model.editor.cursor().to_position();
                model.editor.selection_mut().anchor = pos;
            }

            match direction {
                Direction::Left => move_cursor_word_left(model),
                Direction::Right => move_cursor_word_right(model),
                _ => {} // Up/Down not used for word movement
            }
            model.ensure_cursor_visible();

            model.editor.selection_mut().head = model.editor.cursor().to_position();
            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        EditorMsg::PageUpWithSelection => {
            if model.editor.selection().is_empty() {
                let pos = model.editor.cursor().to_position();
                model.editor.selection_mut().anchor = pos;
            }

            let jump = model.editor.viewport.visible_lines.saturating_sub(2);
            model.editor.cursor_mut().line = model.editor.cursor().line.saturating_sub(jump);

            let desired = model
                .editor
                .cursor()
                .desired_column
                .unwrap_or(model.editor.cursor().column);
            let line_len = model.current_line_length();
            model.editor.cursor_mut().column = desired.min(line_len);
            model.editor.cursor_mut().desired_column = Some(desired);

            model.editor.viewport.top_line = model.editor.viewport.top_line.saturating_sub(jump);
            model.ensure_cursor_visible();

            model.editor.selection_mut().head = model.editor.cursor().to_position();
            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        EditorMsg::PageDownWithSelection => {
            if model.editor.selection().is_empty() {
                let pos = model.editor.cursor().to_position();
                model.editor.selection_mut().anchor = pos;
            }

            let jump = model.editor.viewport.visible_lines.saturating_sub(2);
            let max_line = model.document.line_count().saturating_sub(1);
            model.editor.cursor_mut().line = (model.editor.cursor().line + jump).min(max_line);

            let desired = model
                .editor
                .cursor()
                .desired_column
                .unwrap_or(model.editor.cursor().column);
            let line_len = model.current_line_length();
            model.editor.cursor_mut().column = desired.min(line_len);
            model.editor.cursor_mut().desired_column = Some(desired);

            if model.editor.cursor().line
                >= model.editor.viewport.top_line + model.editor.viewport.visible_lines
            {
                model.editor.viewport.top_line = model
                    .editor
                    .cursor()
                    .line
                    .saturating_sub(model.editor.viewport.visible_lines - 1);
            }
            model.ensure_cursor_visible();

            model.editor.selection_mut().head = model.editor.cursor().to_position();
            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        // === Selection Commands ===
        EditorMsg::SelectAll => {
            use crate::model::editor::Position;

            model.editor.selection_mut().anchor = Position::new(0, 0);
            model.editor.cursor_mut().line = model.document.line_count().saturating_sub(1);
            model.editor.cursor_mut().column = model.current_line_length();
            model.editor.selection_mut().head = model.editor.cursor().to_position();
            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        EditorMsg::SelectWord => {
            use crate::model::editor::Position;

            let line = model.editor.cursor().line;
            let column = model.editor.cursor().column;

            // Get the current line text
            if let Some(line_text) = model.document.get_line(line) {
                let chars: Vec<char> = line_text.chars().collect();

                if column >= chars.len() || chars.is_empty() {
                    return Some(Cmd::Redraw);
                }

                let current_char = chars[column];
                let current_type = char_type(current_char);

                // Find word start - scan backwards
                let mut start_col = column;
                while start_col > 0 && char_type(chars[start_col - 1]) == current_type {
                    start_col -= 1;
                }

                // Find word end - scan forwards
                let mut end_col = column;
                while end_col < chars.len() && char_type(chars[end_col]) == current_type {
                    end_col += 1;
                }

                // Don't include trailing newline
                if end_col > 0 && chars.get(end_col - 1) == Some(&'\n') {
                    end_col -= 1;
                }

                // Set selection from start to end
                model.editor.selection_mut().anchor = Position::new(line, start_col);
                model.editor.selection_mut().head = Position::new(line, end_col);
                model.editor.cursor_mut().column = end_col;
                model.editor.cursor_mut().desired_column = None;
            }

            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        EditorMsg::SelectLine => {
            use crate::model::editor::Position;

            let line = model.editor.cursor().line;
            let line_len = model.document.line_length(line);
            let total_lines = model.document.line_count();

            // Select from start of current line to start of next line (or end of document)
            model.editor.selection_mut().anchor = Position::new(line, 0);

            if line + 1 < total_lines {
                // Select to start of next line (includes the newline)
                model.editor.selection_mut().head = Position::new(line + 1, 0);
                model.editor.cursor_mut().line = line + 1;
                model.editor.cursor_mut().column = 0;
            } else {
                // Last line - select to end of line
                model.editor.selection_mut().head = Position::new(line, line_len);
                model.editor.cursor_mut().column = line_len;
            }
            model.editor.cursor_mut().desired_column = None;

            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        EditorMsg::ExtendSelectionToPosition { line, column } => {
            use crate::model::editor::Position;

            // If selection is empty, anchor at current cursor
            if model.editor.selection().is_empty() {
                let pos = model.editor.cursor().to_position();
                model.editor.selection_mut().anchor = pos;
            }

            // Move cursor to target position
            model.editor.cursor_mut().line = line;
            model.editor.cursor_mut().column = column;
            model.editor.cursor_mut().desired_column = None;

            // Update head
            model.editor.selection_mut().head = Position::new(line, column);
            model.ensure_cursor_visible();
            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        EditorMsg::ClearSelection => {
            model.editor.clear_selection();
            Some(Cmd::Redraw)
        }

        // === Multi-Cursor ===
        EditorMsg::ToggleCursorAtPosition { line, column } => {
            model.editor.toggle_cursor_at(line, column);
            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        EditorMsg::AddCursorAbove => {
            // For each existing cursor, try to add one on the line above
            let new_positions: Vec<_> = model
                .editor
                .cursors
                .iter()
                .filter(|c| c.line > 0)
                .map(|c| {
                    let target_line = c.line - 1;
                    let target_col = c.desired_column.unwrap_or(c.column);
                    let line_len = model.document.line_length(target_line);
                    (target_line, target_col.min(line_len))
                })
                .collect();

            for (line, col) in new_positions {
                model.editor.add_cursor_at(line, col);
            }
            model.editor.deduplicate_cursors();
            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        EditorMsg::AddCursorBelow => {
            // For each existing cursor, try to add one on the line below
            let total_lines = model.document.line_count();
            let new_positions: Vec<_> = model
                .editor
                .cursors
                .iter()
                .filter(|c| c.line + 1 < total_lines)
                .map(|c| {
                    let target_line = c.line + 1;
                    let target_col = c.desired_column.unwrap_or(c.column);
                    let line_len = model.document.line_length(target_line);
                    (target_line, target_col.min(line_len))
                })
                .collect();

            for (line, col) in new_positions {
                model.editor.add_cursor_at(line, col);
            }
            model.editor.deduplicate_cursors();
            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        EditorMsg::CollapseToSingleCursor => {
            model.editor.collapse_to_primary();
            model.editor.clear_selection();
            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        EditorMsg::RemoveCursor(_index) => {
            // TODO: Implement remove specific cursor (Phase 4)
            Some(Cmd::Redraw)
        }

        // === Find & Select ===
        EditorMsg::SelectNextOccurrence => {
            // TODO: Implement select next occurrence (Phase 9)
            Some(Cmd::Redraw)
        }

        EditorMsg::SelectAllOccurrences => {
            // TODO: Implement select all occurrences (Phase 9)
            Some(Cmd::Redraw)
        }

        // === Rectangle Selection ===
        EditorMsg::StartRectangleSelection { line, column } => {
            model.editor.rectangle_selection.active = true;
            model.editor.rectangle_selection.start = Position::new(line, column);
            model.editor.rectangle_selection.current = Position::new(line, column);
            Some(Cmd::Redraw)
        }

        EditorMsg::UpdateRectangleSelection { line, column } => {
            if model.editor.rectangle_selection.active {
                model.editor.rectangle_selection.current = Position::new(line, column);

                // Compute preview cursor positions
                let top_left = model.editor.rectangle_selection.top_left();
                let bottom_right = model.editor.rectangle_selection.bottom_right();
                let cursor_col = model.editor.rectangle_selection.current.column;

                model.editor.rectangle_selection.preview_cursors.clear();
                for preview_line in top_left.line..=bottom_right.line {
                    model
                        .editor
                        .rectangle_selection
                        .preview_cursors
                        .push(Position::new(preview_line, cursor_col));
                }
            }
            Some(Cmd::Redraw)
        }

        EditorMsg::FinishRectangleSelection => {
            if !model.editor.rectangle_selection.active {
                return Some(Cmd::Redraw);
            }

            let top_left = model.editor.rectangle_selection.top_left();
            let bottom_right = model.editor.rectangle_selection.bottom_right();
            // The cursor should be at the "current" position (where user dragged TO)
            let cursor_col = model.editor.rectangle_selection.current.column;

            // Clear existing cursors and selections
            model.editor.cursors.clear();
            model.editor.selections.clear();

            // Create a cursor (and optionally selection) for each line in the rectangle
            for line in top_left.line..=bottom_right.line {
                let line_len = model.document.line_length(line);

                // Clamp columns to line length
                let start_col = top_left.column.min(line_len);
                let end_col = bottom_right.column.min(line_len);
                let clamped_cursor_col = cursor_col.min(line_len);

                // Create cursor at the dragged-to position (clamped to line length)
                let cursor = Cursor::at(line, clamped_cursor_col);
                model.editor.cursors.push(cursor);

                // Create selection if rectangle has width
                if start_col < end_col {
                    // Anchor is the opposite end from cursor, head is at cursor
                    let anchor_col = if cursor_col == start_col { end_col } else { start_col };
                    let selection = Selection {
                        anchor: Position::new(line, anchor_col.min(line_len)),
                        head: Position::new(line, clamped_cursor_col),
                    };
                    model.editor.selections.push(selection);
                } else {
                    // Zero-width: just cursor, no selection
                    let selection = Selection::new(Position::new(line, clamped_cursor_col));
                    model.editor.selections.push(selection);
                }
            }

            // Deactivate rectangle selection mode and clear preview
            model.editor.rectangle_selection.active = false;
            model.editor.rectangle_selection.preview_cursors.clear();

            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        EditorMsg::CancelRectangleSelection => {
            model.editor.rectangle_selection.active = false;
            model.editor.rectangle_selection.preview_cursors.clear();
            Some(Cmd::Redraw)
        }
    }
}

/// Delete the current selection and return (start_offset, deleted_text)
/// Returns None if selection is empty
fn delete_selection(model: &mut AppModel) -> Option<(usize, String)> {
    let selection = model.editor.selection();
    if selection.is_empty() {
        return None;
    }

    let sel_start = selection.start();
    let sel_end = selection.end();

    // Convert positions to buffer offsets
    let start_offset = model.document.cursor_to_offset(sel_start.line, sel_start.column);
    let end_offset = model.document.cursor_to_offset(sel_end.line, sel_end.column);

    // Get the text being deleted
    let deleted_text: String = model
        .document
        .buffer
        .slice(start_offset..end_offset)
        .chars()
        .collect();

    // Delete the range
    model.document.buffer.remove(start_offset..end_offset);

    // Move cursor to selection start
    model.editor.cursor_mut().line = sel_start.line;
    model.editor.cursor_mut().column = sel_start.column;
    model.editor.cursor_mut().desired_column = None;

    // Clear the selection
    model.editor.clear_selection();

    Some((start_offset, deleted_text))
}

/// Get cursor indices sorted by position in reverse document order (last first)
fn cursors_in_reverse_order(model: &AppModel) -> Vec<usize> {
    let mut indices: Vec<usize> = (0..model.editor.cursors.len()).collect();
    indices.sort_by(|&a, &b| {
        let ca = &model.editor.cursors[a];
        let cb = &model.editor.cursors[b];
        // Sort descending: higher line first, then higher column
        cb.line.cmp(&ca.line).then_with(|| cb.column.cmp(&ca.column))
    });
    indices
}

/// Handle document messages (text editing, undo/redo)
pub fn update_document(model: &mut AppModel, msg: DocumentMsg) -> Option<Cmd> {
    match msg {
        DocumentMsg::InsertChar(ch) => {
            let cursor_before = *model.editor.cursor();

            // Multi-cursor: process all cursors in reverse document order
            if model.editor.has_multiple_cursors() {
                let indices = cursors_in_reverse_order(model);

                for idx in indices {
                    // Get cursor position and convert to buffer offset
                    let cursor = &model.editor.cursors[idx];
                    let pos = model.document.cursor_to_offset(cursor.line, cursor.column);

                    // Insert character
                    model.document.buffer.insert_char(pos, ch);

                    // Update this cursor's position (move right by 1)
                    model.editor.cursors[idx].column += 1;
                    model.editor.cursors[idx].desired_column = None;

                    // Clear this cursor's selection
                    let new_pos = model.editor.cursors[idx].to_position();
                    model.editor.selections[idx] = Selection::new(new_pos);
                }

                // Record single edit for undo (simplified - full undo would need batch)
                model.document.push_edit(EditOperation::Insert {
                    position: model.cursor_buffer_position().saturating_sub(1),
                    text: ch.to_string(),
                    cursor_before,
                    cursor_after: *model.editor.cursor(),
                });

                model.document.is_modified = true;
                model.ensure_cursor_visible();
                model.reset_cursor_blink();
                return Some(Cmd::Redraw);
            }

            // Single cursor: existing behavior
            // If there's a selection, delete it first
            if let Some((pos, deleted_text)) = delete_selection(model) {
                // Insert at selection start
                model.document.buffer.insert_char(pos, ch);
                model.set_cursor_from_position(pos + 1);
                model.ensure_cursor_visible();

                // Record as delete + insert (for undo, we need to restore the deleted text)
                model.document.push_edit(EditOperation::Delete {
                    position: pos,
                    text: deleted_text,
                    cursor_before,
                    cursor_after: *model.editor.cursor(),
                });
                model.document.push_edit(EditOperation::Insert {
                    position: pos,
                    text: ch.to_string(),
                    cursor_before: *model.editor.cursor(), // After delete
                    cursor_after: *model.editor.cursor(),
                });
            } else {
                // No selection - normal insert
                let pos = model.cursor_buffer_position();
                model.document.buffer.insert_char(pos, ch);
                model.set_cursor_from_position(pos + 1);
                model.ensure_cursor_visible();

                model.document.push_edit(EditOperation::Insert {
                    position: pos,
                    text: ch.to_string(),
                    cursor_before,
                    cursor_after: *model.editor.cursor(),
                });
            }

            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        DocumentMsg::InsertNewline => {
            let cursor_before = *model.editor.cursor();

            // Multi-cursor: process all cursors in reverse document order
            if model.editor.has_multiple_cursors() {
                let indices = cursors_in_reverse_order(model);

                for idx in indices {
                    // Get cursor position and convert to buffer offset
                    let cursor = &model.editor.cursors[idx];
                    let pos = model.document.cursor_to_offset(cursor.line, cursor.column);

                    // Insert newline
                    model.document.buffer.insert_char(pos, '\n');

                    // Update this cursor's position (move to start of next line)
                    model.editor.cursors[idx].line += 1;
                    model.editor.cursors[idx].column = 0;
                    model.editor.cursors[idx].desired_column = None;

                    // Clear this cursor's selection
                    let new_pos = model.editor.cursors[idx].to_position();
                    model.editor.selections[idx] = Selection::new(new_pos);
                }

                // Record single edit for undo (simplified)
                model.document.push_edit(EditOperation::Insert {
                    position: model.cursor_buffer_position().saturating_sub(1),
                    text: "\n".to_string(),
                    cursor_before,
                    cursor_after: *model.editor.cursor(),
                });

                model.document.is_modified = true;
                model.ensure_cursor_visible();
                model.reset_cursor_blink();
                return Some(Cmd::Redraw);
            }

            // Single cursor: existing behavior
            // If there's a selection, delete it first
            if let Some((pos, deleted_text)) = delete_selection(model) {
                // Insert newline at selection start
                model.document.buffer.insert_char(pos, '\n');
                model.editor.cursor_mut().line += 1;
                model.editor.cursor_mut().column = 0;
                model.editor.cursor_mut().desired_column = None;
                model.ensure_cursor_visible();

                // Record delete + insert
                model.document.push_edit(EditOperation::Delete {
                    position: pos,
                    text: deleted_text,
                    cursor_before,
                    cursor_after: *model.editor.cursor(),
                });
                model.document.push_edit(EditOperation::Insert {
                    position: pos,
                    text: "\n".to_string(),
                    cursor_before: *model.editor.cursor(),
                    cursor_after: *model.editor.cursor(),
                });
            } else {
                // No selection - normal insert
                let pos = model.cursor_buffer_position();
                model.document.buffer.insert_char(pos, '\n');
                model.editor.cursor_mut().line += 1;
                model.editor.cursor_mut().column = 0;
                model.editor.cursor_mut().desired_column = None;
                model.ensure_cursor_visible();

                model.document.push_edit(EditOperation::Insert {
                    position: pos,
                    text: "\n".to_string(),
                    cursor_before,
                    cursor_after: *model.editor.cursor(),
                });
            }

            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        DocumentMsg::DeleteBackward => {
            let cursor_before = *model.editor.cursor();

            // Multi-cursor: process all cursors in reverse document order
            if model.editor.has_multiple_cursors() {
                let indices = cursors_in_reverse_order(model);

                for idx in indices {
                    let cursor = &model.editor.cursors[idx];
                    let pos = model.document.cursor_to_offset(cursor.line, cursor.column);

                    if cursor.column > 0 {
                        // Delete character before cursor
                        model.document.buffer.remove(pos - 1..pos);
                        model.editor.cursors[idx].column -= 1;
                    } else if cursor.line > 0 {
                        // Join with previous line
                        let prev_line_idx = cursor.line - 1;
                        let prev_line = model.document.buffer.line(prev_line_idx);
                        let join_column = prev_line.len_chars().saturating_sub(
                            if prev_line.chars().last() == Some('\n') { 1 } else { 0 },
                        );

                        model.document.buffer.remove(pos - 1..pos);
                        model.editor.cursors[idx].line -= 1;
                        model.editor.cursors[idx].column = join_column;
                    }

                    model.editor.cursors[idx].desired_column = None;

                    // Clear this cursor's selection
                    let new_pos = model.editor.cursors[idx].to_position();
                    model.editor.selections[idx] = Selection::new(new_pos);
                }

                model.document.is_modified = true;
                model.ensure_cursor_visible();
                model.reset_cursor_blink();
                return Some(Cmd::Redraw);
            }

            // Single cursor: existing behavior
            // If there's a selection, delete it
            if let Some((pos, deleted_text)) = delete_selection(model) {
                model.document.push_edit(EditOperation::Delete {
                    position: pos,
                    text: deleted_text,
                    cursor_before,
                    cursor_after: *model.editor.cursor(),
                });
            } else if model.editor.cursor().column > 0 {
                // No selection - delete character before cursor
                let pos = model.cursor_buffer_position();
                let deleted_char = model.document.buffer.char(pos - 1).to_string();

                model.document.buffer.remove(pos - 1..pos);
                model.editor.cursor_mut().column -= 1;

                model.document.push_edit(EditOperation::Delete {
                    position: pos - 1,
                    text: deleted_char,
                    cursor_before,
                    cursor_after: *model.editor.cursor(),
                });
            } else if model.editor.cursor().line > 0 {
                // At start of line - join with previous line
                let pos = model.cursor_buffer_position();

                let prev_line_idx = model.editor.cursor().line - 1;
                let prev_line = model.document.buffer.line(prev_line_idx);
                let join_column = prev_line.len_chars().saturating_sub(
                    if prev_line.chars().last() == Some('\n') {
                        1
                    } else {
                        0
                    },
                );

                model.document.buffer.remove(pos - 1..pos);
                model.editor.cursor_mut().line -= 1;
                model.editor.cursor_mut().column = join_column;

                model.document.push_edit(EditOperation::Delete {
                    position: pos - 1,
                    text: "\n".to_string(),
                    cursor_before,
                    cursor_after: *model.editor.cursor(),
                });
            }
            model.editor.cursor_mut().desired_column = None;
            model.ensure_cursor_visible();
            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        DocumentMsg::DeleteForward => {
            let cursor_before = *model.editor.cursor();

            // Multi-cursor: process all cursors in reverse document order
            if model.editor.has_multiple_cursors() {
                let indices = cursors_in_reverse_order(model);

                for idx in indices {
                    let cursor = &model.editor.cursors[idx];
                    let pos = model.document.cursor_to_offset(cursor.line, cursor.column);

                    // Delete character at cursor (if not at end of buffer)
                    if pos < model.document.buffer.len_chars() {
                        model.document.buffer.remove(pos..pos + 1);
                    }

                    // Cursor position doesn't change for delete forward

                    // Clear this cursor's selection
                    let new_pos = model.editor.cursors[idx].to_position();
                    model.editor.selections[idx] = Selection::new(new_pos);
                }

                model.document.is_modified = true;
                model.ensure_cursor_visible();
                model.reset_cursor_blink();
                return Some(Cmd::Redraw);
            }

            // Single cursor: existing behavior
            // If there's a selection, delete it
            if let Some((pos, deleted_text)) = delete_selection(model) {
                model.document.push_edit(EditOperation::Delete {
                    position: pos,
                    text: deleted_text,
                    cursor_before,
                    cursor_after: *model.editor.cursor(),
                });
            } else {
                // No selection - delete character at cursor
                let pos = model.cursor_buffer_position();
                if pos < model.document.buffer.len_chars() {
                    let deleted_char = model.document.buffer.char(pos).to_string();
                    model.document.buffer.remove(pos..pos + 1);

                    model.document.push_edit(EditOperation::Delete {
                        position: pos,
                        text: deleted_char,
                        cursor_before,
                        cursor_after: *model.editor.cursor(),
                    });
                }
            }
            model.ensure_cursor_visible();
            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        DocumentMsg::Undo => {
            if let Some(op) = model.document.undo_stack.pop() {
                match &op {
                    EditOperation::Insert {
                        position,
                        text,
                        cursor_before,
                        ..
                    } => {
                        model
                            .document
                            .buffer
                            .remove(*position..*position + text.chars().count());
                        *model.editor.cursor_mut() = *cursor_before;
                    }
                    EditOperation::Delete {
                        position,
                        text,
                        cursor_before,
                        ..
                    } => {
                        model.document.buffer.insert(*position, text);
                        *model.editor.cursor_mut() = *cursor_before;
                    }
                }
                model.document.is_modified = true;
                model.document.redo_stack.push(op);
                model.ensure_cursor_visible();
                model.reset_cursor_blink();
                Some(Cmd::Redraw)
            } else {
                None
            }
        }

        DocumentMsg::Redo => {
            if let Some(op) = model.document.redo_stack.pop() {
                match &op {
                    EditOperation::Insert {
                        position,
                        text,
                        cursor_after,
                        ..
                    } => {
                        model.document.buffer.insert(*position, text);
                        *model.editor.cursor_mut() = *cursor_after;
                    }
                    EditOperation::Delete {
                        position,
                        text,
                        cursor_after,
                        ..
                    } => {
                        model
                            .document
                            .buffer
                            .remove(*position..*position + text.chars().count());
                        *model.editor.cursor_mut() = *cursor_after;
                    }
                }
                model.document.is_modified = true;
                model.document.undo_stack.push(op);
                model.ensure_cursor_visible();
                model.reset_cursor_blink();
                Some(Cmd::Redraw)
            } else {
                None
            }
        }

        DocumentMsg::Copy => {
            // Collect text from all selections (or lines if no selection)
            let mut text_to_copy = String::new();

            if model.editor.has_multiple_cursors() {
                // Multi-cursor: collect text from each selection
                for (i, selection) in model.editor.selections.iter().enumerate() {
                    if !selection.is_empty() {
                        let start = selection.start();
                        let end = selection.end();
                        let start_offset = model.document.cursor_to_offset(start.line, start.column);
                        let end_offset = model.document.cursor_to_offset(end.line, end.column);
                        let selected: String = model.document.buffer
                            .slice(start_offset..end_offset)
                            .chars()
                            .collect();
                        if i > 0 {
                            text_to_copy.push('\n');
                        }
                        text_to_copy.push_str(&selected);
                    }
                }
            } else {
                // Single cursor
                let selection = model.editor.selection();
                if !selection.is_empty() {
                    let start = selection.start();
                    let end = selection.end();
                    let start_offset = model.document.cursor_to_offset(start.line, start.column);
                    let end_offset = model.document.cursor_to_offset(end.line, end.column);
                    text_to_copy = model.document.buffer
                        .slice(start_offset..end_offset)
                        .chars()
                        .collect();
                } else {
                    // No selection: copy entire line
                    if let Some(line_text) = model.document.get_line(model.editor.cursor().line) {
                        text_to_copy = line_text;
                    }
                }
            }

            // Copy to clipboard
            if !text_to_copy.is_empty() {
                if let Ok(mut clipboard) = arboard::Clipboard::new() {
                    let _ = clipboard.set_text(&text_to_copy);
                    model.ui.set_status(format!("Copied {} chars", text_to_copy.len()));
                }
            }

            Some(Cmd::Redraw)
        }

        DocumentMsg::Cut => {
            // First copy, then delete selection
            let mut text_to_copy = String::new();
            let has_selection = !model.editor.selection().is_empty()
                || model.editor.selections.iter().any(|s| !s.is_empty());

            if model.editor.has_multiple_cursors() {
                // Multi-cursor: collect text from each selection
                for (i, selection) in model.editor.selections.iter().enumerate() {
                    if !selection.is_empty() {
                        let start = selection.start();
                        let end = selection.end();
                        let start_offset = model.document.cursor_to_offset(start.line, start.column);
                        let end_offset = model.document.cursor_to_offset(end.line, end.column);
                        let selected: String = model.document.buffer
                            .slice(start_offset..end_offset)
                            .chars()
                            .collect();
                        if i > 0 {
                            text_to_copy.push('\n');
                        }
                        text_to_copy.push_str(&selected);
                    }
                }
            } else {
                let selection = model.editor.selection();
                if !selection.is_empty() {
                    let start = selection.start();
                    let end = selection.end();
                    let start_offset = model.document.cursor_to_offset(start.line, start.column);
                    let end_offset = model.document.cursor_to_offset(end.line, end.column);
                    text_to_copy = model.document.buffer
                        .slice(start_offset..end_offset)
                        .chars()
                        .collect();
                }
            }

            // Copy to clipboard
            if !text_to_copy.is_empty() {
                if let Ok(mut clipboard) = arboard::Clipboard::new() {
                    let _ = clipboard.set_text(&text_to_copy);
                }
            }

            // Delete selections
            if has_selection {
                if model.editor.has_multiple_cursors() {
                    // Delete each selection in reverse order
                    let indices = cursors_in_reverse_order(model);
                    for idx in indices {
                        let selection = &model.editor.selections[idx];
                        if !selection.is_empty() {
                            let start = selection.start();
                            let end = selection.end();
                            let start_offset = model.document.cursor_to_offset(start.line, start.column);
                            let end_offset = model.document.cursor_to_offset(end.line, end.column);
                            model.document.buffer.remove(start_offset..end_offset);
                            model.editor.cursors[idx].line = start.line;
                            model.editor.cursors[idx].column = start.column;
                            model.editor.selections[idx] = Selection::new(start);
                        }
                    }
                } else {
                    delete_selection(model);
                }
                model.document.is_modified = true;
                model.ui.set_status(format!("Cut {} chars", text_to_copy.len()));
            }

            model.ensure_cursor_visible();
            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        DocumentMsg::Paste => {
            // Get text from clipboard
            let clipboard_text = if let Ok(mut clipboard) = arboard::Clipboard::new() {
                clipboard.get_text().ok()
            } else {
                None
            };

            if let Some(text) = clipboard_text {
                if text.is_empty() {
                    return Some(Cmd::Redraw);
                }

                let cursor_before = *model.editor.cursor();

                if model.editor.has_multiple_cursors() {
                    let lines: Vec<&str> = text.lines().collect();
                    let cursor_count = model.editor.cursors.len();

                    // If clipboard has same number of lines as cursors, distribute one per cursor
                    if lines.len() == cursor_count {
                        let indices = cursors_in_reverse_order(model);
                        for (i, idx) in indices.iter().enumerate() {
                            let line_to_paste = lines[cursor_count - 1 - i]; // Reverse order
                            let cursor = &model.editor.cursors[*idx];
                            let pos = model.document.cursor_to_offset(cursor.line, cursor.column);
                            model.document.buffer.insert(pos, line_to_paste);
                            model.editor.cursors[*idx].column += line_to_paste.chars().count();
                            let new_pos = model.editor.cursors[*idx].to_position();
                            model.editor.selections[*idx] = Selection::new(new_pos);
                        }
                    } else {
                        // Paste full text at each cursor
                        let indices = cursors_in_reverse_order(model);
                        for idx in indices {
                            let cursor = &model.editor.cursors[idx];
                            let pos = model.document.cursor_to_offset(cursor.line, cursor.column);
                            model.document.buffer.insert(pos, &text);

                            // Update cursor position (move to end of pasted text)
                            let new_offset = pos + text.chars().count();
                            let (new_line, new_col) = model.document.offset_to_cursor(new_offset);
                            model.editor.cursors[idx].line = new_line;
                            model.editor.cursors[idx].column = new_col;
                            let new_pos = model.editor.cursors[idx].to_position();
                            model.editor.selections[idx] = Selection::new(new_pos);
                        }
                    }
                } else {
                    // Single cursor: delete selection first if present
                    let pos = if !model.editor.selection().is_empty() {
                        let (start_pos, _) = delete_selection(model).unwrap();
                        start_pos
                    } else {
                        model.cursor_buffer_position()
                    };

                    model.document.buffer.insert(pos, &text);

                    // Move cursor to end of pasted text
                    let new_offset = pos + text.chars().count();
                    model.set_cursor_from_position(new_offset);

                    model.document.push_edit(EditOperation::Insert {
                        position: pos,
                        text: text.clone(),
                        cursor_before,
                        cursor_after: *model.editor.cursor(),
                    });
                }

                model.document.is_modified = true;
                model.ui.set_status(format!("Pasted {} chars", text.len()));
                model.ensure_cursor_visible();
                model.reset_cursor_blink();
            }

            Some(Cmd::Redraw)
        }
    }
}

/// Handle UI messages (status bar, cursor blink)
pub fn update_ui(model: &mut AppModel, msg: UiMsg) -> Option<Cmd> {
    use crate::model::{SegmentContent, SegmentId, TransientMessage};

    match msg {
        UiMsg::SetStatus(message) => {
            // Legacy: also update the StatusMessage segment
            model.ui.status_bar.update_segment(
                SegmentId::StatusMessage,
                SegmentContent::Text(message.clone()),
            );
            model.ui.set_status(message);
            Some(Cmd::Redraw)
        }

        UiMsg::BlinkCursor => {
            if model.ui.update_cursor_blink(Duration::from_millis(500)) {
                Some(Cmd::Redraw)
            } else {
                None
            }
        }

        UiMsg::UpdateSegment { id, content } => {
            model.ui.status_bar.update_segment(id, content);
            Some(Cmd::Redraw)
        }

        UiMsg::SetTransientMessage { text, duration_ms } => {
            let transient = TransientMessage::new(
                text.clone(),
                Duration::from_millis(duration_ms),
            );
            model.ui.transient_message = Some(transient);
            // Also update the StatusMessage segment
            model.ui.status_bar.update_segment(
                SegmentId::StatusMessage,
                SegmentContent::Text(text),
            );
            Some(Cmd::Redraw)
        }

        UiMsg::ClearTransientMessage => {
            model.ui.transient_message = None;
            model.ui.status_bar.update_segment(
                SegmentId::StatusMessage,
                SegmentContent::Empty,
            );
            Some(Cmd::Redraw)
        }
    }
}

/// Handle app messages (file operations, window events)
pub fn update_app(model: &mut AppModel, msg: AppMsg) -> Option<Cmd> {
    match msg {
        AppMsg::Resize(width, height) => {
            model.resize(width, height);
            Some(Cmd::Redraw)
        }

        AppMsg::SaveFile => match &model.document.file_path {
            Some(path) => {
                model.ui.is_saving = true;
                model.ui.set_status("Saving...");
                Some(Cmd::SaveFile {
                    path: path.clone(),
                    content: model.document.buffer.to_string(),
                })
            }
            None => {
                model.ui.set_status("No file path - cannot save");
                Some(Cmd::Redraw)
            }
        },

        AppMsg::LoadFile(path) => {
            model.ui.is_loading = true;
            model.ui.set_status("Loading...");
            Some(Cmd::LoadFile { path })
        }

        AppMsg::NewFile => {
            // TODO: Implement new file
            model.ui.set_status("New file not yet implemented");
            Some(Cmd::Redraw)
        }

        AppMsg::SaveCompleted(result) => {
            model.ui.is_saving = false;
            match result {
                Ok(_) => {
                    model.document.is_modified = false;
                    if let Some(path) = &model.document.file_path {
                        model.ui.set_status(format!("Saved: {}", path.display()));
                    }
                }
                Err(e) => {
                    model.ui.set_status(format!("Error: {}", e));
                }
            }
            Some(Cmd::Redraw)
        }

        AppMsg::FileLoaded { path, result } => {
            model.ui.is_loading = false;
            match result {
                Ok(content) => {
                    model.document.buffer = ropey::Rope::from(content);
                    model.document.file_path = Some(path.clone());
                    model.document.is_modified = false;
                    model.document.undo_stack.clear();
                    model.document.redo_stack.clear();
                    *model.editor.cursor_mut() = Default::default();
                    model.ui.set_status(format!("Loaded: {}", path.display()));
                }
                Err(e) => {
                    model.ui.set_status(format!("Error: {}", e));
                }
            }
            Some(Cmd::Redraw)
        }

        AppMsg::Quit => {
            // Handled by the event loop
            None
        }
    }
}

// Helper functions for cursor movement

fn move_cursor_up(model: &mut AppModel) {
    if model.editor.cursor_mut().line > 0 {
        model.editor.cursor_mut().line -= 1;

        let desired = model
            .editor
            .cursor()
            .desired_column
            .unwrap_or(model.editor.cursor_mut().column);
        let line_len = model.current_line_length();
        model.editor.cursor_mut().column = desired.min(line_len);
        model.editor.cursor_mut().desired_column = Some(desired);

        let padding = model.editor.scroll_padding;
        let top_boundary = model.editor.viewport.top_line + padding;

        if model.editor.cursor_mut().line < top_boundary && model.editor.viewport.top_line > 0 {
            model.editor.viewport.top_line = model.editor.cursor_mut().line.saturating_sub(padding);
        }
    }
}

fn move_cursor_down(model: &mut AppModel) {
    if model.editor.cursor_mut().line < model.document.line_count().saturating_sub(1) {
        model.editor.cursor_mut().line += 1;

        let desired = model
            .editor
            .cursor()
            .desired_column
            .unwrap_or(model.editor.cursor_mut().column);
        let line_len = model.current_line_length();
        model.editor.cursor_mut().column = desired.min(line_len);
        model.editor.cursor_mut().desired_column = Some(desired);

        let padding = model.editor.scroll_padding;
        let bottom_boundary =
            model.editor.viewport.top_line + model.editor.viewport.visible_lines - padding - 1;
        let max_top = model
            .document
            .line_count()
            .saturating_sub(model.editor.viewport.visible_lines);

        if model.editor.cursor_mut().line > bottom_boundary
            && model.editor.viewport.top_line < max_top
        {
            let desired_top = model.editor.cursor_mut().line + padding + 1;
            model.editor.viewport.top_line = desired_top
                .saturating_sub(model.editor.viewport.visible_lines)
                .min(max_top);
        }
    }
}

fn move_cursor_left(model: &mut AppModel) {
    if model.editor.cursor_mut().column > 0 {
        model.editor.cursor_mut().column -= 1;
        model.editor.cursor_mut().desired_column = None;
    } else if model.editor.cursor_mut().line > 0 {
        model.editor.cursor_mut().line -= 1;
        model.editor.cursor_mut().column = model.current_line_length();
        model.editor.cursor_mut().desired_column = None;
    }
    model.ensure_cursor_visible();
}

fn move_cursor_right(model: &mut AppModel) {
    let line_len = model.current_line_length();
    if model.editor.cursor_mut().column < line_len {
        model.editor.cursor_mut().column += 1;
        model.editor.cursor_mut().desired_column = None;
    } else if model.editor.cursor_mut().line < model.document.line_count().saturating_sub(1) {
        model.editor.cursor_mut().line += 1;
        model.editor.cursor_mut().column = 0;
        model.editor.cursor_mut().desired_column = None;
    }
    model.ensure_cursor_visible();
}

fn move_cursor_word_left(model: &mut AppModel) {
    let pos = model.cursor_buffer_position();
    if pos == 0 {
        return;
    }

    let text: String = model.document.buffer.slice(..pos).chars().collect();
    let chars: Vec<char> = text.chars().collect();
    let mut i = chars.len();

    if i > 0 {
        let current_type = char_type(chars[i - 1]);
        while i > 0 && char_type(chars[i - 1]) == current_type {
            i -= 1;
        }
    }

    // Use move_cursor_to_position to preserve selection
    model.move_cursor_to_position(i);
}

fn move_cursor_word_right(model: &mut AppModel) {
    let pos = model.cursor_buffer_position();
    let total_chars = model.document.buffer.len_chars();
    if pos >= total_chars {
        return;
    }

    let text: String = model.document.buffer.slice(pos..).chars().collect();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;

    if !chars.is_empty() {
        let current_type = char_type(chars[0]);
        while i < chars.len() && char_type(chars[i]) == current_type {
            i += 1;
        }
    }

    // Use move_cursor_to_position to preserve selection
    model.move_cursor_to_position(pos + i);
}
