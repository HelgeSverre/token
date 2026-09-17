# Private agent-session archive

`scripts/agent-archive/agent_archive.py` prepares a private, reviewable
snapshot of Token-related sessions from the local Agentsview archive. It is
deliberately separate from the website prototype and does not publish data,
generate Astro content, or make a transcript public.

The script uses only Python's standard library. Its defaults live under
`target/agent-archive/`, which Git ignores.

For a complete local corpus, use a durable private root outside `target/`.
Build tools may remove `target/` while extraction is running. The commands
below use `<private-archive-root>` for a directory such as
`/Users/helge/.local/share/token-agent-archive`; it remains outside the
repository and is never website source data.

## Research notes and limits

At the time of the initial archive research, the candidate set was about 777
sessions, including about 491 subagent sessions. These counts are discovery
evidence only and will change as Agentsview syncs. A read-only `session get`
check returned a flat session object including `transcript_revision`, which is
the revision used to freeze and verify extraction.

Representative sessions establish the requirements for a later renderer:

- Codex: 8,322 messages, 6,695 tool calls, about 125 million output
  characters, with one output over 2.3 million characters.
- Claude: 949 messages, linked child sessions, and compaction boundaries.
- OpenCode: 493 messages with thinking embedded in content.
- Historical Amp: 307 messages and missing timestamps.
- Devin: separate tool-role messages.

Message ordinals can have gaps. `input_json` is not always valid JSON, and
normalized tool result events can duplicate message-embedded tool results.
The extractor preserves these distinctions without trying to render, replay,
or resolve them.

## Discovery and frozen manifest

First inspect the proposed selection without creating files:

```sh
python3 scripts/agent-archive/agent_archive.py manifest --preview
```

When the selection is ready for human review, freeze it:

```sh
python3 scripts/agent-archive/agent_archive.py manifest
```

The manifest lists sessions under `token-editor`, `token_editor`,
`rust-editor`, and `rust_editor`, including child, automated, and one-shot
sessions. It also reads Agentsview's SQLite identity snapshots with a
read-only connection. A session is included only when it has strong evidence:
the normalized `github.com/HelgeSverre/token` remote, a known historical
checkout root, or a verified linked worktree. The known roots are
`/Users/helge/code/token-editor` and `/Users/helge/code/rust-editor`.

The Codex pattern `/Users/helge/.codex/worktrees/*/token-editor` is a candidate
only when the Token remote or a known Token repository root verifies it. A path
merely containing `token` is not enough. Alias-only legacy records and uncertain
parent/child relationships are marked `review`; extraction skips those unless
explicitly requested. The manifest is frozen: rerun with `--overwrite` only
after reviewing the changed candidate set. Extraction rejects a source whose
transcript revision differs from its frozen manifest entry, so sync cannot
silently substitute a newer transcript.

The checked-in historical Amp archive is a narrow additional strong-evidence
source. `docs/ampcode-threads/T-<uuid>.md` promotes only the matching canonical
Agentsview `amp:T-<uuid>` session. It does not include other Amp records merely
because their project alias is similar.

To create the durable full local corpus, freeze the manifest and keep all paths
under the same private root:

```sh
python3 scripts/agent-archive/agent_archive.py manifest \
  --output <private-archive-root>/manifest.json
python3 scripts/agent-archive/agent_archive.py extract \
  --manifest <private-archive-root>/manifest.json \
  --output-dir <private-archive-root>/sessions
```

## Extraction

Extract the frozen `included` sessions manually:

```sh
python3 scripts/agent-archive/agent_archive.py extract
```

Include records that need relationship or identity review only after inspecting
the manifest:

```sh
python3 scripts/agent-archive/agent_archive.py extract --include-review
```

Each session becomes one JSON file under `target/agent-archive/sessions/`.
`index.json` tracks status, source revision, checksum, message count, and
errors. The extractor walks message pages in ascending ordinal order, advances
from the last actual ordinal so gaps are preserved, and checks the transcript
revision before and after each attempt. It retries revision changes, records an
incomplete result if it cannot obtain a stable transcript, writes files
atomically, and skips only snapshots whose revision and saved index checksum
still match the same frozen manifest.
If any selected session is incomplete, the command exits nonzero after writing
its resumable status to the private index.

