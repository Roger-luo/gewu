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
a line count runs out. Do not split a sentence, declaration, or other semantic
unit across item boundaries merely to meet the estimate.

Keep mechanically repeated configuration together when it supports one review
decision. Settings that differ only by a command name, label, or priority are
usually one item; repeat a shared caveat once and point back to it rather than
making the reviewer rediscover it in several chunks.

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
headers. Use this estimate as planning guidance:

{{height_guidance}}

At planning time the exact prose does not exist, so Gewu cannot measure a
chunk's final rendered height. It does enforce a source-row ceiling for broad
items: an item spanning more than one change may own at most
{{max_chunk_lines}} × {{max_chunks}} non-blank source rows. A one-change item is
exempt so an intentionally summarized generated artifact can be isolated in
its own item. Budget below that ceiling conservatively and leave room for
prose. Exact height is checked when an item is expanded; if its chunks do not
fit, split the views or return a smaller expansion rather than merging
unrelated concerns.

When working directly in the CLI, continue with:

```bash
gewu harness next
```

When a driver asked for the JSON as your response, return only that checkpoint;
planning has no measure/submit inner loop. The driver writes it, validates it,
and runs the continuation command.
