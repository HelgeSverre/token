//! Text editor content rendering (text area, gutter, cursors).

use crate::model::editor::Selection;
use crate::model::{AppModel, Document, EditorState, TextViewportMap};

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
struct EditorRenderContext {
    viewport: TextViewportMap,
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
}

impl EditorRenderContext {
    fn new(
        layout: &geometry::GroupLayout,
        editor: &EditorState,
        document: &Document,
        char_width: f32,
        line_height: usize,
    ) -> Self {
        let viewport = TextViewportMap::new(&editor.viewport, document.line_count());
        let visible_lines = layout.visible_lines(line_height);
        let visible_columns = layout.visible_columns(char_width);

        Self {
            viewport,
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
        }
    }

    #[inline]
    fn line_y(&self, doc_line: usize) -> Option<usize> {
        self.viewport
            .visible_row_for_doc_line(doc_line)
            .map(|visible_row| self.content_y + visible_row * self.line_height)
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
    selection_spans: Vec<(usize, usize)>,
    bracket_visual_cols: [Option<usize>; 2],
}

impl EditorTextBuffers {
    fn new(max_chars: usize) -> Self {
        Self {
            adjusted_tokens: Vec::with_capacity(32),
            display_text: String::with_capacity(max_chars + 16),
            selection_spans: Vec::with_capacity(8),
            bracket_visual_cols: [None, None],
        }
    }
}

/// Geometry and identity for one visible document line.
///
/// Future editor decorations should plug into the stages that consume this
/// type rather than add more feature-local line iteration.
struct VisibleTextLine {
    doc_line: usize,
    y: usize,
    height: usize,
    is_active_line: bool,
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
    ctx: EditorRenderContext,
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
        self.ctx.viewport.left_column()
    }

    #[inline]
    fn line_screen_y(&self, doc_line: usize) -> Option<usize> {
        self.ctx.line_y(doc_line)
    }

    fn selection_span_for_line(
        document: &Document,
        ctx: &EditorRenderContext,
        viewport_left: usize,
        selection: &Selection,
        doc_line: usize,
        line_text: &str,
    ) -> Option<(usize, usize)> {
        if selection.is_empty() {
            return None;
        }

        let sel_start = selection.start();
        let sel_end = selection.end();
        if doc_line < sel_start.line || doc_line > sel_end.line {
            return None;
        }

        let line_len = document.line_length(doc_line);
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

        let visual_start_col = char_col_to_visual_col(line_text, start_col);
        let visual_end_col = char_col_to_visual_col(line_text, end_col);
        Some(ctx.clipped_span_x(visual_start_col, visual_end_col, viewport_left))
    }

    fn rectangle_selection_span_for_line(
        ctx: &EditorRenderContext,
        viewport_left: usize,
        rect_sel: &crate::model::editor::RectangleSelectionState,
        doc_line: usize,
        line_text: &str,
    ) -> Option<(usize, usize)> {
        if !rect_sel.active || doc_line < rect_sel.top_line() || doc_line > rect_sel.bottom_line() {
            return None;
        }

        let left_visual_col = rect_sel.left_visual_col();
        let right_visual_col = rect_sel.right_visual_col();
        let current_visual_col = rect_sel.current_visual_col;

        let line_visual_len = char_col_to_visual_col(line_text, line_text.chars().count());

        if current_visual_col > line_visual_len {
            return None;
        }

        let start_visual = left_visual_col.min(line_visual_len);
        let end_visual = right_visual_col.min(line_visual_len);
        if start_visual >= end_visual {
            return None;
        }

        Some(ctx.clipped_span_x(start_visual, end_visual, viewport_left))
    }

    fn clear_line_background(
        &self,
        frame: &mut Frame,
        y: usize,
        height: usize,
        is_cursor_line: bool,
    ) {
        frame.fill_rect_px(
            self.ctx.rect_x,
            y,
            self.ctx.gutter_width,
            height,
            self.palette.gutter_background,
        );

        let text_area_x = self.ctx.gutter_right_x + 1;
        let text_area_w = self.ctx.rect_w.saturating_sub(self.ctx.gutter_width + 1);
        let bg = if is_cursor_line {
            self.palette.current_line
        } else {
            self.palette.background
        };
        frame.fill_rect_px(text_area_x, y, text_area_w, height, bg);
    }

