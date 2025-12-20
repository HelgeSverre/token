//! Benchmarks for identified performance bottlenecks
//!
//! These benchmarks target the specific hot paths identified in performance analysis:
//! - word_start_before / word_end_after (document.rs)
//! - move_word_left / move_word_right (editable/state.rs)
//! - RopeBuffer::line() double allocation
//! - last_non_whitespace_column / first_non_whitespace_column
//!
//! Run with: cargo bench hot_paths

use ropey::Rope;
use std::borrow::Cow;

#[global_allocator]
static ALLOC: divan::AllocProfiler = divan::AllocProfiler::system();

fn main() {
    divan::main();
}

// ============================================================================
// Utility: CharType for word navigation (copied from src/util.rs)
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CharType {
    Whitespace,
    Punctuation,
    WordChar,
}

fn char_type(c: char) -> CharType {
    if c.is_whitespace() {
        CharType::Whitespace
    } else if c.is_alphanumeric() || c == '_' {
        CharType::WordChar
    } else {
        CharType::Punctuation
    }
}

// ============================================================================
// CURRENT IMPLEMENTATION: word_start_before (allocating)
// ============================================================================

fn word_start_before_current(buffer: &Rope, offset: usize) -> usize {
    if offset == 0 {
        return 0;
    }

    // BOTTLENECK: Allocates ENTIRE prefix as String
    let text: String = buffer.slice(..offset).chars().collect();
    let chars: Vec<char> = text.chars().collect();
    let mut i = chars.len();

    if i == 0 {
        return 0;
    }

    let current_type = char_type(chars[i - 1]);
    while i > 0 && char_type(chars[i - 1]) == current_type {
        i -= 1;
    }

    i
}

// ============================================================================
// OPTIMIZED IMPLEMENTATION: word_start_before (using chars_at for reverse)
// ============================================================================

fn word_start_before_optimized(buffer: &Rope, offset: usize) -> usize {
    if offset == 0 {
        return 0;
    }

    // Use chars_at to get an iterator at a specific position
    // Then iterate backwards by decrementing position
    let mut pos = offset;

    // Get the character just before offset
    let first_char = buffer.char(pos - 1);
    let current_type = char_type(first_char);
    pos -= 1;

    // Continue backwards while same char type
    while pos > 0 {
        let ch = buffer.char(pos - 1);
        if char_type(ch) != current_type {
            break;
        }
        pos -= 1;
    }

    pos
}

// ============================================================================
// CURRENT IMPLEMENTATION: word_end_after (allocating)
// ============================================================================

fn word_end_after_current(buffer: &Rope, offset: usize) -> usize {
    let len = buffer.len_chars();
    if offset >= len {
        return len;
    }

    // BOTTLENECK: Allocates ENTIRE suffix as String
    let text: String = buffer.slice(offset..).chars().collect();
    let chars: Vec<char> = text.chars().collect();

    if chars.is_empty() {
        return offset;
    }

    let mut i = 0;
    let current_type = char_type(chars[0]);
    while i < chars.len() && char_type(chars[i]) == current_type {
        i += 1;
    }

    offset + i
}

// ============================================================================
// OPTIMIZED IMPLEMENTATION: word_end_after (iterator-based)
// ============================================================================

fn word_end_after_optimized(buffer: &Rope, offset: usize) -> usize {
    let len = buffer.len_chars();
    if offset >= len {
        return len;
    }

    let slice = buffer.slice(offset..);
    let mut chars_iter = slice.chars();

    let first_char = match chars_iter.next() {
        Some(c) => c,
        None => return offset,
    };

    let current_type = char_type(first_char);
    let mut count = 1;

    for ch in chars_iter {
        if char_type(ch) != current_type {
            break;
        }
        count += 1;
    }

    offset + count
}

// ============================================================================
// Benchmarks: word_start_before
// ============================================================================

#[divan::bench(args = [1_000, 10_000, 100_000])]
fn word_start_before_current_impl(line_count: usize) {
    let rope = Rope::from_str(&"The quick brown fox jumps over the lazy dog.\n".repeat(line_count));
    let offset = rope.len_chars() / 2; // Middle of document

    let result = word_start_before_current(&rope, offset);
    divan::black_box(result);
}

