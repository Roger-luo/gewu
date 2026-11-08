import { useState } from "react";

import type { Decision, Snapshot, ReviewTarget } from "../protocol";
import { FeedbackJumpList } from "./FeedbackJumpList";
import { InlineMarkdown } from "./Markdown";

/** How many unreviewed chunks show before the list folds. */
const UNREVIEWED_PREVIEW = 5;

export function SummaryScreen({
  snapshot,
  onNavigate,
  onDecide,
}: {
  snapshot: Snapshot;
  onNavigate: (target: ReviewTarget) => void;
  onDecide: (decision: Decision) => void;
}) {
  const { summary, chunks, feedback, generation } = snapshot;
  const [showAll, setShowAll] = useState(false);
  const unresolved = feedback.filter((item) =>
    summary.unresolved_issues.includes(item.id),
  );
  const comments = feedback.filter((item) => item.kind === "note");
  const unreviewed = chunks.filter((chunk) =>
    summary.unreviewed.includes(chunk.id),
  );
  const shown = showAll ? unreviewed : unreviewed.slice(0, UNREVIEWED_PREVIEW);
  const next = chunks.findIndex((chunk) => summary.unreviewed.includes(chunk.id));
  // While chunks remain, reviewing them is the next step, so only a recorded decision stands out.
  const suggested = feedback.length > 0 && next === -1 ? summary.suggested_decision : null;
  const highlighted = summary.decision ?? suggested;
  const remaining = generation.remaining_work_items;

  return (
    <main data-gewu-screen="summary" aria-label="Summary">
      <div className="screen-header">
        <div>
          <div className="eyebrow stage">Stage · Summary</div>
          <h2 className="screen-title" tabIndex={-1}>
            Ready to decide?
          </h2>
        </div>
        <span className="chip">
          {summary.reviewed} / {summary.total} reviewed
        </span>
      </div>

      <section className="panel decision-box">
        <h3>Decision</h3>
        {next !== -1 && (
          <div className="warning unreviewed-warning">
            <p role="status">
              {unreviewed.length === 1
                ? "1 chunk is still unreviewed."
                : `${unreviewed.length} chunks are still unreviewed.`}{" "}
              You can decide anyway, but consider finishing the guided path first.
            </p>
            <button
              type="button"
              className="btn primary"
              onClick={() => onNavigate({ screen: "chunk", index: next })}
            >
              Review next unreviewed step
            </button>
          </div>
        )}
        {summary.batch_scoped && (
          <p className="muted">
            This decision is batch-scoped while generation remains incomplete.
          </p>
        )}
        <p className="suggestion" role="status">
          {feedback.length === 0 ? (
            "You have left no feedback on this review."
          ) : (
            <>
              Suggested from your feedback: <strong>{label(summary.suggested_decision)}</strong>
            </>
          )}
          {summary.decision ? ` · recorded: ${label(summary.decision)}` : ""}
        </p>
        <div className="actions-row">
          {(
            [
              ["approve", "Approve"],
              ["comment", "Comment"],
              ["request_changes", "Request changes"],
            ] as const
          ).map(([value, text]) => (
            <button
              key={value}
              type="button"
              className={highlighted === value ? "btn primary" : "btn"}
              aria-pressed={summary.decision === value}
              onClick={() => onDecide(value)}
            >
              {text}
            </button>
          ))}
        </div>
      </section>

      <div className="summary-grid">
        <section className="panel">
          <h3>Unresolved requests</h3>
          <FeedbackJumpList items={unresolved} chunks={chunks} empty="None open." onNavigate={onNavigate} />
        </section>

        <section className="panel">
          <h3>Comments</h3>
          <FeedbackJumpList items={comments} chunks={chunks} empty="No comments." onNavigate={onNavigate} />
        </section>

        <section className="panel">
          <h3>Unreviewed chunks</h3>
          {unreviewed.length === 0 ? (
            <p className="muted">All generated chunks are reviewed.</p>
          ) : (
            <>
              <ul className="jump-list">
                {shown.map((chunk) => {
                  const index = chunks.findIndex((item) => item.id === chunk.id);
                  return (
                    <li key={chunk.id}>
                      <button
                        type="button"
                        onClick={() =>
                          index >= 0 &&
                          onNavigate({ screen: "chunk", index })
                        }
                      >
                        <InlineMarkdown source={chunk.title} />
                      </button>
                    </li>
                  );
                })}
              </ul>
              {unreviewed.length > UNREVIEWED_PREVIEW && (
                <button
                  type="button"
                  className="btn ghost"
                  aria-expanded={showAll}
                  onClick={() => setShowAll(!showAll)}
                >
                  {showAll ? "Show fewer" : `Show all ${unreviewed.length}`}
                </button>
              )}
            </>
          )}
        </section>

        <section className="panel">
          <h3>Generation</h3>
          {generation.complete ? (
            <p className="muted">Generation complete.</p>
          ) : (
            <p className="muted">
              Pending generation
              {remaining != null && (
                <> · {remaining === 1 ? "1 work item left" : `${remaining} work items left`}</>
              )}
              . Decisions are batch-scoped.
            </p>
          )}
        </section>
      </div>
    </main>
  );
}

function label(decision: Decision): string {
  switch (decision) {
    case "approve":
      return "approve";
    case "comment":
      return "comment";
    case "request_changes":
      return "request changes";
  }
}
