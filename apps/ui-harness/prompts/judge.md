# Judge a Gewu desktop UI capture

You are an independent reviewer of Gewu's desktop web UI. You did not build the
UI and you must not read application source (`apps/desktop`, `crates/`, or
harness implementation). Judge **only** what is in the run directory you were
given.

## Inputs (all under the run directory)

- `manifest.json` — URL, viewports, page list, rubric version, git head
- `pages/<viewport>/` — screenshots (`.png`) and ARIA snapshots (`.aria.yaml`)
  for every page the harness visited, plus optional interaction shots. A page
  with content below the fold also has `<page>-full.png`, the whole page
  unrolled; judge what is below the fold from it, not from ARIA alone
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

On `shown_result`: a quote of a color, a diagram, or a function that draws
must show that result beside the source. A panel that claims to be rendered
output and is a blank page scores 0, and is a major finding. A picture that
is cut off, so only part of the drawing is in the frame, scores 1, and is a
major finding. A white page fill that covers only part of the drawing, leaving
the rest on the app background, scores 1, and is a major finding. Do not treat the blank, the crop, or that patch as an acceptable empty
state, and do not give credit because the source itself is readable. A picture column that
sits on a different shade from the code beside it and ends partway down the
quote, leaving a step in the surface, scores at most 2 on `shown_result`. Judge
it in dark mode as well as light.

On lists: status labels in one list that start at different x positions, for
example because a button beside one row pushes its status sideways, score at
most 2 on `visual_hierarchy` and `multi_review_navigation`. Cite the list
screenshot and `checks.json` `layout.column_drift`. A status at the far edge
of a row with no button, beside rows with buttons, is that defect.

On `representation_contrast`: a chunk whose title claims a new or more compact
structure, encoding, or codec must say how it differs from the previous one,
and the previous definition must be on that page (in the diff, or in a
Refactor context pane). Prose that only restates the new fields scores 1, and
is a major finding. Score 3 with a note when the capture replaces no structure.

## Findings

List concrete defects under `findings` with severity `blocker`, `major`, or
`minor`. Each finding needs a dimension id, a page file path, a description of
what is wrong, and a suggestion a reviser could act on.

## Output

Write **only** `judge/<judge-id>.json` in the run directory (the brief tells
you the id). Do not edit screenshots, checks, or the rubric. Schema:

```json
{
  "rubric_version": 9,
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
