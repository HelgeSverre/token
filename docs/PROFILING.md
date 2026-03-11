# Profiling Guide

Performance analysis tools and workflows for the Token editor on macOS.

## Quick Start

```bash
# Build with debug symbols for profiling
make build-prof

# Headless render benchmark (isolates rendering from windowing)
./target/profiling/profile_render --frames 500 --splits 3 --stats

# Interactive profiling with samply (opens Firefox Profiler)
make profile-samply

# CPU flamegraph
make flamegraph

# Heap profiling with DHAT
make profile-memory

# Chrome trace with named render stages (open in Perfetto)
make profile-chrome
```

## Recommended Workflow

### Step 1: Headless Benchmark

Rule out rendering as the bottleneck. The headless binary runs the render pipeline without windowing or event handling.

```bash
make build-prof
./target/profiling/profile_render --frames 500 --splits 3 --lines 5000 --stats
```

If headless is fast but the live app is slow, the issue is in event handling, input, or windowing.

### Step 2: CPU Profiling

**Samply** (recommended — produces Firefox Profiler recordings):

```bash
make profile-samply
# Or manually:
samply record ./target/profiling/token samples/large_file.rs
```

**xctrace / Instruments** (Apple Silicon friendly):

```bash
./target/profiling/token samples/large_file.rs &
APP_PID=$!
xcrun xctrace record --template "Time Profiler" --attach-pid $APP_PID --time-limit 30s -o /tmp/token.trace
kill $APP_PID
# Open /tmp/token.trace in Instruments.app
```

### Step 3: Memory Profiling

```bash
# DHAT heap profiler (opens web viewer)
make profile-memory

# Instruments Allocations
xcrun xctrace record --template "Allocations" --launch ./target/profiling/token -- samples/large_file.rs -o /tmp/alloc.trace
```

### Step 4: Chrome Trace (Named Stages)

When you need to see exactly which render stages cost time across many frames, capture a Chrome trace. Every `PerfStats` stage appears as a named span in the timeline.

```bash
make profile-chrome
# Interact with the editor, then quit — Perfetto opens automatically
# Drag-and-drop token-trace.json into the browser
```

