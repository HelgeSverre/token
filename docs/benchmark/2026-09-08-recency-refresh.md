# Recency context refresh — 2026-09-08

Commit `56a3be8` reuses exact unchanged recency snapshots. The 32-snippet
near-duplicate refresh median fell from 4.541 ms to 0.656–0.669 ms (about 85%).
Genuinely edited snippets still require indexing and similarity checks: the
same-sized edited fixture measured 4.617–4.707 ms after the change.

## Cause and implementation

The previous idle drain discarded overlapping snapshots before capturing their
replacement, rebuilding token indexes and comparing the whole ring even when
the text had not changed. A five-second native CPU sample of the optimized
32-snippet refresh loop attributed 2,712 of 4,247 main-thread samples (64%) to
the similarity-retention branch, including token string comparisons. A further
640 samples (15%) were in an index-construction sorting branch.

The ring now captures the current bounded text first. If an overlapping snapshot
of the same document has an identical full payload (filename and text), it moves
that snapshot's owned index into the refreshed entry. Stored snapshots are
pairwise dissimilar; retaining an identical immutable payload preserves that
invariant without repeating comparisons. Recency order, current region metadata,
path invalidation, empty captures and the strict Jaccard threshold are preserved.
Changed payloads still follow the full indexing/deduplication path. No revision-only
shortcut, approximate hash equality, extra cache, worker or public API was added.

## Measurements

```sh
CARGO_TARGET_DIR=/tmp/token-managed-server-check.5prvI4 \
  CARGO_BUILD_JOBS=1 just bench-completion recency_
# Prints the benchmark child PID; sample that process during its 12-second loop:
CARGO_TARGET_DIR=/tmp/token-managed-server-check.5prvI4 \
  CARGO_BUILD_JOBS=1 just bench-completion sample-recency-refresh
/usr/bin/sample <printed-pid> 5 1 -file /tmp/token-recency.sample.txt
```

Apple M2 Max, ARM64, macOS 15.6 (24G84), Rust 1.98.0 (`88d9e12ae`), optimized
bench profile with thin LTO, default features, Divan 0.1.21 allocation profiler.
Each case uses 100 samples and 8 or 32 open buffers with 8 KiB identifier-heavy
snippets, configured for 64 lines. The near-duplicate case has approximately
94% shared tokens, sorting first, so Jaccard similarity is just below 0.9.
Setup asserts that all snippets survive. The new edited case replaces the final
character in every buffer before queuing saves, ensuring exact reuse cannot apply.

Divan times idle draining, excluding fixture setup and save queuing. Stable
observation and request attachment are separate cases. The native sample includes
both queuing and draining, but excludes initial fixture construction. None of
these measures network, inference, rendering, presentation or user input latency.

Before: `b712058`, first with the original benchmark and then with the expanded
benchmark but unchanged runtime. After: the implementation committed as `56a3be8`,
then an independent repeat. Only an explanatory benchmark comment changed after
measurement. Builds are excluded. This was an active desktop, not an isolated,
alternating experiment; the first after run had substantial outliers. Full tests
and lint were also launched during its build interval. The independent repeat
ran after those checks had completed.

Medians; after ranges are two observed medians, not confidence intervals:

| Stage                            | Snippets |   Before |          After |
| -------------------------------- | -------: | -------: | -------------: |
| Unchanged near-duplicate refresh |        8 | 579.2 µs | 162.8–165.9 µs |
| Unchanged near-duplicate refresh |       32 | 4.541 ms | 0.656–0.669 ms |
| Unchanged distinct refresh       |        8 | 452.7 µs | 160.7–165.8 µs |
| Unchanged distinct refresh       |       32 | 1.907 ms | 0.651–0.667 ms |
| Edited near-duplicate refresh    |        8 | 577.9 µs | 583.5–597.4 µs |
| Edited near-duplicate refresh    |       32 | 4.539 ms | 4.617–4.707 ms |
| Initial distinct fill            |        8 | 886.2 µs | 440.9–450.2 µs |
| Initial distinct fill            |       32 | 3.594 ms | 1.839–1.870 ms |
| Stable observation               |        8 | 232.7 ns | 248.3–260.1 ns |
| Stable observation               |       32 | 1.030 µs |       1.062 µs |
| Request attachment               |        8 | 2.257 µs | 2.142–2.570 µs |
| Request attachment               |       32 | 7.749 µs | 7.769–8.478 µs |

