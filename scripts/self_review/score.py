"""Rubric version 4: mechanical audit gates plus a checked judge verdict.

The judge's labels are evidence, not trust: the score refuses a judgement that
skips, duplicates, or invents a sentence, or that scores without evidence.
"""
import hashlib

from . import tui

RUBRIC_VERSION = 4
LABELS = {"fact", "decision", "anchor", "orientation", "redundant", "filler", "narrates-review",
          "unsupported", "verdict"}
KEPT = {"fact", "decision", "anchor", "orientation"}
DIMENSIONS = ("reference_usefulness", "decision_surfacing", "change_framing", "learning_order",
              "big_picture", "chunk_connection", "representation_contrast")
QUESTION_KINDS = {"purpose": 2, "connection": 2, "behaviour": 2}
# A reviewer who read only the walkthrough must get five of six right.
COLD_READER = 5 / 6
MINIMALITY = 0.9
DIMENSION_MINIMUM = 2


def checked(units, judge):
    problems = []
    if judge.get("rubric_version") != RUBRIC_VERSION:
        problems.append(f"judge must use rubric_version {RUBRIC_VERSION}")
    expected = {unit["id"] for unit in units}
    seen = {}
    for label in judge.get("labels", []):
        identifier = label.get("id")
        if identifier in seen:
            problems.append(f"{identifier} labelled twice")
        seen[identifier] = label
        if identifier not in expected:
            problems.append(f"{identifier} is not a sentence id")
        if label.get("label") not in LABELS:
            problems.append(f"{identifier} has unknown label {label.get('label')!r}")
        elif label["label"] not in KEPT and not str(label.get("reason", "")).strip():
            problems.append(f"{identifier} is labelled {label['label']} without a reason")
    missing = sorted(expected - seen.keys(), key=lambda name: int(name[1:]))
    if missing:
        problems.append("unlabelled sentences: " + ", ".join(missing))
    for name in DIMENSIONS:
        dimension = judge.get("dimensions", {}).get(name, {})
        if dimension.get("score") not in (0, 1, 2, 3):
            problems.append(f"dimension {name} needs a score from 0 to 3")
        if not dimension.get("evidence"):
            problems.append(f"dimension {name} needs evidence citing chunks")
    problems.extend(question_problems(judge.get("questions", [])))
    if problems:
        raise ValueError("judge output rejected:\n- " + "\n- ".join(problems))
    return seen


def question_problems(questions):
    problems, kinds, ids = [], {}, set()
    for question in questions:
        identifier = question.get("id")
        if identifier in ids:
            problems.append(f"question {identifier} appears twice")
        ids.add(identifier)
        kinds[question.get("kind")] = kinds.get(question.get("kind"), 0) + 1
        options = question.get("options", [])
        if len(options) != 4 or question.get("answer") not in range(4):
            problems.append(f"question {identifier} needs four options and an answer index 0-3")
    if kinds != QUESTION_KINDS:
        problems.append(f"questions must be two each of purpose, connection, behaviour; got {kinds}")
    return problems


def cold_pack(judge):
    """The questions without their answers, for a reader who must not see them."""
    return [{key: question[key] for key in ("id", "kind", "question", "options")}
            for question in judge["questions"]]


def cold_score(judge, reading):
    answers = reading.get("answers", {})
    missing = [q["id"] for q in judge["questions"] if q["id"] not in answers]
    if missing:
        raise ValueError("cold reader left questions unanswered: " + ", ".join(missing))
    wrong = [q for q in judge["questions"] if answers[q["id"]] != q["answer"]]
    return 1 - len(wrong) / len(judge["questions"]), wrong


def key(corpus, *parts):
    return hashlib.sha256("\0".join([corpus, *map(str, parts)]).encode()).hexdigest()[:16]


