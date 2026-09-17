#!/usr/bin/env python3
"""Normalize frozen private Agentsview snapshots into a private staging area.

This is deliberately a bridge, not a publisher.  It reads only completed
snapshot files from ``agent_archive.py extract`` and writes a provider-neutral,
auditable representation under ``target/agent-archive/normalized`` by default.
Its default commands never write into ``website/`` and it never calls Agentsview.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import sys
import tempfile
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


SCHEMA_VERSION = 1
NORMALIZATION_VERSION = 4
DEFAULT_INPUT = Path("target/agent-archive/sessions")
DEFAULT_OUTPUT = Path("target/agent-archive/normalized")
DEFAULT_PREVIEW = Path.home() / ".local" / "share" / "token-agent-archive" / "preview"
Json = dict[str, Any]
LARGE_CONTENT_BYTES = 64 * 1024


def utc_now() -> str:
    return datetime.now(timezone.utc).isoformat(timespec="seconds").replace("+00:00", "Z")


def canonical_bytes(value: Any) -> bytes:
    return (json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":")) + "\n").encode()


def checksum(value: Any) -> str:
    return hashlib.sha256(canonical_bytes(value)).hexdigest()


def atomic_json(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile("wb", dir=path.parent, prefix=f".{path.name}.", delete=False) as file:
        temporary = Path(file.name)
        file.write(canonical_bytes(value))
    os.replace(temporary, path)


def read_json(path: Path) -> Any:
    with path.open(encoding="utf-8") as file:
        return json.load(file)


def safe_text(value: Any) -> str | None:
    if value is None:
        return None
    if isinstance(value, str):
        return value
    if isinstance(value, (int, float, bool)):
        return str(value)
    if isinstance(value, list):
        parts = [safe_text(item) for item in value]
        return "\n".join(part for part in parts if part)
    if isinstance(value, dict):
        for key in ("text", "content", "value", "message", "body"):
            if key in value:
                text = safe_text(value[key])
                if text is not None:
                    return text
        return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"))
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"))


def first_present(value: Json, keys: tuple[str, ...]) -> Any:
    for key in keys:
        candidate = value.get(key)
        if candidate not in (None, "", [], {}):
            return candidate
    return None


def source_fields_without_content(value: Json, excluded: set[str]) -> Json:
    """Retain provider metadata without copying a multi-megabyte body twice.

    Bodies remain byte-for-byte available through the explicitly named content
    references. The original frozen snapshot is the lossless provider record.
    """
    return {key: child for key, child in value.items() if key not in excluded}


def content_reference(value: Any, assets: Path) -> Json | None:
    """Keep text inline unless it would make a session JSON unreasonably large."""
    text = safe_text(value)
    if text is None:
        return None
    encoded = text.encode()
    if len(encoded) <= LARGE_CONTENT_BYTES:
        return {"storage": "inline", "text": text, "bytes": len(encoded), "characters": len(text)}
    digest = hashlib.sha256(encoded).hexdigest()
    filename = f"{digest}.txt"
    destination = assets / filename
    if not destination.exists() or sha256_file(destination) != digest:
        destination.parent.mkdir(parents=True, exist_ok=True)
        with tempfile.NamedTemporaryFile("wb", dir=destination.parent, prefix=f".{filename}.", delete=False) as file:
            temporary = Path(file.name)
            file.write(encoded)
        os.replace(temporary, destination)
    return {
        "storage": "asset", "path": f"assets/{filename}", "href": f"/agent-assets/{filename}", "sha256": digest,
        "bytes": len(encoded), "characters": len(text), "mediaType": "text/plain; charset=utf-8",
    }


def inline_text(reference: Json | None) -> str | None:
    return reference.get("text") if reference and reference.get("storage") == "inline" else None


def valid_timestamp(value: Any) -> str | None:
    text = safe_text(value)
    if not text:
        return None
    try:
        datetime.fromisoformat(text.replace("Z", "+00:00"))
    except ValueError:
        return None
    return text


def role_of(message: Json) -> str:
    raw = str(first_present(message, ("role", "message_role", "author_role", "type")) or "unknown").lower()
    if raw in {"assistant", "agent", "model"}:
        return "assistant"
    if raw in {"user", "human", "customer", "prompt"}:
        return "user"
    if raw in {"system", "developer", "instruction"}:
        return "system"
    if raw in {"tool", "function", "tool_result", "function_result"}:
        return "tool"
    return "unknown"


def is_human_turn(message: Json, role: str) -> bool:
    """Be deliberately conservative about providers which encode system as user."""
    if role != "user":
        return False
    flags = ("is_system", "system", "is_sidechain", "sidechain", "is_tool_result", "tool_derived")
    if any(message.get(flag) is True for flag in flags):
        return False
    provenance = " ".join(str(message.get(key, "")) for key in (
        "source", "source_type", "event_type", "message_type", "kind", "channel",
    )).lower()
    return not any(marker in provenance for marker in ("system", "sidechain", "tool", "function", "compaction"))


def presentation_role(message: Json, source_role: str, human: bool) -> str:
    """Keep user-role system/tool records out of the viewer's turn counter."""
    if source_role != "user" or human:
        return source_role
    provenance = " ".join(str(message.get(key, "")) for key in (
        "source", "source_type", "event_type", "message_type", "kind", "channel",
    )).lower()
    if message.get("is_tool_result") is True or message.get("tool_derived") is True or any(marker in provenance for marker in ("tool", "function")):
        return "tool"
    return "system" if any(message.get(flag) is True for flag in ("is_system", "system", "is_sidechain", "sidechain")) or any(marker in provenance for marker in ("system", "sidechain", "compaction")) else "unknown"


