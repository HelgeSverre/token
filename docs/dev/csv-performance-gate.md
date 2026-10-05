# 1 GiB CSV opening gate

Current milestone: quickly open a 1 GiB CSV into a usable grid. Portable mode is
deferred. These are provisional engineering targets, not measured performance
claims. Text opening and external-change reads have no fixed size cap; available
memory and processing time still limit usable document sizes.

## Targets

Reference machine: Apple M2 MacBook, 32 GB RAM, internal SSD, release build.
Use an idle machine with AC power and record macOS version and power mode.

| Measurement | Initial target |
| --- | --- |
| Open request through full CSV parse and one rendered grid frame | <= 2,000 ms |
| First/last-cell navigation through one rendered frame, p95 of 20 operations | <= 100 ms |
| Sampled process RSS during the run | <= 3 GiB |
| Correctness | All 2,097,152 records, eight columns, first/last cell values intact |

The fixture is exactly 1,073,741,824 bytes (1 GiB), slightly larger than decimal
1 GB. Fixed 512-byte records include UTF-8, quoted commas, escaped quotes and
quoted newlines. It is deliberately a narrow, repeatable workload: repeated
records are not representative of all CSVs. Wide rows, huge fields, other
encodings and ragged data need additional workloads before broad claims.

## Run

On macOS/Linux with Node, a Rust toolchain and a native desktop session:

```sh
just gate-csv
# Or use an already-built release binary:
node scripts/gate-csv.mjs target/release/token
```

The script exits nonzero for a functional failure or missed target and writes a
JSON report into its isolated `target/verification/csv-gate/run-*/` directory.
It never connects to an existing Token window. It reuses a generated fixture;
reserve at least 1 GiB disk space. Close other apps and leave its window alone.
The automation lifecycle helper is currently Unix-only, so this does not yet
qualify Windows performance.

Run five fresh processes; **all five must pass**. Keep reports with the commit
SHA, binary/build provenance and machine details. This is an opt-in development
gate until the implementation passes, then suitable for a dedicated performance
runner. Do not add an expected-red check to required PR CI or waive failures as
success. Do not compare timings from arbitrary shared CI hardware.

Fixture generation, sequential warm-cache pre-read and application startup are
outside the timer. Timing begins before sending the open request to the idle
editor, and includes text loading, mode switching, complete parsing and an
additional rendered frame. `profile_frames(1)` completes on the native frame
path; socket command acknowledgement alone is not treated as presentation.
The gate includes automation/IPC overhead. It does not measure physical display
scan-out. LSP is disabled in the isolated configuration.

RSS is sampled with `ps` every 100 ms and can miss short peaks. Treat it as a
coarse allocation regression gate, not a true peak measurement. Confirm actual
peak memory with platform profiling before declaring the milestone complete.
The sequential pre-read requests warm cache but cannot guarantee residency.
Cold-cache and launch-to-grid timings should be recorded separately; this gate
must never be described as measuring them.

## Implementation order

1. Buffered rope loading and uncapped text opening/external-change reads are in
   place. Image, formatter and preview-resource budgets remain separate.
2. CSV parsing reads rope chunks and constructs final row storage directly,
   preserving quoted multiline records and UTF-8. Moving parsing onto a
   cancellable worker remains outstanding.
3. Remove whole-document conversion from cell editing using record-position
   indexing. Ensure editing, save and external-change handling remain correct.
4. Profile any remaining full-document work in the actual opening pipeline.
5. Run the gate on reference hardware and retain reports. Add varied CSV shapes
   and native Windows coverage before generalizing the result.

The gate deliberately requires all records to be available: a placeholder or
only parsing the first viewport cannot pass. A future lazy/paged implementation
needs a separate explicit contract for first-grid latency, background indexing,
random access and progress/cancellation, rather than silently weakening this one.

## Harness validation

`node scripts/gate-csv.mjs --check-fixture` checks bounded fixture generation on
eight records. This does not run Token or prove the performance target. The CSV
automation snapshot has a unit check for logical multiline records and bounded
cell text; run `just test-one csv_snapshot` on a configured Rust environment.
