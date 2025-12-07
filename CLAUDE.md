# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build Commands

```bash
make build               # Build debug binary
make release             # Build optimized release binary
make run                 # Run release build with sample file
make dev                 # Run debug build (faster compile)
make test                # Run all tests
make test-one TEST=name  # Run specific test
make test-verbose        # Run tests with output
make fmt                 # Format Rust code and markdown
make lint                # Run clippy lints (mirrors CI)
```

### Development Workflow

```bash
make watch               # Start bacon watch mode
make watch-lint          # Watch with clippy
make test-fast           # Fast parallel tests (nextest)
make test-retry          # Tests with retries for flaky tests
```

### Profiling & Benchmarks

```bash
make bench               # Run all benchmarks
make bench-rope          # Rope operation benchmarks
make bench-render        # Rendering benchmarks
make bench-glyph         # Glyph cache benchmarks
make flamegraph          # Generate CPU flamegraph
make profile-samply      # Interactive profiling (Firefox Profiler)
make profile-memory      # Heap profiling with dhat
make coverage            # Generate HTML coverage report
```

### CI & Cross-Compilation

```bash
make ci                  # Test GitHub Actions locally with act
make compile-all         # Build for all platforms
make setup               # Install all dev tools
```

Run `make help` for complete command list.

## Architecture

This is a minimal text editor implementing the **Elm Architecture** in Rust:

```
Message → Update → Command → Render
```

### Core Modules

| Module       | File(s)           | Purpose                                       |
| ------------ | ----------------- | --------------------------------------------- |
| **Model**    | `src/model/`      | AppModel, Document, EditorState, EditorArea   |
| **Messages** | `src/messages.rs` | Msg, EditorMsg, DocumentMsg, UiMsg, LayoutMsg |
| **Update**   | `src/update/`     | Pure state transformation (5 submodules)      |
| **Commands** | `src/commands.rs` | Cmd enum (Redraw, SaveFile, LoadFile, Batch)  |
| **Theme**    | `src/theme.rs`    | YAML theme loading, Color types               |
| **Renderer** | `src/view.rs`     | CPU rendering with fontdue + softbuffer       |
| **Input**    | `src/input.rs`    | handle_key, keyboard→Msg mapping              |
| **App**      | `src/app.rs`      | App struct, winit ApplicationHandler          |
| **Perf**     | `src/perf.rs`     | PerfStats, debug overlay (debug builds only)  |

### Module Structure

```
src/
├── main.rs              # Entry point (~20 lines)
├── lib.rs               # Library root with module exports
├── app.rs               # App struct, ApplicationHandler impl
├── input.rs             # handle_key, keyboard→Msg mapping
├── view.rs              # Renderer, drawing functions
├── perf.rs              # PerfStats, debug overlay (debug only)
├── model/
│   ├── mod.rs           # AppModel struct, layout constants
│   ├── document.rs      # Document (buffer, undo/redo, file_path)
│   ├── editor.rs        # EditorState, Cursor, Selection, Viewport
│   ├── editor_area.rs   # EditorArea, groups, tabs, layout tree
│   ├── ui.rs            # UiState (cursor blink, transient messages)
│   └── status_bar.rs    # StatusBar, StatusSegment, sync_status_bar()
├── update/
│   ├── mod.rs           # Pure dispatcher
│   ├── editor.rs        # Cursor, selection, expand/shrink
│   ├── document.rs      # Text editing, undo/redo
│   ├── layout.rs        # Split views, tabs, groups
│   ├── app.rs           # File operations, window resize
│   └── ui.rs            # Status bar, cursor blink
├── messages.rs          # All message types
├── commands.rs          # Cmd enum
├── theme.rs             # Theme, Color, TabBarTheme
├── overlay.rs           # OverlayConfig, OverlayBounds
└── util.rs              # CharType enum, char classification

tests/                   # Integration tests (400+ tests)
themes/                  # YAML theme files (dark.yaml, fleet-dark.yaml, etc.)
```

### Key Data Structures

- **Rope** (ropey): Efficient text buffer with O(log n) edits
- **Cursor**: `{line, column, desired_column}` - desired_column preserves horizontal position during vertical movement
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
| Expand selection | Option+↑                 |
| Shrink selection | Option+↓                 |

### Multi-Cursor

| Action            | Mac               |
| ----------------- | ----------------- |
| Add/remove cursor | Cmd+Click         |
| Add cursor above  | Option+Option+↑   |
| Add cursor below  | Option+Option+↓   |
| Rectangle select  | Middle mouse drag |

### Editing

| Action      | Mac                 |
| ----------- | ------------------- |
| Undo/Redo   | Cmd+Z / Cmd+Shift+Z |
| Copy        | Cmd+C               |
| Cut         | Cmd+X               |
| Paste       | Cmd+V               |
| Duplicate   | Cmd+D               |
| Delete line | Cmd+Backspace       |

### Debug

| Action       | Key | Notes             |
| ------------ | --- | ----------------- |
| Perf overlay | F2  | Debug builds only |

## Documentation & Design Workflow

Design docs live in `docs/feature/*.md`. Before implementing a feature:

1. Check `docs/ROADMAP.md` for planned work and priorities
2. Read the feature's design doc (e.g., `feature/KEYMAPPING.md`)
3. Implement following the design
4. Update `docs/CHANGELOG.md` when complete

### Key Docs

| Doc                           | Purpose                                    |
| ----------------------------- | ------------------------------------------ |
| `docs/ROADMAP.md`             | Planned features, module structure         |
| `docs/CHANGELOG.md`           | Completed work by date                     |
| `docs/EDITOR_UI_REFERENCE.md` | Comprehensive UI component reference       |
| `docs/GUI-REVIEW-FINDINGS.md` | GUI architecture improvements plan         |
| `docs/feature/*.md`           | Design specs (KEYMAPPING, SPLIT_VIEW, etc) |
