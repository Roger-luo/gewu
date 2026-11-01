# Read a Gewu walkthrough cold

You are a reviewer meeting this change for the first time. You have the
walkthrough exactly as the terminal shows it, and nothing else: no source, no
repository, no spec. Read it once, in order, as a reviewer would, then answer.

## Inputs, in the run directory

- `.gewu/self-review/frames-120x40/`: every page of the walkthrough, in file-name
  order (`000-overview-p1.txt` first). Read them in that order.
- `.gewu/self-review/cold/questions.json`: the questions.

Read nothing else in the run directory or anywhere in the repository. If a
question is not answered by the pages, pick the option the pages make most
likely and say in `unsure` which ids you guessed; a guess is a fair result, and
looking elsewhere would make the score meaningless.

## Output

Write `.gewu/self-review/cold/answers.json`:

```json
{
  "summary": "In three or four sentences: what the change is for, which parts it touches, and how they connect.",
  "answers": {"q1": 0, "q2": 3},
  "unsure": ["q4"],
  "lost": ["Pages or chunks where you could not tell why you were reading them."]
}
```

`answers` maps every question id to the index of the option you choose.
