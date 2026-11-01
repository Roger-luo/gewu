# Gewu harness: investigate

If you can read every change in `.gewu/review-input.json` in full, skip this
paragraph and follow the protocol after it. If you cannot — a repository-scale
input — run `gewu harness research start`, then `gewu harness status`, instead.
That selects bounded, resumable source-page investigation with verified
evidence and a grounded overview; never embed the complete manifest in one
author request.

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
uncertainty in the `summary` of the finding it limits, instead of filling a gap
by inference.

When a change configures a tool, hook, or gate, also inspect command quoting,
filters, installation requirements, and bypass paths literally before
summarizing it. For commands embedded
through several syntaxes (for example configuration, then a shell, then a
regular expression), trace the exact characters through every quoting layer;
do not silently normalize escapes or repair malformed text into the command it
appears to intend. When the bounded tools permit it, execute the exact extracted
command against disposable representative inputs and record exit status and
diagnostics. If you cannot execute it, say so in the finding and describe only
what the text establishes; without a trace, do not claim an input is accepted
or rejected.
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
  "illustration": { "kind": "flow", "steps": ["first part", "what it feeds", "where it ends up"] },
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

`illustration` is the reviewer's map of the change: how its parts connect, so
every chunk after it has a place to sit. Give one whenever the change touches
more than one part. Use `{"kind": "flow", "steps": [...]}` for a sequence —
the path a request, an event, or a decision takes through the changed code —
or `{"kind": "diagram", "text": "..."}` for a small ASCII drawing when the
parts branch or call each other. Keep a diagram under about 12 lines and 70
columns, name the real symbols, and order it the way the walkthrough will go.
Omit it only for a change confined to one place. The map covers the whole
change: every part a work item will walk through has a place on it, so a
review whose sections span several concerns is not mapped by one of them.

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
