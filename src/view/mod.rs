//! View module - rendering code extracted from main.rs
//!
//! Contains the Renderer struct and all rendering-related functionality.

pub mod frame;

pub use frame::{Frame, TextPainter};

use anyhow::Result;
use fontdue::{Font, FontSettings, LineMetrics, Metrics};
use softbuffer::Surface;
use std::collections::HashMap;
use std::num::NonZeroU32;
use std::rc::Rc;
use winit::window::Window;

use token::model::editor_area::{EditorGroup, GroupId, Rect, SplitterBar, Tab};
use token::model::{gutter_border_x, text_start_x, AppModel};

pub type GlyphCacheKey = (char, u32);

/// Get the display title for a tab.
/// Centralizes the logic for determining what text to show in the tab bar.
fn tab_title(model: &AppModel, tab: &Tab) -> String {
    let editor = match model.editor_area.editors.get(&tab.editor_id) {
        Some(e) => e,
        None => return "Untitled".to_string(),
    };
    let doc_id = match editor.document_id {
        Some(id) => id,
        None => return "Untitled".to_string(),
    };
    let document = match model.editor_area.documents.get(&doc_id) {
        Some(d) => d,
        None => return "Untitled".to_string(),
    };
    document.display_name()
}
pub type GlyphCache = HashMap<GlyphCacheKey, (Metrics, Vec<u8>)>;

pub const TAB_BAR_HEIGHT: usize = 28;
pub const TABULATOR_WIDTH: usize = 4;

pub fn expand_tabs_for_display(text: &str) -> String {
    let mut result = String::with_capacity(text.len() * 2);
    let mut visual_col = 0;

    for ch in text.chars() {
        if ch == '\t' {
            let spaces = TABULATOR_WIDTH - (visual_col % TABULATOR_WIDTH);
            for _ in 0..spaces {
                result.push(' ');
            }
            visual_col += spaces;
        } else {
            result.push(ch);
            visual_col += 1;
        }
    }

    result
}

pub fn char_col_to_visual_col(text: &str, char_col: usize) -> usize {
    let mut visual_col = 0;
    for (i, ch) in text.chars().enumerate() {
        if i >= char_col {
            break;
        }
        if ch == '\t' {
            visual_col += TABULATOR_WIDTH - (visual_col % TABULATOR_WIDTH);
        } else {
            visual_col += 1;
        }
    }
    visual_col
}

pub fn visual_col_to_char_col(text: &str, visual_col: usize) -> usize {
    let mut current_visual = 0;
    let mut char_col = 0;

    for ch in text.chars() {
        if current_visual >= visual_col {
            return char_col;
        }

        if ch == '\t' {
            let tab_width = TABULATOR_WIDTH - (current_visual % TABULATOR_WIDTH);
            current_visual += tab_width;
        } else {
            current_visual += 1;
        }
        char_col += 1;
    }

    char_col
}

pub struct Renderer {
    font: Font,
    surface: Surface<Rc<Window>, Rc<Window>>,
    width: u32,
    height: u32,
    font_size: f32,
    line_metrics: LineMetrics,
    glyph_cache: GlyphCache,
    char_width: f32,
}

impl Renderer {
    pub fn new(window: Rc<Window>, context: &softbuffer::Context<Rc<Window>>) -> Result<Self> {
        let scale_factor = window.scale_factor();
        let (width, height) = {
            let size = window.inner_size();
            (size.width, size.height)
        };

        let surface = Surface::new(context, Rc::clone(&window))
            .map_err(|e| anyhow::anyhow!("Failed to create surface: {}", e))?;

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

        Ok(Self {
            font,
            surface,
            width,
            height,
            font_size,
            line_metrics,
            glyph_cache: HashMap::new(),
            char_width,
        })
    }

    pub fn char_width(&self) -> f32 {
        self.char_width
    }

    #[allow(dead_code)]
    pub fn font(&self) -> &Font {
        &self.font
    }

    #[allow(dead_code)]
    pub fn font_size(&self) -> f32 {
        self.font_size
    }

    #[allow(dead_code)]
    pub fn line_height(&self) -> usize {
        self.line_metrics.new_line_size.ceil() as usize
    }

    #[allow(dead_code)]
    pub fn ascent(&self) -> f32 {
        self.line_metrics.ascent
    }

    #[allow(dead_code)]
    pub fn line_metrics(&self) -> &LineMetrics {
        &self.line_metrics
    }

    #[allow(dead_code)]
    pub fn glyph_cache_mut(&mut self) -> &mut GlyphCache {
        &mut self.glyph_cache
    }

