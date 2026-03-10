//! View module - rendering code extracted from main.rs
//!
//! Contains the Renderer struct and all rendering-related functionality.

pub mod button;
pub mod editor_scrollbars;
pub mod editor_special_tabs;
pub mod editor_text;
pub mod frame;
pub mod geometry;
pub mod helpers;
pub mod hit_test;
pub mod modal;
pub mod panels;
pub mod scrollbar;
pub mod selectable_list;
pub mod text_field;
pub mod tree_view;

pub use button::{button_rect, render_button, ButtonState};
pub use frame::{Frame, TextPainter};
pub use helpers::get_tab_display_name;
pub use text_field::{TextFieldContent, TextFieldOptions, TextFieldRenderer};

// Re-export geometry helpers for backward compatibility
pub use geometry::{char_col_to_visual_col, expand_tabs_for_display};

// Re-export hit-test types and functions for use in runtime
#[allow(unused_imports)]
pub use hit_test::{
    hit_test_groups, hit_test_modal, hit_test_previews, hit_test_sidebar, hit_test_sidebar_resize,
    hit_test_splitters, hit_test_status_bar, hit_test_ui, EventResult, HitTarget, MouseEvent,
    Point,
};

use anyhow::Result;
use fontdue::{Font, FontSettings, LineMetrics, Metrics};
use softbuffer::Surface;
use std::collections::HashMap;
use std::num::NonZeroU32;
use std::rc::Rc;
use winit::window::Window;

use crate::commands::{Damage, DamageArea};
use crate::model::editor_area::{EditorGroup, GroupId, Rect, SplitterBar};

/// Check if the damage contains cursor lines (free function to avoid borrow issues)
fn has_cursor_lines_damage(damage: &Damage) -> bool {
    match damage {
        Damage::None | Damage::Full => false,
        Damage::Areas(areas) => areas
            .iter()
            .any(|a| matches!(a, DamageArea::CursorLines(_))),
    }
}
use crate::model::AppModel;

pub type GlyphCacheKey = (char, u32);

pub type GlyphCache = HashMap<GlyphCacheKey, (Metrics, Vec<u8>)>;

/// Controls how preview panes render their content
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewRenderMode {
    /// Only render pane chrome (header, borders); webview handles content
    WebviewChromeOnly,
    /// Render native markdown content (for headless/screenshot use)
    NativeMarkdown,
}

struct RenderPlan {
    window_width: usize,
    window_height: usize,
    window_layout: geometry::WindowLayout,
    splitters: Vec<SplitterBar>,
    effective_damage: Damage,
    render_editor: bool,
    render_status_bar: bool,
    cursor_lines_only: Option<Vec<usize>>,
    show_modal: bool,
    show_drop_overlay: bool,
    #[cfg(debug_assertions)]
    show_perf_overlay: bool,
    #[cfg(debug_assertions)]
    show_debug_overlay: bool,
}

impl RenderPlan {
    #[inline]
    fn uses_cursor_lines_fast_path(&self) -> bool {
        self.cursor_lines_only.is_some()
    }
}

struct RenderSession<'buffer, 'a> {
    frame: Frame<'buffer>,
    painter: TextPainter<'a>,
    model: &'a AppModel,
    plan: &'a RenderPlan,
}

impl<'buffer, 'a> RenderSession<'buffer, 'a> {
    #[allow(clippy::too_many_arguments)]
    fn new(
        buffer: &'buffer mut [u32],
        window_width: usize,
        window_height: usize,
        font: &'a Font,
        glyph_cache: &'a mut GlyphCache,
        font_size: f32,
        ascent: f32,
        char_width: f32,
        line_height: usize,
        model: &'a AppModel,
        plan: &'a RenderPlan,
    ) -> Self {
        Self {
            frame: Frame::new(buffer, window_width, window_height),
            painter: TextPainter::new(
                font,
                glyph_cache,
                font_size,
                ascent,
                char_width,
                line_height,
            ),
            model,
            plan,
        }
    }

    fn render_editor_area_phase(&mut self) {
        Renderer::render_editor_area(
            &mut self.frame,
            &mut self.painter,
            self.model,
            &self.plan.splitters,
        );
    }

    fn render_sidebar_phase(&mut self) {
        let Some(sidebar_rect) = self.plan.window_layout.sidebar_rect else {
            return;
        };

        Renderer::render_sidebar(
            &mut self.frame,
            &mut self.painter,
            self.model,
            sidebar_rect.width.round() as usize,
            sidebar_rect.height.round() as usize,
        );
    }

    fn render_right_dock_phase(&mut self) {
        let Some(dock_rect) = self.plan.window_layout.right_dock_rect else {
            return;
        };

        Renderer::render_dock(
            &mut self.frame,
            &mut self.painter,
            self.model,
            crate::panel::DockPosition::Right,
            dock_rect,
        );
    }

    fn render_bottom_dock_phase(&mut self) {
        let Some(dock_rect) = self.plan.window_layout.bottom_dock_rect else {
            return;
        };

        Renderer::render_dock(
            &mut self.frame,
            &mut self.painter,
            self.model,
            crate::panel::DockPosition::Bottom,
            dock_rect,
        );
    }

    fn render_status_bar_phase(&mut self) {
        Renderer::render_status_bar(
            &mut self.frame,
            &mut self.painter,
            self.model,
            self.plan.window_width,
            self.plan.window_height,
        );
    }

    fn render_modal_phase(&mut self) {
        if !self.plan.show_modal {
            return;
        }

        Renderer::render_modals(
            &mut self.frame,
            &mut self.painter,
            self.model,
            self.plan.window_width,
            self.plan.window_height,
        );
    }

    fn render_drop_overlay_phase(&mut self) {
        if !self.plan.show_drop_overlay {
            return;
        }

        Renderer::render_drop_overlay(
            &mut self.frame,
            &mut self.painter,
            self.model,
            self.plan.window_width,
            self.plan.window_height,
        );
    }

    #[cfg(debug_assertions)]
    fn render_perf_overlay_phase(&mut self, perf: &crate::perf::PerfStats) {
        if !self.plan.show_perf_overlay {
            return;
        }

        crate::perf::render_perf_overlay(
            &mut self.frame,
            &mut self.painter,
            perf,
            &self.model.theme,
        );
    }

    #[cfg(debug_assertions)]
    fn render_debug_overlay_phase(&mut self) {
        if !self.plan.show_debug_overlay {
            return;
        }

        let Some(ref overlay) = self.model.debug_overlay else {
            return;
        };

        let lines = overlay.render_lines(self.model);
        if lines.is_empty() {
            return;
        }

        let max_line_len = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0);
        let overlay_width = (max_line_len as f32 * self.painter.char_width()).ceil() as usize + 20;
        let overlay_height = lines.len() * self.painter.line_height() + 10;

        let overlay_x = self.plan.window_width.saturating_sub(overlay_width + 10);
        let overlay_y = 10;

        let bg_color = self.model.theme.overlay.background.to_argb_u32();
        let fg_color = self.model.theme.overlay.foreground.to_argb_u32();

        for py in overlay_y..(overlay_y + overlay_height).min(self.plan.window_height) {
            for px in overlay_x..(overlay_x + overlay_width).min(self.plan.window_width) {
                self.frame.blend_pixel(px, py, bg_color);
            }
        }

        for (i, line) in lines.iter().enumerate() {
            let text_x = overlay_x + 10;
            let text_y = overlay_y + 5 + i * self.painter.line_height();
            self.painter
                .draw(&mut self.frame, text_x, text_y, line, fg_color);
        }
    }

    #[cfg(debug_assertions)]
    fn record_cache_stats(&self, perf: &mut crate::perf::PerfStats) {
        let stats = self.painter.cache_stats();
        perf.add_cache_stats(stats.hits, stats.misses);
    }
}

