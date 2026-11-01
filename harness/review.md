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

## Bounded source access

The full `.gewu/review-input.json` remains the coverage authority. Agents and
drivers can inspect it through bounded JSON pages instead of copying complete
patches into each request:

```bash
gewu harness context --max-bytes 16384 inventory
gewu harness context --max-bytes 16384 source --change-id <id> --start 1 --end 40
```

Pass a response's non-null `next` object as `--cursor '<JSON object>'` before
the `inventory` or `source` subcommand to continue. Null means the requested
inventory/span is exhausted. Each response includes the global source identity;
cursors from another source or span are rejected. `start` and `end` retain the
original, inclusive hunk-body coordinates, including removed lines.

`max-bytes` bounds the complete compact JSON response, including escaping and
metadata. A source page may end inside a long line, always at a UTF-8 boundary.
Its offset is in the quote body's UTF-8 text, with lines joined by `\n`, matching
Gewu's existing quote normalization. The response also reports the first body
line and byte offset within that line. Concatenate page text to recover the
requested body. Transport boundaries must not determine narrative boundaries.

These commands do not advance the harness or record a review decision. They
do not budget a driver's surrounding prompts or automatically batch its
investigation. The staged integration and acceptance requirements are in
`docs/repository-scale-review.md`.

## Incremental investigation

After `harness start`, select `gewu harness research start` for a large input.
`gewu harness status` then prints the bounded workflow and overview schema.
`research next` produces one author task; `research accept --checkpoint <file>`
validates exact evidence excerpts and persists progress. Research notes must
account for every source page before `research findings` can page the completed
findings and `research assemble --overview <file>` can assemble the investigation.

Limits are chosen at research start: `--page-bytes`, `--request-bytes`,
`--checkpoint-bytes` and `--max-batches`. They bound JSON task/output sizes and
accepted batches; the provider adapter still must budget complete prompts,
actual calls, retries and tokens. Reusing the domain `ResearchRun` avoids
reparsing and revalidating the journal for each CLI invocation.

Research uses the canonical ledger beside the input's harness state. Once
attached, a missing ledger cannot silently select the legacy path. Source-driven
replanning archives the old ledger and overview, retains the attachment, and
allows research on the new source. A loaded journal is replay-validated before
new work is accepted. This validates evidence attribution and completion of
source delivery, not the semantic quality of the findings or overview.

The offline CLI regression scenarios require Python 3 and are included in
`mise run check` and the local hooks. They use scripted fixture responses and
make no model calls.

Local planning can be validated without advancing the harness:

```bash
gewu harness validate-plan --plan <file> --change-id <id>
```

Repeat `--change-id` to select a larger semantic scope. IDs and hunk-body
coordinates remain those of the full manifest. The local plan must cover that
scope and cannot quote outside it. Validate the merged plan again without
scope flags to require coverage of the complete manifest. The bounded author
orchestration under development is documented in `docs/author-driver.md`.
