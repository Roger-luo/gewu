"""Incremental preparation orchestration. Does not advance a human review gate."""
import json

from .planning import plan_scope
from .reduce import compact, fits
from .tasks import deliver
from .storage import locked
from .order import order_plans
from .retained import retained_items, uncovered, validate_remaining


OVERVIEW = """Synthesize an accurate newcomer overview from these source-backed notes.
Return only {"schema_version":1,"source":<source>,"ledger_sha256":<ledger_sha256>,
"intent":{"text":"overall purpose","findings":[0]},"changes":[{"text":"behavior with conditions",
"findings":[0,1]}],"concepts":[{"text":"needed term and definition","findings":[0]}]}.
Use one to five change bullets. Concepts may be empty. Convert finding-N references to integer N.
Every claim must be supported by its cited original findings. Preserve exceptions and unresolved
questions; do not infer repository-wide guarantees from one local configuration. This is an
overview for review, not approval. Intermediate summaries are compressed; do not invent details."""


class Preparation:
    def __init__(self, session, author):
        if author.config["identity"].get("session") != session.identity:
            raise ValueError("author profile must pin the exact session, binary and driver protocol")
        if author.before_call is not None and author.before_call != session.assert_fresh:
            raise ValueError("preparation requires its session freshness guard")
        author.before_call = session.assert_fresh
        self.session, self.author = session, author

    def investigate(self, previous_archive=None):
        session, author = self.session, self.author
        session.assert_fresh()
        if session.stage() != "investigate":
            raise ValueError("investigation stage required")
        with locked(session.root / "driver/workflow"):
            if not (session.root / "harness/research.json").exists():
                limits = author.config["limits"]
                session.native("harness", "research", "start", "--page-bytes", str(min(12_288, limits["request_bytes"] // 2)),
                               "--request-bytes", str(limits["request_bytes"] - 128),
                               "--checkpoint-bytes", str(min(4096, limits["response_bytes"] - 512)),
                               "--max-batches", str(limits["max_calls"]))
            while task := json.loads(session.native("harness", "research", "next")):
                candidate = None
                request = task
                if previous_archive is not None:
                    from pathlib import Path
                    archive = Path(previous_archive)
                    candidate = json.loads(session.native("harness", "research", "reuse-candidate",
                        "--previous-input", str(archive / "review-input.json"),
                        "--previous-ledger", str(archive / "research.json")))
                    if candidate is not None:
                        request = dict(task, reuse_candidate=candidate)
                        request["instructions"] += (
                            '\nThese previous local notes have identical source content and coordinates. '
                            'Recheck their claims against this page. Return {"reuse":true} only if every '
                            'claim and limitation remains supported; otherwise return corrected notes '
                            'using the original schema. Global conclusions will be synthesized afresh.')
                        if not fits(author, "investigate", request):
                            candidate, request = None, task
                def accept(notes):
                    if notes == {"reuse": True}:
                        if candidate is None:
                            raise ValueError("no validated local candidate was supplied")
                        notes = candidate
                    path = session.write("driver/research-response.json", notes)
                    session.native("harness", "research", "accept", "--checkpoint", path)
                deliver(author, "investigate", request, accept)
            findings, cursor, ledger = [], None, None
            while True:
                args = ["harness", "research", "findings"]
                if cursor:
                    args += ["--cursor", json.dumps(cursor)]
                page = json.loads(session.native(*args))
                if page["start_index"] != len(findings) or (ledger and ledger != page["ledger_sha256"]):
                    raise ValueError("finding pages changed during synthesis")
                ledger = page["ledger_sha256"]
                findings.extend(page["findings"])
                cursor = page["next"]
                if cursor is None:
                    break
            source = session.manifest["source"]
            nodes = [{"text": finding["summary"] + "\nEvidence:\n" + "\n".join(finding["evidence"]),
                      "references": [f"finding-{index}"]} for index, finding in enumerate(findings)]
            nodes = compact(author, source, nodes, author.config["limits"]["request_bytes"] // 2)
            request = {"instructions": OVERVIEW, "source": source, "ledger_sha256": ledger, "notes": nodes}
            def assemble(overview):
                path = session.write("driver/overview-response.json", overview)
                session.native("harness", "research", "assemble", "--overview", path)
            deliver(author, "synthesize", request, assemble)
            session.native("harness", "next")
            if session.stage() != "plan":
                raise ValueError("native harness did not accept the assembled investigation")

    def local_plans(self):
        """Return validated local plans. Global semantic ordering is a separate step."""
        session, author = self.session, self.author
        session.assert_fresh()
        if session.stage() != "plan":
            raise ValueError("planning stage required")
        with locked(session.root / "driver/workflow"):
            source = session.manifest["source"]
            findings = session.read("harness/investigation.json")["findings"]
            plans = []
            retained = retained_items(session)
            for change in session.manifest["changes"]:
                remaining = uncovered(change, retained)
                if not remaining:
                    continue
                selected = [(index, finding) for index, finding in enumerate(findings) if change["id"] in finding["change_ids"]]
                scope = [{"id": change["id"], "path": change["path"], "body_lines": change["body_lines"], "uncovered": remaining}]
                plan = plan_scope(author, source, scope, selected,
                                  lambda draft: validate_remaining(session, draft, change["id"], retained),
                                  max_chunk_lines=session.manifest["max_chunk_lines"])
                plans.append({"change_id": change["id"], "path": change["path"], "plan": plan})
            session.write("driver/local-plans.json", plans)
            return plans

    def plan(self):
        plans = self.local_plans()
        session = self.session
        with locked(session.root / "driver/workflow"):
            if session.stage() != "plan":
                raise ValueError("planning stage changed before merge")
            investigation = session.read("harness/investigation.json")
            overview = {key: investigation[key] for key in ("intent", "changes", "concepts")}
            source = session.manifest["source"]
            merged = order_plans(self.author, source, overview, plans) if plans else {"schema_version": 1, "source": source, "items": []}
            for item in merged["items"]:
                item["id"] = source["diff_sha256"][:16] + ":" + item["id"]
            merged["items"] = retained_items(session) + merged["items"]
            session.validate_plan(merged)
            session.write("harness/worklist.json", merged)
            session.native("harness", "next")
            if session.stage() != "expand":
                raise ValueError("native harness did not accept the merged plan")
            return merged
