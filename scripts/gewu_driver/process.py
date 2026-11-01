"""Bounded subprocess adapters; importing this module never invokes an author."""
import hashlib
import json
import os
from pathlib import Path
import signal
import selectors
import subprocess
import tempfile
import time

from .storage import save


class JsonProcess:
    """An argv-only process returning {artifact: object, usage: object} on stdout."""
    def __init__(self, command, evidence, timeout=180, output_bytes=131_072):
        if timeout <= 0 or output_bytes <= 0 or not command:
            raise ValueError("positive process limits and a command are required")
        self.command = list(command)
        self.evidence = Path(evidence)
        self.timeout = timeout
        self.output_bytes = output_bytes
        self.identity = {"command": self.command, "timeout": timeout, "raw_output_bytes": output_bytes,
                         "adapter_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest()}

    def __call__(self, request):
        directory = self.evidence / hashlib.sha256(request.encode()).hexdigest()
        directory.mkdir(parents=True, exist_ok=True)
        (directory / "request.txt").write_text(request)
        started = time.monotonic()
        status = {"status": "pending"}
        process = None
        try:
            with tempfile.TemporaryDirectory(prefix="gewu-author-") as scratch, \
                    (directory / "request.txt").open("rb") as source, \
                    (directory / "stdout.txt").open("wb") as stdout, \
                    (directory / "stderr.txt").open("wb") as stderr:
                process = subprocess.Popen(self.command, stdin=source, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                           cwd=scratch, start_new_session=True)
                status["pid"] = process.pid
                save(directory / "process.json", status)
                retained = 0
                with selectors.DefaultSelector() as selector:
                    selector.register(process.stdout, selectors.EVENT_READ, stdout)
                    selector.register(process.stderr, selectors.EVENT_READ, stderr)
                    while selector.get_map():
                        if time.monotonic() - started > self.timeout:
                            raise ValueError("author process timeout")
                        if process.poll() is not None:
                            # A completed leader does not entitle descendants to
                            # keep running or holding the output pipes open.
                            self._kill_group(process)
                        for key, _ in selector.select(0.02):
                            remaining = self.output_bytes - retained
                            chunk = os.read(key.fileobj.fileno(), min(65_536, remaining + 1))
                            if not chunk:
                                selector.unregister(key.fileobj)
                                key.fileobj.close()
                                continue
                            kept = chunk[:remaining]
                            key.data.write(kept)
                            retained += len(kept)
                            status["retained_output_bytes"] = retained
                            if len(chunk) > remaining:
                                status["output_truncated"] = True
                                raise ValueError("author process output exceeds byte limit")
                try:
                    process.wait(timeout=max(0.001, self.timeout - (time.monotonic() - started)))
                except subprocess.TimeoutExpired as error:
                    raise ValueError("author process timeout") from error
                status["exit_code"] = process.returncode
                if process.returncode:
                    raise ValueError(f"author process exited {process.returncode}; see {directory}")
            status["status"] = "succeeded"
            return self.decode_stdout((directory / "stdout.txt").read_bytes())
        except BaseException as error:
            status.update(status="failed", error=str(error)[:2048])
            raise
        finally:
            if process is not None:
                self._kill_group(process)
                process.wait()
                for pipe in (process.stdout, process.stderr):
                    if pipe and not pipe.closed:
                        pipe.close()
                status["exit_code"] = process.returncode
            status["elapsed_seconds"] = time.monotonic() - started
            save(directory / "process.json", status)

    @staticmethod
    def _kill_group(process):
        try:
            os.killpg(process.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass

    @staticmethod
    def decode(value):
        return value

    def decode_stdout(self, raw):
        return self.decode(json.loads(raw))


class ClaudeProcess(JsonProcess):
    """Isolated, tool-free call with a model pinned in the persisted profile."""
    def __init__(self, evidence, model, max_budget_usd=0.25, **limits):
        if not isinstance(model, str) or not model.strip():
            raise ValueError("pin an author model before creating a live provider")
        if not isinstance(max_budget_usd, (int, float)) or not 0 < max_budget_usd <= 10:
            raise ValueError("pin a positive per-call provider budget at most 10 USD")
        command = ["claude", "-p", "--output-format", "json", "--max-turns", "1", "--tools", "",
                   "--safe-mode", "--strict-mcp-config", "--mcp-config", '{"mcpServers":{}}',
                   "--disable-slash-commands", "--no-session-persistence", "--permission-prompts", "none",
                   "--setting-sources", ""]
        command += ["--model", model, "--max-budget-usd", str(max_budget_usd)]
        super().__init__(command, evidence, **limits)

    @staticmethod
    def decode(value):
        if value.get("is_error"):
            raise ValueError("author provider error: " + str(value.get("result"))[:2048])
        text = value.get("result", "").strip()
        if text.startswith("```json\n") and text.endswith("\n```"):
            text = text[8:-4]
        return {"artifact": json.loads(text), "usage": value.get("usage", {})}
