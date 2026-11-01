"""Offline medium-corpus recovery evidence. Never invokes an external provider."""
import argparse
import hashlib
import json
from pathlib import Path
import sys
import subprocess
from unittest.mock import patch

from gewu_driver import author as author_module
from gewu_driver.author import BudgetedAuthor, Limits
from gewu_driver.evaluation import create
from gewu_driver.expand import Expansion
from gewu_driver.prepare import Preparation
from gewu_driver.reconcile import reconcile
from gewu_driver.session import Session
from gewu_driver.storage import save

sys.path.insert(0, str(Path(__file__).parent / "tests"))
from medium_fixture import MediumFixtureAuthor


def run(source, plan, output, binary):
    names = [entry["path"] for entry in json.loads(plan.read_text())["files"]]
    repo = create(source, names, output / "fixture", binary)
    session = Session(repo, binary)
    # Source-change recovery is a working-tree feature. Frozen reviews deliberately
    # keep their original manifest, even if HEAD changes later.
    with (repo / ".git/info/exclude").open("a") as handle:
        handle.write("\n.gewu/\n")
    base = session.manifest["source"]["base"]
    session.native("harness", "reset")
    subprocess.run([str(binary), "harness", "start", "--base", base, "--working-tree"],
                   cwd=repo, capture_output=True, check=True, timeout=30)
    session = Session(repo, binary)
    provider = MediumFixtureAuthor()
    limits = Limits(max_calls=160, total_reserved_bytes=8_000_000,
                    observed_input_tokens_stop=1000, observed_output_tokens_stop=1000)
    def author_for(current, name="author"):
        return BudgetedAuthor(current.root / name, limits,
                              {"session": current.identity, "provider": "offline-scripted-fixture"}, provider)
    author = author_for(session)
    prep = Preparation(session, author)
    prep.investigate()
    prep.plan()
    initial_calls = len(provider.requests)
    try:
        Expansion(session, author).next_batch(max_turns=1)
    except ValueError as error:
        assert "turn budget" in str(error), str(error)
    else:
        raise AssertionError("fixture must stop after measurement")
    measured_hash = hashlib.sha256((session.root / "driver/expansion-candidate.json").read_bytes()).hexdigest()
    count = len(provider.requests)
    restarted = Session(repo, binary)
    Expansion(restarted, author_for(restarted)).next_batch()
    assert len(provider.requests) == count + 1  # Only the still-needed confirmation.
    assert hashlib.sha256((session.root / "driver/expansion-candidate.json").read_bytes()).hexdigest() == measured_hash
    assert restarted.stage() == "await"
    retained = restarted.read("review-spec.json")["chunks"]
    owned = {q["change_id"] for c in retained for b in c["content"] for q in b.get("quotes", [])}
    changed = next(c for c in session.manifest["changes"] if c["path"] in
                   ("scripts/gewu_driver/storage.py", "scripts/gewu_driver/accounting.py") and c["id"] not in owned)
    deleted = next(c for c in session.manifest["changes"] if c["path"] == "Cargo.toml" and c["id"] not in owned)
    with (repo / changed["path"]).open("a") as handle:
        handle.write("\n# Offline recovery mutation.\n")
    (repo / deleted["path"]).unlink()
    restarted.native("harness", "next")
    assert restarted.stage() == "investigate"
    archive = next((session.root / "harness/research-archive").iterdir())
    fresh = Session(repo, binary)
    count = len(provider.requests)
    prep = Preparation(fresh, author_for(fresh, "author-replanned"))
    prep.investigate(previous_archive=archive)
    investigation = [c["request"] for c in provider.requests[count:] if c["stage"] == "investigate"]
    reused = [r for r in investigation if "reuse_candidate" in r]
    assert reused and all(r["page"]["path"] != changed["path"] for r in reused)
    assert any(r["page"]["path"] == changed["path"] and "reuse_candidate" not in r for r in investigation)
    assert all(r["page"]["path"] != deleted["path"] for r in investigation)
    assert sum(c["stage"] == "synthesize" for c in provider.requests[count:]) == 1
    prep.plan()
    assert fresh.read("review-spec.json")["chunks"] == retained
    Expansion(fresh, author_for(fresh, "author-replanned")).next_batch()
    assert fresh.stage() == "await"
    assert not list(fresh.root.glob("reviews/**/events.jsonl"))
    # Independently exercise ambiguous completion with a retained exact reply.
    isolated = BudgetedAuthor(output / "pending-author", limits, {"fixture": "pending"},
                             lambda _: {"artifact": {"fixture": True}, "usage": {"input_tokens": 1, "output_tokens": 1}})
    real_save = author_module.save
    class Interrupted(BaseException):
        pass
    def interrupt(path, value):
        if Path(path).name == "state.json" and value["calls"][-1]["status"] == "succeeded":
            raise Interrupted()
        real_save(path, value)
    with patch.object(author_module, "save", interrupt):
        try:
            isolated.call("plan", {"fixture": True})
        except Interrupted:
            pass
    state = json.loads((isolated.root / "state.json").read_text())
    key, reserved = state["calls"][0]["key"], state["reserved_bytes"]
    for request in ({"fixture": True}, {"different": True}):
        try:
            isolated.call("plan", request)
        except ValueError as error:
            assert "pending" in str(error)
        else:
            raise AssertionError("pending call admitted")
    try:
        reconcile(isolated, key, "Offline inspected reply", "wrong")
    except ValueError as error:
        assert "inspected hash" in str(error)
    else:
        raise AssertionError("wrong hash accepted")
    reply_hash = hashlib.sha256((isolated.root / "calls/0000/reply.json").read_bytes()).hexdigest()
    reconcile(isolated, key, "Offline inspected reply", reply_hash)
    isolated.invoke = lambda _: (_ for _ in ()).throw(AssertionError("cache invoked provider"))
    assert isolated.call("plan", {"fixture": True}) == {"fixture": True}
    assert json.loads((isolated.root / "state.json").read_text())["reserved_bytes"] == reserved
    save(output / "report.json", {"status": "offline recovery assertions passed", "model_calls": 0,
         "initial_source": session.manifest["source"], "replanned_source": fresh.manifest["source"],
         "files": names, "initial_preparation_calls": initial_calls, "measured_draft_sha256": measured_hash,
         "restart_new_calls": 1, "unchanged_reused_pages": len(reused), "changed": changed["path"],
         "deleted": deleted["path"], "retained_chunks": len(retained), "final_stage": fresh.stage(),
         "pending_reply_sha256": reply_hash, "pending_reserved_bytes_preserved": reserved,
         "limitation": "Scripted protocol evidence only; no semantic quality or human approval."})


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("source", "plan", "output", "binary"):
        parser.add_argument("--" + name, type=Path, required=True)
    args = parser.parse_args()
    run(args.source.resolve(), args.plan.resolve(), args.output.resolve(), args.binary.resolve())
