---
name: gewu-self-review
description: Develops Gewu by reviewing Gewu. Freezes a corpus from this repository's history, has a subagent generate the walkthrough with the binary under test, audits and independently judges it under self-review rubric version 4, records findings in a ledger, and drives each finding to a verified fix. Use when improving review quality, after changing the harness or prompts, or when asked to find and fix issues by using Gewu on itself.
allowed-tools: Bash, Read, Write, Edit, Agent
---

# Gewu Self Review

## Overview

Gewu is its own test case. Each round freezes a slice of this repository's
history, generates a walkthrough of it with the binary under test, and measures
the result: mechanically with `gewu review audit`, and by an independent judge
that labels every sentence. Every defect becomes a ledger finding, and a
finding closes only when a fix lands with a test **and** a later run of the same
corpus no longer reproduces it.

Freezing is what makes this work on a moving target. The corpus is a private
two-commit repository with a private copy of the binary, so you can edit and
rebuild Gewu while a run is in flight without invalidating it.

The tiers, rubric, and corpus rationale are in `docs/self-review.md`. The
harness-friction side of the loop is `gewu-dogfood`; this skill reuses its
author rules.

## Commands

All steps go through one driver; none of them calls a model:

```bash
S="python3 scripts/run_self_review.py"
$S corpora                                   # registered corpora and tiers
$S freeze <corpus> --destination <run>       # or --base X --head Y [--scope P]
$S advance <run>                             # synthetic reader, frozen copies only
$S audit <run>                               # mechanical gates -> audit.json
$S frames <run>                              # every page at 80x24 and 120x40 -> frames-*.json
$S generate <run> --agent claude             # `gewu review generate` writes it, batch by batch
$S sentences <run>                           # numbered prose -> sentences.json
$S cold-pack <run>                           # judge's questions, without answers, for the cold reader
$S score <run>                               # checks judge.json, rubric v4 -> result.json
$S record <run>                              # ledger; prints delta against last run
$S findings                                  # open and regressed findings
$S resolve <key> --status fixed --note "<test or commit>"
```

`<run>` lives under `.gewu/self-review/runs/`, named `<corpus>-<label>`.
Evidence is in `<run>/.gewu/self-review/`; the ledger is
`.gewu/self-review/ledger.json`.

## Process

### 1. Choose the corpus

- **T1**, after a harness or prompt change: freeze the change itself, either
  `--base <commit> --head <commit>`, or `--base HEAD --head WORKTREE` for
  uncommitted edits.
- **T2**, before merging any change to prompts, harness protocol, or schema: every
  T2 corpus in `corpora.json`.
- **T3** (`integrate`, `whole`): only at a milestone, and **only after asking the
  user**. It is the expensive run.

For a T1 comparison, build the base binary in a separate worktree so the head
build is untouched:

```bash
git worktree add /tmp/gewu-base <base-rev>
cargo build --bin gewu --manifest-path /tmp/gewu-base/Cargo.toml --target-dir /tmp/gewu-base-target
```

and freeze the same corpus twice, `--binary /tmp/gewu-base-target/debug/gewu`
and the default `target/debug/gewu`, as `<corpus>-base` and `<corpus>-head`.

### 2. Generate

Prefer `$S generate <run> --agent claude`: it runs the product's own
`gewu review generate` and marks each batch read until the walkthrough is
complete, so the agent path users take is the one measured. The subagent
author below is still useful when you need its friction log.

#### Generate with an author subagent

Spawn a fresh subagent per run. Give it:

- the run directory as its working directory, and
  `<run>/.gewu/self-review/gewu` as `gewu`, called by absolute path;
- the instruction to run `gewu harness status` and follow each printed prompt
  exactly, as the `gewu-review` skill describes;
- **no writes outside `<run>/.gewu/`**; never `git commit`, format, or fix code;
- a friction log in `<run>/.gewu/self-review/friction.json`: a JSON list of
  `{"command", "output", "problem", "category"}`, with exact commands and output
  for every confusing error, guess, or instruction it could not follow;
- that when the harness says it is waiting for the reviewer, it stops and reports.

