import { BackendError } from "../backend/types";
import { MockBackend } from "../backend/mock";
import { mockGitHub } from "../backend/mockGitHub";
import { selfReviewSnapshot, showcaseSnapshot } from "../mock";
import { reviewVenue } from "../publication";
import type { Snapshot } from "../protocol";
import type { WorkbenchBackend } from "./backend";
import type {
  AgentPreset,
  LogEvent,
  RepositoryRefs,
  RepositoryView,
  Settings,
  Source,
  TaskView,
  WorkbenchAction,
  WorkbenchSnapshot,
} from "./protocol";
import { AGENTS, REFS } from "./mock-data";
import { sourceTitle } from "./format";

/** One simulated task: its view, its review, and the batches still to come. */
interface MockTask {
  view: TaskView;
  review: MockBackend | null;
  pending: Snapshot["chunks"];
  log: LogEvent[];
}

/** An in-memory workbench whose agents finish after `delayMs`. */
export class MockWorkbench implements WorkbenchBackend {
  readonly github = mockGitHub;
  private repositories: RepositoryView[] = [];
  private tasks: MockTask[] = [];
  private clock = Date.parse("2026-11-07T09:00:00Z");
  private sequence = 0;

  private readonly delayMs: number;

  constructor(delayMs = 900) {
    this.delayMs = delayMs;
  }

  private settings: Settings = { proofread_rounds: 2, default_agent: null };

  /** Two repositories and reviews in each state a reviewer meets. */
  static seeded(delayMs?: number): MockWorkbench {
    const workbench = new MockWorkbench(delayMs);
    workbench.addRepository("/Users/ada/Code/gewu");
    workbench.addRepository("/Users/ada/Code/notes");
    const [gewu, notes] = workbench.repositories;
    workbench.seed(gewu, { type: "pull_request", number: 42, remote: "origin" }, "claude", "awaiting_reviewer");
    workbench.seed(gewu, { type: "commit_range", base: "9b1d03aa", head: "4f2a9c1e" }, "none", "complete");
    workbench.seed(notes, { type: "working_tree", base: "HEAD" }, "codex", "failed");
    return workbench;
  }

  async workbench(): Promise<WorkbenchSnapshot> {
    const tasks = await Promise.all(this.tasks.map((task) => this.view(task)));
    return structuredClone({
      repositories: this.repositories,
      tasks,
      agents: AGENTS,
      settings: {
        ...this.settings,
        max_proofread_rounds: 5,
        home: "/Users/ada/.gewu",
        gewu: "/Users/ada/.cargo/bin/gewu",
      },
    });
  }

  async dispatch(action: WorkbenchAction): Promise<WorkbenchSnapshot> {
    switch (action.type) {
      case "add_repository":
        this.addRepository(action.path);
        break;
      case "remove_repository":
        this.repository(action.repository_id);
        this.tasks = this.tasks.filter((task) => task.view.repository_id !== action.repository_id);
        this.repositories = this.repositories.filter((repo) => repo.id !== action.repository_id);
        break;
      case "create_task":
        this.create(action);
        break;
      case "continue_task": {
        const task = this.task(action.task_id);
        if (task.view.status === "generating") {
          throw new BackendError("a batch is already running");
        }
        const progress = task.review ? (await task.review.snapshot()).summary : null;
        if (progress && progress.reviewed < progress.total) {
          const left = progress.total - progress.reviewed;
          this.finish(
            task,
            "awaiting_reviewer",
            left === 1
              ? "1 step is still unreviewed. Mark it reviewed, then continue."
              : `${left} steps are still unreviewed. Mark them reviewed, then continue.`,
          );
          break;
        }
        this.generate(task);
        break;
      }
      case "cancel_task": {
        const task = this.task(action.task_id);
        if (task.view.status !== "generating") {
          throw new BackendError("no batch is running");
        }
        this.finish(task, "cancelled", "Cancelled. Continue to resume.");
        break;
      }
      case "delete_task":
        this.task(action.task_id);
        this.tasks = this.tasks.filter((task) => task.view.id !== action.task_id);
        break;
      case "update_settings": {
        const { proofread_rounds, default_agent } = action.settings;
        if (proofread_rounds < 0 || proofread_rounds > 5) {
          throw new BackendError("proofread rounds must be between 0 and 5");
        }
        if (default_agent?.preset_id === "custom" && !default_agent.command?.trim()) {
          throw new BackendError("a custom default agent needs the command that starts it");
        }
        this.settings = structuredClone(action.settings);
        break;
      }
    }
    return this.workbench();
  }

