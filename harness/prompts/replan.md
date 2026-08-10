# Gewu harness: the plan is stale

The code has moved under work that had not been expanded, so the remaining plan
can no longer be trusted. Gewu re-diffs the tree rather than being told about
edits, so it knows the manifest changed but not who changed it.

{{stale}}

Reviewed chunks that still match the code have been kept, along with their
feedback. {{remainder}} change(s) are now uncovered.

Re-investigate only what is uncovered — the manifest at
`.gewu/review-input.json` has been refreshed — and write
`.gewu/harness/investigation.json` for that remainder. Findings for changes
already covered by surviving chunks are not needed.

When the investigation is complete, run:

```bash
gewu harness next
```
