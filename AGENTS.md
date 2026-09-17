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

For JavaScript automation scripts, reuse `scripts/lib/token-automation.mjs`.
See [the automation guide](docs/dev/automation-input.md) for the shared client,
isolated native fixtures, and input limitations. Keep generated artifacts under
`target/verification/`.

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
- Record user-visible application changes in `docs/CHANGELOG.md` under
  `Unreleased`, creating that section when necessary. Do not add documentation,
  handoff, plan-archival or agent-workflow bookkeeping to the changelog.
- Use `ByteSize` constructors for binary limits, capacities, thresholds, and
  displayed sizes. Keep raw bytes at external boundaries, and do not use
  `ByteSize` for pixels, characters, rows, or other unrelated quantities.

## Rendering and Performance

- Keep Settings as its separate preferences page with category navigation and
  form controls. Sharing metadata or layout helpers does not authorize replacing
  this design with a command-palette UI.
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

<!-- jbcontext-instructions-start -->

# Tools

## Semantic Code Search (jbcontext)

You have access to `jbcontext search` for searching the codebase semantically.
Use the `/context-search` skill or run `jbcontext search "<query>"` to find code by meaning, not just keywords.

### Query Tips

- Be descriptive: "Where is a function that validates user email addresses" > "email"
- Include context: "Find error handling middleware for HTTP requests with logging"
- Specify what you're looking for: "React component that renders a modal dialog"

### When to use

`jbcontext search` is a **code-discovery** tool. Reach for it only when a task requires finding or understanding code whose location you don't already know.

Skip it — go straight to the right tool — when:

- the task names the exact file, class, or symbol (keyword grep is faster);
- the relevant file is already open or identified;
- the task doesn't involve locating code at all — git operations (rebase, merge, commit), running tests or builds, shell/statusline/config setup, or reviewing a diff you already have.

### How to use it

- Start with `jbcontext search` before planning, editing, or exact search in unfamiliar code when you do not yet know the right file, subsystem, implementation, or related test.
- Use one focused natural-language query per search.
- Do not start with grep, ripgrep, or find when the search problem is still semantic or exploratory.
- Inspect the first relevant file or directory before issuing another broad semantic search.
- Use another broad `jbcontext search` only if the local path stops being productive.
- Once you know the relevant file, symbol, or directory, switch to direct file reads or exact search for local inspection.
- If you search again after finding a relevant area, narrow with `-p <path>`.

<!-- jbcontext-instructions-end -->

<!-- BEGIN BEADS INTEGRATION v:1 profile:full hash:bacef91e -->
## Issue Tracking with bd (beads)

**IMPORTANT**: This project uses **bd (beads)** for ALL issue tracking. Do NOT use markdown TODOs, task lists, or other tracking methods.

### Why bd?

- Dependency-aware: Track blockers and relationships between issues
- Git-friendly: Dolt-powered version control with native sync
- Agent-optimized: JSON output, ready work detection, discovered-from links
- Prevents duplicate tracking systems and confusion

### Quick Start

**Check for ready work:**

```bash
bd ready --json
```

**Create new issues:**

```bash
bd create "Issue title" --description="Detailed context" -t bug|feature|task -p 0-4 --json
bd create "Issue title" --description="What this issue is about" -p 1 --deps discovered-from:bd-123 --json
```

**Claim and update:**

```bash
bd update <id> --claim --json
bd update bd-42 --priority 1 --json
```

**Complete work:**

```bash
bd close bd-42 --reason "Completed" --json
```

### Issue Types

- `bug` - Something broken
- `feature` - New functionality
- `task` - Work item (tests, docs, refactoring)
- `epic` - Large feature with subtasks
- `chore` - Maintenance (dependencies, tooling)

### Priorities

- `0` - Critical (security, data loss, broken builds)
- `1` - High (major features, important bugs)
- `2` - Medium (default, nice-to-have)
- `3` - Low (polish, optimization)
- `4` - Backlog (future ideas)

### Workflow for AI Agents

1. **Check ready work**: `bd ready` shows unblocked issues
2. **Claim your task atomically**: `bd update <id> --claim`
3. **Work on it**: Implement, test, document
4. **Discover new work?** Create linked issue:
   - `bd create "Found bug" --description="Details about what was found" -p 1 --deps discovered-from:<parent-id>`
