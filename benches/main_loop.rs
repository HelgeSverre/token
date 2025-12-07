//! Benchmarks for the main Msg → Update → Cmd → Render loop
//!
//! Run with: cargo bench main_loop

mod support;
use support::{make_model, BenchRenderer};

use token::messages::{AppMsg, Direction, DocumentMsg, EditorMsg, Msg};
use token::update::update;

#[global_allocator]
static ALLOC: divan::AllocProfiler = divan::AllocProfiler::system();

fn main() {
    divan::main();
}

// ============================================================================
// Update-only benchmarks (no rendering)
// Measures the cost of state transformations per message
// ============================================================================

#[divan::bench(args = [100, 1000])]
fn update_move_cursor_right(iterations: usize) {
    let mut model = make_model(10_000);

    for _ in 0..iterations {
        let cmd = update(
            &mut model,
            Msg::Editor(EditorMsg::MoveCursor(Direction::Right)),
        );
        divan::black_box(cmd);
    }

    divan::black_box(&model);
}

#[divan::bench(args = [100, 1000])]
fn update_move_cursor_down(iterations: usize) {
    let mut model = make_model(10_000);

    for _ in 0..iterations {
        let cmd = update(
            &mut model,
            Msg::Editor(EditorMsg::MoveCursor(Direction::Down)),
        );
        divan::black_box(cmd);
    }

    divan::black_box(&model);
}

#[divan::bench(args = [100, 500])]
fn update_insert_char(iterations: usize) {
    let mut model = make_model(10_000);

    for i in 0..iterations {
        let ch = (b'a' + (i % 26) as u8) as char;
        let cmd = update(&mut model, Msg::Document(DocumentMsg::InsertChar(ch)));
        divan::black_box(cmd);
    }

    divan::black_box(&model);
}

#[divan::bench(args = [100, 500])]
fn update_delete_backward(iterations: usize) {
    let mut model = make_model(10_000);

    // First insert some text to delete
    for i in 0..iterations {
        let ch = (b'a' + (i % 26) as u8) as char;
        update(&mut model, Msg::Document(DocumentMsg::InsertChar(ch)));
    }

    for _ in 0..iterations {
        let cmd = update(&mut model, Msg::Document(DocumentMsg::DeleteBackward));
        divan::black_box(cmd);
    }

    divan::black_box(&model);
}

#[divan::bench(args = [10, 50, 100])]
fn update_page_down(iterations: usize) {
    let mut model = make_model(50_000);

    for _ in 0..iterations {
        let cmd = update(&mut model, Msg::Editor(EditorMsg::PageDown));
        divan::black_box(cmd);
    }

    divan::black_box(&model);
}

#[divan::bench(args = [10, 50, 100])]
fn update_scroll(iterations: usize) {
    let mut model = make_model(50_000);

    for _ in 0..iterations {
        let cmd = update(&mut model, Msg::Editor(EditorMsg::Scroll(3)));
        divan::black_box(cmd);
    }

    divan::black_box(&model);
}

#[divan::bench]
fn update_undo_redo_cycle() {
    let mut model = make_model(1_000);

    // Insert 100 characters
    for i in 0..100 {
        let ch = (b'a' + (i % 26) as u8) as char;
        update(&mut model, Msg::Document(DocumentMsg::InsertChar(ch)));
    }

    // Undo all
    for _ in 0..100 {
        let cmd = update(&mut model, Msg::Document(DocumentMsg::Undo));
        divan::black_box(cmd);
    }

    // Redo all
    for _ in 0..100 {
        let cmd = update(&mut model, Msg::Document(DocumentMsg::Redo));
        divan::black_box(cmd);
    }

    divan::black_box(&model);
}

#[divan::bench]
fn update_select_all() {
    let mut model = make_model(10_000);
    let cmd = update(&mut model, Msg::Editor(EditorMsg::SelectAll));
    divan::black_box(cmd);
    divan::black_box(&model);
}

#[divan::bench]
fn update_resize_window() {
    let mut model = make_model(10_000);

    let sizes = [(800, 600), (1920, 1080), (2560, 1440), (3840, 2160)];
    for (w, h) in sizes.iter().cycle().take(20) {
        let cmd = update(&mut model, Msg::App(AppMsg::Resize(*w, *h)));
        divan::black_box(cmd);
    }

    divan::black_box(&model);
}

// ============================================================================
// Full loop benchmarks: Msg → Update → Cmd → Render
// Measures the complete "frame" cost for common operations
// ============================================================================

#[divan::bench(args = [100, 500])]
fn full_loop_cursor_move_and_render(iterations: usize) {
    let mut model = make_model(10_000);
    let mut renderer = BenchRenderer::new(1920, 1080, model.line_height);

    for _ in 0..iterations {
        let cmd = update(
            &mut model,
            Msg::Editor(EditorMsg::MoveCursor(Direction::Right)),
        );
        if cmd.as_ref().is_some_and(|c| c.needs_redraw()) {
            renderer.render_frame(&model);
        }
    }

    divan::black_box(&model);
}

