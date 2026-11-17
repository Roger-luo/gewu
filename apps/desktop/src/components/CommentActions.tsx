import type { FeedbackTarget } from "../protocol";
import type { AnchorSpan, CommentRequest } from "../anchors";

/** Buttons that open a comment or change request on one block or one explanation. */
export function CommentActions({
  target,
  span,
  onComment,
}: {
  target: FeedbackTarget;
  span: AnchorSpan;
  onComment: (request: CommentRequest) => void;
}) {
  const what = target === "code" ? "code" : "explanation";
  const lines =
    span.start_line === span.end_line
      ? `line ${span.start_line}`
      : `lines ${span.start_line}–${span.end_line}`;
  // Only code comments land on GitHub lines, so only they name the lines.
  const where = target === "code" ? <span className="comment-lines"> · {lines}</span> : null;
  return (
    <div className="comment-actions" role="group" aria-label={`Feedback on this ${what}`}>
      <button
        type="button"
        className="btn small"
        onClick={() => onComment({ kind: "note", target, span })}
      >
        Comment on {what}
        {where}
      </button>
      <button
        type="button"
        className="btn small"
        onClick={() => onComment({ kind: "issue", target, span })}
      >
        Request change on {what}
        {where}
      </button>
    </div>
  );
}
