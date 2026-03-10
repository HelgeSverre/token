//! Editor content rendering (text area, gutter, cursors, scrollbars, image/binary tabs)

use crate::model::AppModel;

use super::frame::{Frame, TextPainter};
use super::geometry::{self, char_col_to_visual_col, expand_tabs_for_display};
use super::{button, scrollbar};

/// Cursor width in pixels
const CURSOR_WIDTH: usize = 2;
/// Cursor inset from top of line in pixels
const CURSOR_INSET: usize = 1;

/// Render only specific cursor lines (optimized path for cursor blink)
///
/// This function redraws only the specified line numbers, which is much faster
/// than redrawing the entire editor area. Used for cursor blink optimization.
///
/// For each dirty line, renders:
/// - Line background (editor bg or current line highlight)
/// - Gutter (line number)
/// - Text content with syntax highlighting
/// - Cursor (if visible and on this line)
pub fn render_cursor_lines_only(
    frame: &mut Frame,
    painter: &mut TextPainter,
    model: &AppModel,
    dirty_lines: &[usize],
) {
    let char_width = painter.char_width();
    let line_height = painter.line_height();

    // Get the focused group and its document
    let focused_group_id = model.editor_area.focused_group_id;
    let Some(group) = model.editor_area.groups.get(&focused_group_id) else {
        return;
    };

    let Some(editor_id) = group.active_editor_id() else {
        return;
    };

    let Some(editor) = model.editor_area.editors.get(&editor_id) else {
        return;
    };

    let Some(doc_id) = editor.document_id else {
        return;
    };

    let Some(document) = model.editor_area.documents.get(&doc_id) else {
        return;
    };

    // Use GroupLayout for all positioning (DPI-aware, single source of truth)
    let layout = geometry::GroupLayout::new(group, model, char_width);

    let visible_lines = layout.visible_lines(line_height);
    let end_line = (editor.viewport.top_line + visible_lines).min(document.buffer.len_lines());

    // Colors
    let bg_color = model.theme.editor.background.to_argb_u32();
    let current_line_color = model.theme.editor.current_line_background.to_argb_u32();
    let gutter_bg_color = model.theme.gutter.background.to_argb_u32();
    let line_num_color = model.theme.gutter.foreground.to_argb_u32();
    let line_num_active_color = model.theme.gutter.foreground_active.to_argb_u32();
    let text_color = model.theme.editor.foreground.to_argb_u32();
    let primary_cursor_color = model.theme.editor.cursor_color.to_argb_u32();
    let secondary_cursor_color = model.theme.editor.secondary_cursor_color.to_argb_u32();

    // Layout-derived values
    let rect_x = layout.rect_x();
    let rect_w = layout.rect_w();
    let gutter_right_x = layout.gutter_right_x;
    let gutter_width = layout.gutter_width();
    let group_text_start_x = layout.text_start_x;

    let text_start_x_offset = layout.text_start_x - rect_x;
    let visible_columns =
        ((rect_w as f32 - text_start_x_offset as f32) / char_width).floor() as usize;
    let max_chars = visible_columns;

    // Reusable buffers
    let mut adjusted_tokens: Vec<crate::syntax::HighlightToken> = Vec::with_capacity(32);
    let mut display_text_buf = String::with_capacity(max_chars + 16);

    for &doc_line in dirty_lines {
        // Skip lines outside viewport
        if doc_line < editor.viewport.top_line || doc_line >= end_line {
            continue;
        }

        // Use layout helper for line Y position
        let Some(y) = layout.line_to_screen_y(doc_line, editor.viewport.top_line, line_height)
        else {
            continue;
        };

        // 1. Clear line background (gutter + text area)
        let is_cursor_line = doc_line == editor.active_cursor().line;

        // Clear gutter area for this line
        frame.fill_rect_px(rect_x, y, gutter_width, line_height, gutter_bg_color);

        // Clear text area for this line
        let text_area_x = gutter_right_x + 1; // After gutter border
        let text_area_w = rect_w.saturating_sub(gutter_width + 1);
        if is_cursor_line {
            frame.fill_rect_px(text_area_x, y, text_area_w, line_height, current_line_color);
        } else {
            frame.fill_rect_px(text_area_x, y, text_area_w, line_height, bg_color);
        }

        // 1b. Render selection highlights for this line
        let selection_color = model.theme.editor.selection_background.to_argb_u32();
        for selection in &editor.selections {
            if selection.is_empty() {
                continue;
            }

            let sel_start = selection.start();
            let sel_end = selection.end();

            // Check if this line is within the selection range
            if doc_line < sel_start.line || doc_line > sel_end.line {
                continue;
            }

            let line_len = document.line_length(doc_line);
            let line_text = document.get_line_cow(doc_line).unwrap_or_default();

            let start_col = if doc_line == sel_start.line {
                sel_start.column
            } else {
                0
            };
            let end_col = if doc_line == sel_end.line {
                sel_end.column
            } else {
                line_len
            };

            let visual_start_col = char_col_to_visual_col(&line_text, start_col);
            let visual_end_col = char_col_to_visual_col(&line_text, end_col);

            let visible_start_col = visual_start_col.saturating_sub(editor.viewport.left_column);
            let visible_end_col = visual_end_col.saturating_sub(editor.viewport.left_column);

            let x_start =
                group_text_start_x + (visible_start_col as f32 * char_width).round() as usize;
            let x_end = (group_text_start_x
                + (visible_end_col as f32 * char_width).round() as usize)
                .min(rect_x + rect_w);

            if x_end > x_start {
                frame.fill_rect_px(
                    x_start,
                    y,
                    x_end.saturating_sub(x_start),
                    line_height,
                    selection_color,
                );
            }
        }

        // 1c. Render rectangle selection highlight for this line
        if editor.rectangle_selection.active {
            let rect_sel = &editor.rectangle_selection;
            let top_line = rect_sel.top_line();
            let bottom_line = rect_sel.bottom_line();

            if doc_line >= top_line && doc_line <= bottom_line {
                let left_visual_col = rect_sel.left_visual_col();
                let right_visual_col = rect_sel.right_visual_col();
                let current_visual_col = rect_sel.current_visual_col;

                let line_text = document.get_line_cow(doc_line).unwrap_or_default();
                let line_visual_len = char_col_to_visual_col(&line_text, line_text.chars().count());

                // Only show highlight if current position is within the line's visual width
                if current_visual_col <= line_visual_len {
                    let start_visual = left_visual_col.min(line_visual_len);
                    let end_visual = right_visual_col.min(line_visual_len);

                    if start_visual < end_visual {
                        let visible_start_col =
                            start_visual.saturating_sub(editor.viewport.left_column);
                        let visible_end_col =
                            end_visual.saturating_sub(editor.viewport.left_column);

                        let x_start = group_text_start_x
                            + (visible_start_col as f32 * char_width).round() as usize;
                        let x_end = (group_text_start_x
                            + (visible_end_col as f32 * char_width).round() as usize)
                            .min(rect_x + rect_w);

                        if x_end > x_start {
                            frame.fill_rect_px(
                                x_start,
                                y,
                                x_end.saturating_sub(x_start),
                                line_height,
                                selection_color,
                            );
                        }
                    }
                }
            }
        }

        // 1d. Render matching bracket highlights for this line
        if let Some((pos_a, pos_b)) = editor.matched_brackets {
            let bracket_bg = model.theme.editor.bracket_match_background.to_argb_u32();
            for &pos in &[pos_a, pos_b] {
                if pos.line == doc_line {
                    let line_text = document.get_line_cow(doc_line).unwrap_or_default();
                    let visual_col = char_col_to_visual_col(&line_text, pos.column);
                    let visible_col = visual_col.saturating_sub(editor.viewport.left_column);
                    if visual_col >= editor.viewport.left_column && visible_col < visible_columns {
                        let x =
                            group_text_start_x + (visible_col as f32 * char_width).round() as usize;
                        let w = char_width.round() as usize;
                        frame.blend_rect_px(x, y, w, line_height, bracket_bg);
                    }
                }
            }
        }

        // 2. Render gutter (line number)
        let line_num_str = format!("{}", doc_line + 1);
        let text_width_px = (line_num_str.len() as f32 * char_width).round() as usize;
        let gutter_text_x =
            gutter_right_x.saturating_sub(model.metrics.padding_medium + text_width_px);
        let line_color = if is_cursor_line {
            line_num_active_color
        } else {
            line_num_color
        };
        painter.draw(frame, gutter_text_x, y, &line_num_str, line_color);

        // 3. Render text content with syntax highlighting
        if let Some(line_text) = document.get_line_cow(doc_line) {
            let expanded_text = expand_tabs_for_display(&line_text);

            display_text_buf.clear();
            for ch in expanded_text
                .chars()
                .skip(editor.viewport.left_column)
                .take(max_chars)
            {
                display_text_buf.push(ch);
            }

            // Get syntax highlights
            let line_tokens = document.get_line_highlights(doc_line);

            adjusted_tokens.clear();
            for t in line_tokens.iter() {
                let visual_start = char_col_to_visual_col(&line_text, t.start_col);
                let visual_end = char_col_to_visual_col(&line_text, t.end_col);

                let start = visual_start.saturating_sub(editor.viewport.left_column);
                let end = visual_end.saturating_sub(editor.viewport.left_column);

                if end > 0 && start < max_chars {
                    adjusted_tokens.push(crate::syntax::HighlightToken {
                        start_col: start,
                        end_col: end.min(max_chars),
                        highlight: t.highlight,
                    });
                }
            }

            if adjusted_tokens.is_empty() {
                painter.draw(frame, group_text_start_x, y, &display_text_buf, text_color);
            } else {
                painter.draw_with_highlights(
                    frame,
                    group_text_start_x,
                    y,
                    &display_text_buf,
                    &adjusted_tokens,
                    &model.theme.syntax,
                    text_color,
                );
            }
        }

        // 4. Render cursors on this line (if visible)
        if model.ui.cursor_visible {
            for (idx, cursor) in editor.cursors.iter().enumerate() {
                if cursor.line != doc_line {
                    continue;
                }

                let line_text = document.get_line_cow(cursor.line).unwrap_or_default();
                let visual_cursor_col = char_col_to_visual_col(&line_text, cursor.column);

                let cursor_in_horizontal_view = visual_cursor_col >= editor.viewport.left_column
                    && visual_cursor_col < editor.viewport.left_column + visible_columns;

                if cursor_in_horizontal_view {
                    let cursor_visual_column = visual_cursor_col - editor.viewport.left_column;
                    let cursor_x = (group_text_start_x as f32
                        + cursor_visual_column as f32 * char_width)
                        .round() as usize;

                    let cursor_color = if idx == 0 {
                        primary_cursor_color
                    } else {
                        secondary_cursor_color
                    };

                    frame.fill_rect_px(
                        cursor_x,
                        y + CURSOR_INSET,
                        CURSOR_WIDTH,
                        line_height.saturating_sub(CURSOR_INSET * 2),
                        cursor_color,
                    );
                }
            }
        }
    }
}

