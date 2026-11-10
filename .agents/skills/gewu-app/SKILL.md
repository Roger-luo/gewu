---
name: gewu-app
description: "Iterates the Gewu desktop app and the generation loop it drives. Use when changing the review UI, the context pane, workbench tasks, agent presets, harness stages the app runs, or the UI rubric, and when a generating review fails with an agent error such as resource_exhausted."
allowed-tools: Bash, Read, Write, Edit
---

# Gewu App

## Overview

The desktop app is a React UI in a Tauri shell. It renders a snapshot and
sends actions; review state stays in Rust. Generation is the same harness the
CLI drives: the workbench starts an ACP agent, the agent writes checkpoints,
and Gewu stops at the reviewer gate. Change the crate that owns the rule, then
the screen that shows it, then prove the screen with a test and a mock review.

`gewu-dogfood` is for harness friction and `gewu-self-review` is for narrative
quality. This skill is for making the app and its generation loop change
safely.

## Where a change goes

| Change | Home |
|---|---|
| What a chunk, quote, or context means | `crates/gewu-review`, `crates/gewu-app` |
| A harness stage, prompt, or checkpoint | `crates/gewu-harness`, `harness/prompts/` |
| Which agent runs, and how a batch ends | `crates/gewu-agent`, `crates/gewu-workbench` |
| What the reviewer sees | `apps/desktop` |
| A UI defect the QA loop should catch again | a new `apps/ui-harness/rubric/vN.json` |

`docs/desktop.md` lists the crates. `docs/design.md` is the behavior a reviewer
is owed; update it when the screen changes. `docs/workbench.md` covers task
states. Domain files over about 200 lines become a module with `mod.rs`.

## Review UI

`apps/desktop` is Vite, React, and TypeScript. `?mock=showcase` serves
`apps/desktop/src/mock/showcase.ts` through `MockBackend`, with no Rust
process. The first chunk has a concept pane; "Rename path helper" has a
refactor pane.

The context pane (`components/ContextPane.tsx`) is the side reference for a
refactor: reason, then the old definition, then the new one. Both definitions
are visible on arrival. A one-line brace block is laid out one field per line
by `layoutCode` in `contextCode.ts`; code that already has line breaks is
left as written, and `.context-code` uses `white-space: pre` so a narrow pane
scrolls instead of breaking tokens. Names the reason cites in backticks that
occur in the old definition and not the new one are wrapped as
`.dropped-field`. The pane width is `--context-width` (default 560), dragged
from the separator or nudged with the arrow keys, and remembered in
`localStorage` key `gewu.context-width` between 360 and 840. Below 1100px the
pane stacks into a disclosure and the handle hides.

A narrative sentence that names a view's lines is shown beside that view, not
as one paragraph above a stack of diffs. `assignNotes` in `quoteNotes.ts`
splits a sentence that lists several ranges into one note per quote.
`?mock=paired` is that layout. Prose in the review body is 16px at a 1.6 line
height; the note column sits beside the code once the step is at least 48rem
wide and stacks above it when narrower.

Highlighting is Shiki via `highlight.ts`. A language the review shows needs a
loader there; an unknown language renders as one color.

The summary screen shows a publication preview of the open notes. A note on
the code of a pull request becomes a review comment (on its lines, or one
summary comment when it has none). A note on the walkthrough becomes an issue
in `Roger-luo/gewu`. A note on a directory, a working tree, or commits
becomes an issue on the reviewed repository, and more than one issue adds a
tracking issue. For a workbench task the draft comes from
`Workbench::publication`; `publication.ts` builds the same JSON for the mock
and a single-review window. Publish sends through `crates/gewu-github` only
after a confirmation dialog, records posts in the task's `published.json`, and
never resends a note. Sign-in is the GitHub CLI's (`gh auth login
--with-token`). Never test publishing against GitHub: use
`gewu_github::Recorder` in Rust (the `gewu ui serve` sandbox uses it too) and
the mock in the desktop tests.

`apps/desktop/src/workbench/format.ts` maps harness stage names to the label
the task list shows. A new stage with no entry shows nothing.

## Verifying a UI change

Run the desktop tests, then open the mock and use the control a reviewer
would: click, resize, and the narrow width.

```bash
pnpm --dir apps/desktop exec vitest run src/ContextPane.test.tsx
pnpm --dir apps/desktop exec vite --host 127.0.0.1 --port 5174
```

Open `http://127.0.0.1:5174/?mock=showcase`. Leave a Vite server already bound
to 5173 alone; it belongs to another session. `mise run ui-check` is the full
UI gate: typecheck, lint, test, and build for `apps/desktop` and
`apps/ui-harness`.

