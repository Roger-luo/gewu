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
  /**
   * Who a note can `@`-mention: the repository's contributors. Empty when
   * signed out; hosts without GitHub leave it out.
   */
  contributors?(): Promise<Contributor[]>;
}

/** Someone who committed to the repository under review. */
export interface Contributor {
  login: string;
  avatar_url: string;
}

/** The signed-in GitHub user. */
export interface GitHubAccount {
  login: string;
  name?: string | null;
  avatar_url: string;
  html_url: string;
}

/** How a sign-in begins: a code to approve on github.com, or a pasted token. */
export type SignInStart =
  | {
      kind: "device";
      user_code: string;
      verification_uri: string;
      /** Seconds between polls. */
      interval: number;
      expires_in: number;
    }
  | { kind: "token" };

/** Where a device sign-in stands. */
export type SignInPoll =
  | { status: "pending" }
  | { status: "slow_down"; interval: number }
  | { status: "signed_in"; account: GitHubAccount }
  | { status: "denied" }
  | { status: "expired" };

/** Gewu's OAuth app, or the GitHub CLI's own login. */
export type SignInMethod = "gewu" | "gh";

/** Whether `gh` could post instead. */
export type CliState =
  | { status: "missing" }
  | { status: "signed_out" }
  | { status: "signed_in"; login: string };

export interface SignInMethods {
  method: SignInMethod;
  cli: CliState;
}

export interface GitHubBackend {
  /** `null` when nobody is signed in. */
  account(): Promise<GitHubAccount | null>;
  /** Starts a sign-in; a device code also opens github.com in the browser. */
  startSignIn(): Promise<SignInStart>;
  /** Checks once whether the device code was approved. */
  pollSignIn(): Promise<SignInPoll>;
  /** Signs in with a pasted token, when `startSignIn` asked for one. */
  signIn(token: string): Promise<GitHubAccount | null>;
  /** Forgets Gewu's token; with the CLI, goes back to Gewu's app. */
  signOut(): Promise<void>;
  /** The method in use and whether `gh` is signed in. */
  methods(): Promise<SignInMethods>;
  /** Switches method; remembered for the next run. */
  chooseMethod(method: SignInMethod): Promise<SignInMethods>;
}

export class BackendError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "BackendError";
  }
}
