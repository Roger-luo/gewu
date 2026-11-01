import sys
from pathlib import Path
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from gewu_driver.author import BudgetedAuthor, Limits
from gewu_driver.evaluation import create, advance_synthetic_batch
from gewu_driver.expand import Expansion
from gewu_driver.prepare import Preparation
from gewu_driver.session import Session
from test_research_cli import ResearchFixture, BINARY
from walkthrough_fixture import ScriptedAuthor


class EvaluationTests(ResearchFixture, unittest.TestCase):
    def test_isolated_evaluation_completes_without_touching_source_review(self):
        destination = self.root / ".gewu/evaluation-copy"
        create(self.root, ["a.rs", "b.rs"], destination, BINARY)
        session = Session(destination, BINARY)
        author = BudgetedAuthor(session.root / "author", Limits(), {"session": session.identity}, ScriptedAuthor())
        preparation = Preparation(session, author)
        preparation.investigate()
        preparation.plan()
        while session.stage() != "complete":
            Expansion(session, author).next_batch()
            advance_synthetic_batch(session)
        self.assertIn("2 of 2 changes covered", session.native("review", "validate"))
        self.assertEqual(self.read("harness/state.json")["stage"], "investigate")
        self.assertFalse((self.root / ".gewu/reviews").exists())
        marker = session.read("evaluation-only.json")
        self.assertIn("not human review", marker["purpose"])
        (session.root / "evaluation-only.json").unlink()
        with self.assertRaises(FileNotFoundError):
            advance_synthetic_batch(session)
