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
- **Messages** (`src/messages.rs`): Msg, EditorMsg, DocumentMsg, UiMsg, LayoutMsg, TextEditMsg
- **Update** (`src/update/`): Pure state transformation (6 submodules including text_edit.rs)
- **Commands** (`src/commands.rs`): Cmd enum (Redraw, SaveFile, LoadFile, Batch)
- **View** (`src/view/`): CPU rendering with fontdue + softbuffer, winit event loop
- **Editable** (`src/editable/`): Unified text editing system (EditableState, StringBuffer, Cursor, Selection)

Key structures: Rope (ropey) for text buffer, Cursor, EditOperation for undo/redo, GlyphCache, EditableState for modal/CSV inputs.

## Code Style

- Rust 2021 edition, run `make fmt` and `make lint` before committing
- Design docs in `docs/feature/*.md`; check `docs/ROADMAP.md` for planned work
- Update `docs/CHANGELOG.md` when features are complete

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
