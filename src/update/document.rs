//! Document update functions for text editing and undo/redo

use crate::model::document::EditorEditState;
use std::ops::Range;

use crate::commands::Cmd;
use crate::messages::DocumentMsg;
use crate::model::{AppModel, Document, EditOperation, Selection};
use crate::util::text::char_type;

use super::editor::{cursors_in_reverse_order, lines_covered_by_all_cursors};
use super::text_edits::{apply_planned_edits, EditCarets, EditOffsetMap, PlannedEdit};

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

/// Redraw and schedule parsing/LSP effects without an edit to shift highlights.
fn redraw_with_syntax_parse(model: &mut AppModel) -> Cmd {
    super::text_edits::edit_effects(model, model.document().id, None)
}

/// Find the start of the word before the given offset
///
/// Uses direct character indexing instead of collecting to String/Vec to avoid
/// allocating the entire document prefix (which could be megabytes for large files).
pub(crate) fn word_start_before(buffer: &ropey::Rope, offset: usize) -> usize {
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
pub(crate) fn word_end_after(buffer: &ropey::Rope, offset: usize) -> usize {
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
pub(super) fn update_document(model: &mut AppModel, msg: DocumentMsg) -> Option<Cmd> {
    let result = update_document_inner(model, msg);
    if model.editor().is_plain_text_mode() {
        super::editor::compute_matched_brackets(model);
    }
    result
}

fn selection_range(document: &Document, selection: &Selection) -> Range<usize> {
    let start = selection.start();
    let end = selection.end();
    document.cursor_to_offset(start.line, start.column)
        ..document.cursor_to_offset(end.line, end.column)
}

/// Merge physical deletions, not UI selections: original caret order is retained
/// for undo and clipboard payloads even when the deleted ranges overlap.
fn merge_ranges(mut ranges: Vec<Range<usize>>) -> Vec<Range<usize>> {
    ranges.retain(|range| !range.is_empty());
    ranges.sort_unstable_by_key(|range| (range.start, range.end));
    let mut merged: Vec<Range<usize>> = Vec::new();
    for range in ranges {
        if let Some(previous) = merged.last_mut() {
            if range.start <= previous.end {
                previous.end = previous.end.max(range.end);
                continue;
            }
        }
        merged.push(range);
    }
    merged
}

fn plan_deletions(document: &Document, ranges: Vec<Range<usize>>) -> Vec<PlannedEdit> {
    merge_ranges(ranges)
        .into_iter()
        .rev()
        .map(|range| PlannedEdit {
            start: range.start,
            deleted: document.buffer.slice(range).to_string(),
            inserted: String::new(),
        })
        .collect()
}

#[derive(Clone, Copy)]
enum DeleteTarget {
    Backward,
    Forward,
    WordBackward,
    WordForward,
    Selection,
}

impl DeleteTarget {
    fn range(self, buffer: &ropey::Rope, offset: usize) -> Range<usize> {
        match self {
            Self::Backward => {
                let start = if offset >= 2
                    && buffer.char(offset - 1) == '\n'
                    && buffer.char(offset - 2) == '\r'
                {
                    offset - 2
                } else {
                    offset.saturating_sub(1)
                };
                start..offset
            }
            Self::Forward => {
                let end = if offset + 1 < buffer.len_chars()
                    && buffer.char(offset) == '\r'
                    && buffer.char(offset + 1) == '\n'
                {
                    offset + 2
                } else {
                    (offset + 1).min(buffer.len_chars())
                };
                offset..end
            }
            Self::WordBackward => word_start_before(buffer, offset)..offset,
            Self::WordForward => offset..word_end_after(buffer, offset),
            Self::Selection => offset..offset,
        }
    }
}

fn apply_at_cursors(
    model: &mut AppModel,
    planned: &[PlannedEdit],
    pristine: &[usize],
    before: Option<EditorEditState>,
) -> Option<Cmd> {
    model.reset_cursor_blink();
    if planned.is_empty() {
        return Some(redraw_with_syntax_parse(model));
    }
    let document_id = model.editor_area.focused_document_id()?;
    let editor_id = model.editor_area.focused_editor_id()?;
    let offsets: Vec<_> = {
        let map = EditOffsetMap::new(planned);
        pristine.iter().map(|&offset| map.map(offset)).collect()
    };
    apply_planned_edits(
        model,
        document_id,
        planned,
        EditCarets::Place {
            editor_id,
            offsets: &offsets,
            before,
        },
    )
}

fn delete_at_cursors(model: &mut AppModel, target: DeleteTarget) -> Option<Cmd> {
    let document = model.document();
    let pristine: Vec<_> = model
        .editor()
        .cursors
        .iter()
        .map(|cursor| document.cursor_to_offset(cursor.line, cursor.column))
        .collect();
    let ranges = model
        .editor()
        .selections
        .iter()
        .zip(&pristine)
        .map(|(selection, &offset)| {
            if selection.is_empty() {
                target.range(&document.buffer, offset)
            } else {
                selection_range(document, selection)
            }
        })
        .collect();
    let planned = plan_deletions(document, ranges);
    apply_at_cursors(model, &planned, &pristine, None)
}

fn selected_text(model: &AppModel) -> String {
    model
        .editor()
        .selections
        .iter()
        .filter(|selection| !selection.is_empty())
        .map(|selection| {
            model
                .document()
                .buffer
                .slice(selection_range(model.document(), selection))
                .to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn copy_or_cut(model: &mut AppModel, cut: bool) -> Option<Cmd> {
    let text = selected_text(model);
    let effects = if cut {
        let effects = delete_at_cursors(model, DeleteTarget::Selection);
        model.ensure_cursor_visible();
        effects
    } else {
        Some(redraw_with_syntax_parse(model))
    };
    if text.is_empty() {
        return effects;
    }
    let verb = if cut { "Cut" } else { "Copied" };
    model
        .ui
        .set_status(format!("{verb} {} chars", text.chars().count()));
    Some(Cmd::Batch(
        effects
            .into_iter()
            .chain([Cmd::CopyToClipboard(text)])
            .collect(),
    ))
}

fn indent_lines(model: &mut AppModel, unindent: bool) -> Option<Cmd> {
    let document_id = model.editor_area.focused_document_id()?;
    let document = model.document();
    let planned: Vec<_> = lines_covered_by_all_cursors(model)
        .into_iter()
        .filter_map(|line| {
            let start = document.cursor_to_offset(line, 0);
            if !unindent {
                return Some(PlannedEdit {
                    start,
                    deleted: String::new(),
                    inserted: "\t".into(),
                });
            }
            let text = document.buffer.line(line);
            let count = if text.chars().next() == Some('\t') {
                1
            } else {
                text.chars().take(4).take_while(|&ch| ch == ' ').count()
            };
            (count > 0).then(|| PlannedEdit {
                start,
                deleted: text.chars().take(count).collect(),
                inserted: String::new(),
            })
        })
        .collect();
    model.reset_cursor_blink();
    if planned.is_empty() {
        model.ensure_cursor_visible();
        return Some(redraw_with_syntax_parse(model));
    }
    apply_planned_edits(model, document_id, &planned, EditCarets::Preserve)
}

fn delete_lines(model: &mut AppModel) -> Option<Cmd> {
    let mut covered = if model.editor().has_multiple_cursors() {
        lines_covered_by_all_cursors(model)
    } else {
        vec![model.editor().primary_cursor().line]
    };
    let document = model.document();
    let total = document.line_count();
    covered.retain(|&line| line < total);
    covered.sort_unstable();
    covered.dedup();
    let runs = merge_ranges(covered.iter().map(|&line| line..line + 1).collect());
    if runs.is_empty() {
        return Some(redraw_with_syntax_parse(model));
    }
    let ranges = runs
        .iter()
        .map(|run| {
            // A trailing run takes its preceding line ending, rather than leaving
            // an extra blank final line. Grouping first handles adjacent EOF lines.
            let start = if run.end == total && run.start > 0 {
                document.cursor_to_offset(run.start - 1, document.line_length(run.start - 1))
            } else {
                document.cursor_to_offset(run.start, 0)
            };
            let end = document.cursor_to_offset(run.end, 0);
            start..end
        })
        .collect();
    let planned = plan_deletions(document, ranges);
    if planned.is_empty() {
        model.reset_cursor_blink();
        return Some(redraw_with_syntax_parse(model));
    }

    // Preserve the whole-line command's preferred-column policy by choosing a
    // surviving line in the pristine document, then mapping its offset normally.
    // No preview buffer or second buffer mutation is needed to place the caret.
    let surviving_line = |line| {
        let index = runs.partition_point(|run| run.end <= line);
        match runs.get(index) {
            Some(run) if run.start <= line => {
                if run.end < total {
                    run.end
                } else {
                    run.start.saturating_sub(1)
                }
            }
            _ => line,
        }
    };
    let contiguous = runs.len() == 1;
    let first_column = model.editor().primary_cursor().column;
    let pristine: Vec<_> = model
        .editor()
        .cursors
        .iter()
        .map(|cursor| {
            let (line, column) = if contiguous {
                (runs[0].start, first_column)
            } else {
                (cursor.line, cursor.column)
            };
            document.cursor_to_offset(surviving_line(line), column)
        })
        .collect();
    let old_top = model.editor().viewport.top_line;
    let deleted_above = covered.partition_point(|&line| line < old_top);
    let result = apply_at_cursors(model, &planned, &pristine, None);
    if deleted_above > 0 && !model.editor().soft_wrap {
        if let Some(editor_id) = model.editor_area.focused_editor_id() {
            model.set_editor_vertical_scroll(editor_id, old_top.saturating_sub(deleted_above));
            model.ensure_cursor_visible();
        }
    }
    result
}

fn duplicate_at_cursors(model: &mut AppModel) -> Option<Cmd> {
    let document_id = model.editor_area.focused_document_id()?;
    let editor_id = model.editor_area.focused_editor_id()?;
    let document = model.document();
    let mut entries = Vec::new();
    for index in cursors_in_reverse_order(model) {
        let cursor = model.editor().cursors[index];
        let selection = &model.editor().selections[index];
        let (start, inserted, relative_caret) = if selection.is_empty() {
            let text = document.get_line(cursor.line).unwrap_or_default();
            let column = cursor.column.min(document.line_length(cursor.line));
            if cursor.line + 1 < document.line_count() {
                (document.cursor_to_offset(cursor.line + 1, 0), text, column)
            } else {
                (document.buffer.len_chars(), format!("\n{text}"), 1 + column)
            }
        } else {
            let range = selection_range(document, selection);
            let text = document.buffer.slice(range.clone()).to_string();
            let length = text.chars().count();
            (range.end, text, length)
        };
        entries.push((
            index,
            relative_caret,
            PlannedEdit {
                start,
                deleted: String::new(),
                inserted,
            },
        ));
    }
    // Equal-point duplicates retain reverse cursor order, matching one copy per
    // cursor on a shared line. Sources above are all captured before mutation.
    entries.sort_by_key(|entry| std::cmp::Reverse(entry.2.start));
    let (placements, planned): (Vec<_>, Vec<_>) = entries
        .into_iter()
        .map(|(index, relative, edit)| ((index, relative), edit))
        .unzip();
    let mut offsets = vec![0; placements.len()];
    {
        let map = EditOffsetMap::new(&planned);
        for (rank, &(index, relative)) in placements.iter().enumerate() {
            // This caret is inside/after its own inserted copy. Only edits applied
            // afterward may shift it, including another insertion at the same point.
            offsets[index] = map.inserted_offset(rank, relative);
        }
    }
    model.reset_cursor_blink();
    apply_planned_edits(
        model,
        document_id,
        &planned,
        EditCarets::Place {
            editor_id,
            offsets: &offsets,
            before: None,
        },
    )
}

/// Plan typing/newlines/paste against the pristine document, then use the shared
/// mutation and position mapper. `close` surrounds nonempty selections with two
/// insertions so peer positions follow the surviving text, including on undo.
fn insert_at_cursors(
    model: &mut AppModel,
    text: &str,
    close: Option<char>,
    distribute_lines: bool,
) -> Option<Cmd> {
    if text.is_empty() {
        return Some(redraw_with_syntax_parse(model));
    }
    // Normalize only overlapping/touching ranges. Disjoint carets retain their
    // original order and active index, including reversed selections.
    let mut before = None;
    if model.editor().has_multiple_cursors() {
        let mut ranges: Vec<_> = model
            .editor()
            .selections
            .iter()
            .map(|selection| (selection.start(), selection.end()))
            .collect();
        ranges.sort_unstable();
        if ranges.windows(2).any(|pair| pair[1].0 <= pair[0].1) {
            before = model
                .editor_area
                .focused_editor_id()
                .map(|id| EditorEditState::capture(id, model.editor()));
            model.editor_mut().merge_overlapping_selections();
        }
    }

    let indices = cursors_in_reverse_order(model);
    // Only enough lines to decide whether distribution is possible. Single
    // caret paste needs no line vector; excess clipboard lines use full paste.
    let lines = (distribute_lines && indices.len() > 1)
        .then(|| text.lines().take(indices.len() + 1).collect::<Vec<_>>());
    let distributed = lines.as_ref().filter(|lines| lines.len() == indices.len());
    let document = model.document();
    let mut ends = vec![0; indices.len()];
    let mut planned = Vec::new();
    for (rank, &index) in indices.iter().enumerate() {
        let selection = model.editor().selections[index];
        let start = selection.start();
        let end = selection.end();
        let start = document.cursor_to_offset(start.line, start.column);
        let end = document.cursor_to_offset(end.line, end.column);
        ends[index] = end;
        if let Some(close) = close.filter(|_| start != end) {
            planned.push(PlannedEdit {
                start: end,
                deleted: String::new(),
                inserted: close.to_string(),
            });
            planned.push(PlannedEdit {
                start,
                deleted: String::new(),
                inserted: text.to_owned(),
            });
        } else {
            let inserted = distributed.map_or(text, |lines| lines[indices.len() - 1 - rank]);
            if start != end || !inserted.is_empty() {
                planned.push(PlannedEdit {
                    start,
                    deleted: document.buffer.slice(start..end).to_string(),
                    inserted: inserted.to_owned(),
                });
            }
        }
    }
    apply_at_cursors(model, &planned, &ends, before)
}

fn update_document_inner(model: &mut AppModel, msg: DocumentMsg) -> Option<Cmd> {
    // Skip text operations for non-text tabs
    if !matches!(model.editor().tab_content, crate::model::TabContent::Text) {
        return None;
    }

    // Editing invalidates occurrence state and semantic selection history.
    // Copy is the sole document command that cannot change either positions or text.
    if !matches!(msg, DocumentMsg::Copy) {
        let editor = model.editor_mut();
        editor.occurrence_state = None;
        editor.clear_selection_history();
    }

    match msg {
        DocumentMsg::InsertChar(ch) => {
            let close = model
                .config
                .auto_surround
                .then(|| surround_pair(ch))
                .flatten();
            insert_at_cursors(model, &ch.to_string(), close, false)
        }

        DocumentMsg::InsertNewline => insert_at_cursors(model, "\n", None, false),

        DocumentMsg::DeleteBackward => delete_at_cursors(model, DeleteTarget::Backward),
        DocumentMsg::DeleteForward => delete_at_cursors(model, DeleteTarget::Forward),
        DocumentMsg::DeleteWordBackward => delete_at_cursors(model, DeleteTarget::WordBackward),
        DocumentMsg::DeleteWordForward => delete_at_cursors(model, DeleteTarget::WordForward),

        DocumentMsg::DeleteLine => delete_lines(model),

        DocumentMsg::Undo => {
            if let Some(edit) = model.document_mut().undo_stack.pop() {
                apply_history_operation(model, &edit, HistoryDirection::Undo);
                let doc = model.document_mut();
                doc.redo_stack.push(edit);
                doc.refresh_modified();
                model.ensure_cursor_visible();
                model.reset_cursor_blink();
            }
            Some(redraw_with_syntax_parse(model))
        }

        DocumentMsg::Redo => {
            if let Some(edit) = model.document_mut().redo_stack.pop() {
                apply_history_operation(model, &edit, HistoryDirection::Redo);
                let doc = model.document_mut();
                doc.undo_stack.push(edit);
                doc.refresh_modified();
                model.ensure_cursor_visible();
                model.reset_cursor_blink();
            }
            Some(redraw_with_syntax_parse(model))
        }

        DocumentMsg::Copy => copy_or_cut(model, false),
        DocumentMsg::Cut => copy_or_cut(model, true),

        DocumentMsg::Paste => Some(Cmd::RequestClipboardPaste),

        DocumentMsg::InsertText(text) => {
            let result = insert_at_cursors(model, &text, None, true);
            if !text.is_empty() {
                model
                    .ui
                    .set_status(format!("Pasted {} chars", text.chars().count()));
            }
            result
        }

        DocumentMsg::Duplicate => duplicate_at_cursors(model),

        DocumentMsg::IndentLines => indent_lines(model, false),
        DocumentMsg::UnindentLines => indent_lines(model, true),
    }
}

#[derive(Clone, Copy)]
enum HistoryDirection {
    Undo,
    Redo,
}

/// Existing panes restore their own lossless state, independent of focus.
/// Panes created after the edit retain live positions mapped through the edits.
fn apply_history_operation(
    model: &mut AppModel,
    edit: &EditOperation,
    direction: HistoryDirection,
) {
    let Some(document_id) = model.editor_area.focused_document_id() else {
        return;
    };
    let states = match edit {
        EditOperation::Batch {
            editors_before,
            editors_after,
            ..
        } => match direction {
            HistoryDirection::Undo => editors_before.as_slice(),
            HistoryDirection::Redo => editors_after.as_slice(),
        },
        EditOperation::Insert { .. }
        | EditOperation::Delete { .. }
        | EditOperation::Replace { .. } => &[],
    };
    // Saved panes restore exact selection state below. Mapping every one of
    // their carets through every atom is redundant and quadratic in batch size.
    let mut positions = super::text_edits::EditPositions::capture(model, document_id, |id| {
        !states.iter().any(|state| state.editor_id == id)
    });
    apply_history_buffer(model, edit, direction, &mut positions);
    positions.restore(model, document_id);

    let cursor = match edit {
        EditOperation::Batch { .. } => {
            for state in states {
                if let Some(editor) = model.editor_area.editors.get_mut(&state.editor_id) {
                    if editor.document_id == Some(document_id) {
                        state.restore(editor);
                    }
                }
            }
            return;
        }
        EditOperation::Insert {
            cursor_before,
            cursor_after,
            ..
        }
        | EditOperation::Delete {
            cursor_before,
            cursor_after,
            ..
        }
        | EditOperation::Replace {
            cursor_before,
            cursor_after,
            ..
        } => match direction {
            HistoryDirection::Undo => cursor_before,
            HistoryDirection::Redo => cursor_after,
        },
    };
    // Legacy/synthetic atomic records have only one saved cursor.
    let editor = model.editor_mut();
    *editor.primary_cursor_mut() = *cursor;
    editor.collapse_selections_to_cursors();
}

/// Apply each atom in history order. Unlike a pristine planned edit list, batch
/// offsets may describe intermediate buffers; update positions at each atom.
fn apply_history_buffer(
    model: &mut AppModel,
    edit: &EditOperation,
    direction: HistoryDirection,
    positions: &mut super::text_edits::EditPositions,
) {
    let (position, deleted, inserted) = match edit {
        EditOperation::Insert { position, text, .. } => (*position, "", text.as_str()),
        EditOperation::Delete { position, text, .. } => (*position, text.as_str(), ""),
        EditOperation::Replace {
            position,
            deleted_text,
            inserted_text,
            ..
        } => (*position, deleted_text.as_str(), inserted_text.as_str()),
        EditOperation::Batch { operations, .. } => {
            match direction {
                HistoryDirection::Undo => {
                    for op in operations.iter().rev() {
                        apply_history_buffer(model, op, direction, positions);
                    }
                }
                HistoryDirection::Redo => {
                    for op in operations {
                        apply_history_buffer(model, op, direction, positions);
                    }
                }
            }
            return;
        }
    };
    let (deleted, inserted) = match direction {
        HistoryDirection::Undo => (inserted, deleted),
        HistoryDirection::Redo => (deleted, inserted),
    };
    let removed = deleted.chars().count();
    positions.transform(position, removed, inserted.chars().count());
    let doc = model.document_mut();
    doc.buffer.remove(position..position + removed);
    doc.buffer.insert(position, inserted);
}
