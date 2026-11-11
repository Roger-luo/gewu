# Gewu harness: polish {{count}} chunk(s)

A fresh reader proofread this batch before the reviewer sees it, and scored
these chunks below 2. Their problems are the brief: rewrite each chunk until
it answers them. {{round}}

{{findings}}

Write `.gewu/harness/revision.json`:

```json
{
  "schema_version": 1,
  "source": { "base": "…", "head": "…", "diff_sha256": "…" },
  "chunks": []
}
```

`source` is the `source` object of `.gewu/review-input.json`, copied field
for field. `chunks` holds the rewritten chunk(s) in full, in the same shape
they have now:

{{chunks}}

Rules:

- Replace exactly these chunk ids, all of them, and nothing else.
- Show the same lines the chunk shows now. Where a view begins and ends and
  what you say about it are yours to change; which code is shown is not.
- Keep each chunk to at most {{chunk_budget}}. In a direct
  CLI session `gewu harness status` measures a draft without advancing; under
  a driver, use its measure action.
- An earlier definition you cannot quote goes in a refactor `context`, which
  is not charged to that budget: `before` is the old definition as written,
  with its fields — not a type alias that keeps its name — `after` is the new
  one, and `reason` names the fields dropped or added and what that buys.
  Find it in the patch's removed lines or with `gewu harness context base
  --path <path>`.
- Say why the change was made in a sentence of its own. When the repository
  does not say, state what the new form enables and put the open question in
  `confidence`; never invent a motive.

{{rubric}}

When working directly in the CLI, run `gewu harness next`; Gewu proofreads the
rewrite again. When a driver asked for the JSON as your response, return only
the checkpoint.
