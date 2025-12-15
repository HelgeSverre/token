# CSV Editor Design Doc Gap Analysis

**Document Reviewed:** `docs/feature/csv-editor.md`
**Analysis Date:** 2025-12-15
**Status:** Reference document for implementation planning

---

## Summary

The CSV editor design document is comprehensive and well-structured. Cross-referencing with the codebase reveals several gaps in the design doc, missing test coverage, and implementation details that need clarification before implementation begins.

---

## 1. Gaps in Design Document

### 1.1 Theme Integration Missing

**Issue:** Design doc proposes `CsvTheme` (lines 1859-1877) but doesn't address:
- How to handle themes without a `csv` section (all current themes lack this)
- Default fallback colors when `csv` YAML section is missing

**Current theme files (none have csv section):**
- `themes/dark.yaml`
- `themes/fleet-dark.yaml`
- `themes/github-dark.yaml`
- `themes/github-light.yaml`

**Recommendation:** Add section on theme fallback defaults:
```rust
impl Default for CsvTheme {
    fn default() -> Self {
        Self {
            header_background: theme.gutter.background,      // Fallback
            header_foreground: theme.gutter.foreground_active,
            row_number_background: theme.gutter.background,
            row_number_foreground: theme.gutter.foreground,
            grid_line: theme.gutter.border_color,
            selection_border: theme.editor.cursor_color,
            number_foreground: theme.syntax.number,
            alternate_row_background: None,
        }
    }
}
```

### 1.2 Input/Keyboard Integration Missing

**Issue:** Design doc lists keyboard shortcuts (section 2.3) but doesn't show integration with actual input handling system.

**Actual codebase locations:**
- Input handling: `src/runtime/input.rs` (not `src/input.rs` as might be assumed)
- Keymap system: `src/keymap/` directory with `keymap.rs`, `winit_adapter.rs`

**Recommendation:** Add section showing how CsvMsg maps through keymap system:
- How key events are captured when in CSV mode
- Integration point with existing `KeymapCommand` enum
- Mode-aware key dispatch (different behavior in text vs CSV mode)

### 1.3 Internal Field Delimiter Edge Case

**Issue:** Design uses `0xFA` as internal field delimiter (line 292):
```rust
const FIELD_DELIMITER: u8 = 0xFA;
```

This doesn't document what happens if CSV data actually contains byte `0xFA`. While rare, this byte IS valid in multi-byte UTF-8 sequences and could theoretically appear in some content.

**Options to consider:**
1. Document that `0xFA` in source data will cause corruption (acceptable if rare)
2. Use escape sequence for internal storage
3. Use a struct-based approach instead of delimited strings

### 1.4 Multi-line Cell Sync Bug

**Issue:** The `find_cell_range()` function (lines 1159-1223) finds rows by counting newlines:
```rust
for (i, _) in content.match_indices('\n').take(pos.row) {
    row_start = i + 1;
}
```

This breaks for CSV files with quoted multi-line fields:
```csv
Name,Description
Alice,"Line 1
Line 2"
Bob,Simple
```

Row 2 ("Bob") would be miscounted because the quoted field contains a newline.

**Recommendation:** Sync logic must use CSV-aware row finding that respects quoted fields. Consider using the `csv` crate's `Reader` for row boundary detection.

### 1.5 Column Letter Algorithm Verification Needed

**Issue:** The `column_letter()` function (lines 381-391) should be verified:
```rust
n = n / 26 - 1;  // Line 389
```

Test cases in doc (lines 1610-1617):
- `column_letter(26)` should return `"AA"`
- `column_letter(701)` should return `"ZZ"`
- `column_letter(702)` should return `"AAA"`

The algorithm needs verification - the `-1` adjustment logic at boundaries should be tested before implementation.

### 1.6 ViewMode State Preservation Undefined

**Issue:** Design doesn't specify what happens to text mode state when toggling modes:

- When switching Text → CSV: Are cursor/selections preserved?
- When switching CSV → Text: Is CsvState preserved for quick re-toggle?
- Memory implications of storing both states?

**Options to document:**
```rust
// Option A: Discard state on toggle (simple, stateless)
pub enum ViewMode {
    Text,
    Csv(CsvState),
}

// Option B: Cache state for round-trip
pub enum ViewMode {
    Text { csv_cache: Option<CsvState> },
    Csv { csv: CsvState, text_cursor: Cursor },
}

// Option C: Always preserve both in EditorState separately
```

### 1.7 Benchmark File Naming Convention

**Issue:** Design proposes `benches/csv_benchmark.rs` (line 1428) but existing benchmarks use different naming:
- `rope_operations.rs`
- `rendering.rs`
- `glyph_cache.rs`
- `layout.rs`
- `search.rs`

**Recommendation:** Rename to `benches/csv.rs` to match convention.

---

## 2. Missing Test Cases

### 2.1 Unit Tests (Design Doc Section 12.1 Gaps)

