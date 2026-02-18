//! Frame abstraction for drawing primitives
//!
//! Provides a simple, safe API for pixel buffer operations instead of
//! direct buffer indexing scattered throughout rendering code.

use crate::model::editor_area::Rect;
use fontdue::Font;

use super::GlyphCache;

/// Blend a foreground color onto a background color using alpha compositing.
///
/// Both colors are in ARGB format (0xAARRGGBB). The alpha value from the
/// foreground color determines the blend ratio.
///
/// Returns the blended color with full opacity (alpha = 0xFF).
#[inline]
pub fn blend_colors(bg: u32, fg: u32, alpha: f32) -> u32 {
    let bg_r = ((bg >> 16) & 0xFF) as f32;
    let bg_g = ((bg >> 8) & 0xFF) as f32;
    let bg_b = (bg & 0xFF) as f32;

    let fg_r = ((fg >> 16) & 0xFF) as f32;
    let fg_g = ((fg >> 8) & 0xFF) as f32;
    let fg_b = (fg & 0xFF) as f32;

    let final_r = (bg_r * (1.0 - alpha) + fg_r * alpha) as u32;
    let final_g = (bg_g * (1.0 - alpha) + fg_g * alpha) as u32;
    let final_b = (bg_b * (1.0 - alpha) + fg_b * alpha) as u32;

    0xFF000000 | (final_r << 16) | (final_g << 8) | final_b
}

/// A frame buffer wrapper providing safe drawing primitives.
///
/// All coordinates are in pixels. Out-of-bounds operations are safely clipped.
pub struct Frame<'a> {
    buffer: &'a mut [u32],
    width: usize,
    height: usize,
}

impl<'a> Frame<'a> {
    /// Create a new frame from a mutable pixel buffer
    ///
    /// If the buffer is smaller than width*height, dimensions are adjusted
    /// to match the actual buffer size to prevent out-of-bounds access.
    pub fn new(buffer: &'a mut [u32], width: usize, height: usize) -> Self {
        let expected_size = width * height;
        let actual_size = buffer.len();

        let (width, height) = if actual_size < expected_size && width > 0 {
            // Buffer is smaller than expected - recalculate height to fit
            let adjusted_height = actual_size / width;
            (width, adjusted_height)
        } else {
            (width, height)
        };

        Self {
            buffer,
            width,
            height,
        }
    }

    /// Get the frame width in pixels
    #[inline]
    #[allow(dead_code)]
    pub fn width(&self) -> usize {
        self.width
    }

    /// Get the frame height in pixels
    #[inline]
    #[allow(dead_code)]
    pub fn height(&self) -> usize {
        self.height
    }

    /// Get mutable access to the underlying pixel buffer
    ///
    /// Use this for low-level operations that need direct buffer access.
    /// Prefer using Frame's drawing methods when possible.
    #[inline]
    #[allow(dead_code)]
    pub fn buffer_mut(&mut self) -> &mut [u32] {
        self.buffer
    }

    /// Clear the entire buffer with a solid color
    #[inline]
    pub fn clear(&mut self, color: u32) {
        self.buffer.fill(color);
    }

    /// Fill a rectangle with a solid color (no alpha blending)
    pub fn fill_rect(&mut self, rect: Rect, color: u32) {
        let x0 = (rect.x.max(0.0) as usize).min(self.width);
        let y0 = (rect.y.max(0.0) as usize).min(self.height);
        let x1 = ((rect.x + rect.width) as usize).min(self.width);
        let y1 = ((rect.y + rect.height) as usize).min(self.height);

        for y in y0..y1 {
            let row_start = y * self.width;
            for x in x0..x1 {
                self.buffer[row_start + x] = color;
            }
        }
    }

    /// Fill a rectangle specified by pixel coordinates
    pub fn fill_rect_px(&mut self, x: usize, y: usize, w: usize, h: usize, color: u32) {
        let x0 = x.min(self.width);
        let y0 = y.min(self.height);
        let x1 = (x + w).min(self.width);
        let y1 = (y + h).min(self.height);

        for py in y0..y1 {
            let row_start = py * self.width;
            for px in x0..x1 {
                self.buffer[row_start + px] = color;
            }
        }
    }

