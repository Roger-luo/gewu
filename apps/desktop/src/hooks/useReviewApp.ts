import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import type { Backend } from "../backend";
import { BackendError } from "../backend";
import { afterReviewed, neighborTarget, pagesFromSnapshot, resumeTarget } from "../navigation";
import type {
  Action,
  FeedbackKind,
  Overlay,
  ReviewPlace,
  Screen,
  Snapshot,
  ReviewTarget,
} from "../protocol";
import { setDocumentReady } from "../ready";

export interface ReviewOptions {
  /** Where to start reading, e.g. where the reader left this task. */
  place?: ReviewPlace;
  onPlace?: (place: ReviewPlace) => void;
  /** Called after each action the backend accepted. */
  onChange?: () => void;
  /** Changes when an agent may have added chunks; the review is reloaded. */
  revision?: number;
}

export function useReviewApp(backend: Backend, options: ReviewOptions = {}) {
  const { place, onPlace, onChange, revision } = options;
  const [snapshot, setSnapshot] = useState<Snapshot | null>(null);
  const [screen, setScreen] = useState<Screen>(place?.screen ?? "overview");
  const [chunkIndex, setChunkIndex] = useState<number | null>(place?.chunkIndex ?? null);
  const [overlay, setOverlay] = useState<Overlay>(null);
  const [feedbackKind, setFeedbackKind] = useState<FeedbackKind>("note");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [bootstrapped, setBootstrapped] = useState(false);
  const [epoch, setEpoch] = useState(0);
  const transitionDepth = useRef(0);
  const restored = useRef(false);

  const markBusy = useCallback(() => {
    transitionDepth.current += 1;
    setDocumentReady(false);
  }, []);

  const markIdle = useCallback(() => {
    transitionDepth.current = Math.max(0, transitionDepth.current - 1);
    setEpoch((value) => value + 1);
  }, []);

  useEffect(() => {
    setDocumentReady(false);
    let cancelled = false;
    (async () => {
      try {
        const next = await backend.snapshot();
        if (cancelled) {
          return;
        }
        setSnapshot(next);
        setBootstrapped(true);
      } catch (err) {
        if (!cancelled) {
          setLoadError(err instanceof Error ? err.message : String(err));
        }
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [backend]);

  const loaded = useRef(false);
  useEffect(() => {
    if (!loaded.current) {
      loaded.current = true;
      return;
    }
    let cancelled = false;
    backend
      .snapshot()
      .then((next) => {
        if (!cancelled) {
          setSnapshot(next);
        }
      })
      .catch(() => {
        // Keep the review on screen; the next revision retries.
      });
    return () => {
      cancelled = true;
    };
  }, [backend, revision]);

  const onPlaceRef = useRef(onPlace);
  const onChangeRef = useRef(onChange);
  const chunkCount = useRef(0);
  useEffect(() => {
    onPlaceRef.current = onPlace;
    onChangeRef.current = onChange;
  }, [onPlace, onChange]);
  useEffect(() => {
    chunkCount.current = snapshot?.chunks.length ?? 0;
  }, [snapshot]);
  useEffect(() => {
    if (bootstrapped) {
      onPlaceRef.current?.({ screen, chunkIndex, chunkCount: chunkCount.current });
    }
  }, [bootstrapped, screen, chunkIndex]);

  useEffect(() => {
    if (!bootstrapped || busy || loadError || transitionDepth.current > 0) {
      setDocumentReady(false);
      return;
    }
    const frame = requestAnimationFrame(() => {
      if (transitionDepth.current === 0) {
        setDocumentReady(true);
      }
    });
    return () => cancelAnimationFrame(frame);
  }, [bootstrapped, busy, loadError, screen, chunkIndex, snapshot, overlay, epoch]);

  const pages = useMemo(
    () => (snapshot ? pagesFromSnapshot(snapshot) : []),
    [snapshot],
  );

  const dispatch = useCallback(
    async (action: Action): Promise<Snapshot | null> => {
      markBusy();
      setBusy(true);
      try {
        const next = await backend.dispatch(action);
        setSnapshot(next);
        setError(null);
        onChangeRef.current?.();
        return next;
      } catch (err) {
        const message =
          err instanceof BackendError
            ? err.message
            : err instanceof Error
              ? err.message
              : String(err);
        setError(message);
        return null;
      } finally {
        setBusy(false);
        markIdle();
      }
    },
    [backend, markBusy, markIdle],
  );

  const goto = useCallback(
    async (target: ReviewTarget): Promise<void> => {
      markBusy();
      try {
        if (target.screen === "chunk") {
          const chunk = snapshot?.chunks[target.index];
          if (!chunk) {
            return;
          }
          setScreen("chunk");
          setChunkIndex(target.index);
          await dispatch({ type: "visit", chunk_id: chunk.id });
          return;
        }
        setScreen(target.screen);
        setChunkIndex(null);
      } finally {
        markIdle();
      }
    },
    [dispatch, markBusy, markIdle, snapshot],
  );

  // Re-apply a remembered place once the snapshot is in, in case the first
  // paint raced the parent’s place map.
  useEffect(() => {
    if (restored.current || !place || !snapshot) {
      return;
    }
    restored.current = true;
    void goto(resumeTarget(snapshot, place));
  }, [goto, place, snapshot]);

  const openFeedback = useCallback((kind: FeedbackKind) => {
    setFeedbackKind(kind);
    setOverlay("feedback");
  }, []);

  const closeOverlay = useCallback(() => {
    setOverlay(null);
  }, []);

  const toggleHelp = useCallback(() => {
    setOverlay((current) => (current === "help" ? null : "help"));
  }, []);

  const movePage = useCallback(
    async (delta: -1 | 1) => {
      const next = neighborTarget(pages, screen, chunkIndex, delta);
      if (next) {
        await goto(next);
      }
    },
    [chunkIndex, goto, pages, screen],
  );

  const toggleReviewed = useCallback(async () => {
    if (screen !== "chunk" || chunkIndex === null || !snapshot) {
      return;
    }
    const chunk = snapshot.chunks[chunkIndex];
    if (!chunk) {
      return;
    }
    const reviewed = !chunk.status.reviewed;
    const next = await dispatch({
      type: "set_reviewed",
      chunk_id: chunk.id,
      reviewed,
    });
    if (next && reviewed) {
      await goto(afterReviewed(next, chunkIndex));
    }
  }, [chunkIndex, dispatch, goto, screen, snapshot]);

  return {
    snapshot,
    screen,
    chunkIndex,
    overlay,
    feedbackKind,
    error,
    busy,
    loadError,
    bootstrapped,
    pages,
    dispatch,
    goto,
    openFeedback,
    closeOverlay,
    toggleHelp,
    movePage,
    toggleReviewed,
    setError,
    setOverlay,
  };
}

export type ReviewController = ReturnType<typeof useReviewApp>;