enum EditorContentKind<'a> {
    Text {
        document: &'a crate::model::Document,
    },
    Csv {
        state: &'a crate::csv::CsvState,
    },
    Image {
        state: &'a crate::image::ImageState,
    },
    BinaryPlaceholder {
        placeholder: &'a crate::model::editor::BinaryPlaceholderState,
    },
}

struct EditorGroupScene<'a> {
    group: &'a EditorGroup,
    group_rect: Rect,
    editor: &'a crate::model::EditorState,
    layout: geometry::GroupLayout,
    is_focused: bool,
    content: EditorContentKind<'a>,
}

impl<'a> EditorGroupScene<'a> {
    fn resolve(
        model: &'a AppModel,
        group_id: GroupId,
        group_rect: Rect,
        is_focused: bool,
        char_width: f32,
    ) -> Option<Self> {
        let group = model.editor_area.groups.get(&group_id)?;
        let editor_id = group.active_editor_id()?;
        let editor = model.editor_area.editors.get(&editor_id)?;
        let layout = geometry::GroupLayout::new(group, model, char_width);

        let content = if let crate::model::editor::TabContent::BinaryPlaceholder(ref placeholder) =
            editor.tab_content
        {
            EditorContentKind::BinaryPlaceholder { placeholder }
        } else if let Some(state) = editor.view_mode.as_image() {
            EditorContentKind::Image { state }
        } else if let Some(state) = editor.view_mode.as_csv() {
            EditorContentKind::Csv { state }
        } else {
            let doc_id = editor.document_id?;
            let document = model.editor_area.documents.get(&doc_id)?;
            EditorContentKind::Text { document }
        };

        Some(Self {
            group,
            group_rect,
            editor,
            layout,
            is_focused,
            content,
        })
    }

    fn render(&self, frame: &mut Frame, painter: &mut TextPainter, model: &AppModel) {
        Renderer::render_tab_bar(frame, painter, model, self.group, &self.layout);
        self.render_content(frame, painter, model);

        if self.should_render_scrollbars(model) {
            self.render_scrollbars(frame, model);
        }

        self.render_unfocused_dim(frame, model);
    }

    fn render_content(&self, frame: &mut Frame, painter: &mut TextPainter, model: &AppModel) {
        match &self.content {
            EditorContentKind::Text { document } => {
                Renderer::render_text_area(
                    frame,
                    painter,
                    model,
                    self.editor,
                    document,
                    &self.layout,
                    self.is_focused,
                );
                Renderer::render_gutter(frame, painter, model, self.editor, document, &self.layout);
            }
            EditorContentKind::Csv { state } => {
                Renderer::render_csv_grid(
                    frame,
                    painter,
                    model,
                    state,
                    &self.layout,
                    self.is_focused,
                );
            }
            EditorContentKind::Image { state } => {
                Renderer::render_image_tab(frame, painter, model, state, &self.layout);
            }
            EditorContentKind::BinaryPlaceholder { placeholder } => {
                Renderer::render_binary_placeholder(
                    frame,
                    painter,
                    model,
                    placeholder,
                    &self.layout,
                );
            }
        }
    }

    fn should_render_scrollbars(&self, model: &AppModel) -> bool {
        model.config.show_scrollbar && matches!(&self.content, EditorContentKind::Text { .. })
    }

    fn render_scrollbars(&self, frame: &mut Frame, model: &AppModel) {
        let document = match &self.content {
            EditorContentKind::Text { document } => *document,
            _ => return,
        };

        Renderer::render_editor_scrollbars(frame, model, self.editor, document, &self.layout);
    }

    fn render_unfocused_dim(&self, frame: &mut Frame, model: &AppModel) {
        if self.is_focused || model.editor_area.groups.len() <= 1 {
            return;
        }

        let dim_color = 0x0A000000_u32;
        frame.blend_rect(self.group_rect, dim_color);
    }
}

enum PreviewContentKind<'a> {
    Hosted,
    NativeHtml {
        document: &'a crate::model::Document,
        preview: &'a crate::markdown::PreviewPane,
    },
    NativeMarkdown {
        document: &'a crate::model::Document,
        preview: &'a crate::markdown::PreviewPane,
    },
}

struct PreviewPaneScene<'a> {
    pane: geometry::Pane,
    line_height: usize,
    char_width: f32,
    content: PreviewContentKind<'a>,
}

impl<'a> PreviewPaneScene<'a> {
    fn resolve(
        model: &'a AppModel,
        preview_id: crate::model::editor_area::PreviewId,
        rect: Rect,
        preview_mode: PreviewRenderMode,
        line_height: usize,
        char_width: f32,
    ) -> Option<Self> {
        let preview = model.editor_area.previews.get(&preview_id)?;
        let document = model.editor_area.documents.get(&preview.document_id)?;
        let pane = geometry::Pane::with_header(rect, &model.metrics);

        let content = if preview_mode == PreviewRenderMode::WebviewChromeOnly {
            PreviewContentKind::Hosted
        } else if document.language == crate::syntax::LanguageId::Html {
            PreviewContentKind::NativeHtml { document, preview }
        } else {
            PreviewContentKind::NativeMarkdown { document, preview }
        };

        Some(Self {
            pane,
            line_height,
            char_width,
            content,
        })
    }

    fn render(&self, frame: &mut Frame, painter: &mut TextPainter, model: &AppModel) {
        Renderer::render_pane(frame, painter, model, &self.pane, Some("Preview"));

        match &self.content {
            PreviewContentKind::Hosted => {}
            PreviewContentKind::NativeHtml { document, preview } => {
                Renderer::render_native_html_preview(
                    frame,
                    painter,
                    model,
                    document,
                    preview,
                    &self.pane,
                    self.line_height,
                    self.char_width,
                );
            }
            PreviewContentKind::NativeMarkdown { document, preview } => {
                Renderer::render_native_markdown_preview(
                    frame,
                    painter,
                    model,
                    document,
                    preview,
                    &self.pane,
                    self.line_height,
                    self.char_width,
                );
            }
        }
    }
}

pub struct Renderer {
    font: Font,
    surface: Surface<Rc<Window>, Rc<Window>>,
    /// Persistent back buffer for partial rendering.
    /// Softbuffer doesn't guarantee buffer contents are preserved between frames,
    /// so we maintain our own buffer and copy to the surface on present.
    back_buffer: Vec<u32>,
    width: u32,
    height: u32,
    font_size: f32,
    line_metrics: LineMetrics,
    glyph_cache: GlyphCache,
    char_width: f32,
    scale_factor: f64,
}

impl Renderer {
    /// Create a new renderer, automatically detecting the window's scale factor
    pub fn new(window: Rc<Window>, context: &softbuffer::Context<Rc<Window>>) -> Result<Self> {
        let scale_factor = window.scale_factor();
        Self::with_scale_factor(window, context, scale_factor)
    }

