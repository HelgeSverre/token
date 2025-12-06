# Roadmap

Planned features and improvements for rust-editor.

For completed work, see [CHANGELOG.md](CHANGELOG.md).
For archived phases, see [archived/old-roadmap-file.md](archived/old-roadmap-file.md).

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

### Structured Status Bar

**Design:** [feature/STATUS_BAR.md](feature/STATUS_BAR.md)

Replace pipe-delimited string with segment-based system:
- Left/center/right alignment
- Per-segment theming
- Transient messages with expiry
- Click actions (future)

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
| Theming System | Complete | [feature/THEMING.md](feature/THEMING.md) |
| Selection & Multi-Cursor | 8/9 phases complete | [feature/SELECTION_MULTICURSOR.md](feature/SELECTION_MULTICURSOR.md) |
| Status Bar | Planned | [feature/STATUS_BAR.md](feature/STATUS_BAR.md) |
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
│   └── ui.rs            # UiState (status, cursor blink, loading states)
├── messages.rs          # Msg, EditorMsg, DocumentMsg, UiMsg, AppMsg, Direction
├── commands.rs          # Cmd enum (Redraw, SaveFile, LoadFile, Batch)
├── update.rs            # update() dispatcher + update_editor/document/ui/app
├── theme.rs             # Theme, Color, YAML theme loading
└── util.rs              # CharType enum, is_punctuation, char_type

tests/
├── common/mod.rs        # Shared test helpers
├── cursor_movement.rs   # 38 tests
├── text_editing.rs      # 21 tests
├── selection.rs         # 16 tests
├── scrolling.rs         # 33 tests
├── edge_cases.rs        # 9 tests
└── status_bar.rs        # 47 tests
```

**Test count:** 185 (10 theme + 11 keyboard + 164 integration)
