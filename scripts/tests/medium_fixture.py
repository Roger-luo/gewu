"""Source-shaped protocol responses for recovery checks, never quality evidence."""
import json


class MediumFixtureAuthor:
    def __init__(self):
        self.requests = []

    def __call__(self, payload):
        call = json.loads(payload)
        self.requests.append(call)
        request = call["request"]
        if "measurement" in request:
            if not request["measurement"]["valid"]:
                raise ValueError("offline fixture draft failed native measurement: " + str(request["measurement"]))
            artifact = {"submit": request["draft_sha256"]}
        elif "reuse_candidate" in request:
            artifact = {"reuse": True}
        elif call["stage"] == "investigate":
            page = request["page"]
            artifact = {"schema_version": 1, "source": page["source"], "request_id": request["id"],
                        "findings": [{"summary": "Synthetic recovery fixture evidence; not a semantic review.",
                                      "evidence": [{"offset": page["offset"], "text": page["text"][:120]}], "questions": []}]}
        elif "nodes" in request:
            artifact = {"nodes": [{"text": "Synthetic recovery fixture only.",
                                   "references": sorted({ref for node in request["nodes"] for ref in node["references"]})}]}
        elif call["stage"] == "synthesize":
            references = sorted({int(ref.removeprefix("finding-")) for node in request["notes"] for ref in node["references"]})
            claim = {"text": "Synthetic fixture only; no semantic quality conclusion.", "findings": references}
            artifact = {"schema_version": 1, "source": request["source"], "ledger_sha256": request["ledger_sha256"],
                        "intent": claim, "changes": [claim], "concepts": []}
        elif "units" in request:
            artifact = {"order": [unit["id"] for unit in reversed(request["units"])], "summary": "Synthetic recovery order."}
        elif "scope" in request:
            change = request["scope"][0]
            items = []
            for span in change["uncovered"]:
                for start in range(span["start"], span["end"] + 1, 24):
                    quote = dict(span, start=start, end=min(start + 23, span["end"]))
                    items.append({"id": "fixture-" + str(start), "title": "Synthetic fixture", "purpose": "Exercise recovery only",
                                  "covers": [quote], "expanded": False})
            artifact = {"schema_version": 1, "source": request["source"], "items": items}
        else:
            chunks = []
            for span in request["item"]["covers"]:
                for start in range(span["start"], span["end"] + 1, 4):
                    quote = dict(span, start=start, end=min(start + 3, span["end"]))
                    chunks.append({"id": "fixture-" + str(start), "title": "Synthetic fixture", "summary": "Recovery fixture only.",
                                   "kind": "concept", "content": [{"type": "changes", "quotes": [quote], "presentation": "added_code"}]})
            artifact = {"schema_version": 1, "source": request["source"], "item_id": request["item"]["id"], "chunks": chunks}
        return {"artifact": artifact, "usage": {"input_tokens": 1, "output_tokens": 1}}
