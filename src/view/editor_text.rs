//! Text editor content rendering (text area, gutter, cursors).

use crate::model::editor::Selection;
use crate::model::{AppModel, Document, EditorState};

use super::frame::{Frame, TextPainter};
use super::geometry::{self, char_col_to_visual_col, column_to_pixel_x, expand_tabs_for_display};

/// Cursor width in pixels.
const CURSOR_WIDTH: usize = 2;
/// Cursor inset from top of line in pixels.
const CURSOR_INSET: usize = 1;

/// Shared theme colors for text editor rendering.
#[derive(Debug, Clone, Copy)]
struct EditorPalette {
    background: u32,
    current_line: u32,
    selection: u32,
    bracket_match: u32,
    text: u32,
    gutter_background: u32,
    gutter_border: u32,
    line_number: u32,
    active_line_number: u32,
    primary_cursor: u32,
    secondary_cursor: u32,
}

impl EditorPalette {
    fn from_model(model: &AppModel) -> Self {
        Self {
            background: model.theme.editor.background.to_argb_u32(),
            current_line: model.theme.editor.current_line_background.to_argb_u32(),
            selection: model.theme.editor.selection_background.to_argb_u32(),
            bracket_match: model.theme.editor.bracket_match_background.to_argb_u32(),
            text: model.theme.editor.foreground.to_argb_u32(),
            gutter_background: model.theme.gutter.background.to_argb_u32(),
            gutter_border: model.theme.gutter.border_color.to_argb_u32(),
            line_number: model.theme.gutter.foreground.to_argb_u32(),
            active_line_number: model.theme.gutter.foreground_active.to_argb_u32(),
            primary_cursor: model.theme.editor.cursor_color.to_argb_u32(),
            secondary_cursor: model.theme.editor.secondary_cursor_color.to_argb_u32(),
        }
    }
}

/// Shared layout-derived values for editor text rendering.
struct EditorRenderContext<'a> {
    layout: &'a geometry::GroupLayout,
    char_width: f32,
    line_height: usize,
    rect_x: usize,
    rect_w: usize,
    content_y: usize,
    content_h: usize,
    gutter_right_x: usize,
    gutter_width: usize,
    text_start_x: usize,
    visible_lines: usize,
    visible_columns: usize,
    end_line: usize,
}

impl<'a> EditorRenderContext<'a> {
    fn new(
        layout: &'a geometry::GroupLayout,
        editor: &EditorState,
        document: &Document,
        char_width: f32,
        line_height: usize,
    ) -> Self {
        let visible_lines = layout.visible_lines(line_height);
        let visible_columns = layout.visible_columns(char_width);

        Self {
            layout,
            char_width,
            line_height,
            rect_x: layout.rect_x(),
            rect_w: layout.rect_w(),
            content_y: layout.content_y(),
            content_h: layout.content_h(),
            gutter_right_x: layout.gutter_right_x,
            gutter_width: layout.gutter_width(),
            text_start_x: layout.text_start_x,
            visible_lines,
            visible_columns,
            end_line: editor
                .viewport
                .top_line
                .saturating_add(visible_lines)
                .min(document.buffer.len_lines()),
        }
    }

    #[inline]
    fn line_y(&self, doc_line: usize, viewport_top: usize) -> Option<usize> {
        self.layout
            .line_to_screen_y(doc_line, viewport_top, self.line_height)
    }

    #[inline]
    fn text_right_x(&self) -> usize {
        self.rect_x + self.rect_w
    }

    #[inline]
    fn pixel_x(&self, visual_col: usize, viewport_left: usize) -> usize {
        column_to_pixel_x(
            visual_col,
            viewport_left,
            self.text_start_x,
            self.char_width,
        )
    }

    #[inline]
    fn clipped_span_x(
        &self,
        start_visual: usize,
        end_visual: usize,
        viewport_left: usize,
    ) -> (usize, usize) {
        (
            self.pixel_x(start_visual, viewport_left),
            self.pixel_x(end_visual, viewport_left)
                .min(self.text_right_x()),
        )
    }

