import type { TaskView } from "./protocol";
import { statusLabel } from "./format";

export function StatusBadge({ task }: { task: TaskView }) {
  const label = statusLabel(task);
  return (
    <span className={`status-badge ${task.status}`}>
      {task.status === "generating" && <span className="spinner" aria-hidden />}
      {label}
    </span>
  );
}
