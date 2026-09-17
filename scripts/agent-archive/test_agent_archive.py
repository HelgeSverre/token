#!/usr/bin/env python3
"""Focused fixture tests for the private archive tool."""

import importlib.util
import json
import tempfile
import unittest
from unittest import mock
from pathlib import Path


SCRIPT = Path(__file__).with_name("agent_archive.py")
SPEC = importlib.util.spec_from_file_location("agent_archive", SCRIPT)
archive = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(archive)


class FakeAgentsview:
    def __init__(self, metadata, pages, revisions=None, failures=None):
        self.metadata = metadata
        self.pages = pages
        self.revisions = list(revisions or [])
        self.failures = set(failures or [])
        self.calls = []

    def __call__(self, command):
        self.calls.append(command)
        session_id = command[3]
        if tuple(command[1:4]) in self.failures:
            raise RuntimeError("simulated CLI failure")
        if command[1:3] == ["session", "get"]:
            revision = self.revisions.pop(0) if self.revisions else self.metadata.get("transcript_revision", "1")
            return {**self.metadata, "id": session_id, "transcript_revision": revision}
        if command[1:3] == ["session", "messages"]:
            offset = int(command[command.index("--from") + 1])
            return {"messages": self.pages.get(offset, [])}
        raise AssertionError(command)


def entry(session_id="codex:one"):
    return {"id": session_id, "inclusion_status": "included", "transcript_revision": "1"}