    #[inline]
    fn contains_visual_col(&self, visual_col: usize, viewport_left: usize) -> bool {
        visual_col >= viewport_left
            && visual_col < viewport_left.saturating_add(self.visible_columns)
    }
}

/// Reused buffers for syntax-highlighted text line rendering.
struct EditorTextBuffers {
    adjusted_tokens: Vec<crate::syntax::HighlightToken>,
    display_text: String,
}

impl EditorTextBuffers {
    fn new(max_chars: usize) -> Self {
        Self {
            adjusted_tokens: Vec::with_capacity(32),
            display_text: String::with_capacity(max_chars + 16),
        }
    }
}

/// Stateful text editor renderer.
///
/// This owns the derived layout/theme state for a render pass and exposes
/// methods for the different text-editor render paths instead of threading the
/// same state through a graph of free functions.
struct TextEditorRenderer<'a> {
    model: &'a AppModel,
    editor: &'a EditorState,
    document: &'a Document,
    ctx: EditorRenderContext<'a>,
    palette: EditorPalette,
    text_buffers: EditorTextBuffers,
}

impl<'a> TextEditorRenderer<'a> {
    fn new(
        model: &'a AppModel,
        editor: &'a EditorState,
        document: &'a Document,
        layout: &'a geometry::GroupLayout,
        char_width: f32,
        line_height: usize,
    ) -> Self {
        let ctx = EditorRenderContext::new(layout, editor, document, char_width, line_height);
        let palette = EditorPalette::from_model(model);
        let text_buffers = EditorTextBuffers::new(ctx.visible_columns);

        Self {
            model,
            editor,
            document,
            ctx,
            palette,
            text_buffers,
        }
    }

    #[inline]
    fn viewport_left(&self) -> usize {
        self.editor.viewport.left_column
    }

    #[inline]
    fn viewport_top(&self) -> usize {
        self.editor.viewport.top_line
    }

    #[inline]
    fn line_screen_y(&self, doc_line: usize) -> Option<usize> {
        self.ctx.line_y(doc_line, self.viewport_top())
    }

    fn selection_span_for_line(
        &self,
        selection: &Selection,
        doc_line: usize,
    ) -> Option<(usize, usize)> {
        if selection.is_empty() {
            return None;
        }

        let sel_start = selection.start();
        let sel_end = selection.end();
        if doc_line < sel_start.line || doc_line > sel_end.line {
            return None;
        }

        let line_len = self.document.line_length(doc_line);
        let line_text = self.document.get_line_cow(doc_line).unwrap_or_default();

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
        Some(
            self.ctx
                .clipped_span_x(visual_start_col, visual_end_col, self.viewport_left()),
        )
    }

    fn rectangle_selection_span_for_line(&self, doc_line: usize) -> Option<(usize, usize)> {
        let rect_sel = &self.editor.rectangle_selection;
        if !rect_sel.active || doc_line < rect_sel.top_line() || doc_line > rect_sel.bottom_line() {
            return None;
        }

        let left_visual_col = rect_sel.left_visual_col();
        let right_visual_col = rect_sel.right_visual_col();
        let current_visual_col = rect_sel.current_visual_col;

        let line_text = self.document.get_line_cow(doc_line).unwrap_or_default();
        let line_visual_len = char_col_to_visual_col(&line_text, line_text.chars().count());

        if current_visual_col > line_visual_len {
            return None;
        }

        let start_visual = left_visual_col.min(line_visual_len);
        let end_visual = right_visual_col.min(line_visual_len);
        if start_visual >= end_visual {
            return None;
        }

        Some(
            self.ctx
                .clipped_span_x(start_visual, end_visual, self.viewport_left()),
        )
    }

    fn clear_line_background(&self, frame: &mut Frame, y: usize, is_cursor_line: bool) {
        frame.fill_rect_px(
            self.ctx.rect_x,
            y,
            self.ctx.gutter_width,
            self.ctx.line_height,
            self.palette.gutter_background,
        );

        let text_area_x = self.ctx.gutter_right_x + 1;
        let text_area_w = self.ctx.rect_w.saturating_sub(self.ctx.gutter_width + 1);
        let bg = if is_cursor_line {
            self.palette.current_line
        } else {
            self.palette.background
        };
        frame.fill_rect_px(text_area_x, y, text_area_w, self.ctx.line_height, bg);
    }

