import type { Action, Snapshot } from "../protocol";

export interface Backend {
  snapshot(): Promise<Snapshot>;
  dispatch(action: Action): Promise<Snapshot>;
}

export class BackendError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "BackendError";
  }
}
