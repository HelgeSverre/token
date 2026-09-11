//! Small form controls shared by Settings and the native component gallery.
//! Callers own interaction state; shared geometry drives painting and hit testing.

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

/// The Settings choice-group geometry. Both painting and hit testing consume
/// these rectangles, and the gallery uses them for the same wrapped layout.
pub fn choice_group_rects(row: WidgetRect, labels: &[&str], scale_factor: f64) -> Vec<WidgetRect> {
    if labels.is_empty() {
        return Vec::new();
    }
    let control = settings_control_rect(row, scale_factor);
    let budget = if row.w < scaled(400.0, scale_factor) {
        row.w
    } else {
        row.w * 2 / 3
    };
    let gap = scaled(super::overlay_surface::dims::CHIP_GAP, scale_factor);
    let widths: Vec<_> = labels
        .iter()
        .map(|label| choice_width(label, scale_factor))
        .collect();
    let total = widths.iter().sum::<usize>() + gap * labels.len().saturating_sub(1);
    if total > budget {
        let mut x = row.x;
        let mut y = row.y
            + scaled(
                if row.w < scaled(400.0, scale_factor) {
                    26.0
                } else {
                    50.0
                },
                scale_factor,
            );
        let h = scaled(22.0, scale_factor);
        return widths
            .into_iter()
            .map(|width| {
                let w = width.min(row.w);
                if x > row.x && x + w > row.x + row.w {
                    x = row.x;
                    y += h + gap;
                }
                let rect = WidgetRect { x, y, w, h };
                x += w + gap;
                rect
            })
            .collect();
    }
    let mut x = control.x + control.w.saturating_sub(total);
    let h = scaled(22.0, scale_factor).min(control.h);
    widths
        .into_iter()
        .map(|w| {
            let rect = WidgetRect {
                x,
                y: control.y + control.h.saturating_sub(h) / 2,
                w,
                h,
            };
            x += w + gap;
            rect
        })
        .collect()
}

/// The popup rows below a Settings select anchor, clamped to the form body.
pub fn select_option_rects(
    anchor: WidgetRect,
    count: usize,
    top: usize,
    bottom: usize,
    scale_factor: f64,
) -> Vec<WidgetRect> {
    if count == 0 {
        return Vec::new();
    }
    let height = scaled(28.0, scale_factor).min(bottom.saturating_sub(top) / count);
    let total = height * count;
    let y = (anchor.y + anchor.h)
        .min(bottom.saturating_sub(total))
        .max(top);
    (0..count)
        .map(|choice| WidgetRect {
            x: anchor.x,
            y: y + choice * height,
            w: anchor.w,
            h: height,
        })
        .collect()
}

/// A Settings disclosure uses its label as the affordance; its row owns the
/// interaction rectangle.
pub fn render_disclosure(
    frame: &mut Frame,
    painter: &mut TextPainter,
    theme: &Theme,
    rect: WidgetRect,
    expanded: bool,
    scale_factor: f64,
) {
    let size = (12.0 * scale_factor) as f32;
    let label = painter.truncate_sized(
        if expanded {
            "▾  Advanced"
        } else {
            "▸  Advanced"
        },
        size,
        rect.w as f32,
        super::helpers::EllipsisSide::End,
    );
    painter.draw_sized(
        frame,
        rect.x,
        rect.y,
        &label,
        size,
        0.0,
        theme.overlay.text_primary.to_argb_u32(),
    );
}

pub fn choice_width(label: &str, scale_factor: f64) -> usize {
    scaled(label.chars().count() as f32 * 7.0 + 16.0, scale_factor)
}

pub(super) fn settings_control_rect(rect: WidgetRect, scale_factor: f64) -> WidgetRect {
    if rect.w < scaled(400.0, scale_factor) {
        WidgetRect {
            x: rect.x,
            y: rect.y + scaled(26.0, scale_factor),
            w: rect.w,
            h: scaled(22.0, scale_factor),
        }
    } else {
        WidgetRect {
            x: rect.x,
            y: rect.y,
            w: rect.w,
            h: scaled(32.0, scale_factor),
        }
    }
}

fn scaled(value: f32, scale_factor: f64) -> usize {
    (value as f64 * scale_factor).round().max(1.0) as usize
}
