import { HttpBackend, request } from "../backend/http";
import { listen } from "@tauri-apps/api/event";

import { command } from "../backend/tauri";
import type {
  Backend,
  Contributor,
  GitHubAccount,
  GitHubBackend,
  GitHubPreferences,
  SignInMethod,
  GitHubSettings,
  SignInPoll,
  SignInStart,
} from "../backend/types";
import type { Action, Snapshot } from "../protocol";
import type { Publication, Receipt } from "../publication";
import type {
  PromptReport,
  PromptView,
  RepositoryRefs,
  TaskLog,
  WorkbenchAction,
  WorkbenchSnapshot,
} from "./protocol";

/** Repositories, tasks, and each task's review. */
export interface WorkbenchBackend {
  workbench(): Promise<WorkbenchSnapshot>;
  dispatch(action: WorkbenchAction): Promise<WorkbenchSnapshot>;
  refs(repositoryId: string): Promise<RepositoryRefs>;
  /** Every prompt agents are sent, for Settings to show. */
  prompts(): Promise<PromptView[]>;
  /** Files a public issue on Gewu about a prompt; call only after the reader confirms. */
  reportPrompt(report: PromptReport): Promise<Receipt>;
  log(taskId: string, after: number): Promise<TaskLog>;
  /** The review of one task, as the review screens consume it. */
  review(taskId: string): Backend;
  /** The GitHub account publishing posts as. */
  github?: GitHubBackend;
  /** Calls `listener` when the app menu's Settings… (⌘,) is chosen; returns how to stop. */
  onOpenSettings?: (listener: () => void) => () => void;
  /** A native folder picker, where the platform has one. */
  pickDirectory?: () => Promise<string | null>;
}

const path = (id: string) => encodeURIComponent(id);

/** The event the app menu's Settings… item sends; see `settings_menu.rs`. */
const OPEN_SETTINGS = "gewu://open-settings";

export class HttpWorkbench implements WorkbenchBackend {
  github = httpGitHub;

  workbench() {
    return request<WorkbenchSnapshot>("/api/workbench");
  }

  dispatch(action: WorkbenchAction) {
    return request<WorkbenchSnapshot>("/api/workbench/dispatch", action);
  }

  refs(repositoryId: string) {
    return request<RepositoryRefs>(`/api/repositories/${path(repositoryId)}/refs`);
  }

  prompts() {
    return request<PromptView[]>("/api/prompts");
  }

  reportPrompt(report: PromptReport) {
    return request<Receipt>("/api/prompts/report", report);
  }

  log(taskId: string, after: number) {
    return request<TaskLog>(`/api/tasks/${path(taskId)}/log?after=${after}`);
  }

  review(taskId: string): Backend {
    const base = `/api/tasks/${path(taskId)}`;
    const review = new HttpBackend(`${base}/snapshot`, `${base}/dispatch`);
    return {
      snapshot: () => review.snapshot(),
      dispatch: (action: Action) => review.dispatch(action),
      publication: () => request<Publication>(`${base}/publication`),
      publish: () => request<Receipt>(`${base}/publish`, {}),
      contributors: () => request<Contributor[]>(`${base}/contributors`),
      github: httpGitHub,
    };
  }
}

/** The HTTP server is the UI sandbox: its GitHub records calls and sends nothing. */
const httpGitHub: GitHubBackend = {
  account: () => request<GitHubAccount | null>("/api/github/account"),
  startSignIn: () => request<SignInStart>("/api/github/sign-in/start", {}),
  pollSignIn: () => request<SignInPoll>("/api/github/sign-in/poll", {}),
  signIn: (token: string) => request<GitHubAccount | null>("/api/github/sign-in", { token }),
  signOut: () => request<null>("/api/github/sign-out", {}).then(() => undefined),
  settings: () => request<GitHubSettings>("/api/github/settings"),
  updateSettings: (preferences: GitHubPreferences) =>
    request<GitHubSettings>("/api/github/settings", preferences),
  chooseMethod: (method: SignInMethod) => request<GitHubSettings>("/api/github/method", { method }),
};

const tauriGitHub: GitHubBackend = {
  account: () => command<GitHubAccount | null>("github_account"),
  startSignIn: () => command<SignInStart>("github_sign_in_start"),
  pollSignIn: () => command<SignInPoll>("github_sign_in_poll"),
  signIn: (token: string) => command<GitHubAccount | null>("github_sign_in", { token }),
  signOut: () => command<null>("github_sign_out").then(() => undefined),
  settings: () => command<GitHubSettings>("github_settings"),
  updateSettings: (preferences: GitHubPreferences) =>
    command<GitHubSettings>("github_update_settings", { preferences }),
  chooseMethod: (method: SignInMethod) => command<GitHubSettings>("github_use_method", { method }),
};

export class TauriWorkbench implements WorkbenchBackend {
  github = tauriGitHub;

  workbench() {
    return command<WorkbenchSnapshot>("workbench");
  }

  dispatch(action: WorkbenchAction) {
    return command<WorkbenchSnapshot>("workbench_dispatch", { action });
  }

  refs(repositoryId: string) {
    return command<RepositoryRefs>("repository_refs", { repositoryId });
  }

  prompts() {
    return command<PromptView[]>("prompts");
  }

  reportPrompt(report: PromptReport) {
    return command<Receipt>("report_prompt", { report });
  }

  onOpenSettings(listener: () => void) {
    const stop = listen(OPEN_SETTINGS, listener);
    return () => void stop.then((unlisten) => unlisten());
  }

  log(taskId: string, after: number) {
    return command<TaskLog>("task_log", { taskId, after });
  }

  review(taskId: string): Backend {
    return {
      snapshot: () => command<Snapshot>("task_snapshot", { taskId }),
      dispatch: (action: Action) => command<Snapshot>("task_dispatch", { taskId, action }),
      publication: () => command<Publication>("publication", { taskId }),
      publish: () => command<Receipt>("publish", { taskId }),
      contributors: () => command<Contributor[]>("contributors", { taskId }),
      github: tauriGitHub,
    };
  }

  pickDirectory = () => command<string | null>("pick_directory");
}
