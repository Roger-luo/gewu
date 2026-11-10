# Judge a Gewu walkthrough (rubric version 4)

You are an independent reviewer of a generated code-review walkthrough. You did
not write it and have not seen the instructions that produced it. Do not read
`harness/`, the author's checkpoints under `.gewu/harness/`, or any friction
log. Judge only what a reader would see against what the repository shows.

## Inputs, all in the frozen copy

- `.gewu/review-spec.json`: the walkthrough. Quotes name 1-based body lines of
  changes in the manifest.
- `.gewu/review-input.json`: the manifest. Each change's `patch` holds the lines
  the quotes refer to.
- `.gewu/self-review/sentences.json`: every sentence a reader sees, numbered.
- `.gewu/self-review/audit.json`: the mechanical audit. Its forward and distant
  references are leads for you to weigh, not verdicts.
- The repository files themselves, for consumers and unchanged code.

## Label every sentence

Label each id in `sentences.json` exactly once. The score rejects a judgement
that skips, repeats, or invents an id.

| Label | Use when |
| --- | --- |
| `fact` | it states something new that the quoted code or its consumers establish |
| `decision` | it puts a question, risk, or tradeoff in front of the reviewer |
| `anchor` | it points back to earlier material the reader needs at this point |
| `orientation` | it tells the reader where this chunk sits in the change: which part of the overview's map, why it comes now, or how it follows from the chunk before |
| `redundant` | an earlier sentence, the title, or the quoted code already says it |
| `filler` | it conveys nothing: an empty transition, an announcement, praise |
| `narrates-review` | it describes how the walkthrough was made — what the author read, ran, or chose — rather than the code |
| `unsupported` | the repository does not establish it, or contradicts it |
| `verdict` | it concludes for the reviewer: correct, safe, fine, looks good |

Test each sentence by deleting it. If the reader loses nothing, it is
`redundant` or `filler`. Prefer `unsupported` over `fact` whenever you could not
check the claim; check claims about callers, conditions, and failure paths
against the code, not against the prose's confidence. A re-shown definition is
fine; a definition explained again is `redundant`.

Connective sentences are not padding when they carry structure. "The caller
of this check is in the runner, next" is `orientation`: it tells a newcomer
how two pieces relate. "Let us now look at the next part" is `filler`. Judge
them by whether a reader who had never seen the change would lose their way
without them.

Every label other than `fact`, `decision`, `anchor`, and `orientation` needs a `reason` naming
what makes it so: the earlier sentence it repeats, the line that contradicts it.

## Score seven dimensions, 0–3

0 absent or wrong, 1 material problems, 2 usable with small gaps, 3 clear and
supported. Each needs `evidence`: chunk ids and what in them earned the score.

- `reference_usefulness`: definitions are in view where a sentence depends on
  them. For each distant reference in the audit, say whether re-showing was
  needed.
- `decision_surfacing`: each chunk gives the reviewer something concrete to judge.
- `change_framing`: for a diff, what changed and why, not only what the code does.
  For a review from an empty base, score whether intent and design are explained.
- `learning_order`: prerequisites before consumers; cross-file relationships named.
- `big_picture`: before the first chunk, a reader learns what the change is
  for, which parts it touches, and how those parts connect — in a map, a flow,
  or a diagram where the change spans several parts. Score 1 if the overview
  only lists changes.
- `chunk_connection`: each chunk makes clear what it adds to that picture and
  how it relates to the chunks around it. Score 1 if chunks read as isolated
  notes the reader has to connect alone.
- `representation_contrast`: when a chunk's point is a new form of a structure
  the repository already had — a data type, an encoding, a stored word, a
  codec — the prose says what the earlier definition stored, what the new one
  stores, and which difference earns a property the chunk claims, such as
  being more compact. The earlier definition is on the page: in a diff quote
  that includes the removal, or in a refactor context pane (`before`, `after`,
  `reason`). Score 1 when the title names the property and the prose only
  restates the new fields. Score 3, and say so, when no chunk replaces or
  redefines a structure. Check the claimed difference against the two
  definitions; a motive the repository does not show is `unsupported` on the
  sentence and scores at most 1 here.

You read the source and the spec, so you know the picture before you start;
the reviewer does not. Score `big_picture`, `chunk_connection`, and
`representation_contrast` as the reviewer would meet them.

## Write questions for a cold reader

A separate reader will see only the rendered pages, with no source and no
spec, and answer your questions. Their score measures whether the walkthrough
teaches the change. Write exactly six multiple-choice questions:

- two about what the change is for and what it alters overall;
- two about how its parts connect: which piece calls, feeds, or depends on
  which, or why one chunk comes before another;
- two about a consequential behaviour a reviewer must understand to judge it.
  When a chunk replaces a representation, one of these asks what the previous
  definition stored, or which difference makes the new one smaller. Take the
  correct answer from the two definitions. A walkthrough that never teaches
  it should make the cold reader miss.

Each has four options, one correct and three plausible to someone who skimmed,
and a `kind` of `purpose`, `connection`, or `behaviour`.
Base the correct answer on the code, not on the walkthrough: if the walkthrough
never teaches it, the reader should miss it, and that is what the score is for.

## Report what else is wrong

`findings` holds problems no sentence label captures: an omitted behavior, a
cross-file relationship never made, a chunk boundary in the middle of a
declaration, a `summary_only` view hiding handwritten code.

## Output

Write `.gewu/self-review/judge.json` and nothing else:

```json
{
  "rubric_version": 4,
  "labels": [{"id": "s1", "label": "fact"}, {"id": "s2", "label": "redundant", "reason": "repeats s1"}],
  "dimensions": {
    "reference_usefulness": {"score": 2, "evidence": ["chunk-x: ..."]},
    "decision_surfacing": {"score": 3, "evidence": ["..."]},
    "change_framing": {"score": 2, "evidence": ["..."]},
    "learning_order": {"score": 3, "evidence": ["..."]},
    "big_picture": {"score": 2, "evidence": ["overview: ..."]},
    "chunk_connection": {"score": 2, "evidence": ["chunk-x: ..."]},
    "representation_contrast": {"score": 1, "evidence": ["chunk-y: title claims a compact word; prose restates the new struct; the old definition is not on the page"]}
  },
  "questions": [
    {"id": "q1", "kind": "purpose", "question": "...", "options": ["...", "...", "...", "..."], "answer": 2}
  ],
  "findings": [{"chunk": "chunk-y", "category": "omission", "detail": "..."}]
}
```
