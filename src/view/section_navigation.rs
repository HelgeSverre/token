//! Category navigation shared by preferences and the development gallery.
//! Owners keep selection state; these rectangles also drive their hit testing.

use super::geometry::WidgetRect;
use super::helpers::EllipsisSide;
use super::{Frame, RoundedRectMaskCache, TextPainter};
use crate::theme::Theme;

pub const ROW_STEP: f32 = 33.0;

/// Return the section under the pointer, excluding gaps between rows.
pub fn section_at(rows: &[WidgetRect], x: f32, y: f32) -> Option<usize> {
    rows.iter().position(|rect| {
        x >= rect.x as f32
            && x < (rect.x + rect.w) as f32
            && y >= rect.y as f32
            && y < (rect.y + rect.h) as f32
    })
}

/// Lay out a vertical list or a row-major grid within the supplied width.
pub fn section_rects(
    bounds: WidgetRect,
    count: usize,
    columns: usize,
    scale: f64,
) -> Vec<WidgetRect> {
    let columns = columns.max(1);
    let width = bounds.w / columns;
    (0..count)
        .map(|index| WidgetRect {
            x: bounds.x + index % columns * width,
            y: bounds.y + index / columns * scaled(ROW_STEP, scale),
            w: width,
            h: scaled(28.0, scale),
        })
        .collect()
}

/// A resolved navigation component. Layout is supplied by the owner so paint
/// and hit testing cannot disagree; the divider is optional for grid layouts.
pub struct SectionNavigation<'a> {
    pub rows: &'a [WidgetRect],
    pub divider: Option<WidgetRect>,
    pub selected: usize,
    pub scale: f64,
}

impl SectionNavigation<'_> {
    pub fn render<'a>(
        &self,
        frame: &mut Frame,
        painter: &mut TextPainter,
        masks: &mut RoundedRectMaskCache,
        theme: &Theme,
        labels: impl IntoIterator<Item = &'a str>,
    ) {
        let colors = &theme.overlay;
        if let Some(rect) = self.divider {
            frame.fill_rect_px(
                rect.x,
                rect.y,
                rect.w,
                rect.h,
                colors.hairline.to_argb_u32(),
            );
        }
        for (index, (rect, label)) in self.rows.iter().zip(labels).enumerate() {
            let selected = index == self.selected;
            if selected {
                frame.fill_rounded_rect(
                    rect.x,
                    rect.y,
                    rect.w,
                    rect.h,
                    scaled(2.0, self.scale),
                    colors.keycap_bg.to_argb_u32(),
                    masks,
                );
            }
            let size = (12.0 * self.scale) as f32;
            let label = painter.truncate_sized(
                label,
                size,
                rect.w.saturating_sub(scaled(20.0, self.scale)) as f32,
                EllipsisSide::End,
            );
            painter.draw_sized(
                frame,
                rect.x + scaled(10.0, self.scale),
                rect.y + scaled(7.0, self.scale),
                &label,
                size,
                0.0,
                if selected {
                    colors.text_bright
                } else {
                    colors.text_dim
                }
                .to_argb_u32(),
            );
        }
    }
}

fn scaled(value: f32, scale: f64) -> usize {
    (value as f64 * scale).round().max(1.0) as usize
}
