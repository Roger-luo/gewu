"""Order semantic file/directory units without sending the full worklist."""
from copy import deepcopy
from pathlib import PurePosixPath

from .tasks import deliver
from .reduce import compact, fits


INSTRUCTIONS = """Order these parts of a source walkthrough by comprehension dependencies.
Return {"order":["every supplied unit id exactly once"],"summary":"source-backed role of this group"}.
Introduce concepts and types before consumers, then wiring, tests and supporting material.
Use the supplied purposes and overview, not alphabetical order. Keep summary under 1200 characters.
Do not invent units or silently omit supporting files. This orders a directory-sized group;
its children's local semantic ordering and source coverage are already validated."""


def order_plans(author, source, overview, plans):
    root = {"id": "directory:.", "children": {}}
    for local in plans:
        parent = root
        parts = PurePosixPath(local["path"]).parts
        for index in range(len(parts) - 1):
            path = "/".join(parts[:index + 1])
            parent = parent["children"].setdefault("directory:" + path, {"id": "directory:" + path, "children": {}})
        key = "change:" + local["change_id"]
        parent["children"][key] = {"id": key, "local": local}

    def visit(node):
        if "local" in node:
            local = node["local"]
            items = deepcopy(local["plan"]["items"])
            for item in items:
                item["id"] = local["change_id"] + ":" + item["id"]
                item["expanded"] = False
            nodes = [{"text": item["title"] + ": " + item["purpose"], "references": [item["id"]]} for item in items]
            nodes = compact(author, source, nodes, author.config["limits"]["request_bytes"] // 8)
            return items, {"id": node["id"], "path": local["path"],
                           "purpose": "; ".join(node["text"] for node in nodes)}

        children = [visit(child) for _, child in sorted(node["children"].items())]
        if len(children) == 1:
            items, description = children[0]
            return items, {"id": node["id"], "purpose": description["purpose"]}
        return arrange(node["id"], children)

    def arrange(identifier, children, depth=0):
        descriptions = [description for _, description in children]
        request = {"instructions": INSTRUCTIONS, "source": source, "overview": overview, "units": descriptions}
        if not fits(author, "plan", request):
            if depth >= 8:
                raise ValueError("ordering hierarchy exceeded depth budget")
            groups, group = [], []
            for child in children:
                candidate = dict(request, units=[d for _, d in group + [child]])
                if group and not fits(author, "plan", candidate):
                    groups.append(group)
                    group = []
                if not fits(author, "plan", dict(request, units=[child[1]])):
                    raise ValueError("one ordering description exceeds request budget")
                group.append(child)
            if group:
                groups.append(group)
            if len(groups) >= len(children):
                raise ValueError("ordering budget cannot combine even two units")
            reduced = [arrange(identifier + ":group:" + str(index), group, depth + 1)
                       for index, group in enumerate(groups)]
            return arrange(identifier, reduced, depth + 1)
        wanted = [description["id"] for description in descriptions]
        def validate(reply):
            order = reply.get("order")
            if not isinstance(order, list) or not all(isinstance(key, str) for key in order) or len(order) != len(wanted) or set(order) != set(wanted):
                raise ValueError("ordering must contain each supplied unit exactly once")
            if not isinstance(reply.get("summary"), str) or not reply["summary"].strip() or len(reply["summary"]) > 1200:
                raise ValueError("ordering summary must be nonempty and under 1200 characters")
        reply = deliver(author, "plan", request, validate)
        by_id = {description["id"]: items for items, description in children}
        return [item for key in reply["order"] for item in by_id[key]], {"id": identifier, "purpose": reply["summary"]}

    if not plans or len({plan["change_id"] for plan in plans}) != len(plans):
        raise ValueError("ordering requires nonempty, unique planning scopes")
    items, _ = visit(root)
    return {"schema_version": 1, "source": source, "items": items}
