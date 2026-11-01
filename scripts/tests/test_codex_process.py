import json
from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from gewu_driver.codex_process import CodexProcess


class CodexProcessTests(unittest.TestCase):
    def test_jsonl_requires_one_response_usage_and_no_tool_activity(self):
        with tempfile.TemporaryDirectory() as root:
            provider = CodexProcess(root, "fixture-model")
            events = [{"type": "item.completed", "item": {"type": "agent_message", "text": '{"valid":true}'}},
                      {"type": "turn.completed", "usage": {"input_tokens": 12, "output_tokens": 4}}]
            encode = lambda records: "\n".join(json.dumps(record) for record in records).encode()
            reply = provider.decode_stdout(encode(events))
            self.assertEqual(reply["artifact"], {"valid": True})
            self.assertEqual(reply["usage"]["input_tokens"], 12)
            commentary = {"type": "item.completed", "item": {"type": "agent_message", "text": "Inspecting supplied source."}}
            self.assertEqual(provider.decode_stdout(encode([commentary] + events)), reply)
            for invalid in [events[:1], events + events, events + [{"type": "turn.failed"}],
                            [{"type": "item.completed", "item": {"type": "command_execution"}}] + events]:
                with self.assertRaises(ValueError):
                    provider.decode_stdout(encode(invalid))
            for event_type in ("item.started", "item.updated"):
                with self.assertRaisesRegex(ValueError, "tool activity"):
                    provider.decode_stdout(encode([{"type": event_type, "item": {"type": "command_execution"}}] + events))
            self.assertIn("--ignore-user-config", provider.command)
            self.assertIn("read-only", provider.command)
