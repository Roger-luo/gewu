# Can Jev clear chunks before the LLM proofreader?

Proofreading adds about 75% to each agent-written batch. The proposal: Jev
answers one yes/no question per proofread dimension first, and chunks it is
confident pass skip the LLM proofreader. Jev never fails a chunk on its own:
a rewrite needs the proofreader's problem list, which Jev cannot write.

Everything below was fixed before any Jev answer or label was seen.

## Data

- Chunks from the ten Cursor walkthroughs of ppvm #276 (tasks 0–9 under
  `/tmp/gewu-cursor-home`), excluding `test` chunks (Gewu writes those and
  never proofreads them).
- **Fit split**: 60 chunks sampled from tasks 0, 1, 2, 6, 9. **Held out**: 60
  sampled from tasks 3, 4, 5, 7, 8. Seed 20261111.
- **Reference labels**: blind subagents proofread each chunk with
  `harness/prompts/proofread.md`, unchanged, in the task's checkout. A chunk
  *fails* when any dimension scores below 2, as in the harness.
- **Ceiling**: 15 held-out chunks are proofread a second time by another
  agent, to measure how often two LLM proofreaders agree on pass/fail.
- **Jev**: `gewu review load --jev --with-state` with a rubric whose six
  questions ask, per dimension, "would this score below 2?" (`true` = a
  problem). The state is the chunk plus the earlier chunks' titles, as for
  the load score. Jev sees no source beyond the quotes.

## Rule

A chunk is *cleared* when every question's answer is below τ. τ is chosen
on the fit split: the largest τ for which at most 5% of cleared chunks
failed the proofreader. That τ is then applied unchanged to the held-out
split.

## Bars (held out)

1. **Misses**: of the chunks Jev clears, at most 10% failed the proofreader,
   and the Wilson 95% upper bound is at most 20%.
2. **Savings**: Jev clears at least 25% of chunks.
3. **Per dimension** (reported, not gating): AUC of Jev's answer against
   "scored below 2", where both labels have at least 5 chunks.

If bars 1 and 2 pass, the tier goes into the harness as opt-in. If not, it
does not, and this file records why.

## Results (2026-11-11)

Labels: Claude subagents with `proofread.md` unchanged (the app's proofreader
is Cursor's model, so absolute fail rates may differ). Jev cost under $0.05.

```text
| dimension | below 2 | AUC |
|---|---|---|
| grounded | 13/120 | 0.596 |
| change_framing | 47/120 | 0.837 |
| rationale | 57/120 | 0.920 |
| representation_contrast | 6/120 | 0.753 |
| minimality | 21/120 | 0.771 |
| connection | 61/120 | 0.563 |
| any (max of answers) | 98/120 | 0.360 |
| (reading time, for comparison) | | 0.491 |

Two LLM proofreaders agree on pass/fail for 14/15 chunks
  grounded: 12/15
  change_framing: 9/15
  rationale: 14/15
  representation_contrast: 15/15
  minimality: 15/15
  connection: 13/15

## Pre-registered rule: all six answers below τ
fail rate: fit 53/60, held out 45/60
no τ clears any fit chunk at ≤5% misses: bars fail

## Exploratory: the five without grounded (not pre-registered)
fail rate: fit 53/60, held out 45/60
τ = 0.61 (fit: clears 1/60)
held out: clears 1/60 (2%); 1 of those failed (100%, Wilson upper 100%)
bar 1 misses: FAIL; bar 2 savings: FAIL
```

**Verdict: the bars fail; the tier is not built.** The failure is structural:
the proofreader fails 98 of 120 chunks (82%; 53/60 fit, 45/60 held out), so
a screen that only clears passing chunks has 7 fit and 15 held-out chunks to
find. Even a perfect screen would barely meet the 25% savings bar on held
out, and on the fit split no τ cleared a single chunk at ≤5% misses.

What was learned:

- Jev tracks the proofreader closely on the two most common failures:
  `rationale` (AUC 0.92, 57 failures) and `change_framing` (0.84, 47). Two
  LLM proofreaders agree with each other on 14/15 and 9/15 chunks there, so
  Jev is near the labels' own consistency on `rationale`.
- It does not track `connection` (0.56, the most common failure) or
  `grounded` (0.60, as expected: it needs the source).
- The max over questions ranks worse than chance (0.36): the uninformative
  questions dominate it. Any later rule should use the informative
  questions only.
- Reading time does not predict proofread failure (0.49), unlike
  overload.

The data suggests a different use, not tested here: Jev as a cheap check on
the *writer's* draft for `rationale` and `change_framing`, before
proofreading, so that fewer chunks fail and fewer rewrite rounds run.
Testing that would need its own pre-registration.

## Since

The expand prompt now ends with three questions (what came before, why, why
now) asked of every chunk before submitting, and `gewu harness status` lists
chunks whose first sentence only points at lines. On these 120 chunks that
check flags 36, of which 29 scored below 2 on `connection`, against 32 of the
other 84. Whether either change lowers the fail rate is for the next app
runs to show: `.gewu/harness/proofreads.jsonl` now keeps every verdict.

### Did it work? (run 10, 2026-11-11)

30 of run 10's 97 non-test chunks, proofread blind by Claude subagents with
the same instructions, against the 120 chunks above:

| below 2 on | run 10 (30) | before (120) |
|---|---|---|
| any dimension | 25 (83%, 95% CI 66–93%) | 98 (82%) |
| `connection` | 2 (7%) | 61 (51%) |
| `rationale` | 11 (37%) | 57 (48%) |
| `change_framing` | 14 (47%) | 47 (39%) |
| `minimality` | 9 (30%) | 21 (18%) |
| `grounded` | 3 (10%) | 13 (11%) |

The "why now" question and the status check fixed `connection`; no chunk
opened with a line range. "What came before" and "why" did not move
`change_framing` or `rationale` beyond noise, so the overall fail rate is
unchanged. The two samples are different runs of the same PR.

Cursor's own proofreader, in the same run, passed all 103 chunks it read:
601 of its 618 scores were 3 and it listed no problems, so proofreading cost
28 of the run's 112 minutes and rewrote nothing.
