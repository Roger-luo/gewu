---
name: gewu-review
description: Generates a Gewu ReviewSpec by following the progressive harness. Use when asked to prepare an interactive code-review guide for a commit range or working-tree changes.
---

# Gewu Review

## Process

1. Start the harness for the requested source:
   - Commit range: `gewu harness start --base <base> --head <head>`
   - Current changes: `gewu harness start --base HEAD --working-tree`
2. Perform only the task printed by the command.
3. Write the requested checkpoint exactly where the prompt specifies.
4. Run the exact continuation command printed by Gewu.
5. Repeat until Gewu prints `# COMPLETE`.

The harness expands one work item at a time and then waits for the human. When
it says it is waiting, stop and tell the user to review; do not generate ahead.
Their feedback may change the code and invalidate the rest of the plan, which is
exactly what expanding lazily avoids paying for.

When the reviewer has asked for something, `gewu harness next` prints the
request rather than more chunks. Do what it says — edit the code, or rewrite the
named chunks into `.gewu/harness/revision.json` — and then close it:

```bash
gewu review resolve --id <feedback-id>
```

Resolve only what you actually addressed; the gate is the reviewer's, and
clearing it without doing the work loses their point silently. If you think a
request is mistaken, say so to the user and stop. `gewu review handover` prints
everything they left, including non-blocking comments.

Add `--max-chunks <n>` or `--max-chunk-lines <n>` only when the reviewer asks
for a different amount of work per batch. Use `--force` only when they say an
open change request is confined to what is already expanded.

If interrupted, run `gewu harness status` and continue from the prompt it
prints. Do not duplicate the review methodology in this skill or advance a
stage without satisfying its checkpoint.
