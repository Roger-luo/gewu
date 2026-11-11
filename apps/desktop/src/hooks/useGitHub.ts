import { useCallback, useEffect, useState } from "react";

import type {
  GitHubAccount,
  GitHubBackend,
  GitHubPreferences,
  SignInMethod,
  GitHubSettings,
  SignInPoll,
  SignInStart,
} from "../backend/types";

export interface GitHubState {
  /** `false` when the host has no GitHub, e.g. a single review window. */
  available: boolean;
  account: GitHubAccount | null;
  /** The sign-in in use and whether `gh` could post instead. */
  settings: GitHubSettings | null;
  checking: boolean;
  error: string | null;
  /**
   * Switches sign-in method. Resolves to who that method is signed in as,
   * or `null` when switching failed (see `error`).
   */
  chooseMethod: (method: SignInMethod) => Promise<{ account: GitHubAccount | null } | null>;
  /** Saves the settings screen's GitHub section; `false` if it failed. */
  updateSettings: (preferences: GitHubPreferences) => Promise<boolean>;
  startSignIn: () => Promise<SignInStart | null>;
  /** Checks once; a signed-in result updates `account`. */
  pollSignIn: () => Promise<SignInPoll | null>;
  signIn: (token: string) => Promise<boolean>;
  signOut: () => Promise<void>;
}

interface State {
  account: GitHubAccount | null;
  settings: GitHubSettings | null;
  checking: boolean;
  error: string | null;
}

const message = (error: unknown) => (error instanceof Error ? error.message : String(error));

/** Who publishing would post as, read once when the screen opens. */
export function useGitHub(github: GitHubBackend | undefined): GitHubState {
  const [state, setState] = useState<State>({
    account: null,
    settings: null,
    checking: github !== undefined,
    error: null,
  });

  useEffect(() => {
    if (!github) {
      return;
    }
    let live = true;
    Promise.all([github.account(), github.settings()]).then(
      ([account, settings]) => live && setState({ account, settings, checking: false, error: null }),
      (err: unknown) =>
        live && setState({ account: null, settings: null, checking: false, error: message(err) }),
    );
    return () => {
      live = false;
    };
  }, [github]);

  const fail = useCallback((err: unknown) => {
    setState((current) => ({ ...current, error: message(err) }));
  }, []);

  const startSignIn = useCallback(async () => {
    if (!github) return null;
    try {
      const start = await github.startSignIn();
      setState((current) => ({ ...current, error: null }));
      return start;
    } catch (err) {
      fail(err);
      return null;
    }
  }, [github, fail]);

  const pollSignIn = useCallback(async () => {
    if (!github) return null;
    try {
      const poll = await github.pollSignIn();
      if (poll.status === "signed_in") {
        setState((current) => ({ ...current, account: poll.account, error: null }));
      }
      return poll;
    } catch (err) {
      fail(err);
      return null;
    }
  }, [github, fail]);

  const signIn = useCallback(
    async (token: string) => {
      if (!github) return false;
      try {
        const account = await github.signIn(token);
        setState((current) => ({ ...current, account, error: null }));
        return account !== null;
      } catch (err) {
        fail(err);
        return false;
      }
    },
    [github, fail],
  );

  const signOut = useCallback(async () => {
    if (!github) return;
    try {
      await github.signOut();
      const [account, settings] = await Promise.all([github.account(), github.settings()]);
      setState({ account, settings, checking: false, error: null });
    } catch (err) {
      fail(err);
    }
  }, [github, fail]);

  const chooseMethod = useCallback(
    async (method: SignInMethod) => {
      if (!github) return null;
      try {
        const settings = await github.chooseMethod(method);
        const account = await github.account();
        setState({ account, settings, checking: false, error: null });
        return { account };
      } catch (err) {
        fail(err);
        return null;
      }
    },
    [github, fail],
  );

  const updateSettings = useCallback(
    async (preferences: GitHubPreferences) => {
      if (!github) return false;
      try {
        const settings = await github.updateSettings(preferences);
        const account = await github.account();
        setState({ account, settings, checking: false, error: null });
        return true;
      } catch (err) {
        fail(err);
        return false;
      }
    },
    [github, fail],
  );

  return {
    available: github !== undefined,
    updateSettings,
    ...state,
    chooseMethod,
    startSignIn,
    pollSignIn,
    signIn,
    signOut,
  };
}
