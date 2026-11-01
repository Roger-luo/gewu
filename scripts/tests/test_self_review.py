"""The self-review loop end to end, with scripted author and judge responses."""
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from gewu_driver.author import BudgetedAuthor, Limits
from gewu_driver.expand import Expansion
from gewu_driver.prepare import Preparation
from gewu_driver.session import Session
from self_review import tui
from self_review.corpus import Run, freeze
from self_review.ledger import Ledger
from self_review.score import DIMENSIONS, cold_pack, score
from self_review.sentences import split, units
from test_research_cli import BINARY
from walkthrough_fixture import ScriptedAuthor


def judge_all(numbered, **overrides):
    labels = [{"id": unit["id"], "label": "fact"} for unit in numbered]
    for identifier, label in overrides.items():
        labels[int(identifier[1:]) - 1] = {"id": identifier, "label": label, "reason": "repeats s1"}
    evidence = {"score": 3, "evidence": ["every chunk"]}
    questions = [{"id": f"q{index + 1}", "kind": kind, "question": f"Question {index + 1}?",
                  "options": ["a", "b", "c", "d"], "answer": 1}
                 for index, kind in enumerate(["purpose", "purpose", "connection", "connection",
                                               "behaviour", "behaviour"])]
    return {"rubric_version": 3, "labels": labels, "findings": [], "questions": questions,
            "dimensions": {name: evidence for name in DIMENSIONS}}


