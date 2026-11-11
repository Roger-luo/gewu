import { useEffect, useRef, type ReactNode } from "react";

import { plainInline } from "../markdown";
import { chunkNavState } from "../navigation";
import type { Snapshot, ReviewTarget } from "../protocol";
import type { GitHubState } from "../hooks/useGitHub";
import { GitHubStatus } from "./GitHubStatus";
import { InlineMarkdown } from "./Markdown";

export function Navigator({
  snapshot,
  screen,
  chunkIndex,
  onNavigate,
  taskTitle,
  github,
  onSettings,
}: {
  snapshot: Snapshot;
  screen: "overview" | "chunk" | "summary";
  chunkIndex: number | null;
  onNavigate: (target: ReviewTarget) => void;
  /** The workbench task this review belongs to. */
  taskTitle?: string;
  github: GitHubState;
  onSettings?: () => void;
}) {
  const { reviewed, total } = snapshot.summary;
  const nav = useRef<HTMLElement>(null);

  useEffect(() => {
    nav.current
      ?.querySelector('[aria-current="page"]')
      ?.scrollIntoView?.({ block: "nearest" });
  }, [screen, chunkIndex]);

  return (
    <nav className="navigator" aria-label="Review navigator" ref={nav}>
      <div className="nav-scroll">
      {taskTitle ? (
        <>
          <p className="eyebrow">Gewu review</p>
          <h1 className="task-title" title={taskTitle}>
            {taskTitle}
          </h1>
        </>
      ) : (
        <h1>Gewu</h1>
      )}
      <p className="progress">
        {reviewed} of {total} reviewed
        {!snapshot.generation.complete &&
          snapshot.generation.remaining_work_items != null && (
            <>
              {" "}
              · {snapshot.generation.remaining_work_items} still generating
            </>
          )}
      </p>
      <ul className="nav-list">
        <li>
          <button
            type="button"
            aria-current={screen === "overview" ? "page" : undefined}
            onClick={() => onNavigate({ screen: "overview" })}
          >
            Overview
          </button>
        </li>
        {renderChunks(snapshot, screen, chunkIndex, onNavigate)}
        <li>
          <button
            type="button"
            aria-current={screen === "summary" ? "page" : undefined}
            onClick={() => onNavigate({ screen: "summary" })}
          >
            Summary
          </button>
        </li>
      </ul>
      <ul className="nav-legend" aria-label="Status legend">
        {LEGEND_ORDER.map((state) => (
          <li key={state}>
            <span className={`nav-state ${state}`} aria-hidden />
            {STATE_LABELS[state]}
          </li>
        ))}
      </ul>
      </div>
      <GitHubStatus github={github} onSettings={onSettings} />
    </nav>
  );
}

type NavState = ReturnType<typeof chunkNavState>;

const STATE_LABELS: Record<NavState, string> = {
  reviewed: "Reviewed",
  unresolved: "Open change request",
  unreviewed: "Not reviewed",
};

const LEGEND_ORDER: NavState[] = ["unreviewed", "reviewed", "unresolved"];

function renderChunks(
  snapshot: Snapshot,
  screen: "overview" | "chunk" | "summary",
  chunkIndex: number | null,
  onNavigate: (target: ReviewTarget) => void,
) {
  const items: ReactNode[] = [];
  let lastSection: string | null | undefined = undefined;

  snapshot.chunks.forEach((chunk, index) => {
    if (chunk.section !== lastSection) {
      lastSection = chunk.section;
      if (chunk.section) {
        items.push(
          <li key={`section-${chunk.section}-${index}`} className="section-label">
            <InlineMarkdown source={chunk.section} />
          </li>,
        );
      }
    }
    const state = chunkNavState(chunk);
    items.push(
      <li key={chunk.id}>
        <button
          type="button"
          aria-label={`${plainInline(chunk.title)}, ${STATE_LABELS[state]}`}
          aria-current={screen === "chunk" && chunkIndex === index ? "page" : undefined}
          onClick={() => onNavigate({ screen: "chunk", index })}
        >
          <span className={`nav-state ${state}`} aria-hidden />
          <span aria-hidden>
            <InlineMarkdown source={chunk.title} />
          </span>
        </button>
      </li>,
    );
  });

  return items;
}