  async refs(repositoryId: string): Promise<RepositoryRefs> {
    this.repository(repositoryId);
    return structuredClone(REFS);
  }

  async log(taskId: string, after: number) {
    const task = this.task(taskId);
    const events = task.log.filter((event) => event.seq >= after);
    return { events: structuredClone(events), next: task.log.length };
  }

  review(taskId: string) {
    const task = this.task(taskId);
    if (!task.review) {
      throw new BackendError("no walkthrough has been written yet");
    }
    return task.review;
  }

  /** A review that publishes to `ada/<repository>`, as its GitHub remote would say. */
  private reviewFor(task: MockTask, snapshot: Snapshot): MockBackend {
    const venue = reviewVenue(task.view.source, `ada/${task.view.repository_name}`, snapshot);
    return new MockBackend(snapshot, { venue, title: task.view.title });
  }

  private addRepository(path: string) {
    const trimmed = path.trim().replace(/\/+$/, "");
    if (!trimmed.startsWith("/") && !trimmed.startsWith("~")) {
      throw new BackendError(`\`${path}\` is not a Git repository: give an absolute path`);
    }
    const id = `repo-${trimmed.replace(/[^a-z0-9]/gi, "").slice(-12)}`;
    if (this.repositories.some((repo) => repo.id === id)) {
      return;
    }
    const name = trimmed.split("/").pop() ?? trimmed;
    this.repositories.push({ id, name, path: trimmed, branch: "main", dirty: name === "gewu", error: null });
  }

  private create(action: Extract<WorkbenchAction, { type: "create_task" }>) {
    const repository = this.repository(action.repository_id);
    const preset = AGENTS.find((agent) => agent.id === action.agent.preset_id);
    if (!preset) {
      throw new BackendError(`unknown agent \`${action.agent.preset_id}\``);
    }
    if (preset.id === "custom" && !action.agent.command?.trim()) {
      throw new BackendError("enter the command that starts the agent");
    }
    if (
      action.source.type === "working_tree" &&
      this.tasks.some(
        (task) => task.view.repository_id === repository.id && task.view.source.type === "working_tree",
      )
    ) {
      throw new BackendError(
        `${repository.name} already has a review of its uncommitted changes; open or delete it first`,
      );
    }
    const task = this.add(repository, action.source, preset, action.title);
    if (preset.kind === "none") {
      task.review = this.reviewFor(task, showcaseSnapshot());
      task.review.append([], { remaining_work_items: 0, complete: true });
      this.record(task, "status", "Raw diff in file order; no agent wrote a walkthrough.");
      this.finish(task, "complete", "Raw diff in file order; no agent wrote a walkthrough.");
      return;
    }
    this.generate(task, preset);
  }

  private add(repository: RepositoryView, source: Source, preset: AgentPreset, title: string | null): MockTask {
    this.clock += 60_000;
    const view: TaskView = {
      id: `task-${(this.sequence++).toString(16)}`,
      repository_id: repository.id,
      repository_name: repository.name,
      title: title?.trim() || sourceTitle(source),
      source,
      agent: { preset_id: preset.id, command: null },
      agent_label: preset.label,
      status: "generating",
      stage: null,
      message: "Preparing the review",
      review: null,
      created_at: this.clock,
      updated_at: this.clock,
    };
    const task: MockTask = { view, review: null, pending: [], log: [] };
    this.tasks.unshift(task);
    this.record(task, "status", `Preparing ${sourceTitle(source)}`);
    return task;
  }

