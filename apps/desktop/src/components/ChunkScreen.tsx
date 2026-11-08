import { useEffect, useState } from "react";

import { Markdown } from "./Markdown";
import { sectionPosition } from "../navigation";
import type { Action, Snapshot } from "../protocol";
import { ChangesBlock } from "./ChangesBlock";
import { ContextPane } from "./ContextPane";
import { FeedbackPanel } from "./FeedbackPanel";

export function ChunkScreen({
  snapshot,
  chunkIndex,
  onDispatch,
  onAddComment,
  onRequestChange,
  lineSelection,
  onToggleLine,
}: {
  snapshot: Snapshot;
  chunkIndex: number;
  onDispatch: (action: Action) => Promise<unknown>;
  onAddComment: () => void;
  onRequestChange: () => void;
  lineSelection: { changeId: string; lines: Set<number> } | null;
  onToggleLine: (changeId: string, line: number) => void;
}) {
  const chunk = snapshot.chunks[chunkIndex];
  const [narrow, setNarrow] = useState(() => {
    if (typeof window === "undefined" || typeof window.matchMedia !== "function") {
      return false;
    }
    return window.matchMedia("(max-width: 1100px)").matches;
  });

  useEffect(() => {
    const media = window.matchMedia("(max-width: 1100px)");
    const onChange = () => setNarrow(media.matches);
    media.addEventListener("change", onChange);
    return () => media.removeEventListener("change", onChange);
  }, []);

  if (!chunk) {
    return (
      <main data-gewu-screen="chunk" aria-label="Review step">
        <p className="muted">Chunk not found.</p>
      </main>
    );
  }

  const section = sectionPosition(snapshot, chunkIndex);
  const feedback = snapshot.feedback.filter(
    (item) => item.chunk_id === chunk.id,
  );
  const showAside = Boolean(chunk.context) && !narrow;

  return (
    <div className={`app-body${showAside ? " has-context" : ""}`}>
      <main data-gewu-screen="chunk" data-gewu-chunk-id={chunk.id} aria-label={chunk.title}>
        <div className="screen-header">
          <div>
            <div className="eyebrow stage">
              {[
                "Stage",
                `Chunk ${chunkIndex + 1} of ${snapshot.chunks.length}`,
                section,
                chunk.kind,
              ]
                .filter(Boolean)
                .join(" · ")}
            </div>
            <h2 className="screen-title" tabIndex={-1}>
              {chunk.title}
            </h2>
          </div>
          <div className="header-actions">
            <div role="group" aria-label="Mark this step">
              <button
                type="button"
                className={chunk.status.reviewed ? "btn" : "btn primary"}
                aria-pressed={chunk.status.reviewed}
                title="Reviewed is separate from comments and change requests"
                onClick={() =>
                  void onDispatch({
                    type: "set_reviewed",
                    chunk_id: chunk.id,
                    reviewed: !chunk.status.reviewed,
                  })
                }
              >
                {chunk.status.reviewed ? "Reviewed" : "Mark reviewed"}
              </button>
            </div>
            <ChunkStatusChip
              reviewed={chunk.status.reviewed}
              openIssues={chunk.status.open_issues}
            />
          </div>
        </div>

        <div className="chunk-toolbar" role="group" aria-label="Leave feedback">
          <span className="toolbar-label">Leave feedback</span>
          <button type="button" className="btn" onClick={onAddComment}>
            Comment
          </button>
          <button type="button" className="btn" onClick={onRequestChange}>
            Request changes
          </button>
        </div>

        <p className="chunk-summary">{chunk.summary}</p>

        {chunk.context && narrow && (
          <div className="context-inline">
            <ContextPane context={chunk.context} />
          </div>
        )}

        {chunk.confidence && (
          <div className="notice uncertainty">
            Uncertainty: {chunk.confidence}
          </div>
        )}
        {chunk.oversized_reason && (
          <div className="notice oversized">
            Oversized: {chunk.oversized_reason}
          </div>
        )}

        {chunk.blocks.map((block, index) =>
          block.type === "narrative" ? (
            <div className="block" key={`narrative-${index}`}>
              <Markdown source={block.markdown} />
            </div>
          ) : (
            <ChangesBlock
              key={`changes-${index}`}
              block={block}
              selected={lineSelection}
              onToggleLine={onToggleLine}
            />
          ),
        )}

        {chunk.status.open_issues > 0 && (
          <p className="notice feedback-jump" role="status">
            {chunk.status.open_issues === 1
              ? "1 open change request — resolve it in Feedback below."
              : `${chunk.status.open_issues} open change requests — resolve them in Feedback below.`}
          </p>
        )}

        <FeedbackPanel
          items={feedback}
          onDispatch={onDispatch}
          openIssues={chunk.status.open_issues}
        />
      </main>
      {showAside && chunk.context && <ContextPane context={chunk.context} />}
    </div>
  );
}

function ChunkStatusChip({
  reviewed,
  openIssues,
}: {
  reviewed: boolean;
  openIssues: number;
}) {
  // Reviewed state lives in the toolbar + navigator; only surface open work
  // (or unreviewed) here so chrome does not repeat the same status three ways.
  if (openIssues > 0) {
    return (
      <span className="chip warn">
        {openIssues === 1
          ? "Open change request"
          : `${openIssues} open change requests`}
      </span>
    );
  }
  if (reviewed) {
    return null;
  }
  return <span className="chip quiet">Unreviewed</span>;
}
