#!/usr/bin/env python3
"""Import the repository's public Amp Markdown archive into viewer records.

The Markdown snapshots are already checked-in public repository content.  This
importer is intentionally separate from the private Agentsview pipeline: it has
no network or database dependency and only reads ``docs/ampcode-threads``.

Amp exports are Markdown rather than a documented machine format.  The parser
therefore recognizes only the stable, visible conventions in this repository
(front matter keys, speaker headings, and tool markers) and preserves anything
it cannot classify as ordinary transcript text.  It does *not* use a YAML
parser or infer timestamps, edits, branches, annotations, or relationships.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
from datetime import datetime
from pathlib import Path
from typing import Any, Iterable


ASSET_THRESHOLD = 96 * 1024
THREAD_ID = re.compile(r"^T-[A-Za-z0-9-]+$")
SPEAKER_HEADING = re.compile(r"^## (User|Assistant)\s*$")
TOOL_USE = re.compile(r"^\*\*Tool Use:\*\*\s+`([^`]+)`\s*$", re.MULTILINE)
TOOL_EVENT = re.compile(r"^\*\*Tool (Result|Error):\*\*\s+`([^`]+)`\s*$", re.MULTILINE)
FENCE = re.compile(r"^\s*```")
EXPLICIT_PATH_KEYS = {"file_path", "filepath", "path", "file"}

Json = dict[str, Any]


def canonical_json(value: Any) -> str:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":")) + "\n"


def write_json(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(canonical_json(value), encoding="utf-8")


def nonempty(value: str) -> str | None:
    value = value.strip()
    return value or None


def parse_front_matter(text: str) -> tuple[dict[str, str], str]:
    """Read the simple visible ``key: value`` header without YAML semantics."""
    lines = text.splitlines(keepends=True)
    if not lines or lines[0].strip() != "---":
        raise ValueError("missing opening front matter delimiter")
    fields: dict[str, str] = {}
    for index in range(1, len(lines)):
        if lines[index].strip() == "---":
            return fields, "".join(lines[index + 1:])
        key, separator, value = lines[index].partition(":")
        if separator and re.fullmatch(r"[A-Za-z][A-Za-z0-9_]*", key.strip()):
            fields[key.strip()] = value.strip().strip('"')
    raise ValueError("missing closing front matter delimiter")


def sections(body: str) -> list[tuple[str, str]]:
    """Split visible speaker headings while ignoring headings inside code fences."""
    found: list[tuple[str, int, int]] = []
    in_fence = False
    offset = 0
    for line in body.splitlines(keepends=True):
        if not in_fence:
            match = SPEAKER_HEADING.match(line.rstrip("\r\n"))
            if match:
                found.append((match.group(1).lower(), offset, offset + len(line)))
        if FENCE.match(line):
            in_fence = not in_fence
        offset += len(line)
    return [(role, body[end:next_start]) for (role, _start, end), (_next_role, next_start, _next_end) in zip(found, found[1:] + [("", len(body), len(body))])]


def first_fenced_block(value: str) -> tuple[str, str]:
    """Return the first Markdown fenced block and all remaining visible prose."""
    match = re.search(r"(?:^|\n)```[^\n]*\n(.*?)(?:\n```)(?:\n|$)", value, re.DOTALL)
    if not match:
        return value.strip(), ""
    raw = match.group(1)
    remainder = (value[:match.start()] + value[match.end():]).strip()
    return raw, remainder


def asset_reference(value: str, assets: Path, threshold: int) -> tuple[str | None, Json | None]:
    """Keep large public values whole without embedding them into static HTML."""
    encoded = value.encode("utf-8")
    if len(encoded) <= threshold:
        return value, None
    digest = hashlib.sha256(encoded).hexdigest()
    filename = f"{digest}.txt"
    destination = assets / filename
    if not destination.exists():
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_bytes(encoded)
    elif destination.read_bytes() != encoded:
        raise ValueError(f"asset digest collision at {destination}")
    return None, {"storage": "asset", "path": f"agent-assets/{filename}", "href": f"/agent-assets/{filename}", "bytes": len(encoded)}


def explicit_input_paths(raw_input: str) -> list[str]:
    """Return only explicit JSON path fields; opaque input is intentionally ignored."""
    try:
        parsed = json.loads(raw_input)
    except json.JSONDecodeError:
        return []
    found: set[str] = set()

    def visit(value: Any, key: str = "") -> None:
        if isinstance(value, dict):
            for child_key, child in value.items():
                visit(child, str(child_key))
        elif isinstance(value, list):
            for child in value:
                visit(child, key)
        elif isinstance(value, str) and key.lower() in EXPLICIT_PATH_KEYS:
            # Do not turn command strings or pasted documents into file evidence.
            if value and "\n" not in value and len(value) < 4096:
                found.add(value)

    visit(parsed)
    return sorted(found)


def message_with_text(message_id: str, role: str, text: str, assets: Path, threshold: int) -> Json | None:
    text = nonempty(text)
    if text is None:
        return None
    inline, content = asset_reference(text, assets, threshold)
    message: Json = {"id": message_id, "role": role}
    if inline is not None:
        message["text"] = inline
    else:
        message["textContent"] = content
    return message


def tool_with_input(tool_id: str, name: str, raw_input: str, assets: Path, threshold: int) -> Json:
    inline, content = asset_reference(raw_input, assets, threshold)
    tool: Json = {"id": tool_id, "name": name, "status": "pending"}
    if inline is not None:
        tool["input"] = inline
    else:
        tool["inputContent"] = content
    paths = explicit_input_paths(raw_input)
    if paths:
        # This private-to-the-import pass marker is consumed when the session
        # derives its `files` field. It is never emitted in public JSON.
        tool["_explicitInputPaths"] = paths
    return tool


def files_from_tools(messages: list[Json]) -> list[Json]:
    evidence: dict[str, list[str]] = {}
    for message in messages:
        message_id = message["id"]
        for tool in message.get("tools", []):
            for path in tool.pop("_explicitInputPaths", []):
                ids = evidence.setdefault(path, [])
                if message_id not in ids:
                    ids.append(message_id)
    return [{"path": path, "kind": "touched", "evidenceMessageIds": evidence[path]} for path in sorted(evidence)]


def append_assistant_section(
    text: str,
    session_id: str,
    messages: list[Json],
    pending_tools: list[Json],
    assets: Path,
    threshold: int,
    next_id: list[int],
    next_tool: list[int],
) -> None:
    position = 0
    for marker in TOOL_USE.finditer(text):
        before = message_with_text(f"m{next_id[0]:05d}", "assistant", text[position:marker.start()], assets, threshold)
        if before:
            messages.append(before)
            next_id[0] += 1
        after_start = marker.end()
        following = TOOL_USE.search(text, after_start)
        following_start = following.start() if following else len(text)
        raw_input, trailing = first_fenced_block(text[after_start:following_start])
        tool = tool_with_input(f"{session_id}:tool-{next_tool[0]:05d}", marker.group(1).strip(), raw_input, assets, threshold)
        next_tool[0] += 1
        call: Json = {"id": f"m{next_id[0]:05d}", "role": "assistant", "agent": "amp", "tools": [tool]}
        messages.append(call)
        pending_tools.append(tool)
        next_id[0] += 1
        if trailing:
            followup = message_with_text(f"m{next_id[0]:05d}", "assistant", trailing, assets, threshold)
            if followup:
                messages.append(followup)
                next_id[0] += 1
        position = following_start
        if following is None:
            break
    else:
        position = 0
    if position < len(text):
        trailing = message_with_text(f"m{next_id[0]:05d}", "assistant", text[position:], assets, threshold)
        if trailing:
            messages.append(trailing)
            next_id[0] += 1


def attach_tool_event(kind: str, external_id: str, event_text: str, pending_tools: list[Json], assets: Path, threshold: int) -> bool:
    if not pending_tools:
        return False
    tool = pending_tools.pop(0)
    tool["sourceEventId"] = external_id
    tool["status"] = "failed" if kind == "Error" else "completed"
    output = nonempty(event_text)
    if output is not None:
        inline, content = asset_reference(output, assets, threshold)
        if inline is not None:
            tool["output"] = inline
        else:
            tool["outputContent"] = content
    return True


def append_user_section(
    text: str,
    messages: list[Json],
    pending_tools: list[Json],
    assets: Path,
    threshold: int,
    next_id: list[int],
) -> None:
    position = 0
    for marker in TOOL_EVENT.finditer(text):
        before = message_with_text(f"m{next_id[0]:05d}", "user", text[position:marker.start()], assets, threshold)
        if before:
            messages.append(before)
            next_id[0] += 1
        end = TOOL_EVENT.search(text, marker.end())
        event_end = end.start() if end else len(text)
        event_text = text[marker.end():event_end].strip()
        if not attach_tool_event(marker.group(1), marker.group(2), event_text, pending_tools, assets, threshold):
            # An unmatched event remains visible, but is never shown as a human prompt.
            preserved = message_with_text(f"m{next_id[0]:05d}", "tool", text[marker.start():event_end], assets, threshold)
            if preserved:
                messages.append(preserved)
                next_id[0] += 1
        position = event_end
        if end is None:
            break
    else:
        position = 0
    if position < len(text):
        trailing = message_with_text(f"m{next_id[0]:05d}", "user", text[position:], assets, threshold)
        if trailing:
            messages.append(trailing)
            next_id[0] += 1


def valid_timestamp(value: str | None) -> str | None:
    if not value:
        return None
    try:
        datetime.fromisoformat(value.replace("Z", "+00:00"))
    except ValueError:
        return None
    return value


def import_thread(path: Path, assets: Path, threshold: int = ASSET_THRESHOLD) -> Json:
    fields, body = parse_front_matter(path.read_text(encoding="utf-8"))
    thread_id = fields.get("threadId")
    if not thread_id or not THREAD_ID.fullmatch(thread_id):
        raise ValueError(f"{path}: missing or unsafe threadId")
    if path.stem != thread_id:
        raise ValueError(f"{path}: filename and threadId differ")
    title = fields.get("title")
    if not title:
        raise ValueError(f"{path}: missing title")
    messages: list[Json] = []
    pending_tools: list[Json] = []
    next_id = [1]
    next_tool = [1]
    for role, content in sections(body):
        if role == "assistant":
            append_assistant_section(content, thread_id, messages, pending_tools, assets, threshold, next_id, next_tool)
        else:
            append_user_section(content, messages, pending_tools, assets, threshold, next_id)
    source = {"kind": "amp_markdown_archive", "path": f"docs/ampcode-threads/{path.name}", "threadId": thread_id}
    if fields.get("archiveSource"):
        source["archiveSource"] = fields["archiveSource"]
    if fields.get("agentMode"):
        source["agentMode"] = fields["agentMode"]
    created = fields.get("created")
    if created:
        # ``startedAt`` must remain null for a malformed source timestamp, but
        # the original visible metadata is still part of the public record.
        source["createdRaw"] = created
    return {
        "id": thread_id,
        "agent": "amp",
        "title": title,
        "startedAt": valid_timestamp(created),
        "source": source,
        "files": files_from_tools(messages),
        "messages": messages,
    }


def import_archive(source: Path, sessions_output: Path, assets_output: Path, threshold: int = ASSET_THRESHOLD) -> Json:
    imported: list[str] = []
    rejected: list[Json] = []
    for path in sorted(source.glob("T-*.md")):
        try:
            session = import_thread(path, assets_output, threshold)
        except ValueError as error:
            rejected.append({"path": path.name, "error": str(error)})
            continue
        write_json(sessions_output / f"{session['id']}.json", session)
        imported.append(session["id"])
    return {"source": str(source), "imported": imported, "rejected": rejected, "assetThresholdBytes": threshold}


def main(argv: Iterable[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, default=Path("docs/ampcode-threads"))
    parser.add_argument("--sessions-output", type=Path, default=Path("website/src/data/agent-sessions"))
    parser.add_argument("--assets-output", type=Path, default=Path("website/public/agent-assets"))
    parser.add_argument("--asset-threshold", type=int, default=ASSET_THRESHOLD)
    args = parser.parse_args(argv)
    if args.asset_threshold < 0:
        parser.error("--asset-threshold must be non-negative")
    result = import_archive(args.source, args.sessions_output, args.assets_output, args.asset_threshold)
    print(json.dumps({"imported": len(result["imported"]), "rejected": result["rejected"], "assetThresholdBytes": result["assetThresholdBytes"]}, ensure_ascii=False))
    return 0 if not result["rejected"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
