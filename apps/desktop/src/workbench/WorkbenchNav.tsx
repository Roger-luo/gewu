import { GitHubStatus } from "../components/GitHubStatus";
import { FolderPlusIcon } from "../components/Icons";
import type { GitHubState } from "../hooks/useGitHub";
import type { RepositoryView, WorkbenchRoute, WorkbenchSnapshot } from "./protocol";

/**
 * The leading sidebar, top to bottom: the two ways to start (New review, Add
 * repository), the repositories to filter reviews by, then the account and Settings.
 */
export function WorkbenchNav({
  snapshot,
  route,
  filter,
  onFilter,
  onNavigate,
  onAddRepository,
  github,
}: {
  snapshot: WorkbenchSnapshot;
  route: WorkbenchRoute;
  filter: string | null;
  onFilter: (repositoryId: string | null) => void;
  onNavigate: (route: WorkbenchRoute) => void;
  onAddRepository: () => void;
  github: GitHubState;
}) {
  const count = (id: string | null) =>
    snapshot.tasks.filter((task) => id === null || task.repository_id === id).length;
  const home = route.screen === "home";
  return (
    <nav className="navigator workbench-nav" aria-label="Workbench">
      <div className="nav-scroll">
      <h1>Gewu</h1>
      <div className="nav-actions">
        <button
          type="button"
          className="btn primary nav-cta"
          aria-current={route.screen === "new-review" ? "page" : undefined}
          onClick={() => onNavigate({ screen: "new-review", repository_id: filter ?? undefined })}
        >
          New review
        </button>
        <button type="button" className="btn nav-add" onClick={onAddRepository}>
          <FolderPlusIcon size={16} />
          Add repository
        </button>
      </div>
      <h2 className="nav-heading" id="repositories-heading">
        Repositories
      </h2>
      <ul className="nav-list" aria-labelledby="repositories-heading">
        <li>
          <button
            type="button"
            aria-current={home && filter === null ? "page" : undefined}
            onClick={() => {
              onFilter(null);
              onNavigate({ screen: "home" });
            }}
          >
            <span className="nav-label">All reviews</span>
            <span className="nav-count">{count(null)}</span>
          </button>
        </li>
        {snapshot.repositories.map((repository) => (
          <li key={repository.id}>
            <button
              type="button"
              aria-current={home && filter === repository.id ? "page" : undefined}
              aria-describedby={`repo-${repository.id}-meta`}
              onClick={() => {
                onFilter(repository.id);
                onNavigate({ screen: "home" });
              }}
            >
              <span className="nav-label">{repository.name}</span>
              <span className="nav-count">{count(repository.id)}</span>
            </button>
            <RepositoryMeta repository={repository} />
          </li>
        ))}
      </ul>
      </div>
      <GitHubStatus
        github={github}
        onSettings={() => onNavigate({ screen: "settings" })}
        settingsOpen={route.screen === "settings"}
      />
    </nav>
  );
}

function RepositoryMeta({ repository }: { repository: RepositoryView }) {
  return (
    <p className={repository.error ? "nav-meta error" : "nav-meta"} id={`repo-${repository.id}-meta`}>
      {repository.error ??
        [repository.branch ?? "detached HEAD", repository.dirty ? "uncommitted changes" : null]
          .filter(Boolean)
          .join(" · ")}
    </p>
  );
}
