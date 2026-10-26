# Gewu harness: rewrite {{count}} chunk(s)

The code is not at fault here. The reviewer is telling you that the walkthrough
explaining it does not do its job, and they have said where.

{{requests}}

Write `.gewu/harness/revision.json`:

```json
{
  "schema_version": 1,
  "source": { "base": "…", "head": "…", "diff_sha256": "…" },
  "chunks": []
}
```

`source` is the `source` object of `.gewu/review-input.json`, copied field
for field. Gewu compares it to reject a checkpoint written against a manifest
that has since moved.

`chunks` holds the rewritten chunk(s) in full, in the same shape they have now.
Here is what the review currently says, to rewrite rather than start from:

{{chunks}}

Rules:

- Replace exactly these chunk ids — all of them, and nothing else. Their ids are
  what the reviewer's feedback is filed against; renaming one orphans the
  request it answers.
- Show the same lines the chunk shows now. Where a view begins and ends, how
  many views there are, and what you say about them are yours to change; which
  code the reviewer is looking at is not.
- Keep each chunk to at most {{max_chunk_lines}} rendered rows. In a direct CLI
  session run `gewu harness status` after writing the file: with a draft in
  place it reports measured heights without advancing. Under a driver, send the
  complete revision through its measure action instead.
- A `context` pane is not charged to those rows, so a reviewer asking to be
  reminded what a type is can be answered with one. Add it where they asked;
  keep it to what they need to read the code in front of them.
- You cannot quote code the chunk does not already show, so a request for
  evidence from elsewhere in the repository — how a method is called, what a
  type is used for — is answered in prose that names the caller, not by adding
  a view of it.
- Answer the request. A rewrite that reads better but leaves the reviewer's
  point unaddressed will come straight back.

{{rubric}}

Then, for each request you have answered:

```bash
gewu review resolve --id <feedback-id>
```

When working directly in the CLI, then run `gewu harness next`. When a driver
asked for the JSON as your response, return only the checkpoint; the driver
writes and validates it before continuation. If you believe a request is
mistaken, say so to the human and stop rather than resolving it.
