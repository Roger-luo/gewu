# Gewu UI harness

Capture the desktop web UI, run mechanical checks, brief an independent judge,
score against a versioned rubric, and rank runs. Agents drive the UI only
through the test hooks in `docs/desktop.md` (`data-gewu-ready`,
`main[data-gewu-screen]`, `window.__gewu`).

## Loop

1. **capture** — open the UI, visit every `window.__gewu.pages()` entry, write
   screenshots + ARIA snapshots, run checks → `manifest.json`, `pages/`,
   `checks.json`
2. **brief** — build a self-contained judge prompt from `prompts/judge.md` +
   the rubric + artifact list → `judge/<id>.brief.md`
3. Independent judge subagent writes `judge/<id>.json` (never reads source)
4. **score** — validate evidence, aggregate judges, apply gates →
   `scorecard.json`
5. **rank** — compare scorecards across runs
6. Reviser follows `prompts/revise.md`, edits `apps/desktop`, re-captures

## Commands

```bash
pnpm --dir apps/ui-harness ui capture --url http://127.0.0.1:5173/?mock=showcase \
  --out output/run-1 --viewports 1440x900,1024x700 --label showcase

pnpm --dir apps/ui-harness ui capture --url http://127.0.0.1:5173/ \
  --out output/run-2 --start "pnpm --dir apps/desktop dev" \
  --wait-url http://127.0.0.1:5173/

# Full workbench user stories (fixture repos + fake ACP agent):
pnpm --dir apps/ui-harness ui journeys --out output/run-1 \
  --gewu target/debug/gewu --static apps/desktop/dist

pnpm --dir apps/ui-harness ui brief --run output/run-1 --judge-id a
pnpm --dir apps/ui-harness ui score --run output/run-1
pnpm --dir apps/ui-harness ui rank --runs output/run-1 output/run-2 --out rank.md
```

`mise run ui-journeys` builds the UI, captures review pages, and runs the seven
journeys into `.gewu/ui-runs/<label>`.

## QA

Journeys replay fixed stories; QA finds the stories nobody wrote down. A
scenario writer reads only the user docs and writes workflows; QA testers then
walk them in the real app, one user action at a time, and report what fails a
reviewer.

1. Write scenarios: give a subagent `prompts/qa-scenarios.md` with an output
   directory → `<out>/scenarios.json`.
2. Start one sandbox per tester and leave it running:
   `mise run ui-sandbox .gewu/qa/<round>/sb1`. It serves `gewu ui serve` on
   throwaway `shop` and `notes` repositories (an origin with pull request #7,
   a fake `gh`) and keeps one browser open. `sandbox.json` lists the URL,
   repository paths, and custom agent commands that write batches, write them
   slowly, or fail.
3. Give each tester `prompts/qa-tester.md`, its sandbox, and a few scenarios.
   Testers act only through `ui act`, which clicks, types, and presses keys by
   accessible role and name, then prints the accessibility tree and saves a
   screenshot under `<sandbox>/shots/`:

   ```bash
   pnpm --dir apps/ui-harness ui act --sandbox "$PWD/.gewu/qa/r1/sb1" click button "Open review"
   ```

4. Each writes `report.json` with reproducible steps per issue. Fix, rebuild,
   start fresh sandboxes, and rerun with testers rotated across scenarios
   until no issues remain. Stop a sandbox with Ctrl-C.

Bin: `gewu-ui` (via `tsx src/cli.ts`). Override rubric with `--rubric <file>`;
by default the highest `rubric/vN.json` is used.

## Setup

```bash
pnpm --dir apps/ui-harness install
pnpm --dir apps/ui-harness exec playwright install chromium
pnpm --dir apps/ui-harness typecheck
pnpm --dir apps/ui-harness lint
pnpm --dir apps/ui-harness test
```

## Run layout

```text
<run>/
  manifest.json
  checks.json
  journey-checks.json          # when journeys ran
  journeys.json
  pages/<viewport>[-dark]/<nn>-<page-id>.png
  pages/<viewport>[-dark]/<nn>-<page-id>.aria.yaml
  pages/<viewport>/<nn>-interaction-<name>.png
  journeys/<nn>-<id>/<mm>-<slug>.png
  journeys/<nn>-<id>/<mm>-<slug>.aria.yaml
  judge/<id>.brief.md
  judge/<id>.json
  scorecard.json
```
