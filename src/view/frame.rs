//! Frame abstraction for drawing primitives
//!
//! Provides a simple, safe API for pixel buffer operations instead of
//! direct buffer indexing scattered throughout rendering code.

use fontdue::Font;
use token::model::editor_area::Rect;

use super::GlyphCache;

/// A frame buffer wrapper providing safe drawing primitives.
///
/// All coordinates are in pixels. Out-of-bounds operations are safely clipped.
pub struct Frame<'a> {
    pub buffer: &'a mut [u32],
    pub width: usize,
    pub height: usize,
}

impl<'a> Frame<'a> {
    /// Create a new frame from a mutable pixel buffer
    pub fn new(buffer: &'a mut [u32], width: usize, height: usize) -> Self {
        Self {
            buffer,
            width,
            height,
        }
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

    /// Set a single pixel (bounds-checked)
    #[inline]
    pub fn set_pixel(&mut self, x: usize, y: usize, color: u32) {
        if x < self.width && y < self.height {
            self.buffer[y * self.width + x] = color;
        }
    }

    /// Get a single pixel (bounds-checked, returns 0 if out of bounds)
    #[inline]
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
        let bg = self.buffer[idx];

        let alpha = ((color >> 24) & 0xFF) as f32 / 255.0;
        if alpha <= 0.0 {
            return;
        }
        if alpha >= 1.0 {
            self.buffer[idx] = color | 0xFF000000;
            return;
        }

        let bg_r = ((bg >> 16) & 0xFF) as f32;
        let bg_g = ((bg >> 8) & 0xFF) as f32;
        let bg_b = (bg & 0xFF) as f32;

        let fg_r = ((color >> 16) & 0xFF) as f32;
        let fg_g = ((color >> 8) & 0xFF) as f32;
        let fg_b = (color & 0xFF) as f32;

        let final_r = (bg_r * (1.0 - alpha) + fg_r * alpha) as u32;
        let final_g = (bg_g * (1.0 - alpha) + fg_g * alpha) as u32;
        let final_b = (bg_b * (1.0 - alpha) + fg_b * alpha) as u32;

        self.buffer[idx] = 0xFF000000 | (final_r << 16) | (final_g << 8) | final_b;
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
        // Fill background
        self.fill_rect_px(x, y, w, h, fill_color);

        // Draw border (1px on each edge)
        // Top
        self.fill_rect_px(x, y, w, 1, border_color);
        // Bottom
        self.fill_rect_px(x, y + h.saturating_sub(1), w, 1, border_color);
        // Left
        self.fill_rect_px(x, y, 1, h, border_color);
        // Right
        self.fill_rect_px(x + w.saturating_sub(1), y, 1, h, border_color);
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

/// Text rendering context wrapping font and glyph cache
pub struct TextPainter<'a> {
    pub font: &'a Font,
    pub glyph_cache: &'a mut GlyphCache,
    pub font_size: f32,
    pub ascent: f32,
}

impl<'a> TextPainter<'a> {
    /// Create a new text painter
    pub fn new(
        font: &'a Font,
        glyph_cache: &'a mut GlyphCache,
        font_size: f32,
        ascent: f32,
    ) -> Self {
        Self {
            font,
            glyph_cache,
            font_size,
            ascent,
        }
    }

    /// Draw text at the specified position
    pub fn draw(&mut self, frame: &mut Frame, x: usize, y: usize, text: &str, color: u32) {
        let mut current_x = x as f32;
        let baseline = y as f32 + self.ascent;

        for ch in text.chars() {
            let key = (ch, self.font_size.to_bits());
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
                                    let bg_pixel = frame.buffer[py * frame.width + px];

                                    let bg_r = ((bg_pixel >> 16) & 0xFF) as f32;
                                    let bg_g = ((bg_pixel >> 8) & 0xFF) as f32;
                                    let bg_b = (bg_pixel & 0xFF) as f32;

                                    let fg_r = ((color >> 16) & 0xFF) as f32;
                                    let fg_g = ((color >> 8) & 0xFF) as f32;
                                    let fg_b = (color & 0xFF) as f32;

                                    let final_r = (bg_r * (1.0 - alpha_f) + fg_r * alpha_f) as u32;
                                    let final_g = (bg_g * (1.0 - alpha_f) + fg_g * alpha_f) as u32;
                                    let final_b = (bg_b * (1.0 - alpha_f) + fg_b * alpha_f) as u32;

                                    frame.buffer[py * frame.width + px] =
                                        0xFF000000 | (final_r << 16) | (final_g << 8) | final_b;
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
