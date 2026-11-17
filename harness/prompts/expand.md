# Gewu harness: expand `{{item_id}}`

Write the walkthrough for one work item. Ignore every other item; they are not
your concern and may never be expanded as planned.

**{{item_title}}** — {{item_purpose}}

Spans this item covers:

{{covers}}

Investigation evidence relevant to those spans. It is your notes, including
what you could not run; it is not text for the reviewer, so do not carry its
limitations or first-person wording into chunk prose:

{{investigation}}

Spans are numbered within the hunk body, from 1. Each change carries
`body_lines`; the file line a body line lands on is shown in the review's own
gutter. To read a span with both — the body number a quote uses, then the
file line the reviewer sees and prose cites — run `gewu harness context source
--change-id <id> --start 1 --end <body_lines> --numbered`.

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
      "oversized_reason": "optional: the indivisible unit that makes this chunk exceed the budget",
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
  them show every line of every span at least once; blank lines and unchanged
  context at a hunk's edges are exempt.
- A chunk is the unit a reviewer comments on. Keep it to at most
  {{chunk_budget}} — title, summary, prose, and quote headers
  included. Validation uses this estimate:

  {{height_guidance}}

  Measurement is still the authority because wrapping depends on the actual
  draft. In a direct CLI session run `gewu harness status`; under a driver send
  the complete draft through its measure action and inspect the returned
  result. Set `oversized_reason` when the chunk is one unit a reviewer reads
  together — a single test, a function with its doc comment, a table — and
  splitting it would put setup and assertion, or doc and body, on different
  stops. Name the unit in the reason. It is not a way to avoid choosing lines:
  the reader pages through an oversized chunk, so a smaller view still wins
  whenever the unit is not indivisible.
- Alternate prose and views. A paragraph says what to look for, the view shows
  the lines that prove it, and the next paragraph picks up from what the view
  showed. A view of five well-chosen lines beats a view of thirty. When one
  change has several call sites, each view gets the sentence for that site in
  the narrative block immediately before it. Do not gather every line range
  into one paragraph above a changes block that holds all of the views: the
  reviewer reads the sentence beside the view whose lines it names, and a
  sentence that lists several ranges is split across them.
- Choose `kind` by the decision the chunk supports: `concept` introduces a new
  name or model; `refactor` compares behavior-preserving structure;
  `integration` covers wiring across components; `test` covers verification;
  `other` is the default for configuration, legal text, manifests, generated
  metadata, and anything that does not fit the earlier purposes. `context.kind`
  may support the chunk but does not have to equal its `kind`.
- User-facing text that lives in a source string — help, messages, labels —
  is read as code, escapes and all. Show the literal, and say in prose what the
  reader will see, so they do not have to decode `\n` sequences to judge it.
- YAML front matter at the top of a `.md` file is not Markdown; show it as
  `added_code`, and the document after it as `rendered_markdown`. Rendered
  Markdown wraps and spaces out, so budget about twice its source rows.
- Choose the presentation that costs the reviewer least: `diff` when
  removed-versus-added behavior matters, `added_code` for new code,
  `rendered_markdown` only for paths ending in `.md`, `summary_only` only for
  mechanically generated artifacts nobody should read line by line. A lockfile
  or a file whose header says a tool writes it counts; name what marks it. Never
  hide legal text, policy, configuration, or other human-authored obligations
  behind `summary_only`.
- A `context` pane is not charged to the budget: reading it is not reviewing.
  Attach one where the chunk turns on a name the reviewer has just met — a type
  this change introduces — and keep it to what they need to read the code in
  front of them. It is still drawn, so it is not free to their patience: keep it
  under half the row budget, and the measurement says when it is not.
