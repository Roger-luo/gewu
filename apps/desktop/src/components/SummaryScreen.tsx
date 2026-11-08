import type { Decision, Snapshot, ReviewTarget } from "../protocol";
import { InlineMarkdown } from "./Markdown";

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
  const unresolved = feedback.filter((item) =>
    summary.unresolved_issues.includes(item.id),
  );
  const unreviewed = chunks.filter((chunk) =>
    summary.unreviewed.includes(chunk.id),
  );

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

      <div className="summary-grid">
        <section className="panel">
          <h3>Unresolved requests</h3>
          {unresolved.length === 0 ? (
            <p className="muted">None open.</p>
          ) : (
            <ul className="jump-list">
              {unresolved.map((item) => {
                const index = chunks.findIndex(
                  (chunk) => chunk.id === item.chunk_id,
                );
                return (
                  <li key={item.id}>
                    <button
                      type="button"
                      onClick={() =>
                        index >= 0 &&
                        onNavigate({ screen: "chunk", index })
                      }
                    >
                      {item.body}
                    </button>
                  </li>
                );
              })}
            </ul>
          )}
        </section>

        <section className="panel">
          <h3>Unreviewed chunks</h3>
          {unreviewed.length === 0 ? (
            <p className="muted">All generated chunks are reviewed.</p>
          ) : (
            <ul className="jump-list">
              {unreviewed.map((chunk) => {
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
          )}
        </section>

        <section className="panel">
          <h3>Generation</h3>
          {generation.complete ? (
            <p className="muted">Generation complete.</p>
          ) : (
            <p className="muted">
              Pending generation
              {generation.remaining_work_items != null && (
                <> · {generation.remaining_work_items} work items left</>
              )}
              . Decisions are batch-scoped.
            </p>
          )}
        </section>
      </div>

      <section className="panel decision-box">
        <h3>Decision</h3>
        {unreviewed.length > 0 && (
          <div className="warning" role="status">
            Some chunks are still unreviewed. You can decide anyway, but consider
            finishing the guided path first.
          </div>
        )}
        {summary.batch_scoped && (
          <p className="muted">
            This decision is batch-scoped while generation remains incomplete.
          </p>
        )}
        <p className="suggestion" role="status">
          Suggested from feedback:{" "}
          <strong>{label(summary.suggested_decision)}</strong>
          {summary.decision
            ? ` · recorded: ${label(summary.decision)}`
            : ""}
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
              className={
                (summary.decision ?? summary.suggested_decision) === value
                  ? "btn primary"
                  : "btn"
              }
              aria-pressed={summary.decision === value}
              onClick={() => onDecide(value)}
            >
              {text}
            </button>
          ))}
        </div>
      </section>
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