/// Render text content (lines, selections, cursors) for an editor group.
///
/// Draws:
/// - Current line highlight
/// - Selection highlights
/// - Text content
/// - Cursors (only if group is focused)
pub fn render_text_area(
    frame: &mut Frame,
    painter: &mut TextPainter,
    model: &AppModel,
    editor: &crate::model::EditorState,
    document: &crate::model::Document,
    layout: &geometry::GroupLayout,
    is_focused: bool,
) {
    let char_width = painter.char_width();
    let line_height = painter.line_height();
    let rect_x = layout.rect_x();
    let rect_w = layout.rect_w();
    let content_y = layout.content_y();
    let content_h = layout.content_h();
    let group_text_start_x = layout.text_start_x;

    let text_start_x_offset = layout.text_start_x - rect_x;
    let visible_lines = layout.visible_lines(line_height);
    let visible_columns =
        ((rect_w as f32 - text_start_x_offset as f32) / char_width).floor() as usize;
    let end_line = (editor.viewport.top_line + visible_lines).min(document.buffer.len_lines());

    // Current line highlight
    let current_line_color = model.theme.editor.current_line_background.to_argb_u32();
    if editor.active_cursor().line >= editor.viewport.top_line
        && editor.active_cursor().line < end_line
    {
        let screen_line = editor.active_cursor().line - editor.viewport.top_line;
        let highlight_y = content_y + screen_line * line_height;
        let highlight_h = line_height.min(content_y + content_h - highlight_y);
        frame.fill_rect_px(rect_x, highlight_y, rect_w, highlight_h, current_line_color);
    }

    // Selection highlights
    let selection_color = model.theme.editor.selection_background.to_argb_u32();
    for selection in &editor.selections {
        if selection.is_empty() {
            continue;
        }

        let sel_start = selection.start();
        let sel_end = selection.end();

        for doc_line in editor.viewport.top_line..end_line {
            if doc_line < sel_start.line || doc_line > sel_end.line {
                continue;
            }

            let screen_line = doc_line - editor.viewport.top_line;
            let y_start = content_y + screen_line * line_height;
            let y_end = (y_start + line_height).min(content_y + content_h);

            let line_len = document.line_length(doc_line);

            // Use get_line_cow for zero-allocation when line is contiguous
            let line_text = document.get_line_cow(doc_line).unwrap_or_default();

            let start_col = if doc_line == sel_start.line {
                sel_start.column
            } else {
                0
            };
            let end_col = if doc_line == sel_end.line {
                sel_end.column
            } else {
                line_len
            };

            let visual_start_col = char_col_to_visual_col(&line_text, start_col);
            let visual_end_col = char_col_to_visual_col(&line_text, end_col);

            let visible_start_col = visual_start_col.saturating_sub(editor.viewport.left_column);
            let visible_end_col = visual_end_col.saturating_sub(editor.viewport.left_column);

            let x_start =
                group_text_start_x + (visible_start_col as f32 * char_width).round() as usize;
            let x_end = (group_text_start_x
                + (visible_end_col as f32 * char_width).round() as usize)
                .min(rect_x + rect_w);

            frame.fill_rect_px(
                x_start,
                y_start,
                x_end.saturating_sub(x_start),
                y_end.saturating_sub(y_start),
                selection_color,
            );
        }
    }

    // Rectangle selection highlight (middle mouse drag preview)
    // Uses visual columns (screen position) for consistent behavior across lines
    if editor.rectangle_selection.active {
        let rect_sel = &editor.rectangle_selection;
        let top_line = rect_sel.top_line();
        let bottom_line = rect_sel.bottom_line();
        let left_visual_col = rect_sel.left_visual_col();
        let right_visual_col = rect_sel.right_visual_col();
        let current_visual_col = rect_sel.current_visual_col;

        let visible_start = top_line.max(editor.viewport.top_line);
        let visible_end = (bottom_line + 1).min(end_line);

        for doc_line in visible_start..visible_end {
            // Use get_line_cow for zero-allocation when line is contiguous
            let line_text = document.get_line_cow(doc_line).unwrap_or_default();
            let line_visual_len = char_col_to_visual_col(&line_text, line_text.chars().count());

            // Only show highlight if current position is within the line's visual width
            // (not dragging past line end)
            if current_visual_col > line_visual_len {
                continue;
            }

            // Clamp visual columns to line's visual width
            let start_visual = left_visual_col.min(line_visual_len);
            let end_visual = right_visual_col.min(line_visual_len);

            // Skip lines where selection would be empty
            if start_visual >= end_visual {
                continue;
            }

            let screen_line = doc_line - editor.viewport.top_line;
            let y_start = content_y + screen_line * line_height;
            let y_end = (y_start + line_height).min(content_y + content_h);

            let visible_start_col = start_visual.saturating_sub(editor.viewport.left_column);
            let visible_end_col = end_visual.saturating_sub(editor.viewport.left_column);

            let x_start =
                group_text_start_x + (visible_start_col as f32 * char_width).round() as usize;
            let x_end = (group_text_start_x
                + (visible_end_col as f32 * char_width).round() as usize)
                .min(rect_x + rect_w);

            if x_end > x_start {
                frame.fill_rect_px(
                    x_start,
                    y_start,
                    x_end.saturating_sub(x_start),
                    y_end.saturating_sub(y_start),
                    selection_color,
                );
            }
        }
    }

    // Matching bracket highlights
    if let Some((pos_a, pos_b)) = editor.matched_brackets {
        let bracket_bg = model.theme.editor.bracket_match_background.to_argb_u32();
        for &pos in &[pos_a, pos_b] {
            if pos.line >= editor.viewport.top_line && pos.line < end_line {
                let screen_line = pos.line - editor.viewport.top_line;
                let y_start = content_y + screen_line * line_height;
                let y_end = (y_start + line_height).min(content_y + content_h);
                let line_text = document.get_line_cow(pos.line).unwrap_or_default();
                let visual_col = char_col_to_visual_col(&line_text, pos.column);
                let visible_col = visual_col.saturating_sub(editor.viewport.left_column);
                if visual_col >= editor.viewport.left_column && visible_col < visible_columns {
                    let x = group_text_start_x + (visible_col as f32 * char_width).round() as usize;
                    let w = char_width.round() as usize;
                    frame.blend_rect_px(x, y_start, w, y_end.saturating_sub(y_start), bracket_bg);
                }
            }
        }
    }

    // Text content with syntax highlighting
    // Reuse buffers to avoid per-line allocations
    let text_color = model.theme.editor.foreground.to_argb_u32();
    let max_chars = visible_columns;
    let mut adjusted_tokens: Vec<crate::syntax::HighlightToken> = Vec::with_capacity(32); // Reused across lines
    let mut display_text_buf = String::with_capacity(max_chars + 16); // Reused for display

    for (screen_line, doc_line) in (editor.viewport.top_line..end_line).enumerate() {
        // Use get_line_cow for zero-allocation when line is contiguous
        if let Some(line_text) = document.get_line_cow(doc_line) {
            let y = content_y + screen_line * line_height;
            if y >= content_y + content_h {
                break;
            }

            // expand_tabs_for_display returns Cow - no allocation if no tabs
            let expanded_text = expand_tabs_for_display(&line_text);

            // Reuse display_text buffer instead of allocating new String each line
            display_text_buf.clear();
            for ch in expanded_text
                .chars()
                .skip(editor.viewport.left_column)
                .take(max_chars)
            {
                display_text_buf.push(ch);
            }

            // Get syntax highlights for this line
            let line_tokens = document.get_line_highlights(doc_line);

            // Reuse adjusted_tokens buffer instead of allocating new Vec each line
            adjusted_tokens.clear();
            for t in line_tokens.iter() {
                // Convert character columns to visual columns (accounting for tabs)
                let visual_start = char_col_to_visual_col(&line_text, t.start_col);
                let visual_end = char_col_to_visual_col(&line_text, t.end_col);

                // Adjust for horizontal scroll
                let start = visual_start.saturating_sub(editor.viewport.left_column);
                let end = visual_end.saturating_sub(editor.viewport.left_column);

                if end > 0 && start < max_chars {
                    adjusted_tokens.push(crate::syntax::HighlightToken {
                        start_col: start,
                        end_col: end.min(max_chars),
                        highlight: t.highlight,
                    });
                }
            }

            if adjusted_tokens.is_empty() {
                painter.draw(frame, group_text_start_x, y, &display_text_buf, text_color);
            } else {
                painter.draw_with_highlights(
                    frame,
                    group_text_start_x,
                    y,
                    &display_text_buf,
                    &adjusted_tokens,
                    &model.theme.syntax,
                    text_color,
                );
            }
        }
    }

    // Cursors: only show in focused group when blink state is visible
    if is_focused && model.ui.cursor_visible {
        let primary_cursor_color = model.theme.editor.cursor_color.to_argb_u32();
        let secondary_cursor_color = model.theme.editor.secondary_cursor_color.to_argb_u32();

        for (idx, cursor) in editor.cursors.iter().enumerate() {
            let cursor_in_vertical_view = cursor.line >= editor.viewport.top_line
                && cursor.line < editor.viewport.top_line + visible_lines;

            // Use get_line_cow for zero-allocation when line is contiguous
            let line_text = document.get_line_cow(cursor.line).unwrap_or_default();
            let visual_cursor_col = char_col_to_visual_col(&line_text, cursor.column);

            let cursor_in_horizontal_view = visual_cursor_col >= editor.viewport.left_column
                && visual_cursor_col < editor.viewport.left_column + visible_columns;

            if cursor_in_vertical_view && cursor_in_horizontal_view {
                let screen_line = cursor.line - editor.viewport.top_line;
                let cursor_visual_column = visual_cursor_col - editor.viewport.left_column;
                let x = (group_text_start_x as f32 + cursor_visual_column as f32 * char_width)
                    .round() as usize;
                let y = content_y + screen_line * line_height;

                let cursor_color = if idx == 0 {
                    primary_cursor_color
                } else {
                    secondary_cursor_color
                };

                // Cursor: 2px wide, line_height - 2 tall, offset by 1px from top
                frame.fill_rect_px(
                    x,
                    y + CURSOR_INSET,
                    CURSOR_WIDTH,
                    line_height.saturating_sub(CURSOR_INSET * 2),
                    cursor_color,
                );
            }
        }
    }

    // Preview cursors for rectangle selection (always visible during drag, no blink)
    if is_focused && editor.rectangle_selection.active {
        let secondary_cursor_color = model.theme.editor.secondary_cursor_color.to_argb_u32();

        for preview_pos in &editor.rectangle_selection.preview_cursors {
            let cursor_in_vertical_view = preview_pos.line >= editor.viewport.top_line
                && preview_pos.line < editor.viewport.top_line + visible_lines;

            if !cursor_in_vertical_view {
                continue;
            }

            // Use get_line_cow for zero-allocation when line is contiguous
            let line_text = document.get_line_cow(preview_pos.line).unwrap_or_default();
            let visual_cursor_col = char_col_to_visual_col(&line_text, preview_pos.column);

            let cursor_in_horizontal_view = visual_cursor_col >= editor.viewport.left_column
                && visual_cursor_col < editor.viewport.left_column + visible_columns;

            if !cursor_in_horizontal_view {
                continue;
            }

            let screen_line = preview_pos.line - editor.viewport.top_line;
            let cursor_visual_column = visual_cursor_col - editor.viewport.left_column;
            let x = (group_text_start_x as f32 + cursor_visual_column as f32 * char_width).round()
                as usize;
            let y = content_y + screen_line * line_height;

            frame.fill_rect_px(
                x,
                y + CURSOR_INSET,
                CURSOR_WIDTH,
                line_height.saturating_sub(CURSOR_INSET * 2),
                secondary_cursor_color,
            );
        }
    }
}

