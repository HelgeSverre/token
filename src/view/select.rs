//! Non-editable select (dropdown/pop-up button), composed from the shared menu surface.
use super::geometry::WidgetRect;
use super::overlay_surface::{
    self, Accessory, Anchor, Body, FlatIndex, OverlaySpec, Row, RowIcon, Section,
    SelectableListViewport, WidthRule,
};
use super::scrollbar::ScrollbarGeometry;
use super::{Frame, RoundedRectMaskCache, TextPainter};
use crate::{layout::PainterMeasure, model::Rect, theme::Theme};

use crate::model::select::SelectState;

pub struct SelectLayout {
    pub panel: WidgetRect,
    pub rows: Vec<WidgetRect>,
    pub first: usize,
    pub scrollbar: Option<ScrollbarGeometry>,
}

impl SelectLayout {
    pub fn option_at(&self, x: f32, y: f32) -> Option<usize> {
        super::section_navigation::section_at(&self.rows, x, y).map(|i| i + self.first)
    }
}

pub struct Select<'a> {
    pub anchor: Rect,
    pub labels: &'a [&'a str],
    pub selected: usize,
    pub state: &'a SelectState,
    pub focused: bool,
    pub scale: f64,
}

impl Select<'_> {
    pub fn render_anchor(&self, frame: &mut Frame, painter: &mut TextPainter, theme: &Theme) {
        super::controls::render_select(
            frame,
            painter,
            theme,
            WidgetRect {
                x: self.anchor.x as usize,
                y: self.anchor.y as usize,
                w: self.anchor.width as usize,
                h: self.anchor.height as usize,
            },
            self.labels
                .get(self.selected)
                .copied()
                .unwrap_or("No options"),
            self.state.open || self.focused,
            self.scale,
        );
    }

    /// Paint last so the popup overlays the owner's content. Returned geometry
    /// is the same measured plan used for pointer interaction.
    pub fn render_popup(
        &self,
        frame: &mut Frame,
        painter: &mut TextPainter,
        masks: &mut RoundedRectMaskCache,
        theme: &Theme,
        size: (usize, usize),
    ) -> Option<SelectLayout> {
        if !self.state.open || self.labels.is_empty() {
            return None;
        }
        let visible =
            (((size.1 as f32 - self.anchor.y - self.anchor.height - 24.0 * self.scale as f32)
                / (28.0 * self.scale as f32)) as usize)
                .clamp(1, 10);
        let viewport = SelectableListViewport::compute_from(
            self.labels.len(),
            self.state.active,
            visible,
            self.state.scroll,
        );
        let rows: Vec<_> = self
            .labels
            .iter()
            .enumerate()
            .map(|(i, label)| Row {
                icon: RowIcon::None,
                label,
                match_indices: &[],
                detail: None,
                detail_style: None,
                accessory: if i == self.selected {
                    Accessory::Check
                } else {
                    Accessory::None
                },
            })
            .collect();
        let sections = [Section {
            title: None,
            rows: &rows,
        }];
        let width = self.anchor.width / self.scale as f32;
        let spec = OverlaySpec {
            tabs: None,
            anchor: Anchor::Menu {
                x: self.anchor.x as usize,
                y: self.anchor.y as usize,
                h: self.anchor.height as usize,
                prefer_below: true,
                width: WidthRule {
                    pct: 0.0,
                    min: width,
                    max: width,
                },
            },
            header: None,
            body: Body::List {
                sections: &sections,
                selected: FlatIndex(viewport.selected_index),
                scroll: viewport.scroll_offset,
                max_visible: visible,
            },
            footer: None,
            hover_row: None,
            docs: None,
        };
        let layout = overlay_surface::layout_measured(
            &spec,
            size.0,
            size.1,
            self.scale,
            &mut PainterMeasure::new(painter),
        );
        overlay_surface::render(
            frame, painter, masks, theme, &spec, size.0, size.1, self.scale, false,
        );
        Some(SelectLayout {
            panel: layout.panel,
            rows: layout.rows,
            first: viewport.scroll_offset,
            scrollbar: layout.scrollbar,
        })
    }
}