    /// Create a new renderer with an explicit scale factor
    pub fn with_scale_factor(
        window: Rc<Window>,
        context: &softbuffer::Context<Rc<Window>>,
        scale_factor: f64,
    ) -> Result<Self> {
        let (width, height) = {
            let size = window.inner_size();
            (size.width, size.height)
        };

        let mut surface = Surface::new(context, Rc::clone(&window))
            .map_err(|e| anyhow::anyhow!("Failed to create surface: {}", e))?;

        // Explicitly resize the surface to match window dimensions
        // This is critical after DPI changes when the physical size changes
        surface
            .resize(
                NonZeroU32::new(width).unwrap_or(NonZeroU32::new(1).unwrap()),
                NonZeroU32::new(height).unwrap_or(NonZeroU32::new(1).unwrap()),
            )
            .map_err(|e| anyhow::anyhow!("Failed to resize surface: {}", e))?;

        let font = Font::from_bytes(
            include_bytes!("../../assets/JetBrainsMono.ttf") as &[u8],
            FontSettings::default(),
        )
        .map_err(|e| anyhow::anyhow!("Failed to load font: {}", e))?;

        let font_size = 14.0 * scale_factor as f32;

        let line_metrics = font
            .horizontal_line_metrics(font_size)
            .expect("Font missing horizontal line metrics");

        let (metrics, _) = font.rasterize('M', font_size);
        let char_width = metrics.advance_width;

        // Initialize back buffer with enough space for the window
        let buffer_size = (width as usize) * (height as usize);
        let back_buffer = vec![0u32; buffer_size];

        Ok(Self {
            font,
            surface,
            back_buffer,
            width,
            height,
            font_size,
            line_metrics,
            glyph_cache: HashMap::new(),
            char_width,
            scale_factor,
        })
    }

    /// Get the current scale factor
    pub fn scale_factor(&self) -> f64 {
        self.scale_factor
    }

    pub fn char_width(&self) -> f32 {
        self.char_width
    }

    pub fn line_height(&self) -> usize {
        self.line_metrics.new_line_size.ceil() as usize
    }

    pub fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    // =========================================================================
    // Damage Tracking Helpers
    // =========================================================================

    /// Compute effective damage, forcing Full for complex overlays
    ///
    /// Forces full redraw when:
    /// - Modal is active (background dim accumulation)
    /// - Drop overlay is showing
    /// - Debug overlays are visible (debug builds only)
    fn compute_effective_damage(
        &self,
        damage: &Damage,
        model: &AppModel,
        #[allow(unused_variables)] perf: &crate::perf::PerfStats,
    ) -> Damage {
        // Force full redraw for complex overlays
        if model.ui.active_modal.is_some() {
            return Damage::Full;
        }

        if model.ui.drop_state.is_hovering {
            return Damage::Full;
        }

        // Debug builds: force full for perf/debug overlays
        #[cfg(debug_assertions)]
        {
            if perf.should_show_overlay() {
                return Damage::Full;
            }
            if let Some(ref overlay) = model.debug_overlay {
                if overlay.visible {
                    return Damage::Full;
                }
            }
        }

        damage.clone()
    }

    #[allow(clippy::too_many_arguments)]
    fn build_render_plan(
        &self,
        model: &mut AppModel,
        perf: &crate::perf::PerfStats,
        damage: &Damage,
        window_width: usize,
        window_height: usize,
        line_height: usize,
        char_width: f32,
    ) -> RenderPlan {
        let window_layout = geometry::WindowLayout::compute(model, line_height);
        let splitters = model
            .editor_area
            .compute_layout_scaled(window_layout.editor_area_rect, model.metrics.splitter_width);
        model
            .editor_area
            .sync_all_viewports(line_height, char_width, &model.metrics);

        let effective_damage = self.compute_effective_damage(damage, model, perf);
        let render_editor = effective_damage.is_full()
            || effective_damage.includes_editor()
            || has_cursor_lines_damage(&effective_damage);
        let render_status_bar =
            effective_damage.is_full() || effective_damage.includes_status_bar();

        let is_text_mode = model
            .editor_area
            .groups
            .get(&model.editor_area.focused_group_id)
            .and_then(|g| g.active_editor_id())
            .and_then(|id| model.editor_area.editors.get(&id))
            .map(|e| {
                matches!(e.tab_content, crate::model::TabContent::Text) && !e.view_mode.is_csv()
            })
            .unwrap_or(false);

        let cursor_lines_only = if is_text_mode {
            effective_damage.cursor_lines_only().map(|v| v.to_vec())
        } else {
            None
        };

        RenderPlan {
            window_width,
            window_height,
            window_layout,
            splitters,
            effective_damage,
            render_editor,
            render_status_bar,
            cursor_lines_only,
            show_modal: model.ui.active_modal.is_some(),
            show_drop_overlay: model.ui.drop_state.is_hovering,
            #[cfg(debug_assertions)]
            show_perf_overlay: perf.should_show_overlay(),
            #[cfg(debug_assertions)]
            show_debug_overlay: model
                .debug_overlay
                .as_ref()
                .map(|overlay| overlay.visible)
                .unwrap_or(false),
        }
    }

    fn clear_back_buffer(&mut self, model: &AppModel, plan: &RenderPlan) {
        let bg_color = model.theme.editor.background.to_argb_u32();
        let mut frame = Frame::new(&mut self.back_buffer, plan.window_width, plan.window_height);

        if plan.effective_damage.is_full() {
            frame.clear(bg_color);
            return;
        }

        if plan.render_editor {
            frame.fill_rect(plan.window_layout.content_rect, bg_color);
        }

        if plan.render_status_bar {
            let status_bg = model.theme.status_bar.background.to_argb_u32();
            frame.fill_rect(plan.window_layout.status_bar_rect, status_bg);
        }
    }

    fn render_cursor_lines_only(
        frame: &mut Frame,
        painter: &mut TextPainter,
        model: &AppModel,
        dirty_lines: &[usize],
    ) {
        editor_text::render_cursor_lines_only(frame, painter, model, dirty_lines);
    }

    /// Render the entire editor area: all groups, preview panes, and splitters.
    ///
    /// This is the top-level widget that orchestrates rendering of:
    /// - All editor groups (each with tab bar, gutter, text area)
    /// - All preview panes (markdown preview)
    /// - Splitter bars between groups
    pub fn render_editor_area(
        frame: &mut Frame,
        painter: &mut TextPainter,
        model: &AppModel,
        splitters: &[SplitterBar],
    ) {
        Self::render_editor_area_with_preview_mode(
            frame,
            painter,
            model,
            splitters,
            PreviewRenderMode::WebviewChromeOnly,
        )
    }

    pub fn render_editor_area_with_preview_mode(
        frame: &mut Frame,
        painter: &mut TextPainter,
        model: &AppModel,
        splitters: &[SplitterBar],
        preview_mode: PreviewRenderMode,
    ) {
        for (&group_id, group) in &model.editor_area.groups {
            let is_focused = group_id == model.editor_area.focused_group_id;
            Self::render_editor_group(frame, painter, model, group_id, group.rect, is_focused);
        }

        // Render preview panes
        for (&preview_id, preview) in &model.editor_area.previews {
            Self::render_preview_pane(
                frame,
                painter,
                model,
                preview_id,
                preview.rect,
                preview_mode,
            );
        }

        Self::render_splitters(frame, splitters, model);
    }