/// Render the gutter (line numbers) for an editor group.
pub fn render_gutter(
    frame: &mut Frame,
    painter: &mut TextPainter,
    model: &AppModel,
    editor: &crate::model::EditorState,
    document: &crate::model::Document,
    layout: &geometry::GroupLayout,
) {
    let gutter_bg_color = model.theme.gutter.background.to_argb_u32();
    let line_num_color = model.theme.gutter.foreground.to_argb_u32();
    let line_num_active_color = model.theme.gutter.foreground_active.to_argb_u32();
    let char_width = painter.char_width();
    let line_height = painter.line_height();

    let rect_x = layout.rect_x();
    let content_y = layout.content_y();
    let content_h = layout.content_h();
    let gutter_right_x = layout.gutter_right_x;
    let gutter_width = layout.gutter_width();

    let visible_lines = layout.visible_lines(line_height);
    let end_line = (editor.viewport.top_line + visible_lines).min(document.buffer.len_lines());

    let gutter_border_color = model.theme.gutter.border_color.to_argb_u32();

    // Draw gutter background
    frame.fill_rect_px(rect_x, content_y, gutter_width, content_h, gutter_bg_color);

    for (screen_line, doc_line) in (editor.viewport.top_line..end_line).enumerate() {
        let y = content_y + screen_line * line_height;
        if y >= content_y + content_h {
            break;
        }

        // Right-align line numbers so they sit just left of the gutter border.
        let line_num_str = format!("{}", doc_line + 1);
        let text_width_px = (line_num_str.len() as f32 * char_width).round() as usize;
        let text_x = gutter_right_x.saturating_sub(model.metrics.padding_medium + text_width_px);

        let line_color = if doc_line == editor.active_cursor().line {
            line_num_active_color
        } else {
            line_num_color
        };
        painter.draw(frame, text_x, y, &line_num_str, line_color);
    }

    // Gutter border
    frame.fill_rect_px(gutter_right_x, content_y, 1, content_h, gutter_border_color);
}

