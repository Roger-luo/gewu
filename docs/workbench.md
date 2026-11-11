# Workbench: the full desktop workflow

The desktop app must let a reviewer do the whole job without a terminal:
choose a repository, choose what to review, have an agent write the
walkthrough, read it, give feedback, ask for the next batch, and move between
several reviews in flight. `docs/desktop.md` covers the review screens; this
document covers everything around them.

## User stories

Each story is also a harness journey (see "Journeys" below). A story the UI
cannot complete is a release blocker, not a polish issue.

1. **Simple diff.** Open the app, add a repository, review the last commit
   with no agent (raw diff in file order), read it, mark chunks reviewed,
   record a decision.
2. **Uncommitted work.** Review the working tree against `HEAD` with an agent;
   watch it run; read the first batch when it is ready.
3. **Pull request.** Pick an open pull request from a list (or type its
   number), pick an agent, start, and come back later.
4. **Directory.** Review everything inside one directory at a ref, as if it
   were newly added.
5. **Many reviews.** Two or more reviews exist, across one or more
   repositories. The reviewer sees each one's state at a glance, switches
   between them without losing their place, asks one for its next batch while
   reading another, cancels or deletes one.
6. **Failure.** The chosen agent is not installed, or it exits early, or the
   range is empty. The UI says what happened and what to do, in the UI.

## Concepts

- **Repository**: a local Git checkout the reviewer added. Identified by a
  stable id; stores its absolute path.
- **Source**: what to review.
  - `commit_range { base, head }`
  - `working_tree { base }`: uncommitted changes against `base`.
  - `pull_request { number, remote }`: resolved to merge base and head.
  - `directory { path, ref }`: every file under `path` at `ref`, reviewed as
    added against the empty tree.
- **Agent**: what writes the walkthrough. One of the presets below, or none.
- **Task**: one review of one source in one repository, by one agent. A task
  owns a working directory holding its `.gewu/` state.

## Task isolation

The harness keeps its state in `<root>/.gewu/` and the agent runs with
`root` as its working directory. Two tasks in one repository therefore need
two roots.

- `commit_range`, `pull_request`, `directory`: the task root is a detached
  Git worktree at the source's head, under
  `<gewu_home>/tasks/<task_id>/checkout`. The agent reads exactly the code
  under review, and deleting the task removes the worktree.
- `working_tree`: the task root is the repository itself, because the changes
  live only there. At most one working-tree task per repository.

`<gewu_home>` is `$GEWU_HOME` when set, else `$HOME/.gewu`.

## Agents

