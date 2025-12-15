# UI System Code Review

**Date:** 2025-12-15
**Scope:** UI, Layout, and Text Rendering Subsystems
**Methodology:** 6 parallel specialized review agents

---

## Executive Summary

| Category | Count | Distribution |
|----------|-------|--------------|
| **Total Findings** | 47 | Critical: 3, High: 10, Medium: 19, Low: 15 |
| **Dead Code Items** | 17 | Safe to remove: 12, Keep: 5 |
| **Duplicate Patterns** | 23 | ~2,500 lines reducible to ~800 |
| **Migration Status** | 85-90% | Phases 0-6 complete, Phase 7 planned |

### Critical Issues Requiring Immediate Attention

1. **Hardcoded gutter width** (`editor_area.rs:451`) - Causes incorrect viewport calculations in split views
2. **min_sizes not enforced** (`editor_area.rs:476-554`) - Panes can be resized to 0, causing crashes
3. **Unsafe pointer dereference** (`editor_area.rs:276-280`) - Potential undefined behavior

---

## 1. Dead Code Inventory

### 1.1 Confirmed Dead (Safe to Remove)

#### Renderer Methods (`src/view/mod.rs`)
| Line | Method | Status | Recommendation |
|------|--------|--------|----------------|
| 98-101 | `font()` | Never called | REMOVE |
| 103-106 | `font_size()` | Never called | REMOVE |
| 108-111 | `line_height()` | Never called | REMOVE |
| 113-116 | `ascent()` | Never called | REMOVE |
| 118-121 | `line_metrics()` | Never called | REMOVE |
| 123-126 | `glyph_cache_mut()` | Never called | REMOVE |
| 1222-1233 | `get_char_width()` | Never called | REMOVE |

#### Geometry Functions (`src/view/geometry.rs`)
| Line | Function | Status | Recommendation |
|------|----------|--------|----------------|
| 37-44 | `compute_visible_lines()` | Never called | REMOVE |
| 47-54 | `compute_visible_columns()` | Never called | REMOVE |
| 133-136 | `is_in_tab_bar()` | Never called | REMOVE |
| 258-298 | `pixel_to_cursor_in_group()` | Never called | REMOVE |
| 317-322 | `group_gutter_rect()` | Never called | REMOVE |
| 326-336 | `group_text_area_rect()` | Never called | REMOVE |

#### Frame Methods (`src/view/frame.rs`)
| Line | Method | Status | Recommendation |
|------|--------|--------|----------------|
| 289-301 | `measure_width()` | Never called | REMOVE |

### 1.2 Actually Used (False Positives)
| Location | Item | Used By |
|----------|------|---------|
| `mod.rs:128-131` | `dimensions()` | `runtime/app.rs` |
| `frame.rs:76-83` | `get_pixel()` | Tests only (KEEP) |

### 1.3 Unused Struct Fields

#### Tab Fields (`src/model/editor_area.rs:68-69`)
```rust
pub struct Tab {
    pub is_pinned: bool,   // Set to false everywhere, never read
    pub is_preview: bool,  // Set to false everywhere, never read
}
```
**Recommendation:** Remove if no plans to implement, or add TODO with tracking issue.

#### SplitContainer.min_sizes (`src/model/editor_area.rs:119`)
- Created in `update/layout.rs:323`
- Removed in `update/layout.rs:390-393`
- **NEVER enforced** in `compute_layout_node()`
- **Impact:** CRITICAL - panes can be resized to 0

---

## 2. Duplicate Code for Refactoring

### 2.1 High Priority

#### Duplicate `tab_title()` Function
**Locations:**
- `src/view/mod.rs:28-42`
- `src/view/geometry.rs:146-160`

**Refactoring:**
```rust
// Extract to shared utility
pub(crate) fn get_tab_display_name(model: &AppModel, tab: &Tab) -> String {
    model.editor_area.editors.get(&tab.editor_id)
        .and_then(|e| e.document_id)
        .and_then(|doc_id| model.editor_area.documents.get(&doc_id))
        .map(|d| d.display_name())
        .unwrap_or_else(|| "Untitled".to_string())
}
```
**Effort:** 15 minutes