    #[allow(dead_code)]
    pub fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    fn render_all_groups_static(
        frame: &mut Frame,
        painter: &mut TextPainter,
        model: &AppModel,
        splitters: &[SplitterBar],
        line_height: usize,
        char_width: f32,
    ) {
        for (&group_id, group) in &model.editor_area.groups {
            let is_focused = group_id == model.editor_area.focused_group_id;
            Self::render_editor_group_static(
                frame,
                painter,
                model,
                group_id,
                group.rect,
                is_focused,
                line_height,
                char_width,
            );
        }

        Self::render_splitters_static(frame, splitters, model);
    }

    fn render_editor_group_static(
        frame: &mut Frame,
        painter: &mut TextPainter,
        model: &AppModel,
        group_id: GroupId,
        group_rect: Rect,
        is_focused: bool,
        line_height: usize,
        char_width: f32,
    ) {
        let group = match model.editor_area.groups.get(&group_id) {
            Some(g) => g,
            None => return,
        };

        let editor_id = match group.active_editor_id() {
            Some(id) => id,
            None => return,
        };

        let editor = match model.editor_area.editors.get(&editor_id) {
            Some(e) => e,
            None => return,
        };

        let doc_id = match editor.document_id {
            Some(id) => id,
            None => return,
        };

        let document = match model.editor_area.documents.get(&doc_id) {
            Some(d) => d,
            None => return,
        };

        let rect_x = group_rect.x as usize;
        let rect_y = group_rect.y as usize;
        let rect_w = group_rect.width as usize;
        let rect_h = group_rect.height as usize;

        Self::render_tab_bar_static(
            frame, painter, model, group, rect_x, rect_y, rect_w, char_width,
        );

        let content_y = rect_y + TAB_BAR_HEIGHT;
        let content_h = rect_h.saturating_sub(TAB_BAR_HEIGHT);

        let text_start_x_offset = text_start_x(char_width).round() as usize;
        let group_text_start_x = rect_x + text_start_x_offset;

        let visible_lines = content_h / line_height;
        let end_line = (editor.viewport.top_line + visible_lines).min(document.buffer.len_lines());

        // Highlight primary cursor line only
        let current_line_color = model.theme.editor.current_line_background.to_argb_u32();
        if editor.active_cursor().line >= editor.viewport.top_line
            && editor.active_cursor().line < end_line
        {
            let screen_line = editor.active_cursor().line - editor.viewport.top_line;
            let highlight_y = content_y + screen_line * line_height;
            let highlight_h = line_height.min(content_y + content_h - highlight_y);
            frame.fill_rect_px(rect_x, highlight_y, rect_w, highlight_h, current_line_color);
        }

        // Render ALL selections (primary + secondary cursors)
        let selection_color = model.theme.editor.selection_background.to_argb_u32();
        for selection in &editor.selections {
            if selection.is_empty() {
                continue;
            }

            let sel_start = selection.start();
            let sel_end = selection.end();

            for doc_line in editor.viewport.top_line..end_line {
                if doc_line < sel_start.line || doc_line > sel_end.line {
                    continue;
                }

                let screen_line = doc_line - editor.viewport.top_line;
                let y_start = content_y + screen_line * line_height;
                let y_end = (y_start + line_height).min(content_y + content_h);

                let line_len = document.line_length(doc_line);

                let line_text = document.get_line(doc_line).unwrap_or_default();
                let line_text_trimmed = if line_text.ends_with('\n') {
                    &line_text[..line_text.len() - 1]
                } else {
                    &line_text
                };

                let start_col = if doc_line == sel_start.line {
                    sel_start.column
                } else {
                    0
                };
                let end_col = if doc_line == sel_end.line {
                    sel_end.column
                } else {
                    line_len
                };

                let visual_start_col = char_col_to_visual_col(line_text_trimmed, start_col);
                let visual_end_col = char_col_to_visual_col(line_text_trimmed, end_col);

                let visible_start_col =
                    visual_start_col.saturating_sub(editor.viewport.left_column);
                let visible_end_col = visual_end_col.saturating_sub(editor.viewport.left_column);

                let x_start =
                    group_text_start_x + (visible_start_col as f32 * char_width).round() as usize;
                let x_end = (group_text_start_x
                    + (visible_end_col as f32 * char_width).round() as usize)
                    .min(rect_x + rect_w);

                frame.fill_rect_px(
                    x_start,
                    y_start,
                    x_end.saturating_sub(x_start),
                    y_end.saturating_sub(y_start),
                    selection_color,
                );
            }
        }

        let line_num_color = model.theme.gutter.foreground.to_argb_u32();
        let line_num_active_color = model.theme.gutter.foreground_active.to_argb_u32();
        let text_color = model.theme.editor.foreground.to_argb_u32();

        for (screen_line, doc_line) in (editor.viewport.top_line..end_line).enumerate() {
            if let Some(line_text) = document.get_line(doc_line) {
                let y = content_y + screen_line * line_height;
                if y >= content_y + content_h {
                    break;
                }

                let line_num_str = format!("{:4} ", doc_line + 1);
                let line_color = if doc_line == editor.active_cursor().line {
                    line_num_active_color
                } else {
                    line_num_color
                };
                painter.draw(frame, rect_x, y, &line_num_str, line_color);

                let visible_text = if line_text.ends_with('\n') {
                    &line_text[..line_text.len() - 1]
                } else {
                    &line_text
                };

                let expanded_text = expand_tabs_for_display(visible_text);

                let max_chars =
                    ((rect_w as f32 - text_start_x_offset as f32) / char_width).floor() as usize;
                let display_text: String = expanded_text
                    .chars()
                    .skip(editor.viewport.left_column)
                    .take(max_chars)
                    .collect();

                painter.draw(frame, group_text_start_x, y, &display_text, text_color);
            }
        }

        // Draw cursors
        if model.ui.cursor_visible {
            let actual_visible_columns =
                ((rect_w as f32 - text_start_x_offset as f32) / char_width).floor() as usize;
            let primary_cursor_color = model.theme.editor.cursor_color.to_argb_u32();
            let secondary_cursor_color = model.theme.editor.secondary_cursor_color.to_argb_u32();

            for (idx, cursor) in editor.cursors.iter().enumerate() {
                let cursor_in_vertical_view = cursor.line >= editor.viewport.top_line
                    && cursor.line < editor.viewport.top_line + visible_lines;

                let line_text = document.get_line(cursor.line).unwrap_or_default();
                let line_text_trimmed = if line_text.ends_with('\n') {
                    &line_text[..line_text.len() - 1]
                } else {
                    &line_text
                };
                let visual_cursor_col = char_col_to_visual_col(line_text_trimmed, cursor.column);

                let cursor_in_horizontal_view = visual_cursor_col >= editor.viewport.left_column
                    && visual_cursor_col < editor.viewport.left_column + actual_visible_columns;

                if cursor_in_vertical_view && cursor_in_horizontal_view {
                    let screen_line = cursor.line - editor.viewport.top_line;
                    let cursor_visual_column = visual_cursor_col - editor.viewport.left_column;
                    let x = (group_text_start_x as f32 + cursor_visual_column as f32 * char_width)
                        .round() as usize;
                    let y = content_y + screen_line * line_height;

                    // TODO: check if this will "always" be the primary cursor, we also have "active_cursor", which is unclear the difference  between them
                    let cursor_color = if idx == 0 {
                        primary_cursor_color
                    } else {
                        secondary_cursor_color
                    };

                    // Cursor: 2px wide, line_height - 2 tall, offset by 1px from top
                    frame.fill_rect_px(x, y + 1, 2, line_height.saturating_sub(2), cursor_color);
                }
            }
        }

        // Gutter border
        let gutter_border_color = model.theme.gutter.border_color.to_argb_u32();
        let border_x = rect_x + gutter_border_x(char_width).round() as usize;
        frame.fill_rect_px(border_x, content_y, 1, content_h, gutter_border_color);

        // Dim non-focused groups when multiple groups exist (4% black overlay)
        if !is_focused && model.editor_area.groups.len() > 1 {
            let dim_color = 0x0A000000_u32; // 4% opacity black (alpha = 10/255 ≈ 4%)
            frame.blend_rect(group_rect, dim_color);
        }
    }