class AgentArchiveTests(unittest.TestCase):
    def test_normalize_remote_accepts_scp_style_git_urls(self):
        self.assertEqual(
            archive.normalize_remote("git@github.com:HelgeSverre/token.git"),
            "github.com/helgesverre/token",
        )

    def test_selection_requires_evidence_for_worktree_and_marks_alias_only_review(self):
        aliases = {
            "legacy": {"id": "legacy", "project": "rust-editor", "agent": "amp", "transcript_revision": "1"},
        }
        identities = {
            "verified": {
                "id": "verified", "project": "other", "agent": "codex",
                "cwd": "/Users/helge/.codex/worktrees/a/token-editor",
                "normalized_remote": "github.com/HelgeSverre/token", "transcript_revision": "3",
            },
            "unverified": {
                "id": "unverified", "project": "other", "agent": "codex",
                "cwd": "/Users/helge/.codex/worktrees/b/token-editor",
                "repository_path": "/Users/helge/code/unrelated-token-clone",
                "worktree_relationship": "linked", "transcript_revision": "1",
            },
        }
        manifest = archive.build_manifest(aliases, identities)
        entries = {item["id"]: item for item in manifest["sessions"]}
        self.assertEqual(entries["legacy"]["inclusion_status"], "review")
        self.assertEqual(entries["verified"]["inclusion_status"], "included")
        self.assertEqual(entries["unverified"]["inclusion_status"], "review")

    def test_checked_in_amp_archive_is_strong_historical_project_evidence(self):
        aliases = {
            "amp:published": {
                "id": "amp:published", "project": "rust-editor", "agent": "amp", "transcript_revision": "1",
            },
            "amp:unpublished": {
                "id": "amp:unpublished", "project": "rust-editor", "agent": "amp", "transcript_revision": "1",
            },
        }
        manifest = archive.build_manifest(aliases, {}, {"amp:published"})
        entries = {item["id"]: item for item in manifest["sessions"]}
        self.assertEqual(entries["amp:published"]["inclusion_status"], "included")
        self.assertIn("repository_amp_archive", {item["kind"] for item in entries["amp:published"]["matching_evidence"]})
        self.assertEqual(entries["amp:unpublished"]["inclusion_status"], "review")

    def test_manifest_adds_relationship_only_children_but_never_auto_includes_them(self):
        identities = {
            "parent": {
                "id": "parent", "project": "token-editor", "agent": "codex",
                "normalized_remote": "github.com/HelgeSverre/token", "transcript_revision": "1",
            },
            "child": {
                "id": "child", "project": "unknown", "agent": "codex",
                "parent_session_id": "parent", "parser_parent_session_id": "parser-parent", "transcript_revision": "1",
            },
            "orphan": {
                "id": "orphan", "project": "token_editor", "agent": "codex",
                "normalized_remote": "github.com/HelgeSverre/token", "parent_session_id": "missing", "transcript_revision": "1",
            },
        }
        manifest = archive.build_manifest({}, identities)
        entries = {item["id"]: item for item in manifest["sessions"]}
        self.assertEqual(entries["child"]["inclusion_status"], "review")
        self.assertIn("relationship_only_child_of_selected_session", entries["child"]["relationship_review"])
        self.assertEqual(entries["child"]["parent_reference"]["parser_parent_session_id"], "parser-parent")
        self.assertEqual(entries["orphan"]["inclusion_status"], "review")
        self.assertIn("parent_not_selected", entries["orphan"]["relationship_review"])

    def test_strong_alias_variants_do_not_create_a_false_parent_project_mismatch(self):
        identities = {
            "parent": {
                "id": "parent", "project": "token-editor", "agent": "codex",
                "normalized_remote": "github.com/HelgeSverre/token", "transcript_revision": "1",
            },
            "child": {
                "id": "child", "project": "token_editor", "agent": "codex", "parent_session_id": "parent",
                "normalized_remote": "github.com/HelgeSverre/token", "transcript_revision": "1",
            },
        }
        manifest = archive.build_manifest({}, identities)
        child = next(item for item in manifest["sessions"] if item["id"] == "child")
        self.assertEqual(child["inclusion_status"], "included")
        self.assertIsNone(child["relationship_review"])

    def test_sparse_ordinal_pagination_advances_after_last_actual_message(self):
        runner = FakeAgentsview({}, {
            0: [{"ordinal": 0, "content": "a"}, {"ordinal": 5, "content": "b"}],
            6: [{"ordinal": 9, "content": "c"}], 10: [],
        })
        messages = archive.fetch_all_messages("agentsview", "codex:one", 2, runner)
        self.assertEqual([item["ordinal"] for item in messages], [0, 5, 9])
        offsets = [call[call.index("--from") + 1] for call in runner.calls]
        self.assertEqual(offsets, ["0", "6", "10"])

    def test_message_page_rejects_non_objects_duplicate_and_unordered_ordinals(self):
        for page in (
            [{"ordinal": 0}, "invalid"],
            [{"ordinal": 1}, {"ordinal": 1}],
            [{"ordinal": 2}, {"ordinal": 1}],
        ):
            with self.subTest(page=page):
                runner = FakeAgentsview({}, {0: page})
                with self.assertRaises(ValueError):
                    archive.fetch_all_messages("agentsview", "codex:one", 10, runner)

    def test_revision_change_retries_then_writes_stable_snapshot(self):
        runner = FakeAgentsview({"agent": "codex"}, {0: [{"ordinal": 0}], 1: []}, revisions=["1", "2", "2", "2"])
        snapshot = archive.snapshot_session("agentsview", {**entry(), "transcript_revision": ""}, 10, 1, runner)
        self.assertEqual(snapshot["extraction"]["source_revision"], "2")
        self.assertEqual(len(snapshot["extraction"]["attempts"]), 2)

    def test_frozen_manifest_rejects_revision_and_message_count_changes(self):
        changed_revision = FakeAgentsview({"agent": "codex", "transcript_revision": "2"}, {})
        with self.assertRaisesRegex(RuntimeError, "frozen manifest revision"):
            archive.snapshot_session("agentsview", entry(), 10, 0, changed_revision)
        wrong_count = FakeAgentsview({"agent": "codex", "transcript_revision": "1"}, {0: [{"ordinal": 0}], 1: []})
        with self.assertRaisesRegex(RuntimeError, "frozen manifest records"):
            archive.snapshot_session("agentsview", {**entry(), "message_count": 2}, 10, 0, wrong_count)

    def test_attachment_inventory_inspects_json_encoded_content_and_input(self):
        references = archive.attachment_references({
            "input_json": '{"attachments":[{"url":"https://example.test/image.png"}]}',
            "content": '{"asset":{"id":"external-asset"}}',
        })
        pointers = {reference["pointer"] for reference in references}
        self.assertIn("$.input_json<json>.attachments", pointers)
        self.assertIn("$.input_json<json>.attachments[0].url", pointers)
        self.assertIn("$.content<json>.asset", pointers)

    def test_extract_resumes_completed_unchanged_without_message_fetch(self):
        runner = FakeAgentsview({"agent": "codex", "transcript_revision": "1"}, {0: [{"ordinal": 0}], 1: []})
        manifest = {"sessions": [entry()]}
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory)
            first = archive.extract_manifest(manifest, "agentsview", output, 10, 0, False, runner)
            self.assertEqual(first["sessions"]["codex:one"]["status"], "complete")
            runner.calls.clear()
            second = archive.extract_manifest(manifest, "agentsview", output, 10, 0, False, runner)
            self.assertEqual(second["sessions"]["codex:one"]["status"], "skipped_unchanged")
            self.assertFalse(any(call[1:3] == ["session", "messages"] for call in runner.calls))

    def test_resume_requires_matching_index_checksum_and_manifest_scope(self):
        runner = FakeAgentsview({"agent": "codex", "transcript_revision": "1"}, {0: [{"ordinal": 0}], 1: []})
        manifest = {"sessions": [entry()]}
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory)
            first = archive.extract_manifest(manifest, "agentsview", output, 10, 0, False, runner)
            record = first["sessions"]["codex:one"]
            snapshot_path = output / record["path"]
            snapshot_path.write_text('{"valid":"but modified"}', encoding="utf-8")
            runner.calls.clear()
            second = archive.extract_manifest(manifest, "agentsview", output, 10, 0, False, runner)
            self.assertEqual(second["sessions"]["codex:one"]["status"], "complete")
            self.assertTrue(any(call[1:3] == ["session", "messages"] for call in runner.calls))
            new_manifest = {"sessions": [entry("codex:two")]}
            third = archive.extract_manifest(new_manifest, "agentsview", output, 10, 0, False, runner)
            self.assertEqual(set(third["sessions"]), {"codex:two"})

    def test_include_review_does_not_extract_excluded_entries(self):
        runner = FakeAgentsview({"agent": "codex", "transcript_revision": "1"}, {0: [],})
        excluded = {**entry("codex:excluded"), "inclusion_status": "excluded"}
        with tempfile.TemporaryDirectory() as directory:
            index = archive.extract_manifest({"sessions": [excluded]}, "agentsview", Path(directory), 10, 0, True, runner)
        self.assertEqual(index["sessions"], {})

    def test_cli_returns_nonzero_when_extraction_index_has_incomplete_session(self):
        with tempfile.TemporaryDirectory() as directory:
            manifest_path = Path(directory) / "manifest.json"
            manifest_path.write_text(json.dumps({"schema_version": 1, "sessions": []}), encoding="utf-8")
            with mock.patch.object(archive, "extract_manifest", return_value={
                "sessions": {"codex:one": {"status": "incomplete"}},
            }):
                exit_code = archive.main(["extract", "--manifest", str(manifest_path)])
        self.assertEqual(exit_code, 1)

    def test_extract_records_cli_failure_as_incomplete(self):
        runner = FakeAgentsview({}, {}, failures={("session", "get", "codex:one")})
        with tempfile.TemporaryDirectory() as directory:
            index = archive.extract_manifest({"sessions": [entry()]}, "agentsview", Path(directory), 10, 0, False, runner)
            self.assertEqual(index["sessions"]["codex:one"]["status"], "incomplete")


if __name__ == "__main__":
    unittest.main()
