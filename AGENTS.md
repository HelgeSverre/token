# AGENTS.md

## Build & Test Commands

```bash
make build                # Build debug binary
make release              # Build optimized release binary
make test                 # Run all tests
make test-one TEST=name   # Run a single test by name
make fmt                  # Format code
make run                  # Run with default sample file
```

## Architecture

Elm Architecture in Rust: `Message → Update → Command → Render`

- **Model** (`src/model/`): AppModel, Document, EditorState, UiState
- **Messages** (`src/messages.rs`): Msg, EditorMsg, DocumentMsg, UiMsg, AppMsg
- **Update** (`src/update.rs`): Pure state transformation
- **Commands** (`src/commands.rs`): Cmd enum (Redraw, SaveFile, LoadFile, Batch)
- **Renderer** (`src/main.rs`): CPU rendering with fontdue + softbuffer, winit event loop

Key structures: Rope (ropey) for text buffer, Cursor, EditOperation for undo/redo, GlyphCache.

## Code Style

- Rust 2021 edition, use `make fmt` before committing
- Design docs in `docs/feature/*.md`; check `docs/ROADMAP.md` for planned work
- Update `docs/CHANGELOG.md` when features are complete
