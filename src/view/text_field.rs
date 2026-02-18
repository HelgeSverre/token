//! Unified text field rendering.
//!
//! Provides `TextFieldRenderer` for rendering text inputs with cursor, selection,
//! and proper text display. Used by modals, CSV cell editor, and potentially
//! other single-line input contexts.

use crate::editable::{Cursor, EditableState, Position, Selection, StringBuffer};

use super::frame::Frame;
use super::TextPainter;

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
        }
    }
}

/// Trait for content that can be rendered as a text field.
///
/// This allows uniform rendering of different editable states.
pub trait TextFieldContent {
    /// Get the text content for line 0 (single-line inputs)
    fn text(&self) -> &str;

    /// Get all cursors
    fn cursors(&self) -> &[Cursor];

    /// Get all selections
    fn selections(&self) -> &[Selection];

    /// Get the active cursor index
    fn active_cursor_index(&self) -> usize;
}

impl TextFieldContent for EditableState<StringBuffer> {
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
    /// Render a text field from an EditableState.
    pub fn render(
        frame: &mut Frame,
        painter: &mut TextPainter,
        content: &dyn TextFieldContent,
        opts: &TextFieldOptions,
    ) {
        let text = content.text();

        // 1. Render selection backgrounds
        for selection in content.selections() {
            if selection.is_empty() {
                continue;
            }

            // For single-line, we only care about column positions
            let start_col = selection.start().column;
            let end_col = selection.end().column;

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
                        opts.y,
                        clamped_width,
                        opts.height,
                        opts.selection_color,
                    );
                }
            }
        }

        // 2. Render text (with horizontal scroll)
        let max_chars = (opts.width as f32 / opts.char_width).ceil() as usize + 1;
        let visible_text: String = text.chars().skip(opts.scroll_x).take(max_chars).collect();

        painter.draw(frame, opts.x, opts.y, &visible_text, opts.text_color);

        // 3. Render cursors
        if opts.cursor_visible {
            for (idx, cursor) in content.cursors().iter().enumerate() {
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
                        opts.y + 1,
                        2,
                        opts.height.saturating_sub(2),
                        color,
                    );
                }
            }
        }
    }

    /// Render a simple text field from raw text and cursor position.
    ///
    /// This is a convenience method for cases where we don't have a full
    /// EditableState yet (during migration).
    #[allow(dead_code)]
    pub fn render_simple(
        frame: &mut Frame,
        painter: &mut TextPainter,
        text: &str,
        cursor_col: usize,
        opts: &TextFieldOptions,
    ) {
        // Render text
        let max_chars = (opts.width as f32 / opts.char_width).ceil() as usize + 1;
        let visible_text: String = text.chars().skip(opts.scroll_x).take(max_chars).collect();

        painter.draw(frame, opts.x, opts.y, &visible_text, opts.text_color);

        // Render cursor
        if opts.cursor_visible {
            let col = cursor_col.saturating_sub(opts.scroll_x);
            let cursor_x = opts.x + (col as f32 * opts.char_width).round() as usize;

            if cursor_x >= opts.x && cursor_x < opts.x + opts.width {
                frame.fill_rect_px(
                    cursor_x,
                    opts.y + 1,
                    2,
                    opts.height.saturating_sub(2),
                    opts.cursor_color,
                );
            }
        }
    }

    /// Calculate the scroll offset needed to keep the cursor visible.
    ///
    /// Returns the new scroll_x value.
    #[allow(dead_code)]
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

/// Simple wrapper for rendering text fields during migration.
///
/// Holds text and cursor position without full EditableState.
#[allow(dead_code)]
pub struct SimpleTextField {
    text: String,
    cursor: Cursor,
    selection: Selection,
}

#[allow(dead_code)]
impl SimpleTextField {
    pub fn new(text: &str) -> Self {
        let cursor = Cursor::new(0, text.chars().count());
        Self {
            text: text.to_string(),
            cursor,
            selection: Selection::collapsed(Position::new(0, cursor.column)),
        }
    }

    pub fn with_cursor(text: &str, cursor_col: usize) -> Self {
        let cursor = Cursor::new(0, cursor_col);
        Self {
            text: text.to_string(),
            cursor,
            selection: Selection::collapsed(Position::new(0, cursor_col)),
        }
    }
}

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
}
