# Write QA scenarios for Gewu

You are a QA lead preparing a test plan for Gewu, a desktop app that walks a
human reviewer through a code change step by step. An AI agent writes the
walkthrough; the reviewer reads it and reaches their own decision.

Read only the user-facing documentation:

- `docs/workbench.md`: repositories, starting reviews, agents, batches.
- `docs/desktop.md`: the review screens, feedback, keyboard use.
- `docs/design.md`: what the product promises the reviewer.

Do not read source code, tests, or the UI harness. Your scenarios must come
from what a user is told to expect, so they catch places where the app breaks
that promise.

## The environment the tester gets

A sandbox serves the real app with two throwaway Git repositories:

- `shop`: a small project with commits, uncommitted changes, a `main` branch,
  and an origin with pull request #7.
- `notes`: a second, smaller repository.
- `shop` also contains `diagrams/lib/illustration.typ`, a Typst library of
  named colors and a `route` function that draws a polyline with an arrowhead.
  `diagrams/lib/atoms.typ` places a grid of dots. Compiling either file by
  itself draws an empty page, and the dots are placed past the edge of that
  page. No PDF, PNG, or SVG of them is committed. A directory review of
  `diagrams` (No agent) shows each file as newly added code.

There are three custom agent commands, typed into the "Custom command" agent
option: `batches` writes a walkthrough in several batches, `slow_batches` does
the same slowly (about 3 seconds per step) so in-flight states can be observed,
and `failing` fails partway. No real AI agents are available. The tester acts
only through the screen: clicking, typing, and keys.

## What to write

Write 10 to 14 scenarios to `<out>/scenarios.json`:

```json
{
  "scenarios": [
    {
      "id": "resume-after-new-batch",
      "persona": "A reviewer who stops halfway and comes back later.",
      "goal": "Continue reviewing the steps that arrived while away.",
      "setup": "shop added; a review started with slow_batches.",
      "steps": [
        "Open the review and mark the first two steps reviewed.",
        "Go back to the review list while generation continues.",
        "When the task shows more steps, open the review again."
      ],
      "expect": [
        "The reviewer can reach the first step they have not reviewed in one obvious action."
      ]
    }
  ]
}
```

Cover real workflows end to end, not single widgets:

- First run: adding a repository, then starting the first review.
- Each review scope: a commit range, uncommitted changes, a pull request, and a directory.
- Reading a walkthrough with the mouse and with only the keyboard.
- Leaving feedback, changing your mind, and deleting it.
- Reaching a decision and seeing it reflected afterwards.
- Generation that is still running, that arrives in batches, that fails, and that is stopped.
- Leaving a review halfway and coming back, including after new batches arrive.
- Juggling two reviews and two repositories.
- Narrow windows and recovering from mistakes, such as bad paths or empty ranges.
- Judging what the code produces, not only what it says. When a change is a
  color, a diagram, a drawing function, a rendered document, or anything a
  person checks by looking, the reviewer must see that result beside the
  source — including a result that was never committed. One scenario must open
  the Typst library in `shop` and expect each color to be visible as a color.
  The same scenario, or another, must reach `route` and expect a drawn
  connector or arrowhead beside that code, and must reach `atoms` and expect
  the whole grid of dots, not a corner of it, on one ground. A white patch
  smaller than the grid fails the scenario. An empty rendered panel fails
  the scenario, and so does a panel that cuts the drawing off: a blank page,
  a cropped one, or a page fill that does not cover the marks does not show
  what the function does. Expect the picture column to sit on the same surface
  as the code beside it, with no step in shade where the picture box ends;
  judge that in dark mode as well as light.

A chunk that presents a new form of an existing structure must teach the
difference. When a walkthrough has such a chunk, one scenario opens it and
expects the prose to say what the previous definition stored and which
difference earns the title's claim (for example, why a stored word is more
compact). The previous definition is on the same page, in the diff or in a
Refactor context pane. A title with a restatement of the new fields fails
the scenario.

The review list must read as a column. One scenario must start a review with
`slow_batches`, leave it generating, and look at the list while other reviews
have action buttons (for example "Open review" or "See how to fix"). Expect the
status labels to start at the same x position in every row, so the generating
row, which has no button, does not push its status to the far edge. A status
that sits at a different x from its neighbors fails the scenario.

Make each `expect` an observable outcome a user would care about, not an
implementation detail. Return the path you wrote.
