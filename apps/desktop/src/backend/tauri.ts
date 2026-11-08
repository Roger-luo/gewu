import { invoke } from "@tauri-apps/api/core";

import type { Action, Snapshot } from "../protocol";
import { BackendError, type Backend } from "./types";

/** Invoke a Tauri command, turning its error string into a `BackendError`. */
export async function command<T>(name: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(name, args);
  } catch (error) {
    throw new BackendError(messageOf(error));
  }
}

export class TauriBackend implements Backend {
  snapshot(): Promise<Snapshot> {
    return command<Snapshot>("snapshot");
  }

  dispatch(action: Action): Promise<Snapshot> {
    return command<Snapshot>("dispatch", { action });
  }
}

function messageOf(error: unknown): string {
  if (typeof error === "string") {
    return error;
  }
  if (error instanceof Error) {
    return error.message;
  }
  return "Tauri command failed";
}