/// Render vertical (and horizontal if needed) scrollbars for a text editor pane.
pub fn render_editor_scrollbars(
    frame: &mut Frame,
    model: &AppModel,
    editor: &crate::model::editor::EditorState,
    document: &crate::model::document::Document,
    layout: &geometry::GroupLayout,
) {
    use scrollbar::{render_scrollbar, ScrollbarColors, ScrollbarGeometry, ScrollbarState};

    let sw = model.metrics.scrollbar_width;
    let colors = ScrollbarColors {
        track: model.theme.scrollbar.track.to_argb_u32(),
        thumb: model.theme.scrollbar.thumb.to_argb_u32(),
        thumb_hover: model.theme.scrollbar.thumb_hover.to_argb_u32(),
    };

    let viewport = &editor.viewport;
    let line_count = document.line_count();
    let visible_lines = layout.visible_lines(model.line_height);
    let visible_columns = layout.visible_columns(model.char_width);

    // Vertical scrollbar (always present when scrollbars are enabled)
    if let Some(v_track) = layout.v_scrollbar_rect(sw) {
        let v_state = ScrollbarState::new(line_count, visible_lines, viewport.top_line);
        let v_geo = ScrollbarGeometry::vertical(v_track, &v_state);
        render_scrollbar(frame, &v_geo, false, &colors);
    }

    // Horizontal scrollbar (only when content is wider than viewport)
    if let Some(h_track) = layout.h_scrollbar_rect(sw) {
        // Compute max line length from visible lines for horizontal scroll
        let top = viewport.top_line;
        let bottom = (top + visible_lines).min(line_count);
        let max_len = (top..bottom)
            .map(|i| document.line_length(i))
            .max()
            .unwrap_or(0);
        let h_state = ScrollbarState::new(max_len, visible_columns, viewport.left_column);
        if h_state.needs_scroll() {
            let h_geo = ScrollbarGeometry::horizontal(h_track, &h_state);
            render_scrollbar(frame, &h_geo, false, &colors);
        }
    }
}