    /// Render a pane with optional header, background, and borders.
    ///
    /// This is a reusable widget for any UI element that needs a consistent
    /// pane layout (preview panes, panels, dialogs, etc.).
    fn render_pane(
        frame: &mut Frame,
        painter: &mut TextPainter,
        model: &AppModel,
        pane: &geometry::Pane,
        title: Option<&str>,
    ) {
        let bg_color = model.theme.editor.background.to_argb_u32();
        let header_bg = model.theme.tab_bar.background.to_argb_u32();
        let header_fg = model.theme.tab_bar.active_foreground.to_argb_u32();
        let border_color = model.theme.tab_bar.border.to_argb_u32();

        // Pane background
        frame.fill_rect_px(pane.x(), pane.y(), pane.width(), pane.height(), bg_color);

        // Header (if present)
        if pane.has_header() {
            frame.fill_rect_px(
                pane.x(),
                pane.y(),
                pane.width(),
                pane.header_height,
                header_bg,
            );

            if let Some(title) = title {
                painter.draw(
                    frame,
                    pane.header_title_x(),
                    pane.header_title_y(&model.metrics),
                    title,
                    header_fg,
                );
            }

            // Header border
            if pane.header_border {
                frame.fill_rect_px(
                    pane.x(),
                    pane.header_border_y(),
                    pane.width(),
                    pane.border_width,
                    border_color,
                );
            }
        }

        // Outer borders (if configured)
        if pane.borders.top {
            frame.fill_rect_px(
                pane.x(),
                pane.y(),
                pane.width(),
                pane.border_width,
                border_color,
            );
        }
        if pane.borders.bottom {
            let y = pane.y() + pane.height().saturating_sub(pane.border_width);
            frame.fill_rect_px(pane.x(), y, pane.width(), pane.border_width, border_color);
        }
        if pane.borders.left {
            frame.fill_rect_px(
                pane.x(),
                pane.y(),
                pane.border_width,
                pane.height(),
                border_color,
            );
        }
        if pane.borders.right {
            let x = pane.x() + pane.width().saturating_sub(pane.border_width);
            frame.fill_rect_px(x, pane.y(), pane.border_width, pane.height(), border_color);
        }
    }

    /// Render a markdown preview pane.
    ///
    /// When `preview_mode` is `WebviewChromeOnly`, this only renders the pane chrome
    /// (background, header, borders) — the webview overlay handles content rendering.
    /// When `NativeMarkdown`, renders markdown content natively (for headless screenshots).
    fn render_preview_pane(
        frame: &mut Frame,
        painter: &mut TextPainter,
        model: &AppModel,
        preview_id: crate::model::editor_area::PreviewId,
        rect: Rect,
        preview_mode: PreviewRenderMode,
    ) {
        let Some(scene) = PreviewPaneScene::resolve(
            model,
            preview_id,
            rect,
            preview_mode,
            painter.line_height(),
            painter.char_width(),
        ) else {
            return;
        };

        scene.render(frame, painter, model);
    }

    /// Native HTML preview: extract visible text content from HTML source and render it
    /// with basic styling (headings, paragraphs, lists) by stripping tags.
    #[allow(clippy::too_many_arguments)]
    fn render_native_html_preview(
        frame: &mut Frame,
        painter: &mut TextPainter,
        model: &AppModel,
        document: &crate::model::document::Document,
        preview: &crate::markdown::PreviewPane,
        pane: &geometry::Pane,
        line_height: usize,
        char_width: f32,
    ) {
        let visible_lines = pane.visible_lines(line_height);
        let text_x = pane.content_x();
        let max_width = pane.max_text_width();
        let max_chars = if char_width > 0.0 {
            (max_width as f32 / char_width) as usize
        } else {
            80
        };

        let text_color = model.theme.editor.foreground.to_argb_u32();
        let heading_color = model.theme.syntax.keyword.to_argb_u32();
        let link_color = model.theme.syntax.string.to_argb_u32();
        let muted_color = model.theme.gutter.foreground.to_argb_u32();

        // Extract visible text from HTML by stripping tags and rendering text content
        let source = document.buffer.to_string();
        let lines = extract_html_text_lines(&source);

        let mut y = pane.content_y();
        for (i, line) in lines.iter().enumerate() {
            if i >= preview.scroll_offset + visible_lines {
                break;
            }
            if i < preview.scroll_offset {
                continue;
            }

            let display: String = line.text.chars().take(max_chars).collect();
            let color = match line.style {
                HtmlTextStyle::Heading => heading_color,
                HtmlTextStyle::Link => link_color,
                HtmlTextStyle::Muted => muted_color,
                HtmlTextStyle::Normal => text_color,
            };
            painter.draw(frame, text_x, y, &display, color);
            y += line_height;
        }
    }

    /// Native markdown preview: simple line-by-line markdown rendering.
    #[allow(clippy::too_many_arguments)]
    fn render_native_markdown_preview(
        frame: &mut Frame,
        painter: &mut TextPainter,
        model: &AppModel,
        document: &crate::model::document::Document,
        preview: &crate::markdown::PreviewPane,
        pane: &geometry::Pane,
        line_height: usize,
        char_width: f32,
    ) {
        let visible_lines = pane.visible_lines(line_height);
        let text_x = pane.content_x();
        let max_width = pane.max_text_width();
        let max_chars = if char_width > 0.0 {
            (max_width as f32 / char_width) as usize
        } else {
            80
        };

        let text_color = model.theme.editor.foreground.to_argb_u32();
        let heading_color = model.theme.syntax.keyword.to_argb_u32();
        let code_bg = model.theme.gutter.background.to_argb_u32();
        let link_color = model.theme.syntax.string.to_argb_u32();

        let mut y = pane.content_y();
        let mut in_code_block = false;

        for line_num in 0..document.buffer.len_lines() {
            if line_num >= preview.scroll_offset + visible_lines {
                break;
            }
            if line_num < preview.scroll_offset {
                continue;
            }

            let line_text = document.buffer.line(line_num).to_string();
            let line_text = line_text.trim_end_matches('\n');

            let trimmed = line_text.trim();

            if trimmed.starts_with("```") {
                in_code_block = !in_code_block;
                y += line_height;
                continue;
            }

            if in_code_block {
                frame.fill_rect_px(
                    text_x.saturating_sub(4),
                    y,
                    max_width + 8,
                    line_height,
                    code_bg,
                );
                let display_line: String = line_text.chars().take(max_chars).collect();
                painter.draw(frame, text_x, y, &display_line, text_color);
            } else if let Some(heading) = trimmed.strip_prefix("# ") {
                let display: String = heading.chars().take(max_chars).collect();
                painter.draw(frame, text_x, y, &display, heading_color);
            } else if let Some(heading) = trimmed.strip_prefix("## ") {
                let display: String = heading.chars().take(max_chars).collect();
                painter.draw(frame, text_x, y, &display, heading_color);
            } else if let Some(heading) = trimmed.strip_prefix("### ") {
                let display: String = heading.chars().take(max_chars).collect();
                painter.draw(frame, text_x, y, &display, heading_color);
            } else if let Some(list_item) = trimmed.strip_prefix("- ") {
                let bullet = format!("• {}", list_item);
                let display: String = bullet.chars().take(max_chars).collect();
                painter.draw(frame, text_x, y, &display, text_color);
            } else if let Some(list_item) = trimmed.strip_prefix("* ") {
                let bullet = format!("• {}", list_item);
                let display: String = bullet.chars().take(max_chars).collect();
                painter.draw(frame, text_x, y, &display, text_color);
            } else if trimmed.starts_with('[') && trimmed.contains("](") {
                let display: String = trimmed.chars().take(max_chars).collect();
                painter.draw(frame, text_x, y, &display, link_color);
            } else {
                let display: String = line_text.chars().take(max_chars).collect();
                painter.draw(frame, text_x, y, &display, text_color);
            }

            y += line_height;
        }
    }

    /// Render an entire editor group: tab bar, gutter, text area, and focus dimming.
    ///
    /// This is the main orchestrator that calls individual widget functions.
    /// Uses GroupLayout for all positioning to ensure DPI-aware, consistent rendering.
    pub fn render_editor_group(
        frame: &mut Frame,
        painter: &mut TextPainter,
        model: &AppModel,
        group_id: GroupId,
        group_rect: Rect,
        is_focused: bool,
    ) {
        let Some(scene) = EditorGroupScene::resolve(
            model,
            group_id,
            group_rect,
            is_focused,
            painter.char_width(),
        ) else {
            return;
        };

        scene.render(frame, painter, model);
    }

