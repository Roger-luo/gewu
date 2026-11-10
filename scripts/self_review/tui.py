"""Terminal UI part of rubric version 2: headless frame gates plus a judge.

`gewu review frames` renders every page a reader can PageDown to, exactly as
the TUI composes it. Its checks are exact; the judge covers what reading the
pages takes. Plain-text captures cannot show colour, so styling is unassessed.
"""
import json
from pathlib import Path
import subprocess

SIZES = ((80, 24), (120, 40))
# Judged quality gates at the reference terminal; 80x24 is a degraded mode
# whose scores are recorded. Mechanical gates hold at every size: a clipped or
# unreachable row is a defect wherever it happens.
REFERENCE = "120x40"
DIMENSIONS = ("orientation", "legibility", "page_breaks", "prose_with_code", "density", "context")
MINIMUM = 2


def capture(root, binary):
    """Writes `frames-<size>.json` and a page directory per size."""
    # Absolute, because the binary runs with the run directory as its cwd.
    evidence = Path(root).resolve() / ".gewu/self-review"
    summary = {}
    for width, height in SIZES:
        size = f"{width}x{height}"
        result = subprocess.run(
            [str(binary), "review", "frames", "--width", str(width), "--height", str(height),
             "--pages", str(evidence / f"frames-{size}")],
            cwd=root, capture_output=True, text=True, timeout=300)
        if not result.stdout.strip():
            raise ValueError(f"frames produced no report at {size}: " + result.stderr[:2048])
        (evidence / f"frames-{size}.json").write_text(result.stdout)
        report = json.loads(result.stdout)
        pages = [len(stage["pages"]) for stage in report["stages"][1:-1]]
        repeated = [page["repeated_rows"] for stage in report["stages"] for page in stage["pages"][1:]]
        summary[size] = {"passes": report["passes"], "pages_per_chunk_max": max(pages, default=0),
                         "repeated_rows_max": max(repeated, default=0),
                         "pages_per_chunk_mean": round(sum(pages) / len(pages), 2) if pages else 0}
    return summary


def checked(judge):
    problems = []
    if judge.get("rubric_version") not in (3, 4):
        problems.append("tui judge must use rubric_version 4")
    for group in ("dimensions", "degraded"):
        for name in DIMENSIONS:
            dimension = judge.get(group, {}).get(name, {})
            if dimension.get("score") not in (0, 1, 2, 3):
                problems.append(f"tui {group} {name} needs a score from 0 to 3")
            if not dimension.get("evidence"):
                problems.append(f"tui {group} {name} needs evidence citing page files")
    if problems:
        raise ValueError("tui judge output rejected:\n- " + "\n- ".join(problems))


def score(frames, judge, add):
    """Gates and dimensions for the terminal UI; `add` records each finding."""
    gates = {}
    for size, report in frames.items():
        defects = {"clipped": [], "unreached": [], "uncued_pages": [], "orphaned_pages": [], "stub_pages": []}
        for stage in report["stages"]:
            for kind in defects:
                if stage.get(kind):
                    defects[kind].append(stage["stage"])
                    add("tui", kind, f"{stage['stage']}@{size}", f"{kind}: {stage[kind]}")
        gates[f"tui_fit_{size}"] = not defects["clipped"]
        gates[f"tui_reach_{size}"] = not defects["unreached"]
        gates[f"tui_cue_{size}"] = not defects["uncued_pages"]
        gates[f"tui_orphan_{size}"] = not defects["orphaned_pages"]
        gates[f"tui_stub_{size}"] = not defects["stub_pages"]
    dimensions = {}
    if judge is not None:
        checked(judge)
        dimensions = {name: judge["dimensions"][name]["score"] for name in DIMENSIONS}
        dimensions.update({f"degraded_{name}": judge["degraded"][name]["score"] for name in DIMENSIONS})
        gates.update({f"tui_{name}": dimensions[name] >= MINIMUM for name in DIMENSIONS})
        for item in judge.get("findings", []):
            add("tui-judge", item.get("category", "other"),
                f"{item.get('stage', '?')}@{item.get('size', '?')}#{item.get('page', '?')}", item["detail"])
    return gates, dimensions


def pages(frames):
    """Pages per chunk at each size: what an oversized chunk costs the reader."""
    return {size: [len(stage["pages"]) for stage in report["stages"][1:-1]] for size, report in frames.items()}
