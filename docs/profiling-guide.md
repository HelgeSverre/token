# Profiling Guide for Token Editor

This guide covers deep performance analysis tools available on macOS for profiling the Token editor. It has been updated with clarifications about the `--profile` usage, Apple Silicon / macOS caveats, `xctrace` examples, sanitizer notes, and `cargo flamegraph` permissions. Follow the recommended workflow first, then consult the troubleshooting section if something doesn't behave as expected.

## Quick Start

Make sure your repo either defines a `profiling` Cargo profile or you build a release-like binary with debug symbols. See the "Cargo profile" section below for an example profile.

```bash
# Build with profiling symbols (release speed + debug info)
cargo build --profile profiling --bin token

# Run headless render benchmark
./target/profiling/profile_render --frames 500 --splits 3 --stats
```

## Recommended Workflow

When investigating performance issues, follow this workflow:

### Step 1: Headless Benchmark (Isolate Rendering)

First, rule out render code by running the headless profiler. The headless binary isolates rendering from windowing/event handling so you can determine whether slowness is in rendering.

```bash
cargo build --profile profiling --bin profile_render
./target/profiling/profile_render --frames 500 --splits 3 --lines 5000 --stats
```

If headless shows good FPS but the live app is slow, the issue is likely in event handling, input processing, or windowing rather than rendering.

### Step 2: Quick Live Profile with `sample` or `xctrace` (30 seconds)

macOS provides `sample` but on modern macOS / Apple Silicon `sample` can be inconsistent or missing. Prefer `xctrace` (Time Profiler) if `sample` is not available.

`sample` (older macOS; simple and fast):

```bash
# Start the app in background (build with profiling profile first)
./target/profiling/token samples/large_file.rs &
APP_PID=$!

# Sample for 30 seconds at ~1ms intervals
sample $APP_PID 30 -file /tmp/token-sample.txt

# Kill the app
kill $APP_PID
```

`xctrace` (recommended on modern macOS / Apple Silicon):

```bash
# Launch and record Time Profiler attached to a running PID for 30s
./target/profiling/token samples/large_file.rs &
APP_PID=$!

xcrun xctrace record --template "Time Profiler" --attach-pid $APP_PID --time-limit 30s -o /tmp/token-xctrace.trace

# Open /tmp/token-xctrace.trace in Instruments.app
```

The output shows:

- Call tree with sample counts per function
- Hot functions and heavy call paths
- Time breakdown by module and thread

Interpreting common patterns:

- High % in `mach_msg2_trap` — app is idle/waiting (good)
- High % in `CFRunLoop*` — event loop overhead
- High % in `render_*` or `TextPainter::draw` — rendering bottleneck
- High % in `poll` syscall — event loop spinning; prefer `WaitUntil` rather than `Poll`

### Step 3: Detailed Analysis with Instruments

Use Instruments (Time Profiler, Allocations, System Trace) for deeper analysis and visualization. `xcrun xctrace` can produce .trace files you open in Instruments.app.

```bash
# Time Profiler (CPU sampling)
xcrun xctrace record --template "Time Profiler" --launch ./target/profiling/token -- samples/large_file.rs -o /tmp/token-time.trace

# Allocations (memory)
xcrun xctrace record --template "Allocations" --launch ./target/profiling/token -- samples/large_file.rs -o /tmp/token-allocations.trace

# System Trace (thread scheduling, syscalls)
xcrun xctrace record --template "System Trace" --launch ./target/profiling/token -- samples/large_file.rs -o /tmp/token-system.trace
```

Open the .trace in Instruments.app for visualization, stack traces, and timeline analysis.

### Step 4: Symbolication (if needed)

If you see addresses instead of function names in a trace, symbolicate the trace. Ensure you pass the built binary or its dSYM directory.

```bash
xcrun xctrace symbolicate --input /path/to/Launch-xxx.trace --dsym ./target/profiling/token
```

Note: Instruments/xctrace will usually find symbols automatically if the binary has debug info and dSYM files were produced.

## Available Tools

### 1. Samply (Recommended - Firefox Profiler format)

Samply produces Firefox Profiler-compatible recordings (great flame graph / timeline UI).

```bash
# Install
cargo install samply

# Profile the app (build with profiling profile first)
cargo build --profile profiling --bin token
samply record ./target/profiling/token samples/large_file.rs

# Profile headless rendering
samply record ./target/profiling/profile_render --frames 1000 --splits 3 --lines 10000
```

Samply recordings can be opened in Firefox Profiler for flame graphs, call tree, and timeline views.

### 2. Instruments / xctrace (Apple)

