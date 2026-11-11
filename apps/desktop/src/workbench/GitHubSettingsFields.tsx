import type { GitHubAccount, GitHubPreferences, GitHubSettings } from "../backend/types";

function cliStatus(settings: GitHubSettings): string {
  switch (settings.cli.status) {
    case "signed_in":
      return `gh is signed in as ${settings.cli.login}.`;
    case "signed_out":
      return "gh is installed but not signed in; run gh auth login.";
    case "missing":
      return "gh is not installed.";
  }
}

/** The settings screen's GitHub section. */
export function GitHubSettingsFields({
  settings,
  account,
  draft,
  onChange,
}: {
  settings: GitHubSettings;
  account: GitHubAccount | null;
  draft: GitHubPreferences;
  onChange: (patch: Partial<GitHubPreferences>) => void;
}) {
  return (
    <section className="panel settings-section" aria-labelledby="settings-github">
      <h3 id="settings-github">GitHub</h3>
      <p className="muted">
        {account
          ? `Publishing posts as ${account.login}.`
          : "Not signed in. Sign in from the account at the bottom of the sidebar."}
        {settings.sandbox && " This is the sandbox: nothing is ever sent."}
      </p>
      <fieldset className="settings-choice">
        <legend>Sign in with</legend>
        <label>
          <input
            type="radio"
            name="github-method"
            checked={draft.method === "gewu"}
            onChange={() => onChange({ method: "gewu" })}
          />
          <span>
            Gewu&apos;s GitHub app
            <span className="muted"> · sign in with your browser and a one-time code; the token stays in your keychain</span>
          </span>
        </label>
        <label>
          <input
            type="radio"
            name="github-method"
            checked={draft.method === "gh"}
            disabled={settings.cli.status === "missing"}
            onChange={() => onChange({ method: "gh" })}
          />
          <span>
            The GitHub CLI
            <span className="muted"> · {cliStatus(settings)}</span>
          </span>
        </label>
      </fieldset>
      {draft.method !== settings.method && (
        <p className="muted field-hint" role="note">
          {draft.method === "gh"
            ? settings.cli.status === "signed_in"
              ? `After you save, publishing posts as ${settings.cli.login} through gh.`
              : "After you save, sign in to gh from the account at the bottom of the sidebar."
            : "After you save, sign in with your browser from the account at the bottom of the sidebar, unless Gewu's app is already signed in."}
        </p>
      )}
      <label className="field">
        <span>OAuth app client ID</span>
        <input
          type="text"
          value={draft.client_id ?? ""}
          placeholder={`${settings.default_client_id} (Gewu's app)`}
          disabled={draft.method !== "gewu"}
          onChange={(event) => onChange({ client_id: event.target.value || null })}
        />
      </label>
      <p className="muted field-hint">
        {draft.method === "gewu"
          ? "Leave blank for Gewu's own app. Set it only if your organization runs its own OAuth app with device flow enabled. Used by the next browser sign-in."
          : "Only used when signing in with Gewu's GitHub app; the GitHub CLI has its own."}
      </p>
      <label className="settings-check">
        <input
          type="checkbox"
          checked={draft.rehearse}
          onChange={(event) => onChange({ rehearse: event.target.checked })}
        />
        <span>
          Rehearse instead of sending
          <span className="muted">
            {" "}
            · Publish shows what it would post, and posts nothing. Notes stay unpublished.
          </span>
        </span>
      </label>
    </section>
  );
}