#[divan::bench(args = [100, 500])]
fn full_loop_insert_char_and_render(iterations: usize) {
    let mut model = make_model(10_000);
    let mut renderer = BenchRenderer::new(1920, 1080, model.line_height);

    for i in 0..iterations {
        let ch = (b'a' + (i % 26) as u8) as char;
        let cmd = update(&mut model, Msg::Document(DocumentMsg::InsertChar(ch)));
        if cmd.as_ref().is_some_and(|c| c.needs_redraw()) {
            renderer.render_frame(&model);
        }
    }

    divan::black_box(&model);
}

#[divan::bench(args = [10, 50])]
fn full_loop_scroll_and_render(iterations: usize) {
    let mut model = make_model(50_000);
    let mut renderer = BenchRenderer::new(1920, 1080, model.line_height);

    for _ in 0..iterations {
        let cmd = update(&mut model, Msg::Editor(EditorMsg::PageDown));
        if cmd.as_ref().is_some_and(|c| c.needs_redraw()) {
            renderer.render_frame(&model);
        }
    }

    divan::black_box(&model);
}

#[divan::bench(args = [(800, 600), (1920, 1080), (2560, 1440)])]
fn full_loop_resize_and_render(size: (u32, u32)) {
    let (w, h) = size;
    let mut model = make_model(50_000);
    let mut renderer = BenchRenderer::new(w as usize, h as usize, model.line_height);

    let cmd = update(&mut model, Msg::App(AppMsg::Resize(w, h)));
    if cmd.as_ref().is_some_and(|c| c.needs_redraw()) {
        renderer.render_frame(&model);
    }

    divan::black_box(&model);
}

// ============================================================================
// Scaling benchmarks: How does performance scale with document size?
// ============================================================================

#[divan::bench(args = [1_000, 10_000, 50_000, 100_000])]
fn scaling_insert_char_by_doc_size(doc_lines: usize) {
    let mut model = make_model(doc_lines);

    for i in 0..100 {
        let ch = (b'a' + (i % 26) as u8) as char;
        let cmd = update(&mut model, Msg::Document(DocumentMsg::InsertChar(ch)));
        divan::black_box(cmd);
    }

    divan::black_box(&model);
}

#[divan::bench(args = [1_000, 10_000, 50_000, 100_000])]
fn scaling_cursor_move_by_doc_size(doc_lines: usize) {
    let mut model = make_model(doc_lines);

    for _ in 0..100 {
        let cmd = update(
            &mut model,
            Msg::Editor(EditorMsg::MoveCursor(Direction::Down)),
        );
        divan::black_box(cmd);
    }

    divan::black_box(&model);
}

#[divan::bench(args = [1_000, 10_000, 50_000, 100_000])]
fn scaling_page_down_by_doc_size(doc_lines: usize) {
    let mut model = make_model(doc_lines);

    for _ in 0..20 {
        let cmd = update(&mut model, Msg::Editor(EditorMsg::PageDown));
        divan::black_box(cmd);
    }

    divan::black_box(&model);
}

// ============================================================================
// Realistic typing simulation
// ============================================================================

#[divan::bench]
fn realistic_typing_paragraph() {
    let mut model = make_model(1_000);
    let mut renderer = BenchRenderer::new(1920, 1080, model.line_height);

    let paragraph = "The quick brown fox jumps over the lazy dog. ";

    for ch in paragraph.chars() {
        let cmd = if ch == ' ' {
            update(&mut model, Msg::Document(DocumentMsg::InsertChar(' ')))
        } else {
            update(&mut model, Msg::Document(DocumentMsg::InsertChar(ch)))
        };

        if cmd.as_ref().is_some_and(|c| c.needs_redraw()) {
            renderer.render_frame(&model);
        }
    }

    divan::black_box(&model);
}

#[divan::bench]
fn realistic_typing_with_newlines() {
    let mut model = make_model(1_000);
    let mut renderer = BenchRenderer::new(1920, 1080, model.line_height);

    // Type 10 lines of text
    for line_num in 0..10 {
        let line = format!("Line {}: The quick brown fox jumps.\n", line_num);
        for ch in line.chars() {
            let cmd = if ch == '\n' {
                update(&mut model, Msg::Document(DocumentMsg::InsertNewline))
            } else {
                update(&mut model, Msg::Document(DocumentMsg::InsertChar(ch)))
            };

            if cmd.as_ref().is_some_and(|c| c.needs_redraw()) {
                renderer.render_frame(&model);
            }
        }
    }

    divan::black_box(&model);
}
