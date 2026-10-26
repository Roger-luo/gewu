Read the draft back, one chunk at a time, and answer for each:

- Does every sentence of `summary` and of each `narrative` block in `content`
  describe something this chunk's own quotes show? Cut what they do not prove.
- Does the prose distinguish configuration from guarantees and observed
  behavior? Ordering or an omitted option does not prove fail-fast execution,
  and a local hook does not prove central enforcement. Name installation,
  bypass, CI, or execution limits when they matter to the stated intent.
- Does every claimed command outcome come from an execution trace? Without
  one, describe the source-derived expectation and the exact configuration,
  including shell quoting, filters, hook installation, and failure messages.
- When one language embeds another, did you trace the literal characters across
  every quoting layer? Do not describe the intended inner command if preserved
  escapes make the actual command different or invalid. If execution was not
  available, say that plainly instead of claiming the check works.
- For an executable policy example, did you report the exact extracted command,
  representative input, exit status, and diagnostic? If not, remove the claimed
  input/output matrix. A malformed configured command is observable behavior,
  not an invitation to explain the corrected command it probably intended.
- For validators, hooks, and policy regexes, did you name consequential edge
  cases and workflow-generated inputs, rather than paraphrasing only the happy
  path? State what is accepted and rejected only after reading the exact
  expression.
- Does wording describe only what is configured? Say "no options are
  configured", not that a tool takes none; describe a whole file class rather
  than an incomplete hand-picked list; do not assert a file's provenance when
  the repository does not establish it.
- Did you verify external-tool semantics before turning a numeric priority into
  an execution order? If the repository does not establish the rule, use the
  same explicit uncertainty everywhere rather than asserting it in one chunk
  and hedging in another.
- Does an architecture statement distinguish a documented role from code that
  currently implements it? Does a generated or managed-file comment remain
  evidence of ownership rather than becoming an unsupported manual-work claim?
- Does the prose name the symbol — `Quote`, `context`, the method — instead of
  borrowing harness workflow terms? The assignment calls owned ranges
  "spans", but that does not make "span", "panel", or "step" a name in the
  code being explained.
- Would the sentence read the same outside a walkthrough? Drop what narrates
  the review itself; the reviewer can see which chunk they are in.
- Is each point made once? "X, not Y", a second phrasing of the same sentence,
  and a sentence that only announces the next one all cost rows and teach
  nothing.
- Are human-facing fields readable as rendered text? Replace serialized JSON,
  escaped quotes, and visible `\n` sequences with prose and fenced examples.
- Does a version or compatibility field have its correct role? Distinguish a
  minimum supported version from the toolchain actually selected, and say when
  the configured workflow never exercises that minimum.
- Do the chunks contradict the earlier investigation overview, including its
  filters and exceptions? Keep each chunk accurate even if the overview is
  wrong, and name the discrepancy in `confidence`; an expansion cannot rewrite
  the investigation checkpoint.
- Does an aggregate claim still hold after checking the evidence for every
  component it covers? Preserve distinctions such as internal relationships
  versus external dependencies, and do not let accurate component details
  contradict an opening generalization.
- Has a foundational product or architecture rule appeared before a chunk uses
  it as justification? Name later or earlier material directly instead of
  relying on positional words such as "below".
- Is unchanged boilerplate reviewed as one coherent unit? Do not split a
  standard license, generated artifact, or other indivisible text into several
  stops unless distinct review decisions genuinely depend on the split.
- Does every quote start and end at a readable semantic boundary? Do not leave
  one chunk ending and the next beginning in the middle of the same sentence,
  declaration, or example.
- Is each product rule or concept defined once before use, with later chunks
  referring back instead of restating it?
- Is every introduced term defined at the level needed to judge the change,
  including what a versioned resolver or a tool-specific resource controls?
  Also distinguish facts the repository establishes from semantics left to an
  external tool.
- Does each view hold lines that are read together? Two definitions a reviewer
  would meet separately get two views and a sentence each, even when the diff
  puts them side by side.
- When a view lists methods, does the prose name a caller? Otherwise the
  reviewer learns what each one does and never when it is used.
- Does `confidence` name something you could not settle from the repository? If
  it hedges or repeats `summary`, leave the field out.