#[divan::bench(args = [1_000, 10_000, 100_000])]
fn word_start_before_optimized_impl(line_count: usize) {
    let rope = Rope::from_str(&"The quick brown fox jumps over the lazy dog.\n".repeat(line_count));
    let offset = rope.len_chars() / 2; // Middle of document

    let result = word_start_before_optimized(&rope, offset);
    divan::black_box(result);
}

#[divan::bench(args = [1_000, 10_000, 100_000])]
fn word_end_after_current_impl(line_count: usize) {
    let rope = Rope::from_str(&"The quick brown fox jumps over the lazy dog.\n".repeat(line_count));
    let offset = rope.len_chars() / 2; // Middle of document

    let result = word_end_after_current(&rope, offset);
    divan::black_box(result);
}

#[divan::bench(args = [1_000, 10_000, 100_000])]
fn word_end_after_optimized_impl(line_count: usize) {
    let rope = Rope::from_str(&"The quick brown fox jumps over the lazy dog.\n".repeat(line_count));
    let offset = rope.len_chars() / 2; // Middle of document

    let result = word_end_after_optimized(&rope, offset);
    divan::black_box(result);
}

// ============================================================================
// Benchmarks: Multiple word navigation operations (realistic typing pattern)
// ============================================================================

#[divan::bench]
fn word_navigation_sequence_current() {
    let rope = Rope::from_str(&"The quick brown fox jumps over the lazy dog.\n".repeat(10_000));
    let mut offset = rope.len_chars() / 2;

    // Simulate 50 word-left + 50 word-right operations
    for _ in 0..50 {
        offset = word_start_before_current(&rope, offset);
    }
    for _ in 0..50 {
        offset = word_end_after_current(&rope, offset);
    }

    divan::black_box(offset);
}

#[divan::bench]
fn word_navigation_sequence_optimized() {
    let rope = Rope::from_str(&"The quick brown fox jumps over the lazy dog.\n".repeat(10_000));
    let mut offset = rope.len_chars() / 2;

    // Simulate 50 word-left + 50 word-right operations
    for _ in 0..50 {
        offset = word_start_before_optimized(&rope, offset);
    }
    for _ in 0..50 {
        offset = word_end_after_optimized(&rope, offset);
    }

    divan::black_box(offset);
}

// ============================================================================
// CURRENT IMPLEMENTATION: RopeBuffer::line() (double allocation)
// ============================================================================

fn rope_buffer_line_current(rope: &Rope, line: usize) -> Option<Cow<'_, str>> {
    if line >= rope.len_lines() {
        return None;
    }
    let line_slice = rope.line(line);
    let s = line_slice.to_string(); // ALLOCATION 1
    let trimmed = s.trim_end_matches(&['\n', '\r'][..]).to_string(); // ALLOCATION 2
    Some(Cow::Owned(trimmed))
}

// ============================================================================
// OPTIMIZED IMPLEMENTATION: RopeBuffer::line() (single allocation with check)
// ============================================================================

fn rope_buffer_line_optimized(rope: &Rope, line: usize) -> Option<Cow<'_, str>> {
    if line >= rope.len_lines() {
        return None;
    }
    let line_slice = rope.line(line);
    let len = line_slice.len_chars();

    // Calculate trim length
    let trim_len = if len > 0 && line_slice.char(len - 1) == '\n' {
        if len > 1 && line_slice.char(len - 2) == '\r' {
            2
        } else {
            1
        }
    } else {
        0
    };

    if trim_len > 0 {
        Some(Cow::Owned(line_slice.slice(..len - trim_len).to_string()))
    } else {
        Some(Cow::Owned(line_slice.to_string()))
    }
}

// ============================================================================
// Benchmarks: RopeBuffer::line()
// ============================================================================

#[divan::bench(args = [100, 1000, 10000])]
fn rope_buffer_line_access_current(iterations: usize) {
    let rope = Rope::from_str(&"The quick brown fox jumps over the lazy dog.\n".repeat(10_000));

    for i in 0..iterations {
        let line_idx = i % rope.len_lines();
        let result = rope_buffer_line_current(&rope, line_idx);
        divan::black_box(result);
    }
}

#[divan::bench(args = [100, 1000, 10000])]
fn rope_buffer_line_access_optimized(iterations: usize) {
    let rope = Rope::from_str(&"The quick brown fox jumps over the lazy dog.\n".repeat(10_000));

    for i in 0..iterations {
        let line_idx = i % rope.len_lines();
        let result = rope_buffer_line_optimized(&rope, line_idx);
        divan::black_box(result);
    }
}