The original before run independently measured 4.686 ms for the unchanged
near-duplicate 32-snippet case. The faster initial-fill result is an observation,
not evidence of exact reuse: its ring starts empty and still builds every index.
The source change rearranges the surrounding drain, but this investigation has
not attributed that fill difference to a specific mechanism.

Edited work did not improve: its median rose about 2–4% at 32 snippets in these
runs. Its first after maximum was 21.78 ms, versus 5.070 ms in the repeat and
4.852 ms before. Unchanged near-duplicate maxima were 5.874 ms and 0.722 ms after,
versus 4.744 ms before. These maxima are not p95 or a real-time guarantee.
The unchanged-path gain is repeatable; do not infer improved latency tails for
the edited path. At the default eight snippets the edited median remains about
0.6 ms. Further scheduling/algorithm changes are not justified by this measurement
alone; revisit if native traces show edited idle drains disrupting interaction.

## Allocation tradeoff

Divan's separately reported decimal-unit categories for a 32-snippet refresh:

| Metric                  | Before, unchanged or edited | After, unchanged |  After, edited |
| ----------------------- | --------------------------: | ---------------: | -------------: |
| `alloc` count / bytes   |              128 / 266.8 KB |    96 / 264.7 KB | 128 / 266.8 KB |
| `grow` count / bytes    |              256 / 522.2 KB |    none reported | 256 / 522.2 KB |
| `shrink` count / bytes  |               32 / 201.2 KB |    none reported |  32 / 201.2 KB |
| `dealloc` count / bytes |              128 / 587.8 KB |    96 / 264.7 KB | 128 / 587.8 KB |
| `max alloc` bytes       |                    6.288 KB |         8.274 KB |       8.274 KB |

Unchanged refresh eliminates index-vector growth/shrink work, not the temporary
text capture. Capturing before removing an old snapshot increases reported peak
tracked bytes by about 2 KB in this fixture. Setup retains the ring outside timing;
these counters are neither total ring retention nor process RSS. Do not combine
fresh allocation, growth and peak categories into one purported memory saving.

## Verification and evidence

The existing ordering/eviction test now checks unchanged background-save recency
and rewritten text with an unchanged revision. No new test functions were added.
All 13 recency-focused tests passed, followed by 2,581 full-suite tests (five
skipped), two doctests (six ignored), strict all-target/all-feature lint,
`just fmt-check` and whitespace checks. Full run ID:
`d0471970-7de3-4555-be5e-462fb2554ebc`. No process-exit warnings appeared in this
run; the historical intermittent warnings remain unresolved. The existing
release-only unused `revision` warning is unchanged.

Scoped diff-based review: **Approve**, no outstanding findings. Checked immutable
index ownership, pairwise-dissimilarity preservation, MRU ordering, rewritten and
empty capture behavior, and unchanged scope/path eligibility. Native/live-service
verification gates are not closed by this benchmark.

- [Original baseline](data/2026-09-08/recency-original.txt)
- [Expanded before baseline](data/2026-09-08/recency-before.txt)
- [Native before sample](data/2026-09-08/recency-before-sample.txt) — binary-images appendix omitted.
- [After](data/2026-09-08/recency-after.txt)
- [Independent repeat](data/2026-09-08/recency-repeat.txt) — initial partial output summarized; remaining output verbatim.

Measured runtime SHA-256 before:
`70737a2cbb86c87abf80e8c18ce5e873e804e6dd322a4be8e1ab3746e10b7067`;
after: `5f62903cdc22c26c4bf1f93108b7f612122be533ce927d79a4bd8bbce8360cab`.
Expanded benchmark SHA-256 during both measurements:
`d2372d995cfea3eb6e6dd8a9b8d76cf615a3e8466b22d7a9e1eb9abe2dd08e53`.
