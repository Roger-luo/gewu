# Gewu harness: proofread {{count}} chunk(s)

You did not write these chunks. Read them as the reviewer will — someone who
knows the codebase but not this change — and score each one before the
reviewer sees it. A chunk that scores below 2 on any dimension goes back to be
rewritten, with your problems as the brief. {{round}}

{{chunks}}

Check every claim against the code, not against the prose. The quotes name
change ids and body lines in `.gewu/review-input.json`; `gewu harness context
source --change-id <id> --start 1 --end <body_lines> --numbered` prints them.
Removed lines in a change's patch are the code as it was; `gewu harness
context base --path <path>` prints the whole file at the base revision.

Score each chunk from 0 to 3 on every dimension:

- `grounded` — every sentence is true of the lines the chunk shows. 3: each
  claim can be checked against a quoted line. 1: a claim the code contradicts,
  or a motive the repository does not show. 0: the prose describes different
  code.
- `change_framing` — the reviewer can tell what changed, not only what the
  code now is. 3: the chunk says what was there before and what replaced it.
  1: it describes the new code as if nothing preceded it.
- `rationale` — the chunk says why the change was made and what it buys,
  explicitly, in a sentence the reviewer can point at. 3: the advantage and
  its motive are stated and tied to lines, a caller, a test, or a document.
  2: the advantage is stated, the motive is left as an honest open question.
  1: the reviewer must infer why. 0: no reason at all.
- `representation_contrast` — when the chunk presents a new form of a
  structure that already existed (a struct, enum, encoding, stored word,
  codec), the earlier definition is on the page beside the new one: in a diff
  quote or a refactor `context` whose `before` is the old definition with its
  fields. The prose names the fields dropped, packed, or added, and says which
  difference earns the property the title claims. A type alias or re-export
  that keeps the old name is not the old definition: score 1 if that is all
  the chunk shows. 0: the old shape is absent. Score 3 when the chunk
  introduces no new form of an existing structure.
- `showing` — when the chunk's point is something a reader would otherwise
  simulate or picture — a loop that transforms data (an elimination, a sift,
  a rewrite pass), a sequence of operations on a structure (a lifecycle, a
  protocol), a record's shape, or a cost that grows with a size — an
  explainer (`stepper`, `layout`, `cost_model`, `timeline`) shows it. 3: an
  explainer shows it, or the chunk has no such point. 1: the prose walks
  through the steps or the states and nothing draws them; the problem names
  the explainer and the example it should step through: one from a test, from
  `.gewu/description.md`, or small illustrative values. 0: the chunk asks the
  reader to trace state across several sentences with no example at all.
  Read the batch as a whole too: when several of its chunks each add one
  operation on the same structure (allocate, lend, return, take, free) and
  no chunk draws them acting in turn, score the first of them 1 and name the
  sequence its stepper should step through.
- `minimality` — every sentence adds something: a fact, a question, or where
  the chunk fits. 1: sentences restate the title, the summary, the overview,
  or the quoted lines; narrate line ranges ("Lines 20–34 implement …") where
  the view already shows them; list every caller, importer, or re-export when
  one caller would say when the code is used; or open with "nothing existed
  before" for a file the overview already called new. Name the sentences to
  cut.
- `connection` — the first sentence says what part of the change this is and
  how it follows from the previous chunk or the overview's map. 1: the chunk
  could be read in any order.

Write `.gewu/harness/proofread.json`:

```json
{
  "schema_version": 1,
  "source": { "base": "…", "head": "…", "diff_sha256": "…" },
  "chunks": [
    {
      "id": "chunk-id",
      "scores": {
        "grounded": 3,
        "change_framing": 2,
        "rationale": 1,
        "representation_contrast": 1,
        "showing": 3,
        "minimality": 3,
        "connection": 2
      },
      "problems": [
        "The quote shows `type Word = u64` but not the old `PauliWord` struct; its `phase` and `support` fields, now dropped, are never named.",
        "The title says the encoding is more compact; no sentence says why the change was wanted or what it saves."
      ]
    }
  ]
}
```

`source` is the `source` object of `.gewu/review-input.json`, copied field for
field. Score every chunk listed above and no other. Every score below 2 needs
at least one problem, written as an instruction a rewrite can act on: name
the missing definition, field, sentence, or claim. Do not rewrite the chunks
yourself, and do not score generously: a 2 means a reviewer would not have to
guess.

When working directly in the CLI, run `gewu harness next`. When a driver asked
for the JSON as your response, return only the checkpoint.
