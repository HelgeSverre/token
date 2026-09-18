//! Native component specimens, using the same painters as application controls.
use super::button::{render_button, ButtonState, ButtonStyle};
use super::controls::{
    choice_group_rects, render_checkbox, render_disclosure, render_field_surface, render_select,
    select_option_rects,
};
use super::frame::RoundedRectMaskCache;
use super::geometry::WidgetRect;
use super::overlay_surface::{
    self, Accessory, Anchor, Body, ChoicePresentation, Documentation, Field, FlatIndex, Header,
    OverlaySpec, Row, RowIcon, Section, WidthRule, Zones,
};
use super::scrollbar::{render_scrollbar, ScrollbarColors, ScrollbarGeometry, ScrollbarState};
use super::{FontRole, Frame, GlyphCache, TextFieldOptions, TextFieldRenderer, TextPainter};
use crate::completion::menu::MenuItemKind;
use crate::editable::{EditConstraints, EditableState, Position, StringBuffer};
use crate::model::gallery::{GalleryState, Preview, Specimen, CATEGORIES};
use crate::model::{Rect, Span, SpanStyle, StyledText};
use crate::theme::Theme;

/// One layout snapshot drives drawing, wheel limits, and pointer handling.
pub struct GalleryLayout {
    pub search: Rect,
    pub theme: Rect,
    pub width_toggle: Rect,
    pub width_segments: Vec<WidgetRect>,
    pub theme_popup: Option<super::select::SelectLayout>,
    pub categories: Vec<WidgetRect>,
    pub viewport: Rect,
    pub scrollbar: ScrollbarGeometry,
    pub rows: Vec<GalleryRow>,
}

pub struct GalleryRow {
    pub specimen: &'static Specimen,
    pub rect: Rect,
    /// Bordered specimen canvas, local to the row and independent of scrolling.
    pub preview: Rect,
    /// Padded content bounds inside the specimen canvas.
    pub content: Rect,
}

const PREVIEW_PAD: f32 = 24.0;
const STACKED_PREVIEW_Y: f32 = 96.0;
const NORMAL_ROW_EXTRA: f32 = 64.0;
const STACKED_ROW_EXTRA: f32 = 144.0;
const DIVIDER_BOTTOM_INSET: f32 = 16.0;

fn preview_pad_x(preview: Preview) -> f32 {
    if matches!(
        preview,
        Preview::SettingsForm
            | Preview::SettingsRecords(_)
            | Preview::SearchCollection(_)
            | Preview::CompletionDocumentation
            | Preview::Editor(_)
    ) {
        48.0
    } else {
        PREVIEW_PAD
    }
}

fn metadata_above(preview: Preview) -> bool {
    matches!(
        preview,
        Preview::SettingsForm
            | Preview::SettingsRecords(_)
            | Preview::SearchCollection(_)
            | Preview::CompletionDocumentation
            | Preview::Editor(_)
    )
}

