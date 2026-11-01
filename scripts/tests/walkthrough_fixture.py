"""Scripted author responses for workflow tests, never quality acceptance."""
import json


class ScriptedAuthor:
    def __init__(self, mode="valid"):
        self.requests = []
        self.mode = mode
        self.expansions = 0

    def __call__(self, payload):
        call = json.loads(payload)
        self.requests.append(call)
        request = call["request"]
        if "measurement" in request:
            if request["measurement"]["valid"]:
                artifact = {"submit": request["draft_sha256"]}
            else:
                artifact = request["draft"]
                artifact["chunks"][0]["summary"] = "The fixture declares a constant."
        elif "reuse_candidate" in request:
            artifact = {"reuse": True}
        elif call["stage"] == "investigate":
            page = request["page"]
            artifact = {"schema_version": 1, "source": page["source"], "request_id": request["id"],
                        "findings": [{"summary": "A fixture module declares a constant.",
                                      "evidence": [{"offset": page["offset"], "text": page["text"]}], "questions": []}]}
        elif call["stage"] == "synthesize":
            references = sorted({int(ref.removeprefix("finding-")) for node in request["notes"] for ref in node["references"]})
            claim = {"text": "Fixture modules declare constants.", "findings": references}
            artifact = {"schema_version": 1, "source": request["source"], "ledger_sha256": request["ledger_sha256"],
                        "intent": claim, "changes": [claim], "concepts": []}
        elif "units" in request:
            artifact = {"order": [unit["id"] for unit in reversed(request["units"])], "summary": "Fixture constant modules."}
        elif "scope" in request:
            change = request["scope"][0]
            artifact = {"schema_version": 1, "source": request["source"], "items": [{"id": "constant", "title": "Fixture constant",
                        "purpose": "Inspect the value", "covers": change.get("uncovered", [{"change_id": change["id"], "start": 1, "end": change["body_lines"]}]),
                        "expanded": False}]}
        elif self.mode == "premature":
            artifact = {"submit": "unmeasured"}
        elif self.mode == "bad-read" and self.expansions == 0:
            self.expansions += 1
            quote = dict(request["item"]["covers"][0], end=999)
            artifact = {"read": {"quote": quote}}
        elif self.mode == "dependency-first" and self.expansions == 0:
            self.expansions += 1
            owned = {q["change_id"] for q in request["item"]["covers"]}
            entry = next(entry for entry in request["corpus_inventory"]["result"]["entries"] if entry["id"] not in owned)
            artifact = {"dependency": {"quote": {"change_id": entry["id"], "start": 1, "end": entry["body_lines"]}}}
        else:
            self.expansions += 1
            summary = "x" * 501 if self.mode == "repair" and self.expansions == 1 else "The fixture declares a constant."
            artifact = {"schema_version": 1, "source": request["source"], "item_id": request["item"]["id"],
                        "chunks": [{"id": "constant", "title": "Fixture constant", "summary": summary, "kind": "concept",
                                    "content": [{"type": "changes", "quotes": request["item"]["covers"], "presentation": "added_code"}]}]}
        return {"artifact": artifact, "usage": {"input_tokens": 10, "output_tokens": 10}}
