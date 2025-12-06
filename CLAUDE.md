# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build Commands

```bash
make build               # Build debug binary
make release             # Build optimized release binary
make run                 # Run with default sample file
make test                # Run all tests
make test-one TEST=name  # Run a specific test
make fmt                 # Format code
```

### Test Files

```bash
make run-large           # 10k lines - performance testing
make run-long            # Long lines - horizontal scroll
make run-unicode         # Unicode/emoji content
make run-zalgo           # Zalgo text corruption

# See `Makefile` for the rest of the commands...
```

## Architecture

This is a minimal text editor implementing the **Elm Architecture** in Rust:

```
Message → Update → Command → Render
```

### Core Modules

| Module       | File              | Purpose                                                 |
| ------------ | ----------------- | ------------------------------------------------------- |
| **Model**    | `src/model/`      | AppModel, Document, EditorState, UiState                |
| **Messages** | `src/messages.rs` | Msg, EditorMsg, DocumentMsg, UiMsg, AppMsg              |
| **Update**   | `src/update.rs`   | Pure state transformation: `Msg → Model mutation → Cmd` |
| **Commands** | `src/commands.rs` | Cmd enum (Redraw, SaveFile, LoadFile, Batch)            |
| **Theme**    | `src/theme.rs`    | YAML theme loading, Color types                         |
| **Renderer** | `src/main.rs`     | CPU rendering with fontdue + softbuffer                 |
| **App**      | `src/main.rs`     | winit event loop, handle_key()                          |

### Key Data Structures

- **Rope** (ropey): Efficient text buffer with O(log n) edits
- **Cursor**: `{line, column, desired_column}` - desired_column preserves position during vertical movement
- **EditOperation**: Captures insert/delete for undo/redo with cursor positions before/after
- **GlyphCache**: `HashMap<(char, font_size_bits), (Metrics, bitmap)>` avoids re-rasterization

### Character Classification for Word Navigation

IntelliJ-style word boundaries using `CharType`:

- **Whitespace**: Navigable unit (stops at both edges)
- **WordChar**: Alphanumerics
- **Punctuation**: Symbols treated as separate units

### Rendering Pipeline

1. Clear framebuffer (#1E1E1E dark background)
2. Render visible lines with line numbers
3. Draw blinking cursor (500ms interval)
4. Render status bar
5. Present via softbuffer

Font: JetBrains Mono embedded in `assets/JetBrainsMono.ttf`

## Key Bindings

### Navigation

| Action          | Mac           | Standard |
| --------------- | ------------- | -------- |
| Line start/end  | Cmd+←/→       | Home/End |
| Word left/right | Option+←/→    | -        |
| Doc start/end   | Ctrl+Home/End | -        |

### Selection

| Action           | Mac                      |
| ---------------- | ------------------------ |
| Extend selection | Shift + any movement key |
| Select word      | Double-click             |
| Select line      | Triple-click             |
| Select all       | Cmd+A                    |

### Multi-Cursor

| Action            | Mac               |
| ----------------- | ----------------- |
| Add/remove cursor | Cmd+Click         |
| Add cursor above  | Option+Option+↑   |
| Add cursor below  | Option+Option+↓   |
| Rectangle select  | Middle mouse drag |

### Editing

| Action    | Mac                 |
| --------- | ------------------- |
| Undo/Redo | Cmd+Z / Cmd+Shift+Z |
| Copy      | Cmd+C               |
| Cut       | Cmd+X               |
| Paste     | Cmd+V               |

### Debug

| Action       | Key | Notes             |
| ------------ | --- | ----------------- |
| Perf overlay | F2  | Debug builds only |

## Documentation & Design Workflow

Design docs live in `docs/feature/*.md`. Before implementing a feature:

1. Check `docs/ROADMAP.md` for planned work and priorities
2. Read the feature's design doc (e.g., `feature/STATUS_BAR.md`)
3. Implement following the design
4. Update `docs/CHANGELOG.md` when complete

### Key Docs

| Doc                 | Purpose                                                                                                 |
| ------------------- | ------------------------------------------------------------------------------------------------------- |
| `docs/ROADMAP.md`   | Planned features, implementation gaps, module structure                                                 |
| `docs/CHANGELOG.md` | Completed work by date                                                                                  |
| `docs/FEEDBACK.md`  | Implementation review and priorities                                                                    |
| `docs/feature/*.md` | Detailed designs (THEMING, SELECTION_MULTICURSOR, STATUS_BAR, SPLIT_VIEW, TEXT-SHRINK-EXPAND-SELECTION) |
