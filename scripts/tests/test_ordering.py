import json
from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from gewu_driver.author import BudgetedAuthor, Limits
from gewu_driver.order import order_plans
from gewu_driver.expand import Expansion


class OrderingTests(unittest.TestCase):
    def test_wide_directory_is_ordered_in_bounded_groups_without_loss(self):
        requests = []
        def invoke(raw):
            requests.append(raw)
            request = json.loads(raw)["request"]
            return {"artifact": {"order": [unit["id"] for unit in reversed(request["units"])],
                                 "summary": "Fixture group of independent constants."}, "usage": {}}
        plans = [{"change_id": str(i), "path": f"file-{i}.rs", "plan": {"items": [
            {"id": "local", "title": "Constant", "purpose": "Defines an independent constant. " * 4}
        ]}} for i in range(30)]
        with tempfile.TemporaryDirectory() as root:
            author = BudgetedAuthor(root, Limits(request_bytes=4096), {"fixture": True}, invoke)
            result = order_plans(author, {}, {}, plans)
        self.assertEqual({item["id"] for item in result["items"]}, {str(i) + ":local" for i in range(30)})
        self.assertEqual(len(result["items"]), 30)
        self.assertGreater(len(requests), 1)
        self.assertTrue(all(len(request.encode()) <= 4096 for request in requests))

    def test_expansion_selects_overlapping_evidence_spans(self):
        item = {"covers": [{"change_id": "file", "start": 10, "end": 20}]}
        finding = {"change_ids": ["file"], "evidence_spans": [{"change_id": "file", "start": 21, "end": 100}]}
        self.assertFalse(Expansion._relevant(item, finding))
        finding["evidence_spans"][0]["start"] = 20
        self.assertTrue(Expansion._relevant(item, finding))