See [Chrome Trace Export (Perfetto)](#chrome-trace-export-perfetto) below for full details.

### Step 5: Debug Overlay (F2)

In debug builds, press F2 for a live performance overlay showing frame time, per-stage breakdown, glyph cache stats, and sparklines. Note: the overlay forces full redraws while visible, so it perturbs its own measurements.

```bash
make dev
# Press F2 in the running editor
```

## Benchmarks

All benchmarks use **Divan** with allocation tracking via `divan::AllocProfiler`.

```bash
make bench               # Run all benchmarks
make bench-rope          # Rope insert/delete/navigate
make bench-render        # Buffer ops, alpha blending, line rendering
make bench-glyph         # Font rasterization, cache hit patterns
make bench-loop          # Main update→render cycle
make bench-search        # Find/replace operations
make bench-layout        # Text measurement, viewport calculation
make bench-syntax        # Syntax highlighting (all 20 languages)
```

Individual bench files live in `benches/`. Run a specific one:

```bash
cargo bench --bench hot_paths
```

## Internal Instrumentation

### Stage-Based Perf System (`src/perf.rs`)

The editor tracks 28 named render stages. In debug builds, `PerfStats` records per-stage timing histories (60 frames). In release builds, all perf code compiles away.

Key APIs:

- `PerfStats::measure_stage(stage)` — returns a `TimerGuard` RAII wrapper
- `PerfStats::time_stage(stage)` — alternative timing API
- Stages cover the full pipeline: `BuildPlan`, `Clear`, `TabBar`, `TextBackground`, `TextGlyphs`, `Gutter`, `Scrollbars`, `Sidebar`, `StatusBar`, `SurfacePresent`, etc.

### Structured Logging (`src/tracing.rs`)

Uses the `tracing` crate with console + file output. File logs rotate daily to `~/.config/token-editor/logs/token.log`.

- `RUST_LOG` — console filter (e.g., `RUST_LOG=token=debug`)
- `TOKEN_FILE_LOG` — file-specific filter (falls back to `RUST_LOG`)

## Chrome Trace Export (Perfetto)

The editor's 28 internal render stages (`PerfStats`) can emit `tracing` spans visible to external profilers. This is controlled by Cargo feature flags and is off by default — zero overhead in normal builds.

### Usage

The quickest way to capture a trace:

```bash
make profile-chrome
```

This builds a release binary with `--features profile-chrome`, opens the token codebase as a workspace (a realistic workload with sidebar, file tree, syntax highlighting), and writes `token-trace.json` when the editor exits. Perfetto UI opens automatically in your browser — drag-and-drop `token-trace.json` into it.

For a custom file or directory:

```bash
cargo build --release --features profile-chrome --bin token
./target/release/token path/to/project/
# Quit the editor (Cmd+Q). The trace flushes on exit.
open https://ui.perfetto.dev
```

### Viewing the Trace

Once Perfetto UI is open:

1. Click **Open trace file** (or drag-and-drop `token-trace.json`)
2. Use the timeline to zoom into individual frames
3. Use Ctrl+F to search by stage name (e.g., `text_glyphs`)

What you'll see:

- **`frame`** — a top-level span wrapping each render frame
- **`render_stage`** — nested spans for each of the 28 named stages, with a `stage` field like `text_glyphs`, `build_plan`, `sidebar`, `status_bar`, etc.

Use Perfetto's search (Ctrl+F) to filter by stage name, e.g., `text_glyphs`, to see only that stage across all frames.

### Combining with Samply

You can profile with samply and Chrome tracing simultaneously to get both CPU stacks and named stage markers:

```bash
cargo build --profile profiling --features profile-chrome --bin token
samply record ./target/profiling/token samples/large.txt
# After quitting: samply opens Firefox Profiler, and token-trace.json is on disk
```

### Profiling Debug Builds

Chrome tracing also works in debug builds. This is useful for correlating stage timings with the F2 overlay:

```bash
cargo build --features profile-chrome
./target/debug/token samples/sample_code.rs
# Press F2 to see the live overlay; quit to write token-trace.json
```

### Feature Flags

| Feature | Effect |
|---------|--------|
| `profile-tracing` | Emit `tracing::info_span!` inside `PerfStats::measure_stage` / `time_stage` |
| `profile-chrome` | Implies `profile-tracing`; adds a `tracing-chrome` subscriber that writes `token-trace.json` |

`profile-tracing` alone emits spans but doesn't write a file — useful if you add your own subscriber (e.g., Tracy). `profile-chrome` is the batteries-included option.

### How It Works

The spans are emitted inside `PerfStats` methods, so all existing call sites (`perf.measure_stage(PerfStage::TabBar, || { ... })`) automatically appear in the trace with no code changes. The `frame` span is opened in `start_frame()` and closed in `record_frame_time()`.

In release builds without the feature, `PerfStats` is a zero-size struct and all methods are `#[inline(always)]` no-ops — the compiler eliminates everything.

### Limitations

4 text sub-stages (`TextBackground`, `TextDecorations`, `TextGlyphs`, `TextCursors`) use manual `record_stage_elapsed` calls in `editor_text.rs` rather than `measure_stage`. These record timing to the F2 overlay but don't emit tracing spans. They can be migrated later.

### Future: Tracy Support

Tracy can be added as another feature flag (`profile-tracy = ["profile-tracing", "dep:tracing-tracy", "dep:tracy-client"]`) without touching `PerfStats` — the spans are already emitted, only a new subscriber layer is needed.

## Cargo Profiles

| Profile     | Purpose                          | Key Settings                                            |
| ----------- | -------------------------------- | ------------------------------------------------------- |
| `dev`       | Fast compile                     | opt-level=0, line-tables-only debug                     |
| `debugging` | Full debug info                  | Inherits dev, debug=true                                |
| `release`   | Local testing                    | opt-level=3, lto=thin, panic=abort                      |
| `profiling` | Release speed + debug symbols    | Inherits release, debug=true, lto=false                 |
| `dist`      | Distribution (max optimization)  | Inherits release, lto=fat, codegen-units=1, strip=true  |

Always use `--profile profiling` (or `make build-prof`) for profiling. The `dist` profile strips symbols.

## Profile-Guided Optimization (PGO)

Not automated yet. Manual steps:

```bash
# 1. Build instrumented binary
RUSTFLAGS="-Cprofile-generate=/tmp/pgo-data" cargo build --release

# 2. Run representative workloads, then quit
./target/release/token samples/large_file.rs

# 3. Merge and rebuild
llvm-profdata merge -o /tmp/pgo-data/merged.profdata /tmp/pgo-data
RUSTFLAGS="-Cprofile-use=/tmp/pgo-data/merged.profdata" cargo build --release
```

## Interpreting Results

**Healthy idle profile** (~30s sample):

```text
mach_msg2_trap        77%   # Idle/waiting (good!)
CFRunLoopDoObservers  21%   # Event handling
render_text_area       1%   # Actual work
```

**Common patterns:**

| Pattern                         | Meaning                  | Action                                           |
| ------------------------------- | ------------------------ | ------------------------------------------------ |
| High `mach_msg2_trap`           | App is idle (good)       | Normal                                           |
| Low `mach_msg2_trap`            | Event loop spinning      | Use `WaitUntil` instead of `Poll`                |
| High CPU while idle             | Unnecessary redraws      | Only `request_redraw` on state change            |
| Memory growth over time         | Per-frame allocations    | Reuse buffers, avoid per-frame heap allocations  |
| Headless fast, live slow        | Not a rendering issue    | Investigate event handling / windowing            |

## Troubleshooting

- **No symbols in traces** — Rebuild with `make build-prof`. Check dSYM is present.
- **`cargo flamegraph` permission errors** — macOS SIP restricts dtrace. Use `make profile-samply` instead.
- **Empty xctrace stacks** — Binary may be stripped. Ensure `debug = true` in the cargo profile.
- **Sanitizers fail on Apple Silicon** — ASan/LSan are limited on arm64 macOS. Use Instruments or DHAT instead.
