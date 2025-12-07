//! Document update functions for text editing and undo/redo

use crate::commands::Cmd;
use crate::messages::DocumentMsg;
use crate::model::{AppModel, Cursor, EditOperation, Position, Selection};

use super::editor::{
    cursors_in_reverse_order, delete_selection, lines_covered_by_all_cursors,
    sync_other_editor_cursors,
};

/// Handle document messages (text editing, undo/redo)
pub fn update_document(model: &mut AppModel, msg: DocumentMsg) -> Option<Cmd> {
    // Clear occurrence selection state on any editing operation
    // (except Copy which doesn't modify the document)
    if !matches!(msg, DocumentMsg::Copy) {
        model.editor_mut().occurrence_state = None;
    }

    match msg {
        DocumentMsg::InsertChar(ch) => {
            let cursor_before = *model.editor().primary_cursor();

            // Multi-cursor: process all cursors in reverse document order
            if model.editor().has_multiple_cursors() {
                let cursors_before: Vec<Cursor> = model.editor().cursors.clone();
                let indices = cursors_in_reverse_order(model);
                let mut operations = Vec::new();

                for idx in indices {
                    // Get cursor position and convert to buffer offset
                    let cursor = model.editor().cursors[idx].clone();
                    let pos = model
                        .document()
                        .cursor_to_offset(cursor.line, cursor.column);

                    // Insert character
                    model.document_mut().buffer.insert_char(pos, ch);

                    // Record individual operation (positions are at time of insert)
                    operations.push(EditOperation::Insert {
                        position: pos,
                        text: ch.to_string(),
                        cursor_before: cursor.clone(),
                        cursor_after: Cursor::at(cursor.line, cursor.column + 1),
                    });

                    // Update this cursor's position (move right by 1)
                    model.editor_mut().cursors[idx].column += 1;
                    model.editor_mut().cursors[idx].desired_column = None;

                    // Clear this cursor's selection
                    let new_pos = model.editor().cursors[idx].to_position();
                    model.editor_mut().selections[idx] = Selection::new(new_pos);
                }

                // Record batch for proper multi-cursor undo
                // Operations are stored in application order (reverse document order)
                // Undo will iterate .rev() to process in forward document order
                let cursors_after: Vec<Cursor> = model.editor().cursors.clone();
                model.document_mut().push_edit(EditOperation::Batch {
                    operations,
                    cursors_before,
                    cursors_after,
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
                let cursor_after = *model.editor().primary_cursor();
                model.document_mut().push_edit(EditOperation::Replace {
                    position: pos,
                    deleted_text,
                    inserted_text: ch.to_string(),
                    cursor_before,
                    cursor_after,
                });
            } else {
                // No selection - normal insert
                let edit_line = cursor_before.line;
                let edit_column = cursor_before.column;
                let pos = model.cursor_buffer_position();
                model.document_mut().buffer.insert_char(pos, ch);
                model.set_cursor_from_position(pos + 1);
                model.ensure_cursor_visible();

                let cursor_after = *model.editor().primary_cursor();
                model.document_mut().push_edit(EditOperation::Insert {
                    position: pos,
                    text: ch.to_string(),
                    cursor_before,
                    cursor_after,
                });

                // Sync cursors in other views
                sync_other_editor_cursors(model, edit_line, edit_column, 0, 1);
            }

            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        DocumentMsg::InsertNewline => {
            let cursor_before = *model.editor().primary_cursor();

            // Multi-cursor: process all cursors in reverse document order
            if model.editor().has_multiple_cursors() {
                let cursors_before: Vec<Cursor> = model.editor().cursors.clone();
                let indices = cursors_in_reverse_order(model);
                let mut operations = Vec::new();

                for idx in indices {
                    let cursor = model.editor().cursors[idx].clone();
                    let pos = model
                        .document()
                        .cursor_to_offset(cursor.line, cursor.column);
                    model.document_mut().buffer.insert_char(pos, '\n');

                    // Record individual operation
                    operations.push(EditOperation::Insert {
                        position: pos,
                        text: "\n".to_string(),
                        cursor_before: cursor.clone(),
                        cursor_after: Cursor::at(cursor.line + 1, 0),
                    });

                    // Move cursor to beginning of next line
                    model.editor_mut().cursors[idx].line += 1;
                    model.editor_mut().cursors[idx].column = 0;
                    model.editor_mut().cursors[idx].desired_column = None;

                    let new_pos = model.editor().cursors[idx].to_position();
                    model.editor_mut().selections[idx] = Selection::new(new_pos);
                }

                // Record batch for proper multi-cursor undo
                // Operations stored in application order; undo iterates .rev()
                let cursors_after: Vec<Cursor> = model.editor().cursors.clone();
                model.document_mut().push_edit(EditOperation::Batch {
                    operations,
                    cursors_before,
                    cursors_after,
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

                let cursor_after = *model.editor().primary_cursor();
                model.document_mut().push_edit(EditOperation::Replace {
                    position: pos,
                    deleted_text,
                    inserted_text: "\n".to_string(),
                    cursor_before,
                    cursor_after,
                });
            } else {
                let edit_line = cursor_before.line;
                let edit_column = cursor_before.column;
                let pos = model.cursor_buffer_position();
                model.document_mut().buffer.insert_char(pos, '\n');
                model.set_cursor_from_position(pos + 1);
                model.ensure_cursor_visible();

                let cursor_after = *model.editor().primary_cursor();
                model.document_mut().push_edit(EditOperation::Insert {
                    position: pos,
                    text: "\n".to_string(),
                    cursor_before,
                    cursor_after,
                });

                // Sync cursors in other views: newline adds 1 line
                sync_other_editor_cursors(model, edit_line, edit_column, 1, 0);
            }

            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        DocumentMsg::DeleteBackward => {
            let cursor_before = *model.editor().primary_cursor();

            // Multi-cursor: process all cursors in reverse document order
            if model.editor().has_multiple_cursors() {
                let cursors_before: Vec<Cursor> = model.editor().cursors.clone();
                let indices = cursors_in_reverse_order(model);
                let mut operations = Vec::new();

                for idx in indices {
                    let selection = model.editor().selections[idx].clone();
                    if !selection.is_empty() {
                        // Delete selection
                        let start = selection.start();
                        let end = selection.end();
                        let start_offset =
                            model.document().cursor_to_offset(start.line, start.column);
                        let end_offset = model.document().cursor_to_offset(end.line, end.column);
                        let deleted_text: String = model
                            .document()
                            .buffer
                            .slice(start_offset..end_offset)
                            .to_string();
                        model.document_mut().buffer.remove(start_offset..end_offset);

                        // Record operation
                        operations.push(EditOperation::Delete {
                            position: start_offset,
                            text: deleted_text,
                            cursor_before: model.editor().cursors[idx].clone(),
                            cursor_after: Cursor::at(start.line, start.column),
                        });

                        model.editor_mut().cursors[idx].line = start.line;
                        model.editor_mut().cursors[idx].column = start.column;
                        model.editor_mut().selections[idx] = Selection::new(start);
                    } else {
                        let cursor = model.editor().cursors[idx].clone();
                        let pos = model
                            .document()
                            .cursor_to_offset(cursor.line, cursor.column);
                        if pos > 0 {
                            let deleted_char: String = model
                                .document()
                                .buffer
                                .slice(pos - 1..pos)
                                .chars()
                                .collect();
                            model.document_mut().buffer.remove(pos - 1..pos);
                            let (new_line, new_col) = model.document().offset_to_cursor(pos - 1);

                            // Record operation
                            operations.push(EditOperation::Delete {
                                position: pos - 1,
                                text: deleted_char,
                                cursor_before: cursor.clone(),
                                cursor_after: Cursor::at(new_line, new_col),
                            });

                            model.editor_mut().cursors[idx].line = new_line;
                            model.editor_mut().cursors[idx].column = new_col;
                            let new_pos = model.editor().cursors[idx].to_position();
                            model.editor_mut().selections[idx] = Selection::new(new_pos);
                        }
                    }
                }

                // Record batch for proper multi-cursor undo
                // Operations stored in application order; undo iterates .rev()
                let cursors_after: Vec<Cursor> = model.editor().cursors.clone();
                model.document_mut().push_edit(EditOperation::Batch {
                    operations,
                    cursors_before,
                    cursors_after,
                });

                model.document_mut().is_modified = true;
                model.ensure_cursor_visible();
                model.reset_cursor_blink();
                return Some(Cmd::Redraw);
            }

            // Single cursor: check for selection
            if let Some((pos, deleted_text)) = delete_selection(model) {
                let cursor_after = *model.editor().primary_cursor();
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

                // Calculate edit info for cursor sync
                let is_newline = deleted_char == "\n";
                let edit_line = cursor_before.line;
                let edit_column = cursor_before.column;

                model.document_mut().buffer.remove(pos - 1..pos);
                model.set_cursor_from_position(pos - 1);
                model.ensure_cursor_visible();

                let cursor_after = *model.editor().primary_cursor();
                model.document_mut().push_edit(EditOperation::Delete {
                    position: pos - 1,
                    text: deleted_char,
                    cursor_before,
                    cursor_after,
                });

                // Sync cursors in other views
                if is_newline {
                    // Deleted newline: removes a line, cursors on later lines shift up
                    sync_other_editor_cursors(model, edit_line.saturating_sub(1), 0, -1, 0);
                } else {
                    // Deleted character: cursors after shift left
                    sync_other_editor_cursors(
                        model,
                        edit_line,
                        edit_column.saturating_sub(1),
                        0,
                        -1,
                    );
                }
            }

            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        DocumentMsg::DeleteForward => {
            let cursor_before = *model.editor().primary_cursor();

            // Multi-cursor: process all cursors in reverse document order
            if model.editor().has_multiple_cursors() {
                let cursors_before: Vec<Cursor> = model.editor().cursors.clone();
                let indices = cursors_in_reverse_order(model);
                let mut operations = Vec::new();

                for idx in indices {
                    let selection = model.editor().selections[idx].clone();
                    if !selection.is_empty() {
                        let start = selection.start();
                        let end = selection.end();
                        let start_offset =
                            model.document().cursor_to_offset(start.line, start.column);
                        let end_offset = model.document().cursor_to_offset(end.line, end.column);
                        let deleted_text: String = model
                            .document()
                            .buffer
                            .slice(start_offset..end_offset)
                            .to_string();
                        model.document_mut().buffer.remove(start_offset..end_offset);

                        // Record operation
                        operations.push(EditOperation::Delete {
                            position: start_offset,
                            text: deleted_text,
                            cursor_before: model.editor().cursors[idx].clone(),
                            cursor_after: Cursor::at(start.line, start.column),
                        });

                        model.editor_mut().cursors[idx].line = start.line;
                        model.editor_mut().cursors[idx].column = start.column;
                        model.editor_mut().selections[idx] = Selection::new(start);
                    } else {
                        let cursor = model.editor().cursors[idx].clone();
                        let pos = model
                            .document()
                            .cursor_to_offset(cursor.line, cursor.column);
                        if pos < model.document().buffer.len_chars() {
                            let deleted_char: String = model
                                .document()
                                .buffer
                                .slice(pos..pos + 1)
                                .chars()
                                .collect();
                            model.document_mut().buffer.remove(pos..pos + 1);

                            // Record operation (cursor doesn't move for delete forward)
                            operations.push(EditOperation::Delete {
                                position: pos,
                                text: deleted_char,
                                cursor_before: cursor.clone(),
                                cursor_after: cursor.clone(),
                            });
                        }
                    }
                }

                // Record batch for proper multi-cursor undo
                // Operations stored in application order; undo iterates .rev()
                let cursors_after: Vec<Cursor> = model.editor().cursors.clone();
                model.document_mut().push_edit(EditOperation::Batch {
                    operations,
                    cursors_before,
                    cursors_after,
                });

                model.document_mut().is_modified = true;
                model.reset_cursor_blink();
                return Some(Cmd::Redraw);
            }

            // Single cursor: check for selection
            if let Some((pos, deleted_text)) = delete_selection(model) {
                let cursor_after = *model.editor().primary_cursor();
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

                // Calculate edit info for cursor sync
                let is_newline = deleted_char == "\n";
                let edit_line = cursor_before.line;
                let edit_column = cursor_before.column;

                model.document_mut().buffer.remove(pos..pos + 1);

                let cursor_after = *model.editor().primary_cursor();
                model.document_mut().push_edit(EditOperation::Delete {
                    position: pos,
                    text: deleted_char,
                    cursor_before,
                    cursor_after,
                });

                // Sync cursors in other views
                if is_newline {
                    // Deleted newline: removes a line
                    sync_other_editor_cursors(model, edit_line, edit_column, -1, 0);
                } else {
                    // Deleted character: cursors after shift left
                    sync_other_editor_cursors(model, edit_line, edit_column, 0, -1);
                }
            }

            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        DocumentMsg::DeleteLine => {
            let total_lines = model.document().line_count();
            if total_lines == 0 {
                return Some(Cmd::Redraw);
            }

            if model.editor().has_multiple_cursors() {
                let cursors_before = model.editor().cursors.clone();
                let covered_lines = lines_covered_by_all_cursors(model);
                let mut operations = Vec::new();
                let _lines_to_delete_count = covered_lines.len();

                for line_idx in covered_lines.iter().copied() {
                    let current_total = model.document().line_count();
                    if current_total == 0 || line_idx >= current_total {
                        continue;
                    }

                    let (start_offset, end_offset) = if line_idx + 1 < current_total {
                        let start = model.document().cursor_to_offset(line_idx, 0);
                        let end = model.document().cursor_to_offset(line_idx + 1, 0);
                        (start, end)
                    } else if line_idx > 0 {
                        let prev_line_len = model.document().line_length(line_idx - 1);
                        let start = model
                            .document()
                            .cursor_to_offset(line_idx - 1, prev_line_len);
                        let end = model.document().buffer.len_chars();
                        (start, end)
                    } else {
                        (0, model.document().buffer.len_chars())
                    };

                    if start_offset < end_offset {
                        let deleted: String = model
                            .document()
                            .buffer
                            .slice(start_offset..end_offset)
                            .chars()
                            .collect();
                        model.document_mut().buffer.remove(start_offset..end_offset);

                        operations.push(EditOperation::Delete {
                            position: start_offset,
                            text: deleted,
                            cursor_before: Cursor::at(line_idx, 0),
                            cursor_after: Cursor::at(line_idx.saturating_sub(1), 0),
                        });
                    }
                }

                let new_line_count = model.document().line_count();
                let min_deleted_line = covered_lines.iter().copied().min().unwrap_or(0);
                let target_line = min_deleted_line.min(new_line_count.saturating_sub(1));
                let target_col = if new_line_count > 0 {
                    model
                        .document()
                        .line_length(target_line)
                        .min(cursors_before.first().map(|c| c.column).unwrap_or(0))
                } else {
                    0
                };

                model.editor_mut().cursors = vec![Cursor::at(target_line, target_col)];
                model.editor_mut().selections =
                    vec![Selection::new(Position::new(target_line, target_col))];
                model.editor_mut().active_cursor_index = 0;

                let cursors_after = model.editor().cursors.clone();
                model.document_mut().push_edit(EditOperation::Batch {
                    operations,
                    cursors_before,
                    cursors_after,
                });

                model.document_mut().is_modified = true;

                let deleted_above_viewport = covered_lines
                    .iter()
                    .filter(|&&l| l < model.editor().viewport.top_line)
                    .count();
                if deleted_above_viewport > 0 {
                    model.editor_mut().viewport.top_line = model
                        .editor()
                        .viewport
                        .top_line
                        .saturating_sub(deleted_above_viewport);
                }

                model.ensure_cursor_visible();
                model.reset_cursor_blink();
                return Some(Cmd::Redraw);
            }

            let cursor_before = *model.editor().primary_cursor();
            let line_idx = model.editor().primary_cursor().line;

            let (start_offset, end_offset) = if line_idx + 1 < total_lines {
                let start = model.document().cursor_to_offset(line_idx, 0);
                let end = model.document().cursor_to_offset(line_idx + 1, 0);
                (start, end)
            } else if line_idx > 0 {
                let prev_line_len = model.document().line_length(line_idx - 1);
                let start = model
                    .document()
                    .cursor_to_offset(line_idx - 1, prev_line_len);
                let end = model.document().buffer.len_chars();
                (start, end)
            } else {
                (0, model.document().buffer.len_chars())
            };

            if start_offset < end_offset {
                let was_last_line = line_idx + 1 >= total_lines && line_idx > 0;

                let deleted: String = model
                    .document()
                    .buffer
                    .slice(start_offset..end_offset)
                    .chars()
                    .collect();
                model.document_mut().buffer.remove(start_offset..end_offset);

                let new_line_count = model.document().line_count();
                if new_line_count == 0 {
                    model.editor_mut().primary_cursor_mut().line = 0;
                    model.editor_mut().primary_cursor_mut().column = 0;
                } else if was_last_line {
                    model.editor_mut().primary_cursor_mut().line = line_idx.saturating_sub(1);
                    let line_len = model
                        .document()
                        .line_length(model.editor().primary_cursor().line);
                    model.editor_mut().primary_cursor_mut().column =
                        model.editor().primary_cursor().column.min(line_len);
                } else {
                    let new_line = line_idx.min(new_line_count.saturating_sub(1));
                    let new_line_len = model.document().line_length(new_line);
                    model.editor_mut().primary_cursor_mut().line = new_line;
                    model.editor_mut().primary_cursor_mut().column =
                        model.editor().primary_cursor().column.min(new_line_len);
                }

                let cursor_after = *model.editor().primary_cursor();
                model.document_mut().push_edit(EditOperation::Delete {
                    position: start_offset,
                    text: deleted,
                    cursor_before,
                    cursor_after,
                });

                model.document_mut().is_modified = true;

                if model.editor().viewport.top_line > 0 {
                    model.editor_mut().viewport.top_line -= 1;
                }
            }

            model.ensure_cursor_visible();
            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        DocumentMsg::Undo => {
            if let Some(edit) = model.document_mut().undo_stack.pop() {
                apply_undo_operation(model, &edit);
                model.document_mut().redo_stack.push(edit);
                model.document_mut().is_modified = true;
                model.editor_mut().collapse_selections_to_cursors();
                model.ensure_cursor_visible();
                model.reset_cursor_blink();
            }
            Some(Cmd::Redraw)
        }

        DocumentMsg::Redo => {
            if let Some(edit) = model.document_mut().redo_stack.pop() {
                apply_redo_operation(model, &edit);
                model.document_mut().undo_stack.push(edit);
                model.document_mut().is_modified = true;
                model.editor_mut().collapse_selections_to_cursors();
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
                let selection = model.editor().primary_selection().clone();
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
                let selection = model.editor().primary_selection().clone();
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

                let cursor_before = *model.editor().primary_cursor();

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
                    if !model.editor().primary_selection().is_empty() {
                        let (pos, deleted_text) = delete_selection(model).unwrap();

                        model.document_mut().buffer.insert(pos, &text);

                        // Move cursor to end of pasted text
                        let new_offset = pos + text.chars().count();
                        model.set_cursor_from_position(new_offset);

                        let cursor_after = *model.editor().primary_cursor();
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

                        let cursor_after = *model.editor().primary_cursor();
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
            // Multi-cursor: process all cursors in reverse document order
            if model.editor().has_multiple_cursors() {
                let cursors_before: Vec<Cursor> = model.editor().cursors.clone();
                let indices = cursors_in_reverse_order(model);
                let mut operations = Vec::new();

                // Track line offset adjustments for cursors processed later
                // (earlier in document, which we process last)
                let mut line_adjustments: Vec<(usize, usize)> = Vec::new(); // (after_line, lines_added)

                for idx in indices {
                    let selection = model.editor().selections[idx].clone();
                    let cursor = model.editor().cursors[idx].clone();

                    if selection.is_empty() {
                        // No selection: duplicate the current line
                        let line_idx = cursor.line;
                        let column = cursor.column;

                        let line_text = model.document().get_line(line_idx).unwrap_or_default();
                        let has_newline = line_text.ends_with('\n');

                        let line_end_offset = if has_newline {
                            model.document().cursor_to_offset(line_idx + 1, 0)
                        } else {
                            model
                                .document()
                                .cursor_to_offset(line_idx, model.document().line_length(line_idx))
                        };

                        let text_to_insert = if has_newline {
                            line_text.clone()
                        } else {
                            format!("\n{}", line_text)
                        };

                        model
                            .document_mut()
                            .buffer
                            .insert(line_end_offset, &text_to_insert);

                        // Count lines added
                        let lines_added = text_to_insert.chars().filter(|&c| c == '\n').count();
                        if !has_newline {
                            line_adjustments.push((line_idx, 1));
                        } else {
                            line_adjustments.push((line_idx, lines_added));
                        }

                        operations.push(EditOperation::Insert {
                            position: line_end_offset,
                            text: text_to_insert,
                            cursor_before: cursor.clone(),
                            cursor_after: Cursor::at(
                                line_idx + 1,
                                column.min(model.document().line_length(line_idx + 1)),
                            ),
                        });

                        // Move cursor to duplicated line
                        model.editor_mut().cursors[idx].line += 1;
                        let new_line_len = model.document().line_length(line_idx + 1);
                        model.editor_mut().cursors[idx].column = column.min(new_line_len);
                        model.editor_mut().cursors[idx].desired_column = None;

                        // Update selection to match cursor
                        let new_pos = model.editor().cursors[idx].to_position();
                        model.editor_mut().selections[idx] = Selection::new(new_pos);
                    } else {
                        // With selection: duplicate selected text after selection end
                        let sel_start = selection.start();
                        let sel_end = selection.end();

                        let start_offset = model
                            .document()
                            .cursor_to_offset(sel_start.line, sel_start.column);
                        let end_offset = model
                            .document()
                            .cursor_to_offset(sel_end.line, sel_end.column);

                        let selected_text: String = model
                            .document()
                            .buffer
                            .slice(start_offset..end_offset)
                            .chars()
                            .collect();

                        model
                            .document_mut()
                            .buffer
                            .insert(end_offset, &selected_text);

                        // Count lines added
                        let lines_added = selected_text.chars().filter(|&c| c == '\n').count();
                        if lines_added > 0 {
                            line_adjustments.push((sel_end.line, lines_added));
                        }

                        let new_offset = end_offset + selected_text.chars().count();
                        let (new_line, new_col) = model.document().offset_to_cursor(new_offset);

                        operations.push(EditOperation::Insert {
                            position: end_offset,
                            text: selected_text,
                            cursor_before: cursor.clone(),
                            cursor_after: Cursor::at(new_line, new_col),
                        });

                        // Move cursor to end of duplicated text, clear selection
                        model.editor_mut().cursors[idx].line = new_line;
                        model.editor_mut().cursors[idx].column = new_col;
                        model.editor_mut().cursors[idx].desired_column = None;

                        let new_pos = Position::new(new_line, new_col);
                        model.editor_mut().selections[idx] = Selection::new(new_pos);
                    }
                }

                let cursors_after: Vec<Cursor> = model.editor().cursors.clone();
                model.document_mut().push_edit(EditOperation::Batch {
                    operations,
                    cursors_before,
                    cursors_after,
                });

                model.document_mut().is_modified = true;
                model.ensure_cursor_visible();
                model.reset_cursor_blink();
                return Some(Cmd::Redraw);
            }

            // Single cursor: existing behavior
            let cursor_before = *model.editor().primary_cursor();
            let selection = model.editor().primary_selection().clone();

            if selection.is_empty() {
                // No selection: duplicate the current line
                let line_idx = model.editor().primary_cursor().line;
                let column = model.editor().primary_cursor().column;

                let line_text = model.document().get_line(line_idx).unwrap_or_default();
                let has_newline = line_text.ends_with('\n');

                let line_end_offset = if has_newline {
                    model.document().cursor_to_offset(line_idx + 1, 0)
                } else {
                    model
                        .document()
                        .cursor_to_offset(line_idx, model.document().line_length(line_idx))
                };

                let text_to_insert = if has_newline {
                    line_text.clone()
                } else {
                    format!("\n{}", line_text)
                };

                model
                    .document_mut()
                    .buffer
                    .insert(line_end_offset, &text_to_insert);

                model.editor_mut().primary_cursor_mut().line += 1;
                let new_line = model.editor().primary_cursor().line;
                model.editor_mut().primary_cursor_mut().column =
                    column.min(model.document().line_length(new_line));

                let cursor_after = *model.editor().primary_cursor();
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

                let selected_text: String = model
                    .document()
                    .buffer
                    .slice(start_offset..end_offset)
                    .chars()
                    .collect();

                model
                    .document_mut()
                    .buffer
                    .insert(end_offset, &selected_text);

                let new_offset = end_offset + selected_text.chars().count();
                model.set_cursor_from_position(new_offset);

                model.editor_mut().clear_selection();

                let cursor_after = *model.editor().primary_cursor();
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

        DocumentMsg::IndentLines => {
            // Multi-cursor: collect unique lines from all cursors/selections
            let covered_lines = lines_covered_by_all_cursors(model);

            if covered_lines.is_empty() {
                return Some(Cmd::Redraw);
            }

            let cursors_before: Vec<Cursor> = model.editor().cursors.clone();
            let mut operations = Vec::new();

            // Insert tabs in reverse document order (highest line first) to preserve offsets
            for &line in &covered_lines {
                let offset = model.document().cursor_to_offset(line, 0);
                model.document_mut().buffer.insert_char(offset, '\t');

                operations.push(EditOperation::Insert {
                    position: offset,
                    text: "\t".to_string(),
                    cursor_before: Cursor::at(line, 0),
                    cursor_after: Cursor::at(line, 1),
                });
            }

            // Adjust all cursors and selections: bump column +1 for each that's on an indented line
            let indented_lines: std::collections::HashSet<usize> =
                covered_lines.iter().copied().collect();
            let editor = model.editor_mut();
            for (cursor, selection) in editor.cursors.iter_mut().zip(editor.selections.iter_mut()) {
                if indented_lines.contains(&cursor.line) {
                    cursor.column += 1;
                }
                if indented_lines.contains(&selection.anchor.line) {
                    selection.anchor.column += 1;
                }
                if indented_lines.contains(&selection.head.line) {
                    selection.head.column += 1;
                }
            }

            let cursors_after: Vec<Cursor> = model.editor().cursors.clone();
            model.document_mut().push_edit(EditOperation::Batch {
                operations,
                cursors_before,
                cursors_after,
            });

            model.document_mut().is_modified = true;
            model.ensure_cursor_visible();
            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }

        DocumentMsg::UnindentLines => {
            // Multi-cursor: collect unique lines from all cursors/selections
            let covered_lines = lines_covered_by_all_cursors(model);

            if covered_lines.is_empty() {
                return Some(Cmd::Redraw);
            }

            let cursors_before: Vec<Cursor> = model.editor().cursors.clone();
            let mut operations = Vec::new();
            let mut removed_per_line: std::collections::HashMap<usize, usize> =
                std::collections::HashMap::new();

            // Process in reverse document order (highest line first) to preserve offsets
            for &line in &covered_lines {
                let line_start = model.document().cursor_to_offset(line, 0);
                let line_text: String = model.document().buffer.line(line).chars().collect();

                let chars_to_remove = if line_text.starts_with('\t') {
                    1
                } else {
                    // Count leading spaces (up to 4)
                    line_text.chars().take_while(|c| *c == ' ').count().min(4)
                };

                if chars_to_remove > 0 {
                    let removed_text: String = line_text.chars().take(chars_to_remove).collect();
                    model
                        .document_mut()
                        .buffer
                        .remove(line_start..line_start + chars_to_remove);

                    removed_per_line.insert(line, chars_to_remove);

                    operations.push(EditOperation::Delete {
                        position: line_start,
                        text: removed_text,
                        cursor_before: Cursor::at(line, chars_to_remove),
                        cursor_after: Cursor::at(line, 0),
                    });
                }
            }

            // Adjust all cursors and selections based on what was removed from their lines
            let editor = model.editor_mut();
            for (cursor, selection) in editor.cursors.iter_mut().zip(editor.selections.iter_mut()) {
                if let Some(&removed) = removed_per_line.get(&cursor.line) {
                    cursor.column = cursor.column.saturating_sub(removed);
                }
                if let Some(&removed) = removed_per_line.get(&selection.anchor.line) {
                    selection.anchor.column = selection.anchor.column.saturating_sub(removed);
                }
                if let Some(&removed) = removed_per_line.get(&selection.head.line) {
                    selection.head.column = selection.head.column.saturating_sub(removed);
                }
            }

            if !operations.is_empty() {
                let cursors_after: Vec<Cursor> = model.editor().cursors.clone();
                model.document_mut().push_edit(EditOperation::Batch {
                    operations,
                    cursors_before,
                    cursors_after,
                });
                model.document_mut().is_modified = true;
            }

            model.ensure_cursor_visible();
            model.reset_cursor_blink();
            Some(Cmd::Redraw)
        }
    }
}

/// Apply an undo operation to the model (reverses the edit)
fn apply_undo_operation(model: &mut AppModel, edit: &EditOperation) {
    match edit {
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
            *model.editor_mut().primary_cursor_mut() = *cursor_before;
        }
        EditOperation::Delete {
            position,
            text,
            cursor_before,
            ..
        } => {
            model.document_mut().buffer.insert(*position, text);
            *model.editor_mut().primary_cursor_mut() = *cursor_before;
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
            *model.editor_mut().primary_cursor_mut() = *cursor_before;
        }
        EditOperation::Batch {
            operations,
            cursors_before,
            ..
        } => {
            // Undo in reverse order
            for op in operations.iter().rev() {
                apply_undo_operation_buffer_only(model, op);
            }
            // Restore all cursors
            let editor = model.editor_mut();
            editor.cursors = cursors_before.clone();
            // Ensure selections array matches
            while editor.selections.len() < editor.cursors.len() {
                editor.selections.push(Selection::new(Position::new(0, 0)));
            }
            editor.selections.truncate(editor.cursors.len());
        }
    }
}

/// Apply undo to buffer only (for batch operations - cursor handled separately)
fn apply_undo_operation_buffer_only(model: &mut AppModel, edit: &EditOperation) {
    match edit {
        EditOperation::Insert { position, text, .. } => {
            model
                .document_mut()
                .buffer
                .remove(*position..*position + text.chars().count());
        }
        EditOperation::Delete { position, text, .. } => {
            model.document_mut().buffer.insert(*position, text);
        }
        EditOperation::Replace {
            position,
            deleted_text,
            inserted_text,
            ..
        } => {
            model
                .document_mut()
                .buffer
                .remove(*position..*position + inserted_text.chars().count());
            model.document_mut().buffer.insert(*position, deleted_text);
        }
        EditOperation::Batch { operations, .. } => {
            for op in operations.iter().rev() {
                apply_undo_operation_buffer_only(model, op);
            }
        }
    }
}

/// Apply a redo operation to the model (re-applies the edit)
fn apply_redo_operation(model: &mut AppModel, edit: &EditOperation) {
    match edit {
        EditOperation::Insert {
            position,
            text,
            cursor_after,
            ..
        } => {
            model.document_mut().buffer.insert(*position, text);
            *model.editor_mut().primary_cursor_mut() = *cursor_after;
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
            *model.editor_mut().primary_cursor_mut() = *cursor_after;
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
            *model.editor_mut().primary_cursor_mut() = *cursor_after;
        }
        EditOperation::Batch {
            operations,
            cursors_after,
            ..
        } => {
            // Redo in forward order
            for op in operations.iter() {
                apply_redo_operation_buffer_only(model, op);
            }
            // Restore all cursors
            let editor = model.editor_mut();
            editor.cursors = cursors_after.clone();
            // Ensure selections array matches
            while editor.selections.len() < editor.cursors.len() {
                editor.selections.push(Selection::new(Position::new(0, 0)));
            }
            editor.selections.truncate(editor.cursors.len());
        }
    }
}

/// Apply redo to buffer only (for batch operations - cursor handled separately)
fn apply_redo_operation_buffer_only(model: &mut AppModel, edit: &EditOperation) {
    match edit {
        EditOperation::Insert { position, text, .. } => {
            model.document_mut().buffer.insert(*position, text);
        }
        EditOperation::Delete { position, text, .. } => {
            model
                .document_mut()
                .buffer
                .remove(*position..*position + text.chars().count());
        }
        EditOperation::Replace {
            position,
            deleted_text,
            inserted_text,
            ..
        } => {
            model
                .document_mut()
                .buffer
                .remove(*position..*position + deleted_text.chars().count());
            model.document_mut().buffer.insert(*position, inserted_text);
        }
        EditOperation::Batch { operations, .. } => {
            for op in operations.iter() {
                apply_redo_operation_buffer_only(model, op);
            }
        }
    }
}
