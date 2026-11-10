import { useState } from "react";

import type { GitHubState } from "../hooks/useGitHub";
import { Dialog } from "../workbench/Dialog";

/** Bottom of the sidebar: the GitHub account publishing would post as. */
export function GitHubStatus({ github }: { github: GitHubState }) {
  const [open, setOpen] = useState(false);
  if (!github.available) {
    return null;
  }
  const { account } = github;
  return (
    <footer className="github-status" aria-label="GitHub account">
      {account ? (
        <>
          <img src={account.avatar_url} alt="" width={28} height={28} />
          <span className="github-login" title={account.name ?? account.login}>
            {account.login}
          </span>
          <button type="button" className="btn small" onClick={() => setOpen(true)}>
            Switch
          </button>
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

function SignInDialog({ github, onClose }: { github: GitHubState; onClose: () => void }) {
  const [token, setToken] = useState("");
  const [busy, setBusy] = useState(false);
  const submit = async () => {
    setBusy(true);
    const signedIn = await github.signIn(token.trim());
    setBusy(false);
    if (signedIn) {
      onClose();
    }
  };
  return (
    <Dialog title="Sign in to GitHub" onClose={onClose}>
      <p className="muted">
        Paste a GitHub token that can write issues and pull-request reviews. Gewu
        hands it to the GitHub CLI (<code>gh auth login --with-token</code>), which
        keeps it. Gewu does not store it. Signing in posts nothing.
      </p>
      <label>
        <span>GitHub token</span>
        <input
          type="password"
          autoComplete="off"
          value={token}
          onChange={(event) => setToken(event.target.value)}
        />
      </label>
      {github.error && (
        <p className="github-error" role="alert">
          {github.error}
        </p>
      )}
      <div className="dialog-actions">
        <button type="button" className="btn" onClick={onClose}>
          Cancel
        </button>
        <button
          type="button"
          className="btn primary"
          disabled={!token.trim() || busy}
          onClick={() => void submit()}
        >
          {busy ? "Signing in…" : "Sign in"}
        </button>
      </div>
    </Dialog>
  );
}
