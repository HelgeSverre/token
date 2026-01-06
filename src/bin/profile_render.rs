//! Profiling binary for render performance analysis
//!
//! This binary opens multiple files in a split layout and renders frames
//! for profiling with samply or other profilers.
//!
//! Usage:
//!   cargo build --profile profiling --bin profile_render
//!   samply record ./target/profiling/profile_render
//!
//! Or to profile with a specific scenario:
//!   samply record ./target/profiling/profile_render --frames 1000 --splits 3

use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::Result;
use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "profile_render")]
#[command(about = "Profile rendering performance with multiple splits")]
struct Args {
    /// Number of frames to render
    #[arg(long, default_value = "500")]
    frames: usize,

    /// Number of editor splits
    #[arg(long, default_value = "3")]
    splits: usize,

    /// Files to open (will cycle through if fewer than splits)
    #[arg(long)]
    files: Vec<PathBuf>,

    /// Generate synthetic content if no files provided
    #[arg(long, default_value = "10000")]
    lines: usize,

    /// Include a CSV file in the splits
    #[arg(long)]
    include_csv: bool,

    /// Window width
    #[arg(long, default_value = "1920")]
    width: u32,

    /// Window height  
    #[arg(long, default_value = "1080")]
    height: u32,

    /// Simulate scrolling during render
    #[arg(long)]
    scroll: bool,

    /// Print timing statistics
    #[arg(long)]
    stats: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();

    eprintln!("Profile Render - Multi-Split Performance Test");
    eprintln!("==============================================");
    eprintln!("Frames: {}", args.frames);
    eprintln!("Splits: {}", args.splits);
    eprintln!("Window: {}x{}", args.width, args.height);
    eprintln!();

    // Create the application model
    let mut model = create_model(&args)?;

    eprintln!(
        "Model created with {} splits",
        model.editor_area.groups.len()
    );
    for (id, group) in &model.editor_area.groups {
        if let Some(editor_id) = group.active_editor_id() {
            if let Some(editor) = model.editor_area.editors.get(&editor_id) {
                if let Some(doc_id) = editor.document_id {
                    if let Some(doc) = model.editor_area.documents.get(&doc_id) {
                        eprintln!("  Group {:?}: {} lines", id, doc.line_count());
                    }
                }
            }
        }
    }
    eprintln!();

    // Set up rendering infrastructure (headless)
    let (font, line_height, char_width, font_size, ascent) = setup_font(args.height);

    let width = args.width as usize;
    let height = args.height as usize;
    let mut buffer: Vec<u32> = vec![0xFF1E1E2E; width * height];
    let mut glyph_cache = std::collections::HashMap::new();

    // Pre-warm the glyph cache with ASCII characters
    for ch in ' '..='~' {
        let (metrics, bitmap) = font.rasterize(ch, font_size);
        glyph_cache.insert(ch, (metrics, bitmap));
    }

    eprintln!(
        "Glyph cache pre-warmed with {} characters",
        glyph_cache.len()
    );
    eprintln!();

    // Compute layout once (as the real renderer does)
    let sidebar_width = 0.0f32;
    let status_bar_height = line_height;
    let available_rect = token::model::editor_area::Rect::new(
        sidebar_width,
        0.0,
        (width as f32) - sidebar_width,
        (height - status_bar_height) as f32,
    );
    let splitters = model
        .editor_area
        .compute_layout_scaled(available_rect, model.metrics.splitter_width);

    eprintln!("Starting render loop ({} frames)...", args.frames);
    eprintln!();

    let mut frame_times: Vec<Duration> = Vec::with_capacity(args.frames);
    let start_time = Instant::now();

    for frame in 0..args.frames {
        let frame_start = Instant::now();

        // Simulate scrolling to exercise different code paths
        if args.scroll && frame % 10 == 0 {
            for (_, editor) in model.editor_area.editors.iter_mut() {
                let max_scroll = 100;
                editor.viewport.top_line = (frame / 10) % max_scroll;
            }
        }

        // Clear the buffer (as real renderer does)
        buffer.fill(0xFF1E1E2E);

        // Render each editor group (the hot path we're profiling)
        for (&group_id, group) in &model.editor_area.groups {
            render_editor_group(
                &mut buffer,
                width,
                height,
                &model,
                group_id,
                group.rect,
                &font,
                &mut glyph_cache,
                font_size,
                ascent,
                line_height,
                char_width,
            );
        }

        // Render splitters
        for splitter in &splitters {
            let rect = splitter.rect;
            let x0 = rect.x as usize;
            let y0 = rect.y as usize;
            let x1 = (rect.x + rect.width) as usize;
            let y1 = (rect.y + rect.height) as usize;
            let color = 0xFF45475A;
            for y in y0..y1.min(height) {
                for x in x0..x1.min(width) {
                    buffer[y * width + x] = color;
                }
            }
        }

        frame_times.push(frame_start.elapsed());

        // Progress indicator
        if (frame + 1) % 100 == 0 {
            eprintln!("  Rendered {} frames...", frame + 1);
        }
    }

