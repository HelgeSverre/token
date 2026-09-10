//! Unified text field rendering.
//!
//! Provides `TextFieldRenderer` for rendering text inputs with cursor, selection,
//! and proper text display. Used by modals, CSV cell editor, and potentially
//! other single-line input contexts.

use crate::editable::Position;
use crate::editable::{Cursor, EditableState, Selection, StringBuffer};

use super::frame::Frame;
use super::geometry::{ModalSpacing, WidgetRect};
use super::{FontRole, TextPainter};

/// Options for rendering a text field.
#[derive(Debug, Clone)]
pub struct TextFieldOptions {
    /// X position of text area in pixels
    pub x: usize,
    /// Y position of text area in pixels
    pub y: usize,
    /// Width of text area in pixels
    pub width: usize,
    /// Height of text area in pixels (typically line_height)
    pub height: usize,
    /// Character width (monospace font)
    pub char_width: f32,
    /// Text foreground color
    pub text_color: u32,
    /// Cursor color
    pub cursor_color: u32,
    /// Selection background color
    pub selection_color: u32,
    /// Whether cursor should be visible (for blinking)
    pub cursor_visible: bool,
    /// Horizontal scroll offset in characters
    pub scroll_x: usize,
    /// First visible logical line and number of drawn lines in a text area.
    pub scroll_y: usize,
    pub rows: usize,
}

impl Default for TextFieldOptions {
    fn default() -> Self {
        Self {
            x: 0,
            y: 0,
            width: 200,
            height: 20,
            char_width: 8.0,
            text_color: 0xFFFFFFFF,
            cursor_color: 0xFFFFFFFF,
            selection_color: 0x40FFFFFF,
            cursor_visible: true,
            scroll_x: 0,
            scroll_y: 0,
            rows: 1,
        }
    }
}

impl TextFieldOptions {
    /// A multiline field. Signed source geometry also handles a parent form
    /// scrolling its top above the window; paint, caret and pointer use this
    /// same projection. The caller clips to the field and parent viewport.
    pub fn for_text_area(
        content: &dyn TextFieldContent,
        rect: crate::model::editor_area::Rect,
        line_height: usize,
        char_width: f32,
    ) -> Self {
        let line_height = line_height.max(1);
        let rows = (rect.height.max(0.0) as usize / line_height).max(1);
        let cursor_line = content
            .cursors()
            .get(content.active_cursor_index())
            .map_or(0, |c| c.line);
        let mut opts = Self::for_text_box(
            content,
            &WidgetRect {
                x: rect.x.max(0.0) as usize,
                y: 0,
                w: rect.width.max(0.0) as usize,
                h: line_height,
            },
            line_height,
            char_width,
        );
        let skipped = (-rect.y / line_height as f32).ceil().max(0.0) as usize;
        opts.y = (rect.y + (skipped * line_height) as f32).max(0.0) as usize;
        opts.scroll_y = cursor_line.saturating_sub(rows - 1) + skipped;
        opts.rows = rows.saturating_sub(skipped);
        opts
    }

    /// Character-grid projection shared with field painting and caret geometry.
    pub fn position_at(&self, x: f64, y: f64) -> Position {
        Position::new(
            self.scroll_y
                + ((y - self.y as f64).max(0.0) / self.height.max(1) as f64).floor() as usize,
            self.scroll_x
                + ((x - self.x as f64).max(0.0) / self.char_width.max(1.0) as f64).round() as usize,
        )
    }

    /// Build the geometry for a modal field input, including the horizontal
    /// scroll needed to keep its active cursor visible. The field box is
    /// inset by `ModalSpacing::input_pad_x` on both sides.
    pub fn for_modal(
        content: &dyn TextFieldContent,
        rect: &WidgetRect,
        line_height: usize,
        char_width: f32,
        scale_factor: f64,
    ) -> Self {
        let padx = ModalSpacing::input_pad_x(scale_factor);
        let inner = WidgetRect {
            x: rect.x + padx,
            y: rect.y,
            w: rect.w.saturating_sub(padx * 2),
            h: rect.h,
        };
        Self::for_text_box(content, &inner, line_height, char_width)
    }

