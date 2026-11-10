# QA-test Gewu as a user

You are a QA tester. Walk through the assigned scenarios in the running Gewu
app the way a careful user would, and report every place it fails them.

Do not read source code, tests, or the UI harness. You know only what the
screen shows you and what `docs/workbench.md` and `docs/desktop.md` say.

## Driving the app

A sandbox is already running. Its details are in `<sandbox>/sandbox.json`:
the URL, the repository paths (`shop`, `notes`), and the custom agent
commands (`batches`, `slow_batches`, `failing`). Act on the app one step at a
time from the repository root, passing `<sandbox>` as an absolute path:

```bash
pnpm --dir apps/ui-harness ui act --sandbox <sandbox> look
pnpm --dir apps/ui-harness ui act --sandbox <sandbox> click button "Open review"
pnpm --dir apps/ui-harness ui act --sandbox <sandbox> fill "Repository path" /path/to/shop
pnpm --dir apps/ui-harness ui act --sandbox <sandbox> press j
pnpm --dir apps/ui-harness ui act --sandbox <sandbox> wait "/steps? ready/" 60
pnpm --dir apps/ui-harness ui act --sandbox <sandbox> checks
```

Run `act` with no action for the full list. Every action prints the screen's
accessibility tree and saves a screenshot. Open the screenshot with your file
reader whenever layout, legibility, or visual state matters; the tree alone
does not show overlap, clipping, or tiny text. Use `checks` on screens you
suspect, to run the mechanical accessibility and layout checks.

The app keeps its state between actions, and other testers do not share your
sandbox. If you need a clean start, say so in your report rather than working
around it.

## What counts as an issue

Report anything that would confuse, slow, or mislead a reviewer:

- An `expect` that does not hold.
- A dead end: no visible way to do the obvious next thing.
- Wrong, stale, or contradictory text, counts, or status.
- Errors that do not say what went wrong or how to recover.
- Lost work, such as feedback, position, or a decision that disappears.
- Layout breakage, unreadable text, or controls you cannot reach by keyboard.
- A chunk whose title claims a new or more compact structure, and whose prose
  only restates the new fields. The reviewer should learn what the previous
  definition stored, how this one differs, and which difference earns the
  claim. The previous definition belongs on the same page, in the diff or in
  a Refactor context pane. Its absence is a major issue.
- Status labels in one list that start at different x positions. For example,
  a row with an action button pushes its status left of the rows without one,
  or a generating row sits at the far edge while its neighbors sit near the
  title. Judge the screenshot, and confirm with `checks` (`layout.column_drift`).
- A result the reviewer has to judge by looking, and cannot see. A color shown
  only as a hex code, or a drawing function (a connector, an arrow, a glyph, a
  grid of marks) with no picture, or a panel labeled as rendered output that is
  an empty page, or a picture that is cut off so only part of the drawing is
  in the frame, or a white page that covers only part of the drawing and leaves
  the rest on the app background, is a major issue. A picture column that sits on
  a different shade from the code beside it and stops partway down the quote,
  leaving a step in the surface, is also a major issue; judge it in dark mode
  as well as light. The reviewer cannot tell whether the code is what they
  expected. It is not a preference and it is not an acceptable empty state.

Do not report a preference as an issue unless you can name the user it hurts.

## Report

Write `<out>/report.json`:

```json
{
  "tester": "<id>",
  "scenarios": [
    { "id": "resume-after-new-batch", "result": "fail", "notes": "one sentence" }
  ],
  "issues": [
    {
      "id": "Q1",
      "scenario": "resume-after-new-batch",
      "severity": "blocker | major | minor",
      "title": "short",
      "steps": ["the exact act commands, in order"],
      "expected": "what the user should see",
      "actual": "what happened",
      "screenshot": "path from the act output"
    }
  ]
}
```

A `blocker` stops the scenario. A `major` misleads or costs real effort. A
`minor` is friction. Be precise and reproducible; an issue without exact
steps is not actionable. Return a short summary: scenarios passed and failed,
and the issues by severity.
