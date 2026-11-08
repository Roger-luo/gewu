import { useId } from "react";

import type { AgentPreset } from "./protocol";

function agentBlurb(id: string): string {
  switch (id) {
    case "claude":
      return "Claude writes a guided walkthrough of the changes.";
    case "codex":
      return "Codex writes a guided walkthrough of the changes.";
    case "cursor":
      return "Cursor's agent writes a guided walkthrough of the changes.";
    case "gemini":
      return "Gemini writes a guided walkthrough of the changes.";
    case "opencode":
      return "OpenCode writes a guided walkthrough of the changes.";
    default:
      return "Writes a guided walkthrough of the changes.";
  }
}

/** One radio per preset, each saying what will run or why it cannot. */
export function AgentPicker({
  agents,
  value,
  onChange,
}: {
  agents: AgentPreset[];
  value: string;
  onChange: (presetId: string) => void;
}) {
  const name = useId();
  return (
    <fieldset className="agent-picker">
      <legend id={`${name}-legend`}>Agent</legend>
      <div role="radiogroup" aria-labelledby={`${name}-legend`} className="agent-options">
        {agents.map((agent) => {
          const detail = `${name}-${agent.id}-detail`;
          return (
            <label key={agent.id} className={agent.available ? "agent-option" : "agent-option unavailable"}>
              <input
                type="radio"
                name={name}
                value={agent.id}
                checked={value === agent.id}
                disabled={!agent.available}
                aria-describedby={detail}
                onChange={() => onChange(agent.id)}
              />
              <span className="radio-dot" aria-hidden />
              <span className="agent-label">{agent.label}</span>
              <span id={detail} className="agent-detail">
                {!agent.available ? (
                  <span className="unavailable-reason">
                    {agent.detail ?? "Not installed"}. Install it to choose it here.
                  </span>
                ) : agent.kind === "none" ? (
                  "Shows the raw diff file by file. Nothing runs."
                ) : agent.id === "custom" ? (
                  "Type its command next to Start review. It must speak ACP on stdio."
                ) : (
                  agentBlurb(agent.id)
                )}
              </span>
            </label>
          );
        })}
      </div>
      <p className="field-hint muted">
        The agent may read the code under review and run <code>gewu harness</code>. It cannot
        edit your files; Gewu refuses anything else and stops the review if files change.
      </p>
    </fieldset>
  );
}
