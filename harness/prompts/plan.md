# Gewu harness: plan

Turn `.gewu/harness/investigation.json` into an ordered work list — the shape of
the story you will tell a reviewer who has not seen this code.

This is a plan, not a review: name the parts of the walkthrough and their order,
and stop. Each part is expanded into chunks later, one at a time, so a
reviewer's early feedback never invalidates work already paid for.

Order the parts by comprehension dependency:

1. New concepts, types, or interfaces.
2. Code that consumes those definitions.
3. Integration and wiring.
4. Tests and supporting changes.

Each item is one sitting: what a reviewer would read in one go and react to as a
whole. An item names the spans of the diff it walks through. A large hunk will
usually need several items — split it where the story changes subject, not where
a line count runs out.

Write `.gewu/harness/worklist.json`:

```json
{
  "schema_version": 1,
  "source": { "base": "…", "head": "…", "diff_sha256": "…" },
  "items": [
    {
      "id": "stable-unique-id",
      "title": "short purpose",
      "purpose": "one sentence on why this is reviewed here",
      "covers": [{ "change_id": "change-id", "start": 1, "end": 40 }],
      "expanded": false
    }
  ]
}
```

`source` is the `source` object of `.gewu/review-input.json`, copied field
for field. Gewu compares it to reject a checkpoint written against a manifest
that has since moved.

`start` and `end` are inclusive line numbers within a change's hunk body — the
lines after its `@@` header, numbered from 1. Each change carries `body_lines`;
use it. Do not use `new_lines`, which counts the new side only and is smaller
whenever the hunk removes anything.

**Every line of every change must be covered by at least one item.** Items may
overlap where the story needs the same lines twice; nothing may be left out.

An item becomes at most {{max_chunks}} chunks, and a chunk holds about
{{max_chunk_lines}} rendered rows including its title, summary, prose, and quote
headers. Count before you commit: an item whose spans cannot fit that cannot be
expanded, and you will be sent back to split it. Never merge unrelated concerns
to make an item look bigger.

When the work list is complete, run:

```bash
gewu harness next
```