Gewu is an ACP client. It uses the official Rust SDK,
[`agent-client-protocol`](https://crates.io/crates/agent-client-protocol)
(the same crate Zed uses), and spawns whatever command the preset names.
The SDK owns JSON-RPC, sessions, and the update stream. Gewu implements
only the client handler: which file reads, writes, and shell commands to
allow. It does not parse each CLI's private `stream-json`.

`autotune-agent` wrote those private parsers because its VM consumes a
schema-validated final message (`AgentResponse`) and journals the exact
request. Gewu does not. The structured artifact is files under
`.gewu/harness/` and `.gewu/review-spec.json`, written by the agent through
the `gewu harness` commands and validated by `gewu-review`. The brief in
`harness/prompts/agent.md` already says that. ACP `session/update` events
are the task log, not a second schema.

| Preset id | Label | Command |
|---|---|---|
| `none` | No agent (raw diff, file order) | No process. Writes the file-ordered fallback spec. |
| `claude` | Claude Code | `npx -y @zed-industries/claude-code-acp` |
| `codex` | Codex | `npx -y @zed-industries/codex-acp` |
| `cursor` | Cursor Agent | `agent acp` |
| `gemini` | Gemini CLI | `gemini --acp` |
| `opencode` | OpenCode | `opencode acp` |
| `custom` | Custom ACP command | A command line the reviewer types |

Availability is the program on `PATH` (`npx` for the two Node adapters).
The working directory of `session/new` is the task root. Permission policy,
applied in the client handler:

- `fs/read_text_file`: any path inside the task root.
- `fs/write_text_file`: only under `<root>/.gewu/harness/`.
- `terminal/create`: only this Gewu binary's `harness` and `review`
  subcommands, with `cwd` = task root.
- anything else: reject, and record the denial.

After every batch Gewu hashes the tree outside `.gewu/` and fails the task
if the agent changed it. Cancel sends `session/cancel`, then kills the
process. A batch that stops before the harness is waiting or complete fails
with the agent's last message.

The SDK's MSRV is 1.88, so the workspace `rust-version` moves from 1.85 to
1.88 when this crate is added. Speak stable protocol v1. Draft v2 is what
T3 Code prefers for usage and plans; Gewu can take it later without a new
client.

## Task lifecycle

```text
created ──▶ generating ──▶ awaiting_reviewer ──(continue)──▶ generating
                │   ▲                │
                │   └──── failed ◀───┤
                ▼                    ▼
            cancelled             complete
```

- `generating`: an agent batch (or the instant `none` preset) is running.
  `stage` names the harness stage (`investigate`, `plan`, `expand`, …).
  Within one batch, after the author writes its chunks, a fresh agent session
  scores them against the proofread rubric (`proofread`) and failing chunks
  are rewritten (`polish`), at most two times, before the batch reaches the
  reviewer.
- `awaiting_reviewer`: a batch is ready; the review can be opened. The
  reviewer asks for the next batch with `continue_task`.
- `complete`: the walkthrough is complete.
- `failed`: `message` says why and what to try.
- `cancelled`: stopped by the reviewer; `continue_task` resumes.

A review can be opened in every state where a validated partial spec exists,
not only `awaiting_reviewer`.

## Protocol

Same shapes over HTTP and Tauri IPC. JSON fields are `snake_case`.

```text
WorkbenchSnapshot {
  repositories: RepositoryView[],
  tasks: TaskView[],              // newest first
  agents: AgentPreset[],
}

RepositoryView { id, name, path, branch: string | null, dirty: bool, error: string | null }

AgentPreset {
  id, label,
  kind: "none" | "acp",
  command: string,                // what will run, shown to the reviewer
  available: bool,
  detail: string | null,          // e.g. "not found on PATH"
}

Source =
  | { type: "commit_range", base, head }
  | { type: "working_tree", base }
  | { type: "pull_request", number, remote }   // remote is required
  | { type: "directory", path, ref }

AgentChoice { preset_id, command: string | null }  // command for custom

TaskView {
  id, repository_id, repository_name, title,
  source: Source, agent: AgentChoice, agent_label: string,
  status: "generating" | "awaiting_reviewer" | "complete" | "failed" | "cancelled",
  stage: string | null,
  message: string | null,          // last status line or failure reason
  review: { reviewed, total, open_issues, remaining_work_items: number | null } | null,
  created_at, updated_at,          // unix milliseconds
}

RepositoryRefs {
  branch: string | null, dirty: bool,
  commits: { sha, subject, author, date }[],         // recent, newest first
  branches: string[],
  pull_requests: { number, title, author, head_ref }[] | null,  // null when `gh` is unavailable
  directories: string[],                              // tracked dirs, depth ≤ 2
}

TaskLog { events: { seq, at, kind: "output" | "tool" | "status" | "denied" | "error", text }[], next: number }

WorkbenchAction =
  | { type: "add_repository", path }
  | { type: "remove_repository", repository_id }
  | { type: "create_task", repository_id, source, agent, title: string | null }
  | { type: "continue_task", task_id }
  | { type: "cancel_task", task_id }
  | { type: "delete_task", task_id }
```

`create_task` and `continue_task` return immediately with the task in
`generating`; the batch runs in the background and the UI polls.
`remove_repository` deletes the repository's reviews first, as `delete_task`
would; the repository's own files are untouched. When the reviewer marks the
last step of a walkthrough the agent has finished, the task becomes `complete`
without another `continue_task`.

### HTTP

| Method | Path | Body → Response |
|---|---|---|
| GET | `/api/health` | → `{ ok: true, mode: "review" \| "workbench" }` |
| GET | `/api/workbench` | → `WorkbenchSnapshot` |
| POST | `/api/workbench/dispatch` | `WorkbenchAction` → `WorkbenchSnapshot` |
| GET | `/api/repositories/<id>/refs` | → `RepositoryRefs` |
| GET | `/api/tasks/<id>/log?after=<seq>` | → `TaskLog` |
| GET | `/api/tasks/<id>/snapshot` | → review `Snapshot` |
| POST | `/api/tasks/<id>/dispatch` | review `Action` → `Snapshot` |

`gewu ui serve` without `--root` serves the workbench. With `--root`, the
legacy single-review endpoints (`/api/snapshot`, `/api/dispatch`) keep
working, so the existing fixtures and harness runs are unchanged.

### Tauri

Commands `mode` (→ `{ mode: "review" \| "workbench" }`), `workbench`,
`workbench_dispatch { action }`, `repository_refs { repository_id }`,
`task_log { task_id, after }`, `task_snapshot { task_id }`,
`task_dispatch { task_id, action }`, plus `pick_directory` (native folder
dialog) for adding a repository.

## UI

- **Sidebar**, top to bottom: the two ways to start, **New review** with
  **Add repository** under it; the repositories to filter by;
  and at its foot the "Account and settings" region: who publishing posts as,
  then **Settings** (gear icon) beside **Switch** or **Sign in to GitHub**.
  Sign out is in the account dialog **Switch** opens. The review navigator
  ends with the same region.
- **Home** (`main[data-gewu-screen="home"]`): repositories in a sidebar;
  tasks as a list with title, source, agent, status, stage, review progress,
  and updated time. Status labels line up in one column whether or not a row
  has an action button. Primary action "New review". Empty state explains the
  first step.
- **New review** (`data-gewu-screen="new-review"`): repository, source type
  (tabs: Commit range, Uncommitted changes, Pull request, Directory) with
  pickers fed by `RepositoryRefs`, agent picker showing availability and the
  exact command, optional title, Start.
- **Settings** (`data-gewu-screen="settings"`): GitHub, agents and
  generation, and where Gewu keeps things; see `docs/desktop.md`.
- **Task** (`data-gewu-screen="task"`, `data-gewu-task-id`): status, stage,
  live log, and actions: Open review, Continue (next batch), Cancel, Delete.
- **Review**: the existing overview, chunk, and summary screens, scoped to one
  task, with a way back to Home and to the task. The navigator shows which
  task is open.

Test hook additions: `Target` gains `{ screen: "home" }`,
`{ screen: "new-review" }`, `{ screen: "settings" }`, and
`{ screen: "task", task_id }`; `state()`
reports `task_id`. The app starts on Home when it is served without
`--root`, and on the review overview in the legacy single-review mode.

### Control names

Journeys drive the UI by role and accessible name only, so these names are
part of the contract:

| Where | Role | Name |
|---|---|---|
| Home | button | `New review` |
| Sidebar | button | `Add repository` (under `New review`) |
| Sidebar | region | `Account and settings`, holding `Settings` and `Switch` or `Sign in to GitHub` |
| Add repository | textbox | `Repository path` |
| Add repository | button | `Add` (and `Choose folder…` under Tauri) |
| Home | list | `Reviews`; each item is a link or button named by the task title |
| New review | combobox | `Repository` |
| New review | tab | `Commit range`, `Uncommitted changes`, `Pull request`, `Directory` |
| Commit range | combobox / textbox | `Base`, `Head` |
| Uncommitted changes | textbox | `Base` |
| Pull request | listbox `Open pull requests` and textbox `Pull request number` |
| Directory | combobox `Directory`, textbox `Ref` |
| New review | radiogroup | `Agent`; each radio named by the preset label |
| New review | textbox | `Title` (optional) |
| New review | button | `Start review` |
| Task | button | `Open review`, `Continue`, `Cancel`, `Delete…` |
| Task | log | `Agent log` |
| Review, Task | button | `Back to reviews` |

## Journeys

The UI harness runs each user story as a scripted journey against a
throwaway Git repository and the `none` preset (plus a fake ACP agent
that speaks protocol v1 and writes only under `.gewu/harness/`), capturing
each step. The rubric scores journeys,
not only pages: a step the script cannot perform through visible controls is
a `journey.blocked` finding and fails the run.