    let total_time = start_time.elapsed();

    eprintln!();
    eprintln!("Render complete!");
    eprintln!();

    if args.stats {
        print_stats(&frame_times, total_time, args.frames);
    } else {
        let avg_ms = total_time.as_secs_f64() * 1000.0 / args.frames as f64;
        let fps = args.frames as f64 / total_time.as_secs_f64();
        eprintln!("Average: {:.2}ms/frame ({:.1} FPS)", avg_ms, fps);
    }

    // Prevent the buffer from being optimized away
    std::hint::black_box(&buffer);

    Ok(())
}

fn create_model(args: &Args) -> Result<token::model::AppModel> {
    use token::config::EditorConfig;
    use token::messages::{LayoutMsg, Msg};
    use token::model::document::Document;
    use token::model::editor::EditorState;
    use token::model::editor_area::EditorArea;
    use token::model::ui::UiState;
    use token::model::AppModel;
    use token::theme::Theme;
    use token::update::update;

    let line_height = 20usize;
    let char_width = 10.0f32;

    // Calculate viewport dimensions
    let status_bar_height = line_height;
    let visible_lines = (args.height as usize).saturating_sub(status_bar_height) / line_height;
    let visible_columns = ((args.width as f32 - 60.0) / char_width).floor() as usize;

    // Generate content for all splits
    let mut doc_contents: Vec<String> = Vec::new();

    if !args.files.is_empty() {
        for path in &args.files {
            let content = std::fs::read_to_string(path)
                .unwrap_or_else(|_| format!("// Failed to load {}\n", path.display()));
            doc_contents.push(content);
        }
    } else {
        doc_contents.push(generate_code_content(args.lines));
        doc_contents.push(generate_rust_content(args.lines));
        if args.include_csv || args.splits >= 3 {
            doc_contents.push(generate_csv_content(args.lines));
        }
    }

    while doc_contents.len() < args.splits {
        let idx = doc_contents.len();
        doc_contents.push(generate_code_content(args.lines / 2 + idx * 100));
    }

    // Create initial model with first document
    let first_content = doc_contents.remove(0);
    let document = Document::with_text(&first_content);
    let editor = EditorState::with_viewport(visible_lines, visible_columns);
    let editor_area = EditorArea::single_document(document, editor);

    let mut model = AppModel {
        editor_area,
        ui: UiState::new(),
        theme: Theme::default(),
        config: EditorConfig::default(),
        window_size: (args.width, args.height),
        line_height,
        char_width,
        metrics: token::model::ScaledMetrics::default(),
        workspace: None,
        #[cfg(debug_assertions)]
        debug_overlay: None,
    };

    // Add more splits using the layout system
    for content in doc_contents.into_iter().take(args.splits - 1) {
        // Split the current focused group horizontally (side by side)
        update(
            &mut model,
            Msg::Layout(LayoutMsg::SplitFocused(
                token::model::editor_area::SplitDirection::Horizontal,
            )),
        );

        // Replace the document content in the new split
        if let Some(doc) = model.editor_area.focused_document_mut() {
            doc.buffer = ropey::Rope::from_str(&content);
        }
    }

    Ok(model)
}

fn setup_font(_window_height: u32) -> (fontdue::Font, usize, f32, f32, f32) {
    use fontdue::{Font, FontSettings};

    let font = Font::from_bytes(
        include_bytes!("../../assets/JetBrainsMono.ttf") as &[u8],
        FontSettings::default(),
    )
    .expect("Failed to load font");

    let scale_factor = 2.0f64; // Simulate retina
    let font_size = 14.0 * scale_factor as f32;

    let line_metrics = font
        .horizontal_line_metrics(font_size)
        .expect("Font missing line metrics");

    let line_height = line_metrics.new_line_size.ceil() as usize;
    let (metrics, _) = font.rasterize('M', font_size);
    let char_width = metrics.advance_width;
    let ascent = line_metrics.ascent;

    (font, line_height, char_width, font_size, ascent)
}

fn generate_code_content(lines: usize) -> String {
    let mut content = String::with_capacity(lines * 80);
    for i in 0..lines {
        content.push_str(&format!(
            "    fn process_document_{}(&mut self, doc: &Document) -> Result<(), Error> {{\n",
            i
        ));
    }
    content
}