Snapshots preserve the returned session metadata, provider message shape,
message-embedded tool data, separate normalized result events, and all returned
thinking fields. Result events can duplicate message-embedded tool results;
they remain separate so a future renderer has to make that choice explicitly.
Snapshots inventory structured attachment-like references but do not fetch
assets; their completeness field states that honestly. The output is
intentionally private and may contain sensitive material. Review and sanitize
snapshots before any future conversion to website data.

## Private normalization and cached browser metadata

After extraction is complete, run the separate offline normalizer. It performs
no network or Agentsview calls and reads only extraction-index records marked
`complete` (or a verified, previously `skipped_unchanged` record):

```sh
python3 scripts/agent-archive/normalize_archive.py normalize
```

It writes only ignored private data under `target/agent-archive/normalized/`:

```text
normalized/
  index.json
  sessions/<sha256-of-session-id>.json
  assets/<sha256-of-content>.txt
```

The normalizer verifies every input snapshot against the checksum in the frozen
extraction index before it is used. A bad input becomes a resumable `rejected`
index record and never replaces an existing normalized session. A rerun skips a
session whose snapshot checksum is unchanged. Use `--force` only when reviewing
the same frozen inputs again. The normalized index also records a
`normalizationVersion`; changing the normalizer's presentation semantics
automatically regenerates records from unchanged private snapshots.

Each private normalized session retains the fixture-compatible core needed by a
future viewer: `id`, dynamic `agent`, `title`, `startedAt`, `branch`,
`parentSessionId`, `files`, and ordered `messages`. A message has its original
ordinal, role, human-turn classification, timestamp, text, thinking, and a
fixture-compatible `tools` list. It also includes private provenance, a
provider-neutral top-level `toolCalls` list, unmatched result events,
attachment inventory, and diagnostics. Unknown message roles become
`unknown_event`; they are preserved and are never silently presented as human
turns.

Human-turn counts exclude system, sidechain, and explicitly tool-derived
user-role events. Tool inputs preserve JSON when parseable and preserve the
original text as opaque input otherwise. The normalizer emits one canonical
tool result, preferring an embedded result and retaining a matching normalized
result event as a private alternate. Unmatched result events remain private
events, not a second displayed result.

Bodies over 64 KiB become content-addressed assets instead of being embedded in
the session JSON. The fixture field is `null` in that case and its companion
`textContent`, `thinkingContent`, `inputContent`, or `outputContent` contains:

```json
{
  "storage": "asset",
  "path": "assets/<sha256>.txt",
  "href": "/agent-assets/<sha256>.txt",
  "sha256": "...",
  "bytes": 123456,
  "characters": 123456,
  "mediaType": "text/plain; charset=utf-8"
}
```

`href` is an explicit future reviewed-publication convention, not a generated
public route. Inline content has the same size fields with `storage: "inline"`
and `text`. The compact normalized index contains only cached listing/facet
metadata: title fallback source, agent, dates, branch, parent relation, human
turn/message/tool counts, tool-name histogram, file-touch count, attachment
count, and event presence. It never embeds transcript or tool-result bodies.
This is the source for paginated or infinite session browsing without loading
all transcript data.

Missing or malformed timestamps normalize to `null`; the original value stays
in private source provenance. Observed tool paths are always `touched`, never
claimed as modified and never assigned diff counters. The normalizer does not
infer commits, edits, attachment availability, Markdown rendering, or replay
timing.

For the durable corpus, normalize the matching private snapshot directory:

```sh
python3 scripts/agent-archive/normalize_archive.py normalize \
  --input-dir <private-archive-root>/sessions \
  --output-dir <private-archive-root>/normalized
```

## Local development preview

`export-preview` creates a local viewer-shaped projection of every complete
normalized session without an allowlist or any claim that its content is safe
for publication. It verifies each normalized checksum, safe route IDs, and
every projected asset before writing.

