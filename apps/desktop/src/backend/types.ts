import type { Action, Snapshot } from "../protocol";
import type { Publication, Receipt } from "../publication";

export interface Backend {
  snapshot(): Promise<Snapshot>;
  dispatch(action: Action): Promise<Snapshot>;
  /**
   * The draft of what publishing would send. Hosts without one (a single
   * review window) leave it out, and the window builds the draft itself.
   */
  publication?(): Promise<Publication>;
  /** Sends the draft. Called only after the reviewer confirms it. */
  publish?(): Promise<Receipt>;
  /** The GitHub account publishing would post as. */
  github?: GitHubBackend;
}

/** The signed-in GitHub user. */
export interface GitHubAccount {
  login: string;
  name?: string | null;
  avatar_url: string;
  html_url: string;
}

export interface GitHubBackend {
  /** `null` when nobody is signed in. */
  account(): Promise<GitHubAccount | null>;
  /** Signs in with a pasted token and returns the account. */
  signIn(token: string): Promise<GitHubAccount | null>;
}

export class BackendError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "BackendError";
  }
}
