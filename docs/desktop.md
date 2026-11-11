# Desktop frontend

Gewu's desktop app is a web UI in a Tauri shell. The web UI is the part agents
see and operate: the same build runs in the Tauri window for a person and in a
headless browser for the UI harness, so what an agent judges is what a reviewer
gets.

## Crates and packages

| Path | Role | Depends on |
|---|---|---|
| `crates/gewu-review` | Review domain: spec, input, session, validation | — |
| `crates/gewu-git` | Reading diffs, trees, and pull requests from Git | `gewu-review` |
| `crates/gewu-harness` | Progressive generation harness, free of terminal output | `gewu-review`, `gewu-git` |
| `crates/gewu-tui` | Terminal review UI | `gewu-review` |
| `crates/gewu-app` | Interface-independent app service: snapshot and actions | `gewu-review` |
| `crates/gewu-http` | HTTP bridge serving the app protocol and the built web UI | `gewu-app` |
| `crates/gewu-desktop` | Tauri shell exposing the app protocol over IPC | `gewu-app` |
| `crates/gewu-cli` | Command parsing and terminal presentation | `gewu-review`, `gewu-git`, `gewu-harness`, `gewu-tui` (and later `gewu-app` / `gewu-http`) |
| `apps/desktop` | Web UI (Vite, React, TypeScript) | app protocol |
| `apps/ui-harness` | Capture, check, judge, score, and rank the web UI | UI test hooks |

Domain rules live in Rust. The web UI renders a snapshot and sends actions; it
never replays events or decides review state itself.

## App protocol

One protocol, two transports. JSON uses `snake_case` field names, matching the
spec files on disk.

### Workspace

A workspace root is a directory holding `.gewu/review-input.json` and
`.gewu/review-spec.json`. The reviewer's events live at
`.gewu/reviews/<spec.session_key()>/events.jsonl`, exactly where the terminal UI
keeps them, so either interface can pick up a session the other left.

### Snapshot

```text
Snapshot {
  review_id: string,                       // spec.session_key()
  source: { base: string, head: string },
  overview: Overview,                      // as in review-spec.json
  // remaining_work_items is null when the spec omits generation; complete is
  // true iff remaining is null or zero. batch_scoped mirrors !complete.
  generation: { remaining_work_items: number | null, complete: bool },
  chunks: ChunkView[],                     // in review order
  feedback: Feedback[],                    // session order, as in the domain
  summary: SummaryView,
  last_visited: string | null,             // live chunk id, if any
}

ChunkView {
  id, index,                               // index is 0-based in review order
  title, summary,
  kind: "concept" | "refactor" | "integration" | "test" | "other",
  section: string | null, confidence: string | null,
  oversized_reason: string | null,
  context: ChunkContext | null,            // as in review-spec.json
  blocks: BlockView[],
  status: { visited: bool, reviewed: bool, open_issues: number, feedback_count: number },
}

BlockView =
  | { type: "narrative", markdown: string }
  | { type: "changes", presentation: "diff" | "added_code" | "rendered_markdown" | "summary_only",
      quotes: QuoteView[] }

QuoteView {
  change_id, path, start, end,             // body-line span, inclusive, 1-based
  language: string | null,                 // from the path's extension, e.g. "rust"
  more_above: bool, more_below: bool,      // hunk lines exist outside the span
  lines: { kind: "added" | "removed" | "context",
           old_line: number | null, new_line: number | null, text: string }[],
  preview: { label, media, data } | null,  // omitted when there is nothing to draw
}

SummaryView {
  reviewed: number, total: number,
  unreviewed: string[],                    // chunk ids
  unresolved_issues: string[],             // feedback ids
  decision: "approve" | "comment" | "request_changes" | null,
  suggested_decision: "approve" | "comment" | "request_changes",
  batch_scoped: bool,                      // generation is incomplete
}
```

### Actions

Tagged by `type`. Every action returns the new `Snapshot`; a rejected action
returns an error with a human-readable message and changes nothing.

```text
{ type: "visit", chunk_id }
{ type: "set_reviewed", chunk_id, reviewed }
{ type: "add_feedback", chunk_id, kind: "note" | "issue", target: "code" | "explanation",
  body, anchor: { change_id, side?: "new" | "old", start_line, end_line } | null }
{ type: "resolve_feedback", feedback_id, resolved }
{ type: "delete_feedback", feedback_id }
{ type: "retarget_feedback", feedback_id, target }
{ type: "set_decision", decision }
```

The service assigns feedback ids and fills an anchor's path and patch hash from
the review input. `side` defaults to `new`; `old` counts the file before the
change, which is the only side a block of pure deletions has. An anchor must
name lines that a chunk quote shows on that side, so a comment on a whole block
anchors to that block's lines.

The summary screen's publication preview is the text those notes would become.
For a workbench task the backend builds it (`Workbench::publication`, from
`crates/gewu-review/src/publication`): pull-request review comments, issues on
the reviewed repository, and issues on `Roger-luo/gewu`. A destination with
more than one issue also has a tracking issue. The repository is `owner/name`
read from the task's Git remote (`origin`, or a pull request's own remote);
without a github.com remote the draft can be read but not sent. Each
destination shows its request, such as `Not sent · POST
/repos/acme/demo/issues`. A single-review window and the mock build the same
draft in `apps/desktop/src/publication.ts`.