fn generate_rust_content(lines: usize) -> String {
    let mut content = String::with_capacity(lines * 80);
    content.push_str("use std::collections::HashMap;\n\n");
    for i in 0..lines {
        match i % 5 {
            0 => content.push_str(&format!("pub struct Handler{} {{\n", i)),
            1 => content.push_str(&format!("    field_{}: String,\n", i)),
            2 => content.push_str(&format!("    data_{}: Vec<u8>,\n", i)),
            3 => content.push_str("}\n\n"),
            _ => content.push_str(&format!("impl Handler{} {{\n", i.saturating_sub(1))),
        }
    }
    content
}

fn generate_csv_content(rows: usize) -> String {
    let mut content = String::with_capacity(rows * 150);
    content.push_str(
        "id,first_name,last_name,email,company,department,job_title,salary,hire_date,country\n",
    );
    for i in 0..rows {
        content.push_str(&format!(
            "{},John{},Smith{},john{}@company.com,Company{},Engineering,Developer,{},{}-01-15,USA\n",
            i, i % 100, i % 50, i, i % 10, 50000 + (i % 100) * 1000, 2020 + (i % 5)
        ));
    }
    content
}

type GlyphCache = std::collections::HashMap<char, (fontdue::Metrics, Vec<u8>)>;

#[allow(clippy::too_many_arguments)]
fn render_editor_group(
    buffer: &mut [u32],
    width: usize,
    height: usize,
    model: &token::model::AppModel,
    group_id: token::model::editor_area::GroupId,
    group_rect: token::model::editor_area::Rect,
    font: &fontdue::Font,
    glyph_cache: &mut GlyphCache,
    font_size: f32,
    ascent: f32,
    line_height: usize,
    char_width: f32,
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

    // Fill background
    let bg_color = 0xFF1E1E2E;
    for y in rect_y..rect_y + rect_h {
        if y >= height {
            break;
        }
        for x in rect_x..rect_x + rect_w {
            if x >= width {
                break;
            }
            buffer[y * width + x] = bg_color;
        }
    }

    // Tab bar
    let tab_bar_height = 28usize;
    let tab_bar_color = 0xFF181825;
    for y in rect_y..rect_y + tab_bar_height.min(rect_h) {
        if y >= height {
            break;
        }
        for x in rect_x..rect_x + rect_w {
            if x >= width {
                break;
            }
            buffer[y * width + x] = tab_bar_color;
        }
    }

    // Content area
    let content_y = rect_y + tab_bar_height;
    let content_h = rect_h.saturating_sub(tab_bar_height);
    let gutter_width = 60usize;
    let text_x = rect_x + gutter_width;

    let visible_lines = content_h / line_height;
    let visible_columns = ((rect_w - gutter_width) as f32 / char_width).floor() as usize;
    let end_line = (editor.viewport.top_line + visible_lines).min(document.line_count());

    // Current line highlight
    let current_line = editor.cursors.first().map(|c| c.line).unwrap_or(0);
    if current_line >= editor.viewport.top_line && current_line < end_line {
        let screen_line = current_line - editor.viewport.top_line;
        let y = content_y + screen_line * line_height;
        let highlight_color = 0xFF2A2A3A;
        for py in y..y + line_height {
            if py >= height {
                break;
            }
            for px in rect_x..rect_x + rect_w {
                if px >= width {
                    break;
                }
                buffer[py * width + px] = highlight_color;
            }
        }
    }

    // Render text - THIS IS THE HOT PATH WE'RE PROFILING
    let text_color = 0xFFCDD6F4;
    let mut display_text_buf = String::with_capacity(visible_columns + 16);

    for (screen_line, doc_line) in (editor.viewport.top_line..end_line).enumerate() {
        // Use the optimized get_line_cow method
        if let Some(line_text) = document.get_line_cow(doc_line) {
            let y = content_y + screen_line * line_height;
            if y >= content_y + content_h || y >= height {
                break;
            }

            // Expand tabs (using Cow to avoid allocation when no tabs)
            let expanded = expand_tabs(&line_text);

            // Build display text with buffer reuse
            display_text_buf.clear();
            for ch in expanded
                .chars()
                .skip(editor.viewport.left_column)
                .take(visible_columns)
            {
                display_text_buf.push(ch);
            }

            // Draw text
            let mut x = text_x as f32;
            let baseline_y = y + (ascent as usize);

            for ch in display_text_buf.chars() {
                let (metrics, bitmap) = glyph_cache
                    .entry(ch)
                    .or_insert_with(|| font.rasterize(ch, font_size));

                let glyph_x = x as i32 + metrics.xmin;
                let glyph_y = baseline_y as i32 - metrics.ymin - metrics.height as i32;

                for gy in 0..metrics.height {
                    for gx in 0..metrics.width {
                        let alpha = bitmap[gy * metrics.width + gx];
                        if alpha > 0 {
                            let px = (glyph_x + gx as i32) as usize;
                            let py = (glyph_y + gy as i32) as usize;
                            if px < width && py < height {
                                let idx = py * width + px;
                                buffer[idx] = blend_pixel(buffer[idx], text_color, alpha);
                            }
                        }
                    }
                }

                x += metrics.advance_width;
            }
        }
    }

    // Render gutter (line numbers)
    let gutter_color = 0xFF6C7086;
    for (screen_line, doc_line) in (editor.viewport.top_line..end_line).enumerate() {
        let y = content_y + screen_line * line_height;
        if y >= height {
            break;
        }

        let num_str = (doc_line + 1).to_string();
        let num_x = rect_x + gutter_width - (num_str.len() as f32 * char_width) as usize - 8;

        let baseline_y = y + (ascent as usize);
        let mut x = num_x as f32;

        for ch in num_str.chars() {
            let (metrics, bitmap) = glyph_cache
                .entry(ch)
                .or_insert_with(|| font.rasterize(ch, font_size));

            let glyph_x = x as i32 + metrics.xmin;
            let glyph_y = baseline_y as i32 - metrics.ymin - metrics.height as i32;

            for gy in 0..metrics.height {
                for gx in 0..metrics.width {
                    let alpha = bitmap[gy * metrics.width + gx];
                    if alpha > 0 {
                        let px = (glyph_x + gx as i32) as usize;
                        let py = (glyph_y + gy as i32) as usize;
                        if px < width && py < height {
                            let idx = py * width + px;
                            buffer[idx] = blend_pixel(buffer[idx], gutter_color, alpha);
                        }
                    }
                }
            }

            x += metrics.advance_width;
        }
    }

    // Cursor
    let cursor_color = 0xFFF5E0DC;
    for cursor in &editor.cursors {
        if cursor.line >= editor.viewport.top_line && cursor.line < end_line {
            let screen_line = cursor.line - editor.viewport.top_line;
            let cursor_x = text_x + (cursor.column as f32 * char_width) as usize;
            let cursor_y = content_y + screen_line * line_height;

            for py in cursor_y..cursor_y + line_height {
                if py >= height {
                    break;
                }
                for px in cursor_x..cursor_x + 2 {
                    if px >= width {
                        break;
                    }
                    buffer[py * width + px] = cursor_color;
                }
            }
        }
    }
}

