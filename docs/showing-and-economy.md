# Do writers show algorithms, and stop narrating the page?

Reading the app's walkthroughs of three pull requests (2026-11-11) found:

- **No explainers where they teach most.** ppvm #233 (four stabilizer
  canonicalizations) had 9 chunks, none with an explainer, though its core is
  Gaussian elimination on a tableau and its description gives a 3-qubit GHZ
  example to step through. bloqade-circuit #901 (a wire dialect) spread one
  register lifecycle — allocate, borrow, release, take, put, discard — over
  six chunks with no picture of it. bloqade-lanes #1131 had a claim and two
  steppers in 12 chunks.
- **Wordiness the rubric allowed.** Sentences narrating line ranges ("Lines
  20–34 implement `multiply` …"), lists of every caller and re-export, and
  "nothing existed before" openers for files the overview called new.
  `minimality` scored those as facts.
- **No description for commit ranges**, so no claim could be quoted.
- **A proofreader that passes everything.** Cursor proofreading its own
  batches failed 1 of 43 chunks across the three, scoring 3 on 249 of 259
  dimensions.

## Changes

- `proofread.md` gains `showing` (a process, shape, or scale the reader would
  simulate is drawn by an explainer) and a wider `minimality` (narrated line
  ranges, caller lists, repeated "new file" openers score 1).
- `expand.md` asks for a stepper over a concrete example for eliminations,
  rewrite passes, and lifecycles, whatever the change is for; lets a
  lifecycle's stepper step through a short program from a test or the
  description; and asks the writer not to narrate the view or list callers.
  `rubric.md` and the stepper reminder say the same.
- A commit range's description is its commit messages.
- Settings choose the proofreading agent; the writer is the default.

## Test (fixed before any result was read)

Cursor writes #233 in full and the first six sections of #901 and #1131 with
the new prompts, proofreading off, so the first draft is measured. The same
PRs' chunks from the app's earlier runs (old prompts, same model) are the
baseline, cut to the same sections. Claude subagents score every chunk with
the new `proofread.md`, unchanged, in batches that mix both conditions under
neutral ids, so a scorer cannot tell which prompt wrote a chunk.

Bars, on the share of chunks scoring below 2 (excluding `test` chunks):

1. `showing` falls by at least half.
2. `minimality` falls by at least 15 points.
3. No other dimension rises by more than 10 points.
4. #233's elimination chunks (`forward_pass`, `canonicalize`,
   `canonicalize_rref`) carry at least one stepper.

Prose words per chunk are reported, not gated.

## Results: first drafts (2026-11-12)