def message_text(message: Json) -> Any:
    return first_present(message, ("text", "content", "message", "body", "content_text"))


def normalize_input(value: Any, assets: Path) -> Json:
    text = safe_text(value)
    if text in (None, ""):
        return {"format": "empty", "content": None}
    try:
        parsed = json.loads(text)
    except (TypeError, json.JSONDecodeError):
        return {"format": "opaque", "content": content_reference(text, assets)}
    return {"format": "json", "value": parsed, "content": content_reference(text, assets)}


def calls_of(message: Json) -> list[Json]:
    calls = first_present(message, ("tool_calls", "tools", "tool_uses", "function_calls", "calls"))
    if isinstance(calls, list):
        return [call for call in calls if isinstance(call, dict)]
    if isinstance(calls, dict):
        return [calls]
    return []


def call_name(call: Json) -> str:
    value = first_present(call, ("name", "tool_name", "function_name", "tool"))
    if isinstance(value, dict):
        value = first_present(value, ("name", "id"))
    return safe_text(value) or "unknown"


def result_from_call(call: Json) -> tuple[Any, str | None, str | None]:
    value = first_present(call, ("result", "output", "result_content", "response", "content"))
    status = safe_text(first_present(call, ("status", "result_status", "state")))
    identifier = safe_text(first_present(call, ("tool_use_id", "tool_call_id", "id", "call_id")))
    return value, status, identifier


def normalized_tool_status(value: Any) -> str:
    """Map provider-specific result states to the viewer's three states.

    Agentsview currently preserves values such as ``errored`` from result events;
    the session browser deliberately exposes the stable presentation vocabulary
    ``completed``, ``failed``, or ``unknown``. The original value remains in the
    private normalized tool-call source metadata.
    """
    text = (safe_text(value) or "").strip().lower()
    if not text:
        return "unknown"
    if any(marker in text for marker in ("fail", "error", "cancel", "abort", "denied", "timeout", "interrupt")):
        return "failed"
    if text in {"completed", "complete", "success", "succeeded", "pass", "passed", "ok", "done", "finished"}:
        return "completed"
    return "unknown"


def explicit_tool_status(call: Json, event: Json | None, source_status: Any) -> str:
    """Use only explicit provider flags or status values for tool outcome.

    A result body that happens to contain words such as "error" is not outcome
    evidence. This keeps unknown outcome states honest while supporting provider
    adapters that expose booleans instead of a status enum.
    """
    values = [call, event] if event is not None else [call]
    failure_flags = {
        "is_error", "isError", "error", "failed", "is_failed", "isFailed",
        "cancelled", "is_cancelled", "isCancelled", "aborted", "timed_out", "timeout",
    }
    success_flags = {"success", "is_success", "isSuccess", "completed", "is_completed", "isCompleted", "ok"}

    def is_true(value: Any) -> bool:
        return value is True or (isinstance(value, str) and value.strip().lower() in {"true", "1", "yes"})

    if any(is_true(item.get(flag)) for item in values if isinstance(item, dict) for flag in failure_flags):
        return "failed"
    if any(is_true(item.get(flag)) for item in values if isinstance(item, dict) for flag in success_flags):
        return "completed"
    return normalized_tool_status(source_status)


def event_key(event: Json) -> tuple[int | None, int | None, str | None]:
    ordinal = first_present(event, ("tool_call_message_ordinal", "message_ordinal", "ordinal"))
    index = first_present(event, ("call_index", "tool_call_index", "index"))
    return (
        ordinal if isinstance(ordinal, int) else None,
        index if isinstance(index, int) else None,
        safe_text(first_present(event, ("tool_use_id", "tool_call_id", "id"))),
    )


def call_key(ordinal: int | None, index: int, call: Json) -> tuple[int | None, int | None, str | None]:
    return ordinal, index, safe_text(first_present(call, ("tool_use_id", "tool_call_id", "id", "call_id")))


def matching_events(event_map: dict[tuple[int | None, int | None, str | None], list[Json]], ordinal: int | None, index: int, call: Json) -> list[Json]:
    """Prefer an exact tool-use id, then use Agentsview's ordinal/index join."""
    exact = event_map.get(call_key(ordinal, index, call), [])
    if exact:
        return exact
    candidates: list[Json] = []
    for (event_ordinal, event_index, _), values in event_map.items():
        if event_ordinal == ordinal and event_index == index:
            candidates.extend(values)
    return candidates


