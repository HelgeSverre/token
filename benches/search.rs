//! Benchmarks for search operations
//!
//! Run with: cargo bench search

use ropey::Rope;

#[global_allocator]
static ALLOC: divan::AllocProfiler = divan::AllocProfiler::system();

fn main() {
    divan::main();
}

// ============================================================================
// Literal string search
// ============================================================================

#[divan::bench(args = [1_000, 10_000, 100_000])]
fn search_literal_string(line_count: usize) {
    let rope = Rope::from_str(&"The quick brown fox jumps over the lazy dog.\n".repeat(line_count));
    let needle = "brown";

    let mut matches = Vec::new();
    for (line_idx, line) in rope.lines().enumerate() {
        let line_str = line.to_string();
        if let Some(col) = line_str.find(needle) {
            matches.push((line_idx, col));
        }
    }
    divan::black_box(matches);
}

#[divan::bench(args = [1_000, 10_000, 100_000])]
fn search_literal_all_occurrences(line_count: usize) {
    let rope = Rope::from_str(&"The quick brown fox jumps over the lazy dog.\n".repeat(line_count));
    let needle = "the"; // Case-sensitive, matches "the" in "the lazy"

    let mut matches = Vec::new();
    for (line_idx, line) in rope.lines().enumerate() {
        let line_str = line.to_string();
        let mut start = 0;
        while let Some(pos) = line_str[start..].find(needle) {
            matches.push((line_idx, start + pos));
            start += pos + needle.len();
        }
    }
    divan::black_box(matches);
}

#[divan::bench(args = [1_000, 10_000, 100_000])]
fn search_case_insensitive(line_count: usize) {
    let rope = Rope::from_str(&"The quick brown fox jumps over the lazy dog.\n".repeat(line_count));
    let needle = "the";

    let mut matches = Vec::new();
    for (line_idx, line) in rope.lines().enumerate() {
        let line_str = line.to_string().to_lowercase();
        let mut start = 0;
        while let Some(pos) = line_str[start..].find(needle) {
            matches.push((line_idx, start + pos));
            start += pos + needle.len();
        }
    }
    divan::black_box(matches);
}

// ============================================================================
// Search in different file content patterns
// ============================================================================

#[divan::bench]
fn search_in_code_like_content() {
    let code = r#"
fn main() {
    let x = 42;
    let y = x + 1;
    println!("Result: {}", y);
}
"#
    .repeat(5000);
    let rope = Rope::from_str(&code);
    let needle = "let";

    let mut matches = Vec::new();
    for (line_idx, line) in rope.lines().enumerate() {
        let line_str = line.to_string();
        if let Some(col) = line_str.find(needle) {
            matches.push((line_idx, col));
        }
    }
    divan::black_box(matches);
}

#[divan::bench]
fn search_rare_pattern() {
    let rope = Rope::from_str(&"The quick brown fox jumps over the lazy dog.\n".repeat(50_000));
    let needle = "xyzzyx"; // Not present

    let mut matches = Vec::new();
    for (line_idx, line) in rope.lines().enumerate() {
        let line_str = line.to_string();
        if let Some(col) = line_str.find(needle) {
            matches.push((line_idx, col));
        }
    }
    assert!(matches.is_empty());
    divan::black_box(matches);
}

#[divan::bench]
fn search_common_pattern() {
    let rope = Rope::from_str(&"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\n".repeat(10_000));
    let needle = "aaa";

    let mut matches = Vec::new();
    for (line_idx, line) in rope.lines().enumerate() {
        let line_str = line.to_string();
        let mut start = 0;
        while let Some(pos) = line_str[start..].find(needle) {
            matches.push((line_idx, start + pos));
            start += pos + 1; // Overlapping matches
        }
    }
    divan::black_box(matches);
}

// ============================================================================
// Search and count only (no position tracking)
// ============================================================================

#[divan::bench(args = [10_000, 50_000, 100_000])]
fn count_occurrences(line_count: usize) {
    let rope = Rope::from_str(&"The quick brown fox jumps over the lazy dog.\n".repeat(line_count));
    let needle = "the";

    let mut count = 0;
    for line in rope.lines() {
        let line_str = line.to_string();
        count += line_str.matches(needle).count();
    }
    divan::black_box(count);
}

// ============================================================================
// First match only (early exit)
// ============================================================================

#[divan::bench(args = [10_000, 100_000])]
fn find_first_occurrence(line_count: usize) {
    let rope = Rope::from_str(&"The quick brown fox jumps over the lazy dog.\n".repeat(line_count));
    let needle = "brown";

    let result = rope.lines().enumerate().find_map(|(line_idx, line)| {
        let line_str = line.to_string();
        line_str.find(needle).map(|col| (line_idx, col))
    });
    divan::black_box(result);
}

#[divan::bench(args = [10_000, 100_000])]
fn find_first_in_second_half(line_count: usize) {
    // Pattern only exists in second half
    let first_half = "The quick fox jumps over the lazy dog.\n".repeat(line_count / 2);
    let second_half = "The quick brown fox jumps.\n".repeat(line_count / 2);
    let rope = Rope::from_str(&format!("{}{}", first_half, second_half));
    let needle = "brown";

    let result = rope.lines().enumerate().find_map(|(line_idx, line)| {
        let line_str = line.to_string();
        line_str.find(needle).map(|col| (line_idx, col))
    });
    divan::black_box(result);
}

// ============================================================================
// Visible range search (simulating viewport-limited search)
// ============================================================================

#[divan::bench(args = [50, 100, 200])]
fn search_visible_range(visible_lines: usize) {
    let rope = Rope::from_str(&"The quick brown fox jumps over the lazy dog.\n".repeat(100_000));
    let needle = "fox";
    let start_line = 50_000; // Middle of document

    let mut matches = Vec::new();
    for line_offset in 0..visible_lines {
        let line_idx = start_line + line_offset;
        if line_idx >= rope.len_lines() {
            break;
        }
        let line = rope.line(line_idx);
        let line_str = line.to_string();
        if let Some(col) = line_str.find(needle) {
            matches.push((line_idx, col));
        }
    }
    divan::black_box(matches);
}

// ============================================================================
// Word boundary search (simulating "whole word" search)
// ============================================================================

fn is_word_boundary(s: &str, pos: usize) -> bool {
    if pos == 0 || pos >= s.len() {
        return true;
    }
    let prev = s.chars().nth(pos.saturating_sub(1));
    let curr = s.chars().nth(pos);
    match (prev, curr) {
        (Some(p), Some(c)) => !p.is_alphanumeric() || !c.is_alphanumeric(),
        _ => true,
    }
}

#[divan::bench(args = [10_000, 50_000])]
fn search_whole_word(line_count: usize) {
    let rope = Rope::from_str(&"The quick brown fox jumps over the lazy dog.\n".repeat(line_count));
    let needle = "the";

    let mut matches = Vec::new();
    for (line_idx, line) in rope.lines().enumerate() {
        let line_str = line.to_string();
        let mut start = 0;
        while let Some(pos) = line_str[start..].find(needle) {
            let abs_pos = start + pos;
            let end_pos = abs_pos + needle.len();
            if is_word_boundary(&line_str, abs_pos) && is_word_boundary(&line_str, end_pos) {
                matches.push((line_idx, abs_pos));
            }
            start = abs_pos + needle.len();
        }
    }
    divan::black_box(matches);
}
