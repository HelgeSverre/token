# Codebase Analysis Plan: rust-editor Benchmarking, Testing & DX

**Created:** 2025-01-XX  
**Status:** ✅ COMPLETE  
**Objective:** Comprehensive analysis to identify hot paths for benchmarking, assess test infrastructure, discover DX improvement opportunities, and document existing performance-related code.

> **Summary:** Analysis complete. See [SYNTHESIS.md](analysis/SYNTHESIS.md) for prioritized action plan.
> 
> **Top Finding:** `cursor_to_offset()` and `offset_to_cursor()` are O(n) but should be O(log n) — 3-line fixes each.

---

## Executive Summary

The rust-editor is a minimal text editor (~2,100+ lines core code) implementing the **Elm Architecture** in Rust. It uses:
- **ropey** for O(log n) text buffer operations
- **fontdue** for CPU font rasterization  
- **softbuffer** for framebuffer presentation
- **winit** for window/event handling

Current state: 253 tests (24 lib + 14 main + 215 integration), existing PerfStats infrastructure (debug builds only), F2 perf overlay, but **no formal benchmarks**.

---

## Analysis Scope

### 1. Hot Paths for Benchmarking

| Category | Files | Key Functions/Areas |
|----------|-------|---------------------|
| **Text Operations** | `src/model/document.rs`, `src/update.rs` | `cursor_to_offset()`, `offset_to_cursor()`, word navigation, selection operations |
| **Rendering Pipeline** | `src/main.rs` (Renderer) | `render()`, `draw_text()`, line rendering loop, selection highlight |
| **Glyph Caching** | `src/main.rs` | `GlyphCache` HashMap lookups, cache miss rasterization |
| **Viewport Scrolling** | `src/update.rs`, `src/model/editor.rs` | `ensure_cursor_visible_with_mode()`, viewport calculations |

### 2. Test Infrastructure Analysis

| Aspect | Files | Current State |
|--------|-------|---------------|
| **Test Helpers** | `tests/common/mod.rs` | `test_model()`, `test_model_with_selection()` |
| **Test Categories** | `tests/*.rs` | cursor_movement, text_editing, selection, scrolling, edge_cases, monkey_tests, status_bar |
| **Test Coverage** | 253 tests total | Good coverage of core operations, lacking perf/benchmark tests |

### 3. DX Improvement Opportunities

| Area | Current State | Potential Improvements |
|------|---------------|------------------------|
| **Build Workflow** | Makefile with basic targets | Watch mode, cargo-watch integration |
| **Debugging** | F2 perf overlay (debug only) | Enhanced debug logging, state inspector |
| **Performance Profiling** | PerfStats struct (partial) | Integrate with criterion, flamegraph |
| **Test Feedback** | `cargo test` output | Coverage reporting, mutation testing |

### 4. Existing Performance Code

| Component | Location | Status |
|-----------|----------|--------|
| **PerfStats struct** | `src/main.rs:31-57` | Frame timing, cache stats, render breakdown (debug only) |
| **Glyph Cache** | `src/main.rs:26-27` | `HashMap<(char, u32), (Metrics, Vec<u8>)>` |
| **Frame Time Tracking** | `src/main.rs:68-76` | Rolling 60-frame window |
| **F2 Toggle** | `src/main.rs` | Shows overlay with FPS, frame time, cache hit rate |

---

## Analysis Agents

### Agent 1: Hot Path Analyzer (Text Operations)
**Focus:** `src/model/document.rs`, `src/update.rs`, `src/util.rs`
**Deliverables:**
- Identify O(n) operations that could benefit from benchmarking
- Document word navigation algorithms (`move_cursor_word_left/right`)
- Analyze `cursor_to_offset()` and `offset_to_cursor()` for large documents
- Identify multi-cursor operations with O(cursors × text) complexity

### Agent 2: Rendering Pipeline Analyzer
**Focus:** `src/main.rs` (Renderer struct and impl)
**Deliverables:**
- Document the rendering pipeline stages
- Identify frame-time-critical paths
- Analyze selection/highlight rendering (nested loops)
- Document glyph cache effectiveness patterns

### Agent 3: Test Infrastructure Reviewer
**Focus:** `tests/`, `src/main.rs` (inline tests), `src/lib.rs`
**Deliverables:**
- Catalog test patterns and helpers
- Identify gaps in test coverage
- Recommend benchmark test structure
- Document test naming conventions

### Agent 4: DX Improvement Specialist
**Focus:** `Makefile`, `CLAUDE.md`, `.github/`, project structure
**Deliverables:**
- Recommend development workflow improvements
- Suggest tooling additions (cargo-watch, bacon, etc.)
- Document debugging strategies for performance issues
- Propose benchmark integration with CI

