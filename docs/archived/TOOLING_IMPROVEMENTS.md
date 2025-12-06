# Tooling & Infrastructure Improvements

> Comprehensive recommendations for performance profiling, benchmarking, and developer experience improvements for the Token text editor.

## Table of Contents

- [Executive Summary](#executive-summary)
- [Performance Profiling](#performance-profiling)
- [Benchmarking](#benchmarking)
- [Code Coverage](#code-coverage)
- [CI/CD Integration](#cicd-integration)
- [Developer Experience](#developer-experience)
- [Implementation Roadmap](#implementation-roadmap)

---

## Executive Summary

This document consolidates findings from Oracle analysis, Librarian research, and codebase analysis to provide actionable recommendations for improving Token's performance tooling and developer experience.

### Key Findings

| Area                 | Current State               | Recommendation                          |
| -------------------- | --------------------------- | --------------------------------------- |
| **Benchmarking**     | None                        | Add Divan or Criterion benchmarks       |
| **CPU Profiling**    | None                        | Add `cargo-flamegraph` + `samply`       |
| **Memory Profiling** | None                        | Add `dhat-rs` + `heaptrack` integration |
| **Code Coverage**    | None                        | Add `cargo-llvm-cov`                    |
| **CI Performance**   | None                        | Add benchmark regression tracking       |
| **Dev Workflow**     | `make` only                 | Add `bacon` for watch mode              |
| **Frame Timing**     | PerfStats exists but unused | Wire up existing infrastructure         |

---

## Performance Profiling

### CPU Profiling

#### 1. cargo-flamegraph (Cross-platform, CI-friendly)

```bash
cargo install flamegraph
```

**Cargo.toml profile:**

```toml
[profile.profiling]
inherits = "release"
debug = true      # Needed for good symbols
lto = false       # Avoid LTO for readable profiles
codegen-units = 1 # Consistent profiling
```

**Makefile targets:**

```make
# Profile build with debug symbols
build-prof:
	cargo build --profile profiling

# CPU flamegraph (open flamegraph.svg after)
flamegraph: build-prof
	cargo flamegraph --profile profiling -- ./target/profiling/token test_files/large.txt

flamegraph-edit: build-prof
	@echo "Perform editing actions, then close the editor"
	cargo flamegraph --profile profiling -o flamegraph-edit.svg -- ./target/profiling/token test_files/large.txt
```

#### 2. samply (macOS/Linux - Interactive)

Best for interactive development on macOS with Firefox Profiler integration.

```bash
cargo install samply
```

**Makefile target:**

```make
# Interactive profiling with Firefox Profiler
profile-samply: build-prof
	samply record ./target/profiling/token test_files/large.txt
```

**Key benefits for Token:**

- Off-CPU analysis reveals UI thread blocking
- Source code view alongside CPU samples
- Timeline view for frame-by-frame analysis

#### 3. Instruments (macOS - Deep Dives)

For detailed macOS analysis:

1. Build with `profiling` profile
2. Open Instruments → Time Profiler
3. Target the `token` process
4. Record while performing interactions

---

### Memory Profiling

#### 1. dhat-rs (Heap Profiler)

**Cargo.toml:**

```toml
[dependencies]
dhat = { version = "0.3", optional = true }

[features]
dhat-heap = ["dhat"]
```

**src/main.rs instrumentation:**

```rust
#[cfg(feature = "dhat-heap")]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

fn main() {
    #[cfg(feature = "dhat-heap")]
    let _profiler = dhat::Profiler::new_heap();

    // ... rest of main
}
```

**Makefile target:**

```make
# Memory profiling with dhat
profile-memory:
	cargo run --features dhat-heap --release -- test_files/large.txt
	@echo "Open dhat-heap.json with dh_view.html"
```

#### 2. heaptrack (Linux)

```bash
sudo apt install heaptrack heaptrack-gui
```

**Makefile target:**

```make
# Heaptrack analysis (Linux only)
profile-heap: build-prof
	heaptrack ./target/profiling/token test_files/large.txt
	heaptrack_gui heaptrack.token.*
```

**What to look for:**

- Glyph cache unbounded growth
- Rope allocations during editing
- Undo stack memory usage

---

## Benchmarking

### Framework Comparison

| Feature           | Criterion        | Divan                       |
| ----------------- | ---------------- | --------------------------- |
| API Style         | Verbose closures | `#[divan::bench]` attribute |
| Memory Tracking   | External only    | Built-in `AllocProfiler`    |
| Thread Contention | Manual           | Built-in support            |
| Maturity          | Established      | Newer, modern               |
| Recommendation    | ✓ Good           | ✓✓ Preferred for GUI apps   |

### Recommended: Divan

**Cargo.toml:**

```toml
[dev-dependencies]
divan = "0.1"

[[bench]]
name = "rope_operations"
harness = false

[[bench]]
name = "rendering"
harness = false

[[bench]]
name = "glyph_cache"
harness = false
```

#### Benchmark: Rope Operations

**benches/rope_operations.rs:**

```rust
use ropey::Rope;

#[global_allocator]
static ALLOC: divan::AllocProfiler = divan::AllocProfiler::system();

fn main() {
    divan::main();
}

#[divan::bench]
fn insert_middle_10k_lines() {
    let mut rope = Rope::from_str(&"foo bar baz\n".repeat(10_000));
    let pos = rope.len_chars() / 2;
    rope.insert(pos, divan::black_box("inserted text\n"));
}

#[divan::bench]
fn delete_middle_10k_lines() {
    let mut rope = Rope::from_str(&"foo bar baz\n".repeat(10_000));
    let start = rope.len_chars() / 2;
    let end = start + 100;
    rope.remove(start..end);
}

#[divan::bench(args = [100, 1000, 10000])]
fn line_iteration(n: usize) {
    let rope = Rope::from_str(&"test line\n".repeat(n));
    for line in rope.lines() {
        divan::black_box(line);
    }
}

#[divan::bench]
fn cursor_to_offset_current() {
    // Benchmark current O(n) implementation
    let rope = Rope::from_str(&"test line\n".repeat(10_000));
    let line = 5000;
    let mut offset = 0;
    for (i, l) in rope.lines().enumerate() {
        if i == line { break; }
        offset += l.len_chars();
    }
    divan::black_box(offset);
}

#[divan::bench]
fn cursor_to_offset_optimized() {
    // Benchmark O(log n) approach
    let rope = Rope::from_str(&"test line\n".repeat(10_000));
    let line = 5000;
    let offset = rope.line_to_char(line);
    divan::black_box(offset);
}
```

#### Benchmark: Glyph Cache

**benches/glyph_cache.rs:**

```rust
use std::collections::HashMap;
use fontdue::{Font, FontSettings};

#[global_allocator]
static ALLOC: divan::AllocProfiler = divan::AllocProfiler::system();

fn main() {
    divan::main();
}

static FONT_BYTES: &[u8] = include_bytes!("../assets/JetBrainsMono-Regular.ttf");

#[divan::bench]
fn glyph_rasterize_cold_cache() {
    let font = Font::from_bytes(FONT_BYTES, FontSettings::default()).unwrap();
    let chars: Vec<char> = "The quick brown fox jumps over the lazy dog".chars().collect();

    for &ch in &chars {
        divan::black_box(font.rasterize(ch, 16.0));
    }
}

#[divan::bench]
fn glyph_cache_lookup_double() {
    // Current implementation: contains_key + get
    let mut cache: HashMap<(char, u32), Vec<u8>> = HashMap::new();
    let key = ('a', 16);
    cache.insert(key, vec![0u8; 256]);

    for _ in 0..1000 {
        if cache.contains_key(&key) {
            divan::black_box(cache.get(&key));
        }
    }
}

#[divan::bench]
fn glyph_cache_lookup_entry() {
    // Optimized: single entry() call
    let mut cache: HashMap<(char, u32), Vec<u8>> = HashMap::new();
    let key = ('a', 16);
    cache.insert(key, vec![0u8; 256]);

    for _ in 0..1000 {
        divan::black_box(cache.entry(key).or_insert_with(|| vec![0u8; 256]));
    }
}
```

#### Benchmark: Rendering

**benches/rendering.rs:**

```rust
#[global_allocator]
static ALLOC: divan::AllocProfiler = divan::AllocProfiler::system();

fn main() {
    divan::main();
}

#[divan::bench(args = [100, 500, 1000, 2000])]
fn clear_buffer(width: usize) {
    let height = width; // Square for simplicity
    let mut buffer: Vec<u32> = vec![0; width * height];
    let bg_color = 0xFF1E1E2E_u32;

    buffer.fill(bg_color);
    divan::black_box(&buffer);
}

#[divan::bench]
fn alpha_blend_text() {
    let mut buffer: Vec<u32> = vec![0xFF1E1E2E; 1920 * 1080];
    let glyph = vec![128u8; 16 * 16]; // 16x16 glyph
    let fg_color = 0xFFCDD6F4_u32;

    // Simulate blending 1000 glyphs
    for i in 0..1000 {
        let base_x = (i % 100) * 10;
        let base_y = (i / 100) * 20;

        for (gy, row) in glyph.chunks(16).enumerate() {
            for (gx, &alpha) in row.iter().enumerate() {
                let px = base_x + gx;
                let py = base_y + gy;
                if px < 1920 && py < 1080 {
                    let idx = py * 1920 + px;
                    let bg = buffer[idx];
                    // Alpha blending
                    let a = alpha as u32;
                    let inv_a = 255 - a;
                    let r = ((fg_color >> 16 & 0xFF) * a + (bg >> 16 & 0xFF) * inv_a) / 255;
                    let g = ((fg_color >> 8 & 0xFF) * a + (bg >> 8 & 0xFF) * inv_a) / 255;
                    let b = ((fg_color & 0xFF) * a + (bg & 0xFF) * inv_a) / 255;
                    buffer[idx] = 0xFF000000 | (r << 16) | (g << 8) | b;
                }
            }
        }
    }
    divan::black_box(&buffer);
}
```

**Makefile targets:**

```make
# Run all benchmarks
bench:
	cargo bench

# Run specific benchmark group
bench-rope:
	cargo bench rope_operations

bench-render:
	cargo bench rendering

bench-glyph:
	cargo bench glyph_cache
```

---

## Code Coverage

### cargo-llvm-cov

```bash
cargo install cargo-llvm-cov
rustup component add llvm-tools-preview
```

**Makefile targets:**

```make
# Generate HTML coverage report
coverage:
	cargo llvm-cov --html --open

# Coverage with nextest (faster)
coverage-fast:
	cargo llvm-cov nextest --html --open

# Generate coverage for CI (codecov format)
coverage-ci:
	cargo llvm-cov --codecov --output-path codecov.json
```

**GitHub Actions integration:**

```yaml
- uses: dtolnay/rust-toolchain@stable
  with:
    components: llvm-tools-preview
- uses: taiki-e/install-action@cargo-llvm-cov
- run: cargo llvm-cov nextest --codecov --output-path codecov.json
- uses: codecov/codecov-action@v4
  with:
    file: codecov.json
```

---

## CI/CD Integration

### Benchmark Regression Detection

#### Simple Approach: Threshold Script

**scripts/bench-check.py:**

```python
#!/usr/bin/env python3
"""Compare benchmark results and flag regressions."""

import json
import sys

REGRESSION_THRESHOLD = 1.20  # 20% slower = regression

def main():
    baseline = json.load(open("bench-baseline.json"))
    current = json.load(open("bench-current.json"))

    regressions = []
    for name, base_time in baseline.items():
        if name in current:
            ratio = current[name] / base_time
            if ratio > REGRESSION_THRESHOLD:
                regressions.append(f"{name}: {ratio:.1%} slower")

    if regressions:
        print("PERFORMANCE REGRESSIONS DETECTED:")
        for r in regressions:
            print(f"  - {r}")
        sys.exit(1)

    print("No significant regressions detected")

if __name__ == "__main__":
    main()
```

#### GitHub Actions Workflow

**.github/workflows/bench.yml:**

```yaml
name: Benchmarks

on:
  push:
    branches: [main]
  pull_request:
  schedule:
    - cron: "0 0 * * *" # Nightly

jobs:
  bench:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - uses: dtolnay/rust-toolchain@stable

      - name: Run benchmarks
        run: cargo bench -- --output-format bencher > bench.txt

      - name: Upload benchmark results
        uses: actions/upload-artifact@v4
        with:
          name: bench-results
          path: bench.txt

      # Optional: GitHub Action Benchmark for trend tracking
      - name: Store benchmark result
        uses: benchmark-action/github-action-benchmark@v1
        with:
          tool: "cargo"
          output-file-path: bench.txt
          github-token: ${{ secrets.GITHUB_TOKEN }}
          auto-push: true
          alert-threshold: "150%"
          comment-on-alert: true
          fail-on-alert: false # Start non-blocking
```

#### Advanced: Bencher.dev

For continuous benchmark tracking with dashboards:

```bash
curl --proto '=https' --tlsv1.2 -sSfL https://bencher.dev/download/install-cli.sh | bash
```

```yaml
- uses: bencherdev/bencher@main
- run: |
    bencher run \
      --project token-editor \
      --token ${{ secrets.BENCHER_API_TOKEN }} \
      --adapter rust_criterion \
      "cargo bench"
```

---

## Developer Experience

### 1. Bacon (Watch Mode) - Replaces cargo-watch

```bash
cargo install bacon
```

**bacon.toml:**

```toml
default_job = "check"

[jobs.check]
command = ["cargo", "check", "--all-targets"]
need_stdout = false

[jobs.clippy]
command = ["cargo", "clippy", "--all-targets", "--", "-D", "warnings"]
need_stdout = false

[jobs.test]
command = ["cargo", "test"]
need_stdout = true

[jobs.nextest]
command = ["cargo", "nextest", "run"]
need_stdout = true

[jobs.bench]
command = ["cargo", "bench"]
need_stdout = true

[jobs.doc]
command = ["cargo", "doc", "--no-deps"]
need_stdout = false

[keybindings]
c = "job:clippy"
t = "job:test"
n = "job:nextest"
b = "job:bench"
d = "job:doc"
```

**Makefile target:**

```make
# Start watch mode
watch:
	bacon

# Watch with clippy
watch-lint:
	bacon clippy
```

### 2. cargo-nextest (Faster Testing)

```bash
cargo install cargo-nextest
```

**Makefile targets:**

```make
# Fast parallel tests
test-fast:
	cargo nextest run

# With retries for flaky tests
test-retry:
	cargo nextest run --retries 2
```

### 3. In-App Frame Profiler

Wire up the existing `PerfStats` infrastructure:

**src/model/mod.rs additions:**

```rust
use std::time::{Duration, Instant};

pub struct FrameProfiler {
    pub frame_start: Instant,
    pub update_duration: Duration,
    pub render_duration: Duration,
    pub frame_times: Vec<Duration>,
}

impl FrameProfiler {
    pub fn new() -> Self {
        Self {
            frame_start: Instant::now(),
            update_duration: Duration::ZERO,
            render_duration: Duration::ZERO,
            frame_times: Vec::with_capacity(120),
        }
    }

    pub fn begin_frame(&mut self) {
        let now = Instant::now();
        if self.frame_times.len() >= 120 {
            self.frame_times.remove(0);
        }
        self.frame_times.push(now.duration_since(self.frame_start));
        self.frame_start = now;
    }

    pub fn avg_fps(&self) -> f64 {
        if self.frame_times.is_empty() { return 0.0; }
        let avg = self.frame_times.iter().sum::<Duration>() / self.frame_times.len() as u32;
        1.0 / avg.as_secs_f64()
    }
}
```

**Feature gate:**

```toml
[features]
dev-prof = []
```

```rust
#[cfg(feature = "dev-prof")]
fn draw_profiler_hud(&self, buffer: &mut [u32], width: usize) {
    // Draw FPS and timing overlay in top-left corner
}
```

**Makefile target:**

```make
# Run with profiler HUD
run-prof:
	cargo run --features dev-prof --release -- test_files/large.txt
```

### 4. Structured Tracing (Optional)

**Cargo.toml:**

```toml
[dependencies]
tracing = { version = "0.1", optional = true }
tracing-subscriber = { version = "0.3", optional = true }
tracing-chrome = { version = "0.7", optional = true }

[features]
tracing = ["dep:tracing", "dep:tracing-subscriber", "dep:tracing-chrome"]
```

**Usage:**

```rust
#[cfg(feature = "tracing")]
use tracing::{instrument, span, Level};

#[cfg_attr(feature = "tracing", instrument)]
fn update(&mut self, msg: Msg) {
    // ...
}

fn render(&mut self) {
    #[cfg(feature = "tracing")]
    let _span = span!(Level::TRACE, "render").entered();
    // ...
}
```

**Makefile target:**

```make
# Run with chrome tracing (output to trace.json)
run-trace:
	cargo run --features tracing --release -- test_files/large.txt
	@echo "Open trace-*.json in chrome://tracing or Perfetto"
```

---

## Implementation Roadmap

### Phase 1: Foundation (1 day)

- [x] Add `[profile.profiling]` to Cargo.toml
- [x] Add `divan` dev-dependency and benchmark stubs
- [x] Add `cargo-llvm-cov` coverage targets to Makefile
- [x] Install `bacon` and create `bacon.toml`

### Phase 2: Benchmarks (1 day)

- [x] Create `benches/rope_operations.rs`
- [x] Create `benches/glyph_cache.rs`
- [x] Create `benches/rendering.rs`
- [x] Add `make bench` targets

### Phase 3: Profiling (0.5 day)

- [x] Add `dhat-rs` optional dependency
- [x] Add flamegraph/samply Makefile targets
- [x] Wire up `PerfStats` for in-app timing

### Phase 4: CI Integration (0.5 day)

- [x] Add `.github/workflows/bench.yml`
- [x] Add coverage reporting to CI
- [x] Set up benchmark artifact storage

---

## Quick Reference

### Installation Commands

```bash
# All recommended tools
cargo install flamegraph samply cargo-llvm-cov cargo-nextest bacon divan

# Rust components
rustup component add llvm-tools-preview
```

### Common Workflows

```bash
# Development loop
bacon                           # Watch mode

# Performance investigation
make flamegraph                 # CPU profile
make profile-memory             # Heap analysis
cargo bench rope_operations     # Specific benchmark

# Before PR
make test-fast                  # Quick tests
make coverage                   # Coverage report
make lint                       # Clippy
```

### New Makefile Targets Summary

```make
# Profiling
build-prof           # Build with debug symbols
flamegraph           # Generate CPU flamegraph
profile-samply       # Interactive profiler (macOS/Linux)
profile-memory       # Heap profiling with dhat

# Benchmarking
bench                # Run all benchmarks
bench-rope           # Rope operation benchmarks
bench-render         # Rendering benchmarks
bench-glyph          # Glyph cache benchmarks

# Coverage
coverage             # HTML coverage report
coverage-ci          # CI-format coverage

# Development
watch                # Bacon watch mode
test-fast            # nextest parallel tests
run-prof             # Run with profiler HUD
run-trace            # Run with chrome tracing
```