    fn render_current_line_highlight(&self, frame: &mut Frame) {
        if self.editor.active_cursor().line < self.viewport_top()
            || self.editor.active_cursor().line >= self.ctx.end_line
        {
            return;
        }

        let screen_line = self.editor.active_cursor().line - self.viewport_top();
        let highlight_y = self.ctx.content_y + screen_line * self.ctx.line_height;
        let highlight_h = self
            .ctx
            .line_height
            .min(self.ctx.content_y + self.ctx.content_h - highlight_y);
        frame.fill_rect_px(
            self.ctx.rect_x,
            highlight_y,
            self.ctx.rect_w,
            highlight_h,
            self.palette.current_line,
        );
    }

    fn render_selection_highlights_for_line(
        &self,
        frame: &mut Frame,
        doc_line: usize,
        y: usize,
        height: usize,
    ) {
        for selection in &self.editor.selections {
            let Some((x_start, x_end)) = self.selection_span_for_line(selection, doc_line) else {
                continue;
            };

            if x_end > x_start {
                frame.fill_rect_px(
                    x_start,
                    y,
                    x_end.saturating_sub(x_start),
                    height,
                    self.palette.selection,
                );
            }
        }

        if let Some((x_start, x_end)) = self.rectangle_selection_span_for_line(doc_line) {
            if x_end > x_start {
                frame.fill_rect_px(
                    x_start,
                    y,
                    x_end.saturating_sub(x_start),
                    height,
                    self.palette.selection,
                );
            }
        }
    }

    fn render_matching_brackets_for_line(
        &self,
        frame: &mut Frame,
        doc_line: usize,
        y: usize,
        height: usize,
    ) {
        if let Some((pos_a, pos_b)) = self.editor.matched_brackets {
            for &pos in &[pos_a, pos_b] {
                if pos.line != doc_line {
                    continue;
                }

                let line_text = self.document.get_line_cow(doc_line).unwrap_or_default();
                let visual_col = char_col_to_visual_col(&line_text, pos.column);
                if self
                    .ctx
                    .contains_visual_col(visual_col, self.viewport_left())
                {
                    let x = self.ctx.pixel_x(visual_col, self.viewport_left());
                    let w = self.ctx.char_width.round() as usize;
                    frame.blend_rect_px(x, y, w, height, self.palette.bracket_match);
                }
            }
        }
    }

    fn render_text_line(
        &mut self,
        frame: &mut Frame,
        painter: &mut TextPainter,
        doc_line: usize,
        y: usize,
    ) {
        let Some(line_text) = self.document.get_line_cow(doc_line) else {
            return;
        };

        let max_chars = self.ctx.visible_columns;
        let expanded_text = expand_tabs_for_display(&line_text);

        self.text_buffers.display_text.clear();
        for ch in expanded_text
            .chars()
            .skip(self.viewport_left())
            .take(max_chars)
        {
            self.text_buffers.display_text.push(ch);
        }

        let line_tokens = self.document.get_line_highlights(doc_line);
        self.text_buffers.adjusted_tokens.clear();
        for t in line_tokens.iter() {
            let visual_start = char_col_to_visual_col(&line_text, t.start_col);
            let visual_end = char_col_to_visual_col(&line_text, t.end_col);
            let start = visual_start.saturating_sub(self.viewport_left());
            let end = visual_end.saturating_sub(self.viewport_left());

            if end > 0 && start < max_chars {
                self.text_buffers
                    .adjusted_tokens
                    .push(crate::syntax::HighlightToken {
                        start_col: start,
                        end_col: end.min(max_chars),
                        highlight: t.highlight,
                    });
            }
        }

        if self.text_buffers.adjusted_tokens.is_empty() {
            painter.draw(
                frame,
                self.ctx.text_start_x,
                y,
                &self.text_buffers.display_text,
                self.palette.text,
            );
        } else {
            painter.draw_with_highlights(
                frame,
                self.ctx.text_start_x,
                y,
                &self.text_buffers.display_text,
                &self.text_buffers.adjusted_tokens,
                &self.model.theme.syntax,
                self.palette.text,
            );
        }
    }

