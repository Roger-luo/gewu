import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { App } from "../App";
import { ErrorBanner } from "../components/ErrorBanner";
import type { WorkbenchBackend } from "./backend";
import { Dialog } from "./Dialog";
import { AddRepositoryForm } from "./AddRepositoryForm";
import { HomeScreen } from "./HomeScreen";
import { NewReviewScreen } from "./NewReviewScreen";
import type { TaskView, WorkbenchRoute } from "./protocol";
import { TaskScreen } from "./TaskScreen";
import { useWorkbench, type WorkbenchController } from "./useWorkbench";
import { useWorkbenchTestApi } from "./useWorkbenchTestApi";
import { WorkbenchNav } from "./WorkbenchNav";

export function WorkbenchApp({ backend }: { backend: WorkbenchBackend }) {
  const wb = useWorkbench(backend);
  const [filter, setFilter] = useState<string | null>(null);
  const [adding, setAdding] = useState(false);
  const { route, snapshot, navigate } = wb;
  useWorkbenchTestApi(wb);
  useFocusOnRoute(route);

  const addRepository = useCallback(
    async (path: string) => {
      const known = new Set(wb.snapshot?.repositories.map((repo) => repo.id));
      const next = await wb.dispatch({ type: "add_repository", path });
      const added = next?.repositories.find((repo) => !known.has(repo.id));
      if (added) {
        setFilter(added.id);
      }
      if (next) {
        setAdding(false);
      }
      return next !== null;
    },
    [wb],
  );
  const { setError } = wb;
  const closeAdding = useCallback(() => {
    setAdding(false);
    setError(null);
  }, [setError]);

  const leaveNewReview = useCallback(() => navigate({ screen: "home" }), [navigate]);

  const task = route.screen === "task" || route.screen === "review"
    ? snapshot?.tasks.find((item) => item.id === route.task_id)
    : undefined;

  useEffect(() => {
    if (snapshot && (route.screen === "task" || route.screen === "review") && !task) {
      navigate({ screen: "home" });
    }
  }, [navigate, route, snapshot, task]);

  if (!snapshot) {
    return wb.loadError ? (
      <div className="boot-error" role="alert">
        <p>Gewu could not load your reviews: {wb.loadError}</p>
        <button type="button" className="btn" onClick={() => void wb.refresh()}>
          Try again
        </button>
      </div>
    ) : (
      <div className="loading">Loading reviews…</div>
    );
  }

  if (route.screen === "review" && task?.review) {
    return <TaskReview key={task.id} wb={wb} task={task} onBack={() => {
      setFilter(task.repository_id);
      navigate({ screen: "home" });
    }} />;
  }

  return (
    <div className="app-shell">
      <WorkbenchNav
        snapshot={snapshot}
        route={route}
        filter={filter}
        onFilter={setFilter}
        onNavigate={navigate}
        onAddRepository={() => setAdding(true)}
      />
      <div className="app-main-column">
        {wb.error && !adding && <ErrorBanner message={wb.error} onDismiss={() => wb.setError(null)} />}
        {wb.loadError && (
          <p className="warning stale">Showing the last known state: {wb.loadError}</p>
        )}
        {route.screen === "home" && (
          <HomeScreen
            snapshot={snapshot}
            filter={filter}
            onNavigate={navigate}
            onAddRepository={addRepository}
            onRemoveRepository={(id) => {
              setFilter(null);
              void wb.dispatch({ type: "remove_repository", repository_id: id });
            }}
            onContinue={(id) => void wb.dispatch({ type: "continue_task", task_id: id })}
            pickDirectory={backend.pickDirectory}
          />
        )}
        {route.screen === "new-review" && (
          <NewReviewScreen
            backend={backend}
            snapshot={snapshot}
            initialRepository={route.repository_id}
            onAddRepository={addRepository}
            onCreate={wb.createTask}
            onCancel={leaveNewReview}
          />
        )}
        {route.screen === "task" && task && (
          <TaskScreen
            backend={backend}
            task={task}
            agent={snapshot.agents.find((agent) => agent.id === task.agent.preset_id)}
            onNavigate={navigate}
            onContinue={() => void wb.dispatch({ type: "continue_task", task_id: task.id })}
            onCancel={() => void wb.dispatch({ type: "cancel_task", task_id: task.id })}
            onDelete={() => void wb.deleteTask(task.id)}
          />
        )}
      </div>
      {adding && (
        <Dialog title="Add a repository" onClose={closeAdding}>
          <AddRepositoryForm
            onAdd={addRepository}
            pickDirectory={backend.pickDirectory}
            onCancel={closeAdding}
            onEdit={() => wb.error && setError(null)}
          />
          {wb.error && <p className="warning" role="alert">{wb.error}</p>}
        </Dialog>
      )}
    </div>
  );
}

/** One task's review, with the task strip and the reader's place kept. */
function TaskReview({
  wb,
  task,
  onBack,
}: {
  wb: WorkbenchController;
  task: TaskView;
  onBack: () => void;
}) {
  const review = useMemo(() => wb.backend.review(task.id), [task.id, wb.backend]);
  const { navigate, places, dispatch } = wb;
  const host = useMemo(() => ({ taskId: task.id, goto: navigate }), [navigate, task.id]);
  return (
    <App
      backend={review}
      task={{
        view: task,
        place: places.get(task.id),
        onPlace: (place) => places.set(task.id, place),
        onChange: () => void wb.refresh(),
        onBack,
        onShowTask: () => navigate({ screen: "task", task_id: task.id }),
        onContinue: () => void dispatch({ type: "continue_task", task_id: task.id }),
        host,
      }}
    />
  );
}

function useFocusOnRoute(route: WorkbenchRoute) {
  const first = useRef(true);
  const key = `${route.screen}:${"task_id" in route ? route.task_id : ""}`;
  useEffect(() => {
    if (first.current) {
      first.current = false;
      return;
    }
    const main = document.querySelector<HTMLElement>("main[data-gewu-screen]");
    main?.scrollTo?.({ top: 0 });
    main?.querySelector<HTMLElement>(".screen-title")?.focus({ preventScroll: true });
  }, [key]);
}