def file_paths(call: Json) -> list[str]:
    found: set[str] = set()

    def visit(value: Any, key: str = "") -> None:
        if isinstance(value, dict):
            for child_key, child in value.items():
                visit(child, str(child_key))
        elif isinstance(value, list):
            for child in value:
                visit(child, key)
        elif isinstance(value, str):
            key_lower = key.lower()
            if key_lower in {"file_path", "filepath", "path", "file"} and value and "\n" not in value and len(value) < 4096:
                found.add(value)
            # Agentsview's canonical tool shape stores provider arguments in
            # `input_json`. Parse only JSON-shaped strings, retain the source
            # untouched, and let the normal recursive key walk find paths.
            if key_lower in {"input_json", "input", "arguments", "params"} and value.lstrip().startswith(("{", "[")):
                try:
                    decoded = json.loads(value)
                except json.JSONDecodeError:
                    return
                visit(decoded, f"{key}<json>")

    visit(call)
    return sorted(found)


def normalized_message_id(message: Json, position: int) -> tuple[str, int | None]:
    ordinal = message.get("ordinal")
    if isinstance(ordinal, int):
        return f"ordinal:{ordinal}", ordinal
    source_id = safe_text(first_present(message, ("id", "message_id", "uuid")))
    return (f"source:{source_id}" if source_id else f"position:{position}"), None


