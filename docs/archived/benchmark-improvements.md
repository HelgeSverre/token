# Benchmark Suite Improvements

**Status:** Completed (Phases 1, 2, 4)  
**Created:** 2025-12-15  
**Completed:** 2025-12-15  
**Effort:** M (3-5 days)

---

## Overview

Audit and improvement plan for the benchmark suite in `benches/`. The current suite has partial coverage with some meaningful benchmarks but also includes tests that don't reflect actual code paths.

### Goals

1. **Accuracy** — Benchmarks should test actual code paths, not fictional patterns
2. **Coverage** — Add missing critical-path benchmarks (syntax highlighting, large files, multi-cursor)
3. **Industry alignment** — Match benchmarking practices from Zed, xi-editor, Lapce
4. **Regression detection** — Enable CI integration and historical tracking

### Non-Goals (This Phase)

- GPU rendering benchmarks (we do CPU software rendering)
- Network/LSP latency benchmarks (future feature)
- End-to-end application benchmarks (use profiling tools instead)

---

## Current State Assessment

### Benchmark Files

| File | Purpose | Value | Issues |
|------|---------|-------|--------|
| `glyph_cache.rs` | HashMap lookup patterns, bitmap allocation | ⚠️ Low | Tests fictional patterns; `glyph_bitmap_alloc` doesn't match actual code |
| `main_loop.rs` | Msg → Update → Cmd cycle | ✅ High | Good coverage; missing multi-cursor |
| `rendering.rs` | Buffer clearing, alpha blending, line rendering | ⚠️ Medium | Uses duplicated blend function, not actual renderer |
| `rope_operations.rs` | Ropey insert/delete/navigation | ✅ High | Solid coverage |
| `support.rs` | Test fixtures (`make_model`, `BenchRenderer`) | ✅ Useful | `BenchRenderer` should use real renderer code |

### Critical Issues

#### 1. `glyph_bitmap_alloc` — Testing Non-Existent Code

```rust
// BENCHMARK (fictional):
fn glyph_bitmap_alloc(font_size: usize) {
    let bitmap_size = font_size * font_size;  // ❌ Not how it works
    vec![0u8; bitmap_size];
}

// ACTUAL CODE in src/view/frame.rs:
let (metrics, bitmap) = self.font.rasterize(ch, self.font_size);  // ✅ Variable size
```

The benchmark tests `vec![0u8; font_size²]` but real code calls `fontdue::Font::rasterize()` which returns variable-sized bitmaps based on actual glyph metrics.

#### 2. Duplicated Rendering Logic

`rendering.rs` and `support.rs` both define `blend_pixel()` independently:

```rust
// In rendering.rs (lines 42-59)
fn blend_pixel(bg: u32, fg: u32, alpha: u8) -> u32 { ... }

// In support.rs (lines 67-84)  
fn blend_pixel(bg: u32, fg: u32, alpha: u8) -> u32 { ... }
```

If the actual `TextPainter::draw()` in `src/view/frame.rs` is optimized, these benchmarks won't reflect it.

#### 3. Missing Critical Benchmarks

| Category | What's Missing | Why It Matters |
|----------|---------------|----------------|
| **Syntax highlighting** | No tree-sitter parsing benchmarks | Will be main CPU bottleneck after implementation |
| **Multi-cursor** | No 100+ cursor operations | Common performance edge case |
| **Large files** | Max 100k lines, need 500k+ | Exposes O(n²) issues |
| **Search/replace** | No find operations | Heavy workload in real usage |
| **Text layout** | No line wrapping/measurement | Critical for viewport rendering |

---

## Industry Comparison

### How Other Rust Editors Benchmark

| Editor | Framework | Notable Benchmarks |
|--------|-----------|-------------------|
| **Zed** | criterion | Rope operations with throughput metrics, 1000-cursor edits, display map conversions, project panel sorting (40k+ files) |
| **xi-editor** | test::Bencher | Diff computation (SSE/AVX variants), realistic editing scenarios, 1M-char files |
| **Lapce** | criterion | Visual line calculations, text layout with wrap modes on 3k-line documents |
| **Helix** | — | No dedicated benchmark suite |

### Key Gaps vs. Industry

1. **No throughput metrics** — Zed measures `Throughput::Bytes` to catch algorithmic complexity
2. **No diff benchmarks** — xi-editor extensively tests diff algorithms
3. **No text layout** — Lapce focuses heavily on visual line calculations
4. **Limited scale testing** — Need pathological cases (very large files, many cursors)

---

## Improvement Plan

### Phase 1: Fix Inaccurate Benchmarks (1 day)

#### 1.1 Rewrite `glyph_cache.rs`

Replace fictional bitmap allocation with actual fontdue rasterization:

