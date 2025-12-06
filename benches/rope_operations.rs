//! Benchmarks for rope/text buffer operations
//!
//! Run with: cargo bench rope_operations

use ropey::Rope;

#[global_allocator]
static ALLOC: divan::AllocProfiler = divan::AllocProfiler::system();

fn main() {
    divan::main();
}

// ============================================================================
// Insert operations
// ============================================================================

#[divan::bench]
fn insert_middle_10k_lines() {
    let mut rope = Rope::from_str(&"foo bar baz\n".repeat(10_000));
    let pos = rope.len_chars() / 2;
    rope.insert(pos, divan::black_box("inserted text\n"));
}

#[divan::bench]
fn insert_start_10k_lines() {
    let mut rope = Rope::from_str(&"foo bar baz\n".repeat(10_000));
    rope.insert(0, divan::black_box("inserted text\n"));
}

#[divan::bench]
fn insert_end_10k_lines() {
    let mut rope = Rope::from_str(&"foo bar baz\n".repeat(10_000));
    let pos = rope.len_chars();
    rope.insert(pos, divan::black_box("inserted text\n"));
}

// ============================================================================
// Delete operations
// ============================================================================

#[divan::bench]
fn delete_middle_10k_lines() {
    let mut rope = Rope::from_str(&"foo bar baz\n".repeat(10_000));
    let start = rope.len_chars() / 2;
    let end = start + 100;
    rope.remove(start..end);
}

#[divan::bench]
fn delete_line_middle() {
    let mut rope = Rope::from_str(&"foo bar baz\n".repeat(10_000));
    let line = 5000;
    let start = rope.line_to_char(line);
    let end = rope.line_to_char(line + 1);
    rope.remove(start..end);
}

// ============================================================================
// Navigation operations (cursor positioning)
// ============================================================================

#[divan::bench(args = [100, 1000, 5000, 9999])]
fn line_to_char(line: usize) {
    let rope = Rope::from_str(&"test line\n".repeat(10_000));
    divan::black_box(rope.line_to_char(line));
}

#[divan::bench(args = [1000, 50000, 99990])]
fn char_to_line(offset: usize) {
    let rope = Rope::from_str(&"test line\n".repeat(10_000));
    divan::black_box(rope.char_to_line(offset));
}

#[divan::bench(args = [100, 1000, 10000])]
fn line_iteration(n: usize) {
    let rope = Rope::from_str(&"test line\n".repeat(n));
    for line in rope.lines() {
        divan::black_box(line);
    }
}

// ============================================================================
// Line access patterns
// ============================================================================

#[divan::bench]
fn get_line_content_middle() {
    let rope = Rope::from_str(&"test line with some content\n".repeat(10_000));
    let line = rope.line(5000);
    divan::black_box(line.to_string());
}

#[divan::bench]
fn get_line_length_middle() {
    let rope = Rope::from_str(&"test line with some content\n".repeat(10_000));
    let line = rope.line(5000);
    divan::black_box(line.len_chars());
}

#[divan::bench(args = [10, 50, 100])]
fn visible_lines_iteration(visible_count: usize) {
    let rope = Rope::from_str(&"test line with some content for display\n".repeat(10_000));
    let start_line = 5000;

    for i in 0..visible_count {
        let line = rope.line(start_line + i);
        divan::black_box(line);
    }
}

// ============================================================================
// Word navigation helpers
// ============================================================================

#[divan::bench]
fn chars_iteration_line() {
    let rope = Rope::from_str(&"The quick brown fox jumps over the lazy dog.\n".repeat(1000));
    let line = rope.line(500);
    for ch in line.chars() {
        divan::black_box(ch);
    }
}

#[divan::bench]
fn slice_to_string() {
    let rope = Rope::from_str(&"The quick brown fox jumps over the lazy dog.\n".repeat(10_000));
    let start = rope.line_to_char(5000);
    let end = start + 45;
    let slice = rope.slice(start..end);
    divan::black_box(slice.to_string());
}