def normalize_snapshot(snapshot: Json, snapshot_checksum: str, manifest_checksum: str, assets: Path) -> tuple[Json, Json]:
    session = snapshot.get("session") if isinstance(snapshot.get("session"), dict) else {}
    manifest_entry = snapshot.get("manifest_entry") if isinstance(snapshot.get("manifest_entry"), dict) else {}
    session_id = safe_text(first_present(session, ("id", "session_id"))) or safe_text(manifest_entry.get("id"))
    if not session_id:
        raise ValueError("snapshot does not contain a session id")
    provider = safe_text(first_present(session, ("agent", "provider"))) or safe_text(manifest_entry.get("provider")) or "unknown"
    raw_messages = snapshot.get("messages")
    if not isinstance(raw_messages, list):
        raise ValueError(f"snapshot {session_id} does not contain a messages array")
    events = snapshot.get("tool_result_events") if isinstance(snapshot.get("tool_result_events"), list) else []
    event_map: dict[tuple[int | None, int | None, str | None], list[Json]] = {}
    for event in events:
        if isinstance(event, dict):
            event_map.setdefault(event_key(event), []).append(event)

    normalized_messages: list[Json] = []
    tool_calls: list[Json] = []
    tool_histogram: dict[str, int] = {}
    touched: set[str] = set()
    missing_timestamps = 0
    unknown_shapes = 0
    human_turns = 0
    matched_event_ids: set[int] = set()
    first_human_text: str | None = None

    for position, raw in enumerate(raw_messages):
        if not isinstance(raw, dict):
            normalized_messages.append({"id": f"position:{position}", "ordinal": None, "role": "unknown", "kind": "unknown_event", "source": {"shape": type(raw).__name__}})
            unknown_shapes += 1
            continue
        message_id, ordinal = normalized_message_id(raw, position)
        source_role = role_of(raw)
        human = is_human_turn(raw, source_role)
        role = presentation_role(raw, source_role, human)
        kind = "human_turn" if human else ("agent_message" if role == "assistant" else ("system" if role == "system" else ("tool_event" if role == "tool" else "unknown_event")))
        source_timestamp = safe_text(first_present(raw, ("timestamp", "created_at", "createdAt", "time")))
        timestamp = valid_timestamp(source_timestamp)
        if not timestamp:
            missing_timestamps += 1
        text_value = message_text(raw)
        text = content_reference(text_value, assets)
        thinking = content_reference(first_present(raw, ("thinking", "reasoning", "analysis")), assets)
        if human:
            human_turns += 1
            if first_human_text is None:
                first_human_text = safe_text(text_value)
        model = safe_text(first_present(raw, ("model", "model_name", "agent_model")))
        source_fields = source_fields_without_content(raw, {"text", "content", "message", "body", "content_text", "thinking", "reasoning", "analysis", "tool_calls", "tools", "tool_uses", "function_calls", "calls"})
        normalized_message: Json = {
            "id": message_id, "ordinal": ordinal, "role": role, "kind": kind,
            "timestamp": timestamp, "provider": safe_text(first_present(raw, ("agent", "provider"))) or provider,
            "model": model,
            # `text` and `tools` retain the fixture contract. The corresponding
            # references retain full content when it is too large for inline HTML.
            "text": inline_text(text), "textContent": text,
            "thinking": inline_text(thinking), "thinkingContent": thinking,
            "tools": [],
            "source": {"sourceMessageId": safe_text(first_present(raw, ("id", "message_id", "uuid"))), "sourceRole": source_role, "timestamp": source_timestamp, "fields": source_fields},
        }
        normalized_messages.append(normalized_message)
        for call_index, call in enumerate(calls_of(raw)):
            name = call_name(call)
            tool_histogram[name] = tool_histogram.get(name, 0) + 1
            paths = file_paths(call)
            touched.update(paths)
            embedded, embedded_status, call_identifier = result_from_call(call)
            matched = matching_events(event_map, ordinal, call_index, call)
            event = matched[0] if matched else None
            if event is not None:
                matched_event_ids.add(id(event))
            canonical_value = embedded if embedded not in (None, "", [], {}) else (event.get("content") if event else None)
            canonical_source = "call_embedded" if embedded not in (None, "", [], {}) else ("result_event" if event else None)
            source_status = embedded_status or (safe_text(event.get("status")) if event else None)
            alternates = []
            if embedded not in (None, "", [], {}) and event is not None:
                alternates.append({"source": "result_event", "status": safe_text(event.get("status")), "content": content_reference(event.get("content"), assets), "eventIndex": event.get("event_index")})
            result_content = content_reference(canonical_value, assets)
            normalized_call: Json = {
                "id": f"{message_id}:call:{call_index}", "messageId": message_id, "messageOrdinal": ordinal,
                "callIndex": call_index, "sourceCallId": call_identifier, "name": name,
                "label": safe_text(first_present(call, ("label", "title", "description"))) or name,
                "status": explicit_tool_status(call, event, source_status), "input": normalize_input(first_present(call, ("input_json", "input", "arguments", "params")), assets),
                "result": {"source": canonical_source, "content": result_content} if canonical_source else None,
                "alternateResults": alternates,
                "subagentSessionId": safe_text(first_present(call, ("subagent_session_id", "child_session_id"))),
                "filePaths": paths,
                "source": {
                    "sourceStatus": source_status,
                    "fields": source_fields_without_content(call, {"input_json", "input", "arguments", "params", "result", "output", "result_content", "response", "content"}),
                },
            }
            tool_calls.append(normalized_call)
            normalized_message["tools"].append({
                "id": normalized_call["id"], "name": name, "label": normalized_call["label"],
                "status": normalized_call["status"],
                "input": inline_text(normalized_call["input"]["content"]), "inputContent": normalized_call["input"]["content"],
                "output": inline_text(result_content), "outputContent": result_content,
                "subagentSessionId": normalized_call["subagentSessionId"],
            })

    unmatched_events = []
    for event in events:
        if not isinstance(event, dict) or id(event) in matched_event_ids:
            continue
        unmatched_events.append({
            "kind": "unmatched_tool_result_event", "key": {"messageOrdinal": event_key(event)[0], "callIndex": event_key(event)[1], "toolUseId": event_key(event)[2]},
            "status": safe_text(event.get("status")), "timestamp": safe_text(event.get("timestamp")),
            "content": content_reference(event.get("content"), assets), "source": {"fields": source_fields_without_content(event, {"content", "result", "output", "result_content"})},
        })
    session_name = safe_text(first_present(session, ("session_name", "title", "name", "display_name")))
    title = session_name or compact_title(first_human_text) or session_id
    parent = safe_text(first_present(session, ("parent_session_id", "parser_parent_session_id"))) or safe_text(manifest_entry.get("parent_session_id"))
    attachment_refs = snapshot.get("extraction", {}).get("attachment_references", []) if isinstance(snapshot.get("extraction"), dict) else []
    attachment_refs = attachment_refs if isinstance(attachment_refs, list) else []
    normalized: Json = {
        "schemaVersion": SCHEMA_VERSION, "normalizationVersion": NORMALIZATION_VERSION,
        "purpose": "private normalized Agentsview staging data; not approved website data",
        "id": session_id, "agent": provider, "title": title, "titleSource": "session_name" if session_name else ("first_human_turn" if first_human_text else "session_id"),
        "startedAt": valid_timestamp(first_present(session, ("started_at", "startedAt", "created_at"))),
        "endedAt": valid_timestamp(first_present(session, ("ended_at", "endedAt"))),
        "branch": safe_text(first_present(session, ("git_branch", "branch"))),
        "parentSessionId": parent,
        "files": [{"path": path, "kind": "touched", "evidenceMessageIds": [call["messageId"] for call in tool_calls if path in call["filePaths"]]} for path in sorted(touched)],
        "messages": normalized_messages, "toolCalls": tool_calls, "events": unmatched_events,
        "attachments": {"status": snapshot.get("extraction", {}).get("completeness", {}).get("attachments", "unknown"), "references": attachment_refs},
        "source": {
            "snapshotChecksum": snapshot_checksum, "manifestChecksum": manifest_checksum,
            "transcriptRevision": str(snapshot.get("extraction", {}).get("source_revision", "")), "provider": provider,
            "manifestEntry": manifest_entry,
            "sessionTimestamps": {"startedAt": safe_text(first_present(session, ("started_at", "startedAt", "created_at"))), "endedAt": safe_text(first_present(session, ("ended_at", "endedAt")))},
        },
        "diagnostics": {"unknownMessageShapes": unknown_shapes, "missingTimestamps": missing_timestamps, "unmatchedResultEvents": len(unmatched_events)},
    }
    summary = {
        "id": session_id, "agent": provider, "title": title, "titleSource": normalized["titleSource"],
        "startedAt": normalized["startedAt"], "endedAt": normalized["endedAt"], "branch": normalized["branch"],
        "parentSessionId": parent, "humanTurns": human_turns, "messageCount": len(normalized_messages),
        "toolCallCount": len(tool_calls), "toolNames": dict(sorted(tool_histogram.items())),
        "fileTouchCount": len(touched), "attachmentCount": len(attachment_refs), "hasUnmatchedEvents": bool(unmatched_events),
    }
    return normalized, summary


