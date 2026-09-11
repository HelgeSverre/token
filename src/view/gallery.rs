//! Native component specimens, using the same painters as application controls.
use super::button::{render_button, ButtonState, ButtonStyle};
use super::controls::{render_checkbox, render_field_surface, render_select};
use super::geometry::WidgetRect;
use super::scrollbar::{render_scrollbar, ScrollbarColors, ScrollbarGeometry, ScrollbarState};
use super::{FontRole, Frame, GlyphCache, TextFieldOptions, TextFieldRenderer, TextPainter};
use crate::editable::{EditConstraints, EditableState, StringBuffer};
use crate::model::gallery::{GalleryState, Specimen, CATEGORIES};
use crate::model::Rect;
use crate::theme::Theme;

/// One layout snapshot drives drawing, wheel limits, and pointer handling.
pub struct GalleryLayout {
    pub search: Rect,
    pub theme: Rect,
    pub width_toggle: Rect,
    pub categories: Vec<Rect>,
    pub viewport: Rect,
    pub scrollbar: ScrollbarGeometry,
    pub rows: Vec<(&'static Specimen, Rect)>,
}

impl GalleryLayout {
    pub fn new(width: usize, height: usize, scale: f64, state: &GalleryState) -> Self {
        let s = scale as f32;
        let w = width as f32;
        let h = height as f32;
        let viewport = Rect::new(
            190.0 * s,
            144.0 * s,
            (w - 206.0 * s).max(0.0),
            (h - 180.0 * s).max(0.0),
        );
        let specimens = state.specimens();
        let total = (specimens.len() as f32 * 118.0 * s) as usize;
        let visible = viewport.height as usize;
        let offset = state
            .scroll
            .clamp(0.0, total.saturating_sub(visible) as f64);
        let scrollbar = ScrollbarGeometry::vertical(
            Rect::new(w - 12.0 * s, viewport.y, 12.0 * s, viewport.height),
            &ScrollbarState::new(total, visible, offset as usize),
        );
        let rows = specimens
            .into_iter()
            .enumerate()
            .map(|(i, spec)| {
                (
                    spec,
                    Rect::new(
                        viewport.x,
                        viewport.y + i as f32 * 118.0 * s - offset as f32,
                        viewport.width,
                        118.0 * s,
                    ),
                )
            })
            .collect();
        Self {
            search: Rect::new(24.0 * s, 76.0 * s, (w - 380.0 * s).max(80.0 * s), 34.0 * s),
            theme: Rect::new(w - 336.0 * s, 76.0 * s, 180.0 * s, 34.0 * s),
            width_toggle: Rect::new(w - 144.0 * s, 76.0 * s, 120.0 * s, 34.0 * s),
            categories: CATEGORIES
                .iter()
                .enumerate()
                .map(|(i, _)| {
                    Rect::new(16.0 * s, (148.0 + i as f32 * 40.0) * s, 154.0 * s, 32.0 * s)
                })
                .collect(),
            viewport,
            scrollbar,
            rows,
        }
    }
}

/// Retains the production font raster caches between redraws. Also used headlessly.
pub struct GalleryRenderer {
    fonts: super::fonts::Fonts,
    code_cache: GlyphCache,
    ui_cache: GlyphCache,
}

impl GalleryRenderer {
    pub fn new() -> anyhow::Result<Self> {
        Ok(Self {
            fonts: super::fonts::Fonts::load("JetBrains Mono", "Inter")?,
            code_cache: GlyphCache::new(),
            ui_cache: GlyphCache::new(),
        })
    }

