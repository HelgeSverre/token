//! View module - rendering code extracted from main.rs
//!
//! Contains the Renderer struct and all rendering-related functionality.

use anyhow::Result;
use fontdue::{Font, FontSettings, LineMetrics, Metrics};
use softbuffer::Surface;
use std::collections::HashMap;
use std::num::NonZeroU32;
use std::rc::Rc;
use winit::window::Window;

use token::model::editor_area::{EditorGroup, GroupId, Rect, SplitterBar};
use token::model::{gutter_border_x, text_start_x, AppModel};

pub const TAB_BAR_HEIGHT: usize = 28;

pub type GlyphCacheKey = (char, u32);
pub type GlyphCache = HashMap<GlyphCacheKey, (Metrics, Vec<u8>)>;

pub const TAB_WIDTH: usize = 4;

pub fn expand_tabs_for_display(text: &str) -> String {
    let mut result = String::with_capacity(text.len() * 2);
    let mut visual_col = 0;

    for ch in text.chars() {
        if ch == '\t' {
            let spaces = TAB_WIDTH - (visual_col % TAB_WIDTH);
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
            visual_col += TAB_WIDTH - (visual_col % TAB_WIDTH);
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
            let tab_width = TAB_WIDTH - (current_visual % TAB_WIDTH);
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
            include_bytes!("../assets/JetBrainsMono.ttf") as &[u8],
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

    #[allow(clippy::too_many_arguments)]
    fn render_all_groups_static(
        buffer: &mut [u32],
        model: &AppModel,
        splitters: &[SplitterBar],
        font: &Font,
        glyph_cache: &mut GlyphCache,
        line_height: usize,
        font_size: f32,
        ascent: f32,
        char_width: f32,
        width: u32,
        height: u32,
    ) {
        for (&group_id, group) in &model.editor_area.groups {
            let is_focused = group_id == model.editor_area.focused_group_id;
            Self::render_editor_group_static(
                buffer,
                model,
                group_id,
                group.rect,
                is_focused,
                font,
                glyph_cache,
                font_size,
                ascent,
                line_height,
                char_width,
                width,
                height,
            );
        }

        Self::render_splitters_static(buffer, splitters, model, width, height);
    }

    #[allow(clippy::too_many_arguments)]
    fn render_editor_group_static(
        buffer: &mut [u32],
        model: &AppModel,
        group_id: GroupId,
        group_rect: Rect,
        is_focused: bool,
        font: &Font,
        glyph_cache: &mut GlyphCache,
        font_size: f32,
        ascent: f32,
        line_height: usize,
        char_width: f32,
        width: u32,
        height: u32,
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
            buffer,
            model,
            group,
            rect_x,
            rect_y,
            rect_w,
            font,
            glyph_cache,
            font_size,
            ascent,
            char_width,
            width,
            height,
        );

        let content_y = rect_y + TAB_BAR_HEIGHT;
        let content_h = rect_h.saturating_sub(TAB_BAR_HEIGHT);

        let text_start_x_offset = text_start_x(char_width).round() as usize;
        let group_text_start_x = rect_x + text_start_x_offset;

        let visible_lines = content_h / line_height;
        let end_line = (editor.viewport.top_line + visible_lines).min(document.buffer.len_lines());

        // Highlight primary cursor line only
        let current_line_color = model.theme.editor.current_line_background.to_argb_u32();
        if editor.active_cursor().line >= editor.viewport.top_line && editor.active_cursor().line < end_line {
            let screen_line = editor.active_cursor().line - editor.viewport.top_line;
            let highlight_y = content_y + screen_line * line_height;

            for py in highlight_y..(highlight_y + line_height).min(content_y + content_h) {
                for px in rect_x..(rect_x + rect_w).min(width as usize) {
                    if py < height as usize {
                        buffer[py * width as usize + px] = current_line_color;
                    }
                }
            }
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

                for py in y_start..y_end {
                    for px in x_start..x_end {
                        if py < height as usize && px < width as usize {
                            buffer[py * width as usize + px] = selection_color;
                        }
                    }
                }
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
                draw_text(
                    buffer,
                    font,
                    glyph_cache,
                    font_size,
                    ascent,
                    width,
                    height,
                    rect_x,
                    y,
                    &line_num_str,
                    line_color,
                );

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

                draw_text(
                    buffer,
                    font,
                    glyph_cache,
                    font_size,
                    ascent,
                    width,
                    height,
                    group_text_start_x,
                    y,
                    &display_text,
                    text_color,
                );
            }
        }

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

                    let cursor_color = if idx == 0 {
                        primary_cursor_color
                    } else {
                        secondary_cursor_color
                    };

                    for dy in 0..(line_height - 2) {
                        for dx in 0..2 {
                            let px = x + dx;
                            let py = y + dy + 1;
                            if px < (rect_x + rect_w).min(width as usize)
                                && py < (content_y + content_h).min(height as usize)
                            {
                                buffer[py * width as usize + px] = cursor_color;
                            }
                        }
                    }
                }
            }
        }

        let gutter_border_color = model.theme.gutter.border_color.to_argb_u32();
        let border_x = rect_x + gutter_border_x(char_width).round() as usize;
        if border_x < (rect_x + rect_w).min(width as usize) {
            for py in content_y..(content_y + content_h).min(height as usize) {
                buffer[py * width as usize + border_x] = gutter_border_color;
            }
        }

        if is_focused && model.editor_area.groups.len() > 1 {
            let focus_color = model.theme.editor.cursor_color.to_argb_u32();
            let border_width = 2;
            for dy in 0..border_width {
                for px in rect_x..(rect_x + rect_w).min(width as usize) {
                    let py = rect_y + dy;
                    if py < height as usize {
                        buffer[py * width as usize + px] = focus_color;
                    }
                }
                for px in rect_x..(rect_x + rect_w).min(width as usize) {
                    let py = (rect_y + rect_h).saturating_sub(border_width) + dy;
                    if py < height as usize {
                        buffer[py * width as usize + px] = focus_color;
                    }
                }
            }
            for dy in rect_y..(rect_y + rect_h).min(height as usize) {
                for dx in 0..border_width {
                    let px = rect_x + dx;
                    if px < width as usize {
                        buffer[dy * width as usize + px] = focus_color;
                    }
                    let px = (rect_x + rect_w).saturating_sub(border_width) + dx;
                    if px < width as usize {
                        buffer[dy * width as usize + px] = focus_color;
                    }
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn render_tab_bar_static(
        buffer: &mut [u32],
        model: &AppModel,
        group: &EditorGroup,
        rect_x: usize,
        rect_y: usize,
        rect_w: usize,
        font: &Font,
        glyph_cache: &mut GlyphCache,
        font_size: f32,
        ascent: f32,
        char_width: f32,
        width: u32,
        height: u32,
    ) {
        let tab_bar_bg = model.theme.tab_bar.background.to_argb_u32();
        for py in rect_y..(rect_y + TAB_BAR_HEIGHT).min(height as usize) {
            for px in rect_x..(rect_x + rect_w).min(width as usize) {
                buffer[py * width as usize + px] = tab_bar_bg;
            }
        }

        let border_color = model.theme.tab_bar.border.to_argb_u32();
        let border_y = (rect_y + TAB_BAR_HEIGHT).saturating_sub(1);
        if border_y < height as usize {
            for px in rect_x..(rect_x + rect_w).min(width as usize) {
                buffer[border_y * width as usize + px] = border_color;
            }
        }

        let mut tab_x = rect_x + 4;
        let tab_height = TAB_BAR_HEIGHT - 4;
        let tab_y = rect_y + 2;

        for (idx, tab) in group.tabs.iter().enumerate() {
            let is_active = idx == group.active_tab_index;

            let editor = model.editor_area.editors.get(&tab.editor_id);
            let doc_id = editor.and_then(|e| e.document_id);
            let document = doc_id.and_then(|id| model.editor_area.documents.get(&id));

            let filename = document
                .and_then(|d| d.file_path.as_ref())
                .and_then(|p| p.file_name())
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "Untitled".to_string());

            let is_modified = document.map(|d| d.is_modified).unwrap_or(false);
            let display_name = if is_modified {
                format!("{}", filename)
            } else {
                filename
            };

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

            for py in tab_y..(tab_y + tab_height).min(height as usize) {
                for px in tab_x..(tab_x + tab_width).min(rect_x + rect_w).min(width as usize) {
                    buffer[py * width as usize + px] = bg_color;
                }
            }

            let text_x = tab_x + 8;
            let text_y = tab_y + 4;
            draw_text(
                buffer,
                font,
                glyph_cache,
                font_size,
                ascent,
                width,
                height,
                text_x,
                text_y,
                &display_name,
                fg_color,
            );

            tab_x += tab_width + 2;
            if tab_x >= rect_x + rect_w {
                break;
            }
        }
    }

    fn render_splitters_static(
        buffer: &mut [u32],
        splitters: &[SplitterBar],
        model: &AppModel,
        width: u32,
        height: u32,
    ) {
        let splitter_color = model.theme.splitter.background.to_argb_u32();

        for splitter in splitters {
            let sx = splitter.rect.x as usize;
            let sy = splitter.rect.y as usize;
            let sw = splitter.rect.width as usize;
            let sh = splitter.rect.height as usize;

            for py in sy..(sy + sh).min(height as usize) {
                for px in sx..(sx + sw).min(width as usize) {
                    buffer[py * width as usize + px] = splitter_color;
                }
            }
        }
    }

    #[cfg(not(debug_assertions))]
    pub fn render(&mut self, model: &mut AppModel, _perf: ()) -> Result<()> {
        self.render_impl(model)
    }

    #[cfg(debug_assertions)]
    pub fn render(
        &mut self,
        model: &mut AppModel,
        perf: &mut super::perf::PerfStats,
    ) -> Result<()> {
        self.render_impl_with_perf(model, perf)
    }

    #[cfg(not(debug_assertions))]
    fn render_impl(&mut self, model: &mut AppModel) -> Result<()> {
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

        let bg_color = model.theme.editor.background.to_argb_u32();
        buffer.fill(bg_color);

        Self::render_all_groups_static(
            &mut buffer,
            model,
            &splitters,
            &self.font,
            &mut self.glyph_cache,
            line_height,
            font_size,
            ascent,
            char_width,
            width,
            height,
        );

        let status_bar_bg = model.theme.status_bar.background.to_argb_u32();
        let status_bar_fg = model.theme.status_bar.foreground.to_argb_u32();
        let status_y = (height as usize).saturating_sub(status_bar_height);
        for py in status_y..height as usize {
            for px in 0..width as usize {
                buffer[py * width as usize + px] = status_bar_bg;
            }
        }

        let available_chars = (width as f32 / char_width).floor() as usize;
        let layout = model.ui.status_bar.layout(available_chars);

        for seg in &layout.left {
            let x_px = (seg.x as f32 * char_width).round() as usize;
            draw_text(
                &mut buffer,
                &self.font,
                &mut self.glyph_cache,
                font_size,
                ascent,
                width,
                height,
                x_px,
                status_y + 2,
                &seg.text,
                status_bar_fg,
            );
        }

        for seg in &layout.right {
            let x_px = (seg.x as f32 * char_width).round() as usize;
            draw_text(
                &mut buffer,
                &self.font,
                &mut self.glyph_cache,
                font_size,
                ascent,
                width,
                height,
                x_px,
                status_y + 2,
                &seg.text,
                status_bar_fg,
            );
        }

        let separator_color = model
            .theme
            .status_bar
            .foreground
            .with_alpha(100)
            .to_argb_u32();
        for &sep_char_x in &layout.separator_positions {
            let x_px = (sep_char_x as f32 * char_width).round() as usize;
            if x_px < width as usize {
                for py in status_y..height as usize {
                    buffer[py * width as usize + x_px] = separator_color;
                }
            }
        }

        buffer
            .present()
            .map_err(|e| anyhow::anyhow!("Failed to present buffer: {}", e))?;
        Ok(())
    }

    #[cfg(debug_assertions)]
    fn render_impl_with_perf(
        &mut self,
        model: &mut AppModel,
        perf: &mut super::perf::PerfStats,
    ) -> Result<()> {
        use std::time::Instant;

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

        let t_clear = Instant::now();
        let bg_color = model.theme.editor.background.to_argb_u32();
        buffer.fill(bg_color);
        perf.clear_time = t_clear.elapsed();

        let t_text = Instant::now();
        Self::render_all_groups_static(
            &mut buffer,
            model,
            &splitters,
            &self.font,
            &mut self.glyph_cache,
            line_height,
            font_size,
            ascent,
            char_width,
            width,
            height,
        );
        perf.text_time = t_text.elapsed();

        let t_status = Instant::now();
        let status_bar_bg = model.theme.status_bar.background.to_argb_u32();
        let status_bar_fg = model.theme.status_bar.foreground.to_argb_u32();
        let status_y = (height as usize).saturating_sub(status_bar_height);

        for py in status_y..height as usize {
            for px in 0..width as usize {
                buffer[py * width as usize + px] = status_bar_bg;
            }
        }

        let available_chars = (width as f32 / char_width).floor() as usize;
        let layout = model.ui.status_bar.layout(available_chars);

        for seg in &layout.left {
            let x_px = (seg.x as f32 * char_width).round() as usize;
            draw_text(
                &mut buffer,
                &self.font,
                &mut self.glyph_cache,
                font_size,
                ascent,
                width,
                height,
                x_px,
                status_y + 2,
                &seg.text,
                status_bar_fg,
            );
        }

        for seg in &layout.right {
            let x_px = (seg.x as f32 * char_width).round() as usize;
            draw_text(
                &mut buffer,
                &self.font,
                &mut self.glyph_cache,
                font_size,
                ascent,
                width,
                height,
                x_px,
                status_y + 2,
                &seg.text,
                status_bar_fg,
            );
        }

        let separator_color = model
            .theme
            .status_bar
            .foreground
            .with_alpha(100)
            .to_argb_u32();
        for &sep_char_x in &layout.separator_positions {
            let x_px = (sep_char_x as f32 * char_width).round() as usize;
            if x_px < width as usize {
                for py in status_y..height as usize {
                    buffer[py * width as usize + x_px] = separator_color;
                }
            }
        }
        perf.status_bar_time = t_status.elapsed();

        if perf.show_overlay {
            super::perf::render_perf_overlay(
                &mut buffer,
                &self.font,
                &mut self.glyph_cache,
                perf,
                &model.theme,
                self.width,
                self.height,
                font_size,
                line_height,
                ascent,
            );
        }

        let t_present = Instant::now();
        buffer
            .present()
            .map_err(|e| anyhow::anyhow!("Failed to present buffer: {}", e))?;
        perf.present_time = t_present.elapsed();
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
}

pub fn draw_text(
    buffer: &mut [u32],
    font: &Font,
    glyph_cache: &mut GlyphCache,
    font_size: f32,
    ascent: f32,
    width: u32,
    height: u32,
    x: usize,
    y: usize,
    text: &str,
    color: u32,
) {
    let mut current_x = x as f32;

    let baseline = y as f32 + ascent;

    for ch in text.chars() {
        let key = (ch, font_size.to_bits());
        if !glyph_cache.contains_key(&key) {
            let (metrics, bitmap) = font.rasterize(ch, font_size);
            glyph_cache.insert(key, (metrics, bitmap));
        }
        let (metrics, bitmap) = glyph_cache.get(&key).unwrap();

        let glyph_top = baseline - metrics.height as f32 - metrics.ymin as f32;

        for bitmap_y in 0..metrics.height {
            for bitmap_x in 0..metrics.width {
                let bitmap_idx = bitmap_y * metrics.width + bitmap_x;
                if bitmap_idx < bitmap.len() {
                    let alpha = bitmap[bitmap_idx];
                    if alpha > 0 {
                        let px = current_x as isize + bitmap_x as isize + metrics.xmin as isize;
                        let py = (glyph_top + bitmap_y as f32) as isize;

                        if px >= 0
                            && py >= 0
                            && (px as usize) < width as usize
                            && (py as usize) < height as usize
                        {
                            let px = px as usize;
                            let py = py as usize;

                            let alpha_f = alpha as f32 / 255.0;
                            let bg_pixel = buffer[py * width as usize + px];

                            let bg_r = ((bg_pixel >> 16) & 0xFF) as f32;
                            let bg_g = ((bg_pixel >> 8) & 0xFF) as f32;
                            let bg_b = (bg_pixel & 0xFF) as f32;

                            let fg_r = ((color >> 16) & 0xFF) as f32;
                            let fg_g = ((color >> 8) & 0xFF) as f32;
                            let fg_b = (color & 0xFF) as f32;

                            let final_r = (bg_r * (1.0 - alpha_f) + fg_r * alpha_f) as u32;
                            let final_g = (bg_g * (1.0 - alpha_f) + fg_g * alpha_f) as u32;
                            let final_b = (bg_b * (1.0 - alpha_f) + fg_b * alpha_f) as u32;

                            buffer[py * width as usize + px] =
                                0xFF000000 | (final_r << 16) | (final_g << 8) | final_b;
                        }
                    }
                }
            }
        }

        current_x += metrics.advance_width;
    }
}

#[cfg(debug_assertions)]
pub fn draw_sparkline(
    buffer: &mut [u32],
    buffer_width: u32,
    buffer_height: u32,
    x: usize,
    y: usize,
    chart_width: usize,
    chart_height: usize,
    data: &std::collections::VecDeque<std::time::Duration>,
    bar_color: u32,
    bg_color: u32,
) {
    if data.is_empty() {
        return;
    }

    for py in y..(y + chart_height) {
        for px in x..(x + chart_width) {
            if px < buffer_width as usize && py < buffer_height as usize {
                buffer[py * buffer_width as usize + px] = bg_color;
            }
        }
    }

    let max_val = data.iter().map(|d| d.as_micros()).max().unwrap_or(1).max(1) as f32;

    let bar_width = (chart_width as f32 / data.len() as f32).max(1.0) as usize;
    let gap = if bar_width > 2 { 1 } else { 0 };

    for (i, duration) in data.iter().enumerate() {
        let normalized = duration.as_micros() as f32 / max_val;
        let bar_height = ((normalized * chart_height as f32) as usize).max(1);
        let bar_x = x + i * bar_width;

        for dy in 0..bar_height {
            let py = y + chart_height - dy - 1;
            for dx in 0..(bar_width - gap) {
                let px = bar_x + dx;
                if px < buffer_width as usize && py < buffer_height as usize {
                    buffer[py * buffer_width as usize + px] = bar_color;
                }
            }
        }
    }
}
