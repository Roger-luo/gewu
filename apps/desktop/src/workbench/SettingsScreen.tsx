import { useState, type FormEvent, type ReactNode } from "react";

import type { GitHubPreferences } from "../backend/types";
import { AccountIcon, AgentIcon, DriveIcon, PromptIcon } from "../components/SettingsIcons";
import type { GitHubState } from "../hooks/useGitHub";
import type { Receipt } from "../publication";
import { AgentsSettings } from "./AgentsSettings";
import { GitHubSettingsFields } from "./GitHubSettingsFields";
import { PromptsSection } from "./PromptsSection";
import type { AgentPreset, PromptReport, PromptView, Settings, SettingsView } from "./protocol";
import { StorageSettings } from "./StorageSettings";

type Tab = "github" | "agents" | "prompts" | "storage";

/** The sections, in the order the sidebar lists them. */
const TABS: { id: Tab; label: string; icon: ReactNode }[] = [
  { id: "github", label: "GitHub", icon: <AccountIcon /> },
  { id: "agents", label: "Agents", icon: <AgentIcon /> },
  { id: "prompts", label: "Prompts", icon: <PromptIcon /> },
  { id: "storage", label: "Storage", icon: <DriveIcon /> },
];

/** Tabs whose fields Save writes. */
const EDITABLE: ReadonlySet<Tab> = new Set(["github", "agents"]);

/**
 * Every option Gewu has, as a page of its own: its sections in the sidebar,
 * as Codex lays out its settings, and the chosen one beside them. Nothing is
 * saved until Save, and the editable sections stay mounted so switching
 * sections keeps edits.
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
  /** Leaves Settings, back to the screen it was opened from. */
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
  const [tab, setTab] = useState<Tab>("github");
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

  const current = TABS.find((item) => item.id === tab) ?? TABS[0];

  return (
    <div className="app-shell settings-page">
      <nav className="navigator settings-nav" aria-label="Settings sections">
        {onBack && (
          <button type="button" className="btn ghost settings-back" onClick={onBack}>
            <span aria-hidden>‹ </span>
            {backLabel}
          </button>
        )}
        <div className="settings-tabs" role="tablist" aria-label="Settings" aria-orientation="vertical">
          {TABS.map((item) => (
            <button
              key={item.id}
              type="button"
              role="tab"
              id={`settings-tab-${item.id}`}
              aria-selected={tab === item.id}
              aria-controls={`settings-panel-${item.id}`}
              className={tab === item.id ? "settings-tab selected" : "settings-tab"}
              onClick={() => setTab(item.id)}
            >
              <span className="settings-tab-icon">{item.icon}</span>
              {item.label}
            </button>
          ))}
        </div>
      </nav>
      <div className="app-main-column">
        <main data-gewu-screen="settings" aria-labelledby="settings-title" className="settings-window">
          <h2 id="settings-title" className="screen-title settings-page-title" tabIndex={-1}>
            {current.label}
          </h2>

          <form id="settings-form" className="settings" onSubmit={(event) => void save(event)} noValidate>
            <div {...panel("github", tab)}>
              {githubDraft && github.settings ? (
                <GitHubSettingsFields
                  settings={github.settings}
                  account={github.account}
                  draft={githubDraft}
                  onChange={(patch) => {
                    setGitHubEdits({ ...githubEdits, ...patch });
                    setState("idle");
                  }}
                />
              ) : (
                <p className="muted">Loading GitHub settings…</p>
              )}
            </div>
            <div {...panel("agents", tab)}>
              <AgentsSettings
                settings={settings}
                agents={agents}
                draft={draft}
                onChange={(next) => {
                  setDraft(next);
                  setState("idle");
                }}
              />
            </div>
            {EDITABLE.has(tab) && (
              <footer className="settings-footer">
                {state === "failed" && (
                  <p className="warning" role="alert">
                    Not saved: {problem ?? github.error ?? "see the message above"}.
                  </p>
                )}
                <span className="muted settings-saved" role="status">
                  {state === "saved" ? "Saved" : ""}
                </span>
                <button type="submit" className="btn primary" disabled={state === "saving"}>
                  {state === "saving" ? "Saving…" : "Save"}
                </button>
              </footer>
            )}
          </form>

          {/* Read-only tabs, outside the form; mounted only when shown. */}
          {tab === "prompts" && loadPrompts && (
            <div {...panel("prompts", tab)}>
              <PromptsSection agents={agents} load={loadPrompts} report={reportPrompt} />
            </div>
          )}
          {tab === "storage" && (
            <div {...panel("storage", tab)}>
              <StorageSettings settings={settings} />
            </div>
          )}
        </main>
      </div>
    </div>
  );
}

function panel(id: Tab, shown: Tab) {
  return {
    role: "tabpanel",
    id: `settings-panel-${id}`,
    "aria-labelledby": `settings-tab-${id}`,
    hidden: id !== shown,
    className: "settings-panel",
  } as const;
}
