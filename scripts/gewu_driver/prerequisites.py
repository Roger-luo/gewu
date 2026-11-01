"""Bounded lexical hints: candidate evidence, never a semantic dependency proof."""
from collections import Counter
import re

from .lookup import lookup
from .storage import encode


IDENTIFIER = re.compile(r"\b[A-Za-z_][A-Za-z_0-9]{2,63}\b")
COMMON = set("def class return import from with try except finally raise None True False self pub use let mut fn impl struct enum const static for while if else match async await function var int str bool string usize Result Option Some vec new get set len isinstance not and or in as is".split())


def prefetch(session, item, max_bytes=4096):
    result = {"selection": "Lexically related candidates, not proven dependencies or exhaustive corpus evidence.", "candidates": []}
    if len(encode(result)) > max_bytes:
        raise ValueError("prerequisite budget cannot hold its envelope")
    if not hasattr(session, "_prerequisite_index"):
        indexed, frequency = [], Counter()
        for change in session.manifest["changes"]:
            terms = set(IDENTIFIER.findall(change["patch"])) - COMMON
            indexed.append((change, terms))
            frequency.update(terms)
        session._prerequisite_index = indexed, frequency
    indexed, frequency = session._prerequisite_index
    owned = {q["change_id"] for q in item["covers"]}
    terms = set().union(*(words for change, words in indexed if change["id"] in owned))
    ranked = []
    for change, words in indexed:
        shared = terms.intersection(words)
        if change["id"] not in owned and shared:
            ranked.append((sum(1 / frequency[word] for word in sorted(shared)), change, shared))
    ranked.sort(key=lambda entry: (-entry[0], entry[1]["id"]))
    for _, change, shared in ranked[:2]:
        symbols = sorted(shared, key=lambda word: (frequency[word], word))[:8]
        envelope = {"role": "prerequisite evidence only; item.covers still controls chunk quotes",
                    "action": "dependency", "result": {}, "shared_identifiers": symbols}
        remaining = max_bytes - len(encode(dict(result, candidates=result["candidates"] + [envelope])))
        if remaining < 512:
            break
        patch = change["patch"]
        body_start = patch.find("\n") + 1 if patch.startswith("@@ ") else 0
        positions = [match.start() for word in symbols
                     if (match := re.compile(r"\b" + re.escape(word) + r"\b").search(patch, body_start))]
        if not positions:
            continue
        line = patch.count("\n", body_start, min(positions)) + 1
        start = max(1, line - 3)
        quote = {"change_id": change["id"], "start": start, "end": min(change["body_lines"], start + 39)}
        try:
            candidate = lookup(session, "dependency", {"quote": quote}, remaining)
        except ValueError:
            continue  # Discovery remains available; never claim absence.
        candidate["shared_identifiers"] = symbols
        if len(encode(dict(result, candidates=result["candidates"] + [candidate]))) <= max_bytes:
            result["candidates"].append(candidate)
    return result
