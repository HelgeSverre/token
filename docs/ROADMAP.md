# Roadmap

Planned features and improvements for rust-editor.

For completed work, see [CHANGELOG.md](CHANGELOG.md).
For archived phases, see [archived/old-roadmap-file.md](archived/old-roadmap-file.md).

---

## Recently Completed

### Structured Status Bar ✅

**Design:** [feature/STATUS_BAR.md](feature/STATUS_BAR.md) | **Completed:** 2025-12-06

Segment-based status bar system:
- Left/right alignment with separators
- `sync_status_bar()` auto-updates from model
- Transient messages with auto-expiry
- 47 tests in `tests/status_bar.rs`

### Overlay System ✅

**Completed:** 2025-12-06

Reusable overlay rendering (`src/overlay.rs`):
- Anchor positioning (TopLeft, TopRight, etc.)
- Alpha-blended backgrounds
- Optional themed borders
- Theme integration (background, foreground, highlight, warning, error, border)

---

## In Progress

### Phase 9: Occurrence Selection (JetBrains-style)

**Status:** Not started

| Task | Shortcut | Description |
|------|----------|-------------|
| `SelectNextOccurrence` | Cmd+J | Add selection at next match of current word/selection |
| `UnselectOccurrence` | Shift+Cmd+J | Remove last added occurrence |
| Occurrence history | - | Track additions for unselect |
| Word detection | - | Reuse `char_type()` for word boundaries |

---

## Planned Features

### Expand/Shrink Selection

**Design:** [feature/TEXT-SHRINK-EXPAND-SELECTION.md](feature/TEXT-SHRINK-EXPAND-SELECTION.md)

Progressive selection expansion:
- Option+Up: Expand (word → line → all)
- Option+Down: Shrink (restore previous)
- Selection history stack

### Split View

**Design:** [feature/SPLIT_VIEW.md](feature/SPLIT_VIEW.md)

Multi-pane editing:
- Horizontal/vertical splits
- Tabs within panes
- Shared documents with independent views
- Layout tree for flexible arrangement

---

## Feature Design Documents

| Feature | Status | Design Doc |
|---------|--------|------------|
| Theming System | ✅ Complete | [feature/THEMING.md](feature/THEMING.md) |
| Selection & Multi-Cursor | ✅ 8/9 phases complete | [feature/SELECTION_MULTICURSOR.md](feature/SELECTION_MULTICURSOR.md) |
| Status Bar | ✅ Complete | [feature/STATUS_BAR.md](feature/STATUS_BAR.md) |
| Overlay System | ✅ Complete | (inline in CHANGELOG) |
| Split View | Planned | [feature/SPLIT_VIEW.md](feature/SPLIT_VIEW.md) |
| Expand/Shrink Selection | Planned | [feature/TEXT-SHRINK-EXPAND-SELECTION.md](feature/TEXT-SHRINK-EXPAND-SELECTION.md) |

---

## Implementation Gaps

Core functionality that needs attention:

| Gap | Priority | Notes |
|-----|----------|-------|
| `Selection::get_text()` | Medium | Needed for clipboard, occurrence search |
| `EditOperation::Batch` | Medium | Proper multi-cursor undo/redo |
| `merge_overlapping_selections()` | Low | Edge case handling |

---

## Current Module Structure

```
src/
├── main.rs              # Entry point, event loop, App struct, Renderer
├── lib.rs               # Library root with module exports
├── model/
│   ├── mod.rs           # AppModel struct (includes Theme), re-exports
│   ├── document.rs      # Document struct (buffer, undo/redo, file_path)
│   ├── editor.rs        # EditorState, Cursor, Selection, Viewport, ScrollRevealMode
│   ├── ui.rs            # UiState (status, cursor blink, loading states)
│   └── status_bar.rs    # StatusBar, StatusSegment, sync_status_bar(), layout
├── messages.rs          # Msg, EditorMsg, DocumentMsg, UiMsg, AppMsg, Direction
├── commands.rs          # Cmd enum (Redraw, SaveFile, LoadFile, Batch)
├── update.rs            # update() dispatcher + update_editor/document/ui/app
├── theme.rs             # Theme, Color, OverlayTheme, YAML theme loading
├── overlay.rs           # OverlayConfig, OverlayBounds, render functions
└── util.rs              # CharType enum, is_punctuation, char_type

themes/
├── dark.yaml            # Default dark theme (VS Code-inspired)
├── fleet-dark.yaml      # JetBrains Fleet dark theme
├── github-dark.yaml     # GitHub dark theme
└── github-light.yaml    # GitHub light theme

tests/
├── common/mod.rs        # Shared test helpers
├── cursor_movement.rs   # 38 tests
├── text_editing.rs      # 38 tests (includes delete line, duplicate)
├── selection.rs         # 16 tests
├── scrolling.rs         # 33 tests
├── edge_cases.rs        # 9 tests
├── monkey_tests.rs      # 34 tests (expanded resize edge cases)
└── status_bar.rs        # 47 tests
```

**Test count:** 246 (17 lib + 14 main + 215 integration)
