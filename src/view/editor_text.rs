//! Text editor content rendering (text area, gutter, cursors).

#[cfg(debug_assertions)]
use std::time::{Duration, Instant};

use crate::model::editor::Selection;
use crate::model::{collect_line_marks, AppModel, Document, EditorState, Mark, TextViewportMap};
use crate::perf::{PerfStage, PerfStats};

use super::frame::{Frame, TextPainter};
use super::geometry::{self, expand_tabs_for_display};
use crate::util::text::{char_col_to_visual_col, TABULATOR_WIDTH};

/// Cursor width in pixels.
const CURSOR_WIDTH: usize = 2;
/// Cursor inset from top of line in pixels.
const CURSOR_INSET: usize = 1;

/// A text-area overdraw decoration spanning a (line, char-col) range.
///
/// Pure pixels on positions the text pass already computed — decorations
/// never move text. `start`/`end` are half-open, like `Selection`. Fed
/// find/replace match highlights (`BackgroundTint`) from
/// `view::mod::find_match_decorations` today (see
/// `docs/feature/find-enhancements.md`); LSP diagnostics is the next
/// planned producer for the other `DecorationKind` variants.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RangeDecoration {
    pub start: (usize, usize),
    pub end: (usize, usize),
    pub kind: DecorationKind,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DecorationKind {
    Underline(u32),
    /// `draw_wavy_underline` from overlay-surface.md.
    Wavy(u32),
    /// Find matches, documentHighlight, bracket match.
    BackgroundTint(u32),
    /// Diagnostic tag `Unnecessary`.
    Faded,
    /// Diagnostic tag `Deprecated`.
    Strikethrough(u32),
}

impl DecorationKind {
    /// Tints draw first (blended under), then line-style decorations, per
    /// editor-decorations.md's Range Decorations rules.
    fn is_tint(self) -> bool {
        matches!(
            self,
            DecorationKind::BackgroundTint(_) | DecorationKind::Faded
        )
    }
}

/// Shared theme colors for text editor rendering.
#[derive(Debug, Clone, Copy)]
struct EditorPalette {
    background: u32,
    current_line: u32,
    selection: u32,
    bracket_match: u32,
    text: u32,
    ghost_text: u32,
    indent_guide: u32,
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
            ghost_text: model.theme.editor.ghost_text.to_argb_u32(),
            indent_guide: model.theme.editor.indent_guide.to_argb_u32(),
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
    viewport: TextViewportMap<'a>,
    char_width: f32,
    line_height: usize,
    rect_x: usize,
    rect_w: usize,
    content_y: usize,
    content_h: usize,
    gutter: geometry::GutterLayout,
    gutter_right_x: usize,
    gutter_width: usize,
    text_start_x: usize,
    visible_lines: usize,
    visible_columns: usize,
}

impl<'a> EditorRenderContext<'a> {
    fn new(
        layout: &geometry::GroupLayout,
        editor: &'a EditorState,
        document: &Document,
        char_width: f32,
        line_height: usize,
    ) -> Self {
        let viewport = editor.viewport_map(document);
        let visible_lines = crate::model::scroll::PixelAxis {
            unit: line_height.max(1) as f64,
            extent: layout.content_h() as f64,
            ..editor.viewport.pixels.y
        }
        .drawn_count();
        let visible_columns = if editor.soft_wrap {
            editor.viewport.visible_columns.saturating_add(1)
        } else {
            crate::model::scroll::PixelAxis {
                unit: (char_width as f64).max(1.0),
                extent: layout.text_width() as f64,
                ..editor.viewport.pixels.x
            }
            .drawn_count()
        };

        Self {
            viewport,
            char_width,
            line_height,
            rect_x: layout.rect_x(),
            rect_w: layout.rect_w(),
            content_y: layout.content_y(),
            content_h: layout.content_h(),
            gutter: layout.gutter,
            gutter_right_x: layout.gutter_right_x,
            gutter_width: layout.gutter_width(),
            text_start_x: layout.text_start_x,
            visible_lines,
            visible_columns,
        }
    }

    #[inline]
    fn text_right_x(&self) -> usize {
        self.rect_x + self.rect_w
    }

    #[inline]
    fn pixel_x(&self, visual_col: usize, viewport_left: usize) -> usize {
        debug_assert_eq!(viewport_left, self.viewport.left_column());
        (self.text_start_x as f64
            + self
                .viewport
                .column_pixel_offset(visual_col, self.char_width))
        .round()
        .max(0.0) as usize
    }

    fn pixel_y(&self, row: usize) -> usize {
        (self.content_y as f64 + self.viewport.row_pixel_offset(row, self.line_height as f64))
            .round()
            .max(0.0) as usize
    }

    fn text_clip(&self) -> crate::model::Rect {
        crate::model::Rect::new(
            self.text_start_x as f32,
            self.content_y as f32,
            self.text_right_x().saturating_sub(self.text_start_x) as f32,
            self.content_h as f32,
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
    indentation_columns: usize,
}

impl EditorTextBuffers {
    fn new(max_chars: usize) -> Self {
        Self {
            adjusted_tokens: Vec::with_capacity(32),
            display_text: String::with_capacity(max_chars + 16),
            selection_spans: Vec::with_capacity(8),
            bracket_visual_cols: [None, None],
            indentation_columns: 0,
        }
    }
}

/// Geometry and identity for one visible document line.
///
/// Future editor decorations should plug into the stages that consume this
/// type rather than add more feature-local line iteration.
#[derive(Clone)]
struct VisibleTextLine<'a> {
    doc_line: usize,
    visual_line: usize,
    segment_start: usize,
    segment_end: usize,
    is_continuation: bool,
    y: usize,
    height: usize,
    is_active_line: bool,
    projected: Option<&'a crate::model::GhostRow>,
}

impl VisibleTextLine<'_> {
    /// Materialize only this visual row, never its whole logical line.
    fn text<'a>(&'a self, document: &'a Document) -> std::borrow::Cow<'a, str> {
        if let Some(row) = self.projected {
            return std::borrow::Cow::Borrowed(&row.text);
        }
        document
            .get_line_slice(self.doc_line)
            .map(|slice| slice.slice(self.segment_start..self.segment_end).into())
            .unwrap_or_default()
    }

    fn visual_column(&self, text: &str, column: usize) -> usize {
        if let Some(row) = self.projected {
            if let Some(source) = row.sources.iter().flatten().find(|s| {
                s.columns.start <= self.segment_start && s.columns.end >= self.segment_end
            }) {
                return source.visual_column(text, column);
            }
        }
        char_col_to_visual_col(text, column.saturating_sub(self.segment_start))
    }

    /// Source-only fragments share row text and geometry. Splitting here keeps
    /// selection, syntax, brackets and diagnostic overlays off inserted ghosts.
    fn source_fragments(&self) -> impl Iterator<Item = Self> + '_ {
        let ranges = if let Some(row) = self.projected {
            row.sources.clone().map(|s| s.map(|s| s.columns))
        } else {
            [Some(self.segment_start..self.segment_end), None]
        };
        ranges.into_iter().flatten().map(|range| Self {
            segment_start: range.start,
            segment_end: range.end,
            ..self.clone()
        })
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
    indent_width: usize,
}

