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

/// Per-corner alpha coverage for a rounded-rect corner of a given physical
/// radius. Coverage is color-independent (just how much of each corner
/// pixel the circular arc covers), so one mask is shared by every themed
/// color drawn at that radius — the cache key is the radius alone.
pub struct RoundedCornerMask {
    radius: usize,
    coverage: Vec<u8>,
}

impl RoundedCornerMask {
    fn compute(radius: usize) -> Self {
        let r = radius as f32;
        let mut coverage = vec![0u8; radius * radius];
        for dy in 0..radius {
            for dx in 0..radius {
                let px = dx as f32 + 0.5;
                let py = dy as f32 + 0.5;
                let dist = ((r - px).powi(2) + (r - py).powi(2)).sqrt();
                // Solid inside the arc, transparent outside, with a ~1px
                // linear falloff band at the boundary for antialiasing.
                let alpha = if dist <= r - 0.5 {
                    255.0
                } else if dist >= r + 0.5 {
                    0.0
                } else {
                    (r + 0.5 - dist).clamp(0.0, 1.0) * 255.0
                };
                coverage[dy * radius + dx] = alpha.round() as u8;
            }
        }
        Self { radius, coverage }
    }

    #[inline]
    fn coverage(&self, dx: usize, dy: usize) -> u8 {
        if dx >= self.radius || dy >= self.radius {
            return 0;
        }
        self.coverage[dy * self.radius + dx]
    }
}

/// Cache of `RoundedCornerMask`s keyed by physical corner radius. Owned by
/// the caller (mirrors `GlyphCache`'s lifetime pattern) and threaded into
/// the rounded-rect primitives so masks survive across frames.
pub type RoundedRectMaskCache = std::collections::HashMap<usize, RoundedCornerMask>;

/// Clipping rectangle in pixel coordinates (inclusive start, exclusive end).
#[derive(Clone, Copy, Debug)]
struct ClipRect {
    x0: usize,
    y0: usize,
    x1: usize,
    y1: usize,
}

