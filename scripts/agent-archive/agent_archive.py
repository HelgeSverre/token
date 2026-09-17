#!/usr/bin/env python3
"""Create and extract private, reviewable Agentsview snapshots for Token.

This tool deliberately does not create website data.  Its only default output is
under target/agent-archive, which is ignored by git.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import sqlite3
import subprocess
import sys
import tempfile
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Callable


SCHEMA_VERSION = 1
PROJECT_ALIASES = ("token-editor", "token_editor", "rust-editor", "rust_editor")
KNOWN_ROOTS = (
    "/Users/helge/code/token-editor",
    "/Users/helge/code/rust-editor",
)
TARGET_REMOTE = "github.com/helgesverre/token"
CODEX_WORKTREE = re.compile(r"^/Users/helge/\.codex/worktrees/[^/]+/token-editor(?:/|$)")
PUBLIC_AMP_ARCHIVE = Path(__file__).resolve().parents[2] / "docs" / "ampcode-threads"
DEFAULT_DB = Path.home() / ".agentsview" / "sessions.db"
DEFAULT_MANIFEST = Path("target/agent-archive/manifest.json")
DEFAULT_SESSIONS = Path("target/agent-archive/sessions")

Json = dict[str, Any]
Runner = Callable[[list[str]], Any]


def utc_now() -> str:
    return datetime.now(timezone.utc).isoformat(timespec="seconds").replace("+00:00", "Z")


def canonical_json(value: Any) -> bytes:
    return (json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":")) + "\n").encode()


def checksum(value: Any) -> str:
    return hashlib.sha256(canonical_json(value)).hexdigest()


def atomic_json(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile("wb", dir=path.parent, delete=False, prefix=f".{path.name}.") as handle:
        temporary = Path(handle.name)
        handle.write(canonical_json(value))
    os.replace(temporary, path)


def read_json(path: Path) -> Any:
    with path.open(encoding="utf-8") as handle:
        return json.load(handle)


def normalize_remote(value: str | None) -> str:
    remote = (value or "").strip().lower().rstrip("/")
    scp = re.match(r"^[^@/]+@([^:/]+):(.+)$", remote)
    if scp:
        remote = f"{scp.group(1)}/{scp.group(2)}"
    remote = re.sub(r"^(?:https?|ssh)://", "", remote)
    remote = re.sub(r"^(?:[^@/]+@)", "", remote)
    if remote.endswith(".git"):
        remote = remote[:-4]
    return remote


def normalized_path(value: str | None) -> str:
    return (value or "").rstrip("/")


def under(path: str, root: str) -> bool:
    path, root = normalized_path(path), normalized_path(root)
    return path == root or path.startswith(root + "/")


def cli_json(command: list[str]) -> Any:
    completed = subprocess.run(command, check=True, capture_output=True, text=True)
    return json.loads(completed.stdout)


def extract_sessions(value: Any) -> list[Json]:
    if isinstance(value, list):
        return [item for item in value if isinstance(item, dict)]
    if isinstance(value, dict) and isinstance(value.get("sessions"), list):
        return [item for item in value["sessions"] if isinstance(item, dict)]
    raise ValueError("Agentsview session list did not return a sessions array")


def list_alias_sessions(agentsview: str, runner: Runner) -> dict[str, Json]:
    found: dict[str, Json] = {}
    for alias in PROJECT_ALIASES:
        cursor: str | None = None
        while True:
            command = [
                agentsview, "session", "list", "--project", alias, "--include-children",
                "--include-automated", "--include-one-shot", "--limit", "500", "--json",
            ]
            if cursor:
                command.extend(["--cursor", cursor])
            page = runner(command)
            for session in extract_sessions(page):
                session_id = session.get("id")
                if isinstance(session_id, str) and session_id:
                    found[session_id] = session
            cursor = page.get("next_cursor") if isinstance(page, dict) else None
            if not isinstance(cursor, str) or not cursor:
                break
    return found


def identity_rows(database: Path) -> dict[str, Json]:
    """Read identity snapshots with SQLite's read-only URI; never mutate Agentsview."""
    if not database.exists():
        raise FileNotFoundError(f"Agentsview database is not available: {database}")
    uri = f"file:{database}?mode=ro"
    connection = sqlite3.connect(uri, uri=True)
    connection.row_factory = sqlite3.Row
    try:
        tables = {row[0] for row in connection.execute("select name from sqlite_master where type='table'")}
        if "sessions" not in tables or "session_project_identity_snapshots" not in tables:
            raise RuntimeError("Agentsview database is missing the session identity snapshot schema")
        rows = connection.execute(
            """
            select s.id, s.project, s.machine, s.agent, s.source_session_id, s.cwd, s.transcript_revision,
                   s.parent_session_id, s.parser_parent_session_id, s.relationship_type,
                   s.deleted_at, s.started_at, s.ended_at, s.message_count,
                   i.root_path, i.git_remote, i.normalized_remote, i.repository_path,
                   i.worktree_root_path, i.worktree_relationship, i.remote_resolution,
                   i.observed_at
            from sessions s
            left join session_project_identity_snapshots i on i.session_id = s.id
            where s.deleted_at is null
            """
        ).fetchall()
        return {row["id"]: dict(row) for row in rows}
    finally:
        connection.close()