    fn render_editor_scrollbars(
        frame: &mut Frame,
        model: &AppModel,
        editor_state: &crate::model::editor::EditorState,
        document: &crate::model::document::Document,
        layout: &geometry::GroupLayout,
    ) {
        editor_scrollbars::render_editor_scrollbars(frame, model, editor_state, document, layout);
    }

    fn render_image_tab(
        frame: &mut Frame,
        painter: &mut TextPainter,
        model: &AppModel,
        img_state: &crate::image::ImageState,
        layout: &geometry::GroupLayout,
    ) {
        editor_special_tabs::render_image_tab(frame, painter, model, img_state, layout);
    }

    fn render_binary_placeholder(
        frame: &mut Frame,
        painter: &mut TextPainter,
        model: &AppModel,
        placeholder: &crate::model::editor::BinaryPlaceholderState,
        layout: &geometry::GroupLayout,
    ) {
        editor_special_tabs::render_binary_placeholder(frame, painter, model, placeholder, layout);
    }

    fn render_tab_bar(
        frame: &mut Frame,
        painter: &mut TextPainter,
        model: &AppModel,
        group: &EditorGroup,
        _layout: &geometry::GroupLayout,
    ) {
        let metrics = &model.metrics;
        let tab_bar = geometry::TabBarLayout::new(group, model, painter.char_width());

        let tab_bar_bg = model.theme.tab_bar.background.to_argb_u32();
        frame.fill_rect_px(
            tab_bar.rect_x,
            tab_bar.rect_y,
            tab_bar.rect_w,
            tab_bar.rect_h,
            tab_bar_bg,
        );

        let border_color = model.theme.tab_bar.border.to_argb_u32();
        frame.fill_rect_px(
            tab_bar.rect_x,
            tab_bar.border_y,
            tab_bar.rect_w,
            metrics.border_width,
            border_color,
        );

        for tab in &tab_bar.tabs {
            let (bg_color, fg_color) = if tab.is_active {
                (
                    model.theme.tab_bar.active_background.to_argb_u32(),
                    model.theme.tab_bar.active_foreground.to_argb_u32(),
                )
            } else {
                (
                    model.theme.tab_bar.inactive_background.to_argb_u32(),
                    model.theme.tab_bar.inactive_foreground.to_argb_u32(),
                )
            };

            frame.fill_rect_px(tab.x, tab.y, tab.width, tab.height, bg_color);
            painter.draw(frame, tab.text_x, tab.text_y, &tab.title, fg_color);
        }
    }

    pub fn render_splitters(frame: &mut Frame, splitters: &[SplitterBar], model: &AppModel) {
        let splitter_color = model.theme.splitter.background.to_argb_u32();

        for splitter in splitters {
            frame.fill_rect(splitter.rect, splitter_color);
        }
    }

    pub fn render_sidebar(
        frame: &mut Frame,
        painter: &mut TextPainter,
        model: &AppModel,
        sidebar_width: usize,
        sidebar_height: usize,
    ) {
        panels::render_sidebar(frame, painter, model, sidebar_width, sidebar_height);
    }

    fn render_dock(
        frame: &mut Frame,
        painter: &mut TextPainter,
        model: &AppModel,
        position: crate::panel::DockPosition,
        rect: crate::model::editor_area::Rect,
    ) {
        panels::render_dock(frame, painter, model, position, rect);
    }

    /// Render the gutter (line numbers and border) for an editor group.
    ///
    /// Draws:
    /// - Line numbers (highlighted for current line)
    /// - Gutter border line
    fn render_gutter(
        frame: &mut Frame,
        painter: &mut TextPainter,
        model: &AppModel,
        editor_state: &crate::model::EditorState,
        document: &crate::model::Document,
        layout: &geometry::GroupLayout,
    ) {
        editor_text::render_gutter(frame, painter, model, editor_state, document, layout);
    }

    fn render_text_area(
        frame: &mut Frame,
        painter: &mut TextPainter,
        model: &AppModel,
        editor_state: &crate::model::EditorState,
        document: &crate::model::Document,
        layout: &geometry::GroupLayout,
        is_focused: bool,
    ) {
        editor_text::render_text_area(
            frame,
            painter,
            model,
            editor_state,
            document,
            layout,
            is_focused,
        );
    }

