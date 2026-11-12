"""`gewu review generate` with stand-in agents: no model is called."""
import json
import os
from pathlib import Path
import stat
import subprocess
import tempfile
import unittest

from test_research_cli import BINARY

# A stand-in agent that follows the brief through the harness: it writes the
# first batch and stops at the proofread, or, started fresh, scores the batch
# — failing it once, then passing the rewrite.
AUTHOR = r'''#!/usr/bin/env python3
import json, pathlib, subprocess

GEWU = "{{gewu}}"
HERE = pathlib.Path(__file__).parent
STATE = pathlib.Path(".gewu/harness")
DIMENSIONS = ["grounded", "change_framing", "rationale", "representation_contrast",
              "showing", "minimality", "connection"]


def harness(*args):
    return subprocess.run([GEWU, "harness", *args], capture_output=True, text=True, check=True).stdout


def write(name, value):
    (STATE / name).write_text(json.dumps(value))


manifest = json.loads(pathlib.Path(".gewu/review-input.json").read_text())
source, change = manifest["source"], manifest["changes"][0]
quote = {"change_id": change["id"], "start": 1, "end": change["body_lines"]}
wrote = False
while True:
    prompt = harness("status")
    stage = prompt.splitlines()[0].removeprefix("# Gewu harness: ").split()[0]
    if stage == "investigate":
        write("investigation.json", {
            "schema_version": 1, "source": source, "intent": "Raise the value.",
            "changes": ["`VALUE` becomes 2."], "concepts": [],
            "findings": [{"change_ids": [change["id"]], "summary": "One constant changes.",
                          "evidence": []}]})
    elif stage == "plan":
        write("worklist.json", {"schema_version": 1, "source": source, "items": [{
            "id": "item-0", "title": "The value", "purpose": "Raise it.",
            "covers": [quote], "expanded": False}]})
    elif stage == "expand":
        write("expansion.json", {"schema_version": 1, "source": source, "item_id": "item-0",
            "chunks": [{"id": "value", "title": "The value", "summary": "`VALUE` becomes 2.",
                        "kind": "other", "content": [{"type": "changes", "quotes": [quote],
                                                      "presentation": "diff"}]}]})
        wrote = True
        with open(HERE / "runs.txt", "a") as runs:
            runs.write("expand\n")
    elif stage == "proofread":
        if wrote:
            break
        with open(HERE / "runs.txt", "a") as runs:
            runs.write("proofread\n")
        scored = len((HERE / "runs.txt").read_text().split()) > 2
        write("proofread.json", {"schema_version": 1, "source": source, "chunks": [{
            "id": "value", "scores": {name: 3 if scored else 1 for name in DIMENSIONS},
            "problems": [] if scored else ["Say what the old definition held."]}]})
    elif stage == "polish":
        chunk = json.loads(pathlib.Path(".gewu/review-spec.json").read_text())["chunks"][0]
        chunk.pop("section", None)
        chunk["summary"] = "`VALUE` becomes 2; the old definition held 1."
        write("revision.json", {"schema_version": 1, "source": source, "chunks": [chunk]})
        wrote = True
    else:
        break
    harness("next")
'''