A published rubric file is immutable. A defect the QA loop should keep
catching goes in the next `apps/ui-harness/rubric/vN.json`, with the version
named in `docs/ui-rubric.md` and the QA prompts under `apps/ui-harness/prompts/`.
The latest published UI rubric is v9.

## How generation reaches the reviewer

One work item becomes one batch. The harness stages are investigate, plan,
expand, repair, proofread, polish, await, revise, and complete. Checkpoints
live in `.gewu/harness/`. After a batch validates it is appended to
`review-spec.json`. With proofreading on, the author stops there and a fresh
agent scores the batch; otherwise the batch waits for the reviewer.

`gewu review generate` and the workbench proofread with two rewrites
(`--proofread-rounds`, `PROOFREAD_ROUNDS` in `crates/gewu-workbench/src/batch.rs`).
`gewu harness start` proofreads only when asked, so an agent driving the
harness by hand is not grading itself. Each driver starts a new agent session
while the status heading is `# Gewu harness: proofread` or `# Gewu harness:
polish`, and stops when the heading is the reviewer gate or the rounds are
spent. `harness/prompts/agent.md` tells the author to stop at the proofread
heading after writing chunks in that session.

Proofreading scores every chunk of the batch from 0 to 3 on `grounded`,
`change_framing`, `rationale`, `representation_contrast`, `minimality`, and
`connection`. The names and the minimum of 2 live in
`crates/gewu-review/src/harness/proofread.rs` and must match
`harness/prompts/proofread.md`. A score below 2 needs a problem a rewrite can
act on. Failing chunks are rewritten through `.gewu/harness/revision.json`
(same ids, same quoted lines) and read again. When the rounds are spent the
batch still reaches the reviewer, and the gate prompt names what is still
below the bar.

A chunk whose point is a new form of an existing structure shows the old
definition itself, names the fields that were dropped or added, and says what
that buys. A type alias that keeps the old name is not the old definition.
`gewu harness context base --path <path>` prints the file at the review's base
revision. The rules are in `harness/prompts/expand.md` and `rubric.md`;
`crates/gewu-harness/src/prompts.rs` asserts the phrases, so a prompt edit
updates that test.

The self-review rubric is separate from the live proofread. It is version 4
in `scripts/self_review/score.py`, including `representation_contrast`. Do not
bump it without the judge prompt, `corpora.json`, and `docs/self-review.md`.

## Agents and a failed batch

Presets are `PRESETS` in `crates/gewu-agent/src/preset.rs`. Cursor Agent is
`agent acp`. Gemini CLI is `gemini --acp`. The new-review screen remembers
`localStorage` key `gewu.agent` and otherwise offers raw diff, then the first
available ACP preset. Availability is "the program is on PATH", and the app
also searches `~/.local/bin`.

The choice for a running review is `agent.preset_id` in
`~/.gewu/tasks/<id>/task.json` (`GEWU_HOME` overrides the home). Read that
file. The log line is not the preset: `crates/gewu-workbench/src/batch.rs`
appends whatever the agent said last, as `It said: …`.
`Error: RetriableError: [resource_exhausted]` is the model provider reporting
gRPC `RESOURCE_EXHAUSTED` (quota or rate limit). The harness stays on the
stage it was in, so Continue starts a fresh agent on that same stage. It is
not a checkpoint failure and not evidence that Gemini was selected.

`fixtures/agents/fake_acp_agent.py` is the ACP stand-in for workbench tests.
A new stage the workbench stops on has to be handled there, or
`an_acp_agent_writes_one_batch_per_reviewer_gate` fails. The proofread prompt
includes a schema example, so parse chunk ids from the listing above
`Check every claim`, not from every `"id"` in the prompt. Python generation
tests live in `scripts/tests/test_generate.py` and use `--agent-program` so
no model is called.

## Before handing off

```bash
mise run check
mise run ui-check
```

`mise run check` is fmt, check, clippy, nextest, doctests, the `gewu` binary,
and `python3 -m unittest discover -s scripts/tests`. Run the Python tests from
`scripts/tests` when an import fails from the repo root. Do not commit unless
asked.
