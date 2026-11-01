"""Bounded prerequisite inspection without granting additional quote coverage."""
import json


def lookup(session, action, query, max_bytes=4096):
    if not isinstance(query, dict):
        raise ValueError("lookup arguments must be an object")
    args = ["harness", "context", "--max-bytes", str(max_bytes)]
    if query.get("cursor") is not None:
        args += ["--cursor", json.dumps(query["cursor"])]
    if action == "inventory":
        args += ["inventory"]
    elif action == "search":
        text = query.get("text")
        if not isinstance(text, str):
            raise ValueError("search requires literal text")
        args += ["search", "--query", text]
    elif action == "dependency":
        quote = query.get("quote")
        if not isinstance(quote, dict) or set(quote) != {"change_id", "start", "end"}:
            raise ValueError("dependency requires an exact source quote")
        if not isinstance(quote["change_id"], str) or any(type(quote[key]) is not int for key in ("start", "end")):
            raise ValueError("dependency requires a string id and integer coordinates")
        args += ["source", "--change-id", quote["change_id"], "--start", str(quote["start"]), "--end", str(quote["end"])]
    else:
        raise ValueError("unknown evidence lookup")
    return {"role": "prerequisite evidence only; item.covers still controls chunk quotes",
            "action": action, "result": json.loads(session.native(*args))}