def compact_title(value: str | None) -> str | None:
    if not value:
        return None
    one_line = " ".join(value.split())
    return one_line[:157].rstrip() + ("…" if len(one_line) > 157 else "")


def normalize_directory(input_dir: Path, output_dir: Path, *, force: bool = False) -> Json:
    source_index_path = input_dir / "index.json"
    source_index = read_json(source_index_path)
    if source_index.get("schema_version") != SCHEMA_VERSION:
        raise ValueError(f"unsupported extraction index schema: {source_index.get('schema_version')}")
    manifest_checksum = source_index.get("manifest_checksum")
    if not isinstance(manifest_checksum, str) or not manifest_checksum:
        raise ValueError("extraction index does not have a manifest checksum")
    output_dir.mkdir(parents=True, exist_ok=True)
    output_index_path = output_dir / "index.json"
    output_index: Json = read_json(output_index_path) if output_index_path.exists() else {}
    if (
        output_index.get("schemaVersion") != SCHEMA_VERSION
        or output_index.get("normalizationVersion") != NORMALIZATION_VERSION
        or output_index.get("manifestChecksum") != manifest_checksum
    ):
        output_index = {
            "schemaVersion": SCHEMA_VERSION,
            "normalizationVersion": NORMALIZATION_VERSION,
            "purpose": "private normalized session index; not website data",
            "manifestChecksum": manifest_checksum,
            "sessions": {},
        }
    source_records = source_index.get("sessions")
    if not isinstance(source_records, dict):
        raise ValueError("extraction index does not have a sessions object")
    previous_outcomes = output_index.get("sessions") if isinstance(output_index.get("sessions"), dict) else {}
    outcomes: dict[str, Json] = {}
    for session_id, record in source_records.items():
        if not isinstance(session_id, str) or not isinstance(record, dict):
            continue
        if record.get("status") not in {"complete", "skipped_unchanged"}:
            outcomes[session_id] = {"status": "blocked", "reason": f"source extraction is {record.get('status', 'invalid')}", "updatedAt": utc_now()}
            continue
        path = record.get("path")
        if not isinstance(path, str) or Path(path).name != path:
            outcomes[session_id] = {"status": "rejected", "error": "invalid extraction index path", "updatedAt": utc_now()}
            continue
        source_path = input_dir / path
        try:
            snapshot = read_json(source_path)
            actual_checksum = checksum(snapshot)
            if actual_checksum != record.get("checksum"):
                raise ValueError("snapshot checksum differs from extraction index")
            snapshot_id = safe_text(first_present(snapshot.get("session", {}) if isinstance(snapshot.get("session"), dict) else {}, ("id", "session_id")))
            if snapshot_id != session_id:
                raise ValueError("snapshot session id differs from extraction index")
            filename = f"{hashlib.sha256(session_id.encode()).hexdigest()}.json"
            destination = output_dir / "sessions" / filename
            previous = previous_outcomes.get(session_id, {})
            if not force and isinstance(previous, dict) and normalized_record_is_intact(output_dir, previous, actual_checksum):
                outcomes[session_id] = {**previous, "status": "skipped_unchanged", "updatedAt": utc_now()}
                continue
            normalized, summary = normalize_snapshot(snapshot, actual_checksum, manifest_checksum, output_dir / "assets")
            atomic_json(destination, normalized)
            outcomes[session_id] = {
                "status": "complete", "path": f"sessions/{filename}", "sourceSnapshotChecksum": actual_checksum,
                "normalizedChecksum": checksum(normalized), "normalizationVersion": NORMALIZATION_VERSION,
                "transcriptRevision": normalized["source"]["transcriptRevision"],
                "summary": summary, "updatedAt": utc_now(),
            }
        except (OSError, ValueError, json.JSONDecodeError) as error:
            outcomes[session_id] = {"status": "rejected", "error": str(error), "updatedAt": utc_now()}
    output_index["sessions"] = outcomes
    output_index["updatedAt"] = utc_now()
    atomic_json(output_index_path, output_index)
    return output_index