```rust
// NEW: Test actual glyph rasterization
#[divan::bench(args = [12.0, 16.0, 20.0, 24.0])]
fn glyph_rasterize(font_size: f32) {
    let font = load_test_font();  // JetBrains Mono from assets/
    for ch in "The quick brown fox".chars() {
        let (_metrics, bitmap) = font.rasterize(ch, font_size);
        divan::black_box(bitmap);
    }
}

// NEW: Test cache hit vs miss patterns
#[divan::bench]
fn glyph_cache_realistic_paragraph() {
    let font = load_test_font();
    let mut cache: GlyphCache = HashMap::new();
    let text = "The quick brown fox jumps over the lazy dog. ".repeat(10);
    
    for ch in text.chars() {
        let key = (ch, 16.0_f32.to_bits());
        cache.entry(key).or_insert_with(|| font.rasterize(ch, 16.0));
    }
    divan::black_box(&cache);
}
```

#### 1.2 Use Actual Renderer Code

Extract blend logic from `src/view/frame.rs` into a testable function:

```rust
// In src/view/blend.rs (new file)
#[inline]
pub fn blend_pixel(bg: u32, fg: u32, alpha: u8) -> u32 { ... }

// In benches/rendering.rs
use token::view::blend::blend_pixel;  // Use actual implementation
```

---

### Phase 2: Add Missing Critical Benchmarks (2 days)

#### 2.1 Syntax Highlighting Benchmarks

```rust
// benches/syntax.rs (NEW FILE)

#[divan::bench(args = ["rust", "php", "markdown"])]
fn parse_full_document(language: &str) {
    let source = load_sample_file(language);  // 1000+ line files
    let parser = create_parser(language);
    let tree = parser.parse(&source, None);
    divan::black_box(tree);
}

#[divan::bench]
fn parse_incremental_after_edit() {
    let mut parser = create_parser("rust");
    let source = load_sample_file("rust");
    let tree = parser.parse(&source, None).unwrap();
    
    // Simulate editing middle of file
    let edit = InputEdit { ... };
    let new_tree = parser.parse(&modified_source, Some(&tree));
    divan::black_box(new_tree);
}

#[divan::bench]
fn query_highlights_visible_range() {
    let tree = parse_sample("rust");
    let query = Query::new(&tree_sitter_rust::language(), HIGHLIGHTS_QUERY);
    let mut cursor = QueryCursor::new();
    
    // Only query visible viewport (50 lines)
    cursor.set_byte_range(visible_start..visible_end);
    for match_ in cursor.matches(&query, tree.root_node(), source.as_bytes()) {
        divan::black_box(match_);
    }
}
```

#### 2.2 Multi-Cursor Benchmarks

```rust
// Add to benches/main_loop.rs

#[divan::bench(args = [10, 100, 1000])]
fn multi_cursor_insert_char(cursor_count: usize) {
    let mut model = make_model(10_000);
    
    // Create multiple cursors
    for i in 0..cursor_count {
        let line = i * 10;
        model.add_cursor_at(line, 0);
    }
    
    // Insert at all cursors simultaneously
    let cmd = update(&mut model, Msg::Document(DocumentMsg::InsertChar('x')));
    divan::black_box(cmd);
}

#[divan::bench(args = [10, 100, 1000])]
fn multi_cursor_delete(cursor_count: usize) {
    // Similar setup, test delete operation
}
```

#### 2.3 Large File Scaling

```rust
// Add to benches/rope_operations.rs

#[divan::bench(args = [100_000, 500_000, 1_000_000])]
fn insert_middle_large_file(line_count: usize) {
    let mut rope = Rope::from_str(&"line content here\n".repeat(line_count));
    let pos = rope.len_chars() / 2;
    rope.insert(pos, "inserted text\n");
    divan::black_box(&rope);
}

#[divan::bench(args = [100_000, 500_000, 1_000_000])]
fn navigate_large_file(line_count: usize) {
    let rope = Rope::from_str(&"line content here\n".repeat(line_count));
    
    // Jump around the file
    for target_line in [0, line_count/4, line_count/2, line_count*3/4, line_count-1] {
        let char_idx = rope.line_to_char(target_line);
        divan::black_box(char_idx);
    }
}
```

#### 2.4 Search Operations

```rust
// benches/search.rs (NEW FILE)

#[divan::bench(args = [1_000, 10_000, 100_000])]
fn search_literal_string(line_count: usize) {
    let rope = Rope::from_str(&"The quick brown fox\n".repeat(line_count));
    let needle = "brown";
    
    let mut matches = Vec::new();
    for (line_idx, line) in rope.lines().enumerate() {
        if let Some(col) = line.to_string().find(needle) {
            matches.push((line_idx, col));
        }
    }
    divan::black_box(matches);
}

#[divan::bench]
fn search_regex_pattern() {
    let rope = Rope::from_str(&"fn foo() { }\nfn bar() { }\n".repeat(5000));
    let pattern = regex::Regex::new(r"fn \w+\(").unwrap();
    
    let mut matches = Vec::new();
    for (line_idx, line) in rope.lines().enumerate() {
        for m in pattern.find_iter(&line.to_string()) {
            matches.push((line_idx, m.start()));
        }
    }
    divan::black_box(matches);
}
```

---

### Phase 3: Add Throughput Metrics (0.5 days)

If switching to criterion (recommended for CI), add throughput tracking:

