"""Reconcile durable reservations with their retained requests before reuse."""
import json

from .storage import digest, encode


def validate(root, state, config):
    if state["config"] != config:
        raise ValueError("author configuration differs from persisted run")
    limits = config["limits"]
    total, stages, keys = 0, {}, set()
    for index, record in enumerate(state["calls"]):
        directory = f"calls/{index:04d}"
        if record["directory"] != directory or record["status"] not in {"pending", "failed", "succeeded"}:
            raise ValueError("inconsistent author record")
        raw = (root / directory / "request.json").read_bytes()
        payload = json.loads(raw)
        key = digest({"config": config, "payload": payload})
        request_bytes = len(raw) + config.get("request_overhead_bytes", 0)
        reservation = request_bytes + limits["response_bytes"]
        if (encode(payload) != raw or record["key"] != key or key in keys
                or record["stage"] != payload["stage"] or record["request_bytes"] != request_bytes
                or record["reserved_bytes"] != reservation or request_bytes > limits["request_bytes"]):
            raise ValueError("inconsistent author request accounting or hash")
        keys.add(key)
        total += reservation
        stages[record["stage"]] = stages.get(record["stage"], 0) + 1
    if total != state["reserved_bytes"] or total > limits["total_reserved_bytes"]:
        raise ValueError("inconsistent total byte reservation accounting")
    if len(state["calls"]) > limits["max_calls"] or any(
        count > limits["stage_calls"].get(stage, limits["max_calls"]) for stage, count in stages.items()
    ):
        raise ValueError("inconsistent call budget accounting")
