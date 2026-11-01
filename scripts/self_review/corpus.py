"""Frozen corpora from Gewu's own history, materialized where nothing moves.

A corpus is a base and head revision of this repository, optionally narrowed to
a scope. Each is copied into a new two-commit repository with a private copy of
the binary under test, so rebuilding Gewu or editing its source cannot change a
review in progress, and a synthetic reader never touches a human session.
"""
import hashlib
import io
import json
from pathlib import Path
import shutil
import subprocess
import tarfile

from gewu_driver.evaluation import advance_synthetic_batch
from gewu_driver.session import Session
from gewu_driver.storage import save

REGISTRY = Path(__file__).with_name("corpora.json")
EMPTY = "empty"
WORKTREE = "WORKTREE"


def registry():
    return json.loads(REGISTRY.read_text())


def named(name):
    corpora = registry()["corpora"]
    if name not in corpora:
        raise ValueError(f"unknown corpus {name!r}; known: {', '.join(sorted(corpora))}")
    return {"name": name, **corpora[name]}


def git(root, *args, stdin=None):
    return subprocess.run(["git", *args], cwd=root, input=stdin, capture_output=True,
                          check=True, timeout=120).stdout


def source_state(source):
    """Identifies the Gewu source a binary was built from, dirty edits included."""
    outside = ("--", ".", ":(exclude,top).gewu")
    untracked = git(source, "ls-files", "-o", "--exclude-standard", "-z", *outside).split(b"\0")
    dirty = hashlib.sha256(git(source, "diff", "HEAD", "--binary", *outside))
    for name in sorted(path for path in untracked if path):
        path = Path(source) / name.decode()
        if path.is_file() and not path.is_symlink():
            dirty.update(name + b"\0" + path.read_bytes())
    return {"head": git(source, "rev-parse", "HEAD").decode().strip(), "dirty_sha256": dirty.hexdigest()}


def files_at(source, revision, scope):
    """Path to bytes for one side of the corpus."""
    if revision == EMPTY:
        return {}
    if revision == WORKTREE:
        listed = git(source, "ls-files", "-co", "--exclude-standard", "-z", "--", *scope).split(b"\0")
        return {name.decode(): (Path(source) / name.decode()).read_bytes()
                for name in listed if name and (Path(source) / name.decode()).is_file()}
    archive = git(source, "archive", "--format=tar", revision, "--", *scope)
    files = {}
    with tarfile.open(fileobj=io.BytesIO(archive)) as tar:
        for member in tar.getmembers():
            if member.isfile():
                files[member.name] = tar.extractfile(member).read()
    return files


def resolve(source, revision):
    if revision in (EMPTY, WORKTREE):
        return revision
    return git(source, "rev-parse", "--verify", revision + "^{commit}").decode().strip()


def freeze(source, corpus, destination, binary):
    source, destination = Path(source).resolve(), Path(destination).resolve()
    if destination.exists():
        raise ValueError("corpus destination must be new")
    if destination.is_relative_to(source) and not destination.is_relative_to(source / ".gewu"):
        raise ValueError("materialize corpora outside the tracked tree, for example under .gewu/")
    scope = list(corpus.get("scope", []))
    base, head = resolve(source, corpus["base"]), resolve(source, corpus["head"])
    if scope:
        # The diff covers the scope only, but the author reads the whole tree:
        # a scoped copy once held just its three files, and the author cited
        # `git/tests.rs`, which existed upstream but not in the copy.
        context = files_at(source, head, [])
        scoped = set(files_at(source, head, scope)) | set(files_at(source, base, scope))
        unchanged = {name: data for name, data in context.items() if name not in scoped}
        sides = [("base", {**unchanged, **files_at(source, base, scope)}), ("head", context)]
    else:
        sides = [("base", files_at(source, base, scope)), ("head", files_at(source, head, scope))]
    state = source_state(source)
    if not sides[1][1]:
        raise ValueError("corpus head has no files in scope")
    destination.mkdir(parents=True)
    git(destination, "init", "-q")
    git(destination, "config", "user.name", "Gewu self-review fixture")
    git(destination, "config", "user.email", "self-review@example.invalid")
    commits = {}
    for side, files in sides:
        for child in destination.iterdir():
            if child.name not in (".git", ".gewu"):
                shutil.rmtree(child) if child.is_dir() else child.unlink()
        for name, data in files.items():
            (destination / name).parent.mkdir(parents=True, exist_ok=True)
            (destination / name).write_bytes(data)
        git(destination, "add", "-A")
        git(destination, "commit", "-q", "--allow-empty", "-m", f"test: frozen {side} of {corpus['name']}")
        commits[side] = git(destination, "rev-parse", "HEAD").decode().strip()
    private = destination / ".gewu/self-review/gewu"
    private.parent.mkdir(parents=True)
    shutil.copy2(binary, private)
    subprocess.run([str(private), "harness", "start", "--base", commits["base"], "--head", commits["head"]],
                   cwd=destination, capture_output=True, check=True, timeout=60)
    head_files = sides[1][1]
    # The marker `advance_synthetic_batch` requires before it fakes a reader.
    save(destination / ".gewu/evaluation-only.json", {
        "purpose": "self-review evaluation; synthetic reads are not human review or approval",
        "root": str(destination), "source": str(source), "commit": commits["head"],
        "files": {name: hashlib.sha256(data).hexdigest() for name, data in head_files.items()}})
    run = {"corpus": corpus, "resolved": {"base": base, "head": head}, "commits": commits,
           "binary_sha256": hashlib.sha256(private.read_bytes()).hexdigest(),
           "gewu_source": state, "rubric_version": registry()["rubric_version"]}
    save(destination / ".gewu/self-review/run.json", run)
    return run


class Run:
    """One materialized corpus and the evidence gathered against it."""

    def __init__(self, root):
        self.root = Path(root).resolve()
        self.evidence = self.root / ".gewu/self-review"
        self.binary = self.evidence / "gewu"
        self.meta = json.loads((self.evidence / "run.json").read_text())

    def read(self, name):
        return json.loads((self.evidence / name).read_text())

    def write(self, name, value):
        save(self.evidence / name, value)

    def advance(self):
        """Stands in for the human between batches, only inside a marked copy."""
        advance_synthetic_batch(Session(self.root, self.binary))

    def audit(self):
        result = subprocess.run([str(self.binary), "review", "audit", "--format", "json"],
                                cwd=self.root, capture_output=True, text=True, timeout=120)
        if not result.stdout.strip():
            raise ValueError("audit produced no report: " + result.stderr[:2048])
        report = json.loads(result.stdout)
        self.write("audit.json", report)
        return report

    def spec(self):
        return json.loads((self.root / ".gewu/review-spec.json").read_text())