    /// Render CSV grid view
    ///
    /// Draws:
    /// - Row numbers column
    /// - Column headers (A, B, C, ...)
    /// - Cell grid with data
    /// - Selected cell highlight
    fn render_csv_grid(
        frame: &mut Frame,
        painter: &mut TextPainter,
        model: &AppModel,
        csv: &crate::csv::CsvState,
        layout: &geometry::GroupLayout,
        is_focused: bool,
    ) {
        use crate::csv::render::{column_to_letters, truncate_text, CsvRenderLayout};

        let char_width = painter.char_width();
        let line_height = painter.line_height();
        let rect_x = layout.rect_x();
        let rect_w = layout.rect_w();
        let content_y = layout.content_y();
        let content_h = layout.content_h();

        let theme = &model.theme;
        let bg_color = theme.editor.background.to_argb_u32();
        let fg_color = theme.editor.foreground.to_argb_u32();
        let header_bg = theme.csv.header_background.to_argb_u32();
        let header_fg = theme.csv.header_foreground.to_argb_u32();
        let grid_line_color = theme.csv.grid_line.to_argb_u32();
        let selection_bg = theme.csv.selected_cell_background.to_argb_u32();
        let selection_border = theme.csv.selected_cell_border.to_argb_u32();
        let number_color = theme.csv.number_foreground.to_argb_u32();

        // Fill background
        frame.fill_rect_px(rect_x, content_y, rect_w, content_h, bg_color);

        // Calculate layout
        let layout =
            CsvRenderLayout::calculate(csv, rect_x, rect_w, content_y, line_height, char_width);

        // Draw column headers background
        frame.fill_rect_px(
            layout.grid_x,
            layout.col_header_y,
            rect_w.saturating_sub(layout.row_header_width),
            layout.col_header_height,
            header_bg,
        );

        // Draw row header background
        frame.fill_rect_px(
            layout.row_header_x,
            content_y,
            layout.row_header_width,
            content_h,
            header_bg,
        );

        // Draw column headers (A, B, C, ...)
        for (i, &(col_idx, col_x)) in layout.visible_columns.iter().enumerate() {
            let col_width_px = layout.column_widths_px.get(i).copied().unwrap_or(50);
            let letter = column_to_letters(col_idx);

            // Center the letter in the column
            let text_width = (letter.len() as f32 * char_width).ceil() as usize;
            let text_x = layout.grid_x + col_x + (col_width_px.saturating_sub(text_width)) / 2;

            painter.draw(frame, text_x, layout.col_header_y, &letter, header_fg);
        }

        // Calculate visible rows
        let visible_rows = content_h.saturating_sub(layout.col_header_height) / line_height;
        let end_row = (csv.viewport.top_row + visible_rows).min(csv.data.row_count());

        // Draw row headers (1, 2, 3, ...)
        for screen_row in 0..visible_rows {
            let data_row = csv.viewport.top_row + screen_row;
            if data_row >= csv.data.row_count() {
                break;
            }

            let y = layout.data_y + screen_row * line_height;
            let row_label = format!("{}", data_row + 1);
            let text_width = (row_label.len() as f32 * char_width).ceil() as usize;
            let text_x = layout.row_header_x + layout.row_header_width - text_width - 8;

            painter.draw(frame, text_x, y, &row_label, header_fg);
        }

        // Draw horizontal grid lines
        for screen_row in 0..=visible_rows {
            let y = layout.data_y + screen_row * line_height;
            if y < content_y + content_h {
                frame.fill_rect_px(
                    layout.grid_x,
                    y,
                    rect_w.saturating_sub(layout.row_header_width),
                    1,
                    grid_line_color,
                );
            }
        }

        // Draw vertical grid lines
        for &(_, col_x) in layout.visible_columns.iter() {
            let x = layout.grid_x + col_x;
            frame.fill_rect_px(x, content_y, 1, content_h, grid_line_color);
        }
        // Right edge of last column
        if let Some(&(_, last_x)) = layout.visible_columns.last() {
            if let Some(&last_w) = layout.column_widths_px.last() {
                let x = layout.grid_x + last_x + last_w;
                if x < rect_x + rect_w {
                    frame.fill_rect_px(x, content_y, 1, content_h, grid_line_color);
                }
            }
        }

        // Pre-calculate selected cell geometry for background drawing
        let selection_geom = if is_focused {
            let sel_row = csv.selected_cell.row;
            let sel_col = csv.selected_cell.col;

            if sel_row >= csv.viewport.top_row && sel_row < end_row {
                layout
                    .visible_columns
                    .iter()
                    .enumerate()
                    .find(|(_, &(col_idx, _))| col_idx == sel_col)
                    .map(|(screen_col, &(_, col_x))| {
                        let col_width_px = layout
                            .column_widths_px
                            .get(screen_col)
                            .copied()
                            .unwrap_or(50);
                        let screen_row = sel_row - csv.viewport.top_row;
                        let cell_x = layout.grid_x + col_x;
                        let cell_y = layout.data_y + screen_row * line_height;
                        (cell_x, cell_y, col_width_px)
                    })
            } else {
                None
            }
        } else {
            None
        };

        // Draw selection background BEFORE cells so text is visible on top
        if let Some((cell_x, cell_y, col_width_px)) = selection_geom {
            frame.fill_rect_px(
                cell_x + 1,
                cell_y + 1,
                col_width_px.saturating_sub(2),
                line_height.saturating_sub(2),
                selection_bg,
            );
        }

        // Draw cells
        for screen_row in 0..visible_rows {
            let data_row = csv.viewport.top_row + screen_row;
            if data_row >= csv.data.row_count() {
                break;
            }

            let y = layout.data_y + screen_row * line_height;

            for (i, &(col_idx, col_x)) in layout.visible_columns.iter().enumerate() {
                let col_width_px = layout.column_widths_px.get(i).copied().unwrap_or(50);
                let col_width_chars = csv.column_widths.get(col_idx).copied().unwrap_or(10);

                let cell_value = csv.data.get(data_row, col_idx);
                let display_text = truncate_text(cell_value, col_width_chars);

                // Determine color and alignment
                let (text_color, align_right) = if crate::csv::render::is_number(cell_value) {
                    (number_color, true)
                } else {
                    (fg_color, false)
                };

                let text_width = (display_text.chars().count() as f32 * char_width).ceil() as usize;
                let text_x = if align_right {
                    layout.grid_x + col_x + col_width_px - text_width - 4
                } else {
                    layout.grid_x + col_x + 4
                };

                painter.draw(frame, text_x, y + 1, &display_text, text_color);
            }
        }

        // Draw selection border AFTER cells (on top)
        if let Some((cell_x, cell_y, col_width_px)) = selection_geom {
            // Draw selection border (2px on all sides)
            frame.fill_rect_px(cell_x, cell_y, col_width_px, 2, selection_border); // top
            frame.fill_rect_px(
                cell_x,
                cell_y + line_height - 2,
                col_width_px,
                2,
                selection_border,
            ); // bottom
            frame.fill_rect_px(cell_x, cell_y, 2, line_height, selection_border); // left
            frame.fill_rect_px(
                cell_x + col_width_px - 2,
                cell_y,
                2,
                line_height,
                selection_border,
            ); // right
        }

        // Draw cell editor if editing
        if let Some(edit_state) = &csv.editing {
            Self::render_csv_cell_editor(
                frame,
                painter,
                model,
                csv,
                &layout,
                edit_state,
                line_height,
                char_width,
            );
        }
    }

    /// Render the cell editor overlay when editing a CSV cell
    #[allow(clippy::too_many_arguments)]
    fn render_csv_cell_editor(
        frame: &mut Frame,
        painter: &mut TextPainter,
        model: &AppModel,
        csv: &crate::csv::CsvState,
        layout: &crate::csv::render::CsvRenderLayout,
        edit_state: &crate::csv::CellEditState,
        line_height: usize,
        char_width: f32,
    ) {
        let pos = &edit_state.position;

        // Check if cell is visible
        if pos.row < csv.viewport.top_row {
            return;
        }
        let screen_row = pos.row - csv.viewport.top_row;
        if screen_row >= csv.viewport.visible_rows {
            return;
        }

        // Find column position
        let col_info = layout
            .visible_columns
            .iter()
            .enumerate()
            .find(|(_, &(col_idx, _))| col_idx == pos.col);

        let (screen_col_idx, col_x) = match col_info {
            Some((idx, &(_, x))) => (idx, x),
            None => return,
        };

        let col_width_px = layout
            .column_widths_px
            .get(screen_col_idx)
            .copied()
            .unwrap_or(50);
        let cell_x = layout.grid_x + col_x;
        let cell_y = layout.data_y + screen_row * line_height;

        // Use input field colors from overlay theme
        let edit_bg = model.theme.overlay.input_background.to_argb_u32();

        // Draw edit background (fill entire cell)
        frame.fill_rect_px(
            cell_x + 1,
            cell_y + 1,
            col_width_px.saturating_sub(2),
            line_height.saturating_sub(2),
            edit_bg,
        );

        // Use TextFieldRenderer with scroll support
        let opts = TextFieldOptions {
            x: cell_x + 4,
            y: cell_y + 1,
            width: col_width_px.saturating_sub(8), // 4px padding each side
            height: line_height.saturating_sub(2),
            char_width,
            text_color: model.theme.overlay.foreground.to_argb_u32(),
            cursor_color: model.theme.editor.cursor_color.to_argb_u32(),
            selection_color: model.theme.editor.selection_background.to_argb_u32(),
            cursor_visible: model.ui.cursor_visible,
            scroll_x: edit_state.scroll_x, // Use scroll offset from state
        };

        TextFieldRenderer::render(frame, painter, &edit_state.editable, &opts);
    }