5. **Complete**: `bd close <id> --reason "Done"`

### Quality
- Use `--acceptance` and `--design` fields when creating issues
- Use `--validate` to check description completeness

### Lifecycle
- `bd defer <id>` / `bd supersede <id>` for issue management
- `bd stale` / `bd orphans` / `bd lint` for hygiene
- `bd human <id>` to flag for human decisions
- `bd formula list` / `bd mol pour <name>` for structured workflows

### Sync

bd stores issue history in Dolt:

- Each write auto-commits to Dolt history
- Use `bd dolt push`/`bd dolt pull` for remote sync
- Do not treat `.beads/issues.jsonl` as the sync protocol

**Architecture in one line:** issues live in a local Dolt DB; sync uses `refs/dolt/data` on your git remote; `.beads/issues.jsonl` is a passive export. See https://github.com/gastownhall/beads/blob/main/docs/core-concepts/sync-concepts.md for details and anti-patterns.

### Important Rules

- ✅ Use bd for ALL task tracking
- ✅ Always use `--json` flag for programmatic use
- ✅ Link discovered work with `discovered-from` dependencies
- ✅ Check `bd ready` before asking "what should I work on?"
- ❌ Do NOT create markdown TODO lists
- ❌ Do NOT use external issue trackers
- ❌ Do NOT duplicate tracking systems

For more details, see README.md and https://github.com/gastownhall/beads/blob/main/docs/getting-started/quickstart.md.

## Agent Context Profiles

The managed Beads block is task-tracking guidance, not permission to override repository, user, or orchestrator instructions.

- **Conservative (default)**: Use `bd` for task tracking. Do not run git commits, git pushes, or Dolt remote sync unless explicitly asked. At handoff, report changed files, validation, and suggested next commands.
- **Minimal**: Keep tool instruction files as pointers to `bd prime`; use the same conservative git policy unless active instructions say otherwise.
- **Team-maintainer**: Only when the repository explicitly opts in, agents may close beads, run quality gates, commit, and push as part of session close. A current "do not commit" or "do not push" instruction still wins.

## Session Completion

This protocol applies when ending a Beads implementation workflow. It is subordinate to explicit user, repository, and orchestrator instructions.

1. **File issues for remaining work** - Create beads for anything that needs follow-up
2. **Run quality gates** (if code changed) - Tests, linters, builds
3. **Update issue status** - Close finished work, update in-progress items
4. **Handle git/sync by active profile**:
   ```bash
   # Conservative/minimal/default: report status and proposed commands; wait for approval.
   git status

   # Team-maintainer opt-in only, unless current instructions forbid it:
   git pull --rebase
   bd dolt push
   git push
   git status
   ```
5. **Hand off** - Summarize changes, validation, issue status, and any blocked sync/commit/push step

**Critical rules:**
- Explicit user or orchestrator instructions override this Beads block.
- Do not commit or push without clear authority from the active profile or the current user request.
- If a required sync or push is blocked, stop and report the exact command and error.

<!-- END BEADS INTEGRATION -->

<!-- BEGIN BEADS CODEX SETUP: generated by bd setup codex -->
## Beads Issue Tracker

Use Beads (`bd`) for durable task tracking in repositories that include it. Use the `beads` skill at `.agents/skills/beads/SKILL.md` (project install) or `~/.agents/skills/beads/SKILL.md` (global install) for Beads workflow guidance, then use the `bd` CLI for issue operations.

### Quick Reference

```bash
bd ready                # Find available work
bd show <id>            # View issue details
bd update <id> --claim  # Claim work
bd close <id>           # Complete work
bd prime                # Refresh Beads context
```

### Rules

- Use `bd` for all task tracking; do not create markdown TODO lists.
- Run `bd prime` when Beads context is missing or stale. Codex 0.129.0+ can load Beads context automatically through native hooks; use `/hooks` to inspect or toggle them.
- Keep persistent project memory in Beads via `bd remember`; do not create ad hoc memory files.

**Architecture in one line:** issues live in a local Dolt DB; sync uses `refs/dolt/data` on your git remote; `.beads/issues.jsonl` is a passive export. See https://github.com/gastownhall/beads/blob/main/docs/core-concepts/sync-concepts.md for details and anti-patterns.
<!-- END BEADS CODEX SETUP -->
