import json
from pathlib import Path
import sys
import tempfile
import time
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from gewu_driver.process import JsonProcess


class AuthorProcessTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)

    def test_exact_request_and_usage_are_retained(self):
        script = 'import json,sys; print(json.dumps({"artifact":json.load(sys.stdin),"usage":{"input_tokens":3}}))'
        process = JsonProcess([sys.executable, "-c", script], self.root)
        reply = process('{"exact":"λ"}')
        self.assertEqual(reply["artifact"], {"exact": "λ"})
        self.assertEqual(reply["usage"]["input_tokens"], 3)
        metadata = json.loads(next(self.root.glob("*/process.json")).read_text())
        self.assertEqual(metadata["status"], "succeeded")
        self.assertEqual(metadata["exit_code"], 0)

    def test_timeout_terminates_the_owned_process_and_retains_evidence(self):
        process = JsonProcess([sys.executable, "-c", "import time; time.sleep(10)"], self.root, timeout=0.1)
        with self.assertRaisesRegex(ValueError, "timeout"):
            process("{}")
        metadata = json.loads(next(self.root.glob("*/process.json")).read_text())
        self.assertEqual(metadata["status"], "failed")
        self.assertLess(metadata["exit_code"], 0)

    def test_raw_output_limit_rejects_a_noisy_provider(self):
        process = JsonProcess([sys.executable, "-c", 'print("x" * 1000000)'], self.root, output_bytes=100)
        with self.assertRaisesRegex(ValueError, "output exceeds"):
            process("{}")
        directory = next(self.root.iterdir())
        self.assertLessEqual(sum((directory / name).stat().st_size for name in ["stdout.txt", "stderr.txt"]), 100)

    def test_descendant_is_stopped_even_when_leader_exits_successfully(self):
        marker = self.root / "descendant-ran"
        script = ("import os,time,json\n"
                  "if os.fork() == 0:\n"
                  " time.sleep(0.3)\n"
                  f" open({str(marker)!r},'w').write('leaked')\n"
                  "else:\n"
                  " print(json.dumps({'artifact':{},'usage':{}}))\n")
        process = JsonProcess([sys.executable, "-c", script], self.root / "evidence")
        self.assertEqual(process("{}"), {"artifact": {}, "usage": {}})
        time.sleep(0.4)
        self.assertFalse(marker.exists())
