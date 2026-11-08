import type { TaskView } from "./protocol";
import { statusLabel } from "./format";

/** Lifecycle status as plain text with a leading mark, matching in-review status. */
export function StatusBadge({ task }: { task: TaskView }) {
  return (
    <span className={`status-text ${task.status}`}>
      <span className="status-mark" aria-hidden />
      {statusLabel(task)}
    </span>
  );
}
