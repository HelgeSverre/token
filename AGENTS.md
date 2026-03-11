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

## Performance Notes

- `make workspace` runs the debug binary (`target/debug/token ./`). That is useful for day-to-day iteration, but not for meaningful renderer performance claims.
- The in-app perf overlay (`F2`, debug builds only) currently forces full redraw while visible. Treat it as a diagnostic breakdown, not a release-equivalent FPS meter.
- Perf instrumentation is stage-based in `src/perf.rs`. If you add or change render timings, extend `PerfStage` / `PerfStats::measure_stage()` there instead of adding one-off counters or a second overlay-specific list.
- File tracing is not always-on. File logging follows `TOKEN_FILE_LOG` when set, otherwise `RUST_LOG`.

## Rendering Guardrails

- Keep `Renderer` monolithic at the top level. Do not split rendering code only because a file is large; add abstractions only when they become a real shared source of truth or a real feature home.
- Prefer domain-specific seams over generic widget layers. Current examples are scene objects (`EditorGroupScene`, dock/preview scenes), shared geometry structs, and `TextEditorRenderer`.
- New text-editor visuals should plug into `src/view/editor_text.rs` and shared viewport helpers, not add fresh line iteration or viewport math in `Renderer`, hit-test code, or update code.
- Do not deepen the assumption that `logical line == rendered row`. When adding text-view logic, prefer APIs and naming that can later work with visual rows / wrapped segments.
- Reuse shared layout helpers (`WindowLayout`, `GroupLayout`, `TabBarLayout`, dock/outline/preview layouts) instead of re-deriving geometry in each render or hit-test path.
- Render order and interaction order should share the same traversal/model helpers. If a tree, tab bar, or viewport has one visible ordering in render code and a different one in hit-testing or selection code, that is usually a bug.
- Text-only fast paths must stay text-only. Gate them through `EditorState::is_plain_text_mode()` so image/CSV/binary tabs do not accidentally opt into text rendering assumptions.

## Releasing a New Version

Releases are automated via **cargo-dist**. Pushing a tag triggers CI to build binaries, create the GitHub release, and publish to Homebrew.

1. **Update version** in `Cargo.toml`
2. **Update `docs/CHANGELOG.md`** with release notes under new version header
3. **Run tests and lint**: `make test && make lint`
4. **Commit changes**:
   ```bash
   git add -A && git commit -m "chore: release vX.Y.Z"
   ```
5. **Create annotated tag and push**:
   ```bash
   git tag -a vX.Y.Z -m "vX.Y.Z - Brief description"
   git push && git push --tags
   ```

cargo-dist CI will handle building platform binaries, creating the GitHub release, and publishing the Homebrew formula.

### Version Numbering

- **Major (X)**: Breaking changes or major rewrites
- **Minor (Y)**: New features, significant improvements
- **Patch (Z)**: Bug fixes, minor improvements
