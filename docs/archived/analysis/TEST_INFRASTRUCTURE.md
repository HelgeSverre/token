# Test Infrastructure Analysis

## Overview

The rust-editor project has a well-structured test suite with **253 tests** across integration tests and inline unit tests. The architecture follows Rust best practices with a shared `common` module for test helpers.

### Test Distribution

| Location                   | Test Count | Type              |
| -------------------------- | ---------- | ----------------- |
| `tests/cursor_movement.rs` | 33         | Integration       |
| `tests/text_editing.rs`    | 33         | Integration       |
| `tests/selection.rs`       | 14         | Integration       |
| `tests/scrolling.rs`       | 34         | Integration       |
| `tests/edge_cases.rs`      | 12         | Integration       |
| `tests/monkey_tests.rs`    | 31         | Stress/Fuzz       |
| `tests/status_bar.rs`      | 44         | Integration (TDD) |
| `src/main.rs`              | ~25        | Unit (binary)     |
| `src/overlay.rs`           | 7          | Unit              |
| `src/theme.rs`             | 10         | Unit              |
| `src/model/editor_area.rs` | ~10        | Unit              |

---

## Test Helper Catalog

### Common Test Helpers (`tests/common/mod.rs`)

| Helper                        | Purpose                                          | Parameters                                                   |
| ----------------------------- | ------------------------------------------------ | ------------------------------------------------------------ |
| `test_model()`                | Creates `AppModel` with text and cursor position | `text: &str`, `line: usize`, `column: usize`                 |
| `test_model_with_selection()` | Creates `AppModel` with active selection         | `text`, `anchor_line`, `anchor_col`, `head_line`, `head_col` |
| `buffer_to_string()`          | Extracts document text for assertions            | `model: &AppModel`                                           |

### Default Test Configuration

```rust
AppModel {
    viewport: Viewport {
        visible_lines: 25,
        visible_columns: 80,
    },
    scroll_padding: 1,
    window_size: (800, 600),
    line_height: 20,
    char_width: 10.0,
}
```

### Additional Inline Helpers

| File                       | Helper                        | Purpose                                                |
| -------------------------- | ----------------------------- | ------------------------------------------------------ |
| `src/main.rs`              | `test_model_with_selection()` | Duplicate for binary tests (needed for `handle_key()`) |
| `src/model/editor_area.rs` | `create_test_editor_area()`   | Creates single-document `EditorArea`                   |

---

## Test Patterns Catalog

| Pattern                             | Example                          | Usage Count | Notes                     |
| ----------------------------------- | -------------------------------- | ----------- | ------------------------- |
| **Setup-Act-Assert**                | Most tests                       | ~250        | Standard pattern          |
| **Sequential Message Dispatch**     | `update(&mut model, Msg::...);`  | ~200        | Tests message flow        |
| **Programmatic Text Generation**    | `(0..100).map(...).collect()`    | ~15         | Large document tests      |
| **Boundary-at-a-time Verification** | Scroll boundary tests            | ~20         | Incremental assertions    |
| **Explicit Edge Value Testing**     | `u32::MAX`, `0`, column `999999` | ~15         | Stress testing            |
| **Section Comments**                | `// ========` dividers           | All files   | Clear organization        |
| **Loop-based Stress Testing**       | `for _ in 0..100 { ... }`        | ~10         | Rapid operation sequences |

### Naming Conventions

```
test_<feature>_<scenario>_<expected_outcome>

Examples:
- test_cursor_buffer_position_start_of_file
- test_smart_home_toggle
- test_undo_insert_char_over_selection_restores_original_text
- test_resize_to_zero_width_does_not_crash
```

---

## Test Categories

### 1. Unit Tests (Inline `#[cfg(test)]` modules)

**Location:** `src/*.rs`

- **overlay.rs**: Overlay positioning, pixel blending
- **theme.rs**: Color parsing, YAML theme loading
- **editor_area.rs**: Layout computation, hit testing
- **main.rs**: Key handling integration (requires binary)

### 2. Integration Tests (`tests/*.rs`)

Tests the public API through the message-update cycle:

- `Msg::Editor(EditorMsg::*)`
- `Msg::Document(DocumentMsg::*)`
- `Msg::App(AppMsg::*)`
- `Msg::Ui(UiMsg::*)`

### 3. Stress/Fuzz Tests (`tests/monkey_tests.rs`)

Categories:

- **Window Resize Edge Cases**: Zero dimensions, max u32, rapid oscillation
- **Empty Document Edge Cases**: Operations on empty buffer
- **Extreme Cursor Positions**: Line/column beyond document bounds
- **Selection Edge Cases**: Inverted selections, out-of-bounds
- **Rapid Operation Sequences**: Interleaved operations
- **Large Document Stress Tests**: 10,000 lines
- **Unicode/Special Characters**: Emoji, CJK, null bytes

### 4. TDD Tests (`tests/status_bar.rs`)

Structured in phases matching implementation:

- Phase 1: Core Data Structures
- Phase 2: Collection Operations
- Phase 3: Sync Function
- Phase 4: Messages & Transient System
- Phase 5: Layout Algorithm
- Phase 8: Backward Compatibility

---

## Coverage Analysis

### Well-Covered Areas ✅

| Feature                          | Test File            | Coverage Level |
| -------------------------------- | -------------------- | -------------- |
| Cursor movement (all directions) | `cursor_movement.rs` | Excellent      |
| Smart Home/End                   | `cursor_movement.rs` | Excellent      |
| Word navigation                  | `cursor_movement.rs` | Excellent      |
| Text insertion/deletion          | `text_editing.rs`    | Excellent      |
| Undo/Redo                        | `text_editing.rs`    | Good           |
| Undo with selection              | `text_editing.rs`    | Good           |
| Duplicate/Delete line            | `text_editing.rs`    | Good           |
| Vertical scrolling               | `scrolling.rs`       | Excellent      |
| Horizontal scrolling             | `scrolling.rs`       | Good           |
| PageUp/Down                      | `scrolling.rs`       | Excellent      |
| Selection helpers                | `selection.rs`       | Good           |
| Rectangle selection              | `selection.rs`       | Basic          |
| Multi-cursor                     | `selection.rs`       | Basic          |
| Word selection                   | `selection.rs`       | Good           |
| Status bar segments              | `status_bar.rs`      | Excellent      |
| Window resize safety             | `monkey_tests.rs`    | Excellent      |

### Coverage Gaps 🔴

| Area                            | Current State | Impact                              |
| ------------------------------- | ------------- | ----------------------------------- |
| **Benchmarks**                  | None          | No performance regression detection |
| **Property-based tests**        | None          | Limited edge case discovery         |
| **Clipboard operations**        | None          | Copy/paste untested                 |
| **File I/O**                    | None          | Save/Load paths untested            |
| **Syntax highlighting**         | None          | Not implemented yet                 |
| **Multi-cursor editing**        | Basic         | Complex scenarios missing           |
| **Rectangle selection editing** | None          | Only cursor placement tested        |
| **Search/Replace**              | None          | Not implemented yet                 |

---

## Test Data Patterns

### Simple Documents

```rust
test_model("hello", 0, 0)
test_model("hello\nworld\n", 1, 0)
```

### Multi-line Generated Documents

```rust
let text = (0..50)
    .map(|i| format!("line{}", i))
    .collect::<Vec<_>>()
    .join("\n");
```

### Varying Line Lengths

```rust
let text = (0..50)
    .map(|i| if i % 3 == 0 { "short" } else { format!("long line {}", i) })
    .collect();
```

### Unicode Content

```rust
test_model("héllo wörld 🎉\n日本語テスト\n", 0, 0)
```

### Special Characters

```rust
let special = ['\0', '\t', '\r', '\n', '🎉', '日', 'é', '\u{FEFF}'];
```

---

## Missing Test Types

### 1. Benchmark Tests (Criterion)

**Proposed structure:**

```toml
# Cargo.toml additions
[dev-dependencies]
criterion = "0.5"

[[bench]]
name = "editor_benchmarks"
harness = false
```

```rust
// benches/editor_benchmarks.rs
use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};

fn bench_insert_char(c: &mut Criterion) {
    let mut group = c.benchmark_group("insert_char");

    for size in [100, 1000, 10000, 100000].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(size),
            size,
            |b, &size| {
                b.iter_batched(
                    || create_document_with_lines(size),
                    |mut model| {
                        for _ in 0..100 {
                            update(&mut model, Msg::Document(DocumentMsg::InsertChar('x')));
                        }
                        black_box(model)
                    },
                    criterion::BatchSize::SmallInput,
                );
            },
        );
    }
    group.finish();
}

fn bench_cursor_movement(c: &mut Criterion) {
    c.bench_function("cursor_down_10k_lines", |b| {
        let text: String = (0..10000).map(|i| format!("line {}\n", i)).collect();
        let mut model = test_model(&text, 0, 0);
        b.iter(|| {
            for _ in 0..1000 {
                update(&mut model, Msg::Editor(EditorMsg::MoveCursor(Direction::Down)));
            }
        });
    });
}

fn bench_undo_redo(c: &mut Criterion) {
    c.bench_function("undo_redo_cycle", |b| {
        b.iter_batched(
            || {
                let mut model = test_model("", 0, 0);
                for i in 0..1000 {
                    update(&mut model, Msg::Document(DocumentMsg::InsertChar('a')));
                }
                model
            },
            |mut model| {
                for _ in 0..500 {
                    update(&mut model, Msg::Document(DocumentMsg::Undo));
                    update(&mut model, Msg::Document(DocumentMsg::Redo));
                }
                black_box(model)
            },
            criterion::BatchSize::SmallInput,
        );
    });
}

criterion_group!(benches, bench_insert_char, bench_cursor_movement, bench_undo_redo);
criterion_main!(benches);
```

### 2. Property-Based Tests (Proptest)

**Proposed additions:**

```toml
# Cargo.toml
[dev-dependencies]
proptest = "1.4"
```