/// Render an image viewer tab
pub fn render_image_tab(
    frame: &mut Frame,
    _painter: &mut TextPainter,
    model: &AppModel,
    img_state: &crate::image::ImageState,
    layout: &geometry::GroupLayout,
) {
    let content_rect = layout.content_rect;
    let bg = model.theme.editor.background.to_argb_u32();
    frame.fill_rect(content_rect, bg);

    let padding = model.metrics.padding_large * 2;
    let dest_x = content_rect.x as usize + padding;
    let dest_y = content_rect.y as usize + padding;
    let dest_w = (content_rect.width as usize).saturating_sub(padding * 2);
    let dest_h = (content_rect.height as usize).saturating_sub(padding * 2);

    if dest_w > 0 && dest_h > 0 {
        // Draw checkerboard pattern for transparency
        let ip = &model.theme.image_preview;
        let check_size = ip.checkerboard_size;
        let light = ip.checkerboard_light.to_argb_u32();
        let dark = ip.checkerboard_dark.to_argb_u32();
        for cy in 0..dest_h {
            for cx in 0..dest_w {
                let px = dest_x + cx;
                let py = dest_y + cy;
                let checker = ((cx / check_size) + (cy / check_size)).is_multiple_of(2);
                frame.set_pixel(px, py, if checker { light } else { dark });
            }
        }

        frame.blit_rgba_scaled(
            &img_state.pixels,
            img_state.width,
            img_state.height,
            dest_x,
            dest_y,
            dest_w,
            dest_h,
        );
    }
}

