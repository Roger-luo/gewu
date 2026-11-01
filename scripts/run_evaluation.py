"""Explicit bounded live evaluation entry point; never a normal human review."""
import argparse
from pathlib import Path

from gewu_driver.author import BudgetedAuthor, Limits
from gewu_driver.evaluation import create, advance_synthetic_batch
from gewu_driver.expand import Expansion
from gewu_driver.prepare import Preparation
from gewu_driver.process import ClaudeProcess
from gewu_driver.codex_process import CodexProcess
from gewu_driver.session import Session
from gewu_driver.storage import save


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, required=True)
    parser.add_argument("--destination", type=Path, required=True)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--model", required=True)
    parser.add_argument("--provider", choices=["claude", "codex"], default="claude")
    parser.add_argument("--file", action="append", default=[])
    parser.add_argument("--resume", action="store_true")
    args = parser.parse_args()
    if not args.resume:
        create(args.source, args.file, args.destination, args.binary)
    session = Session(args.destination, args.binary)
    marker = session.read("evaluation-only.json")
    if marker["root"] != str(session.repo) or marker["source"] != str(args.source.resolve()):
        raise ValueError("evaluation identity mismatch")
    limits = Limits(max_calls=24, total_reserved_bytes=1_048_576,
                    stage_calls={"investigate": 8, "synthesize": 8, "plan": 8, "expand": 16, "repair": 4})
    provider = (ClaudeProcess(session.root / "provider", args.model, max_budget_usd=0.25)
                if args.provider == "claude" else CodexProcess(session.root / "provider", args.model))
    author = BudgetedAuthor(session.root / "author", limits,
                            {"session": session.identity, "provider": provider.identity,
                             "purpose": "isolated generation evaluation"}, provider)
    save(session.root / "evaluation-budget.json", {
        "config": author.config, "provider_call_usd_cap": 0.25 if args.provider == "claude" else None,
        "maximum_provider_calls": 24, "aggregate_provider_cap_usd": 6 if args.provider == "claude" else None,
        "warning": "Codex uses account quota with no dollar cap; byte reservations are not exact token billing."})
    preparation = Preparation(session, author)
    if session.stage() == "investigate":
        print("Investigating frozen evaluation source", flush=True)
        preparation.investigate()
    if session.stage() == "plan":
        print("Planning bounded evaluation walkthrough", flush=True)
        preparation.plan()
    while session.stage() != "complete":
        print("Evaluation stage: " + session.stage(), flush=True)
        if session.stage() in ("expand", "repair"):
            Expansion(session, author).next_batch()
        elif session.stage() == "await":
            advance_synthetic_batch(session)
        else:
            raise ValueError("evaluation needs intervention; no automatic feedback resolution")
    print(session.native("review", "validate"), flush=True)
    print("Generation finished; independent quality and terminal UI review remain required.", flush=True)


if __name__ == "__main__":
    main()