// ============================================================================
// CURRENT IMPLEMENTATION: last_non_whitespace_column (allocating)
// ============================================================================

fn last_non_whitespace_column_current(rope: &Rope, line: usize) -> usize {
    if line >= rope.len_lines() {
        return 0;
    }
    let line_slice = rope.line(line);
    let s: String = line_slice.chars().collect(); // UNNECESSARY ALLOCATION
    let trimmed = s.trim_end_matches(|c: char| c.is_whitespace());
    trimmed.chars().count()
}

// ============================================================================
// OPTIMIZED IMPLEMENTATION: last_non_whitespace_column (index-based reverse)
// ============================================================================

fn last_non_whitespace_column_optimized(rope: &Rope, line: usize) -> usize {
    if line >= rope.len_lines() {
        return 0;
    }
    let line_slice = rope.line(line);
    let total = line_slice.len_chars();

    // Count trailing whitespace by indexing from end
    let mut trailing_ws = 0;
    let mut pos = total;
    while pos > 0 {
        let ch = line_slice.char(pos - 1);
        if !ch.is_whitespace() {
            break;
        }
        trailing_ws += 1;
        pos -= 1;
    }

    total.saturating_sub(trailing_ws)
}

// ============================================================================
// CURRENT IMPLEMENTATION: first_non_whitespace_column
// ============================================================================

fn first_non_whitespace_column_current(rope: &Rope, line: usize) -> usize {
    if line >= rope.len_lines() {
        return 0;
    }
    let line_slice = rope.line(line);
    line_slice
        .chars()
        .take_while(|c| c.is_whitespace() && *c != '\n')
        .count()
}

// ============================================================================
// Benchmarks: whitespace column functions
// ============================================================================

#[divan::bench(args = [100, 1000, 10000])]
fn last_non_whitespace_column_current_impl(iterations: usize) {
    let rope = Rope::from_str(&"    The quick brown fox jumps.    \n".repeat(10_000));

    for i in 0..iterations {
        let line_idx = i % rope.len_lines();
        let result = last_non_whitespace_column_current(&rope, line_idx);
        divan::black_box(result);
    }
}

#[divan::bench(args = [100, 1000, 10000])]
fn last_non_whitespace_column_optimized_impl(iterations: usize) {
    let rope = Rope::from_str(&"    The quick brown fox jumps.    \n".repeat(10_000));

    for i in 0..iterations {
        let line_idx = i % rope.len_lines();
        let result = last_non_whitespace_column_optimized(&rope, line_idx);
        divan::black_box(result);
    }
}

#[divan::bench(args = [100, 1000, 10000])]
fn first_non_whitespace_column_impl(iterations: usize) {
    let rope = Rope::from_str(&"    The quick brown fox jumps.    \n".repeat(10_000));

    for i in 0..iterations {
        let line_idx = i % rope.len_lines();
        let result = first_non_whitespace_column_current(&rope, line_idx);
        divan::black_box(result);
    }
}

// ============================================================================
// move_word_left / move_word_right simulation
// ============================================================================

fn move_word_left_current(rope: &Rope, line: usize, column: usize) -> (usize, usize) {
    if line >= rope.len_lines() {
        return (line, column);
    }

    // CURRENT: Gets line content as String, then collects to Vec<char>
    let line_content: Cow<'_, str> = rope_buffer_line_current(rope, line).unwrap_or_default();
    let chars: Vec<char> = line_content.chars().collect();

    let mut pos = column.min(chars.len());

    // Skip whitespace/punctuation backwards
    while pos > 0 {
        let ct = char_type(chars[pos - 1]);
        if ct == CharType::WordChar {
            break;
        }
        pos -= 1;
    }

    // Skip word characters
    while pos > 0 {
        let ct = char_type(chars[pos - 1]);
        if ct != CharType::WordChar {
            break;
        }
        pos -= 1;
    }

    (line, pos)
}

