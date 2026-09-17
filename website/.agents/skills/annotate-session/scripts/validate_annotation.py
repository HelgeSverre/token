#!/usr/bin/env python3
"""Validate one Token session-annotation v1 sidecar against its source JSON."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
from pathlib import Path
from typing import Any


SHA256 = re.compile(r"^[a-f0-9]{64}$")
SLUG = re.compile(r"^[a-z0-9]+(?:-[a-z0-9]+)*$")
MAX = {
    "sessionId": 512, "title": 160, "summary": 2800, "chapter_title": 120,
    "chapter_summary": 1200, "outcome_summary": 1200, "highlight_title": 120,
    "highlight_summary": 800, "file_path": 1024, "file_summary": 600, "id": 80,
}


def load_json(path: Path) -> Any:
    return json.loads(path.read_text(encoding="utf-8"))


def session_id_from(source: Any) -> str | None:
    if not isinstance(source, dict):
        return None
    session = source.get("session")
    if isinstance(session, dict) and isinstance(session.get("id"), str) and session["id"]:
        return session["id"]
    if isinstance(source.get("sessionId"), str) and source["sessionId"]:
        return source["sessionId"]
    if isinstance(source.get("id"), str) and source["id"]:
        return source["id"]
    return None


def message_references(source: Any) -> list[str]:
    if not isinstance(source, dict) or not isinstance(source.get("messages"), list):
        raise ValueError("source must contain a messages array")
    refs: list[str] = []
    for index, message in enumerate(source["messages"]):
        if not isinstance(message, dict):
            raise ValueError(f"source.messages[{index}] must be an object")
        raw_id = message.get("id")
        if isinstance(raw_id, str) and raw_id:
            reference = raw_id
        else:
            ordinal = message.get("ordinal")
            if type(ordinal) is not int:
                raise ValueError(f"source.messages[{index}] needs a non-empty id or integer ordinal")
            reference = f"ordinal:{ordinal}"
        if reference in refs:
            raise ValueError(f"source produces duplicate message reference {reference!r}")
        refs.append(reference)
    return refs


def message_reference_map(source: Any) -> list[dict[str, Any]]:
    """Return a small source-derived map safe to attach to an annotation job."""
    references = message_references(source)
    assert isinstance(source, dict)
    entries: list[dict[str, Any]] = []
    for reference, message in zip(references, source["messages"]):
        assert isinstance(message, dict)
        entry: dict[str, Any] = {"messageId": reference}
        for key in ("ordinal", "role", "agent", "timestamp"):
            value = message.get(key)
            if value is not None:
                entry[key] = value
        entries.append(entry)
    return entries


def errors_for(source_path: Path, annotation_path: Path) -> list[str]:
    errors: list[str] = []
    try:
        source_bytes = source_path.read_bytes()
        source = json.loads(source_bytes.decode("utf-8"))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as error:
        return [f"source: {error}"]
    try:
        annotation = load_json(annotation_path)
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as error:
        return [f"annotation: {error}"]
    if not isinstance(annotation, dict):
        return ["annotation: must be an object"]

    source_id = session_id_from(source)
    if source_id is None:
        errors.append("source: no session.id, sessionId, or id string")
    try:
        references = message_references(source)
    except ValueError as error:
        errors.append(f"source: {error}")
        references = []
    reference_set = set(references)
    position = {reference: index for index, reference in enumerate(references)}

    allowed = {"schemaVersion", "sessionId", "sourceDigest", "title", "summary", "evidenceMessageIds", "chapters", "outcome", "highlights", "files"}
    unknown = sorted(set(annotation) - allowed)
    if unknown:
        errors.append(f"annotation: unsupported properties: {', '.join(unknown)}")
    for name in ("schemaVersion", "sessionId", "sourceDigest"):
        if name not in annotation:
            errors.append(f"annotation.{name}: required")
    if annotation.get("schemaVersion") != 1:
        errors.append("annotation.schemaVersion: must equal 1")
    string(annotation, "sessionId", MAX["sessionId"], errors, required=True)
    digest = annotation.get("sourceDigest")
    if not isinstance(digest, str) or not SHA256.fullmatch(digest):
        errors.append("annotation.sourceDigest: must be a lowercase SHA-256 digest")
    elif digest != hashlib.sha256(source_bytes).hexdigest():
        errors.append("annotation.sourceDigest: does not match the exact source file bytes")
    if source_id is not None and annotation.get("sessionId") != source_id:
        errors.append("annotation.sessionId: does not match source session ID")

    has_title = "title" in annotation
    has_summary = "summary" in annotation
    string(annotation, "title", MAX["title"], errors)
    string(annotation, "summary", MAX["summary"], errors)
    top_evidence = evidence(annotation, "evidenceMessageIds", reference_set, errors)
    if has_title or has_summary:
        if "evidenceMessageIds" not in annotation:
            errors.append("annotation.evidenceMessageIds: required with title or summary")
    elif "evidenceMessageIds" in annotation:
        errors.append("annotation.evidenceMessageIds: requires title or summary")

    chapters = annotation.get("chapters")
    if chapters is not None:
        if not isinstance(chapters, list) or len(chapters) > 40:
            errors.append("annotation.chapters: must be an array with at most 40 items")
        else:
            seen_ids: set[str] = set()
            previous_start = -1
            for index, chapter in enumerate(chapters):
                chapter_path = f"annotation.chapters[{index}]"
                if not object_with(chapter, {"id", "title", "startMessageId", "endMessageId", "summary", "evidenceMessageIds"}, {"id", "title", "startMessageId", "evidenceMessageIds"}, chapter_path, errors):
                    continue
                chapter_id = chapter.get("id")
                if not isinstance(chapter_id, str) or len(chapter_id) > MAX["id"] or not SLUG.fullmatch(chapter_id):
                    errors.append(f"{chapter_path}.id: must be a lowercase slug")
                elif chapter_id in seen_ids:
                    errors.append(f"{chapter_path}.id: duplicates a chapter ID")
                else:
                    seen_ids.add(chapter_id)
                string(chapter, "title", MAX["chapter_title"], errors, path=chapter_path, required=True)
                string(chapter, "summary", MAX["chapter_summary"], errors, path=chapter_path)
                start = message_ref(chapter, "startMessageId", reference_set, errors, chapter_path, required=True)
                end = message_ref(chapter, "endMessageId", reference_set, errors, chapter_path)
                if start in position:
                    if position[start] < previous_start:
                        errors.append(f"{chapter_path}.startMessageId: chapters must be chronological")
                    previous_start = position[start]
                if start in position and end in position and position[end] < position[start]:
                    errors.append(f"{chapter_path}.endMessageId: must not precede startMessageId")
                evidence(chapter, "evidenceMessageIds", reference_set, errors, chapter_path, required=True)

    outcome = annotation.get("outcome")
    if outcome is not None:
        if object_with(outcome, {"summary", "evidenceMessageIds"}, {"summary", "evidenceMessageIds"}, "annotation.outcome", errors):
            string(outcome, "summary", MAX["outcome_summary"], errors, path="annotation.outcome", required=True)
            evidence(outcome, "evidenceMessageIds", reference_set, errors, "annotation.outcome", required=True)

    highlights = annotation.get("highlights")
    if highlights is not None:
        if not isinstance(highlights, list) or len(highlights) > 30:
            errors.append("annotation.highlights: must be an array with at most 30 items")
        else:
            seen_ids: set[str] = set()
            for index, highlight in enumerate(highlights):
                item_path = f"annotation.highlights[{index}]"
                if not object_with(highlight, {"id", "title", "summary", "messageId", "evidenceMessageIds"}, {"id", "title", "messageId", "evidenceMessageIds"}, item_path, errors):
                    continue
                identifier = highlight.get("id")
                if not isinstance(identifier, str) or len(identifier) > MAX["id"] or not SLUG.fullmatch(identifier):
                    errors.append(f"{item_path}.id: must be a lowercase slug")
                elif identifier in seen_ids:
                    errors.append(f"{item_path}.id: duplicates a highlight ID")
                else:
                    seen_ids.add(identifier)
                string(highlight, "title", MAX["highlight_title"], errors, path=item_path, required=True)
                string(highlight, "summary", MAX["highlight_summary"], errors, path=item_path)
                message_ref(highlight, "messageId", reference_set, errors, item_path, required=True)
                evidence(highlight, "evidenceMessageIds", reference_set, errors, item_path, required=True)

    files = annotation.get("files")
    if files is not None:
        if not isinstance(files, list) or len(files) > 100:
            errors.append("annotation.files: must be an array with at most 100 items")
        else:
            for index, file_note in enumerate(files):
                item_path = f"annotation.files[{index}]"
                if not object_with(file_note, {"path", "summary", "additions", "deletions", "evidenceMessageIds"}, {"path", "evidenceMessageIds"}, item_path, errors):
                    continue
                string(file_note, "path", MAX["file_path"], errors, path=item_path, required=True)
                string(file_note, "summary", MAX["file_summary"], errors, path=item_path)
                for count in ("additions", "deletions"):
                    value = file_note.get(count)
                    if value is not None and (type(value) is not int or value < 0):
                        errors.append(f"{item_path}.{count}: must be a non-negative integer")
                evidence(file_note, "evidenceMessageIds", reference_set, errors, item_path, required=True)
    return errors


def object_with(value: Any, allowed: set[str], required: set[str], path: str, errors: list[str]) -> bool:
    if not isinstance(value, dict):
        errors.append(f"{path}: must be an object")
        return False
    unknown = sorted(set(value) - allowed)
    if unknown:
        errors.append(f"{path}: unsupported properties: {', '.join(unknown)}")
    for key in sorted(required - set(value)):
        errors.append(f"{path}.{key}: required")
    return True


def string(value: dict[str, Any], key: str, maximum: int, errors: list[str], path: str = "annotation", required: bool = False) -> None:
    item = value.get(key)
    if item is None and not required:
        return
    if not isinstance(item, str) or not item or len(item) > maximum:
        errors.append(f"{path}.{key}: must be a non-empty string of at most {maximum} characters")


def message_ref(value: dict[str, Any], key: str, known: set[str], errors: list[str], path: str, required: bool = False) -> str | None:
    item = value.get(key)
    if item is None and not required:
        return None
    if not isinstance(item, str) or not item or len(item) > MAX["sessionId"]:
        errors.append(f"{path}.{key}: must be a non-empty message reference")
        return None
    if item not in known:
        errors.append(f"{path}.{key}: is not a source message reference")
    return item


def evidence(value: dict[str, Any], key: str, known: set[str], errors: list[str], path: str = "annotation", required: bool = False) -> list[str]:
    item = value.get(key)
    if item is None and not required:
        return []
    if not isinstance(item, list) or not item or len(item) > 64:
        errors.append(f"{path}.{key}: must be a non-empty array with at most 64 message references")
        return []
    if len(set(item)) != len(item):
        errors.append(f"{path}.{key}: must not contain duplicate references")
    for index, reference in enumerate(item):
        if not isinstance(reference, str) or not reference or len(reference) > MAX["sessionId"]:
            errors.append(f"{path}.{key}[{index}]: must be a non-empty message reference")
        elif reference not in known:
            errors.append(f"{path}.{key}[{index}]: is not a source message reference")
    return item


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path, help="private extracted source session JSON")
    parser.add_argument("annotation", nargs="?", type=Path, help="optional annotation sidecar JSON")
    parser.add_argument("--print-message-refs", action="store_true", help="print source-derived IDs for worker prompts")
    parser.add_argument("--print-message-map", action="store_true", help="print source-derived IDs and safe routing fields")
    args = parser.parse_args(argv)
    if args.print_message_refs or args.print_message_map:
        try:
            source = load_json(args.source)
            value = message_reference_map(source) if args.print_message_map else message_references(source)
            print(json.dumps(value, ensure_ascii=False, indent=2))
        except (OSError, UnicodeDecodeError, json.JSONDecodeError, ValueError) as error:
            print(f"source: {error}", file=sys.stderr)
            return 1
        if args.annotation is None:
            return 0
    if args.annotation is None:
        parser.error("annotation is required unless --print-message-refs is used alone")
    errors = errors_for(args.source, args.annotation)
    if errors:
        print("Annotation is invalid:", file=sys.stderr)
        print(*(f"- {error}" for error in errors), sep="\n", file=sys.stderr)
        return 1
    print("Annotation is valid.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