- When the chunk's point is a new form of a structure that already existed —
  a struct, an enum, an encoding, a stored word, a codec — the reviewer has
  to see both shapes and be told why one replaced the other. Show the earlier
  definition itself: the old struct or enum with its fields, from the diff's
  removed lines or the base revision (`gewu harness context base --path
  <path>`). A type alias
  or a re-export that keeps the old name is not that definition — it says what
  the name means now, not what it held. Then say, explicitly:
  - which fields or variants were dropped, packed together, or added, by name;
  - what that buys — the property the title claims, such as fewer bytes per
    word, a `Copy` type, or one allocation instead of two — and the lines that
    show it;
  - why the change needed it: the caller, test, document, or commit message
    that motivates it. When the repository does not say, state what the new
    form enables and put the open question in `confidence`; never invent a
    motive.

  When the earlier form still exists beside the new one — the new type is
  added, not swapped in yet — say so, and still show the earlier form, since
  the comparison is the point.

  If the quote shows only the new definition, put the earlier one in a
  refactor `context`: `before` is the old definition as written, `after` the
  new one, and `reason` names the dropped fields and the advantage. A diff
  quote that includes the removed definition already puts it on the page, but
  the narrative still names what was dropped. Restating the new fields, or
  the chunk title, answers none of this.
- "Before" is the base revision. Derive an old record's size from the types
  as they were at the base (`gewu harness context base --path <path>`), not
  from types this change introduces elsewhere in the diff: comparing against
  the new, smaller type both understates this chunk's saving and counts
  another chunk's. Size a structure by what it actually holds at the
  benchmark's scale (a table built over the enlarged candidate set holds
  more than the basis), and say when it is alive: a structure built and
  dropped inside one step saves at that moment, which lowers peak memory only
  if that moment is the peak.
- Do not narrate the view. "Lines 20–34 implement `multiply` and `swap`"
  tells the reviewer what the gutter already shows; say what to look for in
  those lines instead. Say that a file is new once, where it first appears,
  not again in every chunk that quotes it. Name the one caller that shows how
  the code is used, not every caller, importer, and re-export.
- When you name callers, say which are production and which are tests; a
  test-only constructor is not evidence of how the change is used. Name only
  code that calls it at the head, checked with a search of the head; base
  code that does something similar is not a caller. When the diff contains
  no caller of something new, say so plainly rather than finding one.
- A section that delivers part of a measured result states its share once,
  as an estimate when the change measures no part on its own, even when it
  carries no claim block, and says whether the structure is alive at the
  moment the result was measured (peak memory, say).
- Connect the chunk to what the change claims. When `.gewu/description.md`
  claims an effect — faster, smaller, fixes a bug — and this chunk is one of
  the reasons, say how, with the mechanism and its size: not "drops the owned
  key" but "each entry shrinks from about 41 bytes to 5, so the index at
  2^19 strings falls from about 40 MiB to 5 MiB". Name what it costs too — an
  extra comparison, a lock, a precondition — so the reviewer can weigh it. A
  chunk that only lists functions, call sites, and line ranges has told the
  reviewer where things are, not why the change is better.
- Say why each part is there in a sentence the reviewer can point at. For a
  pull request, `.gewu/description.md` ends with its commits' messages under
  `## Commits`; a part's motive is often in the commit that added it. Name
  that commit when you give its reason. Ask too what a reviewer would ask of
  each design choice the chunk shows — why here and not in the pass beside
  it, why reject rather than accept, why all at once rather than one by one
  — and answer it. When no commit, test, or comment says why, say so in the
  narrative, in one sentence that names the alternative: "Nothing in the
  change says why validation runs in the rewrite rather than in
  `CodeBlockValidation`." Never supply a motive yourself, and never leave the
  question unasked: a reviewer who must guess the reason cannot judge it.
