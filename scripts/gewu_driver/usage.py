"""Observed provider usage admission checks, never a billing guarantee."""
import hashlib
import json
import warnings


def admit(root, state, limits):
    caps = {key: limits.get("observed_" + key + "_stop") for key in ("input_tokens", "output_tokens")}
    if not any(caps.values()):
        return
    totals = dict.fromkeys(caps, 0)
    for record in state["calls"]:
        usage = record.get("usage")
        if not isinstance(usage, dict):
            raise ValueError("missing provider usage; inspect outcome before more inference")
        if record["status"] == "succeeded":
            raw = (root / record["directory"] / "reply.json").read_bytes()
            if (hashlib.sha256(raw).hexdigest() != record["reply_sha256"]
                    or json.loads(raw).get("usage") != usage):
                raise ValueError("provider usage differs from retained reply")
        for key in totals:
            value = usage.get(key)
            if type(value) is not int or value < 0:
                raise ValueError("missing or invalid provider usage; inspect before more inference")
            totals[key] += value
        # Claude reports cache reads/writes separately; Codex's cached_input_tokens
        # is already a subset of input_tokens and must not be added again.
        for key in ("cache_read_input_tokens", "cache_creation_input_tokens"):
            value = usage.get(key, 0)
            if type(value) is not int or value < 0:
                raise ValueError("invalid provider cache usage")
            totals["input_tokens"] += value
    if any(cap is not None and totals[key] >= cap for key, cap in caps.items()):
        raise ValueError("observed provider usage stop reached; no further inference")
    warning = limits.get("observed_input_tokens_warning")
    if warning is not None and totals["input_tokens"] >= warning:
        warnings.warn("Observed input usage warning reached; inspect remaining work and budget", RuntimeWarning)