**Publish** sends only after a confirmation dialog that lists every request and
the account it posts as. `crates/gewu-github` sends them one at a time through
a `Transport` and stops at the first refusal, so the receipt is exactly what
exists on GitHub. Each tracking issue is sent last and links its issues by
number. What was posted is kept in the task's `published.json`; those notes
drop out of the next draft and the preview lists them under "Already on
GitHub", so pressing Publish again never posts a note twice.

**Sign in to GitHub** first asks how. **Sign in with your browser** uses Gewu's
GitHub OAuth app (client ID `Ov23liMKKIvqy32L2nuS`, device flow, scope `repo`
for public and private repositories): the dialog shows a code, opens
`github.com/login/device`, and polls until the reviewer approves it. The token
is kept in the system keychain (service `gewu:github.com`, account `oauth`) and
Gewu calls `api.github.com` with it directly. On macOS it goes through
`/usr/bin/security`, as `gh` does: a Keychain item trusts the program that
made it, and Gewu's own binary changes on every rebuild, so reading it
directly would ask for the password after each one. The token is read once
per run and kept in memory (`oauth::Cached`). **Sign out** deletes it from the
keychain; the reviewer can also revoke Gewu under GitHub's Settings →
Applications.

**Use the GitHub CLI** posts through `gh api` with the CLI's own login, so a
reviewer who already uses `gh` has nothing to set up. It also works in
organizations that restrict OAuth apps but have approved the GitHub CLI; when
GitHub refuses a post for that reason, the receipt says to switch. The dialog
lists the CLI first when `gh` is signed in, shows its login, and greys it out
when `gh` is not installed. If `gh` is installed but signed out, it asks for a
token to pass to `gh auth login --with-token`. Leaving the CLI goes back to
Gewu's app; it never signs `gh` out.

The account chip shows which method is in use and has **Switch** to change
it. The choice (`gewu_github::GitHub`) is saved in the workbench home's
`github.json` and outlives the run. `GEWU_GITHUB=gh` starts on the CLI when
nothing is saved, and `GEWU_GITHUB_CLIENT_ID` replaces the client ID.

Typing `@` in a comment suggests the repository's contributors (humans only,
top 100 by commits) and inserts the picked `@login`. The list comes from
`GET /repos/{owner}/{name}/contributors`, a read that notifies nobody, made
when the comment dialog first opens and then cached by the workbench for the
rest of the run (`Workbench::contributors`). Signed out, or for a repository
with no GitHub remote, there are no suggestions. The sandbox's recorder lists
the signed-in login plus `grace` and `linus`.

The `gewu ui serve` sandbox answers with `gewu_github::Recorder` instead: its
device code is `GEWU-0000`, approved on the first poll as `gewu-sandbox`; its
"GitHub CLI" is always signed in as `gewu-sandbox`; and
Publish records the requests without sending them, so a QA agent can press
every button. `mise run desktop-record [home]` opens the desktop window
the same way (`GEWU_GITHUB=record`), with `GEWU_HOME` defaulting to
`.gewu/record-home` so recorded posts never mark real notes as sent. Tests use
the same recorder or the mock and never reach GitHub.

Comments on a block and on an explanation do not need line numbers picked by
hand. The block's buttons anchor to the block's span; the explanation beside a
block targets `explanation` with the same span, so the walkthrough text and the
code it describes stay tied together.

Publishing maps each anchor onto a GitHub review comment: `side: "new"` to
`RIGHT` and `side: "old"` to `LEFT`, with `start_line` and `end_line` as the
comment's line range. Explanation comments describe Gewu's
walkthrough rather than the code, so they are not code review comments.

### Explainers

A chunk's `blocks` may include `{ "type": "explainer", "kind": … }`
(`apps/desktop/src/explainer.ts`, drawn by `components/explainers/`):

- **Claim** (`ClaimCard`): the quoted claim, the measured result, and how the
  chunk serves it.
- **Layout** (`LayoutView`): first one animated record (`LayoutMorph`) that
  plays from the first row to the last on arrival: fields the last row drops
  turn red and say "dropped", then collapse, fields both rows keep stay at
  their width, and the byte total counts down. Row buttons and Replay step
  through it again. Below it, every row as a static bar scaled to its bytes,
  with a legend of exact sizes and "After is N× smaller".
- **Cost model** (`CostModelView`): one slider per parameter (powers of two
  for `log2`) with buttons for its `marks`, a bar and value per series
  recomputed by `compileFormula`, the ratio between two series, and the
  formulas under "How it is computed". **Play** sweeps the first parameter
  from its minimum to its maximum, pausing on the marks, so the bars grow in
  front of the reader.

