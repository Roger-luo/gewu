import json
from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from gewu_driver.author import BudgetedAuthor, Limits
from gewu_driver.expand import Expansion
from gewu_driver.prepare import Preparation
from gewu_driver.session import Session
from gewu_driver.storage import encode
from test_research_cli import ResearchFixture, BINARY
from walkthrough_fixture import ScriptedAuthor


class WalkthroughDriverTests(ResearchFixture, unittest.TestCase):
    def prepare(self, mode="valid"):
        session = Session(self.root, BINARY)
        provider = ScriptedAuthor(mode)
        author = BudgetedAuthor(self.root / ".gewu/author", Limits(), {"session": session.identity, "provider": mode}, provider)
        preparation = Preparation(session, author)
        preparation.investigate()
        merged = preparation.plan()
        self.assertEqual(len(merged["items"]), 2)
        self.assertEqual(session.stage(), "expand")
        return session, provider, author

    def mark_fixture_batch_read(self, session):
        spec = session.read("review-spec.json")
        path = session.root / "reviews" / spec["review_id"] / "events.jsonl"
        path.parent.mkdir(parents=True, exist_ok=True)
        with path.open("a") as handle:
            for chunk in spec["chunks"]:
                handle.write(json.dumps({"at_unix_ms": 0, "type": "review_changed", "chunk_id": chunk["id"], "reviewed": True}) + "\n")
        session.native("harness", "next")

    def test_full_scripted_walkthrough_measures_repairs_and_respects_human_gate(self):
        session, provider, author = self.prepare("repair")
        driver = Expansion(session, author)
        first = driver.next_batch()
        self.assertEqual(session.stage(), "await")
        self.assertLessEqual(len(first["chunks"][0]["summary"]), 500)
        self.assertTrue(any(call["stage"] == "repair" and not call["request"]["measurement"]["valid"] for call in provider.requests))
        initial_guide = next(call["request"]["instructions"] for call in provider.requests if "source_pages" in call["request"])
        self.assertTrue(all(call["request"]["instructions"].startswith(initial_guide)
                            for call in provider.requests if "measurement" in call["request"]))
        count = len(provider.requests)
        with self.assertRaisesRegex(ValueError, "human gates"):
            driver.next_batch()
        self.assertEqual(len(provider.requests), count)
        self.assertFalse(list(session.root.glob("reviews/**/events.jsonl")))
        session.native("harness", "next")
        self.assertEqual(session.stage(), "await")
        self.mark_fixture_batch_read(session)
        driver.next_batch()
        self.assertEqual(session.stage(), "await")
        self.mark_fixture_batch_read(session)
        self.assertEqual(session.stage(), "complete")
        self.assertIn("2 of 2 changes covered", session.native("review", "validate"))
        chunks = session.read("review-spec.json")["chunks"]
        self.assertEqual(len({chunk["id"] for chunk in chunks}), 2)
        for call in provider.requests:
            self.assertLessEqual(len(encode(call)), author.config["limits"]["request_bytes"])
            if "source_pages" in call["request"]:
                owned = {q["change_id"] for q in call["request"]["item"]["covers"]}
                self.assertTrue(all(page["quote"]["change_id"] in owned for page in call["request"]["source_pages"]))

    def test_unmeasured_submit_is_rejected_without_progress(self):
        session, provider, author = self.prepare("premature")
        with self.assertRaisesRegex(ValueError, "valid measured draft"):
            Expansion(session, author).next_batch()
        self.assertEqual(session.stage(), "expand")
        self.assertFalse((session.root / "review-spec.json").exists())

    def test_source_lookup_cannot_escape_the_pending_work_item(self):
        session, _, author = self.prepare()
        items = session.read("harness/worklist.json")["items"]
        with self.assertRaisesRegex(ValueError, "escapes"):
            Expansion(session, author)._read(items[0], items[1]["covers"][0], None, 4096)

    def test_invalid_lookup_is_repaired_without_expanding_source_scope(self):
        session, provider, author = self.prepare("bad-read")
        Expansion(session, author).next_batch()
        repairs = [call for call in provider.requests if "lookup_error" in call["request"]]
        self.assertEqual(len(repairs), 1)
        self.assertEqual(repairs[0]["stage"], "repair")
        self.assertIn("escapes", repairs[0]["request"]["lookup_error"])
        self.assertEqual(session.stage(), "await")

    def test_dependency_evidence_survives_measurement_without_changing_quote_scope(self):
        session, provider, author = self.prepare("dependency-first")
        result = Expansion(session, author).next_batch()
        measured = next(call["request"] for call in provider.requests if "measurement" in call["request"])
        dependency = measured["lookups"][0]["result"]
        owned = {q["change_id"] for q in measured["item"]["covers"]}
        self.assertNotIn(dependency["quote"]["change_id"], owned)
        self.assertEqual(dependency["source"], session.manifest["source"])
        self.assertTrue(all(quote["change_id"] in owned for chunk in result["chunks"]
                            for block in chunk["content"] for quote in block.get("quotes", [])))
        from gewu_driver.lookup import lookup
        matches = lookup(session, "search", {"text": "VALUE"})["result"]["matches"]
        self.assertEqual({hit["path"] for hit in matches}, {"a.rs", "b.rs"})

    def test_measured_draft_resume_reuses_calls(self):
        session, provider, author = self.prepare()
        with self.assertRaisesRegex(ValueError, "turn budget"):
            Expansion(session, author).next_batch(max_turns=1)
        self.assertEqual(session.stage(), "expand")
        count = len(provider.requests)
        restarted = Session(self.root, BINARY)
        resumed = BudgetedAuthor(self.root / ".gewu/author", Limits(),
                                 {"session": restarted.identity, "provider": "valid"}, provider)
        Expansion(restarted, resumed).next_batch()
        self.assertEqual(restarted.stage(), "await")
        self.assertEqual(len(provider.requests), count + 1)

    def test_prefetch_is_bounded_source_bound_and_does_not_grant_coverage(self):
        from gewu_driver.prerequisites import prefetch
        session, provider, author = self.prepare()
        item = session.read("harness/worklist.json")["items"][0]
        owned = {q["change_id"] for q in item["covers"]}
        for budget in (128, 512, 1024, 4096):
            result = prefetch(session, item, budget)
            self.assertLessEqual(len(encode(result)), budget)
            self.assertEqual(result, prefetch(Session(self.root, BINARY), item, budget))
            for candidate in result["candidates"]:
                self.assertEqual(candidate["result"]["source"], session.manifest["source"])
                self.assertNotIn(candidate["result"]["quote"]["change_id"], owned)
        self.assertTrue(result["candidates"])
        with self.assertRaisesRegex(ValueError, "envelope"):
            prefetch(session, item, 1)
        draft = Expansion(session, author).next_batch()
        measured = next(call["request"] for call in provider.requests if "measurement" in call["request"])
        self.assertEqual(measured["prerequisites"], result)
        self.assertTrue(all(q["change_id"] in owned for chunk in draft["chunks"]
                            for block in chunk["content"] for q in block.get("quotes", [])))

    def test_source_edit_rechecks_only_unchanged_local_notes_and_rebuilds_overview(self):
        session, _, author = self.prepare()
        Expansion(session, author).next_batch()
        (self.root / "b.rs").write_text("const VALUE: u32 = 2;\n")
        session.native("harness", "next")
        self.assertEqual(session.stage(), "investigate")
        archive = next((session.root / "harness/research-archive").iterdir())
        self.assertTrue((archive / "review-input.json").exists())
        restarted = Session(self.root, BINARY)
        provider = ScriptedAuthor()
        author = BudgetedAuthor(self.root / ".gewu/author-replanned", Limits(),
                                {"session": restarted.identity, "provider": "replanned"}, provider)
        Preparation(restarted, author).investigate(previous_archive=archive)
        investigations = [call["request"] for call in provider.requests if call["stage"] == "investigate"]
        self.assertEqual(len(investigations), 2)
        reused = [request for request in investigations if "reuse_candidate" in request]
        self.assertEqual(len(reused), 1)
        self.assertEqual(reused[0]["page"]["path"], "a.rs")
        self.assertEqual(sum(call["stage"] == "synthesize" for call in provider.requests), 1)
        self.assertEqual(restarted.stage(), "plan")
        self.assertFalse(list(session.root.glob("reviews/**/events.jsonl")))

    def test_replan_preserves_surviving_chunk_and_completes_without_reexpansion(self):
        session, _, author = self.prepare()
        Expansion(session, author).next_batch()
        surviving = session.read("review-spec.json")["chunks"][0]
        surviving_id = surviving["content"][0]["quotes"][0]["change_id"]
        edited = next(change["path"] for change in session.manifest["changes"] if change["id"] != surviving_id)
        (self.root / edited).write_text("const VALUE: u32 = 2;\n")
        session.native("harness", "next")
        self.assertEqual(session.read("review-spec.json")["chunks"], [surviving])
        archive = next((session.root / "harness/research-archive").iterdir())
        restarted = Session(self.root, BINARY)
        provider = ScriptedAuthor()
        author = BudgetedAuthor(self.root / ".gewu/author-replanned", Limits(),
                                {"session": restarted.identity, "provider": "replanned"}, provider)
        preparation = Preparation(restarted, author)
        preparation.investigate(previous_archive=archive)
        work = preparation.plan()
        self.assertEqual(sum(item["expanded"] for item in work["items"]), 1)
        self.assertEqual(sum("scope" in call["request"] for call in provider.requests), 1)
        Expansion(restarted, author).next_batch()
        self.assertEqual(restarted.stage(), "await")
        chunks = restarted.read("review-spec.json")["chunks"]
        self.assertEqual(len(chunks), 2)
        self.assertEqual(chunks[0], surviving)
        self.mark_fixture_batch_read(restarted)
        self.assertEqual(restarted.stage(), "complete")
        self.assertIn("2 of 2 changes covered", restarted.native("review", "validate"))

    def test_all_retained_work_still_waits_for_human_review(self):
        session, _, author = self.prepare()
        Expansion(session, author).next_batch()
        surviving = session.read("review-spec.json")["chunks"][0]
        surviving_id = surviving["content"][0]["quotes"][0]["change_id"]
        removed = next(change["path"] for change in session.manifest["changes"] if change["id"] != surviving_id)
        (self.root / removed).unlink()
        session.native("harness", "next")
        self.assertEqual(session.stage(), "investigate")
        archive = next((session.root / "harness/research-archive").iterdir())
        restarted = Session(self.root, BINARY)
        provider = ScriptedAuthor()
        author = BudgetedAuthor(self.root / ".gewu/author-replanned", Limits(),
                                {"session": restarted.identity, "provider": "replanned"}, provider)
        preparation = Preparation(restarted, author)
        preparation.investigate(previous_archive=archive)
        work = preparation.plan()
        self.assertTrue(all(item["expanded"] for item in work["items"]))
        calls = len(provider.requests)
        self.assertIsNone(Expansion(restarted, author).next_batch())
        self.assertEqual(len(provider.requests), calls)
        self.assertEqual(restarted.stage(), "await")
        self.assertFalse(list(session.root.glob("reviews/**/events.jsonl")))
        self.mark_fixture_batch_read(restarted)
        self.assertEqual(restarted.stage(), "complete")
