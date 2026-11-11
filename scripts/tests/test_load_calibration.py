"""The load calibration tool, offline: its arithmetic, and `eval` end to end on
synthetic labels with the real binary's rubric. No test calls Jev."""

import contextlib
import io
import json
from pathlib import Path
import sys
import tempfile
import types
import unittest

from test_research_cli import BINARY

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "load_calibration"))

import build  # noqa: E402
import evaluate  # noqa: E402
import stats  # noqa: E402


class StatsTest(unittest.TestCase):
    def test_auc_counts_ties_half(self):
        self.assertEqual(stats.auc([0.9, 0.1], [True, False]), 1.0)
        self.assertEqual(stats.auc([0.5, 0.5], [True, False]), 0.5)
        self.assertIsNone(stats.auc([0.5], [True]))

    def test_enumeration_matches_the_rust_equation(self):
        # Two coin flips at weights 0.6 and 0.5: only both together cross θ.
        self.assertAlmostEqual(stats.p_overloaded([0.5, 0.5], [0.6, 0.5], 0.0, 1.0), 0.25)
        self.assertEqual(stats.p_overloaded([0.0], [0.6], 1.0, 1.0), 1.0)

    def test_weights_follow_the_fitted_boundary(self):
        self.assertEqual(stats.weights_from(-2.0, [1.0, -1.0]), [0.5, 0.0])
        self.assertIsNone(stats.weights_from(0.5, [1.0]))

    def test_the_fit_separates_a_clear_signal(self):
        beta0, beta = stats.fit_logistic([[0.9], [0.8], [0.1], [0.2]] * 5,
                                         [1.0, 1.0, 0.0, 0.0] * 5, steps=3000)
        self.assertGreater(beta[0], 0)
        self.assertLess(beta0, 0)

    def test_allocation_is_proportional_with_a_floor(self):
        share = stats.allocate({0: 182, 1: 47, 5: 18}, 40, 6)
        self.assertEqual(sum(share.values()), 40)
        self.assertGreaterEqual(min(share.values()), 6)
        self.assertGreater(share[0], share[1])

    def test_the_cutoff_respects_the_false_flag_limit(self):
        seconds = [100, 150, 200, 300, 400, 500]
        labels = [False, False, False, True, True, True]
        best, _ = stats.cutoff(seconds, labels, 0.0)
        self.assertGreaterEqual(best[0], 200)
        self.assertLess(best[0], 300)


class EvalTest(unittest.TestCase):
    def setUp(self):
        if not BINARY.exists():
            self.skipTest("build gewu first")
        self.rubric = build.read_rubric(str(BINARY), None)

    def test_labelling_comes_from_the_rubric(self):
        text = build.labelling(self.rubric)
        for question in self.rubric["question"]:
            self.assertIn(f"`{question['name']}`", text)
        self.assertIn(self.rubric["guidance"]["overloaded"].strip()[:40], text)

    def write(self, out, version):
        names = [q["name"] for q in self.rubric["question"]]
        items, jev, labels = [], [], []
        for n in range(40):
            heavy = n % 3 == 0
            item = f"w{n % 2}-{n:03d}"
            items.append({"item": item, "walkthrough": n % 2, "questions": names})
            jev.append({"item": item, "reading_seconds": 400 if heavy else 120,
                        "overloaded": 0.9 if heavy else 0.1,
                        "answers": {name: 0.8 if heavy else 0.2 for name in names}})
            labels.append({"item": item, "overloaded": heavy, "reason": "synthetic",
                           "answers": {name: {"yes": heavy, "reason": "synthetic"}
                                       for name in names}})
        (out / "batches" / "b0").mkdir(parents=True)
        for name, rows in [("items.jsonl", items), ("jev.jsonl", jev),
                           ("batches/b0/labels.jsonl", labels)]:
            (out / name).write_text("".join(json.dumps(row) + "\n" for row in rows))
        (out / "manifest.json").write_text(json.dumps({
            "rubric_version": version, "seed": 1, "jev_usd": 0.001,
            "walkthroughs": [{"index": 0, "path": "a", "held_out": False},
                             {"index": 1, "path": "b", "held_out": True}]}))

    def run_eval(self, out):
        args = types.SimpleNamespace(gewu=str(BINARY), rubric=None, out=str(out))
        with contextlib.redirect_stdout(io.StringIO()):
            evaluate.evaluate(args)

    def test_eval_checks_the_bars_and_proposes_without_editing_the_rubric(self):
        with tempfile.TemporaryDirectory() as directory:
            out = Path(directory)
            self.write(out, self.rubric["version"])
            self.run_eval(out)
            results = (out / "results.md").read_text()
            proposal = (out / "proposal.md").read_text()
        self.assertIn("Held out", results)
        self.assertIn("| Jev ≤ $", results)
        self.assertIn("[budget]\nchunk_seconds = ", proposal)
        self.assertIn("Apply by hand", proposal)

    def test_eval_refuses_items_scored_with_another_rubric(self):
        with tempfile.TemporaryDirectory() as directory:
            out = Path(directory)
            self.write(out, self.rubric["version"] + 1)
            with self.assertRaises(SystemExit):
                self.run_eval(out)


if __name__ == "__main__":
    unittest.main()
