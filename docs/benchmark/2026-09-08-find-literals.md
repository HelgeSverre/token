# ASCII literal Find — 2026-09-08

Commit `780b13e` addresses the remaining dense literal scan cost identified by
the [cold Find investigation](2026-09-08-cold-find.md). In the measured ASCII
fixture, cold scan medians fell from roughly 7.1 ms to 3.4–3.5 ms; full worker
computation fell from 10.2–10.3 ms to 6.5–6.7 ms.

## Implementation and scope

`SearchQuery` can compile an optional single-pattern Aho-Corasick matcher for a
valid, nonempty ASCII literal without whole-word mode. It is used only when the
haystack is also ASCII, where byte and character offsets coincide. Unicode text,
Unicode patterns, whole-word searches and explicit regex queries retain the
existing regex engine. Regex validation/error reporting is unchanged, and a
literal-matcher construction failure falls back to that engine.

The [matcher contract](https://docs.rs/aho-corasick/1.1.4/aho_corasick/struct.AhoCorasick.html#method.find_iter)
provides non-overlapping byte ranges. With one fixed-length pattern, its match
order agrees with Find's existing semantics. The text gate is essential: ASCII
case folding alone cannot preserve Unicode equivalents such as Kelvin sign/`k`
or long s/`s`. No lowercased document copy is created.

`aho-corasick` 1.1.4 was already present through regex. The manifest now names it
directly; the lockfile adds only that dependency edge, with no package version
changes. No application API, worker or cache was added. The separate editor
occurrence-selection API intentionally allows overlapping matches and remains
unchanged; merging it into Find would change that contract.

## Production-path measurements

```sh
CARGO_TARGET_DIR=/tmp/token-managed-server-check.5prvI4 \
  CARGO_BUILD_JOBS=1 just profile-workloads find-cold
```

Before: `9d641b4`, using both runs from the
[overview follow-up](2026-09-08-find-overview.md). After: working tree committed
as `780b13e`, followed by an independent repeat without source changes. The
unchanged fixture uses 10,000/100,000 repeated ASCII lines plus a final marker;
queries are dense `ordinary`, sparse `final_marker`, and absent `missing_marker`.
Ten warmups precede 100 observations per stage/case.

M2 Max, macOS 15.6 (24G84), Rust 1.98.0 (`88d9e12ae`), ARM64, optimized bench
profile with thin LTO, default features. Builds and this task's test/lint runs
are excluded from production-path timings. The desktop was not isolated; ranges
below are the two observed medians, not confidence intervals or tail guarantees.
The existing release-only unused `revision` warning remains.

Medians in milliseconds:

| Lines | Query | Cold before | Cold after | Worker before | Worker after |
| ---: | --- | ---: | ---: | ---: | ---: |
| 10,000 | Dense | 0.767–0.790 | 0.405–0.412 | 1.067–1.106 | 0.699–0.709 |
| 10,000 | Sparse | 0.202 | 0.191 | 0.200–0.202 | 0.187–0.188 |
| 10,000 | Absent | 0.193–0.199 | 0.192–0.193 | 0.193–0.198 | 0.194–0.196 |
| 100,000 | Dense | 7.144–7.187 | 3.428–3.501 | 10.189–10.320 | 6.512–6.700 |
| 100,000 | Sparse | 0.817–0.823 | 0.598–0.599 | 0.807–0.843 | 0.602–0.626 |
| 100,000 | Absent | 0.706–0.714 | 0.614–0.617 | 0.696–0.715 | 0.585–0.626 |

Dense 100,000-line p95 was 3.870/3.815 ms for cold scans and 7.082/6.843 ms for
worker computation. The worker includes overview construction; neither stage
includes native input, thread scheduling, rendering or presentation. Unicode and
regex workloads are deliberately not claimed to gain this acceleration.

## Allocation tradeoff

```sh
CARGO_TARGET_DIR=/tmp/token-managed-server-check.5prvI4 \
  CARGO_BUILD_JOBS=1 just bench-search find_ascii_literal_vs_regex
```

One added case in the existing Divan search suite compares the same 100,000-line
text and `ordinary` query with `is_regex=false` (accelerated) and `true` (original
engine, equivalent query here). Both include query construction and result
collection; text materialization is outside timing. This is a same-build engine
comparison, not a second end-to-end before/after measurement. Each invocation
uses 100 samples per branch. Allocation values matched in the independent repeat.

| Divan metric | Accelerated literal | Equivalent regex |
| --- | ---: | ---: |
| Median, first / repeat | 2.949 / 2.908 ms | 6.480 / 6.556 ms |
| `max alloc` bytes | 2.120 MB | 2.117 MB |
| `alloc` count / bytes | 809 / 60.05 KB | 792 / 57.13 KB |
| `grow` count / bytes | 143 / 2.141 MB | 129 / 2.131 MB |

These are Divan's separately reported allocation/reallocation categories in
decimal units, not process RSS. Peak tracked allocation increases by about 3 KB;
there is also extra constructor allocation and reallocation work. This modest
overhead is accepted for the measured scan-time gain. The allocation repeat was
on the active desktop, including a formatting check; no isolated timing claim
is made. Full deallocation/shrink data and tails remain in the raw output.

## Verification and artifacts

The seven search tests passed, then **2,581 full-suite tests and two doctests**
passed, with five tests and six doctests skipped/ignored. Strict all-target,
all-feature lint and formatting passed. Full nextest run:
`c606968e-7fdd-4bee-8bb8-b9cf66481fac`. This run had no process-exit warnings;
the historical intermittent warnings remain unresolved.

Existing tests now compare the fast path with a clone forced through the
unchanged regex engine, covering both case modes, overlapping candidates,
literal punctuation, empty input, Unicode offsets and case-fold equivalents.
Whole-word and regex tests assert that acceleration is disabled. No test
functions were added. Scoped review: **Approve**, no outstanding findings;
construction fallback, ASCII gates, dependency changes and overlap contracts
were checked against code and the library documentation.

- [After production-path timings](data/2026-09-08/find-literal-after.txt)
- [Independent production repeat](data/2026-09-08/find-literal-repeat.txt)
- [Allocation comparison](data/2026-09-08/find-literal-allocation.txt)
- [Allocation repeat](data/2026-09-08/find-literal-allocation-repeat.txt)

SHA-256: `src/search.rs`
`7e6af484f058783e9accb6f3ed7255db0ec461af2acd4a11468086e28d80dd8b`;
unchanged `benches/editor_workloads.rs`
`f6804a3f937fa20e7fa4bf9ad3d236151a5a965f522a1219718144fbe423156a`;
`benches/search.rs`
`c366cd62d852e4c24e2cfcbd3a575697bf6cebf1eb4edd781d267a1cf57e1822`.