/// A frame buffer wrapper providing safe drawing primitives.
///
/// All coordinates are in pixels. Out-of-bounds operations are safely clipped.
pub struct Frame<'a> {
    buffer: &'a mut [u32],
    width: usize,
    height: usize,
    clip_stack: Vec<ClipRect>,
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
            clip_stack: Vec::new(),
        }
    }

    /// Get the frame width in pixels
    #[inline]
    pub fn width(&self) -> usize {
        self.width
    }

    /// Get the frame height in pixels
    #[inline]
    pub fn height(&self) -> usize {
        self.height
    }

    /// Get mutable access to the underlying pixel buffer
    ///
    /// Use this for low-level operations that need direct buffer access.
    /// Prefer using Frame's drawing methods when possible.
    #[inline]
    pub fn buffer_mut(&mut self) -> &mut [u32] {
        self.buffer
    }

    /// Convert a `Rect` into a frame-bounded `ClipRect`.
    fn clip_rect_from(&self, rect: Rect) -> ClipRect {
        // Clamp width/height to non-negative in f32 space before casting and
        // adding to x/y: a negative width previously could make `x1 < x0`
        // (an inverted clip rect) once both were independently clamped and
        // cast to `usize`.
        let w = rect.width.max(0.0);
        let h = rect.height.max(0.0);
        let x0 = (rect.x.max(0.0) as usize).min(self.width);
        let y0 = (rect.y.max(0.0) as usize).min(self.height);
        let x1 = ((rect.x.max(0.0) + w) as usize).min(self.width);
        let y1 = ((rect.y.max(0.0) + h) as usize).min(self.height);
        ClipRect { x0, y0, x1, y1 }
    }

    /// Set a clipping rectangle. All subsequent drawing operations will be
    /// constrained to this region. Coordinates are in pixels (Rect uses f32).
    ///
    /// Resets any nested clips: the stack is cleared and `rect` becomes the
    /// sole (absolute) clip. Prefer `push_clip`/`pop_clip` for nesting.
    pub fn set_clip(&mut self, rect: Rect) {
        self.clip_stack.clear();
        let clip = self.clip_rect_from(rect);
        self.clip_stack.push(clip);
    }

    /// Push a clipping rectangle, intersecting it with the current clip.
    /// Drawing is constrained to the intersection until the matching
    /// `pop_clip`. Push/pop pairs must be balanced within a frame.
    pub fn push_clip(&mut self, rect: Rect) {
        let mut clip = self.clip_rect_from(rect);
        if let Some(top) = self.clip_stack.last() {
            clip.x0 = clip.x0.max(top.x0);
            clip.y0 = clip.y0.max(top.y0);
            clip.x1 = clip.x1.min(top.x1).max(clip.x0);
            clip.y1 = clip.y1.min(top.y1).max(clip.y0);
        }
        self.clip_stack.push(clip);
    }

    /// Pop the most recently pushed clipping rectangle, restoring the
    /// enclosing clip (or full-frame drawing if the stack empties).
    pub fn pop_clip(&mut self) {
        debug_assert!(
            !self.clip_stack.is_empty(),
            "pop_clip called with no active clip"
        );
        self.clip_stack.pop();
    }

    /// Remove all clipping rectangles, restoring full-frame drawing.
    pub fn clear_clip(&mut self) {
        self.clip_stack.clear();
    }

    /// Effective max x (exclusive), considering clip rect.
    #[inline]
    fn max_x(&self) -> usize {
        self.clip_stack.last().map_or(self.width, |c| c.x1)
    }

    /// Effective max y (exclusive), considering clip rect.
    #[inline]
    fn max_y(&self) -> usize {
        self.clip_stack.last().map_or(self.height, |c| c.y1)
    }

    /// Effective min x (inclusive), considering clip rect.
    #[inline]
    fn min_x(&self) -> usize {
        self.clip_stack.last().map_or(0, |c| c.x0)
    }

    /// Effective min y (inclusive), considering clip rect.
    #[inline]
    fn min_y(&self) -> usize {
        self.clip_stack.last().map_or(0, |c| c.y0)
    }

    /// Clear the entire buffer with a solid color
    #[inline]
    pub fn clear(&mut self, color: u32) {
        self.buffer.fill(color);
    }

    /// Fill a rectangle with a solid color (no alpha blending)
    pub fn fill_rect(&mut self, rect: Rect, color: u32) {
        let x0 = (rect.x.max(0.0) as usize).min(self.width).max(self.min_x());
        let y0 = (rect.y.max(0.0) as usize)
            .min(self.height)
            .max(self.min_y());
        let x1 = ((rect.x + rect.width) as usize).min(self.max_x());
        let y1 = ((rect.y + rect.height) as usize).min(self.max_y());

        for y in y0..y1 {
            let row_start = y * self.width;
            for x in x0..x1 {
                self.buffer[row_start + x] = color;
            }
        }
    }

    /// Fill a rectangle specified by pixel coordinates
    pub fn fill_rect_px(&mut self, x: usize, y: usize, w: usize, h: usize, color: u32) {
        let x0 = x.min(self.width).max(self.min_x());
        let y0 = y.min(self.height).max(self.min_y());
        let x1 = (x + w).min(self.max_x());
        let y1 = (y + h).min(self.max_y());

        for py in y0..y1 {
            let row_start = py * self.width;
            for px in x0..x1 {
                self.buffer[row_start + px] = color;
            }
        }
    }

    /// Fill a rectangle with alpha blending (pixel coordinates, ARGB format)
    pub fn blend_rect_px(&mut self, x: usize, y: usize, w: usize, h: usize, color: u32) {
        let alpha = ((color >> 24) & 0xFF) as f32 / 255.0;
        if alpha <= 0.0 {
            return;
        }
        if alpha >= 1.0 {
            return self.fill_rect_px(x, y, w, h, color | 0xFF000000);
        }

        let x0 = x.min(self.width).max(self.min_x());
        let y0 = y.min(self.height).max(self.min_y());
        let x1 = (x + w).min(self.max_x());
        let y1 = (y + h).min(self.max_y());

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

        let x0 = (rect.x.max(0.0) as usize).min(self.width).max(self.min_x());
        let y0 = (rect.y.max(0.0) as usize)
            .min(self.height)
            .max(self.min_y());
        let x1 = ((rect.x + rect.width) as usize).min(self.max_x());
        let y1 = ((rect.y + rect.height) as usize).min(self.max_y());

        for y in y0..y1 {
            let row_start = y * self.width;
            for x in x0..x1 {
                let idx = row_start + x;
                self.buffer[idx] = blend_colors(self.buffer[idx], color, alpha);
            }
        }
    }

    /// Set a single pixel (bounds-checked, respects clip rect)
    #[inline]
    pub fn set_pixel(&mut self, x: usize, y: usize, color: u32) {
        if x >= self.min_x() && x < self.max_x() && y >= self.min_y() && y < self.max_y() {
            self.buffer[y * self.width + x] = color;
        }
    }

    /// Get a single pixel (bounds-checked, returns 0 if out of bounds)
    #[inline]
    #[cfg(test)]
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
        if x < self.min_x() || x >= self.max_x() || y < self.min_y() || y >= self.max_y() {
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

    /// Blend a pixel with a given alpha value (0.0-1.0), respecting clip rect.
    /// Optimized for text rendering where alpha comes from glyph bitmap.
    #[inline]
    pub fn blend_text_pixel(&mut self, x: usize, y: usize, color: u32, alpha: f32) {
        if x < self.min_x() || x >= self.max_x() || y < self.min_y() || y >= self.max_y() {
            return;
        }
        let idx = y * self.width + x;
        self.buffer[idx] = blend_colors(self.buffer[idx], color, alpha);
    }

    /// Fill a rectangle with alpha blending
    pub fn blend_rect(&mut self, rect: Rect, color: u32) {
        // `color`'s alpha is loop-invariant across the whole rect; extract
        // it once instead of re-deriving it (and re-checking the
        // alpha<=0/alpha>=1 fast paths) on every single pixel.
        let alpha = ((color >> 24) & 0xFF) as f32 / 255.0;
        if alpha <= 0.0 {
            return;
        }
        if alpha >= 1.0 {
            let x0 = (rect.x.max(0.0) as usize).min(self.width).max(self.min_x());
            let y0 = (rect.y.max(0.0) as usize)
                .min(self.height)
                .max(self.min_y());
            let w = ((rect.x + rect.width) as usize)
                .min(self.max_x())
                .saturating_sub(x0);
            let h = ((rect.y + rect.height) as usize)
                .min(self.max_y())
                .saturating_sub(y0);
            return self.fill_rect_px(x0, y0, w, h, color | 0xFF000000);
        }

        let x0 = (rect.x.max(0.0) as usize).min(self.width).max(self.min_x());
        let y0 = (rect.y.max(0.0) as usize)
            .min(self.height)
            .max(self.min_y());
        let x1 = ((rect.x + rect.width) as usize).min(self.max_x());
        let y1 = ((rect.y + rect.height) as usize).min(self.max_y());

        for y in y0..y1 {
            for x in x0..x1 {
                self.blend_text_pixel(x, y, color, alpha);
            }
        }
    }

    /// Blit an RGBA8 image into the frame, scaled to fit within the given rect
    /// while preserving aspect ratio. Centers the image within the rect.
    /// Uses nearest-neighbor scaling for simplicity.
    #[allow(clippy::too_many_arguments)]
    pub fn blit_rgba_scaled(
        &mut self,
        pixels: &[u8],
        img_width: u32,
        img_height: u32,
        dest_x: usize,
        dest_y: usize,
        dest_w: usize,
        dest_h: usize,
    ) {
        if img_width == 0 || img_height == 0 || dest_w == 0 || dest_h == 0 {
            return;
        }

        // Calculate scale to fit while preserving aspect ratio
        let scale_x = dest_w as f64 / img_width as f64;
        let scale_y = dest_h as f64 / img_height as f64;
        let scale = scale_x.min(scale_y);

        let scaled_w = (img_width as f64 * scale) as usize;
        let scaled_h = (img_height as f64 * scale) as usize;

        // Center within destination rect
        let offset_x = dest_x + (dest_w.saturating_sub(scaled_w)) / 2;
        let offset_y = dest_y + (dest_h.saturating_sub(scaled_h)) / 2;

        for dy in 0..scaled_h {
            let py = offset_y + dy;
            if py >= self.height {
                break;
            }
            let src_y = ((dy as f64 / scale) as u32).min(img_height - 1);
            let row_start = py * self.width;

            for dx in 0..scaled_w {
                let px = offset_x + dx;
                if px >= self.width {
                    break;
                }
                let src_x = ((dx as f64 / scale) as u32).min(img_width - 1);
                let src_idx = ((src_y * img_width + src_x) * 4) as usize;

                if src_idx + 3 >= pixels.len() {
                    continue;
                }

                let r = pixels[src_idx] as u32;
                let g = pixels[src_idx + 1] as u32;
                let b = pixels[src_idx + 2] as u32;
                let a = pixels[src_idx + 3] as f32 / 255.0;

                let argb = 0xFF000000 | (r << 16) | (g << 8) | b;

                if a >= 1.0 {
                    self.buffer[row_start + px] = argb;
                } else if a > 0.0 {
                    self.buffer[row_start + px] =
                        blend_colors(self.buffer[row_start + px], argb, a);
                }
            }
        }
    }

    /// Dim the entire frame with a semi-transparent overlay
    /// Useful for modal backgrounds
    pub fn dim(&mut self, alpha: u8) {
        if alpha == 0 {
            return;
        }
        let dim_color = (alpha as u32) << 24; // Black with given alpha
                                              // `alpha` is loop-invariant across the whole frame; extract it once
                                              // instead of re-deriving it on every single pixel.
        let a = alpha as f32 / 255.0;
        if alpha == 0xFF {
            return self.fill_rect_px(
                self.min_x(),
                self.min_y(),
                self.width,
                self.height,
                dim_color | 0xFF000000,
            );
        }
        for y in self.min_y()..self.max_y() {
            for x in self.min_x()..self.max_x() {
                self.blend_text_pixel(x, y, dim_color, a);
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

    /// Fill a rectangle with rounded corners, alpha-blended onto the
    /// background. `radius` is a physical-pixel radius; corner coverage
    /// masks are cached per radius in `mask_cache` (color-independent, see
    /// `RoundedCornerMask`).
    #[allow(clippy::too_many_arguments)]
    pub fn fill_rounded_rect(
        &mut self,
        x: usize,
        y: usize,
        w: usize,
        h: usize,
        radius: usize,
        color: u32,
        mask_cache: &mut RoundedRectMaskCache,
    ) {
        if w == 0 || h == 0 {
            return;
        }
        let radius = radius.min(w / 2).min(h / 2);
        if radius == 0 {
            self.blend_rect_px(x, y, w, h, color);
            return;
        }

        // Middle band spanning the full width, between the top and bottom
        // corner rows.
        if h > 2 * radius {
            self.blend_rect_px(x, y + radius, w, h - 2 * radius, color);
        }

        let mask = mask_cache
            .entry(radius)
            .or_insert_with(|| RoundedCornerMask::compute(radius));
        let base_alpha = ((color >> 24) & 0xFF) as f32;
        let rgb = color & 0x00FF_FFFF;

        for dy in 0..radius {
            // Straight segment of this corner row, between the left/right corners.
            if w > 2 * radius {
                self.blend_rect_px(x + radius, y + dy, w - 2 * radius, 1, color);
                self.blend_rect_px(x + radius, y + h - 1 - dy, w - 2 * radius, 1, color);
            }
            for dx in 0..radius {
                let cov = mask.coverage(dx, dy);
                if cov == 0 {
                    continue;
                }
                let a = (base_alpha * cov as f32 / 255.0).round() as u32;
                let c = (a.min(255) << 24) | rgb;
                self.blend_pixel(x + dx, y + dy, c);
                self.blend_pixel(x + w - 1 - dx, y + dy, c);
                self.blend_pixel(x + dx, y + h - 1 - dy, c);
                self.blend_pixel(x + w - 1 - dx, y + h - 1 - dy, c);
            }
        }
    }

    /// Fill a rectangle with only its top two corners rounded (bottom edge
    /// stays square) — for chrome bands (tab bar, list rows) that sit flush
    /// against the top of a `fill_rounded_rect` panel and must not paint
    /// over its already-antialiased top corners with a square fill.
    #[allow(clippy::too_many_arguments)]
    pub fn fill_rect_top_rounded(
        &mut self,
        x: usize,
        y: usize,
        w: usize,
        h: usize,
        radius: usize,
        color: u32,
        mask_cache: &mut RoundedRectMaskCache,
    ) {
        if w == 0 || h == 0 {
            return;
        }
        let radius = radius.min(w / 2).min(h);
        if radius == 0 {
            self.blend_rect_px(x, y, w, h, color);
            return;
        }

        // Everything below the corner rows: full width, square.
        if h > radius {
            self.blend_rect_px(x, y + radius, w, h - radius, color);
        }

        let mask = mask_cache
            .entry(radius)
            .or_insert_with(|| RoundedCornerMask::compute(radius));
        let base_alpha = ((color >> 24) & 0xFF) as f32;
        let rgb = color & 0x00FF_FFFF;

        for dy in 0..radius {
            if w > 2 * radius {
                self.blend_rect_px(x + radius, y + dy, w - 2 * radius, 1, color);
            }
            for dx in 0..radius {
                let cov = mask.coverage(dx, dy);
                if cov == 0 {
                    continue;
                }
                let a = (base_alpha * cov as f32 / 255.0).round() as u32;
                let c = (a.min(255) << 24) | rgb;
                self.blend_pixel(x + dx, y + dy, c);
                self.blend_pixel(x + w - 1 - dx, y + dy, c);
            }
        }
    }

    /// Fill a rectangle with only its bottom two corners rounded (top edge
    /// stays square) — the footer-band counterpart of
    /// [`Frame::fill_rect_top_rounded`].
    #[allow(clippy::too_many_arguments)]
    pub fn fill_rect_bottom_rounded(
        &mut self,
        x: usize,
        y: usize,
        w: usize,
        h: usize,
        radius: usize,
        color: u32,
        mask_cache: &mut RoundedRectMaskCache,
    ) {
        if w == 0 || h == 0 {
            return;
        }
        let radius = radius.min(w / 2).min(h);
        if radius == 0 {
            self.blend_rect_px(x, y, w, h, color);
            return;
        }

        // Everything above the corner rows: full width, square.
        if h > radius {
            self.blend_rect_px(x, y, w, h - radius, color);
        }

        let mask = mask_cache
            .entry(radius)
            .or_insert_with(|| RoundedCornerMask::compute(radius));
        let base_alpha = ((color >> 24) & 0xFF) as f32;
        let rgb = color & 0x00FF_FFFF;

        for dy in 0..radius {
            let row_y = y + h - 1 - dy;
            if w > 2 * radius {
                self.blend_rect_px(x + radius, row_y, w - 2 * radius, 1, color);
            }
            for dx in 0..radius {
                let cov = mask.coverage(dx, dy);
                if cov == 0 {
                    continue;
                }
                let a = (base_alpha * cov as f32 / 255.0).round() as u32;
                let c = (a.min(255) << 24) | rgb;
                self.blend_pixel(x + dx, row_y, c);
                self.blend_pixel(x + w - 1 - dx, row_y, c);
            }
        }
    }

    /// 1px rounded-rect outline: straight edges plus the antialiased arc
    /// band of the corner mask (skipping the solid interior, so a corner
    /// reads as a thin arc rather than a filled quarter-circle).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn stroke_rounded_rect(
        &mut self,
        x: usize,
        y: usize,
        w: usize,
        h: usize,
        radius: usize,
        color: u32,
        mask_cache: &mut RoundedRectMaskCache,
    ) {
        if w == 0 || h == 0 {
            return;
        }
        let radius = radius.min(w / 2).min(h / 2);

        if w > 2 * radius {
            self.blend_rect_px(x + radius, y, w - 2 * radius, 1, color);
            self.blend_rect_px(
                x + radius,
                y + h.saturating_sub(1),
                w - 2 * radius,
                1,
                color,
            );
        }
        if h > 2 * radius {
            self.blend_rect_px(x, y + radius, 1, h - 2 * radius, color);
            self.blend_rect_px(
                x + w.saturating_sub(1),
                y + radius,
                1,
                h - 2 * radius,
                color,
            );
        }
        if radius == 0 {
            return;
        }

        let mask = mask_cache
            .entry(radius)
            .or_insert_with(|| RoundedCornerMask::compute(radius));
        let base_alpha = ((color >> 24) & 0xFF) as f32;
        let rgb = color & 0x00FF_FFFF;

        for dy in 0..radius {
            for dx in 0..radius {
                let cov = mask.coverage(dx, dy);
                if cov == 0 || cov == 255 {
                    continue;
                }
                let a = (base_alpha * cov as f32 / 255.0).round() as u32;
                let c = (a.min(255) << 24) | rgb;
                self.blend_pixel(x + dx, y + dy, c);
                self.blend_pixel(x + w - 1 - dx, y + dy, c);
                self.blend_pixel(x + dx, y + h - 1 - dy, c);
                self.blend_pixel(x + w - 1 - dx, y + h - 1 - dy, c);
            }
        }
    }

    /// Modal/popup drop shadow: an opaque outline ring plus two nested
    /// translucent rings, each one physical pixel further out. No blur pass
    /// — the software renderer can't afford a full-viewport convolution.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_shadow_rings(
        &mut self,
        x: usize,
        y: usize,
        w: usize,
        h: usize,
        radius: usize,
        scale_factor: f64,
        mask_cache: &mut RoundedRectMaskCache,
    ) {
        // Soft falloff, not an outline: a 22% core fading through a 4%
        // tail. Offsets scale with the display so 2x gets the same logical
        // softness (each base ring strokes `s` consecutive physical
        // offsets).
        const RINGS: [(usize, u8); 4] = [(1, 0x38), (2, 0x22), (3, 0x14), (4, 0x0A)];
        let s = (scale_factor.round() as usize).max(1);
        for (base_offset, alpha) in RINGS {
            let color = (alpha as u32) << 24;
            for k in 0..s {
                let offset = base_offset * s - k;
                let rx = x.saturating_sub(offset);
                let ry = y.saturating_sub(offset);
                let rw = w + offset * 2;
                let rh = h + offset * 2;
                self.stroke_rounded_rect(rx, ry, rw, rh, radius + offset, color, mask_cache);
            }
        }
    }

    /// A 2-row, 4px-period wavy underline (squiggle), used for diagnostic
    /// underlines and shared with `editor-decorations.md`'s `Wavy` kind.
    pub fn draw_wavy_underline(&mut self, x: usize, y: usize, w: usize, color: u32) {
        for i in 0..w {
            let row = if i % 4 < 2 { 0 } else { 1 };
            self.blend_pixel(x + i, y + row, color);
        }
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
        if data.is_empty() || chart_width == 0 || chart_height == 0 {
            return;
        }

        self.fill_rect_px(x, y, chart_width, chart_height, bg_color);

        let sample_count = data.len().min(chart_width);
        if sample_count == 0 {
            return;
        }

        let start_idx = data.len().saturating_sub(sample_count);
        let samples: Vec<u128> = data
            .iter()
            .skip(start_idx)
            .map(|duration| duration.as_micros())
            .collect();
        let max_val = samples.iter().copied().max().unwrap_or(1).max(1) as f32;

        for column in 0..chart_width {
            let sample_idx = column * sample_count / chart_width;
            let sample = samples[sample_idx.min(sample_count - 1)];
            let normalized = sample as f32 / max_val;
            let bar_height = ((normalized * chart_height as f32).round() as usize)
                .max(1)
                .min(chart_height);
            let px = x + column;

            for dy in 0..bar_height {
                let py = y + chart_height - dy - 1;
                self.set_pixel(px, py, bar_color);
            }
        }
    }
}

/// Statistics for glyph cache hit/miss tracking (debug only)
#[cfg(debug_assertions)]
#[derive(Default)]
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
                            let px = current_x.round() as isize
                                + bitmap_x as isize
                                + metrics.xmin as isize;
                            let py = (glyph_top + bitmap_y as f32) as isize;

                            if px >= 0 && py >= 0 {
                                frame.blend_text_pixel(
                                    px as usize,
                                    py as usize,
                                    color,
                                    alpha as f32 / 255.0,
                                );
                            }
                        }
                    }
                }
            }

            current_x += metrics.advance_width;
        }
    }

    /// Draw text at an arbitrary (physical-px) size with optional tracking
    /// (extra advance per character, for the +1px section-header
    /// letterspacing). The glyph cache is already keyed on `(char, size)`,
    /// so a new size is just another cache bucket. Returns the drawn width.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_sized(
        &mut self,
        frame: &mut Frame,
        x: usize,
        y: usize,
        text: &str,
        size: f32,
        tracking: f32,
        color: u32,
    ) -> f32 {
        let ascent = self
            .font
            .horizontal_line_metrics(size)
            .map(|m| m.ascent)
            .unwrap_or(self.ascent);
        let baseline = y as f32 + ascent;
        let mut current_x = x as f32;

        for ch in text.chars() {
            let key = (ch, size.to_bits());

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
                .or_insert_with(|| self.font.rasterize(ch, size));

            let glyph_top = baseline - metrics.height as f32 - metrics.ymin as f32;

            for bitmap_y in 0..metrics.height {
                for bitmap_x in 0..metrics.width {
                    let bitmap_idx = bitmap_y * metrics.width + bitmap_x;
                    if bitmap_idx < bitmap.len() {
                        let alpha = bitmap[bitmap_idx];
                        if alpha > 0 {
                            let px = current_x.round() as isize
                                + bitmap_x as isize
                                + metrics.xmin as isize;
                            let py = (glyph_top + bitmap_y as f32) as isize;

                            if px >= 0 && py >= 0 {
                                frame.blend_text_pixel(
                                    px as usize,
                                    py as usize,
                                    color,
                                    alpha as f32 / 255.0,
                                );
                            }
                        }
                    }
                }
            }

            // Integer advances: JetBrains Mono's advance is 0.6*size —
            // fractional below ~20px — and truncating a float pen makes
            // inter-letter gaps alternate by 1px. Rounding the advance gives
            // uniform cells, and measure_sized applies the same rule so
            // paint == measurement by construction.
            current_x += (metrics.advance_width + tracking).round();
        }

        current_x - x as f32
    }

    /// Measure text width in pixels at an arbitrary size, with tracking.
    pub fn measure_sized(&mut self, text: &str, size: f32, tracking: f32) -> f32 {
        let mut width = 0.0;
        for ch in text.chars() {
            let key = (ch, size.to_bits());
            let (metrics, _) = self
                .glyph_cache
                .entry(key)
                .or_insert_with(|| self.font.rasterize(ch, size));
            // Same integer-advance rule as draw_sized.
            width += (metrics.advance_width + tracking).round();
        }
        width
    }

    /// Line height (px) at an arbitrary size, for laying out chrome that
    /// mixes the 14/13/11 type scale (e.g. keycap chip height).
    pub fn line_height_for_size(&self, size: f32) -> usize {
        self.font
            .horizontal_line_metrics(size)
            .map(|m| m.new_line_size.ceil() as usize)
            .unwrap_or(self.line_height)
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
                            let px = current_x.round() as isize
                                + bitmap_x as isize
                                + metrics.xmin as isize;
                            let py = (glyph_top + bitmap_y as f32) as isize;

                            if px >= 0 && py >= 0 {
                                frame.blend_text_pixel(
                                    px as usize,
                                    py as usize,
                                    color,
                                    alpha as f32 / 255.0,
                                );
                            }
                        }
                    }
                }
            }

            current_x += metrics.advance_width;
        }
    }
}

