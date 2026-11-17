import { ProofreaderField } from "./ProofreaderField";
import type { AgentPreset, Settings, SettingsView } from "./protocol";

/** Which agent writes and proofreads reviews, and how much. */
export function AgentsSettings({
  settings,
  agents,
  draft,
  onChange,
}: {
  settings: SettingsView;
  agents: AgentPreset[];
  draft: Settings;
  onChange: (draft: Settings) => void;
}) {
  const preset = draft.default_agent?.preset_id ?? "";
  return (
    <section className="panel settings-section" aria-labelledby="settings-agents">
      <h3 id="settings-agents">Agents and generation</h3>
      <label className="field">
        <span>Default agent for new reviews</span>
        <select
          value={preset}
          onChange={(event) => {
            const id = event.target.value;
            onChange({
              ...draft,
              default_agent: id ? { preset_id: id, command: id === "custom" ? (draft.default_agent?.command ?? "") : null } : null,
            });
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
              onChange({ ...draft, default_agent: { preset_id: "custom", command: event.target.value } });
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
            onChange({ ...draft, proofread_rounds: Number(event.target.value) });
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
          onChange({ ...draft, proofreader });
        }}
      />
      <label className="settings-check">
        <input
          type="checkbox"
          checked={draft.explain_tests}
          onChange={(event) => {
            onChange({ ...draft, explain_tests: event.target.checked });
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
  );
}
