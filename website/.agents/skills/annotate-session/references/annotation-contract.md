# Session annotation sidecar v1

An annotation is optional, reviewed editorial metadata for one source session
JSON file. It is not a transcript transformation. The final object must pass
[`session-annotation.schema.json`](session-annotation.schema.json).

## Binding and fallback

Every sidecar has these required fields:

```json
{
  "schemaVersion": 1,
  "sessionId": "codex:example",
  "sourceDigest": "a 64-character lowercase SHA-256 digest"
}
```

`sourceDigest` is the SHA-256 digest of the **exact UTF-8 bytes** of the source
JSON file. Do not parse and reserialize the source before hashing; serializer
differences would make a valid annotation look stale. The supplied validator
uses the source file bytes directly.

A consumer must ignore the whole sidecar when it is missing, invalid, has a
different session ID, or has a different digest. It must not make an empty
“annotations” area for a sidecar containing only the three required fields.
It may independently omit any optional section that is absent.

For archive snapshots made by `scripts/agent-archive/agent_archive.py`, the
session ID is `session.id`. A message reference is `message.id` when that is a
non-empty string; otherwise it is `ordinal:<ordinal>`. The validator derives
the same references and rejects citations not present in the source.

## Editorial fields

All fields below are optional. Omit an uncertain section rather than emitting a
placeholder or an empty list.

| Field | Purpose | Evidence requirement |
| --- | --- | --- |
| `title` | Concise session title | Top-level `evidenceMessageIds` is required. |
| `summary` | Brief faithful overview | Top-level `evidenceMessageIds` is required. |
| `chapters` | Ordered reading landmarks | Every chapter has `startMessageId` and evidence. `endMessageId` is optional. |
| `outcome` | Evidence-supported state at the end of the shown work | Required evidence. Do not imply completion from silence. |
| `highlights` | A few useful decisions, tests, blockers, or handoffs | A message anchor and evidence are required. |
| `files` | Explicitly supported touched-file notes | Evidence is required. Additions/deletions are allowed only when an explicit diff/stat supports those exact counts. |

`evidenceMessageIds` preserves auditability for editorial claims. It may cite a
message outside a chapter’s visual interval when needed. The final validator
checks only that IDs exist; reviewers must also judge whether the cited content
actually supports the wording.

The schema deliberately has no fields for agent/provider, branch, commit,
timestamps, workspace, tool status, or inferred Git facts. Render those from
the source only when a future extraction pipeline has a dedicated verified
field. Never add them to an annotation as a shortcut.

## Examples

An entirely unannotated but valid sidecar is useful when review finds no safe
editorial additions:

```json
{
  "schemaVersion": 1,
  "sessionId": "codex:example",
  "sourceDigest": "3c4e5a239489fcf131daf439569e6587c7fd0a9c45584128caa71d91459742e0"
}
```

An abbreviated reviewed annotation:

```json
{
  "schemaVersion": 1,
  "sessionId": "codex:example",
  "sourceDigest": "3c4e5a239489fcf131daf439569e6587c7fd0a9c45584128caa71d91459742e0",
  "title": "Repair selection after soft wrap",
  "summary": "The session traced selection geometry, applied a focused fix, and recorded a passing targeted check.",
  "evidenceMessageIds": ["ordinal:4", "ordinal:11", "ordinal:14"],
  "chapters": [
    {
      "id": "trace-selection-geometry",
      "title": "Trace selection geometry",
      "startMessageId": "ordinal:4",
      "endMessageId": "ordinal:11",
      "summary": "The investigation narrowed the issue to the wrapped-row geometry path.",
      "evidenceMessageIds": ["ordinal:4", "ordinal:8"]
    }
  ],
  "outcome": {
    "summary": "A targeted check passed after the change; this does not establish broader test coverage.",
    "evidenceMessageIds": ["ordinal:14"]
  }
}
```

This example is illustrative. Its claims must not be reused for a different
source session merely because its IDs look similar.
