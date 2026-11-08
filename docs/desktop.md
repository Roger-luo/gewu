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
  body, anchor: { change_id, start_line, end_line } | null }   // new-side lines
{ type: "resolve_feedback", feedback_id, resolved }
{ type: "delete_feedback", feedback_id }
{ type: "retarget_feedback", feedback_id, target }
{ type: "set_decision", decision }
```

The service assigns feedback ids and fills an anchor's path, side, and patch
hash from the review input.

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