Instruments (via GUI) and `xcrun xctrace` (CLI) are the most reliable tools on macOS — especially on Apple Silicon where some DTrace-based tools are restricted.

```bash
# Build with profiling profile
cargo build --profile profiling --bin token

# Time Profiler
xcrun xctrace record --template "Time Profiler" --launch ./target/profiling/token -- samples/large_file.rs -o /tmp/time.trace
```

Recommended templates:

- Time Profiler — CPU hot paths
- Allocations — allocation hotspots and growth
- Leaks — leak detection
- System Trace — scheduling, syscalls, I/O
- Metal System Trace — GPU profiling

### 3. DHAT (Heap profiler)

DHAT is useful for allocation-site analysis. It requires your project to enable DHAT support.

```bash
# Run with DHAT enabled (project must support this feature)
cargo run --features dhat-heap --release -- samples/large_file.rs

# Produces dhat-heap.json — view with:
# https://nnethercote.github.io/dh_view/dh_view.html
```

### 4. cargo-flamegraph / Flamegraph

`cargo-flamegraph` generates an SVG flamegraph using `dtrace` on macOS. On modern macOS (and especially Apple Silicon), `dtrace`-based tooling has additional restrictions.

```bash
# Install (may require privileges)
cargo install flamegraph

# Generate flame graph (may need sudo or codesigning depending on macOS version)
sudo cargo flamegraph --profile profiling --bin token

# Result: flamegraph.svg
```

Caveats:

- Recent macOS releases / SIP may restrict dtrace; you may need elevated permissions or codesign the binary with a special entitlements profile. When `cargo flamegraph` fails due to dtrace permissions, prefer `samply` or `xctrace` instead.

### 5. Criterion Benchmarks

Use Criterion for microbenchmarks with statistical analysis.

```bash
# Run all benchmarks
cargo bench

# Run specific groups
cargo bench hot_paths
cargo bench main_loop
cargo bench rendering

# Save baseline and compare later
cargo bench -- --save-baseline before
# ... make changes ...
cargo bench -- --baseline before
```

Results are saved under `target/criterion/`.

### 6. Divan Benchmarks

If configured, Divan benchmarks provide allocation counting and other insights.

```bash
# Run divan benchmarks (if configured)
cargo bench --bench hot_paths
```

## Profiling Scenarios

### CPU Hot Paths

```bash
# Quick using samply
samply record ./target/profiling/token samples/large_file.rs

# Or Time Profiler with xctrace
xcrun xctrace record --template "Time Profiler" --launch ./target/profiling/token samples/large_file.rs -o /tmp/time.trace
```

### Memory Allocations

```bash
# Quick: DHAT
cargo run --features dhat-heap --release -- samples/large_file.rs

# Detailed: Instruments Allocations
xcrun xctrace record --template "Allocations" --launch ./target/profiling/token -- samples/large_file.rs -o /tmp/alloc.trace
```

### Render Performance

```bash
# Headless benchmark
./target/profiling/profile_render --frames 1000 --splits 3 --lines 10000 --stats

# Profile headless render
samply record ./target/profiling/profile_render --frames 1000 --splits 3
```

### Multi-split & Event Loop Analysis

```bash
# Multi-split
./target/profiling/profile_render --frames 500 --splits 4 --lines 20000 --include-csv --stats

# Event loop / scheduling - use System Trace
xcrun xctrace record --template "System Trace" --time-limit 10s --launch ./target/profiling/token -o /tmp/system.trace
```

## Debug Overlay (F2)

In debug builds, press F2 to toggle the in-app performance overlay. Run a debug build to use this:

```bash
cargo run -- samples/large_file.rs
# Press F2 to toggle overlay during the run
```

The overlay shows:

- Frame time and FPS
- Per-phase render breakdown
- Glyph cache statistics and simple sparklines

## Valgrind Alternative on macOS and Sanitizers

Valgrind is not practical on modern macOS / ARM64. Prefer Instruments and DHAT. AddressSanitizer (ASan) and LeakSanitizer require nightly and have platform limitations.

Sanitizer notes:

```bash
# AddressSanitizer / LeakSanitizer notes
# Requires Rust nightly and unstable flags
cargo +nightly run -Zsanitizer=address -- samples/large_file.rs

# LeakSanitizer (nightly)
cargo +nightly run -Zsanitizer=leak -- samples/large_file.rs
```

Caveats:

- ASan/LSan support on macOS is limited on Apple Silicon (arm64). You may find tools unreliable or unsupported. When sanitizers are unavailable, use Instruments (Allocations/Leaks), DHAT, or manual inspection.
- Use `cargo +nightly` when invoking `-Z` flags.

## Profile-Guided Optimization (PGO)

PGO steps (LLVM-style):