    /// Geometry for text drawn directly in `rect` (no inset) — the overlay
    /// header, whose text box `modal_header_input_rect` already resolves.
    pub fn for_text_box(
        content: &dyn TextFieldContent,
        rect: &WidgetRect,
        line_height: usize,
        char_width: f32,
    ) -> Self {
        let visible_chars = (rect.w as f32 / char_width).ceil() as usize + 1;
        let cursor_col = content
            .cursors()
            .get(content.active_cursor_index())
            .map(|cursor| cursor.column)
            .unwrap_or(0);

        Self {
            x: rect.x,
            y: rect.y + (rect.h.saturating_sub(line_height)) / 2,
            width: rect.w,
            height: line_height,
            char_width,
            scroll_x: TextFieldRenderer::calculate_scroll(cursor_col, 0, visible_chars),
            ..Self::default()
        }
    }
}

/// Trait for content that can be rendered as a text field.
///
/// This allows uniform rendering of different editable states.
pub trait TextFieldContent {
    /// Get the field's full text content.
    fn text(&self) -> &str;

    fn is_multiline(&self) -> bool {
        false
    }

    /// Get all cursors
    fn cursors(&self) -> &[Cursor];

    /// Get all selections
    fn selections(&self) -> &[Selection];

    /// Get the active cursor index
    fn active_cursor_index(&self) -> usize;
}

impl TextFieldContent for EditableState<StringBuffer> {
    fn is_multiline(&self) -> bool {
        self.constraints.allow_multiline
    }
    fn text(&self) -> &str {
        self.buffer.as_str()
    }

    fn cursors(&self) -> &[Cursor] {
        &self.cursors
    }

    fn selections(&self) -> &[Selection] {
        &self.selections
    }

    fn active_cursor_index(&self) -> usize {
        self.active_cursor
    }
}

/// Unified renderer for text fields.
///
/// Handles rendering of:
/// - Selection backgrounds
/// - Text content
/// - Cursor(s)
pub struct TextFieldRenderer;

impl TextFieldRenderer {
    /// Return the rendered active-cursor rectangle for a text field.
    pub fn caret_rect(
        content: &dyn TextFieldContent,
        opts: &TextFieldOptions,
    ) -> Option<WidgetRect> {
        let cursor = content.cursors().get(content.active_cursor_index())?;
        if !(opts.scroll_y..opts.scroll_y + opts.rows).contains(&cursor.line) {
            return None;
        }
        let visible_col = cursor.column.saturating_sub(opts.scroll_x);
        let unclamped_x = opts.x + (visible_col as f32 * opts.char_width).round() as usize;
        let width = 2.min(opts.width.max(1));
        let max_x = opts.x + opts.width.saturating_sub(width);

        Some(WidgetRect {
            x: unclamped_x.clamp(opts.x, max_x),
            y: opts.y + (cursor.line - opts.scroll_y) * opts.height + usize::from(opts.height > 1),
            w: width,
            h: opts.height.saturating_sub(2).max(1),
        })
    }

