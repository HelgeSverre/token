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

### Performance Workflow Notes

- `make workspace` launches `target/debug/token ./` using the dev profile. That is intentionally compile-fast and can be dramatically slower than release for CPU rendering work.
- Use `make release`, `make run`, or the profiling profile when making performance claims. Debug numbers are mainly useful for relative local diagnosis.
- The perf overlay (`F2`, debug builds only) currently forces full redraw while visible. It is useful for stage breakdowns, but it perturbs the numbers it shows.
- Perf instrumentation is stage-based. The source of truth is `src/perf.rs` (`PerfStage`, `PerfStats::measure_stage`, `render_perf_overlay`). `src/runtime/perf.rs` is only a re-export shim for runtime code.
- When adding timings, time the phase that actually owns the work and extend the shared stage registry instead of keeping a separate hard-coded overlay list.
- File tracing is opt-in for hot paths: file logging follows `TOKEN_FILE_LOG` when set, otherwise falls back to `RUST_LOG`.

### Rendering Guardrails

- Keep `Renderer` monolithic as the top-level orchestrator. Do not introduce file/module splits just to reduce line count.
- Add abstractions only when they provide a shared source of truth or a clear feature home. Good examples in the current codebase are:
  - scene objects for content/pane dispatch (`EditorGroupScene`, dock scene, preview scene)
  - shared layout structs (`WindowLayout`, `GroupLayout`, `TabBarLayout`, `OutlinePanelLayout`, `PreviewPaneLayout`, `DockHeaderLayout`)
  - `TextEditorRenderer` for text-editor-specific rendering logic
- Avoid generic GUI-framework abstractions. This is an editor, not a reusable widget toolkit.
- New text-view features should usually land in `src/view/editor_text.rs` and use shared viewport/layout helpers instead of adding feature-local document-line loops elsewhere.
- Do not deepen the assumption that `logical line == rendered row`. Soft wrap and folding will need a visual-line seam, so prefer names and helper boundaries that can evolve toward visual rows / wrapped segments.
- Keep render/hit-test/update paths on the same ordering and geometry helpers. If a feature adds a second traversal or re-derives tab/tree/viewport geometry separately, that should be treated as a design smell.
- Text-only fast paths must remain gated by `EditorState::is_plain_text_mode()` so non-text tabs do not pick up cursor-line or gutter assumptions by accident.

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
| **Update**   | `src/update/`     | Pure state transformation (10 submodules)     |
| **Commands** | `src/commands.rs` | Cmd enum (Redraw, SaveFile, LoadFile, Batch)  |
| **Theme**    | `src/theme.rs`    | YAML theme loading, Color types               |
| **View**     | `src/view/`       | CPU rendering with fontdue + softbuffer       |
| **Runtime**  | `src/runtime/`    | App, input handling, runtime glue (winit)     |
| **Perf**     | `src/perf.rs`     | Stage-based perf stats and debug overlay       |
| **Keymap**   | `src/keymap/`     | Configurable keybindings, command dispatch    |
| **Syntax**   | `src/syntax/`     | Tree-sitter syntax highlighting (20 langs)    |
| **CSV**      | `src/csv/`        | CSV viewer/editor with spreadsheet UI         |
| **Editable** | `src/editable/`   | Unified text editing (cursors, selection)     |

### Module Structure

```
src/
├── main.rs              # Entry point (~20 lines)
├── lib.rs               # Library root with module exports
├── messages.rs          # All message types
├── commands.rs          # Cmd enum
├── perf.rs              # PerfStage, PerfStats, perf overlay implementation
├── theme.rs             # Theme, Color, TabBarTheme
├── overlay.rs           # OverlayConfig, OverlayBounds
├── model/
│   ├── mod.rs           # AppModel struct, layout constants
│   ├── document.rs      # Document (buffer, undo/redo, file_path)
│   ├── editor.rs        # EditorState, Cursor, Selection, Viewport
│   ├── editor_area.rs   # EditorArea, groups, tabs, layout tree
│   ├── workspace.rs     # Workspace, file tree, sidebar
│   ├── ui.rs            # UiState (cursor blink, transient messages)
│   └── status_bar.rs    # StatusBar, StatusSegment, sync_status_bar()
├── update/
│   ├── mod.rs           # Pure dispatcher
│   ├── editor.rs        # Cursor, selection, expand/shrink
│   ├── document.rs      # Text editing, undo/redo
│   ├── layout.rs        # Split views, tabs, groups
│   ├── app.rs           # File operations, window resize
│   ├── ui.rs            # Status bar, cursor blink
│   ├── csv.rs           # CSV view updates
│   ├── syntax.rs        # Syntax highlighting updates
│   ├── text_edit.rs     # Unified text editing dispatch
│   └── workspace.rs     # File tree, workspace updates
├── runtime/
│   ├── mod.rs           # Runtime module exports
│   ├── app.rs           # App struct, winit ApplicationHandler
│   ├── input.rs         # handle_key, keyboard→Msg mapping
│   ├── mouse.rs         # Mouse dispatch and hit-tested actions
│   ├── perf.rs          # Re-export shim for token::perf
│   └── webview.rs       # Markdown preview webview management
├── view/
│   ├── mod.rs           # Renderer, GlyphCache
│   ├── frame.rs         # Frame, TextPainter abstractions
│   ├── geometry.rs      # Geometry helpers
│   ├── helpers.rs       # Rendering helpers
│   └── text_field.rs    # Text field rendering
├── keymap/              # Configurable keybindings
├── syntax/              # Tree-sitter syntax highlighting
├── csv/                 # CSV viewer/editor
├── editable/            # Unified text editing system
└── util/                # Utilities (file validation, text helpers)

tests/                   # Integration tests (600+ tests)
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

1. Build a render plan from damage + window/layout state
2. Clear the persistent back buffer as needed
3. Render editor groups / panes / overlays into the back buffer
4. Copy the back buffer into the softbuffer surface and present

Perf timings for that pipeline are tracked by stage in `src/perf.rs`, including tracked vs untracked frame time.

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

| Doc                                    | Purpose                                    |
| -------------------------------------- | ------------------------------------------ |
| `docs/ROADMAP.md`                      | Planned features, module structure         |
| `docs/CHANGELOG.md`                    | Completed work by date                     |
| `docs/EDITOR_UI_REFERENCE.md`          | Comprehensive UI component reference       |
| `docs/archived/GUI-REVIEW-FINDINGS.md` | GUI architecture improvements plan         |
| `docs/feature/*.md`                    | Design specs (KEYMAPPING, SPLIT_VIEW, etc) |

## Releasing a New Version

When releasing a new version, follow these steps:

1. **Update version** in `Cargo.toml`
2. **Update `docs/CHANGELOG.md`** with release notes under new version header
3. **Run tests and lint**: `make test && make lint`
4. **Commit changes**:
   ```bash
   git add -A && git commit -m "chore: release vX.Y.Z"
   ```
5. **Create annotated tag**:
   ```bash
   git tag -a vX.Y.Z -m "vX.Y.Z - Brief description"
   ```
6. **Push commits and tags**:
   ```bash
   git push && git push --tags
   ```
7. **Create GitHub release**:
   ```bash
   gh release create vX.Y.Z --title "vX.Y.Z - Title" --notes "Release notes here"
   ```

### Version Numbering

- **Major (X)**: Breaking changes or major rewrites
- **Minor (Y)**: New features, significant improvements
- **Patch (Z)**: Bug fixes, minor improvements
