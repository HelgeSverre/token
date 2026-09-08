# Benchmarking & Profiling Guide

This document describes how to run benchmarks and profile the Token editor.

## Reports

- [2026-09-08: recency context refresh](2026-09-08-recency-refresh.md) — exact
  unchanged-snapshot reuse, CPU sample, edited-path control and allocation tradeoff.
- [2026-09-08: ASCII literal Find](2026-09-08-find-literals.md) — gated literal
  matching, before/after CPU timings and constructor allocation tradeoff.
- [2026-09-08: Find overview projection](2026-09-08-find-overview.md) — incremental
  Rope chunk traversal, before/after results and a rejected slower prototype.
- [2026-09-08: cold Find](2026-09-08-cold-find.md) — dense/sparse/absent scans,
  worker computation, independent repeat and native CPU stack samples.
- [2026-09-08: completion response ownership](2026-09-08-completion-responses.md) —
  deferred JSON serialization, before/after allocations and independent repeat.
- [2026-09-08: workspace retrieval](2026-09-08-workspace-retrieval.md) — optimized
  warm declaration-ranking costs and allocations; collection and inference excluded.
- [2026-09-08: Settings scrolling](2026-09-08-settings-scroll.md) — debug versus
  optimized CPU costs, shared backdrop dimming and the opaque-panel follow-up.
- [2026-09-07: current optimized working tree](2026-09-07-current.md) — fresh
  edit/history, Find, CPU-rendering and completion measurements, with raw output
  and a separately identified later dropdown-keystroke refresh.
- [2026-08-11: historical baseline](2026-08-11-baseline.md) — preserved startup,
  search and syntax experiments; includes debug measurements and older fixtures.
- [September refactoring audit](../dev/refactoring-audit-2026-09-06.md) — detailed
  investigations, controlled before/after comparisons and native-check limits.

Keep future dated reports and their data in this directory. Update this index;
do not overwrite historical measurements or put new baseline files in `docs/`.

## Quick Reference

```bash
# Available workflows and suites
just --list
just bench
just bench-search
just bench-search find_ascii_literal_vs_regex
just bench-syntax
just bench-loop

# Warm production-path probes (setup and window presentation excluded)
just profile-workloads
just profile-workloads find
just profile-workloads find-cold
# Twelve-second CPU sampling windows, with the child PID printed at startup
just profile-workloads sample-find-cold
just profile-workloads sample-find-worker
just profile-workloads edit-history
just profile-workloads insertions
just profile-workloads replacements
just profile-workloads settings
# Explicitly unoptimized, for debug-only diagnosis
just profile-workloads-debug settings

# Completion suite; this recipe forwards filters/options to Divan
just bench-completion
just bench-completion recency
just bench-completion sample-recency-refresh
just bench-completion workspace_retrieval_rank
just bench-completion ghost

# Profilers (interactive recipes launch the editor)
just build-prof
just profile-render
just profile-memory
```

These benchmark recipes use Cargo's optimized bench profile. Native profiling
uses separate release/profiling builds as specified in each recipe; `dist` has
different optimization settings again. Record the actual profile and features.
Use `CARGO_BUILD_JOBS=1` before a recipe when build memory or disk headroom is
tight. Run timed workloads serially, after compilation, and record background
load. `just workspace` is a debug build; the F2 overlay forces full redraw while
visible. Neither is evidence for release-equivalent frame rates.

## Benchmark Suites

### completion

Run `just bench-completion` for the existing dropdown and inline-filter probes,
or `just bench-completion recency` for idle-context stages. The recipe forwards
arguments to Divan, so individual names can also be selected.

Recency probes compile the runtime's private implementation into the benchmark;
there is no duplicate algorithm or public profiling API. They measure first idle
fill, full-ring refresh after saves, unchanged-state observation and request
attachment separately. Fixtures contain 8 or 32 open buffers with distinct,
identifier-heavy 8 KiB snippets. Setup is outside timed regions; no user config,
file reads, network, model inference or window presentation is measured.
The near-duplicate refresh case shares about 94% of tokens (Jaccard similarity
just below 90%), with shared tokens sorting first. Setup asserts the retained
count. Unchanged refresh can reuse exact snapshots; `recency_idle_refresh_edited`
changes every payload before queuing saves and still exercises long comparisons.
`sample-recency-refresh` prints its PID and runs a 12-second unchanged-refresh
loop for a native CPU sampler; that loop includes save queuing as well as draining.
Each case uses 100 samples and the existing Rust allocation profiler. These are
local release-stage timings, not end-to-end completion latency or frame rates.

