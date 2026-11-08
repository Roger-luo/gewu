import { HttpBackend, request } from "../backend/http";
import { command } from "../backend/tauri";
import type { Backend } from "../backend/types";
import type { Action, Snapshot } from "../protocol";
import type {
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
  log(taskId: string, after: number): Promise<TaskLog>;
  /** The review of one task, as the review screens consume it. */
  review(taskId: string): Backend;
  /** A native folder picker, where the platform has one. */
  pickDirectory?: () => Promise<string | null>;
}

const path = (id: string) => encodeURIComponent(id);

export class HttpWorkbench implements WorkbenchBackend {
  workbench() {
    return request<WorkbenchSnapshot>("/api/workbench");
  }

  dispatch(action: WorkbenchAction) {
    return request<WorkbenchSnapshot>("/api/workbench/dispatch", action);
  }

  refs(repositoryId: string) {
    return request<RepositoryRefs>(`/api/repositories/${path(repositoryId)}/refs`);
  }

  log(taskId: string, after: number) {
    return request<TaskLog>(`/api/tasks/${path(taskId)}/log?after=${after}`);
  }

  review(taskId: string): Backend {
    const base = `/api/tasks/${path(taskId)}`;
    return new HttpBackend(`${base}/snapshot`, `${base}/dispatch`);
  }
}

export class TauriWorkbench implements WorkbenchBackend {
  workbench() {
    return command<WorkbenchSnapshot>("workbench");
  }

  dispatch(action: WorkbenchAction) {
    return command<WorkbenchSnapshot>("workbench_dispatch", { action });
  }

  refs(repositoryId: string) {
    return command<RepositoryRefs>("repository_refs", { repositoryId });
  }

  log(taskId: string, after: number) {
    return command<TaskLog>("task_log", { taskId, after });
  }

  review(taskId: string): Backend {
    return {
      snapshot: () => command<Snapshot>("task_snapshot", { taskId }),
      dispatch: (action: Action) => command<Snapshot>("task_dispatch", { taskId, action }),
    };
  }

  pickDirectory = () => command<string | null>("pick_directory");
}
