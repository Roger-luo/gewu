import { useCallback, useEffect, useState } from "react";

import type { ReviewPlace } from "../protocol";
import { setDocumentReady } from "../ready";
import type { WorkbenchBackend } from "./backend";
import type { WorkbenchAction, WorkbenchRoute, WorkbenchSnapshot } from "./protocol";

/** Polling interval while a batch runs, and otherwise. */
const BUSY_POLL_MS = 1000;
const IDLE_POLL_MS = 5000;

function messageOf(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

export function useWorkbench(backend: WorkbenchBackend) {
  const [snapshot, setSnapshot] = useState<WorkbenchSnapshot | null>(null);
  const [route, setRoute] = useState<WorkbenchRoute>({ screen: "home" });
  const [error, setError] = useState<string | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [pending, setPending] = useState(0);
  const [places] = useState(() => new Map<string, ReviewPlace>());

  const refresh = useCallback(async () => {
    try {
      setSnapshot(await backend.workbench());
      setLoadError(null);
    } catch (err) {
      setLoadError(messageOf(err));
    }
  }, [backend]);

  useEffect(() => {
    let cancelled = false;
    backend.workbench().then(
      (next) => !cancelled && setSnapshot(next),
      (err: unknown) => !cancelled && setLoadError(messageOf(err)),
    );
    return () => {
      cancelled = true;
    };
  }, [backend]);

  const generating = snapshot?.tasks.some((task) => task.status === "generating") ?? false;
  useEffect(() => {
    const timer = setInterval(() => void refresh(), generating ? BUSY_POLL_MS : IDLE_POLL_MS);
    return () => clearInterval(timer);
  }, [generating, refresh]);

  useEffect(() => {
    if (route.screen === "review") {
      return;
    }
    if (!snapshot || pending > 0) {
      setDocumentReady(false);
      return;
    }
    const frame = requestAnimationFrame(() => setDocumentReady(true));
    return () => cancelAnimationFrame(frame);
  }, [pending, route, snapshot]);

  /** Run `work`, showing its failure in the banner; `null` when it failed. */
  const track = useCallback(async <T,>(work: () => Promise<T>): Promise<T | null> => {
    setPending((count) => count + 1);
    try {
      const result = await work();
      setError(null);
      return result;
    } catch (err) {
      setError(messageOf(err));
      return null;
    } finally {
      setPending((count) => count - 1);
    }
  }, []);

  const dispatch = useCallback(
    async (action: WorkbenchAction): Promise<WorkbenchSnapshot | null> => {
      const next = await track(() => backend.dispatch(action));
      if (next) {
        setSnapshot(next);
      }
      return next;
    },
    [backend, track],
  );

  const navigate = useCallback((next: WorkbenchRoute) => {
    setError(null);
    setRoute(next);
  }, []);

  /** Create a task and open it; the task's id, or `null` when refused. */
  const createTask = useCallback(
    async (action: Extract<WorkbenchAction, { type: "create_task" }>) => {
      const known = new Set(snapshot?.tasks.map((task) => task.id));
      const next = await dispatch(action);
      const created = next?.tasks.find((task) => !known.has(task.id));
      if (created) {
        navigate({ screen: "task", task_id: created.id });
      }
      return created?.id ?? null;
    },
    [dispatch, navigate, snapshot],
  );

  const deleteTask = useCallback(
    async (taskId: string) => {
      if (await dispatch({ type: "delete_task", task_id: taskId })) {
        places.delete(taskId);
        navigate({ screen: "home" });
      }
    },
    [dispatch, navigate, places],
  );

  return {
    backend,
    snapshot,
    route,
    error,
    loadError,
    busy: pending > 0,
    places,
    refresh,
    dispatch,
    track,
    navigate,
    createTask,
    deleteTask,
    setError,
  };
}

export type WorkbenchController = ReturnType<typeof useWorkbench>;
