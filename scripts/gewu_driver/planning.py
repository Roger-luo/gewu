"""Plan bounded semantic scopes; delegate source coverage checks to Gewu."""
from .reduce import compact, fits


INSTRUCTIONS = """Plan a coherent part of a walkthrough for a developer unfamiliar with this repository.
Return a WorkList JSON object with schema_version=1, the supplied global source, and items:
[{"id":"unique-within-this-scope","title":"short purpose","purpose":"behavior and review question",
"covers":[{"change_id":"supplied id","start":1,"end":40}],"expanded":false}].
Cover every uncovered span in the supplied scope (or every hunk-body line if uncovered is absent).
Do not replan retained expanded spans. Preserve original IDs and line coordinates.
Choose semantic boundaries, not investigation page boundaries; do not split a definition or
sentence arbitrarily. Order foundational concepts before their consumers. An item expands to
at most max_chunks chunks, each about max_chunk_lines rendered rows including explanation.
A work item is a review batch of up to max_chunks chunks, not a single function.
Prefer one coherent small module per item when its explanation and source fit within
that many chunks; divide it into readable chunks during expansion. Split larger modules
only when necessary. Do not bury handwritten behavior in summary_only.
Generated artifacts may be summarized only with a concrete reason and visible limitations.
The supplied notes are source-backed but local; preserve unresolved questions rather than
inventing cross-file guarantees. This plan is later merged and checked against the whole manifest."""


def plan_scope(author, source, scope, findings, validate, max_chunks=6, max_chunk_lines=40):
    """`validate` must invoke Gewu's scoped validator before returning success.

    It must not mutate the active harness. Failed validation is returned to a
    bounded repair request; every repair is charged by the same author ledger.
    """
    nodes = [{"text": finding["summary"] + "\nEvidence:\n" + "\n".join(finding["evidence"]),
              "references": [f"finding-{index}"]} for index, finding in findings]
    target = author.config["limits"]["request_bytes"] // 3
    nodes = compact(author, source, nodes, target)
    request = {"instructions": INSTRUCTIONS, "source": source, "scope": scope, "notes": nodes,
               "max_chunks": max_chunks, "max_chunk_lines": max_chunk_lines}
    if not fits(author, "plan", request):
        raise ValueError("planning scope metadata exceeds request budget; split the scope")
    stage = "plan"
    for attempt in range(3):
        draft = author.call(stage, request)
        try:
            if draft.get("source") != source or draft.get("schema_version") != 1:
                raise ValueError("plan source or schema mismatch")
            items = draft.get("items")
            if not isinstance(items, list) or not all(isinstance(item, dict) for item in items):
                raise ValueError("plan items must be an array of objects")
            if any(item.get("expanded", False) for item in items):
                raise ValueError("new work items cannot already be expanded")
            validate(draft)
            return draft
        except ValueError as error:
            if attempt == 2:
                raise
            # Replace the previous repair, never append an unbounded history.
            request = {"instructions": INSTRUCTIONS, "source": source, "scope": scope, "notes": nodes,
                       "max_chunks": max_chunks, "max_chunk_lines": max_chunk_lines,
                       "rejected_draft": draft, "validation_error": str(error)[:2048]}
            if not fits(author, "repair", request):
                raise ValueError("repair context exceeds request budget; no provider call made") from error
            stage = "repair"
