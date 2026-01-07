# Performance Analysis Report

## Executive Summary

Performance benchmarks revealed several bottlenecks in hot paths, particularly word navigation operations. After optimization, we achieved **~6x to ~460x speedup** in word-related operations by eliminating unnecessary heap allocations.

### Key Improvements (After Optimization)

| Operation | Before | After | Speedup | Memory Reduction |
|-----------|--------|-------|---------|------------------|
| `word_navigation_sequence` (50+50 ops) | 67ms | 155µs | **~430x** | 46MB → 966KB |
| `word_start_before` (100K lines) | 8.8ms | 1.5ms | **~6x** | 14.1MB → 9.6MB |
| `word_end_after` (100K lines) | 8.9ms | 1.5ms | **~6x** | 14.1MB → 9.6MB |
| `realistic_word_operations` | 1.35ms | 25µs | **~54x** | 1MB → 98KB |
| `move_word_left` (1000 ops) | 451µs | 396µs | ~14% | 3544 allocs → 502 allocs |

The main issues were excessive heap allocations from collecting entire document prefixes/suffixes into String/Vec just to iterate characters.

## Benchmark Results (Key Findings)

| Benchmark | Time (median) | Allocations | Notes |
|-----------|--------------|-------------|-------|
| realistic_typing_paragraph | 109ms | 1,133 | Typing 45 chars + render |
| realistic_typing_with_newlines | 845ms | 2,973 | 10 lines, major bottleneck |
| update_insert_char (500) | 1.6ms | 4,318 | ~3.2µs per char |
| update_delete_backward (500) | 3.0ms | 7,357 | More expensive than insert |
| multi_cursor_insert_char (500 cursors) | 1.0ms | 3,250 | Scales poorly |

## Identified Bottlenecks

### 1. String Allocations in Word Navigation (Critical)

**Location:** `src/update/document.rs` lines 25-72

```rust
// word_start_before - allocates ENTIRE prefix as String
fn word_start_before(buffer: &ropey::Rope, offset: usize) -> usize {
    let text: String = buffer.slice(..offset).chars().collect(); // ALLOCATES
    let chars: Vec<char> = text.chars().collect();               // ALLOCATES AGAIN
    // ...
}

// word_end_after - allocates ENTIRE suffix as String  
fn word_end_after(buffer: &ropey::Rope, offset: usize) -> usize {
    let text: String = buffer.slice(offset..).chars().collect(); // ALLOCATES
    let chars: Vec<char> = text.chars().collect();               // ALLOCATES AGAIN
    // ...
}
```

**Impact:** For a 100K line file, `word_start_before` at position 50K allocates ~2.5MB just to find the word boundary.

**Fix:** Use ropey's `Chars` iterator directly with `.rev()`:
```rust
fn word_start_before(buffer: &ropey::Rope, offset: usize) -> usize {
    if offset == 0 { return 0; }
    
    let mut pos = offset;
    let current_type = char_type(buffer.char(offset - 1));
    
    for ch in buffer.slice(..offset).chars().rev() {
        if char_type(ch) != current_type {
            break;
        }
        pos -= 1;
    }
    pos
}
```

### 2. RopeBuffer::line() Double Allocation

**Location:** `src/editable/buffer.rs` lines 296-305

```rust
fn line(&self, line: usize) -> Option<Cow<'_, str>> {
    let line_slice = self.rope.line(line);
    let s = line_slice.to_string();                           // ALLOCATION 1
    let trimmed = s.trim_end_matches(&['\n', '\r'][..]).to_string(); // ALLOCATION 2
    Some(Cow::Owned(trimmed))
}
```

**Impact:** Every line access allocates twice. Called frequently during rendering and word navigation.

**Fix:** Check if trimming is needed and avoid second allocation:
```rust
fn line(&self, line: usize) -> Option<Cow<'_, str>> {
    let line_slice = self.rope.line(line);
    let s = line_slice.to_string();
    if s.ends_with('\n') || s.ends_with('\r') {
        Some(Cow::Owned(s.trim_end_matches(&['\n', '\r'][..]).to_string()))
    } else {
        Some(Cow::Owned(s))
    }
}
```

