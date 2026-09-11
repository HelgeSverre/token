//! Small form controls shared by Settings and the native component gallery.
//! Callers own state and hit rectangles; these functions only paint.

use super::geometry::WidgetRect;
use super::{Frame, TextPainter};
use crate::model::Rect;
use crate::theme::Theme;

pub fn render_checkbox(
    frame: &mut Frame,
    painter: &mut TextPainter,
    theme: &Theme,
    rect: WidgetRect,
    checked: bool,
    scale: f64,
) {
    let colors = &theme.overlay;
    frame.draw_bordered_rect(
        rect.x,
        rect.y,
        rect.w,
        rect.h,
        if checked {
            colors.accent
        } else {
            colors.recessed_wash
        }
        .to_argb_u32(),
        colors.hairline.to_argb_u32(),
    );
    if checked {
        let size = (12.0 * scale) as f32;
        let mark =
            painter.truncate_sized("✓", size, rect.w as f32, super::helpers::EllipsisSide::End);
        painter.draw_sized(
            frame,
            rect.x,
            rect.y,
            &mark,
            size,
            0.0,
            colors.text_bright.to_argb_u32(),
        );
    }
}

pub fn render_select(
    frame: &mut Frame,
    painter: &mut TextPainter,
    theme: &Theme,
    rect: WidgetRect,
    label: &str,
    open: bool,
    scale: f64,
) {
    let colors = &theme.overlay;
    frame.draw_bordered_rect(
        rect.x,
        rect.y,
        rect.w,
        rect.h,
        colors.recessed_wash.to_argb_u32(),
        if open { colors.accent } else { colors.hairline }.to_argb_u32(),
    );
    let px = |n: f64| (n * scale).round() as usize;
    let size = (12.0 * scale) as f32;
    let label = painter.truncate_sized(
        label,
        size,
        rect.w.saturating_sub(px(32.0)) as f32,
        super::helpers::EllipsisSide::End,
    );
    frame.push_clip(Rect::new(
        rect.x as f32,
        rect.y as f32,
        rect.w as f32,
        rect.h as f32,
    ));
    painter.draw_sized(
        frame,
        rect.x + px(8.0),
        rect.y + px(7.0),
        &label,
        size,
        0.0,
        colors.text_primary.to_argb_u32(),
    );
    painter.draw_sized(
        frame,
        rect.x + rect.w.saturating_sub(px(20.0)),
        rect.y + px(7.0),
        "▾",
        size,
        0.0,
        colors.text_dim.to_argb_u32(),
    );
    frame.pop_clip();
}

pub fn render_field_surface(frame: &mut Frame, theme: &Theme, rect: WidgetRect, focused: bool) {
    let colors = &theme.overlay;
    frame.draw_bordered_rect(
        rect.x,
        rect.y,
        rect.w,
        rect.h,
        colors.recessed_wash.to_argb_u32(),
        if focused {
            colors.accent
        } else {
            colors.hairline
        }
        .to_argb_u32(),
    );
}
