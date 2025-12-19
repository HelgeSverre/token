# Benchmarking & Profiling Guide

This document describes how to run benchmarks and profile the Token editor.

## Quick Reference

```bash
# Run all benchmarks
cargo bench

# Run a specific benchmark suite
cargo bench --bench rope_operations
cargo bench --bench rendering
cargo bench --bench main_loop
cargo bench --bench syntax
cargo bench --bench layout
cargo bench --bench search
cargo bench --bench glyph_cache
cargo bench --bench workspace

# Run specific benchmark by name pattern
cargo bench insert_middle

# Build with profiling symbols
cargo build --profile profiling

# Heap profiling (requires dhat-heap feature)
cargo run --release --features dhat-heap
```

## Benchmark Suites

### rope_operations
Text buffer operations using the ropey crate. Tests insert, delete, navigation,
and line iteration at various document sizes (10k to 1M lines).

Key benchmarks:
- `insert_middle_*` - Insertion performance at document midpoint
- `line_to_char` / `char_to_line` - Cursor position conversions
- `visible_lines_iteration` - Viewport rendering cost

### rendering
Low-level rendering primitives. Tests buffer clearing, alpha blending,
and simulated frame rendering.

Key benchmarks:
- `alpha_blend_text_line` - Text rendering throughput
- `render_visible_lines` - Full viewport rendering simulation
- `render_line_numbers` - Gutter rendering cost

### main_loop
Full message → update → command → render cycle. Measures end-to-end
latency for user actions.

Key benchmarks:
- `update_insert_char` - Typing latency
- `update_move_cursor_*` - Navigation latency
- `full_loop_*` - Complete update+render cycle
- `scaling_*` - How performance scales with document size
- `multi_cursor_*` - Performance with multiple cursors

### syntax
Tree-sitter parsing and syntax highlighting performance.

Key benchmarks:
- `parse_*_sample` - Initial parse time per language
- `parse_large_rust` - Scaling with document size
- `incremental_parse_*` - Incremental parsing speedup
- `extract_highlights_lookup` - Highlight retrieval for rendering

### layout
Text layout and font metrics calculations.

Key benchmarks:
- `measure_line_width` - Line width calculation
- `full_viewport_layout` - Complete viewport layout
- `calculate_gutter_width` - Dynamic gutter sizing

### search
Find and replace operations.

Key benchmarks:
- Search text finding in documents
- Occurrence highlighting
- Replace operations

### glyph_cache
Font glyph caching and lookup performance.

### workspace (NEW)
File tree and workspace operations.

Key benchmarks:
- `classify_*` - File extension detection
- `count_visible_*` - File tree traversal
- `get_visible_item_*` - Item lookup by index
- `workspace_*` - Workspace state operations
- `large_tree_*` - Scaling with large file trees

## Profiling

### CPU Profiling with Instruments (macOS)

```bash
# Build with profiling profile
cargo build --profile profiling

# Run with Instruments
xcrun xctrace record --template "Time Profiler" --launch ./target/profiling/token samples/large_file.rs
```

### Heap Profiling with dhat

```bash
# Build and run with dhat
cargo run --release --features dhat-heap

# View the output in dhat-viewer
# https://nnethercote.github.io/dh_view/dh_view.html
```

### Flamegraph (Linux)

```bash
# Install cargo-flamegraph
cargo install flamegraph

# Generate flamegraph
cargo flamegraph --profile profiling -- samples/large_file.rs
```

### Samply (macOS/Linux)

```bash
# Install samply
cargo install samply

# Profile the editor
cargo build --profile profiling
samply record ./target/profiling/token samples/large_file.rs
```

## Performance Targets

### Typing Latency
- Target: < 16ms (60fps)
- Measured by: `full_loop_insert_char_and_render`

### Cursor Movement
- Target: < 5ms
- Measured by: `update_move_cursor_*`

### Large File Navigation
- Target: < 50ms for page down in 100k line file
- Measured by: `scaling_page_down_by_doc_size`

### Syntax Highlighting
- Target: < 100ms initial parse for 10k lines
- Measured by: `parse_large_rust`

### File Tree Rendering
- Target: < 10ms for 1000 visible items
- Measured by: `count_visible_*`, `get_visible_item_*`

## Adding New Benchmarks

Benchmarks use the [divan](https://github.com/nvzqz/divan) crate. Example:

```rust
use token::model::AppModel;

#[divan::bench]
fn my_benchmark() {
    let model = make_model(10_000);
    divan::black_box(do_something(&model));
}

#[divan::bench(args = [100, 1000, 10000])]
fn scaled_benchmark(size: usize) {
    let data = create_data(size);
    divan::black_box(process(&data));
}
```

### Guidelines

1. Use `divan::black_box()` to prevent optimization
2. Parameterize with `args = [...]` for scaling tests
3. Group related benchmarks in the same file
4. Name benchmarks descriptively: `{operation}_{context}_{size}`
5. Add the benchmark file to `[[bench]]` in Cargo.toml

## Interpreting Results

Divan output shows:
- **fastest** - Best-case time
- **slowest** - Worst-case time
- **median** - Typical time
- **mean** - Average time
- **allocs** - Memory allocations (with divan::AllocProfiler)

Look for:
- Unexpected scaling (O(n²) when O(n) expected)
- High allocation counts in hot paths
- Large variance between fastest/slowest

## CI Integration

Benchmarks can be run in CI to detect regressions:

```yaml
- name: Run benchmarks
  run: cargo bench -- --save-baseline main

- name: Compare benchmarks
  run: cargo bench -- --baseline main
```