    fn render_gutter_line_number(
        &self,
        frame: &mut Frame,
        painter: &mut TextPainter,
        doc_line: usize,
        y: usize,
        is_active_line: bool,
    ) {
        let line_num_str = format!("{}", doc_line + 1);
        let text_width_px = (line_num_str.len() as f32 * self.ctx.char_width).round() as usize;
        let text_x = self
            .ctx
            .gutter_right_x
            .saturating_sub(self.model.metrics.padding_medium + text_width_px);
        let line_color = if is_active_line {
            self.palette.active_line_number
        } else {
            self.palette.line_number
        };
        painter.draw(frame, text_x, y, &line_num_str, line_color);
    }

    fn render_cursor_at(
        &self,
        frame: &mut Frame,
        line: usize,
        column: usize,
        y: usize,
        color: u32,
    ) {
        let line_text = self.document.get_line_cow(line).unwrap_or_default();
        let visual_cursor_col = char_col_to_visual_col(&line_text, column);

        if !self
            .ctx
            .contains_visual_col(visual_cursor_col, self.viewport_left())
        {
            return;
        }

        let cursor_x = self.ctx.pixel_x(visual_cursor_col, self.viewport_left());
        frame.fill_rect_px(
            cursor_x,
            y + CURSOR_INSET,
            CURSOR_WIDTH,
            self.ctx.line_height.saturating_sub(CURSOR_INSET * 2),
            color,
        );
    }

    fn render_dirty_line_cursors(&self, frame: &mut Frame, doc_line: usize, y: usize) {
        if !self.model.ui.cursor_visible {
            return;
        }

        for (idx, cursor) in self.editor.cursors.iter().enumerate() {
            if cursor.line != doc_line {
                continue;
            }

            let cursor_color = if idx == 0 {
                self.palette.primary_cursor
            } else {
                self.palette.secondary_cursor
            };
            self.render_cursor_at(frame, cursor.line, cursor.column, y, cursor_color);
        }
    }

    fn render_visible_cursors(&self, frame: &mut Frame) {
        if !self.model.ui.cursor_visible {
            return;
        }

        for (idx, cursor) in self.editor.cursors.iter().enumerate() {
            let cursor_in_vertical_view = cursor.line >= self.viewport_top()
                && cursor.line < self.viewport_top() + self.ctx.visible_lines;
            if !cursor_in_vertical_view {
                continue;
            }

            let screen_line = cursor.line - self.viewport_top();
            let y = self.ctx.content_y + screen_line * self.ctx.line_height;
            let cursor_color = if idx == 0 {
                self.palette.primary_cursor
            } else {
                self.palette.secondary_cursor
            };
            self.render_cursor_at(frame, cursor.line, cursor.column, y, cursor_color);
        }
    }

    fn render_preview_cursors(&self, frame: &mut Frame) {
        if !self.editor.rectangle_selection.active {
            return;
        }

        for preview_pos in &self.editor.rectangle_selection.preview_cursors {
            let cursor_in_vertical_view = preview_pos.line >= self.viewport_top()
                && preview_pos.line < self.viewport_top() + self.ctx.visible_lines;
            if !cursor_in_vertical_view {
                continue;
            }

            let screen_line = preview_pos.line - self.viewport_top();
            let y = self.ctx.content_y + screen_line * self.ctx.line_height;
            self.render_cursor_at(
                frame,
                preview_pos.line,
                preview_pos.column,
                y,
                self.palette.secondary_cursor,
            );
        }
    }

    fn render_visible_line(
        &mut self,
        frame: &mut Frame,
        painter: &mut TextPainter,
        doc_line: usize,
        y: usize,
        height: usize,
    ) {
        self.render_selection_highlights_for_line(frame, doc_line, y, height);
        self.render_matching_brackets_for_line(frame, doc_line, y, height);
        self.render_text_line(frame, painter, doc_line, y);
    }

