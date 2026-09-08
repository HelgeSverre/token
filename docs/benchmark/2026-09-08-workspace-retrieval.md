# Workspace retrieval ranking — 2026-09-08

Optimized CPU snapshot for the new opt-in inline context strategy, not a
before/after speedup claim or end-to-end suggestion latency measurement.

## Method

Apple M2 Max, macOS 15.6 (24G84), Rust 1.98.0 (`88d9e12ae`),
`aarch64-apple-darwin`. Cargo's optimized bench profile uses opt-level 3 and thin
LTO, default features, with the existing Divan 0.1.21 allocation profiler.
Builds and timed runs were serial, `CARGO_BUILD_JOBS=1`. This was an active
desktop with unrelated CPU-heavy processes; they were not terminated.

```sh
CARGO_TARGET_DIR=/tmp/token-managed-server-check.5prvI4 \
  CARGO_BUILD_JOBS=1 just bench-completion workspace_retrieval_rank
```

Two independent invocations, each with 100 samples per case. The first included
a fresh optimized build (13m 38s); compilation is outside the measurements.
The existing release-only unused `revision` warning in `src/update/syntax.rs`
was unchanged. This is not the fat-LTO distribution profile or a debug build.

Fixtures contain 32 or 256 synthetic Rust files, each with 16 three-line helper
functions: 512 or 4,096 declaration chunks. Every function references `Widget`
and `transform_widget`, exercising a broad match set; one exact helper name is
also queried. Setup parses/indexes the files outside timing. The timed call
tokenizes and ranks the retained snippets, selects/deduplicates up to eight,
and copies the selected payloads into provider-neutral chunks.

**Excluded:** directory traversal, ignore matching, disk reads, open-buffer
snapshot capture, parsing, worker scheduling, HTTP, inference and rendering.
The 256-file case is not the maximum permitted source-byte workload.

## Results

| Files / declaration chunks | Median, first | Median, repeat | Slowest, first / repeat | Fresh allocations / bytes |
| --- | ---: | ---: | ---: | ---: |
| 32 / 512 | 142.6 µs | 138.6 µs | 193 / 181.3 µs | 534 / 161.1 KB |
| 256 / 4,096 | 1.187 ms | 1.150 ms | 1.631 / 1.406 ms | 4,118 / 1.279 MB |

Allocation counts/bytes matched across runs. Divan reports decimal KB/MB;
fresh allocations are cumulative per iteration, not retained index size.
The full output separates fresh allocation, growth and deallocation.

Ranking scales roughly with the eightfold larger corpus here. Its warm cost
does not justify a more elaborate index yet. The larger case's allocation
pressure is a possible follow-up if a full collection/typing trace shows it
matters. These results do **not** establish total retrieval latency: collection
rechecks files on each request, and its cooperative 250 ms limit cannot bound
a stalled filesystem call. Measure cold/warm collection separately before
tuning that budget or making responsiveness claims. Live model relevance
also remains unmeasured.

## Artifacts

- [First run](data/2026-09-08/retrieval-rank-first.txt)
- [Independent repeat](data/2026-09-08/retrieval-rank-repeat.txt)
- Fixture: `benches/completion.rs::workspace_retrieval_rank`
- Implementation: `src/completion/retrieval.rs`
- [Scope and transmission policy](../user/config-editor.md#workspace-retrieval-context)

The implementation is committed in `09895c3`; this report's commit adds the
benchmark fixture. Source SHA-256 at measurement:

| Input | SHA-256 |
| --- | --- |
| `src/completion/retrieval.rs` | `1aace8bdb1675f9ef90c59280588672262abf3f685ace766dc01c15eec9a32a4` |
| `benches/completion.rs` | `2782405196c49da97209ff35db99a99f50b628fe0301efd79989476f32e74bed` |
| `Cargo.lock` | `33b73512a0dc309ce23b3450ca35e08a55a046a22586beec6e17ba99ee435c1b` |
