#!/usr/bin/env python3
"""Boundary tests for the deterministic public Amp Markdown importer."""

import importlib.util
import json
import tempfile
import unittest
from pathlib import Path


SCRIPT = Path(__file__).with_name("import_amp_markdown.py")
SPEC = importlib.util.spec_from_file_location("import_amp_markdown", SCRIPT)
amp = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(amp)


THREAD_ID = "T-00000000-0000-0000-0000-000000000001"


def source(thread_id=THREAD_ID, created="2026-01-02T03:04:05Z"):
    return "\n".join([
        "---", 'title: "Archive parser boundary"', f"threadId: {thread_id}",
        f"created: {created}", "agentMode: smart", "archiveSource: local Amp session cache", "---", "",
        "# Archive parser boundary", "", "## User", "", "Please inspect the parser.", "",
        "## Assistant", "", "I will read it.", "", "**Tool Use:** `Read`", "", "```json",
        '{"path":"docs/example.md"}', "```", "", "## User", "", "**Tool Result:** `toolu_first`", "", "```",
        "## Assistant", "this heading is part of a result, not a speaker break", "```", "", "## Assistant", "",
        "Next, run the check.", "", "**Tool Use:** `Bash`", "", "```", "echo non-json input", "```", "",
        "## User", "", "**Tool Error:** `toolu_second`", "", "**Error:** command failed", "", "## User", "",
        "Can you explain the failure?", "",
    ])


class AmpMarkdownImportTests(unittest.TestCase):
    def write(self, root: Path, name: str, content: str) -> Path:
        root.mkdir(parents=True, exist_ok=True)
        path = root / name
        path.write_text(content, encoding="utf-8")
        return path

    def test_preserves_speakers_tools_and_non_json_input_without_yaml(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            path = self.write(root / "source", f"{THREAD_ID}.md", source())
            session = amp.import_thread(path, root / "assets")
            self.assertEqual(session["id"], THREAD_ID)
            self.assertEqual(session["startedAt"], "2026-01-02T03:04:05Z")
            self.assertEqual(session["source"]["archiveSource"], "local Amp session cache")
            self.assertEqual([message["role"] for message in session["messages"]], ["user", "assistant", "assistant", "assistant", "assistant", "user"])
            calls = [message["tools"][0] for message in session["messages"] if message.get("tools")]
            self.assertEqual([call["name"] for call in calls], ["Read", "Bash"])
            self.assertEqual(calls[0]["input"], '{"path":"docs/example.md"}')
            self.assertIn("## Assistant", calls[0]["output"])
            self.assertEqual(calls[0]["status"], "completed")
            self.assertEqual(calls[1]["input"], "echo non-json input")
            self.assertEqual(calls[1]["status"], "failed")
            self.assertIn("command failed", calls[1]["output"])
            self.assertEqual(session["messages"][-1]["role"], "user")
            self.assertEqual(session["files"], [{"path": "docs/example.md", "kind": "touched", "evidenceMessageIds": ["m00003"]}])

    def test_invalid_time_is_not_invented(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            path = self.write(root / "source", f"{THREAD_ID}.md", source(created="not a date"))
            session = amp.import_thread(path, root / "assets")
            self.assertIsNone(session["startedAt"])
            self.assertEqual(session["source"]["createdRaw"], "not a date")

    def test_large_values_become_content_addressed_assets(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            huge = "x" * 80
            path = self.write(root / "source", f"{THREAD_ID}.md", source().replace("echo non-json input", huge))
            session = amp.import_thread(path, root / "assets", threshold=64)
            bash = [message["tools"][0] for message in session["messages"] if message.get("tools")][1]
            self.assertNotIn("input", bash)
            reference = bash["inputContent"]
            self.assertEqual(reference["storage"], "asset")
            self.assertEqual((root / "assets" / reference["href"].split("/")[-1]).read_text(), huge)

    def test_file_evidence_keeps_only_explicit_json_path_fields(self):
        self.assertEqual(
            amp.explicit_input_paths('{"path":"src/lib.rs","nested":{"file_path":"tests/archive.rs"},"command":"cat src/nope.rs"}'),
            ["src/lib.rs", "tests/archive.rs"],
        )
        self.assertEqual(amp.explicit_input_paths("git diff src/nope.rs"), [])

    def test_import_reports_bad_threads_and_leaves_fixture_files_alone(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            archive = root / "archive"
            self.write(archive, f"{THREAD_ID}.md", source())
            self.write(archive, "T-not-safe.md", source(thread_id="not-safe"))
            sessions = root / "sessions"
            fixture = sessions / "demo-amp.json"
            fixture.parent.mkdir(parents=True)
            fixture.write_text(json.dumps({"id": "demo-amp"}))
            result = amp.import_archive(archive, sessions, root / "assets")
            self.assertEqual(result["imported"], [THREAD_ID])
            self.assertEqual(len(result["rejected"]), 1)
            self.assertTrue(fixture.exists())
            self.assertTrue((sessions / f"{THREAD_ID}.json").exists())


if __name__ == "__main__":
    unittest.main()
