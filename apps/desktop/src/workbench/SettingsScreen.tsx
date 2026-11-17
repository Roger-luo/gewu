import { useState, type FormEvent } from "react";

import type { GitHubPreferences } from "../backend/types";
import { GearIcon } from "../components/Icons";
import type { GitHubState } from "../hooks/useGitHub";
import { GitHubSettingsFields } from "./GitHubSettingsFields";
import { PromptsSection } from "./PromptsSection";
import { ProofreaderField } from "./ProofreaderField";
import type { Receipt } from "../publication";
import type { AgentPreset, PromptReport, PromptView, Settings, SettingsView } from "./protocol";

/**
 * Every option Gewu has, in one place: GitHub, agents and generation, and
 * where Gewu keeps things. Nothing is saved until Save.
 */
export function SettingsScreen({
  settings,
  agents,
  github,
  onSave,
  onBack,
  backLabel = "Back",
  loadPrompts,
  reportPrompt,
}: {
  /** Returns to the screen Settings was opened from. */
  onBack?: () => void;
  backLabel?: string;
  settings: SettingsView;
  agents: AgentPreset[];
  github: GitHubState;
  /** Resolves to `null` when saved, otherwise why not. */
  onSave: (settings: Settings) => Promise<string | null>;
  /** Every prompt agents are sent; absent where it cannot be read. */
  loadPrompts?: () => Promise<PromptView[]>;
  /** Files a public issue on Gewu about a prompt, after the reader confirms. */
  reportPrompt?: (report: PromptReport) => Promise<Receipt>;
}) {
  const [draft, setDraft] = useState<Settings>({
    proofread_rounds: settings.proofread_rounds,
    default_agent: settings.default_agent,
    explain_tests: settings.explain_tests,
    proofreader: settings.proofreader,
  });
  // Only the GitHub fields edited here. The rest show the live value, so a
  // sign-in from the sidebar while this screen is open is not undone by Save.
  const [githubEdits, setGitHubEdits] = useState<Partial<GitHubPreferences>>({});
  const githubDraft: GitHubPreferences | null = github.settings && {
    method: github.settings.method,
    client_id: github.settings.client_id,
    rehearse: github.settings.rehearse,
    ...githubEdits,
  };
  const [state, setState] = useState<"idle" | "saving" | "saved" | "failed">("idle");
  const [problem, setProblem] = useState<string | null>(null);
  const preset = draft.default_agent?.preset_id ?? "";

  const save = async (event: FormEvent) => {
    event.preventDefault();
    setState("saving");
    // The workbench half is checked first, so a refused value saves nothing.
    const refused = await onSave(draft);
    setProblem(refused);
    const saved = refused === null;
    const edited = Object.keys(githubEdits).length > 0;
    const githubSaved =
      saved && (!githubDraft || !edited || (await github.updateSettings(githubDraft)));
    if (githubSaved) setGitHubEdits({});
    setState(saved && githubSaved ? "saved" : "failed");
  };

  return (
    <main data-gewu-screen="settings" aria-labelledby="settings-title">
      <header className="screen-header new-review-header settings-header">
        <div>
          {onBack && (
            <button type="button" className="btn ghost screen-back" onClick={onBack}>
              <span aria-hidden>‹ </span>
              {backLabel}
            </button>
          )}
          <h2 id="settings-title" className="screen-title settings-title" tabIndex={-1}>
            <GearIcon size={22} />
            Settings
          </h2>
          <p className="muted">Everything Gewu lets you change. Nothing is saved until you press Save.</p>
        </div>
        <div className="new-review-start">
          <span className="muted settings-saved" role="status">
            {state === "saved" ? "Saved" : ""}
          </span>
          <button type="submit" form="settings-form" className="btn primary" disabled={state === "saving"}>
            {state === "saving" ? "Saving…" : "Save"}
          </button>
        </div>
      </header>

      <form id="settings-form" className="settings" onSubmit={(event) => void save(event)} noValidate>
        {githubDraft && github.settings && (
          <GitHubSettingsFields
            settings={github.settings}
            account={github.account}
            draft={githubDraft}
            onChange={(patch) => {
              setGitHubEdits({ ...githubEdits, ...patch });
              setState("idle");
            }}
          />
        )}
        {state === "failed" && (
          <p className="warning" role="alert">
            Not saved: {problem ?? github.error ?? "see the message above"}.
          </p>
        )}

        <section className="panel settings-section" aria-labelledby="settings-agents">
          <h3 id="settings-agents">Agents and generation</h3>
          <label className="field">
            <span>Default agent for new reviews</span>
            <select
              value={preset}
              onChange={(event) => {
                const id = event.target.value;
                setDraft({
                  ...draft,
                  default_agent: id ? { preset_id: id, command: id === "custom" ? (draft.default_agent?.command ?? "") : null } : null,
                });
                setState("idle");
              }}
            >
              <option value="">The one you used last</option>
              {agents.map((agent) => (
                <option key={agent.id} value={agent.id} disabled={!agent.available}>
                  {agent.label}
                  {agent.available ? "" : " (not installed)"}
                </option>
              ))}
            </select>
          </label>
          {preset === "custom" && (
            <label className="field">
              <span>Command that starts the agent</span>
              <input
                type="text"
                value={draft.default_agent?.command ?? ""}
                placeholder="my-agent --acp"
                onChange={(event) => {
                  setDraft({ ...draft, default_agent: { preset_id: "custom", command: event.target.value } });
                  setState("idle");
                }}
              />
            </label>
          )}
          <label className="field">
            <span>Proofread rounds</span>
            <input
              type="number"
              min={0}
              max={settings.max_proofread_rounds}
              value={draft.proofread_rounds}
              onChange={(event) => {
                setDraft({ ...draft, proofread_rounds: Number(event.target.value) });
                setState("idle");
              }}
            />
          </label>
          <p className="muted field-hint">
            After the agent writes a batch, a fresh agent checks it and asks for a rewrite, up to this
            many times, before you see it. Between 0 and {settings.max_proofread_rounds}; 0 turns
            proofreading off. Applies to reviews you start from now on.
          </p>
          <ProofreaderField
            agents={agents}
            value={draft.proofreader}
            onChange={(proofreader) => {
              setDraft({ ...draft, proofreader });
              setState("idle");
            }}
          />
          <label className="settings-check">
            <input
              type="checkbox"
              checked={draft.explain_tests}
              onChange={(event) => {
                setDraft({ ...draft, explain_tests: event.target.checked });
                setState("idle");
              }}
            />
            <span>
              Explain tests
              <span className="muted">
                {" "}
                · The agent walks through tests like the rest of the change. Off, Gewu shows each
                test as written, with its name and doc comment, at once and without agent time.
                Applies to reviews you start from now on.
              </span>
            </span>
          </label>
        </section>

        <section className="panel settings-section" aria-labelledby="settings-storage">
          <h3 id="settings-storage">Where Gewu keeps things</h3>
          <p className="muted">
            These are set before Gewu starts, with environment variables, and shown here so nothing is
            hidden.
          </p>
          <dl className="settings-paths">
            <dt>Reviews, repositories, and settings</dt>
            <dd>
              <code>{settings.home}</code> <span className="muted">· GEWU_HOME</span>
            </dd>
            <dt>Gewu command agents call</dt>
            <dd>
              <code>{settings.gewu}</code> <span className="muted">· GEWU_BIN</span>
            </dd>
            <dt>Never contact GitHub for one launch</dt>
            <dd>
              <code>GEWU_GITHUB=record</code>{" "}
              <span className="muted">· a stand-in GitHub that records instead of sending</span>
            </dd>
          </dl>
        </section>
      </form>
      {/* Outside the form: nothing here is saved. */}
      {loadPrompts && (
        <PromptsSection agents={agents} load={loadPrompts} report={reportPrompt} />
      )}
    </main>
  );
}
