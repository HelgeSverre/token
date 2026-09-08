# Find overview projection — 2026-09-08

Commit `6974ba1` follows the [cold Find CPU investigation](2026-09-08-cold-find.md).
Overview construction now advances through borrowed Rope chunks instead of
constructing a Rope slice for each matched line. Dense worker medians improved
modestly; the underlying match scan is unchanged.

The subsequent [literal-matching follow-up](2026-09-08-find-literals.md)
addresses the measured ASCII scan cost; this report retains the earlier state.

## Change and verification

Nearby matches reuse the remaining chunk suffix. Each step advances character
and line coordinates with Ropey's existing string helpers; gaps seek directly
to the containing chunk and its coordinates. Counting happens before trimming
the suffix so matches between CR and LF preserve the pending line break.
There is no new parser, cache, worker, public API or dependency. The existing
snapshot ownership, lazy `OnceLock` projection and line deduplication remain.

The existing coordinate-oracle test now crosses many chunks with Unicode/CRLF
text, matches individual newline characters, and covers a long single line.
It compares the projection to `Rope::char_to_line`; no test functions were added.
All **96 Find-focused tests**, then **2,581 full-suite tests and two doctests**
passed. Five tests and six doctests were skipped/ignored. Strict lint and
formatting passed. Full nextest run: `ec5fa5f6-f57b-4b4a-baa3-4b7b2436420d`.

That full run reported one **LEAK** warning for the existing inline-worker test
`supersession_disconnects_old_socket_and_serves_new_request`. Its assertions
passed; the process-exit warning remains under investigation. An initial
targeted invocation also failed during test discovery while overlapping builds
refreshed its executable; subsequent completed checks used the final source.
Neither issue is suppressed or treated as resolved by the passing assertions.

Scoped diff-based review checked monotonic offsets, chunk/EOF boundaries,
CRLF interiors, sparse seeks, empty matches and snapshot/lazy-cache behavior.
No outstanding findings; verdict: **Approve**.

## Reproduction and results

```sh
CARGO_TARGET_DIR=/tmp/token-managed-server-check.5prvI4 \
  CARGO_BUILD_JOBS=1 just profile-workloads find-cold
```

The unchanged fixture and environment are described in the
[baseline report](2026-09-08-cold-find.md#method): M2 Max, macOS 15.6, Rust 1.98.0,
optimized bench profile with thin LTO, default features, ten warmups and 100
observations per stage/case. Before is `72ed2f7` (same production code as the
baseline); after is `6974ba1`. Builds are excluded. The desktop was active;
unrelated compilation was observed during the rejected attempt. Final timings,
their repeat and the sample ran after this task's tests/lint and serially.

Medians in milliseconds; before ranges retain both earlier baseline runs.

| Repeated lines | Query | Worker before | Worker after | After repeat |
| ---: | --- | ---: | ---: | ---: |
| 10,000 | Dense | 1.118–1.120 | 1.106 | 1.067 |
| 10,000 | Sparse | 0.201–0.203 | 0.200 | 0.202 |
| 10,000 | Absent | 0.194–0.194 | 0.198 | 0.193 |
| 100,000 | Dense | 11.066–11.642 | 10.189 | 10.320 |
| 100,000 | Sparse | 0.845–0.933 | 0.807 | 0.843 |
| 100,000 | Absent | 0.717–0.828 | 0.715 | 0.696 |

Dense 100,000-line worker p95 was 10.558/10.963 ms after, versus
12.182/11.481 ms before. Cold scan medians remained 7.187/7.144 ms, consistent
with no change to matching. Small sparse/absent differences are not claimed as
optimization gains. This is CPU computation, not native input latency, worker
scheduling, rendering or an allocation measurement.

A five-second native sample of `sample-find-worker` collected 3,846 main-thread
samples. Overview initialization accounted for 1,196 (~31%), compared with
1,398/3,858 (~36%) before. This supports a modest improvement, not elimination
of overview cost. Sampling/inlining and desktop variation limit precision.

## Rejected approach and remaining target

An initial prototype cached whole chunks but recalculated line coordinates from
each chunk's beginning for every match. It passed correctness checks but measured
15.849 ms for dense 100,000-line worker computation—worse than the baseline.
The final implementation advances the suffix instead. The rejected output is
retained below; its repeat was canceled during compilation when superseded and
produced no timing result. No rejected source remains in the application.

The dense regex scan still dominates. Any literal fast path needs separate
measurement and Unicode/case/whole-word equivalence checks; this change does
not claim to resolve that target or the rest of the handoff.

## Artifacts

- [After timings](data/2026-09-08/find-overview-after.txt)
- [Independent repeat](data/2026-09-08/find-overview-repeat.txt)
- [After CPU sample](data/2026-09-08/find-overview-after-sample.txt), Binary Images appendix omitted
- [Rejected prefix-rescan prototype](data/2026-09-08/find-overview-rejected-prefix-scan.txt)

SHA-256 of final `src/model/ui.rs`:
`cd7c81b6d02c7b998c02b1868fc535dc926c8f500fa8f68eb15500c026a8c40d`.
Unchanged fixture `benches/editor_workloads.rs`:
`f6804a3f937fa20e7fa4bf9ad3d236151a5a965f522a1219718144fbe423156a`.