Run `just bench-completion ghost` for editor-side insertion previews: arrival,
alternative cycling, blink updates, matching type-through, width reflow and
warmed CPU rendering. Fixtures use 100 or 10,000 source lines, two alternatives
with eight inserted newlines, and 80-, 4,096- or 65,536-character anchor lines.
Input generation excludes fixture setup; provider responses are already prepared
and returned runtime effects are not executed. Width reflow includes rewrapping
the whole source document. Rendering uses the shared 1100×720 benchmark renderer;
setup asserts the anchor, all ghost rows and following source line are visible
after font/viewport refresh. Plain and ghost frames contain different text, so
their timing difference is not a measure of incremental ghost-rendering overhead.
Each case uses 100 samples and the existing allocation profiler. See the
[native checks and profiling record](../dev/refactoring-audit-2026-09-06.md#ghost-native-checks-and-release-profiling--2026-09-07)
for results and remaining verification limits.

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

CPU-side message → update and update → render probes. Returned commands are not
an executed native event loop: these do not measure OS input delivery, worker
scheduling, display presentation or end-to-end user latency.

Key benchmarks:

- `update_insert_char` - Synchronous typing update cost
- `update_move_cursor_*` - Navigation latency
- `full_loop_*` - CPU update+render cost
- `scaling_*` - How performance scales with document size
- `multi_cursor_*` - Performance with multiple cursors

### Production edit/history workloads

`just profile-workloads edit-history` measures Unicode backward deletion,
selection duplication and whole-line duplication at 1, 100 and 1,000 cursors
in one/two panes. Each fixture reports edit, Undo and Redo separately with
10 warmups and 100 samples per stage. Fixtures reset source/history and pane
state outside timing; text, stack sizes, cursor count/order/desired columns,
directional selections and active index are asserted after every stage.
The source contains `a🙂b` lines plus an untouched final line. Selection
duplication selects the emoji; line duplication uses collapsed carets.
Completion, LSP and bracket matching are disabled. The probe times deterministic
updates only: returned effects, parsing, rendering and presentation are excluded.
These are unsaved scratch-buffer fixtures, not loaded-file fixtures; comparing
an undone buffer with equal saved content is not part of these measurements.
This complements the existing `just profile-workloads insertions` probe, whose
single-line insertion fixture uses 500 samples; their values are not interchangeable.
The [indexed forward-map comparison](../dev/refactoring-audit-2026-09-06.md#controlled-release-comparison)
records a controlled before/after run, including small-cursor overhead and the
remaining high-cursor cost.

### Production rendering and Find workloads

`just profile-workloads` measures cursor movement versus undo-history size,
an explicit document-clone diagnostic, and warmed editor-group rendering at
1920×1080. Short-line fixtures contain 10,000/100,000 lines; wrapped single-line
fixtures range from 1,000 to 1,000,000 characters. Setup, font construction and
window presentation are excluded; warmups populate the glyph cache.

`just profile-workloads find` separates cached match/status access, synchronous
cache misses after edits/query changes, warmed rendering with Find, a pending
typing frame, and a synthetic search-compute/reply/render roundtrip. The last
case executes search inline in the harness: it measures total CPU work, not
background-worker scheduling or native response latency. Ten warmups precede
40 samples per case. Explicit Find actions can still need synchronous fresh
results even though display scans use the worker.

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

Contains both benchmark-only search implementations and production Document
search probes. Use the Document probes for product regression claims; the older
line-oriented fixtures are not substitutes. Record query, case sensitivity,
document size and result count. Use `profile-workloads find` for Find's cache,
update and rendering integration, and `profile-workloads replacements` for the
production Replace All transaction.

### glyph_cache

Font glyph caching and lookup performance.

### workspace

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
# Build with profiling symbols
just build-prof

# Run with Instruments
xcrun xctrace record --template "Time Profiler" --launch ./target/profiling/token samples/sample_code.rs
```

### Heap Profiling with dhat

```bash
# Build and run with dhat (opens the large sample)
just profile-memory

# View the output in dhat-viewer
# https://nnethercote.github.io/dh_view/dh_view.html
```

### Flamegraph (Linux)

```bash
# Requires cargo-flamegraph
just flamegraph
```

### Samply (macOS/Linux)

```bash
# Requires samply; builds with profiling symbols and opens the large sample
just profile-samply
```

## Performance Targets

These are engineering budgets, not measured guarantees. CPU probes cover only
part of the input-to-display path. Measure native presentation separately before
claiming a frame rate or end-to-end latency.

### Typing Latency

- Target: < 16ms CPU update+render budget
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
- `count_visible_*` and `get_visible_item_*` measure traversal/lookup components,
  not a rendered file-tree frame; a native/rendering probe is still required.

## Adding New Benchmarks

Microbenchmarks use Divan. Follow the existing fixtures in
[`benches/completion.rs`](../../benches/completion.rs):
[`with_inputs`](https://docs.rs/divan/latest/divan/struct.Bencher.html#method.with_inputs)
keeps fixture creation outside the measured operation. Use
[`benches/editor_workloads.rs`](../../benches/editor_workloads.rs) for production
update/render probes with explicit warmups, samples and correctness assertions.

### Guidelines

1. Use `divan::black_box()` to prevent optimization
2. Parameterize with `args = [...]` for scaling tests
3. Group related benchmarks in the same file
4. Name benchmarks descriptively: `{operation}_{context}_{size}`
5. Add the benchmark file to `[[bench]]` in Cargo.toml
6. Assert that the intended state is exercised (visible rows, result counts,
   current syntax, undo state); a faster no-op is not an optimization
7. Report setup, allocation profiling, effects and presentation exclusions

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

Run the relevant repository recipes on a stable, otherwise idle runner and
archive their stdout with the revision, dirty-tree status, toolchain, hardware,
profile, features, fixture parameters and sample counts. Compare the same
workload under the same conditions, preferably alternating baseline/candidate
runs; record medians and spread, not just the fastest observation.

Do not use Criterion's `--save-baseline` / `--baseline` options with these Divan
suites. Check forwarded options with `just bench-completion --help`. A dated
report is a snapshot, not a statistically controlled regression verdict.