def public_amp_archive_session_ids(archive: Path = PUBLIC_AMP_ARCHIVE) -> set[str]:
    """Return Amp session IDs which have checked-in Token transcript evidence.

    The historical public archive uses ``T-<uuid>.md`` filenames while
    Agentsview uses canonical ``amp:T-<uuid>`` IDs.  This is deliberate evidence
    for the known historical Token corpus, not a broad project-name heuristic.
    """
    if not archive.is_dir():
        return set()
    return {f"amp:{path.stem}" for path in archive.glob("T-*.md") if path.stem[2:]}


def tool_result_events(database: Path | None, session_id: str) -> tuple[list[Json], str]:
    """Preserve normalized result events separately from message tool calls.

    They may duplicate message-embedded results. Keeping both in the private
    snapshot lets a later renderer choose one representation explicitly rather
    than silently losing provider-specific event data.
    """
    if database is None:
        return [], "not_requested"
    if not database.exists():
        raise RuntimeError(f"Agentsview database is not available for tool result events: {database}")
    try:
        connection = sqlite3.connect(f"file:{database}?mode=ro", uri=True)
        connection.row_factory = sqlite3.Row
        exists = connection.execute(
            "select 1 from sqlite_master where type='table' and name='tool_result_events'"
        ).fetchone()
        if exists is None:
            return [], "unavailable_schema"
        rows = connection.execute(
            """
            select tool_call_message_ordinal, call_index, tool_use_id, agent_id,
                   subagent_session_id, source, status, content, content_length,
                   timestamp, event_index, summary_participates
            from tool_result_events
            where session_id = ?
            order by tool_call_message_ordinal, call_index, event_index
            """,
            (session_id,),
        ).fetchall()
        return [dict(row) for row in rows], "preserved_from_agentsview_database"
    except sqlite3.Error as error:
        raise RuntimeError(f"could not read Agentsview tool result events: {error}") from error
    finally:
        if "connection" in locals():
            connection.close()


def evidence_for(session: Json, verified_amp_ids: set[str] | None = None) -> list[Json]:
    evidence: list[Json] = []
    project = session.get("project", "")
    if project in PROJECT_ALIASES:
        evidence.append({"kind": "project_alias", "value": project, "strength": "weak"})
    paths = [("cwd", session.get("cwd", "")), ("identity_root", session.get("root_path", ""))]
    for label, path in paths:
        if not isinstance(path, str) or not path:
            continue
        for root in KNOWN_ROOTS:
            if under(path, root):
                evidence.append({"kind": "known_workspace", "value": path, "root": root, "strength": "strong"})
                break
    remotes = (session.get("normalized_remote", ""), session.get("git_remote", ""))
    if any(normalize_remote(remote) == TARGET_REMOTE for remote in remotes if isinstance(remote, str)):
        evidence.append({"kind": "normalized_git_remote", "value": TARGET_REMOTE, "strength": "strong"})
    worktree_paths = [session.get(key, "") for key in ("cwd", "root_path", "worktree_root_path")]
    if any(isinstance(path, str) and CODEX_WORKTREE.match(path) for path in worktree_paths):
        repository = session.get("repository_path", "")
        known_repository = any(under(repository, root) for root in KNOWN_ROOTS)
        remote_verified = any(normalize_remote(remote) == TARGET_REMOTE for remote in remotes if isinstance(remote, str))
        evidence.append({
            "kind": "codex_worktree",
            "value": next(path for path in worktree_paths if isinstance(path, str) and CODEX_WORKTREE.match(path)),
            "strength": "strong" if remote_verified or known_repository else "review",
            "verified_by": "git_remote" if remote_verified else ("known_repository" if known_repository else "none"),
        })
    session_id = session.get("id")
    if session.get("agent") == "amp" and isinstance(session_id, str) and session_id in (verified_amp_ids or set()):
        evidence.append({
            "kind": "repository_amp_archive",
            "value": f"docs/ampcode-threads/{session_id.removeprefix('amp:')}.md",
            "strength": "strong",
        })
    return evidence