- Explainers make that argument checkable. Gewu draws them from data you
  give; you never write HTML. They are expected, not decoration: when this
  item delivers a claim from `description.md`, its first chunk quotes the
  claim in a `claim` block; when the point is that a record shrinks, a cost
  scales with a size, an algorithm proceeds in steps, or work moves out of a
  loop, the chunk that shows it carries a `layout`, `cost_model`, `stepper`,
  or `timeline`. This holds with no claim at all: an elimination, a rewrite
  pass, or a structure moved through states by several operations (allocate,
  lend, return, free) is a `stepper` whatever the change is for. Prose that
  says "32 bytes instead of 48" without a layout has left the reviewer to
  picture it; prose that walks through a reduction one sentence at a time has
  made them run it in their head. Add them as `content` blocks:

  ```json
  { "type": "explainer", "kind": "claim",
    "claim": "words copied exactly from description.md",
    "result": "optional measured result it reports, e.g. 940 → 367 MiB (−61%)",
    "share": "required with result: this chunk's part, e.g. about 36 MiB of the 573 MiB drop",
    "how": "Markdown: how this chunk contributes to it" }
  { "type": "explainer", "kind": "layout", "title": "One cache entry",
    "rows": [ { "label": "Before", "fields": [ { "name": "Key copy", "bytes": 24 },
                                               { "name": "u32", "bytes": 4 },
                                               { "name": "pad", "bytes": 4 } ] },
              { "label": "After", "fields": [ { "name": "u32", "bytes": 4 } ] } ],
    "source": "where the sizes come from: a type, a size_of test, a comment" }

  (The sizes in these examples are placeholders; take yours from the code.)
  { "type": "explainer", "kind": "cost_model", "title": "Index memory at size B",
    "params": [ { "id": "b", "label": "Strings in the basis (B)", "min": 32768,
                  "max": 2097152, "default": 524288, "scale": "log2",
                  "marks": [131072, 524288] } ],
    "series": [ { "label": "Before", "formula": "next_pow2(b * 8 / 7) * 41", "unit": "bytes" },
                { "label": "After", "formula": "next_pow2(b * 8 / 7) * 5", "unit": "bytes" } ],
    "note": "optional: what the model leaves out",
    "source": "where the numbers come from" }
  { "type": "explainer", "kind": "stepper", "title": "One lookup, step by step",
    "language": "rust", "source": "src/index.rs:37-41", "start_line": 37,
    "steps": [ { "caption": "Markdown: what happens now", "code": "the code at this step",
                 "highlight": "38-39", "state": [ { "name": "i", "value": "41_027" } ] },
               { "caption": "the next state; omit code to keep the previous step's",
                 "highlight": "40" } ] }
  { "type": "explainer", "kind": "timeline", "title": "Allocations across six steps",
    "steps": 6, "step_label": "step", "unit": "allocations",
    "lanes": [ { "label": "Before", "work": [ { "label": "workspace", "when": "each", "cost": 3 } ] },
               { "label": "After", "work": [ { "label": "workspace (pooled)", "when": "once", "cost": 3 } ] } ] }
  ```

  - `claim`: only for a claim in `description.md`, copied exactly (a phrase
    of a longer sentence is fine, and better when the sentence also claims
    things this chunk does not explain); Gewu rejects a claim it cannot find
    there, and any claim when there is no description. Quote each claim
    once per section: on the first chunk that delivers it, or, when nothing
    here delivers it yet, on the chunk whose code it is about, with `how`
    saying what is still missing.
  - When the description makes a claim this chunk relates to but does not
    credit it with a measured saving — or the new code is not called yet —
    quote the claim with `how` alone, and say in `how` what is and is not
    established. Never supply a `share` the description and the code do not
    support.
  - A measured `result` belongs to the whole change. Never set a part's
    ratio ("8× smaller") beside the whole's number ("−61%") as if one explains
    the other: `share` says how much of the result this chunk accounts for,
    from your cost model or your reasoning, and says whether that share is
    estimated or measured — an estimate set beside a measurement must say so —
    and `how` names what else contributes, and says in which phase the
    saving falls and whether that phase is where the result was measured (a
    peak): savings in different phases do not add up toward one peak. A
    small honest share convinces; an implied large one is
    found out. A share is this chunk's part of the described result, never
    what the new code itself costs (say that in `how` or the narrative); a
    chunk that only enables a result states none. When a structure is counted
    in an earlier section's share too, say so, so the shares are not added
    twice.
  - `layout`: when the point is what a record holds — fields dropped, packed,
    or shrunk. Sizes come from the code: a `size_of` assertion when there is
    one, otherwise the field types, with padding drawn as its own `pad`
    field and the alignment rule named in `source`. Never guess a size; if
    you cannot derive it, do not draw it.
  - `cost_model`: when the claim is about scale — memory or work that grows
    with a size the reviewer can picture. Parameters are whole numbers the
    reviewer drags (1 to 3 of them; `log2` steps through powers of two);
    formulas use only `+ - * / ^`, parentheses, the parameter ids, and
    `log2 ceil floor min max next_pow2`. Put fractions such as a load
    factor in the formula. Every series must be finite and not negative at
    each parameter's min, default, and max. Use the benchmark's own sizes for
    the default and as `marks`, and say in `note` what the model does not
    count. A fact from an outside library, such as a hash table's load
    factor, may be used when `source` names the library it comes from.
  - `stepper`: when the point is how an algorithm proceeds — a lookup, a
    probe sequence, an elimination, a state machine — and a reader would
    otherwise simulate it in their head. Step through one concrete example:
    the one `description.md` gives, a test's input, or small illustrative
    values said to be illustrative. `state` holds the data the reader would
    track, not only counters: a matrix's rows after each pivot, which slots
    of a register are full or lent, which sites of a word a block occupies
    (`word 0: [a a a · ]`). When the point is how several statements
    act on one structure in turn (a lifecycle, a protocol), the stepper's
    `code` may be a short program from a test or the description instead of
    the quoted lines, with `source` naming where it comes from; put it on the
    first chunk of those statements, and let the later chunks refer to it.
    Two to twelve steps; each caption says what happens and why it matters.
    `code` is a faithful excerpt (at most 30 lines), copied from the quoted
    lines or from that program — drop lines with a
    `// …` comment rather than rewriting them, and say in `source` which
    file and lines it shows; a later step changes `code` only when the point is
    that the code changes, and otherwise moves `highlight`. Set `start_line`
    to the file line the code starts at, and number `highlight` the same way,
    so the reader sees the diff's numbers. Keep lines within 100 columns,
    replacing an overlong comment with `// …` rather than rewording it. `state` shows the values a reader would track; use
    real or clearly illustrative values, never invented results.
  - `timeline`: when the point is work moved out of a loop — built once
    instead of every step, pooled instead of reallocated. One lane per
    version, `when` is `once` or `each`, and `cost` (optional) is per run in
    `unit`. Give costs only when the code or a measurement supports them;
    otherwise leave them out and let the lanes show the shape. `once` means
    the work itself happens once: a buffer allocated once but refilled every
    step is two items, the allocation `once` and the refill `each`.
  - Like a context pane, an explainer is measured beside the chunk, not
    charged to its row budget, so put it in the chunk whose code it
    explains, next to the quote. Keep each one smaller than the budget;
    the measurement says when it is not.
  - An explainer supports the narrative and does not replace it: the prose
    still says what to look for in the code. Use as many as teach something
    distinct — a cost model belongs beside the layout it builds on — and none
    in a chunk that only moves code around. Their rows together stay under
    the chunk budget; the measurement reports them.
