import type { AgentPreset } from "./protocol";

/**
 * Which agent proofreads each batch. The writer proofreads by default; a
 * different agent reads the batch with fresh eyes and its own standards.
 */
export function ProofreaderField({
  agents,
  value,
  onChange,
}: {
  agents: AgentPreset[];
  value: string | null;
  onChange: (proofreader: string | null) => void;
}) {
  const choices = agents.filter((agent) => agent.kind === "acp" && agent.id !== "custom");
  return (
    <>
      <label className="field">
        <span>Proofreading agent</span>
        <select value={value ?? ""} onChange={(event) => onChange(event.target.value || null)}>
          <option value="">The agent that wrote the review</option>
          {choices.map((agent) => (
            <option key={agent.id} value={agent.id} disabled={!agent.available}>
              {agent.label}
              {agent.available ? "" : " (not installed)"}
            </option>
          ))}
        </select>
      </label>
      <p className="muted field-hint">
        A different agent often holds the batch to the rubric more strictly than the one that wrote
        it. Applies to batches written from now on.
      </p>
    </>
  );
}
