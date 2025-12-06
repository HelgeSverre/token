//! Update functions for the Elm-style architecture
//!
//! All state transformations flow through these functions.

use std::time::Duration;

use crate::commands::Cmd;
use crate::messages::{AppMsg, Direction, DocumentMsg, EditorMsg, LayoutMsg, Msg, UiMsg};
use crate::model::{
    AppModel, Cursor, EditOperation, EditorGroup, EditorState, GroupId, LayoutNode, Position,
    Selection, SplitContainer, SplitDirection, Tab,
};
use crate::util::char_type;

use crate::model::sync_status_bar;

/// Main update function - dispatches to sub-handlers
pub fn update(model: &mut AppModel, msg: Msg) -> Option<Cmd> {
    let result = match msg {
        Msg::Editor(m) => update_editor(model, m),
        Msg::Document(m) => update_document(model, m),
        Msg::Ui(m) => update_ui(model, m),
        Msg::Layout(m) => update_layout(model, m),
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
            if model.editor_mut().cursor_mut().column == first_non_ws {
                model.editor_mut().cursor_mut().column = 0;
            } else {
                model.editor_mut().cursor_mut().column = first_non_ws;
            }
            model.editor_mut().cursor_mut().desired_column = None;
            model.ensure_cursor_visible();
            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        EditorMsg::MoveCursorLineEnd => {
            let line_end = model.current_line_length();
            let last_non_ws = model.last_non_whitespace_column();
            if model.editor_mut().cursor_mut().column == last_non_ws {
                model.editor_mut().cursor_mut().column = line_end;
            } else {
                model.editor_mut().cursor_mut().column = last_non_ws;
            }
            model.editor_mut().cursor_mut().desired_column = None;
            model.ensure_cursor_visible();
            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        EditorMsg::MoveCursorDocumentStart => {
            model.editor_mut().cursor_mut().line = 0;
            model.editor_mut().cursor_mut().column = 0;
            model.editor_mut().cursor_mut().desired_column = None;
            model.editor_mut().viewport.top_line = 0;
            model.ensure_cursor_visible();
            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        EditorMsg::MoveCursorDocumentEnd => {
            model.editor_mut().cursor_mut().line = model.document().line_count().saturating_sub(1);
            model.editor_mut().cursor_mut().column = model.current_line_length();
            model.editor_mut().cursor_mut().desired_column = None;
            let cursor_line = model.editor().cursor().line;
            let visible_lines = model.editor().viewport.visible_lines;
            if cursor_line >= visible_lines {
                model.editor_mut().viewport.top_line = cursor_line - visible_lines + 1;
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
            let jump = model.editor().viewport.visible_lines.saturating_sub(2);
            model.editor_mut().cursor_mut().line =
                model.editor().cursor().line.saturating_sub(jump);

            let desired = model
                .editor()
                .cursor()
                .desired_column
                .unwrap_or(model.editor().cursor().column);
            let line_len = model.current_line_length();
            model.editor_mut().cursor_mut().column = desired.min(line_len);
            model.editor_mut().cursor_mut().desired_column = Some(desired);

            model.editor_mut().viewport.top_line =
                model.editor().viewport.top_line.saturating_sub(jump);
            // Use top-aligned reveal for upward page movement
            model.ensure_cursor_visible_directional(Some(true));
            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        EditorMsg::PageDown => {
            let jump = model.editor().viewport.visible_lines.saturating_sub(2);
            let max_line = model.document().line_count().saturating_sub(1);
            model.editor_mut().cursor_mut().line =
                (model.editor().cursor().line + jump).min(max_line);

            let desired = model
                .editor()
                .cursor()
                .desired_column
                .unwrap_or(model.editor().cursor().column);
            let line_len = model.current_line_length();
            model.editor_mut().cursor_mut().column = desired.min(line_len);
            model.editor_mut().cursor_mut().desired_column = Some(desired);

            let cursor_line = model.editor().cursor().line;
            let top_line = model.editor().viewport.top_line;
            let visible_lines = model.editor().viewport.visible_lines;
            if cursor_line >= top_line + visible_lines {
                model.editor_mut().viewport.top_line =
                    cursor_line.saturating_sub(visible_lines.saturating_sub(1));
            }
            // Use bottom-aligned reveal for downward page movement
            model.ensure_cursor_visible_directional(Some(false));
            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        EditorMsg::SetCursorPosition { line, column } => {
            model.editor_mut().cursor_mut().line = line;
            model.editor_mut().cursor_mut().column = column;
            model.editor_mut().cursor_mut().desired_column = None;
            model.ensure_cursor_visible();
            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        EditorMsg::Scroll(delta) => {
            let total_lines = model.document().line_count();
            let visible_lines = model.editor().viewport.visible_lines;
            if total_lines <= visible_lines {
                return None;
            }

            let max_top = total_lines.saturating_sub(visible_lines);

            if delta > 0 {
                model.editor_mut().viewport.top_line =
                    (model.editor().viewport.top_line + delta as usize).min(max_top);
            } else if delta < 0 {
                model.editor_mut().viewport.top_line = model
                    .editor()
                    .viewport
                    .top_line
                    .saturating_sub(delta.unsigned_abs() as usize);
            }

            Some(Cmd::Redraw)
        }

        EditorMsg::ScrollHorizontal(delta) => {
            let top_line = model.editor().viewport.top_line;
            let visible_lines = model.editor().viewport.visible_lines;
            let line_count = model.document().line_count();
            let max_line_len = (top_line..top_line + visible_lines)
                .filter_map(|i| {
                    if i < line_count {
                        Some(model.document().line_length(i))
                    } else {
                        None
                    }
                })
                .max()
                .unwrap_or(0);

            let visible_columns = model.editor().viewport.visible_columns;
            if max_line_len <= visible_columns {
                model.editor_mut().viewport.left_column = 0;
                return None;
            }

            let max_left = max_line_len.saturating_sub(visible_columns);

            if delta > 0 {
                model.editor_mut().viewport.left_column =
                    (model.editor().viewport.left_column + delta as usize).min(max_left);
            } else if delta < 0 {
                model.editor_mut().viewport.left_column = model
                    .editor()
                    .viewport
                    .left_column
                    .saturating_sub(delta.unsigned_abs() as usize);
            }

            Some(Cmd::Redraw)
        }

        // === Selection Movement (Shift+key) ===
        EditorMsg::MoveCursorWithSelection(direction) => {
            // If selection is empty, anchor starts at current cursor
            if model.editor().selection().is_empty() {
                let pos = model.editor().cursor().to_position();
                model.editor_mut().selection_mut().anchor = pos;
            }

            // Move the cursor
            match direction {
                Direction::Up => move_cursor_up(model),
                Direction::Down => move_cursor_down(model),
                Direction::Left => move_cursor_left(model),
                Direction::Right => move_cursor_right(model),
            }

            // Update head to new cursor position
            model.editor_mut().selection_mut().head = model.editor().cursor().to_position();
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
            if model.editor().selection().is_empty() {
                let pos = model.editor().cursor().to_position();
                model.editor_mut().selection_mut().anchor = pos;
            }

            let first_non_ws = model.first_non_whitespace_column();
            if model.editor().cursor().column == first_non_ws {
                model.editor_mut().cursor_mut().column = 0;
            } else {
                model.editor_mut().cursor_mut().column = first_non_ws;
            }
            model.editor_mut().cursor_mut().desired_column = None;
            model.ensure_cursor_visible();

            model.editor_mut().selection_mut().head = model.editor().cursor().to_position();
            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        EditorMsg::MoveCursorLineEndWithSelection => {
            if model.editor().selection().is_empty() {
                let pos = model.editor().cursor().to_position();
                model.editor_mut().selection_mut().anchor = pos;
            }

            let line_end = model.current_line_length();
            let last_non_ws = model.last_non_whitespace_column();
            if model.editor().cursor().column == last_non_ws {
                model.editor_mut().cursor_mut().column = line_end;
            } else {
                model.editor_mut().cursor_mut().column = last_non_ws;
            }
            model.editor_mut().cursor_mut().desired_column = None;
            model.ensure_cursor_visible();

            model.editor_mut().selection_mut().head = model.editor().cursor().to_position();
            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        EditorMsg::MoveCursorDocumentStartWithSelection => {
            if model.editor().selection().is_empty() {
                let pos = model.editor().cursor().to_position();
                model.editor_mut().selection_mut().anchor = pos;
            }

            model.editor_mut().cursor_mut().line = 0;
            model.editor_mut().cursor_mut().column = 0;
            model.editor_mut().cursor_mut().desired_column = None;
            model.editor_mut().viewport.top_line = 0;
            model.ensure_cursor_visible();

            model.editor_mut().selection_mut().head = model.editor().cursor().to_position();
            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        EditorMsg::MoveCursorDocumentEndWithSelection => {
            if model.editor().selection().is_empty() {
                let pos = model.editor().cursor().to_position();
                model.editor_mut().selection_mut().anchor = pos;
            }

            model.editor_mut().cursor_mut().line = model.document().line_count().saturating_sub(1);
            model.editor_mut().cursor_mut().column = model.current_line_length();
            model.editor_mut().cursor_mut().desired_column = None;
            let cursor_line = model.editor().cursor().line;
            let visible_lines = model.editor().viewport.visible_lines;
            if cursor_line >= visible_lines {
                model.editor_mut().viewport.top_line = cursor_line - visible_lines + 1;
            }
            model.ensure_cursor_visible();

            model.editor_mut().selection_mut().head = model.editor().cursor().to_position();
            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        EditorMsg::MoveCursorWordWithSelection(direction) => {
            if model.editor().selection().is_empty() {
                let pos = model.editor().cursor().to_position();
                model.editor_mut().selection_mut().anchor = pos;
            }

            match direction {
                Direction::Left => move_cursor_word_left(model),
                Direction::Right => move_cursor_word_right(model),
                _ => {} // Up/Down not used for word movement
            }
            model.ensure_cursor_visible();

            model.editor_mut().selection_mut().head = model.editor().cursor().to_position();
            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        EditorMsg::PageUpWithSelection => {
            if model.editor().selection().is_empty() {
                let pos = model.editor().cursor().to_position();
                model.editor_mut().selection_mut().anchor = pos;
            }

            let jump = model.editor().viewport.visible_lines.saturating_sub(2);
            model.editor_mut().cursor_mut().line =
                model.editor().cursor().line.saturating_sub(jump);

            let desired = model
                .editor()
                .cursor()
                .desired_column
                .unwrap_or(model.editor().cursor().column);
            let line_len = model.current_line_length();
            model.editor_mut().cursor_mut().column = desired.min(line_len);
            model.editor_mut().cursor_mut().desired_column = Some(desired);

            model.editor_mut().viewport.top_line =
                model.editor().viewport.top_line.saturating_sub(jump);
            model.ensure_cursor_visible();

            model.editor_mut().selection_mut().head = model.editor().cursor().to_position();
            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        EditorMsg::PageDownWithSelection => {
            if model.editor().selection().is_empty() {
                let pos = model.editor().cursor().to_position();
                model.editor_mut().selection_mut().anchor = pos;
            }

            let jump = model.editor().viewport.visible_lines.saturating_sub(2);
            let max_line = model.document().line_count().saturating_sub(1);
            model.editor_mut().cursor_mut().line =
                (model.editor().cursor().line + jump).min(max_line);

            let desired = model
                .editor()
                .cursor()
                .desired_column
                .unwrap_or(model.editor().cursor().column);
            let line_len = model.current_line_length();
            model.editor_mut().cursor_mut().column = desired.min(line_len);
            model.editor_mut().cursor_mut().desired_column = Some(desired);

            let cursor_line = model.editor().cursor().line;
            let top_line = model.editor().viewport.top_line;
            let visible_lines = model.editor().viewport.visible_lines;
            if cursor_line >= top_line + visible_lines {
                model.editor_mut().viewport.top_line =
                    cursor_line.saturating_sub(visible_lines.saturating_sub(1));
            }
            model.ensure_cursor_visible();

            model.editor_mut().selection_mut().head = model.editor().cursor().to_position();
            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        // === Selection Commands ===
        EditorMsg::SelectAll => {
            use crate::model::editor::Position;

            model.editor_mut().selection_mut().anchor = Position::new(0, 0);
            model.editor_mut().cursor_mut().line = model.document().line_count().saturating_sub(1);
            model.editor_mut().cursor_mut().column = model.current_line_length();
            model.editor_mut().selection_mut().head = model.editor().cursor().to_position();
            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        EditorMsg::SelectWord => {
            use crate::model::editor::Position;

            let line = model.editor().cursor().line;
            let column = model.editor().cursor().column;

            // Get the current line text
            if let Some(line_text) = model.document().get_line(line) {
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
                model.editor_mut().selection_mut().anchor = Position::new(line, start_col);
                model.editor_mut().selection_mut().head = Position::new(line, end_col);
                model.editor_mut().cursor_mut().column = end_col;
                model.editor_mut().cursor_mut().desired_column = None;
            }

            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        EditorMsg::SelectLine => {
            use crate::model::editor::Position;

            let line = model.editor().cursor().line;
            let line_len = model.document().line_length(line);
            let total_lines = model.document().line_count();

            // Select from start of current line to start of next line (or end of document)
            model.editor_mut().selection_mut().anchor = Position::new(line, 0);

            if line + 1 < total_lines {
                // Select to start of next line (includes the newline)
                model.editor_mut().selection_mut().head = Position::new(line + 1, 0);
                model.editor_mut().cursor_mut().line = line + 1;
                model.editor_mut().cursor_mut().column = 0;
            } else {
                // Last line - select to end of line
                model.editor_mut().selection_mut().head = Position::new(line, line_len);
                model.editor_mut().cursor_mut().column = line_len;
            }
            model.editor_mut().cursor_mut().desired_column = None;

            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        EditorMsg::ExtendSelectionToPosition { line, column } => {
            use crate::model::editor::Position;

            // If selection is empty, anchor at current cursor
            if model.editor().selection().is_empty() {
                let pos = model.editor().cursor().to_position();
                model.editor_mut().selection_mut().anchor = pos;
            }

            // Move cursor to target position
            model.editor_mut().cursor_mut().line = line;
            model.editor_mut().cursor_mut().column = column;
            model.editor_mut().cursor_mut().desired_column = None;

            // Update head
            model.editor_mut().selection_mut().head = Position::new(line, column);
            model.ensure_cursor_visible();
            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        EditorMsg::ClearSelection => {
            model.editor_mut().clear_selection();
            Some(Cmd::Redraw)
        }

        // === Multi-Cursor ===
        EditorMsg::ToggleCursorAtPosition { line, column } => {
            model.editor_mut().toggle_cursor_at(line, column);
            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        EditorMsg::AddCursorAbove => {
            // For each existing cursor, try to add one on the line above
            let cursors_snapshot: Vec<_> = model.editor().cursors.iter().cloned().collect();
            let new_positions: Vec<_> = cursors_snapshot
                .iter()
                .filter(|c| c.line > 0)
                .map(|c| {
                    let target_line = c.line - 1;
                    let target_col = c.desired_column.unwrap_or(c.column);
                    let line_len = model.document().line_length(target_line);
                    (target_line, target_col.min(line_len))
                })
                .collect();

            for (line, col) in new_positions {
                model.editor_mut().add_cursor_at(line, col);
            }
            model.editor_mut().deduplicate_cursors();
            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        EditorMsg::AddCursorBelow => {
            // For each existing cursor, try to add one on the line below
            let total_lines = model.document().line_count();
            let cursors_snapshot: Vec<_> = model.editor().cursors.iter().cloned().collect();
            let new_positions: Vec<_> = cursors_snapshot
                .iter()
                .filter(|c| c.line + 1 < total_lines)
                .map(|c| {
                    let target_line = c.line + 1;
                    let target_col = c.desired_column.unwrap_or(c.column);
                    let line_len = model.document().line_length(target_line);
                    (target_line, target_col.min(line_len))
                })
                .collect();

            for (line, col) in new_positions {
                model.editor_mut().add_cursor_at(line, col);
            }
            model.editor_mut().deduplicate_cursors();
            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        EditorMsg::CollapseToSingleCursor => {
            model.editor_mut().collapse_to_primary();
            model.editor_mut().clear_selection();
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
            model.editor_mut().rectangle_selection.active = true;
            model.editor_mut().rectangle_selection.start = Position::new(line, column);
            model.editor_mut().rectangle_selection.current = Position::new(line, column);
            Some(Cmd::Redraw)
        }

        EditorMsg::UpdateRectangleSelection { line, column } => {
            if model.editor().rectangle_selection.active {
                model.editor_mut().rectangle_selection.current = Position::new(line, column);

                // Compute preview cursor positions
                let top_left = model.editor().rectangle_selection.top_left();
                let bottom_right = model.editor().rectangle_selection.bottom_right();
                let cursor_col = model.editor().rectangle_selection.current.column;

                model
                    .editor_mut()
                    .rectangle_selection
                    .preview_cursors
                    .clear();
                for preview_line in top_left.line..=bottom_right.line {
                    model
                        .editor_mut()
                        .rectangle_selection
                        .preview_cursors
                        .push(Position::new(preview_line, cursor_col));
                }
            }
            Some(Cmd::Redraw)
        }

        EditorMsg::FinishRectangleSelection => {
            if !model.editor().rectangle_selection.active {
                return Some(Cmd::Redraw);
            }

            let top_left = model.editor().rectangle_selection.top_left();
            let bottom_right = model.editor().rectangle_selection.bottom_right();
            // The cursor should be at the "current" position (where user dragged TO)
            let cursor_col = model.editor().rectangle_selection.current.column;

            // Clear existing cursors and selections
            model.editor_mut().cursors.clear();
            model.editor_mut().selections.clear();

            // Create a cursor (and optionally selection) for each line in the rectangle
            for line in top_left.line..=bottom_right.line {
                let line_len = model.document().line_length(line);

                // Clamp columns to line length
                let start_col = top_left.column.min(line_len);
                let end_col = bottom_right.column.min(line_len);
                let clamped_cursor_col = cursor_col.min(line_len);

                // Create cursor at the dragged-to position (clamped to line length)
                let cursor = Cursor::at(line, clamped_cursor_col);
                model.editor_mut().cursors.push(cursor);

                // Create selection if rectangle has width
                if start_col < end_col {
                    // Anchor is the opposite end from cursor, head is at cursor
                    let anchor_col = if cursor_col == start_col {
                        end_col
                    } else {
                        start_col
                    };
                    let selection = Selection {
                        anchor: Position::new(line, anchor_col.min(line_len)),
                        head: Position::new(line, clamped_cursor_col),
                    };
                    model.editor_mut().selections.push(selection);
                } else {
                    // Zero-width: just cursor, no selection
                    let selection = Selection::new(Position::new(line, clamped_cursor_col));
                    model.editor_mut().selections.push(selection);
                }
            }

            // Deactivate rectangle selection mode and clear preview
            model.editor_mut().rectangle_selection.active = false;
            model
                .editor_mut()
                .rectangle_selection
                .preview_cursors
                .clear();

            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        EditorMsg::CancelRectangleSelection => {
            model.editor_mut().rectangle_selection.active = false;
            model
                .editor_mut()
                .rectangle_selection
                .preview_cursors
                .clear();
            Some(Cmd::Redraw)
        }
    }
}

/// Delete the current selection and return (start_offset, deleted_text)
/// Returns None if selection is empty
fn delete_selection(model: &mut AppModel) -> Option<(usize, String)> {
    let selection = model.editor().selection().clone();
    if selection.is_empty() {
        return None;
    }

    let sel_start = selection.start();
    let sel_end = selection.end();

    // Convert positions to buffer offsets
    let start_offset = model
        .document()
        .cursor_to_offset(sel_start.line, sel_start.column);
    let end_offset = model
        .document()
        .cursor_to_offset(sel_end.line, sel_end.column);

    // Get the text being deleted
    let deleted_text: String = model
        .document()
        .buffer
        .slice(start_offset..end_offset)
        .chars()
        .collect();

    // Delete the range
    model.document_mut().buffer.remove(start_offset..end_offset);

    // Move cursor to selection start
    model.editor_mut().cursor_mut().line = sel_start.line;
    model.editor_mut().cursor_mut().column = sel_start.column;
    model.editor_mut().cursor_mut().desired_column = None;

    // Clear the selection
    model.editor_mut().clear_selection();

    Some((start_offset, deleted_text))
}

/// Get cursor indices sorted by position in reverse document order (last first)
fn cursors_in_reverse_order(model: &AppModel) -> Vec<usize> {
    let mut indices: Vec<usize> = (0..model.editor().cursors.len()).collect();
    indices.sort_by(|&a, &b| {
        let ca = &model.editor().cursors[a];
        let cb = &model.editor().cursors[b];
        // Sort descending: higher line first, then higher column
        cb.line
            .cmp(&ca.line)
            .then_with(|| cb.column.cmp(&ca.column))
    });
    indices
}

/// Handle document messages (text editing, undo/redo)
pub fn update_document(model: &mut AppModel, msg: DocumentMsg) -> Option<Cmd> {
    match msg {
        DocumentMsg::InsertChar(ch) => {
            let cursor_before = *model.editor().cursor();

            // Multi-cursor: process all cursors in reverse document order
            if model.editor().has_multiple_cursors() {
                let indices = cursors_in_reverse_order(model);

                for idx in indices {
                    // Get cursor position and convert to buffer offset
                    let cursor = model.editor().cursors[idx].clone();
                    let pos = model
                        .document()
                        .cursor_to_offset(cursor.line, cursor.column);

                    // Insert character
                    model.document_mut().buffer.insert_char(pos, ch);

                    // Update this cursor's position (move right by 1)
                    model.editor_mut().cursors[idx].column += 1;
                    model.editor_mut().cursors[idx].desired_column = None;

                    // Clear this cursor's selection
                    let new_pos = model.editor().cursors[idx].to_position();
                    model.editor_mut().selections[idx] = Selection::new(new_pos);
                }

                // Record single edit for undo (simplified - full undo would need batch)
                let position = model.cursor_buffer_position().saturating_sub(1);
                let cursor_after = *model.editor().cursor();
                model.document_mut().push_edit(EditOperation::Insert {
                    position,
                    text: ch.to_string(),
                    cursor_before,
                    cursor_after,
                });

                model.document_mut().is_modified = true;
                model.ensure_cursor_visible();
                model.reset_cursor_blink();
                return Some(Cmd::Redraw);
            }

            // Single cursor: existing behavior
            // If there's a selection, delete it first and use Replace for atomic undo
            if let Some((pos, deleted_text)) = delete_selection(model) {
                // Insert at selection start
                model.document_mut().buffer.insert_char(pos, ch);
                model.set_cursor_from_position(pos + 1);
                model.ensure_cursor_visible();

                // Record as a single Replace operation for atomic undo
                let cursor_after = *model.editor().cursor();
                model.document_mut().push_edit(EditOperation::Replace {
                    position: pos,
                    deleted_text,
                    inserted_text: ch.to_string(),
                    cursor_before,
                    cursor_after,
                });
            } else {
                // No selection - normal insert
                let pos = model.cursor_buffer_position();
                model.document_mut().buffer.insert_char(pos, ch);
                model.set_cursor_from_position(pos + 1);
                model.ensure_cursor_visible();

                let cursor_after = *model.editor().cursor();
                model.document_mut().push_edit(EditOperation::Insert {
                    position: pos,
                    text: ch.to_string(),
                    cursor_before,
                    cursor_after,
                });
            }

            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        DocumentMsg::InsertNewline => {
            let cursor_before = *model.editor().cursor();

            // Multi-cursor: process all cursors in reverse document order
            if model.editor().has_multiple_cursors() {
                let indices = cursors_in_reverse_order(model);

                for idx in indices {
                    let cursor = model.editor().cursors[idx].clone();
                    let pos = model
                        .document()
                        .cursor_to_offset(cursor.line, cursor.column);
                    model.document_mut().buffer.insert_char(pos, '\n');

                    // Move cursor to beginning of next line
                    model.editor_mut().cursors[idx].line += 1;
                    model.editor_mut().cursors[idx].column = 0;
                    model.editor_mut().cursors[idx].desired_column = None;

                    let new_pos = model.editor().cursors[idx].to_position();
                    model.editor_mut().selections[idx] = Selection::new(new_pos);
                }

                let position = model.cursor_buffer_position().saturating_sub(1);
                let cursor_after = *model.editor().cursor();
                model.document_mut().push_edit(EditOperation::Insert {
                    position,
                    text: "\n".to_string(),
                    cursor_before,
                    cursor_after,
                });

                model.document_mut().is_modified = true;
                model.ensure_cursor_visible();
                model.reset_cursor_blink();
                return Some(Cmd::Redraw);
            }

            // Single cursor: check for selection first
            if let Some((pos, deleted_text)) = delete_selection(model) {
                model.document_mut().buffer.insert_char(pos, '\n');
                model.set_cursor_from_position(pos + 1);
                model.ensure_cursor_visible();

                let cursor_after = *model.editor().cursor();
                model.document_mut().push_edit(EditOperation::Replace {
                    position: pos,
                    deleted_text,
                    inserted_text: "\n".to_string(),
                    cursor_before,
                    cursor_after,
                });
            } else {
                let pos = model.cursor_buffer_position();
                model.document_mut().buffer.insert_char(pos, '\n');
                model.set_cursor_from_position(pos + 1);
                model.ensure_cursor_visible();

                let cursor_after = *model.editor().cursor();
                model.document_mut().push_edit(EditOperation::Insert {
                    position: pos,
                    text: "\n".to_string(),
                    cursor_before,
                    cursor_after,
                });
            }

            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        DocumentMsg::DeleteBackward => {
            let cursor_before = *model.editor().cursor();

            // Multi-cursor: process all cursors in reverse document order
            if model.editor().has_multiple_cursors() {
                let indices = cursors_in_reverse_order(model);

                for idx in indices {
                    let selection = model.editor().selections[idx].clone();
                    if !selection.is_empty() {
                        // Delete selection
                        let start = selection.start();
                        let end = selection.end();
                        let start_offset =
                            model.document().cursor_to_offset(start.line, start.column);
                        let end_offset = model.document().cursor_to_offset(end.line, end.column);
                        model.document_mut().buffer.remove(start_offset..end_offset);
                        model.editor_mut().cursors[idx].line = start.line;
                        model.editor_mut().cursors[idx].column = start.column;
                        model.editor_mut().selections[idx] = Selection::new(start);
                    } else {
                        let cursor = model.editor().cursors[idx].clone();
                        let pos = model
                            .document()
                            .cursor_to_offset(cursor.line, cursor.column);
                        if pos > 0 {
                            model.document_mut().buffer.remove(pos - 1..pos);
                            let (new_line, new_col) = model.document().offset_to_cursor(pos - 1);
                            model.editor_mut().cursors[idx].line = new_line;
                            model.editor_mut().cursors[idx].column = new_col;
                            let new_pos = model.editor().cursors[idx].to_position();
                            model.editor_mut().selections[idx] = Selection::new(new_pos);
                        }
                    }
                }

                model.document_mut().is_modified = true;
                model.ensure_cursor_visible();
                model.reset_cursor_blink();
                return Some(Cmd::Redraw);
            }

            // Single cursor: check for selection
            if let Some((pos, deleted_text)) = delete_selection(model) {
                let cursor_after = *model.editor().cursor();
                model.document_mut().push_edit(EditOperation::Delete {
                    position: pos,
                    text: deleted_text,
                    cursor_before,
                    cursor_after,
                });
                model.document_mut().is_modified = true;
                model.ensure_cursor_visible();
                model.reset_cursor_blink();
                return Some(Cmd::Redraw);
            }

            let pos = model.cursor_buffer_position();
            if pos > 0 {
                let deleted_char: String = model
                    .document()
                    .buffer
                    .slice(pos - 1..pos)
                    .chars()
                    .collect();
                model.document_mut().buffer.remove(pos - 1..pos);
                model.set_cursor_from_position(pos - 1);
                model.ensure_cursor_visible();

                let cursor_after = *model.editor().cursor();
                model.document_mut().push_edit(EditOperation::Delete {
                    position: pos - 1,
                    text: deleted_char,
                    cursor_before,
                    cursor_after,
                });
            }

            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        DocumentMsg::DeleteForward => {
            let cursor_before = *model.editor().cursor();

            // Multi-cursor: process all cursors in reverse document order
            if model.editor().has_multiple_cursors() {
                let indices = cursors_in_reverse_order(model);

                for idx in indices {
                    let selection = model.editor().selections[idx].clone();
                    if !selection.is_empty() {
                        let start = selection.start();
                        let end = selection.end();
                        let start_offset =
                            model.document().cursor_to_offset(start.line, start.column);
                        let end_offset = model.document().cursor_to_offset(end.line, end.column);
                        model.document_mut().buffer.remove(start_offset..end_offset);
                        model.editor_mut().cursors[idx].line = start.line;
                        model.editor_mut().cursors[idx].column = start.column;
                        model.editor_mut().selections[idx] = Selection::new(start);
                    } else {
                        let cursor = model.editor().cursors[idx].clone();
                        let pos = model
                            .document()
                            .cursor_to_offset(cursor.line, cursor.column);
                        if pos < model.document().buffer.len_chars() {
                            model.document_mut().buffer.remove(pos..pos + 1);
                        }
                    }
                }

                model.document_mut().is_modified = true;
                model.reset_cursor_blink();
                return Some(Cmd::Redraw);
            }

            // Single cursor: check for selection
            if let Some((pos, deleted_text)) = delete_selection(model) {
                let cursor_after = *model.editor().cursor();
                model.document_mut().push_edit(EditOperation::Delete {
                    position: pos,
                    text: deleted_text,
                    cursor_before,
                    cursor_after,
                });
                model.document_mut().is_modified = true;
                model.reset_cursor_blink();
                return Some(Cmd::Redraw);
            }

            let pos = model.cursor_buffer_position();
            if pos < model.document().buffer.len_chars() {
                let deleted_char: String = model
                    .document()
                    .buffer
                    .slice(pos..pos + 1)
                    .chars()
                    .collect();
                model.document_mut().buffer.remove(pos..pos + 1);

                let cursor_after = *model.editor().cursor();
                model.document_mut().push_edit(EditOperation::Delete {
                    position: pos,
                    text: deleted_char,
                    cursor_before,
                    cursor_after,
                });
            }

            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        DocumentMsg::DeleteLine => {
            let cursor_before = *model.editor().cursor();
            let line_idx = model.editor().cursor().line;
            let total_lines = model.document().line_count();

            if total_lines == 0 {
                return Some(Cmd::Redraw);
            }

            // Calculate the range to delete
            let (start_offset, end_offset) = if line_idx + 1 < total_lines {
                // Not the last line: delete from start of line to start of next line
                let start = model.document().cursor_to_offset(line_idx, 0);
                let end = model.document().cursor_to_offset(line_idx + 1, 0);
                (start, end)
            } else if line_idx > 0 {
                // Last line but not the only line: delete preceding newline + content
                // The newline is at the end of the previous line
                let prev_line_len = model.document().line_length(line_idx - 1);
                let start = model
                    .document()
                    .cursor_to_offset(line_idx - 1, prev_line_len);
                let end = model.document().buffer.len_chars();
                (start, end)
            } else {
                // Only line: delete everything
                let start = 0;
                let end = model.document().buffer.len_chars();
                (start, end)
            };

            if start_offset < end_offset {
                // Determine if we're deleting the last line (which means cursor goes to prev line end)
                let was_last_line = line_idx + 1 >= total_lines && line_idx > 0;

                let deleted: String = model
                    .document()
                    .buffer
                    .slice(start_offset..end_offset)
                    .chars()
                    .collect();
                model.document_mut().buffer.remove(start_offset..end_offset);

                // Adjust cursor position
                let new_line_count = model.document().line_count();
                if new_line_count == 0 {
                    model.editor_mut().cursor_mut().line = 0;
                    model.editor_mut().cursor_mut().column = 0;
                } else if was_last_line {
                    // Deleted last line: cursor goes to end of previous line
                    model.editor_mut().cursor_mut().line = line_idx.saturating_sub(1);
                    let line_len = model.document().line_length(model.editor().cursor().line);
                    model.editor_mut().cursor_mut().column = line_len;
                } else {
                    // Deleted non-last line: cursor stays at same line index, clamped to valid range
                    let new_line = line_idx.min(new_line_count.saturating_sub(1));
                    let new_line_len = model.document().line_length(new_line);

                    // If the new line is empty (e.g., trailing empty line after \n),
                    // move cursor to end of previous line instead
                    if new_line_len == 0 && new_line > 0 {
                        model.editor_mut().cursor_mut().line = new_line.saturating_sub(1);
                        let prev_line_len =
                            model.document().line_length(model.editor().cursor().line);
                        model.editor_mut().cursor_mut().column = prev_line_len;
                    } else {
                        model.editor_mut().cursor_mut().line = new_line;
                        model.editor_mut().cursor_mut().column =
                            model.editor().cursor().column.min(new_line_len);
                    }
                }

                let cursor_after = *model.editor().cursor();
                model.document_mut().push_edit(EditOperation::Delete {
                    position: start_offset,
                    text: deleted,
                    cursor_before,
                    cursor_after,
                });

                model.document_mut().is_modified = true;
            }

            model.ensure_cursor_visible();
            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        DocumentMsg::Undo => {
            if let Some(edit) = model.document_mut().undo_stack.pop() {
                match &edit {
                    EditOperation::Insert {
                        position,
                        text,
                        cursor_before,
                        ..
                    } => {
                        model
                            .document_mut()
                            .buffer
                            .remove(*position..*position + text.chars().count());
                        *model.editor_mut().cursor_mut() = *cursor_before;
                    }
                    EditOperation::Delete {
                        position,
                        text,
                        cursor_before,
                        ..
                    } => {
                        model.document_mut().buffer.insert(*position, text);
                        *model.editor_mut().cursor_mut() = *cursor_before;
                    }
                    EditOperation::Replace {
                        position,
                        deleted_text,
                        inserted_text,
                        cursor_before,
                        ..
                    } => {
                        model
                            .document_mut()
                            .buffer
                            .remove(*position..*position + inserted_text.chars().count());
                        model.document_mut().buffer.insert(*position, deleted_text);
                        *model.editor_mut().cursor_mut() = *cursor_before;
                    }
                }
                model.document_mut().redo_stack.push(edit);
                model.document_mut().is_modified = true;
                model.editor_mut().clear_selection();
                model.ensure_cursor_visible();
                model.reset_cursor_blink();
            }
            Some(Cmd::Redraw)
        }

        DocumentMsg::Redo => {
            if let Some(edit) = model.document_mut().redo_stack.pop() {
                match &edit {
                    EditOperation::Insert {
                        position,
                        text,
                        cursor_after,
                        ..
                    } => {
                        model.document_mut().buffer.insert(*position, text);
                        *model.editor_mut().cursor_mut() = *cursor_after;
                    }
                    EditOperation::Delete {
                        position,
                        text,
                        cursor_after,
                        ..
                    } => {
                        model
                            .document_mut()
                            .buffer
                            .remove(*position..*position + text.chars().count());
                        *model.editor_mut().cursor_mut() = *cursor_after;
                    }
                    EditOperation::Replace {
                        position,
                        deleted_text,
                        inserted_text,
                        cursor_after,
                        ..
                    } => {
                        model
                            .document_mut()
                            .buffer
                            .remove(*position..*position + deleted_text.chars().count());
                        model.document_mut().buffer.insert(*position, inserted_text);
                        *model.editor_mut().cursor_mut() = *cursor_after;
                    }
                }
                model.document_mut().undo_stack.push(edit);
                model.document_mut().is_modified = true;
                model.editor_mut().clear_selection();
                model.ensure_cursor_visible();
                model.reset_cursor_blink();
            }
            Some(Cmd::Redraw)
        }

        DocumentMsg::Copy => {
            let mut text_to_copy = String::new();

            if model.editor().has_multiple_cursors() {
                // Multi-cursor: collect text from each selection
                let mut parts = Vec::new();
                for (idx, selection) in model.editor().selections.iter().enumerate() {
                    if !selection.is_empty() {
                        let start = selection.start();
                        let end = selection.end();
                        let start_offset =
                            model.document().cursor_to_offset(start.line, start.column);
                        let end_offset = model.document().cursor_to_offset(end.line, end.column);
                        let text: String = model
                            .document()
                            .buffer
                            .slice(start_offset..end_offset)
                            .chars()
                            .collect();
                        parts.push((idx, text));
                    }
                }
                // Join with newlines for clipboard
                text_to_copy = parts
                    .into_iter()
                    .map(|(_, t)| t)
                    .collect::<Vec<_>>()
                    .join("\n");
            } else {
                let selection = model.editor().selection().clone();
                if !selection.is_empty() {
                    let start = selection.start();
                    let end = selection.end();
                    let start_offset = model.document().cursor_to_offset(start.line, start.column);
                    let end_offset = model.document().cursor_to_offset(end.line, end.column);
                    text_to_copy = model
                        .document()
                        .buffer
                        .slice(start_offset..end_offset)
                        .chars()
                        .collect();
                }
            }

            if !text_to_copy.is_empty() {
                if let Ok(mut clipboard) = arboard::Clipboard::new() {
                    let _ = clipboard.set_text(&text_to_copy);
                }
                model
                    .ui
                    .set_status(format!("Copied {} chars", text_to_copy.len()));
            }

            Some(Cmd::Redraw)
        }

        DocumentMsg::Cut => {
            let mut text_to_copy = String::new();
            let has_selection;

            if model.editor().has_multiple_cursors() {
                // Collect text from selections
                let mut parts = Vec::new();
                for selection in model.editor().selections.iter() {
                    if !selection.is_empty() {
                        let start = selection.start();
                        let end = selection.end();
                        let start_offset =
                            model.document().cursor_to_offset(start.line, start.column);
                        let end_offset = model.document().cursor_to_offset(end.line, end.column);
                        let text: String = model
                            .document()
                            .buffer
                            .slice(start_offset..end_offset)
                            .chars()
                            .collect();
                        parts.push(text);
                    }
                }
                text_to_copy = parts.join("\n");
                has_selection = !text_to_copy.is_empty();
            } else {
                let selection = model.editor().selection().clone();
                has_selection = !selection.is_empty();
                if has_selection {
                    let start = selection.start();
                    let end = selection.end();
                    let start_offset = model.document().cursor_to_offset(start.line, start.column);
                    let end_offset = model.document().cursor_to_offset(end.line, end.column);
                    text_to_copy = model
                        .document()
                        .buffer
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
                if model.editor().has_multiple_cursors() {
                    // Delete each selection in reverse order
                    let indices = cursors_in_reverse_order(model);
                    for idx in indices {
                        let selection = model.editor().selections[idx].clone();
                        if !selection.is_empty() {
                            let start = selection.start();
                            let end = selection.end();
                            let start_offset =
                                model.document().cursor_to_offset(start.line, start.column);
                            let end_offset =
                                model.document().cursor_to_offset(end.line, end.column);
                            model.document_mut().buffer.remove(start_offset..end_offset);
                            model.editor_mut().cursors[idx].line = start.line;
                            model.editor_mut().cursors[idx].column = start.column;
                            model.editor_mut().selections[idx] = Selection::new(start);
                        }
                    }
                } else {
                    delete_selection(model);
                }
                model.document_mut().is_modified = true;
                model
                    .ui
                    .set_status(format!("Cut {} chars", text_to_copy.len()));
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

                let cursor_before = *model.editor().cursor();

                if model.editor().has_multiple_cursors() {
                    let lines: Vec<&str> = text.lines().collect();
                    let cursor_count = model.editor().cursors.len();

                    // If clipboard has same number of lines as cursors, distribute one per cursor
                    if lines.len() == cursor_count {
                        let indices = cursors_in_reverse_order(model);
                        for (i, idx) in indices.iter().enumerate() {
                            let line_to_paste = lines[cursor_count - 1 - i]; // Reverse order
                            let cursor = model.editor().cursors[*idx].clone();
                            let pos = model
                                .document()
                                .cursor_to_offset(cursor.line, cursor.column);
                            model.document_mut().buffer.insert(pos, line_to_paste);
                            model.editor_mut().cursors[*idx].column +=
                                line_to_paste.chars().count();
                            let new_pos = model.editor().cursors[*idx].to_position();
                            model.editor_mut().selections[*idx] = Selection::new(new_pos);
                        }
                    } else {
                        // Paste full text at each cursor
                        let indices = cursors_in_reverse_order(model);
                        for idx in indices {
                            let cursor = model.editor().cursors[idx].clone();
                            let pos = model
                                .document()
                                .cursor_to_offset(cursor.line, cursor.column);
                            model.document_mut().buffer.insert(pos, &text);

                            // Update cursor position (move to end of pasted text)
                            let new_offset = pos + text.chars().count();
                            let (new_line, new_col) = model.document().offset_to_cursor(new_offset);
                            model.editor_mut().cursors[idx].line = new_line;
                            model.editor_mut().cursors[idx].column = new_col;
                            let new_pos = model.editor().cursors[idx].to_position();
                            model.editor_mut().selections[idx] = Selection::new(new_pos);
                        }
                    }
                } else {
                    // Single cursor: use Replace if selection exists for atomic undo
                    if !model.editor().selection().is_empty() {
                        let (pos, deleted_text) = delete_selection(model).unwrap();

                        model.document_mut().buffer.insert(pos, &text);

                        // Move cursor to end of pasted text
                        let new_offset = pos + text.chars().count();
                        model.set_cursor_from_position(new_offset);

                        let cursor_after = *model.editor().cursor();
                        model.document_mut().push_edit(EditOperation::Replace {
                            position: pos,
                            deleted_text,
                            inserted_text: text.clone(),
                            cursor_before,
                            cursor_after,
                        });
                    } else {
                        let pos = model.cursor_buffer_position();

                        model.document_mut().buffer.insert(pos, &text);

                        // Move cursor to end of pasted text
                        let new_offset = pos + text.chars().count();
                        model.set_cursor_from_position(new_offset);

                        let cursor_after = *model.editor().cursor();
                        model.document_mut().push_edit(EditOperation::Insert {
                            position: pos,
                            text: text.clone(),
                            cursor_before,
                            cursor_after,
                        });
                    }
                }

                model.document_mut().is_modified = true;
                model.ui.set_status(format!("Pasted {} chars", text.len()));
                model.ensure_cursor_visible();
                model.reset_cursor_blink();
            }

            Some(Cmd::Redraw)
        }

        DocumentMsg::Duplicate => {
            let cursor_before = *model.editor().cursor();
            let selection = model.editor().selection().clone();

            if selection.is_empty() {
                // No selection: duplicate the current line
                let line_idx = model.editor().cursor().line;
                let column = model.editor().cursor().column;

                // Get the current line content
                let line_text = model.document().get_line(line_idx).unwrap_or_default();
                let has_newline = line_text.ends_with('\n');

                // Calculate insert position (end of current line)
                let line_end_offset = if has_newline {
                    // Insert after the newline
                    model.document().cursor_to_offset(line_idx + 1, 0)
                } else {
                    // No newline - insert at end with a newline prefix
                    model
                        .document()
                        .cursor_to_offset(line_idx, model.document().line_length(line_idx))
                };

                // Text to insert: for lines with newline, just the line content
                // For lines without, prefix with newline
                let text_to_insert = if has_newline {
                    line_text.clone()
                } else {
                    format!("\n{}", line_text)
                };

                model
                    .document_mut()
                    .buffer
                    .insert(line_end_offset, &text_to_insert);

                // Move cursor to duplicated line at same column
                model.editor_mut().cursor_mut().line += 1;
                let new_line = model.editor().cursor().line;
                model.editor_mut().cursor_mut().column =
                    column.min(model.document().line_length(new_line));

                let cursor_after = *model.editor().cursor();
                model.document_mut().push_edit(EditOperation::Insert {
                    position: line_end_offset,
                    text: text_to_insert,
                    cursor_before,
                    cursor_after,
                });
            } else {
                // With selection: duplicate the selected text after selection end
                let sel_start = selection.start();
                let sel_end = selection.end();

                let start_offset = model
                    .document()
                    .cursor_to_offset(sel_start.line, sel_start.column);
                let end_offset = model
                    .document()
                    .cursor_to_offset(sel_end.line, sel_end.column);

                // Get selected text
                let selected_text: String = model
                    .document()
                    .buffer
                    .slice(start_offset..end_offset)
                    .chars()
                    .collect();

                // Insert at end of selection
                model
                    .document_mut()
                    .buffer
                    .insert(end_offset, &selected_text);

                // Move cursor to end of duplicated text
                let new_offset = end_offset + selected_text.chars().count();
                model.set_cursor_from_position(new_offset);

                // Clear selection
                model.editor_mut().clear_selection();

                let cursor_after = *model.editor().cursor();
                model.document_mut().push_edit(EditOperation::Insert {
                    position: end_offset,
                    text: selected_text,
                    cursor_before,
                    cursor_after,
                });
            }

            model.document_mut().is_modified = true;
            model.ensure_cursor_visible();
            model.reset_cursor_blink();
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
            let transient = TransientMessage::new(text.clone(), Duration::from_millis(duration_ms));
            model.ui.transient_message = Some(transient);
            // Also update the StatusMessage segment
            model
                .ui
                .status_bar
                .update_segment(SegmentId::StatusMessage, SegmentContent::Text(text));
            Some(Cmd::Redraw)
        }

        UiMsg::ClearTransientMessage => {
            model.ui.transient_message = None;
            model
                .ui
                .status_bar
                .update_segment(SegmentId::StatusMessage, SegmentContent::Empty);
            Some(Cmd::Redraw)
        }
    }
}

/// Handle layout messages (split views, tabs, groups)
pub fn update_layout(model: &mut AppModel, msg: LayoutMsg) -> Option<Cmd> {
    match msg {
        LayoutMsg::SplitFocused(direction) => {
            split_focused_group(model, direction);
            Some(Cmd::Redraw)
        }

        LayoutMsg::SplitGroup {
            group_id,
            direction,
        } => {
            split_group(model, group_id, direction);
            Some(Cmd::Redraw)
        }

        LayoutMsg::CloseGroup(group_id) => {
            close_group(model, group_id);
            Some(Cmd::Redraw)
        }

        LayoutMsg::CloseFocusedGroup => {
            let group_id = model.editor_area.focused_group_id;
            close_group(model, group_id);
            Some(Cmd::Redraw)
        }

        LayoutMsg::FocusGroup(group_id) => {
            if model.editor_area.groups.contains_key(&group_id) {
                model.editor_area.focused_group_id = group_id;
            }
            Some(Cmd::Redraw)
        }

        LayoutMsg::FocusNextGroup => {
            focus_adjacent_group(model, true);
            Some(Cmd::Redraw)
        }

        LayoutMsg::FocusPrevGroup => {
            focus_adjacent_group(model, false);
            Some(Cmd::Redraw)
        }

        LayoutMsg::FocusGroupByIndex(index) => {
            // 1-indexed for keyboard shortcuts (Cmd+1, Cmd+2, etc.)
            let group_ids: Vec<GroupId> = collect_group_ids(&model.editor_area.layout);
            if index > 0 && index <= group_ids.len() {
                model.editor_area.focused_group_id = group_ids[index - 1];
            }
            Some(Cmd::Redraw)
        }

        LayoutMsg::MoveTab { tab_id, to_group } => {
            move_tab(model, tab_id, to_group);
            Some(Cmd::Redraw)
        }

        LayoutMsg::CloseTab(tab_id) => {
            close_tab(model, tab_id);
            Some(Cmd::Redraw)
        }

        LayoutMsg::CloseFocusedTab => {
            if let Some(tab) = model
                .editor_area
                .focused_group()
                .and_then(|g| g.active_tab())
            {
                let tab_id = tab.id;
                close_tab(model, tab_id);
            }
            Some(Cmd::Redraw)
        }

        LayoutMsg::NextTab => {
            if let Some(group) = model.editor_area.focused_group_mut() {
                if !group.tabs.is_empty() {
                    group.active_tab_index = (group.active_tab_index + 1) % group.tabs.len();
                }
            }
            Some(Cmd::Redraw)
        }

        LayoutMsg::PrevTab => {
            if let Some(group) = model.editor_area.focused_group_mut() {
                if !group.tabs.is_empty() {
                    group.active_tab_index = if group.active_tab_index == 0 {
                        group.tabs.len() - 1
                    } else {
                        group.active_tab_index - 1
                    };
                }
            }
            Some(Cmd::Redraw)
        }

        LayoutMsg::SwitchToTab(index) => {
            if let Some(group) = model.editor_area.focused_group_mut() {
                if index < group.tabs.len() {
                    group.active_tab_index = index;
                }
            }
            Some(Cmd::Redraw)
        }
    }
}

// ============================================================================
// Layout Helper Functions
// ============================================================================

/// Split the focused group in the given direction
fn split_focused_group(model: &mut AppModel, direction: SplitDirection) {
    let group_id = model.editor_area.focused_group_id;
    split_group(model, group_id, direction);
}

/// Split a specific group in the given direction
fn split_group(model: &mut AppModel, group_id: GroupId, direction: SplitDirection) {
    // Get the document ID from the active tab in the group to split
    let doc_id = {
        let group = match model.editor_area.groups.get(&group_id) {
            Some(g) => g,
            None => return,
        };
        let editor_id = match group.active_editor_id() {
            Some(id) => id,
            None => return,
        };
        match model.editor_area.editors.get(&editor_id) {
            Some(e) => match e.document_id {
                Some(id) => id,
                None => return,
            },
            None => return,
        }
    };

    // Create a new editor for the same document
    let new_editor_id = model.editor_area.next_editor_id();
    let new_editor = {
        let mut editor = EditorState::new();
        editor.id = Some(new_editor_id);
        editor.document_id = Some(doc_id);
        editor
    };
    model.editor_area.editors.insert(new_editor_id, new_editor);

    // Create a new tab for the new editor
    let new_tab_id = model.editor_area.next_tab_id();
    let new_tab = Tab {
        id: new_tab_id,
        editor_id: new_editor_id,
        is_pinned: false,
        is_preview: false,
    };

    // Create a new group with the new tab
    let new_group_id = model.editor_area.next_group_id();
    let new_group = EditorGroup {
        id: new_group_id,
        tabs: vec![new_tab],
        active_tab_index: 0,
        rect: Default::default(),
    };
    model.editor_area.groups.insert(new_group_id, new_group);

    // Update the layout tree to include the new group
    insert_split_in_layout(
        &mut model.editor_area.layout,
        group_id,
        new_group_id,
        direction,
    );

    // Focus the new group
    model.editor_area.focused_group_id = new_group_id;
}

/// Insert a split into the layout tree, replacing the target group with a split container
fn insert_split_in_layout(
    layout: &mut LayoutNode,
    target_group: GroupId,
    new_group: GroupId,
    direction: SplitDirection,
) {
    match layout {
        LayoutNode::Group(id) if *id == target_group => {
            // Replace this group with a split containing both groups
            *layout = LayoutNode::Split(SplitContainer {
                direction,
                children: vec![
                    LayoutNode::Group(target_group),
                    LayoutNode::Group(new_group),
                ],
                ratios: vec![0.5, 0.5],
                min_sizes: vec![100.0, 100.0],
            });
        }
        LayoutNode::Group(_) => {
            // Not the target group, nothing to do
        }
        LayoutNode::Split(container) => {
            // Recursively search children
            for child in &mut container.children {
                insert_split_in_layout(child, target_group, new_group, direction);
            }
        }
    }
}

/// Close a group and remove it from the layout
fn close_group(model: &mut AppModel, group_id: GroupId) {
    // Don't close the last group
    if model.editor_area.groups.len() <= 1 {
        return;
    }

    // Remove the group from the layout tree
    let removed = remove_group_from_layout(&mut model.editor_area.layout, group_id);
    if !removed {
        return;
    }

    // Clean up the group's tabs and editors
    if let Some(group) = model.editor_area.groups.remove(&group_id) {
        for tab in group.tabs {
            model.editor_area.editors.remove(&tab.editor_id);
        }
    }

    // If we closed the focused group, focus another group
    if model.editor_area.focused_group_id == group_id {
        let group_ids: Vec<GroupId> = collect_group_ids(&model.editor_area.layout);
        if let Some(&new_focus) = group_ids.first() {
            model.editor_area.focused_group_id = new_focus;
        }
    }
}

/// Remove a group from the layout tree, collapsing splits as needed
/// Returns true if the group was found and removed
fn remove_group_from_layout(layout: &mut LayoutNode, group_id: GroupId) -> bool {
    match layout {
        LayoutNode::Group(id) => {
            // Can't remove at this level - parent needs to handle it
            *id == group_id
        }
        LayoutNode::Split(container) => {
            // Find and remove the group from children
            let mut found_index = None;
            for (i, child) in container.children.iter().enumerate() {
                if let LayoutNode::Group(id) = child {
                    if *id == group_id {
                        found_index = Some(i);
                        break;
                    }
                }
            }

            if let Some(index) = found_index {
                container.children.remove(index);
                container.ratios.remove(index);
                if !container.min_sizes.is_empty() {
                    container
                        .min_sizes
                        .remove(index.min(container.min_sizes.len() - 1));
                }

                // Normalize ratios
                let sum: f32 = container.ratios.iter().sum();
                if sum > 0.0 {
                    for ratio in &mut container.ratios {
                        *ratio /= sum;
                    }
                }

                // If only one child remains, collapse the split
                if container.children.len() == 1 {
                    let remaining = container.children.remove(0);
                    *layout = remaining;
                }

                return true;
            }

            // Recursively search children
            for child in &mut container.children {
                if remove_group_from_layout(child, group_id) {
                    // Check if we need to collapse after recursive removal
                    if let LayoutNode::Split(inner) = child {
                        if inner.children.len() == 1 {
                            let remaining = inner.children.remove(0);
                            *child = remaining;
                        }
                    }
                    return true;
                }
            }

            false
        }
    }
}

/// Collect all group IDs from the layout tree (in order)
fn collect_group_ids(layout: &LayoutNode) -> Vec<GroupId> {
    match layout {
        LayoutNode::Group(id) => vec![*id],
        LayoutNode::Split(container) => container
            .children
            .iter()
            .flat_map(collect_group_ids)
            .collect(),
    }
}

/// Focus the next or previous group
fn focus_adjacent_group(model: &mut AppModel, next: bool) {
    let group_ids = collect_group_ids(&model.editor_area.layout);
    if group_ids.len() <= 1 {
        return;
    }

    let current_idx = group_ids
        .iter()
        .position(|&id| id == model.editor_area.focused_group_id)
        .unwrap_or(0);

    let new_idx = if next {
        (current_idx + 1) % group_ids.len()
    } else {
        if current_idx == 0 {
            group_ids.len() - 1
        } else {
            current_idx - 1
        }
    };

    model.editor_area.focused_group_id = group_ids[new_idx];
}

/// Move a tab to a different group
fn move_tab(model: &mut AppModel, tab_id: crate::model::TabId, to_group: GroupId) {
    // Find and remove the tab from its current group
    let mut tab_to_move = None;
    let mut source_group_id = None;

    for (gid, group) in &mut model.editor_area.groups {
        if let Some(idx) = group.tabs.iter().position(|t| t.id == tab_id) {
            tab_to_move = Some(group.tabs.remove(idx));
            source_group_id = Some(*gid);
            // Adjust active tab index if needed
            if group.active_tab_index >= group.tabs.len() && !group.tabs.is_empty() {
                group.active_tab_index = group.tabs.len() - 1;
            }
            break;
        }
    }

    // Add the tab to the target group
    if let (Some(tab), Some(_source)) = (tab_to_move, source_group_id) {
        if let Some(target_group) = model.editor_area.groups.get_mut(&to_group) {
            target_group.tabs.push(tab);
            target_group.active_tab_index = target_group.tabs.len() - 1;
        }
    }
}

/// Close a specific tab
fn close_tab(model: &mut AppModel, tab_id: crate::model::TabId) {
    // Find the tab and its group
    let mut found = None;
    for (gid, group) in &model.editor_area.groups {
        if let Some(idx) = group.tabs.iter().position(|t| t.id == tab_id) {
            found = Some((*gid, idx));
            break;
        }
    }

    let (group_id, tab_idx) = match found {
        Some(f) => f,
        None => return,
    };

    // Get editor_id before removing
    let editor_id = model.editor_area.groups[&group_id].tabs[tab_idx].editor_id;

    // Remove the tab
    if let Some(group) = model.editor_area.groups.get_mut(&group_id) {
        group.tabs.remove(tab_idx);
        if group.active_tab_index >= group.tabs.len() && !group.tabs.is_empty() {
            group.active_tab_index = group.tabs.len() - 1;
        }
    }

    // Remove the editor
    model.editor_area.editors.remove(&editor_id);

    // If the group is now empty, close it (unless it's the last group)
    if model
        .editor_area
        .groups
        .get(&group_id)
        .map_or(false, |g| g.tabs.is_empty())
    {
        if model.editor_area.groups.len() > 1 {
            close_group(model, group_id);
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

        AppMsg::SaveFile => {
            let file_path = model.document().file_path.clone();
            match file_path {
                Some(path) => {
                    let content = model.document().buffer.to_string();
                    model.ui.is_saving = true;
                    model.ui.set_status("Saving...");
                    Some(Cmd::SaveFile { path, content })
                }
                None => {
                    model.ui.set_status("No file path - cannot save");
                    Some(Cmd::Redraw)
                }
            }
        }

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
                    model.document_mut().is_modified = false;
                    if let Some(path) = &model.document().file_path {
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
                    model.document_mut().buffer = ropey::Rope::from(content);
                    model.document_mut().file_path = Some(path.clone());
                    model.document_mut().is_modified = false;
                    model.document_mut().undo_stack.clear();
                    model.document_mut().redo_stack.clear();
                    *model.editor_mut().cursor_mut() = Default::default();
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
    if model.editor().cursor().line > 0 {
        model.editor_mut().cursor_mut().line -= 1;

        let desired = model
            .editor()
            .cursor()
            .desired_column
            .unwrap_or(model.editor().cursor().column);
        let line_len = model.current_line_length();
        model.editor_mut().cursor_mut().column = desired.min(line_len);
        model.editor_mut().cursor_mut().desired_column = Some(desired);

        let padding = model.editor().scroll_padding;
        let top_boundary = model.editor().viewport.top_line + padding;
        let cursor_line = model.editor().cursor().line;

        if cursor_line < top_boundary && model.editor().viewport.top_line > 0 {
            model.editor_mut().viewport.top_line = cursor_line.saturating_sub(padding);
        }
    }
}

fn move_cursor_down(model: &mut AppModel) {
    if model.editor().cursor().line < model.document().line_count().saturating_sub(1) {
        model.editor_mut().cursor_mut().line += 1;

        let desired = model
            .editor()
            .cursor()
            .desired_column
            .unwrap_or(model.editor().cursor().column);
        let line_len = model.current_line_length();
        model.editor_mut().cursor_mut().column = desired.min(line_len);
        model.editor_mut().cursor_mut().desired_column = Some(desired);

        let padding = model.editor().scroll_padding;
        let top_line = model.editor().viewport.top_line;
        let visible_lines = model.editor().viewport.visible_lines;
        let bottom_boundary = top_line
            .saturating_add(visible_lines)
            .saturating_sub(padding)
            .saturating_sub(1);
        let max_top = model.document().line_count().saturating_sub(visible_lines);
        let cursor_line = model.editor().cursor().line;

        if cursor_line > bottom_boundary && model.editor().viewport.top_line < max_top {
            let desired_top = cursor_line + padding + 1;
            model.editor_mut().viewport.top_line =
                desired_top.saturating_sub(visible_lines).min(max_top);
        }
    }
}

fn move_cursor_left(model: &mut AppModel) {
    if model.editor().cursor().column > 0 {
        model.editor_mut().cursor_mut().column -= 1;
        model.editor_mut().cursor_mut().desired_column = None;
    } else if model.editor().cursor().line > 0 {
        model.editor_mut().cursor_mut().line -= 1;
        model.editor_mut().cursor_mut().column = model.current_line_length();
        model.editor_mut().cursor_mut().desired_column = None;
    }
    model.ensure_cursor_visible();
}

fn move_cursor_right(model: &mut AppModel) {
    let line_len = model.current_line_length();
    if model.editor().cursor().column < line_len {
        model.editor_mut().cursor_mut().column += 1;
        model.editor_mut().cursor_mut().desired_column = None;
    } else if model.editor().cursor().line < model.document().line_count().saturating_sub(1) {
        model.editor_mut().cursor_mut().line += 1;
        model.editor_mut().cursor_mut().column = 0;
        model.editor_mut().cursor_mut().desired_column = None;
    }
    model.ensure_cursor_visible();
}

fn move_cursor_word_left(model: &mut AppModel) {
    let pos = model.cursor_buffer_position();
    if pos == 0 {
        return;
    }

    let text: String = model.document().buffer.slice(..pos).chars().collect();
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
    let total_chars = model.document().buffer.len_chars();
    if pos >= total_chars {
        return;
    }

    let text: String = model.document().buffer.slice(pos..).chars().collect();
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
