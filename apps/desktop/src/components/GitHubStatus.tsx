import { useState } from "react";

import type { GitHubState } from "../hooks/useGitHub";
import { SignInDialog } from "./SignInDialog";

/** Bottom of the sidebar: the GitHub account publishing would post as. */
export function GitHubStatus({ github }: { github: GitHubState }) {
  const [open, setOpen] = useState(false);
  if (!github.available) {
    return null;
  }
  const { account, methods } = github;
  const viaCli = methods?.method === "gh";
  return (
    <footer className="github-status" aria-label="GitHub account">
      {account ? (
        <>
          <img src={account.avatar_url} alt="" width={28} height={28} />
          <span className="github-login" title={account.name ?? account.login}>
            {account.login}
            <span className="github-via muted">
              {viaCli ? "via GitHub CLI" : "via Gewu's app"}
            </span>
          </span>
          <button type="button" className="btn small" onClick={() => setOpen(true)}>
            Switch
          </button>
          {!viaCli && (
            <button type="button" className="btn small" onClick={() => void github.signOut()}>
              Sign out
            </button>
          )}
        </>
      ) : github.checking ? (
        <span className="muted">Checking GitHub…</span>
      ) : (
        <button type="button" className="btn small" onClick={() => setOpen(true)}>
          Sign in to GitHub
        </button>
      )}
      {github.error && !open && (
        <p className="github-error" role="status">
          {github.error}
        </p>
      )}
      {open && <SignInDialog github={github} onClose={() => setOpen(false)} />}
    </footer>
  );
}
