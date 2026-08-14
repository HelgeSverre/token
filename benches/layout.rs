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

// ============================================================================
// Layout engine (src/layout): chrome-shaped trees and RowList panels
// ============================================================================

mod engine {
    use token::layout::{
        CellMeasure, Content, Dir, ElementDecl, RowListDecl, Sizing, SizingAxes, TextDecl,
        TextStyle, UiKey, UiTree, Wrap,
    };
    use token::model::editor_area::Rect;
    use token::panel::{DockPosition, PanelId};

    fn measure() -> CellMeasure {
        CellMeasure {
            char_width: 8.4,
            line_height: 19.0,
        }
    }

    /// A dock-chrome-shaped tree (~60 nodes): two docks, tab strips with
    /// text tabs, spacers, content areas with row lists.
    #[divan::bench]
    fn solve_chrome_shaped_tree() {
        let mut t = UiTree::new();
        t.node(
            ElementDecl {
                dir: Dir::Column,
                ..Default::default()
            },
            |t| {
                for (pos, panel) in [
                    (DockPosition::Bottom, PanelId::Problems),
                    (DockPosition::Right, PanelId::Outline),
                ] {
                    t.node(
                        ElementDecl {
                            key: Some(UiKey::Dock(pos)),
                            dir: Dir::Column,
                            sizing: SizingAxes::new(Sizing::GROW, Sizing::Fixed(240.0)),
                            ..Default::default()
                        },
                        |t| {
                            t.node(
                                ElementDecl {
                                    key: Some(UiKey::DockHeader(pos)),
                                    dir: Dir::Row,
                                    sizing: SizingAxes::new(Sizing::GROW, Sizing::Fixed(28.0)),
                                    gap: 4.0,
                                    ..Default::default()
                                },
                                |t| {
                                    for i in 0..12 {
                                        t.text(None, format!("Panel {i}"), TextStyle::sized(13.0));
                                    }
                                    t.leaf(ElementDecl {
                                        sizing: SizingAxes::new(Sizing::GROW, Sizing::GROW),
                                        ..Default::default()
                                    });
                                    t.text(None, "\u{2717}3 \u{26A0}2", TextStyle::sized(11.0));
                                },
                            );
                            t.node(
                                ElementDecl {
                                    key: Some(UiKey::PanelContent(panel)),
                                    sizing: SizingAxes::grow(),
                                    clip: true,
                                    ..Default::default()
                                },
                                |t| {
                                    t.leaf(ElementDecl {
                                        key: Some(UiKey::PanelRows(panel)),
                                        sizing: SizingAxes::grow(),
                                        content: Content::RowList(RowListDecl {
                                            row_height: 22.0,
                                            count: 300,
                                            scroll_offset: 40,
                                        }),
                                        ..Default::default()
                                    });
                                },
                            );
                        },
                    );
                }
            },
        );
        let mut m = measure();
        let snap = t.solve(Rect::new(0.0, 0.0, 1920.0, 1080.0), 1.0, &mut m);
        divan::black_box(snap);
    }

    /// A virtualized panel with 5000 rows stays O(1) in row count.
    #[divan::bench]
    fn solve_row_list_5000_rows() {
        let mut t = UiTree::new();
        t.node(
            ElementDecl {
                dir: Dir::Column,
                ..Default::default()
            },
            |t| {
                t.leaf(ElementDecl {
                    key: Some(UiKey::PanelRows(PanelId::Problems)),
                    sizing: SizingAxes::grow(),
                    content: Content::RowList(RowListDecl {
                        row_height: 22.0,
                        count: 5000,
                        scroll_offset: 2500,
                    }),
                    ..Default::default()
                });
            },
        );
        let mut m = measure();
        let snap = t.solve(Rect::new(0.0, 0.0, 800.0, 400.0), 1.0, &mut m);
        let rows = snap.row_list(UiKey::PanelRows(PanelId::Problems)).unwrap();
        divan::black_box((rows.visible_capacity(), rows.max_scroll()));
    }

    /// A hover-card-shaped tree with word wrapping under CellMeasure.
    #[divan::bench]
    fn solve_hover_tree_with_wrap() {
        let text = "The quick brown fox jumps over the lazy dog. \
                    Pack my box with five dozen liquor jugs. \
                    Sphinx of black quartz, judge my vow."
            .repeat(4);
        let mut t = UiTree::new();
        t.node(
            ElementDecl {
                dir: Dir::Column,
                sizing: SizingAxes::new(Sizing::Fixed(420.0), Sizing::FIT),
                padding: token::layout::Padding::all(12.0),
                gap: 8.0,
                ..Default::default()
            },
            |t| {
                t.leaf(ElementDecl {
                    key: Some(UiKey::OverlayPanel),
                    sizing: SizingAxes::new(Sizing::GROW, Sizing::FIT),
                    content: Content::Text(TextDecl {
                        text,
                        style: TextStyle::sized(13.0),
                        wrap: Wrap::Words,
                    }),
                    ..Default::default()
                });
            },
        );
        let mut m = measure();
        let snap = t.solve(Rect::new(0.0, 0.0, 1920.0, 1080.0), 1.0, &mut m);
        divan::black_box(snap);
    }
}
