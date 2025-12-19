# Soft Wrap

Word wrapping without modifying the document

> **Status:** Planned
> **Priority:** P2
> **Effort:** XL
> **Created:** 2025-12-19
> **Milestone:** 4 - Hard Problems

---

## Table of Contents

1. [Overview](#overview)
2. [Architecture](#architecture)
3. [Data Structures](#data-structures)
4. [Keybindings](#keybindings)
5. [Implementation Plan](#implementation-plan)
6. [Testing Strategy](#testing-strategy)
7. [References](#references)

---

## Overview

### Current State

The editor currently:
- Uses horizontal scrolling for long lines
- Renders one logical line = one visual line
- Cursor positions are `(line, column)` in logical coordinates
- Viewport tracks `top_line` and `left_column` for scrolling

### Goals

1. **Per-pane soft wrap toggle**: Each editor view can independently enable/disable wrapping
2. **Logical/visual line mapping**: Cursor operations work on logical lines, display uses visual lines
3. **Wrap at word boundaries**: Prefer breaking at whitespace
4. **Preserve cursor position**: Toggle wrap without jumping cursor
5. **Correct scrolling**: Viewport calculates visible area based on visual lines
6. **Gutter line numbers**: Show logical line numbers, with continuation indicator for wrapped lines
7. **Selection across wraps**: Selections that span wrapped lines render correctly

### Non-Goals

- Hard wrap (reformatting document)
- Wrap width configuration (use viewport width)
- Wrap at character level only (always prefer word boundaries)
- Bidirectional text support

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         Soft Wrap Conceptual Model                           │
│                                                                              │
│  Logical Document:                                                           │
│  ┌──────────────────────────────────────────────────────────────────────┐   │
│  │ Line 0: "This is a very long line that needs to wrap around..."      │   │
│  │ Line 1: "Short line"                                                 │   │
│  │ Line 2: "Another long line that will require wrapping at the..."    │   │
│  └──────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
│  Visual Lines (with wrap width = 30):                                        │
│  ┌──────────────────────────────────────────────────────────────────────┐   │
│  │ VLine 0: "This is a very long line "      ← Logical Line 0, part 1  │   │
│  │ VLine 1: "that needs to wrap around..."   ← Logical Line 0, part 2  │   │
│  │ VLine 2: "Short line"                     ← Logical Line 1          │   │
│  │ VLine 3: "Another long line that "        ← Logical Line 2, part 1  │   │
│  │ VLine 4: "will require wrapping at "      ← Logical Line 2, part 2  │   │
│  │ VLine 5: "the..."                         ← Logical Line 2, part 3  │   │
│  └──────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
│  WrapCache:                                                                  │
│  ┌──────────────────────────────────────────────────────────────────────┐   │
│  │ logical_line 0 → [WrapSegment { start: 0, len: 26, visual_line: 0 }, │   │
│  │                   WrapSegment { start: 26, len: 24, visual_line: 1 }]│   │
│  │ logical_line 1 → [WrapSegment { start: 0, len: 10, visual_line: 2 }] │   │
│  │ logical_line 2 → [WrapSegment { start: 0, len: 22, visual_line: 3 }, │   │
│  │                   WrapSegment { start: 22, len: 23, visual_line: 4 },│   │
│  │                   WrapSegment { start: 45, len: 6, visual_line: 5 }] │   │
│  │                                                                       │   │
│  │ total_visual_lines: 6                                                 │   │
│  └──────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Coordinate Systems

```
Logical Coordinates: (line, column)
  - Used by: Cursor, Selection, Document operations
  - Line = index into rope.line(n)
  - Column = character offset within that line

Visual Coordinates: (visual_line, visual_column)
  - Used by: Rendering, Mouse hit-testing
  - Visual_line = index after wrapping
  - Visual_column = offset within that visual line segment

Conversion:
  logical_to_visual(line, col) → (visual_line, visual_col)
  visual_to_logical(visual_line, visual_col) → (line, col)
```

### Module Structure

```
src/
├── wrap.rs                   # NEW: WrapCache, wrap computation
├── model/
│   └── editor.rs             # Add soft_wrap: bool, wrap_cache: WrapCache
├── view.rs                   # Use wrap cache for rendering
├── update/
│   └── editor.rs             # Coordinate conversion in cursor movement
└── input.rs                  # Mouse position conversion
```

---

## Data Structures

### WrapCache

```rust
// In src/wrap.rs

use std::collections::HashMap;

/// Cache of wrap information for a document
///
/// This cache is rebuilt when:
/// - Document content changes (edit, load)
/// - Viewport width changes (resize, split)
/// - Soft wrap is toggled on
#[derive(Debug, Clone)]
pub struct WrapCache {
    /// Wrap segments per logical line
    /// Key: logical line index
    /// Value: list of segments that line wraps into
    lines: HashMap<usize, Vec<WrapSegment>>,

    /// Total number of visual lines (sum of all segments)
    pub total_visual_lines: usize,

    /// Width used for wrapping (in characters)
    wrap_width: usize,

    /// Document revision when cache was built
    revision: u64,

    /// Whether cache is valid
    valid: bool,
}

/// A segment of a logical line after wrapping
#[derive(Debug, Clone, Copy)]
pub struct WrapSegment {
    /// Character offset within the logical line where this segment starts
    pub start_col: usize,

    /// Number of characters in this segment
    pub len: usize,

    /// Visual line index (global, not per-logical-line)
    pub visual_line: usize,

    /// Whether this is a continuation (not the first segment)
    pub is_continuation: bool,
}

impl WrapCache {
    pub fn new() -> Self {
        Self {
            lines: HashMap::new(),
            total_visual_lines: 0,
            wrap_width: 80,
            revision: 0,
            valid: false,
        }
    }

    /// Invalidate the cache (e.g., on document edit)
    pub fn invalidate(&mut self) {
        self.valid = false;
    }

    /// Check if cache needs rebuilding
    pub fn needs_rebuild(&self, doc_revision: u64, width: usize) -> bool {
        !self.valid || self.revision != doc_revision || self.wrap_width != width
    }

    /// Rebuild the wrap cache for the entire document
    pub fn rebuild(&mut self, document: &Document, wrap_width: usize, tab_width: usize) {
        self.lines.clear();
        self.wrap_width = wrap_width;
        self.revision = document.revision;

        let mut visual_line = 0;

        for logical_line in 0..document.line_count() {
            let line_text = document.get_line(logical_line)
                .unwrap_or_default()
                .trim_end_matches('\n')
                .to_string();

            let segments = self.compute_line_wraps(&line_text, wrap_width, tab_width, visual_line);
            let segment_count = segments.len();

            self.lines.insert(logical_line, segments);
            visual_line += segment_count;
        }

        self.total_visual_lines = visual_line;
        self.valid = true;
    }

    /// Compute wrap segments for a single line
    fn compute_line_wraps(
        &self,
        line: &str,
        wrap_width: usize,
        tab_width: usize,
        starting_visual_line: usize,
    ) -> Vec<WrapSegment> {
        if line.is_empty() {
            // Empty line still occupies one visual line
            return vec![WrapSegment {
                start_col: 0,
                len: 0,
                visual_line: starting_visual_line,
                is_continuation: false,
            }];
        }

        let mut segments = Vec::new();
        let mut current_start = 0;
        let mut current_visual_width = 0;
        let mut last_break_point = None; // (char_index, visual_width at that point)
        let chars: Vec<char> = line.chars().collect();
        let mut visual_line = starting_visual_line;

        for (i, &ch) in chars.iter().enumerate() {
            // Calculate visual width of this character
            let char_width = if ch == '\t' {
                tab_width - (current_visual_width % tab_width)
            } else {
                1 // TODO: Handle wide Unicode characters
            };

            // Check if this character would exceed wrap width
            if current_visual_width + char_width > wrap_width && current_start < i {
                // Need to wrap
                let break_at = if let Some((break_idx, _)) = last_break_point {
                    // Wrap at last word boundary
                    break_idx + 1 // Include the space in this segment
                } else {
                    // No word boundary found, force break at current position
                    i
                };

                segments.push(WrapSegment {
                    start_col: current_start,
                    len: break_at - current_start,
                    visual_line,
                    is_continuation: !segments.is_empty(),
                });

                visual_line += 1;
                current_start = break_at;
                current_visual_width = 0;
                last_break_point = None;

                // Re-process characters from break point to current
                for j in break_at..=i {
                    let ch = chars[j];
                    let w = if ch == '\t' {
                        tab_width - (current_visual_width % tab_width)
                    } else {
                        1
                    };
                    current_visual_width += w;
                    if ch.is_whitespace() {
                        last_break_point = Some((j, current_visual_width));
                    }
                }
                continue;
            }

            current_visual_width += char_width;

            // Track potential break points (after whitespace)
            if ch.is_whitespace() {
                last_break_point = Some((i, current_visual_width));
            }
        }

        // Final segment
        if current_start < chars.len() || segments.is_empty() {
            segments.push(WrapSegment {
                start_col: current_start,
                len: chars.len() - current_start,
                visual_line,
                is_continuation: !segments.is_empty(),
            });
        }

        segments
    }

    /// Convert logical position to visual position
    pub fn logical_to_visual(&self, line: usize, col: usize) -> (usize, usize) {
        if let Some(segments) = self.lines.get(&line) {
            for seg in segments {
                if col >= seg.start_col && col < seg.start_col + seg.len {
                    return (seg.visual_line, col - seg.start_col);
                }
                if col == seg.start_col + seg.len && seg == segments.last().unwrap() {
                    // Cursor at end of line
                    return (seg.visual_line, seg.len);
                }
            }
            // Past end of line, return last segment's end
            if let Some(last) = segments.last() {
                return (last.visual_line, last.len);
            }
        }
        (line, col) // Fallback (unwrapped)
    }

    /// Convert visual position to logical position
    pub fn visual_to_logical(&self, visual_line: usize, visual_col: usize) -> (usize, usize) {
        for (&logical_line, segments) in &self.lines {
            for seg in segments {
                if seg.visual_line == visual_line {
                    let col = seg.start_col + visual_col.min(seg.len);
                    return (logical_line, col);
                }
            }
        }
        (visual_line, visual_col) // Fallback
    }

    /// Get the visual line for a logical line (first segment)
    pub fn logical_line_to_visual(&self, logical_line: usize) -> usize {
        self.lines
            .get(&logical_line)
            .and_then(|segs| segs.first())
            .map(|seg| seg.visual_line)
            .unwrap_or(logical_line)
    }

    /// Get logical line from visual line
    pub fn visual_line_to_logical(&self, visual_line: usize) -> usize {
        for (&logical_line, segments) in &self.lines {
            for seg in segments {
                if seg.visual_line == visual_line {
                    return logical_line;
                }
            }
        }
        visual_line
    }

    /// Get segments for a logical line
    pub fn get_segments(&self, logical_line: usize) -> Option<&[WrapSegment]> {
        self.lines.get(&logical_line).map(|v| v.as_slice())
    }

    /// Check if a visual line is a continuation (wrapped)
    pub fn is_continuation(&self, visual_line: usize) -> bool {
        for segments in self.lines.values() {
            for seg in segments {
                if seg.visual_line == visual_line {
                    return seg.is_continuation;
                }
            }
        }
        false
    }

    /// Get visual line count for a logical line
    pub fn visual_line_count(&self, logical_line: usize) -> usize {
        self.lines.get(&logical_line).map(|s| s.len()).unwrap_or(1)
    }
}
```

### EditorState Extension

```rust
// In src/model/editor.rs

pub struct EditorState {
    // ... existing fields ...

    /// Whether soft wrap is enabled for this editor
    pub soft_wrap: bool,

    /// Wrap cache (only used when soft_wrap is true)
    pub wrap_cache: WrapCache,
}

impl EditorState {
    /// Toggle soft wrap and rebuild cache if needed
    pub fn toggle_soft_wrap(&mut self, document: &Document, wrap_width: usize, tab_width: usize) {
        self.soft_wrap = !self.soft_wrap;
        if self.soft_wrap {
            self.wrap_cache.rebuild(document, wrap_width, tab_width);
        } else {
            self.wrap_cache.invalidate();
        }
    }

    /// Ensure wrap cache is up to date
    pub fn ensure_wrap_cache(
        &mut self,
        document: &Document,
        wrap_width: usize,
        tab_width: usize,
    ) {
        if self.soft_wrap && self.wrap_cache.needs_rebuild(document.revision, wrap_width) {
            self.wrap_cache.rebuild(document, wrap_width, tab_width);
        }
    }

    /// Get the visual line for the current cursor
    pub fn cursor_visual_line(&self) -> usize {
        if self.soft_wrap {
            let cursor = self.active_cursor();
            self.wrap_cache.logical_to_visual(cursor.line, cursor.column).0
        } else {
            self.active_cursor().line
        }
    }
}
```

### Messages

```rust
// In src/messages.rs

pub enum EditorMsg {
    // ... existing variants ...

    /// Toggle soft wrap for current editor
    ToggleSoftWrap,

    /// Move cursor to next visual line (respects wrapping)
    MoveCursorVisualDown,

    /// Move cursor to previous visual line (respects wrapping)
    MoveCursorVisualUp,
}
```

---

## Keybindings

| Action | Mac | Windows/Linux | Message |
|--------|-----|---------------|---------|
| Toggle soft wrap | Alt+Z | Alt+Z | `ToggleSoftWrap` |
| Move down (visual) | Down | Down | `MoveCursor(Down)` (wrap-aware) |
| Move up (visual) | Up | Up | `MoveCursor(Up)` (wrap-aware) |

When soft wrap is enabled:
- **Up/Down arrow**: Move by visual line (may stay on same logical line)
- **Home/End**: Go to start/end of logical line (not visual segment)
- **Cmd+Up/Down**: Go to document start/end (unchanged)

---

## Implementation Plan

### Phase 1: WrapCache Core

**Estimated effort: 3-4 days**

1. [ ] Create `src/wrap.rs` with `WrapCache` and `WrapSegment`
2. [ ] Implement `compute_line_wraps()` with word boundary detection
3. [ ] Implement `logical_to_visual()` and `visual_to_logical()` conversions
4. [ ] Add comprehensive unit tests for wrap computation
5. [ ] Handle edge cases (empty lines, very long words, tabs)

**Test:** Wrap cache produces correct segments for various line lengths

### Phase 2: EditorState Integration

**Estimated effort: 2 days**

1. [ ] Add `soft_wrap: bool` and `wrap_cache: WrapCache` to `EditorState`
2. [ ] Add `toggle_soft_wrap()` method
3. [ ] Add `ensure_wrap_cache()` method
4. [ ] Invalidate cache on document edit (hook into `push_edit`)
5. [ ] Add `ToggleSoftWrap` message handling

**Test:** Toggle works, cache invalidates on edit

### Phase 3: Rendering with Wrap

**Estimated effort: 4-5 days**

1. [ ] Modify `render_document()` to iterate visual lines
2. [ ] For each visual line, render the correct segment of the logical line
3. [ ] Calculate `visible_visual_lines` for viewport
4. [ ] Update gutter to show logical line numbers correctly
5. [ ] Show continuation indicator (e.g., no line number, or `...`)

```rust
// Rendering pseudocode
fn render_with_wrap(
    &mut self,
    editor: &EditorState,
    document: &Document,
    viewport_visual_start: usize,
    viewport_visual_count: usize,
) {
    for visual_line in viewport_visual_start..(viewport_visual_start + viewport_visual_count) {
        let logical_line = editor.wrap_cache.visual_line_to_logical(visual_line);
        let is_continuation = editor.wrap_cache.is_continuation(visual_line);

        // Find which segment of the logical line this visual line represents
        if let Some(segments) = editor.wrap_cache.get_segments(logical_line) {
            for seg in segments {
                if seg.visual_line == visual_line {
                    let line_text = document.get_line(logical_line).unwrap_or_default();
                    let segment_text = &line_text[seg.start_col..seg.start_col + seg.len];

                    // Render line number (only for first segment)
                    if !is_continuation {
                        self.draw_line_number(logical_line + 1, y);
                    } else {
                        self.draw_continuation_marker(y);
                    }

                    // Render text segment
                    self.draw_text(segment_text, x, y, ...);
                    break;
                }
            }
        }
    }
}
```

**Test:** Long lines wrap visually, line numbers are correct

### Phase 4: Cursor Movement with Wrap

**Estimated effort: 3-4 days**

1. [ ] Modify `MoveCursor(Up)` to move by visual line when wrapped
2. [ ] Modify `MoveCursor(Down)` to move by visual line when wrapped
3. [ ] Preserve `desired_column` across visual line movements
4. [ ] `Home` goes to start of logical line (not visual segment)
5. [ ] `End` goes to end of logical line (not visual segment)
6. [ ] Handle cursor visibility in viewport (scroll by visual lines)

```rust
// Moving cursor up with wrap
fn move_cursor_visual_up(editor: &mut EditorState, document: &Document) {
    let cursor = editor.active_cursor();
    let (visual_line, visual_col) = editor.wrap_cache.logical_to_visual(cursor.line, cursor.column);

    if visual_line > 0 {
        // Move to previous visual line
        let target_visual_line = visual_line - 1;
        let desired = cursor.desired_column.unwrap_or(visual_col);

        // Convert back to logical coordinates
        let (new_logical_line, new_col) = editor.wrap_cache.visual_to_logical(
            target_visual_line,
            desired,
        );

        let cursor = editor.active_cursor_mut();
        cursor.line = new_logical_line;
        cursor.column = new_col.min(document.line_length(new_logical_line));
        cursor.desired_column = Some(desired);
    }
}
```

**Test:** Up/Down arrow navigates within wrapped lines correctly

### Phase 5: Selection Rendering with Wrap

**Estimated effort: 2-3 days**

1. [ ] Calculate selection rectangles per visual line
2. [ ] For selections spanning multiple visual lines, draw separate rects
3. [ ] Handle selection start/end at visual line boundaries
4. [ ] Ensure selection highlighting extends to wrap point

**Test:** Select across wrapped lines, verify visual correctness

### Phase 6: Mouse Interaction with Wrap

**Estimated effort: 2 days**

1. [ ] Convert click Y position to visual line
2. [ ] Convert click X position to visual column
3. [ ] Use `visual_to_logical()` to get cursor position
4. [ ] Handle double-click word selection across wraps
5. [ ] Handle triple-click line selection (select logical line)

**Test:** Click on wrapped line positions cursor correctly

### Phase 7: Viewport Scrolling with Wrap

**Estimated effort: 2 days**

1. [ ] Change viewport tracking to use visual lines
2. [ ] Calculate `visible_visual_lines` from viewport height
3. [ ] Scroll by visual lines, not logical lines
4. [ ] Ensure cursor reveal works with visual line count
5. [ ] Handle Page Up/Down by visual lines

**Test:** Scroll behavior works correctly with wrapped content

### Phase 8: Incremental Cache Updates (Optimization)

**Estimated effort: 3-4 days**

1. [ ] Track edit range (start line, end line)
2. [ ] Only recompute wrap for affected lines
3. [ ] Update visual line indices for lines after edit
4. [ ] Handle insert/delete of entire lines efficiently
5. [ ] Benchmark and optimize for large files

**Test:** Performance acceptable with large wrapped files

---

## Testing Strategy

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_wrap_short_line() {
        let mut cache = WrapCache::new();
        let segments = cache.compute_line_wraps("hello", 80, 4, 0);

        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].start_col, 0);
        assert_eq!(segments[0].len, 5);
        assert_eq!(segments[0].visual_line, 0);
        assert!(!segments[0].is_continuation);
    }

    #[test]
    fn test_wrap_at_word_boundary() {
        let mut cache = WrapCache::new();
        // 15 chars: "hello world foo"
        // Wrap at width 12 should break after "world"
        let segments = cache.compute_line_wraps("hello world foo", 12, 4, 0);

        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0].len, 12); // "hello world "
        assert_eq!(segments[1].start_col, 12);
        assert_eq!(segments[1].len, 3); // "foo"
        assert!(segments[1].is_continuation);
    }

    #[test]
    fn test_wrap_no_break_point() {
        let mut cache = WrapCache::new();
        // Word too long, must force break
        let segments = cache.compute_line_wraps("abcdefghijklmnop", 10, 4, 0);

        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0].len, 10);
        assert_eq!(segments[1].len, 6);
    }

    #[test]
    fn test_logical_to_visual_unwrapped() {
        let doc = Document::with_text("hello\nworld");
        let mut cache = WrapCache::new();
        cache.rebuild(&doc, 80, 4);

        assert_eq!(cache.logical_to_visual(0, 0), (0, 0));
        assert_eq!(cache.logical_to_visual(0, 3), (0, 3));
        assert_eq!(cache.logical_to_visual(1, 0), (1, 0));
    }

    #[test]
    fn test_logical_to_visual_wrapped() {
        let doc = Document::with_text("hello world foo bar baz");
        let mut cache = WrapCache::new();
        cache.rebuild(&doc, 12, 4); // Wrap at 12 chars

        // "hello world " (12) + "foo bar baz" (11)
        assert_eq!(cache.logical_to_visual(0, 0), (0, 0));
        assert_eq!(cache.logical_to_visual(0, 11), (0, 11)); // end of "world"
        assert_eq!(cache.logical_to_visual(0, 12), (1, 0));  // start of "foo"
        assert_eq!(cache.logical_to_visual(0, 15), (1, 3));  // mid "foo bar"
    }

    #[test]
    fn test_visual_to_logical_roundtrip() {
        let doc = Document::with_text("hello world foo bar baz");
        let mut cache = WrapCache::new();
        cache.rebuild(&doc, 12, 4);

        for col in 0..23 {
            let (vis_line, vis_col) = cache.logical_to_visual(0, col);
            let (log_line, log_col) = cache.visual_to_logical(vis_line, vis_col);
            assert_eq!(log_line, 0);
            assert_eq!(log_col, col, "roundtrip failed for col {}", col);
        }
    }

    #[test]
    fn test_empty_line_wrap() {
        let doc = Document::with_text("hello\n\nworld");
        let mut cache = WrapCache::new();
        cache.rebuild(&doc, 80, 4);

        assert_eq!(cache.total_visual_lines, 3);
        assert_eq!(cache.visual_line_count(1), 1); // Empty line = 1 visual line
    }

    #[test]
    fn test_tab_handling() {
        let doc = Document::with_text("a\tb\tc\td\te"); // Tabs expand
        let mut cache = WrapCache::new();
        cache.rebuild(&doc, 10, 4);

        // Each tab-separated char: a(1) + tab(3) + b(1) + tab(3) + c(1) + ...
        // Visual widths: 0, 4, 8, 12, 16 → should wrap
        assert!(cache.total_visual_lines > 1);
    }
}
```

### Integration Tests

```rust
#[test]
fn test_soft_wrap_toggle() {
    let mut model = test_model_with_text("a very long line that will wrap around");

    // Initially no wrap
    assert!(!model.editor().soft_wrap);

    // Toggle on
    dispatch(&mut model, Msg::Editor(EditorMsg::ToggleSoftWrap));
    assert!(model.editor().soft_wrap);

    // Cache should be built
    assert!(model.editor().wrap_cache.valid);
}

