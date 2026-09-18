# Agent session archive

> **Status:** the website viewer described here is implemented on the unmerged
> branch `feature/website-agent-transcripts`, not on `main`. On `main` only the
> importer (`scripts/agent-archive/`) and the source exports
> (`docs/ampcode-threads/`) exist; every `website/` path, the `dev:archive` npm
> script, the website tests, and the icon set named below are branch-only until
> that branch is merged (`npm run build --prefix website` works on both). Replay and
> attachment retrieval are not implemented on either.

`/agent` is a chronological browser; `/agent/<id>` renders a recorded session.
The normal static build uses the 171 already-public Amp Markdown exports imported
from `docs/ampcode-threads/`. Regenerate them manually with:

```sh
python3 scripts/agent-archive/import_amp_markdown.py
```

Generated files live in `website/src/data/agent-sessions/T-*.json` (not checked
in on `main`; produced by the importer, present on the branch). Original
source IDs, file hashes, visible content, and recorded dates are retained.
Unknown dates stay unknown. Files touched come only from explicit tool-input
paths, with links to the message containing that evidence. A missing tool result
remains pending rather than becoming an invented success. Eight fictional
`demo-*.json` fixtures remain available to tests; they are excluded from the
archive when real data is present.

## Full local Agentsview archive

The manual discovery/extraction/normalization workflow is documented in
[agent-archive.md](agent-archive.md). A private presentation projection can be
opened using:

```sh
npm run dev:archive --prefix website -- --port 4322
```

The default projection is `~/.local/share/token-agent-archive/preview`. Set
`AGENT_ARCHIVE_PREVIEW` to use another directory. This mode reads only the files
listed in its `private-preview` receipt, merges any missing public historical
sessions, and deduplicates `amp:T-…`/`T-…` identities. It serves large text assets
through a localhost-only development middleware. It does not copy private data
into the website, and production builds explicitly reject this environment
variable. Public builds continue to use checked-in data. Public transfer of new
Agentsview records uses the separate checksum-bound reviewed exporter.

## Viewer behavior

Detail pages use a standalone viewport-sized workspace with contained scroll
panes. Sidebar anchors scroll only the transcript, highlight the surrounding
message block, and center short messages while leaving space above tall ones.
Subagent links switch the center and right area to a compact inline transcript
with a sticky parent breadcrumb. Cached panes preserve scroll and disclosure
state; browser history and active-session metadata follow navigation.

Long transcripts render 100 messages initially and load additional static HTML
fragments on demand. Deep links load the necessary chunks using original message
IDs, including ordinal gaps. Large text bodies remain separately downloadable
`/agent-assets/<sha256>.txt` assets. These are complete available bodies, not
presentation-truncated output. Unsupported attachments and omitted events are
reported explicitly. Markdown prose, code fences, lists, headings, quotes, and
safe external links are rendered without executing transcript HTML. Unrecognized
syntax stays readable text. Replay and attachment retrieval remain separate work.

## Browser metadata and filters

The build computes `/agent-index.json`: compact metadata, file paths, tool-name
sets, and aggregate counts. The browser initially renders 25 rows and appends batches as its scroll sentinel approaches the viewport, with a keyboard-accessible Load more fallback. Offscreen appended rows use content visibility; returning from a session restores loaded rows and scroll position. Title search,
agent, human-turn, tool, inclusive UTC date range, file path/extension, and
session-content filters operate on that index, never on transcript bodies.
Options within a checkbox group are OR; groups combine with AND. Session-content
options distinguish annotations, failed tool calls, and linked delegated work.
Filter state and ordering persist in the URL. Native filter disclosures remember their collapsed state for the browser session; long tool labels truncate with full names on hover. A new visit
revalidates the index. Exceptionally large catalogues may need index partitioning.

## Optional annotations

Sidecars live in `website/src/data/agent-annotations/<source-filename>.json`.
The build validates the annotation schema, exact source-byte SHA-256, session ID,
and message references. Missing, invalid, stale, or empty annotations fall back
to raw messages and human-turn navigation. Sidecars do not supply provider or
branch identity. Use `website/.agents/skills/annotate-session/SKILL.md` for the
manual annotation workflow, worker/reducer prompts, schemas, and validator.

Time labels expose full UTC timestamps. Smooth scrolling and the native
archive-to-viewer transition respect reduced motion. Unsupported browsers use
ordinary navigation. The neutral icon-and-name treatment uses local ACP registry
SVGs for Codex, Claude, Amp, OpenCode, Kimi CLI, Devin, and Antigravity; unknown providers remain text-only. Alternative treatments are preserved
in `prototypes/agent-showcase.html`.

## Verification

```sh
node --test website/scripts/tests/*.test.mjs                                     # branch only
python3 -m unittest discover -s scripts/agent-archive -p 'test_*.py'
python3 -m unittest discover -s website/.agents/skills/annotate-session/tests -v # branch only
npm run build --prefix website
```

Verify incremental loading, combined facets, empty results/reset, chronological sorting,
subagent navigation, deep links, keyboard disclosures, and long files/tool output
on desktop and mobile. Private-mode production build rejection is intentional.

## Path presentation and continuation navigation

Displayed project paths use `session-paths.mjs` to remove known Token checkout,
legacy `rust-editor`, Claude worktree, and Codex worktree roots. The source JSON
and its checksums remain unchanged; unrelated filesystem paths and URLs are
preserved. Files metadata includes a separate `displayPath`, also used by the
cached filename filter. Transcript text, tool inputs/results, and headings use
the same presentation helper.

Detail headings retain full recorded titles. When the importer derived a compact
title from a human turn, the detail view recovers the complete prompt; catalogue
rows continue to use compact titles. Optional annotation titles can still be
toggled off to reveal the source heading.

Amp's explicit opening handoff preamble establishes predecessor/successor links.
The catalogue builds that graph once and normalizes public `T-…` and private
`amp:T-…` IDs. Detail pages show previous/next links and position within a linear
chain; branches are disclosed separately. Missing or ambiguous predecessors and
cycles are displayed as unresolved references, never guessed or merged. Generic
thread mentions and `read_thread` tool calls do not establish a continuation.
