# Gewu harness: investigate

Inspect every change in `.gewu/review-input.json` and determine what the change
does before organizing the review.

Explore relevant definitions, consumers, integration points, tests, and useful
history. Base every claim on repository evidence. Do not edit project source.

Write `.gewu/harness/investigation.json`:

```json
{
  "schema_version": 1,
  "source": { "base": "…", "head": "…", "diff_sha256": "…" },
  "intent": "one sentence describing the overall change",
  "changes": ["three to five behavioral bullets for the reviewer's first screen"],
  "concepts": ["a name the reviewer must learn to read this change, if any"],
  "findings": [
    {
      "change_ids": ["change-id"],
      "summary": "what these edits do and why they belong together",
      "evidence": ["path or symbol that supports the finding"]
    }
  ]
}
```

`source` is the `source` object of `.gewu/review-input.json`, copied field
for field. Gewu compares it to reject a checkpoint written against a manifest
that has since moved.

Every manifest change ID must appear in at least one finding.

`intent`, `changes`, and `concepts` become the review's opening screen, so
write them for the human who has not seen this code: what the change achieves,
not which files moved. Omit `concepts` when the change introduces no new
vocabulary.

When the checkpoint is complete, run:

```bash
gewu harness next
```
