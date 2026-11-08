import { useEffect, useState, type FormEvent, type KeyboardEvent } from "react";

import { AddRepositoryForm } from "./AddRepositoryForm";
import { AgentPicker } from "./AgentPicker";
import type { WorkbenchBackend } from "./backend";
import { SOURCE_LABELS } from "./format";
import type { RepositoryRefs, SourceType, WorkbenchAction, WorkbenchSnapshot } from "./protocol";
import { SourceFields } from "./SourceFields";
import { draftSource, EMPTY_DRAFT, type SourceDraft } from "./sourceDraft";

const TABS: SourceType[] = ["commit_range", "working_tree", "pull_request", "directory"];
const AGENT_KEY = "gewu.agent";

type CreateTask = Extract<WorkbenchAction, { type: "create_task" }>;

function defaultAgent(snapshot: WorkbenchSnapshot): string {
  const remembered = globalThis.localStorage?.getItem(AGENT_KEY);
  const usable = snapshot.agents.filter((agent) => agent.available);
  if (remembered && usable.some((agent) => agent.id === remembered)) {
    return remembered;
  }
  // Prefer the raw-diff path so a first review does not wait on an agent.
  return (
    usable.find((agent) => agent.id === "none")?.id ??
    usable.find((agent) => agent.kind === "acp" && agent.id !== "custom")?.id ??
    "none"
  );
}

/** Choose a repository, what in it to review, and who writes the walkthrough. */
export function NewReviewScreen({
  backend,
  snapshot,
  initialRepository,
  onAddRepository,
  onCreate,
}: {
  backend: WorkbenchBackend;
  snapshot: WorkbenchSnapshot;
  initialRepository?: string;
  onAddRepository: (path: string) => Promise<boolean>;
  onCreate: (action: CreateTask) => Promise<unknown>;
}) {
  const [chosen, setRepositoryId] = useState(initialRepository ?? "");
  const [type, setType] = useState<SourceType>("commit_range");
  const [draft, setDraft] = useState<SourceDraft>(EMPTY_DRAFT);
  const [agent, setAgent] = useState(() => defaultAgent(snapshot));
  const [command, setCommand] = useState("");
  const [title, setTitle] = useState("");
  const [refs, setRefs] = useState<{ id: string; refs: RepositoryRefs | null; error: string | null } | null>(null);
  const [problem, setProblem] = useState<string | null>(null);
  const [starting, setStarting] = useState(false);

  const repositoryId = snapshot.repositories.some((repository) => repository.id === chosen)
    ? chosen
    : (snapshot.repositories[0]?.id ?? "");

  useEffect(() => {
    if (!repositoryId) {
      return;
    }
    let cancelled = false;
    backend
      .refs(repositoryId)
      .then((next) => !cancelled && setRefs({ id: repositoryId, refs: next, error: null }))
      .catch((error: unknown) => !cancelled && setRefs({ id: repositoryId, refs: null, error: String(error instanceof Error ? error.message : error) }));
    return () => {
      cancelled = true;
    };
  }, [backend, repositoryId]);

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    const source = draftSource(type, draft);
    if (typeof source === "string") {
      setProblem(source);
      return;
    }
    if (agent === "custom" && !command.trim()) {
      setProblem("Enter the command that starts the agent.");
      return;
    }
    setProblem(null);
    setStarting(true);
    globalThis.localStorage?.setItem(AGENT_KEY, agent);
    await onCreate({
      type: "create_task",
      repository_id: repositoryId,
      source,
      agent: { preset_id: agent, command: agent === "custom" ? command.trim() : null },
      title: title.trim() || null,
    });
    setStarting(false);
  };

  const onTabKey = (event: KeyboardEvent) => {
    const step = event.key === "ArrowRight" ? 1 : event.key === "ArrowLeft" ? -1 : 0;
    if (step) {
      event.preventDefault();
      const next = TABS[(TABS.indexOf(type) + step + TABS.length) % TABS.length];
      setType(next);
      document.getElementById(`tab-${next}`)?.focus();
    }
  };

  const current = refs?.id === repositoryId ? refs : null;

  return (
    <main data-gewu-screen="new-review" aria-labelledby="new-review-title">
      <header className="screen-header new-review-header">
        <div>
          <h2 id="new-review-title" className="screen-title" tabIndex={-1}>
            New review
          </h2>
          <p className="muted">
            Choose what to review. The agent writes the walkthrough one batch at a time, and waits
            for you between batches.
          </p>
        </div>
        {snapshot.repositories.length > 0 && (
          <div className="new-review-start">
            {agent === "custom" && (
              <label className="field">
                <span>Agent command</span>
                <input
                  type="text"
                  value={command}
                  placeholder="e.g. opencode acp"
                  spellCheck={false}
                  autoComplete="off"
                  onChange={(event) => setCommand(event.target.value)}
                />
              </label>
            )}
            <button type="submit" form="new-review-form" className="btn primary" disabled={starting}>
              {starting ? "Starting…" : "Start review"}
            </button>
          </div>
        )}
      </header>

      {snapshot.repositories.length === 0 ? (
        <section className="panel empty-state" aria-labelledby="need-repository">
          <h3 id="need-repository">Add a repository first</h3>
          <AddRepositoryForm onAdd={onAddRepository} pickDirectory={backend.pickDirectory} />
        </section>
      ) : (
        <form
          id="new-review-form"
          className="new-review"
          onSubmit={(event) => void submit(event)}
          noValidate
        >
          <label className="field">
            <span>Repository</span>
            <select value={repositoryId} onChange={(event) => setRepositoryId(event.target.value)}>
              {snapshot.repositories.map((repository) => (
                <option key={repository.id} value={repository.id}>
                  {repository.name} — {repository.path}
                </option>
              ))}
            </select>
          </label>
          {current?.error && <p className="warning">Could not read this repository: {current.error}</p>}

          <div className="source-picker">
            <div role="tablist" aria-label="What to review" className="tabs">
              {TABS.map((tab) => (
                <button
                  key={tab}
                  id={`tab-${tab}`}
                  type="button"
                  role="tab"
                  aria-selected={type === tab}
                  aria-controls="source-panel"
                  tabIndex={type === tab ? 0 : -1}
                  onClick={() => setType(tab)}
                  onKeyDown={onTabKey}
                >
                  {SOURCE_LABELS[tab]}
                </button>
              ))}
            </div>
            <div id="source-panel" role="tabpanel" aria-labelledby={`tab-${type}`} className="tab-panel">
              {!current?.refs && !current?.error && <p className="muted loading-refs">Reading commits, branches, and pull requests…</p>}
              <SourceFields type={type} draft={draft} refs={current?.refs ?? null} onChange={(patch) => setDraft((value) => ({ ...value, ...patch }))} />
            </div>
          </div>

          <AgentPicker agents={snapshot.agents} value={agent} onChange={setAgent} />

          <label className="field">
            <span>Title</span>
            <input type="text" value={title} placeholder="Optional; named after the source otherwise" onChange={(event) => setTitle(event.target.value)} />
          </label>

          {problem && <p className="warning" role="alert">{problem}</p>}
        </form>
      )}
    </main>
  );
}
