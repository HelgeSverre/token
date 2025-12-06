# Performance Infrastructure Audit

**Date:** 2024  
**Scope:** Analysis of existing performance monitoring in `/src/main.rs` and `/src/overlay.rs`

---

## 1. Existing Infrastructure Inventory

| Component | Location | Status | Notes |
|-----------|----------|--------|-------|
| `PerfStats` struct | `main.rs:31-58` | ✅ Active | Debug-only, comprehensive field set |
| `reset_frame_stats()` | `main.rs:63-66` | ⚠️ Defined but unused | Resets per-frame cache counters |
| `record_frame_time()` | `main.rs:68-76` | ✅ Active | 60-frame rolling window |
| `avg_frame_time()` | `main.rs:78-84` | ✅ Active | Simple average over window |
| `fps()` | `main.rs:86-93` | ✅ Active | Derived from avg_frame_time |
| `cache_hit_rate()` | `main.rs:95-102` | ✅ Active | Cumulative percentage |
| `render_perf_overlay()` | `main.rs:944-1138` | ✅ Active | Renders overlay on F2 toggle |
| `OverlayConfig` | `overlay.rs:18-53` | ✅ Active | Reusable overlay positioning |
| `OverlayBounds` | `overlay.rs:82-105` | ✅ Active | Computed screen coordinates |
| `blend_pixel()` | `overlay.rs:111-128` | ✅ Active | Alpha blending helper |
| `render_overlay_background()` | `overlay.rs:138-156` | ✅ Active | Semi-transparent BG |
| `render_overlay_border()` | `overlay.rs:166-217` | ✅ Active | 1px border rendering |

### Conditional Compilation Usage

| Line | Purpose |
|------|---------|
| `5` | `VecDeque` import (debug only) |
| `30-58` | `PerfStats` struct definition |
| `60-103` | `PerfStats` impl block |
| `177-180` | `Renderer::render()` debug signature |
| `538-899` | `render_impl_with_perf()` (duplicated render with perf) |
| `944-1138` | `render_perf_overlay()` function |
| `1532-1533` | `perf` field on `App` struct |
| `1555-1556` | `perf` initialization in `App::new()` |
| `1605-1608` | F2 key toggle handler |
| `1840-1852` | `App::render()` debug version with timing |

---

## 2. Metric Coverage

### Currently Measured

| Metric | How | Display |
|--------|-----|---------|
| **Frame time** | `frame_start` → `record_frame_time()` | `{:.1}ms` |
| **FPS** | `1.0 / avg_frame_time()` | `{:.0} fps` |
| **Frame budget %** | `frame_ms / 16.67 * 100` | Bar graph `[████░░░░░░]` |
| **Average frame time** | Rolling 60-frame window | `{:.1}ms` |
| **Glyph cache size** | `glyph_cache.len()` | `{} glyphs` |
| **Cache hit rate** | `total_hits / (hits + misses)` | `{:.1}%` |
| **Cache hits** | `total_cache_hits` | Absolute count |
| **Cache misses** | `total_cache_misses` | Absolute count |

### Defined But Not Populated

The `PerfStats` struct defines these timing fields that are **never set**:

```rust
clear_time: Duration,           // Not timed
line_highlight_time: Duration,  // Not timed
gutter_time: Duration,          // Not timed
text_time: Duration,            // Not timed
cursor_time: Duration,          // Not timed
status_bar_time: Duration,      // Not timed
present_time: Duration,         // Not timed
```

Per-frame cache stats are also not utilized:
```rust
frame_cache_hits: usize,    // Reset exists, but never incremented
frame_cache_misses: usize,  // Reset exists, but never incremented
```

### Missing Metrics

| Metric | Why Important |
|--------|---------------|
| **Input latency** | Time from keypress to visible update |
| **Rope operation time** | Insert/delete performance |
| **Syntax highlighting time** | If implemented later |
| **Memory usage** | Heap allocation tracking |
| **Scroll performance** | Frames during scroll vs static |
| **Large file metrics** | Line count, viewport calculation time |

---

## 3. Render Timing Breakdown (Gap Analysis)

The `render_impl_with_perf()` function at `main.rs:538-899` performs these phases:

| Phase | Lines | Timed? |
|-------|-------|--------|
| Surface resize | 541-550 | ❌ No |
| Buffer clear | 557-559 | ❌ No (`clear_time` unused) |
| Line highlight | 575-592 | ❌ No (`line_highlight_time` unused) |
| Selection highlight | 594-640 | ❌ No |
| Rectangle selection | 642-680 | ❌ No |
| Gutter rendering | 683-740 | ❌ No (`gutter_time` unused) |
| Text rendering | 742-810 | ❌ No (`text_time` unused) |
| Cursor rendering | 812-855 | ❌ No (`cursor_time` unused) |
| Status bar | 857-877 | ❌ No (`status_bar_time` unused) |
| Perf overlay | 880-893 | ❌ No |
| Present buffer | 895-898 | ❌ No (`present_time` unused) |