Each time it stops at the reviewer gate, run `$S advance <run>` and resume the
same subagent with SendMessage. Repeat until the harness prints `# COMPLETE`.

### 3. Measure

```bash
$S audit <run> && $S frames <run> && $S sentences <run>
```

Then spawn two **different** fresh subagents, neither told anything about the
author's session. Both read the run directory:

- the prose judge, with `scripts/self_review/judge.md`, writes `judge.json`;
- the UI judge, with `scripts/self_review/tui_judge.md`, writes `tui-judge.json`
  from the captured pages.

They are independent, so run them in parallel. When the prose judge is done,
run `$S cold-pack <run>` and spawn a third subagent with
`scripts/self_review/cold_reader.md`. It may read only the 120×40 pages and the
packed questions; it writes `cold/answers.json`. Then:

```bash
$S score <run> && $S record <run>
```

`score` rejects a judgement that skips or invents a sentence; send the judge
the error and have it finish rather than editing its labels yourself.

### 4. Triage every finding

Run `$S findings`. For each, read the evidence against the source, then route it:

| Finding | Usual root cause | Fix in |
| --- | --- | --- |
| `unexplained`, `scope-missing` | expansion left a view without prose; diff parser dropped a path | `harness/prompts/expand.md`, expansion validation, `git/diff.rs` |
| `unresolved-reference` | the author invented a name, or the audit misparsed a span | prompts; or `crates/gewu-review/src/audit/` if the audit is wrong |
| `unsupported` | weak grounding in research or expansion | `research.md`, `investigate.md`, `expand.md`, prerequisite context |
| `redundant`, `filler`, `narrates-review` | prompt invites restatement | `expand.md`, `rubric.md` |
| `verdict` | the author concluded for the reviewer | prompts; this breaks the product rule |
| judge `findings` | omission, boundary, order | planning or expansion prompts, work-item validation |
| `friction` | harness asked for the wrong thing | follow `gewu-dogfood` step 4 |
| `clipped`, `unreached`, `uncued_pages`, `orphaned_pages` | layout or paging in the TUI | `crates/gewu-cli/src/tui/`, with a test in `tui/frame.rs` |
| `tui-judge` findings | page breaks, orientation, density | the TUI renderer; or the expand prompt when the walkthrough's shape is the cause |
| a real bug in Gewu's code, found while reviewing it | the code under review | the code, as a normal fix |

A finding you judge mistaken is `--status rejected` with the reason; if the
audit or judge brief caused it, fix that tool too. Never reject to clear the list.

### 5. Fix, then verify by regenerating

- Land each fix with a test where the defect lives, whose doc comment says
  self-review found it. Prompt fixes still need a test that pins the prompt text
  or the validation that enforces it.
- `$S resolve <key> --status fixed --note "<test name or commit>"`.
- Rebuild, freeze the **same corpus** again as a new run, and repeat steps 2–3.
  `record` marks fixed findings `verified` when absent and `regressed` when
  back. Read the delta it prints, not just the status.
- If prompts, harness, or schema changed, run every T2 corpus before calling the
  round done.

### 6. Stop and report

A round ends when the corpus passes every gate in `result.json`, `findings` is
empty for it, and `mise run check` passes. Stop early, and report, after three
regenerations of one corpus that keep failing the same gate. That means the
fix is wrong, not unlucky.

## Guidelines

- Author output varies between runs. A deterministic finding (the audit gates)
  is verified by one clean run. A judged finding needs the category count to
  stay down across corpora; one missing sentence is weak evidence.
- Keep author and judge separate: different subagents, and the judge never
  sees `harness/` or the friction log.
- Never let synthetic reads near a human session: `advance` refuses outside a
  marked frozen copy, and runs belong under `.gewu/self-review/runs/`.
- Compare scores only within one rubric version. Changing `judge.md`,
  `score.py`, or the audit is a rubric change; bump `rubric_version` in
  `corpora.json` and `score.py` together, and re-baseline.
- Report honestly what was fixed, verified, rejected, and left open, with run ids.
