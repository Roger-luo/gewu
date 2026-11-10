import { useId, useState } from "react";

import { avatarFor, readAccount, writeAccount, type GitHubAccount } from "../githubAccount";

/** Bottom of the sidebar: who is linked, without contacting GitHub. */
export function GitHubStatus() {
  const [account, setAccount] = useState<GitHubAccount | null>(() => readAccount());
  const [open, setOpen] = useState(false);

  const link = (login: string) => {
    const next = { login, avatarUrl: avatarFor(login) };
    writeAccount(next);
    setAccount(next);
    setOpen(false);
  };

  const signOut = () => {
    writeAccount(null);
    setAccount(null);
  };

  return (
    <footer className="github-status" aria-label="GitHub account">
      {account ? (
        <>
          <img src={account.avatarUrl} alt="" width={28} height={28} />
          <span className="github-login">{account.login}</span>
          <button type="button" className="btn small" onClick={signOut}>
            Sign out
          </button>
        </>
      ) : (
        <button type="button" className="btn small" onClick={() => setOpen(true)}>
          Sign in to GitHub
        </button>
      )}
      {open && <LinkDialog onCancel={() => setOpen(false)} onLink={link} />}
    </footer>
  );
}

function LinkDialog({
  onCancel,
  onLink,
}: {
  onCancel: () => void;
  onLink: (login: string) => void;
}) {
  const titleId = useId();
  const [login, setLogin] = useState("");
  return (
    <div className="dialog-backdrop">
      <div className="dialog" role="dialog" aria-modal="true" aria-labelledby={titleId}>
        <h2 id={titleId}>Link GitHub</h2>
        <p className="muted">
          Gewu remembers this login on this computer and draws the avatar locally.
          Linking does not call GitHub. The publication preview is what would be
          sent later.
        </p>
        <label>
          <span>GitHub login</span>
          <input
            value={login}
            autoFocus
            onChange={(event) => setLogin(event.target.value)}
          />
        </label>
        <div className="dialog-actions">
          <button type="button" className="btn" onClick={onCancel}>
            Cancel
          </button>
          <button
            type="button"
            className="btn primary"
            disabled={!login.trim()}
            onClick={() => onLink(login.trim())}
          >
            Link account
          </button>
        </div>
      </div>
    </div>
  );
}
