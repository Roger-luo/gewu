"""Develop Gewu by reviewing Gewu: freeze, generate, audit, judge, record, fix.

See docs/self-review.md for the tiers and rubric, and the gewu-self-review
skill for the loop that drives these steps. No step here calls a model.
"""
import argparse
import json
import subprocess
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parent))
from self_review import corpus as corpora
from self_review import tui
from self_review.ledger import Ledger
from self_review.score import cold_pack, score
from self_review.sentences import units

REPOSITORY = Path(__file__).resolve().parents[1]
LEDGER = REPOSITORY / ".gewu/self-review/ledger.json"


def show(value):
    print(json.dumps(value, indent=2, ensure_ascii=False))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    commands.add_parser("corpora", help="list the registered corpora")
    freeze = commands.add_parser("freeze", help="materialize a corpus with a private binary")
    freeze.add_argument("corpus", nargs="?", help="registered corpus name")
    freeze.add_argument("--base", help="ad hoc base revision, `empty` for none")
    freeze.add_argument("--head", help="ad hoc head revision, or WORKTREE for uncommitted edits")
    freeze.add_argument("--scope", action="append", default=[])
    freeze.add_argument("--binary", type=Path, default=REPOSITORY / "target/debug/gewu")
    freeze.add_argument("--destination", type=Path, required=True)
    for name, text in [("advance", "mark the accepted batch read, inside a frozen copy only"),
                       ("audit", "run the mechanical gates"),
                       ("frames", "render every page headlessly at 80x24 and 120x40"),
                       ("sentences", "number the prose for the judge"),
                       ("cold-pack", "give the cold reader the judge's questions without answers"),
                       ("score", "check the judge and apply rubric version 2"),
                       ("record", "add the scored run to the ledger")]:
        command = commands.add_parser(name, help=text)
        command.add_argument("run", type=Path)
    generate = commands.add_parser("generate", help="let `gewu review generate` write the walkthrough")
    generate.add_argument("run", type=Path)
    generate.add_argument("--agent", choices=["claude", "codex"], required=True)
    generate.add_argument("--max-calls", type=int, default=30)
    commands.add_parser("findings", help="list open and regressed findings")
    resolve = commands.add_parser("resolve", help="mark a finding fixed or rejected")
    resolve.add_argument("key")
    resolve.add_argument("--status", choices=["fixed", "rejected"], required=True)
    resolve.add_argument("--note", required=True)
    args = parser.parse_args()

    if args.command == "corpora":
        show(corpora.registry())
    elif args.command == "freeze":
        if args.corpus:
            chosen = corpora.named(args.corpus)
        elif args.base and args.head:
            chosen = {"name": "adhoc", "tier": "T1", "base": args.base, "head": args.head, "scope": args.scope}
        else:
            parser.error("name a corpus, or give --base and --head")
        show(corpora.freeze(REPOSITORY, chosen, args.destination, args.binary))
    elif args.command == "generate":
        run, log, early = corpora.Run(args.run), [], 0
        for _ in range(args.max_calls):
            result = subprocess.run([str(run.binary), "review", "generate", "--agent", args.agent],
                                    cwd=run.root, capture_output=True, text=True)
            log.append({"returncode": result.returncode, "stdout": result.stdout[-4000:],
                        "stderr": result.stderr[-4000:]})
            run.write("generate-log.json", log)
            print(result.stdout.strip().splitlines()[-1] if result.stdout.strip() else result.stderr.strip(),
                  flush=True)
            if result.returncode:
                raise SystemExit("generate failed; see generate-log.json")
            if "walkthrough is complete" in result.stdout:
                break
            if "batch is ready for you" in result.stdout:
                run.advance()
                early = 0
            elif "stopped before the reviewer gate" in result.stdout:
                # An agent that stops twice running is stuck, not unlucky.
                early += 1
                if early >= 2:
                    raise SystemExit("the agent stopped early twice in a row; see generate-log.json")
        else:
            raise SystemExit("generate did not finish within --max-calls")
    elif args.command == "findings":
        show(Ledger(LEDGER).unresolved())
    elif args.command == "resolve":
        Ledger(LEDGER).resolve(args.key, args.status, args.note)
    else:
        run = corpora.Run(args.run)
        if args.command == "advance":
            run.advance()
        elif args.command == "audit":
            report = run.audit()
            show({"coverage_missing": report["coverage"]["missing_paths"],
                  "unexplained": sum(len(c["unexplained"]) for c in report["coverage"]["changes"]),
                  **{name: len(items) for name, items in report["references"].items()}})
        elif args.command == "frames":
            show(tui.capture(run.root, run.binary))
        elif args.command == "sentences":
            numbered = units(run.spec())
            run.write("sentences.json", numbered)
            print(f"{len(numbered)} sentences written to {run.evidence / 'sentences.json'}")
        elif args.command == "cold-pack":
            run.write("cold/questions.json", cold_pack(run.read("judge.json")))
            print(f"questions written to {run.evidence / 'cold/questions.json'}")
        elif args.command == "score":
            friction = run.read("friction.json") if (run.evidence / "friction.json").exists() else []
            optional = lambda name: run.read(name) if (run.evidence / name).exists() else None
            frames = {f"{w}x{h}": optional(f"frames-{w}x{h}.json") for w, h in tui.SIZES}
            result = score(run.meta["corpus"]["name"], run.read("sentences.json"), run.read("judge.json"),
                           run.read("audit.json"), friction,
                           frames if all(frames.values()) else None, optional("tui-judge.json"),
                           optional("cold/answers.json"))
            run.write("result.json", result)
            show({key: result[key] for key in ("passes", "gates", "minimality", "dimensions", "tui", "cold_reader",
                                               "reported")}
                 | {"findings": len(result["findings"])})
        elif args.command == "record":
            show(Ledger(LEDGER).record(run.meta, run.read("result.json")))


if __name__ == "__main__":
    main()
