"""Calibrates Gewu's cognitive-load rubric against labelled chunks.

    mise exec -- python3 scripts/load_calibration build --gewu target/debug/gewu \\
        --walkthrough <checkout or .gewu> ... [--held-out <same>] --out <dir>
    python3 scripts/load_calibration eval --gewu target/debug/gewu --out <dir>

`build` scores each walkthrough with `gewu review load --jev` (TYPESAFE_API_KEY
from `mise.local.toml`; chunk text goes to TypeSafe), samples items, and
writes blind batches plus `labelling.md`, generated from the rubric's
guidance and questions. Labels go in `batches/*/labels.jsonl`, or for other
sources such as reviewer feedback in `labels/*.jsonl`, in the same format.
`eval` checks the rubric's `[bars]` on the held-out walkthroughs and writes
`results.md` and `proposal.md`. Nothing here edits the rubric: a person
applies the proposal, following `[guidance].recalibrate`.
"""

import argparse
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from build import build  # noqa: E402
from evaluate import evaluate  # noqa: E402


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    commands = parser.add_subparsers(dest="command", required=True)
    for name in ("build", "eval"):
        command = commands.add_parser(name)
        command.add_argument("--gewu", required=True, help="the gewu binary")
        command.add_argument("--rubric", help="a rubric file; default: the built-in one")
        command.add_argument("--out", required=True)
    b = commands.choices["build"]
    b.add_argument("--walkthrough", action="append", required=True)
    b.add_argument("--held-out", action="append", default=[],
                   help="a walkthrough to judge on, never fit on; default: a seeded half")
    b.add_argument("--sample", type=int, default=80)
    b.add_argument("--minimum", type=int, default=6, help="items per walkthrough, at least")
    b.add_argument("--batches", type=int, default=4)
    b.add_argument("--seed", type=int, default=20261111)
    args = parser.parse_args()
    build(args) if args.command == "build" else evaluate(args)


if __name__ == "__main__":
    main()
