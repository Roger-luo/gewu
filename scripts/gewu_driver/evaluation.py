"""Isolated generation evaluation. Synthetic reads never touch a user session."""
import hashlib
import json
from pathlib import Path
import shutil
import subprocess

from .storage import save


def create(source, paths, destination, binary):
    source, destination = Path(source).resolve(), Path(destination).resolve()
    if destination.exists():
        raise ValueError("evaluation destination must be new")
    destination.mkdir(parents=True)
    manifest = {}
    for name in paths:
        relative = Path(name)
        if relative.is_absolute() or ".." in relative.parts or relative.parts[0] in {".git", ".gewu"}:
            raise ValueError("evaluation source must be a repository-relative file")
        original = source / relative
        if original.is_symlink() or not original.is_file():
            raise ValueError("evaluation copies regular source files only")
        target = destination / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(original, target)
        manifest[name] = hashlib.sha256(target.read_bytes()).hexdigest()
    if not manifest:
        raise ValueError("evaluation requires a nonempty scope")
    def git(*args, stdin=None):
        return subprocess.run(["git", *args], cwd=destination, input=stdin, text=True,
                              capture_output=True, check=True, timeout=30).stdout.strip()
    git("init", "-q")
    git("config", "user.name", "Gewu evaluation fixture")
    git("config", "user.email", "evaluation@example.invalid")
    git("add", "--", *paths)
    git("commit", "-qm", "test: frozen evaluation corpus")
    empty = git("hash-object", "-w", "-t", "tree", "--stdin", stdin="")
    subprocess.run([str(Path(binary).resolve()), "harness", "start", "--base", empty],
                   cwd=destination, capture_output=True, check=True, timeout=30)
    save(destination / ".gewu/evaluation-only.json", {
        "purpose": "generation evaluation; synthetic reads are not human review or approval",
        "root": str(destination), "source": str(source), "files": manifest, "commit": git("rev-parse", "HEAD")})
    return destination


def advance_synthetic_batch(session):
    marker = session.read("evaluation-only.json")
    if marker["root"] != str(session.repo) or session.repo == Path(marker["source"]):
        raise ValueError("synthetic reads require an isolated marked evaluation copy")
    for name, expected in marker["files"].items():
        if hashlib.sha256((session.repo / name).read_bytes()).hexdigest() != expected:
            raise ValueError("evaluation corpus changed; synthetic advancement refused")
    if session.stage() != "await":
        raise ValueError("synthetic reads require an accepted evaluation batch")
    spec = session.read("review-spec.json")
    path = session.root / "reviews" / spec["review_id"] / "events.jsonl"
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("a") as handle:
        for chunk in spec["chunks"]:
            handle.write(json.dumps({"at_unix_ms": 0, "type": "review_changed", "chunk_id": chunk["id"],
                                     "reviewed": True, "evaluation_only": True}) + "\n")
    session.native("harness", "next")