    fn render_tab_bar_static(
        frame: &mut Frame,
        painter: &mut TextPainter,
        model: &AppModel,
        group: &EditorGroup,
        rect_x: usize,
        rect_y: usize,
        rect_w: usize,
        char_width: f32,
    ) {
        let tab_bar_bg = model.theme.tab_bar.background.to_argb_u32();
        frame.fill_rect_px(rect_x, rect_y, rect_w, TAB_BAR_HEIGHT, tab_bar_bg);

        let border_color = model.theme.tab_bar.border.to_argb_u32();
        let border_y = (rect_y + TAB_BAR_HEIGHT).saturating_sub(1);
        frame.fill_rect_px(rect_x, border_y, rect_w, 1, border_color);

        let mut tab_x = rect_x + 4;
        let tab_height = TAB_BAR_HEIGHT - 4;
        let tab_y = rect_y + 2;

        for (idx, tab) in group.tabs.iter().enumerate() {
            let is_active = idx == group.active_tab_index;
            let display_name = tab_title(model, tab);
            let tab_width = (display_name.len() as f32 * char_width).round() as usize + 16;

            let (bg_color, fg_color) = if is_active {
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

            let actual_tab_width = tab_width.min(rect_x + rect_w - tab_x);
            frame.fill_rect_px(tab_x, tab_y, actual_tab_width, tab_height, bg_color);

            let text_x = tab_x + 8;
            let text_y = tab_y + 4;
            painter.draw(frame, text_x, text_y, &display_name, fg_color);

            tab_x += tab_width + 2;
            if tab_x >= rect_x + rect_w {
                break;
            }
        }
    }

    fn render_splitters_static(frame: &mut Frame, splitters: &[SplitterBar], model: &AppModel) {
        let splitter_color = model.theme.splitter.background.to_argb_u32();

        for splitter in splitters {
            frame.fill_rect(splitter.rect, splitter_color);
        }
    }

    pub fn render(
        &mut self,
        model: &mut AppModel,
        perf: &mut crate::runtime::perf::PerfStats,
    ) -> Result<()> {
        perf.reset_frame_stats();

        if self.width != model.window_size.0 || self.height != model.window_size.1 {
            self.width = model.window_size.0;
            self.height = model.window_size.1;
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
        let width = self.width;
        let height = self.height;

        let status_bar_height = line_height;
        let available_rect = Rect::new(
            0.0,
            0.0,
            width as f32,
            (height as usize).saturating_sub(status_bar_height) as f32,
        );
        let splitters = model.editor_area.compute_layout(available_rect);

        let mut buffer = self
            .surface
            .buffer_mut()
            .map_err(|e| anyhow::anyhow!("Failed to get surface buffer: {}", e))?;

        // Create Frame wrapper for cleaner drawing API
        // Note: We use Frame for new code; legacy code still uses raw buffer slices
        let width_usize = width as usize;
        let height_usize = height as usize;

        {
            let _timer = perf.time_clear();
            let bg_color = model.theme.editor.background.to_argb_u32();
            let mut frame = Frame::new(&mut buffer, width_usize, height_usize);
            frame.clear(bg_color);
        }

        {
            let _timer = perf.time_text();
            let mut frame = Frame::new(&mut buffer, width_usize, height_usize);
            let mut painter =
                TextPainter::new(&self.font, &mut self.glyph_cache, font_size, ascent);
            Self::render_all_groups_static(
                &mut frame,
                &mut painter,
                model,
                &splitters,
                line_height,
                char_width,
            );
        }

        {
            let _timer = perf.time_status_bar();
            let status_bar_bg = model.theme.status_bar.background.to_argb_u32();
            let status_bar_fg = model.theme.status_bar.foreground.to_argb_u32();
            let status_y = (height as usize).saturating_sub(status_bar_height);

            // Use Frame for status bar background
            {
                let mut frame = Frame::new(&mut buffer, width_usize, height_usize);
                frame.fill_rect_px(0, status_y, width_usize, status_bar_height, status_bar_bg);
            }

            let available_chars = (width as f32 / char_width).floor() as usize;
            let layout = model.ui.status_bar.layout(available_chars);

            // Use TextPainter for status bar text
            {
                let mut frame = Frame::new(&mut buffer, width_usize, height_usize);
                let mut painter =
                    TextPainter::new(&self.font, &mut self.glyph_cache, font_size, ascent);

                for seg in &layout.left {
                    let x_px = (seg.x as f32 * char_width).round() as usize;
                    painter.draw(&mut frame, x_px, status_y + 2, &seg.text, status_bar_fg);
                }

                for seg in &layout.right {
                    let x_px = (seg.x as f32 * char_width).round() as usize;
                    painter.draw(&mut frame, x_px, status_y + 2, &seg.text, status_bar_fg);
                }

                // Draw separators
                let separator_color = model
                    .theme
                    .status_bar
                    .foreground
                    .with_alpha(100)
                    .to_argb_u32();
                for &sep_char_x in &layout.separator_positions {
                    let x_px = (sep_char_x as f32 * char_width).round() as usize;
                    frame.fill_rect_px(x_px, status_y, 1, status_bar_height, separator_color);
                }
            }
        }

        #[cfg(debug_assertions)]
        if perf.should_show_overlay() {
            let mut frame = Frame::new(&mut buffer, width_usize, height_usize);
            let mut painter =
                TextPainter::new(&self.font, &mut self.glyph_cache, font_size, ascent);
            crate::runtime::perf::render_perf_overlay(
                &mut frame,
                &mut painter,
                perf,
                &model.theme,
                line_height,
            );
        }

        #[cfg(debug_assertions)]
        if let Some(ref overlay) = model.debug_overlay {
            if overlay.visible {
                let lines = overlay.render_lines(model);
                if !lines.is_empty() {
                    let mut frame = Frame::new(&mut buffer, width_usize, height_usize);
                    let mut painter =
                        TextPainter::new(&self.font, &mut self.glyph_cache, font_size, ascent);

                    // Calculate dimensions
                    let max_line_len = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0);
                    let overlay_width = (max_line_len as f32 * char_width).ceil() as usize + 20;
                    let overlay_height = lines.len() * line_height + 10;

                    // Position in top-right corner (perf overlay is top-left)
                    let overlay_x = width_usize.saturating_sub(overlay_width + 10);
                    let overlay_y = 10;

                    // Render semi-transparent background
                    let bg_color = model.theme.overlay.background.to_argb_u32();
                    let fg_color = model.theme.overlay.foreground.to_argb_u32();

                    for py in overlay_y..(overlay_y + overlay_height).min(height_usize) {
                        for px in overlay_x..(overlay_x + overlay_width).min(width_usize) {
                            frame.blend_pixel(px, py, bg_color);
                        }
                    }

                    // Render text lines
                    for (i, line) in lines.iter().enumerate() {
                        let text_x = overlay_x + 10;
                        let text_y = overlay_y + 5 + i * line_height;
                        painter.draw(&mut frame, text_x, text_y, line, fg_color);
                    }
                }
            }
        }

        {
            let _timer = perf.time_present();
            buffer
                .present()
                .map_err(|e| anyhow::anyhow!("Failed to present buffer: {}", e))?;
        }

        Ok(())
    }

    #[allow(dead_code)]
    fn get_char_width(&mut self) -> f32 {
        let key = ('m', self.font_size.to_bits());
        if let Some((metrics, _)) = self.glyph_cache.get(&key) {
            metrics.advance_width
        } else {
            let (metrics, bitmap) = self.font.rasterize('m', self.font_size);
            let width = metrics.advance_width;
            self.glyph_cache.insert(key, (metrics, bitmap));
            width
        }
    }

    pub fn pixel_to_cursor(&mut self, x: f64, y: f64, model: &AppModel) -> (usize, usize) {
        let line_height = self.line_metrics.new_line_size.ceil() as f64;
        let char_width = self.char_width as f64;
        let text_x = text_start_x(self.char_width).round() as f64;

        let text_start_y = TAB_BAR_HEIGHT as f64;
        let adjusted_y = (y - text_start_y).max(0.0);
        let visual_line = (adjusted_y / line_height).floor() as usize;
        let line = model.editor().viewport.top_line + visual_line;
        let line = line.min(model.document().buffer.len_lines().saturating_sub(1));

        let x_offset = x - text_x;
        let visual_column = if x_offset > 0.0 {
            model.editor().viewport.left_column + (x_offset / char_width).round() as usize
        } else {
            model.editor().viewport.left_column
        };

        let line_text = model.document().get_line(line).unwrap_or_default();
        let line_text_trimmed = if line_text.ends_with('\n') {
            &line_text[..line_text.len() - 1]
        } else {
            &line_text
        };
        let column = visual_col_to_char_col(line_text_trimmed, visual_column);

        let line_len = model.document().line_length(line);
        let column = column.min(line_len);

        (line, column)
    }

    pub fn is_in_status_bar(&self, y: f64) -> bool {
        let line_height = self.line_metrics.new_line_size.ceil() as f64;
        let status_bar_top = self.height as f64 - line_height;
        y >= status_bar_top
    }

    pub fn is_in_tab_bar(&self, y: f64) -> bool {
        y < TAB_BAR_HEIGHT as f64
    }

    /// Returns the tab index at the given x position within a group's tab bar.
    /// Returns None if the click is not on a tab.
    pub fn tab_at_position(&self, x: f64, model: &AppModel, group: &EditorGroup) -> Option<usize> {
        let mut tab_x = 4.0; // Initial padding

        for (idx, tab) in group.tabs.iter().enumerate() {
            let title = tab_title(model, tab);
            let tab_width = (title.len() as f32 * self.char_width).round() as f64 + 16.0;

            if x >= tab_x && x < tab_x + tab_width {
                return Some(idx);
            }

            tab_x += tab_width + 2.0; // tab width + gap
        }

        None
    }
}