    pub fn render(
        &mut self,
        buffer: &mut [u32],
        size: (usize, usize),
        scale: f64,
        state: &GalleryState,
        theme: &Theme,
    ) -> GalleryLayout {
        let layout = GalleryLayout::new(size.0, size.1, scale, state);
        let font_size = (14.0 * scale) as f32;
        let metrics = self
            .fonts
            .editor
            .horizontal_line_metrics(font_size)
            .expect("bundled font metrics");
        let mut painter = TextPainter::new(
            &self.fonts.editor,
            &mut self.code_cache,
            font_size,
            metrics.ascent,
            self.fonts.editor.metrics('M', font_size).advance_width,
            (20.0 * scale) as usize,
        )
        .with_ui_font(&self.fonts.ui, &mut self.ui_cache, FontRole::Ui);
        let mut frame = Frame::new(buffer, size.0, size.1);
        let colors = &theme.overlay;
        let px = |n: f64| (n * scale).round() as usize;
        frame.clear(colors.panel_background.to_argb_u32());
        frame.fill_rect_px(
            0,
            0,
            size.0,
            px(130.0),
            colors.panel_secondary.to_argb_u32(),
        );
        painter.draw_sized(
            &mut frame,
            px(24.0),
            px(20.0),
            "Token / UI Gallery",
            (22.0 * scale) as f32,
            0.0,
            colors.text_bright.to_argb_u32(),
        );
        painter.draw_sized(
            &mut frame,
            px(24.0),
            px(50.0),
            "Development specimens · real application painters · named visual states",
            (12.0 * scale) as f32,
            0.0,
            colors.text_dim.to_argb_u32(),
        );
        paint_field(
            &mut frame,
            &mut painter,
            theme,
            layout.search,
            &state.query,
            true,
        );
        if state.query.text().is_empty() {
            painter.with_font(FontRole::Code).draw_sized(
                &mut frame,
                layout.search.x as usize + px(12.0),
                layout.search.y as usize + px(9.0),
                "Type to filter components…",
                (12.0 * scale) as f32,
                0.0,
                colors.text_dim.to_argb_u32(),
            );
        }
        for (rect, label) in [
            (layout.theme, format!("Theme: {}", theme.name)),
            (
                layout.width_toggle,
                if state.compact {
                    "Width: narrow"
                } else {
                    "Width: wide"
                }
                .to_owned(),
            ),
        ] {
            render_button(
                &mut frame,
                &mut painter,
                theme,
                rect,
                &label,
                ButtonStyle {
                    text_size: Some((12.0 * scale) as f32),
                    ..Default::default()
                },
            );
        }
        for (i, rect) in layout.categories.iter().enumerate() {
            render_button(
                &mut frame,
                &mut painter,
                theme,
                *rect,
                CATEGORIES[i],
                ButtonStyle {
                    state: if i == state.category {
                        ButtonState::Selected
                    } else {
                        ButtonState::Normal
                    },
                    text_size: Some((13.0 * scale) as f32),
                    ..Default::default()
                },
            );
        }
        frame.push_clip(layout.viewport);
        for (spec, rect) in &layout.rows {
            if rect.y + rect.height <= layout.viewport.y
                || rect.y >= layout.viewport.y + layout.viewport.height
            {
                continue;
            }
            let x = rect.x as usize + px(12.0);
            let y = rect.y.max(0.0) as usize;
            frame.fill_rect_px(
                x,
                y + px(112.0),
                rect.width as usize - px(24.0).min(rect.width as usize),
                1,
                colors.hairline.to_argb_u32(),
            );
            painter.draw_sized(
                &mut frame,
                x,
                y + px(12.0),
                spec.id,
                (15.0 * scale) as f32,
                0.0,
                colors.text_bright.to_argb_u32(),
            );
            painter.draw_sized(
                &mut frame,
                x,
                y + px(38.0),
                spec.source,
                (11.0 * scale) as f32,
                0.0,
                colors.text_dim.to_argb_u32(),
            );
            painter.draw_sized(
                &mut frame,
                x,
                y + px(60.0),
                spec.tokens,
                (11.0 * scale) as f32,
                0.0,
                colors.text_dim.to_argb_u32(),
            );
            let preview_w = if state.compact { 130.0 } else { 220.0 } * scale as f32;
            let preview = Rect::new(
                rect.x + rect.width - preview_w - 20.0 * scale as f32,
                rect.y + 28.0 * scale as f32,
                preview_w,
                34.0 * scale as f32,
            );
            // Keep long metadata out of the specimen's area on narrow windows.
            frame.fill_rect_px(
                preview.x as usize - px(8.0).min(preview.x as usize),
                y + px(20.0),
                preview.width as usize + px(16.0),
                px(74.0),
                colors.panel_background.to_argb_u32(),
            );
            paint_specimen(&mut frame, &mut painter, theme, spec, preview, scale);
            painter.draw_sized(
                &mut frame,
                preview.x as usize,
                y + px(76.0),
                "STATIC STATE",
                (10.0 * scale) as f32,
                0.0,
                colors.text_dim.to_argb_u32(),
            );
        }
        if layout.rows.is_empty() {
            painter.draw(
                &mut frame,
                px(210.0),
                px(160.0),
                "No matching components",
                colors.text_dim.to_argb_u32(),
            );
        }
        frame.pop_clip();
        render_scrollbar(
            &mut frame,
            &layout.scrollbar,
            false,
            &ScrollbarColors::from(&theme.scrollbar),
        );
        frame.fill_rect_px(
            0,
            size.1.saturating_sub(px(32.0)),
            size.0,
            px(32.0),
            colors.panel_secondary.to_argb_u32(),
        );
        painter.draw_sized(&mut frame, px(24.0), size.1.saturating_sub(px(24.0)), &format!("{} specimens · wheel / drag scrollbar · Esc clears filter · theme and width controls above", layout.rows.len()), (11.0*scale) as f32, 0.0, colors.text_dim.to_argb_u32());
        layout
    }
}

fn widget(rect: Rect) -> WidgetRect {
    WidgetRect {
        x: rect.x.max(0.0) as usize,
        y: rect.y.max(0.0) as usize,
        w: rect.width.max(0.0) as usize,
        h: rect.height.max(0.0) as usize,
    }
}

fn paint_field(
    frame: &mut Frame,
    painter: &mut TextPainter,
    theme: &Theme,
    rect: Rect,
    content: &EditableState<StringBuffer>,
    focused: bool,
) {
    let mut inner = widget(rect);
    render_field_surface(frame, theme, inner, focused);
    let inset = (painter.char_width() * 0.8) as usize;
    inner.x += inset;
    inner.w = inner.w.saturating_sub(2 * inset);
    let mut opts = TextFieldOptions::for_text_box(
        content,
        &inner,
        painter.line_height(),
        painter.char_width(),
    );
    opts.text_color = theme.overlay.text_primary.to_argb_u32();
    opts.selection_color = theme.editor.selection_background.to_argb_u32();
    opts.cursor_color = theme.editor.cursor_color.to_argb_u32();
    opts.cursor_visible = focused;
    frame.push_clip(rect);
    TextFieldRenderer::render(frame, painter, content, &opts);
    frame.pop_clip();
}

fn paint_specimen(
    frame: &mut Frame,
    painter: &mut TextPainter,
    theme: &Theme,
    spec: &Specimen,
    rect: Rect,
    scale: f64,
) {
    use crate::model::gallery::Preview;
    match spec.preview {
        Preview::ButtonNormal
        | Preview::ButtonHovered
        | Preview::ButtonPressed
        | Preview::ButtonFocused
        | Preview::ButtonSelected
        | Preview::ButtonDisabled
        | Preview::ButtonLongLabel => {
            let state = match spec.preview {
                Preview::ButtonHovered => ButtonState::Hovered,
                Preview::ButtonPressed => ButtonState::Pressed,
                Preview::ButtonSelected => ButtonState::Selected,
                Preview::ButtonDisabled => ButtonState::Disabled,
                _ => ButtonState::Normal,
            };
            render_button(
                frame,
                painter,
                theme,
                rect,
                if matches!(spec.preview, Preview::ButtonLongLabel) {
                    "A deliberately long button label"
                } else {
                    "Configure server"
                },
                ButtonStyle {
                    state,
                    focused: matches!(spec.preview, Preview::ButtonFocused),
                    text_size: Some((12.0 * scale) as f32),
                },
            );
        }
        Preview::FieldUnfocused | Preview::FieldFocused | Preview::FieldSelection => {
            let mut content = EditableState::new(
                StringBuffer::from_text("rust-analyzer"),
                EditConstraints::single_line(),
            );
            content.move_line_end(false);
            if matches!(spec.preview, Preview::FieldSelection) {
                content.select_all();
            }
            paint_field(
                frame,
                painter,
                theme,
                rect,
                &content,
                !matches!(spec.preview, Preview::FieldUnfocused),
            );
        }
        Preview::Checkbox(checked) => {
            let mut r = widget(rect);
            r.w = (16.0 * scale) as usize;
            r.h = r.w;
            render_checkbox(frame, painter, theme, r, checked, scale);
            painter.draw_sized(
                frame,
                r.x + (26.0 * scale) as usize,
                r.y,
                "Inlay hints",
                (12.0 * scale) as f32,
                0.0,
                theme.overlay.text_primary.to_argb_u32(),
            );
        }
        Preview::Select { open } => {
            render_select(
                frame,
                painter,
                theme,
                widget(rect),
                "On focus loss",
                open,
                scale,
            );
        }
        Preview::Panel | Preview::Secondary | Preview::Recessed => {
            let bg = match spec.preview {
                Preview::Secondary => theme.overlay.panel_secondary,
                Preview::Recessed => theme.overlay.recessed_wash,
                _ => theme.overlay.panel_background,
            };
            let r = widget(rect);
            frame.draw_bordered_rect(
                r.x,
                r.y,
                r.w,
                r.h,
                bg.to_argb_u32(),
                theme.overlay.hairline.to_argb_u32(),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn gallery_filters_and_scroll_geometry_share_visible_rows() {
        let mut state = GalleryState {
            category: 1,
            scroll: 100_000.0,
            ..Default::default()
        };
        let layout = GalleryLayout::new(1000, 600, 1.0, &state);
        assert_eq!(layout.rows.len(), 7);
        assert_eq!(
            layout.scrollbar.state.position,
            layout.scrollbar.state.max_position()
        );
        let last = layout.rows.last().unwrap().1;
        assert!((last.y + last.height - layout.viewport.y - layout.viewport.height).abs() < 1.0);
        state.query.insert_text("disabled");
        assert_eq!(state.specimens()[0].id, "button.disabled");
        state.query.insert_text("no match");
        assert!(GalleryLayout::new(1000, 600, 1.0, &state).rows.is_empty());
    }
}
