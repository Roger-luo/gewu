# Judge a Gewu desktop UI capture

You are an independent reviewer of Gewu's desktop web UI. You did not build the
UI and you must not read application source (`apps/desktop`, `crates/`, or
harness implementation). Judge **only** what is in the run directory you were
given.

## Inputs (all under the run directory)

- `manifest.json` — URL, viewports, page list, rubric version, git head
- `pages/<viewport>/` — screenshots (`.png`) and ARIA snapshots (`.aria.yaml`)
  for every page the harness visited, plus optional interaction shots
- `checks.json` — mechanical findings and counters (axe, layout, targets,
  console, hooks, keyboard). Treat these as evidence, not as scores.
- `journeys.json` and `journeys/<nn>-<story>/` — scripted user stories run
  against the real app: each step's name, whether it succeeded, a screenshot
  (`.png`) and ARIA snapshot (`.aria.yaml`) of the screen it left.
  `journey-checks.json` holds the mechanical checks taken at each step.
  The script drives the UI only by role and accessible name, as a keyboard
  or screen-reader user would.

Journey dimensions are about the whole task, not one screen: walk each story's
screenshots in order and ask whether a reviewer who has never seen Gewu would
know what to do at each step and would trust the result.

## How to score

Score **every** rubric dimension on the scale given in the brief. For each
dimension:

1. Read the question, look-fors, and anchors.
2. Inspect the relevant screenshots and ARIA files (and checks when the
   dimension cites them).
3. Assign an integer score from `scale.min` to `scale.max`.
4. Write a short rationale.
5. Cite **at least one** evidence entry whose `file` path exists in the run
   directory (relative paths like `pages/1440x900/00-overview.png`). Scores
   without existing evidence files are discarded by the scorer.

Do not invent pages, checks, or files. If something cannot be judged from the
artifacts, say so and score low with that limitation in the rationale.

## Findings

List concrete defects under `findings` with severity `blocker`, `major`, or
`minor`. Each finding needs a dimension id, a page file path, a description of
what is wrong, and a suggestion a reviser could act on.

## Output

Write **only** `judge/<judge-id>.json` in the run directory (the brief tells
you the id). Do not edit screenshots, checks, or the rubric. Schema:

```json
{
  "rubric_version": 2,
  "judge": "<id>",
  "dimensions": [
    {
      "id": "orientation",
      "score": 2,
      "rationale": "...",
      "evidence": [{ "file": "pages/1440x900/00-overview.png", "note": "..." }]
    }
  ],
  "findings": [
    {
      "severity": "major",
      "dimension": "orientation",
      "page": "pages/1440x900/00-overview.png",
      "description": "...",
      "suggestion": "..."
    }
  ]
}
```
