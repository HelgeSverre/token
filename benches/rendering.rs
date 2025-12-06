//! Benchmarks for rendering operations
//!
//! Run with: cargo bench rendering

#[global_allocator]
static ALLOC: divan::AllocProfiler = divan::AllocProfiler::system();

fn main() {
    divan::main();
}

// ============================================================================
// Buffer clearing
// ============================================================================

#[divan::bench(args = [800, 1280, 1920, 2560])]
fn clear_buffer_fill(width: usize) {
    let height = width * 9 / 16; // 16:9 aspect ratio
    let mut buffer: Vec<u32> = vec![0; width * height];
    let bg_color = 0xFF1E1E2E_u32;
    
    buffer.fill(bg_color);
    divan::black_box(&buffer);
}

#[divan::bench(args = [800, 1280, 1920, 2560])]
fn clear_buffer_iter(width: usize) {
    let height = width * 9 / 16;
    let mut buffer: Vec<u32> = vec![0; width * height];
    let bg_color = 0xFF1E1E2E_u32;
    
    for pixel in buffer.iter_mut() {
        *pixel = bg_color;
    }
    divan::black_box(&buffer);
}

// ============================================================================
// Alpha blending
// ============================================================================

fn blend_pixel(bg: u32, fg: u32, alpha: u8) -> u32 {
    let a = alpha as u32;
    let inv_a = 255 - a;
    
    let bg_r = (bg >> 16) & 0xFF;
    let bg_g = (bg >> 8) & 0xFF;
    let bg_b = bg & 0xFF;
    
    let fg_r = (fg >> 16) & 0xFF;
    let fg_g = (fg >> 8) & 0xFF;
    let fg_b = fg & 0xFF;
    
    let r = (fg_r * a + bg_r * inv_a) / 255;
    let g = (fg_g * a + bg_g * inv_a) / 255;
    let b = (fg_b * a + bg_b * inv_a) / 255;
    
    0xFF000000 | (r << 16) | (g << 8) | b
}

#[divan::bench]
fn alpha_blend_single_glyph() {
    let mut buffer: Vec<u32> = vec![0xFF1E1E2E; 32 * 32];
    let glyph = vec![128u8; 16 * 16];
    let fg_color = 0xFFCDD6F4_u32;
    
    for (gy, row) in glyph.chunks(16).enumerate() {
        for (gx, &alpha) in row.iter().enumerate() {
            if alpha > 0 {
                let idx = gy * 32 + gx;
                buffer[idx] = blend_pixel(buffer[idx], fg_color, alpha);
            }
        }
    }
    divan::black_box(&buffer);
}

#[divan::bench(args = [100, 500, 1000, 2000])]
fn alpha_blend_text_line(glyph_count: usize) {
    let width = 1920;
    let height = 32;
    let mut buffer: Vec<u32> = vec![0xFF1E1E2E; width * height];
    let glyph = vec![128u8; 16 * 16];
    let fg_color = 0xFFCDD6F4_u32;
    
    for i in 0..glyph_count {
        let base_x = (i * 10) % (width - 16);
        let base_y = 8;
        
        for (gy, row) in glyph.chunks(16).enumerate() {
            for (gx, &alpha) in row.iter().enumerate() {
                if alpha > 0 {
                    let px = base_x + gx;
                    let py = base_y + gy;
                    if px < width && py < height {
                        let idx = py * width + px;
                        buffer[idx] = blend_pixel(buffer[idx], fg_color, alpha);
                    }
                }
            }
        }
    }
    divan::black_box(&buffer);
}

// ============================================================================
// Full screen rendering simulation
// ============================================================================

#[divan::bench(args = [25, 50, 100])]
fn render_visible_lines(line_count: usize) {
    let width = 1920;
    let line_height = 20;
    let height = line_count * line_height;
    let mut buffer: Vec<u32> = vec![0xFF1E1E2E; width * height];
    let glyph = vec![128u8; 10 * 16]; // ~10 wide, 16 tall
    let fg_color = 0xFFCDD6F4_u32;
    let chars_per_line = 80;
    
    for line in 0..line_count {
        let base_y = line * line_height + 2;
        
        for char_idx in 0..chars_per_line {
            let base_x = char_idx * 10;
            
            for (gy, row) in glyph.chunks(10).enumerate() {
                for (gx, &alpha) in row.iter().enumerate() {
                    if alpha > 0 {
                        let px = base_x + gx;
                        let py = base_y + gy;
                        if px < width && py < height {
                            let idx = py * width + px;
                            buffer[idx] = blend_pixel(buffer[idx], fg_color, alpha);
                        }
                    }
                }
            }
        }
    }
    divan::black_box(&buffer);
}

// ============================================================================
// Line number rendering
// ============================================================================

#[divan::bench(args = [25, 50, 100])]
fn render_line_numbers(line_count: usize) {
    let gutter_width = 50;
    let line_height = 20;
    let height = line_count * line_height;
    let mut buffer: Vec<u32> = vec![0xFF1E1E2E; gutter_width * height];
    let digit_glyph = vec![128u8; 8 * 16];
    let fg_color = 0xFF6C7086_u32;
    
    for line in 0..line_count {
        let line_num = line + 1;
        let num_str = line_num.to_string();
        let base_y = line * line_height + 2;
        let base_x = gutter_width - (num_str.len() * 8) - 4;
        
        for (digit_idx, _) in num_str.chars().enumerate() {
            let dx = base_x + digit_idx * 8;
            
            for (gy, row) in digit_glyph.chunks(8).enumerate() {
                for (gx, &alpha) in row.iter().enumerate() {
                    if alpha > 0 {
                        let px = dx + gx;
                        let py = base_y + gy;
                        if px < gutter_width && py < height {
                            let idx = py * gutter_width + px;
                            buffer[idx] = blend_pixel(buffer[idx], fg_color, alpha);
                        }
                    }
                }
            }
        }
    }
    divan::black_box(&buffer);
}

// ============================================================================
// Selection highlighting
// ============================================================================

#[divan::bench]
fn highlight_selection_region() {
    let width = 1920;
    let height = 500;
    let mut buffer: Vec<u32> = vec![0xFF1E1E2E; width * height];
    let selection_color = 0xFF45475A_u32;
    
    // Highlight lines 10-20, columns 5-50
    let start_line = 10;
    let end_line = 20;
    let start_col = 5;
    let end_col = 50;
    let line_height = 20;
    let char_width = 10;
    
    for line in start_line..end_line {
        let y_start = line * line_height;
        let y_end = y_start + line_height;
        let x_start = start_col * char_width;
        let x_end = end_col * char_width;
        
        for y in y_start..y_end.min(height) {
            for x in x_start..x_end.min(width) {
                let idx = y * width + x;
                buffer[idx] = selection_color;
            }
        }
    }
    divan::black_box(&buffer);
}

// ============================================================================
// Cursor rendering
// ============================================================================

#[divan::bench(args = [1, 5, 10, 50])]
fn render_cursors(cursor_count: usize) {
    let width = 1920;
    let height = 1080;
    let mut buffer: Vec<u32> = vec![0xFF1E1E2E; width * height];
    let cursor_color = 0xFFF5E0DC_u32;
    let cursor_width = 2;
    let line_height = 20;
    
    for i in 0..cursor_count {
        let x = (i * 100) % (width - cursor_width);
        let y = (i * 30) % (height - line_height);
        
        for cy in 0..line_height {
            for cx in 0..cursor_width {
                let idx = (y + cy) * width + (x + cx);
                if idx < buffer.len() {
                    buffer[idx] = cursor_color;
                }
            }
        }
    }
    divan::black_box(&buffer);
}
