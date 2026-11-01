"""A frozen manifest and the native CLI validators that remain authoritative."""
import hashlib
import json
from pathlib import Path
import subprocess

from .storage import save


class Session:
    def __init__(self, repo, binary):
        self.repo, self.binary = Path(repo).resolve(), Path(binary).resolve()
        self.root = self.repo / ".gewu"
        self.manifest_path = self.root / "review-input.json"
        raw = self.manifest_path.read_bytes()
        self.manifest = json.loads(raw)
        protocol = hashlib.sha256()
        for path in sorted(Path(__file__).parent.glob("*.py")):
            protocol.update(path.name.encode() + b"\0" + path.read_bytes())
        self.identity = {"manifest_sha256": hashlib.sha256(raw).hexdigest(),
                         "binary_sha256": hashlib.sha256(self.binary.read_bytes()).hexdigest(),
                         "driver_sha256": protocol.hexdigest()}

    def assert_fresh(self):
        if hashlib.sha256(self.manifest_path.read_bytes()).hexdigest() != self.identity["manifest_sha256"]:
            raise ValueError("frozen manifest changed; stop and replan before more author calls")
        if hashlib.sha256(self.binary.read_bytes()).hexdigest() != self.identity["binary_sha256"]:
            raise ValueError("frozen Gewu binary changed; do not mix validation protocols")

    def native(self, *args):
        self.assert_fresh()
        result = subprocess.run([str(self.binary), *args], cwd=self.repo, capture_output=True, text=True, timeout=60)
        if result.returncode:
            raise ValueError(result.stderr[:4096] or result.stdout[:4096])
        return result.stdout

    def read(self, name):
        return json.loads((self.root / name).read_bytes())

    def write(self, name, value):
        path = self.root / name
        save(path, value)
        return str(path)

    def validate_plan(self, plan, ids=()):
        path = self.write("driver/plan-candidate.json", plan)
        args = ["harness", "validate-plan", "--plan", path]
        for identifier in ids:
            args += ["--change-id", identifier]
        self.native(*args)

    def stage(self):
        return self.read("harness/state.json")["stage"]
