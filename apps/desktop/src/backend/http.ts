import type { Action, Snapshot } from "../protocol";
import { BackendError, type Backend } from "./types";

async function readError(response: Response): Promise<string> {
  try {
    const body = (await response.json()) as { error?: string };
    if (body.error) {
      return body.error;
    }
  } catch {
    // fall through
  }
  return `Request failed (${response.status})`;
}

/** GET `path`, or POST `body` to it, and parse the JSON reply. */
export async function request<T>(path: string, body?: unknown): Promise<T> {
  let response: Response;
  try {
    response = await fetch(
      path,
      body === undefined
        ? undefined
        : {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify(body),
          },
    );
  } catch {
    throw new BackendError("Gewu is not responding. Is `gewu ui serve` still running?");
  }
  if (!response.ok) {
    throw new BackendError(await readError(response));
  }
  return (await response.json()) as T;
}

/** One review over HTTP; a task's review passes its own paths. */
export class HttpBackend implements Backend {
  private readonly snapshotPath: string;
  private readonly dispatchPath: string;

  constructor(snapshotPath = "/api/snapshot", dispatchPath = "/api/dispatch") {
    this.snapshotPath = snapshotPath;
    this.dispatchPath = dispatchPath;
  }

  snapshot(): Promise<Snapshot> {
    return request<Snapshot>(this.snapshotPath);
  }

  dispatch(action: Action): Promise<Snapshot> {
    return request<Snapshot>(this.dispatchPath, action);
  }
}
