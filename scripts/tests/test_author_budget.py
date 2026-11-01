import json
import hashlib
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from gewu_driver.author import BudgetedAuthor, Limits
from gewu_driver.reconcile import reconcile


class AuthorBudgetTests(unittest.TestCase):
    def test_provider_instruction_overhead_is_charged_before_inference(self):
        class Provider:
            request_overhead_bytes = 101
            def __call__(self, request):
                raise AssertionError("must fail before inference")
        with tempfile.TemporaryDirectory() as root:
            author = BudgetedAuthor(root, Limits(request_bytes=100), {}, Provider())
            with self.assertRaisesRegex(ValueError, "request bytes"):
                author.call("plan", {})

    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        self.calls = []

    def fake(self, request):
        self.calls.append(request)
        return {"artifact": {"answer": "accepted"}, "usage": {"input_tokens": 20, "output_tokens": 5}}

    def author(self, **limits):
        return BudgetedAuthor(self.root, Limits(**limits), {"backend": "fixture", "protocol": "v1"}, self.fake)

    def test_oversized_complete_request_never_reaches_provider(self):
        author = self.author(request_bytes=100)
        with self.assertRaisesRegex(ValueError, "request bytes"):
            author.call("plan", {"instructions": "x" * 101, "source": "small"})
        self.assertEqual(self.calls, [])

    def test_durable_cache_reuses_only_identical_requests(self):
        first = self.author().call("plan", {"source": "frozen", "instructions": "plan"})
        second = self.author().call("plan", {"source": "frozen", "instructions": "plan"})
        self.assertEqual(first, second)
        self.assertEqual(len(self.calls), 1)
        self.author().call("plan", {"source": "changed", "instructions": "plan"})
        self.assertEqual(len(self.calls), 2)

    def test_call_and_byte_reservations_persist_across_restart(self):
        author = self.author(max_calls=1)
        author.call("plan", {"small": 1})
        with self.assertRaisesRegex(ValueError, "call budget"):
            self.author(max_calls=1).call("plan", {"small": 2})
        self.assertEqual(len(self.calls), 1)

    def test_stage_budget_applies_to_repairs_as_well(self):
        author = self.author(stage_calls={"repair": 1})
        author.call("repair", {"draft": 1})
        with self.assertRaisesRegex(ValueError, "stage call budget"):
            author.call("repair", {"draft": 2})
        self.assertEqual(len(self.calls), 1)

    def test_oversized_response_is_not_cached_and_failed_call_is_still_charged(self):
        def large(request):
            self.calls.append(request)
            return {"artifact": {"large": "x" * 1000}, "usage": {}}
        author = BudgetedAuthor(self.root, Limits(response_bytes=100, max_calls=1), {"backend": "fixture"}, large)
        with self.assertRaisesRegex(ValueError, "response bytes"):
            author.call("plan", {"small": 1})
        with self.assertRaisesRegex(ValueError, "failed|budget"):
            author.call("plan", {"small": 1})
        self.assertEqual(len(self.calls), 1)

    def test_pending_outcome_is_not_automatically_retried(self):
        class Interrupted(BaseException):
            pass
        def interrupted(request):
            self.calls.append(request)
            raise Interrupted()
        author = BudgetedAuthor(self.root, Limits(), {"backend": "fixture"}, interrupted)
        with self.assertRaises(Interrupted):
            author.call("plan", {"small": 1})
        with self.assertRaisesRegex(ValueError, "pending"):
            author.call("plan", {"small": 1})
        self.assertEqual(len(self.calls), 1)

    def test_configuration_drift_and_cache_tampering_fail_closed(self):
        self.author().call("plan", {"small": 1})
        with self.assertRaisesRegex(ValueError, "configuration"):
            self.author(max_calls=99).call("plan", {"small": 1})
        result = next((self.root / "calls").glob("*/reply.json"))
        result.write_text(json.dumps({"artifact": {"answer": "tampered"}, "usage": {}}))
        with self.assertRaisesRegex(ValueError, "hash"):
            self.author().call("plan", {"small": 1})
        self.assertEqual(len(self.calls), 1)

    def test_tampered_reservation_cannot_buy_more_calls(self):
        author = self.author(response_bytes=100, total_reserved_bytes=200)
        author.call("plan", {"x": 1})
        path = self.root / "state.json"
        state = json.loads(path.read_bytes())
        state["reserved_bytes"] = 0
        path.write_text(json.dumps(state))
        with self.assertRaisesRegex(ValueError, "accounting"):
            author.call("plan", {"x": 2})
        self.assertEqual(len(self.calls), 1)

    def test_crash_after_reply_save_keeps_pending_and_blocks_a_different_request(self):
        from gewu_driver import author as module
        real_save = module.save
        class Interrupted(BaseException):
            pass
        def crash(path, value):
            if Path(path).name == "state.json" and value["calls"][-1]["status"] == "succeeded":
                raise Interrupted()
            real_save(path, value)
        author = self.author()
        with patch.object(module, "save", crash), self.assertRaises(Interrupted):
            author.call("plan", {"x": 1})
        self.assertTrue(list((self.root / "calls").glob("*/reply.json")))
        with self.assertRaisesRegex(ValueError, "pending"):
            self.author().call("plan", {"different": 2})
        self.assertEqual(len(self.calls), 1)
        state = json.loads((self.root / "state.json").read_bytes())
        key = state["calls"][0]["key"]
        reserved = state["reserved_bytes"]
        with self.assertRaisesRegex(ValueError, "inspected hash"):
            reconcile(author, key, "Fixture verifies retained output", "wrong")
        raw = (self.root / "calls/0000/reply.json").read_bytes()
        reconcile(author, key, "Fixture verifies retained output", hashlib.sha256(raw).hexdigest())
        self.assertEqual(self.author().call("plan", {"x": 1}), {"answer": "accepted"})
        self.assertEqual(len(self.calls), 1)
        state = json.loads((self.root / "state.json").read_bytes())
        self.assertEqual(state["reserved_bytes"], reserved)
        self.assertIn("reconciliation", state["calls"][0])

    def test_retiring_pending_call_preserves_charge_and_blocks_same_request(self):
        class Interrupted(BaseException):
            pass
        def interrupted(request):
            raise Interrupted()
        author = BudgetedAuthor(self.root, Limits(max_calls=2), {"backend": "fixture"}, interrupted)
        with self.assertRaises(Interrupted):
            author.call("plan", {"x": 1})
        state = json.loads((self.root / "state.json").read_bytes())
        key = state["calls"][0]["key"]
        reconcile(author, key, "Fixture confirms no recoverable output")
        with self.assertRaisesRegex(ValueError, "failed"):
            author.call("plan", {"x": 1})
        author.invoke = self.fake
        author.call("plan", {"x": 2})
        with self.assertRaisesRegex(ValueError, "budget"):
            author.call("plan", {"x": 3})
        self.assertEqual(len(self.calls), 1)
