import { useCallback, useMemo, useState } from "react";

import {
  anchorInput,
  anchorLabel,
  selectLine as nextSelection,
  spanSelection,
  type CommentRequest,
  type LineSelection,
} from "../anchors";
import type { ComposerPlace } from "../composer";
import type { FeedbackAnchorInput } from "../protocol";
import type { ReviewController } from "./useReviewApp";

/**
 * The lines a comment will land on and where its box opens: the gutter
 * selection, the block or note it was opened from, and the two combined.
 */
export function useCommentAnchor(app: ReviewController) {
  const [lineSelection, setLineSelection] = useState<LineSelection | null>(null);
  const clearSelection = useCallback(() => setLineSelection(null), []);

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

  // An explanation comment opens under its note; a code comment under the
  // last line it covers; anything else at the end of the chunk.
  const placeNow: ComposerPlace =
    commentOn?.target === "explanation"
      ? {
          at: "note",
          changeId: commentOn.span.change_id,
          side: commentOn.span.side,
          line: commentOn.span.start_line,
        }
      : dialogAnchor
        ? {
            at: "line",
            changeId: dialogAnchor.change_id,
            side: dialogAnchor.side === "old" ? "old" : "new",
            line: dialogAnchor.end_line,
          }
        : { at: "chunk" };
  // The box stays where it opened: picking more lines while typing updates
  // the anchor it will save with, but does not move it and lose the text.
  const composing = app.overlay === "feedback";
  const [openedAt, setOpenedAt] = useState<ComposerPlace | null>(null);
  if (composing && !openedAt) {
    setOpenedAt(placeNow);
  } else if (!composing && openedAt) {
    setOpenedAt(null);
  }
  const composerPlace = openedAt ?? placeNow;

  return {
    lineSelection,
    clearSelection,
    selectLine,
    openComment,
    commentOnLines,
    commentOn,
    dialogAnchor,
    anchorSummary,
    composerPlace,
  };
}