    /// Fill a rectangle with alpha blending (pixel coordinates, ARGB format)
    #[cfg_attr(not(feature = "damage-debug"), allow(dead_code))]
    pub fn blend_rect_px(&mut self, x: usize, y: usize, w: usize, h: usize, color: u32) {
        let alpha = ((color >> 24) & 0xFF) as f32 / 255.0;
        if alpha <= 0.0 {
            return;
        }
        if alpha >= 1.0 {
            return self.fill_rect_px(x, y, w, h, color | 0xFF000000);
        }

        let x0 = x.min(self.width);
        let y0 = y.min(self.height);
        let x1 = (x + w).min(self.width);
        let y1 = (y + h).min(self.height);

        for py in y0..y1 {
            let row_start = py * self.width;
            for px in x0..x1 {
                self.buffer[row_start + px] =
                    blend_colors(self.buffer[row_start + px], color, alpha);
            }
        }
    }

    /// Fill a rectangle with alpha blending (color is ARGB format)
    pub fn fill_rect_blended(&mut self, rect: Rect, color: u32) {
        let alpha = ((color >> 24) & 0xFF) as f32 / 255.0;
        if alpha <= 0.0 {
            return;
        }
        if alpha >= 1.0 {
            return self.fill_rect(rect, color | 0xFF000000);
        }

        let x0 = (rect.x.max(0.0) as usize).min(self.width);
        let y0 = (rect.y.max(0.0) as usize).min(self.height);
        let x1 = ((rect.x + rect.width) as usize).min(self.width);
        let y1 = ((rect.y + rect.height) as usize).min(self.height);

        for y in y0..y1 {
            let row_start = y * self.width;
            for x in x0..x1 {
                let idx = row_start + x;
                self.buffer[idx] = blend_colors(self.buffer[idx], color, alpha);
            }
        }
    }

    /// Set a single pixel (bounds-checked)
    #[inline]
    #[allow(dead_code)]
    pub fn set_pixel(&mut self, x: usize, y: usize, color: u32) {
        if x < self.width && y < self.height {
            self.buffer[y * self.width + x] = color;
        }
    }

    /// Get a single pixel (bounds-checked, returns 0 if out of bounds)
    #[inline]
    #[allow(dead_code)]
    pub fn get_pixel(&self, x: usize, y: usize) -> u32 {
        if x < self.width && y < self.height {
            self.buffer[y * self.width + x]
        } else {
            0
        }
    }

    /// Blend a pixel with alpha (ARGB format, alpha in high byte)
    #[inline]
    pub fn blend_pixel(&mut self, x: usize, y: usize, color: u32) {
        if x >= self.width || y >= self.height {
            return;
        }

        let idx = y * self.width + x;
        let alpha = ((color >> 24) & 0xFF) as f32 / 255.0;
        if alpha <= 0.0 {
            return;
        }
        if alpha >= 1.0 {
            self.buffer[idx] = color | 0xFF000000;
            return;
        }

        self.buffer[idx] = blend_colors(self.buffer[idx], color, alpha);
    }

    /// Fill a rectangle with alpha blending
    pub fn blend_rect(&mut self, rect: Rect, color: u32) {
        let x0 = (rect.x.max(0.0) as usize).min(self.width);
        let y0 = (rect.y.max(0.0) as usize).min(self.height);
        let x1 = ((rect.x + rect.width) as usize).min(self.width);
        let y1 = ((rect.y + rect.height) as usize).min(self.height);

        for y in y0..y1 {
            for x in x0..x1 {
                self.blend_pixel(x, y, color);
            }
        }
    }

    /// Dim the entire frame with a semi-transparent overlay
    /// Useful for modal backgrounds
    pub fn dim(&mut self, alpha: u8) {
        let dim_color = (alpha as u32) << 24; // Black with given alpha
        for y in 0..self.height {
            for x in 0..self.width {
                self.blend_pixel(x, y, dim_color);
            }
        }
    }

    /// Draw a rectangle with a 1px border
    pub fn draw_bordered_rect(
        &mut self,
        x: usize,
        y: usize,
        w: usize,
        h: usize,
        fill_color: u32,
        border_color: u32,
    ) {
        // Fill background (blend to handle semi-transparent overlay backgrounds)
        let alpha = (fill_color >> 24) & 0xFF;
        if alpha == 0xFF {
            self.fill_rect_px(x, y, w, h, fill_color);
        } else {
            self.blend_rect_px(x, y, w, h, fill_color);
        }

        // Draw border (1px on each edge, always opaque)
        let opaque_border = border_color | 0xFF000000;
        // Top
        self.fill_rect_px(x, y, w, 1, opaque_border);
        // Bottom
        self.fill_rect_px(x, y + h.saturating_sub(1), w, 1, opaque_border);
        // Left
        self.fill_rect_px(x, y, 1, h, opaque_border);
        // Right
        self.fill_rect_px(x + w.saturating_sub(1), y, 1, h, opaque_border);
    }

