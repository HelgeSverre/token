# Shared syntax parsing: verification

The generic, Markdown, HTML, and Vue highlighting paths now share host-tree
parsing and cache replacement. Each path retains its existing highlighting and
injection behavior. The shared operation returns the previous edited tree and
source when highlighting needs incremental change information; specialized
paths discard those inputs immediately.

## Outcome

This is a maintainability improvement, with no meaningful runtime improvement
claimed. `src/syntax/parser.rs` loses 196 net lines, including the new regression
test. Four copies of cache lookup, incremental parsing, fallback, and cache
replacement become one implementation. No dependency was added.

## Release benchmark

Measured on an arm64 Mac with rustc 1.98.0, comparing baseline `fc47095` with the
refactored parser. Both release binaries include the identical new
`alternating_document_edit` benchmark. Compilation and test execution finished
before the paired measurements.

Each iteration alternates between the original sample and a version with a
leading newline, so every iteration performs an actual edit. Parser setup and
the initial parse are outside the timed loop. HTML and Vue use the same HTML
sample through their respective code paths; this is not an exhaustive Vue
component workload.

Three runs per binary, alternating before/after order, used 1,000 samples of
10 iterations for each language. Values below are medians of the three run
medians. Positive changes mean slower.

| Path | Before | After | Change |
| --- | ---: | ---: | ---: |
| Rust | 10.050 µs | 9.846 µs | -2.03% |
| Markdown | 136.900 µs | 136.400 µs | -0.37% |
| HTML | 47.230 µs | 47.330 µs | +0.21% |
| Vue | 47.350 µs | 47.140 µs | -0.44% |

Measured Rust allocation counts and allocated bytes per iteration were unchanged
for all four paths. This profiler does not account for all native Tree-sitter
allocations. These local microbenchmarks do not establish editor frame latency,
large-file performance, or a statistically significant speedup.

Run the benchmark with:

```sh
cargo bench --bench syntax -- alternating_document_edit --sample-count 1000 --sample-size 10
```

When running saved benchmark executables directly, include `--bench` to enable
measurement. Without it, Divan only exercises the benchmarks in test mode.
For an independent baseline comparison, apply only the benchmark addition to
`fc47095`, build it, and retain the executable before building the refactor.

## Correctness coverage

The new regression test compares incremental output with a fresh parser after
unchanged text, Unicode replacements, newline insertion/deletion, renames,
emptying/restoring a document, and switches among all four language paths. It
also checks that specialized paths continue to return full highlighting rather
than incremental patches. Existing syntax tests cover bounded patches,
multiline captures, injections, and structural selection.

The exceptional Tree-sitter parse-failure fallback was inspected in the diff,
but failure was not artificially injected. It still invalidates cached trees
and highlights before a full-parse retry.

## Test environment

Runtime tests construct an application that loads editor preferences. Running
with the developer's normal preferences produced completion/inline-suggestion
failures; the completion-menu failure also reproduced with the unchanged
`fc47095` parser. These are not evidence of a parsing regression.

Use an empty configuration directory for reproducible suite execution. The
existing `test_config_dir_uses_dot_config_on_unix` additionally assumes the path
contains `.config`, so name the isolated directory accordingly:

```sh
mkdir -p target/verification/.config
XDG_CONFIG_HOME="$PWD/target/verification/.config" just test --no-fail-fast
just lint
```

No runtime tests or application preferences were changed by this refactor.

Final validation: 2,735 nextest tests passed (7 skipped), plus 2 doctests passed
(6 ignored), with the isolated `.config` directory. The targeted syntax run
passed all 169 tests. `just lint` passed with warnings denied; changed Rust files
passed rustfmt checks and the patch passed `git diff --check`.
