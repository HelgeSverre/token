# Forward multi-cursor edits — 2026-09-08

Commit `f0e1c4e` removes repeated conversion of coincident caret/selection
positions. Two-pane line duplication at 1,000 cursors measured 1.65–1.67 ms
afterward, down from 2.45 ms. The gain is specific to mapped pane positions;
one-pane editing and snapshot-based Undo/Redo do not generally benefit.

## Attribution and change

The current-state baseline confirms the earlier forward-edit/Undo asymmetry:
duplicating lines at 1,000 cursors in two panes took 2.449 ms median, versus
0.150 ms for Undo and 0.166 ms for Redo. A five-second native sample collected
4,237 main-thread samples, 4,193 under the production update entry point.

Peer-position capture accounted for 1,119 samples (26.4%), and restoration for
1,025 (24.2%). These branches repeatedly call the shared Document coordinate
helpers, including Rope line lookup and column clamping. The mutation/history
operation-construction branch accounted for 219 samples (5.2%); the two explicit
editor-history snapshot branches together had six samples. This points to
coordinate work, not evidence that history should be redesigned or dropped.

The shared edit transaction now reuses the caret's converted coordinate when
a selection endpoint is exactly equal to it. A collapsed selection normally
needs one conversion instead of three, both before and after mutation. Distinct
endpoints still use the existing helpers independently. Equality is checked,
not assumed: reversed selections, clipped positions and noncanonical caret/head
combinations retain their semantics. There is no new public API, cache, data
structure, dependency or editing-specific geometry implementation.

## Reproduction and scope

```sh
CARGO_TARGET_DIR=/tmp/token-managed-server-check.5prvI4 \
  CARGO_BUILD_JOBS=1 just profile-workloads edit-history
# Prints the child PID; attach sample during the 12-second loop:
CARGO_TARGET_DIR=/tmp/token-managed-server-check.5prvI4 \
  CARGO_BUILD_JOBS=1 just profile-workloads sample-edit-history
/usr/bin/sample <printed-pid> 5 1 -file /tmp/token-multicursor.sample.txt
```

Apple M2 Max, ARM64, macOS 15.6 (24G84), Rust 1.98.0 (`88d9e12ae`), optimized
bench profile with thin LTO and default features. The baseline is `1019d99` plus
the sampling-mode-only benchmark change. All timing runs use that same harness.
The first optimized build took 6m 23s, excluded from the measurements. The
existing release-only unused `revision` warning is unchanged.

The existing fixture measures deletion, selection duplication and line duplication
at 1, 100 and 1,000 cursors in one/two panes: 54 stage/input combinations.
Ten warmups precede 100 samples of edit, Undo and Redo separately. Source is
Unicode `a🙂b` lines plus an untouched tail. Setup/reset and assertions are outside
the timed update; exact text, selection direction, cursor order/count, active
index, desired columns and history lengths are checked after each stage.
Completion, LSP and bracket matching are disabled. Returned effects, rendering,
native input delivery and presentation are excluded. These are CPU measurements,
not end-to-end user latency or guaranteed frame rates.

The new sampling mode reuses the same fixture/reset/assertions, selecting only
forward line duplication with 1,000 cursors/two panes. Its 12-second loop includes
reset and assertions, but not Undo/Redo. The native sample is therefore distinct
from the stage timer; the call tree identifies production work separately.
No new test functions were added.

## Before/after timings

Medians in microseconds. After ranges are two independent run medians on the
same source, not confidence intervals. All three runs passed every fixture
assertion. This was an active desktop, not an isolated or alternating experiment;
this task's tests/lint finished during compilation, before after-run timing.

| Operation           | Cursors | Panes |    Before |               After |
| ------------------- | ------: | ----: | --------: | ------------------: |
| Delete              |       1 |     2 |     3.542 |         2.917–2.958 |
| Delete              |     100 |     2 |   779.583 |     475.333–490.083 |
| Delete              |   1,000 |     2 | 2,147.209 | 1,381.417–1,422.791 |
| Duplicate selection |       1 |     2 |     4.250 |         3.875–3.958 |
| Duplicate selection |     100 |     2 |   758.125 |     628.000–641.250 |
| Duplicate selection |   1,000 |     2 | 2,550.708 | 2,103.584–2,146.000 |
| Duplicate lines     |       1 |     2 |     4.375 |         3.542–3.667 |
| Duplicate lines     |     100 |     2 |   714.334 |     483.917–495.125 |
| Duplicate lines     |   1,000 |     2 | 2,448.958 | 1,648.041–1,666.334 |
| Delete              |   1,000 |     1 |   966.875 |     954.458–967.292 |
| Duplicate selection |   1,000 |     1 | 1,238.750 | 1,212.334–1,241.959 |
| Duplicate lines     |   1,000 |     1 | 1,158.125 | 1,163.250–1,196.417 |

At 1,000 cursors/two panes, deletion improved about 34–36%, selection duplication
16–18% and line duplication 32–33%. A nonempty selection keeps its distinct anchor
conversion; collapsed selections remove more redundant work. The one-pane author
already has explicitly placed final carets, so its unchanged mapping policy makes
it a useful control. Its 1,000-cursor medians stayed within roughly 3.3% of before.

The two-pane line-duplication p95 fell from 2.587 ms to 1.723–1.772 ms. Other cases
showed noise: one-pane selection duplication at 100 cursors rose from 397.750 µs
to 393.084/536.000 µs median; two-pane line duplication at 100 cursors had
0.511/1.340 ms p95 after, versus 0.766 ms before. Keep the full tables rather than
claiming stable tails. Line-duplication Undo at 1,000 cursors/two panes remained
0.156–0.157 ms and Redo 0.158–0.165 ms. No allocation or RSS reduction is claimed.

The asymmetry is reduced, not eliminated: forward edits still plan mutations,
translate positions and construct history that Undo/Redo already owns. Other
coordinate work remains (for example, locating the earliest edited line and
placing the author's carets). This finding does not warrant changing editing
semantics, dropping history snapshots or adding a parallel transaction API.
Further tuning should be driven by measured interaction costs, not parity with
Undo/Redo as an artificial target.

## Verification

The focused runs passed: 27 tests matching `ordinary_`, five pane-history tests
and ten shared-edit tests. The full suite passed 2,581 tests (five skipped) and
two doctests (six ignored), followed by strict all-target/all-feature lint.
`just fmt-check`, scoped report formatting and whitespace checks also passed.
Full nextest run: `71bf0216-6051-4cf6-ad4a-a75dd775b990`. No process-exit warnings
appeared in this run; this does not explain the historical intermittent warnings.

Scoped diff-based review: **Approve**, no outstanding findings. Checked exact
equality reuse against the unchanged conversion helpers, directional selections,
clamping, scope mapping, no-op navigation state and history restoration. The
sampling mode preserves the original normal-run assertions and timing boundaries.

## Artifacts

- [Before timings](data/2026-09-08/multicursor-before.txt)
- [Native before sample](data/2026-09-08/multicursor-before-sample.txt) — binary-images appendix omitted.
- [After timings](data/2026-09-08/multicursor-after.txt)
- [Independent repeat](data/2026-09-08/multicursor-repeat.txt)

Measured `src/update/text_edits.rs` SHA-256 before:
`4b8e665ef72599627204aebb220f6209ac76df0f0a09199f9600bc34a85a0edb`;
after: `b73ebb107cd5e1b10d9e6d1f2c6804bedc50bd7785255a251a87692782ee7ce4`.
Benchmark SHA-256 for both:
`b2588bf370fd8a646a4aff2989f935fd51ee14060f3400021f3aa8d3c04100ea`.