/// Prefer the most common small indentation increase, not alignment columns
/// or the width of a tab. Sampling is bounded and independent of scroll position.
fn document_indent_width(document: &Document) -> usize {
    let mut increases = [0usize; 9];
    let mut previous = 0;
    for line in document.buffer.lines().take(200) {
        let leading = line
            .chars()
            .take(256)
            .take_while(|ch| matches!(ch, ' ' | '\t'))
            .count();
        if line
            .get_char(leading)
            .is_none_or(|ch| matches!(ch, '\r' | '\n' | ' ' | '\t'))
        {
            continue;
        }
        let width = crate::util::text::visual_width(line.chars().take(leading));
        let increase = width.saturating_sub(previous);
        if (2..=8).contains(&increase) {
            increases[increase] += 1;
        }
        previous = width;
    }
    (2..=8)
        .filter(|&width| increases[width] > 0)
        .max_by_key(|&width| (increases[width], std::cmp::Reverse(width)))
        .unwrap_or(TABULATOR_WIDTH)
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
            indent_width: if model.config.indent_guides && editor.is_plain_text_mode() {
                document_indent_width(document)
            } else {
                TABULATOR_WIDTH
            },
        }
    }

    #[inline]
    fn viewport_left(&self) -> usize {
        self.ctx.viewport.left_column()
    }

    fn selection_span_for_line(
        document: &Document,
        ctx: &EditorRenderContext,
        viewport_left: usize,
        selection: &Selection,
        line: &VisibleTextLine,
        line_text: &str,
    ) -> Option<(usize, usize)> {
        if selection.is_empty() {
            return None;
        }

        let sel_start = selection.start();
        let sel_end = selection.end();
        let doc_line = line.doc_line;
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

        let start_col = start_col.max(line.segment_start).min(line.segment_end);
        let end_col = end_col.max(line.segment_start).min(line.segment_end);
        if end_col <= start_col {
            return None;
        }
        let visual_start_col = line.visual_column(line_text, start_col);
        let visual_end_col = line.visual_column(line_text, end_col);
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

        let line_visual_len = char_col_to_visual_col(line_text, line_text.chars().count());

        // A line has nothing to draw only when it's fully to the left of the
        // whole rectangle. Don't bail based on the live drag column (just
        // one edge of the rectangle) — a shorter intermediate line still
        // needs a span clipped to `line_visual_len`, handled below.
        if line_visual_len <= left_visual_col {
            return None;
        }

        let start_visual = left_visual_col.min(line_visual_len);
        let end_visual = right_visual_col.min(line_visual_len);
        if start_visual >= end_visual {
            return None;
        }

        Some(ctx.clipped_span_x(start_visual, end_visual, viewport_left))
    }

    /// Pixel span of `decoration` on `doc_line`, clamped against the
    /// buffer's current line length. Char-cols go through
    /// `char_col_to_visual_col` for tab expansion, exactly as cursors do.
    fn decoration_span_for_line(
        document: &Document,
        ctx: &EditorRenderContext,
        viewport_left: usize,
        decoration: &RangeDecoration,
        line: &VisibleTextLine,
        line_text: &str,
    ) -> Option<(usize, usize)> {
        let doc_line = line.doc_line;
        let (start_line, start_col) = decoration.start;
        let (end_line, end_col) = decoration.end;
        if start_line > end_line || doc_line < start_line || doc_line > end_line {
            return None;
        }

        let line_len = document.line_length(doc_line);
        let col_start = if doc_line == start_line {
            start_col.min(line_len)
        } else {
            0
        };
        let col_end = if doc_line == end_line {
            end_col.min(line_len)
        } else {
            line_len
        };
        let col_start = col_start.max(line.segment_start).min(line.segment_end);
        let col_end = col_end.max(line.segment_start).min(line.segment_end);
        if col_end <= col_start {
            return None;
        }

        let visual_start = line.visual_column(line_text, col_start);
        let visual_end = line.visual_column(line_text, col_end);
        Some(ctx.clipped_span_x(visual_start, visual_end, viewport_left))
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

    fn prepare_visible_line(&self, screen_line: usize, y: usize) -> Option<VisibleTextLine<'a>> {
        let doc_line = self.ctx.viewport.doc_line_for_visible_row(screen_line)?;
        let segment = self
            .ctx
            .viewport
            .segment_for_visible_row(self.document, screen_line)?;
        let segment_end = segment.end_col().min(self.document.line_length(doc_line));
        // Keep the original line box. Viewport clipping, not shortening the box,
        // determines visible glyphs, underlines, cursors and decorations.
        let height = self.ctx.line_height;

        Some(VisibleTextLine {
            doc_line,
            visual_line: segment.visual_line,
            segment_start: segment.start_col,
            segment_end,
            is_continuation: segment.is_continuation,
            y,
            height,
            is_active_line: doc_line == self.editor.active_cursor().line,
            projected: self.ctx.viewport.ghost_row_for_visible_row(screen_line),
        })
    }

    fn render_line_background_stage(&self, frame: &mut Frame, line: &VisibleTextLine) {
        self.clear_line_background(frame, line.y, line.height, line.is_active_line);
    }

    fn render_current_line_background_stage(&self, frame: &mut Frame) {
        for row in 0..self.ctx.visible_lines {
            let y = self.ctx.pixel_y(row);
            let Some(line) = self.prepare_visible_line(row, y) else {
                break;
            };
            if line.is_active_line {
                frame.fill_rect_px(
                    self.ctx.rect_x,
                    line.y,
                    self.ctx.rect_w,
                    line.height,
                    self.palette.current_line,
                );
            }
        }
    }

    fn collect_line_decorations(&mut self, line: &VisibleTextLine) {
        let document = self.document;
        let ctx = &self.ctx;
        let viewport_left = self.viewport_left();
        let rectangle_selection = &self.editor.rectangle_selection;

        let mut selection_spans = std::mem::take(&mut self.text_buffers.selection_spans);
        selection_spans.clear();
        let mut bracket_visual_cols = [None, None];

        let line_text = line.text(document);

        // Only real indentation, never wrapped continuation text. Use the same
        // tab expansion as glyphs and selections, including projected row text.
        self.text_buffers.indentation_columns = if self.model.config.indent_guides
            && self.editor.is_plain_text_mode()
            && !line.is_continuation
        {
            let leading = line_text
                .chars()
                // Every whitespace character occupies at least one column;
                // indentation beyond the viewport cannot add a visible guide.
                .take(viewport_left.saturating_add(ctx.visible_columns))
                .take_while(|ch| matches!(ch, ' ' | '\t'))
                .count();
            char_col_to_visual_col(&line_text, leading)
        } else {
            0
        };

        for (selection, fragment) in self.editor.selections.iter().flat_map(|selection| {
            line.source_fragments()
                .map(move |fragment| (selection, fragment))
        }) {
            let Some((x_start, x_end)) = Self::selection_span_for_line(
                document,
                ctx,
                viewport_left,
                selection,
                &fragment,
                &line_text,
            ) else {
                continue;
            };

            if x_end > x_start {
                selection_spans.push((x_start, x_end));
            }
        }

        let segment_text = &line_text;
        if let Some((x_start, x_end)) = Self::rectangle_selection_span_for_line(
            ctx,
            viewport_left,
            rectangle_selection,
            line.visual_line,
            segment_text,
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

                for fragment in line.source_fragments() {
                    if pos.column < fragment.segment_start || pos.column >= fragment.segment_end {
                        continue;
                    }
                    let visual_col = fragment.visual_column(&line_text, pos.column);
                    if ctx.contains_visual_col(visual_col, viewport_left) {
                        bracket_visual_cols[slot] = Some(visual_col);
                    }
                }
            }
        }

        self.text_buffers.selection_spans = selection_spans;
        self.text_buffers.bracket_visual_cols = bracket_visual_cols;
    }

    fn render_line_decoration_stage(&self, frame: &mut Frame, line: &VisibleTextLine) {
        // Draw underneath selections and glyphs. Bound traversal by the visible
        // columns even for heavily indented, horizontally scrolled documents.
        let left = self.viewport_left();
        let first = left.div_ceil(self.indent_width) * self.indent_width;
        let end = self
            .text_buffers
            .indentation_columns
            .min(left.saturating_add(self.ctx.visible_columns));
        for column in (first..end).step_by(self.indent_width) {
            let x = self.ctx.pixel_x(column, left);
            if x < self.ctx.text_right_x() {
                frame.blend_rect_px(x, line.y, 1, line.height, self.palette.indent_guide);
            }
        }

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

        let line_text = line.text(document);

        let max_chars = ctx.visible_columns;
        let segment_text = &line_text;
        let expanded_text = expand_tabs_for_display(segment_text);
        let ghost_columns = line.projected.map(|row| {
            char_col_to_visual_col(&row.text, row.ghost.start)
                ..char_col_to_visual_col(&row.text, row.ghost.end)
        });

        text_buffers.display_text.clear();
        for (column, ch) in expanded_text
            .chars()
            .enumerate()
            .skip(viewport_left)
            .take(max_chars)
        {
            // Ghost glyphs are drawn once in their own color below. Overdrawing
            // bright source glyphs would leave bright antialiased fringes.
            text_buffers.display_text.push(
                if ghost_columns
                    .as_ref()
                    .is_some_and(|range| range.contains(&column))
                {
                    ' '
                } else {
                    ch
                },
            );
        }

        let line_tokens = document.get_line_highlights(line.doc_line);
        text_buffers.adjusted_tokens.clear();
        for (t, fragment) in line_tokens
            .iter()
            .flat_map(|t| line.source_fragments().map(move |f| (t, f)))
        {
            let token_start = t
                .start_col
                .max(fragment.segment_start)
                .min(fragment.segment_end);
            let token_end = t
                .end_col
                .max(fragment.segment_start)
                .min(fragment.segment_end);
            if token_end <= token_start {
                continue;
            }
            let visual_start = fragment.visual_column(&line_text, token_start);
            let visual_end = fragment.visual_column(&line_text, token_end);
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
                ctx.pixel_x(viewport_left, viewport_left),
                line.y,
                &text_buffers.display_text,
                self.palette.text,
            );
        } else {
            painter.draw_with_highlights(
                frame,
                ctx.pixel_x(viewport_left, viewport_left),
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
        let line_num_str = if line.is_continuation {
            "↪".to_owned()
        } else {
            format!("{}", line.doc_line + 1)
        };
        let text_width_px =
            (line_num_str.chars().count() as f32 * self.ctx.char_width).round() as usize;
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

    /// Marks-lane glyph for a visible gutter line, if the marks lane is
    /// active and the line has a mark. LSP diagnostics (lsp-integration.md
    /// Phase 2) is the first producer — `GutterLayout` activates `marks_w`
    /// per-document once it has diagnostics (see `GroupLayout::new`).
    fn render_gutter_mark(&self, frame: &mut Frame, line: &VisibleTextLine) {
        if line.is_continuation {
            return;
        }
        let marks_w = self.ctx.gutter.marks_w as usize;
        if marks_w == 0 {
            return;
        }
        let Some(mark) = collect_line_marks(self.document, line.doc_line).mark else {
            return;
        };

        let inset = (marks_w / 4).max(1);
        let size = marks_w.saturating_sub(inset * 2).max(1);
        let x = self.ctx.rect_x + inset;
        let y = line.y + line.height.saturating_sub(size) / 2;
        frame.fill_rect_px(x, y, size, size, self.mark_color(mark));
    }

    fn mark_color(&self, mark: Mark) -> u32 {
        let overlay = &self.model.theme.overlay;
        match mark {
            Mark::Match => self
                .model
                .theme
                .editor
                .bracket_match_background
                .to_argb_u32(),
            Mark::Bookmark => overlay.severity_hint.to_argb_u32(),
            Mark::Info => overlay.severity_info.to_argb_u32(),
            Mark::Warning => overlay.severity_warning.to_argb_u32(),
            Mark::Error => overlay.severity_error.to_argb_u32(),
            Mark::Breakpoint => self.palette.primary_cursor,
        }
    }

    /// Overdraw pass for range decorations — after text, before cursors, per
    /// editor-decorations.md's pass order. Iterates only decorations
    /// intersecting the viewport; stale ranges clamp against the current
    /// buffer or are skipped, never panicking.
    fn render_range_decorations_stage(&self, frame: &mut Frame, decorations: &[RangeDecoration]) {
        if decorations.is_empty() {
            return;
        }

        // Geometry is shared across all decorations in this pass. Logical lines
        // are monotonic but may repeat for wrapped rows. Text is materialized at
        // most once per intersecting row, never for rows no decoration touches.
        let rows: Vec<_> = (0..self.ctx.visible_lines)
            .filter_map(|row| {
                let y = self.ctx.pixel_y(row);
                self.prepare_visible_line(row, y)
                    .map(|line| (line, std::cell::OnceCell::new()))
            })
            .collect();
        let viewport_left = self.viewport_left();
        for decoration in decorations
            .iter()
            .filter(|d| d.kind.is_tint())
            .chain(decorations.iter().filter(|d| !d.kind.is_tint()))
        {
            let (start_line, _) = decoration.start;
            let (end_line, _) = decoration.end;
            if start_line > end_line {
                continue;
            }
            let first = rows.partition_point(|(line, _)| line.doc_line < start_line);
            let end = rows.partition_point(|(line, _)| line.doc_line <= end_line);
            for (line, text) in &rows[first..end] {
                let line_text = text.get_or_init(|| line.text(self.document));
                for fragment in line.source_fragments() {
                    let Some((x_start, x_end)) = Self::decoration_span_for_line(
                        self.document,
                        &self.ctx,
                        viewport_left,
                        decoration,
                        &fragment,
                        line_text,
                    ) else {
                        continue;
                    };

                    self.paint_decoration(frame, x_start, x_end, line.y, decoration.kind);
                }
            }
        }
    }

    fn paint_decoration(
        &self,
        frame: &mut Frame,
        x_start: usize,
        x_end: usize,
        y: usize,
        kind: DecorationKind,
    ) {
        let width = x_end.saturating_sub(x_start);
        if width == 0 {
            return;
        }
        let height = self.ctx.line_height;

        match kind {
            DecorationKind::BackgroundTint(color) => {
                frame.blend_rect_px(x_start, y, width, height, color);
            }
            DecorationKind::Faded => {
                let faded = (self.palette.background & 0x00FF_FFFF) | 0x66_00_00_00;
                frame.blend_rect_px(x_start, y, width, height, faded);
            }
            DecorationKind::Underline(color) => {
                let line_y = y + height.saturating_sub(1);
                frame.fill_rect_px(x_start, line_y, width, 1, color | 0xFF00_0000);
            }
            DecorationKind::Wavy(color) => {
                let line_y = y + height.saturating_sub(2);
                frame.draw_wavy_underline(x_start, line_y, width, color | 0xFF00_0000);
            }
            DecorationKind::Strikethrough(color) => {
                let line_y = y + height / 2;
                frame.fill_rect_px(x_start, line_y, width, 1, color | 0xFF00_0000);
            }
        }
    }

    fn render_cursor_at(
        &self,
        frame: &mut Frame,
        line: &VisibleTextLine,
        column: usize,
        color: u32,
    ) {
        if self
            .ctx
            .viewport
            .visual_line_for_position(line.doc_line, column)
            != line.visual_line
        {
            return;
        }
        let (_, visual_cursor_col) =
            self.ctx
                .viewport
                .display_position(self.document, line.doc_line, column);

        if !self
            .ctx
            .contains_visual_col(visual_cursor_col, self.viewport_left())
        {
            return;
        }

        let cursor_x = self.ctx.pixel_x(visual_cursor_col, self.viewport_left());
        frame.fill_rect_px(
            cursor_x,
            line.y + CURSOR_INSET,
            CURSOR_WIDTH,
            self.ctx.line_height.saturating_sub(CURSOR_INSET * 2),
            color,
        );
    }

    fn render_dirty_line_cursors(&self, frame: &mut Frame, line: &VisibleTextLine) {
        if !self.model.ui.cursor_visible
            || self.model.ui.focus == crate::model::FocusTarget::FindBar
        {
            return;
        }

        for (idx, cursor) in self.editor.cursors.iter().enumerate() {
            if cursor.line != line.doc_line {
                continue;
            }

            let cursor_color = if idx == 0 {
                self.palette.primary_cursor
            } else {
                self.palette.secondary_cursor
            };
            self.render_cursor_at(frame, line, cursor.column, cursor_color);
        }
    }

    fn render_visible_cursors(&self, frame: &mut Frame) {
        if !self.model.ui.cursor_visible
            || self.model.ui.focus == crate::model::FocusTarget::FindBar
        {
            return;
        }

        for (idx, cursor) in self.editor.cursors.iter().enumerate() {
            let Some(screen_line) = self
                .ctx
                .viewport
                .visible_row_for_position(cursor.line, cursor.column)
            else {
                continue;
            };
            let y = self.ctx.pixel_y(screen_line);
            let Some(line) = self.prepare_visible_line(screen_line, y) else {
                continue;
            };
            let cursor_color = if idx == 0 {
                self.palette.primary_cursor
            } else {
                self.palette.secondary_cursor
            };
            self.render_cursor_at(frame, &line, cursor.column, cursor_color);
        }
    }

    fn render_preview_cursors(&self, frame: &mut Frame) {
        if !self.editor.rectangle_selection.active {
            return;
        }

        for preview_pos in &self.editor.rectangle_selection.preview_cursors {
            let Some(screen_line) = self
                .ctx
                .viewport
                .visible_row_for_position(preview_pos.line, preview_pos.column)
            else {
                continue;
            };
            let y = self.ctx.pixel_y(screen_line);
            let Some(line) = self.prepare_visible_line(screen_line, y) else {
                continue;
            };
            self.render_cursor_at(
                frame,
                &line,
                preview_pos.column,
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
        self.render_ghost_text_stage(frame, painter, line);
    }

    /// Ghost glyphs occupy the shared projected rows. The suffix is already
    /// shifted by that projection, including tabs and wrapped continuations.
    fn render_ghost_text_stage(
        &mut self,
        frame: &mut Frame,
        painter: &mut TextPainter,
        line: &VisibleTextLine,
    ) {
        let Some(row) = line.projected else {
            return;
        };
        let viewport_left = self.viewport_left();
        let start = char_col_to_visual_col(&row.text, row.ghost.start).max(viewport_left);
        let end = char_col_to_visual_col(&row.text, row.ghost.end)
            .min(viewport_left.saturating_add(self.ctx.visible_columns));
        if start < end {
            let expanded = expand_tabs_for_display(&row.text);
            let text: String = expanded.chars().skip(start).take(end - start).collect();
            painter.draw(
                frame,
                self.ctx.pixel_x(start, viewport_left),
                line.y,
                &text,
                self.palette.ghost_text,
            );
        }
        // Choice count is UI metadata, not inserted source. Put it after the
        // complete anchor row, never on top of either the ghost or its suffix.
        let cursor = self.editor.active_cursor();
        if self
            .ctx
            .viewport
            .visual_line_for_position(cursor.line, cursor.column)
            == line.visual_line
        {
            if let Some(state) = crate::update::inline::visible(self.model) {
                let (position, count) = state.choice_position();
                let column = char_col_to_visual_col(&row.text, row.text.chars().count());
                if count > 1 && self.ctx.contains_visual_col(column, viewport_left) {
                    let room = self
                        .ctx
                        .visible_columns
                        .saturating_sub(column - viewport_left);
                    let label: String = format!(" [{position}/{count}]")
                        .chars()
                        .take(room)
                        .collect();
                    painter.draw(
                        frame,
                        self.ctx.pixel_x(column, viewport_left),
                        line.y,
                        &label,
                        self.palette.ghost_text,
                    );
                }
            }
        }
    }

    fn render_dirty_line_cursor_stage(&self, frame: &mut Frame, line: &VisibleTextLine) {
        self.render_dirty_line_cursors(frame, line);
    }

    fn render_cursor_lines_only(
        &mut self,
        frame: &mut Frame,
        painter: &mut TextPainter,
        dirty_lines: &[usize],
        decorations: &[RangeDecoration],
    ) {
        for screen_line in 0..self.ctx.visible_lines {
            let y = self.ctx.pixel_y(screen_line);
            let Some(line) = self.prepare_visible_line(screen_line, y) else {
                break;
            };
            if !dirty_lines.contains(&line.doc_line) {
                continue;
            }

            self.render_line_background_stage(frame, &line);
            self.render_gutter_line_number(frame, painter, &line);
            // Regression guard: this fast path repaints lines the full pass
            // decorated — skipping the marks lane and range decorations
            // erased squiggles and gutter dots whenever the cursor or a
            // selection touched a diagnostic line.
            self.render_gutter_mark(frame, &line);
            frame.push_clip(self.ctx.text_clip());
            self.render_line_content_stages(frame, painter, &line);
            self.render_dirty_line_cursor_stage(frame, &line);
            frame.pop_clip();
        }
        frame.push_clip(self.ctx.text_clip());
        self.render_range_decorations_stage(frame, decorations);
        frame.pop_clip();
    }

    fn render_text_area(
        &mut self,
        frame: &mut Frame,
        painter: &mut TextPainter,
        is_focused: bool,
        decorations: &[RangeDecoration],
        perf: &mut PerfStats,
    ) {
        #[cfg(not(debug_assertions))]
        let _ = perf;

        #[cfg(debug_assertions)]
        let mut background_time = Duration::ZERO;
        #[cfg(debug_assertions)]
        let mut decoration_time = Duration::ZERO;
        #[cfg(debug_assertions)]
        let mut glyph_time = Duration::ZERO;
        #[cfg(debug_assertions)]
        let mut cursor_time = Duration::ZERO;

        #[cfg(debug_assertions)]
        {
            let start = Instant::now();
            self.render_current_line_background_stage(frame);
            background_time += start.elapsed();
        }
        #[cfg(not(debug_assertions))]
        self.render_current_line_background_stage(frame);

        frame.push_clip(self.ctx.text_clip());
        for screen_line in 0..self.ctx.visible_lines {
            let y = self.ctx.pixel_y(screen_line);
            if y >= self.ctx.content_y + self.ctx.content_h {
                break;
            }

            let Some(line) = self.prepare_visible_line(screen_line, y) else {
                break;
            };
            #[cfg(debug_assertions)]
            {
                let start = Instant::now();
                self.collect_line_decorations(&line);
                self.render_line_decoration_stage(frame, &line);
                decoration_time += start.elapsed();

                let start = Instant::now();
                self.render_line_text_stage(frame, painter, &line);
                self.render_ghost_text_stage(frame, painter, &line);
                glyph_time += start.elapsed();
            }
            #[cfg(not(debug_assertions))]
            self.render_line_content_stages(frame, painter, &line);
        }

        #[cfg(debug_assertions)]
        {
            let start = Instant::now();
            self.render_range_decorations_stage(frame, decorations);
            decoration_time += start.elapsed();
        }
        #[cfg(not(debug_assertions))]
        self.render_range_decorations_stage(frame, decorations);

        if is_focused {
            #[cfg(debug_assertions)]
            {
                let start = Instant::now();
                self.render_visible_cursors(frame);
                self.render_preview_cursors(frame);
                cursor_time += start.elapsed();
            }
            #[cfg(not(debug_assertions))]
            {
                self.render_visible_cursors(frame);
                self.render_preview_cursors(frame);
            }
        }

        #[cfg(debug_assertions)]
        {
            perf.record_stage_elapsed(PerfStage::TextBackground, background_time);
            perf.record_stage_elapsed(PerfStage::TextDecorations, decoration_time);
            perf.record_stage_elapsed(PerfStage::TextGlyphs, glyph_time);
            perf.record_stage_elapsed(PerfStage::TextCursors, cursor_time);
        }
        frame.pop_clip();
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
            let y = self.ctx.pixel_y(screen_line);
            if y >= self.ctx.content_y + self.ctx.content_h {
                break;
            }

            let Some(line) = self.prepare_visible_line(screen_line, y) else {
                break;
            };
            self.render_gutter_mark(frame, &line);
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

    // Defensive guard: this fast path assumes text-editor invariants
    // (cursor-line highlighting, gutter, plain per-line layout) that only
    // hold for plain-text tabs. `build_render_plan` only ever populates this
    // path for text tabs, but guard here too so a future caller wiring this
    // up for a non-text tab fails safely instead of misrendering.
    debug_assert!(
        editor.is_plain_text_mode(),
        "render_cursor_lines_only called for a non-plain-text editor"
    );
    if !editor.is_plain_text_mode() {
        return;
    }

    let layout = geometry::GroupLayout::new(group, model, char_width);
    // The same producers the full pass uses, scoped to the dirty lines so a
    // repainted line keeps its squiggles/tints (the focused pane also gets
    // find-match tints, mirroring render_editor_content's is_focused rule —
    // this path only ever runs for the focused editor).
    let lo = dirty_lines.iter().copied().min().unwrap_or(0);
    let hi = dirty_lines.iter().copied().max().unwrap_or(0) + 1;
    let mut decorations =
        super::find_match_decorations(model, document, &editor.selections[0], lo..hi);
    decorations.extend(super::diagnostic_decorations(model, document, lo..hi));
    let mut renderer =
        TextEditorRenderer::new(model, editor, document, &layout, char_width, line_height);
    // The full pass repaints the scrollbar band after text; this fast path
    // doesn't, so keep every stage out of that band or a line repaint
    // overwrites the track/ticks with line background.
    let clip_w = layout
        .v_scrollbar_rect(model.metrics.scrollbar_width)
        .map(|sb| (sb.x.max(0.0) as usize).saturating_sub(layout.rect_x()))
        .unwrap_or_else(|| layout.rect_w());
    frame.set_clip(crate::model::editor_area::Rect {
        x: layout.rect_x() as f32,
        y: layout.content_y() as f32,
        width: clip_w as f32,
        height: layout.content_h() as f32,
    });
    // The caller may be painting UI. Cursor redraws use the code font just
    // like full editor groups, without changing the caller's role.
    let mut painter = painter.with_font(super::FontRole::Code);
    renderer.render_cursor_lines_only(frame, &mut painter, dirty_lines, &decorations);
    frame.clear_clip();
}

/// Render text content (lines, selections, cursors) for an editor group.
///
/// `decorations` are overdrawn after text, before cursors (see
/// `RangeDecoration`); pass `&[]` where no producer is wired yet.
#[allow(clippy::too_many_arguments)]
pub fn render_text_area(
    frame: &mut Frame,
    painter: &mut TextPainter,
    model: &AppModel,
    editor: &EditorState,
    document: &Document,
    layout: &geometry::GroupLayout,
    is_focused: bool,
    decorations: &[RangeDecoration],
    perf: &mut PerfStats,
) {
    let char_width = painter.char_width();
    let line_height = painter.line_height();
    let mut renderer =
        TextEditorRenderer::new(model, editor, document, layout, char_width, line_height);
    frame.push_clip(layout.content_rect);
    renderer.render_text_area(frame, painter, is_focused, decorations, perf);
    frame.pop_clip();
}

/// Render the gutter (line numbers) for an editor group.
pub fn render_gutter(
    frame: &mut Frame,
    painter: &mut TextPainter,
    model: &AppModel,
    editor: &EditorState,
    document: &Document,
    layout: &geometry::GroupLayout,
    perf: &mut PerfStats,
) {
    let char_width = painter.char_width();
    let line_height = painter.line_height();
    let renderer =
        TextEditorRenderer::new(model, editor, document, layout, char_width, line_height);
    frame.push_clip(crate::model::Rect::new(
        layout.rect_x() as f32,
        layout.content_y() as f32,
        (layout.gutter_width() + 1) as f32,
        layout.content_h() as f32,
    ));
    perf.measure_stage(PerfStage::Gutter, || renderer.render_gutter(frame, painter));
    frame.pop_clip();
}

#[cfg(test)]
mod tests {
    use super::render_cursor_lines_only;
    use super::{
        render_text_area, DecorationKind, EditorRenderContext, RangeDecoration, TextEditorRenderer,
        VisibleTextLine,
    };
    use crate::model::editor::RectangleSelectionState;
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
        let mut model = AppModel::new(220, 140, 1.0);
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

    #[test]
    fn indent_guides_detect_two_and_four_space_steps_without_using_alignment_or_blank_lines() {
        for (text, expected) in [
            ("(define f\n  (fn (x)\n    (if x\n      value)))\n", 2),
            (
                "fn f() {\n    call(\n         aligned);\n    if x {\n        value\n    }\n}\n",
                4,
            ),
            ("fn f() {\n\tif x {\n\t\tvalue\n\t}\n}\n", 4),
            ("plain\n\ntext\n", 4),
        ] {
            let document = crate::model::Document::with_text(text);
            assert_eq!(
                super::document_indent_width(&document),
                expected,
                "{text:?}"
            );
        }
    }

    #[test]
    fn indent_guides_follow_tab_columns_and_clip_without_marking_wrapped_text() {
        for text in ["\t  \tvalue", "        value", "  \t    value", "        "] {
            for left in [0, 2, 4, 7, 8] {
                for enabled in [false, true] {
                    let mut model = make_text_model();
                    model.document_mut().buffer = Rope::from_str(text);
                    model.editor_mut().selections = vec![Selection::default()];
                    model.editor_mut().matched_brackets = None;
                    model.editor_mut().viewport.left_column = left;
                    model.config.indent_guides = enabled;
                    model.theme.editor.indent_guide = crate::theme::Color::rgb(0x12, 0x34, 0x56);
                    let layout =
                        GroupLayout::new(model.editor_area.focused_group().unwrap(), &model, 8.0);
                    let mut renderer = TextEditorRenderer::new(
                        &model,
                        model.editor(),
                        model.document(),
                        &layout,
                        8.0,
                        20,
                    );
                    // This test isolates visual tab expansion from detection.
                    renderer.indent_width = 4;
                    let mut line = renderer.prepare_visible_line(0, 0).unwrap();
                    for continuation in [false, true] {
                        line.is_continuation = continuation;
                        renderer.collect_line_decorations(&line);
                        let mut pixels = vec![0; 220 * 140];
                        let mut frame = Frame::new(&mut pixels, 220, 140);
                        renderer.render_line_decoration_stage(&mut frame, &line);
                        let expected: Vec<_> = [0, 4]
                            .into_iter()
                            .filter(|&column| enabled && !continuation && column >= left)
                            .map(|column| layout.text_start_x + (column - left) * 8)
                            .collect();
                        let painted: Vec<_> = pixels[..220]
                            .iter()
                            .enumerate()
                            .filter_map(|(x, &pixel)| (pixel == 0xFF123456).then_some(x))
                            .collect();
                        assert_eq!(
                            painted, expected,
                            "{text:?}, left={left}, continuation={continuation}"
                        );
                    }
                }
            }
        }
    }

    /// Frozen pre-optimization traversal. Keep the span/paint primitives shared:
    /// this oracle checks traversal, ordering and clipping, not their geometry.
    fn render_decorations_reference(
        renderer: &TextEditorRenderer<'_>,
        frame: &mut Frame,
        decorations: &[RangeDecoration],
    ) {
        for decoration in decorations
            .iter()
            .filter(|d| d.kind.is_tint())
            .chain(decorations.iter().filter(|d| !d.kind.is_tint()))
        {
            if decoration.start.0 > decoration.end.0 {
                continue;
            }
            for row in 0..renderer.ctx.visible_lines {
                let y = renderer.ctx.content_y + row * renderer.ctx.line_height;
                let Some(line) = renderer.prepare_visible_line(row, y) else {
                    continue;
                };
                if line.doc_line < decoration.start.0 || line.doc_line > decoration.end.0 {
                    continue;
                }
                let text = line.text(renderer.document);
                if let Some((start, end)) = TextEditorRenderer::decoration_span_for_line(
                    renderer.document,
                    &renderer.ctx,
                    renderer.viewport_left(),
                    decoration,
                    &line,
                    &text,
                ) {
                    renderer.paint_decoration(frame, start, end, line.y, decoration.kind);
                }
            }
        }
    }

    #[test]
    fn decoration_traversal_pixels_match_reference_with_wrap_scroll_and_overlaps() {
        let texts = [
            "alpha\r\n\t🙂beta()\r\nomega\n\nend".to_owned(),
            format!("{}\nlast", "a\t🙂bc ".repeat(800)),
            "".to_owned(),
            "plain\n\ttext🙂\n".repeat(30),
        ];
        let endpoints = [
            (0, 0),
            (0, 2),
            (0, usize::MAX),
            (1, 0),
            (1, 5),
            (2, 4),
            (4, 30),
            (60, 999),
        ];
        let kinds = [
            DecorationKind::Underline(0xFF00FF00),
            DecorationKind::BackgroundTint(0x661234FF),
            DecorationKind::Wavy(0xFFFF0000),
            DecorationKind::Faded,
            DecorationKind::Strikethrough(0xFFFFFFFF),
            DecorationKind::BackgroundTint(0x88FF1234),
        ];
        let decorations: Vec<_> = endpoints
            .iter()
            .flat_map(|&start| endpoints.iter().map(move |&end| (start, end)))
            .enumerate()
            .map(|(index, (start, end))| RangeDecoration {
                start,
                end,
                kind: kinds[index % kinds.len()],
            })
            .collect();
        let mut painted = false;
        for text in &texts {
            for wrap in [false, true] {
                for top in [0, 1, 3] {
                    for left in [0, 5] {
                        let mut model = make_text_model();
                        model.document_mut().buffer = Rope::from_str(text);
                        model.char_width = 8.0;
                        model.line_height = 20;
                        model.editor_mut().soft_wrap = wrap;
                        model.resync_viewports();
                        model.editor_area.refresh_wrap_caches();
                        let editor_id = model.editor_area.focused_editor_id().unwrap();
                        model.set_editor_vertical_scroll(editor_id, top);
                        model.editor_mut().viewport.left_column = left;
                        let group = &model.editor_area.groups[&model.editor_area.focused_group_id];
                        let layout = GroupLayout::new(group, &model, 8.0);
                        let renderer = TextEditorRenderer::new(
                            &model,
                            model.editor(),
                            model.document(),
                            &layout,
                            8.0,
                            20,
                        );
                        let width = model.window_size.0 as usize;
                        let height = model.window_size.1 as usize;
                        let background = 0xFF314159;
                        let mut expected = vec![background; width * height];
                        let mut actual = expected.clone();
                        let clip = Rect::new(
                            renderer.ctx.text_start_x as f32,
                            renderer.ctx.content_y as f32,
                            (renderer.ctx.rect_x + renderer.ctx.rect_w)
                                .saturating_sub(renderer.ctx.text_start_x)
                                as f32,
                            renderer.ctx.content_h as f32,
                        );
                        let mut frame = Frame::new(&mut expected, width, height);
                        frame.set_clip(clip);
                        render_decorations_reference(&renderer, &mut frame, &decorations);
                        let mut frame = Frame::new(&mut actual, width, height);
                        frame.set_clip(clip);
                        renderer.render_range_decorations_stage(&mut frame, &decorations);
                        assert!(
                            actual == expected,
                            "wrap={wrap}, top={top}, left={left}, chars={}",
                            text.chars().count()
                        );
                        painted |= actual.iter().any(|&pixel| pixel != background);
                    }
                }
            }
        }
        assert!(painted, "comparison must exercise painted pixels");
    }

    #[test]
    fn rectangle_selection_clips_to_a_shorter_intermediate_line_instead_of_skipping_it() {
        // Regression test for a block-selection bug: dragging a rectangle
        // selection down-and-right used to bail out entirely (`None`) for
        // any intermediate line shorter than the *live drag column*, even
        // though the line clearly overlaps the rectangle's left edge and
        // should get a selection span clipped to its own length.
        let model = make_text_model();
        let char_width = 8.0;
        let group = model
            .editor_area
            .groups
            .get(&model.editor_area.focused_group_id)
            .unwrap();
        let layout = GroupLayout::new(group, &model, char_width);
        let ctx =
            EditorRenderContext::new(&layout, model.editor(), model.document(), char_width, 16);

        // Rectangle drag: started at (line 0, visual col 0), dragged to
        // (line 2, visual col 10) — i.e. a wide rectangle spanning columns
        // [0, 10) across lines 0..=2.
        let rect_sel = RectangleSelectionState {
            active: true,
            start_line: 0,
            start_visual_col: 0,
            current_line: 2,
            current_visual_col: 10,
            preview_cursors: Vec::new(),
        };

        // "alpha" is only 5 chars long — shorter than the drag's right edge
        // (visual col 10) — so it used to be skipped (`None`) entirely.
        let span =
            TextEditorRenderer::rectangle_selection_span_for_line(&ctx, 0, &rect_sel, 0, "alpha");
        assert!(
            span.is_some(),
            "a short line overlapping the rectangle's left edge must still get a clipped span, not None"
        );

        // A line fully to the left of the rectangle (empty, before the
        // rectangle even starts) must still correctly return None.
        let no_overlap = TextEditorRenderer::rectangle_selection_span_for_line(
            &ctx,
            0,
            &RectangleSelectionState {
                active: true,
                start_line: 0,
                start_visual_col: 20,
                current_line: 2,
                current_visual_col: 30,
                preview_cursors: Vec::new(),
            },
            0,
            "alpha",
        );
        assert!(
            no_overlap.is_none(),
            "a line fully left of the rectangle should still have no selection span"
        );
    }

    fn make_render_context(model: &AppModel, char_width: f32) -> EditorRenderContext<'_> {
        let group = model
            .editor_area
            .groups
            .get(&model.editor_area.focused_group_id)
            .unwrap();
        let layout = GroupLayout::new(group, model, char_width);
        EditorRenderContext::new(&layout, model.editor(), model.document(), char_width, 16)
    }

    fn unwrapped_line(doc_line: usize) -> VisibleTextLine<'static> {
        VisibleTextLine {
            doc_line,
            visual_line: doc_line,
            segment_start: 0,
            segment_end: usize::MAX,
            is_continuation: false,
            y: 0,
            height: 16,
            is_active_line: false,
            projected: None,
        }
    }

    #[test]
    fn decoration_span_clamps_columns_past_end_of_line() {
        let model = make_text_model();
        let char_width = 8.0;
        let ctx = make_render_context(&model, char_width);
        let document = model.document();

        // "alpha" (line 0) is 5 chars; a decoration claiming column 999
        // must clamp to the line's real length, not panic or overshoot.
        let decoration = RangeDecoration {
            start: (0, 0),
            end: (0, 999),
            kind: DecorationKind::Underline(0xFFFFFFFF),
        };
        let line_text = document.get_line_cow(0).unwrap();
        let span = TextEditorRenderer::decoration_span_for_line(
            document,
            &ctx,
            0,
            &decoration,
            &unwrapped_line(0),
            &line_text,
        );
        assert!(span.is_some());
        let (x_start, x_end) = span.unwrap();
        assert!(x_start < x_end);
        assert!(x_end <= ctx.text_right_x());
    }

    #[test]
    fn decoration_span_fuzz_stale_ranges_against_shrunk_document_never_panics() {
        let model = make_text_model();
        let char_width = 8.0;
        let ctx = make_render_context(&model, char_width);
        let document = model.document();
        let real_line_count = document.line_count();

        // Sweep a grid of (start_line, start_col) x (end_line, end_col)
        // combinations well past the document's real bounds — simulating
        // decorations computed before edits shrank the buffer.
        for start_line in 0..real_line_count + 4 {
            for start_col in [0usize, 3, 50, 999] {
                for end_line in start_line..start_line + 4 {
                    for end_col in [0usize, 3, 50, 999] {
                        let decoration = RangeDecoration {
                            start: (start_line, start_col),
                            end: (end_line, end_col),
                            kind: DecorationKind::BackgroundTint(0x80FF0000),
                        };
                        for doc_line in 0..real_line_count {
                            let Some(line_text) = document.get_line_cow(doc_line) else {
                                continue;
                            };
                            if let Some((x_start, x_end)) =
                                TextEditorRenderer::decoration_span_for_line(
                                    document,
                                    &ctx,
                                    0,
                                    &decoration,
                                    &unwrapped_line(doc_line),
                                    &line_text,
                                )
                            {
                                assert!(x_start <= x_end, "span must not be inverted");
                                assert!(
                                    x_end <= ctx.text_right_x(),
                                    "span must not overshoot the text area"
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn render_text_area_survives_decorations_referencing_vanished_lines() {
        let model = make_text_model();
        let width = model.window_size.0 as usize;
        let height = model.window_size.1 as usize;
        let mut buffer = vec![0u32; width * height];
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
        let layout = GroupLayout::new(group, &model, char_width);
        let mut perf = crate::perf::PerfStats::default();

        // A mix of an in-range tint and decorations referencing lines the
        // 3-line test document doesn't have — must render without panicking
        // and must skip the vanished-line decorations entirely.
        let decorations = [
            RangeDecoration {
                start: (0, 0),
                end: (0, 3),
                kind: DecorationKind::BackgroundTint(0x80FF0000),
            },
            RangeDecoration {
                start: (999, 0),
                end: (999, 5),
                kind: DecorationKind::Wavy(0xFFFF0000),
            },
            RangeDecoration {
                start: (1, 0),
                end: (500, 5),
                kind: DecorationKind::Underline(0xFF00FF00),
            },
        ];

        render_text_area(
            &mut frame,
            &mut painter,
            &model,
            model.editor(),
            model.document(),
            &layout,
            true,
            &decorations,
            &mut perf,
        );
    }

    fn render_full_editor_group(model: &AppModel) -> Vec<u32> {
        let width = model.window_size.0 as usize;
        let height = model.window_size.1 as usize;
        // The top-level renderer clears the surface before rendering groups.
        let mut buffer = vec![model.theme.editor.background.to_argb_u32(); width * height];
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
        let mut perf = crate::perf::PerfStats::default();

        Renderer::render_editor_group(
            &mut frame,
            &mut painter,
            model,
            group.id,
            group.rect,
            true,
            &mut perf,
        );
        buffer
    }

    fn rerender_cursor_lines(model: &AppModel, buffer: &mut [u32], dirty_lines: &[usize]) {
        let width = model.window_size.0 as usize;
        let height = model.window_size.1 as usize;
        let mut frame = Frame::new(buffer, width, height);
        let (font, font_size, ascent, char_width, line_height) = load_test_font();
        let mut glyph_cache = GlyphCache::default();
        let ui_font = Font::from_bytes(
            include_bytes!("../../assets/Inter-Regular.ttf") as &[u8],
            FontSettings::default(),
        )
        .unwrap();
        let mut ui_cache = GlyphCache::default();
        let mut painter = TextPainter::new(
            &font,
            &mut glyph_cache,
            font_size,
            ascent,
            char_width,
            line_height,
        )
        .with_ui_font(&ui_font, &mut ui_cache, crate::view::FontRole::Ui);

        render_cursor_lines_only(&mut frame, &mut painter, model, dirty_lines);
    }

    #[test]
    fn pixel_scrolled_text_gutter_hits_and_cursor_redraw_share_geometry() {
        for (wrapped, find_open) in [(false, false), (true, false), (false, true), (true, true)] {
            let mut model = make_text_model();
            model.document_mut().buffer =
                Rope::from_str(&"    alpha\tbeta gamma delta epsilon\n".repeat(30));
            model.document_mut().diagnostics = vec![lsp_types::Diagnostic {
                range: lsp_types::Range::new(
                    lsp_types::Position::new(0, 4),
                    lsp_types::Position::new(0, 9),
                ),
                severity: Some(lsp_types::DiagnosticSeverity::WARNING),
                message: "partial-row decoration".into(),
                ..Default::default()
            }];
            let (_, _, _, char_width, line_height) = load_test_font();
            model.char_width = char_width;
            model.line_height = line_height;
            model.config.show_scrollbar = false;
            model.ui.cursor_visible = true;
            model.editor_mut().soft_wrap = wrapped;
            model.editor_mut().cursors = vec![Cursor::at(1, 6)];
            model.editor_mut().clear_selection();
            if find_open {
                let mut find = crate::model::FindReplaceState::default();
                find.set_query("alpha");
                model.ui.open_find(find);
                model.resize(220, 480);
            } else {
                model.resize(220, 140);
            }
            let normal = render_full_editor_group(&model);
            let id = model.editor_area.focused_editor_id().unwrap();
            let doc_id = model.editor_area.focused_document_id().unwrap();
            model
                .editor_area
                .editors
                .get_mut(&id)
                .unwrap()
                .set_pixel_scroll(&model.editor_area.documents[&doc_id], 3.0, 7.0);
            let horizontal = if wrapped { 0 } else { 3 };
            assert_eq!(model.editor().viewport.pixels.x.offset, horizontal as f64);
            assert_eq!(model.editor().viewport.pixels.y.offset, 7.0);
            let scrolled = render_full_editor_group(&model);
            let group = model.editor_area.focused_group().unwrap();
            let layout = crate::view::geometry::GroupLayout::new(group, &model, char_width);
            let width = model.window_size.0 as usize;
            let y0 = layout.content_y();
            let bottom = (y0 + layout.content_h()).min(model.window_size.1 as usize);
            // Content and gutter move by the same vertical displacement; the gutter
            // does not follow horizontal scrolling. Exclude clipped edges.
            for y in y0..bottom.saturating_sub(7) {
                for x in layout.rect_x()..layout.gutter_right_x.saturating_sub(1) {
                    assert_eq!(scrolled[y * width + x], normal[(y + 7) * width + x]);
                }
                for x in layout.text_start_x..width.saturating_sub(horizontal) {
                    assert_eq!(
                        scrolled[y * width + x],
                        normal[(y + 7) * width + x + horizontal]
                    );
                }
            }
            assert_eq!(
                &scrolled[..y0 * width],
                &normal[..y0 * width],
                "tab bar stays fixed"
            );
            let map = model.editor().viewport_map(model.document());
            assert_eq!(
                map.doc_line_for_pixel_y(line_height as f64 - 8.0, line_height as f64),
                0
            );
            assert_eq!(
                map.doc_line_for_pixel_y(line_height as f64 - 7.0, line_height as f64),
                usize::from(!wrapped)
            );
            let mut incremental = scrolled.clone();
            rerender_cursor_lines(&model, &mut incremental, &[0, 1, 2, 3, 4, 5, 6]);
            let mismatch = incremental.iter().zip(&scrolled).position(|(a, b)| a != b);
            assert!(
                mismatch.is_none(),
                "cursor-only mismatch {:?}",
                mismatch.map(|i| (i % width, i / width, incremental[i], scrolled[i]))
            );
        }
    }

    #[test]
    fn ghost_projection_blink_pixels_match_full_render_and_decorations_skip_ghosts() {
        for wrapped in [false, true] {
            let mut model = make_text_model();
            model.document_mut().buffer =
                Rope::from_str(&format!("ab\tcd\nnext\n{}", "last\n".repeat(20)));
            model.document_mut().diagnostics = vec![lsp_types::Diagnostic {
                range: lsp_types::Range::new(
                    lsp_types::Position::new(0, 0),
                    lsp_types::Position::new(0, 5),
                ),
                severity: Some(lsp_types::DiagnosticSeverity::WARNING),
                message: "source only".into(),
                ..Default::default()
            }];
            model.editor_mut().cursors = vec![Cursor::at(0, 1)];
            model.editor_mut().selections = vec![Selection::new(Position::new(0, 1))];
            model.editor_mut().soft_wrap = wrapped;
            let (_, _, _, char_width, line_height) = load_test_font();
            model.char_width = char_width;
            model.line_height = line_height;
            model.resize(220, 140);
            let width = wrapped.then_some(model.editor().viewport.visible_columns);
            let projection = crate::model::GhostProjection::new(
                model.document(),
                Position::new(0, 1),
                "XY\n\tZ",
                width,
            )
            .unwrap();
            model.editor_mut().ghost_text.0 = Some(std::sync::Arc::new(projection));
            for offset in [0.0, 7.0] {
                let (doc, editor) = model.editor_area.focused_document_and_editor_mut().unwrap();
                editor.set_pixel_scroll(doc, 0.0, offset);
                assert_eq!(editor.viewport.pixels.y.offset, offset);
                let before = render_full_editor_group(&model);
                let mut repainted = before.clone();
                rerender_cursor_lines(&model, &mut repainted, &[0]);
                assert_eq!(
                    repainted, before,
                    "blink must not duplicate or erase ghost rows, wrapped={wrapped}, offset={offset}"
                );
            }

            let ctx = make_render_context(&model, 8.0);
            let group = model.editor_area.focused_group().unwrap();
            let layout = GroupLayout::new(group, &model, 8.0);
            let renderer =
                TextEditorRenderer::new(&model, model.editor(), model.document(), &layout, 8.0, 16);
            let decoration = RangeDecoration {
                start: (0, 0),
                end: (0, 5),
                kind: DecorationKind::Underline(0xFFFFFFFF),
            };
            let line = renderer.prepare_visible_line(0, ctx.content_y).unwrap();
            let text = line.text(model.document());
            let spans: Vec<_> = line
                .source_fragments()
                .filter_map(|fragment| {
                    TextEditorRenderer::decoration_span_for_line(
                        model.document(),
                        &ctx,
                        0,
                        &decoration,
                        &fragment,
                        &text,
                    )
                })
                .collect();
            assert_eq!(spans, vec![(ctx.text_start_x, ctx.text_start_x + 8)]);
            let line = renderer
                .prepare_visible_line(1, ctx.content_y + 16)
                .unwrap();
            let text = line.text(model.document());
            let spans: Vec<_> = line
                .source_fragments()
                .filter_map(|fragment| {
                    TextEditorRenderer::decoration_span_for_line(
                        model.document(),
                        &ctx,
                        0,
                        &decoration,
                        &fragment,
                        &text,
                    )
                })
                .collect();
            assert_eq!(
                spans[0].0,
                ctx.text_start_x + 5 * 8,
                "suffix decoration starts after tab-expanded ghost"
            );
        }
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
    fn cursor_line_fast_path_keeps_diagnostics_on_the_repainted_line() {
        // Regression: the fast path skipped the marks lane and the range-
        // decorations stage, so moving the cursor onto (or selecting) a
        // diagnostic line erased its squiggle and gutter dot until the
        // next full redraw.
        let mut model = make_text_model();
        model.ui.cursor_visible = true;
        model.document_mut().diagnostics = vec![lsp_types::Diagnostic {
            range: lsp_types::Range {
                start: lsp_types::Position {
                    line: 1,
                    character: 0,
                },
                end: lsp_types::Position {
                    line: 1,
                    character: 3,
                },
            },
            severity: Some(lsp_types::DiagnosticSeverity::WARNING),
            message: "boom".into(),
            ..Default::default()
        }];

        let before = render_full_editor_group(&model);
        let mut dirty_redraw = before.clone();

        model.ui.cursor_visible = false;
        rerender_cursor_lines(&model, &mut dirty_redraw, &[1]);

        model.ui.cursor_visible = true;
        rerender_cursor_lines(&model, &mut dirty_redraw, &[1]);

        if dirty_redraw != before {
            let width = model.window_size.0 as usize;
            let diffs: Vec<(usize, usize)> = dirty_redraw
                .iter()
                .zip(before.iter())
                .enumerate()
                .filter(|(_, (a, b))| a != b)
                .map(|(i, _)| (i % width, i / width))
                .collect();
            panic!(
                "fast-path repaint of a diagnostic line must be pixel-identical \
                 to the full render; {} px differ, first at {:?}",
                diffs.len(),
                diffs.first()
            );
        }
    }

    #[test]
    fn soft_wrap_cursor_blink_repaints_all_segments_identically() {
        let mut model = make_text_model();
        let (_, _, _, char_width, line_height) = load_test_font();
        model.char_width = char_width;
        model.line_height = line_height;
        model.document_mut().buffer =
            Rope::from("one two three four five six seven eight nine ten eleven twelve\nnext");
        model.editor_mut().cursors = vec![Cursor::at(0, 24)];
        model.editor_mut().selections = vec![Selection::from_positions(
            Position::new(0, 3),
            Position::new(0, 24),
        )];
        model.editor_mut().matched_brackets = Some((Position::new(0, 4), Position::new(0, 23)));
        model.editor_mut().soft_wrap = true;
        model.resync_viewports();
        model.editor_mut().viewport.top_line = 1;
        model.ui.cursor_visible = true;
        let mut incremental = render_full_editor_group(&model);
        model.ui.cursor_visible = false;
        rerender_cursor_lines(&model, &mut incremental, &[0]);
        let full = render_full_editor_group(&model);
        let differences: Vec<_> = incremental
            .iter()
            .zip(&full)
            .enumerate()
            .filter(|(_, (a, b))| a != b)
            .map(|(i, _)| {
                (
                    i % model.window_size.0 as usize,
                    i / model.window_size.0 as usize,
                )
            })
            .collect();
        assert!(
            differences.is_empty(),
            "{} pixels differ, first {:?}",
            differences.len(),
            differences.first()
        );
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
