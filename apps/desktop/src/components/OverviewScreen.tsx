import type { Snapshot } from "../protocol";
import { IllustrationView } from "./IllustrationView";
import { InlineMarkdown } from "./Markdown";

export function OverviewScreen({
  snapshot,
  onStart,
}: {
  snapshot: Snapshot;
  onStart: () => void;
}) {
  const { overview, generation, source, files, summary } = snapshot;
  const resuming = summary.reviewed > 0 && summary.unreviewed.length > 0;

  return (
    <main data-gewu-screen="overview" aria-label="Overview">
      <div className="screen-header">
        <div>
          <div className="eyebrow stage">Stage · Overview</div>
          <h2 className="screen-title" tabIndex={-1}>
            What this change does
          </h2>
        </div>
        <button type="button" className="btn primary" onClick={onStart}>
          {resuming ? "Continue guided review" : "Start guided review"}
        </button>
      </div>

      <p className="intent">
        <InlineMarkdown source={overview.intent} />
      </p>

      {!generation.complete && (
        <div className="notice uncertainty" role="status">
          <strong>Incomplete batch</strong>
          {generation.remaining_work_items != null && (
            <>
              {" · "}
              {generation.remaining_work_items === 1
                ? "1 work item still generating"
                : `${generation.remaining_work_items} work items still generating`}
            </>
          )}
          . A decision now covers only the generated chunks.
        </div>
      )}

      <div className="stack">
        {overview.illustration && (
          <section className="panel">
            <h3>Illustration</h3>
            <IllustrationView illustration={overview.illustration} />
          </section>
        )}

        {overview.changes.length > 0 && (
          <section className="panel">
            <h3>Changes</h3>
            <ul className="changes-list">
              {overview.changes.map((change) => (
                <li key={change}>
                  <InlineMarkdown source={change} />
                </li>
              ))}
            </ul>
          </section>
        )}

        <section className="panel">
          <h3>Scope</h3>
          <p className="muted">
            <code>{source.base}</code> → <code>{source.head}</code>
            {" · "}
            {files.length} {files.length === 1 ? "file" : "files"}
          </p>
          {files.length > 0 && (
            <ul className="scope-list">
              {files.map((file) => (
                <li key={file.path}>
                  <code>{file.path}</code>
                  <span className="line-counts">
                    <span
                      className="count added"
                      aria-label={`${file.added} lines added`}
                    >
                      +{file.added}
                    </span>
                    <span
                      className="count removed"
                      aria-label={`${file.removed} lines removed`}
                    >
                      −{file.removed}
                    </span>
                  </span>
                  {!file.shown && <span className="muted"> · not in a step yet</span>}
                </li>
              ))}
            </ul>
          )}
        </section>


        {overview.concepts && overview.concepts.length > 0 && (
          <section className="panel">
            <h3>Concepts</h3>
            <ul className="concept-list">
              {overview.concepts.map((concept) => (
                <li key={concept}>
                  <InlineMarkdown source={concept} />
                </li>
              ))}
            </ul>
          </section>
        )}

        {generation.complete && (
          <p className="muted">All planned walkthrough items are generated.</p>
        )}
      </div>
    </main>
  );
}