```bash
# Step 1: Build instrumented binary
RUSTFLAGS="-Cprofile-generate=/tmp/pgo-data" cargo build --release

# Step 2: Run representative workloads to collect data
./target/release/token samples/large_file.rs &
# Interact with the app to exercise hot paths, then quit

# Step 3: Merge profile data
llvm-profdata merge -o /tmp/pgo-data/merged.profdata /tmp/pgo-data

# Step 4: Build optimized binary using merged data
RUSTFLAGS="-Cprofile-use=/tmp/pgo-data/merged.profdata" cargo build --release
```

## Cargo profile: `profiling` example

If your repo doesn't have a `profiling` profile, add one to `Cargo.toml`. This produces optimized builds with debug info for better symbolication.

```toml
[profile.profiling]
inherits = "release"
debug = true         # keep debug info for symbolication
opt-level = 3        # optimize
lto = false
debug-assertions = false
overflow-checks = false
codegen-units = 1
```

Alternatively you can use `--release` and set `debug = true` for `profile.release` if you prefer fewer profiles.

## Comparing Performance

```bash
# Save baseline
cargo bench -- --save-baseline before

# After changes...
cargo bench -- --baseline before
```

## Tips & Best Practices

1. Always use `--profile profiling` (or an equivalent release build with debug info) for accurate stacks in profilers.
2. Close other apps to reduce noise.
3. Run multiple times; the first run may have cold caches.
4. Use `--stats` on `profile_render` for quick summaries.
5. Focus on hot paths: Pareto principle often applies (80/20).
6. If headless is fast but live is slow, suspect event loop/input/windowing issues.
7. High idle % in samples is good — a responsive app sleeps when idle.

## Common Issues & Solutions

| Symptom                   | Likely Cause                 | Solution                                                                     |
| ------------------------- | ---------------------------- | ---------------------------------------------------------------------------- |
| Low FPS but headless fast | Event loop spinning          | Use `ControlFlow::WaitUntil` instead of `Poll`; only redraw when needed      |
| High CPU while idle       | Constant redraws             | Only call `request_redraw` on state change                                   |
| Memory growth over time   | Per-frame allocations        | Reuse buffers, avoid per-frame heap allocations, prefer `Cow<str>` or slices |
| Stuttery scrolling        | Allocations / GC-like pauses | Profile allocations and reuse temporaries                                    |

## Troubleshooting

If a tool fails or results look odd, try these checks:

- **No symbols / unreadable stacks**
  - Ensure you built with the `profiling` profile (or `--release` with `debug = true`).
  - Verify the dSYM is present (Xcode or `dsymutil` can help).
  - For `xctrace`, open the .trace file in Instruments.app; Instruments often symbolicates automatically if debug info is present.

- **`xctrace`/Instruments shows no samples or empty call stacks**
  - The binary might be stripped or missing debug info.
  - Try re-building with `debug = true` for the profile you used.
  - Confirm the process ID is correct and that the process was active during recording.

- **`cargo flamegraph` or dtrace permission errors**
  - Modern macOS restricts DTrace; you may need elevated privileges (`sudo`) or codesign the binary with appropriate entitlements.
  - When dtrace is blocked, prefer `samply` or `xctrace`.

- **`sample` missing or not working**
  - `sample` is deprecated on some systems. Use `xcrun xctrace` / Instruments instead.

- **Sanitizers fail or don't run on Apple Silicon**
  - ASan/LSan require nightly and are not fully supported on arm64 macOS. Use Instruments (Allocations/Leaks) or DHAT instead.

- **Permissions / SIP / codesign problems**
  - Some low-level tracing tools require specific system permissions or temporary SIP adjustments. Prefer Apple-supported `xctrace` and Instruments when possible, as they are supported and do not require disabling SIP.

## Quick Troubleshooting Checklist

1. Rebuild with `cargo build --profile profiling --bin token` (or add the profile to Cargo.toml).
2. Use `xcrun xctrace` to record Time Profiler and open the .trace in Instruments.
3. If `cargo flamegraph` errors, try `samply` or `xctrace`.
4. If symbolication is missing, ensure debug info/dSYM is produced for the binary.
5. If things still fail, collect small reproducible steps and capture logs/trace files — those make root cause analysis much easier.

## Example Healthy Profile (30s sample)

```text
Total samples: ~26,000 (30s @ 1ms)

mach_msg2_trap        77%   # Idle/waiting (good!)
CFRunLoopDoObservers  21%   # Event handling
render_text_area       1%   # Actual work
TextPainter::draw    <1%    # Glyph blitting
```

If `mach_msg2_trap` is low or absent, your event loop is likely spinning and consuming CPU.