| Missing Test | Why Important |
|--------------|---------------|
| `test_field_delimiter_in_data` | Verify behavior when source contains `0xFA` |
| `test_column_letter_boundaries` | Verify AA, ZZ, AAA edge cases correctly |
| `test_multiline_quoted_fields` | Ensure quoted newlines don't break row count |
| `test_parse_with_bom` | Handle UTF-8 BOM at file start |
| `test_empty_file` | Edge case: 0 rows, 0 columns |
| `test_single_column_csv` | No delimiters, just values |
| `test_trailing_delimiter` | Row ending with comma: `a,b,c,` |
| `test_unicode_content` | Multi-byte UTF-8 in cells |

### 2.2 Integration Tests (Design Doc Section 12.2 Gaps)

| Missing Test | Why Important |
|--------------|---------------|
| `test_toggle_preserves_text_state` | Cursor/selection after round-trip toggle |
| `test_multi_editor_csv_sync` | Same doc in 2 editors, one in CSV mode |
| `test_external_edit_reparse` | File changed externally while in CSV mode |
| `test_theme_without_csv_section` | Graceful fallback to defaults |
| `test_keyboard_shortcuts_csv_mode` | All keys work correctly in CSV mode |
| `test_status_bar_csv_segments` | Correct segment display for CSV |
| `test_auto_enable_on_file_open` | CSV mode triggers for .csv files |

### 2.3 Stress Tests (Design Doc Section 12.3 Gaps)

| Missing Test | Why Important |
|--------------|---------------|
| `test_wide_columns` | 1000+ char cell values |
| `test_many_columns` | 500+ columns |
| `test_memory_usage` | Verify delimited-string efficiency claim |
| `test_rapid_scroll` | Maintain 60fps during continuous scroll |
| `test_rapid_toggle` | Toggle mode 100x quickly without issues |

### 2.4 Render Tests (Not in Design Doc)

| Missing Test | Why Important |
|--------------|---------------|
| `test_csv_grid_render_empty` | Empty CSV renders without crash |
| `test_csv_viewport_clipping` | Cells outside viewport not rendered |
| `test_csv_selection_highlight` | Active cell has correct border |
| `test_csv_edit_cursor_blink` | Cursor visible/blinks in edit mode |
| `test_csv_horizontal_scroll` | Columns scroll correctly |

---

## 3. Codebase Integration Points

### 3.1 Status Bar - Compatible

**Good news:** `SegmentId::StatusMessage` already exists (`src/model/status_bar.rs` line 21). The design doc's status bar integration (lines 1322-1359) is compatible with existing infrastructure.

### 3.2 Document Revision - Compatible

**Good news:** `Document.revision: u64` already exists (`src/model/document.rs` line 70). The design doc's staleness detection via revision comparison will work as planned.

### 3.3 Frame Drawing Methods - Available

**Good news:** All required Frame methods exist in `src/view/frame.rs`:
- `fill_rect_px()` - Cell backgrounds
- `draw_bordered_rect()` - Grid lines
- `blend_rect()` - Selection overlay

### 3.4 EditorState - Needs ViewMode

**Required change:** `src/model/editor.rs` EditorState struct (lines 273-298) needs new `view_mode: ViewMode` field added.

### 3.5 Message System - Extension Ready

**Required change:** `src/messages.rs` needs new `CsvMsg` enum and `Msg::Csv(CsvMsg)` variant. Pattern is well-established with existing `SyntaxMsg`.

---

## 4. Questions to Resolve Before Implementation

1. **State preservation:** Should toggling CSV mode preserve text mode cursor/selection state for round-trip?

2. **Auto-enable behavior:** Should CSV mode be auto-enabled for .csv files, or always require manual toggle via `Cmd+Shift+T`?

3. **Size limits:** What's the maximum supported row/column count before showing a warning or refusing to enable CSV mode?

4. **Internal delimiter:** Should the `0xFA` internal delimiter be changed to avoid potential conflicts, or is the risk acceptable?

---

## 5. Architecture Notes

### Module Structure (Design Proposal)
```
src/csv/
├── mod.rs           # Module exports
├── parser.rs        # CSV parsing (using `csv` crate)
├── model.rs         # CsvState, CsvData, Cell types
├── viewport.rs      # CsvViewport calculations
├── navigation.rs    # Cell navigation logic
├── editing.rs       # Cell editing and sync (Phase 2)
└── render.rs        # CSV-specific rendering
```

### Alternative: Integrate with View
Consider whether CSV rendering should be in `src/csv/render.rs` or `src/view/csv.rs` alongside existing view code. The latter may provide better cohesion with the rendering pipeline.

---

## 6. Implementation Priority

Based on the gaps identified, recommended priority for Phase 1 (Read-Only Viewer):

1. **Critical (must fix before implementation):**
   - Theme fallback defaults
   - Multi-line cell sync logic

2. **Important (fix during implementation):**
   - Column letter algorithm verification
   - Input/keymap integration documentation
   - ViewMode state lifecycle

3. **Minor (can fix later):**
   - Internal delimiter edge case
   - Benchmark naming convention

---

*This document serves as a reference for implementation planning. No code changes were made as part of this analysis.*