**Conclusion:** The timing infrastructure fields exist but are never populated. Only total frame time is measured.

---

## 4. Cache Statistics Analysis

### Current Implementation

```rust
// In PerfStats
total_cache_hits: usize,
total_cache_misses: usize,

// Hit rate calculation
fn cache_hit_rate(&self) -> f64 {
    let total = self.total_cache_hits + self.total_cache_misses;
    if total > 0 {
        self.total_cache_hits as f64 / total as f64 * 100.0
    } else {
        0.0
    }
}
```

### Issue: No Instrumentation Point

The `glyph_cache` is accessed in `draw_text()` at `main.rs:1140+`, but there's no code incrementing `total_cache_hits` or `total_cache_misses`. The overlay displays these values, but they remain at 0.

**Required fix:** Instrument the glyph lookup to count hits/misses:
```rust
// When cache lookup succeeds
perf.total_cache_hits += 1;
// When new glyph is rasterized
perf.total_cache_misses += 1;
```

---

## 5. Limitations Summary

| Limitation | Impact | Priority |
|------------|--------|----------|
| **Debug-only** | No production profiling | Medium |
| **No export** | Can't log to file/JSON | High |
| **No CI integration** | No regression detection | High |
| **Render phases not timed** | Can't identify bottlenecks | Medium |
| **Cache stats broken** | Always shows 0 hits/misses | Low |
| **No headless mode** | Requires GUI for any measurement | High |
| **Code duplication** | `render_impl` vs `render_impl_with_perf` | Low |

---

## 6. Extension Recommendations

### 6.1 Headless Benchmark Mode

Add a `--benchmark` CLI flag that:
- Loads a file without creating a window
- Simulates N frames of scrolling/editing
- Reports timing statistics to stdout/JSON

```rust
// Proposed structure
struct HeadlessBenchmark {
    model: AppModel,
    operations: Vec<BenchmarkOperation>,
    results: BenchmarkResults,
}
```

### 6.2 JSON/CSV Export

```rust
impl PerfStats {
    fn to_json(&self) -> String {
        format!(r#"{{
            "frame_time_ms": {:.2},
            "avg_frame_time_ms": {:.2},
            "fps": {:.1},
            "cache_size": {},
            "cache_hit_rate": {:.1}
        }}"#,
            self.last_frame_time.as_secs_f64() * 1000.0,
            self.avg_frame_time().as_secs_f64() * 1000.0,
            self.fps(),
            0, // glyph_cache.len() - needs reference
            self.cache_hit_rate()
        )
    }
}
```

### 6.3 Release Build Profiling Hooks

Create a lightweight `ReleasePerfStats` that can be optionally enabled:

```rust
#[cfg(feature = "profiling")]
struct ReleasePerfStats {
    frame_times: Vec<Duration>,
    start: Instant,
}
```

Add to `Cargo.toml`:
```toml
[features]
profiling = []
```

### 6.4 CI Integration

GitHub Actions workflow:
```yaml
- name: Run benchmarks
  run: cargo bench --features profiling
  
- name: Compare with baseline
  uses: benchmark-action/github-action-benchmark@v1
```

---

## 7. Criterion Integration Plan

### 7.1 Using Existing Infrastructure

The current `PerfStats` is tightly coupled to the render loop. For Criterion:

1. **Extract timing-independent logic** from `render_impl_with_perf()`
2. **Create testable render phases** as standalone functions
3. **Use `black_box`** to prevent optimization of measured code

### 7.2 Proposed Benchmark Module Structure

```
benches/
├── render_benchmarks.rs    # Full frame rendering
├── text_benchmarks.rs      # Text buffer operations  
├── glyph_benchmarks.rs     # Font rasterization
└── viewport_benchmarks.rs  # Scroll/viewport calculations
```

### 7.3 Cargo.toml Additions

```toml
[dev-dependencies]
criterion = { version = "0.5", features = ["html_reports"] }

[[bench]]
name = "render_benchmarks"
harness = false

[[bench]]
name = "text_benchmarks"
harness = false
```

---

## 8. Sample Benchmark Code

### benches/text_benchmarks.rs

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use token::model::AppModel;
use token::messages::{Msg, DocumentMsg, EditorMsg};
use token::update::update;

