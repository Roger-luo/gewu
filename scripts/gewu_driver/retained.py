"""Keep native surviving work while planning only uncovered source intervals."""
from copy import deepcopy


def retained_items(session):
    path = session.root / "harness/worklist.json"
    if not path.exists():
        return []
    work = session.read("harness/worklist.json")
    if work["source"] != session.manifest["source"]:
        raise ValueError("retained plan belongs to a different source")
    return [item for item in work["items"] if item["expanded"]]


def uncovered(change, retained):
    spans = sorted((quote for item in retained for quote in item["covers"]
                    if quote["change_id"] == change["id"]), key=lambda quote: quote["start"])
    result, start = [], 1
    for span in spans:
        if span["start"] > start:
            result.append({"change_id": change["id"], "start": start, "end": span["start"] - 1})
        start = max(start, span["end"] + 1)
    if start <= change["body_lines"]:
        result.append({"change_id": change["id"], "start": start, "end": change["body_lines"]})
    return result


def validate_remaining(session, draft, change_id, retained):
    previous = []
    for item in retained:
        local = deepcopy(item)
        local["covers"] = [quote for quote in item["covers"] if quote["change_id"] == change_id]
        if local["covers"]:
            previous.append(local)
    for item in draft["items"]:
        for quote in item["covers"]:
            if any(quote["change_id"] == old["change_id"] and quote["start"] <= old["end"]
                   and old["start"] <= quote["end"] for prior in previous for old in prior["covers"]):
                raise ValueError("new plan overlaps retained expanded work")
    combined = dict(draft, items=previous + draft["items"])
    session.validate_plan(combined, [change_id])