    /// Render the status bar at the bottom of the window.
    ///
    /// This is a standalone widget function that draws:
    /// - Status bar background
    /// - Left-aligned segments (mode, filename, position, etc.)
    /// - Right-aligned segments
    /// - Separators between segments
    pub fn render_status_bar(
        frame: &mut Frame,
        painter: &mut TextPainter,
        model: &AppModel,
        window_width: usize,
        window_height: usize,
    ) {
        let char_width = painter.char_width();
        let line_height = painter.line_height();
        let status_bar_bg = model.theme.status_bar.background.to_argb_u32();
        let status_bar_fg = model.theme.status_bar.foreground.to_argb_u32();
        let status_bar_h = geometry::status_bar_height(line_height);
        let status_y = window_height.saturating_sub(status_bar_h);
        let text_offset_y = model.metrics.padding_small;

        // Background
        frame.fill_rect_px(0, status_y, window_width, status_bar_h, status_bar_bg);

        // Layout calculation
        let available_chars = (window_width as f32 / char_width).floor() as usize;
        let layout = model.ui.status_bar.layout(available_chars);

        // Left segments
        for seg in &layout.left {
            let x_px = (seg.x as f32 * char_width).round() as usize;
            painter.draw(
                frame,
                x_px,
                status_y + text_offset_y,
                &seg.text,
                status_bar_fg,
            );
        }

        // Right segments
        for seg in &layout.right {
            let x_px = (seg.x as f32 * char_width).round() as usize;
            painter.draw(
                frame,
                x_px,
                status_y + text_offset_y,
                &seg.text,
                status_bar_fg,
            );
        }

        // Separators
        let separator_color = model
            .theme
            .status_bar
            .foreground
            .with_alpha(100)
            .to_argb_u32();
        for &sep_char_x in &layout.separator_positions {
            let x_px = (sep_char_x as f32 * char_width).round() as usize;
            frame.fill_rect_px(x_px, status_y, 1, status_bar_h, separator_color);
        }
    }

    pub fn render_modals(
        frame: &mut Frame,
        painter: &mut TextPainter,
        model: &AppModel,
        window_width: usize,
        window_height: usize,
    ) {
        modal::render_modals(frame, painter, model, window_width, window_height);
    }

    fn render_drop_overlay(
        frame: &mut Frame,
        painter: &mut TextPainter,
        model: &AppModel,
        window_width: usize,
        window_height: usize,
    ) {
        modal::render_drop_overlay(frame, painter, model, window_width, window_height);
    }

    pub fn render(
        &mut self,
        model: &mut AppModel,
        perf: &mut crate::perf::PerfStats,
        damage: &Damage,
    ) -> Result<()> {
        // Skip rendering entirely if no damage
        if matches!(damage, Damage::None) {
            return Ok(());
        }

        perf.reset_frame_stats();

        if self.width != model.window_size.0 || self.height != model.window_size.1 {
            self.width = model.window_size.0;
            self.height = model.window_size.1;

            // Resize back buffer to match new window size
            let new_size = (self.width as usize) * (self.height as usize);
            self.back_buffer.resize(new_size, 0);

            self.surface
                .resize(
                    NonZeroU32::new(self.width).unwrap(),
                    NonZeroU32::new(self.height).unwrap(),
                )
                .map_err(|e| anyhow::anyhow!("Failed to resize surface: {}", e))?;
        }

        let line_height = self.line_metrics.new_line_size.ceil() as usize;
        let font_size = self.font_size;
        let ascent = self.line_metrics.ascent;
        let char_width = self.char_width;
        let width_usize = self.width as usize;
        let height_usize = self.height as usize;

        #[cfg(feature = "damage-debug")]
        let status_bar_height = line_height;

        let plan = self.build_render_plan(
            model,
            perf,
            damage,
            width_usize,
            height_usize,
            line_height,
            char_width,
        );

        // All rendering happens to back_buffer (persistent between frames).
        // At the end, we copy to the surface buffer and present.
        if !plan.uses_cursor_lines_fast_path() {
            let _timer = perf.time_clear();
            self.clear_back_buffer(model, &plan);
        }

        if let Some(ref dirty_lines) = plan.cursor_lines_only {
            let mut frame = Frame::new(&mut self.back_buffer, width_usize, height_usize);
            let mut painter = TextPainter::new(
                &self.font,
                &mut self.glyph_cache,
                font_size,
                ascent,
                char_width,
                line_height,
            );
            {
                let _timer = perf.time_text();
                Self::render_cursor_lines_only(&mut frame, &mut painter, model, dirty_lines);
            }
            #[cfg(debug_assertions)]
            {
                let stats = painter.cache_stats();
                perf.add_cache_stats(stats.hits, stats.misses);
            }
        } else {
            let mut session = RenderSession::new(
                &mut self.back_buffer,
                width_usize,
                height_usize,
                &self.font,
                &mut self.glyph_cache,
                font_size,
                ascent,
                char_width,
                line_height,
                model,
                &plan,
            );

            if plan.render_editor {
                {
                    let _timer = perf.time_text();
                    session.render_editor_area_phase();
                }
                session.render_sidebar_phase();
                session.render_right_dock_phase();
                session.render_bottom_dock_phase();
            }

            if plan.render_status_bar {
                let _timer = perf.time_status_bar();
                session.render_status_bar_phase();
            }

            session.render_modal_phase();
            session.render_drop_overlay_phase();

            #[cfg(debug_assertions)]
            session.render_perf_overlay_phase(perf);

            #[cfg(debug_assertions)]
            session.render_debug_overlay_phase();

            #[cfg(debug_assertions)]
            session.record_cache_stats(perf);
        }

        // Debug: visualize damage regions with colored outlines
        #[cfg(feature = "damage-debug")]
        {
            let mut frame = Frame::new(&mut self.back_buffer, width_usize, height_usize);
            Self::render_damage_debug(
                &mut frame,
                &plan.effective_damage,
                model,
                plan.window_width,
                plan.window_height,
                status_bar_height,
                line_height,
                char_width,
            );
        }

        // Copy back buffer to surface and present
        {
            let _timer = perf.time_present();
            let mut buffer = self
                .surface
                .buffer_mut()
                .map_err(|e| anyhow::anyhow!("Failed to get surface buffer: {}", e))?;
            buffer.copy_from_slice(&self.back_buffer);
            buffer
                .present()
                .map_err(|e| anyhow::anyhow!("Failed to present buffer: {}", e))?;
        }

        Ok(())
    }

