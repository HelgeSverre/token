//! Document update functions for text editing and undo/redo

use crate::commands::Cmd;
use crate::messages::DocumentMsg;
use crate::model::{AppModel, Cursor, EditOperation, Position, Selection};
use crate::util::char_type;

use super::editor::{
    cursors_in_reverse_order, delete_selection, lines_covered_by_all_cursors,
    shift_sibling_cursors, sync_other_editor_cursors, sync_other_editor_cursors_for_deleted_text,
    sync_other_editor_cursors_for_single_char_delete, sync_other_editor_cursors_for_text,
};
use super::syntax::schedule_syntax_parse;

/// Returns the matching closing character for an opening surround character.
/// Used to wrap selected text when typing an opening bracket/quote.
fn surround_pair(open: char) -> Option<char> {
    Some(match open {
        '(' => ')',
        '[' => ']',
        '{' => '}',
        '"' => '"',
        '\'' => '\'',
        '`' => '`',
        _ => return None,
    })
}

/// Returns a Cmd that redraws and schedules syntax parsing for the current document
fn redraw_with_syntax_parse(model: &mut AppModel) -> Cmd {
    redraw_with_syntax_parse_shift(model, None)
}

/// Returns a Cmd that redraws and schedules syntax parsing, shifting highlights
/// if edit info is provided as (edit_line, old_line_count, new_line_count).
fn redraw_with_syntax_parse_shift(
    model: &mut AppModel,
    edit_info: Option<(usize, usize, usize)>,
) -> Cmd {
    if let Some(doc_id) = model.document().id {
        if let Some((edit_line, old_count, new_count)) = edit_info {
            if let Some(doc) = model.editor_area.documents.get_mut(&doc_id) {
                if let Some(ref mut highlights) = doc.syntax_highlights {
                    highlights.shift_for_edit(edit_line, old_count, new_count);
                }
            }
        }
        if let Some(parse_cmd) = schedule_syntax_parse(model, doc_id) {
            return Cmd::Batch(vec![Cmd::redraw_editor(), parse_cmd]);
        }
    }
    Cmd::redraw_editor()
}

/// Find the start of the word before the given offset
///
/// Uses direct character indexing instead of collecting to String/Vec to avoid
/// allocating the entire document prefix (which could be megabytes for large files).
fn word_start_before(buffer: &ropey::Rope, offset: usize) -> usize {
    if offset == 0 {
        return 0;
    }

    let mut pos = offset;

    // Get the character type of the char just before offset
    let first_char = buffer.char(pos - 1);
    let current_type = char_type(first_char);
    pos -= 1;

    // Continue backwards while same char type
    while pos > 0 {
        let ch = buffer.char(pos - 1);
        if char_type(ch) != current_type {
            break;
        }
        pos -= 1;
    }

    pos
}

/// Find the end of the word after the given offset
///
/// Uses direct character indexing instead of collecting to String/Vec to avoid
/// allocating the entire document suffix (which could be megabytes for large files).
fn word_end_after(buffer: &ropey::Rope, offset: usize) -> usize {
    let len = buffer.len_chars();
    if offset >= len {
        return len;
    }

    // Get the character type of the char at offset
    let first_char = buffer.char(offset);
    let current_type = char_type(first_char);
    let mut pos = offset + 1;

    // Continue forwards while same char type
    while pos < len {
        let ch = buffer.char(pos);
        if char_type(ch) != current_type {
            break;
        }
        pos += 1;
    }

    pos
}

/// Handle document messages (text editing, undo/redo)
pub fn update_document(model: &mut AppModel, msg: DocumentMsg) -> Option<Cmd> {
    let result = update_document_inner(model, msg);
    if model.editor().is_plain_text_mode() {
        super::editor::compute_matched_brackets(model);
    }
    result
}

