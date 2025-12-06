# Hot Paths Analysis for Benchmarking

This document identifies performance-critical text operations in the rust-editor codebase that would benefit from benchmarking.

## Complexity Analysis Table

| Function | Location | Current Complexity | Scaling Factor | Benchmark Priority |
|----------|----------|-------------------|----------------|-------------------|
| `cursor_to_offset()` | document.rs:121 | O(n) lines | Document line count | **HIGH** |
| `offset_to_cursor()` | document.rs:132 | O(n) lines | Document line count | **HIGH** |
| `move_cursor_word_left()` | update.rs:959 | O(n) chars | Position in document | **HIGH** |
| `move_cursor_word_right()` | update.rs:980 | O(n) chars | Remaining chars in document | **HIGH** |
| `last_non_whitespace_column()` | document.rs:159 | O(n) chars | Line length | MEDIUM |
| `cursors_in_reverse_order()` | update.rs:731 | O(k log k) | Cursor count | MEDIUM |
| Multi-cursor InsertChar | update.rs:751 | O(k × n) | Cursors × line count | **HIGH** |
| Multi-cursor DeleteBackward | update.rs:902 | O(k × n) | Cursors × line count | **HIGH** |
| Multi-cursor Paste | update.rs:574 | O(k × n) | Cursors × line count | **HIGH** |
| `delete_selection()` | update.rs:691 | O(n) | Selection size | MEDIUM |
| Undo/Redo | update.rs:1136 | O(1) | Stack pop + rope op | LOW |
| `SelectWord` | update.rs:423 | O(n) chars | Line length | LOW |
| `line_length()` | document.rs:105 | O(1) | Uses ropey | LOW |

---

## Hot Path Descriptions

### 1. Cursor ↔ Offset Conversion (CRITICAL)