- Put code in a context `example` as code text, without an outer Markdown fence;
  Gewu supplies the visual code boundary. Keep explanation and headings in the
  `definition` or `relation` fields instead of embedding them in the example.
- Write math as TeX between dollar signs, which Gewu typesets: `$G^\dagger P G$`
  inline, `$$…$$` on its own lines for a display equation. Keep a code span for
  code, such as an identifier or an expression as the source spells it; use TeX
  when the sentence states the mathematics, such as a conjugation, a sum, or a
  bound like $O(n^2)$. A literal dollar sign is `\$`.
- Put uncertain interpretation in `confidence` — what the repository leaves
  open, not a risk you already stated; never invent motivation. Do not write
  that you read rather than ran something: the reviewer assumes the walkthrough
  comes from reading. Say "untested" only when no test covers the behaviour,
  because that is a fact about the code.
- The reviewer has read the overview. A chunk sentence that repeats one of its
  bullets, or the finding behind this item, teaches nothing; say what this
  chunk's lines add.
- Say where the chunk fits. Its first sentence tells a reviewer who has not
  seen the code what part of the change this is and how it connects to the
  previous chunk or the overview's map — "The runner acts on the verdict that
  `next_step` returns", not "Next, the runner". The section name above the
  chunk says which part of the walkthrough it is in; the sentence says why it
  is read now. The first chunk of an item opens a new section: say what this
  part of the change does that the previous part made necessary or possible.
- The walkthrough's very first chunk says where on the overview's map it
  begins, and why, when that is not the map's first step.
