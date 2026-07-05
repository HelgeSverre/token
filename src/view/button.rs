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
        ButtonState::Pressed => (
            btn.background_pressed.to_argb_u32(),
            btn.border.to_argb_u32(),
        ),
    };
    let fg = btn.foreground.to_argb_u32();

    let x = rect.x.round() as usize;
    let y = rect.y.round() as usize;
    let w = rect.width.round() as usize;
    let h = rect.height.round() as usize;

    // Draw bordered button
    frame.draw_bordered_rect(x, y, w, h, bg, border);

    // Focus ring: draw an outset ring just outside the button bounds so it
    // reads as a clear, higher-contrast indicator (rather than a thin line
    // lost against the button's own border), and draw it unconditionally
    // regardless of button size so small buttons still get a visible focus
    // indicator.
    if focused {
        let focus_color = btn.focus_ring.to_argb_u32();
        draw_focus_ring(frame, x, y, w, h, focus_color);
    }

    // Center the label text
    let char_width = painter.char_width();
    let line_height = painter.line_height();
    let text_w = (label.len() as f32 * char_width).round() as usize;
    let text_x = x + w.saturating_sub(text_w) / 2;
    let text_y = y + h.saturating_sub(line_height) / 2;
    painter.draw(frame, text_x, text_y, label, fg);
}

/// Draw a focus ring just outside the given rect (an "outset" ring), rather
/// than inset inside it.
///
/// The ring hugs the outer edge of the button bounds directly (no gap) and is
/// drawn unconditionally, regardless of `w`/`h` — including degenerate sizes
/// like `w == 0` or `h == 0` — so small buttons still get a visible focus
/// indicator instead of the ring being skipped entirely.
fn draw_focus_ring(frame: &mut Frame, x: usize, y: usize, w: usize, h: usize, color: u32) {
    const RING_THICKNESS: usize = 1;
    const OUTSET: usize = 1;

    let rx = x.saturating_sub(OUTSET);
    let ry = y.saturating_sub(OUTSET);
    let rw = w + OUTSET * 2;
    let rh = h + OUTSET * 2;

    // Top edge
    frame.fill_rect_px(rx, ry, rw, RING_THICKNESS, color);
    // Bottom edge
    frame.fill_rect_px(
        rx,
        ry + rh.saturating_sub(RING_THICKNESS),
        rw,
        RING_THICKNESS,
        color,
    );
    // Left edge
    frame.fill_rect_px(rx, ry, RING_THICKNESS, rh, color);
    // Right edge
    frame.fill_rect_px(
        rx + rw.saturating_sub(RING_THICKNESS),
        ry,
        RING_THICKNESS,
        rh,
        color,
    );
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

#[cfg(test)]
mod tests {
    use super::*;

    const FOCUS_COLOR: u32 = 0xFF00FFFF;

    /// M13 regression: previously the whole focus-ring block was gated by
    /// `w > 2 && h > 2`, so a tiny (e.g. 2x2 or 3x3) button never got a
    /// focus indicator at all. The ring must now be drawn unconditionally.
    #[test]
    fn focus_ring_is_drawn_for_a_tiny_2x2_button() {
        let width = 20;
        let height = 20;
        let mut buffer = vec![0u32; width * height];
        let mut frame = Frame::new(&mut buffer, width, height);

        // A tiny 2x2 button placed away from the frame edges so the outset
        // ring has room to be drawn on all sides.
        let (bx, by, bw, bh) = (10usize, 10usize, 2usize, 2usize);
        draw_focus_ring(&mut frame, bx, by, bw, bh, FOCUS_COLOR);

        // Ring should be drawn just outside the button bounds (outset by 1px).
        // Top-left corner of the ring.
        assert_eq!(frame.get_pixel(bx - 1, by - 1), FOCUS_COLOR);
        // Top-right corner of the ring.
        assert_eq!(frame.get_pixel(bx + bw, by - 1), FOCUS_COLOR);
        // Bottom-left corner of the ring.
        assert_eq!(frame.get_pixel(bx - 1, by + bh), FOCUS_COLOR);
        // Bottom-right corner of the ring.
        assert_eq!(frame.get_pixel(bx + bw, by + bh), FOCUS_COLOR);

        // Interior of the button itself must be untouched by the ring.
        assert_eq!(frame.get_pixel(bx, by), 0);
    }

    /// M13 regression: a 3x3 button (also previously below the `w > 2 && h >
    /// 2` gate, since neither condition is strictly greater) must also get a
    /// visible ring.
    #[test]
    fn focus_ring_is_drawn_for_a_3x3_button() {
        let width = 20;
        let height = 20;
        let mut buffer = vec![0u32; width * height];
        let mut frame = Frame::new(&mut buffer, width, height);

        let (bx, by, bw, bh) = (5usize, 5usize, 3usize, 3usize);
        draw_focus_ring(&mut frame, bx, by, bw, bh, FOCUS_COLOR);

        assert_eq!(frame.get_pixel(bx - 1, by - 1), FOCUS_COLOR);
        assert_eq!(frame.get_pixel(bx + bw, by + bh), FOCUS_COLOR);
    }

    /// Degenerate zero-size buttons must not panic and should still produce
    /// a (minimal) ring rather than being skipped.
    #[test]
    fn focus_ring_handles_zero_size_button_without_panicking() {
        let width = 10;
        let height = 10;
        let mut buffer = vec![0u32; width * height];
        let mut frame = Frame::new(&mut buffer, width, height);

        draw_focus_ring(&mut frame, 3, 3, 0, 0, FOCUS_COLOR);
        // With w = h = 0, the ring degenerates to a single 2x2 outset block;
        // just assert it did not panic and drew something at the corner.
        assert_eq!(frame.get_pixel(2, 2), FOCUS_COLOR);
    }

    /// Buttons flush against the top-left frame edge must not panic (the
    /// outset would otherwise underflow x/y).
    #[test]
    fn focus_ring_handles_button_at_frame_origin_without_panicking() {
        let width = 10;
        let height = 10;
        let mut buffer = vec![0u32; width * height];
        let mut frame = Frame::new(&mut buffer, width, height);

        // Should not panic even though x - 1 / y - 1 would underflow.
        draw_focus_ring(&mut frame, 0, 0, 2, 2, FOCUS_COLOR);
        assert_eq!(frame.get_pixel(0, 0), FOCUS_COLOR);
    }
}
