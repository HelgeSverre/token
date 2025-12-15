# AGENTS.md

## Build & Test Commands

```bash
make build                       # Build debug binary
make release                     # Build optimized release binary
make test                        # Run all tests
cargo test test_name             # Run single test by name
make fmt                         # Format code (cargo fmt + prettier)
make lint                        # Run clippy lints (mirrors CI)
make run                         # Run release build with sample file
```

## Architecture

Elm Architecture in Rust: `Message → Update → Command → Render`

- **Model** (`src/model/`): AppModel, Document, EditorState, EditorArea, UiState
- **Messages** (`src/messages.rs`): Msg, EditorMsg, DocumentMsg, UiMsg, LayoutMsg
- **Update** (`src/update/`): Pure state transformation (5 submodules)
- **Commands** (`src/commands.rs`): Cmd enum (Redraw, SaveFile, LoadFile, Batch)
- **View** (`src/view/`): CPU rendering with fontdue + softbuffer, winit event loop

Key structures: Rope (ropey) for text buffer, Cursor, EditOperation for undo/redo, GlyphCache.

## Code Style

- Rust 2021 edition, run `make fmt` and `make lint` before committing
- Design docs in `docs/feature/*.md`; check `docs/ROADMAP.md` for planned work
- Update `docs/CHANGELOG.md` when features are complete