def findings(corpus, units, labels, judge, audit, friction):
    """Everything that should be fixed, each with a key stable across runs."""
    found = []

    def add(source, category, location, detail, identity=None):
        found.append({"key": key(corpus, source, category, location, identity or detail), "corpus": corpus,
                      "source": source, "category": category, "location": location, "detail": detail})

    coverage = audit["coverage"]
    for path in coverage["missing_paths"]:
        add("audit", "scope-missing", path, f"{path} never reached the manifest")
    for change in coverage["changes"]:
        for span in change["unexplained"]:
            add("audit", "unexplained", change["path"],
                f"lines {span['start']}-{span['end']} of {change['change_id']} are shown but not narrated",
                change["change_id"] + str(span))
    for item in audit.get("dangling_titles", []):
        add("audit", "dangling-title", item["chunk_id"], f"refers to “{item['title']}”, which no chunk is titled")
    for item in audit.get("split_sentences", []):
        add("audit", "split-sentence", item["chunk_id"], f"{item['path']} view ends mid-sentence at body line {item['line']}")
    for item in audit["references"].get("invisible", []):
        add("audit", "invisible-line", item["chunk_id"], f"`{item['citation']}` is in no view's gutter")
    for item in audit["references"]["unresolved"]:
        add("audit", "unresolved-reference", item["location"], f"`{item['reference']}` names nothing")
    for unit in units:
        label = labels[unit["id"]]
        if label["label"] not in KEPT:
            add("judge", label["label"], unit["location"], f"{unit['text']} — {label['reason']}", unit["text"])
    for item in judge.get("findings", []):
        add("judge", item.get("category", "other"), item.get("chunk", "overview"), item["detail"])
    for item in friction:
        add("friction", item.get("category", "friction"), item.get("command", "harness"), item["problem"])
    return found


def score(corpus, units, judge, audit, friction=(), frames=None, tui_judge=None, reading=None):
    """`frames` maps a size such as `80x24` to its `gewu review frames` report.
    Without frames and a TUI judgement the UI is unassessed, and an unassessed
    dimension is never an implicit pass."""
    labels = checked(units, judge)
    counts = {name: 0 for name in sorted(LABELS)}
    for label in labels.values():
        counts[label["label"]] += 1
    kept = sum(counts[name] for name in KEPT)
    minimality = kept / len(units) if units else 1.0
    dimensions = {name: judge["dimensions"][name]["score"] for name in DIMENSIONS}
    gates = {
        "audit_coverage": not audit["coverage"]["missing_paths"]
        and all(not change["unexplained"] for change in audit["coverage"]["changes"]),
        "audit_references": not audit["references"]["unresolved"],
        "accuracy": counts["unsupported"] == 0,
        "human_judgement": counts["verdict"] == 0,
        "minimality": minimality >= MINIMALITY,
        **{name: value >= DIMENSION_MINIMUM for name, value in dimensions.items()},
    }
    found = findings(corpus, units, labels, judge, audit, list(friction))

    def add(source, category, location, detail):
        found.append({"key": key(corpus, source, category, location, detail), "corpus": corpus,
                      "source": source, "category": category, "location": location, "detail": detail})

    cold = {"status": "unassessed"}
    if reading is not None:
        accuracy, wrong = cold_score(judge, reading)
        gates["cold_reader"] = accuracy >= COLD_READER
        cold = {"status": "assessed", "accuracy": round(accuracy, 3), "summary": reading.get("summary", ""),
                "lost": reading.get("lost", [])}
        for question in wrong:
            add("cold-reader", f"missed-{question['kind']}", question["id"],
                f"{question['question']} — expected “{question['options'][question['answer']]}”")
        for place in reading.get("lost", []):
            add("cold-reader", "lost", "walkthrough", place)

    ui = {"status": "unassessed"}
    if frames:
        tui_gates, tui_dimensions = tui.score(frames, tui_judge, add)
        gates.update(tui_gates)
        ui = {"status": "assessed" if tui_judge is not None else "gates-only",
              "dimensions": tui_dimensions, "pages_per_chunk": tui.pages(frames)}
    return {"rubric_version": RUBRIC_VERSION, "corpus": corpus,
            "passes": all(gates.values()) and ui["status"] == "assessed" and cold["status"] == "assessed",
            "gates": gates, "cold_reader": cold,
            "minimality": round(minimality, 3), "labels": counts, "dimensions": dimensions, "tui": ui,
            "reported": {"forward": len(audit["references"]["forward"]),
                         "distant": len(audit["references"]["distant"])},
            "findings": found}
