import type { Snapshot } from "../protocol";
import { showcaseSnapshot } from "./showcase";

/** Showcase with existing feedback for summary and feedback-list flows. */
export function showcaseFeedbackSnapshot(): Snapshot {
  const base = showcaseSnapshot();
  base.review_id = "demo-showcase-feedback";
  base.chunks[0].status.visited = true;
  base.chunks[0].status.reviewed = true;
  base.chunks[1].status.visited = true;
  base.chunks[1].status.reviewed = false;
  base.last_visited = "event-path";
  base.feedback = [
    {
      id: "fb-note-1",
      chunk_id: "session-key",
      kind: "note",
      target: "explanation",
      body: "The before/after illustration makes the orphaning failure mode clear.",
      anchor: null,
      resolved: false,
    },
    {
      id: "fb-issue-1",
      chunk_id: "event-path",
      kind: "issue",
      target: "code",
      body: "Confirm the Windows path join still uses the same separators as the TUI.",
      anchor: {
        change_id: "c-path",
        path: "crates/gewu-app/src/session.rs",
        side: "new",
        start_line: 41,
        end_line: 41,
        patch_sha256: "mock-patch",
      },
      resolved: false,
    },
    {
      id: "fb-issue-2",
      chunk_id: "docs-note",
      kind: "issue",
      target: "explanation",
      body: "Link to the generation section so batch-scoped is defined once.",
      anchor: null,
      resolved: true,
    },
  ];
  return base;
}