    /// Draw a sparkline chart (used by perf overlay)
    #[cfg(debug_assertions)]
    #[allow(clippy::too_many_arguments)]
    pub fn draw_sparkline(
        &mut self,
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

        self.fill_rect_px(x, y, chart_width, chart_height, bg_color);

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
                    self.set_pixel(px, py, bar_color);
                }
            }
        }
    }
}

/// Statistics for glyph cache hit/miss tracking (debug only)
#[cfg(debug_assertions)]
#[derive(Default)]
#[allow(dead_code)]
pub struct CacheStats {
    pub hits: usize,
    pub misses: usize,
}

/// Text rendering context wrapping font and glyph cache.
///
/// Provides methods for drawing text with proper font metrics and glyph caching.
pub struct TextPainter<'a> {
    font: &'a Font,
    glyph_cache: &'a mut GlyphCache,
    font_size: f32,
    ascent: f32,
    char_width: f32,
    line_height: usize,
    #[cfg(debug_assertions)]
    #[allow(dead_code)]
    cache_stats: CacheStats,
}

impl<'a> TextPainter<'a> {
    /// Create a new text painter
    pub fn new(
        font: &'a Font,
        glyph_cache: &'a mut GlyphCache,
        font_size: f32,
        ascent: f32,
        char_width: f32,
        line_height: usize,
    ) -> Self {
        Self {
            font,
            glyph_cache,
            font_size,
            ascent,
            char_width,
            line_height,
            #[cfg(debug_assertions)]
            cache_stats: CacheStats::default(),
        }
    }

    /// Get the cache statistics (hits and misses)
    #[cfg(debug_assertions)]
    #[inline]
    #[allow(dead_code)]
    pub fn cache_stats(&self) -> &CacheStats {
        &self.cache_stats
    }

    /// Get the character width for monospace layout calculations
    #[inline]
    pub fn char_width(&self) -> f32 {
        self.char_width
    }

    /// Get the line height in pixels
    #[inline]
    pub fn line_height(&self) -> usize {
        self.line_height
    }

    /// Get the number of cached glyphs
    #[inline]
    #[allow(dead_code)]
    pub fn glyph_cache_size(&self) -> usize {
        self.glyph_cache.len()
    }

    /// Draw text at the specified position
    pub fn draw(&mut self, frame: &mut Frame, x: usize, y: usize, text: &str, color: u32) {
        let mut current_x = x as f32;
        let baseline = y as f32 + self.ascent;

        for ch in text.chars() {
            let key = (ch, self.font_size.to_bits());

            // Track cache hit/miss before lookup
            #[cfg(debug_assertions)]
            let is_hit = self.glyph_cache.contains_key(&key);
            #[cfg(debug_assertions)]
            if is_hit {
                self.cache_stats.hits += 1;
            } else {
                self.cache_stats.misses += 1;
            }

            let (metrics, bitmap) = self
                .glyph_cache
                .entry(key)
                .or_insert_with(|| self.font.rasterize(ch, self.font_size));

            let glyph_top = baseline - metrics.height as f32 - metrics.ymin as f32;

            for bitmap_y in 0..metrics.height {
                for bitmap_x in 0..metrics.width {
                    let bitmap_idx = bitmap_y * metrics.width + bitmap_x;
                    if bitmap_idx < bitmap.len() {
                        let alpha = bitmap[bitmap_idx];
                        if alpha > 0 {
                            let px = current_x as isize + bitmap_x as isize + metrics.xmin as isize;
                            let py = (glyph_top + bitmap_y as f32) as isize;

                            if px >= 0 && py >= 0 {
                                let px = px as usize;
                                let py = py as usize;

                                if px < frame.width && py < frame.height {
                                    let alpha_f = alpha as f32 / 255.0;
                                    let idx = py * frame.width + px;
                                    frame.buffer[idx] =
                                        blend_colors(frame.buffer[idx], color, alpha_f);
                                }
                            }
                        }
                    }
                }
            }

            current_x += metrics.advance_width;
        }
    }

    /// Measure text width in pixels
    #[allow(dead_code)]
    pub fn measure_width(&mut self, text: &str) -> f32 {
        let mut width = 0.0;
        for ch in text.chars() {
            let key = (ch, self.font_size.to_bits());
            let (metrics, _) = self
                .glyph_cache
                .entry(key)
                .or_insert_with(|| self.font.rasterize(ch, self.font_size));
            width += metrics.advance_width;
        }
        width
    }

