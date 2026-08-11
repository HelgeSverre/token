# AGENTS.md

This is the canonical instruction file for every coding agent in this repository.
Do not add tool-specific copies such as `CLAUDE.md`; improve this file instead.

## Commands

```bash
just build             # Debug build
just release           # Optimized build
just test              # Full nextest suite plus doctests
just test-one name     # One targeted test/filter
just fmt               # Format Rust and root Markdown files
just fmt-check          # Check formatting without changing files
just lint               # Clippy with the same strictness as CI
just run                # Release build with representative sample files
```

Use `just --list` for profiling, benchmarks, packaging, and other less common
workflows. Prefer repository recipes over invented Cargo command combinations.

## Architecture

The application follows an Elm-style flow:
`Message -> Update -> Command -> Render`.

- `src/model/` owns application and document state.
- `src/messages.rs` defines state-change requests.
- `src/update/` transforms state and returns commands.
- `src/commands.rs` describes effects for the runtime to perform.
- `src/runtime/` owns winit integration, input dispatch, and side effects.
- `src/view/` owns CPU rendering and hit testing.
- `src/editable/` is the shared editing system; `src/syntax/` owns language
  detection, parsing, highlighting, outline, injections, and syntax selection.

Keep update handlers deterministic. Put I/O and platform work behind commands or
in the runtime. Treat current code and tests as the source of truth; plans and
feature docs can describe intent but may lag implementation.

## Working Rules

- Use Rust 2021 idioms. Run `just fmt`, targeted tests while iterating, then
  `just test` and `just lint` before handing off a substantial change.
- Preserve unrelated work in a dirty tree. Stage explicit files rather than
  relying on `git add -A`.
- Record user-visible changes in `docs/CHANGELOG.md` under `Unreleased`, creating
  that section when necessary.
- Use `ByteSize` constructors for binary limits, capacities, thresholds, and
  displayed sizes. Keep raw bytes at external boundaries, and do not use
  `ByteSize` for pixels, characters, rows, or other unrelated quantities.

## Rendering and Performance

- Keep `Renderer` as the top-level orchestrator. Extract domain-specific code
  only when it creates a shared source of truth or a clear feature home.
- Reuse layout, viewport, and traversal helpers across rendering, hit testing,
  and interaction. Independently derived geometry or ordering is a common bug.
- Put text-editor visuals in `src/view/editor_text.rs` and shared viewport code.
  Avoid new feature-local line loops and assumptions that a logical line is one
  rendered row.
- Gate text-only fast paths with `EditorState::is_plain_text_mode()` so image,
  CSV, binary, and other special tabs cannot enter text rendering paths.
- Do not make performance claims from `just workspace`: it is a debug build. The
  F2 overlay also forces full redraw while visible, so use it for stage diagnosis,
  not release-equivalent frame rates.
- Extend the shared stages in `src/perf.rs` when instrumenting rendering; do not
  add one-off timers or a second overlay-specific stage list.

## Releases

Preparing a release does not authorize publishing it.

1. Update the version in `Cargo.toml` and `Cargo.lock`.
2. Turn the changelog's `Unreleased` section into `vX.Y.Z - YYYY-MM-DD`.
3. Run `just test && just lint` and commit only the release files with
   `chore: release vX.Y.Z`.
4. Only when explicitly asked to publish, create and push the exact annotated
   tag: `git tag -a vX.Y.Z -m "vX.Y.Z - Summary"` then
   `git push origin vX.Y.Z`.

The tag triggers cargo-dist, which builds artifacts, creates the GitHub release,
and publishes Homebrew. Do not also run `gh release create`.