- A risk the overview already states is not stated again. Show the lines it
  lives in, point to it in a clause, and ask only what these lines add.
- A risk you found by reading the code belongs in the narrative of the chunk
  that shows that code: once, as a
  question the reviewer can check against the lines in front of them, never as
  a verdict. A later chunk that touches it names that chunk by its title, in
  quotation marks, instead of raising it again; titles are what the reviewer
  sees, and unquoted they read as a broken sentence.
- Write each narrative block as a paragraph, not a list of facts; the reviewer
  reads it as an argument, so give them its shape. These rules come from
  scientific-writing guides (Gopen and Swan's "The Science of Scientific
  Writing", Williams's *Style*), which describe how readers find their way
  through a dense paragraph:
  - One point per paragraph, stated in its first sentence; the sentences after
    it give the lines, callers, tests, or commits that support it. A reader who
    reads only first sentences should still follow the chunk. A paragraph whose
    point you cannot name is an inventory: ask what its facts add up to — what
    they let a caller do, what they cost, what they promise, what they leave out
    — and open with that ("The crate root exports more than its doc comment promises:
    a second trait and three types it needs"). Facts that add up to nothing the
    view does not already show are cut, not joined.
  - Begin each sentence with what the reader already has and end it with the
    new thing you want them to take away. Inside a paragraph, what they have is
    the symbol the last sentence ended on; at a paragraph's start, it is the
    point the previous paragraph made or the view they just read. The next
    sentence then starts where this one stopped, and the paragraph reads as one
    line of thought instead of a fresh start every time.
  - Keep one subject running through a paragraph: the type or function it is
    about, as the subject of verbs that say what it does ("`InverseTableau`
    evaluates the rule on the gate's generators"), rather than a new noun, or a
    noun made from a verb ("the evaluation of"), in each sentence.
  - Join facts that depend on each other into one sentence, with the word that
    states how: because, so, which, so that, unless, instead of. Use a
    connective only for a relation the code shows; a "however" with no contrast
    or a "therefore" with no consequence is worse than none, and "and" or
    "while" between two unrelated facts only hides that they are unrelated.
  - Vary the length. Three short sentences in a row that each start a new
    subject read as an inventory, and the reader has to work out how they
    relate; write the sentence that relates them. A sentence carrying more than
    two clauses of new facts splits.

  Not: "`Cache::get` takes a key. It hashes the key. The hash picks a shard.
  Each shard has its own lock. This avoids contention." But: "`Cache::get`
  hashes the key to pick one of sixteen shards, each behind its own lock, so
  two lookups wait for each other only when their keys land in the same
  shard."

  An explainer or the concept card beside a paragraph carries the detail the
  reader looks up; the paragraph says what to notice in it, in a clause, and
  goes on with the argument.
- `context`, `confidence`, and `oversized_reason` are optional chunk-level
  fields. Omit them when they do not add information. When `context` is present,
  every field of its selected shape is required: a concept has `definition`,
  `example`, and `relation`; a refactor has `before`, `after`, and `reason`.

Before you submit, answer three questions for every chunk, from its prose
alone. A proofreader asks exactly these, and most rewrites are for one of them:

1. **What came before?** Name what the base did here — the function, map,
   call, or loop these lines replace — or say plainly that nothing preceded
   them. A chunk about a new file still replaces something.
2. **Why?** Point at the sentence that says why the change was made and what
   it buys. A chunk that relies on an earlier chunk's reason restates it in a
   clause and names that chunk by its title.
3. **Why now?** The first sentence says how this chunk follows the one
   before it. One that opens with "Lines 85–92 …" or "In `file.rs`, …" has
   said where, not why now; `gewu harness status` lists those.

Then make one pass over every sentence. Delete it if the reviewer would lose
nothing — if it restates the quoted lines, their doc comment, the title, the
summary, the overview, or a sentence before it. Keep sentences that add a fact,
ask a question, or say where the chunk fits. A judge applies exactly this test,
and a first draft typically loses a sixth of its sentences to it. Cutting is
not the only fix: where two short sentences state facts that depend on each
other, join them into the one sentence that says how, so the paragraph keeps
its line of thought as it gets shorter.

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
