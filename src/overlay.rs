//! Reusable overlay rendering system
//!
//! Provides types and functions for rendering positioned overlays
//! with semi-transparent backgrounds (e.g., performance stats, tooltips).

/// Position anchor for overlays
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayAnchor {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    Center,
}

/// Configuration for rendering an overlay
#[derive(Debug, Clone)]
pub struct OverlayConfig {
    /// Where the overlay is anchored on screen
    pub anchor: OverlayAnchor,
    /// Width in pixels
    pub width: usize,
    /// Height in pixels
    pub height: usize,
    /// Margin from viewport edge in pixels
    pub margin: usize,
    /// Background color in ARGB format (alpha in high byte)
    pub background: u32,
}

impl OverlayConfig {
    /// Create a new overlay config with the given anchor and dimensions
    pub fn new(anchor: OverlayAnchor, width: usize, height: usize) -> Self {
        Self {
            anchor,
            width,
            height,
            margin: 10,
            background: 0xE0202020, // 88% alpha dark gray
        }
    }

    /// Set the margin (builder pattern)
    pub fn with_margin(mut self, margin: usize) -> Self {
        self.margin = margin;
        self
    }

    /// Set the background color (builder pattern)
    pub fn with_background(mut self, background: u32) -> Self {
        self.background = background;
        self
    }

    /// Calculate screen position from anchor and viewport dimensions
    pub fn compute_bounds(&self, viewport_width: usize, viewport_height: usize) -> OverlayBounds {
        let x = match self.anchor {
            OverlayAnchor::TopLeft | OverlayAnchor::BottomLeft => self.margin,
            OverlayAnchor::TopRight | OverlayAnchor::BottomRight => {
                viewport_width.saturating_sub(self.width + self.margin)
            }
            OverlayAnchor::Center => viewport_width.saturating_sub(self.width) / 2,
        };

        let y = match self.anchor {
            OverlayAnchor::TopLeft | OverlayAnchor::TopRight => self.margin,
            OverlayAnchor::BottomLeft | OverlayAnchor::BottomRight => {
                viewport_height.saturating_sub(self.height + self.margin)
            }
            OverlayAnchor::Center => viewport_height.saturating_sub(self.height) / 2,
        };

        OverlayBounds {
            x,
            y,
            width: self.width,
            height: self.height,
        }
    }
}

/// Computed overlay bounds (screen coordinates)
#[derive(Debug, Clone, Copy)]
pub struct OverlayBounds {
    /// X position in pixels
    pub x: usize,
    /// Y position in pixels
    pub y: usize,
    /// Width in pixels
    pub width: usize,
    /// Height in pixels
    pub height: usize,
}

impl OverlayBounds {
    /// Get the right edge X coordinate
    pub fn right(&self) -> usize {
        self.x + self.width
    }

    /// Get the bottom edge Y coordinate
    pub fn bottom(&self) -> usize {
        self.y + self.height
    }
}

/// Blend a source pixel (with alpha) onto a destination pixel
///
/// Both colors are in ARGB format. The source alpha determines
/// the blend ratio.
#[inline]
pub fn blend_pixel(src: u32, dst: u32) -> u32 {
    let alpha = ((src >> 24) & 0xFF) as u32;
    if alpha == 0 {
        return dst;
    }
    if alpha == 255 {
        return src | 0xFF000000;
    }

    let inv_alpha = 255 - alpha;

    let r = ((((src >> 16) & 0xFF) * alpha + ((dst >> 16) & 0xFF) * inv_alpha) / 255) & 0xFF;
    let g = ((((src >> 8) & 0xFF) * alpha + ((dst >> 8) & 0xFF) * inv_alpha) / 255) & 0xFF;
    let b = (((src & 0xFF) * alpha + (dst & 0xFF) * inv_alpha) / 255) & 0xFF;

    0xFF000000 | (r << 16) | (g << 8) | b
}

/// Render an overlay background with alpha blending
///
/// # Arguments
/// * `buffer` - The framebuffer to render into
/// * `bounds` - The overlay bounds
/// * `background` - Background color in ARGB format
/// * `buffer_width` - Width of the framebuffer in pixels
/// * `buffer_height` - Height of the framebuffer in pixels
pub fn render_overlay_background(
    buffer: &mut [u32],
    bounds: &OverlayBounds,
    background: u32,
    buffer_width: usize,
    buffer_height: usize,
) {
    let y_end = bounds.bottom().min(buffer_height);
    let x_end = bounds.right().min(buffer_width);

    for py in bounds.y..y_end {
        for px in bounds.x..x_end {
            let idx = py * buffer_width + px;
            if idx < buffer.len() {
                buffer[idx] = blend_pixel(background, buffer[idx]);
            }
        }
    }
}