#[test]
fn test_cursor_movement_with_wrap() {
    let mut model = test_model_with_long_line();
    dispatch(&mut model, Msg::Editor(EditorMsg::ToggleSoftWrap));

    // Position cursor at start
    set_cursor(&mut model, 0, 0);

    // Move down should move within wrapped line
    dispatch(&mut model, Msg::Editor(EditorMsg::MoveCursor(Direction::Down)));

    // Cursor should still be on logical line 0, but different visual line
    assert_eq!(model.editor().active_cursor().line, 0);
    // Column should be in the second visual segment
}
```

### Manual Testing Checklist

- [ ] Toggle wrap with Alt+Z
- [ ] Long lines wrap at word boundaries
- [ ] Very long words force-break
- [ ] Line numbers show for first segment only
- [ ] Continuation lines show marker (not line number)
- [ ] Up/Down navigate visual lines
- [ ] Home goes to logical line start
- [ ] End goes to logical line end
- [ ] Selection renders correctly across wraps
- [ ] Click positions cursor correctly
- [ ] Scroll works by visual lines
- [ ] Resize updates wrap positions
- [ ] Tabs expand correctly in wrapped lines

---

## Edge Cases and Invariants

### Edge Cases

1. **Empty lines**: One visual line per empty logical line
2. **Very narrow viewport**: May create many visual lines per logical line
3. **Long unbreakable words**: Force break at wrap width
4. **Cursor at wrap point**: Can be at end of one segment or start of next
5. **Selection across multiple wraps**: Render highlight for each segment
6. **Tab at wrap boundary**: Complete tab or split to next line
7. **Resize during edit**: Rebuild cache with new width
8. **Multi-byte Unicode**: Calculate visual width correctly (future)

### Invariants

1. `total_visual_lines >= document.line_count()` (each logical line = 1+ visual lines)
2. Sum of segment lengths for a line = logical line length
3. Visual lines are contiguous (no gaps)
4. Logical-to-visual-to-logical roundtrip is identity
5. Cache is invalid after any document edit until rebuilt

---

## Performance Considerations

1. **Cache invalidation**: Only invalidate, don't rebuild on every edit
2. **Lazy rebuild**: Rebuild on next render, not on edit
3. **Incremental updates**: For small edits, only recompute affected lines
4. **Viewport-only computation**: Could compute only visible lines + buffer
5. **Parallel computation**: Large files could use rayon for parallel line wrapping

---

## References

- VS Code word wrap: https://code.visualstudio.com/docs/editor/codebasics#_how-do-i-turn-on-word-wrap
- Ropey line operations: https://docs.rs/ropey/latest/ropey/
- Existing viewport: `src/model/editor.rs` (Viewport struct)
- Existing rendering: `src/view.rs`