    fn prepare_visible_line(&self, doc_line: usize, y: usize) -> VisibleTextLine {
        let bottom_y = self.ctx.content_y + self.ctx.content_h;
        let height = (y + self.ctx.line_height).min(bottom_y) - y;

        VisibleTextLine {
            doc_line,
            y,
            height,
            is_active_line: doc_line == self.editor.active_cursor().line,
        }
    }

    fn render_line_background_stage(&self, frame: &mut Frame, line: &VisibleTextLine) {
        self.clear_line_background(frame, line.y, line.height, line.is_active_line);
    }

    fn render_current_line_background_stage(&self, frame: &mut Frame) {
        let Some(screen_line) = self
            .ctx
            .viewport
            .visible_row_for_doc_line(self.editor.active_cursor().line)
        else {
            return;
        };
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

    fn collect_line_decorations(&mut self, line: &VisibleTextLine) {
        let document = self.document;
        let ctx = &self.ctx;
        let viewport_left = self.viewport_left();
        let rectangle_selection = &self.editor.rectangle_selection;

        let mut selection_spans = std::mem::take(&mut self.text_buffers.selection_spans);
        selection_spans.clear();
        let mut bracket_visual_cols = [None, None];

        let Some(line_text) = document.get_line_cow(line.doc_line) else {
            self.text_buffers.selection_spans = selection_spans;
            self.text_buffers.bracket_visual_cols = bracket_visual_cols;
            return;
        };

        for selection in &self.editor.selections {
            let Some((x_start, x_end)) = Self::selection_span_for_line(
                document,
                ctx,
                viewport_left,
                selection,
                line.doc_line,
                &line_text,
            ) else {
                continue;
            };

            if x_end > x_start {
                selection_spans.push((x_start, x_end));
            }
        }

        if let Some((x_start, x_end)) = Self::rectangle_selection_span_for_line(
            ctx,
            viewport_left,
            rectangle_selection,
            line.doc_line,
            &line_text,
        ) {
            if x_end > x_start {
                selection_spans.push((x_start, x_end));
            }
        }

        if let Some((pos_a, pos_b)) = self.editor.matched_brackets {
            for (slot, pos) in [pos_a, pos_b].into_iter().enumerate() {
                if pos.line != line.doc_line {
                    continue;
                }

                let visual_col = char_col_to_visual_col(&line_text, pos.column);
                if ctx.contains_visual_col(visual_col, viewport_left) {
                    bracket_visual_cols[slot] = Some(visual_col);
                }
            }
        }

        self.text_buffers.selection_spans = selection_spans;
        self.text_buffers.bracket_visual_cols = bracket_visual_cols;
    }

    fn render_line_decoration_stage(&self, frame: &mut Frame, line: &VisibleTextLine) {
        for &(x_start, x_end) in &self.text_buffers.selection_spans {
            frame.fill_rect_px(
                x_start,
                line.y,
                x_end.saturating_sub(x_start),
                line.height,
                self.palette.selection,
            );
        }

        let bracket_width = self.ctx.char_width.round() as usize;
        for visual_col in self.text_buffers.bracket_visual_cols.into_iter().flatten() {
            let x = self.ctx.pixel_x(visual_col, self.viewport_left());
            frame.blend_rect_px(
                x,
                line.y,
                bracket_width,
                line.height,
                self.palette.bracket_match,
            );
        }
    }

    fn render_line_text_stage(
        &mut self,
        frame: &mut Frame,
        painter: &mut TextPainter,
        line: &VisibleTextLine,
    ) {
        let document = self.document;
        let ctx = &self.ctx;
        let viewport_left = self.viewport_left();
        let model = self.model;
        let text_buffers = &mut self.text_buffers;

        let Some(line_text) = document.get_line_cow(line.doc_line) else {
            return;
        };

        let max_chars = ctx.visible_columns;
        let expanded_text = expand_tabs_for_display(&line_text);

        text_buffers.display_text.clear();
        for ch in expanded_text.chars().skip(viewport_left).take(max_chars) {
            text_buffers.display_text.push(ch);
        }

        let line_tokens = document.get_line_highlights(line.doc_line);
        text_buffers.adjusted_tokens.clear();
        for t in line_tokens.iter() {
            let visual_start = char_col_to_visual_col(&line_text, t.start_col);
            let visual_end = char_col_to_visual_col(&line_text, t.end_col);
            let start = visual_start.saturating_sub(viewport_left);
            let end = visual_end.saturating_sub(viewport_left);

            if end > 0 && start < max_chars {
                text_buffers
                    .adjusted_tokens
                    .push(crate::syntax::HighlightToken {
                        start_col: start,
                        end_col: end.min(max_chars),
                        highlight: t.highlight,
                    });
            }
        }

        if text_buffers.adjusted_tokens.is_empty() {
            painter.draw(
                frame,
                ctx.text_start_x,
                line.y,
                &text_buffers.display_text,
                self.palette.text,
            );
        } else {
            painter.draw_with_highlights(
                frame,
                ctx.text_start_x,
                line.y,
                &text_buffers.display_text,
                &text_buffers.adjusted_tokens,
                &model.theme.syntax,
                self.palette.text,
            );
        }
    }

    fn render_gutter_line_number(
        &self,
        frame: &mut Frame,
        painter: &mut TextPainter,
        line: &VisibleTextLine,
    ) {
        let line_num_str = format!("{}", line.doc_line + 1);
        let text_width_px = (line_num_str.len() as f32 * self.ctx.char_width).round() as usize;
        let text_x = self
            .ctx
            .gutter_right_x
            .saturating_sub(self.model.metrics.padding_medium + text_width_px);
        let line_color = if line.is_active_line {
            self.palette.active_line_number
        } else {
            self.palette.line_number
        };
        painter.draw(frame, text_x, line.y, &line_num_str, line_color);
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
            let Some(screen_line) = self.ctx.viewport.visible_row_for_doc_line(cursor.line) else {
                continue;
            };
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
            let Some(screen_line) = self.ctx.viewport.visible_row_for_doc_line(preview_pos.line)
            else {
                continue;
            };
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

    fn render_line_content_stages(
        &mut self,
        frame: &mut Frame,
        painter: &mut TextPainter,
        line: &VisibleTextLine,
    ) {
        self.collect_line_decorations(line);
        self.render_line_decoration_stage(frame, line);
        self.render_line_text_stage(frame, painter, line);
    }

    fn render_dirty_line_cursor_stage(&self, frame: &mut Frame, line: &VisibleTextLine) {
        self.render_dirty_line_cursors(frame, line.doc_line, line.y);
    }

    fn render_cursor_lines_only(
        &mut self,
        frame: &mut Frame,
        painter: &mut TextPainter,
        dirty_lines: &[usize],
    ) {
        for &doc_line in dirty_lines {
            if !self.ctx.viewport.contains_doc_line(doc_line) {
                continue;
            }

            let Some(y) = self.line_screen_y(doc_line) else {
                continue;
            };

            let line = self.prepare_visible_line(doc_line, y);
            self.render_line_background_stage(frame, &line);
            self.render_gutter_line_number(frame, painter, &line);
            self.render_line_content_stages(frame, painter, &line);
            self.render_dirty_line_cursor_stage(frame, &line);
        }
    }

    fn render_text_area(&mut self, frame: &mut Frame, painter: &mut TextPainter, is_focused: bool) {
        self.render_current_line_background_stage(frame);

        for screen_line in 0..self.ctx.visible_lines {
            let Some(doc_line) = self.ctx.viewport.doc_line_for_visible_row(screen_line) else {
                break;
            };
            let y = self.ctx.content_y + screen_line * self.ctx.line_height;
            if y >= self.ctx.content_y + self.ctx.content_h {
                break;
            }

            let line = self.prepare_visible_line(doc_line, y);
            self.render_line_content_stages(frame, painter, &line);
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

        for screen_line in 0..self.ctx.visible_lines {
            let Some(doc_line) = self.ctx.viewport.doc_line_for_visible_row(screen_line) else {
                break;
            };
            let y = self.ctx.content_y + screen_line * self.ctx.line_height;
            if y >= self.ctx.content_y + self.ctx.content_h {
                break;
            }

            let line = self.prepare_visible_line(doc_line, y);
            self.render_gutter_line_number(frame, painter, &line);
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

#[cfg(test)]
mod tests {
    use super::render_cursor_lines_only;
    use crate::model::{AppModel, Cursor, Position, Rect, Selection};
    use crate::view::geometry::GroupLayout;
    use crate::view::{Frame, GlyphCache, Renderer, TextPainter};
    use fontdue::{Font, FontSettings};
    use ropey::Rope;

    fn load_test_font() -> (Font, f32, f32, f32, usize) {
        let font = Font::from_bytes(
            include_bytes!("../../assets/JetBrainsMono.ttf") as &[u8],
            FontSettings::default(),
        )
        .expect("test font should load");
        let font_size = 14.0;
        let line_metrics = font
            .horizontal_line_metrics(font_size)
            .expect("font should expose horizontal metrics");
        let (metrics, _) = font.rasterize('M', font_size);

        (
            font,
            font_size,
            line_metrics.ascent,
            metrics.advance_width,
            line_metrics.new_line_size.ceil() as usize,
        )
    }

    fn make_text_model() -> AppModel {
        let mut model = AppModel::new(220, 140, 1.0, vec![]);
        let group_id = model.editor_area.focused_group_id;
        let tab_bar_height = model.metrics.tab_bar_height as f32;
        model.editor_area.groups.get_mut(&group_id).unwrap().rect = Rect::new(
            0.0,
            0.0,
            model.window_size.0 as f32,
            model.window_size.1 as f32 + tab_bar_height,
        );

        model.document_mut().buffer = Rope::from("alpha\n\tbeta()\nomega\n");
        let editor = model.editor_mut();
        editor.cursors = vec![Cursor::at(1, 6)];
        editor.selections = vec![Selection::from_positions(
            Position::new(1, 1),
            Position::new(1, 5),
        )];
        editor.matched_brackets = Some((Position::new(1, 5), Position::new(1, 6)));

        model
    }

    fn render_full_editor_group(model: &AppModel) -> Vec<u32> {
        let width = model.window_size.0 as usize;
        let height = model.window_size.1 as usize;
        let mut buffer = vec![0; width * height];
        let mut frame = Frame::new(&mut buffer, width, height);
        let (font, font_size, ascent, char_width, line_height) = load_test_font();
        let mut glyph_cache = GlyphCache::default();
        let mut painter = TextPainter::new(
            &font,
            &mut glyph_cache,
            font_size,
            ascent,
            char_width,
            line_height,
        );
        let group = model
            .editor_area
            .groups
            .get(&model.editor_area.focused_group_id)
            .unwrap();

        Renderer::render_editor_group(&mut frame, &mut painter, model, group.id, group.rect, true);
        buffer
    }

    fn rerender_cursor_lines(model: &AppModel, buffer: &mut [u32], dirty_lines: &[usize]) {
        let width = model.window_size.0 as usize;
        let height = model.window_size.1 as usize;
        let mut frame = Frame::new(buffer, width, height);
        let (font, font_size, ascent, char_width, line_height) = load_test_font();
        let mut glyph_cache = GlyphCache::default();
        let mut painter = TextPainter::new(
            &font,
            &mut glyph_cache,
            font_size,
            ascent,
            char_width,
            line_height,
        );

        render_cursor_lines_only(&mut frame, &mut painter, model, dirty_lines);
    }

    fn extract_active_line_band(model: &AppModel, buffer: &[u32]) -> Vec<u32> {
        let width = model.window_size.0 as usize;
        let (_, _, _, char_width, line_height) = load_test_font();
        let group = model
            .editor_area
            .groups
            .get(&model.editor_area.focused_group_id)
            .unwrap();
        let layout = GroupLayout::new(group, model, char_width);
        let y = layout.content_y() + model.editor().active_cursor().line * line_height;
        let max_y = layout.content_y() + layout.content_h();
        let height = (y + line_height).min(max_y).saturating_sub(y);
        let start_x = layout.rect_x();
        let band_width = layout
            .v_scrollbar_rect(model.metrics.scrollbar_width)
            .map(|rect| rect.x.round() as usize - start_x)
            .unwrap_or_else(|| layout.rect_w());
        let mut band = Vec::with_capacity(band_width * height);

        for row in y..y + height {
            let row_start = row * width + start_x;
            let row_end = row_start + band_width;
            band.extend_from_slice(&buffer[row_start..row_end]);
        }

        band
    }

    #[test]
    fn cursor_line_fast_path_matches_full_render_after_cursor_visibility_change() {
        let mut model = make_text_model();
        model.ui.cursor_visible = true;
        let before = render_full_editor_group(&model);
        let mut dirty_redraw = before.clone();

        model.ui.cursor_visible = false;
        rerender_cursor_lines(&model, &mut dirty_redraw, &[1]);

        let full_redraw = render_full_editor_group(&model);
        assert_eq!(
            extract_active_line_band(&model, &dirty_redraw),
            extract_active_line_band(&model, &full_redraw),
            "cursor-line fast path should match a full text render after cursor visibility changes"
        );
    }
}
