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
  return (
    <div className="comment-actions" role="group" aria-label={`Feedback on this ${what}`}>
      <button
        type="button"
        className="btn small"
        onClick={() => onComment({ kind: "note", target, span })}
      >
        Comment on {what}
      </button>
      <button
        type="button"
        className="btn small"
        onClick={() => onComment({ kind: "issue", target, span })}
      >
        Request change on {what}
      </button>
    </div>
  );
}