def promote_reviewed(normalized_dir: Path, allowlist_path: Path, destination: Path) -> Json:
    """Copy exact reviewed records to another *private* directory.

    This intentionally has no website default and keeps the allowlist checksum-bound.
    A later sanitizing/publication workflow may consume this output after review.
    """
    index = read_json(normalized_dir / "index.json")
    allowlist = read_json(allowlist_path)
    if allowlist.get("schemaVersion") != SCHEMA_VERSION or not isinstance(allowlist.get("sessions"), list):
        raise ValueError("allowlist must have schemaVersion 1 and a sessions array")
    approved = {item.get("id"): item for item in allowlist["sessions"] if isinstance(item, dict) and item.get("status") == "approved"}
    if len(approved) != sum(isinstance(item, dict) and item.get("status") == "approved" for item in allowlist["sessions"]):
        raise ValueError("allowlist contains duplicate approved session ids")
    written: list[str] = []
    destination.mkdir(parents=True, exist_ok=True)
    for session_id, approval in approved.items():
        record = index.get("sessions", {}).get(session_id)
        if not isinstance(record, dict) or record.get("status") not in {"complete", "skipped_unchanged"}:
            raise ValueError(f"approved session is not normalized and complete: {session_id}")
        if approval.get("normalizedChecksum") != record.get("normalizedChecksum"):
            raise ValueError(f"allowlist checksum does not match normalized session: {session_id}")
        relative_session = safe_relative_path(record.get("path"), "sessions")
        source = normalized_dir / relative_session
        session = read_json(source)
        if checksum(session) != record["normalizedChecksum"]:
            raise ValueError(f"normalized session checksum differs from its index: {session_id}")
        asset_references = find_asset_references(session)
        if not all(verify_asset_reference(normalized_dir, reference) for reference in asset_references):
            raise ValueError(f"normalized session has a missing or invalid asset: {session_id}")
        target = destination / relative_session
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, target)
        for reference in asset_references:
            relative_asset = safe_relative_path(reference.get("path"), "assets")
            asset_source = normalized_dir / relative_asset
            asset_target = destination / relative_asset
            asset_target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(asset_source, asset_target)
        written.append(session_id)
    receipt = {"schemaVersion": SCHEMA_VERSION, "purpose": "private reviewed transfer receipt; not public publication", "allowlistChecksum": checksum(allowlist), "sessions": sorted(written), "createdAt": utc_now()}
    atomic_json(destination / "receipt.json", receipt)
    return receipt


def require_public_approval(item: Json, record: Json) -> None:
    if item.get("status") != "approved_public":
        raise ValueError(f"session is not explicitly approved for public projection: {item.get('id')}")
    if item.get("normalizedChecksum") != record.get("normalizedChecksum"):
        raise ValueError(f"allowlist checksum does not match normalized session: {item.get('id')}")
    review = item.get("review")
    if not isinstance(review, dict) or any(review.get(field) is not True for field in ("publicTextReviewed", "publicAssetsReviewed", "thinkingReviewed")):
        raise ValueError(f"public approval must explicitly review text, assets, and thinking: {item.get('id')}")


def public_content(reference: Any, source_root: Path, destination: Path) -> tuple[str | None, Json | None]:
    if not isinstance(reference, dict):
        return None, None
    if reference.get("storage") == "inline":
        return reference.get("text") if isinstance(reference.get("text"), str) else None, None
    if reference.get("storage") != "asset" or not verify_asset_reference(source_root, reference):
        raise ValueError("presentation references a missing or invalid asset")
    source_relative = safe_relative_path(reference.get("path"), "assets")
    filename = source_relative.name
    target_relative = Path("agent-assets") / filename
    target = destination / target_relative
    target.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(source_root / source_relative, target)
    return None, {
        "storage": "asset", "path": str(target_relative), "href": f"/agent-assets/{filename}",
        "sha256": reference["sha256"], "bytes": reference["bytes"], "characters": reference.get("characters"),
        "mediaType": reference.get("mediaType", "text/plain; charset=utf-8"),
    }


def public_session_projection(session: Json, normalized_dir: Path, destination: Path) -> Json:
    messages: list[Json] = []
    dedicated_thinking_messages = 0
    for source_message in session.get("messages", []):
        if not isinstance(source_message, dict):
            continue
        if source_message.get("thinkingContent") is not None:
            dedicated_thinking_messages += 1
        text, text_content = public_content(source_message.get("textContent"), normalized_dir, destination)
        projected_tools = []
        for source_tool in source_message.get("tools", []):
            if not isinstance(source_tool, dict):
                continue
            input_text, input_content = public_content(source_tool.get("inputContent"), normalized_dir, destination)
            output_text, output_content = public_content(source_tool.get("outputContent"), normalized_dir, destination)
            projected_tools.append({
                "id": source_tool.get("id"), "name": source_tool.get("name"), "label": source_tool.get("label"),
                "status": source_tool.get("status"), "input": input_text, "inputContent": input_content,
                "output": output_text, "outputContent": output_content, "subagentSessionId": source_tool.get("subagentSessionId"),
            })
        messages.append({
            "id": source_message.get("id"), "ordinal": source_message.get("ordinal"), "role": source_message.get("role"),
            "kind": source_message.get("kind"), "timestamp": source_message.get("timestamp"),
            "agent": source_message.get("provider"), "model": source_message.get("model"),
            "text": text, "textContent": text_content, "tools": projected_tools,
        })
    return {
        "schemaVersion": SCHEMA_VERSION, "id": session["id"], "agent": session.get("agent", "unknown"),
        "title": session.get("title"), "titleSource": session.get("titleSource"), "startedAt": session.get("startedAt"),
        "endedAt": session.get("endedAt"), "branch": session.get("branch"), "parentSessionId": session.get("parentSessionId"),
        "files": session.get("files", []), "messages": messages,
        "publication": {
            "sourceNormalizedChecksum": checksum(session), "reviewRequiredForChanges": True,
            "omitted": {
                "unmatchedResultEvents": len(session.get("events", [])) if isinstance(session.get("events"), list) else 0,
                "attachmentReferences": len(session.get("attachments", {}).get("references", [])) if isinstance(session.get("attachments"), dict) and isinstance(session.get("attachments", {}).get("references", []), list) else 0,
                "dedicatedThinkingMessages": dedicated_thinking_messages,
            },
        },
    }


