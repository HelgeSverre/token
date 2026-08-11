# Performance baseline — 2026-08-11

Captured on the development machine before search optimization with the release
benchmark profile.

## Startup stress test

Startup was measured on an M2 Max MacBook Pro running macOS 15.6. The workload
opened the Token repository as a workspace and loaded `src/syntax/parser.rs`,
the largest tracked Rust source file at 139,260 bytes and 3,953 physical lines.
Each result is the median of 12 warm launches after two warmups, with 250 ms
between launches. The timer ran from process creation until the first complete
frame was presented; temporary internal checkpoints were used to avoid counting
automation-socket scheduling latency.

| Build | Original | Font preparation | Conventional batch | Current range |
| --- | ---: | ---: | ---: | ---: |
| Release (`profile.release`) | 166.4 ms | 123.2 ms | **86.1 ms** | 83.2–103.3 ms |
| Distribution (`profile.dist`) | — | 111.0 ms | Not rerun | 99.0–126.7 ms |
| `just workspace` debug build | — | ~143 ms | 120.0 ms | 118.5–131.5 ms |

The accepted optimization parses the full embedded JetBrains Mono font on a
worker while macOS initializes the event loop and window. Font parsing cost
about 39 ms in an optimized build and previously sat directly on the critical
path. Startup-critical font dependencies are optimized in the dev profile so
the literal `just workspace` candidate remains representative without
optimizing Token's own debug code.

The conventional follow-up overlaps model, keymap, initial-file, and workspace
preparation with AppKit initialization. The recursive workspace watcher and
standard macOS application menu are installed 50 ms after the first frame, and
macOS no longer decodes a window icon that winit ignores on that platform. The
menu retains About, Services, Hide, Hide Others, Show All, and Quit behavior;
bundled builds still provide their richer About content through Info.plist and
`Credits.rtf` resources.

One instrumented release run from the conventional batch reached these
cumulative points from the start of `main`:

| Checkpoint | Time from `main` |
| --- | ---: |
| CLI ready | 1.0 ms |
| Event loop ready | 43.3 ms |
| Workspace and initial file ready | 43.5 ms |
| Application resumed | 54.2 ms |
| Native window ready | 80.4 ms |
| Renderer ready | 80.8 ms |
| First frame presented | 91.1 ms |

The 86.1 ms headline is the median of the external process-to-first-frame
samples, not the single checkpoint run. A true 50 ms result is still not
realistic in the current single-process AppKit/winit architecture: Apple-owned
application and titled-window initialization consume most of that budget
before Token draws. Reaching 50 ms would require a product-level technique such
as a persistent background process or prewarmed window.

```bash
cargo bench --bench search
cargo bench --bench syntax incremental
```

Representative medians from the pre-change repository-wide suites:

| Benchmark | Input | Median | Allocated |
| --- | ---: | ---: | ---: |
| `count_occurrences` | 100,000 lines | 11.86 ms | 14.28 MB / 109,261 allocations |
| `search_case_insensitive` | 100,000 lines | 16.10 ms | 18.78 MB / 209,262 allocations |
| `incremental_parse_middle_edit` | 5,000 lines | 3.741 ms | 515.3 KB / 2,014 allocations |
| `incremental_parse_small_edit` | 5,000 lines | 4.193 ms | 515.4 KB / 2,014 allocations |

The original search suite measures line-oriented benchmark implementations,
not `Document::find_all_occurrences_with_options`; dedicated Document-level
benchmarks were added with the optimization and should be used for subsequent
comparisons.

After the change, the dedicated 100,000-line case-sensitive Document benchmark
measured 4.284 ms median with three allocations. The initial Unicode-preserving
case-insensitive implementation measured 40.27 ms on this ASCII fixture and was
therefore not accepted as the final fast path; ASCII folding now has a dedicated
allocation-light path.

Final dedicated Document medians for 100,000 lines were 4.757 ms
case-sensitive and 6.388 ms case-insensitive, each with three fresh allocations
reported by Divan (result-vector growth is reported separately).

The syntax benchmark begins with contiguous strings and therefore excludes the
main-thread `Rope::to_string()` snapshot. Use `token automate profile` for the
real presentation stages and automation-driven typing scenarios for end-to-end
latency.

## End-to-end syntax profiling

`token automate syntax-profile` now separates snapshot, worker queue,
tree-sitter parse, highlight-query traversal, outline extraction, application,
and edit-to-present latency. Representative debug-build measurements on the
deterministic Rust demo were:

| Scenario | Snapshot | Parse | Highlight query | Outline | Apply | Edit → present |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Small edit, small document | 0.012 ms | 0.041 ms | 0.286 ms | 0.084 ms | 0.043 ms | 38.2 ms |
| Insert 5,000 lines | 0.071 ms | 84.6 ms | 99.1 ms | 27.6 ms | 3.56 ms | 255.7 ms |
| Small edit after 5,000-line insert | 0.050 ms | 30.8 ms | 101.4 ms | 27.1 ms | 4.16 ms | 203.7 ms |

The small-document profile initially took about 91.6 ms end to end despite
only 0.32 ms of parsing/highlighting. Profiling revealed that worker completion
did not wake winit; adding an event-loop wake reduced repeated samples to about
37.6–38.3 ms, which is now predominantly the intentional 30 ms debounce plus
render scheduling. On the large incremental case, full-document highlight
query traversal and outline extraction cost substantially more than the Rope
snapshot. Optimize their invalidation/range behavior before considering a
syntax-engine replacement.

After adding expanded changed-line highlight patches and demand-driven outline
extraction, three small edits in the same 5,000-line document measured:

| Stage | Before | After |
| --- | ---: | ---: |
| Highlight query | 101.4 ms | 1.19–1.26 ms |
| Outline, panel closed | 27.1 ms | <0.001 ms |
| Apply highlights | 4.16 ms | 0.12–0.14 ms |
| Edit → present | 203.7 ms | 80.5–87.7 ms |

The remaining large-document cost is primarily incremental tree parsing and
edit computation at roughly 39–45 ms in these debug-build samples. Markdown,
HTML, and Vue retain full highlighting because their multi-pass/injection
pipelines require specialized invalidation rules. When the outline panel is
open, extraction remains intentionally current and cost about 26 ms here.
