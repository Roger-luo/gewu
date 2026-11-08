import { useCallback, useMemo, useState } from "react";

import type { Backend } from "./backend";
import { ChunkScreen } from "./components/ChunkScreen";
import { ErrorBanner } from "./components/ErrorBanner";
import { FeedbackDialog } from "./components/FeedbackDialog";
import { HelpOverlay } from "./components/HelpOverlay";
import { Navigator } from "./components/Navigator";
import { OverviewScreen } from "./components/OverviewScreen";
import { SummaryScreen } from "./components/SummaryScreen";
import { useFocusOnNavigate } from "./hooks/useFocusOnNavigate";
import { useKeyboard } from "./hooks/useKeyboard";
import { useReviewApp } from "./hooks/useReviewApp";
import { useTestApi, type TestApiHost } from "./hooks/useTestApi";
import { firstUnreviewed } from "./navigation";
import type { FeedbackAnchorInput, FeedbackTarget, ReviewPlace } from "./protocol";
import { TaskBar } from "./workbench/TaskBar";
import type { TaskView } from "./workbench/protocol";

/** The workbench task a review belongs to. */
export interface ReviewTask {
  view: TaskView;
  place?: ReviewPlace;
  onPlace: (place: ReviewPlace) => void;
  /** The reader changed the review, which can change the task, e.g. finish it. */
  onChange: () => void;
  onBack: () => void;
  onShowTask: () => void;
  onContinue: () => void;
  host: TestApiHost;
}

export function App({ backend, task }: { backend: Backend; task?: ReviewTask }) {
  const app = useReviewApp(backend, {
    place: task?.place,
    onPlace: task?.onPlace,
    onChange: task?.onChange,
    revision: task?.view.updated_at,
  });
  const [lineSelection, setLineSelection] = useState<{
    changeId: string;
    lines: Set<number>;
  } | null>(null);

  useKeyboard(app);
  useTestApi(app, task?.host);
  useFocusOnNavigate(app.screen, app.chunkIndex);

  const toggleLine = useCallback((changeId: string, line: number) => {
    setLineSelection((current) => {
      if (!current || current.changeId !== changeId) {
        return { changeId, lines: new Set([line]) };
      }
      const next = new Set(current.lines);
      if (next.has(line)) {
        next.delete(line);
      } else {
        next.add(line);
      }
      return next.size === 0 ? null : { changeId, lines: next };
    });
  }, []);

  const anchor = useMemo<FeedbackAnchorInput>(() => {
    if (!lineSelection || lineSelection.lines.size === 0) {
      return null;
    }
    const lines = [...lineSelection.lines].sort((a, b) => a - b);
    return {
      change_id: lineSelection.changeId,
      start_line: lines[0],
      end_line: lines[lines.length - 1],
    };
  }, [lineSelection]);

  const anchorSummary = useMemo(() => {
    if (!anchor || !app.snapshot || app.chunkIndex === null) {
      return null;
    }
    const chunk = app.snapshot.chunks[app.chunkIndex];
    for (const block of chunk?.blocks ?? []) {
      if (block.type !== "changes") {
        continue;
      }
      const quote = block.quotes.find((item) => item.change_id === anchor.change_id);
      if (quote) {
        return `${quote.path}:${anchor.start_line}-${anchor.end_line}`;
      }
    }
    return `${anchor.change_id}:${anchor.start_line}-${anchor.end_line}`;
  }, [anchor, app.chunkIndex, app.snapshot]);

  if (app.loadError) {
    return (
      <div className="boot-error" role="alert">
        <p>Failed to load review: {app.loadError}</p>
        {task && (
          <button type="button" className="btn" onClick={task.onBack}>
            Back to reviews
          </button>
        )}
      </div>
    );
  }

  if (!app.bootstrapped || !app.snapshot) {
    return <div className="loading">Loading review…</div>;
  }

  const snapshot = app.snapshot;
  const currentChunk =
    app.screen === "chunk" && app.chunkIndex !== null
      ? (snapshot.chunks[app.chunkIndex] ?? null)
      : null;
  // A remembered place can outlive its chunk when the agent replans.
  const screenShown = app.screen === "chunk" && !currentChunk ? "overview" : app.screen;

  const submitFeedback = async (body: string, target: FeedbackTarget) => {
    if (!currentChunk) {
      return;
    }
    const result = await app.dispatch({
      type: "add_feedback",
      chunk_id: currentChunk.id,
      kind: app.feedbackKind,
      target,
      body,
      anchor,
    });
    if (result) {
      setLineSelection(null);
      app.closeOverlay();
    }
  };

  return (
    <div className="app-shell">
      <Navigator
        snapshot={snapshot}
        screen={app.screen}
        chunkIndex={app.chunkIndex}
        onNavigate={(target) => void app.goto(target)}
        taskTitle={task?.view.title}
      />
      <div className="app-main-column">
        {task && (
          <TaskBar
            task={task.view}
            progress={snapshot.summary}
            onBack={task.onBack}
            onShowTask={task.onShowTask}
            onContinue={task.onContinue}
          />
        )}
        {app.error && (
          <ErrorBanner message={app.error} onDismiss={() => app.setError(null)} />
        )}
        {screenShown === "overview" && (
          <OverviewScreen
            snapshot={snapshot}
            onStart={() => void app.goto({ screen: "chunk", index: firstUnreviewed(snapshot) })}
          />
        )}
        {currentChunk && app.chunkIndex !== null && (
          <ChunkScreen
            snapshot={snapshot}
            chunkIndex={app.chunkIndex}
            onDispatch={app.dispatch}
            onToggleReviewed={app.toggleReviewed}
            onAddComment={() => app.openFeedback("note")}
            onRequestChange={() => app.openFeedback("issue")}
            lineSelection={lineSelection}
            onToggleLine={toggleLine}
          />
        )}
        {app.screen === "summary" && (
          <SummaryScreen
            snapshot={snapshot}
            onNavigate={(target) => void app.goto(target)}
            onDecide={(decision) =>
              void app.dispatch({ type: "set_decision", decision })
            }
          />
        )}
      </div>

      {app.overlay === "help" && (
        <HelpOverlay onClose={app.closeOverlay} />
      )}
      {app.overlay === "feedback" && currentChunk && (
        <FeedbackDialog
          kind={app.feedbackKind}
          anchorSummary={anchorSummary}
          onCancel={app.closeOverlay}
          onSubmit={(body, target) => void submitFeedback(body, target)}
        />
      )}
    </div>
  );
}