def export_reviewed(normalized_dir: Path, allowlist_path: Path, destination: Path) -> Json:
    """Create a presentation-only, checksum-approved staging payload.

    The destination defaults to ignored private staging. This is never invoked
    automatically and does not claim that the reviewer redacted prose.
    """
    index = read_json(normalized_dir / "index.json")
    allowlist = read_json(allowlist_path)
    if allowlist.get("schemaVersion") != SCHEMA_VERSION or not isinstance(allowlist.get("sessions"), list):
        raise ValueError("allowlist must have schemaVersion 1 and a sessions array")
    approvals = [item for item in allowlist["sessions"] if isinstance(item, dict) and item.get("status") == "approved_public"]
    ids = [item.get("id") for item in approvals]
    if any(not isinstance(value, str) or not value for value in ids) or len(ids) != len(set(ids)):
        raise ValueError("public allowlist contains invalid or duplicate approved session ids")
    destination.mkdir(parents=True, exist_ok=True)
    summary_records = []
    for approval in approvals:
        session_id = approval["id"]
        record = index.get("sessions", {}).get(session_id)
        if not isinstance(record, dict) or record.get("status") not in {"complete", "skipped_unchanged"}:
            raise ValueError(f"approved session is not normalized and complete: {session_id}")
        require_public_approval(approval, record)
        relative = safe_relative_path(record.get("path"), "sessions")
        source = read_json(normalized_dir / relative)
        if source.get("id") != session_id or checksum(source) != record.get("normalizedChecksum"):
            raise ValueError(f"normalized source identity or checksum differs from its index: {session_id}")
        validate_public_references(source)
        projected = public_session_projection(source, normalized_dir, destination)
        filename = f"{hashlib.sha256(session_id.encode()).hexdigest()}.json"
        atomic_json(destination / "sessions" / filename, projected)
        summary_records.append({"id": session_id, "path": f"sessions/{filename}", "summary": record.get("summary", {}), "sourceNormalizedChecksum": record["normalizedChecksum"]})
    output = {"schemaVersion": SCHEMA_VERSION, "purpose": "reviewed presentation staging; manual website copy required", "allowlistChecksum": checksum(allowlist), "sessions": sorted(summary_records, key=lambda item: item["id"]), "generatedAt": utc_now()}
    atomic_json(destination / "index.json", output)
    return output


def export_preview(normalized_dir: Path, destination: Path) -> Json:
    """Create a private, unreviewed presentation projection for local development.

    This verifies the same normalized checksums, route identifiers, and referenced
    assets as the reviewed exporter, but deliberately has no allowlist and makes
    no public-review assertion. ``destination`` is intended for ignored
    ``target/`` output only; callers that want website data must use the explicit
    reviewed export workflow instead.
    """
    index = read_json(normalized_dir / "index.json")
    records = index.get("sessions")
    if not isinstance(records, dict):
        raise ValueError("normalized index does not have a sessions object")
    destination.mkdir(parents=True, exist_ok=True)
    preview_records: list[Json] = []
    for session_id in sorted(records):
        record = records[session_id]
        if not isinstance(record, dict) or record.get("status") not in {"complete", "skipped_unchanged"}:
            continue
        relative = safe_relative_path(record.get("path"), "sessions")
        source = read_json(normalized_dir / relative)
        if source.get("id") != session_id or checksum(source) != record.get("normalizedChecksum"):
            raise ValueError(f"normalized source identity or checksum differs from its index: {session_id}")
        validate_public_references(source)
        projected = public_session_projection(source, normalized_dir, destination)
        filename = f"{hashlib.sha256(session_id.encode()).hexdigest()}.json"
        projected_path = destination / "sessions" / filename
        atomic_json(projected_path, projected)
        preview_records.append({
            "id": session_id,
            "path": f"sessions/{filename}",
            "checksum": checksum(projected),
        })
    receipt = {
        "schemaVersion": SCHEMA_VERSION,
        "purpose": "private-preview",
        "normalizedIndexChecksum": checksum(index),
        "sessions": preview_records,
        "generatedAt": utc_now(),
    }
    atomic_json(destination / "index.json", receipt)
    atomic_json(destination / "receipt.json", receipt)
    return receipt


def collect_asset_references(value: Any) -> set[str]:
    results: set[str] = set()
    if isinstance(value, dict):
        if value.get("storage") == "asset" and isinstance(value.get("path"), str):
            results.add(str(safe_relative_path(value["path"], "assets")))
        for child in value.values():
            results.update(collect_asset_references(child))
    elif isinstance(value, list):
        for child in value:
            results.update(collect_asset_references(child))
    return results


