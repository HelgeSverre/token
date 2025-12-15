//! Editor update functions for cursor movement, selection, and viewport scrolling.

use std::time::Duration;

use crate::commands::Cmd;
use crate::messages::{Direction, EditorMsg};
use crate::model::{
    AppModel, Cursor, OccurrenceState, Position, SegmentContent, SegmentId, Selection,
    TransientMessage,
};
use crate::util::{char_type, CharType};

/// Handle editor messages (cursor movement, viewport scrolling)
pub fn update_editor(model: &mut AppModel, msg: EditorMsg) -> Option<Cmd> {
    // Clear occurrence selection state and selection history on non-selection cursor movements
    // and selection-clearing operations (but NOT on ExpandSelection/ShrinkSelection)
    match &msg {
        EditorMsg::MoveCursor(_)
        | EditorMsg::MoveCursorLineStart
        | EditorMsg::MoveCursorLineEnd
        | EditorMsg::MoveCursorDocumentStart
        | EditorMsg::MoveCursorDocumentEnd
        | EditorMsg::MoveCursorWord(_)
        | EditorMsg::PageUp
        | EditorMsg::PageDown
        | EditorMsg::SetCursorPosition { .. }
        | EditorMsg::ClearSelection
        | EditorMsg::CollapseToSingleCursor => {
            model.editor_mut().occurrence_state = None;
            model.editor_mut().clear_selection_history();
        }
        // Also clear on selection-modifying operations (except Expand/Shrink)
        EditorMsg::MoveCursorWithSelection(_)
        | EditorMsg::MoveCursorLineStartWithSelection
        | EditorMsg::MoveCursorLineEndWithSelection
        | EditorMsg::MoveCursorDocumentStartWithSelection
        | EditorMsg::MoveCursorDocumentEndWithSelection
        | EditorMsg::MoveCursorWordWithSelection(_)
        | EditorMsg::PageUpWithSelection
        | EditorMsg::PageDownWithSelection
        | EditorMsg::SelectAll
        | EditorMsg::SelectWord
        | EditorMsg::SelectLine
        | EditorMsg::ExtendSelectionToPosition { .. }
        | EditorMsg::SelectNextOccurrence
        | EditorMsg::SelectAllOccurrences => {
            model.editor_mut().clear_selection_history();
        }
        _ => {}
    }

    match msg {
        EditorMsg::MoveCursor(direction) => {
            {
                let doc = model.document().clone();
                let editor = model.editor_mut();
                match direction {
                    Direction::Up => editor.move_all_cursors_up(&doc),
                    Direction::Down => editor.move_all_cursors_down(&doc),
                    Direction::Left => editor.move_all_cursors_left(&doc),
                    Direction::Right => editor.move_all_cursors_right(&doc),
                }
                editor.collapse_selections_to_cursors();
            }
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
            {
                let doc = model.document().clone();
                let editor = model.editor_mut();
                editor.move_all_cursors_line_start(&doc);
                editor.collapse_selections_to_cursors();
            }
            model.ensure_cursor_visible();
            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        EditorMsg::MoveCursorLineEnd => {
            {
                let doc = model.document().clone();
                let editor = model.editor_mut();
                editor.move_all_cursors_line_end(&doc);
                editor.collapse_selections_to_cursors();
            }
            model.ensure_cursor_visible();
            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        EditorMsg::MoveCursorDocumentStart => {
            {
                let editor = model.editor_mut();
                editor.move_all_cursors_document_start();
                editor.viewport.top_line = 0;
                editor.collapse_selections_to_cursors();
            }
            model.ensure_cursor_visible();
            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        EditorMsg::MoveCursorDocumentEnd => {
            {
                let doc = model.document().clone();
                let editor = model.editor_mut();
                editor.move_all_cursors_document_end(&doc);
                let cursor_line = editor.active_cursor().line;
                let visible_lines = editor.viewport.visible_lines;
                if cursor_line >= visible_lines {
                    editor.viewport.top_line = cursor_line - visible_lines + 1;
                }
                editor.collapse_selections_to_cursors();
            }
            model.ensure_cursor_visible();
            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        EditorMsg::MoveCursorWord(direction) => {
            {
                let doc = model.document().clone();
                let editor = model.editor_mut();
                match direction {
                    Direction::Left => editor.move_all_cursors_word_left(&doc),
                    Direction::Right => editor.move_all_cursors_word_right(&doc),
                    _ => {}
                }
                editor.collapse_selections_to_cursors();
            }
            model.ensure_cursor_visible();
            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        EditorMsg::PageUp => {
            let jump = model.editor().viewport.visible_lines.saturating_sub(2);
            {
                let doc = model.document().clone();
                let editor = model.editor_mut();
                editor.page_up_all_cursors(&doc, jump);
                editor.viewport.top_line = editor.viewport.top_line.saturating_sub(jump);
                editor.collapse_selections_to_cursors();
            }
            model.ensure_cursor_visible_directional(Some(true));
            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        EditorMsg::PageDown => {
            let jump = model.editor().viewport.visible_lines.saturating_sub(2);
            {
                let doc = model.document().clone();
                let editor = model.editor_mut();
                editor.page_down_all_cursors(&doc, jump);
                let cursor_line = editor.active_cursor().line;
                let top_line = editor.viewport.top_line;
                let visible_lines = editor.viewport.visible_lines;
                if cursor_line >= top_line + visible_lines {
                    editor.viewport.top_line =
                        cursor_line.saturating_sub(visible_lines.saturating_sub(1));
                }
                editor.collapse_selections_to_cursors();
            }
            model.ensure_cursor_visible_directional(Some(false));
            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        EditorMsg::SetCursorPosition { line, column } => {
            model.editor_mut().primary_cursor_mut().line = line;
            model.editor_mut().primary_cursor_mut().column = column;
            model.editor_mut().primary_cursor_mut().desired_column = None;
            model.editor_mut().collapse_selections_to_cursors();
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
            {
                let doc = model.document().clone();
                let editor = model.editor_mut();
                match direction {
                    Direction::Up => editor.move_all_cursors_up_with_selection(&doc),
                    Direction::Down => editor.move_all_cursors_down_with_selection(&doc),
                    Direction::Left => editor.move_all_cursors_left_with_selection(&doc),
                    Direction::Right => editor.move_all_cursors_right_with_selection(&doc),
                }
            }
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
            {
                let doc = model.document().clone();
                let editor = model.editor_mut();
                editor.move_all_cursors_line_start_with_selection(&doc);
            }
            model.ensure_cursor_visible();
            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        EditorMsg::MoveCursorLineEndWithSelection => {
            {
                let doc = model.document().clone();
                let editor = model.editor_mut();
                editor.move_all_cursors_line_end_with_selection(&doc);
            }
            model.ensure_cursor_visible();
            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        EditorMsg::MoveCursorDocumentStartWithSelection => {
            {
                let editor = model.editor_mut();
                editor.move_all_cursors_document_start_with_selection();
                editor.viewport.top_line = 0;
            }
            model.ensure_cursor_visible();
            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        EditorMsg::MoveCursorDocumentEndWithSelection => {
            {
                let doc = model.document().clone();
                let editor = model.editor_mut();
                editor.move_all_cursors_document_end_with_selection(&doc);
                let cursor_line = editor.active_cursor().line;
                let visible_lines = editor.viewport.visible_lines;
                if cursor_line >= visible_lines {
                    editor.viewport.top_line = cursor_line - visible_lines + 1;
                }
            }
            model.ensure_cursor_visible();
            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        EditorMsg::MoveCursorWordWithSelection(direction) => {
            {
                let doc = model.document().clone();
                let editor = model.editor_mut();
                match direction {
                    Direction::Left => editor.move_all_cursors_word_left_with_selection(&doc),
                    Direction::Right => editor.move_all_cursors_word_right_with_selection(&doc),
                    _ => {}
                }
            }
            model.ensure_cursor_visible();
            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        EditorMsg::PageUpWithSelection => {
            let jump = model.editor().viewport.visible_lines.saturating_sub(2);
            {
                let doc = model.document().clone();
                let editor = model.editor_mut();
                editor.page_up_all_cursors_with_selection(&doc, jump);
                editor.viewport.top_line = editor.viewport.top_line.saturating_sub(jump);
            }
            model.ensure_cursor_visible();
            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        EditorMsg::PageDownWithSelection => {
            let jump = model.editor().viewport.visible_lines.saturating_sub(2);
            {
                let doc = model.document().clone();
                let editor = model.editor_mut();
                editor.page_down_all_cursors_with_selection(&doc, jump);
                let cursor_line = editor.active_cursor().line;
                let top_line = editor.viewport.top_line;
                let visible_lines = editor.viewport.visible_lines;
                if cursor_line >= top_line + visible_lines {
                    editor.viewport.top_line =
                        cursor_line.saturating_sub(visible_lines.saturating_sub(1));
                }
            }
            model.ensure_cursor_visible();
            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        // === Selection Commands ===
        EditorMsg::SelectAll => {
            use crate::model::editor::Position;
            use crate::model::Cursor;

            let last_line = model.document().line_count().saturating_sub(1);
            let last_col = model.document().line_length(last_line);
            let start = Position::new(0, 0);
            let end = Position::new(last_line, last_col);

            // Collapse to single cursor + single full-document selection
            let editor = model.editor_mut();
            editor.cursors.clear();
            editor.selections.clear();
            editor.cursors.push(Cursor::from_position(end));
            editor
                .selections
                .push(Selection::from_positions(start, end));

            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        EditorMsg::SelectWord => {
            let doc = model.document().clone();
            {
                let editor = model.editor_mut();

                for i in 0..editor.cursors.len() {
                    if let Some((_word, start, end)) = editor.word_under_cursor_at(&doc, i) {
                        editor.selections[i].anchor = start;
                        editor.selections[i].head = end;
                        editor.cursors[i].line = end.line;
                        editor.cursors[i].column = end.column;
                        editor.cursors[i].desired_column = None;
                    }
                    // If no word under cursor (whitespace), leave selection unchanged
                }

                editor.merge_overlapping_selections();
            }

            model.ensure_cursor_visible();
            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        EditorMsg::SelectLine => {
            use crate::model::editor::Position;

            let total_lines = model.document().line_count();
            let doc = model.document().clone();
            {
                let editor = model.editor_mut();

                for i in 0..editor.cursors.len() {
                    let line = editor.cursors[i].line;
                    let line_len = doc.line_length(line);

                    let start = Position::new(line, 0);
                    let end = if line + 1 < total_lines {
                        // Include newline by selecting to start of next line
                        Position::new(line + 1, 0)
                    } else {
                        // Last line - select to end of line
                        Position::new(line, line_len)
                    };

                    editor.selections[i].anchor = start;
                    editor.selections[i].head = end;
                    editor.cursors[i].line = end.line;
                    editor.cursors[i].column = end.column;
                    editor.cursors[i].desired_column = None;
                }

                editor.merge_overlapping_selections();
            }

            model.ensure_cursor_visible();
            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        EditorMsg::ExtendSelectionToPosition { line, column } => {
            use crate::model::editor::Position;

            let new_pos = Position::new(line, column);
            {
                let editor = model.editor_mut();

                // If multiple cursors, collapse to primary first
                if editor.cursors.len() > 1 {
                    editor.cursors.truncate(1);
                    editor.selections.truncate(1);
                    editor.active_cursor_index = 0;
                }

                // Single selection semantics
                let sel = &mut editor.selections[0];
                let cur = &mut editor.cursors[0];

                if sel.is_empty() {
                    sel.anchor = cur.to_position();
                }
                sel.head = new_pos;

                cur.line = new_pos.line;
                cur.column = new_pos.column;
                cur.desired_column = None;
            }
            model.ensure_cursor_visible();
            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        EditorMsg::ClearSelection => {
            model.editor_mut().clear_selection();
            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        EditorMsg::CollapseToSingleCursor => {
            model.editor_mut().collapse_to_primary();
            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        // === Multi-Cursor Operations ===
        EditorMsg::AddCursorAbove => {
            let current = *model.editor().top_cursor();
            if current.line > 0 {
                let new_line = current.line - 1;
                let line_len = model.document().line_length(new_line);
                let new_col = current.column.min(line_len);
                model.editor_mut().add_cursor_at(new_line, new_col);
            }
            model.ensure_cursor_visible();
            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        EditorMsg::AddCursorBelow => {
            let current = *model.editor().bottom_cursor();
            let total_lines = model.document().line_count();

            if current.line + 1 < total_lines {
                let new_line = current.line + 1;
                let line_len = model.document().line_length(new_line);
                let new_col = current.column.min(line_len);
                model.editor_mut().add_cursor_at(new_line, new_col);
            }
            model.ensure_cursor_visible();
            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        EditorMsg::ToggleCursorAtPosition { line, column } => {
            model.editor_mut().toggle_cursor_at(line, column);
            model.ensure_cursor_visible();
            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        EditorMsg::RemoveCursor(index) => {
            let editor = model.editor_mut();
            if index < editor.cursors.len() && editor.cursors.len() > 1 {
                editor.cursors.remove(index);
                editor.selections.remove(index);
            }
            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        EditorMsg::SelectNextOccurrence => {
            // Get the search text from current selection or word under cursor
            let (search_text, just_selected_word) = {
                let selection = *model.editor().primary_selection();
                if !selection.is_empty() {
                    (selection.get_text(model.document()), false)
                } else if let Some((word, start, end)) =
                    model.editor().word_under_cursor(model.document())
                {
                    // Select the word first (for visual feedback)
                    model.editor_mut().primary_selection_mut().anchor = start;
                    model.editor_mut().primary_selection_mut().head = end;
                    model.editor_mut().primary_cursor_mut().line = end.line;
                    model.editor_mut().primary_cursor_mut().column = end.column;
                    (word, true) // Mark that we just selected the word
                } else {
                    return Some(Cmd::Redraw); // No word under cursor
                }
            };

            // If we just selected a word, stop here - don't find next occurrence yet
            // This matches standard IDE behavior (VS Code, IntelliJ)
            if just_selected_word {
                model.reset_cursor_blink();
                return Some(Cmd::Redraw);
            }

            if search_text.is_empty() {
                return Some(Cmd::Redraw);
            }

            // Compute the end offset of the current primary selection/cursor
            // This is where we start searching from on first invocation
            let primary_end_offset = {
                let sel = model.editor().primary_selection();
                if !sel.is_empty() {
                    let end = sel.end();
                    model.document().cursor_to_offset(end.line, end.column)
                } else {
                    let cur = model.editor().primary_cursor();
                    model.document().cursor_to_offset(cur.line, cur.column)
                }
            };

            // Initialize or update occurrence state
            let search_start = {
                if let Some(ref state) = model.editor().occurrence_state {
                    if state.search_text == search_text {
                        state.last_search_offset
                    } else {
                        // Different search text, start from current selection
                        model.editor_mut().occurrence_state = None;
                        primary_end_offset
                    }
                } else {
                    // First invocation: start from end of current selection
                    primary_end_offset
                }
            };

            // Find the next unselected occurrence, looping to skip already-selected ones
            let mut search_offset = search_start;
            let mut found_new = false;
            let mut iterations = 0;
            let max_iterations = model.document().buffer.len_chars() + 1; // Safety limit

            while iterations < max_iterations {
                iterations += 1;

                let Some((start_off, end_off)) = model
                    .document()
                    .find_next_occurrence(&search_text, search_offset)
                else {
                    // No occurrences at all
                    break;
                };

                let (start_line, start_col) = model.document().offset_to_cursor(start_off);
                let (end_line, end_col) = model.document().offset_to_cursor(end_off);

                // Check if this occurrence is already selected (avoid duplicates)
                let already_selected = model
                    .editor()
                    .cursors
                    .iter()
                    .any(|c| c.line == end_line && c.column == end_col);

                if !already_selected {
                    // Add new cursor and selection
                    let new_cursor = Cursor::at(end_line, end_col);
                    let new_selection = Selection::from_anchor_head(
                        Position::new(start_line, start_col),
                        Position::new(end_line, end_col),
                    );

                    let cursor_idx = model.editor().cursors.len();
                    model.editor_mut().cursors.push(new_cursor);
                    model.editor_mut().selections.push(new_selection);

                    // Update occurrence state
                    if let Some(ref mut state) = model.editor_mut().occurrence_state {
                        state.last_search_offset = end_off;
                        state.added_cursor_indices.push(cursor_idx);
                    } else {
                        model.editor_mut().occurrence_state = Some(OccurrenceState {
                            search_text: search_text.clone(),
                            added_cursor_indices: vec![cursor_idx],
                            last_search_offset: end_off,
                        });
                    }

                    found_new = true;
                    break;
                } else {
                    // Skip this occurrence and continue searching from its end
                    search_offset = end_off;

                    // Detect wrap-around: if we've come back to or past our starting point
                    if search_offset >= search_start && iterations > 1 {
                        // We've wrapped around and all occurrences are selected
                        break;
                    }
                }
            }

            if found_new {
                // Ensure new cursor is visible
                let doc = model.document().clone();
                model.editor_mut().ensure_cursor_visible(&doc);
            } else {
                let msg = if iterations > 1 {
                    "All occurrences selected".to_string()
                } else {
                    "No occurrences found".to_string()
                };
                model.ui.transient_message = Some(TransientMessage::new(
                    msg.clone(),
                    Duration::from_millis(1500),
                ));
                model
                    .ui
                    .status_bar
                    .update_segment(SegmentId::StatusMessage, SegmentContent::Text(msg));
            }

            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        EditorMsg::UnselectOccurrence => {
            // Extract the index to remove (if any) and whether to clear state
            let (idx_to_remove, should_clear_state) = {
                let editor = model.editor_mut();
                if let Some(ref mut state) = editor.occurrence_state {
                    let idx = state.added_cursor_indices.pop();
                    let should_clear = state.added_cursor_indices.is_empty();
                    (idx, should_clear)
                } else {
                    (None, false)
                }
            };

            // Now remove the cursor/selection if we have an index
            if let Some(idx) = idx_to_remove {
                let editor = model.editor_mut();
                if idx < editor.cursors.len() && editor.cursors.len() > 1 {
                    editor.cursors.remove(idx);
                    editor.selections.remove(idx);
                }
            }

            // Clear occurrence state if needed
            if should_clear_state {
                model.editor_mut().occurrence_state = None;
            }

            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        EditorMsg::SelectAllOccurrences => {
            // Get search text from current selection or word under cursor
            let search_text = {
                let selection = *model.editor().primary_selection();
                if !selection.is_empty() {
                    selection.get_text(model.document())
                } else if let Some((word, start, end)) =
                    model.editor().word_under_cursor(model.document())
                {
                    // Select the word first (for visual feedback)
                    model.editor_mut().primary_selection_mut().anchor = start;
                    model.editor_mut().primary_selection_mut().head = end;
                    model.editor_mut().primary_cursor_mut().line = end.line;
                    model.editor_mut().primary_cursor_mut().column = end.column;
                    word
                } else {
                    return Some(Cmd::Redraw); // No word under cursor
                }
            };

            if search_text.is_empty() {
                return Some(Cmd::Redraw);
            }

            // Find all occurrences
            let occurrences = model.document().find_all_occurrences(&search_text);

            if occurrences.is_empty() {
                return Some(Cmd::Redraw);
            }

            // Build cursors and selections for all occurrences
            let mut new_cursors = Vec::new();
            let mut new_selections = Vec::new();

            for (start_off, end_off) in &occurrences {
                let (start_line, start_col) = model.document().offset_to_cursor(*start_off);
                let (end_line, end_col) = model.document().offset_to_cursor(*end_off);

                let start_pos = Position::new(start_line, start_col);
                let end_pos = Position::new(end_line, end_col);

                new_cursors.push(Cursor::at(end_line, end_col));
                new_selections.push(Selection::from_anchor_head(start_pos, end_pos));
            }

            // Replace editor state with all occurrences selected
            let editor = model.editor_mut();
            editor.cursors = new_cursors;
            editor.selections = new_selections;
            editor.deduplicate_cursors();

            // Set up occurrence state
            let cursor_count = editor.cursors.len();
            editor.occurrence_state = Some(OccurrenceState {
                search_text,
                added_cursor_indices: (0..cursor_count).collect(),
                last_search_offset: occurrences.last().map(|(_, e)| *e).unwrap_or(0),
            });

            // Show feedback
            let msg = format!("{} occurrences selected", cursor_count);
            model.ui.transient_message = Some(TransientMessage::new(
                msg.clone(),
                Duration::from_millis(1500),
            ));
            model
                .ui
                .status_bar
                .update_segment(SegmentId::StatusMessage, SegmentContent::Text(msg));

            model.ensure_cursor_visible();
            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        // === Expand/Shrink Selection ===
        EditorMsg::ExpandSelection => {
            expand_selection(model);
            model.ensure_cursor_visible();
            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        EditorMsg::ShrinkSelection => {
            shrink_selection(model);
            model.ensure_cursor_visible();
            model.reset_cursor_blink();
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
pub(crate) fn delete_selection(model: &mut AppModel) -> Option<(usize, String)> {
    let selection = *model.editor().primary_selection();
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
    model.editor_mut().primary_cursor_mut().line = sel_start.line;
    model.editor_mut().primary_cursor_mut().column = sel_start.column;
    model.editor_mut().primary_cursor_mut().desired_column = None;

    // Clear the selection
    model.editor_mut().clear_selection();

    Some((start_offset, deleted_text))
}

// ============================================================================
// Expand/Shrink Selection
// ============================================================================

/// Expand selection to next semantic level: cursor → word → line → all
/// Supports multiple cursors - each cursor/selection expands independently
fn expand_selection(model: &mut AppModel) {
    // For multi-cursor, we need to expand each selection independently
    // Exception: if ANY selection would expand to "all", we expand all to "all"
    let cursor_count = model.editor().cursors.len();

    // First pass: determine what each selection would expand to
    let mut should_select_all = false;
    let mut expansions: Vec<Option<Selection>> = Vec::with_capacity(cursor_count);

    for idx in 0..cursor_count {
        let current = model.editor().selections[idx];
        let cursor = &model.editor().cursors[idx];

        let new_selection = if current.is_empty() {
            // Level 0 → 1: Select word under cursor
            if let Some((_word, start, end)) =
                model.editor().word_under_cursor_at(model.document(), idx)
            {
                Some(Selection::from_positions(start, end))
            } else {
                // No word under cursor, try to select line
                select_line_at(model, cursor.line)
            }
        } else if is_line_selection_at(&current, model, cursor.line) {
            // Level 2 → 3: Select all
            // Check line selection BEFORE word selection because a single-word line
            // matches both patterns - we want line → all, not line → line
            should_select_all = true;
            None // Will be set to select_all below
        } else if is_word_selection_at(&current, model) {
            // Level 1 → 2: Select line
            select_line_at(model, cursor.line)
        } else {
            // Arbitrary selection: expand to line if within single line, else to all
            if is_within_single_line(&current) {
                select_line_at(model, cursor.line)
            } else {
                should_select_all = true;
                None
            }
        };

        expansions.push(new_selection);
    }

    // If any cursor needs select_all, apply to all cursors (collapse to single selection)
    if should_select_all {
        if let Some(sel) = select_all(model) {
            // Push current selections to history before expanding
            let selections_to_save: Vec<Selection> = model.editor().selections.clone();
            for selection in selections_to_save {
                model.editor_mut().selection_history.push(selection);
            }

            // Collapse to single cursor with full document selection
            model.editor_mut().cursors.truncate(1);
            model.editor_mut().selections.truncate(1);
            model.editor_mut().active_cursor_index = 0;

            model.editor_mut().selections[0].anchor = sel.anchor;
            model.editor_mut().selections[0].head = sel.head;
            model.editor_mut().cursors[0].line = sel.head.line;
            model.editor_mut().cursors[0].column = sel.head.column;
            model.editor_mut().cursors[0].desired_column = None;
        }
        return;
    }

    // Apply expansions to each cursor
    // Clone selections first to avoid borrow conflicts
    let selections_to_save: Vec<Selection> = model.editor().selections.clone();
    for idx in 0..cursor_count {
        // Push current selection to history before expanding
        model
            .editor_mut()
            .selection_history
            .push(selections_to_save[idx]);

        if let Some(sel) = &expansions[idx] {
            model.editor_mut().selections[idx].anchor = sel.anchor;
            model.editor_mut().selections[idx].head = sel.head;
            model.editor_mut().cursors[idx].line = sel.head.line;
            model.editor_mut().cursors[idx].column = sel.head.column;
            model.editor_mut().cursors[idx].desired_column = None;
        }
    }
}

/// Shrink selection to previous level (restore from history)
fn shrink_selection(model: &mut AppModel) {
    if let Some(previous) = model.editor_mut().selection_history.pop() {
        // Restore previous selection
        model.editor_mut().active_selection_mut().anchor = previous.anchor;
        model.editor_mut().active_selection_mut().head = previous.head;

        // Update cursor to match selection head
        model.editor_mut().active_cursor_mut().line = previous.head.line;
        model.editor_mut().active_cursor_mut().column = previous.head.column;
        model.editor_mut().active_cursor_mut().desired_column = None;
    } else {
        // No history - collapse selection to cursor position
        model.editor_mut().clear_selection();
    }
}

/// Check if selection exactly covers a word boundary (alias for multi-cursor compatibility)
fn is_word_selection_at(selection: &Selection, model: &AppModel) -> bool {
    if selection.start().line != selection.end().line {
        return false; // Multi-line is not a word
    }

    let line = selection.start().line;
    if let Some(line_text) = model.document().get_line(line) {
        let line_text = line_text.trim_end_matches('\n');
        let chars: Vec<char> = line_text.chars().collect();
        let start_col = selection.start().column;
        let end_col = selection.end().column;

        if start_col >= chars.len() || end_col > chars.len() || start_col >= end_col {
            return false;
        }

        // Check all chars in selection are same type (word chars)
        let first_type = char_type(chars[start_col]);
        if first_type != CharType::WordChar {
            return false;
        }

        let all_same = (start_col..end_col).all(|i| char_type(chars[i]) == first_type);
        if !all_same {
            return false;
        }

        // Check boundaries are at type transitions
        let at_word_start =
            start_col == 0 || char_type(chars[start_col.saturating_sub(1)]) != first_type;
        let at_word_end = end_col >= chars.len() || char_type(chars[end_col]) != first_type;

        at_word_start && at_word_end
    } else {
        false
    }
}

/// Check if selection covers exactly one line (including trailing newline)
/// `cursor_line` is used to verify the selection is for this specific cursor's line
fn is_line_selection_at(selection: &Selection, model: &AppModel, cursor_line: usize) -> bool {
    let start = selection.start();
    let end = selection.end();

    // Must start at column 0 and be on the cursor's line
    if start.column != 0 || start.line != cursor_line {
        return false;
    }

    // Selection must be on a single line and end at line length
    if start.line == end.line {
        let line_len = model.document().line_length(start.line);
        return end.column == line_len;
    }

    false
}

/// Check if selection is entirely within a single line
fn is_within_single_line(selection: &Selection) -> bool {
    selection.start().line == selection.end().line
}

/// Create selection covering a specific line (NOT including newline)
/// Selection ends at the last character of the line, not at the start of the next line
fn select_line_at(model: &AppModel, line: usize) -> Option<Selection> {
    let total_lines = model.document().line_count();

    if line >= total_lines {
        return None;
    }

    let start = Position::new(line, 0);
    let line_len = model.document().line_length(line);
    let end = Position::new(line, line_len);

    Some(Selection::from_positions(start, end))
}

/// Create selection covering the current line (including newline if present)
#[allow(dead_code)]
fn select_current_line(model: &AppModel) -> Option<Selection> {
    select_line_at(model, model.editor().active_cursor().line)
}

/// Create selection covering the entire document
fn select_all(model: &AppModel) -> Option<Selection> {
    let total_lines = model.document().line_count();
    if total_lines == 0 {
        return Some(Selection::new(Position::new(0, 0)));
    }

    let last_line = total_lines.saturating_sub(1);
    let last_col = model.document().line_length(last_line);

    Some(Selection::from_positions(
        Position::new(0, 0),
        Position::new(last_line, last_col),
    ))
}

/// Synchronize cursors in other editors viewing the same document after an edit.
///
/// Call this after any document modification to update cursor positions in other views.
///
/// - `edit_line`: The line where the edit started
/// - `edit_column`: The column where the edit started
/// - `lines_delta`: Change in line count (positive = lines added, negative = lines removed)
/// - `column_delta`: Change in column on the edit line (for same-line character inserts/deletes)
pub(crate) fn sync_other_editor_cursors(
    model: &mut AppModel,
    edit_line: usize,
    edit_column: usize,
    lines_delta: isize,
    column_delta: isize,
) {
    // Get the current editor and document IDs
    let editor_id = match model.editor_area.focused_editor_id() {
        Some(id) => id,
        None => return,
    };
    let doc_id = match model.editor_area.focused_document_id() {
        Some(id) => id,
        None => return,
    };

    model.editor_area.adjust_other_editors_cursors(
        editor_id,
        doc_id,
        edit_line,
        edit_column,
        lines_delta,
        column_delta,
    );
}

/// Get cursor indices sorted by position in reverse document order (last first)
pub(crate) fn cursors_in_reverse_order(model: &AppModel) -> Vec<usize> {
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

/// Get unique line indices covered by all cursors/selections, sorted in reverse document order.
/// For line-based operations (DeleteLine, Indent, etc.) that should act on each line only once.
pub(crate) fn lines_covered_by_all_cursors(model: &AppModel) -> Vec<usize> {
    use std::collections::BTreeSet;

    let editor = model.editor();
    let mut lines = BTreeSet::new();

    for (cursor, selection) in editor.cursors.iter().zip(editor.selections.iter()) {
        if selection.is_empty() {
            lines.insert(cursor.line);
        } else {
            let start = selection.start();
            let end = selection.end();
            for line in start.line..=end.line {
                lines.insert(line);
            }
        }
    }

    // Return in reverse document order (highest line first) for safe deletion
    lines.into_iter().rev().collect()
}