/// Render a binary file placeholder tab
pub fn render_binary_placeholder(
    frame: &mut Frame,
    painter: &mut TextPainter,
    model: &AppModel,
    placeholder: &crate::model::editor::BinaryPlaceholderState,
    layout: &geometry::GroupLayout,
) {
    let content_rect = layout.content_rect;
    let bg = model.theme.editor.background.to_argb_u32();
    let fg = model.theme.editor.foreground.to_argb_u32();
    let dim_fg = model.theme.gutter.foreground.to_argb_u32();
    frame.fill_rect(content_rect, bg);

    let char_width = painter.char_width();
    let line_height = painter.line_height();
    let btn_label = geometry::BINARY_PLACEHOLDER_BUTTON_LABEL;
    let bp_layout = geometry::binary_placeholder_layout(
        content_rect,
        line_height,
        char_width,
        model.metrics.padding_large,
        model.metrics.padding_medium,
        btn_label,
    );

    // Filename
    let filename = placeholder
        .path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy();
    let name_x = bp_layout
        .center_x
        .saturating_sub((filename.len() as f32 * char_width / 2.0) as usize);
    painter.draw(frame, name_x, bp_layout.name_y, &filename, fg);

    // File size
    let size_str = format_file_size(placeholder.size_bytes);
    let size_x = bp_layout
        .center_x
        .saturating_sub((size_str.len() as f32 * char_width / 2.0) as usize);
    painter.draw(frame, size_x, bp_layout.size_y, &size_str, dim_fg);

    // "Open with Default Application" button
    let btn_rect = bp_layout.button_rect;

    let btn_state = if model.ui.hover == crate::model::ui::HoverRegion::Button {
        button::ButtonState::Hovered
    } else {
        button::ButtonState::Normal
    };

    button::render_button(
        frame,
        painter,
        &model.theme,
        btn_rect,
        btn_label,
        btn_state,
        true,
    );
}

fn format_file_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.1} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}
