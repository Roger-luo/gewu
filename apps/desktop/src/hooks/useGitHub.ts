import { useCallback, useEffect, useState } from "react";

import type { GitHubAccount, GitHubBackend } from "../backend/types";

export interface GitHubState {
  /** `false` when the host has no GitHub, e.g. a single review window. */
  available: boolean;
  account: GitHubAccount | null;
  checking: boolean;
  error: string | null;
  signIn: (token: string) => Promise<boolean>;
}

const message = (error: unknown) => (error instanceof Error ? error.message : String(error));

/** Who publishing would post as, read once when the review opens. */
export function useGitHub(github: GitHubBackend | undefined): GitHubState {
  const [state, setState] = useState<{
    account: GitHubAccount | null;
    checking: boolean;
    error: string | null;
  }>({ account: null, checking: github !== undefined, error: null });

  useEffect(() => {
    if (!github) {
      return;
    }
    let live = true;
    github.account().then(
      (account) => live && setState({ account, checking: false, error: null }),
      (err: unknown) => live && setState({ account: null, checking: false, error: message(err) }),
    );
    return () => {
      live = false;
    };
  }, [github]);

  const signIn = useCallback(
    async (token: string) => {
      if (!github) {
        return false;
      }
      try {
        const account = await github.signIn(token);
        setState({ account, checking: false, error: null });
        return account !== null;
      } catch (err) {
        setState((current) => ({ ...current, error: message(err) }));
        return false;
      }
    },
    [github],
  );

  return { available: github !== undefined, ...state, signIn };
}
