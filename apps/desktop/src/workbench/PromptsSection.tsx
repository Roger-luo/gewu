import { useEffect, useState } from "react";

import type { Receipt } from "../publication";
import { PromptReportForm } from "./PromptReportForm";
import type { AgentPreset, PromptReport, PromptView } from "./protocol";

/**
 * Every prompt Gewu sends agents, read-only: what each one is for, the text
 * every agent gets, and what the chosen backend gets added. Shown so nothing
 * about how Gewu instructs agents is hidden, and so a reader who sees
 * something wrong can say so.
 */
export function PromptsSection({
  agents,
  load,
  report,
}: {
  agents: AgentPreset[];
  load: () => Promise<PromptView[]>;
  /** Files a report about a prompt; absent where reports cannot be sent. */
  report?: (report: PromptReport) => Promise<Receipt>;
}) {
  const [open, setOpen] = useState(false);
  const [prompts, setPrompts] = useState<PromptView[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [selected, setSelected] = useState<string | null>(null);
  const [backend, setBackend] = useState("");

  // Fetched when first opened: the texts are long, and most visits to
  // Settings are about something else.
  useEffect(() => {
    if (!open || prompts) return;
    let live = true;
    load()
      .then((all) => {
        if (!live) return;
        setPrompts(all);
        setSelected((current) => current ?? all[0]?.id ?? null);
      })
      .catch((err: unknown) => live && setError(err instanceof Error ? err.message : String(err)));
    return () => {
      live = false;
    };
  }, [open, prompts, load]);

  const backends = agents.filter((agent) => agent.kind === "acp" && agent.id !== "custom");
  const prompt = prompts?.find((item) => item.id === selected) ?? null;
  const addition = prompt?.additions.find((item) => item.backend === backend) ?? null;
  const backendLabel = backends.find((agent) => agent.id === backend)?.label ?? backend;

  return (
    <section className="panel settings-section prompts-section" aria-labelledby="settings-prompts">
      <h3 id="settings-prompts">Prompts agents are sent</h3>
      <p className="muted">
        Gewu tells agents how to write a walkthrough in these prompts, sent one stage at a time. Every
        agent gets the same text; a backend that needs more gets an addition after it. They ship with
        Gewu and are shown here so nothing is hidden. Text in <code>{"{{double braces}}"}</code> is
        filled in for each run.
      </p>
      <details onToggle={(event) => setOpen(event.currentTarget.open)}>
        <summary>Show the prompts</summary>
        {error && (
          <p className="warning" role="alert">
            Could not load the prompts: {error}
          </p>
        )}
        {!prompts && !error && <p className="muted">Loading…</p>}
        {prompts && (
          <div className="prompts-browser">
            <nav aria-label="Prompts">
              <ol className="prompts-list">
                {prompts.map((item) => (
                  <li key={item.id}>
                    <button
                      type="button"
                      className={item.id === selected ? "prompt-tab selected" : "prompt-tab"}
                      aria-current={item.id === selected ? "true" : undefined}
                      onClick={() => setSelected(item.id)}
                    >
                      {item.title}
                      {item.additions.length > 0 && (
                        <span className="muted" title="A backend adds to this prompt">
                          {" "}
                          +
                        </span>
                      )}
                    </button>
                  </li>
                ))}
              </ol>
            </nav>
            {prompt && (
              <article className="prompt-view" aria-label={`${prompt.title} prompt`}>
                <header className="prompt-view-header">
                  <div>
                    <h4>{prompt.title}</h4>
                    <p className="muted">
                      {prompt.when} <code>harness/prompts/{prompt.id}.md</code>
                    </p>
                  </div>
                  <label className="field prompt-backend">
                    <span>As sent to</span>
                    <select value={backend} onChange={(event) => setBackend(event.target.value)}>
                      <option value="">Any agent</option>
                      {backends.map((agent) => (
                        <option key={agent.id} value={agent.id}>
                          {agent.label}
                        </option>
                      ))}
                    </select>
                  </label>
                </header>
                <pre className="prompt-text">{prompt.general}</pre>
                {addition ? (
                  <>
                    <h5 className="prompt-addition-title">Added for {backendLabel}</h5>
                    <pre className="prompt-text prompt-addition">{addition.text}</pre>
                  </>
                ) : (
                  backend && (
                    <p className="muted">{backendLabel} adds nothing here: it gets the text above.</p>
                  )
                )}
                {report && (
                  <PromptReportForm
                    key={prompt.id}
                    promptId={prompt.id}
                    promptTitle={prompt.title}
                    backend={backend || null}
                    backendLabel={backend ? backendLabel : null}
                    onSend={report}
                  />
                )}
              </article>
            )}
          </div>
        )}
      </details>
    </section>
  );
}