#### Line Trimming Pattern (7 occurrences)
**Locations:** `mod.rs:431, 486, 543, 616, 660` and `geometry.rs:212, 287`

**Pattern:**
```rust
let line_text_trimmed = if line_text.ends_with('\n') {
    &line_text[..line_text.len() - 1]
} else {
    &line_text
};
```

**Refactoring:**
```rust
#[inline]
pub fn trim_line_ending(text: &str) -> &str {
    text.strip_suffix('\n').unwrap_or(text)
}
```
**Effort:** 30 minutes

#### visible_columns Calculation (3 occurrences)
**Locations:** `mod.rs:552, 607, 649`

**Issue:** Same calculation repeated:
```rust
((rect_w as f32 - text_start_x_offset as f32) / char_width).floor() as usize
```

**Refactoring:** Calculate once at function start, reuse cached value.
**Effort:** 20 minutes

### 2.2 Medium Priority

#### Alpha Blending Calculation (3 occurrences)
**Locations:** `frame.rs:104-116, 260-276, 370-386`

**Refactoring:**
```rust
#[inline]
fn blend_colors(bg: u32, fg: u32, alpha: f32) -> u32 {
    // Unified blending logic
}
```
**Effort:** 1 hour

#### Multi-Cursor Loop Pattern (22 occurrences)
**Location:** `src/model/editor.rs:1103-1303`

**Pattern:**
```rust
pub fn move_all_cursors_X(&mut self, doc: &Document) {
    for i in 0..self.cursors.len() {
        self.move_cursor_X_at(doc, i);
    }
    self.deduplicate_cursors();
}
```

**Refactoring:**
```rust
fn apply_to_all_cursors<F>(&mut self, f: F) where F: FnMut(&mut Self, usize) {
    for i in 0..self.cursors.len() { f(self, i); }
    self.deduplicate_cursors();
}
```
**Effort:** 3 hours

### 2.3 Summary Table

| Pattern | Occurrences | Lines | Complexity | Priority |
|---------|-------------|-------|------------|----------|
| tab_title duplicate | 2 | 30 | Low | High |
| Line trimming | 7 | 35 | Low | High |
| visible_columns | 3 | 9 | Low | High |
| Alpha blending | 3 | 45 | Medium | Medium |
| Multi-cursor loops | 22 | 132 | Medium | High |
| Selection extension | 10 | 60 | Medium | High |
| Single vs multi branch | 8 | 800 | Very High | Medium |
| **Total Reducible** | **200+** | **~2,500** | - | - |

---

## 3. Migration Completeness

### 3.1 Phase Status Summary

| Phase | Description | Status | Completion |
|-------|-------------|--------|------------|
| 0 | Elm-Style Restructure | Mostly Complete | 95% |
| 1 | Frame/Painter Abstraction | Complete | 100% |
| 2 | Widget Extraction | Complete | 100% |
| 3 | Modal/Focus System | Complete | 100% |
| 4 | Command Palette | Complete | 100% |
| 5 | Compositor/Mouse Blocking | Complete | 100% |
| 6 | Goto/Find Modals | Mostly Complete | 80% |
| 7 | Damage Tracking | Not Started | 0% |

### 3.2 Incomplete Items

#### Phase 0 Deviations
- `overlay.rs` NOT moved to `view/overlay.rs` (minor)
- `messages.rs` and `commands.rs` NOT renamed (optional per plan)
- Tests not fully externalized: `main.rs` has 1040+ lines of tests

#### Phase 6 Gap
- **Find/Replace Confirm handler** needs verification
- Modal opens and renders, but search execution unclear

### 3.3 Bonus Features (Beyond Plan)
- ThemePicker modal (complete)
- Keymap system with user config (12 files, ~95KB)
- Tree-sitter syntax highlighting with incremental parsing
- 22 command palette commands (vs 17 planned)

