//! Benchmarks for text layout and measurement operations
//!
//! Run with: cargo bench layout

mod support;
use support::make_model;

use fontdue::{Font, FontSettings};
use ropey::Rope;
use std::collections::HashMap;

#[global_allocator]
static ALLOC: divan::AllocProfiler = divan::AllocProfiler::system();

fn main() {
    divan::main();
}

fn load_test_font() -> Font {
    Font::from_bytes(
        include_bytes!("../assets/JetBrainsMono.ttf") as &[u8],
        FontSettings::default(),
    )
    .expect("Failed to load font")
}

// ============================================================================
// Line width measurement
// ============================================================================

#[divan::bench(args = [80, 120, 200, 500])]
fn measure_line_width(max_chars: usize) {
    let font = load_test_font();
    let font_size = 16.0_f32;
    let line = "x".repeat(max_chars);

    let mut width = 0.0_f32;
    for ch in line.chars() {
        let metrics = font.metrics(ch, font_size);
        width += metrics.advance_width;
    }
    divan::black_box(width);
}

#[divan::bench]
fn measure_realistic_line() {
    let font = load_test_font();
    let font_size = 16.0_f32;
    let line = "    fn process_document(&mut self, doc: &Document) -> Result<(), Error> {";

    let mut width = 0.0_f32;
    for ch in line.chars() {
        let metrics = font.metrics(ch, font_size);
        width += metrics.advance_width;
    }
    divan::black_box(width);
}

#[divan::bench]
fn measure_mixed_content_line() {
    let font = load_test_font();
    let font_size = 16.0_f32;
    // Line with various character types: ASCII, numbers, symbols
    let line = "const CONFIG_PATH: &str = \"./config/settings_2024.yaml\"; // TODO: fix";

    let mut width = 0.0_f32;
    for ch in line.chars() {
        let metrics = font.metrics(ch, font_size);
        width += metrics.advance_width;
    }
    divan::black_box(width);
}

// ============================================================================
// Visible lines calculation
// ============================================================================

#[divan::bench(args = [25, 50, 100])]
fn calculate_visible_lines(viewport_lines: usize) {
    let model = make_model(50_000);
    let doc = model.document();
    let viewport = &model.editor().viewport;

    let mut lines: Vec<Option<String>> = Vec::with_capacity(viewport_lines);
    for i in 0..viewport_lines {
        let line_idx = viewport.top_line + i;
        if line_idx < doc.line_count() {
            lines.push(doc.get_line(line_idx));
        }
    }
    divan::black_box(lines);
}

#[divan::bench(args = [25, 50, 100])]
fn collect_visible_line_strings(viewport_lines: usize) {
    let model = make_model(50_000);
    let doc = model.document();
    let viewport = &model.editor().viewport;

    let mut lines: Vec<String> = Vec::with_capacity(viewport_lines);
    for i in 0..viewport_lines {
        let line_idx = viewport.top_line + i;
        if line_idx < doc.line_count() {
            if let Some(line) = doc.get_line(line_idx) {
                lines.push(line);
            }
        }
    }
    divan::black_box(lines);
}

// ============================================================================
// Character position calculations
// ============================================================================

#[divan::bench]
fn char_position_in_line() {
    let font = load_test_font();
    let font_size = 16.0_f32;
    let line = "The quick brown fox jumps over the lazy dog.";
    let target_col = 20;

    let mut x = 0.0_f32;
    for (i, ch) in line.chars().enumerate() {
        if i >= target_col {
            break;
        }
        let metrics = font.metrics(ch, font_size);
        x += metrics.advance_width;
    }
    divan::black_box(x);
}

#[divan::bench(args = [20, 50, 100])]
fn char_positions_multiple_cols(target_col: usize) {
    let font = load_test_font();
    let font_size = 16.0_f32;
    let line = "x".repeat(200);

    let mut x = 0.0_f32;
    for (i, ch) in line.chars().enumerate() {
        if i >= target_col {
            break;
        }
        let metrics = font.metrics(ch, font_size);
        x += metrics.advance_width;
    }
    divan::black_box(x);
}

// ============================================================================
// Column from X position (inverse operation)
// ============================================================================

#[divan::bench]
fn column_from_x_position() {
    let font = load_test_font();
    let font_size = 16.0_f32;
    let line = "The quick brown fox jumps over the lazy dog.";
    let target_x = 200.0_f32; // Pixels from left

    let mut current_x = 0.0_f32;
    let mut col = 0;
    for ch in line.chars() {
        let metrics = font.metrics(ch, font_size);
        if current_x + metrics.advance_width / 2.0 > target_x {
            break;
        }
        current_x += metrics.advance_width;
        col += 1;
    }
    divan::black_box(col);
}

