# Amp conversation archive

This directory preserves the 171 Amp conversations referenced by
[`BUILDING_WITH_AI.md`](../BUILDING_WITH_AI.md). Keeping snapshots in Git makes
the project's development record available even when the original Amp profile
or thread sharing links are not.

Start with the chronological [thread index](INDEX.md), or follow the contextual
links in [`BUILDING_WITH_AI.md`](../BUILDING_WITH_AI.md).

## Archival strategy

This is a hybrid migration. GitHub is the canonical public record for every Amp
conversation cited by the project, while historical documents keep their Amp,
Oracle, and Librarian terminology where it provides useful context. The archive
does not attempt to mirror unrelated conversations from the author's Amp
account.

## Related document inventory

| Document | Role |
| --- | --- |
| [`BUILDING_WITH_AI.md`](../BUILDING_WITH_AI.md) | Maintained methodology and the complete contextual thread reference |
| [`archived/blog-post.md`](../archived/blog-post.md) | Historical narrative with a smaller selection of thread links |
| [`archived/misc/AMP_THREAD_ANALYSIS.md`](../archived/misc/AMP_THREAD_ANALYSIS.md) | Analysis of the first ten Amp threads |
| [`archived/misc/ORACLE_CONSULTATIONS.md`](../archived/misc/ORACLE_CONSULTATIONS.md) | Summary of Oracle-assisted design and review work |
| [`archived/misc/librarian-research-findings.md`](../archived/misc/librarian-research-findings.md) | Summary of Librarian research across the threads |
| [`archived/CODEBASE_REVIEW.md`](../archived/CODEBASE_REVIEW.md) | Historical architecture review |
| [`archived/UI_SYSTEM_CODEREVIEW.md`](../archived/UI_SYSTEM_CODEREVIEW.md) | Historical UI-system review |
| [`archived/FEEDBACK_BUGFIX_SESSION.md`](../archived/FEEDBACK_BUGFIX_SESSION.md) | Historical implementation feedback |
| [`archived/MULTI_CURSOR_SELECTION_GAPS.md`](../archived/MULTI_CURSOR_SELECTION_GAPS.md) | Historical multi-cursor gap analysis |
| [`archived/TOOLING_IMPROVEMENTS.md`](../archived/TOOLING_IMPROVEMENTS.md) | Historical tooling recommendations |
| [`archived/analysis/DX_IMPROVEMENTS.md`](../archived/analysis/DX_IMPROVEMENTS.md) | Historical developer-experience analysis |
| [`archived/syntax-highlighting.md`](../archived/syntax-highlighting.md) | Historical tree-sitter implementation plan |

The archived documents are supporting context, not current implementation
specifications. They mention Amp, Oracle, or Librarian but did not contain
retired profile or thread URLs that needed migration unless noted below.

## Inventory and provenance

Before this migration, the public project documentation depended on these Amp
links:

| Document | Retired Amp references | Migration |
| --- | ---: | --- |
| `docs/BUILDING_WITH_AI.md` | 192 thread links (171 unique), 1 profile link | Local transcript links |
| `website/src/pages/built-with-ai.astro` | 13 generated thread links, 2 profile links | GitHub archive links |
| `README.md` | 3 profile links | Repository archive links |
| `docs/archived/blog-post.md` | 6 thread links, 2 profile links | Local archive links |

The website homepage previously mentioned the Amp profile, but it had already
been changed to point readers to the repository before this migration.

Of the 171 snapshots, 146 were produced by `amp threads markdown`. The remaining
25 older UUID-era conversations were no longer available from that command and
were rendered from the local Amp session cache. The fallback keeps visible user
and assistant text, tool calls, and tool results, while omitting hidden thinking,
matching the scope of Amp's Markdown export.

## Publication review

The import normalized local home-directory paths and redacted internal Amp user
identifiers and crash-reporting keys. It also confirmed that every referenced
conversation has a matching transcript and that the migrated public-facing
documents contain no retired Amp profile or thread URLs. Before publication,
Gitleaks and TruffleHog were run over the final 41 MB archive; neither reported
a secret.

These are historical snapshots. They can contain commands, intermediate ideas,
failed approaches, and output that no longer describes the current codebase.
Treat current code and maintained documentation as the source of truth.
