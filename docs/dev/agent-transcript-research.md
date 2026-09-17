# Agent transcript viewer research

**Research date:** 2026-09-16  
**Source:** the frozen private manifest at `target/agent-archive/manifest.json`, read-only Agentsview SQLite queries, and structural inspection of the first, median, and final message and tool call for every selected session. The private machine-readable report is `target/verification/agent-viewer/largest-50-analysis.json`.

This document contains no transcript text or tool output. It is a design input
for the public session viewer, not publication approval for any session.

## Selection and terms

The sample is the 50 largest manifest entries with `inclusion_status` equal to
`included`, ranked by the frozen `message_count`. The selected IDs have SHA-256
checksum `60530ff394329b96ef2272f1747d7e7a51f115a2e240b3438a46578e081d7db6`.
The manifest’s selection rules cover the four Token aliases and verified Token
checkout/worktree evidence; see [the archive guide](agent-archive.md).

For this report, a **human turn** is a `user`-role message that is non-system,
non-sidechain, and whose source type is not tool-derived. It is useful for a
session-list filter, but it is deliberately not yet a provider-independent
publication contract: a normalizer must make the final distinction.

A **file touch** is a distinct non-empty `tool_calls.file_path`. It can mean a
file was read as well as edited, so it must never be labelled “files modified.”

## What the 50 sessions contain

| Measure | Result |
| --- | ---: |
| Normalized messages | 31,790 |
| Human turns under the stated definition | 530 total; median 0.5; p90 28; maximum 126 |
| Tool calls | 27,187 |
| Tool-result events | 17,917 |
| Tool-result characters | 203.4M; median 794; p90 12,520; maximum 2,355,534 |
| Sessions with a parent reference | 25 of 50 |
| Sessions with a recorded Git branch | 30 of 50 |
| Sessions with an observed tool file path | 32 of 50 |

The sample contains 30 Claude sessions, 18 Codex sessions, and one session
each from Devin and OpenCode. It has 30,945 assistant messages, 672 `user`
messages, 166 `tool` messages, and seven `system` messages. There are 5,125
sidechain messages and 232 messages with a dedicated thinking field. The stored
message models span fourteen values, so a fixed agent or model label would be
incorrect even inside many single sessions.

There is no reliable native display title across providers: no selected session
has `display_name`, 15 have `session_name`, and 49 have a first prompt. The
browser’s visible title therefore needs a reviewed editorial title when one is
available, then the provider session name, then a safely truncated first prompt.
The source and fallback must remain explicit in the publication data so search
does not promise a human-written title where one does not exist.

Tool execution is the dominant material. Bash is 21,667 of the 27,187 calls;
edit-like calls total 2,032 and read-like calls 1,370. Inputs are not uniformly
JSON: 14,944 calls have empty or non-JSON `input_json`, while 12,243 parse as
JSON. A renderer must preserve an opaque input representation rather than
requiring a JSON object.

Agentsview stores both call-embedded results and normalized result events. In
this sample, events contain 190.0M characters, nearly the 203.4M stored on tool
calls. They are alternate representations of much of the same execution, not a
second conversation to display.

## Design decisions supported by the data

The browser should list parent sessions by default. Half of the largest records
are delegated child runs, and many have a single human turn. Present child runs
inside their parent’s session detail as a compact “delegated runs” section with
provider, outcome, duration, and a link. The session browser may offer an
explicit **Include delegated runs** filter, but should not mix them into the
default chronological list.

The transcript should be an activity timeline grouped into **human turns** and
**agent work groups**, rather than one card per normalized message. An agent
work group contains its prose and the adjacent calls it issued. Tool rows stay
compact and collapsed by default; expose a precise output length and let the
reader expand output on demand. Very large output needs bounded in-page
rendering or separate loading rather than becoming initial HTML.

Render exactly one canonical result for a call. Prefer a call-embedded result
when available; otherwise use its matching normalized result event. Preserve
the unused representation in private source data for audit, but do not show it
as a duplicate terminal panel.

