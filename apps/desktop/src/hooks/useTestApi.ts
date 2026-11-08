import { useEffect, useRef } from "react";

import type { GewuTestApi, WorkbenchTarget } from "../protocol";
import type { ReviewController } from "./useReviewApp";

/** The workbench around a review, when the review belongs to a task. */
export interface TestApiHost {
  taskId: string;
  goto(target: WorkbenchTarget): void;
}

export function useTestApi(app: ReviewController, host?: TestApiHost): void {
  const appRef = useRef(app);
  const hostRef = useRef(host);

  useEffect(() => {
    appRef.current = app;
    hostRef.current = host;
  }, [app, host]);

  useEffect(() => {
    const api: GewuTestApi = {
      state: () => ({
        screen: appRef.current.screen,
        task_id: hostRef.current?.taskId ?? null,
        chunk_index: appRef.current.chunkIndex,
        chunk_count: appRef.current.snapshot?.chunks.length ?? 0,
        overlay:
          appRef.current.overlay === "help" ||
          appRef.current.overlay === "feedback"
            ? appRef.current.overlay
            : null,
      }),
      goto: async (target) => {
        switch (target.screen) {
          case "overview":
          case "summary":
          case "chunk":
            await appRef.current.goto(target);
            break;
          default:
            hostRef.current?.goto(target);
        }
        await waitUntilReady();
      },
      pages: () => appRef.current.pages,
    };
    window.__gewu = api;
    return () => {
      if (window.__gewu === api) {
        delete window.__gewu;
      }
    };
  }, []);
}

export function waitUntilReady(): Promise<void> {
  return new Promise((resolve) => {
    let frames = 0;
    const tick = () => {
      // Two frames first, so a navigation has started before readiness counts.
      if (frames++ > 1 && document.documentElement.dataset.gewuReady === "true") {
        resolve();
        return;
      }
      requestAnimationFrame(tick);
    };
    requestAnimationFrame(tick);
  });
}
