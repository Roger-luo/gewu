import { canContinue, continueHeld } from "./format";
import type { TaskView } from "./protocol";
import { StatusBadge } from "./StatusBadge";

/** The task strip above a review: where it came from and what runs next. */
export function TaskBar({
  task,
  progress,
  onBack,
  onShowTask,
  onContinue,
}: {
  task: TaskView;
  /** Live review progress when the walkthrough is open. */
  progress?: { reviewed: number; total: number } | null;
  onBack: () => void;
  onShowTask: () => void;
  onContinue: () => void;
}) {
  const more = canContinue(task);
  const held = continueHeld(task, progress);
  const finishing =
    task.status === "awaiting_reviewer" && (task.review?.remaining_work_items ?? 0) === 0;
  const message = held
    ? "Mark every step reviewed before asking for the next batch."
    : finishing
      ? "Mark every step reviewed to finish the walkthrough."
      : task.message;
  return (
    <div className="task-bar" role="region" aria-label="Review task">
      <button type="button" className="btn ghost" onClick={onBack}>
        <span aria-hidden>‹ </span>Back to reviews
      </button>
      <div className="task-bar-status" aria-live="polite">
        <StatusBadge task={task} />
        {message && (
          <span className="muted task-bar-message" title={message}>
            {message}
          </span>
        )}
      </div>
      <div className="task-bar-actions">
        <button type="button" className="btn" onClick={onShowTask}>
          Agent log
        </button>
        {more && (
          <button
            type="button"
            className={held ? "btn" : "btn primary"}
            disabled={held}
            aria-disabled={held}
            title={held ? "Mark every step reviewed before asking for the next batch" : undefined}
            onClick={onContinue}
          >
            Continue
          </button>
        )}
      </div>
    </div>
  );
}
