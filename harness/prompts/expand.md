# Gewu harness: expand `{{item_id}}`

Write the walkthrough for one work item. Ignore every other item; they are not
your concern and may never be expanded as planned.

**{{item_title}}** — {{item_purpose}}

Spans this item covers:

{{covers}}

Investigation evidence relevant to those spans:

{{investigation}}

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
      "context": {
        "kind": "concept",
        "definition": "brief supporting definition",
        "example": "minimal example",
        "relation": "how it relates to this chunk"
      },
      "confidence": "optional uncertainty the repository could not settle",
      "oversized_reason": "optional reason a single indivisible line exceeds the budget",
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

- `summary` must be nonempty and at most 500 characters; this is a validation limit, not just a style preference.

- At most {{max_chunks}} chunks. Quote only lines this item covers, and between
  them show every line of every span at least once.
- A chunk is the unit a reviewer comments on. Keep it to at most
  {{max_chunk_lines}} rendered rows — title, summary, prose, and quote headers
  included. Validation uses this estimate:

  {{height_guidance}}

  Measurement is still the authority because wrapping depends on the actual
  draft. In a direct CLI session run `gewu harness status`; under a driver send
  the complete draft through its measure action and inspect the returned
  result. Explain `oversized_reason` only when a single indivisible line cannot
  fit.
- Alternate prose and views. Say what to look for, show the lines that prove it,
  move on. A view of five well-chosen lines beats a view of thirty.
- Choose `kind` by the decision the chunk supports: `concept` introduces a new
  name or model; `refactor` compares behavior-preserving structure;
  `integration` covers wiring across components; `test` covers verification;
  `other` is the default for configuration, legal text, manifests, generated
  metadata, and anything that does not fit the earlier purposes. `context.kind`
  may support the chunk but does not have to equal its `kind`.
- Choose the presentation that costs the reviewer least: `diff` when
  removed-versus-added behavior matters, `added_code` for new code,
  `rendered_markdown` only for paths ending in `.md`, `summary_only` only for
  mechanically generated artifacts nobody should read line by line. Never
  hide legal text, policy, configuration, or other human-authored obligations
  behind `summary_only`.
- A `context` pane is not charged to the budget: reading it is not reviewing.
  Attach one where the chunk turns on a name the reviewer has just met — a type
  this change introduces — and keep it to what they need to read the code in
  front of them. It is still drawn, so it is not free to their patience.
- Put code in a context `example` as code text, without an outer Markdown fence;
  Gewu supplies the visual code boundary. Keep explanation and headings in the
  `definition` or `relation` fields instead of embedding them in the example.
- Put uncertain interpretation in `confidence`; never invent motivation.
- `context`, `confidence`, and `oversized_reason` are optional chunk-level
  fields. Omit them when they do not add information. When `context` is present,
  every field of its selected shape is required: a concept has `definition`,
  `example`, and `relation`; a refactor has `before`, `after`, and `reason`.

In a direct CLI session, write `expansion.json` and run `gewu harness status`
before submitting: it reports each chunk's measured height against the budget,
and any `context` pane alongside it, without advancing. Under a driver, use its
measure action on the complete draft instead. Use the result rather than
guessing which sentence to cut.

{{rubric}}

When working directly in the CLI, continue with:

```bash
gewu harness next
```

When a driver asked for the JSON as your response, return only that checkpoint;
the driver writes it, validates it, and runs the continuation command.
