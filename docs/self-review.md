# Developing Gewu by reviewing Gewu

Gewu changes under its own review. Two things follow. The code under review
moves on every commit, so a score of a review of `HEAD` mixes harness quality
with corpus drift. And the harness can change the prompts that generate its own
review, so one run proves nothing on its own. The answer to both is the same:
measure against frozen corpora, compare against a baseline binary, and review
the whole repository only at milestones.

This extends `repository-scale-review.md`. Its gates A–F and rubric version 1
remain the record of past evidence; new evaluations use rubric version 2 below.

## Tiers

| Tier | When | Corpus | Model calls |
| --- | --- | --- | --- |
| T0 | Every commit | none; `mise run check` and offline scenarios | none |
| T1: change review | Any change to the harness, prompts, gates, or review TUI | the change itself, as a committed range | small |
| T2: regression | Before merging a T1 change; whenever prompts, harness protocol, or spec schema change | the frozen corpora below | medium |
| T3: whole repository | Spec or harness schema version bump, release tag, or an architecture change that moves crate boundaries | all tracked files at a recorded commit | large |

**T1** follows `.claude/skills/gewu-dogfood`. Generate the review twice on the
same committed range: once with the base binary, once with the head binary.
The head run shows the change acting on itself; only the comparison says
whether it helped. Score both under the same rubric version.

**T2** corpora are frozen ranges of this repository's history, registered in
`scripts/self_review/corpora.json`, so a score that moves was moved by the
harness:

| Corpus | Range or scope | Exercises |
| --- | --- | --- |
| `fix-domain` | `8d2c319^..8d2c319` | a small fix across domain and tests |
| `feat-cli` | `7c65e4e^..7c65e4e` | a small feature through the TUI |
| `docs-only` | `f08c7ed^..f08c7ed` | prose and config with a generated lockfile |
| `domain-files` | `spec.rs`, `validation.rs`, `height.rs` at `20b5737`, from empty | a directory-style review |

The Gate D canary and medium candidate were built from `scripts/` before it was
tracked, so they cannot be rebuilt from history; they remain v1 evidence only.
`integrate` (`20b5737`, 31 files) is a T3 rehearsal.

**T3** is gate F. Run it only when its trigger holds and T2 has passed under the
same binary; between milestones, the T2 corpora carry the signal.

Every evaluation records the corpus, binary hash, rubric version, judge model,
and author usage. Scores compare only within one rubric version.

## Rubric version 3

A review is accepted only when every mechanical gate passes and the judged
dimensions meet their minimums. Unknown is unassessed, never an implicit pass.

### Mechanical gates

`gewu review audit` checks these against a validated, complete review. It exits
non-zero when a gate fails and prints the evidence; `--format json` is for
evaluation records.

**C: coverage.** Three layers, each exact:

1. *Scope → manifest.* Every path `git diff --name-only base head -- <scope>`
   lists has a change in the manifest, including deletions, renames, mode
   changes, and binaries. `--scope` takes the pathspec the reviewer asked for;
   a directory review is a diff from the empty tree.
2. *Manifest → shown.* Every body line that needs review is in some quote:
   all but blank lines and the unchanged context at a hunk's edges
   (`Change::needs_review`). Enforced by validation before the audit runs.
3. *Shown → explained.* Every such line sits in a quote whose chunk prose
   (summary or narrative) names the quote's file or one of its distinctive
   identifiers. Lines with no identifier need only the prose. A summary alone
   counts: requiring a separate narrative block would reward padding.
   Lines shown only as `summary_only` are disclosed as summarized, not failed;
   the judge checks each is a generated artifact.

Layer 3 is necessary, not sufficient: it catches a view nobody wrote about,
never prose that is about the view but wrong.

**R: references.**

- *Resolvable* (gate): every inline code span shaped like a symbol or path
  names something in the diff or in the reviewed tree. A path the code builds
  at run time resolves when the diff contains it literally; an upper-case
  placeholder segment such as `calls/NNNN` resolves by its prefix. Commands,
  expressions, and fenced examples are not checked.
- *Visible* (reported): `line N` or `lines N-M` citations that no view of the
  chunk shows in its gutter, skipping ones that name another file. Not a gate:
  an anchor to an earlier chunk or to unchanged code outside the diff is
  legitimate, so the judge weighs each. `gewu harness status` lists them while
  the author drafts, which catches a citation stranded by trimming a view.