const KEYCAP_SIZE_LOGICAL: f32 = 11.0;
const KEYCAP_MIN_WIDTH_LOGICAL: f32 = 17.0;
const KEYCAP_PAD_X_LOGICAL: f32 = 4.0;

/// The width `draw_keycap` will paint at, without painting anything — lets
/// callers reserve layout space for a chip (e.g. accessory width in
/// `overlay_surface::render_list`) before drawing it.
pub fn keycap_width(painter: &mut TextPainter, label: &str, scale_factor: f64) -> usize {
    let scale = |v: f32| (v as f64 * scale_factor).round().max(1.0) as usize;
    let size = (KEYCAP_SIZE_LOGICAL as f64 * scale_factor) as f32;
    let text_w = painter.measure_sized(label, size, 0.0);
    let pad_x = scale(KEYCAP_PAD_X_LOGICAL);
    let min_width = scale(KEYCAP_MIN_WIDTH_LOGICAL);
    (text_w.ceil() as usize + pad_x * 2).max(min_width)
}

/// A single keycap chip: bordered rounded rect + centered 11px label.
/// Returns the chip's width, so callers laying out a row of chips
/// (`binding_chips`) can advance past it. Sizes/paddings are 11px-scale
/// logical constants, scaled to physical px like the rest of the chrome.
#[allow(clippy::too_many_arguments)]
pub fn draw_keycap(
    frame: &mut Frame,
    painter: &mut TextPainter,
    mask_cache: &mut RoundedRectMaskCache,
    x: usize,
    y: usize,
    label: &str,
    bg: u32,
    border: u32,
    fg: u32,
    scale_factor: f64,
) -> usize {
    const PAD_Y_LOGICAL: f32 = 2.0;
    const RADIUS_LOGICAL: f32 = 4.0;

    let scale = |v: f32| (v as f64 * scale_factor).round().max(1.0) as usize;

    let size = (KEYCAP_SIZE_LOGICAL as f64 * scale_factor) as f32;
    let text_w = painter.measure_sized(label, size, 0.0);
    let pad_y = scale(PAD_Y_LOGICAL);
    let radius = scale(RADIUS_LOGICAL);
    let border_w = scale(1.0);
    let border_bottom_w = border_w + scale(1.0);

    let text_h = painter.line_height_for_size(size);
    let width = keycap_width(painter, label, scale_factor);
    let height = text_h + pad_y * 2;

    frame.fill_rounded_rect(x, y, width, height, radius, border, mask_cache);

    let inner_x = x + border_w;
    let inner_y = y + border_w;
    let inner_w = width.saturating_sub(border_w * 2);
    let inner_h = height.saturating_sub(border_w + border_bottom_w);
    let inner_radius = radius.saturating_sub(border_w);
    frame.fill_rounded_rect(
        inner_x,
        inner_y,
        inner_w,
        inner_h,
        inner_radius,
        bg,
        mask_cache,
    );

    let text_x = x + (width.saturating_sub(text_w.ceil() as usize)) / 2;
    let text_y = y + (height.saturating_sub(text_h)) / 2;
    painter.draw_sized(frame, text_x, text_y, label, size, 0.0, fg);

    width
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use std::time::Duration;

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
    fn translucent_active_tab_highlight_blends_instead_of_painting_solid() {
        // Regression: the dock header painted the active-tab highlight
        // (`selection_background`, a translucent white like #FFFFFF1A) with
        // `fill_rect_px`, which ignores alpha and produced a solid white block
        // that hid the white tab title. It must alpha-blend over the panel bg.
        let mut buffer = vec![0xFF25_2526u32; 40 * 20]; // panel background
        let mut frame = Frame::new(&mut buffer, 40, 20);

        frame.blend_rect_px(0, 0, 10, 10, 0x1AFF_FFFF); // ~10% white

        let px = frame.get_pixel(2, 2);
        assert_ne!(px, 0xFFFF_FFFF, "highlight must not be solid white");
        // A subtle lightening of the dark background, still opaque.
        assert_eq!(px >> 24, 0xFF);
        assert!((px & 0xFF) > 0x26 && (px & 0xFF) < 0x60);
    }

    #[test]
    fn push_clip_intersects_with_current_clip() {
        let mut buffer = vec![0u32; 100 * 100];
        let mut frame = Frame::new(&mut buffer, 100, 100);

        frame.push_clip(Rect::new(10.0, 10.0, 40.0, 40.0));
        frame.push_clip(Rect::new(30.0, 0.0, 40.0, 100.0));
        // Effective clip: x 30..50, y 10..50.
        frame.fill_rect(Rect::new(0.0, 0.0, 100.0, 100.0), 0xFFFF0000);

        assert_eq!(frame.get_pixel(35, 20), 0xFFFF0000);
        assert_eq!(frame.get_pixel(20, 20), 0); // inside outer, outside inner
        assert_eq!(frame.get_pixel(60, 20), 0); // inside inner arg, outside outer
    }

    #[test]
    fn pop_clip_restores_enclosing_clip() {
        let mut buffer = vec![0u32; 100 * 100];
        let mut frame = Frame::new(&mut buffer, 100, 100);

        frame.push_clip(Rect::new(10.0, 10.0, 40.0, 40.0));
        frame.push_clip(Rect::new(30.0, 30.0, 10.0, 10.0));
        frame.pop_clip();
        // Back to the outer clip.
        frame.fill_rect(Rect::new(0.0, 0.0, 100.0, 100.0), 0xFF00FF00);
        assert_eq!(frame.get_pixel(15, 15), 0xFF00FF00);

        frame.pop_clip();
        // Stack empty: full-frame drawing again.
        frame.fill_rect(Rect::new(0.0, 0.0, 100.0, 100.0), 0xFF0000FF);
        assert_eq!(frame.get_pixel(5, 5), 0xFF0000FF);
    }

    #[test]
    fn set_clip_resets_the_stack() {
        let mut buffer = vec![0u32; 100 * 100];
        let mut frame = Frame::new(&mut buffer, 100, 100);

        frame.push_clip(Rect::new(10.0, 10.0, 20.0, 20.0));
        frame.push_clip(Rect::new(15.0, 15.0, 5.0, 5.0));
        // set_clip is absolute: it discards the nested stack.
        frame.set_clip(Rect::new(60.0, 60.0, 20.0, 20.0));
        frame.fill_rect(Rect::new(0.0, 0.0, 100.0, 100.0), 0xFFFF00FF);
        assert_eq!(frame.get_pixel(65, 65), 0xFFFF00FF);
        assert_eq!(frame.get_pixel(15, 15), 0);
    }

    #[test]
    fn push_clip_with_empty_intersection_draws_nothing() {
        let mut buffer = vec![0u32; 100 * 100];
        let mut frame = Frame::new(&mut buffer, 100, 100);

        frame.push_clip(Rect::new(0.0, 0.0, 20.0, 20.0));
        frame.push_clip(Rect::new(50.0, 50.0, 20.0, 20.0)); // disjoint
        frame.fill_rect(Rect::new(0.0, 0.0, 100.0, 100.0), 0xFFFFFFFF);
        assert!(buffer.iter().all(|&px| px == 0));
    }

    #[test]
    fn set_clip_with_negative_width_produces_empty_not_inverted_clip() {
        // Regression test: `set_clip` used to cast `rect.x + rect.width` to
        // usize before clamping, so a negative width could (after `x0`'s own
        // clamp) leave `x1 < x0` — an inverted clip rect. It must now
        // collapse to an empty (x1 == x0), not inverted, region.
        let mut buffer = vec![0u32; 20 * 20];
        let mut frame = Frame::new(&mut buffer, 20, 20);

        frame.set_clip(Rect {
            x: 10.0,
            y: 10.0,
            width: -100.0,
            height: -100.0,
        });

        assert!(frame.min_x() <= frame.max_x());
        assert!(frame.min_y() <= frame.max_y());
    }

    #[test]
    fn blend_rect_matches_per_pixel_blend_pixel_reference() {
        // Regression test: `blend_rect` was refactored to extract alpha once
        // and use a fast path for opaque colors, instead of re-deriving
        // alpha on every pixel via `blend_pixel`. Output must be identical.
        let rect = Rect {
            x: 2.0,
            y: 3.0,
            width: 6.0,
            height: 5.0,
        };
        let color = 0x80336699;

        let mut buffer_a = vec![0xFFFFFFFF_u32; 12 * 12];
        let mut frame_a = Frame::new(&mut buffer_a, 12, 12);
        frame_a.blend_rect(rect, color);

        // Reference: the old per-pixel `blend_pixel` loop.
        let mut buffer_b = vec![0xFFFFFFFF_u32; 12 * 12];
        let mut frame_b = Frame::new(&mut buffer_b, 12, 12);
        for y in 3..8 {
            for x in 2..8 {
                frame_b.blend_pixel(x, y, color);
            }
        }

        assert_eq!(buffer_a, buffer_b);
    }

    #[test]
    fn dim_matches_per_pixel_blend_pixel_reference() {
        let mut buffer_a = vec![0xFFFFFFFF_u32; 10 * 10];
        let mut frame_a = Frame::new(&mut buffer_a, 10, 10);
        frame_a.dim(96);

        let mut buffer_b = vec![0xFFFFFFFF_u32; 10 * 10];
        let mut frame_b = Frame::new(&mut buffer_b, 10, 10);
        let dim_color = (96_u32) << 24;
        for y in 0..10 {
            for x in 0..10 {
                frame_b.blend_pixel(x, y, dim_color);
            }
        }

        assert_eq!(buffer_a, buffer_b);
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

    #[test]
    fn test_frame_with_clip_restricts_fill_rect() {
        let mut buffer = vec![0u32; 100 * 100];
        let mut frame = Frame::new(&mut buffer, 100, 100);
        frame.set_clip(Rect {
            x: 10.0,
            y: 10.0,
            width: 30.0,
            height: 30.0,
        });

        // Fill the entire frame — should be clipped to 10..40 x 10..40
        frame.fill_rect(
            Rect {
                x: 0.0,
                y: 0.0,
                width: 100.0,
                height: 100.0,
            },
            0xFFFF0000,
        );

        // Inside clip: should be red
        assert_eq!(frame.get_pixel(20, 20), 0xFFFF0000);
        // Outside clip: should be untouched (0)
        assert_eq!(frame.get_pixel(5, 5), 0);
        assert_eq!(frame.get_pixel(50, 50), 0);
        // Edge of clip: 10 is inside, 40 is outside (exclusive)
        assert_eq!(frame.get_pixel(10, 10), 0xFFFF0000);
        assert_eq!(frame.get_pixel(39, 39), 0xFFFF0000);
        assert_eq!(frame.get_pixel(40, 40), 0);
    }

    #[test]
    fn test_frame_no_clip_unchanged_behavior() {
        let mut buffer = vec![0u32; 50 * 50];
        let mut frame = Frame::new(&mut buffer, 50, 50);
        // No clip set — default behavior
        frame.fill_rect(
            Rect {
                x: 0.0,
                y: 0.0,
                width: 50.0,
                height: 50.0,
            },
            0xFF00FF00,
        );
        assert_eq!(frame.get_pixel(0, 0), 0xFF00FF00);
        assert_eq!(frame.get_pixel(49, 49), 0xFF00FF00);
    }

    #[test]
    fn test_frame_clip_set_pixel() {
        let mut buffer = vec![0u32; 20 * 20];
        let mut frame = Frame::new(&mut buffer, 20, 20);
        frame.set_clip(Rect {
            x: 5.0,
            y: 5.0,
            width: 10.0,
            height: 10.0,
        });

        // Inside clip
        frame.set_pixel(7, 7, 0xFFAA0000);
        assert_eq!(frame.get_pixel(7, 7), 0xFFAA0000);

        // Outside clip — should be ignored
        frame.set_pixel(3, 3, 0xFFBB0000);
        assert_eq!(frame.get_pixel(3, 3), 0);

        // At clip boundary (exclusive end)
        frame.set_pixel(15, 15, 0xFFCC0000);
        assert_eq!(frame.get_pixel(15, 15), 0);
    }

    #[test]
    fn test_frame_clip_blend_pixel() {
        let mut buffer = vec![0xFFFFFFFF_u32; 20 * 20]; // white
        let mut frame = Frame::new(&mut buffer, 20, 20);
        frame.set_clip(Rect {
            x: 5.0,
            y: 5.0,
            width: 10.0,
            height: 10.0,
        });

        // Blend inside clip — should modify pixel
        frame.blend_pixel(7, 7, 0x80000000); // 50% black
        let result = frame.get_pixel(7, 7);
        let r = (result >> 16) & 0xFF;
        assert!(r > 100 && r < 160, "R channel: {}", r);

        // Blend outside clip — should remain white
        frame.blend_pixel(3, 3, 0x80000000);
        assert_eq!(frame.get_pixel(3, 3), 0xFFFFFFFF);
    }

    #[test]
    fn test_draw_sparkline_stays_within_chart_bounds() {
        let sentinel = 0xFF112233;
        let mut buffer = vec![sentinel; 20 * 20];
        let mut frame = Frame::new(&mut buffer, 20, 20);
        let history: VecDeque<_> = (1..=16).map(Duration::from_micros).collect();

        frame.draw_sparkline(4, 5, 6, 4, &history, 0xFFFF0000, 0xFF000000);

        for y in 0..20 {
            for x in 0..20 {
                let inside_chart = (4..10).contains(&x) && (5..9).contains(&y);
                if !inside_chart {
                    assert_eq!(
                        frame.get_pixel(x, y),
                        sentinel,
                        "sparkline should not paint outside its chart bounds at ({x}, {y})"
                    );
                }
            }
        }
    }

    #[test]
    fn fill_rounded_rect_true_corner_is_transparent() {
        // At a real corner radius, the outermost pixel of each corner sits
        // clearly outside the arc and must stay untouched (alpha 0), not
        // painted like a square rect would.
        let mut buffer = vec![0u32; 20 * 20];
        let mut frame = Frame::new(&mut buffer, 20, 20);
        let mut cache = RoundedRectMaskCache::new();

        frame.fill_rounded_rect(2, 2, 16, 16, 6, 0xFFFF0000, &mut cache);

        assert_eq!(
            frame.get_pixel(2, 2),
            0,
            "true corner pixel must be untouched"
        );
        assert_eq!(
            frame.get_pixel(17, 2),
            0,
            "true corner pixel must be untouched"
        );
        assert_eq!(
            frame.get_pixel(2, 17),
            0,
            "true corner pixel must be untouched"
        );
        assert_eq!(
            frame.get_pixel(17, 17),
            0,
            "true corner pixel must be untouched"
        );
    }

    #[test]
    fn fill_rounded_rect_interior_and_edge_midpoints_are_opaque() {
        let mut buffer = vec![0u32; 20 * 20];
        let mut frame = Frame::new(&mut buffer, 20, 20);
        let mut cache = RoundedRectMaskCache::new();

        frame.fill_rounded_rect(2, 2, 16, 16, 6, 0xFFFF0000, &mut cache);

        // Center of the rect: fully inside, opaque red.
        assert_eq!(frame.get_pixel(10, 10), 0xFFFF0000);
        // Midpoint of the top edge: outside the corner arcs, straight edge.
        assert_eq!(frame.get_pixel(10, 2), 0xFFFF0000);
        // Midpoint of the left edge.
        assert_eq!(frame.get_pixel(2, 10), 0xFFFF0000);
    }

    #[test]
    fn fill_rounded_rect_mask_cache_reused_across_calls_at_same_radius() {
        let mut buffer = vec![0u32; 20 * 20];
        let mut frame = Frame::new(&mut buffer, 20, 20);
        let mut cache = RoundedRectMaskCache::new();

        frame.fill_rounded_rect(0, 0, 10, 10, 4, 0xFFFF0000, &mut cache);
        assert_eq!(cache.len(), 1, "one mask cached for radius 4");
        frame.fill_rounded_rect(10, 10, 10, 10, 4, 0xFF00FF00, &mut cache);
        assert_eq!(
            cache.len(),
            1,
            "second call at the same physical radius must reuse the cached mask, not add a new one"
        );
    }

    #[test]
    fn fill_rounded_rect_zero_radius_matches_fill_rect() {
        let mut buffer_a = vec![0u32; 10 * 10];
        let mut frame_a = Frame::new(&mut buffer_a, 10, 10);
        let mut cache = RoundedRectMaskCache::new();
        frame_a.fill_rounded_rect(1, 1, 6, 6, 0, 0xFF0000FF, &mut cache);

        let mut buffer_b = vec![0u32; 10 * 10];
        let mut frame_b = Frame::new(&mut buffer_b, 10, 10);
        frame_b.blend_rect_px(1, 1, 6, 6, 0xFF0000FF);

        assert_eq!(buffer_a, buffer_b);
    }

    #[test]
    fn fill_rect_top_rounded_leaves_top_corners_transparent_and_squares_the_bottom() {
        let mut buffer = vec![0u32; 20 * 20];
        let mut frame = Frame::new(&mut buffer, 20, 20);
        let mut cache = RoundedRectMaskCache::new();

        frame.fill_rect_top_rounded(2, 2, 16, 16, 6, 0xFFFF0000, &mut cache);

        assert_eq!(
            frame.get_pixel(2, 2),
            0,
            "top-left corner must stay untouched, matching fill_rounded_rect"
        );
        assert_eq!(
            frame.get_pixel(17, 2),
            0,
            "top-right corner must stay untouched, matching fill_rounded_rect"
        );
        // Bottom corners are square, not rounded: fully painted.
        assert_eq!(frame.get_pixel(2, 17), 0xFFFF0000);
        assert_eq!(frame.get_pixel(17, 17), 0xFFFF0000);
    }

    #[test]
    fn fill_rect_bottom_rounded_leaves_bottom_corners_transparent_and_squares_the_top() {
        let mut buffer = vec![0u32; 20 * 20];
        let mut frame = Frame::new(&mut buffer, 20, 20);
        let mut cache = RoundedRectMaskCache::new();

        frame.fill_rect_bottom_rounded(2, 2, 16, 16, 6, 0xFFFF0000, &mut cache);

        assert_eq!(
            frame.get_pixel(2, 17),
            0,
            "bottom-left corner must stay untouched, matching fill_rounded_rect"
        );
        assert_eq!(
            frame.get_pixel(17, 17),
            0,
            "bottom-right corner must stay untouched, matching fill_rounded_rect"
        );
        // Top corners are square, not rounded: fully painted.
        assert_eq!(frame.get_pixel(2, 2), 0xFFFF0000);
        assert_eq!(frame.get_pixel(17, 2), 0xFFFF0000);
    }

    #[test]
    fn draw_wavy_underline_alternates_rows_every_two_pixels() {
        let mut buffer = vec![0u32; 10 * 3];
        let mut frame = Frame::new(&mut buffer, 10, 3);
        frame.draw_wavy_underline(0, 0, 8, 0xFFFFFFFF);

        for i in 0..8usize {
            let expect_row0 = i % 4 < 2;
            assert_eq!(
                frame.get_pixel(i, 0) == 0xFFFFFFFF,
                expect_row0,
                "col {i} row 0"
            );
            assert_eq!(
                frame.get_pixel(i, 1) == 0xFFFFFFFF,
                !expect_row0,
                "col {i} row 1"
            );
        }
    }

    #[test]
    fn draw_shadow_rings_paints_outside_the_source_rect() {
        let mut buffer = vec![0u32; 30 * 30];
        let mut frame = Frame::new(&mut buffer, 30, 30);
        let mut cache = RoundedRectMaskCache::new();

        // White ground so translucent black rings are observable in the
        // blended result.
        frame.fill_rect_px(0, 0, 30, 30, 0xFFFF_FFFF);
        frame.draw_shadow_rings(10, 10, 10, 10, 4, 1.0, &mut cache);

        // A pixel just outside the panel's straight edge (top, above the
        // corner band) should be darkened by the first ring.
        let near = frame.get_pixel(15, 9) & 0xFF;
        assert_ne!(
            near, 0xFF,
            "shadow ring must paint just outside the panel edge"
        );
        // The falloff property: the outermost ring is fainter (lighter on
        // white) than the innermost — a soft shadow, not an outline.
        let far = frame.get_pixel(15, 6) & 0xFF;
        assert_ne!(far, 0xFF, "tail ring must paint at offset 4");
        assert!(
            far > near,
            "shadow must fade outward (near {near:#04X} vs far {far:#04X})"
        );
        // Far outside all rings, untouched white.
        assert_eq!(frame.get_pixel(0, 0) & 0xFF, 0xFF);
    }

    #[test]
    fn measure_sized_uses_uniform_integer_cells() {
        // JetBrains Mono's advance is 0.6*size — fractional at 13px/11px.
        // draw_sized/measure_sized round each advance so every glyph gets
        // the same integer cell (uneven 7/8/8/8/8 gaps were visible in the
        // palette), and paint == measurement by construction.
        let font = Font::from_bytes(
            include_bytes!("../../assets/JetBrainsMono.ttf") as &[u8],
            fontdue::FontSettings::default(),
        )
        .expect("test font should load");
        let mut glyph_cache = super::super::GlyphCache::default();
        let mut painter = TextPainter::new(&font, &mut glyph_cache, 14.0, 11.0, 8.0, 18);

        for size in [13.0f32, 11.0] {
            let one = painter.measure_sized("M", size, 0.0);
            let five = painter.measure_sized("MMMMM", size, 0.0);
            assert_eq!(one.fract(), 0.0, "cell at {size}px must be integer");
            assert_eq!(five, one * 5.0, "cells at {size}px must be uniform");
        }
    }

    #[test]
    fn draw_keycap_respects_minimum_width_and_paints_the_label() {
        let font = Font::from_bytes(
            include_bytes!("../../assets/JetBrainsMono.ttf") as &[u8],
            fontdue::FontSettings::default(),
        )
        .expect("test font should load");
        let mut glyph_cache = super::super::GlyphCache::default();
        let mut painter = TextPainter::new(&font, &mut glyph_cache, 14.0, 11.0, 8.0, 18);

        let mut buffer = vec![0u32; 60 * 30];
        let mut frame = Frame::new(&mut buffer, 60, 30);
        let mut mask_cache = RoundedRectMaskCache::new();

        // A single narrow glyph ("a") is narrower than the 17px logical
        // minimum, so the chip must still be at least that wide.
        let width = draw_keycap(
            &mut frame,
            &mut painter,
            &mut mask_cache,
            2,
            2,
            "a",
            0xFF35373C,
            0xFF4A4D53,
            0xFFA5A8AE,
            1.0,
        );

        assert!(
            width >= 17,
            "keycap must respect the 17px minimum width, got {width}"
        );
        // Something was painted (border/background), not left transparent.
        assert_ne!(frame.get_pixel(2, 2 + 4), 0);
    }
}
