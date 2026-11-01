"""Ephemeral, read-only Codex author with retained JSONL usage evidence."""
import json
import hashlib
from pathlib import Path

from .process import JsonProcess


class CodexProcess(JsonProcess):
    def __init__(self, evidence, model, **limits):
        if not isinstance(model, str) or not model.strip():
            raise ValueError("pin an author model")
        instructions = Path(__file__).with_name("codex_author.txt")
        self.request_overhead_bytes = len(instructions.read_bytes())
        command = ["codex", "exec", "--ignore-user-config", "--ephemeral", "--skip-git-repo-check",
                   "--sandbox", "read-only", "--model", model, "--json",
                   "-c", 'approval_policy="never"', "-c", 'model_reasoning_effort="low"',
                   "-c", 'web_search="disabled"', "-c", "project_doc_max_bytes=0",
                   "-c", "model_instructions_file=" + json.dumps(str(instructions))]
        for feature in ("shell_tool", "apps", "plugins", "hooks", "multi_agent", "computer_use",
                        "browser_use", "browser_use_external", "view_image", "memories", "goals"):
            command += ["--disable", feature]
        command += ["-"]
        super().__init__(command, evidence, **limits)
        self.identity["instructions_sha256"] = hashlib.sha256(instructions.read_bytes()).hexdigest()

    def __call__(self, request):
        raw = Path(__file__).with_name("codex_author.txt").read_bytes()
        if hashlib.sha256(raw).hexdigest() != self.identity["instructions_sha256"]:
            raise ValueError("author instructions changed; stop before inference")
        return super().__call__(request)

    def decode_stdout(self, raw):
        events = [json.loads(line) for line in raw.splitlines() if line.strip()]
        if any(event.get("type") in ("error", "turn.failed") for event in events):
            raise ValueError("Codex author failed; inspect retained JSONL")
        messages, usage = [], None
        for event in events:
            if event.get("type") in ("item.started", "item.updated", "item.completed"):
                if event.get("item", {}).get("type") not in ("agent_message", "reasoning"):
                    raise ValueError("unexpected author tool activity; inspect retained JSONL")
            if event.get("type") == "item.completed":
                item = event.get("item", {})
                if item.get("type") == "agent_message":
                    messages.append(item["text"])
                elif item.get("type") != "reasoning":
                    raise ValueError("unexpected author tool activity; inspect retained JSONL")
            elif event.get("type") == "turn.completed":
                if usage is not None:
                    raise ValueError("unexpected multiple author turns")
                usage = event.get("usage")
        if not messages or not isinstance(usage, dict):
            raise ValueError("author requires a final JSON response and usage evidence")
        # CLI emits commentary as agent_message too. Its last completed
        # message is the response; earlier commentary remains in raw evidence.
        text = messages[-1].strip()
        if text.startswith("```json\n") and text.endswith("\n```"):
            text = text[8:-4]
        return {"artifact": json.loads(text), "usage": usage}
