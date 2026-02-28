//! Image rendering for the view layer
//!
//! Renders checkerboard background and scaled image pixels
//! into the framebuffer using nearest-neighbor sampling.

use crate::image::ImageState;
use crate::theme::ImageTheme;
use crate::view::frame::Frame;

/// Render an image in the given screen rectangle.
///
/// 1. Fills the area with checkerboard pattern
/// 2. Blits visible portion of the image with nearest-neighbor scaling
/// 3. Centers the image if it's smaller than the viewport
pub fn render_image(
    frame: &mut Frame,
    image: &ImageState,
    theme: &ImageTheme,
    area_x: usize,
    area_y: usize,
    area_width: usize,
    area_height: usize,
) {
    let cell = theme.checkerboard_cell_size.max(1) as usize;
    let light = theme.checkerboard_light.to_argb_u32();
    let dark = theme.checkerboard_dark.to_argb_u32();

    // Compute how big the image is on screen
    let scaled_width = (image.width as f64 * image.scale) as usize;
    let scaled_height = (image.height as f64 * image.scale) as usize;

    // Center offset when image is smaller than viewport
    let center_x = if scaled_width < area_width {
        (area_width - scaled_width) as f64 / 2.0
    } else {
        0.0
    };
    let center_y = if scaled_height < area_height {
        (area_height - scaled_height) as f64 / 2.0
    } else {
        0.0
    };

    let buf_width = frame.width();
    let buf_height = frame.height();
    let buffer = frame.buffer_mut();
    let img_w = image.width;
    let img_h = image.height;
    let scale = image.scale;
    let off_x = image.offset_x;
    let off_y = image.offset_y;

    for sy in 0..area_height {
        let screen_y = area_y + sy;
        if screen_y >= buf_height {
            break;
        }

        let row_start = screen_y * buf_width;

        for sx in 0..area_width {
            let screen_x = area_x + sx;
            if screen_x >= buf_width {
                break;
            }

            // Map screen pixel to image coordinates
            let img_x_f = (sx as f64 - center_x) / scale + off_x;
            let img_y_f = (sy as f64 - center_y) / scale + off_y;

            // Checkerboard for background
            let checker_col = (sx / cell) & 1;
            let checker_row = (sy / cell) & 1;
            let bg = if (checker_col ^ checker_row) == 0 {
                light
            } else {
                dark
            };

            let pixel_idx = row_start + screen_x;

            // Check if this screen pixel maps to a valid image pixel
            let img_x = img_x_f as i64;
            let img_y = img_y_f as i64;

            if img_x >= 0
                && img_y >= 0
                && (img_x as u32) < img_w
                && (img_y as u32) < img_h
            {
                let src_idx = ((img_y as u32 * img_w + img_x as u32) * 4) as usize;

                if src_idx + 3 < image.pixels.len() {
                    let r = image.pixels[src_idx] as u32;
                    let g = image.pixels[src_idx + 1] as u32;
                    let b = image.pixels[src_idx + 2] as u32;
                    let a = image.pixels[src_idx + 3] as u32;

                    if a == 255 {
                        buffer[pixel_idx] = 0xFF000000 | (r << 16) | (g << 8) | b;
                    } else if a == 0 {
                        buffer[pixel_idx] = bg;
                    } else {
                        // Alpha blend over checkerboard
                        let inv_a = 255 - a;
                        let bg_r = (bg >> 16) & 0xFF;
                        let bg_g = (bg >> 8) & 0xFF;
                        let bg_b = bg & 0xFF;
                        let out_r = (r * a + bg_r * inv_a) / 255;
                        let out_g = (g * a + bg_g * inv_a) / 255;
                        let out_b = (b * a + bg_b * inv_a) / 255;
                        buffer[pixel_idx] = 0xFF000000 | (out_r << 16) | (out_g << 8) | out_b;
                    }
                } else {
                    buffer[pixel_idx] = bg;
                }
            } else {
                buffer[pixel_idx] = bg;
            }
        }
    }
}
