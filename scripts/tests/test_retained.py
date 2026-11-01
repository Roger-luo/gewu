from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from gewu_driver.retained import uncovered, validate_remaining


class RetainedTests(unittest.TestCase):
    def test_partial_overlapping_and_adjacent_retained_spans_have_exact_complement(self):
        change = {"id": "a", "body_lines": 12}
        retained = [{"covers": [{"change_id": "a", "start": a, "end": b}]} for a, b in [(2, 4), (4, 6), (7, 8), (10, 10)]]
        self.assertEqual(uncovered(change, retained), [
            {"change_id": "a", "start": 1, "end": 1},
            {"change_id": "a", "start": 9, "end": 9},
            {"change_id": "a", "start": 11, "end": 12}])

    def test_partial_plan_combines_retained_scope_and_rejects_overlap(self):
        calls = []
        class Session:
            def validate_plan(self, plan, ids):
                calls.append((plan, ids))
        retained = [{"id": "old", "expanded": True, "covers": [
            {"change_id": "a", "start": 1, "end": 4},
            {"change_id": "b", "start": 1, "end": 9}]}]
        draft = {"items": [{"id": "new", "expanded": False, "covers": [{"change_id": "a", "start": 5, "end": 9}]}]}
        validate_remaining(Session(), draft, "a", retained)
        self.assertEqual(len(calls), 1)
        self.assertEqual(calls[0][0]["items"][0]["covers"], retained[0]["covers"][:1])
        self.assertEqual(len(retained[0]["covers"]), 2)
        draft["items"][0]["covers"][0]["start"] = 4
        with self.assertRaisesRegex(ValueError, "overlaps"):
            validate_remaining(Session(), draft, "a", retained)
        self.assertEqual(len(calls), 1)