fn update_document_inner(model: &mut AppModel, msg: DocumentMsg) -> Option<Cmd> {
    // Skip text operations for non-text tabs
    if !matches!(model.editor().tab_content, crate::model::TabContent::Text) {
        return None;
    }

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
                let close = surround_pair(ch);
                let any_has_selection = model.editor().selections.iter().any(|s| !s.is_empty());
                let do_surround =
                    model.config.auto_surround && close.is_some() && any_has_selection;

                // Overlapping selections (e.g. from SelectAllOccurrences, which only
                // deduplicates cursors but does not merge overlapping ranges) would
                // corrupt the buffer when processed in reverse order below, since each
                // selection's offset is recomputed against an already-mutated buffer.
                // Merging first guarantees non-overlapping ranges, making reverse-order
                // processing safe.
                if do_surround {
                    model.editor_mut().merge_overlapping_selections();
                }

                let cursors_before: Vec<Cursor> = model.editor().cursors.clone();
                let indices = cursors_in_reverse_order(model);
                let mut operations = Vec::new();

                for idx in indices {
                    let cursor = model.editor().cursors[idx];
                    let selection = model.editor().selections[idx];

                    if do_surround && !selection.is_empty() {
                        let Some(close) = close else { continue };
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

                        model.document_mut().buffer.remove(start_offset..end_offset);
                        let wrapped = format!("{ch}{selected_text}{close}");
                        model.document_mut().buffer.insert(start_offset, &wrapped);

                        let new_offset = start_offset + wrapped.len();
                        let (new_line, new_col) = model.document().offset_to_cursor(new_offset);

                        operations.push(EditOperation::Replace {
                            position: start_offset,
                            deleted_text: selected_text,
                            inserted_text: wrapped,
                            cursor_before: cursor,
                            cursor_after: Cursor::at(new_line, new_col),
                        });

                        model.editor_mut().cursors[idx] = Cursor::at(new_line, new_col);
                        model.editor_mut().cursors[idx].desired_column = None;
                        let new_pos = Position::new(new_line, new_col);
                        model.editor_mut().selections[idx] = Selection::new(new_pos);
                    } else {
                        let pos = model
                            .document()
                            .cursor_to_offset(cursor.line, cursor.column);

                        model.document_mut().buffer.insert_char(pos, ch);

                        operations.push(EditOperation::Insert {
                            position: pos,
                            text: ch.to_string(),
                            cursor_before: cursor,
                            cursor_after: Cursor::at(cursor.line, cursor.column + 1),
                        });

                        model.editor_mut().cursors[idx].column += 1;
                        model.editor_mut().cursors[idx].desired_column = None;

                        let new_pos = model.editor().cursors[idx].to_position();
                        model.editor_mut().selections[idx] = Selection::new(new_pos);
                    }
                }

                // Record batch for proper multi-cursor undo
                let cursors_after: Vec<Cursor> = model.editor().cursors.clone();
                model.document_mut().push_edit(EditOperation::Batch {
                    operations,
                    cursors_before,
                    cursors_after,
                });

                model.document_mut().is_modified = true;
                model.ensure_cursor_visible();
                model.reset_cursor_blink();
                return Some(redraw_with_syntax_parse(model));
            }

            // Single cursor: check for surround-selection behavior
            let selection = *model.editor().primary_selection();
            if model.config.auto_surround && !selection.is_empty() {
                if let Some(close) = surround_pair(ch) {
                    // Surround selection: wrap selected text with open/close pair
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

                    // Remove selected text, insert open + text + close
                    model.document_mut().buffer.remove(start_offset..end_offset);
                    let wrapped = format!("{ch}{selected_text}{close}");
                    model.document_mut().buffer.insert(start_offset, &wrapped);

                    // Position cursor after the closing char
                    let new_offset = start_offset + wrapped.len();
                    model.set_cursor_from_position(new_offset);
                    model.ensure_cursor_visible();

                    let cursor_after = *model.editor().primary_cursor();
                    model.document_mut().push_edit(EditOperation::Replace {
                        position: start_offset,
                        deleted_text: selected_text,
                        inserted_text: wrapped,
                        cursor_before,
                        cursor_after,
                    });

                    model.reset_cursor_blink();
                    return Some(redraw_with_syntax_parse(model));
                }
            }

            // If there's a selection (non-surround char), delete it first and use Replace for atomic undo
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
            Some(redraw_with_syntax_parse(model))
        }

        DocumentMsg::InsertNewline => {
            let cursor_before = *model.editor().primary_cursor();
            let edit_line = cursor_before.line;
            let old_line_count = model.document().line_count();

            // Multi-cursor: process all cursors in reverse document order
            if model.editor().has_multiple_cursors() {
                let cursors_before: Vec<Cursor> = model.editor().cursors.clone();
                let indices = cursors_in_reverse_order(model);
                let mut operations = Vec::new();

                for idx in indices {
                    let cursor = model.editor().cursors[idx];
                    let insert_line = cursor.line;
                    let pos = model
                        .document()
                        .cursor_to_offset(cursor.line, cursor.column);
                    model.document_mut().buffer.insert_char(pos, '\n');

                    // Record individual operation
                    operations.push(EditOperation::Insert {
                        position: pos,
                        text: "\n".to_string(),
                        cursor_before: cursor,
                        cursor_after: Cursor::at(cursor.line + 1, 0),
                    });

                    // Move cursor to beginning of next line
                    model.editor_mut().cursors[idx].line += 1;
                    model.editor_mut().cursors[idx].column = 0;
                    model.editor_mut().cursors[idx].desired_column = None;

                    let new_pos = model.editor().cursors[idx].to_position();
                    model.editor_mut().selections[idx] = Selection::new(new_pos);

                    // Adjust all OTHER cursors that are AFTER this insertion point
                    // (they need to shift down by 1 line)
                    shift_sibling_cursors(model.editor_mut(), idx, insert_line, 1, None);
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
                let new_line_count = model.document().line_count();
                return Some(redraw_with_syntax_parse_shift(
                    model,
                    Some((edit_line, old_line_count, new_line_count)),
                ));
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
            let new_line_count = model.document().line_count();
            Some(redraw_with_syntax_parse_shift(
                model,
                Some((edit_line, old_line_count, new_line_count)),
            ))
        }

        DocumentMsg::DeleteBackward => {
            let cursor_before = *model.editor().primary_cursor();
            let old_line_count = model.document().line_count();

            // Multi-cursor: process all cursors in reverse document order
            if model.editor().has_multiple_cursors() {
                let cursors_before: Vec<Cursor> = model.editor().cursors.clone();
                let indices = cursors_in_reverse_order(model);
                let mut operations = Vec::new();

                for idx in indices {
                    let selection = model.editor().selections[idx];
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
                            cursor_before: model.editor().cursors[idx],
                            cursor_after: Cursor::at(start.line, start.column),
                        });

                        model.editor_mut().cursors[idx].line = start.line;
                        model.editor_mut().cursors[idx].column = start.column;
                        model.editor_mut().selections[idx] = Selection::new(start);
                    } else {
                        let cursor = model.editor().cursors[idx];
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
                            let is_newline = deleted_char == "\n";
                            let deleted_from_line = cursor.line;

                            model.document_mut().buffer.remove(pos - 1..pos);
                            let (new_line, new_col) = model.document().offset_to_cursor(pos - 1);

                            // Record operation
                            operations.push(EditOperation::Delete {
                                position: pos - 1,
                                text: deleted_char,
                                cursor_before: cursor,
                                cursor_after: Cursor::at(new_line, new_col),
                            });

                            model.editor_mut().cursors[idx].line = new_line;
                            model.editor_mut().cursors[idx].column = new_col;
                            let new_pos = model.editor().cursors[idx].to_position();
                            model.editor_mut().selections[idx] = Selection::new(new_pos);

                            // If we deleted a newline, adjust all other cursors below this point
                            // new_col is the column where the merge point is (end of previous line)
                            if is_newline {
                                shift_sibling_cursors(
                                    model.editor_mut(),
                                    idx,
                                    deleted_from_line,
                                    -1,
                                    Some(new_col),
                                );
                            }
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
                return Some(redraw_with_syntax_parse(model));
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
                return Some(redraw_with_syntax_parse(model));
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

                // Sync cursors in other views. Backspace deletes the character
                // *before* the cursor, so the sync point differs by branch:
                // for a deleted newline it's the end of the previous line;
                // otherwise it's one column left on the same line.
                let (sync_line, sync_column) = if is_newline {
                    (edit_line.saturating_sub(1), 0)
                } else {
                    (edit_line, edit_column.saturating_sub(1))
                };
                sync_other_editor_cursors_for_single_char_delete(
                    model,
                    sync_line,
                    sync_column,
                    is_newline,
                );
            }

            model.reset_cursor_blink();
            let new_line_count = model.document().line_count();
            let edit_line = cursor_before.line.min(new_line_count.saturating_sub(1));
            Some(redraw_with_syntax_parse_shift(
                model,
                Some((edit_line, old_line_count, new_line_count)),
            ))
        }

        DocumentMsg::DeleteWordBackward => {
            let cursor_before = *model.editor().primary_cursor();
            let old_line_count = model.document().line_count();

            // Multi-cursor: process all cursors in reverse document order
            if model.editor().has_multiple_cursors() {
                let cursors_before: Vec<Cursor> = model.editor().cursors.clone();
                let indices = cursors_in_reverse_order(model);
                let mut operations = Vec::new();

                for idx in indices {
                    let selection = model.editor().selections[idx];
                    if !selection.is_empty() {
                        // Delete selection (same as DeleteBackward)
                        let start = selection.start();
                        let end = selection.end();
                        let start_offset =
                            model.document().cursor_to_offset(start.line, start.column);
                        let end_offset = model.document().cursor_to_offset(end.line, end.column);

                        if start_offset < end_offset {
                            let deleted_text: String = model
                                .document()
                                .buffer
                                .slice(start_offset..end_offset)
                                .to_string();
                            model.document_mut().buffer.remove(start_offset..end_offset);

                            operations.push(EditOperation::Delete {
                                position: start_offset,
                                text: deleted_text,
                                cursor_before: model.editor().cursors[idx],
                                cursor_after: Cursor::at(start.line, start.column),
                            });

                            model.editor_mut().cursors[idx].line = start.line;
                            model.editor_mut().cursors[idx].column = start.column;
                            model.editor_mut().selections[idx] = Selection::new(start);
                        }
                    } else {
                        // No selection: delete word to the left
                        let cursor = model.editor().cursors[idx];
                        let end_offset = model
                            .document()
                            .cursor_to_offset(cursor.line, cursor.column);

                        if end_offset == 0 {
                            continue;
                        }

                        let start_offset = word_start_before(&model.document().buffer, end_offset);
                        if start_offset >= end_offset {
                            continue;
                        }

                        let deleted_text: String = model
                            .document()
                            .buffer
                            .slice(start_offset..end_offset)
                            .to_string();
                        model.document_mut().buffer.remove(start_offset..end_offset);

                        let (new_line, new_col) = model.document().offset_to_cursor(start_offset);

                        operations.push(EditOperation::Delete {
                            position: start_offset,
                            text: deleted_text,
                            cursor_before: cursor,
                            cursor_after: Cursor::at(new_line, new_col),
                        });

                        model.editor_mut().cursors[idx].line = new_line;
                        model.editor_mut().cursors[idx].column = new_col;
                        let new_pos = model.editor().cursors[idx].to_position();
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
                let new_line_count = model.document().line_count();
                let edit_line = cursor_before.line.min(new_line_count.saturating_sub(1));
                return Some(redraw_with_syntax_parse_shift(
                    model,
                    Some((edit_line, old_line_count, new_line_count)),
                ));
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
                let new_line_count = model.document().line_count();
                let edit_line = cursor_before.line.min(new_line_count.saturating_sub(1));
                return Some(redraw_with_syntax_parse_shift(
                    model,
                    Some((edit_line, old_line_count, new_line_count)),
                ));
            }

            // No selection: delete word to the left
            let end_offset = model.cursor_buffer_position();
            if end_offset > 0 {
                let start_offset = word_start_before(&model.document().buffer, end_offset);
                if start_offset < end_offset {
                    let deleted_text: String = model
                        .document()
                        .buffer
                        .slice(start_offset..end_offset)
                        .to_string();
                    model.document_mut().buffer.remove(start_offset..end_offset);

                    model.set_cursor_from_position(start_offset);
                    model.ensure_cursor_visible();

                    let cursor_after = *model.editor().primary_cursor();
                    model.document_mut().push_edit(EditOperation::Delete {
                        position: start_offset,
                        text: deleted_text,
                        cursor_before,
                        cursor_after,
                    });

                    model.document_mut().is_modified = true;
                }
            }

            model.reset_cursor_blink();
            let new_line_count = model.document().line_count();
            let edit_line = cursor_before.line.min(new_line_count.saturating_sub(1));
            Some(redraw_with_syntax_parse_shift(
                model,
                Some((edit_line, old_line_count, new_line_count)),
            ))
        }

        DocumentMsg::DeleteWordForward => {
            let cursor_before = *model.editor().primary_cursor();
            let old_line_count = model.document().line_count();

            // Multi-cursor: process all cursors in reverse document order
            if model.editor().has_multiple_cursors() {
                let cursors_before: Vec<Cursor> = model.editor().cursors.clone();
                let indices = cursors_in_reverse_order(model);
                let mut operations = Vec::new();

                for idx in indices {
                    let selection = model.editor().selections[idx];
                    if !selection.is_empty() {
                        // Delete selection (same as DeleteForward)
                        let start = selection.start();
                        let end = selection.end();
                        let start_offset =
                            model.document().cursor_to_offset(start.line, start.column);
                        let end_offset = model.document().cursor_to_offset(end.line, end.column);

                        if start_offset < end_offset {
                            let deleted_text: String = model
                                .document()
                                .buffer
                                .slice(start_offset..end_offset)
                                .to_string();
                            model.document_mut().buffer.remove(start_offset..end_offset);

                            operations.push(EditOperation::Delete {
                                position: start_offset,
                                text: deleted_text,
                                cursor_before: model.editor().cursors[idx],
                                cursor_after: Cursor::at(start.line, start.column),
                            });

                            model.editor_mut().cursors[idx].line = start.line;
                            model.editor_mut().cursors[idx].column = start.column;
                            model.editor_mut().selections[idx] = Selection::new(start);
                        }
                    } else {
                        // No selection: delete word to the right
                        let cursor = model.editor().cursors[idx];
                        let start_offset = model
                            .document()
                            .cursor_to_offset(cursor.line, cursor.column);

                        let buffer_len = model.document().buffer.len_chars();
                        if start_offset >= buffer_len {
                            continue;
                        }

                        let end_offset = word_end_after(&model.document().buffer, start_offset);
                        if end_offset <= start_offset {
                            continue;
                        }

                        let deleted_text: String = model
                            .document()
                            .buffer
                            .slice(start_offset..end_offset)
                            .to_string();
                        model.document_mut().buffer.remove(start_offset..end_offset);

                        operations.push(EditOperation::Delete {
                            position: start_offset,
                            text: deleted_text,
                            cursor_before: cursor,
                            cursor_after: cursor, // Cursor stays in place
                        });

                        // Cursor position doesn't change for forward delete
                        let new_pos = model.editor().cursors[idx].to_position();
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
                let new_line_count = model.document().line_count();
                let edit_line = cursor_before.line.min(new_line_count.saturating_sub(1));
                return Some(redraw_with_syntax_parse_shift(
                    model,
                    Some((edit_line, old_line_count, new_line_count)),
                ));
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
                let new_line_count = model.document().line_count();
                let edit_line = cursor_before.line.min(new_line_count.saturating_sub(1));
                return Some(redraw_with_syntax_parse_shift(
                    model,
                    Some((edit_line, old_line_count, new_line_count)),
                ));
            }

            // No selection: delete word to the right
            let start_offset = model.cursor_buffer_position();
            let buffer_len = model.document().buffer.len_chars();
            if start_offset < buffer_len {
                let end_offset = word_end_after(&model.document().buffer, start_offset);
                if end_offset > start_offset {
                    let deleted_text: String = model
                        .document()
                        .buffer
                        .slice(start_offset..end_offset)
                        .to_string();
                    model.document_mut().buffer.remove(start_offset..end_offset);

                    // Cursor position doesn't change for forward delete
                    model.ensure_cursor_visible();

                    let cursor_after = *model.editor().primary_cursor();
                    model.document_mut().push_edit(EditOperation::Delete {
                        position: start_offset,
                        text: deleted_text,
                        cursor_before,
                        cursor_after,
                    });

                    model.document_mut().is_modified = true;
                }
            }

            model.reset_cursor_blink();
            let new_line_count = model.document().line_count();
            let edit_line = cursor_before.line.min(new_line_count.saturating_sub(1));
            Some(redraw_with_syntax_parse_shift(
                model,
                Some((edit_line, old_line_count, new_line_count)),
            ))
        }

        DocumentMsg::DeleteForward => {
            let cursor_before = *model.editor().primary_cursor();
            let old_line_count = model.document().line_count();

            // Multi-cursor: process all cursors in reverse document order
            if model.editor().has_multiple_cursors() {
                let cursors_before: Vec<Cursor> = model.editor().cursors.clone();
                let indices = cursors_in_reverse_order(model);
                let mut operations = Vec::new();

                for idx in indices {
                    let selection = model.editor().selections[idx];
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
                            text: deleted_text.clone(),
                            cursor_before: model.editor().cursors[idx],
                            cursor_after: Cursor::at(start.line, start.column),
                        });

                        model.editor_mut().cursors[idx].line = start.line;
                        model.editor_mut().cursors[idx].column = start.column;
                        model.editor_mut().selections[idx] = Selection::new(start);

                        // Sync cursors in other views
                        sync_other_editor_cursors_for_deleted_text(
                            model,
                            start.line,
                            start.column,
                            &deleted_text,
                        );
                    } else {
                        let cursor = model.editor().cursors[idx];
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
                            let is_newline = deleted_char == "\n";
                            model.document_mut().buffer.remove(pos..pos + 1);

                            // Record operation (cursor doesn't move for delete forward)
                            operations.push(EditOperation::Delete {
                                position: pos,
                                text: deleted_char,
                                cursor_before: cursor,
                                cursor_after: cursor,
                            });

                            // Sync cursors in other views
                            sync_other_editor_cursors_for_single_char_delete(
                                model,
                                cursor.line,
                                cursor.column,
                                is_newline,
                            );
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
                let new_line_count = model.document().line_count();
                let edit_line = cursor_before.line.min(new_line_count.saturating_sub(1));
                return Some(redraw_with_syntax_parse_shift(
                    model,
                    Some((edit_line, old_line_count, new_line_count)),
                ));
            }

            // Single cursor: check for selection
            if let Some((pos, deleted_text)) = delete_selection(model) {
                let cursor_after = *model.editor().primary_cursor();
                let (edit_line, edit_column) = model.document().offset_to_cursor(pos);
                model.document_mut().push_edit(EditOperation::Delete {
                    position: pos,
                    text: deleted_text.clone(),
                    cursor_before,
                    cursor_after,
                });
                model.document_mut().is_modified = true;
                model.reset_cursor_blink();

                // Sync cursors in other views
                sync_other_editor_cursors_for_deleted_text(
                    model,
                    edit_line,
                    edit_column,
                    &deleted_text,
                );

                let new_line_count = model.document().line_count();
                let edit_line = cursor_before.line.min(new_line_count.saturating_sub(1));
                return Some(redraw_with_syntax_parse_shift(
                    model,
                    Some((edit_line, old_line_count, new_line_count)),
                ));
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
                sync_other_editor_cursors_for_single_char_delete(
                    model,
                    edit_line,
                    edit_column,
                    is_newline,
                );
            }

            model.reset_cursor_blink();
            let new_line_count = model.document().line_count();
            let edit_line = cursor_before.line.min(new_line_count.saturating_sub(1));
            Some(redraw_with_syntax_parse_shift(
                model,
                Some((edit_line, old_line_count, new_line_count)),
            ))
        }

        DocumentMsg::DeleteLine => {
            let total_lines = model.document().line_count();
            let old_line_count = total_lines;
            if total_lines == 0 {
                return Some(redraw_with_syntax_parse(model));
            }

            if model.editor().has_multiple_cursors() {
                let cursors_before = model.editor().cursors.clone();
                let mut covered_lines = lines_covered_by_all_cursors(model);
                covered_lines.sort_unstable();
                covered_lines.dedup();
                let mut operations = Vec::new();

                // Check if deleted lines are contiguous
                let is_contiguous =
                    covered_lines.len() <= 1 || covered_lines.windows(2).all(|w| w[1] == w[0] + 1);

                // Delete lines in reverse order to preserve indices
                for line_idx in covered_lines.iter().rev().copied() {
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

                if is_contiguous {
                    // Contiguous lines deleted: collapse to single cursor (existing behavior)
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
                } else {
                    // Non-contiguous lines deleted: preserve cursor count
                    // Map each cursor to its new position after deletions
                    use std::collections::HashSet;
                    let deleted_set: HashSet<usize> = covered_lines.iter().copied().collect();

                    let mut new_cursors = Vec::with_capacity(cursors_before.len());
                    let mut new_selections = Vec::with_capacity(cursors_before.len());

                    for c in &cursors_before {
                        if new_line_count == 0 {
                            // Whole document gone
                            new_cursors.push(Cursor::at(0, 0));
                            new_selections.push(Selection::new(Position::new(0, 0)));
                            continue;
                        }

                        // Count how many deleted lines are above this cursor's original line
                        let deleted_above = covered_lines.iter().filter(|&&l| l < c.line).count();

                        if !deleted_set.contains(&c.line) {
                            // Line wasn't deleted: just shift up by deleted_above
                            let new_line = c.line.saturating_sub(deleted_above);
                            let line_len = model.document().line_length(new_line);
                            let new_col = c.column.min(line_len);
                            new_cursors.push(Cursor::at(new_line, new_col));
                            new_selections.push(Selection::new(Position::new(new_line, new_col)));
                        } else {
                            // Line was deleted: find the closest non-deleted line
                            // First try to go to the line that slid into this position
                            let new_line = c.line.saturating_sub(deleted_above);
                            let target_line = new_line.min(new_line_count.saturating_sub(1));
                            let line_len = model.document().line_length(target_line);
                            let new_col = c.column.min(line_len);
                            new_cursors.push(Cursor::at(target_line, new_col));
                            new_selections
                                .push(Selection::new(Position::new(target_line, new_col)));
                        }
                    }

                    model.editor_mut().cursors = new_cursors;
                    model.editor_mut().selections = new_selections;
                    model.editor_mut().active_cursor_index = 0;
                    model.editor_mut().deduplicate_cursors();
                }

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
                    if let Some(editor_id) = model.editor_area.focused_editor_id() {
                        let next_top_line = model
                            .editor()
                            .viewport
                            .top_line
                            .saturating_sub(deleted_above_viewport);
                        model.set_editor_vertical_scroll(editor_id, next_top_line);
                    }
                }

                model.ensure_cursor_visible();
                model.reset_cursor_blink();
                let new_line_count = model.document().line_count();
                let min_line = covered_lines.iter().copied().min().unwrap_or(0);
                let edit_line = min_line.min(new_line_count.saturating_sub(1));
                return Some(redraw_with_syntax_parse_shift(
                    model,
                    Some((edit_line, old_line_count, new_line_count)),
                ));
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

                let new_pos = model.editor().primary_cursor().to_position();
                *model.editor_mut().primary_selection_mut() = Selection::new(new_pos);

                let cursor_after = *model.editor().primary_cursor();
                model.document_mut().push_edit(EditOperation::Delete {
                    position: start_offset,
                    text: deleted,
                    cursor_before,
                    cursor_after,
                });

                model.document_mut().is_modified = true;

                // Only shift the scroll position up if the deleted line was actually
                // above the viewport; deleting a line at/below top_line must not move it.
                if line_idx < model.editor().viewport.top_line {
                    if let Some(editor_id) = model.editor_area.focused_editor_id() {
                        let next_top_line = model.editor().viewport.top_line.saturating_sub(1);
                        model.set_editor_vertical_scroll(editor_id, next_top_line);
                    }
                }
            }

            model.ensure_cursor_visible();
            model.reset_cursor_blink();
            let new_line_count = model.document().line_count();
            Some(redraw_with_syntax_parse_shift(
                model,
                Some((
                    line_idx.min(new_line_count.saturating_sub(1)),
                    old_line_count,
                    new_line_count,
                )),
            ))
        }

        DocumentMsg::Undo => {
            if let Some(edit) = model.document_mut().undo_stack.pop() {
                apply_undo_operation(model, &edit);
                let doc = model.document_mut();
                doc.redo_stack.push(edit);
                doc.is_modified = doc.saved_revision != Some(doc.undo_stack.len());
                model.editor_mut().collapse_selections_to_cursors();
                model.ensure_cursor_visible();
                model.reset_cursor_blink();
            }
            Some(redraw_with_syntax_parse(model))
        }

        DocumentMsg::Redo => {
            if let Some(edit) = model.document_mut().redo_stack.pop() {
                apply_redo_operation(model, &edit);
                let doc = model.document_mut();
                doc.undo_stack.push(edit);
                doc.is_modified = doc.saved_revision != Some(doc.undo_stack.len());
                model.editor_mut().collapse_selections_to_cursors();
                model.ensure_cursor_visible();
                model.reset_cursor_blink();
            }
            Some(redraw_with_syntax_parse(model))
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
                let selection = *model.editor().primary_selection();
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

            let mut cmd = redraw_with_syntax_parse(model);
            if !text_to_copy.is_empty() {
                model
                    .ui
                    .set_status(format!("Copied {} chars", text_to_copy.len()));
                cmd = Cmd::Batch(vec![cmd, Cmd::CopyToClipboard(text_to_copy)]);
            }
            Some(cmd)
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
                let selection = *model.editor().primary_selection();
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

            // Delete selections with proper undo support
            if has_selection {
                if model.editor().has_multiple_cursors() {
                    // Multi-cursor cut: delete each selection and record as Batch
                    let cursors_before: Vec<Cursor> = model.editor().cursors.clone();
                    let indices = cursors_in_reverse_order(model);
                    let mut operations = Vec::new();

                    for idx in indices {
                        let selection = model.editor().selections[idx];
                        if !selection.is_empty() {
                            let start = selection.start();
                            let end = selection.end();
                            let start_offset =
                                model.document().cursor_to_offset(start.line, start.column);
                            let end_offset =
                                model.document().cursor_to_offset(end.line, end.column);

                            let deleted_text: String = model
                                .document()
                                .buffer
                                .slice(start_offset..end_offset)
                                .chars()
                                .collect();

                            model.document_mut().buffer.remove(start_offset..end_offset);

                            operations.push(EditOperation::Delete {
                                position: start_offset,
                                text: deleted_text,
                                cursor_before: model.editor().cursors[idx],
                                cursor_after: Cursor::at(start.line, start.column),
                            });

                            model.editor_mut().cursors[idx].line = start.line;
                            model.editor_mut().cursors[idx].column = start.column;
                            model.editor_mut().selections[idx] = Selection::new(start);
                        }
                    }

                    if !operations.is_empty() {
                        let cursors_after: Vec<Cursor> = model.editor().cursors.clone();
                        model.document_mut().push_edit(EditOperation::Batch {
                            operations,
                            cursors_before,
                            cursors_after,
                        });
                    }
                } else {
                    // Single-cursor cut: delete selection and record as Delete
                    let cursor_before = *model.editor().primary_cursor();
                    if let Some((pos, deleted_text)) = delete_selection(model) {
                        let cursor_after = *model.editor().primary_cursor();
                        model.document_mut().push_edit(EditOperation::Delete {
                            position: pos,
                            text: deleted_text,
                            cursor_before,
                            cursor_after,
                        });
                    }
                }
                model.document_mut().is_modified = true;
                model
                    .ui
                    .set_status(format!("Cut {} chars", text_to_copy.len()));
            }

            model.ensure_cursor_visible();
            model.reset_cursor_blink();

            let mut cmd = redraw_with_syntax_parse(model);
            if !text_to_copy.is_empty() {
                cmd = Cmd::Batch(vec![cmd, Cmd::CopyToClipboard(text_to_copy)]);
            }
            Some(cmd)
        }

        DocumentMsg::Paste => Some(Cmd::RequestClipboardPaste),

        // `InsertText` (single atomic multi-char insert, e.g. IME commit or a
        // legacy-bridge insert) needs the exact same semantics as pasting
        // text: one rope insert, one undo record, proper multi-cursor
        // distribution and peer-cursor sync — so it shares this arm.
        DocumentMsg::PasteText(text) | DocumentMsg::InsertText(text) => {
            if text.is_empty() {
                return Some(redraw_with_syntax_parse(model));
            }

            let cursor_before = *model.editor().primary_cursor();
            let paste_edit_line = cursor_before.line;
            let old_line_count = model.document().line_count();

            if model.editor().has_multiple_cursors() {
                let cursors_before: Vec<Cursor> = model.editor().cursors.clone();
                let mut operations = Vec::new();

                let lines: Vec<&str> = text.lines().collect();
                let cursor_count = model.editor().cursors.len();

                // If clipboard has same number of lines as cursors, distribute one per cursor
                if lines.len() == cursor_count {
                    let indices = cursors_in_reverse_order(model);
                    for (i, idx) in indices.iter().enumerate() {
                        let line_to_paste = lines[cursor_count - 1 - i]; // Reverse order
                        let cursor = model.editor().cursors[*idx];
                        let pos = model
                            .document()
                            .cursor_to_offset(cursor.line, cursor.column);
                        model.document_mut().buffer.insert(pos, line_to_paste);

                        let char_count = line_to_paste.chars().count();
                        model.editor_mut().cursors[*idx].column += char_count;
                        let new_pos = model.editor().cursors[*idx].to_position();
                        model.editor_mut().selections[*idx] = Selection::new(new_pos);

                        operations.push(EditOperation::Insert {
                            position: pos,
                            text: line_to_paste.to_string(),
                            cursor_before: cursor,
                            cursor_after: Cursor::at(
                                model.editor().cursors[*idx].line,
                                model.editor().cursors[*idx].column,
                            ),
                        });
                    }
                } else {
                    // Paste full text at each cursor
                    let indices = cursors_in_reverse_order(model);
                    let lines_added = text.chars().filter(|&c| c == '\n').count();

                    for idx in indices {
                        let cursor = model.editor().cursors[idx];
                        let insert_line = cursor.line;
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

                        operations.push(EditOperation::Insert {
                            position: pos,
                            text: text.clone(),
                            cursor_before: cursor,
                            cursor_after: Cursor::at(new_line, new_col),
                        });

                        // Adjust other cursors for lines added
                        if lines_added > 0 {
                            shift_sibling_cursors(
                                model.editor_mut(),
                                idx,
                                insert_line,
                                lines_added as isize,
                                None,
                            );
                        }
                    }
                }

                // Record batch for proper multi-cursor undo
                if !operations.is_empty() {
                    let cursors_after: Vec<Cursor> = model.editor().cursors.clone();
                    model.document_mut().push_edit(EditOperation::Batch {
                        operations,
                        cursors_before,
                        cursors_after,
                    });
                }
            } else {
                // Single cursor: use Replace if selection exists for atomic undo
                if !model.editor().primary_selection().is_empty() {
                    let Some((pos, deleted_text)) = delete_selection(model) else {
                        let cursor_pos = model.cursor_buffer_position();
                        let (edit_line, edit_column) =
                            model.document().offset_to_cursor(cursor_pos);
                        model.document_mut().buffer.insert(cursor_pos, &text);
                        let new_offset = cursor_pos + text.chars().count();
                        model.set_cursor_from_position(new_offset);
                        model.document_mut().is_modified = true;
                        model.ui.set_status(format!("Pasted {} chars", text.len()));
                        model.ensure_cursor_visible();
                        model.reset_cursor_blink();
                        sync_other_editor_cursors_for_text(model, edit_line, edit_column, &text);
                        return Some(redraw_with_syntax_parse_shift(
                            model,
                            Some((
                                paste_edit_line,
                                old_line_count,
                                model.document().line_count(),
                            )),
                        ));
                    };

                    let (edit_line, edit_column) = model.document().offset_to_cursor(pos);
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

                    sync_other_editor_cursors_for_text(model, edit_line, edit_column, &text);
                } else {
                    let pos = model.cursor_buffer_position();
                    let (edit_line, edit_column) = model.document().offset_to_cursor(pos);

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

                    sync_other_editor_cursors_for_text(model, edit_line, edit_column, &text);
                }
            }

            model.document_mut().is_modified = true;
            model.ui.set_status(format!("Pasted {} chars", text.len()));
            model.ensure_cursor_visible();
            model.reset_cursor_blink();
            let new_line_count = model.document().line_count();
            Some(redraw_with_syntax_parse_shift(
                model,
                Some((paste_edit_line, old_line_count, new_line_count)),
            ))
        }

        DocumentMsg::Duplicate => {
            // Multi-cursor: process all cursors in reverse document order
            if model.editor().has_multiple_cursors() {
                let cursors_before: Vec<Cursor> = model.editor().cursors.clone();
                let indices = cursors_in_reverse_order(model);
                let mut operations = Vec::new();

                for idx in indices {
                    let selection = model.editor().selections[idx];
                    let cursor = model.editor().cursors[idx];

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

                        // Count lines added by this duplication
                        let lines_added = text_to_insert.chars().filter(|&c| c == '\n').count();
                        let effective_lines_added = if !has_newline { 1 } else { lines_added };

                        operations.push(EditOperation::Insert {
                            position: line_end_offset,
                            text: text_to_insert,
                            cursor_before: cursor,
                            cursor_after: Cursor::at(
                                line_idx + 1,
                                column.min(model.document().line_length(line_idx + 1)),
                            ),
                        });

                        // Move THIS cursor to duplicated line
                        model.editor_mut().cursors[idx].line += 1;
                        let new_line_len = model.document().line_length(line_idx + 1);
                        model.editor_mut().cursors[idx].column = column.min(new_line_len);
                        model.editor_mut().cursors[idx].desired_column = None;

                        // Update selection to match cursor
                        let new_pos = model.editor().cursors[idx].to_position();
                        model.editor_mut().selections[idx] = Selection::new(new_pos);

                        // Adjust ALL OTHER cursors that are AFTER this insertion point
                        // (they need to shift down by the number of lines added)
                        shift_sibling_cursors(
                            model.editor_mut(),
                            idx,
                            line_idx,
                            effective_lines_added as isize,
                            None,
                        );
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

                        let new_offset = end_offset + selected_text.chars().count();
                        let (new_line, new_col) = model.document().offset_to_cursor(new_offset);

                        operations.push(EditOperation::Insert {
                            position: end_offset,
                            text: selected_text,
                            cursor_before: cursor,
                            cursor_after: Cursor::at(new_line, new_col),
                        });

                        // Move cursor to end of duplicated text, clear selection
                        model.editor_mut().cursors[idx].line = new_line;
                        model.editor_mut().cursors[idx].column = new_col;
                        model.editor_mut().cursors[idx].desired_column = None;

                        let new_pos = Position::new(new_line, new_col);
                        model.editor_mut().selections[idx] = Selection::new(new_pos);

                        // Adjust cursors after this insertion if lines were added
                        if lines_added > 0 {
                            shift_sibling_cursors(
                                model.editor_mut(),
                                idx,
                                sel_end.line,
                                lines_added as isize,
                                None,
                            );
                        }
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
                return Some(redraw_with_syntax_parse(model));
            }

            // Single cursor: existing behavior
            let cursor_before = *model.editor().primary_cursor();
            let selection = *model.editor().primary_selection();

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

                let new_pos = model.editor().primary_cursor().to_position();
                *model.editor_mut().primary_selection_mut() = Selection::new(new_pos);

                let lines_added = text_to_insert.chars().filter(|&c| c == '\n').count();
                let effective_lines_added = if !has_newline { 1 } else { lines_added };

                let cursor_after = *model.editor().primary_cursor();
                model.document_mut().push_edit(EditOperation::Insert {
                    position: line_end_offset,
                    text: text_to_insert,
                    cursor_before,
                    cursor_after,
                });

                sync_other_editor_cursors(
                    model,
                    line_idx,
                    model.document().line_length(line_idx),
                    effective_lines_added as isize,
                    0,
                );
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
                    text: selected_text.clone(),
                    cursor_before,
                    cursor_after,
                });

                sync_other_editor_cursors_for_text(
                    model,
                    sel_end.line,
                    sel_end.column,
                    &selected_text,
                );
            }

            model.document_mut().is_modified = true;
            model.ensure_cursor_visible();
            model.reset_cursor_blink();
            Some(redraw_with_syntax_parse(model))
        }

        DocumentMsg::IndentLines => {
            // Multi-cursor: collect unique lines from all cursors/selections
            let covered_lines = lines_covered_by_all_cursors(model);

            if covered_lines.is_empty() {
                return Some(redraw_with_syntax_parse(model));
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

            // Sync peer editors on the same document: every indented line
            // shifted its own columns right by 1.
            let line_deltas: std::collections::HashMap<usize, isize> =
                indented_lines.iter().map(|&line| (line, 1)).collect();
            super::editor::sync_other_editor_cursors_for_line_shifts(model, &line_deltas);

            model.document_mut().is_modified = true;
            model.ensure_cursor_visible();
            model.reset_cursor_blink();
            Some(redraw_with_syntax_parse(model))
        }

        DocumentMsg::UnindentLines => {
            // Multi-cursor: collect unique lines from all cursors/selections
            let covered_lines = lines_covered_by_all_cursors(model);

            if covered_lines.is_empty() {
                return Some(redraw_with_syntax_parse(model));
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

                // Sync peer editors on the same document: every unindented
                // line shifted its own columns left by however much was removed.
                let line_deltas: std::collections::HashMap<usize, isize> = removed_per_line
                    .iter()
                    .map(|(&line, &removed)| (line, -(removed as isize)))
                    .collect();
                super::editor::sync_other_editor_cursors_for_line_shifts(model, &line_deltas);
            }

            model.ensure_cursor_visible();
            model.reset_cursor_blink();
            Some(redraw_with_syntax_parse(model))
        }
    }
}

/// Extract the single-cursor `cursor_before` field from a non-batch
/// `EditOperation`. Returns `None` for `Batch`, which restores its whole
/// `cursors_before` vector instead (see `apply_undo_operation`).
fn edit_cursor_before(edit: &EditOperation) -> Option<Cursor> {
    match edit {
        EditOperation::Insert { cursor_before, .. }
        | EditOperation::Delete { cursor_before, .. }
        | EditOperation::Replace { cursor_before, .. } => Some(*cursor_before),
        EditOperation::Batch { .. } => None,
    }
}

/// Extract the single-cursor `cursor_after` field from a non-batch
/// `EditOperation`. Returns `None` for `Batch`, which restores its whole
/// `cursors_after` vector instead (see `apply_redo_operation`).
fn edit_cursor_after(edit: &EditOperation) -> Option<Cursor> {
    match edit {
        EditOperation::Insert { cursor_after, .. }
        | EditOperation::Delete { cursor_after, .. }
        | EditOperation::Replace { cursor_after, .. } => Some(*cursor_after),
        EditOperation::Batch { .. } => None,
    }
}

/// Restore `editor.cursors`/`editor.selections` from a batch's saved cursor
/// vector, padding/truncating `selections` to match `cursors` length (a
/// batch's operations may have added/removed cursors along the way).
fn restore_batch_cursors(model: &mut AppModel, cursors: &[Cursor]) {
    let editor = model.editor_mut();
    editor.cursors = cursors.to_vec();
    while editor.selections.len() < editor.cursors.len() {
        editor.selections.push(Selection::new(Position::new(0, 0)));
    }
    editor.selections.truncate(editor.cursors.len());
}

/// Apply an undo operation to the model (reverses the edit)
fn apply_undo_operation(model: &mut AppModel, edit: &EditOperation) {
    apply_undo_operation_buffer_only(model, edit);
    if let EditOperation::Batch { cursors_before, .. } = edit {
        restore_batch_cursors(model, cursors_before);
    } else if let Some(cursor_before) = edit_cursor_before(edit) {
        *model.editor_mut().primary_cursor_mut() = cursor_before;
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
    apply_redo_operation_buffer_only(model, edit);
    if let EditOperation::Batch { cursors_after, .. } = edit {
        restore_batch_cursors(model, cursors_after);
    } else if let Some(cursor_after) = edit_cursor_after(edit) {
        *model.editor_mut().primary_cursor_mut() = cursor_after;
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