/// Render a 1px border around overlay bounds
///
/// # Arguments
/// * `buffer` - The framebuffer to render into
/// * `bounds` - The overlay bounds
/// * `border_color` - Border color in ARGB format
/// * `buffer_width` - Width of the framebuffer in pixels
/// * `buffer_height` - Height of the framebuffer in pixels
pub fn render_overlay_border(
    buffer: &mut [u32],
    bounds: &OverlayBounds,
    border_color: u32,
    buffer_width: usize,
    buffer_height: usize,
) {
    let y_end = bounds.bottom().min(buffer_height);
    let x_end = bounds.right().min(buffer_width);

    // Top edge
    if bounds.y < buffer_height {
        for px in bounds.x..x_end {
            let idx = bounds.y * buffer_width + px;
            if idx < buffer.len() {
                buffer[idx] = border_color | 0xFF000000;
            }
        }
    }

    // Bottom edge
    let bottom_y = y_end.saturating_sub(1);
    if bottom_y > bounds.y && bottom_y < buffer_height {
        for px in bounds.x..x_end {
            let idx = bottom_y * buffer_width + px;
            if idx < buffer.len() {
                buffer[idx] = border_color | 0xFF000000;
            }
        }
    }

    // Left edge
    if bounds.x < buffer_width {
        for py in bounds.y..y_end {
            let idx = py * buffer_width + bounds.x;
            if idx < buffer.len() {
                buffer[idx] = border_color | 0xFF000000;
            }
        }
    }

    // Right edge
    let right_x = x_end.saturating_sub(1);
    if right_x > bounds.x && right_x < buffer_width {
        for py in bounds.y..y_end {
            let idx = py * buffer_width + right_x;
            if idx < buffer.len() {
                buffer[idx] = border_color | 0xFF000000;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_overlay_anchor_top_left() {
        let config = OverlayConfig::new(OverlayAnchor::TopLeft, 100, 50).with_margin(10);
        let bounds = config.compute_bounds(800, 600);

        assert_eq!(bounds.x, 10);
        assert_eq!(bounds.y, 10);
    }

    #[test]
    fn test_overlay_anchor_top_right() {
        let config = OverlayConfig::new(OverlayAnchor::TopRight, 100, 50).with_margin(10);
        let bounds = config.compute_bounds(800, 600);

        assert_eq!(bounds.x, 800 - 100 - 10);
        assert_eq!(bounds.y, 10);
    }

    #[test]
    fn test_overlay_anchor_bottom_right() {
        let config = OverlayConfig::new(OverlayAnchor::BottomRight, 100, 50).with_margin(10);
        let bounds = config.compute_bounds(800, 600);

        assert_eq!(bounds.x, 800 - 100 - 10);
        assert_eq!(bounds.y, 600 - 50 - 10);
    }

    #[test]
    fn test_overlay_anchor_center() {
        let config = OverlayConfig::new(OverlayAnchor::Center, 100, 50);
        let bounds = config.compute_bounds(800, 600);

        assert_eq!(bounds.x, (800 - 100) / 2);
        assert_eq!(bounds.y, (600 - 50) / 2);
    }

    #[test]
    fn test_blend_pixel_fully_opaque() {
        let src = 0xFF_FF_00_00; // Opaque red
        let dst = 0xFF_00_FF_00; // Opaque green
        let result = blend_pixel(src, dst);
        assert_eq!(result, 0xFF_FF_00_00); // Red wins
    }

    #[test]
    fn test_blend_pixel_fully_transparent() {
        let src = 0x00_FF_00_00; // Transparent red
        let dst = 0xFF_00_FF_00; // Opaque green
        let result = blend_pixel(src, dst);
        assert_eq!(result, 0xFF_00_FF_00); // Green unchanged
    }

    #[test]
    fn test_blend_pixel_half_alpha() {
        let src = 0x80_FF_00_00; // 50% alpha red
        let dst = 0xFF_00_00_00; // Opaque black
        let result = blend_pixel(src, dst);

        // Should be roughly 50% red
        let r = (result >> 16) & 0xFF;
        assert!(r > 120 && r < 135, "Expected ~128, got {}", r);
    }
}
