export type Screen = "overview" | "chunk" | "summary";
export type Overlay = "help" | "feedback" | null;

export type Target =
  | { screen: "overview" }
  | { screen: "summary" }
  | { screen: "chunk"; index: number };

export interface PageInfo {
  id: string;
  label: string;
  target: Target;
}

/** A state worth screenshotting; `target` is the page it must be opened on. */
export interface InteractionShot {
  name: string;
  target?: Target;
}

export interface GewuState {
  screen: Screen;
  chunk_index: number | null;
  chunk_count: number;
  overlay: Overlay;
}

export interface GewuTestApi {
  state(): GewuState;
  goto(target: Target): Promise<void>;
  pages(): PageInfo[];
}

export type CheckKey =
  | "axe.critical"
  | "axe.serious"
  | "axe.moderate"
  | "axe.minor"
  | "layout.horizontal_overflow"
  | "layout.clipped_text"
  | "layout.overlap"
  | "layout.misaligned"
  | "text.too_small"
  | "text.raw_markdown"
  | "code.unhighlighted"
  | "illustration.ascii"
  | "illustration.too_small"
  | "targets.too_small"
  | "console.errors"
  | "hooks.missing"
  | "keyboard.failures"
  | "journey.blocked";

export const CHECK_KEYS: CheckKey[] = [
  "axe.critical",
  "axe.serious",
  "axe.moderate",
  "axe.minor",
  "layout.horizontal_overflow",
  "layout.clipped_text",
  "layout.overlap",
  "layout.misaligned",
  "text.too_small",
  "text.raw_markdown",
  "code.unhighlighted",
  "illustration.ascii",
  "illustration.too_small",
  "targets.too_small",
  "console.errors",
  "hooks.missing",
  "keyboard.failures",
  "journey.blocked",
];

export interface CheckFinding {
  key: CheckKey;
  page: string;
  viewport: string;
  selector: string;
  description: string;
  evidence?: string;
}

export interface ChecksFile {
  counters: Record<CheckKey, number>;
  findings: CheckFinding[];
}

export interface Manifest {
  url: string;
  label: string | null;
  captured_at: string;
  git: { head: string | null; dirty: boolean };
  viewports: string[];
  color_schemes: ("light" | "dark")[];
  pages: PageInfo[];
  rubric_version: number | null;
  interactions: string[];
}

export interface RubricScale {
  min: number;
  max: number;
  anchors: Record<string, string>;
}

export interface RubricDimension {
  id: string;
  name: string;
  weight: number;
  question: string;
  look_for: string[];
  anchors: Record<string, string>;
  evidence: string[];
  sources: string[];
}

export interface RubricGate {
  id: string;
  check: CheckKey;
  max: number;
  effect: "fail" | "cap";
  cap?: number;
  description: string;
}

export interface RubricSource {
  id: string;
  title: string;
  url: string;
}

export interface Rubric {
  version: number;
  title: string;
  scale: RubricScale;
  dimensions: RubricDimension[];
  gates: RubricGate[];
  sources: RubricSource[];
}

export interface JudgeEvidence {
  file: string;
  note: string;
}

export interface JudgeDimension {
  id: string;
  score: number;
  rationale: string;
  evidence: JudgeEvidence[];
}

export interface JudgeFinding {
  severity: "blocker" | "major" | "minor";
  dimension: string;
  page: string;
  description: string;
  suggestion: string;
}

export interface JudgeOutput {
  rubric_version: number;
  judge: string;
  dimensions: JudgeDimension[];
  findings: JudgeFinding[];
}

export interface ScorecardDimension {
  id: string;
  mean: number;
  scores: number[];
  disagreement: boolean;
  weight: number;
}

export interface ScorecardGateResult {
  id: string;
  check: CheckKey;
  count: number;
  max: number;
  effect: "fail" | "cap";
  passed: boolean;
  description: string;
}

export interface Scorecard {
  rubric_version: number;
  judges: string[];
  total: number;
  capped_total: number | null;
  gates_failed: string[];
  dimensions: ScorecardDimension[];
  gates: ScorecardGateResult[];
  validation: { judge: string; issues: string[] }[];
  top_findings: JudgeFinding[];
}

declare global {
  interface Window {
    __gewu?: GewuTestApi;
  }
}