def selection_from_evidence(evidence: list[Json]) -> tuple[str, str]:
    strong = any(item["strength"] == "strong" for item in evidence)
    if strong:
        return "included", "strong"
    if evidence:
        return "review", "weak"
    return "excluded", "none"


def build_manifest(
    alias_sessions: dict[str, Json], identities: dict[str, Json],
    verified_amp_ids: set[str] | None = None,
) -> Json:
    verified_amp_ids = public_amp_archive_session_ids() if verified_amp_ids is None else verified_amp_ids
    candidates: dict[str, Json] = dict(alias_sessions)
    for session_id, identity in identities.items():
        evidence = evidence_for(identity, verified_amp_ids)
        if any(item["strength"] in ("strong", "review") for item in evidence):
            candidates[session_id] = {**identity, **candidates.get(session_id, {})}

    # Preserve children of selected sessions for human review even when their
    # own cwd/project evidence is absent. This is deliberately one-way: a
    # selected child does not pull an unverified parent into extraction.
    changed = True
    while changed:
        changed = False
        selected_ids = set(candidates)
        for session_id, identity in identities.items():
            parent_ids = {
                identity.get("parent_session_id"), identity.get("parser_parent_session_id"),
            }
            if session_id not in candidates and any(parent_id in selected_ids for parent_id in parent_ids if parent_id):
                candidates[session_id] = {**identity, "_relationship_only_candidate": True}
                changed = True

    entries: list[Json] = []
    for session_id in sorted(candidates):
        merged = {**identities.get(session_id, {}), **candidates[session_id]}
        evidence = evidence_for(merged, verified_amp_ids)
        status, confidence = selection_from_evidence(evidence)
        direct_parent_id = merged.get("parent_session_id") or None
        parser_parent_id = merged.get("parser_parent_session_id") or None
        parent_id = direct_parent_id or parser_parent_id
        relationship_only = bool(merged.get("_relationship_only_candidate"))
        if relationship_only:
            status, confidence = "review", "relationship_only"
        entry: Json = {
            "id": session_id,
            "provider": merged.get("agent", ""),
            "project": merged.get("project", ""),
            "machine": merged.get("machine", ""),
            "source_session_id": merged.get("source_session_id", ""),
            "workspace": {
                "cwd": merged.get("cwd", ""), "identity_root": merged.get("root_path", ""),
                "repository_path": merged.get("repository_path", ""),
                "worktree_root_path": merged.get("worktree_root_path", ""),
                "normalized_remote": normalize_remote(merged.get("normalized_remote") or merged.get("git_remote")),
            },
            "matching_evidence": evidence,
            "confidence": confidence,
            "inclusion_status": status,
            "parent_session_id": parent_id or None,
            "parent_reference": {
                "parent_session_id": direct_parent_id,
                "parser_parent_session_id": parser_parent_id,
                "selected_parent_id": parent_id or None,
                "selected_parent_source": "parent_session_id" if direct_parent_id else (
                    "parser_parent_session_id" if parser_parent_id else None
                ),
            },
            "relationship_type": merged.get("relationship_type", ""),
            "transcript_revision": str(merged.get("transcript_revision", "")),
            "message_count": merged.get("message_count"),
            "started_at": merged.get("started_at"),
            "ended_at": merged.get("ended_at"),
            "relationship_review": "relationship_only_child_of_selected_session" if relationship_only else None,
        }
        entries.append(entry)

    entry_by_id = {entry["id"]: entry for entry in entries}
    for entry in entries:
        parent_id = entry["parent_session_id"]
        if not parent_id:
            continue
        parent = entry_by_id.get(parent_id)
        if parent is None:
            entry["relationship_review"] = "parent_not_selected; parent was not imported automatically"
            entry["inclusion_status"] = "review"
        elif parent["inclusion_status"] != "included" or not same_project_identity(entry, parent):
            mismatch = "parent_child_project_or_selection_mismatch"
            entry["relationship_review"] = (
                f"{entry['relationship_review']}; {mismatch}" if entry["relationship_review"] else mismatch
            )
            entry["inclusion_status"] = "review"

    return {
        "schema_version": SCHEMA_VERSION,
        "generated_at": utc_now(),
        "purpose": "private Agentsview discovery manifest; not website publication data",
        "selection": {
            "project_aliases": list(PROJECT_ALIASES), "normalized_remote": TARGET_REMOTE,
            "known_checkout_roots": list(KNOWN_ROOTS),
            "codex_worktree_rule": ".codex/worktrees/*/token-editor requires the Token remote or a known Token repository root",
            "repository_amp_archive_rule": "docs/ampcode-threads/T-<uuid>.md verifies the matching amp:T-<uuid> historical Token session",
        },
        "sessions": entries,
    }


