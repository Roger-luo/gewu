Read the draft back, one chunk at a time, and answer for each:

- Does every sentence of `summary` and of each `narrative` block in `content`
  describe something this chunk's own quotes show? Cut what they do not prove.
- Does an architecture statement distinguish a documented role from code that
  currently implements it? Does a generated or managed-file comment remain
  evidence of ownership rather than becoming an unsupported manual-work claim?
- Does the prose name the symbol — `Quote`, `context`, the method — instead of
  borrowing harness workflow terms? The assignment calls owned ranges
  "spans", but that does not make "span", "panel", or "step" a name in the
  code being explained.
- Would the sentence read the same outside a walkthrough? Drop what narrates
  how the review was made. A sentence that tells a newcomer how this chunk
  connects to the last one or to the overview's map is not narration; keep it.
- When the change claims an effect (in `.gewu/description.md`) and this
  chunk delivers part of it, does the reviewer learn how, with numbers they
  could check — bytes per entry, work per step, the size it scales with —
  and what it costs? A chunk that names functions and line ranges without
  the effect has not shown that the change is an improvement.
- Is every number in an explainer traceable to its `source`, and does a
  cost model's default match the size the claim was measured at?
- Does a chunk that cites a measured result say its own share of it, and
  is that share consistent with its cost model? Placing this chunk's ratio
  beside the whole change's number, so the reader credits this part with the
  whole, is misleading even when every number is true.
- Could a reviewer who skipped the previous chunk tell, from this chunk alone,
  what part of the change they are reading and why it matters?
- Do the summary and the narrative say different things? The summary says what
  the chunk shows; the narrative says what to look for in it. A narrative that
  opens by restating the summary costs a sentence and teaches nothing.
- Is a finding stated as what is true — "nothing else calls `unwritten`" — not
  as what you did to learn it — "a search of `crates/` finds"?
- Do you call code "unchanged" only when it is outside the diff? In a review
  from an empty base every line under review is new.
- Is each point made once? "X, not Y", a second phrasing of the same sentence,
  and a sentence that only announces the next one all cost rows and teach
  nothing.
- Does an exclusive claim — "only", "never", "every", "no other" — rest on a
  search of the whole repository, tests included? `gewu harness context
  search` covers the diff; reach the rest with the repository access your
  session has. If you could not search it all, drop the quantifier.
- Does a sentence restate the chunk's title, the overview, or what the quoted
  line already says plainly? Cut it; the reviewer reads all three. A summary
  says what this chunk adds, not the change as a whole.
- Does the prose point only at lines a view shows? Lines between two views of
  the diff are hidden behind an elision marker: quote them, or do not lean on
  them. Code outside the diff cannot be quoted; say it is unchanged and where
  it lives, or put it in a `context` pane.
- Does every line number you cite appear in the gutter the reviewer sees? Those
  are new-file numbers; removed lines have none, so name them by what they say.
- Are human-facing fields readable as rendered text? Replace serialized JSON,
  escaped quotes, and visible `\n` sequences with prose and fenced examples.
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
- Does every quote start and end at a readable semantic boundary? Do not leave
  one chunk ending and the next beginning in the middle of the same sentence,
  declaration, or example.
- When this chunk presents a new form of a structure that already existed, can
  a reviewer see the earlier definition — its fields, not an alias that keeps
  its name — beside the new one? Does the prose name the fields that were
  dropped or added, say what the new form buys, and say why the change needed
  it? A sentence that restates the new fields does not answer it. The earlier
  definition is in the diff quote or in a refactor `context` (`before`,
  `after`, `reason`).
- Does the chunk say why its change was made, not only what it is? A reviewer
  who must guess the motive cannot judge whether the change achieves it.
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

When the change configures tools, hooks, policy, builds, or generated files,
also ask the following. Skip them for a change that has none.

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
- Does a version or compatibility field have its correct role? Distinguish a
  minimum supported version from the toolchain actually selected, and say when
  the configured workflow never exercises that minimum.
- Is unchanged boilerplate reviewed as one coherent unit? Do not split a
  standard license, generated artifact, or other indivisible text into several
  stops unless distinct review decisions genuinely depend on the split.
