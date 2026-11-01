import json
from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from gewu_driver.author import BudgetedAuthor, Limits
from gewu_driver.prepare import Preparation
from gewu_driver.session import Session
from test_research_cli import ResearchFixture, BINARY


class PreparationDriverTests(ResearchFixture, unittest.TestCase):
    def test_binary_replacement_blocks_native_execution_and_author_calls(self):
        binary = self.root / ".gewu/fake-gewu"
        binary.write_text("#!/bin/sh\nexit 0\n")
        binary.chmod(0o755)
        session = Session(self.root, binary)
        calls = []
        author = BudgetedAuthor(self.root / ".gewu/author", Limits(), {"session": session.identity},
                                lambda request: calls.append(request))
        Preparation(session, author)
        binary.write_text("#!/bin/sh\nexit 99\n")
        with self.assertRaisesRegex(ValueError, "binary changed"):
            session.native("harness", "status")
        with self.assertRaisesRegex(ValueError, "binary changed"):
            author.call("plan", {"small": "request"})
        self.assertEqual(calls, [])

    def test_manifest_drift_blocks_preparation_before_paid_calls(self):
        session = Session(self.root, BINARY)
        calls = []
        author = BudgetedAuthor(self.root / ".gewu/author", Limits(), {"session": session.identity},
                                lambda request: calls.append(request))
        driver = Preparation(session, author)
        manifest = session.manifest.copy()
        manifest["schema_version"] = 999
        session.write("review-input.json", manifest)
        with self.assertRaisesRegex(ValueError, "manifest changed"):
            driver.local_plans()
        self.assertEqual(calls, [])

    def test_scripted_end_to_end_preparation_and_cached_planning_resume(self):
        session = Session(self.root, BINARY)
        calls = []
        def provider(payload):
            value = json.loads(payload)
            calls.append(value["stage"])
            request = value["request"]
            if value["stage"] == "investigate":
                page = request["page"]
                artifact = {"schema_version": 1, "source": page["source"], "request_id": request["id"],
                            "findings": [{"summary": "A fixture module declares a constant.",
                                          "evidence": [{"offset": page["offset"], "text": page["text"]}], "questions": []}]}
            elif value["stage"] == "synthesize":
                claim = {"text": "Two fixture modules declare constants.", "findings": [0, 1]}
                artifact = {"schema_version": 1, "source": request["source"], "ledger_sha256": request["ledger_sha256"],
                            "intent": claim, "changes": [claim], "concepts": []}
            else:
                change = request["scope"][0]
                artifact = {"schema_version": 1, "source": request["source"], "items": [{"id": "constant", "title": "Fixture constant",
                            "purpose": "Inspect the value", "covers": [{"change_id": change["id"], "start": 1, "end": change["body_lines"]}],
                            "expanded": False}]}
            return {"artifact": artifact, "usage": {"input_tokens": 10, "output_tokens": 10}}
        identity = {"session": session.identity, "provider": "scripted"}
        author = BudgetedAuthor(self.root / ".gewu/author", Limits(), identity, provider)
        driver = Preparation(session, author)
        driver.investigate()
        self.assertEqual(session.stage(), "plan")
        plans = driver.local_plans()
        self.assertEqual(len(plans), 2)
        self.assertEqual(calls, ["investigate", "investigate", "synthesize", "plan", "plan"])
        resumed = Preparation(Session(self.root, BINARY), BudgetedAuthor(self.root / ".gewu/author", Limits(), identity, provider))
        self.assertEqual(resumed.local_plans(), plans)
        self.assertEqual(len(calls), 5)
        self.assertEqual(session.stage(), "plan")
        self.assertFalse(list((self.root / ".gewu").glob("reviews/**/events.jsonl")))
