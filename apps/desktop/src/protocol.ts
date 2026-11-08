/** App protocol types. Field names are snake_case to match the JSON contract. */

export type ChunkKind =
  | "concept"
  | "refactor"
  | "integration"
  | "test"
  | "other";

export type ChangePresentation =
  | "diff"
  | "added_code"
  | "rendered_markdown"
  | "summary_only";

export type LineKind = "added" | "removed" | "context";

export type Decision = "approve" | "comment" | "request_changes";

export type FeedbackKind = "note" | "issue";

export type FeedbackTarget = "code" | "explanation";

export type Illustration =
  | { kind: "before_after"; before: string; after: string }
  | { kind: "flow"; steps: string[] }
  | { kind: "example"; input: string; output: string }
  | { kind: "diagram"; text: string };

export type ChunkContext =
  | {
      kind: "concept";
      definition: string;
      example: string;
      relation: string;
    }
  | {
      kind: "refactor";
      before: string;
      after: string;
      reason: string;
    };

export interface Overview {
  intent: string;
  changes: string[];
  illustration?: Illustration | null;
  concepts?: string[];
}

export interface QuoteLine {
  kind: LineKind;
  old_line: number | null;
  new_line: number | null;
  text: string;
}

export interface QuoteView {
  change_id: string;
  path: string;
  start: number;
  end: number;
  language: string | null;
  more_above: boolean;
  more_below: boolean;
  lines: QuoteLine[];
}

export type BlockView =
  | { type: "narrative"; markdown: string }
  | {
      type: "changes";
      presentation: ChangePresentation;
      quotes: QuoteView[];
    };

export interface ChunkStatus {
  visited: boolean;
  reviewed: boolean;
  open_issues: number;
  feedback_count: number;
}

export interface ChunkView {
  id: string;
  index: number;
  title: string;
  summary: string;
  kind: ChunkKind;
  section: string | null;
  confidence: string | null;
  oversized_reason: string | null;
  context: ChunkContext | null;
  blocks: BlockView[];
  status: ChunkStatus;
}

export interface LineAnchor {
  change_id: string;
  path: string;
  side: string;
  start_line: number;
  end_line: number;
  patch_sha256: string;
}

export interface Feedback {
  id: string;
  chunk_id: string;
  kind: FeedbackKind;
  target: FeedbackTarget;
  body: string;
  anchor: LineAnchor | null;
  resolved: boolean;
}

export interface SummaryView {
  reviewed: number;
  total: number;
  unreviewed: string[];
  unresolved_issues: string[];
  decision: Decision | null;
  suggested_decision: Decision;
  batch_scoped: boolean;
}

/** One file of the review's whole scope; `shown` once a generated step quotes it. */
export interface FileView {
  path: string;
  added: number;
  removed: number;
  shown: boolean;
}

export interface Snapshot {
  review_id: string;
  source: { base: string; head: string };
  overview: Overview;
  files: FileView[];
  generation: {
    remaining_work_items: number | null;
    complete: boolean;
  };
  chunks: ChunkView[];
  feedback: Feedback[];
  summary: SummaryView;
  last_visited: string | null;
}

export type FeedbackAnchorInput = {
  change_id: string;
  start_line: number;
  end_line: number;
} | null;

export type Action =
  | { type: "visit"; chunk_id: string }
  | { type: "set_reviewed"; chunk_id: string; reviewed: boolean }
  | {
      type: "add_feedback";
      chunk_id: string;
      kind: FeedbackKind;
      target: FeedbackTarget;
      body: string;
      anchor: FeedbackAnchorInput;
    }
  | { type: "resolve_feedback"; feedback_id: string; resolved: boolean }
  | { type: "delete_feedback"; feedback_id: string }
  | { type: "retarget_feedback"; feedback_id: string; target: FeedbackTarget }
  | { type: "set_decision"; decision: Decision };

export type Screen = "overview" | "chunk" | "summary";

export type Overlay = "help" | "feedback" | null;

/** A page of one review. */
export type ReviewTarget =
  | { screen: "overview" }
  | { screen: "summary" }
  | { screen: "chunk"; index: number };

/** A workbench page, served without `--root`. */
export type WorkbenchTarget =
  | { screen: "home" }
  | { screen: "new-review" }
  | { screen: "task"; task_id: string }
  | { screen: "review"; task_id: string };

export type Target = ReviewTarget | WorkbenchTarget;

export interface GewuTestState {
  screen: Screen | "home" | "new-review" | "task";
  /** The task open on the task or review screens, in the workbench. */
  task_id: string | null;
  chunk_index: number | null;
  chunk_count: number;
  overlay: Overlay;
}

export interface GewuTestPage<T extends Target = Target> {
  id: string;
  label: string;
  target: T;
}

/** Where a reader was in one review, kept while they visit another. */
export interface ReviewPlace {
  screen: Screen;
  chunkIndex: number | null;
}

export interface GewuTestApi {
  state(): GewuTestState;
  goto(target: Target): Promise<void>;
  pages(): GewuTestPage[];
}

declare global {
  interface Window {
    __gewu?: GewuTestApi;
  }
}
