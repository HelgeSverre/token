#!/usr/bin/env python3
"""Offline tests for the private snapshot normalizer."""

import importlib.util
import json
import tempfile
import unittest
from pathlib import Path


SCRIPT = Path(__file__).with_name("normalize_archive.py")
SPEC = importlib.util.spec_from_file_location("normalize_archive", SCRIPT)
normalizer = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(normalizer)


def snapshot():
    return {
        "schema_version": 1,
        "session": {
            "id": "codex:opaque", "agent": "codex", "session_name": "Archive fixture",
            "started_at": "not a timestamp", "git_branch": "feature/archive",
        },
        "manifest_entry": {"id": "codex:opaque", "provider": "codex", "inclusion_status": "included"},
        "messages": [
            {"ordinal": 0, "role": "user", "content": "internal compaction", "is_sidechain": True},
            {"ordinal": 5, "role": "user", "content": "Please inspect the archive."},
            {
                "ordinal": 9, "role": "assistant", "timestamp": "2026-09-16T10:01:02Z", "model": "test-model",
                "content": "I am checking the source.", "provider_specific": {"preserved": True},
                "tool_calls": [
                    {"id": "tool-1", "name": "bash", "input_json": "echo not-json", "output": "x" * 70000, "status": "completed", "file_path": "src/lib.rs"},
                    {"id": "tool-2", "name": "read", "input_json": '{"path":"Cargo.toml"}', "status": "completed", "path": "Cargo.toml"},
                ],
            },
            {"ordinal": 12, "role": "tool", "content": "provider tool role"},
            {"ordinal": 13, "role": "mystery", "content": "unrecognized shape"},
        ],
        "tool_result_events": [
            {"tool_call_message_ordinal": 9, "call_index": 0, "tool_use_id": "tool-1", "content": "alternate result", "status": "completed", "event_index": 0},
            {"tool_call_message_ordinal": 99, "call_index": 0, "content": "unmatched", "status": "failed", "event_index": 1},
        ],
        "extraction": {"source_revision": "4", "completeness": {"attachments": "referenced_not_fetched"}, "attachment_references": [{"pointer": "$.messages[2].image", "key": "image", "value": "external"}]},
    }


def write_extraction(input_dir: Path, source):
    input_dir.mkdir(parents=True)
    name = "source.json"
    (input_dir / name).write_text(json.dumps(source), encoding="utf-8")
    index = {"schema_version": 1, "manifest_checksum": "frozen-manifest", "sessions": {"codex:opaque": {"status": "complete", "path": name, "checksum": normalizer.checksum(source)}}}
    (input_dir / "index.json").write_text(json.dumps(index), encoding="utf-8")


