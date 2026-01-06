//! View module - rendering code extracted from main.rs
//!
//! Contains the Renderer struct and all rendering-related functionality.

pub mod frame;
pub mod geometry;
pub mod helpers;
pub mod text_field;

pub use frame::{Frame, TextPainter};
pub use helpers::get_tab_display_name;
pub use text_field::{TextFieldContent, TextFieldOptions, TextFieldRenderer};

// Re-export geometry helpers for backward compatibility
pub use geometry::{char_col_to_visual_col, expand_tabs_for_display};

use anyhow::Result;
use fontdue::{Font, FontSettings, LineMetrics, Metrics};
use softbuffer::Surface;
use std::collections::HashMap;
use std::num::NonZeroU32;
use std::rc::Rc;
use winit::window::Window;

use token::commands::{Damage, DamageArea};
use token::model::editor_area::{EditorGroup, GroupId, Rect, SplitterBar};

/// Check if the damage contains cursor lines (free function to avoid borrow issues)
fn has_cursor_lines_damage(damage: &Damage) -> bool {
    match damage {
        Damage::None | Damage::Full => false,
        Damage::Areas(areas) => areas
            .iter()
            .any(|a| matches!(a, DamageArea::CursorLines(_))),
    }
}
use token::model::AppModel;

/// Context for sidebar rendering, holding constant values throughout tree traversal.
struct SidebarRenderContext {
    sidebar_width: usize,
    sidebar_height: usize,
    row_height: usize,
    indent: f32,
    scroll_offset: usize,
    // Colors
    text_color: u32,
    selection_bg: u32,
    selection_fg: u32,
    folder_icon_color: u32,
}

pub type GlyphCacheKey = (char, u32);

