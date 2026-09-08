# Completion response ownership — 2026-09-08

Follow-up to the [September allocation finding](2026-09-07-current.md#findings-and-next-profiling-targets).
The converter now retains the original typed completion item and serializes it
only when the runtime sends a resolve request. It no longer builds a second JSON
representation for every candidate or deep-clones that JSON during debouncing.

## Reproduction and scope

```sh
CARGO_TARGET_DIR=/tmp/token-managed-server-check.5prvI4 \
  CARGO_BUILD_JOBS=1 just bench-completion convert_server_response
```

Before: clean `f42f0d0`. After: working-tree code subsequently committed as
`68cf62a`, followed by an independent repeat without source changes. The existing
`benches/completion.rs::convert_server_response` fixture is unchanged: 200 or
1,000 ts-ls-shaped items carrying label/kind/sort/detail, a primary text edit and
opaque `data`, with resolve support enabled. Input construction is outside timing.

Apple M2 Max, macOS 15.6 (24G84), Rust 1.98.0 (`88d9e12ae`),
`aarch64-apple-darwin`; Cargo optimized bench profile, default features, opt-level
3 and thin LTO. Divan 0.1.21 allocation profiling, 100 samples per case in each
invocation. Builds and timed runs were serial with one Cargo build job. This was
an active desktop, not an isolated runner. Compilation is excluded; the existing
release-only unused `revision` warning was unchanged.

This measures **response conversion**, not LSP parsing/network latency, menu
refiltering, rendering or the eventual resolve request. Required serialization
still happens for selected items when resolve is actually sent. Unused candidates
never pay it. Debug trace formatting is also outside these optimized measurements.

## Results

| Items | Before median | After median | Repeat median | Fresh allocations, before → after | Fresh bytes, before → after |
| --- | ---: | ---: | ---: | ---: | ---: |
| 200 | 134.6 µs | 22.52 µs | 25.72 µs | 7,602 → 2,002 | 879.2 KB → 167.3 KB |
| 1,000 | 894.5 µs | 138.5 µs | 155.5 µs | 38,002 → 10,002 | 4.398 MB → 838.5 KB |

Allocation counts/bytes matched in both after runs. At 1,000 items this is about
81% fewer freshly allocated bytes and 74% fewer fresh allocations. These are
cumulative per-operation allocation measurements in Divan's decimal KB/MB, not
retained menu size or process RSS.

Medians improved substantially in this fixture, but the repeat's slowest
1,000-item observation was **2.084 ms**, versus 1.237 ms before and 183.2 µs in
the first after run. The desktop was not load-controlled; preserve that tail
rather than infer uniformly lower latency or a new frame-time guarantee.

The ownership change addresses the measured allocation source without a new
cache, worker, serializer or test harness. Further changes to individual field
ownership need another workload demonstrating value; per-keystroke refiltering
and the other September profiling targets are separate concerns.

## Verification and artifacts

216 completion-focused tests passed, then 2,581 full-suite tests and two doctests
passed (`be248240-c876-4626-a891-d859c89fe6bb`), with five tests and six doctests
skipped/ignored. Strict all-target/all-feature lint and formatting passed.
Existing round-trip, snippet, initial-edit, resolve/cancellation and rendering
fixtures were adapted; no new tests were added. The full suite had no exit warnings,
which does not establish the cause of historical intermittent warnings.

- [Before output](data/2026-09-08/completion-response-before.txt)
- [After output](data/2026-09-08/completion-response-after.txt)
- [Independent repeat](data/2026-09-08/completion-response-repeat.txt)
- [Implementation and review](../dev/refactoring-audit-2026-09-06.md#completion-response-ownership--2026-09-08)

SHA-256 of the unchanged fixture: `2782405196c49da97209ff35db99a99f50b628fe0301efd79989476f32e74bed`.
After-conversion source `src/completion/lsp.rs`:
`f164b32b5b5c1103bc36ada33be474b4b0dc9eb8b81daee390e2af061f79d4a7`.
