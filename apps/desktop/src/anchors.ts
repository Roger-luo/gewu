import type {
  FeedbackAnchorInput,
  FeedbackKind,
  FeedbackTarget,
  QuoteView,
} from "./protocol";

/** The lines one quote covers on one side: the unit a comment anchors to. */
export interface AnchorSpan {
  change_id: string;
  path: string;
  side: "new" | "old";
  start_line: number;
  end_line: number;
}

/**
 * The lines a reader picked in one change: a contiguous run from `anchor`,
 * the line clicked first, to the line shift-clicked last.
 */
export interface LineSelection {
  changeId: string;
  anchor: number;
  lines: Set<number>;
}

/**
 * The selection after clicking `line`. A plain click picks that line alone,
 * or clears it when it was the whole selection; a shift-click extends the run
 * from the anchor, the way GitHub's gutter does.
 */
export function selectLine(
  current: LineSelection | null,
  changeId: string,
  line: number,
  extend: boolean,
): LineSelection | null {
  if (extend && current && current.changeId === changeId) {
    return { changeId, anchor: current.anchor, lines: run(current.anchor, line) };
  }
  if (current?.changeId === changeId && current.lines.size === 1 && current.lines.has(line)) {
    return null;
  }
  return { changeId, anchor: line, lines: new Set([line]) };
}

/** A selection covering exactly `span`. */
export function spanSelection(span: AnchorSpan): LineSelection {
  return {
    changeId: span.change_id,
    anchor: span.start_line,
    lines: run(span.start_line, span.end_line),
  };
}

/**
 * The span a comment on `line` covers: the selection when the line is in it,
 * else that line alone.
 */
export function lineSpan(
  block: AnchorSpan,
  selected: Set<number>,
  line: number,
): AnchorSpan {
  if (!selected.has(line)) {
    return { ...block, start_line: line, end_line: line };
  }
  const lines = [...selected];
  return { ...block, start_line: Math.min(...lines), end_line: Math.max(...lines) };
}

/** `span` narrowed to the selected lines, when any are inside it. */
export function narrowSpan(span: AnchorSpan, selected: Set<number>): AnchorSpan {
  const inside = [...selected].filter(
    (line) => line >= span.start_line && line <= span.end_line,
  );
  if (inside.length === 0) {
    return span;
  }
  return { ...span, start_line: Math.min(...inside), end_line: Math.max(...inside) };
}

function run(from: number, to: number): Set<number> {
  const [low, high] = from <= to ? [from, to] : [to, from];
  return new Set(Array.from({ length: high - low + 1 }, (_, index) => low + index));
}

/** A comment opened from a code block or an explanation, not from line numbers. */
export interface CommentRequest {
  kind: FeedbackKind;
  target: FeedbackTarget;
  span: AnchorSpan;
}

/**
 * The span a whole quote covers. New-side numbers are used when the quote has
 * any, so a block of pure deletions falls back to the old side. Null when the
 * quote carries no line numbers, such as a rendered document.
 */
export function quoteSpan(quote: QuoteView): AnchorSpan | null {
  const side = quote.lines.some((line) => line.new_line !== null) ? "new" : "old";
  const numbers = quote.lines.flatMap((line) => {
    const number = side === "new" ? line.new_line : line.old_line;
    return number === null ? [] : [number];
  });
  if (numbers.length === 0) {
    return null;
  }
  return {
    change_id: quote.change_id,
    path: quote.path,
    side,
    start_line: Math.min(...numbers),
    end_line: Math.max(...numbers),
  };
}

/** The wire form of a span, as `add_feedback` expects it. */
export function anchorInput(span: AnchorSpan): NonNullable<FeedbackAnchorInput> {
  return {
    change_id: span.change_id,
    side: span.side,
    start_line: span.start_line,
    end_line: span.end_line,
  };
}

/** How an anchor reads on screen, e.g. `src/lib.rs:10-12 (before)`. */
export function anchorLabel(anchor: {
  path: string;
  side: string;
  start_line: number;
  end_line: number;
}): string {
  const lines =
    anchor.start_line === anchor.end_line
      ? `${anchor.start_line}`
      : `${anchor.start_line}-${anchor.end_line}`;
  const before = anchor.side === "old" ? " (before)" : "";
  return `${anchor.path}:${lines}${before}`;
}
