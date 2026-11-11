"""`build`: score walkthroughs, sample the chunks to label, and write blind batches."""

import json
import os
import random
import subprocess

from stats import allocate

PRICE_PER_TOKEN = 0.042 / 1e6

LABELLING = """# Label walkthrough chunks for cognitive load

You are labelling chunks of a code-review walkthrough, to calibrate Gewu's
cognitive-load score (rubric v{version}). Your labels are the reference the
score is judged against.

{labelling}

## Inputs

Your batch directory holds one file per item, `<item>.md`, the chunk exactly
as the scorer reads it. Its first line names the questions that apply to it.
`# Earlier chunks in this walkthrough` lists what the reader has already read;
`# This chunk: …` is the chunk to label, with prose, quoted code, explainers
(`[explainer]` JSON, which the reader sees drawn), and any `[context pane]`.
Read only your batch's files and this one.

## Labels

- `overloaded`: {overloaded}
- Each question the item's first line names, yes or no, as worded below.

{questions}

## Output

Write `labels.jsonl` in your batch directory, one JSON object per item:

```json
{{"item": "w0-017", "overloaded": true, "reason": "…",
 "answers": {{"dense_code": {{"yes": true, "reason": "…"}}}}}}
```

Include exactly the questions the item's first line names, and label every
item exactly once. Reply with how many items you labelled and how many you
marked overloaded.
"""


def gewu_json(gewu, *args):
    out = subprocess.run([gewu, *args], check=True, capture_output=True, text=True)
    return json.loads(out.stdout)


def read_rubric(gewu, path):
    return gewu_json(gewu, "review", "rubric", "--format", "json",
                     *(["--rubric", path] if path else []))


def state_dir(path):
    """A walkthrough's `.gewu` directory, given it or the checkout holding it."""
    path = os.path.abspath(path)
    nested = os.path.join(path, ".gewu")
    return nested if os.path.isdir(nested) else path


def labelling(rubric):
    questions = "\n".join(
        f"- `{q['name']}`: {q['instructions']} Yes: {q['when_true']} No: {q['when_false']}"
        for q in rubric["question"])
    guidance = rubric["guidance"]
    return LABELLING.format(version=rubric["version"], labelling=guidance["labelling"].strip(),
                            overloaded=guidance["overloaded"].strip(), questions=questions)


def split(walkthroughs, held_out, rng):
    """Held-out walkthroughs: those named, or a seeded half. Never all, never none."""
    if held_out:
        held = {state_dir(path) for path in held_out}
    else:
        shuffled = walkthroughs[:]
        rng.shuffle(shuffled)
        held = set(shuffled[: len(shuffled) // 2])
    if not held or held >= set(walkthroughs):
        raise SystemExit("calibration needs at least one walkthrough to fit and one held out")
    return held


def sample(chunks, count, rng):
    """`count` chunks, split between test and other kinds in proportion."""
    tests = [c for c in chunks if c["kind"] == "test"]
    others = [c for c in chunks if c["kind"] != "test"]
    take_tests = round(count * len(tests) / len(chunks)) if chunks else 0
    picked = rng.sample(tests, min(take_tests, len(tests)))
    return picked + rng.sample(others, min(count - len(picked), len(others)))


def build(args):
    rng = random.Random(args.seed)
    rubric = read_rubric(args.gewu, args.rubric)
    walkthroughs = [state_dir(path) for path in args.walkthrough]
    held = split(walkthroughs, args.held_out, rng)
    os.makedirs(args.out, exist_ok=True)
    scored = {}
    for index, path in enumerate(walkthroughs):
        scored[index] = gewu_json(
            args.gewu, "review", "load", "--jev", "--with-state", "--format", "json",
            "--input", os.path.join(path, "review-input.json"),
            "--spec", os.path.join(path, "review-spec.json"),
            *(["--rubric", args.rubric] if args.rubric else []))
    share = allocate({i: len(v["chunks"]) for i, v in scored.items()}, args.sample, args.minimum)
    items, jev, chars = [], [], 0
    for index, value in scored.items():
        chunks = value["chunks"]
        chars += sum(len(c["state"]) for c in chunks)
        position = {c["id"]: n for n, c in enumerate(chunks)}
        for chunk in sorted(sample(chunks, share[index], rng), key=lambda c: position[c["id"]]):
            item = f"w{index}-{position[chunk['id']]:03d}"
            items.append({"item": item, "walkthrough": index, "chunk": chunk["id"],
                          "kind": chunk["kind"], "questions": sorted(chunk["answers"]),
                          "state": chunk["state"]})
            jev.append({"item": item, "answers": chunk["answers"],
                        "reading_seconds": chunk["reading_seconds"],
                        "overloaded": chunk["overloaded"]})
    rng.shuffle(items)
    for n, item in enumerate(items):
        batch = os.path.join(args.out, "batches", f"b{n % args.batches}")
        os.makedirs(batch, exist_ok=True)
        with open(os.path.join(batch, item["item"] + ".md"), "w") as file:
            file.write(f"Questions: {', '.join(item['questions'])}\n\n{item['state']}")
    write(os.path.join(args.out, "labelling.md"), labelling(rubric))
    write_jsonl(os.path.join(args.out, "items.jsonl"),
                [{k: v for k, v in item.items() if k != "state"} for item in items])
    write_jsonl(os.path.join(args.out, "jev.jsonl"), jev)
    spend = chars / 4 * PRICE_PER_TOKEN
    manifest = {"rubric_version": rubric["version"], "seed": args.seed, "jev_usd": spend,
                "walkthroughs": [{"index": i, "path": p, "held_out": p in held}
                                 for i, p in enumerate(walkthroughs)]}
    write(os.path.join(args.out, "manifest.json"), json.dumps(manifest, indent=2))
    failures = sum(len(v["failures"]) for v in scored.values())
    print(f"{len(items)} items in {args.batches} batches; {failures} Jev failures; "
          f"about ${spend:.4f} of Jev. Labellers read {args.out}/labelling.md and one batch.")


def write(path, text):
    with open(path, "w") as file:
        file.write(text)


def write_jsonl(path, rows):
    write(path, "".join(json.dumps(row) + "\n" for row in rows))


def read_jsonl(path):
    with open(path) as file:
        return [json.loads(line) for line in file if line.strip()]