- *Dangling titles* (reported): a quoted or italicized phrase of three or more
  words that names no chunk and appears nowhere in the diff, as when a
  cross-reference outlives a renamed chunk.
- *Echoes* (reported): sentences whose distinctive words mostly repeat the
  overview, the title, or an earlier sentence. Calibrated on 342 judged
  sentences: 44% precision and 20% recall at the chosen threshold, because
  redundancy is usually paraphrase. A lead for the judge, not a measure of
  minimality, and deliberately not shown to authors while drafting.
- *Forward* (reported): prose names a definition the walkthrough shows only in
  a later chunk.
- *Distant* (reported): prose names a reviewed definition last shown at least
  `--anchor-distance` chunks back (default 5), with nothing in the chunk or its
  context pane to re-anchor it.

Forward and distant references are input for the judge, not failures: an
overview of what is coming is a legitimate forward reference, and a definition
the reader just met may not need re-showing.

The audit also records each chunk's prose words against shown lines. Treat it
as a trend across generations of the same corpus, not a threshold.

### Judged dimensions

An independent judge, with no access to the author's prompts or context,
labels every sentence of the overview and every chunk's prose:

| Label | Meaning |
| --- | --- |
| `fact` | new, supported by the quoted code or its consumers |
| `decision` | a question, risk, or tradeoff the reviewer can assess |
| `anchor` | re-points to earlier material the reader needs now |
| `orientation` | says where the chunk sits in the change, or how it follows from the one before |
| `redundant` | says again what an earlier sentence or the code itself says |
| `filler` | conveys nothing: transitions, announcements, praise |
| `narrates-review` | describes the walkthrough instead of the code |
| `unsupported` | not established by the repository |
| `verdict` | concludes for the reviewer: "correct", "safe", "looks good" |

Each score cites chunk ids and sentences.

| Dimension | Measure | Minimum |
| --- | --- | --- |
| Accuracy | no `unsupported` sentence on a consequential claim | hard gate |
| Human judgement | no `verdict` sentence | hard gate |
| Minimality | share of sentences labelled `fact`, `decision`, or `anchor`: would deleting this sentence lose anything? | ≥ 0.9 |
| Reference usefulness | definitions are in view where a sentence depends on them; each distant reference the audit reported is either re-shown or judged unneeded | 2 of 3 |
| Decision surfacing | each chunk gives the reviewer something concrete to judge | 2 of 3 |
| Change framing | for diffs, what changed and why, not only what the code does | 2 of 3 |
| Learning order | prerequisites before consumers; cross-file relationships explicit | 2 of 3 |
| Big picture | before the first chunk the reader knows what the change is for, which parts it touches, and how they connect | 2 of 3 |
| Chunk connection | each chunk says what it adds to that picture and how it relates to its neighbours | 2 of 3 |
| Cold reader | share of six multiple-choice questions, written by the code-aware judge, that a reader with only the rendered pages answers correctly | ≥ 5 of 6 |
| Stability | on regeneration after a small source change, unchanged chunks keep their prose | reported |

Version 3 answers a reviewer's complaint that walkthroughs gave no big picture
and chunks read as isolated notes. Version 2 judged sentences one at a time,
with judges who had read the source before the prose, and its minimality
pressure labelled connective sentences as padding. So version 3 keeps
`orientation` sentences, scores the picture and the connections directly, and
adds the cold reader: a fresh reader who sees only the 120×40 pages and must
answer questions about purpose, connections, and consequential behaviour. That
measures what the walkthrough teaches, not what the judge already knew.

Minimality and reference usefulness pull against each other only when the same
thing is said twice. **Repeat the code, not the explanation.** Re-showing a
definition from ten chunks back is an `anchor`; explaining it again is
`redundant`.

Accuracy outranks minimality: a short false sentence is worse than a long true
one.

### Terminal UI

A walkthrough is only as good as the screen it is read on, and an oversized
chunk is read across pages. `gewu review frames --width W --height H` composes
every page a reader reaches by pressing PageDown through the overview, each
chunk, and the summary, using the TUI's own layout code, with no terminal. It
runs at 80×24 and 120×40.

Mechanical gates, at each size:

| Gate | Fails when |
| --- | --- |
| Fit | a row is wider than the screen, so the terminal clips its end |
| Reach | some row of a stage is on no page PageDown visits |
| Cue | a stage taller than its body has a page without the `view a–b / n` position |
| Orphan | a page ends on a source label or heading whose content is on the next |
| Stub | a page other than the last shows two or fewer rows the reader has not already seen |

A separate judge reads the pages under `scripts/self_review/tui_judge.md` and
scores six dimensions: orientation, legibility, page breaks, prose with code,
density, and context. **120×40 is the reference terminal**: there each
dimension needs at least 2. 80×24 is a supported degraded mode whose judged
scores are recorded beside them but not gated. A 40-row chunk takes two or
three pages there, and the pinned `↳` lead (below) is what keeps it usable.
The mechanical gates hold at both sizes, because a clipped or unreachable row is
a defect wherever it happens.

When a page opens on code whose introducing paragraph has scrolled off, the
chunk header pins that paragraph's first sentence, marked `↳`. The header grows
by that row only while code from the paragraph is on or above the page top, so
the reachable range stays stable. The scorer records pages per
chunk at each size, which shows the cost of oversized chunks over time. The
captures are plain text, so colour and highlighting are unassessed; the
interactive capture in `scripts/check_terminal.py`, which needs `pyte`, covers
key handling, feedback entry, and resume at milestones.

A run passes only when the UI has been assessed: frames at both sizes and a UI
judgement.

### Calibration

Before a rubric version is trusted, the judge must, without the answer key,
reject seeded examples of each defect and accept a concise accurate one. For
version 2 that adds, beyond the version 1 set: padding and review narration, a
view with no prose, a dangling symbol, a verdict sentence, a re-explained
definition, and a needed re-anchor that is missing. The audit gates are
calibrated by their tests in `crates/gewu-review/tests/audit.rs`.

## The loop

The `gewu-self-review` skill drives each round, and
`scripts/run_self_review.py` does every step that needs no model:

1. **Freeze** a corpus into a private two-commit repository under
   `.gewu/self-review/runs/`, with a private copy of the binary under test.
   Rebuilding or editing Gewu cannot disturb a run in progress.
2. **Generate.** `run_self_review.py generate <run> --agent claude` runs
   `gewu review generate` in the frozen copy, the same command a reviewer
   uses, so the product's own agent path is what gets measured. (Earlier
   rounds used an author subagent following the harness directly.) Between batches,
   `advance` stands in for the reader, and it refuses to run outside a marked
   frozen copy.
3. **Measure.** `audit` runs the mechanical gates, and `frames` renders every
   page at both sizes for the UI gates. `sentences` numbers the prose.
   A separate judge subagent labels it under `scripts/self_review/judge.md`.
   `score` refuses a judgement that skips or invents a sentence, then applies
   this rubric.
4. **Record** the run in `.gewu/self-review/ledger.json`. Every audit failure,
   non-kept sentence label, judge finding, and friction item becomes a keyed
   finding.
5. **Fix** each finding with a test, and mark it `fixed` with that test's name.
6. **Verify** by regenerating the same corpus. `record` marks a fixed finding
   `verified` when it is absent and `regressed` when it returns, and prints the
   score delta against the previous run.

Audit findings are deterministic for a given spec, so one clean run verifies
them. Judged findings vary with the author's wording. Watch their category
counts across runs and corpora, not one sentence's absence.

## Running an audit

```bash
mise exec -- cargo build --bin gewu
target/debug/gewu review audit                     # in the reviewed repository
target/debug/gewu review audit --scope crates/gewu-review --format json
```

The audit reads the head tree from Git for symbols and paths, so run it where
the review's `source.head` exists. All retained canaries pass; seeding a
narration-free chunk and an invented symbol into a copy of
`canary-prefetch-v1` fails both gates with the exact lines and span.

## What generation costs

`gewu review generate` prints an estimate before each agent run, a progress
line while it works, and what the batch used after. Its defaults come from two
reviews generated with Claude Code's default model on this machine: a 68-line
TUI feature took three batches and $1.38 (1.2M tokens), and a 248-line verdict
fix four batches and $2.24 (2.1M tokens). A batch averaged about 480k tokens,
$0.52, and 85 seconds; the first, which also investigates and plans, about
half again as much. Once a review has a batch on record, its own average
replaces these, so the estimate tracks the code and model actually in use.
