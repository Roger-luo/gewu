# Cognitive-load score: calibration bars

Written 2026-11-11, before any labelled chunk was scored. The bars and the
protocol below are fixed. They are not moved after the results are seen, and
any failure is reported as a failure. Results go in the "Results" section at
the end; nothing above it changes.

## Question

Does `gewu review load --jev` (criteria v2: TypeSafe Jev's answers to the
[`CRITERIA`](../crates/gewu-review/src/load/criteria.rs) questions, combined
with reading time by the equation in
[`load/mod.rs`](../crates/gewu-review/src/load/mod.rs)) tell overloaded chunks
from the rest, well enough to report it to a reviewer? And does Jev add
anything over reading time alone?

## Data

- Walkthroughs: the six Cursor runs of ppvm PR #276 under
  `/tmp/gewu-cursor-home/tasks/` with indices 0, 1, 2, 4, 5, 8 (380 chunks).
  This is the reviewer's own repository; it is sent to TypeSafe only with
  `--jev`, and the chunk text is not committed to this repository.
- Sample: 80 chunks, drawn by `scripts/load_calibration.py build` with seed
  20261111, stratified by run in proportion to its chunk count (at least 6 per
  run), and within a run by kind (test and other).
- Split, by run so the held-out runs are unseen: **fit** = runs 0, 1, 2;
  **held out** = runs 4, 5, 8.

## Labels