fn move_word_left_optimized(rope: &Rope, line: usize, column: usize) -> (usize, usize) {
    if line >= rope.len_lines() {
        return (line, column);
    }

    let line_slice = rope.line(line);
    let line_len = line_slice.len_chars();
    let line_len_no_newline = if line_len > 0 && line_slice.char(line_len - 1) == '\n' {
        line_len - 1
    } else {
        line_len
    };

    let mut pos = column.min(line_len_no_newline);
    if pos == 0 {
        return (line, 0);
    }

    // Use index-based reverse iteration - no allocation
    let mut in_word = false;

    while pos > 0 {
        let ch = line_slice.char(pos - 1);
        let ct = char_type(ch);
        if !in_word {
            if ct == CharType::WordChar {
                in_word = true;
            }
            pos -= 1;
        } else {
            if ct != CharType::WordChar {
                break;
            }
            pos -= 1;
        }
    }

    (line, pos)
}

#[divan::bench(args = [100, 500, 1000])]
fn move_word_left_current_impl(iterations: usize) {
    let rope = Rope::from_str(&"The quick brown fox jumps over the lazy dog.\n".repeat(10_000));

    for i in 0..iterations {
        let line = i % rope.len_lines();
        let column = 20; // Middle of line
        let result = move_word_left_current(&rope, line, column);
        divan::black_box(result);
    }
}

#[divan::bench(args = [100, 500, 1000])]
fn move_word_left_optimized_impl(iterations: usize) {
    let rope = Rope::from_str(&"The quick brown fox jumps over the lazy dog.\n".repeat(10_000));

    for i in 0..iterations {
        let line = i % rope.len_lines();
        let column = 20; // Middle of line
        let result = move_word_left_optimized(&rope, line, column);
        divan::black_box(result);
    }
}

// ============================================================================
// Combined realistic typing benchmark (like realistic_typing_with_newlines)
// ============================================================================

#[divan::bench]
fn realistic_word_operations_current() {
    let rope = Rope::from_str(&"The quick brown fox jumps over the lazy dog.\n".repeat(1_000));

    // Simulate editing: word navigation + word deletion patterns
    for line in 0..10 {
        let line_idx = line * 100;
        if line_idx >= rope.len_lines() {
            break;
        }

        // Navigate by word
        let (_, col) = move_word_left_current(&rope, line_idx, 30);
        divan::black_box(col);

        // Get line content
        let content = rope_buffer_line_current(&rope, line_idx);
        divan::black_box(&content);

        // Find word boundaries
        let offset = rope.line_to_char(line_idx) + 20;
        let start = word_start_before_current(&rope, offset);
        let end = word_end_after_current(&rope, offset);
        divan::black_box((start, end));

        // Whitespace operations
        let last_ws = last_non_whitespace_column_current(&rope, line_idx);
        divan::black_box(last_ws);
    }
}

#[divan::bench]
fn realistic_word_operations_optimized() {
    let rope = Rope::from_str(&"The quick brown fox jumps over the lazy dog.\n".repeat(1_000));

    // Same operations but optimized
    for line in 0..10 {
        let line_idx = line * 100;
        if line_idx >= rope.len_lines() {
            break;
        }

        // Navigate by word
        let (_, col) = move_word_left_optimized(&rope, line_idx, 30);
        divan::black_box(col);

        // Get line content
        let content = rope_buffer_line_optimized(&rope, line_idx);
        divan::black_box(&content);

        // Find word boundaries
        let offset = rope.line_to_char(line_idx) + 20;
        let start = word_start_before_optimized(&rope, offset);
        let end = word_end_after_optimized(&rope, offset);
        divan::black_box((start, end));

        // Whitespace operations
        let last_ws = last_non_whitespace_column_optimized(&rope, line_idx);
        divan::black_box(last_ws);
    }
}

// ============================================================================
// Allocation count verification benchmarks
// ============================================================================

#[divan::bench]
fn allocation_heavy_word_delete_current() {
    let rope = Rope::from_str(&"The quick brown fox jumps over the lazy dog.\n".repeat(10_000));

    // Simulate 100 word delete operations (Ctrl+Backspace)
    for i in 0..100 {
        let offset = (i * 1000) % rope.len_chars();
        let start = word_start_before_current(&rope, offset);
        divan::black_box(start);
    }
}

#[divan::bench]
fn allocation_heavy_word_delete_optimized() {
    let rope = Rope::from_str(&"The quick brown fox jumps over the lazy dog.\n".repeat(10_000));

    // Simulate 100 word delete operations (Ctrl+Backspace)
    for i in 0..100 {
        let offset = (i * 1000) % rope.len_chars();
        let start = word_start_before_optimized(&rope, offset);
        divan::black_box(start);
    }
}
