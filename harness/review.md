# Gewu review-generation guide

Use the progressive agent harness instead of embedding review methodology in a
human-readable guide:

```bash
gewu harness start --base <base> --head <head>
```

For current working-tree changes:

```bash
gewu harness start --base HEAD --working-tree
```

The harness investigates the whole change set once, plans an ordered work list,
then expands **one item at a time**. After each expansion it stops and hands the
review to the human:

```bash
gewu review open
```

Run `gewu harness next` again when they have finished that batch. Gewu decides
what happens by reading their session and re-diffing the tree: it expands the
next item, waits, re-plans if their edits reached unexpanded work, hands their
requests back, or completes.

## When the reviewer asks for something

A request is aimed at one of two things, and Gewu routes it accordingly:

- **the code** — Gewu prints the request against the lines it names. Edit the
  source, then `gewu review resolve --id <feedback-id>`.
- **the walkthrough** — the code is fine; the explanation of it is not. Gewu
  prints the chunks to rewrite and what was said about each. Write
  `.gewu/harness/revision.json`, then resolve.

Neither is a suggestion: an unresolved request holds the harness shut, because
the reviewer decides what the review covers. To read everything they left,
including comments that block nothing:

```bash
gewu review handover            # or --format json
```

If a request is filed against the wrong one of the two, say so rather than
working around it:

```bash
gewu review retarget --id <feedback-id> --target code|explanation
```

Useful flags:

- `--max-chunks <n>` — chunks one work item may expand into.
- `--max-chunk-lines <n>` — rendered rows a chunk may occupy. Raise it only to
  ask deliberately for chunks larger than one terminal viewport.
- `--force` — expand the next item even while change requests are unresolved.

Follow each Markdown prompt and run its exact continuation command. Resume an
interrupted run with `gewu harness status`.
