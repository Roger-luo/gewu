/** Workbench protocol (docs/workbench.md). Field names match the JSON. */

export type Source =
  | { type: "commit_range"; base: string; head: string }
  | { type: "working_tree"; base: string }
  | { type: "pull_request"; number: number; remote: string }
  | { type: "directory"; path: string; ref: string };

export type SourceType = Source["type"];

export interface AgentChoice {
  preset_id: string;
  command: string | null;
}

export interface AgentPreset {
  id: string;
  label: string;
  kind: "none" | "acp";
  command: string;
  available: boolean;
  detail: string | null;
}

export interface RepositoryView {
  id: string;
  name: string;
  path: string;
  branch: string | null;
  dirty: boolean;
  error: string | null;
}

export type TaskStatus =
  | "generating"
  | "awaiting_reviewer"
  | "complete"
  | "failed"
  | "cancelled";

export interface ReviewProgress {
  reviewed: number;
  total: number;
  open_issues: number;
  remaining_work_items: number | null;
}

export interface TaskView {
  id: string;
  repository_id: string;
  repository_name: string;
  title: string;
  source: Source;
  agent: AgentChoice;
  agent_label: string;
  status: TaskStatus;
  stage: string | null;
  message: string | null;
  review: ReviewProgress | null;
  /** Unix milliseconds. */
  created_at: number;
  updated_at: number;
}

export interface WorkbenchSnapshot {
  repositories: RepositoryView[];
  tasks: TaskView[];
  agents: AgentPreset[];
}

export interface CommitSummary {
  sha: string;
  subject: string;
  author: string;
  date: string;
}

export interface PullRequestSummary {
  number: number;
  title: string;
  author: string;
  head_ref: string;
}

export interface RepositoryRefs {
  branch: string | null;
  dirty: boolean;
  commits: CommitSummary[];
  branches: string[];
  pull_requests: PullRequestSummary[] | null;
  directories: string[];
}

export type LogKind = "output" | "tool" | "status" | "denied" | "error";

export interface LogEvent {
  seq: number;
  at: number;
  kind: LogKind;
  text: string;
}

export interface TaskLog {
  events: LogEvent[];
  next: number;
}

export type WorkbenchAction =
  | { type: "add_repository"; path: string }
  | { type: "remove_repository"; repository_id: string }
  | {
      type: "create_task";
      repository_id: string;
      source: Source;
      agent: AgentChoice;
      title: string | null;
    }
  | { type: "continue_task"; task_id: string }
  | { type: "cancel_task"; task_id: string }
  | { type: "delete_task"; task_id: string };

export type WorkbenchScreen = "home" | "new-review" | "task" | "review";

export type WorkbenchRoute =
  | { screen: "home" }
  | { screen: "new-review"; repository_id?: string }
  | { screen: "task"; task_id: string }
  | { screen: "review"; task_id: string };