---

## 4. Rendering Pipeline Issues

### 4.1 Critical: Hardcoded Gutter Width

**Location:** `src/model/editor_area.rs:451`

```rust
let gutter_width = 50.0;  // HARDCODED - WRONG!
```

**Actual formula:** `char_width * 5 + 4.0 + 1.0 + 8.0` = ~57-67px

**Impact:**
- `visible_columns` calculation wrong for split views
- Cursors can render past viewport bounds
- Selection highlights may overflow into gutter

**Fix:**
```rust
let gutter_width = text_start_x(char_width);  // Use actual formula
```

### 4.2 High: Repeated Calculations

**text_start_x() called 5+ times per render:**
- Lines 392, 552, 607, 649 in `render_text_area()`
- Should be calculated ONCE and cached

**Estimated performance impact:** 2-5% of render time

### 4.3 Positive Findings

- Glyph cache working correctly with efficient `entry()` API
- Color lookups properly cached outside line loops
- Early exit for off-screen lines implemented
- Line height consistently calculated via `LineMetrics`

---

## 5. Layout System Issues

### 5.1 Critical: min_sizes Not Enforced

**Location:** `src/model/editor_area.rs:476-554`

**Issue:** `SplitContainer.min_sizes` is populated but NEVER checked during `compute_layout_node()`.

**Impact:**
- Panes can be resized to 0 width/height
- Division by zero in viewport calculations
- Potential render crashes with 0-dimension buffers

**Fix:**
```rust
// In compute_layout_node, after calculating child_size:
let min_size = container.min_sizes.get(i).copied().unwrap_or(50.0);
let child_size = (total_size * ratio).max(min_size);
```

### 5.2 High: Unsafe Pointer Dereference

**Location:** `src/model/editor_area.rs:276-280`

```rust
let doc_ptr = self.documents.get(&doc_id).unwrap() as *const Document;
let editor = self.editors.get_mut(&editor_id).unwrap();
let doc = unsafe { &*doc_ptr };  // UNSAFE!
```

**Risk:** If HashMap reallocates during the function, pointer becomes dangling.

**Fix:** Extract needed data before mutable borrow, or split method.

### 5.3 High: Invalid Index Risks

**active_tab_index:** Can become invalid if tabs removed without updating index.

**focused_group_id:** Can point to non-existent group after closure.

**Recommendation:** Add `#[cfg(debug_assertions)] fn assert_invariants()` that validates data structure integrity.

### 5.4 Medium: Stack Overflow Risk

**Location:** Recursive `compute_layout_node()` with no depth limit.

**Fix:** Add depth counter, error if depth > 50.

---

## 6. API Consistency Issues

### 6.1 Encapsulation Issues

**Frame/TextPainter fields are public:**
```rust
pub struct Frame<'a> {
    pub buffer: &'a mut [u32],  // Should be private
    pub width: usize,            // Should be private
    pub height: usize,           // Should be private
}
```

**Recommendation:** Make fields private, add getters if needed.

### 6.2 Naming Inconsistencies

| Current | Recommended | Reason |
|---------|-------------|--------|
| `point_in_modal()` | `is_in_modal()` | Match `is_in_status_bar()` pattern |
| `pixel_to_line_and_visual_column()` | `pixel_to_line_visual_col()` | Shorter, consistent |

### 6.3 Duplicated Constants

```rust
// In src/view/geometry.rs:
pub const TABULATOR_WIDTH: usize = 4;

// In src/util/text.rs:
pub const TABULATOR_WIDTH: usize = 4;
```

**Fix:** Single source of truth, re-export if needed.

### 6.4 Documentation Gaps

Missing `///` documentation on public geometry functions:
- `tab_at_position()`, `pixel_to_cursor()`, `modal_bounds()`
- `group_content_rect()`, `group_gutter_rect()`, `group_text_area_rect()`

