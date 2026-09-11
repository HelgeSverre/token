//! A single-select segmented control: all mutually exclusive choices stay visible.
use super::button::{render_button, ButtonState, ButtonStyle};
use super::geometry::WidgetRect;
use super::{Frame, TextPainter};
use crate::{model::Rect, theme::Theme};

/// Equal-width, adjoining segments. The remainder belongs to the last segment.
pub fn segment_rects(bounds: Rect, count: usize) -> Vec<WidgetRect> {
    if count == 0 {
        return Vec::new();
    }
    (0..count)
        .map(|i| {
            let left = bounds.x as usize + bounds.width as usize * i / count;
            let right = bounds.x as usize + bounds.width as usize * (i + 1) / count;
            WidgetRect {
                x: left,
                y: bounds.y as usize,
                w: right - left,
                h: bounds.height as usize,
            }
        })
        .collect()
}

pub struct SegmentedControl<'a> {
    pub segments: &'a [WidgetRect],
    pub labels: &'a [&'a str],
    pub selected: usize,
    pub focused: bool,
    pub scale: f64,
}

impl SegmentedControl<'_> {
    pub fn render(&self, frame: &mut Frame, painter: &mut TextPainter, theme: &Theme) {
        for (i, (rect, label)) in self.segments.iter().zip(self.labels).enumerate() {
            render_button(
                frame,
                painter,
                theme,
                Rect::new(rect.x as f32, rect.y as f32, rect.w as f32, rect.h as f32),
                label,
                ButtonStyle {
                    state: if i == self.selected {
                        ButtonState::Selected
                    } else {
                        ButtonState::Normal
                    },
                    focused: self.focused && i == self.selected,
                    text_size: Some((12.0 * self.scale) as f32),
                },
            );
        }
    }
}