```rust
use criterion::{criterion_group, criterion_main, Criterion, Throughput};

fn rope_insert_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("rope_insert");
    
    for size in [1_000, 10_000, 100_000].iter() {
        let input = "line\n".repeat(*size);
        group.throughput(Throughput::Bytes(input.len() as u64));
        
        group.bench_with_input(
            BenchmarkId::from_parameter(size),
            &input,
            |b, input| {
                b.iter(|| {
                    let mut rope = Rope::from_str(input);
                    rope.insert(rope.len_chars() / 2, "inserted\n");
                    rope
                });
            },
        );
    }
    group.finish();
}
```

---

### Phase 4: Text Layout Benchmarks (0.5 days)

```rust
// benches/layout.rs (NEW FILE)

#[divan::bench(args = [80, 120, 200])]
fn measure_line_width(max_chars: usize) {
    let font = load_test_font();
    let line = "x".repeat(max_chars);
    
    let mut width = 0.0f32;
    for ch in line.chars() {
        let metrics = font.metrics(ch, 16.0);
        width += metrics.advance_width;
    }
    divan::black_box(width);
}

#[divan::bench(args = [25, 50, 100])]
fn calculate_visible_lines(viewport_lines: usize) {
    let model = make_model(50_000);
    let doc = model.document();
    let viewport = &model.editor().viewport;
    
    let mut lines = Vec::with_capacity(viewport_lines);
    for i in 0..viewport_lines {
        let line_idx = viewport.top_line + i;
        if line_idx < doc.line_count() {
            lines.push(doc.line(line_idx));
        }
    }
    divan::black_box(lines);
}
```

---

## File Structure After Changes

```
benches/
├── glyph_cache.rs      # REWRITTEN: Test actual fontdue rasterization
├── main_loop.rs        # UPDATED: Add multi-cursor benchmarks
├── rendering.rs        # UPDATED: Use actual blend_pixel from src/
├── rope_operations.rs  # UPDATED: Add large file scaling tests
├── support.rs          # UPDATED: Remove duplicated blend_pixel
├── syntax.rs           # NEW: Tree-sitter parsing benchmarks
├── search.rs           # NEW: Find/replace operations
└── layout.rs           # NEW: Text measurement and viewport calculations
```

---

## Framework Consideration: divan vs criterion

### Current: divan

**Pros:**
- Simpler API
- Built-in allocation profiling (`AllocProfiler`)
- Faster iteration during development

**Cons:**
- Less ecosystem adoption
- No built-in HTML reports
- No historical regression tracking

### Recommended: Keep divan, add criterion for CI

```toml
# Cargo.toml
[dev-dependencies]
divan = "0.1"           # Quick local benchmarks
criterion = "0.5"       # CI with regression tracking

[[bench]]
name = "quick"
harness = false         # divan for local dev

[[bench]]
name = "ci"
harness = false         # criterion for CI tracking
```

---

## Implementation Checklist

### Phase 1: Fix Inaccurate Benchmarks ✅ (Completed 2025-12-15)
- [x] Rewrite `glyph_bitmap_alloc` to use actual fontdue
- [x] Extract `blend_pixel` to `src/lib.rs::rendering` module
- [x] Update `rendering.rs` to use extracted function
- [x] Remove duplicated code from `support.rs`
- [x] Add test font loading helper

### Phase 2: Add Missing Benchmarks ✅ (Completed 2025-12-15)
- [ ] Create `benches/syntax.rs` with tree-sitter benchmarks (deferred — feature not ready)
- [x] Add multi-cursor benchmarks to `main_loop.rs`
- [x] Add 500k+ line tests to `rope_operations.rs`
- [x] Create `benches/search.rs` for find/replace
- [x] Create `benches/layout.rs` for text measurement

### Phase 3: Throughput Metrics (Future)
- [ ] Add criterion as dev dependency
- [ ] Create `benches/ci.rs` with throughput tracking
- [ ] Document CI integration approach

### Phase 4: Text Layout ✅ (Completed 2025-12-15)
- [x] Add line width measurement benchmarks
- [x] Add visible line calculation benchmarks
- [ ] Add line wrapping benchmarks (future — when soft wrap implemented)

---

## Success Criteria

1. **No fictional benchmarks** — All benchmarks test actual code paths
2. **Critical path coverage** — Syntax, multi-cursor, search, large files all benchmarked
3. **Scale diversity** — Tests from 1k to 1M+ lines
4. **Reproducibility** — Benchmarks produce consistent results across runs
5. **CI-ready** — Can detect 10%+ regressions in critical paths

---

## References

- [Zed benchmarks](https://github.com/zed-industries/zed/tree/main/crates) — Comprehensive suite with throughput metrics
- [xi-editor benchmarks](https://github.com/xi-editor/xi-editor/tree/master/rust/rope/benches) — Realistic editing scenarios
- [Lapce visual_line.rs](https://github.com/lapce/lapce/blob/ba703dd4aabb264e359385db12baaa266a697a2e/lapce-app/benches/visual_line.rs) — Text layout focus
- [divan documentation](https://docs.rs/divan/latest/divan/)
- [criterion documentation](https://bheisler.github.io/criterion.rs/book/)
