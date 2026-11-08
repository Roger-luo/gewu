import { useState } from "react";

import type { Action, Feedback } from "../protocol";
import { DeleteConfirmDialog } from "./DeleteConfirmDialog";

export function FeedbackPanel({
  items,
  onDispatch,
  openIssues = 0,
}: {
  items: Feedback[];
  onDispatch: (action: Action) => Promise<unknown>;
  openIssues?: number;
}) {
  const [deleting, setDeleting] = useState<Feedback | null>(null);

  return (
    <section className="feedback-panel" aria-labelledby="feedback-heading">
      <h3 id="feedback-heading">Feedback</h3>
      {openIssues > 0 && (
        <p className="muted feedback-note" role="status">
          {openIssues === 1
            ? "1 open change request still needs resolve or reopen."
            : `${openIssues} open change requests still need resolve or reopen.`}
        </p>
      )}

      {items.length === 0 ? (
        <p className="muted">No feedback on this chunk yet.</p>
      ) : (
        <ul className="feedback-list">
          {items.map((item) => (
            <FeedbackItem
              key={item.id}
              item={item}
              onDispatch={onDispatch}
              onDelete={() => setDeleting(item)}
            />
          ))}
        </ul>
      )}

      {deleting && (
        <DeleteConfirmDialog
          onCancel={() => setDeleting(null)}
          onConfirm={() => {
            const id = deleting.id;
            setDeleting(null);
            void onDispatch({ type: "delete_feedback", feedback_id: id });
          }}
        />
      )}
    </section>
  );
}

function FeedbackItem({
  item,
  onDispatch,
  onDelete,
}: {
  item: Feedback;
  onDispatch: (action: Action) => Promise<unknown>;
  onDelete: () => void;
}) {
  const kind = item.kind === "note" ? "Comment" : "Change request";
  const otherTarget = item.target === "code" ? "explanation" : "code";
  const subject = item.body.length > 40 ? `${item.body.slice(0, 40)}…` : item.body;

  return (
    <li className={`feedback-item${item.resolved ? " resolved" : ""}`}>
      <div className="feedback-meta">
        <span className={`kind ${item.kind}`}>{kind}</span>
        <span className="muted">
          on {item.target}
          {item.anchor
            ? ` · ${item.anchor.path}:${item.anchor.start_line}`
            : " · whole chunk"}
        </span>
        {item.resolved && <span className="chip">Resolved</span>}
      </div>
      <p className="feedback-body">{item.body}</p>
      <div className="actions-row">
        {item.kind === "issue" && (
          <button
            type="button"
            className="btn small"
            aria-label={`${item.resolved ? "Reopen" : "Resolve"}: ${subject}`}
            onClick={() =>
              void onDispatch({
                type: "resolve_feedback",
                feedback_id: item.id,
                resolved: !item.resolved,
              })
            }
          >
            {item.resolved ? "Reopen" : "Resolve"}
          </button>
        )}
        <button
          type="button"
          className="btn small"
          aria-label={`Move to ${otherTarget}: ${subject}`}
          onClick={() =>
            void onDispatch({
              type: "retarget_feedback",
              feedback_id: item.id,
              target: otherTarget,
            })
          }
        >
          Move to {otherTarget}
        </button>
        <button
          type="button"
          className="btn small danger"
          aria-label={`Delete: ${subject}`}
          onClick={onDelete}
        >
          Delete…
        </button>
      </div>
    </li>
  );
}