fn expand_tabs(text: &str) -> std::borrow::Cow<'_, str> {
    use std::borrow::Cow;

    if !text.contains('\t') {
        return Cow::Borrowed(text);
    }

    let mut result = String::with_capacity(text.len() * 2);
    let mut visual_col = 0;
    const TAB_WIDTH: usize = 4;

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

    Cow::Owned(result)
}

#[inline]
fn blend_pixel(bg: u32, fg: u32, alpha: u8) -> u32 {
    if alpha == 0 {
        return bg;
    }
    if alpha == 255 {
        return fg;
    }

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

fn print_stats(frame_times: &[Duration], total_time: Duration, frame_count: usize) {
    let mut sorted: Vec<_> = frame_times.to_vec();
    sorted.sort();

    let min = sorted.first().unwrap();
    let max = sorted.last().unwrap();
    let median = sorted[sorted.len() / 2];
    let p95 = sorted[(sorted.len() as f64 * 0.95) as usize];
    let p99 = sorted[(sorted.len() as f64 * 0.99) as usize];
    let avg = total_time / frame_count as u32;
    let fps = frame_count as f64 / total_time.as_secs_f64();

    eprintln!("Frame Time Statistics:");
    eprintln!("  Min:    {:>8.2}ms", min.as_secs_f64() * 1000.0);
    eprintln!("  Max:    {:>8.2}ms", max.as_secs_f64() * 1000.0);
    eprintln!("  Avg:    {:>8.2}ms", avg.as_secs_f64() * 1000.0);
    eprintln!("  Median: {:>8.2}ms", median.as_secs_f64() * 1000.0);
    eprintln!("  P95:    {:>8.2}ms", p95.as_secs_f64() * 1000.0);
    eprintln!("  P99:    {:>8.2}ms", p99.as_secs_f64() * 1000.0);
    eprintln!();
    eprintln!("  FPS:    {:>8.1}", fps);
    eprintln!("  Total:  {:>8.2}s", total_time.as_secs_f64());
}
