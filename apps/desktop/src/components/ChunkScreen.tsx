import { useEffect, useState, type ReactNode } from "react";

import { InlineMarkdown, Markdown } from "./Markdown";
import type { CommentRequest } from "../anchors";
import { plainInline } from "../markdown";
import { assignNotes } from "../quoteNotes";
import { sectionPosition } from "../navigation";
import type { Action, Snapshot } from "../protocol";
import { ChangesBlock } from "./ChangesBlock";
import { ExplainerView } from "./explainers/ExplainerView";
import { ContextPane } from "./ContextPane";
import { FeedbackPanel } from "./FeedbackPanel";

/** Chunk kinds in the reader's words; "other" says nothing, so it is left out. */
const KIND_LABELS: Record<string, string | undefined> = {
  concept: "New idea",
  refactor: "Refactor",
  integration: "Wiring",
  test: "Tests",
  other: undefined,
};

export function ChunkScreen({
  snapshot,
  chunkIndex,
  onDispatch,
  onToggleReviewed,
  onAddComment,
  onRequestChange,
  lineSelection,
  onToggleLine,
  onComment,
}: {
  snapshot: Snapshot;
  chunkIndex: number;
  onDispatch: (action: Action) => Promise<unknown>;
  onToggleReviewed: () => Promise<void>;
  onAddComment: () => void;
  onRequestChange: () => void;
  lineSelection: { changeId: string; lines: Set<number> } | null;
  onToggleLine: (changeId: string, line: number) => void;
  onComment: (request: CommentRequest) => void;
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
  const language =
    chunk.blocks
      .flatMap((block) => (block.type === "changes" ? block.quotes : []))
      .find((quote) => quote.language)?.language ?? null;

  return (
    <div className={`app-body${showAside ? " has-context" : ""}`}>
      <main
        data-gewu-screen="chunk"
        data-gewu-chunk-id={chunk.id}
        aria-label={plainInline(chunk.title)}
      >
        <div className="screen-header">
          <div>
            <div className="eyebrow stage">
              <InlineMarkdown
                source={[
                  `Step ${chunkIndex + 1} of ${snapshot.chunks.length}`,
                  section,
                  KIND_LABELS[chunk.kind],
                ]
                  .filter(Boolean)
                  .join(" · ")}
              />
            </div>
            <h2 className="screen-title" tabIndex={-1}>
              <InlineMarkdown source={chunk.title} />
            </h2>
          </div>
        </div>

        <div className="chunk-toolbar">
          <ChunkStatusChip
            reviewed={chunk.status.reviewed}
            openIssues={chunk.status.open_issues}
          />
          <div className="toolbar-actions">
            <div className="toolbar-group" role="group" aria-label="Leave feedback">
              <button type="button" className="btn" onClick={onAddComment}>
                Comment
              </button>
              <button type="button" className="btn" onClick={onRequestChange}>
                Request changes
              </button>
            </div>
            <div className="toolbar-group" role="group" aria-label="Mark this step">
              <button
                type="button"
                className={chunk.status.reviewed ? "btn" : "btn primary"}
                aria-pressed={chunk.status.reviewed}
                title="Marks this step read and opens the next unreviewed one"
                onClick={() => void onToggleReviewed()}
              >
                {chunk.status.reviewed ? "Reviewed" : "Mark reviewed"}
              </button>
            </div>
          </div>
        </div>

        <p className="chunk-summary">
          <InlineMarkdown source={chunk.summary} />
        </p>

        {chunk.context && narrow && (
          <details className="context-disclosure">
            <summary>
              {chunk.context.kind === "concept"
                ? "Concept: definition and example"
                : "Refactor: reason and signatures"}
            </summary>
            <ContextPane context={chunk.context} language={language} />
          </details>
        )}

        {chunk.confidence && (
          <div className="notice uncertainty">
            Uncertainty: <InlineMarkdown source={chunk.confidence} />
          </div>
        )}
        {chunk.oversized_reason && (
          <div className="notice oversized">
            Oversized: <InlineMarkdown source={chunk.oversized_reason} />
          </div>
        )}

        <ChunkBody
          blocks={chunk.blocks}
          lineSelection={lineSelection}
          onToggleLine={onToggleLine}
          onComment={onComment}
        />

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
      {showAside && chunk.context && <ContextPane context={chunk.context} language={language} />}
    </div>
  );
}

function ChunkBody({
  blocks,
  lineSelection,
  onToggleLine,
  onComment,
}: {
  blocks: Snapshot["chunks"][number]["blocks"];
  lineSelection: { changeId: string; lines: Set<number> } | null;
  onToggleLine: (changeId: string, line: number) => void;
  onComment: (request: CommentRequest) => void;
}) {
  const rendered: ReactNode[] = [];
  let pending = "";
  const flushLead = (key: string) => {
    const markdown = pending;
    pending = "";
    if (!markdown) return;
    rendered.push(
      <div className="block" key={key}>
        <Markdown source={markdown} />
      </div>,
    );
  };
  blocks.forEach((block, index) => {
    if (block.type === "narrative") {
      pending = pending ? `${pending}\n\n${block.markdown}` : block.markdown;
      return;
    }
    if (block.type === "explainer") {
      flushLead(`lead-${index}`);
      rendered.push(
        <div className="block" key={`explainer-${index}`}>
          <ExplainerView explainer={block} />
        </div>,
      );
      return;
    }
    const assigned = assignNotes(pending, block.quotes);
    pending = assigned.lead;
    flushLead(`lead-${index}`);
    rendered.push(
      <ChangesBlock
        key={`changes-${index}`}
        block={block}
        notes={assigned.notes}
        selected={lineSelection}
        onToggleLine={onToggleLine}
        onComment={onComment}
      />,
    );
  });
  flushLead("lead-end");
  return <>{rendered}</>;
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
      <span className="step-status warn">
        <span className="nav-state unresolved" aria-hidden />
        {openIssues === 1
          ? "Open change request"
          : `${openIssues} open change requests`}
      </span>
    );
  }
  if (reviewed) {
    return null;
  }
  return (
    <span className="step-status">
      <span className="nav-state unreviewed" aria-hidden />
      Not reviewed
    </span>
  );
}