Cursor could not be used unattended, so both conditions were written by
Claude Haiku 5.5 through Claude Code's ACP adapter, old prompts from commit
5544f9d and new from this change, with the same agent fixes in both. The
baseline from the app's Cursor runs was dropped: it differed in model. 97
chunks (#233 to its end, #901 to its eighth section), scored blind by three
Claude Sonnet subagents, which read the code lightly (10–20 tool calls for
about 33 chunks each), so `grounded` means little here.

| below 2 on | old | new |
|---|---|---|
| grounded | 1/54 (2%, 0–10) | 2/43 (5%, 1–15) |
| change_framing | 39/54 (72%, 59–82) | 26/43 (60%, 46–74) |
| rationale | 19/54 (35%, 24–49) | 20/43 (47%, 33–61) |
| representation_contrast | 0/54 (0%, 0–7) | 2/43 (5%, 1–15) |
| showing | 7/54 (13%, 6–24) | 7/43 (16%, 8–30) |
| minimality | 9/54 (17%, 9–29) | 7/43 (16%, 8–30) |
| connection | 10/54 (19%, 10–31) | 5/43 (12%, 5–24) |
| any | 44/54 (81%, 69–90) | 34/43 (79%, 65–89) |

Bars 1–3 fail: `showing` 13% → 16%, `minimality` 17% → 16%, `rationale`
35% → 47% (intervals overlap throughout). Bar 4 passes: `canonicalize`
carries a stepper over the description's GHZ example, the rows after each
pass. #901's new run drew the register lifecycle the old one lacked, two
steppers whose state is the register's slots (`[q0, ·, q2, q3]`) and which
values are live; the old run's six explainers were all `claim`s. Prose
length did not change (84 → 92 and 63 → 65 words per chunk).

The writer drew steppers on 2 of #233's eight or so loop chunks. The scorer
failed the other six on `showing`, each with a brief a rewrite can act on
("a stepper on a small permutation, e.g. [2,0,1] on 3 qubits"). It failed
no #901 chunk, though the old run never drew the register lifecycle: one
lifecycle spread over several chunks is not something a chunk-at-a-time
reader misses. So the new dimension finds what the writer misses; whether a rewrite
acts on it is the next test.

## Test 2: with proofreading (fixed before running)

#233 again, old and new prompts, Haiku writing and proofreading, two
proofread rounds as the app's default; scored blind as above, the earlier
scores not reused. Bars: `showing` below 2 at most half the old rate; no
other dimension up by more than 10 points.

## Results: with proofreading (2026-11-12)

30 chunks (#233 to its eleventh section), scored blind by one Claude Sonnet
subagent:

| below 2 on | old | new |
|---|---|---|
| grounded | 1/15 (7%, 1–30) | 2/15 (13%, 4–38) |
| change_framing | 0/15 (0%, -0–20) | 0/15 (0%, -0–20) |
| rationale | 3/15 (20%, 7–45) | 2/15 (13%, 4–38) |
| representation_contrast | 0/15 (0%, -0–20) | 0/15 (0%, -0–20) |
| showing | 6/15 (40%, 20–64) | 4/15 (27%, 11–52) |
| minimality | 3/15 (20%, 7–45) | 0/15 (0%, -0–20) |
| connection | 0/15 (0%, -0–20) | 0/15 (0%, -0–20) |
| any | 8/15 (53%, 30–75) | 5/15 (33%, 15–58) |

The `showing` bar fails, narrowly: 40% → 27%, against at most 20%. No
other dimension rose by more than 10 points. Reported: `minimality`
failures 3 → 0, prose 123 → 93 words per chunk (−25%), and steppers 1 → 3.
The pipeline worked as designed where the proofreader looked: the in-run
proofreader (Haiku) failed the two clipped-gauge passes on `showing` with a
brief to step through the GHZ example, and the rewrite drew both. It passed
four other loops (`forward_pass`, the permutation, the bottom-push
elimination, the Gottesman rounds) that the blind scorer failed.

So `gewu harness status` now also lists chunks that quote an added `for`,
`while`, or `loop` and carry no explainer, as it lists line-locator
openings: the writer sees them whatever the proofreader's standards.

## Test 3: with the loop check (fixed before running)

#233 once more with this change, proofreading as in test 2. Its chunks are
scored blind beside test 2's old-prompt chunks, scored again with them.
Bar: `showing` below 2 at most half the old rate.

## Results: with the loop check (2026-11-12)

32 chunks, test 3's beside test 2's old-prompt chunks, scored blind and
afresh by one Claude Sonnet subagent:

| below 2 on | old | new |
|---|---|---|
| grounded | 0/15 (0%, -0–20) | 0/17 (0%, 0–18) |
| change_framing | 7/15 (47%, 25–70) | 11/17 (65%, 41–83) |
| rationale | 10/15 (67%, 42–85) | 11/17 (65%, 41–83) |
| representation_contrast | 0/15 (0%, -0–20) | 0/17 (0%, 0–18) |
| showing | 7/15 (47%, 25–70) | 2/17 (12%, 3–34) |
| minimality | 14/15 (93%, 70–99) | 11/17 (65%, 41–83) |
| connection | 2/15 (13%, 4–38) | 2/17 (12%, 3–34) |
| any | 15/15 (100%, 80–100) | 14/17 (82%, 59–94) |

The bar passes: `showing` 47% → 12%, against at most 23%. Steppers 1 → 6,
prose 123 → 80 words per chunk. The in-run proofreader failed 6 of 11
verdicts, against 2 in test 2, so the loop check sent more batches back.

Not gated, and not to be over-read: this scorer was far stricter than test
2's on the same old chunks (`minimality` 93% against 20%, `rationale` 67%
against 20%), so scores move as much between scorers as between prompts,
and `change_framing` rose 47% → 65% within intervals that overlap. One PR,
one model, about 15 chunks a side: the direction on `showing` held in
every test with proofreading, the size is uncertain.

## One sequence over several chunks

What test 1 left open: #901's old run told a register's lifecycle one
statement per chunk, and no chunk-at-a-time reader missed a picture. Two
changes: the proofreader, who reads a batch, scores the first of several
chunks that each add an operation on one structure 1 on `showing` when none
draws them in turn; and `gewu harness status` notes a batch of three or more
non-test chunks with no explainer at all.

### Test 4 (fixed before running)

#901 and #1131 with everything above, Haiku writing and proofreading, two
rounds, to the same eight and six sections as before. Bars: #901's state
section carries a stepper whose `state` is a register's slots or its live
values; and, scored blind, at most 20% of each PR's non-test chunks fall
below 2 on `showing`.

### Results (2026-11-12)

Both bars pass. #901's state section carries a stepper on borrowing whose
`state` is the register's slots (`[q0, ·, q2]`) and the lent wire, beside
three more steppers in the walkthrough and three in #1131's (localizing
blocks, expanding a slot, resolving and stamping blocks). Scored blind by a
Claude Sonnet subagent, 1 of #901's 31 chunks and 0 of #1131's 22 fall below
2 on `showing`; the one is the register-composition chunk (split, join, take,
put), with a brief for its stepper. The same scorer failed 9 of #901's
chunks on `rationale` and 6 on `minimality`, and 3 and 7 of #1131's: those
remain the most common failures.

## Line numbers in prose

Five of #1131's seven `minimality` failures in test 4 narrated line numbers
the gutter shows ("lines 140–156 …"). `gewu harness status` now names chunks
whose prose cites more than one.

### Test 5 (fixed before running)

#1131 again, as in test 4, scored blind. Bar: `minimality` below 2 on at
most half test 4's rate (7/22, so at most 16%), with `showing` still at most
20%.

### Results (2026-11-12)

The `minimality` bar fails: 7/22 (32%) → 5/23 (22%), against at most 16%.
`showing` holds at 0/23. The check did what it targets: #1131's prose cited
line numbers 24 times in 7 chunks in test 4 and not once in test 5, and
none of the five remaining failures is about line numbers. They are
openings that restate the summary (three) and wiring told in one narrative
per one-line hunk (one), plus one chunk narrating its code. `connection`
failures rose from 0 to 8 of 23, and `rationale` stayed the most common
failure (10 of 23), as in every test here and in docs/proofread-prescreen.md.
Neither was among the problems this study set out to fix; both are open.
