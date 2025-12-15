//! Shared helpers for benchmarks

use token::config::EditorConfig;
use token::model::document::Document;
use token::model::editor::EditorState;
use token::model::editor_area::EditorArea;
use token::model::ui::UiState;
use token::model::AppModel;
use token::theme::Theme;

/// Create an AppModel with the specified number of lines
pub fn make_model(lines: usize) -> AppModel {
    let window_width = 1920u32;
    let window_height = 1080u32;
    let line_height = 20;
    let char_width = 10.0f32;

    let text = "The quick brown fox jumps over the lazy dog.\n".repeat(lines);
    let document = Document::with_text(&text);

    let status_bar_height = line_height;
    let visible_lines = (window_height as usize).saturating_sub(status_bar_height) / line_height;
    let visible_columns = ((window_width as f32 - 60.0) / char_width).floor() as usize;

    let editor = EditorState::with_viewport(visible_lines, visible_columns);
    let editor_area = EditorArea::single_document(document, editor);

    AppModel {
        editor_area,
        ui: UiState::new(),
        theme: Theme::default(),
        config: EditorConfig::default(),
        window_size: (window_width, window_height),
        line_height,
        char_width,
        #[cfg(debug_assertions)]
        debug_overlay: None,
    }
}

/// Simplified renderer for benchmarking the render phase without actual GPU/windowing
pub struct BenchRenderer {
    pub width: usize,
    pub height: usize,
    pub line_height: usize,
    pub char_width: usize,
    buffer: Vec<u32>,
    glyph: Vec<u8>,
}

impl BenchRenderer {
    pub fn new(width: usize, height: usize, line_height: usize) -> Self {
        let buffer = vec![0xFF1E1E2E; width * height];
        let glyph = vec![128u8; 10 * 16]; // ~10 wide, 16 tall fake glyph
        Self {
            width,
            height,
            line_height,
            char_width: 10,
            buffer,
            glyph,
        }
    }

    #[inline]
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

    /// Render a frame simulating the work done by the real Renderer
    /// This exercises buffer clearing, visible line iteration, and glyph blending
    pub fn render_frame(&mut self, model: &AppModel) {
        let bg_color = 0xFF1E1E2E_u32;
        self.buffer.fill(bg_color);

        let doc = model.document();
        let editor = model.editor();
        let viewport = &editor.viewport;

        let visible_lines = (self.height / self.line_height).min(50);
        let chars_per_line = (self.width / self.char_width).min(180);
        let fg_color = 0xFFCDD6F4_u32;
        let glyph_width = 10;
        let glyph_height = 16;

        for line_offset in 0..visible_lines {
            let line_idx = viewport.top_line + line_offset;
            if line_idx >= doc.line_count() {
                break;
            }

            let base_y = line_offset * self.line_height + 2;
            if base_y + glyph_height > self.height {
                continue;
            }

            for char_idx in 0..chars_per_line {
                let base_x = 60 + char_idx * self.char_width; // 60px for gutter

                for (gy, row) in self.glyph.chunks(glyph_width).enumerate() {
                    for (gx, &alpha) in row.iter().enumerate() {
                        if alpha > 0 {
                            let px = base_x + gx;
                            let py = base_y + gy;
                            if px < self.width && py < self.height {
                                let idx = py * self.width + px;
                                self.buffer[idx] =
                                    Self::blend_pixel(self.buffer[idx], fg_color, alpha);
                            }
                        }
                    }
                }
            }
        }

        divan::black_box(&self.buffer);
    }
}