---

## 7. Recommendations

### 7.1 Immediate Actions (Critical)

1. **Fix hardcoded gutter width** in `sync_all_viewports()`
   - File: `src/model/editor_area.rs:451`
   - Effort: 5 minutes

2. **Enforce min_sizes** in `compute_layout_node()`
   - File: `src/model/editor_area.rs:476-554`
   - Effort: 30 minutes

3. **Replace unsafe pointer** with safe borrow pattern
   - File: `src/model/editor_area.rs:276-280`
   - Effort: 1 hour

### 7.2 Short-term Refactoring (High Priority)

4. **Extract `tab_title()` to shared utility**
   - Effort: 15 minutes

5. **Create `trim_line_ending()` helper**
   - Effort: 30 minutes

6. **Cache layout calculations** in `render_text_area()`
   - Effort: 20 minutes

7. **Remove dead code** (12 items identified)
   - Effort: 1 hour

8. **Add invariant validation** for debug builds
   - Effort: 2 hours

### 7.3 Long-term Improvements (Medium Priority)

9. **Unify multi-cursor loop pattern** (22 functions)
   - Effort: 3 hours

10. **Externalize tests from main.rs** (1040+ lines)
    - Effort: 3 hours

11. **Add documentation** to public geometry functions
    - Effort: 2 hours

12. **Make Frame/TextPainter fields private**
    - Effort: 1 hour

### 7.4 Deferred (Low Priority)

13. **Phase 7 Damage Tracking** - defer until profiling proves need
14. **Rename messages.rs/commands.rs** - high churn, minimal benefit
15. **Move overlay.rs to view/** - organizational only

---

## 8. Files Requiring Attention

### Critical Priority
| File | Issue | Lines |
|------|-------|-------|
| `src/model/editor_area.rs` | Hardcoded gutter, min_sizes, unsafe pointer | 276, 451, 476-554 |

### High Priority
| File | Issue | Lines |
|------|-------|-------|
| `src/view/mod.rs` | Dead code, duplicate tab_title, repeated calcs | 28-42, 98-131, 552-649 |
| `src/view/geometry.rs` | Dead functions, duplicate tab_title | 37-54, 133-336, 146-160 |
| `src/view/frame.rs` | Dead method, public fields | 76-83, 289-301 |

### Medium Priority
| File | Issue |
|------|-------|
| `src/model/editor.rs` | 22 duplicate multi-cursor loop patterns |
| `src/update/document.rs` | Single vs multi-cursor code duplication |
| `src/main.rs` | 1040+ lines of tests should be externalized |

---

## 9. Testing Recommendations

### Unit Tests Needed
1. `test_text_start_x_consistency()` - Verify all code paths use same formula
2. `test_min_sizes_enforcement()` - Panes respect minimum dimensions
3. `test_active_tab_index_bounds()` - Index always valid

### Integration Tests Needed
1. `test_split_view_cursor_bounds()` - Cursor renders correctly at edges
2. `test_split_view_resize_limits()` - Cannot resize below minimum
3. `test_empty_group_handling()` - No crash when last tab closed

---

## 10. Conclusion

The UI system is **well-architected** with clean Elm-style separation and proper modal/compositor infrastructure. The codebase has successfully completed 85-90% of the planned GUI cleanup migration.

**Key Strengths:**
- Clean Model-Update-View separation
- Proper Frame/TextPainter abstractions
- Comprehensive widget extraction
- Working modal focus capture
- Strong test coverage (700+ tests)

**Key Weaknesses:**
- 3 critical bugs (gutter width, min_sizes, unsafe pointer)
- ~2,500 lines of duplicate code
- 17 dead code items cluttering the API
- Some data structure invariants not enforced

**Overall Grade:** B+ (would be A- after fixing critical issues)

**Estimated Total Fix Effort:**
- Critical fixes: 2-3 hours
- High-priority refactoring: 8-10 hours
- Complete cleanup: 20-30 hours