def same_project_identity(left: Json, right: Json) -> bool:
    if left["project"] == right["project"]:
        return True
    return (
        left["project"] in PROJECT_ALIASES and right["project"] in PROJECT_ALIASES
        and left["confidence"] == "strong" and right["confidence"] == "strong"
    )


def manifest_summary(manifest: Json) -> Json:
    entries = manifest["sessions"]
    return {
        "sessions": len(entries),
        "included": sum(entry["inclusion_status"] == "included" for entry in entries),
        "review": sum(entry["inclusion_status"] == "review" for entry in entries),
        "providers": sorted({entry["provider"] for entry in entries if entry["provider"]}),
    }


def get_session(agentsview: str, session_id: str, runner: Runner) -> Json:
    value = runner([agentsview, "session", "get", session_id, "--json"])
    if not isinstance(value, dict):
        raise ValueError(f"session get for {session_id} did not return an object")
    return value


def fetch_all_messages(agentsview: str, session_id: str, page_size: int, runner: Runner) -> list[Json]:
    messages: list[Json] = []
    next_ordinal = 0
    seen_ordinals: set[int] = set()
    while True:
        page = runner([
            agentsview, "session", "messages", session_id, "--from", str(next_ordinal),
            "--limit", str(page_size), "--direction", "asc", "--json",
        ])
        if not isinstance(page, dict) or not isinstance(page.get("messages"), list):
            raise ValueError(f"message page for {session_id} did not return a messages array")
        if any(not isinstance(item, dict) for item in page["messages"]):
            raise ValueError(f"message page for {session_id} contains a non-object message")
        current = page["messages"]
        if not current:
            return messages
        ordinals = [item.get("ordinal") for item in current]
        if not all(isinstance(ordinal, int) for ordinal in ordinals):
            raise ValueError(f"message page for {session_id} has a non-integer ordinal")
        if ordinals != sorted(ordinals) or len(ordinals) != len(set(ordinals)):
            raise ValueError(f"message page for {session_id} has unordered or duplicate ordinals")
        if any(ordinal in seen_ordinals for ordinal in ordinals):
            raise ValueError(f"message pagination for {session_id} repeated an ordinal")
        if min(ordinals) < next_ordinal:
            raise ValueError(f"message pagination for {session_id} moved backwards")
        messages.extend(current)
        seen_ordinals.update(ordinals)
        advanced_to = max(ordinals) + 1
        if advanced_to <= next_ordinal:
            raise ValueError(f"message pagination for {session_id} did not advance")
        next_ordinal = advanced_to