Show the provider’s current name dynamically from session metadata, with the
specific model only where it changes within an agent work group. `agent_label`
is empty in this sample, so it cannot be relied on for the displayed name.
There should be no fixed “Human + agent” participant field: every non-subagent
session already has that relationship, and delegated runs belong in their own
section.

Keep session chrome to one factual line, rather than repeating the title and
metadata in the sidebar, header, and footer. The recommended order is:

1. The browser row: title, provider badge, start date, human-turn count, and
   optional branch.
2. The detail header: title, dynamic agent/provider name, absolute session
   start date plus relative age, optional branch, and a stable URL containing
   the session ID.
3. Optional detail content: outcome, delegated runs, and reviewed annotations.

The browser row should be the only persistent summary when scrolling. Do not
repeat a “read-only” or participant label in the footer. The URL may be
`/agent/<session-id>`; the opaque ID needs no prominent visual treatment.

## Git and change metadata

The native session table stores `git_branch`, but no commit SHA or parent/base
commit. In the sample, branch values exist for 30 sessions and are only
`main`, `feat/ui-gallery`, and `refactor/clay-layout-rebased`. Commit references
cannot be safely inferred as session facts from arbitrary terminal output.

Tool inputs mention Git 3,847 times, `git commit` 397 times, and `numstat` 109
times. That makes an optional, reviewed enrichment pass feasible, but it is not
a native fact source. A robust enrichment pass can correlate a session’s
workspace and time interval with a local Git log, then record an evidence level
and only display a commit when the match is unambiguous.

Tool-level file paths cover 32 sessions: the median is 11 distinct touched
paths, p90 is 26, and the maximum is 63. Use the fallback label **Files
touched** when these fields are available. Show **Changes** with `+N −N` only
when a reviewed annotation supplies a diff-derived value. Hide the section
entirely when neither is available.

## Publication and annotation implications

The unannotated renderer must stand on its own: title, provider, date, agent
work groups, human turns, canonical tool results, and optional branch/file
touches are sufficient. Do not reserve blank panels for summaries, outcomes,
commits, or change counters.

Reviewed annotations may add an editorial title, a concise summary, modified
file list, diff counters, commit correlation, visible milestones, and a chosen
representative excerpt. Each field must be independently optional, retain
provenance, and fall back to the raw normalized timeline when omitted.

## Required implementation fixtures

Use the private report’s 50 session records for regression fixtures. The
implementation should cover: a compact parent-only session, a parent with many
delegated runs, a multi-model session, a session with no branch, a session with
a branch, a session with only file touches, non-JSON tool input, a failed tool
result, a compact-boundary message, a separate tool-role message, and a tool
output above two million characters.

## Follow-up: paths, outcomes, and Claude task artifacts

The imported archive needs presentation-only workspace path normalization. The
same Token repository appears as the historical `rust-editor` checkout, the
current `token-editor` checkout, Codex worktrees, and Claude worktrees. Keep
the provider path in source data, but show a Token-relative path anywhere that
path is displayed. Do not shorten a look-alike directory, a temporary file, or
a URL containing a path-shaped substring.

The private preview sampled 58,825 tool calls: 23,821 have an explicit
completed state, 39 explicit failed state, and 34,965 have no reliable result
state. The public historical Amp set adds 12,488 completed, 86 failed, and 36
pending calls. The renderer must therefore distinguish completed, failed,
pending, and unavailable status; a word in output prose is not evidence of an
outcome. When a provider does expose a precise state, retain it in the label:
cancelled, denied, timed out, interrupted, killed, queued, running, and waiting
are not interchangeable.

Claude has a separate background-task convention. 122 system task notifications
use `<task-notification>` records; 109 are completed, 12 failed, and one killed.
121 name a temporary task output file. Four later one-line Read records point to
that form of file, for example session `02e98a86-449d-4e86-bc96-550535b2a5dc`
at `ordinal:17`. Two can be joined to a matching notification by exact output
path. Render these records as a temporary task artifact and, where the matching
notification exists, show its recorded state and summary. The artifact body is
not part of the captured Read result, so never fabricate its contents; retain
the original reference in a disclosure for audit.
