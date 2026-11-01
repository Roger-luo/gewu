import json
from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from gewu_driver.author import BudgetedAuthor, Limits


class UsageGuardTests(unittest.TestCase):
    def test_separate_provider_cache_usage_counts_toward_input_stop(self):
        with tempfile.TemporaryDirectory() as directory:
            def provider(payload):
                return {"artifact": {}, "usage": {"input_tokens": 1, "output_tokens": 2,
                        "cache_read_input_tokens": 20, "cache_creation_input_tokens": 10}}
            author = BudgetedAuthor(directory, Limits(observed_input_tokens_stop=30), {}, provider)
            author.call("plan", {"first": True})
            with self.assertRaisesRegex(ValueError, "usage stop"):
                author.call("plan", {"second": True})

    def test_crossing_stops_next_inference_across_resume_but_allows_cache(self):
        for field in ("input_tokens", "output_tokens"):
            with self.subTest(field=field), tempfile.TemporaryDirectory() as directory:
                calls = []
                def provider(payload):
                    calls.append(payload)
                    return {"artifact": {"ok": True}, "usage": {"input_tokens": 30, "output_tokens": 10, "cached_input_tokens": 20}}
                limits = Limits(**{"observed_" + field + "_stop": 25 if field == "input_tokens" else 9})
                author = BudgetedAuthor(directory, limits, {}, provider)
                author.call("plan", {"first": True})
                resumed = BudgetedAuthor(directory, limits, {}, provider)
                self.assertEqual(resumed.call("plan", {"first": True}), {"ok": True})
                with self.assertRaisesRegex(ValueError, "usage stop"):
                    resumed.call("plan", {"second": True})
                self.assertEqual(len(calls), 1)

    def test_missing_invalid_or_failed_usage_blocks_more_inference(self):
        for usage in ({}, {"input_tokens": True, "output_tokens": 2}, {"input_tokens": -1, "output_tokens": 2}, None):
            with self.subTest(usage=usage), tempfile.TemporaryDirectory() as directory:
                calls = []
                def provider(payload):
                    calls.append(payload)
                    if usage is None:
                        raise ValueError("transport failed")
                    return {"artifact": {}, "usage": usage}
                author = BudgetedAuthor(directory, Limits(observed_input_tokens_stop=100), {}, provider)
                try:
                    author.call("plan", {"first": True})
                except ValueError:
                    pass
                with self.assertRaisesRegex(ValueError, "usage"):
                    author.call("plan", {"second": True})
                self.assertEqual(len(calls), 1)

    def test_cached_tokens_not_double_counted_and_record_tampering_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            def provider(payload):
                return {"artifact": {}, "usage": {"input_tokens": 30, "output_tokens": 2, "cached_input_tokens": 20}}
            author = BudgetedAuthor(directory, Limits(observed_input_tokens_stop=40, observed_input_tokens_warning=25), {}, provider)
            author.call("plan", {"first": True})
            with self.assertWarns(RuntimeWarning):
                author.call("plan", {"second": True})
            path = Path(directory) / "state.json"
            state = json.loads(path.read_text())
            state["calls"][0]["usage"]["input_tokens"] = 0
            path.write_text(json.dumps(state))
            with self.assertRaisesRegex(ValueError, "retained reply"):
                author.call("plan", {"third": True})