- One labeller per batch: a fresh Claude agent (Claude Code's default model),
  with the instructions in `scripts/load_calibration_label.md`. It reads each
  chunk's state exactly as Jev reads it (`--with-state`) and nothing else: not
  Jev's answers, not reading time, not the QA reports' verdicts.
- Each item gets `overloaded` (yes/no: would a careful reviewer new to the
  change have to re-read the chunk or hold more in mind than they can at once
  to judge it), and a yes/no for every question that applies to it, with a
  one-line reason.
- The labeller is a reference, not ground truth. Label noise is not measured
  (no item is labelled twice); this is a limitation, stated with the results.

## Models compared

- **M0, reading time alone:** `P = 1` when `w_t·max(0, T/T_budget − 1) ≥ θ`,
  with the score `T` for ranking (AUC).
- **M1, criteria v2 as shipped:** the equation with its built-in weights,
  `θ = 1`, `T_budget = 180 s`, `w_t = 0.5`. Not fitted; reported as is.
- **M2, fitted:** logistic regression on the fit split of the label
  `overloaded` on each applicable question's `p` (0 where it does not apply)
  and `max(0, T/T_budget − 1)`, L2 penalty 1.0, by gradient descent.
  Weights follow from the fit: with intercept `β₀ < 0`, `w_k = max(0, β_k) / −β₀`
  and `θ = 1`, so `L ≥ 1` is the fitted decision boundary. A negative
  coefficient drops the question (weight 0). M2 is evaluated through the same
  equation (`P(L ≥ θ)` by enumeration), not through the logistic curve.

## Bars (held-out split unless stated)

| # | Bar | Pass when |
|---|---|---|
| 1 | Each question tracks its own label (whole sample) | AUC of `p` against the labeller's yes/no ≥ 0.65, for every question with ≥ 5 yes and ≥ 5 no labels. A question that fails is reported for rewording. |
| 2 | M2 ranks overloaded chunks | AUC ≥ 0.75 |
| 3 | M2 decides | accuracy at `P ≥ 0.5` ≥ 75%, and Brier ≤ 0.20 |
| 4 | Jev earns its egress | AUC(M2) ≥ AUC(M0) + 0.05 |
| 5 | Spend | Jev ≤ $0.10 for the whole protocol |

The score may gate generation only if bars 2–4 pass. Otherwise it stays
advisory, and what failed decides the next change: rewording (bar 1), more
signals (bar 2–3), or dropping Jev for reading time alone (bar 4).

## Results

Run 2026-11-11 with criteria v2 (`gewu review load --jev --with-state`) and
`scripts/load_calibration.py` as committed with this section. 380 chunks were
scored, with no Jev failures, for about $0.03 (estimated from state length;
$0.04 including the two v1 runs before the bars were written). Four labellers
labelled 20 items each, 80 in all.

Output of `load_calibration.py eval`, unedited:

Labelled items: 80, 18 overloaded.

### Bar 1: each question against its own label (whole sample)

| question | n | yes | AUC | pass |
|---|---|---|---|---|
| distant_recall | 80 | 17 | 0.61 | **FAIL** |
| many_new_names | 80 | 26 | 0.93 | pass |
| several_points | 80 | 31 | 0.93 | pass |
| unframed_change | 60 | 12 | 0.89 | pass |
| disconnected | 60 | 3 | 0.69 | n/a (too few of one label) |
| missing_contrast | 60 | 0 | – | n/a (too few of one label) |
| dense_code | 80 | 6 | 0.97 | pass |
| padded | 60 | 20 | 0.72 | pass |

### Fit (runs [0, 1, 2], 52 items)

β₀ = -2.750; distant_recall -0.193, many_new_names 0.777, several_points 0.479, unframed_change -0.408, disconnected 0.247, missing_contrast 0.524, dense_code 0.228, padded 1.211, time 0.686

Fitted weights (θ = 1): distant_recall 0.00, many_new_names 0.28, several_points 0.17, unframed_change 0.00, disconnected 0.09, missing_contrast 0.19, dense_code 0.08, padded 0.44, time 0.25

### Held out (runs 4, 5, 8; 28 items, 9 overloaded)

| model | AUC | accuracy at 0.5 | Brier |
|---|---|---|---|
| M0 reading time | 0.96 | – | – |
| M1 shipped v2 | 0.90 | 0.46 | 0.37 |
| M2 fitted | 0.75 | 0.75 | 0.17 |

| # | bar | result | pass |
|---|---|---|---|
| 1 | every judged question AUC ≥ 0.65 | see table | **FAIL** |
| 2 | AUC(M2) ≥ 0.75 | 0.75 | **FAIL** |
| 3 | accuracy ≥ 0.75 and Brier ≤ 0.20 | 0.75, 0.17 | pass |
| 4 | AUC(M2) ≥ AUC(M0) + 0.05 | 0.75 vs 0.96 | **FAIL** |

**Verdict: bars 1, 2, and 4 fail; bar 3 and bar 5 pass. The score stays advisory.**

- Bar 2 misses narrowly. The exact AUC is 0.7485.
- Bar 4 is the decisive result. On held-out runs, reading time alone ranks
  overloaded chunks at AUC 0.96, while the fitted Jev model reaches 0.75. The
  fitted weights put most of their mass on `padded` (0.44) and
  `many_new_names` (0.28). They drop `distant_recall` and `unframed_change`.
- Jev does answer most questions the way the labeller does. On its own label,
  `many_new_names`, `several_points` and `dense_code` reach AUC 0.93–0.97 and
  `unframed_change` 0.89. Those signs do not predict the overall `overloaded`
  label beyond what reading time already gives.
- `distant_recall` fails bar 1 (0.61) even with the earlier chunks in the
  state, so it should be reworded or dropped. `disconnected` and
  `missing_contrast` had too few "yes" labels to judge.

Limitations:

- No item was labelled twice, so label noise is unknown (stated in the bars).
- Checked after the results: the labeller sees a chunk's length. Raw state length alone predicts its
  label at AUC 0.78 on the whole sample, against 0.87 for reading time.
  Reading time adds to length, but a labeller who leans on length favours M0.
- The held-out split has 28 items, 9 of them overloaded. Its AUCs are rough.

As pre-registered, a bar 4 failure points to reading time alone. Jev remains
available as an advisory detail: its per-question answers say what makes a
chunk heavy, which reading time cannot.

## Since

The run above was criteria v2. Its questions, weights, and settings now live
in the rubric file [`crates/gewu-review/rubrics/load.toml`](../crates/gewu-review/rubrics/load.toml),
which also holds the bars, the labelling guidance, and the recalibration
procedure. The script became the general tool `scripts/load_calibration`
(any walkthroughs, labels from agents or reviewer feedback). It generates
the labelling instructions from the rubric, so they are no longer a separate
file. Rubric v3 applied this run's budget proposal: 220 s, chosen on the
fitted walkthroughs and checked on the held-out ones. Later calibrations
record their results in the rubric's `[provenance]` and, when they are
substantial, in a new section here.
