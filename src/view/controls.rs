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
    let size = (13.0 * scale) as f32;
    // Text, then a chevron box on the trailing edge; the label never runs
    // under the chevron.
    let chevron_box = px(28.0);
    let label = painter.truncate_sized(
        label,
        size,
        rect.w.saturating_sub(px(10.0) + chevron_box) as f32,
        super::helpers::EllipsisSide::End,
    );
    frame.push_clip(Rect::new(
        rect.x as f32,
        rect.y as f32,
        rect.w as f32,
        rect.h as f32,
    ));
    let line_height = painter.line_height_for_size(size);
    let text_y = rect.y + rect.h.saturating_sub(line_height) / 2;
    painter.draw_sized(
        frame,
        rect.x + px(10.0),
        text_y,
        &label,
        size,
        0.0,
        colors.text_primary.to_argb_u32(),
    );
    draw_chevron_down(
        frame,
        rect.x + rect.w.saturating_sub(chevron_box / 2),
        rect.y + rect.h / 2,
        scale,
        if open {
            colors.text_primary
        } else {
            colors.text_dim
        }
        .to_argb_u32(),
    );
    frame.pop_clip();
}

/// A downward chevron centred on (`cx`, `cy`): two diagonal strokes, four
/// logical pixels each way, drawn as scaled squares so it stays crisp at any
/// display scale without a font glyph.
fn draw_chevron_down(frame: &mut Frame, cx: usize, cy: usize, scale: f64, color: u32) {
    let half = ((4.0 * scale).round() as usize).max(2);
    let thick = ((1.5 * scale).round() as usize).max(1);
    let top = cy.saturating_sub(half.div_ceil(2));
    for step in 0..=half {
        let y = top + step;
        frame.fill_rect_px((cx + step).saturating_sub(half), y, thick, thick, color);
        frame.fill_rect_px(cx + half - step, y, thick, thick, color);
    }
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
