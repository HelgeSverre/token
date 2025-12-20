# Profiling Guide for Token Editor

This guide covers deep performance analysis tools available on macOS for profiling the Token editor.

## Quick Start

```bash
# Build with profiling symbols (release speed + debug info)
cargo build --profile profiling

# Run headless render benchmark
./target/profiling/profile_render --frames 500 --splits 3 --stats
```

## Recommended Workflow

When investigating performance issues, follow this workflow:

### Step 1: Headless Benchmark (Isolate Rendering)

First, rule out render code by running the headless profiler:

```bash
cargo build --profile profiling --bin profile_render
./target/profiling/profile_render --frames 500 --splits 3 --lines 5000 --stats
```

If headless shows good FPS (~300+) but live app is slow, the issue is in event handling, not rendering.

### Step 2: Quick Live Profile with `sample` (30 seconds)

Apple's `sample` command is the fastest way to get a CPU profile of the live app:

```bash
# Start the app in background
./target/profiling/token samples/large_file.rs &
APP_PID=$!

# Sample for 30 seconds at 1ms intervals
sample $APP_PID 30 -file /tmp/token-sample.txt

# Kill the app
kill $APP_PID
```

The output shows:
- **Call tree** with sample counts per function
- **Heavy/hot** functions highlighted
- Time breakdown by module

**Interpreting results:**

| Pattern | Meaning |
|---------|---------|
| High % in `mach_msg2_trap` | App is idle/waiting (good!) |
| High % in `CFRunLoop*` | Event loop overhead |
| High % in `render_*` or `TextPainter::draw` | Rendering is the bottleneck |
| High % in `poll` syscall | Event loop spinning (bad - use WaitUntil) |

### Step 3: Detailed Analysis with Instruments

For deeper investigation, use Instruments Time Profiler:

```bash
xcrun xctrace record --template "Time Profiler" --time-limit 60s \
  --launch ./target/profiling/token samples/large_file.rs

# Output: Launch-xxx.trace file
# Open in Instruments.app for visualization
```

### Step 4: Symbolication (if needed)

If stack traces show addresses instead of function names:

```bash
xcrun xctrace symbolicate --input Launch-xxx.trace --dsym ./target/profiling/token
```

## Available Tools

### 1. Samply (Recommended - Like Firefox Profiler)

**Samply** is a sampling profiler that outputs to the Firefox Profiler format - excellent for visualizing call stacks and flame graphs.

```bash
# Install
cargo install samply

# Profile the actual app
cargo build --profile profiling
samply record ./target/profiling/token samples/large_file.rs

# Profile headless rendering
samply record ./target/profiling/profile_render --frames 1000 --splits 3 --lines 10000
```

Opens automatically in Firefox Profiler with:
- Flame graphs
- Call tree
- Timeline view
- Stack chart

### 2. Instruments (Apple's Native Profiler)

**Instruments** is Apple's comprehensive profiling suite - the most powerful tool on macOS.

```bash
# Build with profiling profile
cargo build --profile profiling

# Time Profiler (CPU sampling)
xcrun xctrace record --template "Time Profiler" --launch ./target/profiling/token

# Allocations (memory profiling)
xcrun xctrace record --template "Allocations" --launch ./target/profiling/token

# System Trace (comprehensive - CPU, I/O, scheduling)
xcrun xctrace record --template "System Trace" --launch ./target/profiling/token
```

Or use the GUI:
1. Open Instruments.app (Xcode → Open Developer Tool → Instruments)
2. Choose a template (Time Profiler, Allocations, etc.)
3. Select target: `./target/profiling/token`

**Templates for different investigations:**
- **Time Profiler**: CPU hot paths, function timing
- **Allocations**: Memory allocation patterns, leaks
- **Leaks**: Memory leak detection
- **System Trace**: Thread scheduling, syscalls, I/O
- **Metal System Trace**: GPU profiling (if applicable)

### 3. DHAT (Heap Profiling)

Built-in heap profiler for allocation analysis:

```bash
# Run with DHAT enabled
cargo run --features dhat-heap --release -- samples/large_file.rs

# Generates dhat-heap.json - view at:
# https://nnethercote.github.io/dh_view/dh_view.html
```

Shows:
- Allocation hot spots
- Allocation sizes
- Allocation lifetimes
- Total bytes allocated per call site

### 4. Cargo Flamegraph

Generate SVG flame graphs directly:

```bash
# Install (requires dtrace permissions on macOS)
cargo install flamegraph

# Generate flame graph
sudo cargo flamegraph --profile profiling -- samples/large_file.rs

# Opens flamegraph.svg in browser
```

**Note**: On macOS, you may need to:
```bash
# Disable SIP temporarily or use sudo
sudo cargo flamegraph --profile profiling --bin token
```

### 5. Criterion Benchmarks

Micro-benchmarks with statistical analysis:

```bash
# Run all benchmarks
cargo bench

# Run specific benchmark groups
cargo bench hot_paths
cargo bench main_loop
cargo bench rendering

# Generate HTML reports
cargo bench -- --save-baseline main
# ... make changes ...
cargo bench -- --baseline main
```

Reports saved to `target/criterion/`.

### 6. Divan Benchmarks

Alternative benchmarker with allocation counting:

```bash
# Run divan benchmarks (if configured)
cargo bench --bench hot_paths
```

## Profiling Scenarios

### CPU Hot Paths

```bash
# Use samply for quick analysis
samply record ./target/profiling/token samples/large_file.rs

# Or Instruments for detailed analysis
xcrun xctrace record --template "Time Profiler" --launch ./target/profiling/token samples/large_file.rs
```

### Memory Allocations

```bash
# Quick: Use DHAT
cargo run --features dhat-heap --release -- samples/large_file.rs

# Detailed: Use Instruments Allocations
xcrun xctrace record --template "Allocations" --launch ./target/profiling/token
```

### Render Performance

```bash
# Headless benchmark (isolates render logic from windowing)
./target/profiling/profile_render --frames 1000 --splits 3 --lines 10000 --stats

# Profile the headless benchmark
samply record ./target/profiling/profile_render --frames 1000 --splits 3
```

### Multi-Split Performance

```bash
# Test with multiple splits and large files
./target/profiling/profile_render --frames 500 --splits 4 --lines 20000 --include-csv --stats
```

### Event Loop Analysis

```bash
# System Trace shows thread scheduling, event handling
xcrun xctrace record --template "System Trace" --time-limit 10s --launch ./target/profiling/token
```

## Debug Overlay (F2)

In debug builds, press **F2** to toggle the performance overlay showing:
- Frame time (ms) and FPS
- Render phase breakdown (Clear, Text, Present, etc.)
- Glyph cache statistics
- Sparkline history for each phase

```bash
# Run debug build to use overlay
cargo run -- samples/large_file.rs
# Press F2 to toggle overlay
```

## Valgrind Alternative on macOS

Valgrind doesn't work on modern macOS (ARM64). Alternatives:

| Tool | Purpose | Command |
|------|---------|---------|
| **Instruments/Allocations** | Memory profiling | `xcrun xctrace record --template Allocations` |
| **Instruments/Leaks** | Leak detection | `xcrun xctrace record --template Leaks` |
| **DHAT** | Heap profiling | `cargo run --features dhat-heap` |
| **AddressSanitizer** | Memory errors | `RUSTFLAGS="-Zsanitizer=address" cargo run` |
| **LeakSanitizer** | Leak detection | `RUSTFLAGS="-Zsanitizer=leak" cargo run` (nightly) |

## Profile-Guided Optimization (PGO)

For maximum performance in release builds:

```bash
# Step 1: Build instrumented binary
RUSTFLAGS="-Cprofile-generate=/tmp/pgo-data" cargo build --release

# Step 2: Run representative workloads
./target/release/token samples/large_file.rs &
# ... perform typical operations, then quit

# Step 3: Merge profile data
llvm-profdata merge -o /tmp/pgo-data/merged.profdata /tmp/pgo-data

# Step 4: Build optimized binary
RUSTFLAGS="-Cprofile-use=/tmp/pgo-data/merged.profdata" cargo build --release
```

## Comparing Performance

```bash
# Establish baseline
cargo bench -- --save-baseline before

# Make changes...

# Compare against baseline
cargo bench -- --baseline before

# View comparison in target/criterion/
```

## Tips

1. **Always use `--profile profiling`** for accurate stack traces in profilers
2. **Close other apps** when profiling to reduce noise
3. **Run multiple times** - first run may have cold cache effects
4. **Use `--stats` flag** on profile_render for quick timing summary
5. **Focus on hot paths** - 80% of time is usually in 20% of code
6. **Headless vs live discrepancy** usually indicates event loop issues, not render code
7. **High idle % is good** - a well-behaved app should show 70%+ in `mach_msg2_trap` when not actively rendering

## Common Issues & Solutions

| Symptom | Likely Cause | Solution |
|---------|-------------|----------|
| Low FPS but headless fast | Event loop spinning | Use `ControlFlow::WaitUntil` instead of `Poll` |
| High CPU when idle | Constant redraws | Only request_redraw when state changes |
| Memory growth over time | Per-frame allocations | Reuse buffers, return `Cow<str>` instead of `String` |
| Stuttery scrolling | GC pauses or allocations | Profile with Allocations template |

## Example Profile Output (Healthy App)

A well-optimized app should show this pattern in a 30-second sample:

```
Total samples: ~26,000 (30s @ 1ms)

mach_msg2_trap        77%   # Idle/waiting (good!)
CFRunLoopDoObservers  21%   # Event handling
render_text_area       1%   # Actual work
TextPainter::draw    <1%    # Glyph blitting
```

If `mach_msg2_trap` is low or absent, the event loop is spinning.