  private generate(task: MockTask, preset?: AgentPreset) {
    task.view.status = "generating";
    task.view.stage = task.review ? "expand" : "investigate";
    task.view.message = "The agent is writing the next batch";
    if (preset && !preset.available) {
      this.finish(
        task,
        "failed",
        `Could not start \`${preset.command.split(" ")[0]}\`: not found on PATH. Install it, or start a new review with a different agent.`,
      );
      return;
    }
    this.record(task, "status", `Starting ${preset?.command ?? "the agent"}`);
    this.record(task, "tool", "gewu harness status");
    this.record(task, "output", "Reading the changes and planning one step per concern.");
    setTimeout(() => this.batch(task), this.delayMs);
  }

  private batch(task: MockTask) {
    if (task.view.status !== "generating" || !this.tasks.includes(task)) {
      return;
    }
    if (!task.review) {
      const full = showcaseSnapshot();
      task.pending = full.chunks.slice(2);
      full.chunks = full.chunks.slice(0, 2);
      full.generation = { remaining_work_items: task.pending.length, complete: false };
      task.review = this.reviewFor(task, full);
    } else {
      task.review.append(task.pending, { remaining_work_items: 0, complete: true });
      task.pending = [];
    }
    this.record(task, "tool", "Write .gewu/harness/expansion.json");
    this.record(task, "tool", "gewu harness next");
    if (task.pending.length === 0) {
      this.finish(task, "complete", "The walkthrough is complete.");
    } else {
      this.finish(task, "awaiting_reviewer", "A batch is ready to review. Continue when you have read it.");
    }
  }

  private finish(task: MockTask, status: TaskView["status"], message: string) {
    this.clock += 30_000;
    task.view.status = status;
    task.view.stage = status === "complete" ? "complete" : status === "awaiting_reviewer" ? "await" : task.view.stage;
    task.view.message = message;
    task.view.updated_at = this.clock;
    this.record(task, status === "failed" ? "error" : "status", message);
  }

  private seed(repository: RepositoryView, source: Source, presetId: string, status: TaskView["status"]) {
    const preset = AGENTS.find((agent) => agent.id === presetId) ?? AGENTS[0];
    const task = this.add(repository, source, preset, null);
    if (status === "failed") {
      this.record(task, "status", `Starting ${preset.command}`);
      this.record(task, "denied", "`git stash` is not a Gewu command; only `gewu harness` and read-only `gewu review` are allowed");
      this.finish(task, "failed", "The agent stopped at investigate before the reviewer gate. It was refused: `git stash` is not a Gewu command.");
      return;
    }
    const full = status === "complete" ? selfReviewSnapshot() : showcaseSnapshot();
    if (status === "awaiting_reviewer") {
      task.pending = full.chunks.slice(2);
      full.chunks = full.chunks.slice(0, 2);
      full.generation = { remaining_work_items: task.pending.length, complete: false };
    }
    task.review = this.reviewFor(task, full);
    this.finish(
      task,
      status,
      status === "complete" ? "Raw diff in file order; no agent wrote a walkthrough." : "A batch is ready to review. Continue when you have read it.",
    );
  }

  private record(task: MockTask, kind: LogEvent["kind"], text: string) {
    task.log.push({ seq: task.log.length, at: this.clock, kind, text });
  }

  private async view(task: MockTask): Promise<TaskView> {
    if (!task.review) {
      return task.view;
    }
    const { summary, generation } = await task.review.snapshot();
    return {
      ...task.view,
      review: {
        reviewed: summary.reviewed,
        total: summary.total,
        open_issues: summary.unresolved_issues.length,
        remaining_work_items: generation.remaining_work_items,
        decision: summary.decision,
      },
    };
  }

  private task(id: string): MockTask {
    const task = this.tasks.find((item) => item.view.id === id);
    if (!task) {
      throw new BackendError(`task \`${id}\` not found`);
    }
    return task;
  }

  private repository(id: string): RepositoryView {
    const repository = this.repositories.find((repo) => repo.id === id);
    if (!repository) {
      throw new BackendError(`repository \`${id}\` not found`);
    }
    return repository;
  }
}
