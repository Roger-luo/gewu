---
name: gewu-dogfood
description: Runs Gewu against its own repository to find defects in the harness, putting a subagent in the generating agent's chair and treating its friction log as the deliverable. Use when changing the harness, the prompts, the verdict gates, or the review TUI, and before trusting that such a change works.
allowed-tools: Bash, Read, Write, Edit, Agent
---

# Gewu Dogfood

## Overview

Gewu's hardest defects are not on the code paths a unit test reaches. They are
in the round trip: a prompt that asks for something validation forbids, a gate
ordered so the reviewer's point waits behind work they never asked about, a
budget that silently rules out what a reviewer requested. None of these fail a
test. They cost an agent a wasted cycle, and they surface only when an agent
that does not know the implementation is made to follow the prompts literally.

So put a subagent in the generating agent's chair, make it obey the harness, and
collect what went wrong. **The friction log is the deliverable.** Whatever
walkthrough it writes is a by-product.

## When to use

Use before trusting any change to `crates/gewu-review/src/harness/`,
`crates/gewu-cli/src/harness/`, `harness/prompts/*.md`, the verdict gates in
`progress.rs`, or the feedback flow in `crates/gewu-cli/src/tui/`. Use also when
a review holds real human feedback nobody has acted on yet, to check the
handover actually carries it.

Do not use for changes confined to rendering, highlighting, or Git parsing.
Those have honest unit tests, and a dogfood run tells you nothing new.

## Process

### 1. Fix the ground before you start

A working-tree review re-diffs on every `harness next`. Editing any source file
while one is open changes hunk ids, and the harness correctly concludes the plan
is stale. Every wasted run in this tool's first dogfood came from exactly that.

Prefer a committed range, which cannot move:

```bash
gewu harness start --base <commit> --head <commit>
```

If you must dogfood a working-tree review — replaying a session a human already
left feedback in, say — snapshot the repository and work entirely in the copy:

```bash
rsync -a --exclude target /path/to/gewu/ /tmp/gewu-dogfood/
```

The copy keeps `.git` and `.gewu`, so the manifest still describes its own
working tree. Build the binary in the real repository and call it by absolute
path from the copy, so your implementation changes never touch the tree under
review.

### 2. Get the feedback into the shape the harness routes

```bash
gewu review handover                # what the reviewer left, with the code it names
gewu review handover --format json  # the same, for scripting the ids out
```

Feedback recorded before a target existed reads as `code`. Where it is plainly
about the walkthrough, say so, or the harness sends the agent to edit source
that was never at fault:

```bash
gewu review retarget --id <feedback-id> --target explanation
```

Classifying someone else's feedback is a judgement about what they meant. Do it
only where the wording leaves no doubt, and say in your report that you did.

### 3. Put a subagent in the generating agent's chair

Give it the working directory, the absolute path to the binary, and the
instruction to start from `gewu harness status` and follow each printed prompt
exactly. Then, in order of how much they matter:

- **Forbid writing anything outside `.gewu/harness/`.** No formatting, no
  fixing what it notices in passing, no command that writes to the repository.
  One stray edit invalidates the manifest and the run is over.
- **Demand a friction log with exact commands and exact output**: every
  confusing error, everything it had to guess, every instruction it could not
  follow, every validation message that did not say how to fix the problem.
  Without this the agent reports success and the defects go unrecorded.
- **Tell it to stop where the harness stops.** When a prompt says it is waiting
  for the reviewer, that is the end of its work.
- Give it what it cannot infer: which crate is which, that a reviewer will check
  its claims against the code, and what the last reviewer rejected.

### 4. Triage the log against the source

Take nothing on faith. A subagent reports what it experienced, and some of it
will be wrong — in the first run, one reported gap was already answered by a
line in the prompt it had read. For each item find the code or prompt line that
explains it, then sort:

- **Ordering and routing defects** — the harness asked for the wrong thing, or
  in the wrong order. These are the valuable ones; they live in `progress.rs`
  or the runner.
- **Impossible instructions** — a prompt asks for something validation rejects
  or a budget forbids. Fix whichever is wrong. Do not paper over it in prose.
- **Misleading messages** — a prompt claiming a cause it cannot know, or stating
  a bound that differs from the check. Cheap, and each one taxes every run.
- **Noise** — output that buries the answer. Fix it if it is in an inner loop.

### 5. Land each fix with a test that names its origin

A dogfood finding is a regression waiting to happen, because nothing else
exercises it. Put the test where the defect lives — verdict ordering in
`progress.rs`, stage transitions in `runner/tests.rs`, key handling in
`tui/mod.rs` — and say in the doc comment that dogfooding found it, so the next
reader does not delete it as redundant.

### 6. Verify the whole loop by hand, then `mise run check`

Rebuild, restore a pristine copy of the session, and drive the round trip with
the fixed binary rather than trusting the unit tests:

```
handover -> retarget -> harness next -> write the checkpoint
         -> harness status (measure) -> harness next (accept)
         -> harness next (still gated) -> review resolve -> harness next (moves on)
```

Once the walkthrough is complete, run `gewu review audit` in the reviewed
copy. A failing coverage or reference gate is a finding like any other; see
`docs/self-review.md` for the tiers and rubric.

The gate must still hold before the feedback is resolved. That step is the one
worth checking by hand: it separates a loop that closes from one that merely
runs.

## Guidelines

- The friction log is the point. A run that produced a beautiful walkthrough and
  no findings has told you nothing.
- Never let a fix to the harness be validated only by the harness. Every finding
  needs a test that fails without the fix.
- Two subagents in parallel work well when one drives the loop and the other
  makes a systemic fix the feedback asked for; they touch different files.
- Back up `.gewu` before anything destructive. `gewu harness reset` deletes
  `.gewu/reviews/` outright, and a session is the only record of what a human
  concluded.
- Report honestly which findings you fixed, which you judged mistaken, and which
  you left. A dogfood report claiming everything is now fine is the one failure
  mode that makes the next run pointless.