class GenerateTests(unittest.TestCase):
    def setUp(self):
        directory = tempfile.TemporaryDirectory(prefix="gewu-generate-")
        self.addCleanup(directory.cleanup)
        # One level down, so the stand-in agents, origin, and clones the tests
        # make beside the repository stay inside this test's directory.
        self.root = Path(directory.name) / "repository"
        self.root.mkdir()
        for args in (["init", "-q"], ["config", "user.email", "fixture@example.invalid"],
                     ["config", "user.name", "Fixture"]):
            self.git(*args)
        (self.root / "a.rs").write_text("const VALUE: u32 = 1;\n")
        self.git("add", ".")
        self.git("commit", "-qm", "test: base")
        (self.root / "a.rs").write_text("const VALUE: u32 = 2;\n")
        self.git("commit", "-qam", "test: head")

    def git(self, *args):
        subprocess.run(["git", *args], cwd=self.root, check=True, capture_output=True)

    def agent(self, body):
        path = self.root.parent / f"agent-{abs(hash(body))}.sh"
        path.write_text("#!/bin/sh\n" + body + "\n")
        path.chmod(path.stat().st_mode | stat.S_IEXEC)
        return path

    def generate(self, agent, *extra):
        return subprocess.run([str(BINARY), "review", "generate", "--agent", "claude",
                               "--agent-program", str(agent), *extra],
                              cwd=self.root, capture_output=True, text=True)

    def test_an_agent_that_stops_early_is_reported_with_the_stage(self):
        idle = self.agent("exit 0")
        result = self.generate(idle, "--base", "HEAD^", "--head", "HEAD")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("stopped before the reviewer gate", result.stdout)
        self.assertIn("investigate", result.stdout)
        # Continuing needs no range: the started review is picked up.
        again = self.generate(idle)
        self.assertEqual(again.returncode, 0, again.stderr)

    def test_an_agent_that_edits_the_project_is_refused(self):
        meddler = self.agent('echo "// edited" >> a.rs')
        result = self.generate(meddler, "--base", "HEAD^", "--head", "HEAD")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("changed files outside `.gewu/`", result.stderr)

    def test_the_agent_receives_the_brief_and_restricted_tools(self):
        recorder = self.agent('printf "%s\\n" "$@" > "$(dirname "$0")/argv.txt"')
        self.generate(recorder, "--base", "HEAD^", "--head", "HEAD")
        argv = (self.root.parent / "argv.txt").read_text()
        self.assertIn(f"{BINARY.resolve()} harness status", argv)
        self.assertIn("dontAsk", argv)
        self.assertIn("Write(.gewu/harness/**)", argv)
        self.assertNotIn("\nBash\n", argv, "unrestricted shell must not be granted")
        self.assertIn("Bash(gewu harness:*)", argv, "a bare `gewu` is a spelling agents use")

    def test_a_refused_tool_call_is_named_not_mistaken_for_an_early_stop(self):
        denied = self.agent(
            'echo \'{"result": "Could not run it.", "permission_denials": '
            '[{"tool_name": "Bash", "tool_input": {"command": "gewu harness status"}}]}\'')
        result = self.generate(denied, "--base", "HEAD^", "--head", "HEAD")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("after being refused", result.stderr)
        self.assertIn("Bash: gewu harness status", result.stderr)

    def test_the_estimate_comes_first_and_each_batch_reports_what_it_cost(self):
        # A reviewer asked to know what generation will spend before it runs.
        before = subprocess.run([str(BINARY), "review", "generate", "--agent", "claude", "--estimate",
                                 "--base", "HEAD^", "--head", "HEAD"], cwd=self.root,
                                capture_output=True, text=True)
        self.assertEqual(before.returncode, 0, before.stderr)
        self.assertIn("batch(es) left", before.stdout)
        self.assertIn("Gewu's own measured runs", before.stdout)
        self.assertFalse((self.root / ".gewu/harness/state.json").exists(), "estimating starts nothing")
        reporting = self.agent(
            'echo \'{"result": "Wrote a batch.", "total_cost_usd": 0.5, "duration_ms": 90000, "usage": {"input_tokens": 1000, '
            '"output_tokens": 2000, "cache_creation_input_tokens": 3000, "cache_read_input_tokens": 94000}}\'')
        result = self.generate(reporting, "--base", "HEAD^", "--head", "HEAD")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("Estimate:", result.stdout)
        self.assertIn("This batch took 1m 30s and used 100k tokens ($0.50)", result.stdout)
        again = self.generate(reporting)
        self.assertIn("this review's batches so far", again.stdout)
        self.assertIn("about 1m 30s a batch", again.stdout)
        self.assertIn("this review so far: 200k tokens ($1.00)", again.stdout)

    def test_a_slow_agent_shows_progress_while_it_runs(self):
        # A reviewer could not tell whether a minutes-long run was working.
        slow = self.agent("sleep 2")
        result = self.generate(slow, "--base", "HEAD^", "--head", "HEAD")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("first batch · investigating the change", result.stderr)
        self.assertIn("left", result.stderr)

    def test_a_pull_request_is_reviewed_from_its_merge_base_and_continued(self):
        # A reviewer asked for one command to review a PR by number.
        origin = self.root.parent / "origin.git"
        subprocess.run(["git", "clone", "-q", "--bare", str(self.root), str(origin)], check=True)
        self.git("switch", "-q", "-c", "feature", "HEAD^")
        (self.root / "b.rs").write_text("const OTHER: u32 = 1;\n")
        self.git("add", "b.rs")
        self.git("commit", "-qm", "test: feature")
        self.git("push", "-q", str(origin), "feature:refs/pull/1/head")
        merge_base = subprocess.run(["git", "rev-parse", "HEAD^"], cwd=self.root, capture_output=True,
                                    text=True, check=True).stdout.strip()
        reviewer = self.root.parent / "reviewer"
        subprocess.run(["git", "clone", "-q", str(origin), str(reviewer)], check=True)
        idle = self.agent("exit 0")

        def review(*extra):
            return subprocess.run([str(BINARY), "review", "pr", "1", "--agent", "claude",
                                   "--agent-program", str(idle), *extra],
                                  cwd=reviewer, capture_output=True, text=True)

        estimate = review("--estimate")
        self.assertEqual(estimate.returncode, 0, estimate.stderr)
        self.assertIn(f"into main, from merge base {merge_base[:12]}", estimate.stdout)
        first = review()
        self.assertEqual(first.returncode, 0, first.stderr)
        manifest = json.loads((reviewer / ".gewu/review-input.json").read_text())
        self.assertEqual(manifest["source"]["base"], merge_base, "main's newer commit is not the base")
        self.assertEqual([change["path"] for change in manifest["changes"]], ["b.rs"])
        again = review()
        self.assertIn("Continuing the review already started", again.stdout)

    def test_a_fresh_agent_proofreads_each_batch_until_it_passes(self):
        # A walkthrough reads as complete to whoever wrote it, so the author
        # stops at the proofread and Gewu starts another agent to score it.
        script = self.root.parent / "author.py"
        script.write_text(AUTHOR.replace("{{gewu}}", str(BINARY.resolve())))
        script.chmod(script.stat().st_mode | stat.S_IEXEC)
        result = self.generate(script, "--base", "HEAD^", "--head", "HEAD")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout.count("A fresh agent proofreads"), 2, result.stdout)
        self.assertIn("A batch is ready for you", result.stdout)
        runs = (self.root.parent / "runs.txt").read_text().split()
        self.assertEqual(runs, ["expand", "proofread", "proofread"], "the author never scores its own batch")
        spec = json.loads((self.root / ".gewu/review-spec.json").read_text())
        self.assertIn("old definition", spec["chunks"][0]["summary"], "the polished chunk was kept")

    def test_nothing_to_continue_and_no_provider_are_refused(self):
        idle = self.agent("exit 0")
        self.assertNotEqual(self.generate(idle).returncode, 0, "no review was started")
        missing = subprocess.run([str(BINARY), "review", "generate"], cwd=self.root,
                                 capture_output=True, text=True)
        self.assertIn("--agent", missing.stderr)


if __name__ == "__main__":
    unittest.main()
