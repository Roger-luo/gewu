# Judge a Gewu walkthrough as it appears in the terminal

You are an independent reviewer of how a code-review walkthrough reads in
Gewu's terminal UI. You did not build the UI or write the walkthrough. Judge
only what is on screen. Do not read `harness/`, `crates/`, or `.gewu/harness/`.

## Inputs, all in the run directory

- `.gewu/self-review/frames-80x24/` and `.gewu/self-review/frames-120x40/`:
  every page a reader reaches by pressing PageDown through the overview, each
  chunk, and the summary. Files are named `<stage>-<name>-p<page>.txt`; each
  is exactly one terminal screen.
- `.gewu/self-review/frames-80x24.json` and `frames-120x40.json`: the same pages
  with the mechanical results: `clipped` rows, `unreached` rows, `uncued_pages`
  that scroll without a position cue, `orphaned_pages`, and each page's
  `repeated_rows` from the page before.
- `.gewu/review-spec.json`: the walkthrough, for what each chunk intends.

The captures are plain text. Colour, bold, and syntax highlighting are not
visible, so do not score them; say so if a judgement depends on them.

## Score six dimensions, 0–3, at the reference size

0 broken, 1 material problems, 2 usable with small gaps, 3 clear. Every score
needs `evidence`: page files and what on them earned the score.

120×40 is the reference terminal: score `dimensions` there. 80×24 is a
supported degraded mode; score it separately under `degraded`, with the same
six names and the same evidence rule. Degraded scores are recorded, not gated,
so be as strict there as at the reference.

- `orientation`: on every page the reader can tell which chunk they are in,
  which file and lines the code is from, and where they are within the chunk.
- `legibility`: prose and code wrap cleanly; the line-number gutter and diff
  markers stay aligned on wrapped rows; nothing reads as garbled.
- `page_breaks`: pages break where a reader can pause. Prose is not cut mid
  sentence, and a short definition is not split across pages when it would
  have fit. For a chunk with an `oversized_reason`, the unit reads naturally
  across its pages.
- `prose_with_code`: the sentence that says what to look for is on screen with
  the code it is about, or immediately before it. The reader is not asked to
  remember a previous page to read this one. A page that opens on code pins the
  first sentence of its introducing paragraph in the header, marked `↳`; judge
  whether that is enough.
- `density`: the screen is spent on content. Count chrome rows (headers,
  footers, blank padding) against content rows on typical pages, and report the
  ratio at each size.
- `context`: where a chunk has a context pane, it helps without crowding out the
  code: inline below the chunk under 140 columns, beside it from 140. Score 3 with a note if no
  chunk has one.

## Report what else is wrong

`findings` holds specific defects: a page that hides the code its prose
introduces, a header with nothing under it, a wrapped row that breaks an
identifier, a cue that is wrong, an overview or summary that does not orient.

## Output

Write `.gewu/self-review/tui-judge.json` and nothing else:

```json
{
  "rubric_version": 3,
  "dimensions": {
    "orientation": {"score": 3, "evidence": ["002-...-p1.txt: ..."]},
    "legibility": {"score": 3, "evidence": ["..."]},
    "page_breaks": {"score": 2, "evidence": ["..."]},
    "prose_with_code": {"score": 3, "evidence": ["..."]},
    "density": {"score": 2, "evidence": ["content 31 of 40 rows on ..."]},
    "context": {"score": 3, "evidence": ["no chunk has a context pane"]}
  },
  "degraded": {
    "orientation": {"score": 3, "evidence": ["80x24 002-...-p2.txt: ..."]},
    "...": "the same six dimensions at 80x24"
  },
  "findings": [{"stage": "chunk-id", "size": "80x24", "page": 2, "category": "page-break", "detail": "..."}]
}
```