class NormalizeArchiveTests(unittest.TestCase):
    def test_provider_statuses_and_json_tool_arguments_normalize_for_viewer_facets(self):
        self.assertEqual(normalizer.normalized_tool_status("completed"), "completed")
        self.assertEqual(normalizer.normalized_tool_status("errored"), "failed")
        self.assertEqual(normalizer.normalized_tool_status("cancelled"), "failed")
        self.assertEqual(normalizer.normalized_tool_status("token_limit"), "unknown")
        self.assertEqual(normalizer.normalized_tool_status(None), "unknown")
        self.assertEqual(normalizer.explicit_tool_status({"is_error": True}, None, None), "failed")
        self.assertEqual(normalizer.explicit_tool_status({}, {"isSuccess": True}, None), "completed")
        self.assertEqual(normalizer.explicit_tool_status({}, None, None), "unknown")
        self.assertEqual(
            normalizer.file_paths({"input_json": '{"path":"src/main.rs","nested":{"file_path":"Cargo.toml"}}'}),
            ["Cargo.toml", "src/main.rs"],
        )

    def test_normalizes_fixture_contract_without_tool_result_duplication(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            write_extraction(root / "input", snapshot())
            index = normalizer.normalize_directory(root / "input", root / "normalized")
            record = index["sessions"]["codex:opaque"]
            self.assertEqual(record["status"], "complete")
            self.assertEqual(record["summary"]["humanTurns"], 1)
            self.assertEqual(record["summary"]["toolNames"], {"bash": 1, "read": 1})
            self.assertEqual(record["summary"]["fileTouchCount"], 2)
            session = normalizer.read_json(root / "normalized" / record["path"])
            self.assertIsNone(session["startedAt"])
            self.assertEqual(session["source"]["sessionTimestamps"]["startedAt"], "not a timestamp")
            self.assertEqual([message["ordinal"] for message in session["messages"]], [0, 5, 9, 12, 13])
            self.assertEqual(session["messages"][0]["role"], "system")
            self.assertEqual(session["messages"][0]["kind"], "system")
            self.assertEqual(session["messages"][1]["kind"], "human_turn")
            self.assertEqual(session["messages"][4]["role"], "unknown")
            fixture_tool = session["messages"][2]["tools"][0]
            self.assertIsNone(fixture_tool["output"])
            self.assertEqual(fixture_tool["outputContent"]["storage"], "asset")
            self.assertEqual(fixture_tool["outputContent"]["href"], "/agent-assets/" + fixture_tool["outputContent"]["path"].split("/")[-1])
            self.assertTrue((root / "normalized" / fixture_tool["outputContent"]["path"]).exists())
            call = session["toolCalls"][0]
            self.assertEqual(call["result"]["source"], "call_embedded")
            self.assertEqual(len(call["alternateResults"]), 1)
            self.assertEqual(len(session["events"]), 1)
            self.assertEqual(session["toolCalls"][0]["input"]["format"], "opaque")
            self.assertEqual(session["toolCalls"][1]["input"]["format"], "json")
            self.assertTrue(all("added" not in file and "removed" not in file for file in session["files"]))

    def test_rejects_checksum_mismatch_without_writing_a_session(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = snapshot()
            write_extraction(root / "input", source)
            index_path = root / "input" / "index.json"
            index = normalizer.read_json(index_path)
            index["sessions"]["codex:opaque"]["checksum"] = "wrong"
            index_path.write_text(json.dumps(index), encoding="utf-8")
            result = normalizer.normalize_directory(root / "input", root / "normalized")
            self.assertEqual(result["sessions"]["codex:opaque"]["status"], "rejected")
            self.assertFalse((root / "normalized" / "sessions").exists())

    def test_rerun_skips_same_frozen_snapshot(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            write_extraction(root / "input", snapshot())
            normalizer.normalize_directory(root / "input", root / "normalized")
            second = normalizer.normalize_directory(root / "input", root / "normalized")
            self.assertEqual(second["sessions"]["codex:opaque"]["status"], "skipped_unchanged")

    def test_forced_rerun_has_the_same_normalized_checksum(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            write_extraction(root / "input", snapshot())
            first = normalizer.normalize_directory(root / "input", root / "normalized")
            second = normalizer.normalize_directory(root / "input", root / "normalized", force=True)
            self.assertEqual(
                first["sessions"]["codex:opaque"]["normalizedChecksum"],
                second["sessions"]["codex:opaque"]["normalizedChecksum"],
            )

    def test_normalization_version_invalidates_an_older_output_index(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            write_extraction(root / "input", snapshot())
            normalizer.normalize_directory(root / "input", root / "normalized")
            index_path = root / "normalized" / "index.json"
            index = normalizer.read_json(index_path)
            index["normalizationVersion"] = normalizer.NORMALIZATION_VERSION - 1
            normalizer.atomic_json(index_path, index)
            regenerated = normalizer.normalize_directory(root / "input", root / "normalized")
            record = regenerated["sessions"]["codex:opaque"]
            self.assertEqual(record["status"], "complete")
            self.assertEqual(record["normalizationVersion"], normalizer.NORMALIZATION_VERSION)
            self.assertEqual(regenerated["normalizationVersion"], normalizer.NORMALIZATION_VERSION)

    def test_promotion_requires_exact_checksum_bound_allowlist_and_copies_assets(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            write_extraction(root / "input", snapshot())
            index = normalizer.normalize_directory(root / "input", root / "normalized")
            checksum = index["sessions"]["codex:opaque"]["normalizedChecksum"]
            allowlist = {"schemaVersion": 1, "sessions": [{"id": "codex:opaque", "status": "approved", "normalizedChecksum": checksum}]}
            allowlist_path = root / "allowlist.json"
            allowlist_path.write_text(json.dumps(allowlist), encoding="utf-8")
            receipt = normalizer.promote_reviewed(root / "normalized", allowlist_path, root / "reviewed")
            self.assertEqual(receipt["sessions"], ["codex:opaque"])
            self.assertTrue((root / "reviewed" / "receipt.json").exists())
            allowlist["sessions"][0]["normalizedChecksum"] = "wrong"
            allowlist_path.write_text(json.dumps(allowlist), encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "allowlist checksum"):
                normalizer.promote_reviewed(root / "normalized", allowlist_path, root / "reviewed-again")

    def test_resume_repairs_a_tampered_session_or_asset_and_blocks_incomplete_source(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            write_extraction(root / "input", snapshot())
            first = normalizer.normalize_directory(root / "input", root / "normalized")
            record = first["sessions"]["codex:opaque"]
            session_path = root / "normalized" / record["path"]
            normalized = normalizer.read_json(session_path)
            asset_path = root / "normalized" / normalized["messages"][2]["tools"][0]["outputContent"]["path"]
            session_path.write_text("{}", encoding="utf-8")
            repaired = normalizer.normalize_directory(root / "input", root / "normalized")
            self.assertEqual(repaired["sessions"]["codex:opaque"]["status"], "complete")
            asset_path.write_text("tampered", encoding="utf-8")
            repaired_asset = normalizer.normalize_directory(root / "input", root / "normalized")
            self.assertEqual(repaired_asset["sessions"]["codex:opaque"]["status"], "complete")
            self.assertEqual(asset_path.stat().st_size, 70000)
            extraction_index = normalizer.read_json(root / "input" / "index.json")
            extraction_index["sessions"]["codex:opaque"]["status"] = "incomplete"
            (root / "input" / "index.json").write_text(json.dumps(extraction_index), encoding="utf-8")
            blocked = normalizer.normalize_directory(root / "input", root / "normalized")
            self.assertEqual(blocked["sessions"]["codex:opaque"]["status"], "blocked")

    def test_export_reviewed_is_presentation_only_and_requires_explicit_text_asset_thinking_review(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            write_extraction(root / "input", snapshot())
            index = normalizer.normalize_directory(root / "input", root / "normalized")
            checksum = index["sessions"]["codex:opaque"]["normalizedChecksum"]
            allowlist = {"schemaVersion": 1, "sessions": [{
                "id": "codex:opaque", "status": "approved_public", "normalizedChecksum": checksum,
                "review": {"publicTextReviewed": True, "publicAssetsReviewed": True, "thinkingReviewed": True},
            }]}
            allowlist_path = root / "public-allowlist.json"
            allowlist_path.write_text(json.dumps(allowlist), encoding="utf-8")
            receipt = normalizer.export_reviewed(root / "normalized", allowlist_path, root / "website-ready")
            self.assertEqual(len(receipt["sessions"]), 1)
            exported = normalizer.read_json(root / "website-ready" / receipt["sessions"][0]["path"])
            self.assertNotIn("source", exported)
            self.assertNotIn("toolCalls", exported)
            self.assertNotIn("events", exported)
            self.assertNotIn("thinking", exported["messages"][2])
            self.assertEqual(exported["messages"][0]["role"], "system")
            self.assertEqual(exported["publication"]["omitted"], {"unmatchedResultEvents": 1, "attachmentReferences": 1, "dedicatedThinkingMessages": 0})
            output = exported["messages"][2]["tools"][0]["outputContent"]
            self.assertEqual(output["path"].split("/")[0], "agent-assets")
            self.assertTrue((root / "website-ready" / output["path"]).exists())
            allowlist["sessions"][0]["review"]["thinkingReviewed"] = False
            allowlist_path.write_text(json.dumps(allowlist), encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "text, assets, and thinking"):
                normalizer.export_reviewed(root / "normalized", allowlist_path, root / "bad")

    def test_export_preview_projects_every_complete_record_without_review_assertions(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            write_extraction(root / "input", snapshot())
            normalizer.normalize_directory(root / "input", root / "normalized")
            receipt = normalizer.export_preview(root / "normalized", root / "preview")
            self.assertEqual(receipt["purpose"], "private-preview")
            self.assertEqual(len(receipt["sessions"]), 1)
            entry = receipt["sessions"][0]
            self.assertEqual(entry["id"], "codex:opaque")
            exported = normalizer.read_json(root / "preview" / entry["path"])
            self.assertEqual(normalizer.checksum(exported), entry["checksum"])
            self.assertNotIn("source", exported)
            self.assertTrue((root / "preview" / "receipt.json").exists())
            output = exported["messages"][2]["tools"][0]["outputContent"]
            self.assertTrue((root / "preview" / output["path"]).exists())

    def test_export_rejects_unsafe_session_and_subagent_routes(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            write_extraction(root / "input", snapshot())
            index = normalizer.normalize_directory(root / "input", root / "normalized")
            record = index["sessions"]["codex:opaque"]
            source_path = root / "normalized" / record["path"]
            normalized = normalizer.read_json(source_path)
            normalized["messages"][2]["tools"][0]["subagentSessionId"] = "../not-a-route"
            normalizer.atomic_json(source_path, normalized)
            record["normalizedChecksum"] = normalizer.checksum(normalized)
            normalizer.atomic_json(root / "normalized" / "index.json", index)
            allowlist = {"schemaVersion": 1, "sessions": [{"id": "codex:opaque", "status": "approved_public", "normalizedChecksum": record["normalizedChecksum"], "review": {"publicTextReviewed": True, "publicAssetsReviewed": True, "thinkingReviewed": True}}]}
            allowlist_path = root / "allowlist.json"
            allowlist_path.write_text(json.dumps(allowlist), encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "subagent session id"):
                normalizer.export_reviewed(root / "normalized", allowlist_path, root / "website-ready")


if __name__ == "__main__":
    unittest.main()