fn row_extra(preview: Preview) -> f32 {
    if metadata_above(preview) {
        STACKED_ROW_EXTRA
    } else {
        NORMAL_ROW_EXTRA
    }
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
        Preview::SelectOptions => (field_width, 171.0),
        Preview::ChoiceGroup => (field_width, if compact { 74.0 } else { 48.0 }),
        Preview::Disclosure { .. } => (field_width, 22.0),
        Preview::SearchField => (popup_width, 184.0),
        Preview::ListRow => (popup_width, 176.0),
        Preview::FormValidation => (popup_width, 96.0),
        Preview::SettingsForm => (if compact { 400.0 } else { 580.0 }, 600.0),
        Preview::SettingsRecords(_) => (if compact { 500.0 } else { 580.0 }, 600.0),
        Preview::SearchCollection(crate::model::gallery::SearchCollectionPreview::Grouped) => {
            (if compact { 500.0 } else { 580.0 }, 420.0)
        }
        Preview::SearchCollection(
            crate::model::gallery::SearchCollectionPreview::Loading
            | crate::model::gallery::SearchCollectionPreview::Empty,
        ) => (if compact { 500.0 } else { 580.0 }, 220.0),
        Preview::CompletionDocumentation => (if compact { 500.0 } else { 580.0 }, 320.0),
        Preview::HoverDocumentation | Preview::SignatureHelp => (popup_width, 150.0),
        Preview::MenuRows { .. } => (popup_width, 168.0),
        Preview::Chrome(
            crate::model::gallery::ChromePreview::BottomPanel
            | crate::model::gallery::ChromePreview::RightPanel,
        ) => (popup_width, 140.0),
        Preview::Chrome(
            crate::model::gallery::ChromePreview::ProblemsPopulated
            | crate::model::gallery::ChromePreview::UsagesPopulated,
        ) => (popup_width, 300.0),
        Preview::Chrome(crate::model::gallery::ChromePreview::TerminalContent) => {
            (popup_width, 180.0)
        }
        Preview::Chrome(crate::model::gallery::ChromePreview::DocumentDrag) => (popup_width, 48.0),
        Preview::Chrome(_) => (popup_width, 32.0),
        Preview::Editor(_) => (if compact { 500.0 } else { 580.0 }, 240.0),
        Preview::OverlayTabs => (popup_width, 56.0),
        Preview::Scrollbar {
            horizontal: true, ..
        } => (popup_width, 110.0),
        Preview::Scrollbar {
            horizontal: false, ..
        } => (popup_width, 160.0),
        Preview::Splitter { .. } => (field_width, 72.0),
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
            .map(|spec| {
                let content_h = specimen_size(spec.preview, state.compact).1;
                (content_h + PREVIEW_PAD * 2.0 + row_extra(spec.preview)).max(112.0) * s
            })
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
                let stacked = metadata_above(spec.preview);
                let pad_x = preview_pad_x(spec.preview);
                let canvas_w = control_w + pad_x * 2.0;
                let canvas_h = control_h + PREVIEW_PAD * 2.0;
                let height = (canvas_h + row_extra(spec.preview)).max(112.0) * s;
                let rect = Rect::new(viewport.x, row_y, viewport.width, height);
                row_y += height;
                let max_preview_w = if state.compact { 284.0 } else { 424.0 };
                let metadata_w = (viewport.width / s - max_preview_w - 48.0).clamp(180.0, 420.0);
                let preview_x = if stacked {
                    ((viewport.width / s - canvas_w) / 2.0).max(12.0)
                } else {
                    (metadata_w + 32.0)
                        .min(viewport.width / s - canvas_w - 12.0)
                        .max(12.0)
                };
                let preview = Rect::new(
                    preview_x * s,
                    if stacked { STACKED_PREVIEW_Y } else { 16.0 } * s,
                    canvas_w * s,
                    canvas_h * s,
                );
                GalleryRow {
                    specimen: spec,
                    rect,
                    preview,
                    content: Rect::new(
                        preview.x + pad_x * s,
                        preview.y + PREVIEW_PAD * s,
                        control_w * s,
                        control_h * s,
                    ),
                }
            })
            .collect();
        Self {
            search: Rect::new(24.0 * s, 76.0 * s, (w - 468.0 * s).max(80.0 * s), 29.0 * s),
            theme: Rect::new(w - 424.0 * s, 76.0 * s, 220.0 * s, 29.0 * s),
            width_toggle: Rect::new(w - 188.0 * s, 76.0 * s, 164.0 * s, 29.0 * s),
            width_segments: super::segmented_control::segment_rects(
                Rect::new(w - 188.0 * s, 76.0 * s, 164.0 * s, 29.0 * s),
                2,
            ),
            theme_popup: None,
            categories: super::section_navigation::section_rects(
                WidgetRect {
                    x: (16.0 * s) as usize,
                    y: (148.0 * s) as usize,
                    w: (154.0 * s) as usize,
                    h: 0,
                },
                CATEGORIES.len(),
                1,
                scale,
            ),
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
    specimen_buffer: Vec<u32>,
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
            specimen_buffer: Vec::new(),
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
        let mut layout = GalleryLayout::new(size.0, size.1, scale, state);
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
            state.focus == crate::model::gallery::GalleryFocus::Filter,
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
        let theme_labels: Vec<_> = state.theme_names.iter().map(String::as_str).collect();
        let theme_select = super::select::Select {
            anchor: layout.theme,
            labels: &theme_labels,
            selected: state.selected_theme,
            state: &state.theme_select,
            focused: state.focus == crate::model::gallery::GalleryFocus::Theme,
            scale,
        };
        for (rect, label) in [
            (layout.theme, "Theme"),
            (layout.width_toggle, "Preview width"),
        ] {
            painter.draw_sized(
                &mut frame,
                rect.x as usize,
                px(58.0),
                label,
                (11.0 * scale) as f32,
                0.0,
                colors.text_dim.to_argb_u32(),
            );
        }
        theme_select.render_anchor(&mut frame, &mut painter, theme);
        super::segmented_control::SegmentedControl {
            segments: &layout.width_segments,
            labels: &["Narrow", "Wide"],
            selected: usize::from(!state.compact),
            focused: state.focus == crate::model::gallery::GalleryFocus::Width,
            scale,
        }
        .render(&mut frame, &mut painter, theme);
        super::section_navigation::SectionNavigation {
            rows: &layout.categories,
            divider: Some(WidgetRect {
                x: px(180.0),
                y: px(144.0),
                w: px(1.0),
                h: layout.viewport.height as usize,
            }),
            selected: state.category,
            scale,
        }
        .render(
            &mut frame,
            &mut painter,
            &mut self.masks,
            theme,
            CATEGORIES.iter().copied(),
        );
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
            let (preview_x, preview_y, preview_w, preview_h) =
                crate::layout::snapshot::snap(row.preview);
            self.specimen_buffer.resize(preview_w * preview_h, 0);
            {
                // Local rendering keeps popup anchoring and unsigned paint
                // coordinates independent of the gallery's scroll position.
                let mut tile = Frame::new(&mut self.row_buffer, row_size.0, row_size.1);
                tile.clear(colors.panel_background.to_argb_u32());
                let metadata_width = if metadata_above(row.specimen.preview) {
                    row_size.0.saturating_sub(px(24.0))
                } else {
                    row.preview.x as usize - px(32.0)
                };
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
                    size: (preview_w, preview_h),
                };
                let canvas_bg = darken_argb(colors.panel_background.to_argb_u32());
                {
                    let mut specimen = Frame::new(&mut self.specimen_buffer, preview_w, preview_h);
                    specimen.clear(canvas_bg);
                    let local_content = Rect::new(
                        row.content.x - row.preview.x,
                        row.content.y - row.preview.y,
                        row.content.width,
                        row.content.height,
                    );
                    paint_specimen(
                        &mut specimen,
                        &mut painter,
                        &mut context,
                        row.specimen,
                        local_content,
                    );
                }
                for y in 0..preview_h {
                    for x in 0..preview_w {
                        tile.set_pixel(
                            preview_x + x,
                            preview_y + y,
                            self.specimen_buffer[y * preview_w + x],
                        );
                    }
                }
                let stroke = ((2.0 * scale).round().max(2.0)) as usize;
                let dash = (4.0 * scale).round().max(2.0) as usize;
                let step = (8.0 * scale).round().max(4.0) as usize;
                tile.stroke_dotted_rect(
                    row.preview,
                    stroke,
                    dash,
                    step,
                    colors.hairline.to_argb_u32(),
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
                    row_size.1.saturating_sub(px(DIVIDER_BOTTOM_INSET as f64)),
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
        layout.theme_popup =
            theme_select.render_popup(&mut frame, &mut painter, &mut self.masks, theme, size);
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

fn darken_argb(color: u32) -> u32 {
    let channel = |shift| ((color >> shift) & 0xff_u32) * 82_u32 / 100_u32;
    (color & 0xff00_0000) | (channel(16) << 16) | (channel(8) << 8) | channel(0)
}

fn gallery_modal_model(
    theme: &Theme,
    painter: &TextPainter,
    size: (usize, usize),
    scale: f64,
) -> crate::model::AppModel {
    let mut model = crate::model::AppModel::new(size.0 as u32, size.1 as u32, scale);
    model.theme = theme.clone();
    model.line_height = painter.line_height();
    model.char_width = painter.char_width();
    model.recompute_tab_bar_height_from_line_height();
    model
}

fn paint_search_collection(
    frame: &mut Frame,
    painter: &mut TextPainter,
    masks: &mut RoundedRectMaskCache,
    theme: &Theme,
    size: (usize, usize),
    scale: f64,
    preview: crate::model::gallery::SearchCollectionPreview,
) {
    use crate::model::gallery::SearchCollectionPreview;
    use crate::model::{FileFinderState, ModalState, SearchTab};
    use std::path::{Path, PathBuf};

    let mut model = gallery_modal_model(theme, painter, size, scale);
    let mut state = crate::model::ui::CommandPaletteState::default();
    state.set_input(match preview {
        SearchCollectionPreview::Grouped => "render",
        SearchCollectionPreview::Loading => "workspace",
        SearchCollectionPreview::Empty => "definitely-no-match",
    });
    match preview {
        SearchCollectionPreview::Grouped => {
            state.active_tab = SearchTab::All;
            state.matches.truncate(4);
            state.files_available = true;
            let root = PathBuf::from("/workspace/token");
            let mut files = FileFinderState::new(Vec::new(), root.clone());
            files.results = [
                "src/view/gallery.rs",
                "src/view/overlay_surface.rs",
                "src/view/panels.rs",
                "src/model/ui.rs",
                "docs/ui/LIST.md",
            ]
            .iter()
            .enumerate()
            .map(|(index, relative)| {
                crate::model::FileMatch::from_path(
                    &root.join(relative),
                    Path::new(&root),
                    100 - index as u32,
                    Vec::new(),
                )
            })
            .collect();
            files.selected_index = 1;
            state.files = Some(files);
            state.all_selected = 2;
        }
        SearchCollectionPreview::Loading => {
            state.active_tab = SearchTab::Symbols;
            state.files_available = true;
            state.symbols.available = true;
            state.symbols.searching = true;
        }
        SearchCollectionPreview::Empty => {
            state.active_tab = SearchTab::All;
            state.matches.clear();
            state.files_available = true;
            state.files = Some(FileFinderState::new(
                Vec::new(),
                PathBuf::from("/workspace/token"),
            ));
            state.symbols.available = true;
        }
    }
    model.ui.active_modal = Some(ModalState::CommandPalette(state));
    super::modal::render_modals(frame, painter, &model, size.0, size.1, masks);
}

fn paint_settings_records(
    frame: &mut Frame,
    painter: &mut TextPainter,
    masks: &mut RoundedRectMaskCache,
    theme: &Theme,
    size: (usize, usize),
    scale: f64,
    preview: crate::model::gallery::SettingsRecordsPreview,
) {
    use crate::model::gallery::SettingsRecordsPreview;
    use crate::model::ModalState;

    let mut model = gallery_modal_model(theme, painter, size, scale);
    if matches!(preview, SettingsRecordsPreview::Empty) {
        model.config.lsp.servers.clear();
    }
    let selected = match preview {
        SettingsRecordsPreview::Selected => Some("rust-analyzer"),
        SettingsRecordsPreview::Empty => None,
    };
    let mut state = crate::settings::SettingsState::new(&model.config);
    state.form = Some(crate::settings::forms::SettingsForm::language_server(
        selected,
        &model.config,
    ));
    state.refresh_entries(&model.config);
    model.ui.active_modal = Some(ModalState::Settings(state));
    super::modal::render_modals(frame, painter, &model, size.0, size.1, masks);
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
        Preview::SearchCollection(preview) => {
            paint_search_collection(frame, painter, masks, theme, size, scale, preview)
        }
        Preview::SettingsRecords(preview) => {
            paint_settings_records(frame, painter, masks, theme, size, scale, preview)
        }
        Preview::Chrome(kind) => {
            super::gallery_chrome::render(frame, painter, theme, rect, scale, kind)
        }
        Preview::Editor(kind) => {
            super::gallery_editor::render(frame, painter, theme, rect, scale, kind)
        }
        Preview::OverlayTabs => {
            let tabs = [
                ("Files", overlay_surface::TabCount::N(12)),
                ("Commands", overlay_surface::TabCount::Pending),
                ("Symbols", overlay_surface::TabCount::Unavailable),
            ];
            let overlay = OverlaySpec {
                tabs: Some(overlay_surface::TabBar {
                    tabs: &tabs,
                    active: 0,
                }),
                anchor: menu_anchor(rect, scale),
                header: None,
                body: Body::List {
                    sections: &[],
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
        Preview::Scrollbar {
            horizontal,
            hovered,
            fits,
            end,
        } => {
            let surface = widget(rect);
            frame.fill_rounded_rect(
                surface.x,
                surface.y,
                surface.w,
                surface.h,
                (4.0 * scale) as usize,
                theme.overlay.panel_secondary.to_argb_u32(),
                masks,
            );
            let inset = (10.0 * scale) as usize;
            let scrollbar_size = (12.0 * scale) as usize;
            let viewport = WidgetRect {
                x: surface.x + inset,
                y: surface.y + inset,
                w: surface.w.saturating_sub(inset * 2),
                h: surface.h.saturating_sub(inset * 2),
            };
            frame.push_clip(Rect::new(
                viewport.x as f32,
                viewport.y as f32,
                viewport.w as f32,
                viewport.h as f32,
            ));
            let scroll_shift = if end { 42.0 } else { 12.0 };
            for (index, label) in [
                "Overview",
                "Editor settings",
                "Language services",
                "Appearance",
                "Key bindings",
                "Advanced",
            ]
            .iter()
            .enumerate()
            {
                let y = viewport.y as f32
                    + (index as f32 * 26.0 - if fits { 0.0 } else { scroll_shift }) * scale as f32;
                painter.draw_sized(
                    frame,
                    viewport.x + (8.0 * scale) as usize,
                    y.max(0.0) as usize,
                    label,
                    (11.0 * scale) as f32,
                    0.0,
                    theme.overlay.text_primary.to_argb_u32(),
                );
                frame.fill_rect_px(
                    viewport.x + (8.0 * scale) as usize,
                    (y + 19.0 * scale as f32).max(0.0) as usize,
                    viewport.w.saturating_sub((28.0 * scale) as usize),
                    1,
                    theme.overlay.hairline.to_argb_u32(),
                );
            }
            frame.pop_clip();

            let (total, visible, offset) = if fits {
                (100, 100, 0)
            } else if end {
                (400, 100, 300)
            } else {
                (400, 100, 90)
            };
            let state = ScrollbarState::new(total, visible, offset);
            let geometry = if horizontal {
                ScrollbarGeometry::horizontal(
                    Rect::new(
                        viewport.x as f32,
                        (viewport.y + viewport.h.saturating_sub(scrollbar_size)) as f32,
                        viewport.w as f32,
                        scrollbar_size as f32,
                    ),
                    &state,
                )
            } else {
                ScrollbarGeometry::vertical(
                    Rect::new(
                        (viewport.x + viewport.w.saturating_sub(scrollbar_size)) as f32,
                        viewport.y as f32,
                        scrollbar_size as f32,
                        viewport.h as f32,
                    ),
                    &state,
                )
            };
            render_scrollbar(
                frame,
                &geometry,
                hovered,
                &ScrollbarColors::from(&theme.scrollbar),
            );
        }
        Preview::Splitter { horizontal } => {
            frame.fill_rect(rect, theme.editor.background.to_argb_u32());
            let mut model = crate::model::AppModel::new(1, 1, scale);
            model.theme = theme.clone();
            let boundary = if horizontal {
                Rect::new(
                    rect.x,
                    rect.y + rect.height / 2.0,
                    rect.width,
                    model.metrics.splitter_width,
                )
            } else {
                Rect::new(
                    rect.x + rect.width / 2.0,
                    rect.y,
                    model.metrics.splitter_width,
                    rect.height,
                )
            };
            super::Renderer::render_splitters(
                frame,
                &[crate::model::editor_area::SplitterBar {
                    direction: if horizontal {
                        crate::model::editor_area::SplitDirection::Horizontal
                    } else {
                        crate::model::editor_area::SplitDirection::Vertical
                    },
                    rect: boundary,
                    index: 0,
                }],
                &model,
            );
        }
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
            let rows = [
                ("serde_json::Value", "crate"),
                ("serde_json::Map", "crate"),
                ("serde::Serialize", "dependency"),
                ("serialize_document", "src/document.rs"),
                ("deserialize_config", "src/config.rs"),
            ]
            .map(|(label, detail)| Row {
                icon: RowIcon::None,
                label,
                match_indices: &[0, 1, 2, 3, 4],
                detail: Some(detail),
                detail_style: None,
                accessory: Accessory::None,
            });
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
                    max_visible: rows.len(),
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
        Preview::SettingsForm => {
            let rows = [
                Row {
                    icon: RowIcon::None,
                    label: "Code completion",
                    match_indices: &[],
                    detail: Some("Enable completion requests"),
                    detail_style: None,
                    accessory: Accessory::Choices {
                        labels: &["Disabled", "Enabled"],
                        active: Some(1),
                        presentation: ChoicePresentation::Checkbox,
                    },
                },
                Row {
                    icon: RowIcon::None,
                    label: "Auto-save",
                    match_indices: &[],
                    detail: Some("When modified files are saved"),
                    detail_style: None,
                    accessory: Accessory::Choices {
                        labels: &["Off", "Focus", "Idle", "Both"],
                        active: Some(1),
                        presentation: ChoicePresentation::Select,
                    },
                },
                Row {
                    icon: RowIcon::None,
                    label: "Advanced settings",
                    match_indices: &[],
                    detail: None,
                    detail_style: None,
                    accessory: Accessory::Choices {
                        labels: &["Reveal"],
                        active: None,
                        presentation: ChoicePresentation::Disclosure,
                    },
                },
                Row {
                    icon: RowIcon::None,
                    label: "Cursor blink",
                    match_indices: &[],
                    detail: Some("Caret animation speed"),
                    detail_style: None,
                    accessory: Accessory::Choices {
                        labels: &["Off", "Slow", "Fast"],
                        active: Some(2),
                        presentation: ChoicePresentation::Buttons,
                    },
                },
                Row {
                    icon: RowIcon::None,
                    label: "Indent guides",
                    match_indices: &[],
                    detail: Some("Show vertical indentation lines"),
                    detail_style: None,
                    accessory: Accessory::Choices {
                        labels: &["Hidden", "Visible"],
                        active: Some(1),
                        presentation: ChoicePresentation::Checkbox,
                    },
                },
            ];
            let sections = [Section {
                title: Some("Editor"),
                rows: &rows,
            }];
            let overlay = OverlaySpec {
                tabs: None,
                anchor: Anchor::Settings {
                    width: WidthRule {
                        pct: 0.0,
                        min: rect.width / scale as f32,
                        max: rect.width / scale as f32,
                    },
                    subpage: true,
                    hovered_choice: None,
                    actions_row: None,
                    collection: None,
                },
                header: None,
                body: Body::List {
                    sections: &sections,
                    selected: FlatIndex(1),
                    scroll: 0,
                    max_visible: rows.len(),
                },
                footer: None,
                hover_row: None,
                docs: None,
            };
            render_overlay(frame, painter, masks, theme, &overlay, size, scale);
        }
        Preview::CompletionDocumentation => {
            let rows = [
                Row {
                    icon: RowIcon::KindBadge(MenuItemKind::Method),
                    label: "render_component",
                    match_indices: &[0, 1, 2, 3, 4, 5],
                    detail: Some("fn(&Theme) -> Frame"),
                    detail_style: Some(SpanStyle::Code),
                    accessory: Accessory::None,
                },
                Row {
                    icon: RowIcon::KindBadge(MenuItemKind::Function),
                    label: "render_overlay",
                    match_indices: &[0, 1, 2, 3, 4, 5],
                    detail: None,
                    detail_style: None,
                    accessory: Accessory::None,
                },
                Row {
                    icon: RowIcon::KindBadge(MenuItemKind::Function),
                    label: "render_gallery",
                    match_indices: &[0, 1, 2, 3, 4, 5],
                    detail: Some("fn(&GalleryState)"),
                    detail_style: Some(SpanStyle::Code),
                    accessory: Accessory::None,
                },
                Row {
                    icon: RowIcon::KindBadge(MenuItemKind::Field),
                    label: "render_cache",
                    match_indices: &[0, 1, 2, 3, 4, 5],
                    detail: Some("GlyphCache"),
                    detail_style: Some(SpanStyle::Code),
                    accessory: Accessory::None,
                },
                Row {
                    icon: RowIcon::KindBadge(MenuItemKind::Module),
                    label: "renderer",
                    match_indices: &[0, 1, 2, 3, 4, 5],
                    detail: Some("crate::view"),
                    detail_style: Some(SpanStyle::Code),
                    accessory: Accessory::None,
                },
            ];
            let sections = [Section {
                title: Some("Completions"),
                rows: &rows,
            }];
            let mut docs = StyledText::default();
            docs.push_styled(
                "fn render_component(theme: &Theme) -> Frame\n",
                SpanStyle::Code,
            );
            docs.push_str(
                "\nRenders a component with production layout, clipping, and theme roles.\n\n",
            );
            docs.push_str(
                "The returned frame uses the same geometry for painting and hit testing.",
            );
            let logical_width = rect.width / scale as f32;
            let menu_width = if logical_width <= 500.0 { 180.0 } else { 210.0 };
            let overlay = OverlaySpec {
                tabs: None,
                anchor: Anchor::Cursor {
                    x: (rect.x + rect.width - menu_width * scale as f32) as usize,
                    y: rect.y as usize,
                    h: (18.0 * scale) as usize,
                    prefer_below: true,
                    width: WidthRule {
                        pct: 0.0,
                        min: menu_width,
                        max: menu_width,
                    },
                },
                header: None,
                body: Body::List {
                    sections: &sections,
                    selected: FlatIndex(0),
                    scroll: 0,
                    max_visible: rows.len(),
                },
                footer: None,
                hover_row: None,
                docs: Some(Documentation::from(&docs)),
            };
            render_overlay(frame, painter, masks, theme, &overlay, size, scale);
        }
        Preview::HoverDocumentation | Preview::SignatureHelp => {
            let signature = if matches!(spec.preview, Preview::SignatureHelp) {
                "render(frame: &mut Frame, theme: &Theme)"
            } else {
                "pub fn render_component(theme: &Theme) -> Frame"
            };
            let active_start = signature.find("theme").unwrap_or(0);
            let code_spans = [Span {
                range: active_start..active_start + "theme".len(),
                style: SpanStyle::Accent,
            }];
            let zones = Zones {
                banner: matches!(spec.preview, Preview::HoverDocumentation).then_some((
                    overlay_surface::Severity::Info,
                    "Production renderer",
                    "Token",
                )),
                code: Some(signature),
                code_spans: &code_spans,
                text: Some(if matches!(spec.preview, Preview::SignatureHelp) {
                    "Theme values are resolved before painting. (1 of 2)"
                } else {
                    "Uses shared layout and painter paths. Inline code and prose retain their font roles."
                }),
                ..Default::default()
            };
            let overlay = OverlaySpec {
                tabs: None,
                anchor: Anchor::Cursor {
                    x: rect.x as usize,
                    y: rect.y as usize,
                    h: (18.0 * scale) as usize,
                    prefer_below: true,
                    width: WidthRule {
                        pct: 0.0,
                        min: (rect.width / scale as f32).max(240.0),
                        max: (rect.width / scale as f32).max(240.0),
                    },
                },
                header: None,
                body: Body::Zones(zones),
                footer: None,
                hover_row: None,
                docs: None,
            };
            render_overlay(frame, painter, masks, theme, &overlay, size, scale);
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
            let options = [
                "On focus loss",
                "On window change",
                "After idle delay",
                "On explicit save",
                "Never",
            ];
            let mut anchor = widget(rect);
            anchor.h = (29.0 * scale) as usize;
            render_select(frame, painter, theme, anchor, "On focus loss", true, scale);
            for (index, option) in options
                .iter()
                .zip(select_option_rects(
                    anchor,
                    options.len(),
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
            let first = [
                Row {
                    icon: RowIcon::None,
                    label: "Format Document",
                    match_indices: &[],
                    detail: None,
                    detail_style: None,
                    accessory: Accessory::Keycaps(&keycaps),
                },
                Row {
                    icon: RowIcon::None,
                    label: "Rename Symbol",
                    match_indices: &[],
                    detail: None,
                    detail_style: None,
                    accessory: Accessory::None,
                },
                Row {
                    icon: RowIcon::None,
                    label: "Go to Definition",
                    match_indices: &[],
                    detail: None,
                    detail_style: None,
                    accessory: Accessory::None,
                },
            ];
            let second = [
                Row {
                    icon: RowIcon::None,
                    label: "Copy Path",
                    match_indices: &[],
                    detail: None,
                    detail_style: None,
                    accessory: Accessory::None,
                },
                Row {
                    icon: RowIcon::None,
                    label: "Reveal in File Explorer",
                    match_indices: &[],
                    detail: None,
                    detail_style: None,
                    accessory: Accessory::None,
                },
            ];
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
                    max_visible: 6,
                },
                footer: None,
                hover_row: hover.then_some(FlatIndex(1)),
                docs: None,
            };
            render_overlay(frame, painter, masks, theme, &overlay, size, scale);
        }
        Preview::ListRow => {
            let rows = [
                (MenuItemKind::Method, "render_component", "fn(&Theme)"),
                (MenuItemKind::Function, "render_overlay", "fn(&OverlaySpec)"),
                (MenuItemKind::Field, "render_cache", "GlyphCache"),
                (MenuItemKind::Module, "renderer", "crate::view"),
                (MenuItemKind::Type, "RenderTarget", "struct"),
            ]
            .map(|(kind, label, detail)| Row {
                icon: RowIcon::KindBadge(kind),
                label,
                match_indices: &[0, 1, 2, 3, 4, 5],
                detail: Some(detail),
                detail_style: None,
                accessory: Accessory::None,
            });
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
                    max_visible: rows.len(),
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
    fn production_compositions_render_at_compact_width_and_hidpi() {
        let mut renderer = GalleryRenderer::new().unwrap();
        let theme = Theme::default_dark();
        for scale in [1.0, 2.0] {
            for spec in crate::model::gallery::SPECIMENS.iter().filter(|s| {
                matches!(
                    s.preview,
                    Preview::Chrome(_)
                        | Preview::Editor(_)
                        | Preview::SettingsRecords(_)
                        | Preview::SearchCollection(_)
                        | Preview::OverlayTabs
                        | Preview::Scrollbar { .. }
                        | Preview::Splitter { .. }
                )
            }) {
                let mut state = GalleryState {
                    compact: true,
                    ..Default::default()
                };
                state.query.insert_text(spec.id);
                let size = ((900.0 * scale) as usize, (900.0 * scale) as usize);
                let mut pixels = vec![0; size.0 * size.1];
                let layout = renderer.render(&mut pixels, size, scale, &state, &theme);
                let last_category = layout.categories.last().unwrap();
                assert!(last_category.y + last_category.h <= size.1 - (32.0 * scale) as usize);
                let row = layout
                    .rows
                    .iter()
                    .find(|row| row.specimen.id == spec.id)
                    .unwrap();
                let x = (row.rect.x + row.preview.x) as usize;
                let y = (row.rect.y + row.preview.y) as usize;
                let non_background = (y..y + row.preview.height as usize).any(|y| {
                    pixels[y * size.0 + x..y * size.0 + x + row.preview.width as usize]
                        .iter()
                        .any(|pixel| *pixel != theme.overlay.panel_background.to_argb_u32())
                });
                assert!(non_background, "blank specimen: {}", spec.id);
            }
        }
    }

    #[test]
    fn dropdown_reveals_last_option_and_segments_share_hit_geometry() {
        for scale in [1.0, 2.0] {
            let mut state = GalleryState {
                theme_names: (0..30).map(|i| format!("Theme {i}")).collect(),
                selected_theme: 29,
                ..Default::default()
            };
            state.theme_select.open(29);
            let mut renderer = GalleryRenderer::new().unwrap();
            let size = ((900.0 * scale) as usize, (440.0 * scale) as usize);
            let mut pixels = vec![0; size.0 * size.1];
            let layout = renderer.render(&mut pixels, size, scale, &state, &Theme::default_dark());
            let popup = layout.theme_popup.unwrap();
            let last = popup.rows.last().unwrap();
            assert_eq!(
                popup.option_at((last.x + last.w / 2) as f32, (last.y + last.h / 2) as f32),
                Some(29)
            );
            assert!(popup.panel.y + popup.panel.h <= size.1);
            for (index, rect) in layout.width_segments.iter().enumerate() {
                assert_eq!(
                    super::super::section_navigation::section_at(
                        &layout.width_segments,
                        (rect.x + rect.w / 2) as f32,
                        (rect.y + rect.h / 2) as f32
                    ),
                    Some(index)
                );
            }
        }
    }

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
                                assert_eq!(row.content.width, row.content.height)
                            }
                            Preview::FieldFocused
                            | Preview::FieldUnfocused
                            | Preview::FieldSelection => {
                                assert_eq!(row.content.height, 34.0 * scale as f32)
                            }
                            Preview::ChoiceGroup => {
                                let choices = choice_group_rects(
                                    widget(row.content),
                                    &["Automatic", "On", "Off"],
                                    scale,
                                );
                                assert!(choices.iter().all(|choice| (choice.y + choice.h) as f32
                                    <= row.content.y + row.content.height));
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
