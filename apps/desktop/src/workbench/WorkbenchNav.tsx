import type { RepositoryView, WorkbenchRoute, WorkbenchSnapshot } from "./protocol";

/** The leading sidebar: start a review, and filter reviews by repository. */
export function WorkbenchNav({
  snapshot,
  route,
  filter,
  onFilter,
  onNavigate,
  onAddRepository,
}: {
  snapshot: WorkbenchSnapshot;
  route: WorkbenchRoute;
  filter: string | null;
  onFilter: (repositoryId: string | null) => void;
  onNavigate: (route: WorkbenchRoute) => void;
  onAddRepository: () => void;
}) {
  const count = (id: string | null) =>
    snapshot.tasks.filter((task) => id === null || task.repository_id === id).length;
  const home = route.screen === "home";
  return (
    <nav className="navigator workbench-nav" aria-label="Workbench">
      <h1>Gewu</h1>
      <button
        type="button"
        className="btn primary nav-cta"
        aria-current={route.screen === "new-review" ? "page" : undefined}
        onClick={() => onNavigate({ screen: "new-review", repository_id: filter ?? undefined })}
      >
        New review
      </button>
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
      <button type="button" className="btn nav-secondary" onClick={onAddRepository}>
        Add repository
      </button>
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
