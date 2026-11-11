import { useState } from "react";

import type { GitHubState } from "../hooks/useGitHub";
import { GearIcon } from "./Icons";
import { SignInDialog } from "./SignInDialog";

/**
 * Bottom of the sidebar: who publishing posts as, then the app-wide
 * controls in one row: Settings, and Switch or Sign in.
 */
export function GitHubStatus({
  github,
  onSettings,
  settingsOpen = false,
}: {
  github: GitHubState;
  /** Opens Settings; absent where there is none, e.g. a single review window. */
  onSettings?: () => void;
  settingsOpen?: boolean;
}) {
  const [open, setOpen] = useState(false);
  if (!github.available && !onSettings) {
    return null;
  }
  const { account, settings } = github;
  const viaCli = settings?.method === "gh";
  return (
    // A region, not a <footer>: inside the sidebar's <nav> a footer is not a
    // landmark, so browsers drop its name.
    <section className="github-status" aria-label="Account and settings">
      {github.available && (
        <div className="github-identity">
          {account ? (
            <>
              <img src={account.avatar_url} alt="" width={28} height={28} />
              <span className="github-login" title={account.name ?? account.login}>
                {account.login}
                <span className="github-via muted">
                  {viaCli ? "via GitHub CLI" : "via Gewu's GitHub app"}
                </span>
              </span>
            </>
          ) : (
            <span className="github-login muted">
              {github.checking ? "Checking GitHub…" : "Not signed in to GitHub"}
            </span>
          )}
        </div>
      )}
      <div className="github-actions">
        {onSettings && (
          <button
            type="button"
            className="btn small settings-button"
            aria-current={settingsOpen ? "page" : undefined}
            onClick={onSettings}
          >
            <GearIcon />
            Settings
          </button>
        )}
        {github.available && !github.checking && (
          // "Sign in" fits beside Settings in a narrow sidebar; the name
          // says what it signs in to.
          <button
            type="button"
            className="btn small"
            aria-label={account ? "Switch" : "Sign in to GitHub"}
            title={account ? "Change how Gewu signs in to GitHub" : "Sign in to GitHub"}
            onClick={() => setOpen(true)}
          >
            {account ? "Switch" : "Sign in"}
          </button>
        )}
      </div>
      {github.error && !open && (
        <p className="github-error" role="status">
          {github.error}
        </p>
      )}
      {open && <SignInDialog github={github} onClose={() => setOpen(false)} />}
    </section>
  );
}
