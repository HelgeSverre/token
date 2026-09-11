//! Native component specimens, using the same painters as application controls.
use super::button::{render_button, ButtonState, ButtonStyle};
use super::controls::{
    choice_group_rects, render_checkbox, render_disclosure, render_field_surface, render_select,
    select_option_rects,
};
use super::frame::RoundedRectMaskCache;
use super::geometry::WidgetRect;
use super::overlay_surface::{
    self, Accessory, Anchor, Body, Field, FlatIndex, Header, OverlaySpec, Row, RowIcon, Section,
    WidthRule,
};
use super::scrollbar::{render_scrollbar, ScrollbarColors, ScrollbarGeometry, ScrollbarState};
use super::{FontRole, Frame, GlyphCache, TextFieldOptions, TextFieldRenderer, TextPainter};
use crate::completion::menu::MenuItemKind;
use crate::editable::{EditConstraints, EditableState, Position, StringBuffer};
use crate::model::gallery::{GalleryState, Preview, Specimen, CATEGORIES};
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
    pub rows: Vec<GalleryRow>,
}

pub struct GalleryRow {
    pub specimen: &'static Specimen,
    pub rect: Rect,
    /// Local to the row, independent of its scroll position.
    pub preview: Rect,
}

/// Fixture constraints, not a control's stretch allocation. Heights describe
/// the actual example; row spacing is derived from them separately.
fn specimen_size(preview: Preview, compact: bool) -> (f32, f32) {
    let field_width = if compact { 130.0 } else { 220.0 };
    let popup_width = if compact { 260.0 } else { 400.0 };
    match preview {
        Preview::IconButton => (22.0, 22.0),
        Preview::ButtonNormal
        | Preview::ButtonHovered
        | Preview::ButtonPressed
        | Preview::ButtonFocused
        | Preview::ButtonSelected
        | Preview::ButtonDisabled
        | Preview::ButtonLongLabel => (field_width, 22.0),
        Preview::FieldUnfocused
        | Preview::FieldFocused
        | Preview::FieldSelection
        | Preview::Panel
        | Preview::Secondary
        | Preview::Recessed => (field_width, 34.0),
        Preview::FieldMultiline => (if compact { 220.0 } else { 320.0 }, 100.0),
        Preview::Checkbox(_) => (field_width, 14.0),
        Preview::Select { .. } => (field_width, 29.0),
        Preview::SelectOptions => (field_width, 113.0),
        Preview::ChoiceGroup => (field_width, if compact { 74.0 } else { 48.0 }),
        Preview::Disclosure { .. } => (field_width, 22.0),
        Preview::SearchField => (popup_width, 72.0),
        Preview::ListRow => (popup_width, 56.0),
        Preview::FormValidation => (popup_width, 96.0),
        Preview::MenuRows { .. } => (popup_width, 80.0),
    }
}

