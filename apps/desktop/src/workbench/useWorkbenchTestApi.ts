import { useEffect, useRef } from "react";

import { waitUntilReady } from "../hooks/useTestApi";
import type { GewuTestApi, GewuTestPage } from "../protocol";
import type { WorkbenchController } from "./useWorkbench";

/**
 * `window.__gewu` on the workbench screens. The review screens install their
 * own, which hands workbench targets back here.
 */
export function useWorkbenchTestApi(wb: WorkbenchController): void {
  const ref = useRef(wb);
  useEffect(() => {
    ref.current = wb;
  }, [wb]);

  const reviewing = wb.route.screen === "review";
  useEffect(() => {
    if (reviewing) {
      return;
    }
    const api: GewuTestApi = {
      state: () => {
        const route = ref.current.route;
        return {
          screen: route.screen === "review" ? "overview" : route.screen,
          task_id: "task_id" in route ? route.task_id : null,
          chunk_index: null,
          chunk_count: 0,
          overlay: null,
        };
      },
      goto: async (target) => {
        switch (target.screen) {
          case "home":
          case "new-review":
          case "task":
          case "review":
            ref.current.navigate(target);
            break;
          default:
            throw new Error(`Open a review before going to ${target.screen}`);
        }
        await waitUntilReady();
      },
      pages: () => {
        const tasks = ref.current.snapshot?.tasks ?? [];
        const pages: GewuTestPage[] = [
          { id: "home", label: "Home", target: { screen: "home" } },
          { id: "new-review", label: "New review", target: { screen: "new-review" } },
        ];
        for (const task of tasks) {
          pages.push({ id: `task-${task.id}`, label: task.title, target: { screen: "task", task_id: task.id } });
        }
        return pages;
      },
    };
    window.__gewu = api;
    return () => {
      if (window.__gewu === api) {
        delete window.__gewu;
      }
    };
  }, [reviewing]);
}