Bars animate with [Motion](https://motion.dev) under `MotionConfig
reducedMotion="user"`, so the system's reduce-motion setting turns it off.
`?mock=explainers` shows all three on a step modeled on a performance pull
request.

### Settings

The workbench's **Settings** screen shows every option, so none is reachable
only through an environment variable:

- **GitHub** (`gewu_github::GitHub`, saved in the home's `github.json`): sign
  in with Gewu's GitHub app or the GitHub CLI; another OAuth app's client ID;
  and **Rehearse instead of sending**, which makes Publish record what it would
  post through `Workbench::rehearse` and send nothing. A rehearsal never writes
  `published.json`, so the notes stay unpublished.
- **Agents and generation** (`gewu_workbench::Settings`, in `settings.json`):
  the agent preselected for a new review, and proofread rounds (0 to 5, read
  when a review starts).
- **Where Gewu keeps things**: `GEWU_HOME`, `GEWU_BIN`, and `GEWU_GITHUB=record`,
  shown read-only because they are set before Gewu starts.

Nothing is saved until **Save**; a refused value is reported beside the form
and saves nothing.

### Seeing what the code produces

A quote of source is not always enough to judge the change. When the file is
Typst (`.typ`):

- Each `rgb(...)` and `luma(...)` literal is drawn as that color, on its line,
  so a reviewer can see whether `#243746` is the ink they expect.
- Beside the source, Gewu shows a picture of the result. A PNG, SVG, or PDF
  with the same name sitting next to the file is that picture, even when it is
  not part of the commit, and only when that picture actually draws something.
  A Typst document is rendered when the `typst` tool is available. A library,
  which draws nothing by itself, becomes a small example of the step: a sheet
  of the named colors, or a call to the function the step quotes (for example
  `route` drawn as a polyline with an arrowhead, or `atoms` drawn as the whole
  grid of dots). An empty page is not shown, and a viewport that cuts off part
  of the drawing is widened so every mark is on screen. A white page fill that
  would cover only some of those marks is left out, so the figure is not a
  small white square on the surrounding surface.

`preview.media` is `image/svg+xml` (`data` is the markup) or `image/png`
(`data` is base64). The picture is absent when nothing above can be drawn.

### Transports

- HTTP (`gewu ui serve --root <workspace> --static <dist> --port <port>`, or
  `gewu_http::Server::{start,serve}`): bind `127.0.0.1` only (`port` `0` for
  ephemeral). `GET /api/health`, `GET /api/snapshot`, `POST /api/dispatch`
  with an action body. Errors are `{ "error": string }` with status 400.
  Every other path serves the static directory, falling back to `index.html`.
- Tauri IPC (`crates/gewu-desktop`, binary `gewu-desktop`): commands
  `snapshot` and `dispatch { action }` with the same shapes. Workspace root
  comes from `--root <dir>`, else `GEWU_ROOT`, else the current directory.
  Dev: `mise run desktop [root]` (Vite on `:5173` + Tauri). Release/debug
  binary embeds `apps/desktop/dist` when that build exists.
- `frontendDist` and `mise run check`: `tauri.conf.json` points at
  `apps/desktop/dist`, which `generate_context!` requires at compile time.
  When that directory has no `index.html`, `crates/gewu-desktop/build.rs`
  overrides via `TAURI_CONFIG` to the committed `ui-placeholder/` so the
  workspace compiles without Node or a prior UI build. It never writes into
  `apps/desktop/dist`.

## UI test hooks

The harness drives the UI through these, never through CSS classes or layout.

- `document.documentElement.dataset.gewuReady === "true"` once the snapshot is
  rendered and no transition is running.
- The main region is `<main data-gewu-screen="overview" | "chunk" | "summary">`;
  on a chunk screen it also carries `data-gewu-chunk-id`.
- Landmarks: `<nav aria-label="Review navigator">`, `<main>`, and, when a chunk
  has context, `<aside aria-label="Context">`. Dialogs use `role="dialog"` with
  an accessible name.
- `window.__gewu`:

  ```ts
  type Target = { screen: "overview" } | { screen: "summary" } | { screen: "chunk"; index: number };
  interface GewuTestApi {
    state(): { screen: "overview" | "chunk" | "summary"; chunk_index: number | null;
               chunk_count: number; overlay: "help" | "feedback" | null };
    goto(target: Target): Promise<void>;   // resolves once ready again
    pages(): { id: string; label: string; target: Target }[];  // every page, review order
  }
  ```

- Keyboard: `j` / `k` next and previous page, `o` overview, `s` summary, `r`
  toggle reviewed, `c` comment, `x` request change, `?` help, `Esc` close.

## UI harness

`apps/ui-harness` turns the UI into evidence an agent can judge.

```text
<run>/
  manifest.json            # url, git head, fixture, viewports, rubric version
  pages/<viewport>/<nn>-<page-id>.png
  pages/<viewport>/<nn>-<page-id>.aria.yaml
  checks.json              # mechanical findings per page and viewport
  judge/<judge-id>.json    # rubric scores written by an independent judge
  scorecard.json           # mechanical gates plus aggregated judge scores
```

The rubric lives in `apps/ui-harness/rubric/` and is versioned. A judge scores
every dimension with cited page files as evidence; scores without evidence do
not count. See `docs/ui-rubric.md` for where the rubric comes from and how to
revise it.