impl GalleryLayout {
    pub fn new(width: usize, height: usize, scale: f64, state: &GalleryState) -> Self {
        let s = scale as f32;
        let w = width as f32;
        let h = height as f32;
        let viewport = Rect::new(
            190.0 * s,
            144.0 * s,
            (w - 206.0 * s).clamp(0.0, 1100.0 * s),
            (h - 180.0 * s).max(0.0),
        );
        let specimens = state.specimens();
        let total = (specimens
            .iter()
            .map(|spec| (specimen_size(spec.preview, state.compact).1 + 64.0).max(112.0) * s)
            .sum::<f32>()) as usize;
        let visible = viewport.height as usize;
        let offset = state
            .scroll
            .clamp(0.0, total.saturating_sub(visible) as f64);
        let scrollbar = ScrollbarGeometry::vertical(
            Rect::new(w - 12.0 * s, viewport.y, 12.0 * s, viewport.height),
            &ScrollbarState::new(total, visible, offset as usize),
        );
        let mut row_y = viewport.y - offset as f32;
        let rows = specimens
            .into_iter()
            .map(|spec| {
                let (control_w, control_h) = specimen_size(spec.preview, state.compact);
                let height = (control_h + 64.0).max(112.0) * s;
                let rect = Rect::new(viewport.x, row_y, viewport.width, height);
                row_y += height;
                let max_preview_w = if state.compact { 260.0 } else { 400.0 };
                let metadata_w = (viewport.width / s - max_preview_w - 48.0).clamp(180.0, 420.0);
                GalleryRow {
                    specimen: spec,
                    rect,
                    preview: Rect::new(
                        (metadata_w + 32.0) * s,
                        16.0 * s,
                        control_w * s,
                        control_h * s,
                    ),
                }
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
    masks: RoundedRectMaskCache,
    row_buffer: Vec<u32>,
}

struct SpecimenPaintContext<'a> {
    masks: &'a mut RoundedRectMaskCache,
    theme: &'a Theme,
    scale: f64,
    size: (usize, usize),
}

impl GalleryRenderer {
    pub fn new() -> anyhow::Result<Self> {
        Ok(Self {
            fonts: super::fonts::Fonts::load("JetBrains Mono", "Inter")?,
            code_cache: GlyphCache::new(),
            ui_cache: GlyphCache::new(),
            masks: RoundedRectMaskCache::new(),
            row_buffer: Vec::new(),
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
        for row in &layout.rows {
            let rect = row.rect;
            if rect.y + rect.height <= layout.viewport.y
                || rect.y >= layout.viewport.y + layout.viewport.height
            {
                continue;
            }
            let row_size = (rect.width.ceil() as usize, rect.height.ceil() as usize);
            self.row_buffer.resize(row_size.0 * row_size.1, 0);
            {
                // Local rendering keeps popup anchoring and unsigned paint
                // coordinates independent of the gallery's scroll position.
                let mut tile = Frame::new(&mut self.row_buffer, row_size.0, row_size.1);
                tile.clear(colors.panel_background.to_argb_u32());
                let metadata_width = row.preview.x as usize - px(32.0);
                for (label, y, font_size, color) in [
                    (row.specimen.id, 12.0, 15.0, colors.text_bright),
                    (row.specimen.source, 38.0, 11.0, colors.text_dim),
                    (row.specimen.tokens, 60.0, 11.0, colors.text_dim),
                ] {
                    let font_size = (font_size * scale) as f32;
                    let label = painter.truncate_sized(
                        label,
                        font_size,
                        metadata_width as f32,
                        super::helpers::EllipsisSide::End,
                    );
                    painter.draw_sized(
                        &mut tile,
                        px(12.0),
                        px(y),
                        &label,
                        font_size,
                        0.0,
                        color.to_argb_u32(),
                    );
                }
                let mut context = SpecimenPaintContext {
                    masks: &mut self.masks,
                    theme,
                    scale,
                    size: row_size,
                };
                paint_specimen(
                    &mut tile,
                    &mut painter,
                    &mut context,
                    row.specimen,
                    row.preview,
                );
                painter.draw_sized(
                    &mut tile,
                    row.preview.x as usize,
                    (row.preview.y + row.preview.height) as usize + px(8.0),
                    "STATIC STATE",
                    (10.0 * scale) as f32,
                    0.0,
                    colors.text_dim.to_argb_u32(),
                );
                tile.fill_rect_px(
                    px(12.0),
                    row_size.1.saturating_sub(px(6.0)),
                    row_size.0.saturating_sub(px(24.0)),
                    1,
                    colors.hairline.to_argb_u32(),
                );
            }
            // Copy only visible scanlines. set_pixel honors the gallery clip.
            let first = (layout.viewport.y - rect.y).max(0.0).ceil() as usize;
            let last = ((layout.viewport.y + layout.viewport.height - rect.y)
                .max(0.0)
                .ceil() as usize)
                .min(row_size.1);
            for y in first..last {
                for x in 0..row_size.0 {
                    frame.set_pixel(
                        rect.x as usize + x,
                        (rect.y + y as f32).round() as usize,
                        self.row_buffer[y * row_size.0 + x],
                    );
                }
            }
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
    let mut opts = if content.constraints.allow_multiline {
        TextFieldOptions::for_text_area(
            content,
            Rect::new(
                inner.x as f32,
                inner.y as f32,
                inner.w as f32,
                inner.h as f32,
            ),
            painter.line_height(),
            painter.char_width(),
        )
    } else {
        TextFieldOptions::for_text_box(content, &inner, painter.line_height(), painter.char_width())
    };
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
    context: &mut SpecimenPaintContext<'_>,
    spec: &Specimen,
    rect: Rect,
) {
    let theme = context.theme;
    let scale = context.scale;
    let size = context.size;
    let masks = &mut *context.masks;
    use crate::model::gallery::Preview;
    match spec.preview {
        Preview::ButtonNormal
        | Preview::ButtonHovered
        | Preview::ButtonPressed
        | Preview::ButtonFocused
        | Preview::ButtonSelected
        | Preview::ButtonDisabled
        | Preview::ButtonLongLabel
        | Preview::IconButton => {
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
                } else if matches!(spec.preview, Preview::IconButton) {
                    "×"
                } else {
                    "Configure server"
                },
                ButtonStyle {
                    state,
                    focused: matches!(spec.preview, Preview::ButtonFocused),
                    text_size: Some((11.0 * scale) as f32),
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
        Preview::FieldMultiline => {
            let mut content = EditableState::new(
                StringBuffer::from_text("{\n  \"check\": true,\n  \"command\": \"clippy\"\n}"),
                EditConstraints::editor(),
            );
            content.set_cursor_position(Position::new(0, 0), false);
            paint_field(frame, painter, theme, rect, &content, true);
        }
        Preview::SearchField => {
            let rows = [Row {
                icon: RowIcon::None,
                label: "serde_json::Value",
                match_indices: &[0, 1, 2, 3, 4],
                detail: Some("crate"),
                detail_style: None,
                accessory: Accessory::None,
            }];
            let sections = [Section {
                title: None,
                rows: &rows,
            }];
            let overlay = OverlaySpec {
                tabs: None,
                anchor: menu_anchor(rect, scale),
                header: Some(Header {
                    glyph: Some('\u{276F}'),
                    text: "serde",
                    placeholder: "Search",
                    caret: Some(5),
                    selection: None,
                    scope: None,
                }),
                body: Body::List {
                    sections: &sections,
                    selected: FlatIndex(0),
                    scroll: 0,
                    max_visible: 1,
                },
                footer: None,
                hover_row: None,
                docs: None,
            };
            render_overlay(frame, painter, masks, theme, &overlay, size, scale);
        }
        Preview::FormValidation => {
            let fields = [Field {
                label: "Workspace folder",
                trailing: Some("Path must be absolute"),
                trailing_is_error: true,
            }];
            let overlay = OverlaySpec {
                tabs: None,
                anchor: menu_anchor(rect, scale),
                header: None,
                body: Body::Fields {
                    fields: &fields,
                    focused: 0,
                },
                footer: None,
                hover_row: None,
                docs: None,
            };
            render_overlay(frame, painter, masks, theme, &overlay, size, scale);
            if let Some(field) = overlay_surface::layout_measured(
                &overlay,
                size.0,
                size.1,
                scale,
                &mut crate::layout::PainterMeasure::new(painter),
            )
            .fields
            .first()
            {
                let content = EditableState::new(
                    StringBuffer::from_text("relative/path"),
                    EditConstraints::single_line(),
                );
                paint_field(
                    frame,
                    painter,
                    theme,
                    Rect::new(
                        field.input.x as f32,
                        field.input.y as f32,
                        field.input.w as f32,
                        field.input.h as f32,
                    ),
                    &content,
                    true,
                );
            }
        }
        Preview::Checkbox(checked) => {
            let mut r = widget(rect);
            r.w = (14.0 * scale) as usize;
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
        Preview::SelectOptions => {
            let mut anchor = widget(rect);
            anchor.h = (29.0 * scale) as usize;
            render_select(frame, painter, theme, anchor, "On focus loss", true, scale);
            for (index, option) in ["On focus loss", "On window change", "Never"]
                .iter()
                .zip(select_option_rects(
                    anchor,
                    3,
                    anchor.y + anchor.h,
                    widget(rect).y + widget(rect).h,
                    scale,
                ))
                .enumerate()
            {
                render_button(
                    frame,
                    painter,
                    theme,
                    Rect::new(
                        option.1.x as f32,
                        option.1.y as f32,
                        option.1.w as f32,
                        option.1.h as f32,
                    ),
                    option.0,
                    ButtonStyle {
                        state: if index == 1 {
                            ButtonState::Hovered
                        } else {
                            ButtonState::Normal
                        },
                        text_size: Some((11.0 * scale) as f32),
                        ..Default::default()
                    },
                );
            }
        }
        Preview::ChoiceGroup => {
            let labels = ["Automatic", "On", "Off"];
            for (index, choice) in choice_group_rects(widget(rect), &labels, scale)
                .into_iter()
                .enumerate()
            {
                render_button(
                    frame,
                    painter,
                    theme,
                    Rect::new(
                        choice.x as f32,
                        choice.y as f32,
                        choice.w as f32,
                        choice.h as f32,
                    ),
                    labels[index],
                    ButtonStyle {
                        state: if index == 0 {
                            ButtonState::Selected
                        } else {
                            ButtonState::Normal
                        },
                        text_size: Some((11.0 * scale) as f32),
                        ..Default::default()
                    },
                );
            }
        }
        Preview::Disclosure { expanded } => {
            let mut label = widget(rect);
            label.y += (8.0 * scale) as usize;
            render_disclosure(frame, painter, theme, label, expanded, scale);
        }
        Preview::MenuRows { hover } => {
            let keycaps = overlay_surface::binding_chips("⌘K ⌘C");
            let first = [Row {
                icon: RowIcon::None,
                label: "Format Document",
                match_indices: &[],
                detail: None,
                detail_style: None,
                accessory: Accessory::Keycaps(&keycaps),
            }];
            let second = [Row {
                icon: RowIcon::None,
                label: "Rename Symbol",
                match_indices: &[],
                detail: None,
                detail_style: None,
                accessory: Accessory::None,
            }];
            let sections = [
                Section {
                    title: None,
                    rows: &first,
                },
                Section {
                    title: None,
                    rows: &second,
                },
            ];
            let overlay = OverlaySpec {
                tabs: None,
                anchor: menu_anchor(rect, scale),
                header: None,
                body: Body::List {
                    sections: &sections,
                    selected: FlatIndex(0),
                    scroll: 0,
                    max_visible: 3,
                },
                footer: None,
                hover_row: hover.then_some(FlatIndex(1)),
                docs: None,
            };
            render_overlay(frame, painter, masks, theme, &overlay, size, scale);
        }
        Preview::ListRow => {
            let rows = [Row {
                icon: RowIcon::KindBadge(MenuItemKind::Method),
                label: "render_component",
                match_indices: &[0, 1, 2, 3, 4, 5],
                detail: Some("fn(&Theme)"),
                detail_style: None,
                accessory: Accessory::None,
            }];
            let sections = [Section {
                title: Some("Completions"),
                rows: &rows,
            }];
            let overlay = OverlaySpec {
                tabs: None,
                anchor: menu_anchor(rect, scale),
                header: None,
                body: Body::List {
                    sections: &sections,
                    selected: FlatIndex(0),
                    scroll: 0,
                    max_visible: 2,
                },
                footer: None,
                hover_row: None,
                docs: None,
            };
            render_overlay(frame, painter, masks, theme, &overlay, size, scale);
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

fn menu_anchor(rect: Rect, scale: f64) -> Anchor<'static> {
    Anchor::Menu {
        x: rect.x.max(0.0) as usize,
        y: rect.y.max(0.0) as usize,
        h: 0,
        prefer_below: true,
        width: WidthRule {
            pct: 0.0,
            min: (rect.width / scale as f32).max(160.0),
            max: (rect.width / scale as f32).max(160.0),
        },
    }
}

fn render_overlay(
    frame: &mut Frame,
    painter: &mut TextPainter,
    masks: &mut RoundedRectMaskCache,
    theme: &Theme,
    spec: &OverlaySpec,
    size: (usize, usize),
    scale: f64,
) {
    overlay_surface::render(
        frame, painter, masks, theme, spec, size.0, size.1, scale, true,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn specimen_constraints_survive_width_scale_and_scroll_changes() {
        for scale in [1.0, 1.5, 2.0] {
            for compact in [false, true] {
                for width in [900, 1720] {
                    let state = GalleryState {
                        compact,
                        ..Default::default()
                    };
                    let layout = GalleryLayout::new(
                        (width as f64 * scale) as usize,
                        (800.0 * scale) as usize,
                        scale,
                        &state,
                    );
                    let scrolled = GalleryLayout::new(
                        (width as f64 * scale) as usize,
                        (800.0 * scale) as usize,
                        scale,
                        &GalleryState {
                            scroll: 179.5,
                            compact,
                            ..Default::default()
                        },
                    );
                    for (row, shifted) in layout.rows.iter().zip(&scrolled.rows) {
                        assert_eq!(
                            (
                                row.preview.x,
                                row.preview.y,
                                row.preview.width,
                                row.preview.height
                            ),
                            (
                                shifted.preview.x,
                                shifted.preview.y,
                                shifted.preview.width,
                                shifted.preview.height
                            ),
                        );
                        assert!(row.preview.x + row.preview.width <= row.rect.width);
                        assert!(
                            row.preview.y + row.preview.height + 24.0 * scale as f32
                                <= row.rect.height
                        );
                        match row.specimen.preview {
                            Preview::IconButton => {
                                assert_eq!(row.preview.width, row.preview.height)
                            }
                            Preview::FieldFocused
                            | Preview::FieldUnfocused
                            | Preview::FieldSelection => {
                                assert_eq!(row.preview.height, 34.0 * scale as f32)
                            }
                            Preview::ChoiceGroup => {
                                let choices = choice_group_rects(
                                    widget(row.preview),
                                    &["Automatic", "On", "Off"],
                                    scale,
                                );
                                assert!(choices.iter().all(|choice| (choice.y + choice.h) as f32
                                    <= row.preview.y + row.preview.height));
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn gallery_filters_and_scroll_geometry_share_visible_rows() {
        let mut state = GalleryState {
            category: 1,
            scroll: 100_000.0,
            ..Default::default()
        };
        let layout = GalleryLayout::new(1000, 600, 1.0, &state);
        assert_eq!(layout.rows.len(), 8);
        assert_eq!(
            layout.scrollbar.state.position,
            layout.scrollbar.state.max_position()
        );
        let last = layout.rows.last().unwrap().rect;
        assert!((last.y + last.height - layout.viewport.y - layout.viewport.height).abs() < 1.0);
        state.query.insert_text("disabled");
        assert_eq!(state.specimens()[0].id, "button.disabled");
        state.query.insert_text("no match");
        assert!(GalleryLayout::new(1000, 600, 1.0, &state).rows.is_empty());
    }
}
