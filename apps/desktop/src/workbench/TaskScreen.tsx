import { useState } from "react";

import { AgentLog } from "./AgentLog";
import type { WorkbenchBackend } from "./backend";
import { Dialog } from "./Dialog";
import {
  canContinue,
  continueHeld,
  progressLabel,
  relativeTime,
  sourceSummary,
  stageLabel,
} from "./format";
import type { AgentPreset, TaskView, WorkbenchRoute } from "./protocol";
import { StatusBadge } from "./StatusBadge";
import { InlineMarkdown } from "../components/Markdown";

/** One review task: what it covers, how far the agent got, and what to do next. */
export function TaskScreen({
  backend,
  task,
  agent,
  onNavigate,
  onContinue,
  onCancel,
  onDelete,
  onStartAgain,
}: {
  backend: WorkbenchBackend;
  task: TaskView;
  agent: AgentPreset | undefined;
  onNavigate: (route: WorkbenchRoute) => void;
  onContinue: () => void;
  onCancel: () => void;
  onDelete: () => void;
  onStartAgain: () => void;
}) {
  const [confirming, setConfirming] = useState(false);
  const [restarting, setRestarting] = useState(false);
  const progress = progressLabel(task);
  const stage = task.status === "generating" ? stageLabel(task.stage) : null;
  const failed = task.status === "failed";
  const held = continueHeld(task);
  const command = task.agent.command ?? agent?.command;
  const emptyRange = failed && /nothing to review/i.test(task.message ?? "");
  const missingAgent = failed && /could not start/i.test(task.message ?? "");

  return (
    <main data-gewu-screen="task" data-gewu-task-id={task.id} aria-labelledby="task-title">
      <div className="screen-back">
        <button type="button" className="btn ghost" onClick={() => onNavigate({ screen: "home" })}>
          <span aria-hidden>‹ </span>Back to reviews
        </button>
      </div>
      <header className="screen-header">
        <p className="eyebrow">
          {task.repository_name} · {sourceSummary(task.source)}
        </p>
        <h2 id="task-title" className="screen-title" tabIndex={-1}>
          {task.title}
        </h2>
        <div className="status-line" aria-live="polite">
          <StatusBadge task={task} />
          {stage && <span className="muted">{stage}</span>}
          {progress && <span className="muted">{progress}</span>}
        </div>
      </header>

      {task.outdated && (
        <div className="notice outdated" role="status">
          <p>
            An older version of Gewu prepared this review, so it lacks {task.outdated}. Starting
            again prepares it with this version.
          </p>
          <button type="button" className="btn primary" onClick={() => setRestarting(true)}>
            Start again…
          </button>
        </div>
      )}

      {task.message && (
        <p className={failed ? "notice failure" : "notice"} role={failed ? "alert" : undefined}>
          <InlineMarkdown source={task.message} />
        </p>
      )}

      <div className="actions-row">
        {task.review && (
          <button
            type="button"
            className="btn primary"
            onClick={() => onNavigate({ screen: "review", task_id: task.id })}
          >
            Open review
          </button>
        )}
        {failed && (
          <button
            type="button"
            className="btn primary"
            onClick={() =>
              onNavigate({ screen: "new-review", repository_id: task.repository_id })
            }
          >
            {emptyRange
              ? "Change what to review"
              : missingAgent
                ? "Choose another agent"
                : "Start a new review"}
          </button>
        )}
        {task.status !== "complete" && canContinue(task) && (
          <button
            type="button"
            className={failed || task.review || held ? "btn" : "btn primary"}
            disabled={held}
            aria-disabled={held}
            title={
              held
                ? "Mark every step reviewed before asking for the next batch"
                : failed
                  ? "Retry with the same settings"
                  : undefined
            }
            onClick={onContinue}
          >
            {failed ? "Retry" : "Continue"}
          </button>
        )}
        {task.status === "generating" && (
          <button type="button" className="btn" onClick={onCancel}>
            Cancel
          </button>
        )}
        <button type="button" className="btn danger" onClick={() => setConfirming(true)}>
          Delete…
        </button>
      </div>
      {task.status === "awaiting_reviewer" && (
        <p className="muted field-hint">
          {held
            ? "The agent writes ahead of you as far as Settings › Agents allows; finishing a batch starts the next."
            : "Continue asks the agent for the next batch. It reads what you marked reviewed and any feedback you left first."}
        </p>
      )}

      <dl className="task-details">
        <dt>Repository</dt>
        <dd>{task.repository_name}</dd>
        <dt>Source</dt>
        <dd>{sourceSummary(task.source)}</dd>
        <dt>Agent</dt>
        <dd>
          <div>{task.agent_label}</div>
          {command && (
            <details className="agent-command-details">
              <summary>Show command</summary>
              <code className="agent-command">{command}</code>
            </details>
          )}
        </dd>
        <dt>Started</dt>
        <dd>{relativeTime(task.created_at)}</dd>
        <dt>Updated</dt>
        <dd>{relativeTime(task.updated_at)}</dd>
      </dl>

      <AgentLog key={task.id} backend={backend} task={task} />

      {restarting && (
        <Dialog title="Start this review again?" onClose={() => setRestarting(false)}>
          <p className="muted">
            Gewu prepares a new review of the same source with the same agent, then deletes this
            one with its progress and feedback. Your repository is not changed.
          </p>
          <div className="dialog-actions">
            <button type="button" className="btn" onClick={() => setRestarting(false)}>
              Keep this review
            </button>
            <button
              type="button"
              className="btn primary"
              onClick={() => {
                setRestarting(false);
                onStartAgain();
              }}
            >
              Start again
            </button>
          </div>
        </Dialog>
      )}

      {confirming && (
        <Dialog title="Delete this review?" onClose={() => setConfirming(false)}>
          <p className="muted">
            This stops the agent if it is running and removes the review, its feedback, and
            {task.source.type === "working_tree"
              ? " Gewu's state in the repository. Your uncommitted changes stay as they are."
              : " the separate checkout Gewu made for it. Your repository is not changed."}
          </p>
          <div className="dialog-actions">
            <button type="button" className="btn" onClick={() => setConfirming(false)}>
              Keep review
            </button>
            <button
              type="button"
              className="btn danger"
              onClick={() => {
                setConfirming(false);
                onDelete();
              }}
            >
              Delete review
            </button>
          </div>
        </Dialog>
      )}
    </main>
  );
}