    /// Draw text with syntax highlighting
    ///
    /// Applies per-character colors based on highlight tokens.
    /// Falls back to default_color for characters without highlighting.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_with_highlights(
        &mut self,
        frame: &mut Frame,
        x: usize,
        y: usize,
        text: &str,
        tokens: &[crate::syntax::HighlightToken],
        syntax_theme: &crate::theme::SyntaxTheme,
        default_color: u32,
    ) {
        if tokens.is_empty() {
            // No highlighting, use default color
            self.draw(frame, x, y, text, default_color);
            return;
        }

        let mut current_x = x as f32;
        let baseline = y as f32 + self.ascent;

        let mut token_idx = 0;

        for (col, ch) in text.chars().enumerate() {
            // Advance token_idx past any tokens that end before or at this column
            while token_idx < tokens.len() && tokens[token_idx].end_col <= col {
                token_idx += 1;
            }

            // Determine color for this character
            let color = if token_idx < tokens.len()
                && col >= tokens[token_idx].start_col
                && col < tokens[token_idx].end_col
            {
                syntax_theme
                    .color_for_highlight(tokens[token_idx].highlight)
                    .to_argb_u32()
            } else {
                default_color
            };

            // Draw the character
            let key = (ch, self.font_size.to_bits());

            // Track cache hit/miss before lookup
            #[cfg(debug_assertions)]
            let is_hit = self.glyph_cache.contains_key(&key);
            #[cfg(debug_assertions)]
            if is_hit {
                self.cache_stats.hits += 1;
            } else {
                self.cache_stats.misses += 1;
            }

            let (metrics, bitmap) = self
                .glyph_cache
                .entry(key)
                .or_insert_with(|| self.font.rasterize(ch, self.font_size));

            let glyph_top = baseline - metrics.height as f32 - metrics.ymin as f32;

            for bitmap_y in 0..metrics.height {
                for bitmap_x in 0..metrics.width {
                    let bitmap_idx = bitmap_y * metrics.width + bitmap_x;
                    if bitmap_idx < bitmap.len() {
                        let alpha = bitmap[bitmap_idx];
                        if alpha > 0 {
                            let px = current_x as isize + bitmap_x as isize + metrics.xmin as isize;
                            let py = (glyph_top + bitmap_y as f32) as isize;

                            if px >= 0 && py >= 0 {
                                let px = px as usize;
                                let py = py as usize;

                                if px < frame.width && py < frame.height {
                                    let alpha_f = alpha as f32 / 255.0;
                                    let idx = py * frame.width + px;
                                    frame.buffer[idx] =
                                        blend_colors(frame.buffer[idx], color, alpha_f);
                                }
                            }
                        }
                    }
                }
            }

            current_x += metrics.advance_width;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frame_fill_rect() {
        let mut buffer = vec![0u32; 100 * 100];
        let mut frame = Frame::new(&mut buffer, 100, 100);

        frame.fill_rect(
            Rect {
                x: 10.0,
                y: 10.0,
                width: 20.0,
                height: 20.0,
            },
            0xFFFF0000,
        );

        // Check a pixel inside the rect
        assert_eq!(frame.get_pixel(15, 15), 0xFFFF0000);
        // Check a pixel outside the rect
        assert_eq!(frame.get_pixel(5, 5), 0);
    }

    #[test]
    fn test_frame_blend_pixel() {
        let mut buffer = vec![0xFFFFFFFF_u32; 10 * 10]; // White background
        let mut frame = Frame::new(&mut buffer, 10, 10);

        // Blend 50% black
        frame.blend_pixel(5, 5, 0x80000000);

        let result = frame.get_pixel(5, 5);
        // Should be grayish (around 128 for each channel)
        let r = (result >> 16) & 0xFF;
        let g = (result >> 8) & 0xFF;
        let b = result & 0xFF;
        assert!(r > 100 && r < 160, "R channel: {}", r);
        assert!(g > 100 && g < 160, "G channel: {}", g);
        assert!(b > 100 && b < 160, "B channel: {}", b);
    }

    #[test]
    fn test_frame_out_of_bounds() {
        let mut buffer = vec![0u32; 10 * 10];
        let mut frame = Frame::new(&mut buffer, 10, 10);

        // These should not panic
        frame.set_pixel(100, 100, 0xFFFFFFFF);
        frame.blend_pixel(100, 100, 0x80FFFFFF);
        assert_eq!(frame.get_pixel(100, 100), 0);
    }
}
