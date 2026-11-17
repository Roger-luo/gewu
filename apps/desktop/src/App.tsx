import { useCallback, useMemo, useState } from "react";

import {
  anchorInput,
  anchorLabel,
  selectLine as nextSelection,
  spanSelection,
  type CommentRequest,
  type LineSelection,
} from "./anchors";
import type { Backend } from "./backend";
import { ChunkScreen } from "./components/ChunkScreen";
import { ErrorBanner } from "./components/ErrorBanner";
import { FeedbackDialog } from "./components/FeedbackDialog";
import { HelpOverlay } from "./components/HelpOverlay";
import { Navigator } from "./components/Navigator";
import { OverviewScreen } from "./components/OverviewScreen";
import { SummaryScreen } from "./components/SummaryScreen";
import { useFocusOnNavigate } from "./hooks/useFocusOnNavigate";
import { useContributors } from "./hooks/useContributors";
import { useGitHub, type GitHubState } from "./hooks/useGitHub";
import { useKeyboard } from "./hooks/useKeyboard";
import { usePublication } from "./hooks/usePublication";
import { useReviewApp } from "./hooks/useReviewApp";
import { useTestApi, type TestApiHost } from "./hooks/useTestApi";
import { firstUnreviewed } from "./navigation";
import { localPublication, rangeTitle, reviewVenue, type Publication } from "./publication";
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
  /** Opens the workbench's Settings. */
  onSettings?: () => void;
  /** The workbench's GitHub account state. */
  github?: GitHubState;
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
  const [lineSelection, setLineSelection] = useState<LineSelection | null>(null);

  useKeyboard(app);
  useTestApi(app, task?.host);
  useFocusOnNavigate(app.screen, app.chunkIndex);

  const selectLine = useCallback((changeId: string, line: number, extend: boolean) => {
    setLineSelection((current) => nextSelection(current, changeId, line, extend));
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

  // A comment opened from a block or an explanation carries its own span,
  // which wins over the line-number selection.
  const commentOn = app.feedbackRequest;
  const openComment = useCallback(
    (request: CommentRequest) => {
      app.openFeedback(request.kind, { target: request.target, span: request.span });
    },
    [app],
  );
  // A comment opened from one line selects what it covers, so the gutter
  // shows the lines the comment will land on.
  const commentOnLines = useCallback(
    (request: CommentRequest) => {
      setLineSelection(spanSelection(request.span));
      openComment(request);
    },
    [openComment],
  );

  // Lines the reader selected inside the block they comment on narrow the
  // block's span. Found by QA: "Comment on code" with line 9 selected used to
  // anchor to the whole block.
  const narrowed = useMemo(
    () =>
      commentOn?.target === "code" && anchor && anchor.change_id === commentOn.span.change_id
        ? { ...commentOn.span, start_line: anchor.start_line, end_line: anchor.end_line }
        : null,
    [anchor, commentOn],
  );
  const dialogAnchor = narrowed
    ? anchorInput(narrowed)
    : commentOn
      ? anchorInput(commentOn.span)
      : anchor;

  const anchorSummary = useMemo(() => {
    if (narrowed) {
      return anchorLabel(narrowed);
    }
    if (commentOn) {
      return anchorLabel(commentOn.span);
    }
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
        return anchorLabel({
          path: quote.path,
          side: "new",
          start_line: anchor.start_line,
          end_line: anchor.end_line,
        });
      }
    }
    return `${anchor.change_id}:${anchor.start_line}-${anchor.end_line}`;
  }, [anchor, app.chunkIndex, app.snapshot, commentOn, narrowed]);

  // Inside the workbench, share its account state so a sign-in made here
  // shows in its sidebar and Settings too.
  const own = useGitHub(task?.github ? undefined : backend.github);
  const github = task?.github ?? own;
  const contributors = useContributors(
    backend,
    app.overlay === "feedback",
    github.account?.login ?? null,
  );
  const localDraft = (): Publication =>
    app.snapshot
      ? localPublication(
          reviewVenue(
            task?.view.source ?? null,
            task?.view.repository_name ?? "this repository",
            app.snapshot,
          ),
          task?.view.title ?? rangeTitle(app.snapshot.source),
          app.snapshot,
        )
      : { plan: {}, requests: [], blocked: null, published: [] };
  const publication = usePublication(
    backend,
    localDraft,
    JSON.stringify(app.snapshot?.feedback ?? []),
    app.screen === "summary",
  );

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
      anchor: dialogAnchor,
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
        github={github}
        onSettings={task?.onSettings}
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
            onSelectLine={selectLine}
            onComment={openComment}
            onCommentLines={commentOnLines}
          />
        )}
        {app.screen === "summary" && (
          <SummaryScreen
            snapshot={snapshot}
            publication={publication}
            account={github.account}
            rehearsing={github.settings?.rehearse ?? false}
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
          initialTarget={commentOn?.target}
          contributors={contributors}
          onCancel={app.closeOverlay}
          onSubmit={(body, target) => void submitFeedback(body, target)}
        />
      )}
    </div>
  );
}