    /// Render debug visualization of damage regions
    #[cfg(feature = "damage-debug")]
    #[allow(clippy::too_many_arguments)]
    fn render_damage_debug(
        frame: &mut Frame,
        damage: &Damage,
        model: &AppModel,
        width: usize,
        height: usize,
        status_bar_height: usize,
        line_height: usize,
        char_width: f32,
    ) {
        // Semi-transparent colors for different damage types (alpha = 0x80 = 50%)
        const RED: u32 = 0x80FF0000; // EditorArea - red
        const BLUE: u32 = 0x800000FF; // StatusBar - blue
        const GREEN: u32 = 0x8000FF00; // CursorLines - green
        const YELLOW: u32 = 0x80FFFF00; // Full - yellow

        match damage {
            Damage::None => {
                // No damage - nothing to visualize
            }
            Damage::Full => {
                // Draw yellow border around entire window
                Self::draw_rect_outline_blended(frame, 0, 0, width, height, YELLOW, 3);
            }
            Damage::Areas(areas) => {
                for area in areas {
                    match area {
                        DamageArea::EditorArea => {
                            // Red outline around editor area (everything except status bar)
                            let editor_height = height.saturating_sub(status_bar_height);
                            Self::draw_rect_outline_blended(
                                frame,
                                0,
                                0,
                                width,
                                editor_height,
                                RED,
                                3,
                            );
                        }
                        DamageArea::StatusBar => {
                            // Blue outline around status bar
                            let status_y = height.saturating_sub(status_bar_height);
                            Self::draw_rect_outline_blended(
                                frame,
                                0,
                                status_y,
                                width,
                                status_bar_height,
                                BLUE,
                                3,
                            );
                        }
                        DamageArea::CursorLines(lines) => {
                            // Draw green highlight over each damaged cursor line
                            // Use GroupLayout for consistent, DPI-aware positioning
                            let focused_group_id = model.editor_area.focused_group_id;
                            if let Some(group) = model.editor_area.groups.get(&focused_group_id) {
                                if let Some(editor_id) = group.active_editor_id() {
                                    if let Some(editor) = model.editor_area.editors.get(&editor_id)
                                    {
                                        let layout =
                                            geometry::GroupLayout::new(group, model, char_width);

                                        for &doc_line in lines {
                                            if let Some(y) = layout.line_to_screen_y(
                                                doc_line,
                                                editor.viewport.top_line,
                                                line_height,
                                            ) {
                                                // Fill with semi-transparent green
                                                frame.blend_rect_px(
                                                    layout.rect_x(),
                                                    y,
                                                    layout.rect_w(),
                                                    line_height,
                                                    GREEN,
                                                );
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    /// Draw a rectangle outline with alpha blending (border only, not filled)
    #[cfg(feature = "damage-debug")]
    fn draw_rect_outline_blended(
        frame: &mut Frame,
        x: usize,
        y: usize,
        width: usize,
        height: usize,
        color: u32,
        thickness: usize,
    ) {
        // Top edge
        frame.blend_rect_px(x, y, width, thickness, color);
        // Bottom edge
        frame.blend_rect_px(
            x,
            y + height.saturating_sub(thickness),
            width,
            thickness,
            color,
        );
        // Left edge
        frame.blend_rect_px(x, y, thickness, height, color);
        // Right edge
        frame.blend_rect_px(
            x + width.saturating_sub(thickness),
            y,
            thickness,
            height,
            color,
        );
    }

    /// Convert pixel coordinates to document line and column.
    /// Delegates to geometry module for the actual calculation.
    pub fn pixel_to_cursor(&mut self, x: f64, y: f64, model: &AppModel) -> (usize, usize) {
        let line_height = self.line_metrics.new_line_size.ceil() as f64;
        geometry::pixel_to_cursor(x, y, self.char_width, line_height, model)
    }

    /// Convert pixel coordinates to line and visual column (screen position).
    /// Used for rectangle selection where the raw visual column is needed,
    /// independent of any specific line's text content.
    pub fn pixel_to_line_and_visual_column(
        &mut self,
        x: f64,
        y: f64,
        model: &AppModel,
    ) -> (usize, usize) {
        let line_height = self.line_metrics.new_line_size.ceil() as f64;
        geometry::pixel_to_line_and_visual_column(x, y, self.char_width, line_height, model)
    }

    /// Check if a y-coordinate is within the status bar region.
    /// Delegates to geometry module for the actual calculation.
    pub fn is_in_status_bar(&self, y: f64) -> bool {
        let line_height = self.line_metrics.new_line_size.ceil() as usize;
        geometry::is_in_status_bar(y, self.height, line_height)
    }

    /// Hit-test a CSV cell given window coordinates.
    /// Returns None if the click is outside the data grid or editor is not in CSV mode.
    pub fn pixel_to_csv_cell(
        &self,
        x: f64,
        y: f64,
        model: &AppModel,
    ) -> Option<crate::csv::CellPosition> {
        let group = model.editor_area.focused_group()?;
        let editor = model.editor_area.focused_editor()?;
        let csv = editor.view_mode.as_csv()?;

        let line_height = self.line_metrics.new_line_size.ceil() as usize;

        crate::csv::render::pixel_to_csv_cell(
            csv,
            &group.rect,
            x,
            y,
            line_height,
            self.char_width,
            model.metrics.tab_bar_height,
        )
    }
}

// ---------------------------------------------------------------------------
// HTML text extraction for native preview
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
enum HtmlTextStyle {
    Normal,
    Heading,
    Link,
    Muted,
}

struct HtmlTextLine {
    text: String,
    style: HtmlTextStyle,
}

/// Extract visible text lines from HTML source by stripping tags.
/// Produces styled lines for headings (`<h1>`–`<h6>`), links (`<a>`),
/// and regular paragraph/list text. Skips `<script>`, `<style>`, and `<head>` content.
fn extract_html_text_lines(html: &str) -> Vec<HtmlTextLine> {
    let mut lines = Vec::new();
    let mut current_text = String::new();
    let mut current_style = HtmlTextStyle::Normal;
    let mut skip_content = false;
    let mut in_body = false;
    let mut has_body_tag = false;

    // Check if document has a <body> tag; if not, treat everything as body
    let lower = html.to_lowercase();
    if lower.contains("<body") {
        has_body_tag = true;
    } else {
        in_body = true;
    }

    let mut chars = html.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '<' {
            // Collect tag name
            let mut tag = String::new();
            while let Some(&c) = chars.peek() {
                if c == '>' || c == ' ' {
                    break;
                }
                tag.push(c);
                chars.next();
            }
            // Skip to end of tag
            while let Some(&c) = chars.peek() {
                if c == '>' {
                    chars.next();
                    break;
                }
                chars.next();
            }

            let tag_lower = tag.to_lowercase();

            match tag_lower.as_str() {
                "body" => {
                    in_body = true;
                }
                "/body" => {
                    in_body = false;
                }
                "script" | "style" => {
                    skip_content = true;
                }
                "/script" | "/style" => {
                    skip_content = false;
                }
                "head" if has_body_tag => {
                    skip_content = true;
                }
                "/head" => {
                    skip_content = false;
                }
                "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                    flush_line(&mut current_text, current_style, &mut lines);
                    current_style = HtmlTextStyle::Heading;
                }
                "/h1" | "/h2" | "/h3" | "/h4" | "/h5" | "/h6" => {
                    flush_line(&mut current_text, current_style, &mut lines);
                    current_style = HtmlTextStyle::Normal;
                }
                "a" => {
                    current_style = HtmlTextStyle::Link;
                }
                "/a" => {
                    current_style = HtmlTextStyle::Normal;
                }
                "br" | "br/" => {
                    flush_line(&mut current_text, current_style, &mut lines);
                }
                "p" | "/p" | "div" | "/div" | "li" | "tr" | "hr" | "hr/" => {
                    flush_line(&mut current_text, current_style, &mut lines);
                    if tag_lower == "li" {
                        current_text.push_str("• ");
                    } else if tag_lower == "hr" || tag_lower == "hr/" {
                        lines.push(HtmlTextLine {
                            text: "───────────────────────────────".to_string(),
                            style: HtmlTextStyle::Muted,
                        });
                    }
                }
                _ => {}
            }
        } else if !skip_content && in_body {
            // Handle HTML entities
            if ch == '&' {
                let mut entity = String::new();
                while let Some(&c) = chars.peek() {
                    if c == ';' {
                        chars.next();
                        break;
                    }
                    if c == '<' || c == ' ' || entity.len() > 8 {
                        break;
                    }
                    entity.push(c);
                    chars.next();
                }
                match entity.as_str() {
                    "amp" => current_text.push('&'),
                    "lt" => current_text.push('<'),
                    "gt" => current_text.push('>'),
                    "quot" => current_text.push('"'),
                    "apos" => current_text.push('\''),
                    "nbsp" => current_text.push(' '),
                    _ => {
                        current_text.push('&');
                        current_text.push_str(&entity);
                    }
                }
            } else if ch == '\n' {
                // Collapse whitespace - newlines become spaces unless we're between block elements
                if !current_text.is_empty() && !current_text.ends_with(' ') {
                    current_text.push(' ');
                }
            } else {
                current_text.push(ch);
            }
        }
    }

    flush_line(&mut current_text, current_style, &mut lines);
    lines
}

fn flush_line(text: &mut String, style: HtmlTextStyle, lines: &mut Vec<HtmlTextLine>) {
    let trimmed = text.trim().to_string();
    if !trimmed.is_empty() {
        lines.push(HtmlTextLine {
            text: trimmed,
            style,
        });
    }
    text.clear();
}
