#[cfg(feature = "dhat-heap")]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

use anyhow::Result;
use fontdue::{Font, FontSettings, LineMetrics, Metrics};
use softbuffer::{Context, Surface};
use std::collections::HashMap;
#[cfg(debug_assertions)]
use std::collections::VecDeque;
use std::num::NonZeroU32;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, KeyCode, ModifiersState, NamedKey, PhysicalKey};
use winit::window::Window;

// Import from library modules
use token::commands::Cmd;
use token::messages::{AppMsg, Direction, DocumentMsg, EditorMsg, LayoutMsg, Msg, UiMsg};
use token::model::editor::Position;
use token::model::editor_area::{GroupId, Rect, SplitDirection, SplitterBar};
use token::model::{gutter_border_x, text_start_x, AppModel};
use token::update::update;

const TAB_BAR_HEIGHT: usize = 28;

// Glyph cache key: (character, font_size as bits)
type GlyphCacheKey = (char, u32);
type GlyphCache = HashMap<GlyphCacheKey, (Metrics, Vec<u8>)>;

/// Tab width in spaces for visual rendering
const TAB_WIDTH: usize = 4;

/// Expand tabs to spaces for display rendering
fn expand_tabs_for_display(text: &str) -> String {
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

/// Convert character column to visual column (accounting for tab expansion)
fn char_col_to_visual_col(text: &str, char_col: usize) -> usize {
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

/// Convert visual column to character column (accounting for tab expansion)
/// Returns the character column that corresponds to the given visual column.
/// If the visual column falls within a tab's expanded space, returns the tab's position.
fn visual_col_to_char_col(text: &str, visual_col: usize) -> usize {
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

    // Visual column is past end of text - return text length
    char_col
}

// Performance monitoring (debug builds only)
#[cfg(debug_assertions)]
const PERF_HISTORY_SIZE: usize = 60;

#[cfg(debug_assertions)]
#[derive(Default)]
#[allow(dead_code)] // Some fields reserved for future detailed timing
struct PerfStats {
    // Frame timing
    frame_start: Option<Instant>,
    last_frame_time: Duration,
    frame_times: VecDeque<Duration>, // Rolling window for avg/histogram

    // Render breakdown (current frame)
    clear_time: Duration,
    line_highlight_time: Duration,
    gutter_time: Duration,
    text_time: Duration,
    cursor_time: Duration,
    status_bar_time: Duration,
    present_time: Duration,

    // Render breakdown history (for sparklines)
    clear_history: VecDeque<Duration>,
    highlight_history: VecDeque<Duration>,
    gutter_history: VecDeque<Duration>,
    text_history: VecDeque<Duration>,
    cursor_history: VecDeque<Duration>,
    status_history: VecDeque<Duration>,
    present_history: VecDeque<Duration>,

    // Cache stats (reset per frame)
    frame_cache_hits: usize,
    frame_cache_misses: usize,

    // Cumulative cache stats
    total_cache_hits: usize,
    total_cache_misses: usize,

    // Display toggle
    show_overlay: bool,
}

#[cfg(debug_assertions)]
#[allow(dead_code)] // Some methods reserved for future use
impl PerfStats {
    fn reset_frame_stats(&mut self) {
        self.frame_cache_hits = 0;
        self.frame_cache_misses = 0;
    }

    fn record_frame_time(&mut self) {
        if let Some(start) = self.frame_start.take() {
            self.last_frame_time = start.elapsed();
            self.frame_times.push_back(self.last_frame_time);
            if self.frame_times.len() > 60 {
                self.frame_times.pop_front();
            }
        }
    }

    fn avg_frame_time(&self) -> Duration {
        if self.frame_times.is_empty() {
            return Duration::ZERO;
        }
        let total: Duration = self.frame_times.iter().sum();
        total / self.frame_times.len() as u32
    }

    fn fps(&self) -> f64 {
        let avg = self.avg_frame_time();
        if avg.as_secs_f64() > 0.0 {
            1.0 / avg.as_secs_f64()
        } else {
            0.0
        }
    }

    fn cache_hit_rate(&self) -> f64 {
        let total = self.total_cache_hits + self.total_cache_misses;
        if total > 0 {
            self.total_cache_hits as f64 / total as f64 * 100.0
        } else {
            0.0
        }
    }

    fn record_render_history(&mut self) {
        fn push_history(history: &mut VecDeque<Duration>, value: Duration) {
            history.push_back(value);
            if history.len() > PERF_HISTORY_SIZE {
                history.pop_front();
            }
        }

        push_history(&mut self.clear_history, self.clear_time);
        push_history(&mut self.highlight_history, self.line_highlight_time);
        push_history(&mut self.gutter_history, self.gutter_time);
        push_history(&mut self.text_history, self.text_time);
        push_history(&mut self.cursor_history, self.cursor_time);
        push_history(&mut self.status_history, self.status_bar_time);
        push_history(&mut self.present_history, self.present_time);
    }
}

// All colors now come from model.theme

// Model, Cursor, Viewport, EditOperation, CharType, char_type all imported from token::

// Msg, update, and Cmd imported from token:: library

// ============================================================================
// VIEW - Render the model to screen
// ============================================================================

struct Renderer {
    font: Font,
    surface: Surface<Rc<Window>, Rc<Window>>,
    width: u32,
    height: u32,
    font_size: f32,
    line_metrics: LineMetrics,
    glyph_cache: GlyphCache,
    char_width: f32, // Cached from actual font metrics for consistent positioning
}

impl Renderer {
    fn new(window: Rc<Window>, context: &Context<Rc<Window>>) -> Result<Self> {
        let scale_factor = window.scale_factor();
        let (width, height) = {
            let size = window.inner_size();
            (size.width, size.height)
        };

        let surface = Surface::new(context, Rc::clone(&window))
            .map_err(|e| anyhow::anyhow!("Failed to create surface: {}", e))?;

        // Load JetBrains Mono font
        let font = Font::from_bytes(
            include_bytes!("../assets/JetBrainsMono.ttf") as &[u8],
            FontSettings::default(),
        )
        .map_err(|e| anyhow::anyhow!("Failed to load font: {}", e))?;

        // Base font size 14pt, scaled for HiDPI
        let font_size = 14.0 * scale_factor as f32;

        // Get font line metrics for proper baseline positioning
        let line_metrics = font
            .horizontal_line_metrics(font_size)
            .expect("Font missing horizontal line metrics");

        // Get actual character width from font metrics (use 'M' as reference for monospace)
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

    fn char_width(&self) -> f32 {
        self.char_width
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

        let current_line_color = model.theme.editor.current_line_background.to_argb_u32();
        if editor.cursor().line >= editor.viewport.top_line && editor.cursor().line < end_line {
            let screen_line = editor.cursor().line - editor.viewport.top_line;
            let highlight_y = content_y + screen_line * line_height;

            for py in highlight_y..(highlight_y + line_height).min(content_y + content_h) {
                for px in rect_x..(rect_x + rect_w).min(width as usize) {
                    if py < height as usize {
                        buffer[py * width as usize + px] = current_line_color;
                    }
                }
            }
        }

        let selection = editor.selection();
        if !selection.is_empty() {
            let selection_color = model.theme.editor.selection_background.to_argb_u32();
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

                // Get line text for tab expansion calculation
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

                // Convert character columns to visual columns (accounting for tabs)
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
                let line_color = if doc_line == editor.cursor().line {
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

                // Expand tabs to spaces for rendering
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

                // Convert character column to visual column (accounting for tabs)
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
        group: &token::model::editor_area::EditorGroup,
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
                format!("● {}", filename)
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
    fn render(&mut self, model: &mut AppModel, _perf: ()) -> Result<()> {
        self.render_impl(model)
    }

    #[cfg(debug_assertions)]
    fn render(&mut self, model: &mut AppModel, perf: &mut PerfStats) -> Result<()> {
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
    fn render_impl_with_perf(&mut self, model: &mut AppModel, perf: &mut PerfStats) -> Result<()> {
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
            render_perf_overlay(
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

    fn pixel_to_cursor(&mut self, x: f64, y: f64, model: &AppModel) -> (usize, usize) {
        let line_height = self.line_metrics.new_line_size.ceil() as f64;
        let char_width = self.char_width as f64;
        let text_x = text_start_x(self.char_width).round() as f64;

        // Account for tab bar height - text starts below the tab bar
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

        // Convert visual column to character column (accounting for tabs)
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

    /// Check if a y pixel position is within the status bar area
    fn is_in_status_bar(&self, y: f64) -> bool {
        let line_height = self.line_metrics.new_line_size.ceil() as f64;
        let status_bar_top = self.height as f64 - line_height;
        y >= status_bar_top
    }

    /// Check if a y pixel position is within the tab bar area
    fn is_in_tab_bar(&self, y: f64) -> bool {
        y < TAB_BAR_HEIGHT as f64
    }
}

#[cfg(debug_assertions)]
fn render_perf_overlay(
    buffer: &mut [u32],
    font: &Font,
    glyph_cache: &mut GlyphCache,
    perf: &PerfStats,
    theme: &token::theme::Theme,
    width: u32,
    height: u32,
    font_size: f32,
    line_height: usize,
    ascent: f32,
) {
    use token::overlay::{
        render_overlay_background, render_overlay_border, OverlayAnchor, OverlayConfig,
    };

    let width_usize = width as usize;
    let height_usize = height as usize;

    // Configure and render overlay background using theme colors
    let config = OverlayConfig::new(OverlayAnchor::TopRight, 380, 480)
        .with_margin(10)
        .with_background(theme.overlay.background.to_argb_u32());

    let bounds = config.compute_bounds(width_usize, height_usize);
    render_overlay_background(
        buffer,
        &bounds,
        config.background,
        width_usize,
        height_usize,
    );

    // Render border if theme specifies one
    if let Some(border_color) = &theme.overlay.border {
        render_overlay_border(
            buffer,
            &bounds,
            border_color.to_argb_u32(),
            width_usize,
            height_usize,
        );
    }

    let text_color = theme.overlay.foreground.to_argb_u32();
    let highlight_color = theme.overlay.highlight.to_argb_u32();
    let warning_color = theme.overlay.warning.to_argb_u32();
    let error_color = theme.overlay.error.to_argb_u32();

    let text_x = bounds.x + 8;
    let mut text_y = bounds.y + 4;

    // Title
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
        "Performance",
        text_color,
    );
    text_y += line_height;

    // Frame time
    let frame_ms = perf.last_frame_time.as_secs_f64() * 1000.0;
    let fps = perf.fps();
    let budget_pct = (frame_ms / 16.67 * 100.0).min(999.0);
    let frame_color = if budget_pct < 80.0 {
        highlight_color
    } else if budget_pct < 100.0 {
        warning_color
    } else {
        error_color
    };

    let frame_text = format!("Frame: {:.1}ms ({:.0} fps)", frame_ms, fps);
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
        &frame_text,
        frame_color,
    );
    text_y += line_height;

    // Budget bar
    let bar_chars = (budget_pct / 10.0).min(10.0) as usize;
    let bar = format!(
        "[{}{}] {:.0}%",
        "█".repeat(bar_chars),
        "░".repeat(10 - bar_chars),
        budget_pct
    );
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
        &bar,
        frame_color,
    );
    text_y += line_height + 4;

    // Average frame time
    let avg_ms = perf.avg_frame_time().as_secs_f64() * 1000.0;
    let avg_text = format!("Avg: {:.1}ms", avg_ms);
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
        &avg_text,
        text_color,
    );
    text_y += line_height + 4;

    // Cache stats
    let cache_size = glyph_cache.len();
    let hit_rate = perf.cache_hit_rate();
    let cache_text = format!("Cache: {} glyphs", cache_size);
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
        &cache_text,
        text_color,
    );
    text_y += line_height;

    let hit_color = if hit_rate > 99.0 {
        highlight_color
    } else if hit_rate > 90.0 {
        warning_color
    } else {
        error_color
    };
    let hits_text = format!("Hits: {} ({:.1}%)", perf.total_cache_hits, hit_rate);
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
        &hits_text,
        hit_color,
    );
    text_y += line_height;

    let miss_text = format!("Miss: {}", perf.total_cache_misses);
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
        &miss_text,
        text_color,
    );
    text_y += line_height + 4;

    // Render breakdown with sparklines
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
        "Render breakdown:",
        text_color,
    );
    text_y += line_height;

    // Chart dimensions
    let chart_width = 180;
    let chart_height = 20;
    let chart_x = text_x + 80;
    let chart_bg = theme.overlay.background.with_alpha(200).to_argb_u32();

    let breakdown_with_history: [(&str, Duration, &VecDeque<Duration>, u32); 7] = [
        ("Clear", perf.clear_time, &perf.clear_history, 0xFF7AA2F7), // Blue
        (
            "Highlight",
            perf.line_highlight_time,
            &perf.highlight_history,
            0xFF9ECE6A,
        ), // Green
        ("Text", perf.text_time, &perf.text_history, 0xFFE0AF68),    // Yellow/Orange
        ("Cursor", perf.cursor_time, &perf.cursor_history, 0xFFBB9AF7), // Purple
        ("Gutter", perf.gutter_time, &perf.gutter_history, 0xFF7DCFFF), // Cyan
        (
            "Status",
            perf.status_bar_time,
            &perf.status_history,
            0xFFF7768E,
        ), // Pink
        (
            "Present",
            perf.present_time,
            &perf.present_history,
            0xFFFF9E64,
        ), // Orange
    ];

    for (name, duration, history, color) in breakdown_with_history {
        let us = duration.as_micros();
        let breakdown_text = format!("{:>7}:", name);
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
            &breakdown_text,
            text_color,
        );

        // Draw sparkline next to label
        draw_sparkline(
            buffer,
            width,
            height,
            chart_x,
            text_y + 2,
            chart_width,
            chart_height,
            history,
            color,
            chart_bg,
        );

        // Draw current value at end
        let value_text = format!("{} µs", us);
        let value_x = chart_x + chart_width + 6;
        draw_text(
            buffer,
            font,
            glyph_cache,
            font_size,
            ascent,
            width,
            height,
            value_x,
            text_y,
            &value_text,
            color,
        );

        text_y += chart_height + 4;
    }
}

fn draw_text(
    buffer: &mut [u32],
    font: &Font,
    glyph_cache: &mut GlyphCache,
    font_size: f32,
    ascent: f32,
    width: u32,
    height: u32,
    x: usize,
    y: usize, // line_top position
    text: &str,
    color: u32,
) {
    let mut current_x = x as f32;

    // Calculate baseline position: line_top + ascent
    let baseline = y as f32 + ascent;

    for ch in text.chars() {
        // Use cached glyph or rasterize
        let key = (ch, font_size.to_bits());
        if !glyph_cache.contains_key(&key) {
            let (metrics, bitmap) = font.rasterize(ch, font_size);
            glyph_cache.insert(key, (metrics, bitmap));
        }
        let (metrics, bitmap) = glyph_cache.get(&key).unwrap();

        // Draw the rasterized glyph
        // Position glyph for PositiveYDown coordinate system
        // (matches fontdue's layout.rs: y = -height - ymin)
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

                            // Blend the glyph with background based on alpha
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

        // Advance to the next character position
        current_x += metrics.advance_width;
    }
}

/// Draw a sparkline (bar chart) showing duration history
#[cfg(debug_assertions)]
fn draw_sparkline(
    buffer: &mut [u32],
    buffer_width: u32,
    buffer_height: u32,
    x: usize,
    y: usize,
    chart_width: usize,
    chart_height: usize,
    data: &VecDeque<Duration>,
    bar_color: u32,
    bg_color: u32,
) {
    if data.is_empty() {
        return;
    }

    // Draw background
    for py in y..(y + chart_height) {
        for px in x..(x + chart_width) {
            if px < buffer_width as usize && py < buffer_height as usize {
                buffer[py * buffer_width as usize + px] = bg_color;
            }
        }
    }

    // Find max value for scaling
    let max_val = data.iter().map(|d| d.as_micros()).max().unwrap_or(1).max(1) as f32;

    let bar_width = (chart_width as f32 / data.len() as f32).max(1.0) as usize;
    let gap = if bar_width > 2 { 1 } else { 0 };

    for (i, duration) in data.iter().enumerate() {
        let normalized = duration.as_micros() as f32 / max_val;
        let bar_height = ((normalized * chart_height as f32) as usize).max(1);
        let bar_x = x + i * bar_width;

        // Draw vertical bar from bottom up
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

// ============================================================================
// INPUT HANDLING
// ============================================================================

fn handle_key(
    model: &mut AppModel,
    key: Key,
    physical_key: PhysicalKey,
    ctrl: bool,
    shift: bool,
    alt: bool,
    logo: bool,
    option_double_tapped: bool,
) -> Option<Cmd> {
    // === Numpad Shortcuts (no modifiers needed) ===
    match physical_key {
        PhysicalKey::Code(KeyCode::Numpad1) => {
            return update(model, Msg::Layout(LayoutMsg::FocusGroupByIndex(1)));
        }
        PhysicalKey::Code(KeyCode::Numpad2) => {
            return update(model, Msg::Layout(LayoutMsg::FocusGroupByIndex(2)));
        }
        PhysicalKey::Code(KeyCode::Numpad3) => {
            return update(model, Msg::Layout(LayoutMsg::FocusGroupByIndex(3)));
        }
        PhysicalKey::Code(KeyCode::Numpad4) => {
            return update(model, Msg::Layout(LayoutMsg::FocusGroupByIndex(4)));
        }
        PhysicalKey::Code(KeyCode::NumpadSubtract) => {
            return update(
                model,
                Msg::Layout(LayoutMsg::SplitFocused(SplitDirection::Horizontal)),
            );
        }
        PhysicalKey::Code(KeyCode::NumpadAdd) => {
            return update(
                model,
                Msg::Layout(LayoutMsg::SplitFocused(SplitDirection::Vertical)),
            );
        }
        _ => {}
    }

    match key {
        // Double-tap Option + Arrow for multi-cursor (must be before other alt combinations)
        Key::Named(NamedKey::ArrowUp) if alt && option_double_tapped => {
            update(model, Msg::Editor(EditorMsg::AddCursorAbove))
        }
        Key::Named(NamedKey::ArrowDown) if alt && option_double_tapped => {
            update(model, Msg::Editor(EditorMsg::AddCursorBelow))
        }
        // Undo/Redo (Ctrl/Cmd+Z, Ctrl/Cmd+Shift+Z, Ctrl/Cmd+Y)
        Key::Character(ref s) if (ctrl || logo) && s.eq_ignore_ascii_case("z") => {
            if shift {
                update(model, Msg::Document(DocumentMsg::Redo))
            } else {
                update(model, Msg::Document(DocumentMsg::Undo))
            }
        }
        Key::Character(ref s) if (ctrl || logo) && s.eq_ignore_ascii_case("y") => {
            update(model, Msg::Document(DocumentMsg::Redo))
        }

        // Save file (Ctrl+S on Windows/Linux, Cmd+S on macOS)
        Key::Character(ref s) if s.eq_ignore_ascii_case("s") && (ctrl || logo) => {
            update(model, Msg::App(AppMsg::SaveFile))
        }

        // Select All (Cmd+A on macOS, Ctrl+A elsewhere)
        Key::Character(ref s) if s.eq_ignore_ascii_case("a") && (ctrl || logo) => {
            update(model, Msg::Editor(EditorMsg::SelectAll))
        }

        // Copy (Cmd+C on macOS, Ctrl+C elsewhere)
        Key::Character(ref s) if s.eq_ignore_ascii_case("c") && (ctrl || logo) => {
            update(model, Msg::Document(DocumentMsg::Copy))
        }

        // Cut (Cmd+X on macOS, Ctrl+X elsewhere)
        Key::Character(ref s) if s.eq_ignore_ascii_case("x") && (ctrl || logo) => {
            update(model, Msg::Document(DocumentMsg::Cut))
        }

        // Paste (Cmd+V on macOS, Ctrl+V elsewhere)
        Key::Character(ref s) if s.eq_ignore_ascii_case("v") && (ctrl || logo) => {
            update(model, Msg::Document(DocumentMsg::Paste))
        }

        // Duplicate line/selection (Cmd+D on macOS, Ctrl+D elsewhere)
        Key::Character(ref s) if s.eq_ignore_ascii_case("d") && (ctrl || logo) => {
            update(model, Msg::Document(DocumentMsg::Duplicate))
        }

        // Select next occurrence (Cmd+J on macOS, Ctrl+J elsewhere)
        Key::Character(ref s) if s.eq_ignore_ascii_case("j") && (ctrl || logo) && !shift => {
            update(model, Msg::Editor(EditorMsg::SelectNextOccurrence))
        }

        // Unselect last occurrence (Shift+Cmd+J on macOS, Shift+Ctrl+J elsewhere)
        Key::Character(ref s) if s.eq_ignore_ascii_case("j") && (ctrl || logo) && shift => {
            update(model, Msg::Editor(EditorMsg::UnselectOccurrence))
        }

        // === Split View Shortcuts ===

        // Split horizontal (Shift+Option+Cmd+H)
        Key::Character(ref s) if s.eq_ignore_ascii_case("h") && logo && shift && alt => update(
            model,
            Msg::Layout(LayoutMsg::SplitFocused(SplitDirection::Horizontal)),
        ),

        // Split vertical (Shift+Option+Cmd+V)
        Key::Character(ref s) if s.eq_ignore_ascii_case("v") && logo && shift && alt => update(
            model,
            Msg::Layout(LayoutMsg::SplitFocused(SplitDirection::Vertical)),
        ),

        // Close tab (Cmd+W)
        Key::Character(ref s) if s.eq_ignore_ascii_case("w") && logo && !shift && !alt => {
            update(model, Msg::Layout(LayoutMsg::CloseFocusedTab))
        }

        // Next tab (Option+Cmd+Right)
        Key::Named(NamedKey::ArrowRight) if logo && alt && !shift => {
            update(model, Msg::Layout(LayoutMsg::NextTab))
        }

        // Previous tab (Option+Cmd+Left)
        Key::Named(NamedKey::ArrowLeft) if logo && alt && !shift => {
            update(model, Msg::Layout(LayoutMsg::PrevTab))
        }

        // Focus group by index (Shift+Cmd+1/2/3/4)
        Key::Character(ref s) if s == "1" && logo && shift && !alt => {
            update(model, Msg::Layout(LayoutMsg::FocusGroupByIndex(1)))
        }
        Key::Character(ref s) if s == "2" && logo && shift && !alt => {
            update(model, Msg::Layout(LayoutMsg::FocusGroupByIndex(2)))
        }
        Key::Character(ref s) if s == "3" && logo && shift && !alt => {
            update(model, Msg::Layout(LayoutMsg::FocusGroupByIndex(3)))
        }
        Key::Character(ref s) if s == "4" && logo && shift && !alt => {
            update(model, Msg::Layout(LayoutMsg::FocusGroupByIndex(4)))
        }

        // Focus next/previous group (Ctrl+Tab / Ctrl+Shift+Tab)
        Key::Named(NamedKey::Tab) if ctrl && !shift => {
            update(model, Msg::Layout(LayoutMsg::FocusNextGroup))
        }
        Key::Named(NamedKey::Tab) if ctrl && shift => {
            update(model, Msg::Layout(LayoutMsg::FocusPrevGroup))
        }

        // Indent/Unindent (Tab / Shift+Tab)
        Key::Named(NamedKey::Tab) if shift && !(ctrl || logo) => {
            update(model, Msg::Document(DocumentMsg::UnindentLines))
        }
        Key::Named(NamedKey::Tab) if !(ctrl || logo) => {
            if model.editor().selection().is_empty() {
                update(model, Msg::Document(DocumentMsg::InsertChar('\t')))
            } else {
                update(model, Msg::Document(DocumentMsg::IndentLines))
            }
        }

        // Escape: clear selection or collapse to single cursor
        Key::Named(NamedKey::Escape) => {
            if model.editor().has_multiple_cursors() {
                update(model, Msg::Editor(EditorMsg::CollapseToSingleCursor))
            } else if !model.editor().selection().is_empty() {
                update(model, Msg::Editor(EditorMsg::ClearSelection))
            } else {
                None
            }
        }

        // Document navigation with selection (Shift+Ctrl+Home/End)
        Key::Named(NamedKey::Home) if ctrl && shift => update(
            model,
            Msg::Editor(EditorMsg::MoveCursorDocumentStartWithSelection),
        ),
        Key::Named(NamedKey::End) if ctrl && shift => update(
            model,
            Msg::Editor(EditorMsg::MoveCursorDocumentEndWithSelection),
        ),

        // Document navigation (Ctrl+Home/End)
        Key::Named(NamedKey::Home) if ctrl => {
            model.editor_mut().clear_selection();
            update(model, Msg::Editor(EditorMsg::MoveCursorDocumentStart))
        }
        Key::Named(NamedKey::End) if ctrl => {
            model.editor_mut().clear_selection();
            update(model, Msg::Editor(EditorMsg::MoveCursorDocumentEnd))
        }

        // Line navigation with selection (Shift+Cmd+Arrow on macOS)
        Key::Named(NamedKey::ArrowLeft) if logo && shift => update(
            model,
            Msg::Editor(EditorMsg::MoveCursorLineStartWithSelection),
        ),
        Key::Named(NamedKey::ArrowRight) if logo && shift => update(
            model,
            Msg::Editor(EditorMsg::MoveCursorLineEndWithSelection),
        ),

        // Line navigation (Cmd+Arrow on macOS)
        Key::Named(NamedKey::ArrowLeft) if logo => {
            model.editor_mut().clear_selection();
            update(model, Msg::Editor(EditorMsg::MoveCursorLineStart))
        }
        Key::Named(NamedKey::ArrowRight) if logo => {
            model.editor_mut().clear_selection();
            update(model, Msg::Editor(EditorMsg::MoveCursorLineEnd))
        }

        // Line navigation with selection (Shift+Home/End)
        Key::Named(NamedKey::Home) if shift => update(
            model,
            Msg::Editor(EditorMsg::MoveCursorLineStartWithSelection),
        ),
        Key::Named(NamedKey::End) if shift => update(
            model,
            Msg::Editor(EditorMsg::MoveCursorLineEndWithSelection),
        ),

        // Line navigation (Home/End keys)
        Key::Named(NamedKey::Home) => {
            model.editor_mut().clear_selection();
            update(model, Msg::Editor(EditorMsg::MoveCursorLineStart))
        }
        Key::Named(NamedKey::End) => {
            model.editor_mut().clear_selection();
            update(model, Msg::Editor(EditorMsg::MoveCursorLineEnd))
        }

        // Page navigation with selection (Shift+PageUp/Down)
        Key::Named(NamedKey::PageUp) if shift => {
            update(model, Msg::Editor(EditorMsg::PageUpWithSelection))
        }
        Key::Named(NamedKey::PageDown) if shift => {
            update(model, Msg::Editor(EditorMsg::PageDownWithSelection))
        }

        // Page navigation
        Key::Named(NamedKey::PageUp) => {
            if !model.editor().selection().is_empty() {
                // Jump to selection START, then page up
                let start = model.editor().selection().start();
                model.editor_mut().cursor_mut().line = start.line;
                model.editor_mut().cursor_mut().column = start.column;
                model.editor_mut().clear_selection();
            }
            update(model, Msg::Editor(EditorMsg::PageUp))
        }
        Key::Named(NamedKey::PageDown) => {
            if !model.editor().selection().is_empty() {
                // Jump to selection END, then page down
                let end = model.editor().selection().end();
                model.editor_mut().cursor_mut().line = end.line;
                model.editor_mut().cursor_mut().column = end.column;
                model.editor_mut().clear_selection();
            }
            update(model, Msg::Editor(EditorMsg::PageDown))
        }

        // Word navigation with selection (Shift+Option/Alt + Arrow)
        Key::Named(NamedKey::ArrowLeft) if alt && shift => update(
            model,
            Msg::Editor(EditorMsg::MoveCursorWordWithSelection(Direction::Left)),
        ),
        Key::Named(NamedKey::ArrowRight) if alt && shift => update(
            model,
            Msg::Editor(EditorMsg::MoveCursorWordWithSelection(Direction::Right)),
        ),

        // Word navigation (Option/Alt + Arrow)
        Key::Named(NamedKey::ArrowLeft) if alt => {
            model.editor_mut().clear_selection();
            update(
                model,
                Msg::Editor(EditorMsg::MoveCursorWord(Direction::Left)),
            )
        }
        Key::Named(NamedKey::ArrowRight) if alt => {
            model.editor_mut().clear_selection();
            update(
                model,
                Msg::Editor(EditorMsg::MoveCursorWord(Direction::Right)),
            )
        }

        // Arrow keys with selection (Shift+Arrow)
        Key::Named(NamedKey::ArrowUp) if shift => update(
            model,
            Msg::Editor(EditorMsg::MoveCursorWithSelection(Direction::Up)),
        ),
        Key::Named(NamedKey::ArrowDown) if shift => update(
            model,
            Msg::Editor(EditorMsg::MoveCursorWithSelection(Direction::Down)),
        ),
        Key::Named(NamedKey::ArrowLeft) if shift => update(
            model,
            Msg::Editor(EditorMsg::MoveCursorWithSelection(Direction::Left)),
        ),
        Key::Named(NamedKey::ArrowRight) if shift => update(
            model,
            Msg::Editor(EditorMsg::MoveCursorWithSelection(Direction::Right)),
        ),

        // Arrow keys (with selection: jump to start/end, then optionally move)
        Key::Named(NamedKey::ArrowUp) => {
            if !model.editor().selection().is_empty() {
                // Jump to selection START, then move up
                let start = model.editor().selection().start();
                model.editor_mut().cursor_mut().line = start.line;
                model.editor_mut().cursor_mut().column = start.column;
                model.editor_mut().clear_selection();
                update(model, Msg::Editor(EditorMsg::MoveCursor(Direction::Up)))
            } else {
                update(model, Msg::Editor(EditorMsg::MoveCursor(Direction::Up)))
            }
        }
        Key::Named(NamedKey::ArrowDown) => {
            if !model.editor().selection().is_empty() {
                // Jump to selection END, then move down
                let end = model.editor().selection().end();
                model.editor_mut().cursor_mut().line = end.line;
                model.editor_mut().cursor_mut().column = end.column;
                model.editor_mut().clear_selection();
                update(model, Msg::Editor(EditorMsg::MoveCursor(Direction::Down)))
            } else {
                update(model, Msg::Editor(EditorMsg::MoveCursor(Direction::Down)))
            }
        }
        Key::Named(NamedKey::ArrowLeft) => {
            if !model.editor().selection().is_empty() {
                // Jump to selection START (no additional move)
                let start = model.editor().selection().start();
                model.editor_mut().cursor_mut().line = start.line;
                model.editor_mut().cursor_mut().column = start.column;
                model.editor_mut().clear_selection();
                model.ensure_cursor_visible();
                model.reset_cursor_blink();
                Some(Cmd::Redraw)
            } else {
                update(model, Msg::Editor(EditorMsg::MoveCursor(Direction::Left)))
            }
        }
        Key::Named(NamedKey::ArrowRight) => {
            if !model.editor().selection().is_empty() {
                // Jump to selection END (no additional move)
                let end = model.editor().selection().end();
                model.editor_mut().cursor_mut().line = end.line;
                model.editor_mut().cursor_mut().column = end.column;
                model.editor_mut().clear_selection();
                model.ensure_cursor_visible();
                model.reset_cursor_blink();
                Some(Cmd::Redraw)
            } else {
                update(model, Msg::Editor(EditorMsg::MoveCursor(Direction::Right)))
            }
        }

        // Editing
        Key::Named(NamedKey::Enter) => update(model, Msg::Document(DocumentMsg::InsertNewline)),
        Key::Named(NamedKey::Backspace) if ctrl || logo => {
            update(model, Msg::Document(DocumentMsg::DeleteLine))
        }
        Key::Named(NamedKey::Backspace) => {
            update(model, Msg::Document(DocumentMsg::DeleteBackward))
        }
        Key::Named(NamedKey::Delete) => update(model, Msg::Document(DocumentMsg::DeleteForward)),
        Key::Named(NamedKey::Space) if !(ctrl || logo) => {
            update(model, Msg::Document(DocumentMsg::InsertChar(' ')))
        }

        // Character input (only when no Ctrl/Cmd)
        Key::Character(ref s) if !(ctrl || logo) => {
            let mut cmd = None;
            for ch in s.chars() {
                cmd = update(model, Msg::Document(DocumentMsg::InsertChar(ch))).or(cmd);
            }
            cmd
        }

        _ => None,
    }
}

// ============================================================================
// APPLICATION - Main event loop
// ============================================================================

struct App {
    model: AppModel,
    renderer: Option<Renderer>,
    window: Option<Rc<Window>>,
    context: Option<Context<Rc<Window>>>,
    last_tick: Instant,
    modifiers: ModifiersState,
    mouse_position: Option<(f64, f64)>,
    /// For double/triple click detection
    last_click_time: Instant,
    last_click_position: Option<(usize, usize)>,
    click_count: u32,
    /// For double-tap Option key detection (AddCursorAbove/Below)
    last_option_press: Option<Instant>,
    option_double_tapped: bool,
    /// Track left mouse button state for drag selection
    left_mouse_down: bool,
    /// Last time auto-scroll was triggered during drag selection
    last_auto_scroll: Option<Instant>,
    /// Mouse position when left button was pressed (for drag threshold)
    drag_start_position: Option<(f64, f64)>,
    /// True once drag distance threshold exceeded
    drag_active: bool,
    /// Channel sender for async command results
    msg_tx: Sender<Msg>,
    /// Channel receiver for async command results
    msg_rx: Receiver<Msg>,
    /// Performance stats (debug builds only)
    #[cfg(debug_assertions)]
    perf: PerfStats,
}

impl App {
    fn new(window_width: u32, window_height: u32, file_path: Option<PathBuf>) -> Self {
        let (msg_tx, msg_rx) = mpsc::channel();
        Self {
            model: AppModel::new(window_width, window_height, file_path),
            renderer: None,
            window: None,
            context: None,
            last_tick: Instant::now(),
            modifiers: ModifiersState::empty(),
            mouse_position: None,
            last_click_time: Instant::now(),
            last_click_position: None,
            click_count: 0,
            last_option_press: None,
            option_double_tapped: false,
            left_mouse_down: false,
            last_auto_scroll: None,
            drag_start_position: None,
            drag_active: false,
            msg_tx,
            msg_rx,
            #[cfg(debug_assertions)]
            perf: PerfStats::default(),
        }
    }

    fn init_renderer(&mut self, window: Rc<Window>, context: &Context<Rc<Window>>) -> Result<()> {
        let renderer = Renderer::new(window, context)?;

        // Sync actual char_width from renderer to model for accurate viewport calculations
        self.model.set_char_width(renderer.char_width());

        self.renderer = Some(renderer);
        Ok(())
    }

    /// Try to auto-scroll during drag selection when mouse is at/beyond viewport edges.
    /// Returns Some(Cmd::Redraw) if scroll occurred, None otherwise.
    /// Throttled to ~12 lines/second (80ms interval) for controllable scrolling.
    fn try_auto_scroll_for_drag(&mut self, y: f64) -> Option<Cmd> {
        const AUTO_SCROLL_INTERVAL_MS: u64 = 80;

        let line_height = self.model.line_height as f64;
        let window_height = self.model.window_size.1 as f64;
        let status_bar_top = window_height - line_height;

        // Determine if we're above or below the text area
        let scroll_direction = if y < 0.0 {
            Some(-1) // Above viewport, scroll up
        } else if y >= status_bar_top {
            Some(1) // Below viewport (in/past status bar), scroll down
        } else {
            None // Within viewport, no auto-scroll
        };

        let direction = scroll_direction?;

        // Check throttle
        let now = Instant::now();
        if let Some(last) = self.last_auto_scroll {
            if now.duration_since(last) < Duration::from_millis(AUTO_SCROLL_INTERVAL_MS) {
                return None; // Too soon
            }
        }

        self.last_auto_scroll = Some(now);
        update(&mut self.model, Msg::Editor(EditorMsg::Scroll(direction)))
    }

    fn handle_event(&mut self, event: &WindowEvent) -> Option<Cmd> {
        match event {
            WindowEvent::Resized(size) => update(
                &mut self.model,
                Msg::App(AppMsg::Resize(size.width, size.height)),
            ),
            WindowEvent::ModifiersChanged(mods) => {
                self.modifiers = mods.state();
                None
            }
            WindowEvent::KeyboardInput { event, .. } => {
                // Detect Option key double-tap for AddCursorAbove/Below
                let is_option_key = matches!(
                    event.physical_key,
                    PhysicalKey::Code(KeyCode::AltLeft) | PhysicalKey::Code(KeyCode::AltRight)
                );

                if is_option_key {
                    if event.state == ElementState::Pressed && !event.repeat {
                        let now = Instant::now();
                        if let Some(last) = self.last_option_press {
                            // Double-tap threshold: 300ms
                            if now.duration_since(last) < Duration::from_millis(300) {
                                self.option_double_tapped = true;
                            }
                        }
                        self.last_option_press = Some(now);
                    } else if event.state == ElementState::Released {
                        // Reset double-tap state on Option release
                        self.option_double_tapped = false;
                    }
                }

                if event.state == ElementState::Pressed {
                    // F2 toggles perf overlay (debug builds only)
                    #[cfg(debug_assertions)]
                    if event.logical_key == Key::Named(NamedKey::F2) {
                        self.perf.show_overlay = !self.perf.show_overlay;
                        return Some(Cmd::Redraw);
                    }

                    let ctrl = self.modifiers.control_key();
                    let shift = self.modifiers.shift_key();
                    let alt = self.modifiers.alt_key();
                    let logo = self.modifiers.super_key(); // Cmd on macOS
                    handle_key(
                        &mut self.model,
                        event.logical_key.clone(),
                        event.physical_key,
                        ctrl,
                        shift,
                        alt,
                        logo,
                        self.option_double_tapped,
                    )
                } else {
                    None
                }
            }
            WindowEvent::RedrawRequested => {
                // Actually perform the render here
                if let Err(e) = self.render() {
                    eprintln!("Render error: {}", e);
                }
                None
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.mouse_position = Some((position.x, position.y));

                // Update rectangle selection if active
                if self.model.editor().rectangle_selection.active {
                    if let Some(renderer) = &mut self.renderer {
                        let (line, column) =
                            renderer.pixel_to_cursor(position.x, position.y, &self.model);
                        return update(
                            &mut self.model,
                            Msg::Editor(EditorMsg::UpdateRectangleSelection { line, column }),
                        );
                    }
                }
                // Left-button drag selection with distance threshold and throttled auto-scroll
                else if self.left_mouse_down {
                    const DRAG_THRESHOLD_PIXELS: f64 = 4.0;

                    if let Some(renderer) = &mut self.renderer {
                        // Check if drag threshold has been exceeded
                        if !self.drag_active {
                            if let Some((start_x, start_y)) = self.drag_start_position {
                                let dx = position.x - start_x;
                                let dy = position.y - start_y;
                                let distance = (dx * dx + dy * dy).sqrt();

                                if distance >= DRAG_THRESHOLD_PIXELS {
                                    // Threshold exceeded - initialize selection anchor at START position
                                    self.drag_active = true;
                                    let (start_line, start_col) =
                                        renderer.pixel_to_cursor(start_x, start_y, &self.model);
                                    self.model.editor_mut().selection_mut().anchor =
                                        Position::new(start_line, start_col);
                                }
                            }
                        }

                        // Only update selection if drag is active
                        if self.drag_active {
                            let (line, column) =
                                renderer.pixel_to_cursor(position.x, position.y, &self.model);

                            // Update cursor and selection head
                            self.model.editor_mut().cursor_mut().line = line;
                            self.model.editor_mut().cursor_mut().column = column;
                            self.model.editor_mut().selection_mut().head =
                                Position::new(line, column);

                            // Try throttled auto-scroll if mouse is at/beyond edges
                            self.try_auto_scroll_for_drag(position.y);

                            return Some(Cmd::Redraw);
                        }
                    }
                }
                None
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                if let Some((x, y)) = self.mouse_position {
                    if let Some(renderer) = &mut self.renderer {
                        // Ignore clicks on the status bar
                        if renderer.is_in_status_bar(y) {
                            return None;
                        }

                        // Ignore clicks on the tab bar
                        if renderer.is_in_tab_bar(y) {
                            return None;
                        }

                        self.left_mouse_down = true;
                        self.drag_start_position = Some((x, y));
                        self.drag_active = false;

                        let (line, column) = renderer.pixel_to_cursor(x, y, &self.model);
                        let now = Instant::now();
                        let double_click_time = Duration::from_millis(300);

                        // Detect double/triple click
                        let is_rapid_click =
                            now.duration_since(self.last_click_time) < double_click_time;
                        let is_same_position = self.last_click_position == Some((line, column));

                        if is_rapid_click && is_same_position {
                            self.click_count += 1;
                            if self.click_count > 3 {
                                self.click_count = 1;
                            }
                        } else {
                            self.click_count = 1;
                        }

                        self.last_click_time = now;
                        self.last_click_position = Some((line, column));

                        // Shift+Click extends selection (always single-click behavior)
                        if self.modifiers.shift_key() {
                            return update(
                                &mut self.model,
                                Msg::Editor(EditorMsg::ExtendSelectionToPosition { line, column }),
                            );
                        }

                        // Option+Click (macOS) toggles cursor at position
                        if self.modifiers.alt_key() {
                            return update(
                                &mut self.model,
                                Msg::Editor(EditorMsg::ToggleCursorAtPosition { line, column }),
                            );
                        }

                        // Handle click count
                        match self.click_count {
                            2 => {
                                // Double-click: select word
                                // First set cursor position, then select word
                                update(
                                    &mut self.model,
                                    Msg::Editor(EditorMsg::SetCursorPosition { line, column }),
                                );
                                return update(&mut self.model, Msg::Editor(EditorMsg::SelectWord));
                            }
                            3 => {
                                // Triple-click: select line
                                update(
                                    &mut self.model,
                                    Msg::Editor(EditorMsg::SetCursorPosition { line, column }),
                                );
                                return update(&mut self.model, Msg::Editor(EditorMsg::SelectLine));
                            }
                            _ => {
                                // Single click: clear selection and set cursor
                                self.model.editor_mut().clear_selection();
                                return update(
                                    &mut self.model,
                                    Msg::Editor(EditorMsg::SetCursorPosition { line, column }),
                                );
                            }
                        }
                    }
                }
                None
            }
            // Left mouse button release - end drag selection
            WindowEvent::MouseInput {
                state: ElementState::Released,
                button: MouseButton::Left,
                ..
            } => {
                self.left_mouse_down = false;
                self.last_auto_scroll = None;
                self.drag_start_position = None;
                self.drag_active = false;
                None
            }
            // Middle mouse button - rectangle selection
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Middle,
                ..
            } => {
                if let Some((x, y)) = self.mouse_position {
                    if let Some(renderer) = &mut self.renderer {
                        // Ignore clicks on the status bar
                        if renderer.is_in_status_bar(y) {
                            return None;
                        }

                        // Ignore clicks on the tab bar
                        if renderer.is_in_tab_bar(y) {
                            return None;
                        }

                        let (line, column) = renderer.pixel_to_cursor(x, y, &self.model);
                        return update(
                            &mut self.model,
                            Msg::Editor(EditorMsg::StartRectangleSelection { line, column }),
                        );
                    }
                }
                None
            }
            WindowEvent::MouseInput {
                state: ElementState::Released,
                button: MouseButton::Middle,
                ..
            } => {
                if self.model.editor().rectangle_selection.active {
                    return update(
                        &mut self.model,
                        Msg::Editor(EditorMsg::FinishRectangleSelection),
                    );
                }
                None
            }
            WindowEvent::MouseWheel { delta, .. } => {
                use winit::event::MouseScrollDelta;
                let (h_delta, v_delta) = match delta {
                    MouseScrollDelta::LineDelta(x, y) => {
                        // y is positive for scroll up, negative for scroll down
                        // We want positive to mean scroll down, so negate y
                        // x is already positive for scroll right on macOS (including Shift+scroll)
                        ((x * 3.0) as i32, (-y * 3.0) as i32)
                    }
                    MouseScrollDelta::PixelDelta(pos) => {
                        // Convert pixels to lines/columns (approximate)
                        let line_height = self.model.line_height as f64;
                        let char_width = self.model.char_width as f64;
                        ((pos.x / char_width) as i32, (-pos.y / line_height) as i32)
                    }
                };

                // Handle vertical scroll
                let v_cmd = if v_delta != 0 {
                    update(&mut self.model, Msg::Editor(EditorMsg::Scroll(v_delta)))
                } else {
                    None
                };

                // Handle horizontal scroll
                let h_cmd = if h_delta != 0 {
                    update(
                        &mut self.model,
                        Msg::Editor(EditorMsg::ScrollHorizontal(h_delta)),
                    )
                } else {
                    None
                };

                // Return Redraw if either scrolled
                v_cmd.or(h_cmd)
            }
            _ => None,
        }
    }

    #[cfg(not(debug_assertions))]
    fn render(&mut self) -> Result<()> {
        if let Some(renderer) = &mut self.renderer {
            renderer.render(&mut self.model, ())?;
        }
        Ok(())
    }

    #[cfg(debug_assertions)]
    fn render(&mut self) -> Result<()> {
        self.perf.frame_start = Some(Instant::now());

        if let Some(renderer) = &mut self.renderer {
            renderer.render(&mut self.model, &mut self.perf)?;
        }

        self.perf.record_frame_time();
        self.perf.record_render_history();
        Ok(())
    }

    fn tick(&mut self) -> Option<Cmd> {
        // Handle animations
        update(&mut self.model, Msg::Ui(UiMsg::BlinkCursor))
    }

    /// Process a command, potentially spawning async operations
    fn process_cmd(&self, cmd: Cmd) {
        match cmd {
            Cmd::None => {}
            Cmd::Redraw => {
                // Handled by the caller requesting a window redraw
            }
            Cmd::SaveFile { path, content } => {
                let tx = self.msg_tx.clone();
                std::thread::spawn(move || {
                    let result = std::fs::write(&path, content).map_err(|e| e.to_string());
                    let _ = tx.send(Msg::App(AppMsg::SaveCompleted(result)));
                });
            }
            Cmd::LoadFile { path } => {
                let tx = self.msg_tx.clone();
                std::thread::spawn(move || {
                    let result = std::fs::read_to_string(&path).map_err(|e| e.to_string());
                    let _ = tx.send(Msg::App(AppMsg::FileLoaded { path, result }));
                });
            }
            Cmd::Batch(cmds) => {
                for cmd in cmds {
                    self.process_cmd(cmd);
                }
            }
        }
    }

    /// Process pending async messages from the channel
    fn process_async_messages(&mut self) -> bool {
        let mut needs_redraw = false;
        while let Ok(msg) = self.msg_rx.try_recv() {
            if let Some(cmd) = update(&mut self.model, msg) {
                if cmd.needs_redraw() {
                    needs_redraw = true;
                }
                self.process_cmd(cmd);
            }
        }
        needs_redraw
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            let window_attributes = Window::default_attributes()
                .with_title("Token")
                .with_inner_size(LogicalSize::new(800, 600));

            let window = Rc::new(event_loop.create_window(window_attributes).unwrap());
            let context = Context::new(Rc::clone(&window)).unwrap();

            self.init_renderer(Rc::clone(&window), &context).unwrap();
            self.window = Some(window);
            self.context = Some(context);
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        let should_exit = matches!(event, WindowEvent::CloseRequested);
        let should_redraw = if let Some(window) = &self.window {
            if window_id == window.id() && !should_exit {
                if let Some(cmd) = self.handle_event(&event) {
                    let needs_redraw = cmd.needs_redraw();
                    self.process_cmd(cmd);
                    needs_redraw
                } else {
                    false
                }
            } else {
                false
            }
        } else {
            false
        };

        if should_exit {
            event_loop.exit();
        } else if should_redraw {
            if let Some(window) = &self.window {
                window.request_redraw();
            }
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        // Use Poll to make event loop responsive to scrolling and user input
        // This ensures immediate response to mouse wheel, touchpad, and keyboard events
        event_loop.set_control_flow(ControlFlow::Poll);

        // Process any pending async messages
        if self.process_async_messages() {
            if let Some(window) = &self.window {
                window.request_redraw();
            }
        }

        // Only tick for cursor blinking animation
        let now = Instant::now();
        if now.duration_since(self.last_tick) > Duration::from_millis(500) {
            self.last_tick = now;
            if self.tick().is_some() {
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
        }
    }
}

// ============================================================================
// MAIN - Entry point
// ============================================================================

fn main() -> Result<()> {
    #[cfg(feature = "dhat-heap")]
    let _profiler = dhat::Profiler::new_heap();

    env_logger::init();

    // Parse command-line arguments
    let args: Vec<String> = std::env::args().collect();
    let file_path = if args.len() > 1 {
        Some(PathBuf::from(&args[1]))
    } else {
        None
    };

    let event_loop = EventLoop::new()?;
    let mut app = App::new(800, 600, file_path);

    event_loop.run_app(&mut app)?;

    Ok(())
}

// ============================================================================
// TESTS - Keyboard handling tests that require handle_key()
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use token::model::{
        Cursor, Document, EditorArea, EditorState, Position, RectangleSelectionState, Selection,
        UiState, Viewport,
    };
    use token::theme::Theme;

    /// Create a test model with given text and a selection (anchor to head)
    /// The cursor will be at the head position
    fn test_model_with_selection(
        text: &str,
        anchor_line: usize,
        anchor_col: usize,
        head_line: usize,
        head_col: usize,
    ) -> AppModel {
        let cursor = Cursor {
            line: head_line,
            column: head_col,
            desired_column: None,
        };
        let selection = Selection {
            anchor: Position::new(anchor_line, anchor_col),
            head: Position::new(head_line, head_col),
        };
        let document = Document::with_text(text);
        let editor = EditorState {
            id: None,
            document_id: None,
            cursors: vec![cursor],
            selections: vec![selection],
            viewport: Viewport {
                top_line: 0,
                left_column: 0,
                visible_lines: 25,
                visible_columns: 80,
            },
            scroll_padding: 1,
            rectangle_selection: RectangleSelectionState::default(),
            occurrence_state: None,
        };
        let editor_area = EditorArea::single_document(document, editor);
        AppModel {
            editor_area,
            ui: UiState::new(),
            theme: Theme::default(),
            window_size: (800, 600),
            line_height: 20,
            char_width: 10.0,
        }
    }

    // ========================================================================
    // Arrow Keys with Selection Tests
    // These tests require handle_key() which is in the binary, not the library
    // ========================================================================

    #[test]
    fn test_left_arrow_with_selection_jumps_to_start() {
        // When text is selected and Left is pressed, cursor should go to selection START
        // Text: "hello world" with "llo wo" selected (columns 2-8)
        let mut model = test_model_with_selection("hello world\n", 0, 2, 0, 8);
        // Selection: anchor at col 2, head/cursor at col 8

        // Press Left (without shift)
        handle_key(
            &mut model,
            Key::Named(NamedKey::ArrowLeft),
            PhysicalKey::Code(KeyCode::ArrowLeft),
            false,
            false,
            false,
            false,
            false,
        );

        // Selection should be cleared
        assert!(
            model.editor().selection().is_empty(),
            "Selection should be cleared"
        );
        // Cursor should be at selection START (column 2), not moved left from 8
        assert_eq!(
            model.editor().cursor().column,
            2,
            "Cursor should jump to selection start (col 2), not stay at col 8 or move to col 7"
        );
    }

    #[test]
    fn test_right_arrow_with_selection_jumps_to_end() {
        // When text is selected and Right is pressed, cursor should go to selection END
        // Text: "hello world" with "llo wo" selected (columns 2-8)
        let mut model = test_model_with_selection("hello world\n", 0, 2, 0, 8);

        // Press Right (without shift)
        handle_key(
            &mut model,
            Key::Named(NamedKey::ArrowRight),
            PhysicalKey::Code(KeyCode::ArrowRight),
            false,
            false,
            false,
            false,
            false,
        );

        // Selection should be cleared
        assert!(
            model.editor().selection().is_empty(),
            "Selection should be cleared"
        );
        // Cursor should be at selection END (column 8), not moved right from 8
        assert_eq!(
            model.editor().cursor().column,
            8,
            "Cursor should jump to selection end (col 8), not move to col 9"
        );
    }

    #[test]
    fn test_up_arrow_with_selection_moves_from_start() {
        // When text is selected and Up is pressed, cursor should:
        // 1. Jump to selection START
        // 2. Move up one line from there
        // Selection spans line 1, cols 2-8
        let mut model =
            test_model_with_selection("hello world\nfoo bar baz\nthird line\n", 1, 2, 1, 8);
        // Cursor is at line 1, col 8 (head of selection)

        // Press Up (without shift)
        handle_key(
            &mut model,
            Key::Named(NamedKey::ArrowUp),
            PhysicalKey::Code(KeyCode::ArrowUp),
            false,
            false,
            false,
            false,
            false,
        );

        // Selection should be cleared
        assert!(
            model.editor().selection().is_empty(),
            "Selection should be cleared"
        );
        // Cursor should be on line 0 (moved up from line 1)
        assert_eq!(
            model.editor().cursor().line,
            0,
            "Cursor should move up to line 0"
        );
        // Cursor should be at column 2 (selection start column)
        assert_eq!(
            model.editor().cursor().column,
            2,
            "Cursor should be at column 2 (selection start column)"
        );
    }

    #[test]
    fn test_down_arrow_with_selection_moves_from_end() {
        // When text is selected and Down is pressed, cursor should:
        // 1. Jump to selection END
        // 2. Move down one line from there
        // Selection spans line 1, cols 2-8
        let mut model =
            test_model_with_selection("hello world\nfoo bar baz\nthird line\n", 1, 2, 1, 8);
        // Cursor is at line 1, col 8 (head of selection)

        // Press Down (without shift)
        handle_key(
            &mut model,
            Key::Named(NamedKey::ArrowDown),
            PhysicalKey::Code(KeyCode::ArrowDown),
            false,
            false,
            false,
            false,
            false,
        );

        // Selection should be cleared
        assert!(
            model.editor().selection().is_empty(),
            "Selection should be cleared"
        );
        // Cursor should be on line 2 (moved down from line 1)
        assert_eq!(
            model.editor().cursor().line,
            2,
            "Cursor should move down to line 2"
        );
        // Cursor should be at column 8 (selection end column)
        assert_eq!(
            model.editor().cursor().column,
            8,
            "Cursor should be at column 8 (selection end column)"
        );
    }

    // ========================================================================
    // Home/End with Selection Tests
    // ========================================================================

    #[test]
    fn test_home_with_selection_uses_head_line() {
        // Home should cancel selection and go to start of line where HEAD is
        // Selection: anchor at (0, 5), head at (1, 8)
        let mut model =
            test_model_with_selection("hello world\nfoo bar baz\nthird line\n", 0, 5, 1, 8);
        // Head is on line 1

        // Press Home
        handle_key(
            &mut model,
            Key::Named(NamedKey::Home),
            PhysicalKey::Code(KeyCode::Home),
            false,
            false,
            false,
            false,
            false,
        );

        // Selection should be cleared
        assert!(
            model.editor().selection().is_empty(),
            "Selection should be cleared"
        );
        // Cursor should stay on line 1 (where head was)
        assert_eq!(
            model.editor().cursor().line,
            1,
            "Cursor should stay on line 1 (head line)"
        );
        // Cursor should be at start of line (smart home: first non-ws char, but for "foo" that's 0)
        assert_eq!(
            model.editor().cursor().column,
            0,
            "Cursor should be at start of line"
        );
    }

    #[test]
    fn test_end_with_selection_uses_head_line() {
        // End should cancel selection and go to end of line where HEAD is
        // Selection: anchor at (0, 5), head at (1, 2)
        let mut model =
            test_model_with_selection("hello world\nfoo bar baz\nthird line\n", 0, 5, 1, 2);
        // Head is on line 1

        // Press End
        handle_key(
            &mut model,
            Key::Named(NamedKey::End),
            PhysicalKey::Code(KeyCode::End),
            false,
            false,
            false,
            false,
            false,
        );

        // Selection should be cleared
        assert!(
            model.editor().selection().is_empty(),
            "Selection should be cleared"
        );
        // Cursor should stay on line 1 (where head was)
        assert_eq!(
            model.editor().cursor().line,
            1,
            "Cursor should stay on line 1 (head line)"
        );
        // Cursor should be at end of line 1 ("foo bar baz" has length 11)
        assert_eq!(
            model.editor().cursor().column,
            11,
            "Cursor should be at end of line (col 11)"
        );
    }

    // ========================================================================
    // PageUp/PageDown with Selection Tests
    // ========================================================================

    #[test]
    fn test_pageup_with_selection_moves_from_start() {
        // PageUp should cancel selection and move up from selection START
        // Create text with many lines
        let text = (0..30).map(|i| format!("line {}\n", i)).collect::<String>();
        // Selection: anchor at (15, 2), head at (15, 5) - both on line 15
        let mut model = test_model_with_selection(&text, 15, 2, 15, 5);
        model.editor_mut().viewport.visible_lines = 10;

        // Press PageUp
        handle_key(
            &mut model,
            Key::Named(NamedKey::PageUp),
            PhysicalKey::Code(KeyCode::PageUp),
            false,
            false,
            false,
            false,
            false,
        );

        // Selection should be cleared
        assert!(
            model.editor().selection().is_empty(),
            "Selection should be cleared"
        );
        // Cursor should have moved up from selection start (line 15, col 2)
        // PageUp moves ~8 lines (visible_lines - 2)
        assert!(
            model.editor().cursor().line < 15,
            "Cursor should have moved up from line 15"
        );
        // Column should be from selection start (col 2)
        assert_eq!(
            model.editor().cursor().column,
            2,
            "Cursor column should be at selection start col (2)"
        );
    }

    #[test]
    fn test_pagedown_with_selection_moves_from_end() {
        // PageDown should cancel selection and move down from selection END
        // Create text with many lines
        let text = (0..30).map(|i| format!("line {}\n", i)).collect::<String>();
        // Selection: anchor at (5, 2), head at (5, 5) - both on line 5
        let mut model = test_model_with_selection(&text, 5, 2, 5, 5);
        model.editor_mut().viewport.visible_lines = 10;

        // Press PageDown
        handle_key(
            &mut model,
            Key::Named(NamedKey::PageDown),
            PhysicalKey::Code(KeyCode::PageDown),
            false,
            false,
            false,
            false,
            false,
        );

        // Selection should be cleared
        assert!(
            model.editor().selection().is_empty(),
            "Selection should be cleared"
        );
        // Cursor should have moved down from selection end (line 5, col 5)
        // PageDown moves ~8 lines (visible_lines - 2)
        assert!(
            model.editor().cursor().line > 5,
            "Cursor should have moved down from line 5"
        );
        // Column should be from selection end (col 5)
        assert_eq!(
            model.editor().cursor().column,
            5,
            "Cursor column should be at selection end col (5)"
        );
    }

    // ========================================================================
    // Large Document Viewport Focus Tests
    // ========================================================================

    #[test]
    fn test_select_all_then_right_arrow_scrolls_to_end() {
        // Create a 500-line document (each line has newline, so 500 lines total, last is empty)
        let text = (0..500)
            .map(|i| format!("line {}\n", i))
            .collect::<String>();
        let mut model = test_model_with_selection(&text, 0, 0, 0, 0);
        model.editor_mut().viewport.visible_lines = 25;
        model.editor_mut().viewport.top_line = 0;

        let total_lines = model.document().line_count();

        // Select all (Cmd+A)
        update(&mut model, Msg::Editor(EditorMsg::SelectAll));

        // Verify selection spans entire document
        assert_eq!(model.editor().selection().anchor, Position::new(0, 0));
        let last_line = total_lines.saturating_sub(1);
        assert_eq!(
            model.editor().cursor().line,
            last_line,
            "Cursor should be at last line"
        );

        // Press Right arrow - should clear selection and position cursor at end
        handle_key(
            &mut model,
            Key::Named(NamedKey::ArrowRight),
            PhysicalKey::Code(KeyCode::ArrowRight),
            false,
            false,
            false,
            false,
            false,
        );

        // Selection should be cleared
        assert!(
            model.editor().selection().is_empty(),
            "Selection should be cleared"
        );

        // Cursor should be at end of document
        assert_eq!(
            model.editor().cursor().line,
            last_line,
            "Cursor should be at last line"
        );

        // Viewport should have scrolled to show the cursor
        // The cursor should be visible within the viewport
        let viewport_end = model.editor().viewport.top_line + model.editor().viewport.visible_lines;
        assert!(
            model.editor().cursor().line >= model.editor().viewport.top_line,
            "Cursor (line {}) should be >= viewport top (line {})",
            model.editor().cursor().line,
            model.editor().viewport.top_line
        );
        assert!(
            model.editor().cursor().line < viewport_end,
            "Cursor (line {}) should be < viewport end (line {})",
            model.editor().cursor().line,
            viewport_end
        );
    }

    #[test]
    fn test_pageup_scrolls_cursor_to_viewport_top() {
        // Create a 100-line document
        let text = (0..100)
            .map(|i| format!("line {}\n", i))
            .collect::<String>();
        let mut model = test_model_with_selection(&text, 0, 0, 0, 0);
        model.editor_mut().viewport.visible_lines = 20;
        model.editor_mut().scroll_padding = 1;

        // Position cursor at about half the viewport height (line 10)
        // and set viewport to start at line 0
        model.editor_mut().cursor_mut().line = 10;
        model.editor_mut().viewport.top_line = 0;

        // Press PageUp - cursor should jump above the viewport,
        // and viewport should adjust to show cursor at top
        handle_key(
            &mut model,
            Key::Named(NamedKey::PageUp),
            PhysicalKey::Code(KeyCode::PageUp),
            false,
            false,
            false,
            false,
            false,
        );

        // PageUp moves visible_lines - 2 = 18 lines up
        // From line 10, that would be line 0 (clamped)
        assert_eq!(
            model.editor().cursor().line,
            0,
            "Cursor should be at line 0 after PageUp"
        );

        // Viewport should adjust to show cursor
        // With cursor at line 0, viewport.top_line should be 0
        assert_eq!(
            model.editor().viewport.top_line,
            0,
            "Viewport should scroll to top to show cursor"
        );

        // Cursor should be visible
        assert!(
            model.editor().cursor().line >= model.editor().viewport.top_line,
            "Cursor should be visible (>= viewport top)"
        );
    }

    #[test]
    fn test_pageup_from_middle_adjusts_viewport() {
        // Create a 100-line document
        let text = (0..100)
            .map(|i| format!("line {}\n", i))
            .collect::<String>();
        let mut model = test_model_with_selection(&text, 0, 0, 0, 0);
        model.editor_mut().viewport.visible_lines = 20;
        model.editor_mut().scroll_padding = 1;

        // Position cursor at line 50 with viewport showing lines 40-60
        model.editor_mut().cursor_mut().line = 50;
        model.editor_mut().viewport.top_line = 40;

        // Press PageUp - cursor should move up 18 lines (20 - 2)
        // From line 50, cursor goes to line 32
        handle_key(
            &mut model,
            Key::Named(NamedKey::PageUp),
            PhysicalKey::Code(KeyCode::PageUp),
            false,
            false,
            false,
            false,
            false,
        );

        // Cursor should be at line 32 (50 - 18)
        assert_eq!(
            model.editor().cursor().line,
            32,
            "Cursor should be at line 32"
        );

        // Line 32 was above the viewport (which was at 40-60)
        // Viewport should have adjusted to show the cursor
        // Cursor should be visible and near the top of viewport
        assert!(
            model.editor().viewport.top_line <= model.editor().cursor().line,
            "Cursor (line {}) should be >= viewport top (line {})",
            model.editor().cursor().line,
            model.editor().viewport.top_line
        );

        // Cursor should be within visible range
        let viewport_end = model.editor().viewport.top_line + model.editor().viewport.visible_lines;
        assert!(
            model.editor().cursor().line < viewport_end,
            "Cursor should be visible within viewport"
        );
    }

    // ========================================================================
    // Cmd+Z / Cmd+Shift+Z Keybinding Tests (macOS)
    // ========================================================================

    #[test]
    fn test_cmd_z_triggers_undo_not_insert_z() {
        // Test that Cmd+Z (logo=true) triggers undo and doesn't insert 'z'
        let mut model = test_model_with_selection("hello", 0, 5, 0, 5);

        // Make a change: insert 'X'
        update(&mut model, Msg::Document(DocumentMsg::InsertChar('X')));
        assert_eq!(model.document().buffer.to_string(), "helloX");
        assert_eq!(model.editor().cursor().column, 6);

        // Simulate Cmd+Z: logo=true, ctrl=false
        handle_key(
            &mut model,
            Key::Character("z".into()),
            PhysicalKey::Code(KeyCode::KeyZ),
            false, // ctrl
            false, // shift
            false, // alt
            true,  // logo (Cmd on macOS)
            false, // option_double_tapped
        );

        // Undo should have run, and no 'z' should be typed
        assert_eq!(
            model.document().buffer.to_string(),
            "hello",
            "Cmd+Z should undo the insert, not type 'z'"
        );
        assert_eq!(model.editor().cursor().column, 5);
    }

    #[test]
    fn test_cmd_shift_z_triggers_redo_not_insert_z() {
        // Test that Cmd+Shift+Z (logo=true, shift=true) triggers redo
        let mut model = test_model_with_selection("hello", 0, 5, 0, 5);

        // Make a change and undo it
        update(&mut model, Msg::Document(DocumentMsg::InsertChar('X')));
        assert_eq!(model.document().buffer.to_string(), "helloX");

        update(&mut model, Msg::Document(DocumentMsg::Undo));
        assert_eq!(model.document().buffer.to_string(), "hello");

        // Simulate Cmd+Shift+Z: logo=true, shift=true
        handle_key(
            &mut model,
            Key::Character("z".into()),
            PhysicalKey::Code(KeyCode::KeyZ),
            false, // ctrl
            true,  // shift
            false, // alt
            true,  // logo (Cmd on macOS)
            false, // option_double_tapped
        );

        // Redo should have run
        assert_eq!(
            model.document().buffer.to_string(),
            "helloX",
            "Cmd+Shift+Z should redo the insert"
        );
        assert_eq!(model.editor().cursor().column, 6);
    }

    #[test]
    fn test_ctrl_z_still_works_for_undo() {
        // Ensure Ctrl+Z still works (for non-macOS)
        let mut model = test_model_with_selection("hello", 0, 5, 0, 5);

        update(&mut model, Msg::Document(DocumentMsg::InsertChar('X')));
        assert_eq!(model.document().buffer.to_string(), "helloX");

        // Simulate Ctrl+Z
        handle_key(
            &mut model,
            Key::Character("z".into()),
            PhysicalKey::Code(KeyCode::KeyZ),
            true,  // ctrl
            false, // shift
            false, // alt
            false, // logo
            false,
        );

        assert_eq!(
            model.document().buffer.to_string(),
            "hello",
            "Ctrl+Z should undo"
        );
    }
}