pub type GlyphCache = HashMap<GlyphCacheKey, (Metrics, Vec<u8>)>;

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

    #[allow(dead_code)]
    pub fn font(&self) -> &Font {
        &self.font
    }

    #[allow(dead_code)]
    pub fn font_size(&self) -> f32 {
        self.font_size
    }

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
        #[allow(unused_variables)] perf: &crate::runtime::perf::PerfStats,
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

    /// Render only specific cursor lines (optimized path for cursor blink)
    ///
    /// This function redraws only the specified line numbers, which is much faster
    /// than redrawing the entire editor area. Used for cursor blink optimization.
    ///
    /// For each dirty line, renders:
    /// - Line background (editor bg or current line highlight)
    /// - Gutter (line number)
    /// - Text content with syntax highlighting
    /// - Cursor (if visible and on this line)
    fn render_cursor_lines_only(
        frame: &mut Frame,
        painter: &mut TextPainter,
        model: &AppModel,
        dirty_lines: &[usize],
    ) {
        let char_width = painter.char_width();
        let line_height = painter.line_height();

        // Get the focused group and its document
        let focused_group_id = model.editor_area.focused_group_id;
        let Some(group) = model.editor_area.groups.get(&focused_group_id) else {
            return;
        };

        let Some(editor_id) = group.active_editor_id() else {
            return;
        };

        let Some(editor) = model.editor_area.editors.get(&editor_id) else {
            return;
        };

        let Some(doc_id) = editor.document_id else {
            return;
        };

        let Some(document) = model.editor_area.documents.get(&doc_id) else {
            return;
        };

        // Use GroupLayout for all positioning (DPI-aware, single source of truth)
        let layout = geometry::GroupLayout::new(group, model, char_width);

        let visible_lines = layout.visible_lines(line_height);
        let end_line = (editor.viewport.top_line + visible_lines).min(document.buffer.len_lines());

        // Colors
        let bg_color = model.theme.editor.background.to_argb_u32();
        let current_line_color = model.theme.editor.current_line_background.to_argb_u32();
        let gutter_bg_color = model.theme.gutter.background.to_argb_u32();
        let line_num_color = model.theme.gutter.foreground.to_argb_u32();
        let line_num_active_color = model.theme.gutter.foreground_active.to_argb_u32();
        let text_color = model.theme.editor.foreground.to_argb_u32();
        let primary_cursor_color = model.theme.editor.cursor_color.to_argb_u32();
        let secondary_cursor_color = model.theme.editor.secondary_cursor_color.to_argb_u32();

        // Layout-derived values
        let rect_x = layout.rect_x();
        let rect_w = layout.rect_w();
        let gutter_right_x = layout.gutter_right_x;
        let gutter_width = layout.gutter_width();
        let group_text_start_x = layout.text_start_x;

        let text_start_x_offset = layout.text_start_x - rect_x;
        let visible_columns =
            ((rect_w as f32 - text_start_x_offset as f32) / char_width).floor() as usize;
        let max_chars = visible_columns;

        // Reusable buffers
        let mut adjusted_tokens: Vec<token::syntax::HighlightToken> = Vec::with_capacity(32);
        let mut display_text_buf = String::with_capacity(max_chars + 16);

        for &doc_line in dirty_lines {
            // Skip lines outside viewport
            if doc_line < editor.viewport.top_line || doc_line >= end_line {
                continue;
            }

            // Use layout helper for line Y position
            let Some(y) = layout.line_to_screen_y(doc_line, editor.viewport.top_line, line_height)
            else {
                continue;
            };

            // 1. Clear line background (gutter + text area)
            let is_cursor_line = doc_line == editor.active_cursor().line;

            // Clear gutter area for this line
            frame.fill_rect_px(rect_x, y, gutter_width, line_height, gutter_bg_color);

            // Clear text area for this line
            let text_area_x = gutter_right_x + 1; // After gutter border
            let text_area_w = rect_w.saturating_sub(gutter_width + 1);
            if is_cursor_line {
                frame.fill_rect_px(text_area_x, y, text_area_w, line_height, current_line_color);
            } else {
                frame.fill_rect_px(text_area_x, y, text_area_w, line_height, bg_color);
            }

            // 2. Render gutter (line number)
            let line_num_str = format!("{}", doc_line + 1);
            let text_width_px = (line_num_str.len() as f32 * char_width).round() as usize;
            let gutter_text_x = gutter_right_x.saturating_sub(4 + text_width_px);
            let line_color = if is_cursor_line {
                line_num_active_color
            } else {
                line_num_color
            };
            painter.draw(frame, gutter_text_x, y, &line_num_str, line_color);

            // 3. Render text content with syntax highlighting
            if let Some(line_text) = document.get_line_cow(doc_line) {
                let expanded_text = expand_tabs_for_display(&line_text);

                display_text_buf.clear();
                for ch in expanded_text
                    .chars()
                    .skip(editor.viewport.left_column)
                    .take(max_chars)
                {
                    display_text_buf.push(ch);
                }

                // Get syntax highlights
                let line_tokens = document.get_line_highlights(doc_line);

                adjusted_tokens.clear();
                for t in line_tokens.iter() {
                    let visual_start = char_col_to_visual_col(&line_text, t.start_col);
                    let visual_end = char_col_to_visual_col(&line_text, t.end_col);

                    let start = visual_start.saturating_sub(editor.viewport.left_column);
                    let end = visual_end.saturating_sub(editor.viewport.left_column);

                    if end > 0 && start < max_chars {
                        adjusted_tokens.push(token::syntax::HighlightToken {
                            start_col: start,
                            end_col: end.min(max_chars),
                            highlight: t.highlight,
                        });
                    }
                }

                if adjusted_tokens.is_empty() {
                    painter.draw(frame, group_text_start_x, y, &display_text_buf, text_color);
                } else {
                    painter.draw_with_highlights(
                        frame,
                        group_text_start_x,
                        y,
                        &display_text_buf,
                        &adjusted_tokens,
                        &model.theme.syntax,
                        text_color,
                    );
                }
            }

            // 4. Render cursors on this line (if visible)
            if model.ui.cursor_visible {
                for (idx, cursor) in editor.cursors.iter().enumerate() {
                    if cursor.line != doc_line {
                        continue;
                    }

                    let line_text = document.get_line_cow(cursor.line).unwrap_or_default();
                    let visual_cursor_col = char_col_to_visual_col(&line_text, cursor.column);

                    let cursor_in_horizontal_view = visual_cursor_col
                        >= editor.viewport.left_column
                        && visual_cursor_col < editor.viewport.left_column + visible_columns;

                    if cursor_in_horizontal_view {
                        let cursor_visual_column = visual_cursor_col - editor.viewport.left_column;
                        let cursor_x = (group_text_start_x as f32
                            + cursor_visual_column as f32 * char_width)
                            .round() as usize;

                        let cursor_color = if idx == 0 {
                            primary_cursor_color
                        } else {
                            secondary_cursor_color
                        };

                        frame.fill_rect_px(
                            cursor_x,
                            y + 1,
                            2,
                            line_height.saturating_sub(2),
                            cursor_color,
                        );
                    }
                }
            }
        }
    }

    /// Render the entire editor area: all groups and splitters.
    ///
    /// This is the top-level widget that orchestrates rendering of:
    /// - All editor groups (each with tab bar, gutter, text area)
    /// - Splitter bars between groups
    fn render_editor_area(
        frame: &mut Frame,
        painter: &mut TextPainter,
        model: &AppModel,
        splitters: &[SplitterBar],
    ) {
        for (&group_id, group) in &model.editor_area.groups {
            let is_focused = group_id == model.editor_area.focused_group_id;
            Self::render_editor_group(frame, painter, model, group_id, group.rect, is_focused);
        }

        Self::render_splitters(frame, splitters, model);
    }

    /// Render an entire editor group: tab bar, gutter, text area, and focus dimming.
    ///
    /// This is the main orchestrator that calls individual widget functions.
    /// Uses GroupLayout for all positioning to ensure DPI-aware, consistent rendering.
    fn render_editor_group(
        frame: &mut Frame,
        painter: &mut TextPainter,
        model: &AppModel,
        group_id: GroupId,
        group_rect: Rect,
        is_focused: bool,
    ) {
        let char_width = painter.char_width();

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

        // Create GroupLayout - single source of truth for all positioning
        let layout = geometry::GroupLayout::new(group, model, char_width);

        // Tab bar
        Self::render_tab_bar(frame, painter, model, group, &layout);

        // Check view mode and dispatch to appropriate renderer
        if let Some(csv_state) = editor.view_mode.as_csv() {
            // CSV mode: render grid
            Self::render_csv_grid(frame, painter, model, csv_state, &layout, is_focused);
        } else {
            // Text mode: render normal text area

            // Text area (background highlights, text, cursors)
            Self::render_text_area(frame, painter, model, editor, document, &layout, is_focused);

            // Gutter (line numbers, border) - drawn on top of text area background
            Self::render_gutter(frame, painter, model, editor, document, &layout);
        }

        // Dim non-focused groups when multiple groups exist (4% black overlay)
        if !is_focused && model.editor_area.groups.len() > 1 {
            let dim_color = 0x0A000000_u32; // 4% opacity black (alpha = 10/255 ≈ 4%)
            frame.blend_rect(group_rect, dim_color);
        }
    }

    fn render_tab_bar(
        frame: &mut Frame,
        painter: &mut TextPainter,
        model: &AppModel,
        group: &EditorGroup,
        layout: &geometry::GroupLayout,
    ) {
        let metrics = &model.metrics;
        let char_width = painter.char_width();
        let rect_x = layout.rect_x();
        let rect_y = layout.rect_y();
        let rect_w = layout.rect_w();
        let tab_bar_height = layout.tab_bar_height;

        let tab_bar_bg = model.theme.tab_bar.background.to_argb_u32();
        frame.fill_rect_px(rect_x, rect_y, rect_w, tab_bar_height, tab_bar_bg);

        let border_color = model.theme.tab_bar.border.to_argb_u32();
        let border_y = (rect_y + tab_bar_height).saturating_sub(1);
        frame.fill_rect_px(rect_x, border_y, rect_w, metrics.border_width, border_color);

        let mut tab_x = rect_x + metrics.padding_medium;
        let tab_height = tab_bar_height.saturating_sub(metrics.padding_medium);
        let tab_y = rect_y + metrics.padding_small;

        for (idx, tab) in group.tabs.iter().enumerate() {
            let is_active = idx == group.active_tab_index;
            let display_name = get_tab_display_name(model, tab);
            let tab_width = (display_name.len() as f32 * char_width).round() as usize
                + metrics.padding_large * 2;

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

            let text_x = tab_x + metrics.padding_large;
            let text_y = tab_y + metrics.padding_medium;
            painter.draw(frame, text_x, text_y, &display_name, fg_color);

            tab_x += tab_width + metrics.padding_small;
            if tab_x >= rect_x + rect_w {
                break;
            }
        }
    }

    fn render_splitters(frame: &mut Frame, splitters: &[SplitterBar], model: &AppModel) {
        let splitter_color = model.theme.splitter.background.to_argb_u32();

        for splitter in splitters {
            frame.fill_rect(splitter.rect, splitter_color);
        }
    }

    /// Render the sidebar (file tree) for a workspace.
    fn render_sidebar(
        frame: &mut Frame,
        painter: &mut TextPainter,
        model: &AppModel,
        sidebar_width: usize,
        sidebar_height: usize,
    ) {
        let Some(workspace) = &model.workspace else {
            return;
        };

        let theme = &model.theme.sidebar;
        let metrics = &model.metrics;

        // Draw sidebar background
        let bg_color = theme.background.to_argb_u32();
        frame.fill_rect(
            Rect::new(0.0, 0.0, sidebar_width as f32, sidebar_height as f32),
            bg_color,
        );

        // Draw resize border on the right edge
        let border_color = theme.border.to_argb_u32();
        let border_x = sidebar_width.saturating_sub(1);
        frame.fill_rect(
            Rect::new(border_x as f32, 0.0, 1.0, sidebar_height as f32),
            border_color,
        );

        // Build render context with all constant values
        let ctx = SidebarRenderContext {
            sidebar_width,
            sidebar_height,
            row_height: metrics.file_tree_row_height,
            indent: metrics.file_tree_indent,
            scroll_offset: workspace.scroll_offset,
            text_color: theme.foreground.to_argb_u32(),
            selection_bg: theme.selection_background.to_argb_u32(),
            selection_fg: theme.selection_foreground.to_argb_u32(),
            folder_icon_color: theme.folder_icon.to_argb_u32(),
        };

        let mut y = 0usize;
        let mut visible_index = 0usize;

        // Helper function to render a tree node recursively.
        // visible_index tracks the global flattened index of items.
        // Items before scroll_offset are counted but not drawn.
        // y only advances for items that are actually drawn.
        #[allow(clippy::too_many_arguments)]
        fn render_node(
            frame: &mut Frame,
            painter: &mut TextPainter,
            node: &token::model::FileNode,
            workspace: &token::model::Workspace,
            ctx: &SidebarRenderContext,
            y: &mut usize,
            visible_index: &mut usize,
            depth: usize,
        ) {
            // If we've started rendering and filled the viewport, bail out
            if *visible_index >= ctx.scroll_offset && *y >= ctx.sidebar_height {
                return;
            }

            // Capture this node's global index, then advance the counter
            let idx = *visible_index;
            *visible_index += 1;

            // Only draw if this item is at or after scroll_offset
            let is_visible_row = idx >= ctx.scroll_offset;

            if is_visible_row {
                // If we're beyond the viewport height, stop drawing
                if *y >= ctx.sidebar_height {
                    return;
                }

                let x_offset = (depth as f32 * ctx.indent) as usize + 8; // 8px left padding

                // Check if this item is selected
                let is_selected = workspace
                    .selected_item
                    .as_ref()
                    .map(|p| p == &node.path)
                    .unwrap_or(false);

                // Draw selection background with alpha blending
                if is_selected {
                    frame.fill_rect_blended(
                        Rect::new(
                            0.0,
                            *y as f32,
                            ctx.sidebar_width as f32,
                            ctx.row_height as f32,
                        ),
                        ctx.selection_bg,
                    );
                }

                // Draw expand/collapse indicator for directories
                let icon_x = x_offset;
                let text_x = x_offset + 20; // Space for +/- indicator plus gap

                // Text is positioned with y as the top of the text area
                // Add small vertical padding to center text in the row
                let text_y = *y + 2; // Small top padding

                if node.is_dir {
                    let is_expanded = workspace.is_expanded(&node.path);
                    // Use +/- indicators: - for expanded, + for collapsed
                    let indicator = if is_expanded { "-" } else { "+" };
                    let icon_color = if is_selected {
                        ctx.selection_fg
                    } else {
                        ctx.folder_icon_color
                    };
                    painter.draw(frame, icon_x, text_y, indicator, icon_color);
                }

                // Draw file/folder name, truncating if too long
                let fg = if is_selected {
                    ctx.selection_fg
                } else {
                    ctx.text_color
                };

                // Calculate available width for text (sidebar_width minus text_x and right padding)
                let right_padding = 8;
                let available_width = ctx.sidebar_width.saturating_sub(text_x + right_padding);

                // Truncate the name if it's too long (estimate char width as ~8px for now)
                // Use Cow to avoid allocation when no truncation needed
                let estimated_char_width = 8;
                let max_chars = available_width / estimated_char_width;
                let needs_truncation = node.name.len() > max_chars && max_chars > 3;

                if needs_truncation {
                    // Only allocate when truncation is needed
                    let truncated = &node.name[..max_chars.saturating_sub(2)];
                    let mut display_name = String::with_capacity(truncated.len() + 3);
                    display_name.push_str(truncated);
                    display_name.push('…');
                    painter.draw(frame, text_x, text_y, &display_name, fg);
                } else {
                    // No allocation - use the name directly
                    painter.draw(frame, text_x, text_y, &node.name, fg);
                }

                // Only advance y for items that are actually drawn
                *y += ctx.row_height;
            }

            // Always recurse into children if expanded (even if parent is above viewport)
            // Children may scroll into view even when their parent folder header is not visible
            if node.is_dir && workspace.is_expanded(&node.path) {
                for child in &node.children {
                    render_node(
                        frame,
                        painter,
                        child,
                        workspace,
                        ctx,
                        y,
                        visible_index,
                        depth + 1,
                    );
                }
            }
        }

        // Render all root nodes
        for node in &workspace.file_tree.roots {
            render_node(
                frame,
                painter,
                node,
                workspace,
                &ctx,
                &mut y,
                &mut visible_index,
                0,
            );
            // Early exit if viewport is filled
            if visible_index >= ctx.scroll_offset && y >= ctx.sidebar_height {
                break;
            }
        }
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
        editor: &token::model::EditorState,
        document: &token::model::Document,
        layout: &geometry::GroupLayout,
    ) {
        let gutter_bg_color = model.theme.gutter.background.to_argb_u32();
        let line_num_color = model.theme.gutter.foreground.to_argb_u32();
        let line_num_active_color = model.theme.gutter.foreground_active.to_argb_u32();
        let char_width = painter.char_width();
        let line_height = painter.line_height();

        let rect_x = layout.rect_x();
        let content_y = layout.content_y();
        let content_h = layout.content_h();
        let gutter_right_x = layout.gutter_right_x;
        let gutter_width = layout.gutter_width();

        let visible_lines = layout.visible_lines(line_height);
        let end_line = (editor.viewport.top_line + visible_lines).min(document.buffer.len_lines());

        let gutter_border_color = model.theme.gutter.border_color.to_argb_u32();

        // Draw gutter background
        frame.fill_rect_px(rect_x, content_y, gutter_width, content_h, gutter_bg_color);

        for (screen_line, doc_line) in (editor.viewport.top_line..end_line).enumerate() {
            let y = content_y + screen_line * line_height;
            if y >= content_y + content_h {
                break;
            }

            // Right-align line numbers so they sit just left of the gutter border.
            let line_num_str = format!("{}", doc_line + 1);
            let text_width_px = (line_num_str.len() as f32 * char_width).round() as usize;
            // 4px padding from the border
            let text_x = gutter_right_x.saturating_sub(4 + text_width_px);

            let line_color = if doc_line == editor.active_cursor().line {
                line_num_active_color
            } else {
                line_num_color
            };
            painter.draw(frame, text_x, y, &line_num_str, line_color);
        }

        // Gutter border
        frame.fill_rect_px(gutter_right_x, content_y, 1, content_h, gutter_border_color);
    }

    /// Render text content (lines, selections, cursors) for an editor group.
    ///
    /// Draws:
    /// - Current line highlight
    /// - Selection highlights
    /// - Text content
    /// - Cursors (only if group is focused)
    fn render_text_area(
        frame: &mut Frame,
        painter: &mut TextPainter,
        model: &AppModel,
        editor: &token::model::EditorState,
        document: &token::model::Document,
        layout: &geometry::GroupLayout,
        is_focused: bool,
    ) {
        let char_width = painter.char_width();
        let line_height = painter.line_height();
        let rect_x = layout.rect_x();
        let rect_w = layout.rect_w();
        let content_y = layout.content_y();
        let content_h = layout.content_h();
        let group_text_start_x = layout.text_start_x;

        let text_start_x_offset = layout.text_start_x - rect_x;
        let visible_lines = layout.visible_lines(line_height);
        let visible_columns =
            ((rect_w as f32 - text_start_x_offset as f32) / char_width).floor() as usize;
        let end_line = (editor.viewport.top_line + visible_lines).min(document.buffer.len_lines());

        // Current line highlight
        let current_line_color = model.theme.editor.current_line_background.to_argb_u32();
        if editor.active_cursor().line >= editor.viewport.top_line
            && editor.active_cursor().line < end_line
        {
            let screen_line = editor.active_cursor().line - editor.viewport.top_line;
            let highlight_y = content_y + screen_line * line_height;
            let highlight_h = line_height.min(content_y + content_h - highlight_y);
            frame.fill_rect_px(rect_x, highlight_y, rect_w, highlight_h, current_line_color);
        }

        // Selection highlights
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

                // Use get_line_cow for zero-allocation when line is contiguous
                let line_text = document.get_line_cow(doc_line).unwrap_or_default();

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

                let visual_start_col = char_col_to_visual_col(&line_text, start_col);
                let visual_end_col = char_col_to_visual_col(&line_text, end_col);

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

        // Rectangle selection highlight (middle mouse drag preview)
        // Uses visual columns (screen position) for consistent behavior across lines
        if editor.rectangle_selection.active {
            let rect_sel = &editor.rectangle_selection;
            let top_line = rect_sel.top_line();
            let bottom_line = rect_sel.bottom_line();
            let left_visual_col = rect_sel.left_visual_col();
            let right_visual_col = rect_sel.right_visual_col();
            let current_visual_col = rect_sel.current_visual_col;

            let visible_start = top_line.max(editor.viewport.top_line);
            let visible_end = (bottom_line + 1).min(end_line);

            for doc_line in visible_start..visible_end {
                // Use get_line_cow for zero-allocation when line is contiguous
                let line_text = document.get_line_cow(doc_line).unwrap_or_default();
                let line_visual_len = char_col_to_visual_col(&line_text, line_text.chars().count());

                // Only show highlight if current position is within the line's visual width
                // (not dragging past line end)
                if current_visual_col > line_visual_len {
                    continue;
                }

                // Clamp visual columns to line's visual width
                let start_visual = left_visual_col.min(line_visual_len);
                let end_visual = right_visual_col.min(line_visual_len);

                // Skip lines where selection would be empty
                if start_visual >= end_visual {
                    continue;
                }

                let screen_line = doc_line - editor.viewport.top_line;
                let y_start = content_y + screen_line * line_height;
                let y_end = (y_start + line_height).min(content_y + content_h);

                let visible_start_col = start_visual.saturating_sub(editor.viewport.left_column);
                let visible_end_col = end_visual.saturating_sub(editor.viewport.left_column);

                let x_start =
                    group_text_start_x + (visible_start_col as f32 * char_width).round() as usize;
                let x_end = (group_text_start_x
                    + (visible_end_col as f32 * char_width).round() as usize)
                    .min(rect_x + rect_w);

                if x_end > x_start {
                    frame.fill_rect_px(
                        x_start,
                        y_start,
                        x_end.saturating_sub(x_start),
                        y_end.saturating_sub(y_start),
                        selection_color,
                    );
                }
            }
        }

        // Text content with syntax highlighting
        // Reuse buffers to avoid per-line allocations
        let text_color = model.theme.editor.foreground.to_argb_u32();
        let max_chars = visible_columns;
        let mut adjusted_tokens: Vec<token::syntax::HighlightToken> = Vec::with_capacity(32); // Reused across lines
        let mut display_text_buf = String::with_capacity(max_chars + 16); // Reused for display

        for (screen_line, doc_line) in (editor.viewport.top_line..end_line).enumerate() {
            // Use get_line_cow for zero-allocation when line is contiguous
            if let Some(line_text) = document.get_line_cow(doc_line) {
                let y = content_y + screen_line * line_height;
                if y >= content_y + content_h {
                    break;
                }

                // expand_tabs_for_display returns Cow - no allocation if no tabs
                let expanded_text = expand_tabs_for_display(&line_text);

                // Reuse display_text buffer instead of allocating new String each line
                display_text_buf.clear();
                for ch in expanded_text
                    .chars()
                    .skip(editor.viewport.left_column)
                    .take(max_chars)
                {
                    display_text_buf.push(ch);
                }

                // Get syntax highlights for this line
                let line_tokens = document.get_line_highlights(doc_line);

                // Reuse adjusted_tokens buffer instead of allocating new Vec each line
                adjusted_tokens.clear();
                for t in line_tokens.iter() {
                    // Convert character columns to visual columns (accounting for tabs)
                    let visual_start = char_col_to_visual_col(&line_text, t.start_col);
                    let visual_end = char_col_to_visual_col(&line_text, t.end_col);

                    // Adjust for horizontal scroll
                    let start = visual_start.saturating_sub(editor.viewport.left_column);
                    let end = visual_end.saturating_sub(editor.viewport.left_column);

                    if end > 0 && start < max_chars {
                        adjusted_tokens.push(token::syntax::HighlightToken {
                            start_col: start,
                            end_col: end.min(max_chars),
                            highlight: t.highlight,
                        });
                    }
                }

                if adjusted_tokens.is_empty() {
                    painter.draw(frame, group_text_start_x, y, &display_text_buf, text_color);
                } else {
                    painter.draw_with_highlights(
                        frame,
                        group_text_start_x,
                        y,
                        &display_text_buf,
                        &adjusted_tokens,
                        &model.theme.syntax,
                        text_color,
                    );
                }
            }
        }

        // Cursors: only show in focused group when blink state is visible
        if is_focused && model.ui.cursor_visible {
            let primary_cursor_color = model.theme.editor.cursor_color.to_argb_u32();
            let secondary_cursor_color = model.theme.editor.secondary_cursor_color.to_argb_u32();

            for (idx, cursor) in editor.cursors.iter().enumerate() {
                let cursor_in_vertical_view = cursor.line >= editor.viewport.top_line
                    && cursor.line < editor.viewport.top_line + visible_lines;

                // Use get_line_cow for zero-allocation when line is contiguous
                let line_text = document.get_line_cow(cursor.line).unwrap_or_default();
                let visual_cursor_col = char_col_to_visual_col(&line_text, cursor.column);

                let cursor_in_horizontal_view = visual_cursor_col >= editor.viewport.left_column
                    && visual_cursor_col < editor.viewport.left_column + visible_columns;

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

                    // Cursor: 2px wide, line_height - 2 tall, offset by 1px from top
                    frame.fill_rect_px(x, y + 1, 2, line_height.saturating_sub(2), cursor_color);
                }
            }
        }

        // Preview cursors for rectangle selection (always visible during drag, no blink)
        if is_focused && editor.rectangle_selection.active {
            let secondary_cursor_color = model.theme.editor.secondary_cursor_color.to_argb_u32();

            for preview_pos in &editor.rectangle_selection.preview_cursors {
                let cursor_in_vertical_view = preview_pos.line >= editor.viewport.top_line
                    && preview_pos.line < editor.viewport.top_line + visible_lines;

                if !cursor_in_vertical_view {
                    continue;
                }

                // Use get_line_cow for zero-allocation when line is contiguous
                let line_text = document.get_line_cow(preview_pos.line).unwrap_or_default();
                let visual_cursor_col = char_col_to_visual_col(&line_text, preview_pos.column);

                let cursor_in_horizontal_view = visual_cursor_col >= editor.viewport.left_column
                    && visual_cursor_col < editor.viewport.left_column + visible_columns;

                if !cursor_in_horizontal_view {
                    continue;
                }

                let screen_line = preview_pos.line - editor.viewport.top_line;
                let cursor_visual_column = visual_cursor_col - editor.viewport.left_column;
                let x = (group_text_start_x as f32 + cursor_visual_column as f32 * char_width)
                    .round() as usize;
                let y = content_y + screen_line * line_height;

                frame.fill_rect_px(
                    x,
                    y + 1,
                    2,
                    line_height.saturating_sub(2),
                    secondary_cursor_color,
                );
            }
        }
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
        csv: &token::csv::CsvState,
        layout: &geometry::GroupLayout,
        is_focused: bool,
    ) {
        use token::csv::render::{column_to_letters, truncate_text, CsvRenderLayout};

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
                let (text_color, align_right) = if token::csv::render::is_number(cell_value) {
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
        csv: &token::csv::CsvState,
        layout: &token::csv::render::CsvRenderLayout,
        edit_state: &token::csv::CellEditState,
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
        let edit_fg = model.theme.overlay.foreground.to_argb_u32();
        let cursor_color = model.theme.editor.cursor_color.to_argb_u32();

        // Draw edit background (fill entire cell)
        frame.fill_rect_px(
            cell_x + 1,
            cell_y + 1,
            col_width_px.saturating_sub(2),
            line_height.saturating_sub(2),
            edit_bg,
        );

        // Draw edit text
        let text_x = cell_x + 4;
        let buffer_text = edit_state.buffer();
        painter.draw(frame, text_x, cell_y + 1, &buffer_text, edit_fg);

        // Draw cursor if visible (blinking)
        if model.ui.cursor_visible {
            let cursor_char_pos = edit_state.cursor_char_position();
            let cursor_x = text_x + (cursor_char_pos as f32 * char_width).round() as usize;
            frame.fill_rect_px(
                cursor_x,
                cell_y + 2,
                2,
                line_height.saturating_sub(4),
                cursor_color,
            );
        }
    }

    /// Render the status bar at the bottom of the window.
    ///
    /// This is a standalone widget function that draws:
    /// - Status bar background
    /// - Left-aligned segments (mode, filename, position, etc.)
    /// - Right-aligned segments
    /// - Separators between segments
    fn render_status_bar(
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

        // Background
        frame.fill_rect_px(0, status_y, window_width, status_bar_h, status_bar_bg);

        // Layout calculation
        let available_chars = (window_width as f32 / char_width).floor() as usize;
        let layout = model.ui.status_bar.layout(available_chars);

        // Left segments
        for seg in &layout.left {
            let x_px = (seg.x as f32 * char_width).round() as usize;
            painter.draw(frame, x_px, status_y + 2, &seg.text, status_bar_fg);
        }

        // Right segments
        for seg in &layout.right {
            let x_px = (seg.x as f32 * char_width).round() as usize;
            painter.draw(frame, x_px, status_y + 2, &seg.text, status_bar_fg);
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

    /// Render the active modal overlay.
    ///
    /// Draws:
    /// - Dimmed background over entire window
    /// - Modal dialog box (centered)
    /// - Modal content (title, input field, command list for palette)
    fn render_modals(
        frame: &mut Frame,
        painter: &mut TextPainter,
        model: &AppModel,
        window_width: usize,
        window_height: usize,
    ) {
        use token::commands::filter_commands;
        use token::model::ModalState;
        use token::theme::ThemeSource;
        let char_width = painter.char_width();
        let line_height = painter.line_height();

        let Some(ref modal) = model.ui.active_modal else {
            return;
        };

        // 1. Dim background (40% black overlay)
        frame.dim(0x66); // 102/255 ≈ 40% opacity

        // Theme colors
        let bg_color = model.theme.overlay.background.to_argb_u32();
        let fg_color = model.theme.overlay.foreground.to_argb_u32();
        let highlight_color = model.theme.overlay.highlight.to_argb_u32();
        let dim_color = model.theme.overlay.foreground.with_alpha(128).to_argb_u32();
        let selection_bg = model.theme.overlay.selection_background.to_argb_u32();
        let input_bg = model.theme.overlay.input_background.to_argb_u32();
        let border_color = model
            .theme
            .overlay
            .border
            .map(|c| c.to_argb_u32())
            .unwrap_or(0xFF444444);

        // Handle different modal types
        match modal {
            ModalState::ThemePicker(state) => {
                // Theme picker: sectioned list (User / Builtin)
                let themes = &state.themes;

                // Count themes by source for section headers
                let has_user = themes.iter().any(|t| t.source == ThemeSource::User);
                let has_builtin = themes.iter().any(|t| t.source == ThemeSource::Builtin);
                let section_count = has_user as usize + has_builtin as usize;

                // Calculate visible rows: themes + section headers
                let total_rows = themes.len() + section_count;
                let list_height = total_rows * line_height;
                let modal_height = 8 + line_height + 8 + list_height + 8; // title + gap + list + padding
                let modal_width = 400;
                let modal_x = (window_width.saturating_sub(modal_width)) / 2;
                let modal_y = window_height / 4;

                frame.draw_bordered_rect(
                    modal_x,
                    modal_y,
                    modal_width,
                    modal_height,
                    bg_color,
                    border_color,
                );

                // Title
                let title_x = modal_x + 12;
                let title_y = modal_y + 8;
                painter.draw(frame, title_x, title_y, "Switch Theme", fg_color);

                // Theme list with sections
                let list_y = title_y + line_height + 8;
                let clamped_selected = state.selected_index.min(themes.len().saturating_sub(1));

                let mut current_y = list_y;
                let mut current_source: Option<ThemeSource> = None;
                let dim_color = 0xFF666666; // Dimmed color for section headers

                for (i, theme_info) in themes.iter().enumerate() {
                    // Draw section header when source changes
                    if current_source != Some(theme_info.source) {
                        current_source = Some(theme_info.source);
                        let header = match theme_info.source {
                            ThemeSource::User => "User Themes",
                            ThemeSource::Builtin => "Built-in Themes",
                        };
                        painter.draw(frame, modal_x + 12, current_y, header, dim_color);
                        current_y += line_height;
                    }

                    let is_selected = i == clamped_selected;

                    if is_selected {
                        frame.fill_rect_px(
                            modal_x + 4,
                            current_y,
                            modal_width - 8,
                            line_height,
                            selection_bg,
                        );
                    }

                    // Draw theme name with indent
                    let label_x = modal_x + 24;
                    painter.draw(frame, label_x, current_y, &theme_info.name, fg_color);

                    // Show checkmark for current theme
                    if model.theme.name == theme_info.name || model.config.theme == theme_info.id {
                        let check_x = modal_x + modal_width - 30;
                        painter.draw(frame, check_x, current_y, "✓", highlight_color);
                    }

                    current_y += line_height;
                }
            }

            ModalState::CommandPalette(state) => {
                let input_text = state.input();
                let filtered_commands = filter_commands(&input_text);
                let max_visible_items = 8;

                let (modal_x, modal_y, modal_width, modal_height) = geometry::modal_bounds(
                    window_width,
                    window_height,
                    line_height,
                    true,
                    filtered_commands.len(),
                );

                frame.draw_bordered_rect(
                    modal_x,
                    modal_y,
                    modal_width,
                    modal_height,
                    bg_color,
                    border_color,
                );

                // Title
                let title_x = modal_x + 12;
                let title_y = modal_y + 8;
                painter.draw(frame, title_x, title_y, "Command Palette", fg_color);

                // Input field background
                let input_x = modal_x + 12;
                let input_y = title_y + line_height + 4;
                let input_width = modal_width - 24;
                let input_height = line_height + 8;
                frame.fill_rect_px(input_x, input_y, input_width, input_height, input_bg);

                // Render text field using unified renderer
                let text_x = input_x + 8;
                let text_y = input_y + 4;
                let text_width = input_width - 16;
                let opts = TextFieldOptions {
                    x: text_x,
                    y: text_y,
                    width: text_width,
                    height: line_height,
                    char_width,
                    text_color: fg_color,
                    cursor_color: highlight_color,
                    selection_color: selection_bg,
                    cursor_visible: model.ui.cursor_visible,
                    scroll_x: 0,
                };
                TextFieldRenderer::render(frame, painter, &state.editable, &opts);

                // Command list
                if !filtered_commands.is_empty() {
                    let list_y = input_y + input_height + 8;
                    let total_items = filtered_commands.len();
                    let clamped_selected = state.selected_index.min(total_items.saturating_sub(1));

                    // Compute scroll offset to keep selected item visible
                    let scroll_offset = if clamped_selected >= max_visible_items {
                        clamped_selected + 1 - max_visible_items
                    } else {
                        0
                    };

                    for (i, cmd) in filtered_commands
                        .iter()
                        .skip(scroll_offset)
                        .take(max_visible_items)
                        .enumerate()
                    {
                        let actual_index = scroll_offset + i;
                        let item_y = list_y + i * line_height;
                        let is_selected = actual_index == clamped_selected;

                        if is_selected {
                            frame.fill_rect_px(
                                modal_x + 4,
                                item_y,
                                modal_width - 8,
                                line_height,
                                selection_bg,
                            );
                        }

                        painter.draw(frame, modal_x + 16, item_y, cmd.label, fg_color);

                        if let Some(kb) = cmd.keybinding {
                            let kb_width =
                                (kb.chars().count() as f32 * char_width).round() as usize;
                            let kb_x = modal_x + modal_width - kb_width - 16;
                            painter.draw(frame, kb_x, item_y, kb, dim_color);
                        }
                    }

                    // Show "and X more" for items after the visible window
                    let items_after = total_items.saturating_sub(scroll_offset + max_visible_items);
                    if items_after > 0 {
                        let more_y = list_y + max_visible_items * line_height;
                        let more_text = format!("... and {} more", items_after);
                        painter.draw(frame, modal_x + 16, more_y, &more_text, dim_color);
                    }
                }
            }

            ModalState::GotoLine(state) => {
                let (modal_x, modal_y, modal_width, modal_height) =
                    geometry::modal_bounds(window_width, window_height, line_height, false, 0);

                frame.draw_bordered_rect(
                    modal_x,
                    modal_y,
                    modal_width,
                    modal_height,
                    bg_color,
                    border_color,
                );

                // Title
                let title_x = modal_x + 12;
                let title_y = modal_y + 8;
                painter.draw(frame, title_x, title_y, "Go to Line", fg_color);

                // Input field background
                let input_x = modal_x + 12;
                let input_y = title_y + line_height + 4;
                let input_width = modal_width - 24;
                let input_height = line_height + 8;
                frame.fill_rect_px(input_x, input_y, input_width, input_height, input_bg);

                // Render text field using unified renderer
                let text_x = input_x + 8;
                let text_y = input_y + 4;
                let text_width = input_width - 16;
                let opts = TextFieldOptions {
                    x: text_x,
                    y: text_y,
                    width: text_width,
                    height: line_height,
                    char_width,
                    text_color: fg_color,
                    cursor_color: highlight_color,
                    selection_color: selection_bg,
                    cursor_visible: model.ui.cursor_visible,
                    scroll_x: 0,
                };
                TextFieldRenderer::render(frame, painter, &state.editable, &opts);
            }

            ModalState::FindReplace(state) => {
                let (modal_x, modal_y, modal_width, modal_height) =
                    geometry::modal_bounds(window_width, window_height, line_height, false, 0);

                frame.draw_bordered_rect(
                    modal_x,
                    modal_y,
                    modal_width,
                    modal_height,
                    bg_color,
                    border_color,
                );

                // Title
                let title_x = modal_x + 12;
                let title_y = modal_y + 8;
                painter.draw(frame, title_x, title_y, "Find", fg_color);

                // Input field background
                let input_x = modal_x + 12;
                let input_y = title_y + line_height + 4;
                let input_width = modal_width - 24;
                let input_height = line_height + 8;
                frame.fill_rect_px(input_x, input_y, input_width, input_height, input_bg);

                // Render text field using unified renderer
                let text_x = input_x + 8;
                let text_y = input_y + 4;
                let text_width = input_width - 16;
                let opts = TextFieldOptions {
                    x: text_x,
                    y: text_y,
                    width: text_width,
                    height: line_height,
                    char_width,
                    text_color: fg_color,
                    cursor_color: highlight_color,
                    selection_color: selection_bg,
                    cursor_visible: model.ui.cursor_visible,
                    scroll_x: 0,
                };
                TextFieldRenderer::render(frame, painter, state.focused_editable(), &opts);
            }
        }
    }

    /// Render the file drop overlay when files are being dragged over the window.
    fn render_drop_overlay(
        frame: &mut Frame,
        painter: &mut TextPainter,
        model: &AppModel,
        window_width: usize,
        window_height: usize,
    ) {
        let char_width = painter.char_width();
        let line_height = painter.line_height();

        // Semi-transparent overlay covering the entire window
        frame.dim(0x80); // 50% dim

        // Draw centered drop zone
        let text = model.ui.drop_state.display_text();
        let text_len = text.chars().count();

        let box_width = ((text_len as f32 + 4.0) * char_width).round() as usize;
        let box_height = line_height * 3;
        let box_x = (window_width.saturating_sub(box_width)) / 2;
        let box_y = (window_height.saturating_sub(box_height)) / 2;

        let bg_color = model.theme.overlay.background.to_argb_u32();
        let border_color = model.theme.overlay.highlight.to_argb_u32();
        let fg_color = model.theme.overlay.foreground.to_argb_u32();

        frame.draw_bordered_rect(box_x, box_y, box_width, box_height, bg_color, border_color);

        // Centered text
        let text_x = box_x + (box_width - (text_len as f32 * char_width).round() as usize) / 2;
        let text_y = box_y + line_height;

        painter.draw(frame, text_x, text_y, &text, fg_color);
    }

    pub fn render(
        &mut self,
        model: &mut AppModel,
        perf: &mut crate::runtime::perf::PerfStats,
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
        let width = self.width;
        let height = self.height;

        let status_bar_height = line_height;

        // Calculate sidebar offset if workspace is open and sidebar is visible
        let sidebar_width = model
            .workspace
            .as_ref()
            .filter(|ws| ws.sidebar_visible)
            .map(|ws| ws.sidebar_width(model.metrics.scale_factor))
            .unwrap_or(0.0);

        // Editor area starts after sidebar
        let available_rect = Rect::new(
            sidebar_width,
            0.0,
            (width as f32) - sidebar_width,
            (height as usize).saturating_sub(status_bar_height) as f32,
        );
        let splitters = model
            .editor_area
            .compute_layout_scaled(available_rect, model.metrics.splitter_width);

        // Create Frame wrapper for cleaner drawing API
        // Note: We use Frame for new code; legacy code still uses raw buffer slices
        let width_usize = width as usize;
        let height_usize = height as usize;

        // Compute effective damage: force Full for complex overlays
        // (Must be done before borrowing buffer)
        let effective_damage = self.compute_effective_damage(damage, model, perf);

        // Determine what regions need to be rendered
        let render_editor = effective_damage.is_full()
            || effective_damage.includes_editor()
            || has_cursor_lines_damage(&effective_damage);
        let render_status_bar =
            effective_damage.is_full() || effective_damage.includes_status_bar();

        // All rendering happens to back_buffer (persistent between frames).
        // At the end, we copy to the surface buffer and present.

        // Clear and render based on damage
        let bg_color = model.theme.editor.background.to_argb_u32();

        // Check for cursor-lines-only damage early (before clearing)
        let cursor_lines_damage = effective_damage.cursor_lines_only().map(|v| v.to_vec());

        if effective_damage.is_full() {
            // Full redraw: clear entire screen
            let _timer = perf.time_clear();
            let mut frame = Frame::new(&mut self.back_buffer, width_usize, height_usize);
            frame.clear(bg_color);
        } else if cursor_lines_damage.is_some() {
            // Cursor lines only: clear just those line rectangles
            // (clearing happens in render_cursor_lines_only for precise line bounds)
        } else {
            // Partial redraw: only clear damaged regions
            let _timer = perf.time_clear();
            let mut frame = Frame::new(&mut self.back_buffer, width_usize, height_usize);

            if render_editor {
                // Clear editor area (everything except status bar)
                let editor_rect = Rect::new(
                    0.0,
                    0.0,
                    width as f32,
                    height_usize.saturating_sub(status_bar_height) as f32,
                );
                frame.fill_rect(editor_rect, bg_color);
            }

            if render_status_bar {
                // Clear status bar area
                let status_bg = model.theme.status_bar.background.to_argb_u32();
                let status_rect = Rect::new(
                    0.0,
                    height_usize.saturating_sub(status_bar_height) as f32,
                    width as f32,
                    status_bar_height as f32,
                );
                frame.fill_rect(status_rect, status_bg);
            }
        }

        // Render editor area if needed
        // Check for cursor-lines-only damage for optimized rendering
        let cursor_lines_only = effective_damage.cursor_lines_only();

        if let Some(dirty_lines) = cursor_lines_only {
            // Optimized path: only redraw specific cursor lines
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
        } else if render_editor {
            // Full editor area render
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
                Self::render_editor_area(&mut frame, &mut painter, model, &splitters);
            }
            #[cfg(debug_assertions)]
            {
                let stats = painter.cache_stats();
                perf.add_cache_stats(stats.hits, stats.misses);
            }
        }

        // Render sidebar if workspace is open and editor is being rendered
        if sidebar_width > 0.0 && render_editor {
            let mut frame = Frame::new(&mut self.back_buffer, width_usize, height_usize);
            let mut painter = TextPainter::new(
                &self.font,
                &mut self.glyph_cache,
                font_size,
                ascent,
                char_width,
                line_height,
            );
            Self::render_sidebar(
                &mut frame,
                &mut painter,
                model,
                sidebar_width as usize,
                height_usize.saturating_sub(status_bar_height),
            );
            #[cfg(debug_assertions)]
            {
                let stats = painter.cache_stats();
                perf.add_cache_stats(stats.hits, stats.misses);
            }
        }

        // Render status bar if needed
        if render_status_bar {
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
                let _timer = perf.time_status_bar();
                Self::render_status_bar(&mut frame, &mut painter, model, width_usize, height_usize);
            }
            #[cfg(debug_assertions)]
            {
                let stats = painter.cache_stats();
                perf.add_cache_stats(stats.hits, stats.misses);
            }
        }

        // Render modals (layer 2 - on top of editor and status bar)
        if model.ui.active_modal.is_some() {
            let mut frame = Frame::new(&mut self.back_buffer, width_usize, height_usize);
            let mut painter = TextPainter::new(
                &self.font,
                &mut self.glyph_cache,
                font_size,
                ascent,
                char_width,
                line_height,
            );
            Self::render_modals(&mut frame, &mut painter, model, width_usize, height_usize);
            #[cfg(debug_assertions)]
            {
                let stats = painter.cache_stats();
                perf.add_cache_stats(stats.hits, stats.misses);
            }
        }

        // Render drop overlay (layer 3 - on top of modals)
        if model.ui.drop_state.is_hovering {
            let mut frame = Frame::new(&mut self.back_buffer, width_usize, height_usize);
            let mut painter = TextPainter::new(
                &self.font,
                &mut self.glyph_cache,
                font_size,
                ascent,
                char_width,
                line_height,
            );
            Self::render_drop_overlay(&mut frame, &mut painter, model, width_usize, height_usize);
            #[cfg(debug_assertions)]
            {
                let stats = painter.cache_stats();
                perf.add_cache_stats(stats.hits, stats.misses);
            }
        }

        #[cfg(debug_assertions)]
        if perf.should_show_overlay() {
            let mut frame = Frame::new(&mut self.back_buffer, width_usize, height_usize);
            let mut painter = TextPainter::new(
                &self.font,
                &mut self.glyph_cache,
                font_size,
                ascent,
                char_width,
                line_height,
            );
            crate::runtime::perf::render_perf_overlay(&mut frame, &mut painter, perf, &model.theme);
        }

        #[cfg(debug_assertions)]
        if let Some(ref overlay) = model.debug_overlay {
            if overlay.visible {
                let lines = overlay.render_lines(model);
                if !lines.is_empty() {
                    let mut frame = Frame::new(&mut self.back_buffer, width_usize, height_usize);
                    let mut painter = TextPainter::new(
                        &self.font,
                        &mut self.glyph_cache,
                        font_size,
                        ascent,
                        char_width,
                        line_height,
                    );

                    // Calculate dimensions
                    let max_line_len = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0);
                    let overlay_width =
                        (max_line_len as f32 * painter.char_width()).ceil() as usize + 20;
                    let overlay_height = lines.len() * painter.line_height() + 10;

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
                        let text_y = overlay_y + 5 + i * painter.line_height();
                        painter.draw(&mut frame, text_x, text_y, line, fg_color);
                    }
                }
            }
        }

        // Debug: visualize damage regions with colored outlines
        #[cfg(feature = "damage-debug")]
        {
            let mut frame = Frame::new(&mut self.back_buffer, width_usize, height_usize);
            Self::render_damage_debug(
                &mut frame,
                &effective_damage,
                model,
                width_usize,
                height_usize,
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

    /// Returns the tab index at the given x position within a group's tab bar.
    /// Returns None if the click is not on a tab.
    /// Delegates to geometry module for the actual calculation.
    pub fn tab_at_position(&self, x: f64, model: &AppModel, group: &EditorGroup) -> Option<usize> {
        geometry::tab_at_position(x, self.char_width, model, group)
    }

    /// Hit-test a CSV cell given window coordinates.
    /// Returns None if the click is outside the data grid or editor is not in CSV mode.
    pub fn pixel_to_csv_cell(
        &self,
        x: f64,
        y: f64,
        model: &AppModel,
    ) -> Option<token::csv::CellPosition> {
        let group = model.editor_area.focused_group()?;
        let editor = model.editor_area.focused_editor()?;
        let csv = editor.view_mode.as_csv()?;

        let line_height = self.line_metrics.new_line_size.ceil() as usize;

        token::csv::render::pixel_to_csv_cell(
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
