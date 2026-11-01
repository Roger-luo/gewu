import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from gewu_driver.author import BudgetedAuthor, Limits
from gewu_driver.planning import plan_scope
from gewu_driver.reduce import compact
from gewu_driver.storage import encode
from test_research_cli import ResearchFixture, BINARY


class BoundedPlanningTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)

    def author(self, callback, **kwargs):
        return BudgetedAuthor(self.root, Limits(**kwargs), {"provider": "scripted"}, callback)

    def test_hierarchical_synthesis_preserves_refs_and_rejects_missing_ref(self):
        nodes = [{"text": "source " * 200, "references": [f"f-{i}"]} for i in range(6)]
        def compress(request):
            group = json.loads(request)["request"]["nodes"]
            refs = [ref for node in group for ref in node["references"]]
            return {"artifact": {"nodes": [{"text": "Concrete condensed fixture behavior.", "references": refs}]}, "usage": {}}
        author = self.author(compress, request_bytes=4096, response_bytes=1024)
        result = compact(author, {"head": "frozen"}, nodes, 600)
        self.assertLessEqual(len(encode(result)), 600)
        self.assertEqual({ref for node in result for ref in node["references"]}, {f"f-{i}" for i in range(6)})
        def omit(request):
            return {"artifact": {"nodes": [{"text": "Incomplete", "references": ["f-0"]}]}, "usage": {}}
        other = BudgetedAuthor(self.root / "negative", Limits(request_bytes=4096), {"provider": "omission"}, omit)
        with self.assertRaisesRegex(ValueError, "omitted or invented"):
            compact(other, {"head": "frozen"}, nodes, 600)

    def test_irreducible_node_fails_before_any_provider_call(self):
        calls = []
        author = self.author(lambda request: calls.append(request), request_bytes=1000)
        with self.assertRaisesRegex(ValueError, "one synthesis node"):
            compact(author, {}, [{"text": "x" * 3000, "references": ["f0"]}], 500)
        self.assertEqual(calls, [])

    def test_reference_set_that_cannot_fit_consumes_no_calls(self):
        calls = []
        author = self.author(lambda request: calls.append(request))
        nodes = [{"text": "source", "references": [f"finding-{i}" for i in range(20)]}]
        with self.assertRaisesRegex(ValueError, "references cannot fit"):
            compact(author, {}, nodes, 50)
        self.assertEqual(calls, [])


class NativePlanningTests(ResearchFixture, unittest.TestCase):
    def test_local_plan_cannot_quote_outside_scope_and_merge_requires_all_changes(self):
        manifest = self.read("review-input.json")
        first, second = manifest["changes"]
        source = manifest["source"]
        scope = [{"id": first["id"], "path": first["path"], "body_lines": first["body_lines"]}]
        def draft(change):
            return {"schema_version": 1, "source": source, "items": [{"id": "local", "title": "Fixture constant",
                    "purpose": "Inspect constant value", "covers": [{"change_id": change["id"], "start": 1,
                    "end": change["body_lines"]}], "expanded": False}]}
        calls = []
        def provider(request):
            calls.append(json.loads(request))
            return {"artifact": draft(second if len(calls) == 1 else first), "usage": {}}
        author = BudgetedAuthor(self.root / ".gewu/author", Limits(), {"provider": "fixture"}, provider)
        path = self.root / ".gewu/local-plan.json"
        def validate(plan):
            path.write_text(json.dumps(plan))
            result = subprocess.run([str(BINARY), "harness", "validate-plan", "--plan", str(path),
                                     "--change-id", first["id"]], cwd=self.root, capture_output=True, text=True)
            if result.returncode:
                raise ValueError(result.stderr)
        result = plan_scope(author, source, scope, [(0, {"summary": "A constant is declared.", "evidence": [first["path"]]})], validate)
        self.assertEqual(result, draft(first))
        self.assertEqual([call["stage"] for call in calls], ["plan", "repair"])
        self.cli("harness", "validate-plan", "--plan", str(path), ok=False)
        merged = draft(first)
        item = draft(second)["items"][0]
        item["id"] = "second"
        merged["items"].append(item)
        path.write_text(json.dumps(merged))
        self.cli("harness", "validate-plan", "--plan", str(path))
        self.assertEqual(self.read("harness/state.json")["stage"], "investigate")