    fn render_cursor_lines_only(
        &mut self,
        frame: &mut Frame,
        painter: &mut TextPainter,
        dirty_lines: &[usize],
    ) {
        for &doc_line in dirty_lines {
            if doc_line < self.viewport_top() || doc_line >= self.ctx.end_line {
                continue;
            }

            let Some(y) = self.line_screen_y(doc_line) else {
                continue;
            };

            let is_cursor_line = doc_line == self.editor.active_cursor().line;
            self.clear_line_background(frame, y, is_cursor_line);
            self.render_selection_highlights_for_line(frame, doc_line, y, self.ctx.line_height);
            self.render_matching_brackets_for_line(frame, doc_line, y, self.ctx.line_height);
            self.render_gutter_line_number(frame, painter, doc_line, y, is_cursor_line);
            self.render_text_line(frame, painter, doc_line, y);
            self.render_dirty_line_cursors(frame, doc_line, y);
        }
    }

    fn render_text_area(&mut self, frame: &mut Frame, painter: &mut TextPainter, is_focused: bool) {
        self.render_current_line_highlight(frame);

        for (screen_line, doc_line) in (self.viewport_top()..self.ctx.end_line).enumerate() {
            let y = self.ctx.content_y + screen_line * self.ctx.line_height;
            if y >= self.ctx.content_y + self.ctx.content_h {
                break;
            }

            let line_height_px =
                (y + self.ctx.line_height).min(self.ctx.content_y + self.ctx.content_h) - y;
            self.render_visible_line(frame, painter, doc_line, y, line_height_px);
        }

        if is_focused {
            self.render_visible_cursors(frame);
            self.render_preview_cursors(frame);
        }
    }

    fn render_gutter(&self, frame: &mut Frame, painter: &mut TextPainter) {
        frame.fill_rect_px(
            self.ctx.rect_x,
            self.ctx.content_y,
            self.ctx.gutter_width,
            self.ctx.content_h,
            self.palette.gutter_background,
        );

        for (screen_line, doc_line) in (self.viewport_top()..self.ctx.end_line).enumerate() {
            let y = self.ctx.content_y + screen_line * self.ctx.line_height;
            if y >= self.ctx.content_y + self.ctx.content_h {
                break;
            }

            self.render_gutter_line_number(
                frame,
                painter,
                doc_line,
                y,
                doc_line == self.editor.active_cursor().line,
            );
        }

        frame.fill_rect_px(
            self.ctx.gutter_right_x,
            self.ctx.content_y,
            1,
            self.ctx.content_h,
            self.palette.gutter_border,
        );
    }
}

/// Render only specific cursor lines (optimized path for cursor blink).
pub fn render_cursor_lines_only(
    frame: &mut Frame,
    painter: &mut TextPainter,
    model: &AppModel,
    dirty_lines: &[usize],
) {
    let char_width = painter.char_width();
    let line_height = painter.line_height();

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

    let layout = geometry::GroupLayout::new(group, model, char_width);
    let mut renderer =
        TextEditorRenderer::new(model, editor, document, &layout, char_width, line_height);
    renderer.render_cursor_lines_only(frame, painter, dirty_lines);
}

/// Render text content (lines, selections, cursors) for an editor group.
pub fn render_text_area(
    frame: &mut Frame,
    painter: &mut TextPainter,
    model: &AppModel,
    editor: &EditorState,
    document: &Document,
    layout: &geometry::GroupLayout,
    is_focused: bool,
) {
    let char_width = painter.char_width();
    let line_height = painter.line_height();
    let mut renderer =
        TextEditorRenderer::new(model, editor, document, layout, char_width, line_height);
    renderer.render_text_area(frame, painter, is_focused);
}

/// Render the gutter (line numbers) for an editor group.
pub fn render_gutter(
    frame: &mut Frame,
    painter: &mut TextPainter,
    model: &AppModel,
    editor: &EditorState,
    document: &Document,
    layout: &geometry::GroupLayout,
) {
    let char_width = painter.char_width();
    let line_height = painter.line_height();
    let renderer =
        TextEditorRenderer::new(model, editor, document, layout, char_width, line_height);
    renderer.render_gutter(frame, painter);
}
