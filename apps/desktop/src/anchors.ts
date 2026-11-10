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
