#!/usr/bin/env python3
import hashlib
import importlib.util
import json
import tempfile
import unittest
from pathlib import Path


SCRIPT = Path(__file__).parents[1] / "scripts" / "validate_annotation.py"
SPEC = importlib.util.spec_from_file_location("validate_annotation", SCRIPT)
validator = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(validator)


class AnnotationValidationTests(unittest.TestCase):
    def test_rich_fixture_matches_the_contract(self):
        fixtures = Path(__file__).with_name("fixtures")
        self.assertEqual(
            validator.errors_for(fixtures / "rich-source.json", fixtures / "rich-annotation.json"),
            [],
        )

    def write_source(self, directory: Path) -> tuple[Path, str]:
        source = directory / "source.json"
        source.write_bytes(b'{\n  "session": {"id": "codex:fixture"},\n  "messages": [{"ordinal": 0}, {"id": "message-1", "ordinal": 2}, {"ordinal": 7}]\n}\n')
        return source, hashlib.sha256(source.read_bytes()).hexdigest()

    def write_annotation(self, directory: Path, value: dict) -> Path:
        annotation = directory / "annotation.json"
        annotation.write_text(json.dumps(value), encoding="utf-8")
        return annotation

    def minimal(self, digest: str) -> dict:
        return {"schemaVersion": 1, "sessionId": "codex:fixture", "sourceDigest": digest}

    def test_allows_unannotated_sidecar(self):
        with tempfile.TemporaryDirectory() as temporary:
            source, digest = self.write_source(Path(temporary))
            annotation = self.write_annotation(Path(temporary), self.minimal(digest))
            self.assertEqual(validator.errors_for(source, annotation), [])

    def test_builds_a_message_map_without_transcript_text(self):
        with tempfile.TemporaryDirectory() as temporary:
            source, _ = self.write_source(Path(temporary))
            source_value = json.loads(source.read_text(encoding="utf-8"))
            source_value["messages"][0]["text"] = "private transcript content"
            self.assertEqual(
                validator.message_reference_map(source_value),
                [{"messageId": "ordinal:0", "ordinal": 0}, {"messageId": "message-1", "ordinal": 2}, {"messageId": "ordinal:7", "ordinal": 7}],
            )

    def test_allows_evidence_bound_editorial_sections(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            source, digest = self.write_source(directory)
            value = self.minimal(digest) | {
                "title": "Fix focus order", "summary": "The session verified a focused change.",
                "evidenceMessageIds": ["ordinal:0", "message-1"],
                "chapters": [{"id": "investigate", "title": "Investigate", "startMessageId": "ordinal:0", "endMessageId": "message-1", "evidenceMessageIds": ["ordinal:0"]}],
                "outcome": {"summary": "A focused check reported success.", "evidenceMessageIds": ["ordinal:7"]},
                "highlights": [{"id": "check", "title": "Run a check", "messageId": "ordinal:7", "evidenceMessageIds": ["ordinal:7"]}],
                "files": [{"path": "src/view.rs", "additions": 2, "deletions": 1, "evidenceMessageIds": ["message-1"]}],
            }
            annotation = self.write_annotation(directory, value)
            self.assertEqual(validator.errors_for(source, annotation), [])

    def test_rejects_stale_exact_byte_digest(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            source, digest = self.write_source(directory)
            annotation = self.write_annotation(directory, self.minimal(digest))
            source.write_bytes(source.read_bytes() + b" ")
            self.assertIn("annotation.sourceDigest: does not match the exact source file bytes", validator.errors_for(source, annotation))

    def test_rejects_unknown_and_unsupported_message_reference(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            source, digest = self.write_source(directory)
            annotation = self.write_annotation(directory, self.minimal(digest) | {
                "title": "Unsupported evidence", "evidenceMessageIds": ["made-up"], "branch": "main",
            })
            errors = validator.errors_for(source, annotation)
            self.assertTrue(any("unsupported properties: branch" in error for error in errors))
            self.assertTrue(any("is not a source message reference" in error for error in errors))

    def test_title_requires_top_level_evidence(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            source, digest = self.write_source(directory)
            annotation = self.write_annotation(directory, self.minimal(digest) | {"title": "Uncited"})
            self.assertIn("annotation.evidenceMessageIds: required with title or summary", validator.errors_for(source, annotation))


if __name__ == "__main__":
    unittest.main()
