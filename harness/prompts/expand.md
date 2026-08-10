# Gewu harness: expand `{{item_id}}`

Write the walkthrough for one work item. Ignore every other item; they are not
your concern and may never be expanded as planned.

**{{item_title}}** — {{item_purpose}}

Spans this item covers:

{{covers}}

Spans are numbered within the hunk body, from 1. Each change carries
`body_lines`; the file line a body line lands on is shown in the review's own
gutter, so you do not need to compute it.

A reviewer cannot learn anything from a wall of diff. Your job is to break these
spans into views small enough to understand, each introduced by prose that says
what to look for before they look at it. Show the same lines twice when the
reader needs the earlier context again — a view of lines 1-25 followed by a view
of lines 10-50 is good narrative, not duplication.

Write `.gewu/harness/expansion.json`:

```json
{
  "schema_version": 1,
  "source": { "base": "…", "head": "…", "diff_sha256": "…" },
  "item_id": "{{item_id}}",
  "chunks": [
    {
      "id": "stable-unique-id",
      "title": "short purpose",
      "summary": "one or two sentences on observable intent",
      "kind": "concept|refactor|integration|test|other",
      "content": [
        { "type": "narrative", "markdown": "what to look for in the next view" },
        {
          "type": "changes",
          "quotes": [{ "change_id": "change-id", "start": 1, "end": 24 }],
          "presentation": "diff|added_code|rendered_markdown|summary_only"
        }
      ]
    }
  ]
}
```

`source` is the `source` object of `.gewu/review-input.json`, copied field
for field. Gewu compares it to reject a checkpoint written against a manifest
that has since moved.

Rules:

- At most {{max_chunks}} chunks. Quote only lines this item covers, and between
  them show every line of every span at least once.
- A chunk is the unit a reviewer comments on. Keep it to at most
  {{max_chunk_lines}} rendered rows — title, summary, prose, and quote headers
  included. Explain `oversized_reason` only when a single line cannot fit.
- Alternate prose and views. Say what to look for, show the lines that prove it,
  move on. A view of five well-chosen lines beats a view of thirty.
- Choose the presentation that costs the reviewer least: `diff` when
  removed-versus-added behavior matters, `added_code` for new code,
  `rendered_markdown` for document sections, `summary_only` for generated
  artifacts nobody should read line by line.
- A `context` pane is not charged to the budget: reading it is not reviewing.
  Attach one where the chunk turns on a name the reviewer has just met — a type
  this change introduces — and keep it to what they need to read the code in
  front of them. It is still drawn, so it is not free to their patience.
- Put uncertain interpretation in `confidence`; never invent motivation.

To see what a draft costs before submitting it, write `expansion.json` and run
`gewu harness status`: it reports each chunk's measured height against the
budget, and any `context` pane alongside it, without advancing. Use it rather
than guessing which sentence to cut.

{{rubric}}

When the expansion is complete, run:

```bash
gewu harness next
```
