# Gewu harness: investigate

Inspect every change in `.gewu/review-input.json` and determine what the change
does before organizing the review.

Explore relevant definitions, consumers, integration points, tests, and useful
history. Base every claim on repository evidence. Do not edit project source.
Under a driver, use the frozen checkout and the bounded evidence tools it
offers. In a direct CLI session, use the read-only repository inspection
commands available in your environment; do not edit source. History and
base-side content are optional evidence sources: if they are unavailable or the
base contains no file, do not spend requests looking for them. Treat that
repository view as the whole evidence base for this run. When runtime behavior
was not executed, describe only what the source or configuration establishes;
never turn "configured to run" into "runs" or "passes". Record a limitation or
uncertainty instead of filling a gap by
inference. Inspect command quoting, filters, installation requirements, and
bypass paths literally before summarizing a tool or gate. For commands embedded
through several syntaxes (for example configuration, then a shell, then a
regular expression), trace the exact characters through every quoting layer;
do not silently normalize escapes or repair malformed text into the command it
appears to intend. When the bounded tools permit it, execute the exact extracted
command against disposable representative inputs and record exit status and
diagnostics. Without that trace, do not claim an input is accepted or rejected.
Enumerate meaningful edge cases, including workflow-generated inputs, when they
affect how a gate is used.

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
not which files moved. Every overview claim must keep the conditions and
exceptions that make its detailed finding true. Define every tool, protocol,
or domain term the chunks rely on; omit `concepts` only when the change
introduces no new vocabulary.

Before making an aggregate claim about a group of packages, services, tools, or
other components, check every member against its own evidence. Preserve
meaningful distinctions such as internal relationships versus external
dependencies, even when a shorter generalization sounds approximately true.
Reconcile each opening claim with the detailed findings before writing the
checkpoint.

Do not infer external-tool semantics from a field name or number: verify how a
priority orders work, or state that the repository does not establish it. Do
not infer that a managed file is manually maintained, or that an architectural
role is implemented when the named crate contains only a placeholder. Keep
those qualifications in the overview as well as the detailed finding.

Write human-facing fields as prose, Markdown, or the code they are meant to
show. Never paste a serialized JSON string into an overview, context field, or
narrative: escaped quotes and `\n` sequences are storage syntax, not something a
reviewer should have to decode.

When working directly in the CLI, continue with:

```bash
gewu harness next
```

When a driver asked for the JSON as your response, return only that checkpoint;
investigation has no measure/submit inner loop. The driver writes it, validates
it, and runs the continuation command.
