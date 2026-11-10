import type { Decision } from "../protocol";
import type { Source, SourceType, TaskStatus, TaskView } from "./protocol";

/** The title the backend gives a task when the reviewer gives none. */
export function sourceTitle(source: Source): string {
  switch (source.type) {
    case "commit_range":
      return `${source.base}..${source.head}`;
    case "working_tree":
      return `Uncommitted changes against ${source.base}`;
    case "pull_request":
      return `Pull request #${source.number}`;
    case "directory":
      return `${source.path.replace(/^\/+|\/+$/g, "") || "/"} at ${source.ref}`;
  }
}

export const SOURCE_LABELS: Record<SourceType, string> = {
  commit_range: "Commit range",
  working_tree: "Uncommitted changes",
  pull_request: "Pull request",
  directory: "Directory",
};

/** What the source covers, in words, for a task row. */
export function sourceSummary(source: Source): string {
  switch (source.type) {
    case "commit_range":
      return `Commits ${short(source.base)}..${short(source.head)}`;
    case "working_tree":
      return `Uncommitted changes against ${source.base}`;
    case "pull_request":
      return `Pull request #${source.number}`;
    case "directory":
      return `Directory ${source.path || "/"} at ${short(source.ref)}`;
  }
}

function short(revision: string): string {
  return /^[0-9a-f]{12,}$/.test(revision) ? revision.slice(0, 8) : revision;
}

export const STATUS_LABELS: Record<TaskStatus, string> = {
  generating: "Generating",
  awaiting_reviewer: "Ready to read",
  complete: "Walkthrough ready",
  failed: "Failed",
  cancelled: "Cancelled",
};

/** "Complete" only when every written step is reviewed; else generation finished first. */
export function statusLabel(task: TaskView): string {
  if (task.status !== "complete") {
    return STATUS_LABELS[task.status];
  }
  if (task.review && task.review.total > 0 && task.review.reviewed >= task.review.total) {
    return "Complete";
  }
  if (task.agent.preset_id === "none") {
    return "Raw diff ready";
  }
  return "Walkthrough ready";
}

/** Progress through what exists so far, e.g. "2 of 5 steps reviewed". */
/** Harness stages as a reviewer would describe the agent's work. */
const STAGES: Record<string, string> = {
  investigate: "Reading the changes",
  plan: "Planning the steps",
  expand: "Writing steps",
  checkpoint: "Checking the batch",
  rewrite: "Revising a step",
  proofread: "Proofreading the batch",
  polish: "Improving weak steps",
};

/** What the agent is doing now, or `null` for a stage with nothing to say. */
export function stageLabel(stage: string | null): string | null {
  return stage ? (STAGES[stage] ?? null) : null;
}

const DECIDED: Record<Decision, string> = {
  approve: "you approved",
  comment: "you commented",
  request_changes: "you requested changes",
};

export function progressLabel(task: TaskView): string | null {
  const review = task.review;
  if (!review) {
    return null;
  }
  let label = `${review.reviewed} of ${review.total} ${review.total === 1 ? "step" : "steps"} reviewed`;
  if (review.remaining_work_items && task.status !== "complete") {
    label += ` · ${review.remaining_work_items} still to write`;
  }
  if (review.open_issues > 0) {
    label += ` · ${review.open_issues} open change request${review.open_issues === 1 ? "" : "s"}`;
  }
  if (review.decision) {
    label += ` · ${DECIDED[review.decision]}`;
  }
  return label;
}

/** A commit's ISO date as "Nov 7, 2026". */
export function shortDate(iso: string): string {
  const at = Date.parse(iso);
  return Number.isNaN(at)
    ? iso
    : new Date(at).toLocaleDateString(undefined, { month: "short", day: "numeric", year: "numeric" });
}

/** "just now", "5 min ago", "3 h ago", else the date. */
export function relativeTime(at: number, now = Date.now()): string {
  const seconds = Math.max(0, Math.round((now - at) / 1000));
  if (seconds < 45) {
    return "just now";
  }
  const minutes = Math.round(seconds / 60);
  if (minutes < 60) {
    return `${minutes} min ago`;
  }
  const hours = Math.round(minutes / 60);
  if (hours < 24) {
    return `${hours} h ago`;
  }
  return new Date(at).toLocaleDateString(undefined, { month: "short", day: "numeric" });
}

/** Whether Continue belongs on screen (enabled or held). */
export function canContinue(task: TaskView): boolean {
  if (task.status === "failed" || task.status === "cancelled") {
    return true;
  }
  // The last batch is in; finishing those steps closes the walkthrough — no Continue.
  if (task.status !== "awaiting_reviewer") {
    return false;
  }
  return (task.review?.remaining_work_items ?? 0) > 0;
}

/** Held until every written step is marked reviewed. */
export function continueHeld(
  task: TaskView,
  progress?: { reviewed: number; total: number } | null,
): boolean {
  if (!canContinue(task) || task.status !== "awaiting_reviewer") {
    return false;
  }
  const review = progress ?? task.review;
  return review != null && review.reviewed < review.total;
}