### Agent 5: Performance Infrastructure Reviewer
**Focus:** `src/main.rs` (PerfStats), `src/overlay.rs`
**Deliverables:**
- Document existing PerfStats implementation
- Recommend extensions for automated benchmarking
- Design headless benchmark export (JSON stats)
- Propose criterion benchmark structure

---

## Deliverables

### Primary Documentation

| Document | Location | Description |
|----------|----------|-------------|
| **Hot Paths Report** | `docs/analysis/HOT_PATHS.md` | Detailed analysis of performance-critical code paths |
| **Benchmark Plan** | `docs/analysis/BENCHMARK_PLAN.md` | Recommended benchmarks with justification |
| **Test Infrastructure Report** | `docs/analysis/TEST_INFRASTRUCTURE.md` | Current state and recommendations |
| **DX Recommendations** | `docs/analysis/DX_IMPROVEMENTS.md` | Actionable DX improvements |
| **Performance Code Audit** | `docs/analysis/PERFORMANCE_AUDIT.md` | Documentation of existing perf infrastructure |

### Secondary Artifacts

- Benchmark skeleton code (criterion setup)
- Enhanced Makefile targets for benchmarking
- Updated ROADMAP.md with performance-related tasks

---

## Analysis Sequence

```
┌─────────────────────────────────────────────────────────────────┐
│  Phase 1: Parallel Deep Analysis                                │
├─────────────────────────────────────────────────────────────────┤
│  Agent 1: Text Operations    │  Agent 2: Rendering Pipeline    │
│  Agent 3: Test Infrastructure│  Agent 4: DX Analysis           │
│  Agent 5: Perf Infrastructure│                                 │
└─────────────────────────────────────────────────────────────────┘
                                 │
                                 ▼
┌─────────────────────────────────────────────────────────────────┐
│  Phase 2: Synthesis                                             │
├─────────────────────────────────────────────────────────────────┤
│  - Cross-reference findings                                     │
│  - Prioritize benchmarks by impact                              │
│  - Generate actionable recommendations                          │
└─────────────────────────────────────────────────────────────────┘
                                 │
                                 ▼
┌─────────────────────────────────────────────────────────────────┐
│  Phase 3: Documentation                                         │
├─────────────────────────────────────────────────────────────────┤
│  - Create all deliverable documents                             │
│  - Update ROADMAP.md with performance tasks                     │
│  - Generate benchmark skeleton if approved                      │
└─────────────────────────────────────────────────────────────────┘
```

---

## Key Questions to Answer

1. **Hot Paths:**
   - Which operations scale poorly with document size (10k+ lines)?
   - What is the glyph cache hit rate under typical use?
   - How does multi-cursor count affect edit performance?

2. **Testing:**
   - Are there property-based tests for text operations?
   - Is there benchmark coverage for rendering?
   - How are edge cases with large files tested?

3. **DX:**
   - What is the current debug-to-fix cycle time?
   - Are there watch-mode workflows for rapid iteration?
   - How is performance regression detected?

4. **Performance Infrastructure:**
   - Can PerfStats be extended for automated benchmarks?
   - What export format would enable CI performance tracking?
   - Is the overlay system adequate for profiling?

---

## Completion Status

**Phase 1: ✅ Complete** - All 5 agents deployed and delivered reports
**Phase 2: ✅ Complete** - Synthesis document created with prioritized recommendations  
**Phase 3: ✅ Complete** - All documentation generated

### Generated Documents

| Document | Status | Key Findings |
|----------|--------|--------------|
| [HOT_PATHS.md](analysis/HOT_PATHS.md) | ✅ | `cursor_to_offset` is O(n), should be O(log n) |
| [RENDERING_PIPELINE.md](analysis/RENDERING_PIPELINE.md) | ✅ | ~8.8M FLOPs/frame in text rendering |
| [TEST_INFRASTRUCTURE.md](analysis/TEST_INFRASTRUCTURE.md) | ✅ | 253 tests, but no benchmarks |
| [DX_IMPROVEMENTS.md](analysis/DX_IMPROVEMENTS.md) | ✅ | Missing watch mode, criterion |
| [PERFORMANCE_AUDIT.md](analysis/PERFORMANCE_AUDIT.md) | ✅ | PerfStats exists but unused |
| [SYNTHESIS.md](analysis/SYNTHESIS.md) | ✅ | Prioritized action plan |

---

## Top Priority Actions

From the synthesis, these are the highest-impact, lowest-effort fixes:

### Immediate (3-line fixes)
1. **T1-1:** Replace `cursor_to_offset()` loop with `line_to_char()` 
2. **T1-2:** Replace `offset_to_cursor()` loop with `char_to_line()`

### This Week
3. **T1-4:** Set up criterion benchmark harness
4. **T2-3:** Add `make watch` target for faster iteration

See [SYNTHESIS.md](analysis/SYNTHESIS.md) for full implementation roadmap.
