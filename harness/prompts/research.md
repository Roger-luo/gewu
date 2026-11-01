# Gewu harness: bounded investigation

This run requires the durable research ledger. Do not replace it with a
handwritten whole-input checkpoint or copy the full manifest into a prompt.

1. If this is a new source revision and no current research ledger exists, run
   `gewu harness research start`. Its limits are persisted before author work.
2. Run `gewu harness research next`. It returns one bounded JSON task. Inspect
   that page and answer its instructions in a JSON file; submit with
   `gewu harness research accept --checkpoint <file>`. Invalid notes leave the
   pending task unchanged. Repeat until `next` returns `null`.
3. Read completed findings using `gewu harness research findings`. Continue
   with `--cursor '<next object>'`. Do not concatenate every page into a single
   model request. Resolve open questions using bounded source lookups before
   making cross-file claims. Page boundaries are transport boundaries, not
   semantic work-item boundaries.
4. Write a grounded overview and submit it with
   `gewu harness research assemble --overview <file>`. Then run
   `gewu harness next` to validate and enter planning.

The overview has this shape. Copy `source` and `ledger_sha256` from a findings
page. Every text field cites zero-based finding indexes; the page's
`start_index` gives the index of its first finding. Citations must actually
support the claim; existence checks do not establish semantic accuracy.

```json
{
  "schema_version": 1,
  "source": { "base": "…", "head": "…", "diff_sha256": "…" },
  "ledger_sha256": "…",
  "intent": { "text": "overall source-backed intent", "findings": [0] },
  "changes": [{ "text": "behavior with its conditions", "findings": [0, 1] }],
  "concepts": [{ "text": "a needed term and its definition", "findings": [0] }]
}
```

Use one to five change bullets. Concepts may be empty. Keep the complete
overview within the persisted checkpoint byte budget. Accepted notes and
overview references are retained; assembly does not record a reviewer verdict.
The author transport must separately bound the entire model request and count
actual calls, retries and usage—these CLI task limits do not measure inference.