def attachment_references(value: Any, pointer: str = "$") -> list[Json]:
    references: list[Json] = []
    keywords = ("attachment", "asset", "image", "media", "file", "url")
    if isinstance(value, dict):
        for key, child in value.items():
            child_pointer = f"{pointer}.{key}"
            if any(word in key.lower() for word in keywords) and child not in (None, "", [], {}):
                references.append({"pointer": child_pointer, "key": key, "value": child})
            references.extend(attachment_references(child, child_pointer))
    elif isinstance(value, list):
        for index, child in enumerate(value):
            references.extend(attachment_references(child, f"{pointer}[{index}]"))
    elif isinstance(value, str):
        # Tool input and some provider content are JSON encoded inside a
        # string. Inspect valid JSON without changing the original content.
        if value.lstrip().startswith(("{", "[")):
            try:
                decoded = json.loads(value)
            except json.JSONDecodeError:
                pass
            else:
                references.extend(attachment_references(decoded, f"{pointer}<json>"))
    return references


def snapshot_session(
    agentsview: str, entry: Json, page_size: int, retries: int, runner: Runner,
    database: Path | None = None,
) -> Json:
    session_id = entry["id"]
    attempts: list[Json] = []
    for attempt in range(1, retries + 2):
        before = get_session(agentsview, session_id, runner)
        before_revision = str(before.get("transcript_revision", ""))
        expected_revision = str(entry.get("transcript_revision", ""))
        if expected_revision and before_revision != expected_revision:
            raise RuntimeError(
                f"source transcript revision {before_revision!r} differs from frozen manifest revision {expected_revision!r}; "
                "regenerate and review the manifest before extracting"
            )
        messages = fetch_all_messages(agentsview, session_id, page_size, runner)
        result_events, result_event_status = tool_result_events(database, session_id)
        after = get_session(agentsview, session_id, runner)
        after_revision = str(after.get("transcript_revision", ""))
        attempts.append({"attempt": attempt, "before_revision": before_revision, "after_revision": after_revision})
        if before_revision == after_revision:
            expected_count = entry.get("message_count")
            if isinstance(expected_count, int) and len(messages) != expected_count:
                raise RuntimeError(
                    f"page walk yielded {len(messages)} messages but frozen manifest records {expected_count}"
                )
            references = attachment_references(messages)
            references.extend(attachment_references(result_events, "$.tool_result_events"))
            return {
                "schema_version": SCHEMA_VERSION,
                "session": after,
                "manifest_entry": entry,
                "messages": messages,
                "tool_result_events": result_events,
                "extraction": {
                    "completed_at": utc_now(), "attempts": attempts, "source_revision": after_revision,
                    "manifest_revision": entry.get("transcript_revision", ""), "message_count": len(messages),
                    "message_ordinals": [message["ordinal"] for message in messages],
                    "completeness": {
                        "messages": "complete_stable_page_walk",
                        "attachments": "referenced_not_fetched" if references else "no_structured_references_found",
                        "thinking": "preserved_as_returned; publication review_required",
                        "tool_result_events": result_event_status,
                    },
                    "attachment_references": references,
                },
            }
    raise RuntimeError(f"transcript revision changed during all {retries + 1} extraction attempts: {attempts}")


def existing_complete(path: Path, source_revision: str, expected_checksum: str | None) -> tuple[str, int | None] | None:
    if not path.exists():
        return None
    try:
        snapshot = read_json(path)
        if snapshot.get("extraction", {}).get("source_revision") != source_revision:
            return None
        actual_checksum = checksum(snapshot)
        if not expected_checksum or actual_checksum != expected_checksum:
            return None
        message_count = snapshot.get("extraction", {}).get("message_count")
        return actual_checksum, message_count if isinstance(message_count, int) else None
    except (OSError, ValueError, json.JSONDecodeError):
        return None


