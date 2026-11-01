"""Provider-independent, durable limits around complete author requests.

Byte reservations bound agent-supplied prompts and replies, not a model's
unobservable internal context. Provider-reported token usage is retained as
evidence. No retry or reconciliation of an ambiguous call happens implicitly.
"""
from dataclasses import asdict, dataclass, field
import hashlib
import json
from pathlib import Path
from typing import Optional
import time

from .storage import digest, encode, locked, save
from .accounting import validate
from .usage import admit


@dataclass(frozen=True)
class Limits:
    request_bytes: int = 32_768
    response_bytes: int = 16_384
    total_reserved_bytes: int = 1_048_576
    max_calls: int = 32
    stage_calls: dict = field(default_factory=dict)
    observed_input_tokens_stop: Optional[int] = None
    observed_output_tokens_stop: Optional[int] = None
    observed_input_tokens_warning: Optional[int] = None


class BudgetedAuthor:
    def __init__(self, root, limits, identity, invoke, before_call=None):
        self.root = Path(root)
        values = asdict(limits)
        for name in list(values):
            if name.startswith("observed_"):
                if values[name] is None:
                    del values[name]  # Preserve legacy profiles when the guard is disabled.
                elif type(values[name]) is not int or values[name] <= 0:
                    raise ValueError("observed usage limits must be positive integers")
        if (limits.observed_input_tokens_warning is not None and
                (limits.observed_input_tokens_stop is None or
                 limits.observed_input_tokens_warning >= limits.observed_input_tokens_stop)):
            raise ValueError("usage warning must be below the input stop")
        for name in ("request_bytes", "response_bytes", "total_reserved_bytes", "max_calls"):
            if type(values[name]) is not int or values[name] <= 0:
                raise ValueError("author limits must be positive integers")
        if any(type(value) is not int or value <= 0 for value in values["stage_calls"].values()):
            raise ValueError("stage limits must be positive integers")
        # Freeze the exact supplied profile, even if the caller mutates its dicts.
        overhead = getattr(invoke, "request_overhead_bytes", 0)
        if type(overhead) is not int or overhead < 0:
            raise ValueError("provider request overhead must be a nonnegative integer")
        self.config = json.loads(encode({"schema_version": 1, "limits": values, "identity": identity,
                                       "request_overhead_bytes": overhead}))
        self.invoke = invoke
        self.before_call = before_call

    def call(self, stage, request):
        if stage not in {"investigate", "synthesize", "plan", "expand", "repair", "revise"}:
            raise ValueError("unknown author stage")
        payload = {"stage": stage, "request": request}
        raw = encode(payload)
        limits = self.config["limits"]
        request_bytes = len(raw) + self.config["request_overhead_bytes"]
        if request_bytes > limits["request_bytes"]:
            raise ValueError("complete author request bytes exceed budget")
        key = digest({"config": self.config, "payload": payload})
        with locked(self.root):
            if self.before_call is not None:
                self.before_call()
            state_path = self.root / "state.json"
            state = json.loads(state_path.read_bytes()) if state_path.exists() else {
                "config": self.config, "reserved_bytes": 0, "calls": []}
            validate(self.root, state, self.config)
            for record in state["calls"]:
                if record["key"] == key:
                    return self._cached(record, raw)
            if any(record["status"] == "pending" for record in state["calls"]):
                raise ValueError("pending call has an unknown outcome; inspect its evidence before continuing")
            admit(self.root, state, limits)
            if len(state["calls"]) >= limits["max_calls"]:
                raise ValueError("author call budget exhausted")
            stage_count = sum(record["stage"] == stage for record in state["calls"])
            if stage_count >= limits["stage_calls"].get(stage, limits["max_calls"]):
                raise ValueError("author stage call budget exhausted")
            reservation = request_bytes + limits["response_bytes"]
            if state["reserved_bytes"] + reservation > limits["total_reserved_bytes"]:
                raise ValueError("author total byte reservation budget exhausted")
            directory = self.root / "calls" / f"{len(state['calls']):04d}"
            save(directory / "request.json", payload)
            record = {"key": key, "stage": stage, "status": "pending", "directory": str(directory.relative_to(self.root)),
                      "request_bytes": request_bytes, "reserved_bytes": reservation, "started_at": time.time()}
            state["calls"].append(record)
            state["reserved_bytes"] += reservation
            # Charge before inference. A crash never refunds an uncertain call.
            save(state_path, state)
            try:
                reply = self.invoke(raw.decode())
                reply_bytes = encode(reply)
                record["response_bytes"] = len(reply_bytes)
                if isinstance(reply, dict) and isinstance(reply.get("usage"), dict) and len(encode(reply["usage"])) <= 4096:
                    record["usage"] = reply["usage"]
                if len(reply_bytes) > limits["response_bytes"]:
                    raise ValueError("author response bytes exceed budget")
                if not isinstance(reply, dict) or not isinstance(reply.get("artifact"), dict) or not isinstance(reply.get("usage"), dict):
                    raise ValueError("provider must return an artifact object and a usage object")
                save(directory / "reply.json", reply)
                record.update(status="succeeded", response_bytes=len(reply_bytes),
                              reply_sha256=hashlib.sha256(reply_bytes).hexdigest(), usage=reply["usage"])
            except Exception as error:
                record.update(status="failed", error=str(error)[:2048], finished_at=time.time())
                save(state_path, state)
                raise
            record["finished_at"] = time.time()
            save(state_path, state)
            return reply["artifact"]

    def _cached(self, record, request):
        if record["status"] != "succeeded":
            raise ValueError(f"previous author call is {record['status']}; no automatic retry")
        directory = self.root / record["directory"]
        if (directory / "request.json").read_bytes() != request:
            raise ValueError("cached request hash mismatch")
        raw = (directory / "reply.json").read_bytes()
        if hashlib.sha256(raw).hexdigest() != record["reply_sha256"]:
            raise ValueError("cached reply hash mismatch")
        return json.loads(raw)["artifact"]
