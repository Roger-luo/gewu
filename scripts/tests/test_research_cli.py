"""Offline CLI scenarios; all author responses are deterministic test fixtures."""
import json
from pathlib import Path
import subprocess
import tempfile
import unittest

BINARY = Path(__file__).resolve().parents[2] / "target/debug/gewu"


class ResearchFixture:
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory(prefix="gewu-research-")
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        self.git("init", "-q")
        self.git("config", "user.email", "fixture@example.invalid")
        self.git("config", "user.name", "Fixture")
        for name in ["a.rs", "b.rs"]:
            (self.root / name).write_text("const VALUE: u32 = 1;\n")
        self.git("add", ".")
        self.git("commit", "-qm", "test: fixture")
        empty = self.git("hash-object", "-w", "-t", "tree", "--stdin", stdin="").strip()
        self.cli("harness", "start", "--base", empty, "--working-tree")

    def git(self, *args, stdin=None):
        return subprocess.run(["git", *args], cwd=self.root, input=stdin, text=True,
                              capture_output=True, check=True, timeout=30).stdout

    def cli(self, *args, ok=True):
        result = subprocess.run([str(BINARY), *args], cwd=self.root, text=True,
                                capture_output=True, timeout=30)
        self.assertEqual(result.returncode == 0, ok, result.stderr + result.stdout)
        return result.stdout

    def read(self, name):
        return json.loads((self.root / ".gewu" / name).read_text())

    def save(self, name, value):
        path = self.root / ".gewu" / name
        path.write_text(json.dumps(value))
        return str(path)

    def investigate(self):
        self.cli("harness", "research", "start", "--page-bytes", "1024", "--request-bytes", "4096")
        while task := json.loads(self.cli("harness", "research", "next")):
            notes = {"schema_version": 1, "source": task["page"]["source"], "request_id": task["id"],
                     "findings": [{"summary": "A constant is declared in this fixture.",
                                   "evidence": [{"offset": task["page"]["offset"], "text": task["page"]["text"]}],
                                   "questions": []}]}
            path = self.save("notes.json", notes)
            self.cli("harness", "research", "accept", "--checkpoint", path)
            before = self.read("harness/research.json")
            self.cli("harness", "research", "accept", "--checkpoint", path, ok=False)
            self.assertEqual(before, self.read("harness/research.json"))
        page = json.loads(self.cli("harness", "research", "findings"))
        claim = {"text": "Two fixture modules declare constants.", "findings": [0, 1]}
        overview = {"schema_version": 1, "source": page["source"], "ledger_sha256": page["ledger_sha256"],
                    "intent": claim, "changes": [claim], "concepts": []}
        path = self.save("overview.json", overview)
        self.cli("harness", "research", "assemble", "--overview", path)


class ResearchCLI(ResearchFixture, unittest.TestCase):
    def test_missing_ledger_cannot_bypass_attached_research(self):
        self.investigate()
        self.assertTrue(self.read("harness/state.json")["research_required"])
        ledger = self.root / ".gewu/harness/research.json"
        ledger.rename(ledger.with_name("hidden.json"))
        self.assertIn("needs correction", self.cli("harness", "next"))
        self.assertEqual(self.read("harness/state.json")["stage"], "investigate")
        self.cli("harness", "research", "--ledger", "other.json", "start", ok=False)

    def test_source_edit_archives_evidence_and_allows_new_research(self):
        self.investigate()
        old = self.read("harness/research.json")
        self.assertIn("harness: plan", self.cli("harness", "next"))
        manifest = self.read("review-input.json")
        items = [{"id": f"item-{index}", "title": change["path"], "purpose": "Inspect fixture constant",
                  "covers": [{"change_id": change["id"], "start": 1, "end": change["body_lines"]}],
                  "expanded": False} for index, change in enumerate(manifest["changes"])]
        self.save("harness/worklist.json", {"schema_version": 1, "source": manifest["source"], "items": items})
        self.cli("harness", "next")
        chunk = {"id": "chunk-a", "title": "First fixture constant", "summary": "A constant is declared.",
                 "kind": "other", "content": [{"type": "changes", "quotes": items[0]["covers"], "presentation": "diff"}]}
        self.save("harness/expansion.json", {"schema_version": 1, "source": manifest["source"],
                  "item_id": items[0]["id"], "chunks": [chunk]})
        self.cli("harness", "next")
        self.assertEqual(self.read("harness/state.json")["stage"], "await")
        (self.root / "b.rs").write_text("const VALUE: u32 = 2;\n")
        continuation = self.cli("harness", "next")
        self.assertIn("research start", continuation)
        self.assertIn("research next", continuation)
        self.assertNotIn("Re-investigate only what is uncovered", continuation)
        state = self.read("harness/state.json")
        self.assertEqual(state["stage"], "investigate")
        self.assertTrue(state["research_required"])
        self.assertFalse((self.root / ".gewu/harness/research.json").exists())
        archives = list((self.root / ".gewu/harness/research-archive").glob("*/research.json"))
        self.assertEqual(len(archives), 1)
        self.assertEqual(json.loads(archives[0].read_text()), old)
        self.investigate()
        self.assertIn("harness: plan", self.cli("harness", "next"))
        self.assertFalse(list((self.root / ".gewu").glob("reviews/**/events.jsonl")))

    def test_raw_checkpoint_limit_precedes_parsing_and_leaves_state_unchanged(self):
        self.cli("harness", "research", "start")
        before = self.read("harness/research.json")
        path = self.root / ".gewu/large.json"
        path.write_text(" " * 5000 + "{}")
        self.cli("harness", "research", "accept", "--checkpoint", str(path), ok=False)
        self.assertEqual(before, self.read("harness/research.json"))


if __name__ == "__main__":
    unittest.main()
