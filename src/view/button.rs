//! Button rendering primitives
//!
//! Provides a simple, pure-function API for rendering themed buttons.
//! No widget tree or stored state — callers determine visual state
//! from model and pass it to the render function.

use crate::model::editor_area::Rect;
use crate::theme::Theme;

use super::frame::{Frame, TextPainter};

/// Visual state of a button, determined by the caller from UI interaction state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ButtonState {
    /// Default idle state
    #[default]
    Normal,
    /// Mouse is hovering over the button
    Hovered,
    /// Mouse button is pressed on the button
    Pressed,
}

/// Render a button with centered text label
///
/// The button rect defines the full clickable/visual area.
/// Visual state (hover, press) is determined by the caller.
pub fn render_button(
    frame: &mut Frame,
    painter: &mut TextPainter,
    theme: &Theme,
    rect: Rect,
    label: &str,
    state: ButtonState,
    focused: bool,
) {
    let btn = &theme.button;

    let (bg, border) = match state {
        ButtonState::Normal => (btn.background.to_argb_u32(), btn.border.to_argb_u32()),
        ButtonState::Hovered => (btn.background_hover.to_argb_u32(), btn.border.to_argb_u32()),
        ButtonState::Pressed => (btn.background_pressed.to_argb_u32(), btn.border.to_argb_u32()),
    };
    let fg = btn.foreground.to_argb_u32();

    let x = rect.x.round() as usize;
    let y = rect.y.round() as usize;
    let w = rect.width.round() as usize;
    let h = rect.height.round() as usize;

    // Draw bordered button
    frame.draw_bordered_rect(x, y, w, h, bg, border);

    // Focus ring: draw a second border 1px inside in the focus color
    if focused {
        let focus_color = btn.focus_ring.to_argb_u32();
        if w > 2 && h > 2 {
            // Top inner border
            frame.fill_rect_px(x + 1, y + 1, w - 2, 1, focus_color);
            // Bottom inner border
            frame.fill_rect_px(x + 1, y + h - 2, w - 2, 1, focus_color);
            // Left inner border
            frame.fill_rect_px(x + 1, y + 1, 1, h - 2, focus_color);
            // Right inner border
            frame.fill_rect_px(x + w - 2, y + 1, 1, h - 2, focus_color);
        }
    }

    // Center the label text
    let char_width = painter.char_width();
    let line_height = painter.line_height();
    let text_w = (label.len() as f32 * char_width).round() as usize;
    let text_x = x + w.saturating_sub(text_w) / 2;
    let text_y = y + h.saturating_sub(line_height) / 2;
    painter.draw(frame, text_x, text_y, label, fg);
}

/// Calculate a button rect centered horizontally at the given position
///
/// Returns a Rect sized to fit the label with standard padding.
pub fn button_rect(
    center_x: usize,
    y: usize,
    label: &str,
    char_width: f32,
    line_height: usize,
    padding_h: usize,
    padding_v: usize,
) -> Rect {
    let text_w = (label.len() as f32 * char_width).round() as usize;
    let w = text_w + padding_h * 2;
    let h = line_height + padding_v * 2;
    let x = center_x.saturating_sub(w / 2);
    Rect::new(x as f32, y as f32, w as f32, h as f32)
}
