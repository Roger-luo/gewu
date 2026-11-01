"""Bounded synthesis that retains an explicit chain to original findings."""
from .storage import encode


INSTRUCTIONS = """Compress these source-backed investigation notes for later semantic planning.
Return {"nodes":[{"text":"concrete behavior, conditions, relationships and unresolved questions",
"references":["original finding id", "..."]}]} only. Every input reference must occur in the
output; do not invent references. Merge related statements, retain caveats and unresolved
questions, and never replace specific behavior with generic praise or a list of filenames.
The output is an intermediate synthesis, not approval or a substitute for inspectable evidence.
Keep the serialized result within output_bytes. Original evidence remains retrievable by reference."""


def fits(author, stage, request):
    return len(encode({"stage": stage, "request": request})) + author.config.get("request_overhead_bytes", 0) <= author.config["limits"]["request_bytes"]


def compact(author, source, nodes, target_bytes, rounds=4):
    """Each round must shrink, preserve every reference, and fit every call.

    Original findings are never mutated. Cache keys include their complete
    current content, source identity and this stage's exact instructions.
    """
    original = {ref for node in nodes for ref in node["references"]}
    if len(encode([{"text": "x", "references": sorted(original)}])) > target_bytes:
        raise ValueError("required synthesis references cannot fit the target budget")
    current = nodes
    for _ in range(rounds):
        if len(encode(current)) <= target_bytes:
            return current
        groups, group = [], []
        for node in current:
            request = {"instructions": INSTRUCTIONS, "source": source, "nodes": group + [node],
                       "output_bytes": min(target_bytes, author.config["limits"]["response_bytes"] // 2)}
            if group and not fits(author, "synthesize", request):
                groups.append(group)
                group = []
                request["nodes"] = [node]
            if not fits(author, "synthesize", request):
                raise ValueError("one synthesis node exceeds the complete request budget")
            group.append(node)
        if group:
            groups.append(group)
        reduced = []
        output_bytes = min(target_bytes, author.config["limits"]["response_bytes"] // 2)
        for group in groups:
            refs = sorted({ref for node in group for ref in node["references"]})
            if len(encode({"nodes": [{"text": "x", "references": refs}]})) > output_bytes:
                raise ValueError("required synthesis references cannot fit the response budget")
        for group in groups:
            request = {"instructions": INSTRUCTIONS, "source": source, "nodes": group, "output_bytes": output_bytes}
            reply = author.call("synthesize", request)
            wanted = {ref for node in group for ref in node["references"]}
            output = reply.get("nodes")
            if not isinstance(output, list) or not output or len(encode(reply)) > output_bytes:
                raise ValueError("synthesis output is empty or exceeds its bound")
            seen = set()
            for node in output:
                if not isinstance(node, dict) or set(node) != {"text", "references"} or not isinstance(node["text"], str) or not node["text"].strip():
                    raise ValueError("synthesis node needs concrete text and references")
                if not isinstance(node["references"], list) or not node["references"] or not all(isinstance(ref, str) for ref in node["references"]):
                    raise ValueError("invalid synthesis references")
                seen.update(node["references"])
            if seen != wanted:
                raise ValueError("synthesis omitted or invented source references")
            reduced.extend(output)
        if len(encode(reduced)) >= len(encode(current)):
            raise ValueError("synthesis did not shrink; refusing repeated unproductive calls")
        current = reduced
    if len(encode(current)) > target_bytes:
        raise ValueError("synthesis round budget exhausted")
    if {ref for node in current for ref in node["references"]} != original:
        raise ValueError("synthesis coverage changed")
    return current
