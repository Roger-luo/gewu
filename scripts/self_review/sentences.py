"""Reader-facing prose, cut into numbered units a judge must label one by one.

Splitting is deterministic so the score can prove the judge labelled every
unit exactly once; a judge that skims cannot pass by omission.
"""
import re

# A sentence ends at terminal punctuation followed by space and a capital,
# a backtick, or an opening bracket. Abbreviations may split early; that costs
# the judge an extra label, never a missed sentence.
BOUNDARY = re.compile(r"(?<=[.!?])\s+(?=[A-Z`(\[*_\"])")
BULLET = re.compile(r"^\s*(?:[-*+]|\d+[.)])\s+")


def split(markdown):
    """Sentences of Markdown prose, skipping fenced examples and headings."""
    units, paragraph, fenced = [], [], False

    def flush():
        text = " ".join(paragraph).strip()
        paragraph.clear()
        units.extend(part.strip() for part in BOUNDARY.split(text) if part.strip())

    for line in markdown.splitlines():
        stripped = line.strip()
        if stripped.startswith(("```", "~~~")):
            flush()
            fenced = not fenced
            continue
        if fenced:
            continue
        if not stripped or stripped.startswith("#"):
            flush()
            continue
        if BULLET.match(line):
            flush()
            stripped = BULLET.sub("", line).strip()
        paragraph.append(stripped)
    flush()
    return units


def fields(spec):
    """Every prose field a reader sees, with where it lives."""
    overview = spec["overview"]
    yield "overview", "intent", overview["intent"]
    for index, text in enumerate(overview.get("changes", [])):
        yield "overview", f"changes[{index}]", text
    for index, text in enumerate(overview.get("concepts", [])):
        yield "overview", f"concepts[{index}]", text
    for chunk in spec["chunks"]:
        yield chunk["id"], "summary", chunk["summary"]
        for index, block in enumerate(chunk["content"]):
            if block["type"] == "narrative":
                yield chunk["id"], f"content[{index}]", block["markdown"]
        context = chunk.get("context") or {}
        for key in ("definition", "example", "relation", "before", "after", "reason"):
            if context.get(key):
                yield chunk["id"], f"context.{key}", context[key]
        if chunk.get("confidence"):
            yield chunk["id"], "confidence", chunk["confidence"]


def units(spec):
    numbered = []
    for location, field, text in fields(spec):
        for sentence in split(text):
            numbered.append({"id": f"s{len(numbered) + 1}", "location": location,
                             "field": field, "text": sentence})
    return numbered
