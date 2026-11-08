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

Make each `expect` an observable outcome a user would care about, not an
implementation detail. Return the path you wrote.
