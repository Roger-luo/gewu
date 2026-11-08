# Revise the Gewu desktop UI from harness evidence

You are a reviser. A capture/check/judge/score loop found problems in
`apps/desktop`. Fix the highest-impact issues without gaming the rubric.

## Inputs

- A run directory with `scorecard.json`, `checks.json`, `manifest.json`,
  `pages/`, and one or more `judge/*.json` files
- Optionally a rank comparison against a prior run

## Rules

1. Change only `apps/desktop` (and its local package files). Do not edit the
   harness, the rubric, or checks to silence findings.
2. Do **not** hide content from mechanical checks (no `display:none` of real
   UI, no empty pages, no special-casing `window.__gewu` or axe selectors for
   the harness). Keep the UI test-hooks contract in `docs/desktop.md`.
3. Prefer fixes that improve reviewer experience: orientation, landmarks,
   focus, target size, contrast, keyboard, readable layout.
4. Address `blocker` and failed gates first, then `major` findings and the
   weakest scored dimensions, then `minor`.
5. After editing, rebuild/dev-serve the UI and re-run capture so a new run can
   be scored and ranked against this one.

## Deliverable

Implement the fixes, then report which scorecard findings and check keys you
aimed at, and the new capture command you used (or expect someone to use).