def extract_manifest(
    manifest: Json, agentsview: str, output_dir: Path, page_size: int, retries: int,
    include_review: bool, runner: Runner, database: Path | None = None,
) -> Json:
    output_dir.mkdir(parents=True, exist_ok=True)
    index_path = output_dir / "index.json"
    manifest_checksum = checksum(manifest)
    index: Json = read_json(index_path) if index_path.exists() else {}
    if index.get("schema_version") != SCHEMA_VERSION or index.get("manifest_checksum") != manifest_checksum:
        index = {
            "schema_version": SCHEMA_VERSION, "purpose": "private Agentsview extraction index",
            "manifest_checksum": manifest_checksum, "sessions": {},
        }
    outcomes: dict[str, Json] = index.setdefault("sessions", {})
    for entry in manifest["sessions"]:
        allowed_statuses = {"included", "review"} if include_review else {"included"}
        if entry["inclusion_status"] not in allowed_statuses:
            continue
        session_id = entry["id"]
        destination = output_dir / f"{hashlib.sha256(session_id.encode()).hexdigest()}.json"
        try:
            current = get_session(agentsview, session_id, runner)
            current_revision = str(current.get("transcript_revision", ""))
            prior = outcomes.get(session_id, {})
            previous = existing_complete(destination, current_revision, prior.get("checksum"))
            if previous:
                old_checksum, old_message_count = previous
                outcomes[session_id] = {
                    "status": "skipped_unchanged", "path": destination.name,
                    "source_revision": current_revision, "checksum": old_checksum,
                    "message_count": old_message_count, "updated_at": utc_now(),
                }
                atomic_json(index_path, index)
                continue
            snapshot = snapshot_session(agentsview, entry, page_size, retries, runner, database)
            atomic_json(destination, snapshot)
            outcomes[session_id] = {
                "status": "complete", "path": destination.name,
                "source_revision": snapshot["extraction"]["source_revision"], "checksum": checksum(snapshot),
                "message_count": snapshot["extraction"]["message_count"], "updated_at": utc_now(),
            }
        except (OSError, ValueError, RuntimeError, subprocess.CalledProcessError) as error:
            outcomes[session_id] = {"status": "incomplete", "error": str(error), "updated_at": utc_now()}
        atomic_json(index_path, index)
    index["updated_at"] = utc_now()
    atomic_json(index_path, index)
    return index


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    manifest = commands.add_parser("manifest", help="discover and freeze a private selection manifest")
    manifest.add_argument("--output", type=Path, default=DEFAULT_MANIFEST)
    manifest.add_argument("--database", type=Path, default=DEFAULT_DB)
    manifest.add_argument("--agentsview", default="agentsview")
    manifest.add_argument("--overwrite", action="store_true")
    manifest.add_argument("--preview", action="store_true", help="print summary without writing a manifest")
    extract = commands.add_parser("extract", help="extract stable private session snapshots from a frozen manifest")
    extract.add_argument("--manifest", type=Path, default=DEFAULT_MANIFEST)
    extract.add_argument("--output-dir", type=Path, default=DEFAULT_SESSIONS)
    extract.add_argument("--database", type=Path, default=DEFAULT_DB, help="read-only source for normalized result events")
    extract.add_argument("--agentsview", default="agentsview")
    extract.add_argument("--page-size", type=int, default=500)
    extract.add_argument("--retries", type=int, default=2)
    extract.add_argument("--include-review", action="store_true")
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv or sys.argv[1:])
    if args.command == "manifest":
        if args.output.exists() and not args.overwrite and not args.preview:
            raise SystemExit(f"manifest already exists (frozen): {args.output}; pass --overwrite to refresh it")
        aliases = list_alias_sessions(args.agentsview, cli_json)
        manifest = build_manifest(aliases, identity_rows(args.database))
        if args.preview:
            print(json.dumps(manifest_summary(manifest), indent=2))
        else:
            atomic_json(args.output, manifest)
            print(json.dumps({"output": str(args.output), **manifest_summary(manifest)}, indent=2))
        return 0
    manifest = read_json(args.manifest)
    if manifest.get("schema_version") != SCHEMA_VERSION:
        raise SystemExit(f"unsupported manifest schema: {manifest.get('schema_version')}")
    if args.page_size < 1 or args.retries < 0:
        raise SystemExit("--page-size must be positive and --retries cannot be negative")
    index = extract_manifest(
        manifest, args.agentsview, args.output_dir, args.page_size, args.retries,
        args.include_review, cli_json, args.database,
    )
    incomplete = sum(value["status"] == "incomplete" for value in index["sessions"].values())
    print(json.dumps({
        "output": str(args.output_dir),
        "complete": sum(value["status"] == "complete" for value in index["sessions"].values()),
        "incomplete": incomplete,
        "skipped_unchanged": sum(value["status"] == "skipped_unchanged" for value in index["sessions"].values()),
    }, indent=2))
    return 1 if incomplete else 0


if __name__ == "__main__":
    raise SystemExit(main())