Or better, use ropey's slice and iterate:
```rust
fn line(&self, line: usize) -> Option<Cow<'_, str>> {
    let line_slice = self.rope.line(line);
    let len = line_slice.len_chars();
    let trim_len = if len > 0 && line_slice.char(len - 1) == '\n' {
        if len > 1 && line_slice.char(len - 2) == '\r' { 2 } else { 1 }
    } else { 0 };
    Some(Cow::Owned(line_slice.slice(..len - trim_len).to_string()))
}
```

### 3. EditableState Word Movement Allocations

**Location:** `src/editable/state.rs` lines 357-390 and 424-451

```rust
// move_word_left
let line_content = self.buffer.line(self.cursors[idx].line).unwrap_or_default();
let chars: Vec<char> = line_content.chars().collect();  // ALLOCATES

// move_word_right  
let line_content = self.buffer.line(self.cursors[idx].line).unwrap_or_default();
let chars: Vec<char> = line_content.chars().collect();  // ALLOCATES
```

**Impact:** Word movement allocates entire line content just to iterate characters.

**Fix:** Add a `chars_at_line()` iterator method to TextBuffer trait.

### 4. last_non_whitespace_column String Collection

**Location:** `src/editable/buffer.rs` line 351

```rust
fn last_non_whitespace_column(&self, line: usize) -> usize {
    let line_slice = self.rope.line(line);
    let s: String = line_slice.chars().collect();  // UNNECESSARY ALLOCATION
    let trimmed = s.trim_end_matches(|c: char| c.is_whitespace());
    trimmed.chars().count()
}
```

**Fix:** Iterate in reverse without allocating:
```rust
fn last_non_whitespace_column(&self, line: usize) -> usize {
    let line_slice = self.rope.line(line);
    let total = line_slice.len_chars();
    let trailing_ws = line_slice.chars().rev()
        .take_while(|c| c.is_whitespace())
        .count();
    total.saturating_sub(trailing_ws)
}
```

### 5. Rendering Performance

**Location:** `render_visible_lines` benchmark

| Lines | Time |
|-------|------|
| 25 | 693µs |
| 50 | 1.4ms |
| 100 | 2.2ms |

This scales linearly which is expected, but rendering 100 lines takes ~2.2ms which limits us to ~450 FPS theoretical max. Consider:
- Dirty rect tracking to skip unchanged regions
- Glyph cache optimization for repeated characters

### 6. Multi-Split Rendering Allocations (Fixed)

**Problem:** With 3+ splits open (especially with large CSV + code files), FPS dropped from 60 to ~7fps.

**Root Cause:** Per-line allocations in the text rendering loop, multiplied by number of splits:
- `document.get_line()` allocates a String for each line
- `expand_tabs_for_display()` allocates a new String even when no tabs present
- `display_text` creates a new String via `.collect()` for each line
- `adjusted_tokens` creates a new Vec for each line

**Impact:** For 3 splits × 35 lines each × 4 allocations = **420+ allocations per frame** just for text rendering.

**Fixes Applied:**
1. Added `Document::get_line_cow()` that returns `Cow<str>` - zero allocation when line is contiguous in rope
2. Changed `expand_tabs_for_display()` to return `Cow<str>` - no allocation when no tabs in line
3. Reuse `display_text_buf` String buffer across all lines (clear + append vs allocate)
4. Reuse `adjusted_tokens` Vec buffer across all lines (clear + push vs allocate)
5. Applied same `get_line_cow()` optimization to selection highlighting and cursor rendering

**Expected Improvement:** ~3-5x reduction in per-frame allocations for multi-split scenarios.

### 7. Headless vs Actual App Performance Gap (Resolved)

**Observation:** The `profile_render` binary shows ~380 FPS (2.6ms/frame) but the actual app showed ~7 FPS in multi-split scenarios with large files.

**Root Cause Analysis:**

The headless profiler only measures CPU rendering time. The actual app's frame time was affected by:

1. **`ControlFlow::Poll`** - The event loop was spinning continuously, calling `about_to_wait` on every iteration
2. **Constant redraws** - Even when nothing changed, the app was constantly re-rendering
3. **`buffer.present()` with VSync** - Each present() call blocks up to 16.67ms when VSync is enabled
4. **CPU saturation** - The constant polling consumed CPU cycles that could be used for rendering

**Fix Applied:**