// ============================================================================
// Cached metrics lookup (simulating monospace optimization)
// ============================================================================

#[divan::bench]
fn cached_char_width_lookup() {
    let font = load_test_font();
    let font_size = 16.0_f32;

    // Precalculate char width (monospace assumption)
    let char_width = font.metrics('M', font_size).advance_width;

    // Use cached width for 1000 column calculations
    for col in 0..1000 {
        let x = col as f32 * char_width;
        divan::black_box(x);
    }
}

#[divan::bench]
fn uncached_char_width_per_char() {
    let font = load_test_font();
    let font_size = 16.0_f32;
    let line = "M".repeat(1000);

    let mut x = 0.0_f32;
    for ch in line.chars() {
        let metrics = font.metrics(ch, font_size);
        x += metrics.advance_width;
    }
    divan::black_box(x);
}

// ============================================================================
// Tab expansion in layout
// ============================================================================

#[divan::bench]
fn measure_line_with_tabs() {
    let font = load_test_font();
    let font_size = 16.0_f32;
    let tab_width = 4;
    let char_width = font.metrics('M', font_size).advance_width;
    let line = "\t\tfn main() {\n\t\t\tprintln!(\"Hello\");\n\t\t}";

    let mut visual_col = 0;
    for ch in line.chars() {
        if ch == '\t' {
            let spaces_to_next = tab_width - (visual_col % tab_width);
            visual_col += spaces_to_next;
        } else if ch != '\n' {
            visual_col += 1;
        }
    }
    let total_width = visual_col as f32 * char_width;
    divan::black_box(total_width);
}

// ============================================================================
// Full viewport layout simulation
// ============================================================================

#[divan::bench(args = [25, 50, 100])]
fn full_viewport_layout(visible_lines: usize) {
    let font = load_test_font();
    let font_size = 16.0_f32;
    let rope = Rope::from_str(&"The quick brown fox jumps over the lazy dog.\n".repeat(10_000));
    let start_line = 5000;

    let mut layout_data: Vec<(usize, f32)> = Vec::with_capacity(visible_lines);

    for line_offset in 0..visible_lines {
        let line_idx = start_line + line_offset;
        if line_idx >= rope.len_lines() {
            break;
        }

        let line = rope.line(line_idx);
        let mut width = 0.0_f32;
        for ch in line.chars() {
            let metrics = font.metrics(ch, font_size);
            width += metrics.advance_width;
        }

        layout_data.push((line_idx, width));
    }

    divan::black_box(layout_data);
}

#[divan::bench(args = [25, 50, 100])]
fn viewport_layout_with_cache(visible_lines: usize) {
    let font = load_test_font();
    let font_size = 16.0_f32;
    let rope = Rope::from_str(&"The quick brown fox jumps over the lazy dog.\n".repeat(10_000));
    let start_line = 5000;

    // Pre-warm cache with common ASCII characters
    let mut width_cache: HashMap<char, f32> = HashMap::new();
    for ch in ' '..='~' {
        width_cache.insert(ch, font.metrics(ch, font_size).advance_width);
    }

    let mut layout_data: Vec<(usize, f32)> = Vec::with_capacity(visible_lines);

    for line_offset in 0..visible_lines {
        let line_idx = start_line + line_offset;
        if line_idx >= rope.len_lines() {
            break;
        }

        let line = rope.line(line_idx);
        let mut width = 0.0_f32;
        for ch in line.chars() {
            let char_width = *width_cache
                .entry(ch)
                .or_insert_with(|| font.metrics(ch, font_size).advance_width);
            width += char_width;
        }

        layout_data.push((line_idx, width));
    }

    divan::black_box(layout_data);
}

// ============================================================================
// Gutter width calculation
// ============================================================================

#[divan::bench(args = [100, 1_000, 10_000, 100_000, 1_000_000])]
fn calculate_gutter_width(line_count: usize) {
    let font = load_test_font();
    let font_size = 16.0_f32;

    // Calculate number of digits needed
    let digits = line_count.to_string().len();

    // Measure width of digits
    let digit_width = font.metrics('9', font_size).advance_width;
    let gutter_width = (digits as f32 * digit_width) + 16.0; // 16px padding

    divan::black_box(gutter_width);
}