    /// Render a text field from an EditableState.
    pub fn render(
        frame: &mut Frame,
        painter: &mut TextPainter,
        content: &dyn TextFieldContent,
        opts: &TextFieldOptions,
    ) {
        // All editable fields share the editor font and grid geometry.
        let mut painter = painter.with_font(FontRole::Code);
        let text = content.text();

        for (slot, line) in text
            .splitn(
                if content.is_multiline() {
                    usize::MAX
                } else {
                    1
                },
                '\n',
            )
            .skip(opts.scroll_y)
            .take(opts.rows)
            .enumerate()
        {
            let line_index = opts.scroll_y + slot;
            let y = opts.y + slot * opts.height;
            // Selections are clipped independently on each logical line.
            for selection in content.selections() {
                if selection.is_empty() {
                    continue;
                }
                let start = selection.start();
                let end = selection.end();
                if line_index < start.line || line_index > end.line {
                    continue;
                }
                let start_col = if line_index == start.line {
                    start.column
                } else {
                    0
                };
                let end_col = if line_index == end.line {
                    end.column
                } else {
                    line.chars().count() + 1
                };

                // Adjust for horizontal scroll
                let visible_start = start_col.saturating_sub(opts.scroll_x);
                let visible_end = end_col.saturating_sub(opts.scroll_x);

                if visible_end > visible_start {
                    let sel_x = opts.x + (visible_start as f32 * opts.char_width).round() as usize;
                    let sel_width =
                        ((visible_end - visible_start) as f32 * opts.char_width).round() as usize;

                    // Clamp to visible width
                    let clamped_width = sel_width.min(opts.width.saturating_sub(sel_x - opts.x));

                    if clamped_width > 0 {
                        frame.fill_rect_px(
                            sel_x,
                            y,
                            clamped_width,
                            opts.height,
                            opts.selection_color,
                        );
                    }
                }
            }
            // Render visible text with the same character grid as pointer input.
            let max_chars = (opts.width as f32 / opts.char_width).ceil() as usize + 1;
            let visible_text: String = line
                .chars()
                .skip(opts.scroll_x)
                .take(max_chars)
                .map(|ch| if ch == '\t' { ' ' } else { ch })
                .collect();

            painter.draw(frame, opts.x, y, &visible_text, opts.text_color);
        }

        // 3. Render cursors
        if opts.cursor_visible {
            for (idx, cursor) in content.cursors().iter().enumerate() {
                if !(opts.scroll_y..opts.scroll_y + opts.rows).contains(&cursor.line) {
                    continue;
                }
                let col = cursor.column.saturating_sub(opts.scroll_x);
                let cursor_x = opts.x + (col as f32 * opts.char_width).round() as usize;

                // Check if cursor is visible in viewport
                if cursor_x >= opts.x && cursor_x < opts.x + opts.width {
                    let color = if idx == content.active_cursor_index() {
                        opts.cursor_color
                    } else {
                        // Slightly dimmer for secondary cursors
                        opts.cursor_color & 0x80FFFFFF
                    };

                    // 2px wide cursor bar
                    frame.fill_rect_px(
                        cursor_x,
                        opts.y + (cursor.line - opts.scroll_y) * opts.height + 1,
                        2,
                        opts.height.saturating_sub(2),
                        color,
                    );
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn render_modal_input(
        frame: &mut Frame,
        painter: &mut TextPainter,
        content: &dyn TextFieldContent,
        rect: &WidgetRect,
        line_height: usize,
        char_width: f32,
        background_color: u32,
        text_color: u32,
        cursor_color: u32,
        selection_color: u32,
        cursor_visible: bool,
        scale_factor: f64,
    ) {
        frame.fill_rect_px(rect.x, rect.y, rect.w, rect.h, background_color);

        let opts = TextFieldOptions {
            text_color,
            cursor_color,
            selection_color,
            cursor_visible,
            ..TextFieldOptions::for_modal(content, rect, line_height, char_width, scale_factor)
        };
        Self::render(frame, painter, content, &opts);
    }

    /// Calculate the scroll offset needed to keep the cursor visible.
    ///
    /// Returns the new scroll_x value.
    pub fn calculate_scroll(cursor_col: usize, scroll_x: usize, visible_chars: usize) -> usize {
        // Keep some margin around the cursor
        let margin = 2;

        if cursor_col < scroll_x + margin {
            // Cursor is too far left, scroll left
            cursor_col.saturating_sub(margin)
        } else if cursor_col >= scroll_x + visible_chars.saturating_sub(margin) {
            // Cursor is too far right, scroll right
            cursor_col.saturating_sub(visible_chars.saturating_sub(margin + 1))
        } else {
            // Cursor is visible, no change
            scroll_x
        }
    }
}

/// Test fixture for rendering text fields without a full editable model.
///
/// Holds text and cursor position without full EditableState.
#[cfg(test)]
pub struct SimpleTextField {
    text: String,
    cursor: Cursor,
    selection: Selection,
}

#[cfg(test)]
impl SimpleTextField {
    pub fn new(text: &str) -> Self {
        let cursor = Cursor::at(0, text.chars().count());
        Self {
            text: text.to_string(),
            cursor,
            selection: Selection::new(Position::new(0, cursor.column)),
        }
    }

    pub fn with_cursor(text: &str, cursor_col: usize) -> Self {
        let cursor = Cursor::at(0, cursor_col);
        Self {
            text: text.to_string(),
            cursor,
            selection: Selection::new(Position::new(0, cursor_col)),
        }
    }
}

#[cfg(test)]
impl TextFieldContent for SimpleTextField {
    fn text(&self) -> &str {
        &self.text
    }

    fn cursors(&self) -> &[Cursor] {
        std::slice::from_ref(&self.cursor)
    }

    fn selections(&self) -> &[Selection] {
        std::slice::from_ref(&self.selection)
    }

    fn active_cursor_index(&self) -> usize {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_scroll_cursor_visible() {
        // Cursor at column 5, scroll at 0, 20 chars visible
        assert_eq!(TextFieldRenderer::calculate_scroll(5, 0, 20), 0);
    }

    #[test]
    fn test_calculate_scroll_cursor_too_far_right() {
        // Cursor at column 25, scroll at 0, 20 chars visible
        let scroll = TextFieldRenderer::calculate_scroll(25, 0, 20);
        assert!(scroll > 0);
        // Cursor should be visible with margin
        assert!(25 >= scroll && 25 < scroll + 20);
    }

    #[test]
    fn test_calculate_scroll_cursor_too_far_left() {
        // Cursor at column 5, scroll at 10, 20 chars visible
        let scroll = TextFieldRenderer::calculate_scroll(5, 10, 20);
        assert!(scroll <= 5);
    }

    #[test]
    fn test_simple_text_field() {
        let field = SimpleTextField::new("hello");
        assert_eq!(field.text(), "hello");
        assert_eq!(field.cursors().len(), 1);
        assert_eq!(field.cursors()[0].column, 5); // cursor at end
    }

    #[test]
    fn test_simple_text_field_with_cursor() {
        let field = SimpleTextField::with_cursor("hello", 2);
        assert_eq!(field.cursors()[0].column, 2);
    }

    #[test]
    fn caret_rect_matches_scrolled_text_field_cursor() {
        let field = SimpleTextField::with_cursor("abcdefghijklmnopqrstuvwxyz", 20);
        let opts = TextFieldOptions {
            x: 100,
            y: 40,
            width: 80,
            height: 20,
            char_width: 8.0,
            scroll_x: 15,
            ..TextFieldOptions::default()
        };

        assert_eq!(
            TextFieldRenderer::caret_rect(&field, &opts),
            Some(WidgetRect {
                x: 140,
                y: 41,
                w: 2,
                h: 18,
            })
        );
    }

    #[test]
    fn render_modal_input_keeps_cursor_visible_for_a_long_query() {
        use crate::view::frame::Frame;
        use crate::view::geometry::WidgetRect;
        use crate::view::{GlyphCache, TextPainter};
        use fontdue::{Font, FontSettings};

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
        let char_width = metrics.advance_width;
        let line_height = line_metrics.new_line_size.ceil() as usize;

        // A narrow field (20 chars wide) with a query far longer than that,
        // cursor at the very end. Before this fix, `scroll_x` was hardcoded
        // to 0, so the cursor (at column 60) would be computed far outside
        // `[opts.x, opts.x + opts.width)` and never drawn at all.
        let long_query = "x".repeat(60);
        let field = SimpleTextField::with_cursor(&long_query, 60);

        let width = 200;
        let height = 40;
        let mut buffer = vec![0u32; width * height];
        let mut frame = Frame::new(&mut buffer, width, height);
        let mut glyph_cache = GlyphCache::default();
        let mut painter = TextPainter::new(
            &font,
            &mut glyph_cache,
            font_size,
            line_metrics.ascent,
            char_width,
            line_height,
        );

        let rect = WidgetRect {
            x: 0,
            y: 0,
            w: width,
            h: height,
        };

        TextFieldRenderer::render_modal_input(
            &mut frame,
            &mut painter,
            &field,
            &rect,
            line_height,
            char_width,
            0xFF000000,
            0xFFFFFFFF,
            0xFFFF0000,
            0xFF444444,
            true,
            1.0,
        );

        // The cursor is drawn as a solid 2px-wide bar in `cursor_color`
        // (0xFFFF0000). If scroll_x were still pinned at 0, no such pixel
        // would appear anywhere in the buffer.
        let cursor_drawn = buffer.contains(&0xFFFF0000);
        assert!(
            cursor_drawn,
            "cursor must be scrolled into view and actually drawn for a long query"
        );
    }
}