Changed `ControlFlow::Poll` to `ControlFlow::WaitUntil(next_blink)` in [`src/runtime/app.rs`](file:///Users/helge/code/token-editor/src/runtime/app.rs#L1430-L1439):

- Event loop now sleeps until the next cursor blink timer (500ms) instead of spinning
- Redraws only occur when: user input, async messages, file system changes, or cursor blink timer
- Reduces CPU usage from ~100% idle to near 0% idle
- Allows the system to properly schedule VSync without blocking

**Expected Improvement:**

- Idle CPU usage: ~100% → ~0%
- Responsive FPS when editing: should track VSync at 60 FPS
- No more spinning when the app is idle

## Optimization Status

All high-priority optimizations have been implemented:

### Phase 1: Word Navigation (Completed)
1. ✅ **DONE:** Fix `word_start_before` / `word_end_after` to use direct character indexing
2. ✅ **DONE:** Fix `move_word_left`/`move_word_right` to use `char_at()` instead of `Vec<char>`
3. ✅ **DONE:** Fix `last_non_whitespace_column` to iterate in reverse without allocation
4. ✅ **DONE:** Optimize `RopeBuffer::line()` to avoid double allocation

### Phase 2: Multi-Split Rendering (Completed)
5. ✅ **DONE:** Add `Document::get_line_cow()` for zero-allocation line access
6. ✅ **DONE:** Optimize `expand_tabs_for_display()` to return `Cow<str>` (no alloc when no tabs)
7. ✅ **DONE:** Reuse `display_text_buf` String across lines (buffer reuse)
8. ✅ **DONE:** Reuse `adjusted_tokens` Vec across lines (buffer reuse)
9. ✅ **DONE:** Use `get_line_cow()` in selection highlighting, cursor rendering

### Phase 3: Event Loop Optimization (Completed)
10. ✅ **DONE:** Changed `ControlFlow::Poll` to `ControlFlow::WaitUntil` for cursor blink timer
11. ✅ **DONE:** Event loop now sleeps when idle, only wakes for input/timers/async messages

### Phase 4: Future Optimizations (TODO)
12. **TODO (Low Priority):** Add dirty rect tracking to rendering
13. **TODO:** Investigate glyph rasterization caching across frames
14. **TODO:** Consider GPU-accelerated rendering for complex scenes

## Profiling Commands

```bash
# Run benchmarks
cargo bench main_loop
cargo bench rendering
cargo bench hot_paths      # Benchmarks for word navigation hot paths

# Profile headless rendering (multi-split scenario)
cargo build --profile profiling --bin profile_render
./target/profiling/profile_render --frames 500 --splits 3 --lines 5000 --stats

# Profile with samply (headless - for render logic only)
samply record ./target/profiling/profile_render --frames 1000 --splits 3 --lines 10000

# Profile actual application with samply
cargo build --profile profiling
samply record ./target/profiling/token samples/large_data.csv

# Generate flamegraph
cargo flamegraph --profile profiling -- samples/large_file.txt
```

### profile_render Options

The `profile_render` binary simulates multi-split rendering without a window:

```bash
./target/profiling/profile_render --help

Options:
  --frames <N>       Number of frames to render (default: 500)
  --splits <N>       Number of editor splits (default: 3)
  --lines <N>        Lines per document (default: 10000)
  --files <paths>    Load specific files instead of generating content
  --include-csv      Include a CSV file in the splits
  --width/--height   Window dimensions (default: 1920x1080)
  --scroll           Simulate scrolling during render
  --stats            Print detailed timing statistics
```

### Comparing Current vs Optimized Implementations

The `benches/hot_paths.rs` file contains both current (allocating) and optimized (iterator-based) implementations side-by-side for comparison:

```bash
# Compare word navigation implementations
cargo bench hot_paths -- "word_start"
cargo bench hot_paths -- "word_navigation"
cargo bench hot_paths -- "realistic"
```

## Memory Allocation Patterns

From divan's allocation profiler, the `realistic_typing_with_newlines` benchmark shows:
- 2,973 allocations for 10 lines of text
- ~297 allocations per line typed
- Peak allocation: 8.4MB (mostly from document buffer growth)

Target: Reduce to <50 allocations per edit operation by eliminating unnecessary String/Vec collections.