def safe_relative_path(value: Any, prefix: str) -> Path:
    if not isinstance(value, str):
        raise ValueError("path is not a string")
    path = Path(value)
    if path.is_absolute() or ".." in path.parts or not path.parts or path.parts[0] != prefix:
        raise ValueError(f"unsafe {prefix} path: {value}")
    return path


def safe_session_id(value: Any) -> bool:
    if not isinstance(value, str) or not value or value in {".", ".."}:
        return False
    return not any(character in value for character in ("/", "\\", "?", "#")) and not any(ord(character) < 32 or ord(character) == 127 for character in value)


def validate_public_references(session: Json) -> None:
    if not safe_session_id(session.get("id")):
        raise ValueError("session id is not safe for a public route")
    parent = session.get("parentSessionId")
    if parent is not None and not safe_session_id(parent):
        raise ValueError("parent session id is not safe for a public route")
    for message in session.get("messages", []):
        if not isinstance(message, dict):
            continue
        for tool in message.get("tools", []):
            if not isinstance(tool, dict):
                continue
            child = tool.get("subagentSessionId")
            if child is not None and not safe_session_id(child):
                raise ValueError("subagent session id is not safe for a public route")


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as file:
        for chunk in iter(lambda: file.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def verify_asset_reference(root: Path, reference: Json) -> bool:
    if reference.get("storage") != "asset":
        return True
    try:
        relative = safe_relative_path(reference.get("path"), "assets")
    except ValueError:
        return False
    path = root / relative
    if not path.is_file() or not isinstance(reference.get("sha256"), str):
        return False
    if sha256_file(path) != reference["sha256"]:
        return False
    return not isinstance(reference.get("bytes"), int) or path.stat().st_size == reference["bytes"]


def normalized_record_is_intact(root: Path, record: Json, expected_source_checksum: str) -> bool:
    if record.get("status") not in {"complete", "skipped_unchanged"}:
        return False
    if record.get("sourceSnapshotChecksum") != expected_source_checksum:
        return False
    if record.get("normalizationVersion") != NORMALIZATION_VERSION:
        return False
    try:
        relative = safe_relative_path(record.get("path"), "sessions")
        session = read_json(root / relative)
    except (OSError, ValueError, json.JSONDecodeError):
        return False
    if checksum(session) != record.get("normalizedChecksum"):
        return False
    if session.get("normalizationVersion") != NORMALIZATION_VERSION:
        return False
    return all(verify_asset_reference(root, reference) for reference in find_asset_references(session))


def find_asset_references(value: Any) -> list[Json]:
    references: list[Json] = []
    if isinstance(value, dict):
        if value.get("storage") == "asset":
            references.append(value)
        for child in value.values():
            references.extend(find_asset_references(child))
    elif isinstance(value, list):
        for child in value:
            references.extend(find_asset_references(child))
    return references


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    normalize = commands.add_parser("normalize", help="convert completed private snapshots to a private normalized staging area")
    normalize.add_argument("--input-dir", type=Path, default=DEFAULT_INPUT)
    normalize.add_argument("--output-dir", type=Path, default=DEFAULT_OUTPUT)
    normalize.add_argument("--force", action="store_true")
    promote = commands.add_parser("promote", help="copy only checksum-bound approved sessions to an explicit private destination")
    promote.add_argument("--normalized-dir", type=Path, default=DEFAULT_OUTPUT)
    promote.add_argument("--allowlist", required=True, type=Path)
    promote.add_argument("--destination", required=True, type=Path)
    export = commands.add_parser("export-reviewed", help="create checksum-approved presentation staging; defaults outside website")
    export.add_argument("--normalized-dir", type=Path, default=DEFAULT_OUTPUT)
    export.add_argument("--allowlist", required=True, type=Path)
    export.add_argument("--destination", type=Path, default=DEFAULT_OUTPUT / "website-ready")
    preview = commands.add_parser("export-preview", help="create an unreviewed private presentation projection for local development")
    preview.add_argument("--normalized-dir", type=Path, default=DEFAULT_OUTPUT)
    preview.add_argument("--destination", type=Path, default=DEFAULT_PREVIEW)
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv or sys.argv[1:])
    if args.command == "normalize":
        result = normalize_directory(args.input_dir, args.output_dir, force=args.force)
        statuses = [record.get("status") for record in result["sessions"].values()]
        print(json.dumps({"output": str(args.output_dir), "complete": statuses.count("complete"), "skippedUnchanged": statuses.count("skipped_unchanged"), "rejected": statuses.count("rejected")}, indent=2))
        return 1 if "rejected" in statuses or "blocked" in statuses else 0
    if args.command == "promote":
        result = promote_reviewed(args.normalized_dir, args.allowlist, args.destination)
        print(json.dumps({"output": str(args.destination), "approved": len(result["sessions"])}, indent=2))
        return 0
    if args.command == "export-preview":
        result = export_preview(args.normalized_dir, args.destination)
        print(json.dumps({"output": str(args.destination), "previewed": len(result["sessions"])}, indent=2))
        return 0
    result = export_reviewed(args.normalized_dir, args.allowlist, args.destination)
    print(json.dumps({"output": str(args.destination), "approved": len(result["sessions"])}, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
