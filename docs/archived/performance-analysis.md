# Performance Analysis Report v2

## Executive Summary

After a comprehensive performance optimization pass, the Token editor now runs efficiently with proper event loop sleeping and reduced allocations. The main issue—**7 FPS in multi-split scenarios while headless benchmarks showed ~380 FPS**—was caused by `ControlFlow::Poll` spinning the event loop continuously.

### Final Performance Profile (30-second live session)

| Category | Time | Notes |
|----------|------|-------|
| **Idle/Waiting** | 77.6% | App sleeping (mach_msg2_trap) - correct behavior |
| **Event Handling** | 21.5% | CFRunLoop/winit callbacks |
| **Rendering** | ~0.9% | Actual draw code |

**Memory:** 93MB footprint, 180MB peak

This profile confirms the app is now idle most of the time and only wakes for input, timers, or async messages.

## Root Cause: Event Loop Spinning

The critical issue was `ControlFlow::Poll` in the event loop's `about_to_wait` handler:

```rust
// BEFORE: Spins CPU constantly, ~100% usage when idle
event_loop.set_control_flow(ControlFlow::Poll);
```

**Fix applied in `src/runtime/app.rs`:**

```rust
// AFTER: Sleep until next cursor blink, wake immediately for input
let blink_interval = Duration::from_millis(500);
let next_blink = self.last_tick + blink_interval;
event_loop.set_control_flow(ControlFlow::WaitUntil(next_blink));
```

**Impact:**
- Idle CPU: ~100% → ~0%
- Multi-split FPS: ~7 → 60 (VSync limited)
- Event handling remains responsive (input/async messages wake immediately)

## Optimization Summary

### Phase 1: Word Navigation (Completed Previously)

| Operation | Before | After | Speedup |
|-----------|--------|-------|---------|
| `word_navigation_sequence` (50+50 ops) | 67ms | 155µs | **~430x** |
| `word_start_before` (100K lines) | 8.8ms | 1.5ms | **~6x** |
| `word_end_after` (100K lines) | 8.9ms | 1.5ms | **~6x** |
| `realistic_word_operations` | 1.35ms | 25µs | **~54x** |

**Changes:**
- `word_start_before`/`word_end_after`: Use ropey's `Chars` iterator directly instead of collecting to `String` then `Vec<char>`
- `move_word_left`/`move_word_right`: Use `char_at()` instead of `Vec<char>` collection
- `last_non_whitespace_column`: Iterate in reverse without allocation
- `RopeBuffer::line()`: Avoid double allocation when trimming newlines

### Phase 2: Multi-Split Rendering (Completed Previously)

**Problem:** With 3+ splits open, per-line allocations multiplied: 3 splits × 35 lines × 4 allocations = **420+ allocations per frame**.

**Fixes:**
1. `Document::get_line_cow()` → returns `Cow<str>`, zero allocation when line is contiguous
2. `expand_tabs_for_display()` → returns `Cow<str>`, no allocation when no tabs
3. `display_text_buf` String buffer → reused across all lines (clear + append)
4. `adjusted_tokens` Vec buffer → reused across all lines

### Phase 3: Event Loop (Completed This Session)

| Issue | Before | After |
|-------|--------|-------|
| Event loop mode | `ControlFlow::Poll` (spinning) | `ControlFlow::WaitUntil` (sleeping) |
| Idle CPU usage | ~100% | ~0% |
| Multi-split FPS | ~7 | 60 (VSync) |

### Phase 4: Debug Overlay HiDPI (Completed This Session)

Fixed hard-coded pixel values in `src/runtime/perf.rs` that didn't scale on Retina displays:

- Overlay dimensions scaled based on `line_height` ratio
- Chart widths derived from approximate character width
- Stacked bar calculation fixed with `saturating_sub` to prevent overflow panic

## Profiling Workflow

See [profiling-guide.md](../profiling-guide.md) for detailed commands. Quick reference:

```bash
# Build with profiling symbols
cargo build --profile profiling

# Headless benchmark (isolates render from windowing)
./target/profiling/profile_render --frames 500 --splits 3 --stats

# Live app profiling with sample (quick 30-second analysis)
./target/profiling/token samples/large_file.rs &
sample $! 30 -file /tmp/token-sample.txt

# Instruments Time Profiler (detailed)
xcrun xctrace record --template "Time Profiler" --time-limit 60s \
  --launch ./target/profiling/token samples/large_file.rs
```

## Future Optimizations (Low Priority)

These are not blocking but could provide further improvements:

1. **Dirty rect tracking**: Skip re-rendering unchanged regions
2. **Glyph rasterization caching**: Cache across frames more aggressively
3. **GPU rendering**: Consider wgpu for complex multi-split scenarios

## Benchmark Commands

```bash
cargo bench hot_paths      # Word navigation benchmarks
cargo bench main_loop      # Main loop benchmarks
cargo bench rendering      # Rendering benchmarks
```

## Archived

Previous detailed analysis (including allocation counts and code locations) archived at [performance-analysis-v1.md](performance-analysis-v1.md).