fn bench_insert_char(c: &mut Criterion) {
    let mut group = c.benchmark_group("insert_char");
    
    for size in [100, 1000, 10000, 100000] {
        group.bench_with_input(
            BenchmarkId::from_parameter(size),
            &size,
            |b, &size| {
                b.iter_batched(
                    || {
                        let mut model = AppModel::new(800, 600, None);
                        // Pre-populate buffer
                        let content = "x".repeat(size);
                        model.document.buffer = ropey::Rope::from_str(&content);
                        model
                    },
                    |mut model| {
                        update(&mut model, Msg::Document(DocumentMsg::Insert('a')));
                        black_box(model)
                    },
                    criterion::BatchSize::SmallInput,
                );
            },
        );
    }
    group.finish();
}

fn bench_delete_char(c: &mut Criterion) {
    let mut model = AppModel::new(800, 600, None);
    model.document.buffer = ropey::Rope::from_str(&"x".repeat(10000));
    
    c.bench_function("delete_char_10k", |b| {
        b.iter(|| {
            let mut m = model.clone();
            update(&mut m, Msg::Document(DocumentMsg::Backspace));
            black_box(m)
        });
    });
}

fn bench_cursor_movement(c: &mut Criterion) {
    let mut group = c.benchmark_group("cursor_movement");
    
    let mut model = AppModel::new(800, 600, None);
    let content = (0..1000).map(|i| format!("Line {}\n", i)).collect::<String>();
    model.document.buffer = ropey::Rope::from_str(&content);
    
    group.bench_function("move_down", |b| {
        b.iter(|| {
            let mut m = model.clone();
            for _ in 0..100 {
                update(&mut m, Msg::Editor(EditorMsg::MoveCursor(
                    token::messages::Direction::Down
                )));
            }
            black_box(m)
        });
    });
    
    group.finish();
}

criterion_group!(benches, bench_insert_char, bench_delete_char, bench_cursor_movement);
criterion_main!(benches);
```

### benches/glyph_benchmarks.rs

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use fontdue::{Font, FontSettings};
use std::collections::HashMap;

type GlyphCache = HashMap<(char, u32), (fontdue::Metrics, Vec<u8>)>;

fn bench_glyph_rasterization(c: &mut Criterion) {
    let font = Font::from_bytes(
        include_bytes!("../assets/JetBrainsMono.ttf") as &[u8],
        FontSettings::default(),
    ).unwrap();
    
    let font_size = 28.0; // 14.0 * 2.0 HiDPI
    
    c.bench_function("rasterize_single_glyph", |b| {
        b.iter(|| {
            let (metrics, bitmap) = font.rasterize(black_box('A'), font_size);
            black_box((metrics, bitmap))
        });
    });
    
    c.bench_function("rasterize_ascii_set", |b| {
        b.iter(|| {
            let mut cache = GlyphCache::new();
            for ch in ' '..='~' {
                let (metrics, bitmap) = font.rasterize(ch, font_size);
                cache.insert((ch, font_size.to_bits()), (metrics, bitmap));
            }
            black_box(cache)
        });
    });
}

fn bench_cache_lookup(c: &mut Criterion) {
    let font = Font::from_bytes(
        include_bytes!("../assets/JetBrainsMono.ttf") as &[u8],
        FontSettings::default(),
    ).unwrap();
    
    let font_size = 28.0;
    let mut cache = GlyphCache::new();
    
    // Pre-populate cache
    for ch in ' '..='~' {
        let (metrics, bitmap) = font.rasterize(ch, font_size);
        cache.insert((ch, font_size.to_bits()), (metrics, bitmap));
    }
    
    c.bench_function("cache_hit", |b| {
        b.iter(|| {
            black_box(cache.get(&('M', font_size.to_bits())))
        });
    });
}

criterion_group!(benches, bench_glyph_rasterization, bench_cache_lookup);
criterion_main!(benches);
```

---

## 9. Immediate Action Items

| Priority | Action | Effort |
|----------|--------|--------|
| 🔴 High | Add `criterion` to dev-dependencies | 5 min |
| 🔴 High | Create `benches/text_benchmarks.rs` skeleton | 30 min |
| 🟡 Medium | Instrument glyph cache hit/miss counting | 15 min |
| 🟡 Medium | Populate render phase timing fields | 1 hour |
| 🟢 Low | Add `--benchmark` headless mode | 2-4 hours |
| 🟢 Low | Create CI benchmark workflow | 1 hour |

---

## 10. Summary

The codebase has a solid foundation for performance monitoring:
- **Good:** `PerfStats` struct with rolling window, overlay rendering, F2 toggle
- **Partial:** Render phase timing fields exist but aren't used
- **Missing:** Criterion benchmarks, CI integration, export capability, headless mode

The overlay infrastructure in `overlay.rs` is well-designed and testable. The main gaps are in populating the existing timing fields and creating automated benchmark harnesses for regression testing.
