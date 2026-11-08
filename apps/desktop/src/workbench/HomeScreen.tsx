import { AddRepositoryForm } from "./AddRepositoryForm";
import { progressLabel, relativeTime, sourceSummary, statusLabel } from "./format";
import type { TaskView, WorkbenchRoute, WorkbenchSnapshot } from "./protocol";
import { StatusBadge } from "./StatusBadge";

/** Every review, newest first, with its state at a glance. */
export function HomeScreen({
  snapshot,
  filter,
  onNavigate,
  onAddRepository,
  onRemoveRepository,
  pickDirectory,
}: {
  snapshot: WorkbenchSnapshot;
  filter: string | null;
  onNavigate: (route: WorkbenchRoute) => void;
  onAddRepository: (path: string) => Promise<boolean>;
  onRemoveRepository: (repositoryId: string) => void;
  pickDirectory?: () => Promise<string | null>;
}) {
  const repository = snapshot.repositories.find((repo) => repo.id === filter) ?? null;
  const tasks = snapshot.tasks.filter((task) => !repository || task.repository_id === repository.id);
  // Count by the same label each card shows, so the tally cannot say
  // "walkthrough ready" while a card says "Raw diff ready".
  const counts = new Map<string, number>();
  for (const task of tasks) {
    const label = statusLabel(task);
    counts.set(label, (counts.get(label) ?? 0) + 1);
  }
  const tally = [...counts.entries()].map(([label, count]) => `${count} ${label.toLowerCase()}`);

  return (
    <main data-gewu-screen="home" aria-labelledby="home-title">
      <header className="screen-header">
        <p className="eyebrow" title={repository?.path}>
          {repository ? (repository.branch ?? repository.name) : "All repositories"}
        </p>
        <h2 id="home-title" className="screen-title" tabIndex={-1}>
          {repository ? `Reviews in ${repository.name}` : "Reviews"}
        </h2>
        {tasks.length > 0 && (
          <p className="muted">
            {tasks.length} review{tasks.length === 1 ? "" : "s"} · {tally.join(" · ")}
          </p>
        )}
      </header>

      {snapshot.repositories.length === 0 ? (
        <section className="panel empty-state" aria-labelledby="first-step">
          <h3 id="first-step">Start by adding a Git repository</h3>
          <p className="muted">
            Gewu reads the repository you choose. Reviews of commits, pull requests, and
            directories run in a separate checkout, so your working copy is never changed.
          </p>
          <AddRepositoryForm onAdd={onAddRepository} pickDirectory={pickDirectory} />
        </section>
      ) : tasks.length === 0 ? (
        <section className="panel empty-state" aria-labelledby="no-reviews">
          <h3 id="no-reviews">No reviews {repository ? `in ${repository.name} ` : ""}yet</h3>
          <p className="muted">
            Use <strong>New review</strong> to choose a commit range, your uncommitted changes,
            a pull request, or a directory, and the agent that writes the walkthrough.
          </p>
          {repository && (
            <button type="button" className="btn" onClick={() => onRemoveRepository(repository.id)}>
              Remove {repository.name} from Gewu
            </button>
          )}
        </section>
      ) : (
        <ul className="task-list" aria-label="Reviews">
          {tasks.map((task) => (
            <TaskRow key={task.id} task={task} showRepository={!repository} onNavigate={onNavigate} />
          ))}
        </ul>
      )}
    </main>
  );
}

function TaskRow({
  task,
  showRepository,
  onNavigate,
}: {
  task: TaskView;
  showRepository: boolean;
  onNavigate: (route: WorkbenchRoute) => void;
}) {
  const meta = `task-${task.id}-meta`;
  const progress = progressLabel(task);
  return (
    <li className={`task-row ${task.status}`}>
      <button
        type="button"
        className="task-row-main"
        aria-describedby={meta}
        onClick={() => onNavigate({ screen: "task", task_id: task.id })}
      >
        <span className="task-row-title">{task.title}</span>
      </button>
      <StatusBadge task={task} />
      <div className="task-row-meta" id={meta}>
        <span>
          {[showRepository ? task.repository_name : null, sourceSummary(task.source), task.agent_label]
            .filter(Boolean)
            .join(" · ")}
        </span>
        {progress && <span>{progress}</span>}
        {(task.status === "failed" || task.status === "generating") && task.message && (
          <span className={task.status === "failed" ? "task-row-message error" : "task-row-message"}>
            {task.message}
          </span>
        )}
        <span className="task-row-time">Updated {relativeTime(task.updated_at)}</span>
      </div>
      {task.review && (
        <button
          type="button"
          className="btn small task-row-open"
          aria-label={`Open review of ${task.title}`}
          onClick={() => onNavigate({ screen: "review", task_id: task.id })}
        >
          Open review
        </button>
      )}
      {task.status === "failed" && (
        <button
          type="button"
          className="btn small task-row-open"
          onClick={() => onNavigate({ screen: "task", task_id: task.id })}
        >
          See how to fix
        </button>
      )}
    </li>
  );
}