**Location:** [document.rs:121-145](file:///Users/helge/code/rust-editor/src/model/document.rs#L121-L145)

**What it does:**
- `cursor_to_offset(line, column)` converts (line, column) position to byte offset
- `offset_to_cursor(offset)` converts byte offset back to (line, column)

**Current Implementation:**
```rust
pub fn cursor_to_offset(&self, line: usize, column: usize) -> usize {
    let mut pos = 0;
    for i in 0..line {  // O(n) iteration over ALL preceding lines
        if i < self.buffer.len_lines() {
            pos += self.buffer.line(i).len_chars();
        }
    }
    pos + column.min(self.line_length(line))
}
```

**Why it's critical:**
- Called on **every keystroke** for single cursor operations
- Called **k times per keystroke** for multi-cursor operations (see update.rs:757, 830, 907)
- Called for selection operations (copy/cut/paste, update.rs:508-509, 536-538)
- Scales linearly with document size

**Recommended Benchmark Scenarios:**
| Scenario | Lines | Column | Expected Behavior |
|----------|-------|--------|-------------------|
| Small doc, start | 100 | 0 | Baseline |
| Small doc, end | 100 | 99 | ~100 iterations |
| Medium doc, middle | 10,000 | 5000 | ~5000 iterations |
| Large doc, end | 100,000 | 99999 | ~100k iterations |

**Optimization Opportunity:** Use ropey's `line_to_char()` method which is O(log n).

---

### 2. Word Navigation (CRITICAL)

**Location:** [update.rs:959-1000](file:///Users/helge/code/rust-editor/src/update.rs#L959-L1000)

**What it does:**
- `move_cursor_word_left()` - scans backwards from cursor to find word boundary
- `move_cursor_word_right()` - scans forward from cursor to find word boundary

**Current Implementation:**
```rust
fn move_cursor_word_left(model: &mut AppModel) {
    let pos = model.cursor_buffer_position();  // Already O(n) via cursor_to_offset
    let text: String = model.document.buffer.slice(..pos).chars().collect();  // O(pos) allocation!
    let chars: Vec<char> = text.chars().collect();  // Another O(pos) allocation
    // ... then scan backwards
}
```

**Why it's critical:**
- Collects ALL text from document start to cursor into a String
- Then converts to `Vec<char>` - **double allocation**
- For a cursor at line 50000, allocates ~50000+ chars
- Triggered by Ctrl+Left/Right (common navigation)

**Recommended Benchmark Scenarios:**
| Scenario | Cursor Position | Line Length | Expected Behavior |
|----------|-----------------|-------------|-------------------|
| Start of document | 0 | N/A | Baseline (no alloc) |
| Line 100, col 50 | ~5000 | 80 | ~5KB allocation |
| Line 10000, col 40 | ~400000 | 80 | ~400KB allocation |
| End of 100k doc | ~4000000 | 80 | ~4MB allocation |

**Optimization Opportunity:** Use ropey's iterator directly without collecting to String.

---

### 3. Multi-Cursor Operations (HIGH)

**Location:** [update.rs:751-782](file:///Users/helge/code/rust-editor/src/update.rs#L751-L782) (InsertChar)

**What it does:**
- For each cursor (in reverse order), performs text operation
- Each operation calls `cursor_to_offset()` per cursor

**Pattern:**
```rust
let indices = cursors_in_reverse_order(model);  // O(k log k) sort
for idx in indices {
    let cursor = &model.editor.cursors[idx];
    let pos = model.document.cursor_to_offset(cursor.line, cursor.column);  // O(n) each!
    model.document.buffer.insert_char(pos, ch);  // O(log n) - ropey is efficient
    // ...
}
```

**Why it's critical:**
- Total complexity: **O(k × n)** where k = cursor count, n = line count
- With 50 cursors in a 10k line document: 50 × 10000 = 500,000 iterations
- Called on every character typed

**Recommended Benchmark Scenarios:**
| Scenario | Cursors | Document Lines | Operation |
|----------|---------|----------------|-----------|
| Single cursor baseline | 1 | 10000 | InsertChar |
| 10 cursors | 10 | 10000 | InsertChar |
| 50 cursors | 50 | 10000 | InsertChar |
| 100 cursors | 100 | 10000 | InsertChar |
| 50 cursors, large doc | 50 | 100000 | InsertChar |

---

### 4. `cursors_in_reverse_order()` (MEDIUM)

**Location:** [update.rs:731-742](file:///Users/helge/code/rust-editor/src/update.rs#L731-L742)

**What it does:**
- Creates sorted list of cursor indices for reverse document order processing
- Essential for multi-cursor edits (prevents offset invalidation)

**Current Implementation:**
```rust
fn cursors_in_reverse_order(model: &AppModel) -> Vec<usize> {
    let mut indices: Vec<usize> = (0..model.editor.cursors.len()).collect();
    indices.sort_by(|&a, &b| { /* compare positions */ });
    indices
}
```

**Why it matters:**
- Called 8+ times throughout document operations (see grep results)
- O(k log k) sorting on each call
- Vec allocation on each call

**Recommended Benchmark Scenarios:**
| Cursor Count | Expected Sort Time |
|--------------|-------------------|
| 10 | Baseline |
| 50 | ~5x baseline |
| 200 | ~20x baseline |
| 1000 | ~100x baseline |

---

### 5. `last_non_whitespace_column()` (MEDIUM)

**Location:** [document.rs:159-167](file:///Users/helge/code/rust-editor/src/model/document.rs#L159-L167)

**What it does:**
- Finds the last non-whitespace character on a line
- Used for smart End key behavior

**Current Implementation:**
```rust
pub fn last_non_whitespace_column(&self, line_idx: usize) -> usize {
    let line = self.buffer.line(line_idx);
    let line_str: String = line.chars().collect();  // O(line_length) allocation
    let trimmed = line_str.trim_end_matches(|c: char| c.is_whitespace());
    trimmed.len()
}
```

**Why it matters:**
- Allocates entire line to String
- Could use iterator-based approach instead
- Called on Home/End key presses

**Recommended Benchmark Scenarios:**
| Line Length | Expected Behavior |
|-------------|-------------------|
| 80 chars | Baseline |
| 500 chars | ~6x allocation |
| 2000 chars (long line) | ~25x allocation |
| 10000 chars (minified) | ~125x allocation |

---

### 6. Selection Text Extraction (MEDIUM)

**Location:** [update.rs:709-714](file:///Users/helge/code/rust-editor/src/update.rs#L709-L714) (delete_selection)

**Pattern used throughout Copy/Cut/Paste:**
```rust
let deleted_text: String = model
    .document
    .buffer
    .slice(start_offset..end_offset)
    .chars()
    .collect();  // O(selection_size) allocation
```

**Why it matters:**
- Allocates proportional to selection size
- Large selections (select all in 100k file) = large allocations
- Required for undo stack

**Recommended Benchmark Scenarios:**
| Selection Size | Expected Behavior |
|----------------|-------------------|
| 100 chars | Baseline |
| 10000 chars | ~100x allocation |
| 1M chars | ~10000x allocation |

---

## Benchmark Recommendations

### Suggested Benchmark Functions

```rust
// benches/hot_paths.rs

use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};

/// Benchmark cursor_to_offset at various document positions
fn bench_cursor_to_offset(c: &mut Criterion) {
    let mut group = c.benchmark_group("cursor_to_offset");
    
    for lines in [100, 1_000, 10_000, 100_000] {
        let doc = create_document_with_lines(lines, 80);
        let target_line = lines / 2;  // Middle of document
        
        group.bench_with_input(
            BenchmarkId::new("lines", lines),
            &(doc, target_line),
            |b, (doc, line)| {
                b.iter(|| doc.cursor_to_offset(*line, 40));
            },
        );
    }
    group.finish();
}

/// Benchmark word navigation with varying cursor positions
fn bench_word_navigation(c: &mut Criterion) {
    let mut group = c.benchmark_group("word_navigation");
    
    for lines in [100, 1_000, 10_000] {
        let mut model = create_model_with_lines(lines, 80);
        // Position cursor at end
        model.editor.cursor_mut().line = lines - 1;
        model.editor.cursor_mut().column = 40;
        
        group.bench_with_input(
            BenchmarkId::new("word_left_from_line", lines),
            &model,
            |b, model| {
                b.iter(|| {
                    let mut m = model.clone();
                    move_cursor_word_left(&mut m);
                });
            },
        );
    }
    group.finish();
}

/// Benchmark multi-cursor insert with varying cursor counts
fn bench_multicursor_insert(c: &mut Criterion) {
    let mut group = c.benchmark_group("multicursor_insert");
    
    let doc_lines = 10_000;
    for cursor_count in [1, 10, 50, 100] {
        let mut model = create_model_with_lines(doc_lines, 80);
        add_cursors_at_intervals(&mut model, cursor_count, doc_lines / cursor_count);
        
        group.bench_with_input(
            BenchmarkId::new("cursors", cursor_count),
            &model,
            |b, model| {
                b.iter(|| {
                    let mut m = model.clone();
                    update_document(&mut m, DocumentMsg::InsertChar('x'));
                });
            },
        );
    }
    group.finish();
}

/// Benchmark cursors_in_reverse_order sorting
fn bench_cursor_sorting(c: &mut Criterion) {
    let mut group = c.benchmark_group("cursor_sorting");
    
    for count in [10, 50, 100, 500] {
        let model = create_model_with_cursors(count);
        
        group.bench_with_input(
            BenchmarkId::new("cursors", count),
            &model,
            |b, model| {
                b.iter(|| cursors_in_reverse_order(model));
            },
        );
    }
    group.finish();
}

/// Benchmark selection extraction for undo
fn bench_selection_extraction(c: &mut Criterion) {
    let mut group = c.benchmark_group("selection_extraction");
    
    for size in [100, 1_000, 10_000, 100_000] {
        let doc = create_document_with_chars(size);
        
        group.bench_with_input(
            BenchmarkId::new("chars", size),
            &doc,
            |b, doc| {
                b.iter(|| {
                    let _text: String = doc.buffer.slice(0..size).chars().collect();
                });
            },
        );
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_cursor_to_offset,
    bench_word_navigation,
    bench_multicursor_insert,
    bench_cursor_sorting,
    bench_selection_extraction,
);
criterion_main!(benches);
```

---

## Priority Summary

### Immediate (HIGH) - Fix before optimizing rendering:
1. **`cursor_to_offset`** - Use `line_to_char()` from ropey
2. **`move_cursor_word_left/right`** - Avoid collecting to String; use iterators
3. **Multi-cursor operations** - Cache line offsets or batch conversions

### Soon (MEDIUM) - Optimize for edge cases:
4. **`last_non_whitespace_column`** - Use iterator instead of collect
5. **`cursors_in_reverse_order`** - Consider caching or maintaining sorted order

### Later (LOW) - Already efficient:
6. **Undo/Redo** - Stack operations are O(1), ropey ops are O(log n)
7. **`line_count`/`line_length`** - Already O(1) via ropey

---

## Quick Wins

### Replace `cursor_to_offset` with ropey native:
```rust
pub fn cursor_to_offset(&self, line: usize, column: usize) -> usize {
    if line >= self.buffer.len_lines() {
        return self.buffer.len_chars();
    }
    let line_start = self.buffer.line_to_char(line);  // O(log n)!
    line_start + column.min(self.line_length(line))
}
```

### Replace `offset_to_cursor` with ropey native:
```rust
pub fn offset_to_cursor(&self, offset: usize) -> (usize, usize) {
    let clamped = offset.min(self.buffer.len_chars());
    let line = self.buffer.char_to_line(clamped);  // O(log n)!
    let line_start = self.buffer.line_to_char(line);
    (line, clamped - line_start)
}
```

These two changes alone would reduce the complexity of most hot paths from O(n) to O(log n).
