"""Explicit resolution of an interrupted provider call, without refund or retry."""
import hashlib
import json
import time

from .accounting import validate
from .storage import encode, locked, save


def reconcile(author, key, reason, reply_sha256=None):
    """Accept a retained reply by inspected hash, or retire the call as failed.

    The caller supplies an audit reason. This never invokes the provider, buys
    back budget, or makes a failed request retryable. Accepting transport output
    does not replace domain validation when the checkpoint is subsequently used.
    """
    if not isinstance(reason, str) or not reason.strip() or len(reason) > 2048:
        raise ValueError("reconciliation requires a nonempty bounded audit reason")
    with locked(author.root):
        if author.before_call is not None:
            author.before_call()
        path = author.root / "state.json"
        state = json.loads(path.read_bytes())
        validate(author.root, state, author.config)
        record = next((record for record in state["calls"] if record["key"] == key), None)
        if record is None or record["status"] != "pending":
            raise ValueError("reconciliation requires the exact pending call key")
        resolution = {"reason": reason, "resolved_at": time.time(), "reply_sha256": reply_sha256}
        if reply_sha256 is not None:
            reply_path = author.root / record["directory"] / "reply.json"
            limit = author.config["limits"]["response_bytes"]
            with reply_path.open("rb") as handle:
                raw = handle.read(limit + 1)
            if len(raw) > limit:
                raise ValueError("retained reply exceeds response budget")
            if hashlib.sha256(raw).hexdigest() != reply_sha256:
                raise ValueError("retained reply differs from inspected hash")
            reply = json.loads(raw)
            if (not isinstance(reply, dict) or not isinstance(reply.get("artifact"), dict)
                    or not isinstance(reply.get("usage"), dict) or encode(reply) != raw):
                raise ValueError("retained reply must be a canonical provider result")
            record.update(status="succeeded", response_bytes=len(raw), reply_sha256=reply_sha256,
                          usage=reply["usage"])
        else:
            record.update(status="failed", error="Interrupted call retired: " + reason)
        record.update(reconciliation=resolution, finished_at=resolution["resolved_at"])
        save(path, state)
