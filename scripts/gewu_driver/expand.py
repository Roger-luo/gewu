"""Span-local expansion with native measurement and an explicit submit loop."""
from copy import deepcopy
import hashlib
import json

from .prepare import Preparation
from .reduce import compact, fits
from .storage import digest, encode, locked
from .lookup import lookup
from .contract import expansion_contract
from .prerequisites import prefetch


TOOLS = """Return an expansion JSON object, or {"read":{"quote":{"change_id":"...","start":1,"end":20},
"cursor":null}} to inspect an assigned source span. Source pages may be partial; use their next
cursor to continue. Never treat a partial page as the whole file. Do not summarize handwritten
behavior merely to fit. Chunk IDs will be namespaced by work item. After native measurement,
inspect the report and return {"submit":"the supplied draft hash"} only when it is valid, or
return a corrected complete expansion. A changed draft must be measured again.
For related source return {"search":{"text":"literal symbol or import","cursor":null}},
{"inventory":{"cursor":null}}, or {"dependency":{"quote":{"change_id":"...","start":1,"end":20},"cursor":null}}.
These inspect the frozen corpus without extending item.covers: quotes in chunks must remain
assigned, but explanations may connect them to verified prerequisite evidence with path/line references.
The local packet is not the whole corpus. Before claiming definitions or callers unavailable,
inspect relevant corpus search/inventory and source. A literal search is not semantic absence proof.
Describe uninspected relationships as unverified, never as absent from the supplied repository."""


class Expansion:
    def __init__(self, session, author):
        Preparation(session, author)  # Install the same frozen-session guard.
        self.session, self.author = session, author

    def next_batch(self, max_turns=12):
        session, author = self.session, self.author
        session.assert_fresh()
        if session.stage() not in ("expand", "repair"):
            raise ValueError("expansion requires the native expand/repair stage; human gates are not bypassed")
        with locked(session.root / "driver/workflow"):
            item = next((item for item in session.read("harness/worklist.json")["items"] if not item["expanded"]), None)
            if item is None:
                session.native("harness", "next")
                if session.stage() not in ("await", "complete"):
                    raise ValueError("retained work requires a new native workflow decision")
                return None
            guide = expansion_contract(session.manifest["max_chunk_lines"])
            findings = session.read("harness/investigation.json")["findings"]
            nodes = [{"text": finding["summary"] + "\n" + "\n".join(finding["evidence"]),
                      "references": [f"finding-{index}"]} for index, finding in enumerate(findings)
                     if self._relevant(item, finding)]
            nodes = compact(author, session.manifest["source"], nodes, author.config["limits"]["request_bytes"] // 8)
            request = {"instructions": guide + "\n" + TOOLS, "source": session.manifest["source"], "item": item, "notes": nodes,
                       "source_pages": [], "corpus_inventory": lookup(session, "inventory", {}, 2048),
                       "prerequisites": prefetch(session, item)}
            available = author.config["limits"]["request_bytes"] - author.config.get("request_overhead_bytes", 0) - len(encode({"stage": "expand", "request": request})) - 1024
            page_bytes = min(8192, available // max(1, len(item["covers"])))
            if page_bytes < 1:
                raise ValueError("expansion instructions and scope exceed request budget")
            for quote in item["covers"]:
                request["source_pages"].append(self._read(item, quote, None, page_bytes))
            pending, report = None, None
            current, stage = request, "expand"
            for _ in range(max_turns):
                if not fits(author, stage, current):
                    raise ValueError("expansion context exceeds budget; refine this unexpanded work item")
                response = author.call(stage, current)
                action = next(iter(response), None)
                if len(response) == 1 and action in ("search", "inventory", "dependency"):
                    current = deepcopy(current)
                    try:
                        result = lookup(session, action, response[action], min(4096, page_bytes))
                        current.setdefault("lookups", []).append(result)
                    except ValueError as error:
                        current["lookup_error"] = str(error)[:2048]
                        stage = "repair"
                    continue
                if set(response) == {"read"}:
                    query = response["read"]
                    try:
                        if not isinstance(query, dict) or "quote" not in query:
                            raise ValueError("read requires a quote and optional cursor")
                        page = self._read(item, query["quote"], query.get("cursor"), min(4096, page_bytes))
                    except ValueError as error:
                        current = deepcopy(current)
                        current["lookup_error"] = str(error)[:2048]
                        current["lookup_instruction"] = "read is restricted to item.covers. Use dependency for prerequisite evidence from other valid spans; chunk quotes remain assigned."
                        stage = "repair"
                        continue
                    current = deepcopy(current)
                    current.setdefault("lookups", []).append(page)
                    continue
                if set(response) == {"submit"}:
                    if pending is None or not report["valid"] or response["submit"] != digest(pending):
                        raise ValueError("only the exact valid measured draft may be submitted")
                    session.write("harness/expansion.json", pending)
                    outcome = session.native("harness", "next")
                    if session.stage() != "await":
                        raise ValueError("native expansion submission failed: " + outcome[:2048])
                    return pending
                pending = self._namespace(item, response)
                path = session.write("driver/expansion-candidate.json", pending)
                try:
                    report = json.loads(session.native("harness", "measure-expansion", "--draft", path))
                except ValueError as error:
                    report = {"valid": False, "errors": [str(error)[:2048]], "chunks": []}
                lookups = current.get("lookups", [])
                current = {"instructions": guide + "\n" + TOOLS + "\nInspect this measurement. Preserve meaning when fixing the draft; request source if needed.",
                           "source": session.manifest["source"], "item": item, "draft": pending,
                           "measurement": report, "draft_sha256": digest(pending), "lookups": lookups,
                           "prerequisites": request["prerequisites"]}
                stage = "expand" if report["valid"] else "repair"
            raise ValueError("expansion turn budget exhausted")

    @staticmethod
    def _relevant(item, finding):
        spans = finding.get("evidence_spans")
        if spans:
            return any(a["change_id"] == b["change_id"] and a["start"] <= b["end"] and b["start"] <= a["end"]
                       for a in item["covers"] for b in spans)
        # Legacy checkpoints have only change-level attribution.
        return bool({q["change_id"] for q in item["covers"]}.intersection(finding["change_ids"]))

    def _read(self, item, quote, cursor, max_bytes):
        if not isinstance(quote, dict) or set(quote) != {"change_id", "start", "end"}:
            raise ValueError("source lookup needs an exact quote")
        start, end = quote["start"], quote["end"]
        if type(start) is not int or type(end) is not int or start < 1 or end < start:
            raise ValueError("invalid source lookup range")
        next_line = start
        for owned in sorted((q for q in item["covers"] if q["change_id"] == quote["change_id"]), key=lambda q: q["start"]):
            if owned["start"] <= next_line <= owned["end"]:
                next_line = owned["end"] + 1
        if next_line <= end:
            raise ValueError("source lookup escapes this work item's assigned spans")
        args = ["harness", "context", "--max-bytes", str(max_bytes)]
        if cursor is not None:
            args += ["--cursor", json.dumps(cursor)]
        args += ["source", "--change-id", quote["change_id"], "--start", str(start), "--end", str(end)]
        return json.loads(self.session.native(*args))

    @staticmethod
    def _namespace(item, draft):
        result = deepcopy(draft)
        prefix = hashlib.sha256(item["id"].encode()).hexdigest()[:16] + ":"
        if isinstance(result.get("chunks"), list):
            for chunk in result["chunks"]:
                if isinstance(chunk, dict) and isinstance(chunk.get("id"), str) and not chunk["id"].startswith(prefix):
                    chunk["id"] = prefix + chunk["id"]
        return result