```rust
// tests/property_tests.rs
use proptest::prelude::*;

proptest! {
    #[test]
    fn undo_redo_inverse(s in "\\PC{0,100}") {
        let mut model = test_model("", 0, 0);
        for c in s.chars() {
            update(&mut model, Msg::Document(DocumentMsg::InsertChar(c)));
        }
        let after_insert = buffer_to_string(&model);

        // Undo all
        for _ in 0..s.len() {
            update(&mut model, Msg::Document(DocumentMsg::Undo));
        }

        // Redo all
        for _ in 0..s.len() {
            update(&mut model, Msg::Document(DocumentMsg::Redo));
        }

        prop_assert_eq!(buffer_to_string(&model), after_insert);
    }

    #[test]
    fn cursor_never_exceeds_bounds(
        text in "([^\n]{0,50}\n){0,20}",
        moves in prop::collection::vec(0..4u8, 0..100)
    ) {
        let mut model = test_model(&text, 0, 0);

        for m in moves {
            let dir = match m {
                0 => Direction::Up,
                1 => Direction::Down,
                2 => Direction::Left,
                _ => Direction::Right,
            };
            update(&mut model, Msg::Editor(EditorMsg::MoveCursor(dir)));

            let line_count = model.document.line_count();
            prop_assert!(model.editor.cursor().line < line_count);

            let line_len = model.document.line_length(model.editor.cursor().line);
            prop_assert!(model.editor.cursor().column <= line_len);
        }
    }

    #[test]
    fn selection_bounds_always_valid(
        text in "\\PC{10,200}",
        start in 0usize..10,
        end in 0usize..10
    ) {
        let len = text.len().min(10);
        let s = start % len;
        let e = end % len;

        let mut model = test_model(&text, 0, 0);
        // Set selection and verify operations don't panic
        model.editor.selection_mut().anchor.column = s;
        model.editor.selection_mut().head.column = e;

        update(&mut model, Msg::Document(DocumentMsg::DeleteBackward));
        // If we get here without panic, test passes
    }
}
```

### 3. Fuzzing (cargo-fuzz)

```rust
// fuzz/fuzz_targets/message_sequence.rs
#![no_main]
use libfuzzer_sys::fuzz_target;
use arbitrary::Arbitrary;

#[derive(Arbitrary, Debug)]
enum FuzzMsg {
    InsertChar(char),
    DeleteBackward,
    DeleteForward,
    MoveCursor(u8),
    Undo,
    Redo,
}

fuzz_target!(|messages: Vec<FuzzMsg>| {
    let mut model = test_model("initial text\nwith lines\n", 0, 0);

    for msg in messages {
        let _ = match msg {
            FuzzMsg::InsertChar(c) => update(&mut model, Msg::Document(DocumentMsg::InsertChar(c))),
            FuzzMsg::DeleteBackward => update(&mut model, Msg::Document(DocumentMsg::DeleteBackward)),
            // ... etc
        };
    }
});
```

---

## Recommendations

### Immediate Actions

1. **Add criterion benchmarks** for regression detection:
   - Insert/delete operations at various document sizes
   - Cursor movement through large documents
   - Undo/redo stack performance
   - Rendering frame time (if testable)

2. **Consolidate test helpers** - The `test_model_with_selection()` is duplicated in `main.rs` and `common/mod.rs`. Consider exposing it from the library.

3. **Add clipboard tests** - When clipboard functionality is stabilized.

### Medium-term Improvements

4. **Proptest for invariants**:
   - Cursor never exceeds document bounds
   - Selection anchor/head always valid positions
   - Undo/redo are perfect inverses
   - Buffer length matches expected after operations

5. **Integration test for file I/O** - Mock filesystem or use temp files.

6. **Visual regression tests** - Snapshot testing for rendered output.

### Long-term Goals

7. **Fuzzing infrastructure** - Continuous fuzz testing via cargo-fuzz.

8. **Benchmark CI integration** - Track performance over time.

9. **Coverage reporting** - `cargo-llvm-cov` integration.

---

## Makefile Integration

Current test commands from `Makefile`:

```makefile
make test           # Run all tests
make test-one TEST=name  # Run single test by name
```

Proposed additions:

```makefile
bench:
	cargo bench

bench-save:
	cargo bench -- --save-baseline main

bench-compare:
	cargo bench -- --baseline main

coverage:
	cargo llvm-cov --html

fuzz:
	cargo +nightly fuzz run message_sequence -- -max_len=1000
```

---

## Summary

The test suite is **well-organized** with clear patterns and comprehensive coverage of core editing functionality. The main gaps are:

| Gap                    | Priority | Effort |
| ---------------------- | -------- | ------ |
| Benchmarks (criterion) | High     | Medium |
| Property-based tests   | Medium   | Low    |
| Clipboard tests        | Medium   | Low    |
| File I/O tests         | Medium   | Medium |
| Fuzzing                | Low      | High   |

The existing monkey tests provide good stress coverage, but adding proptest would systematically discover edge cases that manual test writing misses.