class SelfReviewTests(unittest.TestCase):
    def setUp(self):
        directory = tempfile.TemporaryDirectory(prefix="gewu-self-review-")
        self.addCleanup(directory.cleanup)
        self.source = Path(directory.name) / "gewu"
        self.source.mkdir()
        for args in (["init", "-q"], ["config", "user.email", "fixture@example.invalid"],
                     ["config", "user.name", "Fixture"]):
            self.git(*args)
        for name in ("a.rs", "b.rs"):
            (self.source / name).write_text("const VALUE: u32 = 1;\n")
        (self.source / "docs.md").write_text("Out of scope.\n")
        self.git("add", ".")
        self.git("commit", "-qm", "test: fixture")
        corpus = {"name": "fixture", "base": "empty", "head": "HEAD", "scope": ["a.rs", "b.rs"]}
        self.destination = self.source / ".gewu/corpus"
        freeze(self.source, corpus, self.destination, BINARY)
        self.run_ = Run(self.destination)

    def git(self, *args):
        subprocess.run(["git", *args], cwd=self.source, check=True, capture_output=True, timeout=30)

    def generate(self):
        session = Session(self.destination, self.run_.binary)
        author = BudgetedAuthor(session.root / "author", Limits(), {"session": session.identity}, ScriptedAuthor())
        Preparation(session, author).investigate()
        Preparation(session, author).plan()
        while session.stage() != "complete":
            Expansion(session, author).next_batch()
            self.run_.advance()

    def test_freeze_isolates_scope_and_binary_from_later_edits(self):
        tracked = subprocess.run(["git", "ls-files"], cwd=self.destination, capture_output=True,
                                 text=True, check=True).stdout.split()
        # The whole tree is there to read; only the scope is in the diff.
        self.assertEqual(tracked, ["a.rs", "b.rs", "docs.md"])
        manifest = json.loads((self.destination / ".gewu/review-input.json").read_text())
        self.assertEqual(sorted({change["path"] for change in manifest["changes"]}), ["a.rs", "b.rs"])
        self.assertNotEqual(self.run_.binary.resolve(), BINARY.resolve())
        (self.source / "a.rs").write_text("const VALUE: u32 = 2;\n")
        self.assertIn("VALUE: u32 = 1", (self.destination / "a.rs").read_text())
        with self.assertRaises(ValueError):
            freeze(self.source, {"name": "x", "base": "empty", "head": "HEAD"}, self.source / "inside", BINARY)

    def test_generated_review_is_audited_and_judge_output_is_checked(self):
        self.generate()
        audit = self.run_.audit()
        self.assertEqual(audit["coverage"]["missing_paths"], [])
        numbered = units(self.run_.spec())
        self.assertTrue(numbered)
        # The scripted chunks carry a summary but no narration, so the audit
        # must report every line as shown but unexplained.
        result = score("fixture", numbered, judge_all(numbered), audit)
        self.assertFalse(result["gates"]["audit_coverage"])
        self.assertEqual(sorted(f["location"] for f in result["findings"] if f["category"] == "unexplained"),
                         ["a.rs", "b.rs"])
        with self.assertRaisesRegex(ValueError, "unlabelled sentences"):
            score("fixture", numbered, {**judge_all(numbered), "labels": []}, audit)

    def test_a_fixed_finding_is_verified_only_when_the_next_run_lacks_it(self):
        numbered = [{"id": "s1", "location": "c", "field": "summary", "text": "Declares `VALUE`."},
                    {"id": "s2", "location": "c", "field": "content[0]", "text": "It declares `VALUE`."}]
        audit = {"coverage": {"missing_paths": [], "changes": []},
                 "references": {"unresolved": [], "forward": [], "distant": []}}
        ledger = Ledger(self.source / ".gewu/self-review/ledger.json")
        meta = self.run_.meta
        redundant = score("fixture", numbered, judge_all(numbered, s2="redundant"), audit)
        self.assertFalse(redundant["gates"]["minimality"])
        ledger.record(meta, redundant)
        [finding] = ledger.unresolved()
        with self.assertRaises(ValueError):
            ledger.resolve(finding["key"], "fixed", " ")
        ledger.resolve(finding["key"], "fixed", "expand prompt: test_expand_drops_restatement")
        self.assertEqual(ledger.unresolved(), [])
        clean = score("fixture", numbered, judge_all(numbered), audit)
        delta = ledger.record(meta, clean)["delta"]
        self.assertEqual(delta["labels"]["redundant"], -1)
        self.assertEqual(ledger.data["findings"][finding["key"]]["status"], "verified")
        ledger.record(meta, redundant)
        self.assertEqual([entry["status"] for entry in ledger.unresolved()], ["regressed"])

    def test_terminal_ui_is_gated_by_frames_and_unassessed_without_a_judge(self):
        self.generate()
        summary = tui.capture(self.destination, self.run_.binary)
        self.assertEqual(set(summary), {"80x24", "120x40"})
        self.assertTrue(all(size["passes"] for size in summary.values()), summary)
        self.assertTrue((self.run_.evidence / "frames-80x24").is_dir())
        frames = {size: self.run_.read(f"frames-{size}.json") for size in summary}
        numbered = units(self.run_.spec())
        audit = self.run_.audit()
        gates_only = score("fixture", numbered, judge_all(numbered), audit, frames=frames)
        self.assertEqual(gates_only["tui"]["status"], "gates-only")
        self.assertTrue(gates_only["gates"]["tui_reach_80x24"])
        self.assertFalse(gates_only["passes"])
        evidence = {"score": 3, "evidence": ["001-...-p1.txt"]}
        degraded = {"score": 1, "evidence": ["80x24 001-...-p2.txt"]}
        tui_judge = {"rubric_version": 3, "dimensions": {name: evidence for name in tui.DIMENSIONS},
                     "degraded": {name: degraded for name in tui.DIMENSIONS},
                     "findings": [{"stage": "x", "size": "80x24", "page": 1, "category": "page-break",
                                   "detail": "breaks mid sentence"}]}
        judged = score("fixture", numbered, judge_all(numbered), audit, frames=frames, tui_judge=tui_judge)
        self.assertEqual(judged["tui"]["status"], "assessed")
        # 80x24 is recorded, not gated: low degraded scores do not fail.
        self.assertEqual(judged["tui"]["dimensions"]["degraded_density"], 1)
        self.assertTrue(judged["gates"]["tui_density"])
        self.assertIn("page-break", [finding["category"] for finding in judged["findings"]])
        with self.assertRaisesRegex(ValueError, "tui dimensions density"):
            del tui_judge["dimensions"]["density"]
            score("fixture", numbered, judge_all(numbered), audit, frames=frames, tui_judge=tui_judge)

    def test_numbered_source_uses_the_numbers_quotes_use(self):
        # Self-review authors decoded escaped patches by hand to find them.
        change = json.loads((self.destination / ".gewu/review-input.json").read_text())["changes"][0]
        printed = subprocess.run(
            [str(self.run_.binary), "harness", "context", "source", "--change-id", change["id"],
             "--start", "1", "--end", str(change["body_lines"]), "--numbered"],
            cwd=self.destination, capture_output=True, text=True, check=True).stdout
        self.assertEqual(printed, "   1     1 │+const VALUE: u32 = 1;\n")

    def test_an_empty_range_is_refused_with_a_way_forward(self):
        # A reviewer opened an empty walkthrough after `--base <HEAD> --head HEAD`.
        (self.source / "a.rs").write_text("const VALUE: u32 = 3;\n")
        for args in (["harness", "start", "--base", "HEAD", "--head", "HEAD"],
                     ["review", "prepare", "--base", "HEAD", "--head", "HEAD"]):
            result = subprocess.run([str(BINARY), *args], cwd=self.source, capture_output=True, text=True)
            self.assertNotEqual(result.returncode, 0, args)
            self.assertIn("nothing to review", result.stderr)
            self.assertIn("--working-tree", result.stderr)

    def test_the_cold_reader_gates_on_questions_answered_from_the_pages_alone(self):
        numbered = [{"id": "s1", "location": "c", "field": "summary", "text": "Declares `VALUE`."}]
        audit = {"coverage": {"missing_paths": [], "changes": []},
                 "references": {"unresolved": [], "forward": [], "distant": []}}
        judge = judge_all(numbered)
        packed = cold_pack(judge)
        self.assertNotIn("answer", packed[0])
        right = {"answers": {q["id"]: 1 for q in packed}}
        self.assertTrue(score("fixture", numbered, judge, audit, reading=right)["gates"]["cold_reader"])
        two_wrong = {"answers": {**right["answers"], "q3": 0, "q4": 0}, "lost": ["chunk two"]}
        result = score("fixture", numbered, judge, audit, reading=two_wrong)
        self.assertFalse(result["gates"]["cold_reader"])
        self.assertEqual(result["cold_reader"]["accuracy"], 0.667)
        self.assertIn("missed-connection", [f["category"] for f in result["findings"]])
        self.assertFalse(score("fixture", numbered, judge, audit)["passes"], "unassessed is not a pass")
        with self.assertRaisesRegex(ValueError, "two each of purpose"):
            score("fixture", numbered, {**judge, "questions": judge["questions"][:5]}, audit)

    def test_sentences_skip_fences_and_split_bullets(self):
        markdown = "Adds `Quote`. It covers lines.\n\n```rust\nlet x = 1;\n```\n- First item.\n- Second."
        self.assertEqual(split(markdown), ["Adds `Quote`.", "It covers lines.", "First item.", "Second."])


if __name__ == "__main__":
    unittest.main()