```sh
python3 scripts/agent-archive/normalize_archive.py export-preview \
  --normalized-dir <private-archive-root>/normalized \
  --destination <private-archive-root>/preview
```

The preview has `index.json` and `receipt.json`, each with purpose
`private-preview`. Their `sessions` entries contain the canonical session ID,
projection path, and checksum. A development server must load only those listed
entries, so stale files are never part of the preview. This command does not
write to `website/`, does not use review flags, and is not publication approval.

## Explicit review transfer

Normalization is not publication approval. A reviewer must create an exact,
checksum-bound allowlist such as:

```json
{
  "schemaVersion": 1,
  "sessions": [
    {
      "id": "codex:opaque-session-id",
      "status": "approved",
      "normalizedChecksum": "copy this exact checksum from normalized/index.json"
    }
  ]
}
```

Then run an explicit transfer to a named private review directory:

```sh
python3 scripts/agent-archive/normalize_archive.py promote \
  --allowlist target/agent-archive/review-allowlist.json \
  --destination target/agent-archive/reviewed
```

The command rejects duplicate approvals, incomplete source records, checksum
mismatches, missing assets, and unsafe asset paths. It copies only allowlisted
records and their referenced assets, then writes a transfer receipt. It has no
website default and does not sanitize or publish data. A later publication pass
must review/redact this transfer and deliberately choose any files that belong
under `website/src/data/` or a public assets route.

## Reviewed presentation staging

After a separate content review, an explicit public-review allowlist can create
a **presentation-only staging payload**. This removes normalized audit/source
fields, top-level tool-call records, unmatched events, attachment inventory,
and dedicated thinking fields. It retains the compact viewer message/tool shape
and only copies assets referenced by that presentation shape.

The projected session records an explicit, content-free omission inventory in
`publication.omitted`: counts for unmatched result events, attachment
references, and dedicated-thinking messages. These facts remain in the private
normalized record; the counters prevent a future renderer from implying that
the public transcript is complete. User-role records identified as system,
sidechain, compaction, or tool-derived normalize to non-user viewer roles, so
the website human-turn counter remains aligned with the cached index summary.

Every approved record must name the exact normalized checksum and affirm all
three review surfaces, even if one is absent:

```json
{
  "schemaVersion": 1,
  "sessions": [{
    "id": "codex:opaque-session-id",
    "status": "approved_public",
    "normalizedChecksum": "copy this exact checksum from normalized/index.json",
    "review": {
      "publicTextReviewed": true,
      "publicAssetsReviewed": true,
      "thinkingReviewed": true
    }
  }]
}
```

Run the projection manually:

```sh
python3 scripts/agent-archive/normalize_archive.py export-reviewed \
  --allowlist target/agent-archive/public-review-allowlist.json
```

Its default destination is the ignored private
`target/agent-archive/normalized/website-ready/`, containing `sessions/` and
`agent-assets/`. It does not write to `website/` unless a later, explicit
`--destination` selects it. The command verifies the
source session and asset checksums before copying and refuses approval records
without all three explicit review flags. It cannot prove prose has been
redacted or that embedded thinking was recognized by a provider, so the review
flags are a human assertion rather than automatic sanitization. After
inspection, a separate deliberate change may copy selected staged records into
`website/src/data/agent-sessions/` and selected assets into
`website/public/agent-assets/`.

Opaque session, parent, and subagent IDs must be safe route segments before
projection. The command rejects slash, backslash, query/hash, dot-only, and
control-character IDs rather than inventing a replacement; ordinary opaque IDs
containing colons and dashes remain intact.

## Limits and future work

Do not run a bulk extraction until the manifest has been reviewed. The archive
toolchain does not infer publication consent, redact secrets, render Markdown,
replay timing, download attachments, correlate commits, or create Astro
collections. Those remain separate publication work after review of private
normalized data.

Run the focused offline tests with:

```sh
python3 -m unittest \
  scripts/agent-archive/test_agent_archive.py \
  scripts/agent-archive/test_normalize_archive.py
```
